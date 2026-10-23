# M5.5 desktop memory checks

Measured on 2026-10-14 on Linux/Wayland, tiny-skia, release builds. Baseline was
the pre-change `target/release/draugr-time` saved before rebuilding. Each process
used isolated data/config directories; no personal history was loaded. The
`--benchmark` fixture contains 5,000 window events, opens a 1280×820 window and
closes it after seven seconds. Read `/proc/<pid>/smaps_rollup` at four seconds
(window open), nine and eleven seconds (window closed). Separate `--minimized`
launches were sampled at four seconds without starting recording.

| Scenario | Before PSS | After PSS | Before RSS | After RSS |
|---|---:|---:|---:|---:|
| Tray only, no window opened | 32.33 MiB | 11.43 MiB | 59.16 MiB | 15.09 MiB |
| Window open, 5,000 events | 45.44 MiB | 31.93 MiB | 72.43 MiB | 36.19 MiB |
| After closing window | 36.44 MiB | 17.48 MiB | 63.42 MiB | 21.75 MiB |

PSS decreased about 65% for tray-only startup and 52% after closing the fixture
window. At nine and eleven seconds the post-close values were identical. These
are observations on one machine, not a promised memory bound or an isolated
measurement of each change. Earlier repeat runs had post-close PSS around
18.7 MiB; resident shared-library pages and system activity affect PSS.

Closing does not restore the exact cold-start baseline: the final run retained
about 6 MiB more PSS than tray-only startup (private dirty memory: 4.81 vs
2.31 MiB). Code/font/runtime caches and preserved editing/undo state may remain.
At the time of these measurements, tag undo snapshots covered whole history
and were kept for up to 20 actions. The follow-up below replaces those snapshots.
No heaptrack attribution or minute-long settling measurement was performed.
One repeated native Wayland run timed out in the runtime diagnostic (30 seconds).
A diagnostic rerun and three further isolated runtime runs passed; the timeout
was not reproduced and no stack of the stalled process was captured. Its cause
is not established; real-session stability still needs monitoring.

Verified locally:

- Linux dependency tree and dynamic linkage have no GTK/GDK/Pango/Cairo or
  libappindicator; ksni and ashpd use the existing zbus/async-io stack.
- Installed KDE application icons and preview-process icons resolve to raw local PNG/SVG images (both platform tests passed).
- Native Wayland smoke test renders 5,000 fixture events.
- Runtime fixture writes to isolated SQLite; lifecycle fixture closes the window,
  keeps recording without day reloads, then reopens and displays the new samples.
- Real ksni D-Bus menu dispatch works without GUI ticks on a private session bus.
  Run `dbus-run-session -- cargo test --package draugr-time native_ksni_menu_dispatches_without_gui_ticks -- --ignored --test-threads=1`.
- Rust regression tests cover releasing buffers, stale results, lazy/shared search,
  tail replacement, raw icon handles, day-scoped tag loads, original interval
  bounds, outside-day overlap, whole-history delete totals and v2→v3 migration.

Manual verification remaining:

- Linux X11 and Wayland: right-click the tray while the window is hidden, test
  Open, Settings, quick tags, Pause, Resume and Quit. Disable the SNI host and
  verify visible-window fallback. GNOME requires an SNI/AppIndicator extension.
- Linux folder selection: with the desktop's xdg-desktop-portal backend installed,
  open Settings → Choose folder, select an absolute path, restart and verify the
  selected data directory. Cancel must leave the preference unchanged.
- Windows/macOS: build and run, verify raw PNG application icons, close/reopen,
  unsaved tag edits and undo, tray menus and native folder selection. macOS also
  needs a check that background sampling and permissions still work when hidden.

For longer measurements, open and close the normal application on an isolated
fixture database and sample smaps_rollup after a minute. Compare PSS, not only RSS;
check preserved undo separately after editing a large history.

## M5.5 follow-up: typed row undo

Tag editing and rule-history application now retain typed changes only for rows
actually changed. Renaming one tag in a fixture with 5,000 ranges retains two
Tag values and no ranges; inserting a nonoverlapping range under an existing
tag retains one TagRange. Undo no longer constructs whole-history snapshots.
Regression tests cover splits with gaps/reused IDs, LIFO restoration, subtree
and merge restoration including rules, independent external edits, conflicting
edits and rollback when new dependent rows would be orphaned.

The earlier PSS/RSS figures were not rerun for the undo-only follow-up.
Week/report retention is addressed and measured in the later follow-up below;
renderer/font buffers remain managed by Iced. Bulk operations still
retain all changed rows, rather than a fixed memory bound. Manual desktop
verification on Linux/X11/Wayland, Windows and macOS: tag/split/move/rename,
apply rules, undo repeatedly, close/reopen and confirm undo still works.

Follow-up verification (offline, 2026-10-14): `cargo fmt --check` passed;
`cargo clippy --all-targets -- -D warnings` passed; `cargo test` passed with
164 tests, 0 failures and 7 ignored environment-dependent checks. The 55 core
tests include the new undo regressions. Full workspace tests and clippy ran
with `CARGO_NET_OFFLINE=true`; no crates.io access was needed.

## M5.5 follow-up: Week and reports

Reports now load one shared range report, then clip its ordered intervals to
local days to aggregate timesheet cells. Days retain only totals, not copies of
intervals or full tracks. Week retains three run-length compressed color bands
(computer state, diagnostic overlay, assigned tags), bounded by 1,440 cells per
band. On DST days these bins divide elapsed day length rather than representing
exact wall-clock minutes. Subminute visual detail may merge; numeric totals,
selection in Day and exported intervals remain exact.

Core history reads approximately 8,192 events at a time, clipping complete
prefixes at timestamp boundaries and preserving live tails. This removes the
100,000-event limit from reports and JSON archive export while keeping the
localhost event-query cap. Report labels use interned Arc<str>; equivalent
adjacent intervals are compared before label allocation. A timestamp with more
than 8,192 simultaneous buckets is read together; normal tracker buckets do
not approach this case. Full output intervals and raw JSON archives still
require memory proportional to their output size.

Leaving Week/Reports drops data, day-owned canvas caches and pending-load
ownership before invoking MemoryReclaimer. Old completions cannot repopulate a
hidden screen and are reclaimed when delivered. This includes day buttons,
sidebar navigation, tray settings/quick tagging and away tagging. Week geometry
rebuilds only for new data, changed dimensions or changed theme.

### Release measurements

Linux X11 through XWayland, tiny-skia, 2026-10-14. Built the pre-report-fix source
in an isolated /tmp copy and the current source with the same Rust toolchain and
cached dependencies, offline. Each GUI process used isolated data/config paths
and the existing --benchmark fixture (5,000 events per generated day). Sampled
smaps_rollup at 4 seconds with the window open, and at 9/11 seconds after its
automatic close at 7 seconds. One run per table row; these are synthetic
observations, not bounds for personal history. The undo fix is present in both
builds, so these figures isolate the subsequent report/Week work.

| Scenario | Before PSS | After PSS | Before RSS | After RSS |
|---|---:|---:|---:|---:|
| Week open, 1280×820 | 76.55 MiB | 35.62 MiB | 85.54 MiB | 44.62 MiB |
| Week closed, 11 seconds | 19.50 MiB | 19.33 MiB | 26.48 MiB | 26.34 MiB |
| Reports open, default 1280×820 | 32.92 MiB | 29.25 MiB | 41.98 MiB | 38.09 MiB |
| Reports closed, 11 seconds | 19.43 MiB | 19.05 MiB | 26.48 MiB | 25.89 MiB |

Week PSS decreased about 53%. Observed canvas build maxima in the Week fixture
were 4.69 ms before and 0.02 ms after; the latter reflects compressed/cached
Week geometry. A separate monthly CLI fixture had 93,000 alternating window
events with repeated long titles plus 31 active-state events. Exact CSV output
matched before/after. Linux wait4 max RSS and elapsed-time observations are
recorded below; fixture generation streamed rows to avoid inheriting a large
Python parent working set in the measured child.

| Monthly CLI report | Before | After |
|---|---:|---:|
| Peak RSS | 159.65 MiB | 26.62 MiB |
| Elapsed time | 0.318 s | 0.234 s |

The final implementation reduced this fixture's peak RSS about 83%. Report
prefixes are bounded by event count, rather than iterating over every calendar
day, so large empty timestamp ranges also complete directly.

Size checks with the new release:

| Actual Day window | Open PSS | Closed PSS at 11 seconds |
|---|---:|---:|
| 1280×820 | 30.89 MiB | 20.37 MiB |
| 1920×1020 | 33.60 MiB | 20.53 MiB |

The window manager clamped requests for 2560×1440 and 3840×2160 to 1920×1020;
PNG screenshot dimensions confirmed the actual sizes. This is not a real 4K
measurement. In a repeated 1920×1020 run, the softbuffer shared-memory mapping
held 7.47 MiB RSS while open and disappeared after close. Font-file mappings
held 0.64 MiB RSS, unchanged after close, on this fixture. This does not measure
additional CJK/emoji fallback fonts in personal window titles. Iced's installed
tiny-skia compositor resizes softbuffer and replaces its clip mask when the
surface size changes; window destruction releases the surface. No renderer or
font-backend replacement was warranted by these observations.

### Verification and remaining manual checks

Regression tests cover >100,000 events in one dense interval (report and JSON
archive), buffered live tails, cross-batch overlap/legacy-AFK cutoff parity,
string identity and JSON output, large empty timestamp ranges, cross-midnight
day totals, 23/25-hour bin boundaries, bounded bands, geometry cache reuse and
resize/theme invalidation, navigation cleanup and stale completions.

Final offline checks: `cargo fmt --check` passed;
`cargo clippy --all-targets -- -D warnings` passed; `cargo test` passed with
174 tests, 0 failures and 7 ignored environment-dependent checks.
`cargo build --release --workspace` passed. The resulting executables are in
`target/release/draugr-time` and `target/release/draugr-time-cli`.
A native Wayland `--smoke-test --screen Week` run passed with 5,000 fixture
events and a 0.02 ms observed maximum canvas build time; the X11/XWayland
benchmark runs above also exited successfully. All fixture data and temporary
measurement scripts/logs stayed under /tmp; personal history was not loaded.

Manually verify Windows/macOS and a real Linux desktop session with personal
history: Week → Day/Reports/Settings and tray/away navigation; report totals,
colors, CSV/JSON export, theme changes and close/reopen with undo preserved.
On real 1440p/4K displays, measure smaps_rollup before/after maximize, restore
and close; compare font mappings after Cyrillic/CJK/emoji titles. Subminute
Week bins are intentionally approximate; use Day for exact selection/editing.

## M12 slice 3 — Week and adaptive navigation (2026-10-23)

Week now paints tags and application context in two lanes, with pixel aggregation
and cached geometry. Previous-week report intervals are discarded after deriving
tag totals. Navigation measurements are cached by language/font version. No new
subscription or idle timer was introduced.

The final release diagnostic (1280×820, isolated fixture, PNG capture enabled)
measured PSS 45.60 MiB at 4s, 49.61 MiB at 6s and 20.40 MiB at both 9s/11s after
closing. Closed-window CPU ticks did not increase between the latter samples.
Canvas build maximum was 2.75 ms. An earlier pre-Week run measured 44.86 MiB open
and 19.78 MiB closed; the final observation is higher. These short diagnostics
include capture/PNG work and benchmark ticking, and the old fixture retained
future events that the new fixture clips. They cannot establish unchanged idle
CPU/memory; a controlled longer comparison on real history remains unverified.

Final native dark/light desktop and dark narrow diagnostics completed. Light
narrow wrote its PNG but timed out before finishing automatic teardown on both
attempts. Two earlier diagnostic runs also timed out. The cause is unestablished;
do not count these as passing lifecycle checks. See the [slice report](m12-slice-3.md)
and [raw measurements](screenshots/m12-week-measurements.json).

## M12 slice 4 — Tags and Rules (2026-10-23)

No new timer was added. Management metadata and tag statistics load on a worker;
view() does no SQL. Subtree statistics stream historical ranges and retain only totals.
The tag chart caches geometry. Data, chart and ordinary rule-preview caches are dropped
on close or leaving management screens; unsaved fields remain, and a pending checked
history confirmation retains its snapshot. The drag release subscription exists only
while a rule is being dragged. Tests cover cache release and stale response rejection.

Final widget screenshots use an isolated in-memory DB. Native CPU/PSS were not measured
again in this slice; neither
zero regression nor successful native teardown is claimed. The previous Wayland timeouts
remain unresolved. Live editor previews retain the pre-existing 100,000-event cap and
synchronous recalculation. Full metadata refresh scans tag history and computes coverage
per enabled rule, so very large histories need separate responsiveness measurements.

## 2026-10-23 — Minimized-window history refresh

Losing focus now stops live history refresh as well as animations. Previously a
minimized window retained its window ID and continued loading/rebuilding history.
The load dispatch also checks focus, covering queued ticks and macOS's required
sampling timer. Focus re-enables loading without waiting for the previous refresh
interval. Existing history stays available; one in-flight load may still complete.
Visible unfocused windows use the same policy because Wayland does not provide a
portable minimized-state query through Iced.

Regression command:
`cargo test -p draugr-time unfocused_window_stops_history_loads_and_resumes_on_focus`.
It failed before the fix because history refresh remained enabled after focus
loss, and passes after the fix. It delivers 7,200 queued ticks without starting
history loads, then verifies that focus restores loading even after a recent load.
This is a dispatch regression test, not a two-hour RSS/PSS measurement or proof of
the reported 300 MiB allocation's provenance.

Manual verification on Linux X11/Wayland, Windows and macOS: open today's Day
screen, minimize for two hours while recording changing applications, and compare
process memory before/after. Restore and verify the missing interval appears;
repeat while a different window has focus. Confirm tray status and recording
continue throughout. On Linux, compare `/proc/<pid>/smaps_rollup` PSS and RSS.
