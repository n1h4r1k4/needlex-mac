//! Follows what is playing on the Mac.
//!
//! macOS has no public "now playing" API for other apps, so this talks to the
//! players through their AppleScript dictionaries (the same thing Shortcuts
//! and Raycast use). macOS asks once per player whether NeedleX may control
//! it. Supported: Spotify and Apple Music.
//!
//! A long-running `osascript` (see `media.js`) prints a JSON snapshot of every
//! player 2-3 times a second; this worker picks the session to follow, turns it
//! into `media:track` / `media:state` events and sends the playback commands.

use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager as _};

use crate::AppState;

const POLLER_JS: &str = include_str!("media.js");

/// Writes the artwork of Apple Music's current track to the file in argv[1].
const MUSIC_ART_SCPT: &str = r#"on run argv
  set outPath to item 1 of argv
  tell application "Music"
    if player state is stopped then return "none"
    if (count of artworks of current track) is 0 then return "none"
    set d to raw data of artwork 1 of current track
  end tell
  set f to open for access (POSIX file outPath) with write permission
  try
    set eof f to 0
    write d to f
    close access f
  on error
    try
      close access f
    end try
    return "error"
  end try
  return "ok"
end run
"#;

pub enum MediaCmd {
    Toggle,
    Next,
    Prev,
    Seek(f64),
    Resend,
    /// Fresh snapshot of all players from the poller.
    Snapshot(Poll),
    /// Artwork finished downloading for the track with this key.
    Art { key: String, url: String },
}

#[derive(Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Session {
    app: String,
    name: String,
    title: String,
    artist: String,
    album: String,
    duration_ms: f64,
    position_ms: f64,
    playing: bool,
    art_url: String,
    can_seek: bool,
}

#[derive(Deserialize, Default, Debug)]
#[serde(default)]
pub struct Poll {
    sessions: Vec<Session>,
    denied: Vec<String>,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    pub key: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub art: Option<String>,
    pub source: String,
    pub source_name: String,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayState {
    pub active: bool,
    pub playing: bool,
    pub position_ms: f64,
    pub duration_ms: f64,
    pub has_timeline: bool,
    pub can_next: bool,
    pub can_prev: bool,
    pub can_seek: bool,
}

/// Puts one of our helper scripts in the app's cache folder and returns its path.
fn write_script(app: &AppHandle, name: &str, body: &str) -> PathBuf {
    let dir = app
        .path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("NeedleX"));
    let _ = std::fs::create_dir_all(&dir);
    let p = dir.join(name);
    if std::fs::read_to_string(&p).ok().as_deref() != Some(body) {
        let _ = std::fs::write(&p, body);
    }
    p
}

pub fn spawn(app: AppHandle, rx: Receiver<MediaCmd>) {
    let tx = app.state::<AppState>().media_tx.lock().unwrap().clone();
    let poller = write_script(&app, "media-poller.js", POLLER_JS);
    let art_script = write_script(&app, "music-artwork.applescript", MUSIC_ART_SCPT);
    spawn_poller(poller, tx.clone());

    std::thread::spawn(move || {
        let mut w = Worker {
            app,
            tx,
            art_script,
            current: String::new(),
            sessions: Vec::new(),
            denied: Vec::new(),
            track: Track::default(),
            last_snapshot: Instant::now(),
            hold: None,
        };
        let debug = std::env::var_os("NEEDLEX_DEBUG").is_some();
        let mut last_debug = String::new();
        loop {
            match rx.recv_timeout(Duration::from_millis(1500)) {
                Ok(MediaCmd::Snapshot(p)) => {
                    if debug {
                        let line = format!("{:?} denied={:?}", p.sessions.iter().map(|s| (&s.app, &s.title, &s.artist, s.playing, s.art_url.len())).collect::<Vec<_>>(), p.denied);
                        if line != last_debug {
                            eprintln!("[media] {line}");
                            last_debug = line;
                        }
                    }
                    w.last_snapshot = Instant::now();
                    w.sessions = p.sessions;
                    if let Some((app, want, until)) = w.hold.clone() {
                        let s = w.sessions.iter_mut().find(|s| s.app == app);
                        match s {
                            Some(s) if s.playing != want && Instant::now() < until => s.playing = want,
                            _ => w.hold = None,
                        }
                    }
                    w.set_denied(p.denied);
                    w.update();
                }
                Ok(MediaCmd::Art { key, url }) => {
                    if debug {
                        eprintln!("[media] artwork {} bytes for current={}", url.len(), key == w.track.key);
                    }
                    if key == w.track.key {
                        w.track.art = Some(url);
                        w.emit_track();
                    }
                }
                Ok(MediaCmd::Resend) => w.emit_track(),
                Ok(cmd) => w.exec(cmd),
                Err(RecvTimeoutError::Timeout) => {
                    // poller stuck or restarting: don't show a stale song forever
                    if w.last_snapshot.elapsed() > Duration::from_secs(5) && !w.sessions.is_empty() {
                        w.sessions.clear();
                        w.update();
                    }
                }
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    });
}

/// Keeps one `osascript` poller running; restarts it if it ever exits.
fn spawn_poller(script: PathBuf, tx: Sender<MediaCmd>) {
    std::thread::spawn(move || loop {
        let child = Command::new("/usr/bin/osascript")
            .arg("-l")
            .arg("JavaScript")
            .arg(&script)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut c) = child {
            if let Some(out) = c.stdout.take() {
                for line in BufReader::new(out).lines() {
                    let Ok(line) = line else { break };
                    let Ok(p) = serde_json::from_str::<Poll>(&line) else { continue };
                    if tx.send(MediaCmd::Snapshot(p)).is_err() {
                        let _ = c.kill();
                        return;
                    }
                }
            }
            let _ = c.kill();
            let _ = c.wait();
        }
        std::thread::sleep(Duration::from_secs(2));
    });
}

struct Worker {
    app: AppHandle,
    tx: Sender<MediaCmd>,
    art_script: PathBuf,
    /// player we are following ("spotify" / "music")
    current: String,
    sessions: Vec<Session>,
    denied: Vec<String>,
    track: Track,
    last_snapshot: Instant,
    /// after play / pause from NeedleX: (player, wanted state, until) – polls
    /// taken before the player reacted don't flip the button back
    hold: Option<(String, bool, Instant)>,
}

impl Worker {
    /// Prefer: the player we already follow if it is playing → any playing
    /// player → the player we already follow → the first one.
    fn pick(&self) -> Option<Session> {
        let same = self.sessions.iter().find(|s| s.app == self.current);
        if let Some(s) = same.filter(|s| s.playing) {
            return Some(s.clone());
        }
        if let Some(s) = self.sessions.iter().find(|s| s.playing) {
            return Some(s.clone());
        }
        same.or_else(|| self.sessions.first()).cloned()
    }

    fn set_denied(&mut self, denied: Vec<String>) {
        if denied != self.denied {
            self.denied = denied.clone();
            *self.app.state::<AppState>().denied.lock().unwrap() = denied;
            crate::rebuild_menu(&self.app);
        }
    }

    fn exec(&mut self, cmd: MediaCmd) {
        let Some(s) = self.pick() else { return };
        let toggle = matches!(cmd, MediaCmd::Toggle);
        let target = match s.app.as_str() {
            "spotify" => "Spotify",
            "music" => "Music",
            _ => return,
        };
        let stmt = match cmd {
            MediaCmd::Toggle => "playpause".to_string(),
            MediaCmd::Next => "next track".to_string(),
            // Music's "back track" restarts the song first, like the media keys
            MediaCmd::Prev if target == "Music" => "back track".to_string(),
            MediaCmd::Prev => "previous track".to_string(),
            MediaCmd::Seek(ms) => format!("set player position to {:.3}", (ms / 1000.0).max(0.0)),
            _ => return,
        };
        let script = format!("tell application \"{target}\" to {stmt}");
        std::thread::spawn(move || {
            let _ = Command::new("/usr/bin/osascript")
                .arg("-e")
                .arg(script)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        });
        // show play / pause right away; the poller confirms it a moment later
        if toggle {
            let since = self.last_snapshot.elapsed().as_secs_f64() * 1000.0;
            if let Some(x) = self.sessions.iter_mut().find(|x| x.app == s.app) {
                if x.playing {
                    x.position_ms += since;
                }
                x.playing = !x.playing;
                self.hold = Some((x.app.clone(), x.playing, Instant::now() + Duration::from_millis(1500)));
            }
            self.last_snapshot = Instant::now();
            self.update();
        }
    }

    fn update(&mut self) {
        let Some(s) = self.pick() else {
            self.clear();
            return;
        };
        self.current = s.app.clone();

        let key = format!("{}\u{1f}{}\u{1f}{}\u{1f}{}", s.app, s.title, s.artist, s.album);
        if key != self.track.key {
            self.track = Track {
                key: key.clone(),
                title: s.title.clone(),
                artist: s.artist.clone(),
                album: s.album.clone(),
                art: None,
                source: s.app.clone(),
                source_name: s.name.clone(),
            };
            self.emit_track();
            self.fetch_art(&s, key);
        }

        let playing = s.playing;
        self.app.state::<AppState>().playing.store(playing, Ordering::Relaxed);
        let duration_ms = s.duration_ms.max(0.0);
        let mut position_ms = s.position_ms.max(0.0);
        if duration_ms > 0.0 {
            position_ms = position_ms.min(duration_ms);
        }
        let state = PlayState {
            active: true,
            playing,
            position_ms,
            duration_ms,
            has_timeline: duration_ms > 1000.0,
            can_next: true,
            can_prev: true,
            can_seek: s.can_seek,
        };
        let _ = self.app.emit("media:state", state);
    }

    /// Player artwork as a data URL (Spotify hands out a 640 px cover URL;
    /// Apple Music's artwork is exported through AppleScript).
    fn fetch_art(&self, s: &Session, key: String) {
        let tx = self.tx.clone();
        if !s.art_url.is_empty() {
            let http = self.app.state::<AppState>().http.clone();
            let url = s.art_url.clone();
            tauri::async_runtime::spawn(async move {
                let Ok(r) = http.get(&url).send().await else { return };
                let Ok(b) = r.bytes().await else { return };
                if let Some(data) = data_url(&b) {
                    let _ = tx.send(MediaCmd::Art { key, url: data });
                }
            });
        } else if s.app == "music" {
            let script = self.art_script.clone();
            let out = std::env::temp_dir().join(format!("needlex-art-{}.img", std::process::id()));
            std::thread::spawn(move || {
                let ok = Command::new("/usr/bin/osascript")
                    .arg(&script)
                    .arg(&out)
                    .stdin(Stdio::null())
                    .stderr(Stdio::null())
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "ok")
                    .unwrap_or(false);
                if ok {
                    if let Some(data) = std::fs::read(&out).ok().as_deref().and_then(data_url) {
                        let _ = tx.send(MediaCmd::Art { key, url: data });
                    }
                }
                let _ = std::fs::remove_file(&out);
            });
        }
    }

    fn clear(&mut self) {
        if !self.track.key.is_empty() {
            self.track = Track::default();
            self.emit_track();
        }
        self.app.state::<AppState>().playing.store(false, Ordering::Relaxed);
        let _ = self.app.emit("media:state", PlayState::default());
    }

    fn emit_track(&self) {
        let _ = self.app.emit("media:track", self.track.clone());
    }
}

fn data_url(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 64 {
        return None;
    }
    let mime = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        "image/png"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"RIFF") {
        "image/webp"
    } else if bytes.starts_with(b"GIF") {
        "image/gif"
    } else if bytes.starts_with(b"MM\0*") || bytes.starts_with(b"II*\0") {
        "image/tiff"
    } else {
        "image/jpeg"
    };
    Some(format!("data:{mime};base64,{}", STANDARD.encode(bytes)))
}
