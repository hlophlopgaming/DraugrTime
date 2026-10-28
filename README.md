<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="logo/whiteDraugr128.png">
  <img src="logo/blackDraugr128.png" alt="Draugr Time" width="96" height="96">
</picture>
<picture>
  <source media="(prefers-color-scheme: dark)" srcset="logo/whiteCrown128.png">
  <img src="logo/blackCrown128.png" alt="Draugr Time tray crown" width="32" height="32">
</picture>

# Draugr Time

**Local activity tracking with a native Rust desktop app.**

[![Listed on Remade](https://remade.wiki/badges/listed-on-remade-classic.svg)](https://remade.wiki/projects/draugr-time/)
[![CI](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml/badge.svg)](https://github.com/hlophlopgaming/DraugrTime/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/license-GPLv3-blue.svg)](LICENSE)
![Platforms](https://img.shields.io/badge/platforms-Windows%20%7C%20macOS%20%7C%20Linux-informational)
![Rust + Iced](https://img.shields.io/badge/built%20with-Rust%20%2B%20Iced-orange)

**English** · [Русский](README.ru.md) · [Deutsch](README.de.md) · [Français](README.fr.md) · [Español](README.es.md) · [Português](README.pt-BR.md) · [简体中文](README.zh-CN.md) · [日本語](README.ja.md)

</div>

Draugr Time records your active application, window title and active/away time,
then shows them on a daily timeline. It is a free, GPL-3.0 project inspired by
ManicTime's workflow. Your history stays on your computer.

**In active development.** The interface and features are still evolving.

## What works

- Native **Iced** UI with light, dark and system themes.
- Active-window and idle tracking on Windows, macOS and Linux X11.
- Wayland backends for GNOME, KDE, Sway and Hyprland. GNOME/KDE need a local
  compositor bridge; see [Wayland setup](docs/wayland.md).
- Day timeline with computer, application, document and tag tracks; zoom, pan,
  range selection, duration summaries, hierarchical tags and away-time tagging.
- Background recording, tray controls, pause/resume and optional startup at login.
- Day overview, weekly timelines, monthly reports and timesheets; CSV/JSON export,
  ActivityWatch import and a separate headless `draugr-time-cli`.
- Local SQLite storage and a selectable data folder.
- First-run setup, application exclusions, recording schedules and confirmed range deletion.

One executable contains the UI, tracking, storage and localhost API. There is
no web frontend or separate service to launch. Normal launches have no demo mode.

## Screenshots

These screenshots show generated sample history, not personal activity.

![Draugr Time — light theme](docs/screenshots/draugr-time-light.png)

![Draugr Time — dark theme](docs/screenshots/draugr-time-dark.png)

## Build and run

Install stable Rust. Linux needs librsvg and libxkbcommon-x11 development packages,
a StatusNotifierItem tray host and xdg-desktop-portal with a desktop backend. Windows needs MSVC; macOS needs Xcode
command-line tools. For ready-to-run archives and installers, use the
[releases page](https://github.com/hlophlopgaming/DraugrTime/releases). Windows
offers an installer and a portable zip, macOS a DMG, and Linux an AppImage plus
a portable archive. Extract archives before launching: Windows/Linux run
`draugr-time`; macOS opens `Draugr Time.app`. See [first-run setup](docs/m10.md).

```sh
git clone https://github.com/hlophlopgaming/DraugrTime.git
cd DraugrTime
cargo run --release --package draugr-time
```

The executable is `target/release/draugr-time` (`.exe` on Windows). Press
**Start recording** in onboarding to collect activity. Closing the window keeps recording
in the tray; **Quit** stops the application.

See [development instructions](docs/DEVELOPMENT.md) for checks and diagnostics,
and [desktop usage](docs/desktop.md) for settings and platform notes.

## Data and privacy

- History is stored locally in SQLite. No account, cloud uploads or telemetry.
- Application network traffic stays on localhost; compositor IPC stays local.
- Settings → Privacy can exclude applications entirely or mask their window
  titles: masked applications keep their time, but titles are stored as `[masked]`.
- New installations use the OS local-data folder under `DraugrTime`. Existing
  installations can continue using their legacy `DagdaTime` folder.
- Settings → Data folder selects another folder for the next launch; existing
  history remains in its current location.

## Roadmap

- [x] **M0–M3.5:** workspace, storage, native watchers, local API and desktop shell.
- [x] **M4:** Day timeline, range selection and summaries.
- [x] **M5:** tag editing, tag manager and away-time tagging.
- [x] **M5.5:** GTK-free Linux tray, day-data cleanup and lower desktop memory usage.
- [x] **M6:** automatic tagging rules with preview/history undo; eight interface languages, OS detection, saved selection and English fallback. Platform/translation review: [M6 verification](docs/m6.md).
- [x] **M7:** Week view, reports, CSV/JSON export and ActivityWatch import.
- [x] ~~**M9:** browser extension~~ — dropped: window titles already cover browser pages.
- [x] **M10 — 1.0:** app icon everywhere, built-in CLI on Linux/macOS, first-run onboarding, minimal privacy settings.
- [x] **M11 — 1.1:** installers: Windows installer + portable zip, Linux AppImage, macOS DMG (Flatpak deferred).
- [x] **M11.1:** title masking for selected applications.
- [ ] **M12:** design, backups, notes, shortcuts, file paths, timed "Don't track for…" pauses, autostart prompt, ActivityWatch categories import/export, Freeform-style scenes.
- [ ] **M13:** Android companion with a share-target inbox (links with previews) and freeform boards.

Details: [PLAN.md](PLAN.md) · [Architecture decisions](docs/decisions.md).

## Contributing

Open an [issue](https://github.com/hlophlopgaming/DraugrTime/issues) to discuss a
change. Help with desktop permissions, Wayland integrations and real OS testing
is welcome. Agent instructions are in [AGENTS.md](AGENTS.md).

## License

[GNU GPLv3](LICENSE). Draugr Time is independent of ManicTime and ActivityWatch.

M6 adds regex auto-tagging with priority, live preview and reversible history application.
Settings offers English, Russian, German, French, Spanish, Portuguese, Simplified Chinese
and Japanese, or the system language. See [M6 usage and verification](docs/m6.md).

Day overview, week timelines, reports and local console: [M7 guide](docs/m7.md).

On Linux/macOS the CLI is built into the desktop executable and runs without a
window: `draugr-time report --from 2026-10-01 --to 2026-10-16 --json`.
Use `draugr-time --help`; Windows uses the separate `draugr-time-cli.exe`.
