<div align="center">

<img src="src-tauri/icons/128x128@2x.png" width="96" alt="NeedleX icon" />

# NeedleX

**A music island for the Mac notch, with lyric effects on your desktop and lock screen.**

NeedleX follows what's playing in Spotify or Apple Music. It shows it in a dynamic island around
the notch, turns the lyrics into animated effects on your desktop wallpaper, and puts a record
player on your lock screen.

![macOS 13+](https://img.shields.io/badge/macOS-13%2B-black?logo=apple)
![Tauri 2](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-stable-B7410E?logo=rust)
![Swift helper](https://img.shields.io/badge/Swift-helper-F05138?logo=swift&logoColor=white)
[![CI](https://github.com/n1h4r1k4/needlex-mac/actions/workflows/ci.yml/badge.svg)](https://github.com/n1h4r1k4/needlex-mac/actions/workflows/ci.yml)

<img src="docs/images/fx-words.jpg" width="900" alt="Big Words lyrics effect in full screen" />

</div>

> The screenshots use a made-up demo song ("City Lights" by *NeedleX Demo*) with an original
> cover and lyrics. Every page can be opened in a browser to see this demo
> (see [CONTRIBUTING.md](CONTRIBUTING.md#previewing-the-ui-in-a-browser)).

## Contents

- [Features](#features)
- [Requirements](#requirements)
- [Install](#install)
- [Using NeedleX](#using-needlex)
- [Permissions and privacy](#permissions-and-privacy)
- [Troubleshooting](#troubleshooting)
- [How it works](#how-it-works)
- [Project layout](#project-layout)
- [Contributing](#contributing)

## Features

### The island

A black island grows out of the notch while music plays. It shows the cover, title, artist and a
visualizer driven by the real audio level. Point at it to open it.

<table>
  <tr>
    <td width="50%"><img src="docs/images/isl-player.jpg" alt="Island player" /><br /><sub><b>Player</b>: scrub, play / pause, skip</sub></td>
    <td width="50%"><img src="docs/images/isl-lyrics.jpg" alt="Island lyrics" /><br /><sub><b>Synced lyrics</b>: click a line to jump to it</sub></td>
  </tr>
  <tr>
    <td><img src="docs/images/isl-weather.jpg" alt="Island weather page" /><br /><sub><b>Weather</b>: now, today's high / low, the next hours</sub></td>
    <td><img src="docs/images/isl-calendar.jpg" alt="Island calendar page" /><br /><sub><b>Calendar</b>: the month and today's events</sub></td>
  </tr>
</table>

- **Pages:** swipe sideways with two fingers in the open island, or click the dots, to switch
  between Player, Weather and Calendar. With nothing playing, pointing at the notch opens Weather
  and Calendar.
- **Peek:** a short "Now playing" banner when the song changes.
  <br /><img src="docs/images/isl-peek.jpg" width="560" alt="Now playing peek" />
- **Two shapes:** the classic notch, or a floating pill.
- **Liquid glass:** an optional look in which the island bends the desktop behind it.
- **Move it:** grab the island's left or right edge and drag. It snaps back to the centre like a
  magnet.
- **Hide it:** use the eye button, or tray → *Hide notch*.
- **Never steals focus:** clicks pass through everywhere except the island itself.

### Lyric effects

Click the cover in full screen, or pick *Lyrics* as your desktop wallpaper, and the lyrics play
word by word in the effect you choose (tray → *Lyrics effect*). The words take the cover's colours.

<table>
  <tr>
    <td width="33%"><img src="docs/images/fx-words.jpg" alt="Big Words" /><br /><sub><b>Big Words</b>: one big word at a time; some words get a little picture</sub></td>
    <td width="33%"><img src="docs/images/fx-lens.jpg" alt="Lens" /><br /><sub><b>Lens</b>: the next word waits in a label</sub></td>
    <td width="33%"><img src="docs/images/fx-stars.jpg" alt="Stars" /><br /><sub><b>Stars</b>: each line becomes a constellation</sub></td>
  </tr>
  <tr>
    <td><img src="docs/images/fx-terminal.jpg" alt="Terminal" /><br /><sub><b>Terminal</b>: typed in monospace with a red cursor</sub></td>
    <td><img src="docs/images/fx-aura.jpg" alt="Aura" /><br /><sub><b>Aura</b>: the cover melted into a glow</sub></td>
    <td><img src="docs/images/fx-lines.jpg" alt="Scrolling lines" /><br /><sub><b>Scrolling lines</b>: the whole song next to the cover</sub></td>
  </tr>
</table>

**Backgrounds** (tray → *Lyrics effect* → *Background*): *Simple* is each effect's own look. The
other three show the cover:

<table>
  <tr>
    <td width="33%"><img src="docs/images/bg-glass.jpg" alt="Glass background" /><br /><sub><b>Glass</b>: behind frosted glass</sub></td>
    <td width="33%"><img src="docs/images/bg-liquid.jpg" alt="Liquid background" /><br /><sub><b>Liquid</b>: slowly drifting liquid drops</sub></td>
    <td width="33%"><img src="docs/images/bg-foggy.jpg" alt="Foggy background" /><br /><sub><b>Foggy</b>: in drifting mist</sub></td>
  </tr>
</table>

**Show album cover** (same menu) keeps the cover and a player card on the left, with the lyrics on
the right:

<img src="docs/images/with-cover.jpg" width="640" alt="Terminal effect with the album cover beside it" />

Songs without synced lyrics still get an effect. Plain lyrics are spread across the song;
instrumentals show the title and artist.

### Desktop wallpaper

Tray → *Desktop wallpaper*:

- **Lyrics:** while music plays, the desktop shows the lyrics in your chosen effect, behind your
  windows and under your desktop icons, on every Space. Your own wallpaper comes back half a
  minute after the music stops.
- **Turntable / Record sleeve / Cover with aura:** an object sits on your own wallpaper. Drag it
  anywhere, or click it to play or pause.

<table>
  <tr>
    <td width="50%"><img src="docs/images/wall-words.jpg" alt="Lyrics on the desktop" /><br /><sub>Big Words on your own wallpaper</sub></td>
    <td width="50%"><img src="docs/images/desk-turntable.jpg" alt="Turntable on the desktop" /><br /><sub>Turntable</sub></td>
  </tr>
  <tr>
    <td><img src="docs/images/desk-sleeve.jpg" alt="Record sleeve on the desktop" /><br /><sub>Record sleeve</sub></td>
    <td><img src="docs/images/desk-aura.jpg" alt="Cover with aura on the desktop" /><br /><sub>Cover with aura</sub></td>
  </tr>
</table>

### Lock screen

Tray → *Desktop wallpaper* → *Also on the lock screen*. While music plays, a record turns under
the clock, with a player card so you can play, pause and skip without unlocking. Click the record
for the big cover, then click again for the lyrics in your effect. Start typing your password and
NeedleX fades away.

<table>
  <tr>
    <td width="33%"><img src="docs/images/lock-record.jpg" alt="Lock screen record" /><br /><sub>Record and player card</sub></td>
    <td width="33%"><img src="docs/images/lock-cover.jpg" alt="Lock screen cover" /><br /><sub>Click: the big cover</sub></td>
    <td width="33%"><img src="docs/images/lock-lyrics.jpg" alt="Lock screen lyrics" /><br /><sub>Click again: the lyrics</sub></td>
  </tr>
</table>

### Full screen

Click the cover in the open island, or let NeedleX open by itself after a few idle minutes (tray →
*Full screen when idle*; it never opens over games, video or presentations).

<img src="docs/images/cover.jpg" width="640" alt="Full-screen cover view" />

| Key | Action |
| --- | --- |
| <kbd>Space</kbd> | Play / pause |
| <kbd>←</kbd> / <kbd>→</kbd> | Previous / next song |
| <kbd>Enter</kbd> | Switch between the cover and the lyrics |
| <kbd>Esc</kbd> | Leave full screen (or triple-click on the left or right edge) |

### Extras

- **Beat glow:** a thin light around the screen edge that pulses in the cover's colours.
- **Glitter:** sparkles in full screen, in white, many colours or the cover's colours.
- **Hi-res covers:** up to 2000 px, looked up on iTunes / Deezer when a player only gives a small one.
- **Any screen:** NeedleX follows display changes. Tray → *Show notch on* picks the monitor.

## Requirements

- **macOS 13 Ventura or later**, on Apple silicon or Intel. Two extras need a newer macOS: liquid
  glass needs macOS 14, and the real audio visualizer needs macOS 14.2.
- **Spotify** or **Apple Music** (the Music app). Browser tabs and other players can't be followed,
  because macOS has no public "now playing" interface.
- To build it you need the free Xcode Command Line Tools, Rust and Node.js.

## Install

There's no signed download yet, so you build NeedleX yourself. It takes a few minutes.

```bash
# 1. tools (once)
xcode-select --install                                           # Swift + clang
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # Rust
brew install node                                                # or any Node.js 18+

# 2. get the code
git clone https://github.com/n1h4r1k4/needlex-mac.git
cd needlex-mac
npm install

# 3. a free, local signing identity (once – keeps macOS permissions across rebuilds)
./scripts/make-signing-identity.sh

# 4. build and install
npm run build
cp -R src-tauri/target/release/bundle/macos/NeedleX.app /Applications/
open /Applications/NeedleX.app
```

NeedleX is a menu-bar app: it has no Dock icon, and everything is in its tray menu at the top right.

To update later, run `git pull`, `npm run build` and copy the app again. To uninstall, quit it from
the tray, then delete `/Applications/NeedleX.app`, `~/Library/Application Support/com.needlex.island`
and `~/Library/Caches/com.needlex.island`.

## Using NeedleX

Everything is in the tray menu:

| Menu | What it does |
| --- | --- |
| Hide notch | Hide the island (untick to bring it back) |
| Floating island (pill shape) | Use a floating pill instead of the notch shape |
| Hide when nothing is playing | Hide the island when no music is playing |
| Beat glow | Show the light around the screen edge |
| Real audio visualizer | Drive the bars and glow with the real sound level |
| Liquid glass | Use the refracting glass look for the island |
| Full screen when idle | Off / 1 / 3 / 5 / 10 / 15 minutes |
| Show notch on | Choose which display shows the island |
| Glitter (full screen) | Off / White / Multicolour / Album colours |
| Desktop wallpaper | Off / Lyrics / Turntable / Record sleeve / Cover with aura, and *Also on the lock screen* |
| Lyrics effect | Pick the effect, its *Background* and *Show album cover* |
| Open full screen · Re-center notch · Launch at login · Quit | |

## Permissions and privacy

NeedleX has no account, no analytics and no tracking. Each permission is asked for the first time
it is needed:

| Permission | Used for | Needed? |
| --- | --- | --- |
| Automation (Spotify / Music) | Reading the song and the play / pause / skip buttons | Yes |
| System Audio Recording | The real visualizer and beat glow. Only the loudness is measured; no audio is kept. | Optional |
| Screen Recording | *Liquid glass* only (a small picture of the area behind the island) | Optional, off by default |
| Location Services | The weather page (rough location, to about 1 km) | Optional |
| Calendars | The calendar page. Events never leave your Mac. | Optional |

NeedleX contacts these services:

- **[LRCLIB](https://lrclib.net)** for lyrics. It sends the song title, artist and length.
- **iTunes Search / Deezer** for high-resolution covers. It sends the title, artist and album.
- **[Open-Meteo](https://open-meteo.com)** for the weather. It sends coordinates rounded to about 1 km.
- **Apple's geocoder** for the name of your town. It sends coordinates.

## Troubleshooting

<details>
<summary><b>macOS keeps asking for a permission after every build</b></summary>

Run `./scripts/make-signing-identity.sh` once and build with `npm run build`. macOS ties
permissions to an app's signature. An ad-hoc signature changes on every build, but the local
identity stays the same. If you were already asked many times, reset NeedleX's permissions once
with `tccutil reset All com.needlex.island`.
</details>

<details>
<summary><b>Nothing shows up when music plays</b></summary>

Check *System Settings → Privacy & Security → Automation → NeedleX*: Spotify and/or Music must be
ticked. The tray menu shows *Allow NeedleX to control …* when access is missing.
</details>

<details>
<summary><b>The weather page says to turn on Location Services</b></summary>

Turn on *System Settings → Privacy & Security → Location Services → NeedleX*.
</details>

<details>
<summary><b>The lock screen doesn't show NeedleX</b></summary>

Make sure *Also on the lock screen* is ticked and music is playing. The lock-screen view uses a
private macOS framework. If a macOS update changes it, the lock screen simply stays as macOS draws
it.
</details>

<details>
<summary><b>Logs</b></summary>

```bash
pkill -x needlex
open --env NEEDLEX_DEBUG=1 --stderr /tmp/needlex.log /Applications/NeedleX.app
tail -f /tmp/needlex.log
```

The log shows the player, the window state and each lyrics view. More switches are listed in
[CONTRIBUTING.md](CONTRIBUTING.md#debugging).
</details>

## How it works

- **App:** [Tauri 2](https://tauri.app). The windows are web pages (plain HTML, CSS and JavaScript,
  with WebGL for the liquid and glow effects). The Rust side handles the players, the windows, the
  tray and the network.
- **Players:** a small JavaScript-for-Automation script asks Spotify and Music for the current song
  through their AppleScript dictionaries.
- **Windows:** non-activating panels above the menu bar (the island), at the desktop level (the
  wallpaper) and above the lock screen (through SkyLight). None of them take focus.
- **Swift helper** (`needlex-helper`): a small companion process. It measures the system audio
  level (Core Audio process tap), takes the liquid-glass picture (ScreenCaptureKit), names your
  town and reads calendar events (EventKit).

## Project layout

```
src/                     the pages (no framework, no bundler)
  index.html main.js     the island
  full.html  full.js     full screen, desktop lyrics and lock screen
  desk.html  desk.js     the desktop objects (turntable, sleeve, aura)
  flow.js  edge.js       WebGL: liquid / flow backgrounds, beat glow
  glass.js glitter.js    liquid glass, glitter
  demo.js                the made-up demo song used in a browser
src-tauri/
  src/main.rs            settings, tray menu, windows, commands
  src/media.rs media.js  Spotify / Music poller
  src/lyrics.rs          LRCLIB search and LRC parser
  src/lock.rs mac.rs     lock screen, AppKit / CoreGraphics glue
  src/location.rs weather.rs   location and weather
  helper/helper.swift    the Swift companion
scripts/                 signing and screenshot scripts
docs/images/             README screenshots
```

## Contributing

Bug reports, ideas and pull requests are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md)
and the [Code of Conduct](CODE_OF_CONDUCT.md) first. Report security problems privately, as
described in [SECURITY.md](SECURITY.md).

NeedleX isn't affiliated with Apple, Spotify, LRCLIB, Deezer or Open-Meteo. Their names belong to
their owners.
