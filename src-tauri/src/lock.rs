//! The now-playing wallpaper on the lock screen.
//!
//! When the screen locks, a window with the full view (in lock mode: clock,
//! cover / lyrics, the bottom left clear for the password field) is created on
//! top of the lock screen; it is closed again as soon as the Mac is unlocked,
//! so nothing of it is left over the desktop. It never takes the keyboard, and
//! the mouse only over what can be clicked (record, cover, player card,
//! lyrics). The page itself only shows up while music plays.

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{display, mac, AppState};

pub fn spawn(app: AppHandle) {
    spawn_hits(app.clone());
    std::thread::spawn(move || {
        let mut was_locked = false;
        loop {
            std::thread::sleep(Duration::from_millis(250));
            let locked = mac::screen_locked();
            if !locked && app.get_webview_window("lock").is_some() && !was_locked {
                // never leave it over the desktop (it sits above everything)
                let a = app.clone();
                let _ = app.run_on_main_thread(move || hide(&a));
            }
            if locked == was_locked {
                continue;
            }
            was_locked = locked;
            let wanted = app.state::<AppState>().settings.lock().unwrap().lock_screen;
            if crate::debug() {
                eprintln!("[lock] screen locked={locked} lock-screen view wanted={wanted}");
            }
            let a = app.clone();
            if locked && wanted {
                let _ = app.run_on_main_thread(move || show(&a));
            } else if !locked {
                let _ = app.run_on_main_thread(move || hide(&a));
            }
        }
    });
}

/// Main thread only.
pub fn show(app: &AppHandle) {
    if app.get_webview_window("lock").is_some() {
        return;
    }
    let Some(mon) = display::target_monitor(app) else { return };
    let (x, y, w, h) = display::logical_frame(&mon);
    let built = WebviewWindowBuilder::new(app, "lock", WebviewUrl::App("full.html".into()))
        .title("NeedleX Lock Screen")
        .transparent(true)
        .decorations(false)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .focused(false)
        .focusable(false)
        .accept_first_mouse(true)
        .inner_size(w, h)
        .position(x, y)
        .build();
    let win = match built {
        Ok(win) => win,
        Err(e) => {
            eprintln!("NeedleX: lock-screen window failed: {e}");
            return;
        }
    };
    let _ = win.set_ignore_cursor_events(true);
    let on_top = win.ns_window().map(mac::setup_lock_window).unwrap_or(false);
    if crate::debug() {
        eprintln!("[lock] window created, moved above the lock screen: {on_top}");
    }
    if !on_top {
        // without SkyLight it would only cover the desktop – not wanted
        eprintln!("NeedleX: can't draw on the lock screen on this macOS");
        let _ = win.destroy();
    }
}

/// Main thread only.
pub fn hide(app: &AppHandle) {
    app.state::<AppState>().lock_hit.lock().unwrap().clear();
    if let Some(w) = app.get_webview_window("lock") {
        let _ = w.destroy();
    }
}

/// The lock-screen window takes the mouse only over the parts the page
/// reported as clickable; everywhere else clicks go to the lock screen.
/// It also tells the page when you start typing or point at the bottom of the
/// screen, so it can uncover your picture and the password field.
fn spawn_hits(app: AppHandle) {
    std::thread::spawn(move || {
        let mut ignoring: Option<bool> = None;
        let mut revealed: Option<bool> = None;
        let mut reveal_until = Instant::now();
        let mut origin: Option<(f64, f64)> = None;
        let mut origin_at = Instant::now() - Duration::from_secs(10);
        loop {
            std::thread::sleep(Duration::from_millis(40));
            let Some(win) = app.get_webview_window("lock") else {
                ignoring = None;
                revealed = None;
                origin = None;
                std::thread::sleep(Duration::from_millis(300));
                continue;
            };
            if origin.is_none() || origin_at.elapsed() > Duration::from_secs(1) {
                origin_at = Instant::now();
                origin = match (win.outer_position(), win.scale_factor()) {
                    (Ok(p), Ok(s)) => {
                        let l = p.to_logical::<f64>(s);
                        Some((l.x, l.y))
                    }
                    _ => None,
                };
            }
            let rects = app.state::<AppState>().lock_hit.lock().unwrap().clone();
            let inside = match (origin, mac::cursor()) {
                (Some((wx, wy)), Some((px, py))) => {
                    let held = ignoring == Some(false) && mac::left_button_down();
                    held || rects.iter().any(|r| {
                        px >= wx + r.x && px <= wx + r.x + r.w && py >= wy + r.y && py <= wy + r.y + r.h
                    })
                }
                _ => false,
            };
            let ignore = !inside;
            if ignoring != Some(ignore) && win.set_ignore_cursor_events(ignore).is_ok() {
                ignoring = Some(ignore);
            }

            // typing (the password), or the pointer near the bottom: show the login area
            let near_login = match (origin, mac::cursor(), win.inner_size(), win.scale_factor()) {
                (Some((_, wy)), Some((_, py)), Ok(sz), Ok(s)) => py - wy > sz.height as f64 / s * 0.72,
                _ => false,
            };
            if mac::key_idle_secs() < 1.5 || near_login {
                reveal_until = Instant::now() + Duration::from_secs(if near_login { 2 } else { 12 });
            }
            let reveal = Instant::now() < reveal_until;
            if revealed != Some(reveal) {
                revealed = Some(reveal);
                let _ = app.emit_to("lock", "lock:reveal", reveal);
            }
        }
    });
}
