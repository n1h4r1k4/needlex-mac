//! Opens the full-screen view automatically when the Mac has been idle (no
//! mouse / keyboard / trackpad input) for the chosen number of minutes while
//! music plays. It stays out of the way of full-screen video, presentations,
//! video calls and the lock screen.

use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::{mac, AppState};

fn idle_ms() -> u64 {
    (mac::idle_secs().max(0.0) * 1000.0) as u64
}

/// false while something keeps the display awake on purpose (video players,
/// Keynote / PowerPoint slideshows, video calls) or the screen is locked.
fn screen_is_free() -> bool {
    if mac::screen_locked() {
        return false;
    }
    let Ok(out) = Command::new("/usr/bin/pmset").args(["-g", "assertions"]).output() else { return true };
    let text = String::from_utf8_lossy(&out.stdout);
    // summary lines look like "   PreventUserIdleDisplaySleep    1"
    !text.lines().any(|l| {
        let mut it = l.split_whitespace();
        it.next() == Some("PreventUserIdleDisplaySleep") && it.next().map(|n| n != "0").unwrap_or(false)
    })
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let mut fired = false;
        loop {
            std::thread::sleep(Duration::from_secs(2));
            let state = app.state::<AppState>();
            let minutes = state.settings.lock().unwrap().auto_full_minutes;
            let idle = idle_ms();
            if idle < 4000 {
                fired = false; // user is back – arm again for the next idle period
            }
            if minutes == 0 || fired {
                continue;
            }
            if idle < minutes as u64 * 60_000 || !state.playing.load(Ordering::Relaxed) {
                continue;
            }
            let full_open = app
                .get_webview_window("full")
                .and_then(|f| f.is_visible().ok())
                .unwrap_or(false);
            if full_open || !screen_is_free() {
                continue;
            }
            fired = true;
            crate::show_full(&app, true);
        }
    });
}
