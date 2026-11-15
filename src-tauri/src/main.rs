mod art;
mod display;
mod helper;
mod hover;
mod idle;
mod lock;
mod lyrics;
mod mac;
mod media;
mod weather;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::menu::{CheckMenuItem, CheckMenuItemBuilder, Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::TrayIconBuilder;
use tauri::image::Image;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};

use media::MediaCmd;

/// Auto full-screen choices offered in the tray (minutes, 0 = off).
const IDLE_CHOICES: [u32; 6] = [0, 1, 3, 5, 10, 15];
/// Glitter colour choices offered in the tray.
const GLITTER_CHOICES: [(&str, &str); 4] = [
    ("off", "Off"),
    ("white", "White"),
    ("multi", "Multicolour"),
    ("album", "Album colours"),
];
/// Live desktop wallpaper looks offered in the tray.
const WALL_CHOICES: [(&str, &str); 5] = [
    ("off", "Off"),
    ("lyrics", "Lyrics (in the lyrics effect)"),
    ("turntable", "Turntable"),
    ("sleeve", "Record sleeve"),
    ("aura", "Cover with aura"),
];
/// How the lyrics are shown – on the desktop, the lock screen and in full screen.
const LYRIC_CHOICES: [(&str, &str); 6] = [
    ("words", "Big Words"),
    ("lens", "Lens"),
    ("stars", "Stars"),
    ("terminal", "Terminal"),
    ("aura", "Aura"),
    ("lines", "Scrolling lines"),
];

/// The looks that sit on your own wallpaper as an object you can drag around.
pub fn wall_is_object(look: &str) -> bool {
    matches!(look, "turntable" | "sleeve" | "aura")
}

/// Page that draws a wallpaper look (None = off).
fn wall_page(look: &str) -> Option<&'static str> {
    match look {
        "lyrics" => Some("full.html"),
        l if wall_is_object(l) => Some("desk.html"),
        _ => None,
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub floating: bool,
    pub live_lyrics: bool,
    pub hide_idle: bool,
    pub glow: bool,
    /// Open the full-screen view after this many idle minutes (0 = off).
    pub auto_full_minutes: u32,
    /// Notch hidden from the screen (bring it back from the tray).
    pub notch_hidden: bool,
    /// Horizontal offset of the notch from the screen centre, logical px.
    pub notch_offset: f64,
    /// Monitor (system name) the notch lives on; empty = main display.
    pub monitor: String,
    /// Glitter in the full-screen view: "off" | "white" | "multi" | "album".
    pub glitter: String,
    /// Drive the visualizer / beat glow from the real sound output
    /// (asks for "System Audio Recording"; otherwise a soft fake animation).
    pub visualizer: bool,
    /// Liquid glass that refracts what's behind the island
    /// (asks for "Screen Recording"; otherwise a plain black notch).
    pub glass: bool,
    /// Live desktop wallpaper while music plays: "off" | "lyrics" (the lyrics
    /// fill the desktop) | "turntable" | "sleeve" | "aura" (an object on your
    /// own wallpaper).
    pub wallpaper: String,
    /// Lyrics effect everywhere: "words" (Big Words) | "lens" | "stars" |
    /// "terminal" | "aura" | "lines".
    pub lyric_style: String,
    /// The now-playing view on the lock screen too (with the clock).
    pub lock_screen: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            floating: false,
            live_lyrics: true,
            hide_idle: true,
            glow: true,
            auto_full_minutes: 3,
            notch_hidden: false,
            notch_offset: 0.0,
            monitor: String::new(),
            glitter: "off".into(),
            visualizer: true,
            glass: false,
            wallpaper: "lyrics".into(),
            lyric_style: "words".into(),
            lock_screen: true,
        }
    }
}

pub struct AppState {
    pub media_tx: Mutex<Sender<MediaCmd>>,
    pub hit: Mutex<hover::Rect>,
    pub lyrics_cache: Mutex<HashMap<String, lyrics::LyricsResult>>,
    pub art_cache: Mutex<HashMap<String, Option<String>>>,
    /// one cover lookup at a time (both windows ask for the same song)
    pub art_lock: tokio::sync::Mutex<()>,
    pub http: reqwest::Client,
    pub settings: Mutex<Settings>,
    /// Something is playing right now (set by the media watcher).
    pub playing: AtomicBool,
    /// The island window moved – hover detection must re-read its position.
    pub geo_dirty: AtomicBool,
    /// The notch is being dragged – keep the window interactive.
    pub dragging: AtomicBool,
    /// Zoom applied to the island so it fits small / heavily-scaled screens.
    pub zoom: Mutex<f64>,
    /// System names of the monitors listed in the tray, in menu order.
    pub monitors: Mutex<Vec<String>>,
    pub tray_items: Mutex<HashMap<String, CheckMenuItem<Wry>>>,
    /// Players that macOS won't let us control (user said no to Automation).
    pub denied: Mutex<Vec<String>>,
    /// App that was in front before the full-screen view took the focus.
    pub prev_front: AtomicI32,
    /// Where the desktop object is (CSS px in the wallpaper window).
    pub wall_hit: Mutex<hover::Rect>,
    /// The wallpaper window moved or was resized.
    pub wall_dirty: AtomicBool,
    /// The clickable parts of the lock-screen view (CSS px in its window).
    pub lock_hit: Mutex<Vec<hover::Rect>>,
    /// Location and forecast for the island's weather page.
    pub weather: weather::WeatherCache,
}

fn settings_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("settings.json"))
}

fn load_settings(app: &AppHandle) -> Settings {
    let mut s: Settings = settings_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    // earlier versions kept the lyrics style in the wallpaper setting
    if matches!(s.wallpaper.as_str(), "words" | "stars" | "lines") {
        s.lyric_style = std::mem::replace(&mut s.wallpaper, "lyrics".into());
    }
    s
}

fn save_settings(app: &AppHandle, s: &Settings) {
    if let Some(p) = settings_path(app) {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, serde_json::to_string_pretty(s).unwrap_or_default());
    }
}

pub fn update_setting(app: &AppHandle, f: impl FnOnce(&mut Settings)) {
    let state = app.state::<AppState>();
    let s = {
        let mut s = state.settings.lock().unwrap();
        f(&mut s);
        s.clone()
    };
    save_settings(app, &s);
    let _ = app.emit("settings", s);
}

/// NEEDLEX_DEBUG is set: log what's going on to stderr.
pub fn debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("NEEDLEX_DEBUG").is_some())
}

// ---------- commands ----------

/// Lets the pages log (NEEDLEX_DEBUG only).
#[tauri::command]
fn debug_log(window: tauri::WebviewWindow, msg: String) {
    if debug() {
        eprintln!("[{}] {msg}", window.label());
    }
}

#[tauri::command]
fn media_control(state: State<AppState>, action: String) {
    let cmd = match action.as_str() {
        "toggle" => MediaCmd::Toggle,
        "next" => MediaCmd::Next,
        "prev" => MediaCmd::Prev,
        _ => return,
    };
    let _ = state.media_tx.lock().unwrap().send(cmd);
}

#[tauri::command]
fn media_seek(state: State<AppState>, ms: f64) {
    let _ = state.media_tx.lock().unwrap().send(MediaCmd::Seek(ms.max(0.0)));
}

#[tauri::command]
fn media_resend(state: State<AppState>) {
    let _ = state.media_tx.lock().unwrap().send(MediaCmd::Resend);
}

#[tauri::command]
fn set_hit_rect(state: State<AppState>, x: f64, y: f64, w: f64, h: f64) {
    *state.hit.lock().unwrap() = hover::Rect { x, y, w, h };
}

#[tauri::command]
fn set_wall_hit(state: State<AppState>, x: f64, y: f64, w: f64, h: f64) {
    *state.wall_hit.lock().unwrap() = hover::Rect { x, y, w, h };
}

#[tauri::command]
fn set_lock_hit(state: State<AppState>, rects: Vec<hover::Rect>) {
    *state.lock_hit.lock().unwrap() = rects;
}

/// The Aa button in the full-screen view.
#[tauri::command]
fn set_lyric_style(app: AppHandle, style: String) {
    if LYRIC_CHOICES.iter().any(|(id, _)| *id == style) {
        apply_lyric_style(&app, style);
    }
}

fn apply_lyric_style(app: &AppHandle, style: String) {
    update_setting(app, |s| s.lyric_style = style.clone());
    let state = app.state::<AppState>();
    let items = state.tray_items.lock().unwrap();
    for (id, _) in LYRIC_CHOICES {
        if let Some(i) = items.get(&format!("lyr_{id}")) {
            let _ = i.set_checked(id == style);
        }
    }
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings.lock().unwrap().clone()
}

#[tauri::command]
fn open_full(app: AppHandle) {
    show_full(&app, false);
}

#[tauri::command]
fn close_full(app: AppHandle) {
    if let Some(full) = app.get_webview_window("full") {
        let _ = full.hide();
    }
    let state = app.state::<AppState>();
    let hidden = state.settings.lock().unwrap().notch_hidden;
    if let (Some(main), false) = (app.get_webview_window("main"), hidden) {
        let _ = main.show();
        state.geo_dirty.store(true, Ordering::Relaxed);
    }
    // hand the keyboard back to the app you were using
    let pid = state.prev_front.swap(0, Ordering::Relaxed);
    let _ = app.run_on_main_thread(move || mac::activate_pid(pid));
}

/// Hide / show the notch (button in the island, or the tray).
#[tauri::command]
fn set_notch_hidden(app: AppHandle, hidden: bool) {
    apply_notch_hidden(&app, hidden);
}

/// Start dragging the notch sideways (tracked natively until the button is released).
#[tauri::command]
fn drag_notch_start(app: AppHandle) {
    display::drag(app);
}

// ---------- windows ----------

/// Opens the full-screen view on the notch's monitor. `auto` = idle timer.
pub fn show_full(app: &AppHandle, auto: bool) {
    let Some(full) = app.get_webview_window("full") else { return };
    if !full.is_visible().unwrap_or(false) {
        let a = app.clone();
        let _ = app.run_on_main_thread(move || {
            let pid = mac::frontmost_pid();
            if pid > 0 {
                a.state::<AppState>().prev_front.store(pid, Ordering::Relaxed);
            }
        });
    }
    display::fit_full(app);
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide();
    }
    let _ = full.show();
    // keyboard to the view (Esc, Space, arrows) without activating NeedleX
    let f2 = full.clone();
    let _ = app.run_on_main_thread(move || {
        if let Ok(w) = f2.ns_window() {
            mac::focus_webview(w);
        }
    });
    let _ = app.emit_to("full", "full:open", serde_json::json!({ "auto": auto }));
    let _ = app.state::<AppState>().media_tx.lock().unwrap().send(MediaCmd::Resend);
}

fn apply_notch_hidden(app: &AppHandle, hidden: bool) {
    update_setting(app, |s| s.notch_hidden = hidden);
    let full_open = app
        .get_webview_window("full")
        .and_then(|f| f.is_visible().ok())
        .unwrap_or(false);
    if let Some(main) = app.get_webview_window("main") {
        if hidden {
            let _ = main.hide();
        } else if !full_open {
            let _ = main.show();
            app.state::<AppState>().geo_dirty.store(true, Ordering::Relaxed);
        }
    }
    if let Some(i) = app.state::<AppState>().tray_items.lock().unwrap().get("hide_notch") {
        let _ = i.set_checked(hidden);
    }
}

fn create_full_window(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, "full", WebviewUrl::App("full.html".into()))
        .title("NeedleX Now Playing")
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .focused(false)
        .inner_size(800.0, 600.0)
        .build()?;
    if let Some(full) = app.get_webview_window("full") {
        if let Ok(w) = full.ns_window() {
            mac::setup_full_window(w);
        }
    }
    Ok(())
}

/// Puts the live wallpaper on the desktop – or takes it off – to match the setting.
fn apply_wall(app: &AppHandle) {
    let state = app.state::<AppState>();
    let look = state.settings.lock().unwrap().wallpaper.clone();
    *state.wall_hit.lock().unwrap() = hover::Rect::default();
    let Some(page) = wall_page(&look) else {
        // hidden, not closed: it is a panel (see mac.rs)
        if let Some(w) = app.get_webview_window("wall") {
            let _ = w.hide();
        }
        return;
    };
    let win = match app.get_webview_window("wall") {
        Some(w) => {
            // lyrics and objects are drawn by different pages
            if let Ok(url) = w.url() {
                if !url.path().ends_with(page) {
                    if let Ok(u) = url.join(page) {
                        let _ = w.navigate(u);
                    }
                }
            }
            w
        }
        None => match create_wall_window(app, page) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("NeedleX: wallpaper window failed: {e}");
                return;
            }
        },
    };
    let objects = wall_is_object(&look);
    let w2 = win.clone();
    let _ = app.run_on_main_thread(move || {
        if let Ok(ns) = w2.ns_window() {
            mac::set_wall_level(ns, objects);
        }
    });
    let _ = win.set_ignore_cursor_events(true);
    display::fit_wall(app);
    let _ = win.show();
}

fn create_wall_window(app: &AppHandle, page: &str) -> tauri::Result<tauri::WebviewWindow> {
    let win = WebviewWindowBuilder::new(app, "wall", WebviewUrl::App(page.into()))
        .title("NeedleX Wallpaper")
        .transparent(true)
        .decorations(false)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .visible(false)
        .focused(false)
        .focusable(false)
        .accept_first_mouse(true)
        .inner_size(800.0, 600.0)
        .build()?;
    if let Ok(ns) = win.ns_window() {
        mac::setup_wall_window(ns);
    }
    let _ = win.set_ignore_cursor_events(true);
    Ok(win)
}

// ---------- tray ----------

fn idle_label(m: u32) -> String {
    match m {
        0 => "Off".into(),
        1 => "After 1 minute".into(),
        m => format!("After {m} minutes"),
    }
}

/// Builds the tray menu from the current settings and display setup.
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let s = app.state::<AppState>().settings.lock().unwrap().clone();
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);
    let check = |id: &str, label: &str, on: bool| CheckMenuItemBuilder::with_id(id, label).checked(on).build(app);

    let hide_notch = check("hide_notch", "Hide notch", s.notch_hidden)?;
    let floating = check("floating", "Floating island (pill shape)", s.floating)?;
    let live = check("live", "Live lyrics under the island", s.live_lyrics)?;
    let hide = check("hide", "Hide when nothing is playing", s.hide_idle)?;
    let glow = check("glow", "Beat glow", s.glow)?;
    let auto = check("autostart", "Launch at login", autostart_on)?;
    let vis = check("visualizer", "Real audio visualizer", s.visualizer)?;
    let glass = check("glass", "Liquid glass (uses Screen Recording)", s.glass)?;
    let lockscreen = check("lockscreen", "Also on the lock screen", s.lock_screen)?;

    let mut items: Vec<CheckMenuItem<Wry>> = vec![
        hide_notch.clone(), floating.clone(), live.clone(), hide.clone(), glow.clone(), auto.clone(),
        vis.clone(), glass.clone(), lockscreen.clone(),
    ];

    let mut idle_menu = SubmenuBuilder::new(app, "Full screen when idle");
    for m in IDLE_CHOICES {
        let it = check(&format!("idle_{m}"), &idle_label(m), s.auto_full_minutes == m)?;
        idle_menu = idle_menu.item(&it);
        items.push(it);
    }
    let idle_menu = idle_menu.build()?;

    // which display the notch lives on
    let mons = display::monitors(app);
    let prim = app.primary_monitor().ok().flatten().and_then(|m| m.name().cloned()).unwrap_or_default();
    let names: Vec<String> = mons.iter().map(|m| m.name().cloned().unwrap_or_default()).collect();
    let chosen_exists = names.iter().any(|n| *n == s.monitor);
    let mut disp_menu = SubmenuBuilder::new(app, "Show notch on");
    let main_item = check("mon_main", "Main display", s.monitor.is_empty() || !chosen_exists)?;
    disp_menu = disp_menu.item(&main_item).separator();
    items.push(main_item);
    for (i, m) in mons.iter().enumerate() {
        let name = &names[i];
        let it = check(&format!("mon_{i}"), &display::monitor_label(i, m, *name == prim), chosen_exists && *name == s.monitor)?;
        disp_menu = disp_menu.item(&it);
        items.push(it);
    }
    let disp_menu = disp_menu.build()?;

    // glitter in the full-screen view
    let mut glit_menu = SubmenuBuilder::new(app, "Glitter (full screen)");
    for (id, label) in GLITTER_CHOICES {
        let it = check(&format!("glitter_{id}"), label, s.glitter == id)?;
        glit_menu = glit_menu.item(&it);
        items.push(it);
    }
    let glit_menu = glit_menu.build()?;

    // live desktop wallpaper
    let mut wall_menu = SubmenuBuilder::new(app, "Desktop wallpaper");
    for (id, label) in WALL_CHOICES {
        let it = check(&format!("wall_{id}"), label, s.wallpaper == id)?;
        wall_menu = wall_menu.item(&it);
        if id == "off" || id == "lyrics" {
            wall_menu = wall_menu.separator();
        }
        items.push(it);
    }
    let wall_menu = wall_menu.separator().item(&lockscreen).build()?;

    // lyrics effect (desktop, lock screen, full screen)
    let mut lyr_menu = SubmenuBuilder::new(app, "Lyrics effect");
    for (id, label) in LYRIC_CHOICES {
        let it = check(&format!("lyr_{id}"), label, s.lyric_style == id)?;
        lyr_menu = lyr_menu.item(&it);
        if id == "aura" {
            lyr_menu = lyr_menu.separator();
        }
        items.push(it);
    }
    let lyr_menu = lyr_menu.build()?;

    let open_full_item = MenuItemBuilder::with_id("open_full", "Open full screen").build(app)?;
    let recenter = MenuItemBuilder::with_id("recenter", "Re-center notch").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit NeedleX").accelerator("Cmd+Q").build(app)?;

    let mut menu = MenuBuilder::new(app);
    // macOS said no to reading a player: offer the way to fix it
    let denied = app.state::<AppState>().denied.lock().unwrap().clone();
    if !denied.is_empty() {
        let names = denied.iter().map(|d| if d == "Music" { "Apple Music" } else { d.as_str() }).collect::<Vec<_>>().join(" and ");
        let fix = MenuItemBuilder::with_id("perm_automation", format!("⚠︎ Allow NeedleX to control {names}…")).build(app)?;
        menu = menu.item(&fix).separator();
    }
    let menu = menu
        .items(&[&hide_notch, &recenter, &open_full_item])
        .item(&disp_menu)
        .separator()
        .item(&wall_menu)
        .item(&lyr_menu)
        .item(&idle_menu)
        .item(&glit_menu)
        .separator()
        .items(&[&floating, &live, &hide, &glow, &vis, &glass])
        .separator()
        .item(&auto)
        .separator()
        .item(&quit)
        .build()?;

    let state = app.state::<AppState>();
    *state.monitors.lock().unwrap() = names;
    let mut map = state.tray_items.lock().unwrap();
    map.clear();
    for it in items {
        map.insert(it.id().as_ref().to_string(), it);
    }
    Ok(menu)
}

/// Rebuilds the tray menu (e.g. after monitors were plugged in / removed).
pub fn rebuild_menu(app: &AppHandle) {
    if let (Some(tray), Ok(menu)) = (app.tray_by_id("tray"), build_menu(app)) {
        let _ = tray.set_menu(Some(menu));
    }
}

fn on_menu(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    let item = state.tray_items.lock().unwrap().get(id).cloned();
    match id {
        "quit" => app.exit(0),
        "open_full" => show_full(app, false),
        "recenter" => {
            update_setting(app, |s| s.notch_offset = 0.0);
            display::place_island(app, 0.0);
            if state.settings.lock().unwrap().notch_hidden {
                apply_notch_hidden(app, false);
            }
        }
        "hide_notch" => {
            let hidden = !state.settings.lock().unwrap().notch_hidden;
            apply_notch_hidden(app, hidden);
        }
        "autostart" => {
            let al = app.autolaunch();
            let on = !al.is_enabled().unwrap_or(false);
            let _ = if on { al.enable() } else { al.disable() };
            if let Some(i) = item {
                let _ = i.set_checked(al.is_enabled().unwrap_or(on));
            }
        }
        "perm_automation" => mac::open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Automation"),
        "floating" | "live" | "hide" | "glow" | "visualizer" | "glass" | "lockscreen" => {
            let mut new_val = false;
            update_setting(app, |s| {
                let field = match id {
                    "floating" => &mut s.floating,
                    "live" => &mut s.live_lyrics,
                    "hide" => &mut s.hide_idle,
                    "visualizer" => &mut s.visualizer,
                    "glass" => &mut s.glass,
                    "lockscreen" => &mut s.lock_screen,
                    _ => &mut s.glow,
                };
                *field = !*field;
                new_val = *field;
            });
            if let Some(i) = item {
                let _ = i.set_checked(new_val);
            }
            if id == "visualizer" {
                helper::set_audio(new_val);
            }
        }
        other if other.starts_with("idle_") => {
            let m: u32 = other[5..].parse().unwrap_or(0);
            update_setting(app, |s| s.auto_full_minutes = m);
            let items = state.tray_items.lock().unwrap();
            for c in IDLE_CHOICES {
                if let Some(i) = items.get(&format!("idle_{c}")) {
                    let _ = i.set_checked(c == m);
                }
            }
        }
        other if other.starts_with("wall_") => {
            let look = other["wall_".len()..].to_string();
            update_setting(app, |s| s.wallpaper = look.clone());
            {
                let items = state.tray_items.lock().unwrap();
                for (id, _) in WALL_CHOICES {
                    if let Some(i) = items.get(&format!("wall_{id}")) {
                        let _ = i.set_checked(id == look);
                    }
                }
            }
            apply_wall(app);
        }
        other if other.starts_with("lyr_") => {
            apply_lyric_style(app, other["lyr_".len()..].to_string());
        }
        other if other.starts_with("glitter_") => {
            let g = other["glitter_".len()..].to_string();
            update_setting(app, |s| s.glitter = g.clone());
            let items = state.tray_items.lock().unwrap();
            for (id, _) in GLITTER_CHOICES {
                if let Some(i) = items.get(&format!("glitter_{id}")) {
                    let _ = i.set_checked(id == g);
                }
            }
        }
        other if other.starts_with("mon_") => {
            let name = if other == "mon_main" {
                String::new()
            } else {
                let i: usize = other[4..].parse().unwrap_or(usize::MAX);
                state.monitors.lock().unwrap().get(i).cloned().unwrap_or_default()
            };
            // a new screen starts centred
            update_setting(app, |s| {
                s.monitor = name;
                s.notch_offset = 0.0;
            });
            let a = app.clone();
            std::thread::spawn(move || {
                display::relayout(&a);
                rebuild_menu(&a);
            });
        }
        _ => {}
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;
    let icon = Image::from_bytes(include_bytes!("../icons/tray.png"))?;
    TrayIconBuilder::with_id("tray")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("NeedleX")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, ev| on_menu(app, ev.id().as_ref()))
        .build(app)?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .invoke_handler(tauri::generate_handler![
            media_control,
            media_seek,
            media_resend,
            set_hit_rect,
            set_wall_hit,
            set_lock_hit,
            set_lyric_style,
            debug_log,
            get_settings,
            open_full,
            close_full,
            set_notch_hidden,
            drag_notch_start,
            lyrics::fetch_lyrics,
            art::hires_art,
            weather::get_weather,
            weather::get_calendar,
            art::capture_backdrop
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let settings = load_settings(&handle);
            let (tx, rx) = mpsc::channel();
            app.manage(AppState {
                media_tx: Mutex::new(tx),
                hit: Mutex::new(hover::Rect::default()),
                lyrics_cache: Mutex::new(HashMap::new()),
                art_cache: Mutex::new(HashMap::new()),
                art_lock: tokio::sync::Mutex::new(()),
                http: lyrics::client(),
                settings: Mutex::new(settings.clone()),
                playing: AtomicBool::new(false),
                geo_dirty: AtomicBool::new(true),
                dragging: AtomicBool::new(false),
                zoom: Mutex::new(1.0),
                monitors: Mutex::new(Vec::new()),
                tray_items: Mutex::new(HashMap::new()),
                denied: Mutex::new(Vec::new()),
                prev_front: AtomicI32::new(0),
                wall_hit: Mutex::new(hover::Rect::default()),
                wall_dirty: AtomicBool::new(true),
                lock_hit: Mutex::new(Vec::new()),
                weather: Default::default(),
            });
            // menu-bar app: no Dock icon, no app menu
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            // the island starts hidden and is shown once launching has
            // finished (RunEvent::Ready), so macOS puts it on every Space –
            // full-screen ones too – as the window of a menu-bar app
            if let Some(main) = handle.get_webview_window("main") {
                if let Ok(w) = main.ns_window() {
                    mac::setup_island_window(w);
                }
            }

            create_full_window(&handle)?;
            build_tray(&handle)?;
            display::spawn(handle.clone()); // first layout + watches display changes
            media::spawn(handle.clone(), rx);
            hover::spawn(handle.clone());
            hover::spawn_wall(handle.clone());
            lock::spawn(handle.clone());
            helper::spawn(handle.clone());
            if std::env::var_os("NEEDLEX_DEBUG").is_some() {
                let h = handle.clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(2));
                    let h2 = h.clone();
                    let _ = h.run_on_main_thread(move || {
                        for label in ["main", "full", "wall", "lock"] {
                            if let Some(w) = h2.get_webview_window(label) {
                                let ns = w.ns_window().unwrap_or(std::ptr::null_mut());
                                eprintln!("[debug] {label}: {} pos={:?} size={:?}", mac::describe_window(ns), w.outer_position().ok(), w.outer_size().ok());
                            }
                        }
                    });
                });
            }
            if let Some(secs) = std::env::var("NEEDLEX_DEBUG_LOCK").ok().and_then(|v| v.parse::<u64>().ok()) {
                // shows the lock-screen window for a few seconds without locking
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    let h2 = h.clone();
                    let _ = h.run_on_main_thread(move || lock::show(&h2));
                    std::thread::sleep(std::time::Duration::from_secs(secs));
                    let h3 = h.clone();
                    let _ = h.run_on_main_thread(move || lock::hide(&h3));
                });
            }
            if std::env::var_os("NEEDLEX_DEBUG_DEMO").is_some() {
                // scripted walk through the views, for checking a build
                let h = handle.clone();
                std::thread::spawn(move || {
                    let wait = |s: u64| std::thread::sleep(std::time::Duration::from_secs(s));
                    wait(4);
                    let _ = h.emit_to("main", "island:hover", true);
                    wait(5);
                    let _ = h.emit_to("main", "island:hover", false);
                    wait(2);
                    show_full(&h, false);
                    wait(7);
                    close_full(h.clone());
                });
            }
            idle::spawn(handle);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building NeedleX")
        .run(|app, event| {
            if let tauri::RunEvent::Ready = event {
                let hidden = app.state::<AppState>().settings.lock().unwrap().notch_hidden;
                if let (Some(main), false) = (app.get_webview_window("main"), hidden) {
                    let _ = main.show();
                    app.state::<AppState>().geo_dirty.store(true, Ordering::Relaxed);
                }
                apply_wall(app);
            }
        });
}
