//! Small pieces of macOS glue: CoreGraphics for the mouse / idle time and
//! AppKit (through objc2) for window levels, Spaces and app activation.
//!
//! The island, full-screen and wallpaper windows are turned into non-activating
//! panels (NSPanel) – so they must only ever be hidden, never closed: only
//! panels may float over other apps' full-screen Spaces, and a non-activating
//! panel can be clicked (or typed into) without pulling you out of the app you
//! are using – like Spotlight. tao's window class adds no AppKit state of its
//! own beyond its Rust ivars and NSPanel adds no ivars to NSWindow, so swapping
//! the class keeps the object layout intact.

use std::ffi::c_void;
use std::sync::OnceLock;

use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};

extern "C" {
    fn object_setClass(obj: *mut AnyObject, cls: *const AnyClass) -> *const AnyClass;
    fn dlopen(path: *const std::ffi::c_char, mode: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const std::ffi::c_char) -> *mut c_void;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreate(source: *const c_void) -> *mut c_void;
    fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
    fn CGEventSourceButtonState(state: i32, button: u32) -> bool;
    fn CGEventSourceSecondsSinceLastEventType(state: i32, event_type: u32) -> f64;
    fn CGSessionCopyCurrentDictionary() -> *const c_void;
    fn CGWindowLevelForKey(key: i32) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
    fn CFStringCreateWithCString(alloc: *const c_void, s: *const std::ffi::c_char, encoding: u32) -> *const c_void;
    fn CFGetTypeID(cf: *const c_void) -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFBooleanGetValue(b: *const c_void) -> bool;
}

const HID_STATE: i32 = 1; // kCGEventSourceStateHIDSystemState
const ANY_INPUT: u32 = u32::MAX; // kCGAnyInputEventType

/// Above the menu bar (24) and its status items (25), below pop-up menus (101).
pub const ISLAND_LEVEL: isize = 26;
/// The full-screen view covers the Dock and the menu bar.
pub const FULL_LEVEL: isize = 27;
/// Above the lock screen (loginwindow uses 2001–2004), below the cursor.
pub const LOCK_LEVEL: isize = 2010;

const DESKTOP_LEVEL_KEY: i32 = 2; // kCGDesktopWindowLevelKey (the wallpaper)
const DESKTOP_ICON_LEVEL_KEY: i32 = 18; // kCGDesktopIconWindowLevelKey (Finder's icons)

/// Mouse position in global points, origin at the top-left of the main display
/// (the same space as logical window positions).
pub fn cursor() -> Option<(f64, f64)> {
    unsafe {
        let e = CGEventCreate(std::ptr::null());
        if e.is_null() {
            return None;
        }
        let p = CGEventGetLocation(e);
        CFRelease(e);
        Some((p.x, p.y))
    }
}

pub fn left_button_down() -> bool {
    unsafe { CGEventSourceButtonState(HID_STATE, 0) }
}

/// Seconds since the last keyboard / mouse / trackpad input.
pub fn idle_secs() -> f64 {
    unsafe { CGEventSourceSecondsSinceLastEventType(HID_STATE, ANY_INPUT) }
}

/// Seconds since a key was last pressed.
pub fn key_idle_secs() -> f64 {
    unsafe { CGEventSourceSecondsSinceLastEventType(HID_STATE, 10) } // kCGEventKeyDown
}

pub fn screen_locked() -> bool {
    unsafe {
        let d = CGSessionCopyCurrentDictionary();
        if d.is_null() {
            return false;
        }
        let key = CFStringCreateWithCString(std::ptr::null(), c"CGSSessionScreenIsLocked".as_ptr(), 0x0800_0100);
        let v = CFDictionaryGetValue(d, key);
        let locked = !v.is_null() && (CFGetTypeID(v) != CFBooleanGetTypeID() || CFBooleanGetValue(v));
        CFRelease(key);
        CFRelease(d);
        locked
    }
}

// NSWindowCollectionBehavior bits
const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
const STATIONARY: usize = 1 << 4;
const IGNORES_CYCLE: usize = 1 << 6;
const FULL_SCREEN_AUXILIARY: usize = 1 << 8;

const NONACTIVATING_PANEL: usize = 1 << 7; // NSWindowStyleMaskNonactivatingPanel

extern "C-unwind" fn yes(_this: *mut AnyObject, _sel: Sel) -> Bool {
    Bool::YES
}
extern "C-unwind" fn no(_this: *mut AnyObject, _sel: Sel) -> Bool {
    Bool::NO
}

/// NSPanel subclass; `key` = may receive the keyboard.
fn panel_class(key: bool) -> &'static AnyClass {
    static ISLAND: OnceLock<&'static AnyClass> = OnceLock::new();
    static FULL: OnceLock<&'static AnyClass> = OnceLock::new();
    let (cell, name) = if key { (&FULL, c"NXFullPanel") } else { (&ISLAND, c"NXIslandPanel") };
    cell.get_or_init(|| {
        let mut b = ClassBuilder::new(name, class!(NSPanel)).expect("panel class");
        let f: extern "C-unwind" fn(*mut AnyObject, Sel) -> Bool = if key { yes } else { no };
        unsafe {
            b.add_method(sel!(canBecomeKeyWindow), f);
            b.add_method(sel!(canBecomeMainWindow), no as extern "C-unwind" fn(*mut AnyObject, Sel) -> Bool);
        }
        b.register()
    })
}

unsafe fn make_panel(w: *mut AnyObject, key: bool) {
    object_setClass(w, panel_class(key));
    let mask: usize = msg_send![w, styleMask];
    let _: () = msg_send![w, setStyleMask: mask | NONACTIVATING_PANEL];
    let _: () = msg_send![w, setFloatingPanel: true];
    let _: () = msg_send![w, setBecomesKeyOnlyIfNeeded: !key];
    let _: () = msg_send![w, setWorksWhenModal: true];
    let _: () = msg_send![w, setHidesOnDeactivate: false];
    let prevents = sel!(_setPreventsActivation:);
    let responds: bool = msg_send![w, respondsToSelector: prevents];
    if responds {
        let _: () = msg_send![w, _setPreventsActivation: true];
    }
}

/// The island: floats over the menu bar on every Space (also next to
/// full-screen apps), stays out of Cmd-Tab / Mission Control, and clicking it
/// does not pull focus away from the app you are using. Main thread only,
/// before the window is first shown.
pub fn setup_island_window(ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    let w = ns_window as *mut AnyObject;
    unsafe {
        make_panel(w, false);
        let _: () = msg_send![w, setLevel: ISLAND_LEVEL];
        let _: () = msg_send![w, setCollectionBehavior: CAN_JOIN_ALL_SPACES | STATIONARY | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY];
        let _: () = msg_send![w, setHasShadow: false];
    }
}

/// The full-screen view: above the Dock and menu bar, on the current Space
/// (full-screen ones too), takes the keyboard without activating NeedleX.
/// Main thread only, before the window is first shown.
pub fn setup_full_window(ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    let w = ns_window as *mut AnyObject;
    unsafe {
        make_panel(w, true);
        let _: () = msg_send![w, setLevel: FULL_LEVEL];
        let _: () = msg_send![w, setCollectionBehavior: CAN_JOIN_ALL_SPACES | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY];
        let _: () = msg_send![w, setHasShadow: false];
    }
}

/// The live wallpaper: a window on the desktop itself, behind every app
/// window, on every Space, untouched by Mission Control / Show Desktop.
/// Main thread only.
pub fn setup_wall_window(ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    let w = ns_window as *mut AnyObject;
    unsafe {
        // a panel, so clicking a desktop object doesn't activate NeedleX
        make_panel(w, false);
        let _: () = msg_send![w, setCollectionBehavior: CAN_JOIN_ALL_SPACES | STATIONARY | IGNORES_CYCLE];
        let _: () = msg_send![w, setHasShadow: false];
    }
    set_wall_level(ns_window, false);
}

/// Just above the wallpaper (under the desktop icons), or – for the objects
/// you can drag around – just above the desktop icons. Main thread only.
pub fn set_wall_level(ns_window: *mut c_void, above_icons: bool) {
    if ns_window.is_null() {
        return;
    }
    let key = if above_icons { DESKTOP_ICON_LEVEL_KEY } else { DESKTOP_LEVEL_KEY };
    let level = unsafe { CGWindowLevelForKey(key) } as isize + 1;
    let w = ns_window as *mut AnyObject;
    unsafe {
        let _: () = msg_send![w, setLevel: level];
    }
}

// ---------------------------------------------------------------- Spaces
//
// The island, the full-screen view and the wallpaper are on every Space
// (canJoinAllSpaces), full-screen ones included. Over a long session macOS sometimes drops such a
// window from some Spaces anyway, and from then on it is missing there – the
// notch shows up next to some apps but not others – until NeedleX restarts:
// ordering the window in again or setting its collection behaviour again
// doesn't bring it back. SkyLight (private, looked up at run time) can.

const SKYLIGHT: &std::ffi::CStr = c"/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight";
const ALL_SPACES_MASK: i32 = 7; // kCGSAllSpacesMask

struct SpaceApi {
    cid: i32,
    /// SLSCopyManagedDisplaySpaces → CFArray of { "Spaces": [{ "ManagedSpaceID": … }] } per display
    display_spaces: unsafe extern "C" fn(i32) -> *mut AnyObject,
    /// SLSCopySpacesForWindows(cid, mask, windows) → CFArray of space ids
    window_spaces: unsafe extern "C" fn(i32, i32, *mut AnyObject) -> *mut AnyObject,
    /// SLSAddWindowsToSpaces(cid, windows, spaces)
    add: unsafe extern "C" fn(i32, *mut AnyObject, *mut AnyObject),
}

fn space_api() -> Option<&'static SpaceApi> {
    static API: OnceLock<Option<SpaceApi>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        let h = dlopen(SKYLIGHT.as_ptr(), 2);
        if h.is_null() {
            return None;
        }
        let sym = |name: &std::ffi::CStr| {
            let p = dlsym(h, name.as_ptr());
            (!p.is_null()).then_some(p)
        };
        let main_connection: unsafe extern "C" fn() -> i32 = std::mem::transmute(sym(c"SLSMainConnectionID")?);
        Some(SpaceApi {
            cid: main_connection(),
            display_spaces: std::mem::transmute(sym(c"SLSCopyManagedDisplaySpaces")?),
            window_spaces: std::mem::transmute(sym(c"SLSCopySpacesForWindows")?),
            add: std::mem::transmute(sym(c"SLSAddWindowsToSpaces")?),
        })
    })
    .as_ref()
}

/// Space ids in an NSArray of NSNumbers (a CFArray we own; released here).
unsafe fn take_ids(list: *mut AnyObject) -> Vec<u64> {
    if list.is_null() {
        return Vec::new();
    }
    let n: usize = msg_send![list, count];
    let ids = (0..n)
        .map(|i| {
            let v: *mut AnyObject = msg_send![list, objectAtIndex: i];
            let id: u64 = msg_send![v, unsignedLongLongValue];
            id
        })
        .collect();
    CFRelease(list as *const c_void);
    ids
}

unsafe fn all_spaces(api: &SpaceApi) -> Vec<u64> {
    let displays = (api.display_spaces)(api.cid);
    if displays.is_null() {
        return Vec::new();
    }
    let k_spaces: *mut AnyObject = msg_send![class!(NSString), stringWithUTF8String: c"Spaces".as_ptr()];
    let k_id: *mut AnyObject = msg_send![class!(NSString), stringWithUTF8String: c"ManagedSpaceID".as_ptr()];
    let mut ids = Vec::new();
    let nd: usize = msg_send![displays, count];
    for i in 0..nd {
        let d: *mut AnyObject = msg_send![displays, objectAtIndex: i];
        let spaces: *mut AnyObject = msg_send![d, objectForKey: k_spaces];
        if spaces.is_null() {
            continue;
        }
        let ns: usize = msg_send![spaces, count];
        for j in 0..ns {
            let s: *mut AnyObject = msg_send![spaces, objectAtIndex: j];
            let id: *mut AnyObject = msg_send![s, objectForKey: k_id];
            if !id.is_null() {
                let v: u64 = msg_send![id, unsignedLongLongValue];
                ids.push(v);
            }
        }
    }
    CFRelease(displays as *const c_void);
    ids
}

/// Puts a visible all-Spaces window (island, full-screen view, wallpaper) back
/// on every Space macOS dropped it from. Cheap when nothing is missing; runs
/// every second or so. Main thread only. Returns the Spaces it was put back on.
pub fn rejoin_all_spaces(ns_window: *mut c_void) -> Vec<u64> {
    if ns_window.is_null() {
        return Vec::new();
    }
    let w = ns_window as *mut AnyObject;
    // its own pool: the temporary arrays / strings are freed right away
    objc2::rc::autoreleasepool(|_| unsafe {
        // a hidden window is on no Space; it's put on all of them when shown
        let visible: bool = msg_send![w, isVisible];
        let Some(api) = space_api().filter(|_| visible) else { return Vec::new() };
        let num: isize = msg_send![w, windowNumber];
        if num <= 0 {
            return Vec::new();
        }
        let n: *mut AnyObject = msg_send![class!(NSNumber), numberWithInt: num as i32];
        let windows: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: n];
        let on = take_ids((api.window_spaces)(api.cid, ALL_SPACES_MASK, windows));
        if on.is_empty() {
            return Vec::new(); // not on screen (yet)
        }
        let missing: Vec<u64> = all_spaces(api).into_iter().filter(|s| !on.contains(s)).collect();
        if missing.is_empty() {
            return missing;
        }
        let list: *mut AnyObject = msg_send![class!(NSMutableArray), arrayWithCapacity: missing.len()];
        for s in &missing {
            let v: *mut AnyObject = msg_send![class!(NSNumber), numberWithUnsignedLongLong: *s];
            let _: () = msg_send![list, addObject: v];
        }
        (api.add)(api.cid, windows, list);
        missing
    })
}

// ---------------------------------------------------------------- lock screen
//
// The lock screen covers every normal window, whatever its level: it lives in
// a space of its own (SkyLight absolute level 300). A window in a space at the
// level macOS uses for Notification Center on the lock screen (400), marked as
// allowed to show without a login, is drawn on top of it – that's how
// lock-screen music players do it. SkyLight is a private framework, so it is
// looked up at run time; if anything is missing the lock screen just stays as
// macOS draws it.

/// kSLSSpaceAbsoluteLevelNotificationCenterAtScreenLock (the lock screen is 300).
const SPACE_LEVEL_ABOVE_LOCK_SCREEN: i32 = 400;

struct SkyLight {
    cid: i32,
    space: u64,
    add_windows: unsafe extern "C" fn(i32, u64, *mut AnyObject, i32) -> i32,
}

fn skylight() -> Option<&'static SkyLight> {
    static SKY: OnceLock<Option<SkyLight>> = OnceLock::new();
    SKY.get_or_init(|| unsafe {
        let h = dlopen(SKYLIGHT.as_ptr(), 2);
        if h.is_null() {
            return None;
        }
        let sym = |name: &std::ffi::CStr| {
            let p = dlsym(h, name.as_ptr());
            (!p.is_null()).then_some(p)
        };
        let main_connection: unsafe extern "C" fn() -> i32 = std::mem::transmute(sym(c"SLSMainConnectionID")?);
        let space_create: unsafe extern "C" fn(i32, i32, i32) -> u64 = std::mem::transmute(sym(c"SLSSpaceCreate")?);
        let set_level: unsafe extern "C" fn(i32, u64, i32) -> i32 = std::mem::transmute(sym(c"SLSSpaceSetAbsoluteLevel")?);
        let show_spaces: unsafe extern "C" fn(i32, *mut AnyObject) -> i32 = std::mem::transmute(sym(c"SLSShowSpaces")?);
        let add_windows: unsafe extern "C" fn(i32, u64, *mut AnyObject, i32) -> i32 =
            std::mem::transmute(sym(c"SLSSpaceAddWindowsAndRemoveFromSpaces")?);
        let cid = main_connection();
        let space = space_create(cid, 1, 0);
        if space == 0 {
            return None;
        }
        set_level(cid, space, SPACE_LEVEL_ABOVE_LOCK_SCREEN);
        let n: *mut AnyObject = msg_send![class!(NSNumber), numberWithUnsignedLongLong: space];
        let list: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: n];
        show_spaces(cid, list);
        Some(SkyLight { cid, space, add_windows })
    })
    .as_ref()
}

/// Puts the lock-screen window over the lock screen, moved into the SkyLight
/// space. It can't become key, so the password field keeps the keyboard.
/// Main thread only. Returns false if that isn't possible.
///
/// Unlike the other windows it stays a plain window (no panel class swap):
/// it is closed on every unlock, and AppKit can't tear down a window whose
/// class was swapped after its observers were registered.
pub fn setup_lock_window(ns_window: *mut c_void) -> bool {
    if ns_window.is_null() {
        return false;
    }
    let w = ns_window as *mut AnyObject;
    unsafe {
        // above loginwindow's own windows (levels 2001–2004)
        let _: () = msg_send![w, setLevel: LOCK_LEVEL];
        let _: () = msg_send![w, setHasShadow: false];
        let _: () = msg_send![w, setIgnoresMouseEvents: true];
        // allowed to be on screen while the Mac is locked
        let _: () = msg_send![w, setCanBecomeVisibleWithoutLogin: true];
        // clicks on the record / cover / player must not activate NeedleX
        // (the password field has to keep the keyboard)
        let prevents = sel!(_setPreventsActivation:);
        let responds: bool = msg_send![w, respondsToSelector: prevents];
        if responds {
            let _: () = msg_send![w, _setPreventsActivation: true];
        }
        let Some(sky) = skylight() else { return false };
        // into the space above the lock screen first, then on screen
        let add = |w: *mut AnyObject| -> Option<(isize, i32)> {
            let num: isize = msg_send![w, windowNumber];
            if num <= 0 {
                return None;
            }
            let n: *mut AnyObject = msg_send![class!(NSNumber), numberWithInt: num as i32];
            let list: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: n];
            Some((num, (sky.add_windows)(sky.cid, sky.space, list, 7)))
        };
        add(w);
        let _: () = msg_send![w, orderFrontRegardless];
        let Some((num, r)) = add(w) else { return false };
        if crate::debug() {
            eprintln!("[lock] SkyLight space {} (connection {}): added window {num} -> {r}", sky.space, sky.cid);
        }
        true
    }
}

/// Makes the window key and gives the keyboard to its web view. Main thread only.
pub fn focus_webview(ns_window: *mut c_void) {
    if ns_window.is_null() {
        return;
    }
    let w = ns_window as *mut AnyObject;
    unsafe {
        let _: () = msg_send![w, makeKeyAndOrderFront: std::ptr::null::<AnyObject>()];
        let content: *mut AnyObject = msg_send![w, contentView];
        if let Some(view) = find_webview(content, 0) {
            let _: bool = msg_send![w, makeFirstResponder: view];
        }
    }
}

unsafe fn find_webview(view: *mut AnyObject, depth: u32) -> Option<*mut AnyObject> {
    if view.is_null() || depth > 4 {
        return None;
    }
    let is_web: bool = msg_send![view, isKindOfClass: class!(WKWebView)];
    if is_web {
        return Some(view);
    }
    let subviews: *mut AnyObject = msg_send![view, subviews];
    let n: usize = msg_send![subviews, count];
    for i in 0..n {
        let v: *mut AnyObject = msg_send![subviews, objectAtIndex: i];
        if let Some(found) = find_webview(v, depth + 1) {
            return Some(found);
        }
    }
    None
}

/// Process id of the app in front (0 when unknown or when it is us).
pub fn frontmost_pid() -> i32 {
    unsafe {
        let ws: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        if ws.is_null() {
            return 0;
        }
        let app: *mut AnyObject = msg_send![ws, frontmostApplication];
        if app.is_null() {
            return 0;
        }
        let pid: i32 = msg_send![app, processIdentifier];
        if pid == std::process::id() as i32 {
            0
        } else {
            pid
        }
    }
}

/// Gives the focus back to the app that was in front before.
pub fn activate_pid(pid: i32) {
    if pid <= 0 {
        return;
    }
    unsafe {
        let app: *mut AnyObject =
            msg_send![class!(NSRunningApplication), runningApplicationWithProcessIdentifier: pid];
        if !app.is_null() {
            let _: bool = msg_send![app, activateWithOptions: 0usize];
        }
    }
}

/// Opens a System Settings pane / URL with the default handler.
pub fn open_url(url: &str) {
    let _ = std::process::Command::new("/usr/bin/open").arg(url).spawn();
}

/// One line describing a window, for NEEDLEX_DEBUG logging. Main thread only.
pub fn describe_window(ns_window: *mut c_void) -> String {
    if ns_window.is_null() {
        return "null".into();
    }
    let w = ns_window as *mut AnyObject;
    unsafe {
        let visible: bool = msg_send![w, isVisible];
        let on_space: bool = msg_send![w, isOnActiveSpace];
        let level: isize = msg_send![w, level];
        let occl: usize = msg_send![w, occlusionState];
        let alpha: f64 = msg_send![w, alphaValue];
        let ignores: bool = msg_send![w, ignoresMouseEvents];
        let num: isize = msg_send![w, windowNumber];
        let cb: usize = msg_send![w, collectionBehavior];
        format!("#{num} behavior={cb:#x} visible={visible} onActiveSpace={on_space} level={level} occlusion={occl} alpha={alpha} ignoresMouse={ignores}")
    }
}
