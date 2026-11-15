# NeedleX — a dynamic-island music player (macOS port)

> This is the **macOS** branch of NeedleX. The UI is unchanged; the Windows-only
> parts (SMTC, WASAPI meter, Win32 windows, GDI capture) were replaced with
> macOS equivalents. See [macOS notes](#macos-notes) below.

A black "notch" at the top of the screen that follows whatever is playing:
Spotify, Apple Music, and any browser tab (YouTube Music, Spotify Web,
SoundCloud, YouTube…) in Chrome, Edge, Brave, Firefox and Opera.

- **Auto-detection** through Windows' media sessions (SMTC): title, artist, album, cover art, position, controls
- **Compact pill** with cover, title and a visualizer driven by the real audio level
- **Peek** animation when the song changes
- **Hover to expand**: big cover, album-tinted "black glass", scrubbable progress, prev / play / next
- **Synced lyrics** from LRCLIB: Apple-Music-style scrolling panel (click a line to seek)
  plus a karaoke **live lyric** pill under the island
- **Beat glow** in the album's colour; classic (notch) or floating (pill) shape
- **Full-screen view**: click the album art in the island
  - cover view: serif clock, big cover over a soft glow in the album's colours, frosted card with spinning record, progress and controls
  - click the cover for word-by-word lyrics in two styles (switch with the **Aa** button):
    **Stars** (constellations over the liquid, zoom-streaked cover) and **Lens** (the cover in depth-of-field focus; each word appears made of the cover's stretched pixels, then settles: small previous word above, big word, cream label for the next word below)
  - click again to go back to the cover; **triple-click on the left or right side** (or press Esc) to exit
  - Space = play/pause, ←/→ = previous/next, Enter = switch cover/lyrics
  - preview in a browser: `src/full.html?mode=lyrics&style=stars` (demo song)
- **Liquid glass** island (refracts the desktop behind it; black like a notch at the top, clear glass toward the bottom) and glass card in full screen
- **Hi-res covers** (up to 2000 px) looked up on iTunes / Deezer, since players only hand Windows small thumbnails
- **Auto full screen when idle**: tray → *Full screen when idle* (Off / 1 / 3 / 5 / 10 / 15 min, default 3). Only while music plays, never over games, full-screen video or presentations
- **Hide notch**: eye button in the open notch, or tray → *Hide notch* (untick to bring it back)
- **Move the notch**: point at its left/right edge, grab the grip and drag; it snaps to the centre like a magnet. Tray → *Re-center notch* resets it
- Click-through everywhere except the island; never steals focus
- **Edge light**: a thin line around the whole screen that pulses with the music, with brighter sections circling the edge and comets on strong beats (tray → *Beat glow*)
- **Glitter** in full screen (off by default): tray → *Glitter (full screen)* → White / Multicolour / Album colours. Twinkles with the music, sits under the glass card and lyrics
- **Any screen / scale**: the notch fits small and high-DPI screens, follows display changes, and tray → *Show notch on* picks the monitor
- **Lyrics effects** (tray → *Lyrics effect*, or the Aa button in full screen) – used on the desktop, the
  lock screen and in full screen, words in the cover's colour:
  *Big Words* (on your own wallpaper; some words get a little picture – lights 💡, song ♪, love ♥ …),
  *Lens* (black, the next word waiting in a label), *Stars* (one constellation at a time, centred),
  *Terminal* (typed in monospace with a red cursor), *Aura* (the cover's colours blurred),
  *Scrolling lines* (the whole song next to the cover)
- **Desktop wallpaper** (tray → *Desktop wallpaper*), behind all your windows, on every Space:
  - *Lyrics*: while music plays the desktop shows the lyrics in the chosen effect (the cover while
    paused); your own wallpaper comes back half a minute after the music stops. Desktop icons stay on top
  - *Turntable* / *Record sleeve* / *Cover with aura*: an object on your own wallpaper – drag it
    anywhere, click it to play / pause
- **Lock screen** (tray → *Desktop wallpaper* → *Also on the lock screen*): while music plays the record
  turns under the clock with a player card (play / pause / skip without unlocking). Click the record for
  the big cover, click the cover for the lyrics (in the chosen effect), click again to go back.
  The bottom of the screen stays clear for the password field / Touch ID
- **Island pages**: swipe sideways with two fingers in the open island (or click the dots) –
  *Player* · *Weather* (now, today's high / low, rain coming, the next six hours; Open-Meteo, your rough
  location) · *Calendar* (the month, today's events from the Calendar app). With nothing playing,
  hovering the notch opens Weather and Calendar
- Tray menu: shape, live lyrics, hide when idle, beat glow, launch at startup

## Run / build (macOS)
Needs the Xcode Command Line Tools (`xcode-select --install`), Rust (`rustup`) and Node – all free.
```
npm install
npm run dev      # development
npm run build    # -> src-tauri/target/release/bundle/macos/NeedleX.app
cp -R src-tauri/target/release/bundle/macos/NeedleX.app /Applications/
```
The build also compiles the small Swift companion `src-tauri/helper/helper.swift`
(bundled as `NeedleX.app/Contents/MacOS/needlex-helper`).

## macOS notes
- **Players:** Spotify and Apple Music, read and controlled through their AppleScript
  dictionaries (`src-tauri/src/media.js`). macOS asks once per player
  (*Privacy & Security → Automation*). If you said no, the tray shows
  *Allow NeedleX to control …* which opens that settings page.
  Browser tabs are not supported: macOS has no public "now playing" API.
- **Lock screen:** drawn through a private macOS framework (SkyLight), like other lock-screen music
  players; if a macOS update removes it, the lock screen simply stays as macOS draws it.
- **Permissions:** *Automation* (Spotify / Music) is required. *System Audio Recording*
  powers the real visualizer and beat glow (tray → *Real audio visualizer*; without it
  the bars use a soft fake animation). *Screen Recording* is only used by
  tray → *Liquid glass*, which is **off** by default (the island is a plain black notch).
  *Location Services* (weather) and *Calendars* are asked for the first time you open those island pages.
- **Windows:** both windows are non-activating panels above the menu bar and Dock, on
  every Space including full-screen apps; clicking the island never takes focus away
  from your app. It's a menu-bar app (no Dock icon) – everything is in the tray menu.
- **Debugging:** `open --env NEEDLEX_DEBUG=1 --stderr /tmp/needlex.log NeedleX.app` logs
  window state and player snapshots; adding `--env NEEDLEX_DEBUG_DEMO=1` walks through
  the expanded and full-screen views once.

## Layout
- `src-tauri/src/media.rs` + `media.js` – AppleScript poller for Spotify / Apple Music (session picking, art, controls)
- `src-tauri/src/mac.rs` – CoreGraphics / AppKit glue (cursor, idle time, panels, window levels)
- `src-tauri/src/helper.rs` + `helper/helper.swift` – audio level (Core Audio tap) and liquid-glass snapshots (ScreenCaptureKit)
- `src-tauri/src/lyrics.rs` – LRCLIB search, YouTube title cleanup, LRC parser
- `src-tauri/src/hover.rs` – hover detection and click-through window styles
- `src/` – island UI (HTML/CSS/JS); `full.*` is also the lyrics wallpaper, `desk.*` the desktop objects
