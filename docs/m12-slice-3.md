# M12 design slice 3 — Week

Implemented only the Week slice from PLAN.md. The screen uses the shared shell,
fonts, palette, focusable buttons and card styles from slices 1–2. Tags, Rules,
Settings/tray redesigns and the motion pass remain later slices.

## Result

- Header: Week title, local date range, recorded-day count and a single Active /
  Tagged / Untagged summary strip. Active-per-day divides by days with recording,
  not by future days. The gap link opens Day with an untagged interval selected.
- Seven 64 px day rows, plus separators, with sessions, a 07:00–21:00 local-clock
  axis, 10 px tag lane, 16 px application/activity lane and daily totals. Today
  is tinted and future days explicitly say “No data yet”. Date, timeline and
  total portions all open Day; the date and total controls also work by keyboard.
- Dense context is aggregated to pixels before painting, so subpixel switches
  remain visible. Tags retain their colors; untagged blocks have dashed outlines;
  away intervals are hatched. Diagnostic unobserved bands remain as a thin red
  overlay within the activity lane. All numeric totals cover full local days.
- Timesheet: tag rows × seven dates, hours, total column and CSV export with an
  additional daily/weekly Total row in the file. CSV uses four decimal places
  rather than the display's one decimal, and escapes commas/quotes/newlines.
- “This week”: tag totals, relative bars, signed time changes against the previous
  seven local dates, and five most-used applications with existing local icons.
  Clicking a tag in either the panel or table highlights it on the timeline.
- “Open full report” carries the displayed week's dates into Reports. Privacy
  footer, ghost date navigation, no permanent Refresh/legend/report toolbar and
  no filled accent action buttons in the Week panels.
- Narrow layout: top navigation, one vertical scroll through main content and
  the panel below; the timesheet alone scrolls horizontally. All new copy has
  English, Russian, German, French, Spanish, Portuguese, Chinese and Japanese
  translations. Navigation targets grow to fit the longest translated label, using measured
  font bounds and shared padding; labels stay on one line.

## Screenshots

Native screenshots use the existing `--benchmark --screen Week` fixture and
isolated data/config directories under `/tmp`; they do not access personal
history. Native recording is actually off in this diagnostic, so REC correctly
shows that state. No artwork sample values or new demo mode were added.

- [Adaptive navigation: English / Russian / German](screenshots/m12-navigation-auto-size.png)
- [Dark native, 1280×820](screenshots/m12-week-native-dark-1280.png)
- [Light native, 1280×820](screenshots/m12-week-native-light-1280.png)
- [Dark native, 640×820](screenshots/m12-week-native-dark-640.png)
- [Light native, 640×820](screenshots/m12-week-native-light-640.png)
- [Dark beside source artwork](screenshots/m12-week-comparison-dark.png)
- [Light beside source artwork](screenshots/m12-week-comparison-light.png)
- [Whole timesheet, dark, 1280×1000](screenshots/m12-week-en-dark-1280x1000.png)
- [Whole timesheet, light, 1280×1000](screenshots/m12-week-en-light-1280x1000.png)
- [Narrow full layout, Russian dark](screenshots/m12-week-ru-dark-700x1700.png)
- [Narrow full layout, German light](screenshots/m12-week-de-light-700x1700.png)

The explicit render test also produces English/Russian/German snapshots in both
colors at 1280×820, 1280×1000, 700×820 and 700×1700. It loads the bundled fonts and
paints the actual Iced view with tiny-skia; these are not HTML approximations.
The source artwork files are `docs/design/Week@1x.png` and
`docs/design/Week · light@1x.png` (1280×909). Comparison images keep the source at
its native scale and place the requested 1280×820 capture alongside it.

## Differences from the artwork and reasons

1. Sample dates, day/session counts, tags, durations, colors and apps come from
   the existing fixture/runtime. Today and REC use actual current state, rather
   than pretending it is Thursday or that this screenshot is recording.
2. The spec requires 24 px main padding; the PNG uses about 20 px at the top.
   Shared cards use the existing 12 px radius, within the spec's 12–14 range,
   20 px timeline padding, 20 px horizontal / 16 px vertical timesheet padding,
   and accessible minimum targets. The Untagged cell keeps the shared neutral
   card surface with an amber label, instead of the PNG's additional cell tint;
   this follows the summary component's textual definition.
   Local font metrics and these constraints cause small baseline/spacing shifts.
3. The artwork is 909 px tall. At the requested 820 px height the main column
   scrolls to reach the lower timesheet rows; rows are not squeezed below 64 px.
   At narrow widths the entire screen scrolls vertically and the table scrolls
   horizontally, as required by the textual spec.
4. Session count means contiguous observed active stretches, independent of
   application/document changes. “Per day” uses days with recording. These
   calculations are explicit because the artwork does not define them.
5. Future dates say “No data yet”; past/today dates with zero observed active
   time say “No activity in this range” instead. This avoids describing a sleep,
   away-only or unrecorded past day as future data. Existing inactive colors are
   preserved under context, away is hatched, and diagnostic gaps stay visible.
6. Untagged ranges are dashed, in accordance with the design principles, even
   though the Week artwork leaves much of the tag lane empty. Untagged is listed
   after actual tags; empty numeric table cells display a dash (CSV keeps zero).
7. Application icons come from the installed local icon cache, with the existing
   colored initial fallback. Native icons, the bundled logo and rail glyphs can
   differ from the artwork's sample badges. Stored tag colors are retained;
   app colors use the shared stable categorical palette keyed by the existing
   Report application label. Reports already group identical display names; Day
   keeps executable/name identities, so its app colors can differ. This slice
   preserves that reporting model instead of changing the core report format.
   The Tagged summary
   uses the specified violet categorical color rather than an accent action.
8. The right panel includes the required local-storage privacy line, absent from
   the Week PNG. Report/export/review links remain transparent and secondary to
   the data. Keyboard focus rings are the shared 2 px accent rings.
9. The user explicitly requested automatic navigation sizing after reporting
   repeated overflow. The rail now grows beyond its 72 px minimum to fit the
   longest translated label, with one-line labels and an explicit icon/text gap.
   The narrow top bar uses those same target widths and scrolls horizontally
   when needed. Long duration values can still wrap within summary cells; no
   English-only abbreviations or smaller fonts were introduced.
10. Selection/highlight/date changes are immediate. Animation remains slice 6;
    this slice adds no idle timer or motion subscription.

## Verification

See [validation output](screenshots/m12-week-validation.txt),
[native run results](screenshots/m12-week-native-results.json) and
[release measurements](screenshots/m12-week-measurements.json).

The navigation fit test checks all eight supported languages against actual
font bounds, including control padding and the complete icon/text stack.
Tests cover contiguous sessions/gaps, day-boundary clipping, pixel aggregation
of 5,000+ events, future-event clipping, previous-week date boundaries, previous
period totals, escaped timesheet CSV, gap-to-Day navigation, full-report dates,
and the existing canvas cache/theme/size/highlight regressions. Existing palette
contrast tests check text at ≥4.5:1 in both themes, including interactive styles;
focus/keyboard behavior uses the already-tested shared button component.

Manual verification still needed: Windows/macOS build and native run; X11,
HiDPI and long real tag names; keyboard Tab/Enter through Week rows, tag highlight,
CSV save/cancel, and reopening the corresponding full report with real history.
A long-running real tracking session and minute-long idle/settling measurements
are not covered by the short benchmark diagnostics.


### Final checks and observed limits

- `cargo fmt --check`: exit 0, no output.
- `cargo clippy --all-targets -- -D warnings`: exit 0.
- `cargo test`: 244 passed, 0 failed, 11 ignored environment/artifact checks.
- Explicit Week screenshot test: passed; 24 actual Iced renders in three
  languages, both themes and four window sizes. Observed debug canvas maximum
  36.54 ms while other builds were running; this is not a release frame budget.
- `cargo build --release --offline`: passed, 2m 08s. The final executable is
  `target/release/draugr-time` (Linux x86-64).
- Final native dark/light 1280×820 and dark 640×820 diagnostics completed.
  Light 640×820 wrote the attached PNG, but its automatic diagnostic did not
  finish within 30 seconds on either attempt; those processes were killed and
  waited. No cause has been established. Two earlier diagnostics without an
  output PNG also timed out. This is an unresolved Wayland diagnostic/lifecycle
  risk, not a passing lifecycle result.
- The final release memory diagnostic with capture completed; canvas maximum
  2.75 ms. PSS was 45.60 MiB at 4s and 49.61 MiB at 6s with the window open,
  then 20.40 MiB at both 9s and 11s after closing. No additional CPU ticks were
  observed between those two closed-window samples.
- An earlier pre-Week baseline with capture measured 44.86 MiB open and
  19.78 MiB closed. The final observation is higher, so no “zero memory
  regression” claim is made. These single short runs include screenshot/PNG
  work, allocator retention and diagnostic timers; the old fixture also retained
  future-day events, so workloads are not identical. They do not replace a
  controlled minute-long idle comparison on real history.

For testing an updated executable, use the tray's Quit command to stop the old
process before starting the new build; closing a window keeps that process alive.
