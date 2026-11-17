//! Keeps the notch correct on every screen setup: any resolution, Retina or
//! not, several displays with different scales, displays being plugged in or
//! removed, resolution changes while running.
//!
//! macOS positions windows in points, so everything here is logical px:
//! a display's logical frame is its physical frame divided by its own scale.
//!
//! * The island UI is designed for a 560×520 (CSS px) canvas. On small or
//!   heavily-scaled screens the whole island is zoomed down (webview zoom), so
//!   its layout – and every coordinate the UI reports – stays identical.
//! * A watcher re-applies the layout whenever the display setup changes.

use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, Monitor};

use crate::{mac, AppState};

pub const ISLAND_W: f64 = 560.0;
pub const ISLAND_H: f64 = 520.0;
/// Within this many logical px of the middle the notch snaps to the centre.
pub const MAGNET_PX: f64 = 46.0;

pub fn monitors(app: &AppHandle) -> Vec<Monitor> {
    app.available_monitors().unwrap_or_default()
}

/// The monitor the notch should live on (user choice → primary → first).
pub fn target_monitor(app: &AppHandle) -> Option<Monitor> {
    let want = app.state::<AppState>().settings.lock().unwrap().monitor.clone();
    let all = monitors(app);
    if !want.is_empty() {
        if let Some(m) = all.iter().find(|m| m.name().map(|n| *n == want).unwrap_or(false)) {
            return Some(m.clone());
        }
    }
    app.primary_monitor().ok().flatten().or_else(|| all.into_iter().next())
}

pub fn monitor_label(i: usize, m: &Monitor, primary: bool) -> String {
    let (_, _, w, h) = logical_frame(m);
    let name = m.name().cloned().unwrap_or_else(|| format!("Display {}", i + 1));
    format!("{name} – {w:.0}×{h:.0}{}", if primary { " (main)" } else { "" })
}

/// Zoom that makes the island fit the monitor (never enlarges past 100 %).
fn fit_zoom(m: &Monitor) -> f64 {
    let s = m.scale_factor();
    let (lw, lh) = (m.size().width as f64 / s, m.size().height as f64 / s);
    (lh * 0.92 / ISLAND_H).min(lw / ISLAND_W).clamp(0.45, 1.0)
}

/// Logical frame (x, y, w, h) of a display.
pub fn logical_frame(m: &Monitor) -> (f64, f64, f64, f64) {
    let s = m.scale_factor();
    let (p, z) = (m.position(), m.size());
    (p.x as f64 / s, p.y as f64 / s, z.width as f64 / s, z.height as f64 / s)
}

/// Puts the island on the target monitor at `offset` logical px from the
/// top-centre, with a magnet at the middle. Returns the applied offset.
pub fn place_island(app: &AppHandle, offset: f64) -> f64 {
    let Some(win) = app.get_webview_window("main") else { return 0.0 };
    let Some(mon) = target_monitor(app) else { return 0.0 };
    let zoom = *app.state::<AppState>().zoom.lock().unwrap();
    let ww = ISLAND_W * zoom;
    let (mx, my, mw, _) = logical_frame(&mon);
    let max = ((mw - ww) / 2.0).max(0.0);
    let mut off = offset.clamp(-max, max);
    if off.abs() < MAGNET_PX {
        off = 0.0;
    }
    let x = mx + (mw - ww) / 2.0 + off;
    let _ = win.set_position(LogicalPosition::new(x.round(), my));
    app.state::<AppState>().geo_dirty.store(true, Ordering::Relaxed);
    off
}

/// Size, zoom and position of the island for the current display setup;
/// also re-fits the full-screen view if it is open.
pub fn relayout(app: &AppHandle) {
    let Some(win) = app.get_webview_window("main") else { return };
    let Some(mon) = target_monitor(app) else { return };
    let zoom = fit_zoom(&mon);
    *app.state::<AppState>().zoom.lock().unwrap() = zoom;
    let off = app.state::<AppState>().settings.lock().unwrap().notch_offset;

    let _ = win.set_size(LogicalSize::new(ISLAND_W * zoom, ISLAND_H * zoom));
    let _ = win.set_zoom(zoom);
    place_island(app, off);

    if app
        .get_webview_window("full")
        .and_then(|f| f.is_visible().ok())
        .unwrap_or(false)
    {
        fit_full(app);
    }
    fit_wall(app);
    let _ = tauri::Emitter::emit(app, "display", zoom);
}

/// Make the full-screen window cover exactly the notch's monitor (menu bar
/// and Dock included).
pub fn fit_full(app: &AppHandle) {
    fit_screen(app, "full");
}

/// The desktop wallpaper covers the notch's monitor too.
pub fn fit_wall(app: &AppHandle) {
    fit_screen(app, "wall");
    app.state::<AppState>().wall_dirty.store(true, Ordering::Relaxed);
}

fn fit_screen(app: &AppHandle, label: &str) {
    let Some(win) = app.get_webview_window(label) else { return };
    let Some(mon) = target_monitor(app) else { return };
    let (x, y, w, h) = logical_frame(&mon);
    // size first: AppKit keeps the bottom edge when a window grows, so
    // positioning first would push the top off the screen
    let _ = win.set_size(LogicalSize::new(w, h));
    let _ = win.set_position(LogicalPosition::new(x, y));
}

fn signature(app: &AppHandle) -> String {
    let want = app.state::<AppState>().settings.lock().unwrap().monitor.clone();
    let prim = app.primary_monitor().ok().flatten().and_then(|m| m.name().cloned()).unwrap_or_default();
    let mut s = format!("{want}|{prim}|");
    for m in monitors(app) {
        let (p, z) = (m.position(), m.size());
        s += &format!("{}:{},{},{}x{}@{};", m.name().cloned().unwrap_or_default(), p.x, p.y, z.width, z.height, m.scale_factor());
    }
    s
}

fn names(app: &AppHandle) -> Vec<String> {
    monitors(app).iter().map(|m| m.name().cloned().unwrap_or_default()).collect()
}

/// Watches for display changes (plug/unplug, resolution, scale, primary).
pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut last = String::new();
        let mut last_names: Vec<String> = Vec::new();
        loop {
            let sig = signature(&app);
            if sig != last {
                last = sig;
                relayout(&app);
                let n = names(&app);
                if n != last_names {
                    last_names = n;
                    crate::rebuild_menu(&app);
                }
            }
            keep_on_all_spaces(&app);
            std::thread::sleep(Duration::from_millis(1200));
        }
    });
}

/// The island, the full-screen view and the wallpaper belong on every Space,
/// full-screen apps' too. macOS sometimes drops them from some Spaces, and the
/// notch then goes missing next to those apps: put them back.
fn keep_on_all_spaces(app: &AppHandle) {
    let a = app.clone();
    let _ = app.run_on_main_thread(move || {
        for label in ["main", "full", "wall"] {
            let Some(w) = a.get_webview_window(label) else { continue };
            let Ok(ns) = w.ns_window() else { continue };
            let back = mac::rejoin_all_spaces(ns);
            if !back.is_empty() {
                eprintln!("NeedleX: {label} window was missing from Spaces {back:?}, put it back");
            }
        }
    });
}

/// Drag the notch sideways with the mouse. Tracking happens here (global
/// screen points) so it is exact at any scale, and ends by itself when the
/// left button is released.
pub fn drag(app: AppHandle) {
    let state = app.state::<AppState>();
    if state.dragging.swap(true, Ordering::SeqCst) {
        return;
    }
    let from = state.settings.lock().unwrap().notch_offset;
    std::thread::spawn(move || {
        let start = mac::cursor().map(|c| c.0).unwrap_or(0.0);
        let mut applied = from;
        let mut last_x = f64::NAN;
        loop {
            std::thread::sleep(Duration::from_millis(8));
            if !mac::left_button_down() {
                break;
            }
            let Some((x, _)) = mac::cursor() else { continue };
            if x == last_x {
                continue;
            }
            last_x = x;
            let want = from + (x - start);
            applied = place_island(&app, want);
            let _ = tauri::Emitter::emit(&app, "notch:drag", serde_json::json!({ "applied": applied, "want": want, "done": false }));
        }
        app.state::<AppState>().dragging.store(false, Ordering::SeqCst);
        crate::update_setting(&app, |s| s.notch_offset = applied);
        let _ = tauri::Emitter::emit(&app, "notch:drag", serde_json::json!({ "applied": applied, "want": applied, "done": true }));
    });
}
