# M12 design slice 1

Implemented local Palette tokens, bundled UI/monospace fonts and a responsive
shell around the existing screens. Screen internals remain scheduled for the
later design slices. The Day divider, selection, summary and existing keyboard
shortcuts are retained. No database migration or runtime network access added.

The design source was found at `20dc591`, reachable from `origin/main`.
`docs/design/README.md` lists mockup filenames, but no reference PNGs are
present in that commit. Comparison is therefore against `docs/design.md`:
72 px rail, 36 px logo, REC immediately after Settings, 24 px main padding,
16 px gaps, 344 px side panel (220 px on Settings), and a 760 px breakpoint.
At narrow widths, the existing dense screen body scrolls horizontally and the
whole shell vertically; the side panel is below the main content. Its final
screen-specific contents and Settings section links are deferred to slices 2–5.

F6 enters or leaves rail keyboard focus; arrow keys move focus; Enter or Space
activates a screen/REC; Escape or Tab returns to search. REC starts recording,
resumes when paused, or pauses for the existing 15-minute preset. Timed-pause
redesign and animations remain outside this slice.

## Screenshots

Native Iced/tiny-skia Wayland captures, 5,000 generated events, isolated
XDG data/config folders; no personal history or demo data shipped:

| Theme | 1280×820 | 640×820 |
|---|---|---|
| Dark | [Desktop](screenshots/m12-slice1-dark.png) | [Narrow](screenshots/m12-slice1-dark-narrow.png) |
| Light | [Desktop](screenshots/m12-slice1-light.png) | [Narrow](screenshots/m12-slice1-light-narrow.png) |

Compared visually: both themes use the specified neutral surfaces and accent;
rail targets, placement and desktop panel widths match the text specification.
At 640 px the navigation is on top, headers wrap, and wide legacy content is
clipped within its horizontal scroll view. Scroll down to reach the side panel.
The earlier timeline colors, summary layout, tracking menu and screen-specific
controls have not received their later design passes. Pixel comparison with
mockup PNGs remains unavailable.

## Verification

Tests were written before palette/breakpoint implementation and before rail
keyboard behavior. Contrast tests cover primary/secondary/meta, accent, away
and danger text on background, side and card surfaces in both themes (4.5:1).
Native widget drawing tests check the rail and side panel widths; keyboard
regression checks focus, activation and preservation of the selected day.
`fc-scan` confirms the embedded family names Inter Tight and JetBrains Mono.

Final workspace checks:

```text
cargo fmt --check                         passed (no output)
cargo clippy --all-targets -- -D warnings  passed
cargo test                               223 passed; 0 failed; 9 ignored
```

The ignored checks require an OS session, portal, installed applications or
native integration. Fixture screenshot runs reported maximum timeline geometry
build times of 1.93–2.91 ms (debug build, parallel runs); this is not a controlled
performance comparison. No new
idle timer or animation loop was added; the existing ticking regression tests
remain. An idle CPU/PSS comparison against a baseline was not performed.

One of four parallel capture processes (desktop light) saved its PNG but did
not exit automatically and was terminated. An isolated repeat completed
successfully (maximum geometry build 1.89 ms). The cause of the first process
stall was not established; no assertion of lifecycle stability follows from
these captures.

## Manual OS verification remaining

- Linux Wayland: interactively resize across 760 px, scroll horizontally and
  down to the side panel, use F6/arrow/Enter and the REC control through
  recording/paused/error states. Check long Russian/German labels and CJK
  fallback glyphs. Captures were verified locally, pointer interactions were
  not exhaustively exercised.
- Linux X11: repeat the above, including theme switching and close/reopen;
  native captures in this slice used Wayland only.
- Windows: check font embedding in the packaged build, DPI scaling (100/150/
  200%), narrow scrolling, keyboard focus, REC, system theme changes and tray
  close/reopen. No Windows runtime was available here.
- macOS: repeat at Retina scale, check system appearance and menu-bar
  close/reopen, and ensure F6 is delivered (Fn may be needed with media keys).
  No macOS runtime was available here.


## 2026-10-23 — PR #13 integration

Reviewed the brand refresh in
[PR #13](https://github.com/hlophlopgaming/DraugrTime/pull/13).
Fixed Rust formatting that blocked CI, and the portable Linux release archive
still copying the old black mark instead of `appIcon128.png`; added a failing
packaging regression test before correcting the workflow. Verified all nine
regenerated assets match the bundled files. Removed an unused Python import.

Local combined M12 + PR checks: `cargo fmt --check` passed,
`cargo clippy --all-targets -- -D warnings` passed, `cargo test` passed
(228 tests, 0 failures, 9 environment-dependent checks ignored), and
`python3 -m unittest discover -s tests` passed (11 tests).
The local M12 commit is retained separately from the reviewed PR, along with
the existing Linux app-label/icon changes and Android/Scenes roadmap.

PR CI [run 110](https://github.com/hlophlopgaming/DraugrTime/actions/runs/39854805153)
passed on Linux, Windows and macOS, including native launch and installer
assembly. PR #13 was merged as `731f767cd083bb7adb3732ac537f0043ca1054a5`.
Real desktop-panel appearance, Windows/macOS DPI/theme behavior and manual
tray interaction remain the platform checks listed above.

All four screenshots above were refreshed after integrating the new brand.
Native Wayland fixture runs rendered 5,000 events, with maximum geometry build
between 1.68 and 1.96 ms in this run. The first light/narrow capture timed out;
an isolated retry exited successfully and produced the final 640×820 PNG.
This repeats the local capture/lifecycle limitation already noted above and
was not reproduced by the PR's cross-platform native-launch CI.
