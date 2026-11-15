# Security policy

## Supported versions

Only the latest code on `main` is supported. Please check that a problem still happens there
before reporting it.

## Reporting a vulnerability

Please **don't open a public issue** for security problems. Report them privately instead:
open this repository on GitHub and go to **Security → Advisories → Report a vulnerability**.

Please include:

- what the problem is and what someone could do with it,
- the steps to reproduce it (macOS version, NeedleX commit),
- any idea you have for a fix.

You'll get a reply as soon as possible, normally within a week. Once a fix is ready, it will be
released and you'll be credited in the advisory if you'd like to be.

## Scope

NeedleX runs locally. It reads the current song from Spotify / Music, asks for optional
permissions (system audio level, screen area for the glass look, location, calendars) and talks
to LRCLIB, iTunes / Deezer, Open-Meteo and Apple's geocoder. Anything that could expose data
beyond what the README's privacy section describes is in scope. Examples are a page loading
remote code, data leaving the Mac unexpectedly, or a permission being misused.

The lock-screen view uses a private macOS framework (SkyLight). Problems in macOS itself should be
reported to Apple.
