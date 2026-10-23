# M12 slice 6 — motion pass

The desktop now has interruptible transitions for Application/Document lanes,
Whole day children, tag-tree branches, highlights, tooltips, selection actions,
applied tag growth, Day range and summary segments, REC pauses, REC menus and the
first-launch question. Both themes reuse the existing palette and local fonts.
No network service, dependency, database migration or user-facing string was added.

## Behaviour

- Lane expansion/collapse takes 260/200 ms, grows/shrinks the track and fades/slides
  lanes by 4 px. Chevron SVGs rotate through 90 degrees. Tracks and labels clip to
  their current height. Reversing a transition continues from its current value.
- Summary children and tag branches reveal their height/fade in 180 ms. Hidden or
  clipped child controls are excluded from focus and input during the reveal.
- Highlight fades take 150 ms, keeping the existing 16%/30% dimming. Rapidly switching
  apps preserves the current opacity mixture. This overlay reuses event geometry.
- Tooltip delay is 120 ms with one wakeup, followed by a 100 ms fade/rise. The existing
  tooltip widget keeps its placement and dismissal. Keyboard detail cards remain
  immediate. Selection follows the pointer directly; its action bar enters over
  120 ms only after release. Applied tag blocks grow over 200 ms.
- Day summary/range segment backgrounds slide for 160 ms, using actual child bounds
  so translated labels can have different widths. REC tint transitions over 200 ms;
  its dot remains static. REC menus enter/close over 140/90 ms. The first-launch
  question fades/scales from 0.98 to 1 over 180 ms and retains its isolated focus scope.
- Theme changes are instant. History loads cancel transitions. Real date navigation
  that loads data settles instantly; fixture dates fade available content for 140 ms.
  **An old/new day cross-fade is not implemented:** no second day/history snapshot is
  retained. Native pick-list/tray menu internals keep toolkit behaviour; tooltip
  chrome still belongs to Iced rather than a separate custom popover renderer.

## Motion preference and idle behaviour

MotionPreference isolates platform reads. Linux reads GNOME enable-animations or KDE
AnimationDurationFactor from the local Settings portal, and listens for changes while
a window is open. Windows reads SPI_GETCLIENTAREAANIMATION; macOS reads NSWorkspace
accessibilityDisplayShouldReduceMotion. The preference is refreshed on focus. An
unknown/unavailable value disables animation; Linux desktops whose portal exposes
neither key consequently use instant transitions. Windows/macOS changes made without
refocusing are picked up on the next focus, rather than by a new background poller.

The 16 ms subscription exists only during visible, focused transitions and removes
itself when they complete. Focus loss, close/hide and reduced motion settle immediately.
Existing collector/history timers retain their previous behaviour. Tooltip delay and
existing toast expiry are single wakeups, not idle animation loops. No event history
is cloned or retained for animation. A regression test verifies that highlight/menu
frames reuse the 5,000-event fixture's cached event geometry.

## Review artifacts

These are actual Iced/tiny-skia widgets, rendered headlessly with bundled fonts and
5,000 generic fixture events. They are not native screenshots or a native FPS trace.
Five frames sample expansion at 0/65/130/195/260 ms; the GIFs hold the final frame and
then reset. They demonstrate the states, not continuous 60 FPS playback.

| View | Dark | Light |
|---|---|---|
| Expansion preview, 1280×820 | [GIF](screenshots/m12-motion-dark.gif) | [GIF](screenshots/m12-motion-light.gif) |
| Intermediate frame, 1280×820 | [PNG](screenshots/m12-motion-dark-1280x820-130.png) | [PNG](screenshots/m12-motion-light-1280x820-130.png) |
| Narrow German, 700×820 | [PNG](screenshots/m12-motion-dark-700x820-130.png) | [PNG](screenshots/m12-motion-light-700x820-130.png) |
| Full narrow layout, 700×1600 | [PNG](screenshots/m12-motion-dark-700x1600-260.png) | [PNG](screenshots/m12-motion-light-700x1600-260.png) |

Compare the final frames with [Day dark](<design/Day — improved@1x.png>) and
[Day light](<design/Day — improved · light@1x.png>). The existing shell, data, track
order, hour chart and panel remain. Expansion temporarily changes only track height,
lane position/opacity and chevrons. The fixture's application names/totals differ from
the artwork; all 5,000 switches remain real canvas rectangles. Narrow review frames
use German to check long labels. Existing slice-5 screenshots cover the startup dialog.

Regenerate frames with:

```sh
cargo test -p draugr-time render_motion_review_screenshots -- --ignored --test-threads=1
```

## Verification and remaining native checks

`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.
`cargo test`: **273 passed, 0 failed, 14 ignored**. The explicit widget
renderer passed separately. Full command output:
[validation](screenshots/m12-motion-validation.txt).
Regression tests cover easing endpoints/reversal, delay without frame polling, expiry,
reduced motion, focus loss, retained data/selection, selection release, typed portal
values, clipped input/layout, rapid highlight retargeting, cache reuse and pixel-identical
initial expansion (no first-frame jump). The review renderer exercises both themes and
all three sizes on the 5,000-event fixture and checks the existing canvas build budget.

Manual verification remains:

- Linux GNOME/KDE: toggle OS animations while open, then reopen from the tray and
  verify the preference persists. On another compositor, verify instant fallback when
  its portal exposes neither preference. Test a rapid track reversal, tree branch
  reversal, tooltip delay/dismissal and dragging outside the canvas before release.
- Windows: toggle Animation effects, refocus the app and exercise the same controls;
  verify minimizing/closing stops animation work. macOS: toggle Accessibility → Display
  → Reduce motion, refocus and check the startup question, menu and lanes.
- All platforms: traverse reveal children/segmented controls with Tab/Shift+Tab and
  Enter; open a tag picker from selection and verify its overlay remains usable.
- Remeasure native idle CPU/PSS and close/reopen against docs/desktop-memory.md. Unit
  tests and headless captures establish timer gating/cache reuse, not a measured native
  memory baseline or native frame rate. No CPU/PSS regression claim is made here.
