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

// ---------------------------------------------------------------- lock screen
//
// The lock screen covers every normal window. A window in a SkyLight space of
// its own, with a high absolute level, is still drawn on top of it (that's how
// lock-screen music players do it). SkyLight is a private framework, so it is
// looked up at run time; if anything is missing the lock screen just stays as
// macOS draws it.

struct SkyLight {
    cid: i32,
    space: u64,
    add_windows: unsafe extern "C" fn(i32, u64, *mut AnyObject, i32) -> i32,
}

fn skylight() -> Option<&'static SkyLight> {
    static SKY: OnceLock<Option<SkyLight>> = OnceLock::new();
    SKY.get_or_init(|| unsafe {
        let h = dlopen(c"/System/Library/PrivateFrameworks/SkyLight.framework/Versions/A/SkyLight".as_ptr(), 2);
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
        set_level(cid, space, 100);
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
        // macOS draws the lock screen at window levels 2001–2004: go just above
        let _: () = msg_send![w, setLevel: LOCK_LEVEL];
        let _: () = msg_send![w, setHasShadow: false];
        let _: () = msg_send![w, setIgnoresMouseEvents: true];
        // clicks on the record / cover / player must not activate NeedleX
        // (the password field has to keep the keyboard)
        let prevents = sel!(_setPreventsActivation:);
        let responds: bool = msg_send![w, respondsToSelector: prevents];
        if responds {
            let _: () = msg_send![w, _setPreventsActivation: true];
        }
        let _: () = msg_send![w, orderFrontRegardless];
        let Some(sky) = skylight() else { return false };
        let num: isize = msg_send![w, windowNumber];
        if num <= 0 {
            return false;
        }
        let n: *mut AnyObject = msg_send![class!(NSNumber), numberWithInt: num as i32];
        let list: *mut AnyObject = msg_send![class!(NSArray), arrayWithObject: n];
        let r = (sky.add_windows)(sky.cid, sky.space, list, 7);
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
