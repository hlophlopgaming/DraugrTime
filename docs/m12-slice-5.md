# M12 slice 5 — Settings, tray and first-launch autostart

Settings now uses six selectable sections, shared theme tokens and controls, square
switches and removable excluded/masked application chips. Tracking owns the schedule
and away-prompt options; Status owns platform setup and collector diagnostics.
Section changes preserve unsaved fields. History deletion retains its explicit range,
confirmation and error handling. General retains all eight languages and theme selection;
Data retains folder selection, atomic history import and access to report export.

The native tray replaces indefinite pause with a submenu of 5/10/15/30/60 minutes,
marks the last preset used in this process, explains automatic resumption and supports
Resume and +5/+15. Both entry points share the recording controls. The countdown does
not rewrite the tray image every second. A persisted first-launch question offers
startup registration through the existing platform provider; old installations never
get the new question. Errors remain visible and allow retrying.

## Screenshots and comparison

These are actual Iced widgets rendered with tiny-skia and bundled local fonts, using
an isolated in-memory recording session and generic privacy lists. They are widget
captures, not screenshots of a native OS window. No personal activity is loaded.
The explicit renderer covers all sections plus the autostart dialog in English,
Russian and German, both themes, at 1280×820, 700×820 and 700×1600.

| View | Dark | Light |
|---|---|---|
| Tracking, 1280×820 | [Dark](screenshots/m12-settings-tracking-en-dark-1280x820.png) | [Light](screenshots/m12-settings-tracking-en-light-1280x820.png) |
| Privacy, 1280×820 | [Dark](screenshots/m12-settings-privacy-en-dark-1280x820.png) | [Light](screenshots/m12-settings-privacy-en-light-1280x820.png) |
| Tracking, narrow German | [Dark](screenshots/m12-settings-tracking-de-dark-700x820.png) | [Light](screenshots/m12-settings-tracking-de-light-700x820.png) |
| Privacy, full narrow Russian layout | [Dark](screenshots/m12-settings-privacy-ru-dark-700x1600.png) | [Light](screenshots/m12-settings-privacy-ru-light-700x1600.png) |
| Autostart, 1280×820 | [Dark](screenshots/m12-settings-autostart-en-dark-1280x820.png) | [Light](screenshots/m12-settings-autostart-en-light-1280x820.png) |

Compare with [Settings dark](<design/Settings@1x.png>),
[Settings light](<design/Settings · light@1x.png>) and
[autostart](<design/First launch — autostart@1x.png>).
The rail, 220 px section navigation, neutral cards, square switches and destructive
privacy action follow the artboards. Differences and their reasons:

- The selected section is one card, following the written spec, rather than showing
  Tracking, Privacy, Data and Status simultaneously as in the tall artboard.
- Existing labels and away-prompt duration choices are preserved. The fixed five-minute
  idle detector is not advertised as configurable. Global repeat-last shortcuts,
  backups and ActivityWatch categories have no backend here and get no fake controls.
- Chips remove entries from a semicolon-separated draft, saved explicitly. An existing
  text field adds entries; there is no new application-picker dialog.
- The Settings section list precedes the card at narrow widths and wraps into tabs.
  Labels can wrap/grow; the page scrolls vertically rather than retaining 900 px width.
- Startup uses the local brand mark, theme background and existing text. Only the dialog
  participates in focus navigation; the artboard's dimmed background screen is omitted.
  A failed recording start changes the status badge and exposes its error.
- Native tray typography, spacing and menu appearance belong to the OS. Status, current
  application and selected-day total retain separate descriptive lines. The total still
  explicitly says selected day; an always-live today total is not claimed.
- Pauses reuse current daemon behavior. Durable pause restoration across process restart
  belongs to PLAN.md's separate timed-pause task. No new animation subscription is added;
  the motion pass remains slice 6.

## Verification

`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.
`cargo test`: **263 passed, 0 failed, 13 ignored**. The explicit widget renderer
and private-bus native tray dispatch each passed separately.
See [validation output](screenshots/m12-settings-validation.txt).
Tests cover old/new preferences, one-time prompting and failed persistence, startup
state/error eligibility, suppressed background shortcuts, retained drafts, chip removal,
all five pause IDs, and extending/resuming through the tray's actual message handlers.
Existing tests cover provider failures, schedule validation, localization completeness,
text contrast, keyboard focus, idle subscriptions and the 5,000-event Day fixture.
The native Linux D-Bus menu dispatch test also runs on a private local bus.

To regenerate widget captures:

```sh
cargo test -p draugr-time render_settings_review_screenshots -- --ignored --test-threads=1
```

Manual verification remains: Windows/macOS startup registration and permission controls;
Linux X11/Wayland real-panel submenu/checkmark appearance and keyboard navigation;
closing/reopening while paused, and each theme on an actual desktop. Use a throwaway
history when checking delete, import or schedule changes. Native idle CPU/PSS was not
remeasured; these captures and unit tests cannot establish a new memory baseline.
