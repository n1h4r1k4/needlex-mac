# Contributing to NeedleX

Thanks for wanting to help! Bug reports, ideas, docs fixes and code are all welcome.

## Ways to help

- **Report a bug.** Open an issue with the *Bug report* template. Include your macOS version, the
  player (Spotify / Music) and, if you can, a log (see [Debugging](#debugging)).
- **Suggest a feature.** Use the *Feature request* template, and describe what you'd like to see.
  A screenshot or a sketch helps.
- **Send a pull request.** For anything bigger than a small fix, please open an issue first so we
  can agree on the approach.

Please be kind and constructive. Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## Development setup

You need macOS 13 or later, the Xcode Command Line Tools, Rust (stable) and Node.js 18+.

```bash
git clone https://github.com/n1h4r1k4/needlex-mac.git
cd needlex-mac
npm install
./scripts/make-signing-identity.sh   # once: keeps macOS permissions across rebuilds
npm run dev                          # run with live reload of the pages
npm run build                        # release build + local signing
```

The build also compiles the Swift companion `src-tauri/helper/helper.swift`, which is bundled as
`NeedleX.app/Contents/MacOS/needlex-helper`.

Run the tests with:

```bash
cd src-tauri && cargo test
```

## Previewing the UI in a browser

The pages in `src/` run in any browser without the app. They then play a made-up demo song
(`src/demo.js`), so you can work on the look without Spotify. Open the files directly, for
example `open "src/full.html?mode=lyrics&style=stars"`.

| Page | Parameters |
| --- | --- |
| `index.html` (island) | `view=expanded\|lyrics\|peek`, `page=weather\|calendar` |
| `full.html` (full screen / desktop / lock screen) | `mode=lyrics`, `style=words\|lens\|stars\|terminal\|aura\|lines`, `bg=simple\|glass\|liquid\|foggy`, `cover=1`, `wall`, `lock`, `lk=record\|cover\|lyrics`, `t=<ms>` (song position), `nolyr=1`, `glitter=white\|multi\|album`, `snap=1` (skip transitions) |
| `desk.html` (desktop objects) | `look=turntable\|sleeve\|aura`, `paused` |

The README screenshots are made from these pages with `./scripts/screenshots.sh`. On its own,
the script uses the demo song. To use real songs, first make a song file for each one with
`scripts/screenshots/song.py`, which takes a cover URL and an LRCLIB id. Then pass the files as
`SONG_A=a.js SONG_B=b.js ./scripts/screenshots.sh`. Keep those song files out of the repository.

## Releasing

Installed copies of NeedleX update themselves from the latest GitHub release. They only accept an
update that is signed with the updater key, which the maintainer keeps in `~/.needlex-signing/`
(`updater.key` and `updater-password`, made with `npx tauri signer generate`).

1. Set the new version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`.
2. Rename *Unreleased* in `CHANGELOG.md` to `## <version> – <date>`. This becomes the update's
   notes. You can also add `docs/releases/v<version>.md` for the release page.
3. Commit and push `main`.
4. Run `npm run release`. It builds the universal app, disk image, zip and signed update in
   `dist/`, then publishes the GitHub release `v<version>`. It checks everything it needs before
   the build starts.

Installed copies offer the new version within a few hours, or right away with *Check for
Updates…*. The *Release* workflow can also build releases on GitHub when a version tag is pushed,
but only if the repository has the updater key as the secrets `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.

## Code style

- **JavaScript / CSS / HTML:** plain files with no framework and no bundler. Keep them that way.
  Use two-space indents and single quotes, and add a short comment explaining *why* when something
  isn't obvious.
- **Rust:** run `cargo fmt` and `cargo clippy` in `src-tauri`. Keep macOS glue in `mac.rs`, and
  avoid `unwrap()` on anything that comes from outside the app (players, network, files).
- **Swift helper:** a single file that talks over stdin / stdout. If you add a command, document
  it in the header comment.
- **Performance matters.** NeedleX runs all day. Avoid timers and redraws when nothing is visible
  or playing, and test with Activity Monitor open.
- **Privacy matters.** Don't add analytics, and don't send anything new to a server without
  mentioning it in the README's privacy section.

## Pull requests

1. Fork the repo and create a branch from `main`.
2. Keep each pull request focused on one change. Update the README if you change behaviour or
   menus.
3. Make sure `cargo test` passes and the app builds (`npm run build`).
4. Describe what you changed and how you tested it. Add screenshots or a short recording for
   visual changes.

Commit messages: a short summary line (under about 70 characters), then a blank line and the
details if needed.

## Debugging

```bash
pkill -x needlex
open --env NEEDLEX_DEBUG=1 --stderr /tmp/needlex.log /Applications/NeedleX.app
```

| Variable | Effect |
| --- | --- |
| `NEEDLEX_DEBUG=1` | Logs player snapshots, the window state and each lyrics view |
| `NEEDLEX_DEBUG_DEMO=1` | Walks through the expanded and full-screen views once |
| `NEEDLEX_DEBUG_LOCK=<seconds>` | Shows the lock-screen view for a few seconds without locking |
| `NEEDLEX_DEBUG_WEATHER=1` | Looks up the weather at start and logs the result |

## Releasing (maintainers)

1. Bump the version in `src-tauri/tauri.conf.json`, `src-tauri/Cargo.toml` and `package.json`.
2. Add the version to `CHANGELOG.md`. You can also write full release notes in
   `docs/releases/v<version>.md`, with the title on the first line as `<!-- title: … -->`.
3. Commit, then tag and push: `git tag -a v<version> -m "NeedleX <version>" && git push origin v<version>`.

The *Release* workflow builds the app for Apple silicon and Intel (`scripts/release.sh`) and
publishes the disk image, the zip and their checksums as a GitHub release. To build the same files
locally, run `./scripts/release.sh`; they end up in `dist/`.
