//! The island window is mostly transparent. It is click-through everywhere
//! except over the island itself; we poll the global cursor to detect hover
//! (the webview can't see the mouse while the window ignores it).
//!
//! macOS works in points (logical px) everywhere, so all geometry here is
//! logical: window position, cursor and the hit rectangle (CSS px × zoom).

use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::{mac, AppState};

#[derive(Clone, Copy, Default, serde::Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut inside = false;
        let mut ignoring: Option<bool> = None;
        let mut geo: Option<(f64, f64)> = None; // logical window origin
        let mut geo_at = Instant::now() - Duration::from_secs(10);
        loop {
            std::thread::sleep(Duration::from_millis(30));
            let Some(win) = app.get_webview_window("main") else { continue };

            let state = app.state::<AppState>();
            let dirty = state.geo_dirty.swap(false, std::sync::atomic::Ordering::Relaxed);
            if dirty || geo_at.elapsed() > Duration::from_millis(700) {
                geo_at = Instant::now();
                geo = match (win.outer_position(), win.scale_factor()) {
                    // a hidden window can't be hovered
                    (Ok(p), Ok(s)) if win.is_visible().unwrap_or(true) => {
                        let l = p.to_logical::<f64>(s);
                        Some((l.x, l.y))
                    }
                    _ => None,
                };
            }
            let Some((wx, wy)) = geo else {
                if inside {
                    inside = false;
                    let _ = app.emit("island:hover", false);
                }
                continue;
            };
            // UI coordinates are CSS px of a page that may be zoomed down to fit the screen
            let zoom = *state.zoom.lock().unwrap();
            let r = *state.hit.lock().unwrap();

            let Some((px, py)) = mac::cursor() else { continue };
            let x0 = wx + r.x * zoom;
            let y0 = wy + r.y * zoom;
            // while the notch is dragged the window must keep the mouse
            let dragging = state.dragging.load(std::sync::atomic::Ordering::Relaxed);
            let now_inside = dragging
                || r.w > 0.0
                    && px >= x0
                    && px <= x0 + r.w * zoom
                    && py >= y0 - 2.0 // include the very top pixel row of the screen
                    && py <= y0 + r.h * zoom;

            let ignore = !now_inside;
            if ignoring != Some(ignore) && win.set_ignore_cursor_events(ignore).is_ok() {
                ignoring = Some(ignore);
            }
            if now_inside != inside {
                inside = now_inside;
                let _ = app.emit("island:hover", inside);
            }
        }
    });
}

/// The desktop objects (turntable, sleeve, aura) can be dragged around and
/// clicked: the wallpaper window takes the mouse only over the object (and
/// while a drag that started there goes on), everywhere else clicks reach the
/// desktop and its icons.
pub fn spawn_wall(app: AppHandle) {
    std::thread::spawn(move || {
        let mut ignoring: Option<bool> = None;
        let mut geo: Option<(f64, f64)> = None;
        let mut geo_at = Instant::now() - Duration::from_secs(10);
        let mut n: u32 = 0;
        loop {
            std::thread::sleep(Duration::from_millis(40));
            let Some(win) = app.get_webview_window("wall") else {
                ignoring = None;
                geo = None;
                std::thread::sleep(Duration::from_millis(400));
                continue;
            };
            let state = app.state::<AppState>();
            let objects = crate::wall_is_object(&state.settings.lock().unwrap().wallpaper);
            let r = *state.wall_hit.lock().unwrap();
            let ignore = if !objects || r.w <= 0.0 {
                true
            } else {
                if state.wall_dirty.swap(false, std::sync::atomic::Ordering::Relaxed) || geo_at.elapsed() > Duration::from_millis(700) {
                    geo_at = Instant::now();
                    geo = match (win.outer_position(), win.scale_factor()) {
                        (Ok(p), Ok(s)) if win.is_visible().unwrap_or(true) => {
                            let l = p.to_logical::<f64>(s);
                            Some((l.x, l.y))
                        }
                        _ => None,
                    };
                }
                match (geo, mac::cursor()) {
                    (Some((wx, wy)), Some((px, py))) => {
                        let held = ignoring == Some(false) && mac::left_button_down();
                        let inside = px >= wx + r.x && px <= wx + r.x + r.w && py >= wy + r.y && py <= wy + r.y + r.h;
                        !(inside || held)
                    }
                    _ => true,
                }
            };
            // re-applied now and then: a new or reloaded window starts out taking the mouse
            n = n.wrapping_add(1);
            if (ignoring != Some(ignore) || n % 25 == 0) && win.set_ignore_cursor_events(ignore).is_ok() {
                ignoring = Some(ignore);
            }
        }
    });
}
