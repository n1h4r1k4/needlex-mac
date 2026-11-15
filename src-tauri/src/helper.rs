//! Runs `needlex-helper`, the small Swift companion bundled next to the app
//! binary, and talks to it over stdin / stdout (see helper/helper.swift):
//!
//! * real-time output level for the visualizer and beat glow (`audio:level`)
//! * screenshots of the area behind the island for the liquid glass
//! * the Mac's location (weather) and calendar events (island pages)
//!
//! If the helper is missing or keeps failing, NeedleX simply runs without
//! those two extras (the UI has fallbacks for both).

use std::io::{BufRead, BufReader, Write};
use std::process::{ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::AppState;

struct Helper {
    stdin: Mutex<Option<ChildStdin>>,
    snap_waiter: Mutex<Option<Sender<Result<(), String>>>>,
    /// callers waiting for a `loc` / `cal` answer
    waiters: Mutex<Vec<(&'static str, Sender<Result<String, String>>)>>,
}

static HELPER: OnceLock<Helper> = OnceLock::new();

fn helper() -> &'static Helper {
    HELPER.get_or_init(|| Helper { stdin: Mutex::new(None), snap_waiter: Mutex::new(None), waiters: Mutex::new(Vec::new()) })
}

/// Sends `cmd` and waits for the helper's answer line starting with `cmd `
/// (or `cmd err …`). Blocking – call it off the main thread.
pub fn ask(cmd: &'static str, timeout: Duration) -> Result<String, String> {
    let (tx, rx) = mpsc::channel();
    helper().waiters.lock().unwrap().push((cmd, tx));
    if !send(cmd) {
        return Err("helper not running".into());
    }
    rx.recv_timeout(timeout).map_err(|_| "timed out".to_string())?
}

fn answer(cmd: &str, rest: &str) {
    let res = match rest.strip_prefix("err ") {
        Some(e) => Err(e.to_string()),
        None => Ok(rest.to_string()),
    };
    let mut ws = helper().waiters.lock().unwrap();
    let (mine, others): (Vec<_>, Vec<_>) = ws.drain(..).partition(|(c, _)| *c == cmd);
    *ws = others;
    for (_, w) in mine {
        let _ = w.send(res.clone());
    }
}

fn send(line: &str) -> bool {
    let mut guard = helper().stdin.lock().unwrap();
    let Some(stdin) = guard.as_mut() else { return false };
    let ok = writeln!(stdin, "{line}").and_then(|_| stdin.flush()).is_ok();
    if !ok {
        *guard = None;
    }
    ok
}

/// Turns the audio level meter on or off (tray → Real audio visualizer).
pub fn set_audio(on: bool) {
    send(if on { "audio on" } else { "audio off" });
}

/// Screenshot of a screen area (global points) into a JPEG file, leaving out
/// NeedleX's own windows. Blocks for at most a few seconds.
pub fn snap(x: f64, y: f64, w: f64, h: f64, path: &std::path::Path) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    *helper().snap_waiter.lock().unwrap() = Some(tx);
    if !send(&format!("snap {x:.1} {y:.1} {w:.1} {h:.1} {}", path.display())) {
        return Err("helper not running".into());
    }
    rx.recv_timeout(Duration::from_secs(4)).map_err(|_| "timed out".to_string())?
}

pub fn spawn(app: AppHandle) {
    std::thread::spawn(move || {
        let Some(path) = std::env::current_exe().ok().and_then(|e| Some(e.parent()?.join("needlex-helper"))) else { return };
        let mut failures = 0;
        while failures < 5 {
            let child = Command::new(&path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            let Ok(mut child) = child else { return };
            *helper().stdin.lock().unwrap() = child.stdin.take();
            set_audio(app.state::<AppState>().settings.lock().unwrap().visualizer);

            let started = std::time::Instant::now();
            let mut silent = false;
            if let Some(out) = child.stdout.take() {
                for line in BufReader::new(out).lines() {
                    let Ok(line) = line else { break };
                    if let Some(v) = line.strip_prefix("lvl ") {
                        let Ok(peak) = v.trim().parse::<f64>() else { continue };
                        // like the Windows meter: one zero, then quiet until sound returns
                        if peak < 0.002 {
                            if silent {
                                continue;
                            }
                            silent = true;
                        } else {
                            silent = false;
                        }
                        let _ = app.emit("audio:level", peak);
                    } else if let Some(r) = line.strip_prefix("snap ") {
                        let res = if r == "ok" { Ok(()) } else { Err(r.trim_start_matches("err ").to_string()) };
                        if let Some(w) = helper().snap_waiter.lock().unwrap().take() {
                            let _ = w.send(res);
                        }
                    } else if let Some(r) = line.strip_prefix("loc ") {
                        answer("loc", r);
                    } else if let Some(r) = line.strip_prefix("cal ") {
                        answer("cal", r);
                    } else if let Some(msg) = line.strip_prefix("log ") {
                        eprintln!("needlex-helper: {msg}");
                    }
                }
            }
            *helper().stdin.lock().unwrap() = None;
            let _ = child.kill();
            let _ = child.wait();
            failures = if started.elapsed() > Duration::from_secs(60) { 0 } else { failures + 1 };
            std::thread::sleep(Duration::from_secs(3));
        }
    });
}
