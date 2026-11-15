//! Where the Mac is, for the island's weather page.
//!
//! macOS only asks "Allow NeedleX to use your location?" for a real app, so
//! this runs in NeedleX itself: a plain helper program never gets an answer
//! (its request just waits forever). A CLLocationManager lives on the main
//! thread for the life of the app; `current` waits for it from a worker
//! thread. Only a rough fix (about a kilometre) is asked for, and it is
//! stopped as soon as one arrives.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use objc2::encode::{Encode, Encoding};
use objc2::runtime::{AnyClass, AnyObject, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};
use tauri::AppHandle;

#[link(name = "CoreLocation", kind = "framework")]
extern "C" {}

#[repr(C)]
#[derive(Clone, Copy)]
struct Coord {
    lat: f64,
    lon: f64,
}

unsafe impl Encode for Coord {
    const ENCODING: Encoding = Encoding::Struct("CLLocationCoordinate2D", &[f64::ENCODING, f64::ENCODING]);
}

// CLAuthorizationStatus
const NOT_DETERMINED: i32 = 0;
const RESTRICTED: i32 = 1;
const DENIED: i32 = 2;

/// the CLLocationManager (main thread only; never released)
static MANAGER: AtomicUsize = AtomicUsize::new(0);
/// updates are running (stopped again once a fix arrives)
static UPDATING: AtomicBool = AtomicBool::new(false);
/// the newest fix from the delegate
static FIX: Mutex<Option<(Instant, f64, f64)>> = Mutex::new(None);

/// how old a fix may be and still count
const FRESH: Duration = Duration::from_secs(30 * 60);

unsafe fn coord_of(loc: *mut AnyObject) -> Option<(f64, f64)> {
    if loc.is_null() {
        return None;
    }
    let c: Coord = msg_send![loc, coordinate];
    (c.lat.is_finite() && c.lon.is_finite() && (c.lat != 0.0 || c.lon != 0.0)).then_some((c.lat, c.lon))
}

extern "C-unwind" fn did_update(_this: *mut AnyObject, _sel: Sel, manager: *mut AnyObject, locations: *mut AnyObject) {
    unsafe {
        let last: *mut AnyObject = msg_send![locations, lastObject];
        if let Some((lat, lon)) = coord_of(last) {
            *FIX.lock().unwrap() = Some((Instant::now(), lat, lon));
            let _: () = msg_send![manager, stopUpdatingLocation];
            UPDATING.store(false, Ordering::SeqCst);
        }
    }
}

extern "C-unwind" fn did_fail(_this: *mut AnyObject, _sel: Sel, _manager: *mut AnyObject, _error: *mut AnyObject) {
    // "location unknown right now" keeps trying by itself; a refusal shows
    // up in the authorization status, which `current` watches
}

extern "C-unwind" fn did_change_auth(_this: *mut AnyObject, _sel: Sel, manager: *mut AnyObject) {
    unsafe {
        let status: i32 = msg_send![manager, authorizationStatus];
        if status > DENIED && !UPDATING.swap(true, Ordering::SeqCst) {
            let _: () = msg_send![manager, startUpdatingLocation];
        }
    }
}

fn delegate_class() -> &'static AnyClass {
    static CLS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLS.get_or_init(|| {
        let mut b = ClassBuilder::new(c"NXLocationDelegate", class!(NSObject)).expect("location delegate class");
        unsafe {
            b.add_method(
                sel!(locationManager:didUpdateLocations:),
                did_update as extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
            );
            b.add_method(
                sel!(locationManager:didFailWithError:),
                did_fail as extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject, *mut AnyObject),
            );
            b.add_method(
                sel!(locationManagerDidChangeAuthorization:),
                did_change_auth as extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject),
            );
        }
        b.register()
    })
}

/// Main thread only: makes sure a request is under way; returns the
/// authorization status and a usable fix, if there is one.
unsafe fn poll() -> (i32, Option<(f64, f64)>) {
    let mut m = MANAGER.load(Ordering::SeqCst) as *mut AnyObject;
    if m.is_null() {
        m = msg_send![class!(CLLocationManager), new];
        if m.is_null() {
            return (DENIED, None);
        }
        let delegate: *mut AnyObject = msg_send![delegate_class(), new];
        let _: () = msg_send![m, setDelegate: delegate];
        let _: () = msg_send![m, setDesiredAccuracy: 1000.0f64]; // kCLLocationAccuracyKilometer
        MANAGER.store(m as usize, Ordering::SeqCst);
    }
    let status: i32 = msg_send![m, authorizationStatus];
    match status {
        NOT_DETERMINED => {
            let _: () = msg_send![m, requestWhenInUseAuthorization];
        }
        RESTRICTED | DENIED => return (status, None),
        _ => {}
    }
    if let Some((at, lat, lon)) = *FIX.lock().unwrap() {
        if at.elapsed() < FRESH {
            return (status, Some((lat, lon)));
        }
    }
    if status > DENIED {
        // the system may already know where we are
        let loc: *mut AnyObject = msg_send![m, location];
        if !loc.is_null() {
            let ts: *mut AnyObject = msg_send![loc, timestamp];
            let age: f64 = if ts.is_null() {
                f64::MAX
            } else {
                let since: f64 = msg_send![ts, timeIntervalSinceNow];
                -since
            };
            if age < FRESH.as_secs_f64() {
                if let Some(fix) = coord_of(loc) {
                    return (status, Some(fix));
                }
            }
        }
        if !UPDATING.swap(true, Ordering::SeqCst) {
            let _: () = msg_send![m, startUpdatingLocation];
        }
    }
    (status, None)
}

/// Blocking (the first time it waits for your answer to macOS's question).
/// Call it off the main thread.
pub fn current(app: &AppHandle, timeout: Duration) -> Result<(f64, f64), String> {
    let start = Instant::now();
    loop {
        let (tx, rx) = mpsc::channel();
        app.run_on_main_thread(move || {
            let _ = tx.send(unsafe { poll() });
        })
        .map_err(|e| e.to_string())?;
        let (status, fix) = rx.recv_timeout(Duration::from_secs(5)).map_err(|_| "location timed out".to_string())?;
        if crate::debug() {
            eprintln!("[location] status={status} fix={}", fix.is_some());
        }
        if status == DENIED || status == RESTRICTED {
            return Err("location denied".into());
        }
        if let Some(fix) = fix {
            return Ok(fix);
        }
        if start.elapsed() > timeout {
            return Err(if status == NOT_DETERMINED { "no answer about location" } else { "location timed out" }.into());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}
