# Architectural decisions

## 2026-10-13 — M0 workspace

Use four workspace packages: `draugr-time-core`, `draugr-time-watchers`,
`draugr-time-daemon`, and `draugr-time`. Prefix package names to avoid a
collision with Rust's `core` crate. Watchers and daemon depend on the core;
the desktop app remains separate from the daemon.

## 2026-10-13 — Validation

Check workspace manifests and the CI matrix with Python's standard-library
unittest. CI compiles the native application on all three desktop operating
systems. Runtime GUI
behavior requires a manual launch; compiling does not verify window rendering.

## 2026-10-13 — M1 SQLite storage and migrations

Use synchronous `rusqlite` with bundled SQLite, avoiding OS-specific SQLite
installations and keeping storage testable in memory on every OS. The initial
numbered SQL migration creates the four tables from PLAN.md and the event
index. Record its version in SQLite `user_version`; apply schema changes and
the version update together in an immediate transaction. Future schema changes
must add a migration, never modify `001_initial.sql`. Reject newer schemas
without changing them. Enable foreign keys on every connection and use a
five-second busy timeout for concurrent access.

## 2026-10-13 — M1 heartbeat semantics

`heartbeat(bucket, data, ts, pulsetime)` accepts JSON values; both time arguments
use milliseconds. The first heartbeat creates `[ts, ts]`. Only the latest event
in the same bucket may merge; compare parsed JSON rather than serialized key
order. Extend its end to `ts` only when data matches and the gap is strictly
less than `pulsetime`, as specified in PLAN.md. Do not extrapolate beyond the
last heartbeat. Require nonnegative Unix timestamps, positive pulsetime and a
nonempty bucket. Reject timestamps earlier than the bucket's last end to avoid
silently shortening or overlapping recorded events; historical import belongs
to M7. Read/merge/insert executes in one immediate transaction so competing
writers cannot race. The core never reads the wall clock or makes network calls.

## 2026-10-13 — Wayland moves to M3

At the user's request, move the GNOME, KDE and wlroots backends and the explicit
unsupported-backend UI acceptance criterion from M8 to M3. Keep M8 reserved
to preserve the remaining milestone identifiers. M2 still covers Windows,
macOS and X11. M3 must provide a visible fallback before the timeline arrives
in M4; it cannot silently ignore unsupported Wayland sessions.

## 2026-10-13 — M2 watcher interfaces and native APIs

Keep `WindowWatcher`, `IdleWatcher`, `WindowInfo` and a controllable `MockWatcher`
in the watchers crate. Errors distinguish permission, unsupported session and
platform failure. Both native and mock implementations use the same traits;
the daemon's debug sampling depends only on those traits.

Use `windows-sys` for foreground window, UTF-16 title, PID, executable name
and current-session idle time. Own process handles with the standard library's
`OwnedHandle`. Compute idle from wrapping 32-bit ticks to match GetLastInputInfo;
idle periods longer than a full 49.7-day counter cycle cannot be reconstructed.
Use `x11rb` EWMH properties and XScreenSaver for X11, with WM_NAME fallback
for legacy titles and WM_CLASS for application identity. Restrict connections
to local Unix-socket displays and reject Wayland sessions, since XWayland cannot
report native Wayland windows. Native Wayland backends arrive in M3.

Use AppKit `NSWorkspace` via `objc2` on the main thread, pumping its run loop
to refresh cached application state. Read the focused window title through
Accessibility; explicitly report missing permission. Keep CoreFoundation Create/
Copy ownership in RAII objects and bound Accessibility messaging to 0.5 seconds.
Use CoreGraphics combined-session idle time. Unsafe FFI stays in OS-specific
watcher modules, never in sampling or storage logic.

API references: [Win32 process image](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-queryfullprocessimagenamew),
[Win32 idle input](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getlastinputinfo),
[AppKit frontmost application](https://developer.apple.com/documentation/appkit/nsworkspace/frontmostapplication),
[CoreGraphics idle time](https://developer.apple.com/documentation/coregraphics/cgeventsource/secondssincelasteventtype(_:eventtype:)).

## 2026-10-13 — M2 diagnostic CLI and validation

Name the daemon package's executable `tracker` to provide `tracker debug` as
specified in PLAN.md. Emit one flushed JSON line per sample so window titles
cannot inject extra output lines. Report window and idle errors independently;
retry on the next tick. Poll on a monotonic one-second interval, subtracting
sample time; slow native calls can delay a tick. Do not write events or start
an API server until M3.

Mock tests run without a desktop on every OS. Dedicated Xvfb tests on Linux
verify actual window properties and polling interval, avoiding changes to a
user's desktop. Windows/macOS compilation is checked by CI; real focus, input
and Accessibility permissions still require the manual checks in docs/DEVELOPMENT.md.

## 2026-10-13 — Application UI plan and M3.5 app shell

At the user's request, PLAN.md now specifies the desktop UI explicitly (§5),
modeled on ManicTime's interaction model: a main Day view with timeline tracks,
range selection, a summary panel and tagging, plus Week, Reports, Tags, Rules,
Settings, onboarding, a "while you were away" prompt and a tray menu. A new
milestone M3.5 (app shell: navigation, status, tray, pause/resume, empty and
error states) sits between M3 and M4, so the app is usable as a GUI before the
timeline lands and no milestone numbers change. The UI framework choice moves
from M4 to M3.5. M3 gains `GET /status` and pause/resume endpoints for the UI.

## 2026-10-13 — M3 recording and API

Use Axum on 127.0.0.1:56123, authenticated with a randomly generated local
token (0600 on Unix). Browser Origin requests are denied. The desktop uses
in-process tracking control and snapshots; external local clients use the API.
SQLite access is serialized in one shared connection. CRUD uses only a fixed
collection whitelist with bound values, validates ranges and rejects tag cycles.
No schema migration is needed: M1's tables already cover the M3 API.

Poll native watchers on the daemon's main thread (required by macOS AppKit);
the HTTP server runs on a separate Tokio runtime. Close intervals at the next
observed transition, never across a gap of three seconds or more. Do not record
application intervals while away. Pause generations break intervals even when
pause/resume happens between polls; pause also stops native polling. Watcher and
storage failures are visible in status. GUI and daemon remain separate processes.

## 2026-10-13 — M3 Wayland backends

GNOME and KDE use bundled, opt-in compositor bridges on session D-Bus. Reject
stale samples after three seconds. GNOME includes Mutter's idle measurement;
KDE, Sway and Hyprland use ext-idle-notify, preferring version 2 input-only idle.
Sway and Hyprland window data comes from bounded, timed Unix-socket IPC queries.
Unsupported compositors or missing components are errors in `/status`, not empty
success. Idle history before process startup cannot be recovered from the idle
protocol. Installation and compositor checks are in docs/wayland.md.

## 2026-10-13 — Single desktop executable and selectable data directory

The desktop executable embeds the watcher polling loop, SQLite and localhost
API. It never launches a tracker child process. Native window queries run on
the GUI main thread (required by macOS AppKit); persistence runs in a worker.
An OS file lock prevents a second recording process.
This supersedes the separate daemon/sidecar runtime decision: the user wants
one desktop application. Closing hides to the tray; Quit stops recording.

Data defaults to the OS local application-data directory. A native folder
picker saves the next-launch directory in the default folder. The current
process keeps its original directory so the token and SQLite cannot diverge.
The UI explicitly states that existing history stays in the old location.

## 2026-10-13 — M4 Day timeline

Use a viewport-sized native canvas with subpixel aggregation for 5,000-event
days and native controls for summaries. Selection and totals use exact clipped
interval unions rather than rendered pixels. Local calendar boundaries handle
23/25-hour DST days; storage/API remain UTC milliseconds. Unknown gaps are
unobserved, not invented computer-off events. Demo fixtures are opt-in and
never persisted. Tag editing remains M5.

## 2026-10-13 — KDE bridge verified and local application icons

The declarative KWin package requires contents/ui/main.qml. Installation with
kpackagetool6 exposed the incorrect code/ path; correcting it and enabling the
script changed both real KDE/Wayland collectors to ready without restarting
KWin. Add a package-layout regression test. The bridge is installed per-user.

Application and document summaries and timeline tooltips show local system
icons. Linux matches the canonical process executable against desktop-entry
Exec, then uses the app name to disambiguate entries sharing a launcher such as
Steam before resolving the icon through the installed theme. Windows extracts
the executable icon through the Shell/GDI APIs; macOS asks NSWorkspace for the
bundle or executable icon. Lookups return bounded image data and are cached.
Icons are local and require no database schema change. Ambiguous or missing
matches use the stable initial badge.

## 2026-10-13 — Process identity for grouping and icons

At the user's request, newly recorded window `app` values identify the full
executable path when available: Linux resolves /proc/PID/exe, Windows preserves
QueryFullProcessImageNameW, macOS uses NSRunningApplication.executableURL.
The Day summary groups by that path across PID/window-title changes, displays
the basename, and keeps different executables with the same basename separate.
Existing JSON events retain their original identity; there is no table/schema
change or speculative rewrite of historical process identities.

Linux icons require an exact canonical executable match against a desktop
entry's Exec, with Name and StartupWMClass used only to disambiguate entries
that share an executable. ELF executables generally have no embedded icon; the
verified desktop entry supplies it. Missing or ambiguous metadata uses the
fallback badge.

## 2026-10-13 — Iced login startup and app icon visibility

Add opt-in per-user startup to the native desktop using the OS-native XDG
Autostart, macOS LaunchAgent, or Windows Run registration. Launch with
`--autostart --minimized`, which starts collection and keeps the window in the
tray. Use the current executable path; users need to toggle the setting again
after moving the binary. On Linux, populate the preview with up to five
installed applications, using their resolved executable paths and desktop
names so shared launchers resolve correctly without writing preview data. Keep
synthetic placeholders when no installed app icon resolves. Show real process
icons in the Applications summary, and draw them in the Applications track
only when a block is wide enough to read. The same icon provider is used on
Linux, Windows, and macOS.

## 2026-10-13 — One Iced process per user

Use an OS file lock in Draugr Time's default per-user data directory before
starting the Iced event loop. A second launch exits cleanly; the OS releases the
lock when the process exits, including after a crash. Keep the lock independent
of the user-selected history directory so changing storage cannot start a
second copy.

## 2026-10-13 — Iced system, light, and dark themes

Keep a three-state theme choice: System, Light, or Dark. Use Iced's native
system-theme notifications on Windows and macOS. On Linux, read the XDG portal's
appearance color-scheme over the existing session D-Bus connection and poll
while System is selected, because Linux theme detection is disabled with Iced's
default features off. Fall back to Light when the portal has no preference.

## 2026-10-13 — Performance pass: storage I/O, day query, idle UI

Measured with the same SQLite engine (Python sqlite3) on the real schema:

- **Tracking writes.** Each one-second tick ran up to four separate immediate
  transactions in rollback-journal mode with `synchronous=FULL`: ~54 KiB written
  and several fsyncs per second (~4.4 GiB per day of tracking). The database now
  uses WAL with `synchronous=NORMAL` (durable across application crashes; a power
  loss can drop the last commits) and the tracker writes a whole tick through one
  `Storage::batch()` transaction: ~5 KiB per tick, about 10x less, and a tick is
  now atomic. A further step, if needed, is flushing unchanged extensions less
  often than every second.
- **Day query.** `/events` combined `(?1 IS NULL OR bucket=?1)` with only an
  upper bound on `start_ts`, so SQLite scanned the whole history: 60–100 ms per
  request on a year of data (~1M events), growing linearly, while holding the
  storage mutex the tracker needs. Because events within a bucket never overlap
  (heartbeats are nondecreasing; a new event starts at or after the previous end),
  the query seeks to the last event starting at or before `from` and range-scans
  `idx_events_bucket_start` from there; buckets are enumerated with index skips.
  Same rows, 0.1–7 ms regardless of history size, no migration. **Any importer
  (M7) must preserve the non-overlap invariant**, or add an `end_ts` index instead.

## 2026-10-14 — Review fixes for the Iced app, icons and release

- **Stale loads.** An Iced history load started before a date change, demo
  toggle or recording start could finish afterwards and replace the new view;
  after toggling the demo on, real history could stay on screen labelled as a
  preview. Loads now carry a generation number and results from an older
  generation are dropped.
- **Idle redraws.** Every 2 s poll rebuilt summaries and cleared the canvas
  cache, even when nothing had changed, and past days were re-read forever.
  Identical snapshots are now ignored, only today is re-polled, and the Linux
  theme portal is polled only while a window is open.
- **Tray.** The Iced tray pushed every menu text and a new icon to the OS once
  per second. It now pushes only changed values.
- **Linux icon lookup.** Each cache miss re-read every `.desktop` file and
  resolved each Exec through PATH; the preview picker did this per candidate,
  so startup on the UI thread grew quadratically with installed applications.
  Entries are now parsed once into a shared index, rebuilt after 5 minutes so
  new installs are found. Matching rules are unchanged.
- **Test modes.** `--smoke-test`, `--runtime-test` and `--lifecycle-test`
  default to a temporary data directory when `DRAUGR_TIME_DATA_DIR` is unset, so
  a local run cannot write fixture activity into real history.
  `--autostart-test` restores the previous login-item setting.
- **Release archives.** `actions/upload-artifact` drops file modes; the release
  job restores the executable bit before packing the Linux and macOS binaries.

## 2026-10-14 — Iced Linux collection independent of the GUI

Run Linux native sampling and persistence together in the background tracking
thread once per second. Sampling previously required Iced `Message::Tick`, so
delays in the window event loop left unobserved gaps while other applications
were active. The GUI now only reads status and history on Linux. Pause, resume,
backend retries, and shutdown still use the existing shared control. Preserve
main-thread sampling on macOS, as required by AppKit, and the existing Windows
path; this fix is scoped to the reported Linux issue. Test autonomous recording
with MockWatcher fixture samples and no GUI pulses or channel sender.

## 2026-10-14 — Iced timeline zoom anchored to the pointer

Keep the time under the pointer at the same horizontal position when zooming,
subject to day bounds. Use a 10% step for buttons and line-wheel input, with
reciprocal zoom-out. Scale pixel-wheel input by distance (100 pixels per step)
so touchpad events do not each trigger a full zoom step. Selection remains the
button/keyboard anchor; otherwise use the viewport centre.

## 2026-10-14 — Dedicated GTK event loop for the Iced Linux tray

Create and operate the Linux AppIndicator tray and its GTK menu on one dedicated
thread with a continuously running GLib main loop. Previously GTK events were
pumped only by Iced GUI ticks, making tray interaction depend on the window's
event loop. Pass status updates through a channel and retain the existing menu
event receiver for application actions. Remove main-thread GTK initialization
so all tray GTK objects remain on their owning thread. Test event dispatch and
status updates without GUI ticks using a mock backend and the real GLib loop;
right-click menus still need verification in an actual Linux desktop session.

## 2026-10-14 — Remove interactive demo mode after capturing screenshots

Capture light and dark screenshots with synthetic history. Normal desktop
launches start with an empty Day view until Start recording is pressed. Keep
synthetic data only for unit tests and explicit smoke/benchmark diagnostics;
it is never written to real history.

## 2026-10-14 — Draugr Time and one native desktop

At the user's request, adopt Draugr Time as the product name and make Iced the
only desktop UI, using its tiny-skia renderer. Remove the retired web shell,
web frontend, proxy commands, build tooling and dependencies. The application
package and executable are `draugr-time`; supporting crates use the
`draugr-time-` prefix. CI builds, launches and packages one desktop binary.
Frontend-specific decisions for the retired stack were removed from this log;
storage and platform decisions remain.

New installations use the OS local-data directory under `DraugrTime`. If that
folder does not exist and the legacy `DagdaTime` folder does, keep using the
legacy folder, including its saved data-directory choice and history. There
is no automatic data copy or schema change. `DRAUGR_TIME_DATA_DIR` takes
precedence over the compatible `DAGDATIME_DATA_DIR` environment variable.

Keep installed compositor bridge IDs, D-Bus interfaces and the instance lock
filename stable. Renaming these would silently disable existing GNOME/KDE
tracking or allow duplicate collection. Bridge display names use Draugr Time.
On a normal launch, migrate an enabled legacy startup registration to Draugr
Time. Register the current executable before disabling the old entry; failures
leave the legacy entry enabled. An existing disabled preference stays disabled.
Diagnostic runs never migrate the user's startup settings. Moving the binary
later still requires toggling startup to update its registered path.

## 2026-10-14 — Buffer live tails and load desktop history incrementally

Stage tracker ticks in memory and persist every 30 seconds. First observations,
window/AFK transitions, loss of an observed backend, pause and orderly shutdown
flush immediately. `Storage::events` overlays live ends, including events whose
persisted end is before the query boundary. Stage whole ticks before committing;
a failed write retains the previous overlay for retry. The existing durable
heartbeat/batch API flushes and invalidates cached tails before writing. No schema
change. A process crash can lose up to 30 seconds of stable interval extensions;
SQLite NORMAL/WAL durability remains unchanged. Call `Shared::shutdown` for
shutdown so pending data is flushed and the API is notified.

Fetch the last start timestamp inclusively for each displayed bucket, replacing
only that tail (equal-start events included). Keep the stale-load generation
check. Copy query results under the storage lock, build segments and resolve icons
after releasing it. Reload tags/ranges on a local CRUD revision or SQLite
`data_version` change; an external commit also invalidates event cursors. Rebuild
synthetic unobserved/untagged intervals from the merged tracks. Filter by indices,
cache lowercase search text, and intern segment strings as `Arc<str>` within each
loaded batch. Separate the timeline grid cache from event geometry.

Deliver tray menu events through an Iced channel subscription. Background workers
publish status only when relevant fields change; timestamps and idle-second
increments alone do not wake the GUI. Tick once per second with an open window;
keep ticks for explicit lifecycle/benchmark diagnostics. Windows joins Linux in
sampling independently of the GUI. macOS retains a one-second hidden-window tick
because AppKit sampling belongs on the main thread. The localhost API uses one
current-thread Tokio runtime and a Notify shutdown signal instead of polling.
Linux/Windows sampling subtracts work time from the one-second sleep.

Keep only the last foreground Windows process handle/path plus a reusable UTF-16
buffer. The retained process identity and exit check prevent stale paths on PID
reuse without an unbounded PID cache. Pipeline X11 window-property requests after
the dependent active-window query (two RTT phases, rather than one per property).
GNOME/KDE bridges push focus/title changes, with lightweight one-second liveness /
idle heartbeats. Preserve the old D-Bus methods for existing extensions. GNOME
resends focus when a new daemon has no sample; KDE additionally resends every 30
seconds for restart recovery. A missing bridge still expires after three seconds.

Validation: the 600-second stable two-bucket SQLite simulation writes 2,480,272
bytes of WAL with durable per-tick batches versus 90,672 bytes with buffered ticks
(27.4x less, autocheckpoint disabled for measurement). This is a write-volume
fixture, not a battery-life measurement. Tests cover overlay query boundaries,
periodic flush, rollback/retry, pause/shutdown persistence, incremental/full-history
equivalence with gaps and tags, string sharing and bridge heartbeats.


## 2026-10-14 — Plan evidence-based computer states before tagging

At the user's request, schedule M4.5 before M5 to investigate and implement
reliable local distinctions between active application use, AFK, sleep/off and
tracking health. Reserve `Unobserved` for diagnostic observation failures, with a
reason; keep pause, stopped recording and unexplained recording gaps separate.
Heartbeat gaps alone do not prove shutdown or AFK. Select platform mechanisms and
reconstruction rules only after the investigation; this decision changes the plan,
not the current tracker behavior or database schema.

## 2026-10-14 — M4.5 computer evidence separate from activity and collection health

Store `computer` events in the existing event schema with independent machine,
activity, recording-health, lock, window-availability, source and reason fields.
Keep AFK/window buckets compatible. The desktop uses legacy AFK only before the
first computer event; computer evidence then drives the Computer track, while
valid idle survives missing window permission. Unobserved is a diagnostic with a
reason; unexplained sampling gaps and restart gaps are No recording. Pause and
orderly stop remain explicit and never imply power-off. A restart after a pending
sleep marker does not extend sleep without a received wake boundary.

Register native lifecycle callbacks behind ComputerWatcher independently of focus
watchers. Linux uses logind, own graphical session LockedHint and a delay inhibitor;
Windows uses a dedicated hidden notification window with WTS initial state;
macOS uses main-thread NSWorkspace observers. Close observed work, persist the
state boundary and flush before returning from a pre-sleep/shutdown callback.
Inhibitor denial is exposed; a slow/failing disk may exceed the OS grace period.
Receipt-time spans include preparation/scheduling overhead. Clamp delayed callback
boundaries to the last stored end. Do not query focus while known locked/asleep.

Shutdown intent is not completed shutdown. Local-source research did not validate
reliable retained physical-off boundaries on all platforms, so no history parser
or inferred off interval is added. Unsupported lock/history capabilities remain
visible in Status & permissions. Full capability table, sources, limitations and
manual OS checks: `docs/computer-state-research.md`. No schema change.

Keep newly received OS state in memory even when its database marker fails. Only
extend a sleep/lock interval across a long gap if the stored tail already has the
same state, so a failed suspend marker cannot turn sleep into active time. A
SQLite-trigger regression test exercises this failure without requiring an OS.

## 2026-10-14 — M5 tag editing and away prompts

Tag hierarchy remains the existing `parent_id` tree; comma-separated paths reuse
non-archived siblings. A new migration 002 adds `archived` without changing the
original migration. Archive hides a tag from suggestions and retains its history.
Renaming updates all historical labels. Merge moves ranges, rules and children;
merging into a descendant is rejected. Confirmed subtree deletion removes its
ranges and rules; the dialog shows affected interval count and union duration.

The desktop uses core storage commands under the runtime storage mutex; it never
issues SQL. Core tag operations are atomic SQLite transactions. Split preserves
both outside portions of every intersecting block; Replace deletes entire
intersecting blocks. Edits, moves and resizes exclude their original block from
conflict handling. Canvas blocks retain their full persisted interval alongside
the day-clipped display interval, avoiding lost history across midnight.

Undo retains the last 20 tag actions in memory, including tags, ranges and rules.
It compares the current state inside the write transaction before restoring a
snapshot, rejecting undo after another local client edits metadata. Event history
is untouched. This is intentionally simple for M5; large tag histories should use
row deltas instead of snapshots. Undo does not survive restarting the application.

Away prompts use full computer-state intervals, independently of the viewed day.
Only contiguous, observed AFK followed by active use qualifies: awake, unlocked,
with recording or a valid activity observation during diagnostic `unobserved`.
Sleep/off/lock, pause, stopped/unknown recording and unexplained gaps never count
as AFK. Adjacent AFK samples with different diagnostic health merge for the prompt.
Prompts start at 15 minutes, have recent-tag shortcuts, and can be disabled or
changed to 30/60 minutes. The threshold persists locally in `away-prompt.json`.
Already completed intervals at application startup are not prompted again.


## 2026-10-14 — M5.5 desktop memory lifecycle

Use tray-icon's existing `ksni` feature on Linux instead of libappindicator/GTK3,
and rfd's xdg-portal + async-std (async-io) backend for folder selection. Keep the
native menu API and Windows/macOS providers. This removes GTK, GDK, Pango and
Cairo from the application's dependencies. Do not enable rfd's Tokio backend:
Cargo unifies zbus features across the portal, tray and watchers, and that backend
makes existing blocking calls outside a Tokio runtime panic. A private-bus SNI
fixture covers real menu dispatch; lifecycle diagnostics provide a fixture host
only when no panel exists, and log that distinction explicitly.

Start-minimized launches with no window or renderer. Close invalidates pending
loads and releases day tracks, summaries, search strings, tag metadata and icon
handles; unsaved editing state, selection, away prompt, metrics and undo remain.
Reopen reloads the day. Linux/glibc calls malloc_trim(0) behind MemoryReclaimer
when Iced emits Closed, after it drops window/compositor resources, and when a
late stale load is dropped without a window. Other platforms use a no-op.
Do not introduce a different global allocator.

Lowercase search strings exist only during a nonempty search and are shared by
Arc allocation identity. Cache entries retain the Arc to prevent pointer reuse;
every rebuild discards entries no longer referenced by the current tracks.
The previous incremental loader already truncated search entries at replace_from;
a regression test preserves correct results after changing a tail's title/app.

Read Tag/TagRange directly from SQLite; load only half-open intervals overlapping
the selected day, preserving original cross-day bounds. Migration 003 adds a
start_ts index. Recent-tag IDs and subtree delete totals use separate focused
queries; delete totals stream intervals and union their duration. Saving edits
checks overlaps against the requested bounds, not just the displayed day.
API JSON contracts are unchanged. SQLite cache_size=-512 bounds its suggested
page-cache budget per connection; it is not a hard cap on all SQLite memory.

Pass PNG/SVG bytes to Iced without base64, retain only icon availability in the
provider cache and known identities in the load cursor. Keep Status typed through
worker/UI/tray; JSON serialization remains at API/storage boundaries. One bounded
day-loading worker replaces Tokio spawn_blocking for snapshots. Theme and folder
operations retain their existing executor paths.

Measurements and platform checks: [desktop-memory.md](desktop-memory.md).


## 2026-10-14 — Separate Day overview in M7

Add the user-requested visual Day overview to M7 alongside reports, Week view,
export/import and CLI. Preserve the existing Day view for range selection and
editing; the overview summarizes active time with project/tag and application
rings, a duration legend and a compact full-day timeline. Use the existing Iced
canvas and interval calculations, with rendering-independent aggregates shared
with equivalent reports/CLI. This provides a visual summary without duplicating
tracking or changing the main editing workflow. Scope and acceptance criteria
are in PLAN.md §5.5 and M7; the supplied image is retained in
[day-overview-reference.png](references/day-overview-reference.png).


## 2026-10-14 — M5.6 resizable Day panels

Use Iced's existing PaneGrid for a horizontal divider between the activity
timeline and selection summary. Keep minimum pane height at 160 logical pixels,
with native hover/drag feedback and resize cursor. Alt+Up/Down adjusts the ratio
by five percentage points. Store only the two-pane layout in desktop State so
it survives day changes and hide/reopen within the process; no new dependencies,
database fields or persisted preferences are needed for this request.

Keep fixed timeline track heights and put the timeline content in a vertical
scrollable when its pane is reduced. This preserves alignment of labels, canvas
geometry and hit-testing, while the existing summary scrolls independently.
Panel resizing does not change the selected range, zoom, search or event data.
The separate chart-based overview remains scoped to M7. Native pointer-event tests
verify resize cursor and divider drag without accidental time selection. Local
Linux/Wayland fixture smoke, runtime and hide/reopen checks passed; real pointer
checks on Windows/macOS/X11 and persisted restart preferences are not included.

## 2026-10-14 — M6 rule assignment and safe history undo

Keep regex assignment in core Storage and call it from both Tracker and the
Rules screen. Clip matching window/web context to observed awake, unlocked,
active intervals with recording/unobserved health; use legacy AFK observations
only when there is no computer evidence in the queried interval. Larger
priority wins and the oldest rule ID resolves ties. Existing tag ranges win,
including previous automatic assignments; changes to rules affect future
recording and uncovered history, rather than silently rewriting manual edits.
Disabled rules and archived target tags do not assign time. URL matching is
ready for locally recorded web events; browser capture itself remains M9.

Add migration 004 for an enabled flag without modifying earlier migrations.
Validate regex syntax and cap compiled regex size with Rust's regex library.
Background assignments merge only the current automatically inserted tail
while its metadata revision and stored bounds are unchanged. Do not infer
activity across missing samples, pause, sleep, lock or unknown recording gaps.

Recompute history application inside an immediate write transaction and compare
with the confirmed preview. Retain only inserted rows in its undo record, and
verify those rows before removing them; later unrelated tracking/manual edits
must survive undo. Keep the existing bounded, process-local undo stack.

## 2026-10-14 — M6 local translations and OS language selection

Per the user's expanded scope, bundle eight catalogs: en, ru, de, fr, es, pt,
zh (Simplified) and ja. Use English source text as stable keys with named
placeholders and localized short duration units, avoiding plural inflection
for compact timeline/report labels. A catalog test verifies complete key sets
and matching placeholder names across all eight languages. User-entered tag
names, notes, document titles, executable names and stored API values stay intact.
Raw OS/library diagnostic details remain verbatim under translated error labels;
missing translations and unsupported languages fall back to English.

Read OS language behind LocaleProvider: Linux locale environment, macOS
AppleLanguages via the local defaults command, Windows user UI language via
kernel32. An explicit language overrides detection; choosing System default
redetects it. Persist an atomic language.json in the default application-data
folder, separately from the history folder selector. Rebuild canvas and tray
labels immediately when the language changes. Use local system fonts for CJK
fallback; native font coverage and translation quality require platform checks.

## 2026-10-14 — M7 reports, overview and local interchange

Keep the time partition in core::Report, independent of Iced. Sweep half-open
UTC millisecond boundaries of observed activity, application/web context and tag
ranges; count each active millisecond once. Use the highest tag-range ID for an
overlap, roll its tag up to the top-level project, and retain explicit Untagged,
Unknown application and Unknown document groups. Awake/unlocked active time with
unobserved window health remains active, with unknown context; sleep, off, AFK,
pause, stopped collection and unexplained gaps do not become work. Legacy AFK
ends at the first computer observation globally, including on later queries and
exports. Auto-tagging reuses the same observed-active predicate.

The editing Day view retains its existing summaries and selection behavior.
The overview converts its already classified tracks into the same core partition;
Week, Reports and CLI query core Storage directly. Reports use active time as
the denominator and top-level projects as tag groups/filters. GUI date ranges
and CLI calendar dates use consecutive local midnights, so DST days may have
23 or 25 hours; persisted/exported timestamps stay UTC milliseconds.

Use the existing Iced canvas for the donut and compact timelines, native legend
buttons plus explicit arrow-key highlighting, and bundled translations. Collapse
application slices smaller than 2% of the day's total into Other per project;
list their names and allow expansion without changing totals. Load report ranges
on a blocking worker, discard stale generations, and release cached report/day
partitions when the window closes. GUI custom ranges are capped at 366 days;
the existing 100,000-event query limit remains explicit for both GUI and CLI.

JSON export is a versioned history archive (events, tags and tag ranges), whereas
CSV is an aggregated report. Import supports that archive and ActivityWatch
currentwindow, afkstatus and web.tab.current buckets. Validate tags through the
existing CRUD boundary, remap IDs, then insert everything in one transaction.
Reject conflicting/overlapping event buckets and unsupported ActivityWatch bucket
types rather than silently losing multi-host data or breaking indexed queries.
Import never replaces existing history. No schema changes are required.

Ship draugr-time-cli as a separate console executable, particularly for Windows
where the GUI binary has the windows subsystem. It does not initialize Iced,
watchers, tray or autostart. Status alone reads the authenticated localhost API;
reports/export/import use the selected local database. No remote calls or new
third-party libraries were added; core reuses the installed chrono dependency
for ActivityWatch RFC3339 timestamps.

## 2026-10-14 — v0.2.0 branding and console distribution

Use the owner's four original PNG assets unchanged. README pictures select dark
or light variants; the native sidebar and About panel select the Draugr logo by
UI theme. Set the native window icon from the supplied Draugr image. The tray
uses the supplied crown's alpha mask at 32×32 with nearest-neighbor scaling and
the existing state colors, preserving tracking/paused/away/error feedback.
No downloaded assets or new image dependencies are needed.

Package the existing console binary alongside the GUI in every platform release
archive. Keep personal monthly reports and database verification snapshots under
/tmp, outside the published source tree. Ship the accumulated M5.5/M5.6/M6/M7 work
as v0.2.0; browser capture and installers remain future milestones.

## 2026-10-14 — Windows CI fix and v0.2.1

Windows CI rejected a needless return in the cfg-selected locale-provider branch;
remove it while retaining its existing fallback and API calls. Publish v0.2.1
from the corrected commit. The already-pushed v0.2.0 candidate tag stays intact,
and its failed check prevents the release workflow from publishing its binaries.

## 2026-10-14 — M5.5 bounded tag undo

Keep the existing 20-action desktop undo stack, but store typed before/after
rows only for changed tags, ranges and rules. Read affected rows within the
existing immediate SQLite write transaction; record actual inserted range IDs
because SQLite can reuse deleted IDs during a split. Avoid whole-history reads
and JSON snapshots, including when undoing applied rules. No persistent schema
change or new dependency is needed.

Undo validates changed rows within an immediate write transaction and restores
only those rows, with deferred foreign-key validation at commit. Unrelated
external edits now survive undo instead of blocking it; changed affected rows
or new dependents that would become orphaned reject undo atomically. A bulk
merge/delete/apply still retains memory proportional to its changed rows.

## 2026-10-14 — M5.5 report and Week memory follow-up

Keep one full Report for the selected range. Store only active time and tag/day
sums for the timesheet; Week additionally retains run-length compressed colors
in 1,440 elapsed-time bins per local day. Build status/diagnostic bands from
computer and legacy AFK buckets only, never application/document tracks. Exact
report intervals and totals remain independent of this visual approximation,
including cross-midnight boundaries and 23/25-hour local days.

Read history in complete event prefixes of approximately 8,192 events. Split
at the first omitted timestamp, include spanning events/live tails and keep the
global legacy-AFK cutoff. Calculate each prefix once, merge adjacent equivalent
intervals and intern label strings in one Arc<str> pool for the whole report.
Enable the existing serde dependency's rc feature to retain JSON string output.
The localhost events API keeps its 100,000-event safety limit; reports and JSON
archive export use prefixes to support larger ranges. Archive export deduplicates
spanning event IDs and still necessarily retains its raw export payload.

Week canvas geometry belongs to its loaded data and is cached by dimensions and
theme. Centralize screen changes so sidebar, day buttons, tray settings/quick
entry and away tagging all discard report data, invalidate pending results and
call the existing MemoryReclaimer. Stale completions are dropped and reclaimed.
Preserve Day editing state and the typed undo stack. No schema/platform/renderer
changes or new dependencies are needed; window buffers and fallback fonts are
measured as renderer resources, rather than removed.

## 2026-10-14 — v0.2.2 memory maintenance release

Publish the M5.5 undo/report/Week memory follow-ups and large-history reporting
fixes as v0.2.2. Reuse the existing three-platform CI and tag-triggered release
workflow; archives contain both desktop and CLI executables. Keep synthetic
measurement limits and remaining real-platform checks in the release notes.

## 2026-10-15 — M7 responsive history reads, export replacement and Week grid

Desktop reports and JSON export use Shared's complete event prefixes, releasing
storage between reads and aggregating outside the mutex. Reuse core archive
clipping and Report::calculate, merge spanning IDs/adjacent report intervals and
intern repeated report labels. Reject a changed metadata/external-writer revision
rather than exporting mixed edits. This remains a live read, not a database-wide
snapshot: ordinary tracker heartbeats may extend the live tail between prefixes.
The GUI reads status with try_lock and retains its previous status during
contention. macOS sampling likewise uses a nonblocking status snapshot before
calling its existing main-thread watcher. Preserve the control/storage lock order
and tracker pause/lifecycle synchronization.

Use the already-installed tempfile crate directly in the app. Write and sync a
unique temporary file beside the selected destination, then persist it over the
confirmed target. A failed write/replacement leaves the old destination intact
and removes the temporary file. Platform save dialogs still own overwrite consent.

Keep legacy AFK's global first-computer cutoff, already used by the Day runtime,
core reports and archives. Remove empty/reversed Week intervals before classifying
them. Week refresh computes its dates locally and preserves Reports' custom range.

Week uses one cached canvas with a common local wall-clock axis, native date
buttons and colored tag legend controls. Cache daily clock extents and use slices
of the sorted report for hover queries. Keep bounded elapsed-minute state bands;
map them through real local-hour boundaries for the shared grid. Missing DST
hours remain empty; repeated wall hours occupy the same column. Exact UTC
intervals, real durations and daily totals remain authoritative. The overview's
elapsed-time axis uses real local-hour timestamps, including missing/repeated
hours, instead of dividing a DST day into 24 equal durations.

Crop Week to whole hours around observed active intervals and today's current
time; empty weeks retain 00–24. Highlight today, draw its current-time marker and
leave future time blank. Hover merges contiguous intervals of the same tag for
the tooltip and highlights that tag throughout the week. Native legend buttons
retain keyboard access to the same highlight; date buttons open Day. Compare
active-time totals with small bars scaled against the week's busiest day.

## 2026-10-15 — v0.2.3 Week and responsiveness release

Publish the M7 Week redesign and report/export correctness fixes as v0.2.3.
Reuse the existing tag-triggered, three-platform CI/release workflow. Keep its
build gating and package layout unchanged; record behavior, local validation
and remaining platform checks in docs/releases/v0.2.3.md.


## 2026-10-15 — M7 report sweep allocation and bucket dispatch

Keep the shared report algorithm and its overlap precedence, but collect clipped
boundary changes in one timestamp-sorted vector and process equal-time batches.
Use dedicated computer/AFK/window/web index sets instead of a dynamic bucket map;
unknown bucket boundaries remain harmless. This removes per-boundary allocations
and repeated hashing without changing report, UI, API or CLI output. Preserve
arbitrary input order and overlapping events with the existing `(id, index)`
sets. Criterion results and the frozen original-sweep correctness oracle are
recorded in BENCHMARKS.md; Criterion is development-only.


## 2026-10-15 — M7 overview legend and timeline scrolling

Use the existing application icon handles and aligned native controls for the
overview name/duration/percentage table. Cache the compact timeline geometry
between redraws so scrolling does not rebuild all interval shapes. Clear the
cache whenever the overview view is rebuilt, preserving updates to data,
highlight, theme and date; canvas also invalidates on size changes.

## 2026-10-16 — M10 release 1.0

- Follow the updated upstream PLAN: M10 contains icons, built-in Unix CLI,
  two-screen onboarding and minimal privacy. Defer installers to M11 and
  backups/title masking to M12.
- Keep onboarding, exclusions and schedule in atomically replaced `settings.json`
  in the active data directory. Corrupt preferences stop tracking startup.
- Filter excluded window observations before storage, rules and current status,
  retaining independent active/away evidence. Delete intersecting event/tag ranges
  transactionally, preserve outside pieces, invalidate live tails and UI undo.
- Schedule against the local wall clock with weekday/everyday, same-day HH:MM
  windows; persist interval timestamps as UTC milliseconds. Off-hours recording
  is an explicit pause and never creates application time across the gap.
- Route Unix CLI arguments before initializing the GUI; reuse the existing CLI.
  Embed Windows resources with winresource, ship a macOS `.app` in its archive,
  and install a user-scoped Linux desktop entry/hicolor icon on explicit request.
- Add migration 005 and nullable `tag_ranges.precedence_id`: retained fragments
  keep their original creation precedence instead of overtaking newer tags.
  Reports, undo and archive import preserve this ordering; existing ranges use ID.

## 2026-10-17 — Event-driven GUI wakeups and Linux theme signals

- **One-second tick.** Linux and Windows already sample on a worker that pushes
  status changes (`tray::Event::Status`), so the GUI timer only has to load
  history. It now runs while a window is open and either the shown day has not
  been loaded yet, or the Day screen shows a day that still contains the current
  time. A loaded past day, the other screens and the preview no longer wake the
  GUI every second. The tick body is unchanged. macOS keeps ticking because AppKit
  sampling stays on the main thread; lifecycle, benchmark, smoke, runtime and
  onboarding checks keep their ticks. Leaving the Day screen pauses today's
  refresh, as closing the window already did; returning resumes it within a
  second. The decision lives in `ticking::TickDemand` with unit tests.
- **Linux theme.** Replace the two-second `ReadOne` poll with a subscription to
  the XDG portal's `SettingChanged` signal over the existing zbus stack. The
  stream reads the current value after subscribing, then emits only
  `org.freedesktop.appearance` / `color-scheme` changes. Iced's
  `linux-theme-detection` would do the same through the extra `mundy` crate with
  its own connection; reusing zbus adds no dependency.
- **Renderer.** tiny-skia stays the only renderer. An opt-in `gpu` feature backed
  by wgpu was considered but rejected on 2026-10-17: a mostly-hidden tracker pays
  more in resident/device memory and shader compilation than it saves in redraw
  CPU, so the extra dependency is not worth it.

## 2026-10-17 — M11 installers

- **About links.** The Settings → About panel gains two clickable buttons that
  open the user's default browser: the GitHub repository and the creator
  `hlophlopgaming` (profile link). Opening is user-initiated, so it does not
  violate the no-network rule. `open_url` uses the OS launcher (`xdg-open` /
  `open` / `cmd /C start`) through `std::process::Command`; no new dependency.
  The URLs are `&'static str` constants, so no link construction is needed.
- **Windows installer.** Inno Setup installs both executables per user
  (`PrivilegesRequired=lowest`, `%LOCALAPPDATA%\Programs\Draugr Time`), offers a
  Start-menu/desktop shortcut and adds the install directory to the user PATH so
  `draugr-time-cli` works from a terminal. The installer only removes files it
  installed, so uninstalling leaves history in `%LOCALAPPDATA%\DraugrTime` in
  place. A portable zip is still produced for users who prefer it.
- **macOS DMG.** The existing `.app` bundle is placed in a UDZO DMG with a
  `/Applications` symlink for drag-install; `hdiutil` builds it in CI.
- **Linux AppImage.** A hand-assembled AppDir (`usr/bin/draugr-time`, `AppRun`,
  the desktop entry and hicolor icon) is packed with `appimagetool`. The desktop
  entry keeps `Exec=draugr-time` and the icon name `org.draugrtime.app`.
- **Flatpak deferred.** Per the user's request, Flatpak is deferred to a later
  pass; it needs a generated offline cargo-sources manifest and the GNOME SDK in
  CI, which is disproportionate for this milestone. It is recorded in PLAN's
  "Later" list.
- **Release CI.** Packaging runs in the per-OS `check` job (which already builds
  the release binary) and uploads the installer as an extra artifact; the
  `release` job renames and attaches every installer next to the existing
  archives. No new secrets or signing: Windows and macOS outputs stay unsigned.

## 2026-10-18 — AppImage login startup path

- The Linux startup registration used `std::env::current_exe()`, which inside an
  AppImage points into the temporary squashfs mount (`/tmp/.mount_…/usr/bin/…`)
  that is removed when the process exits, so the XDG autostart entry never
  launched at login. Startup now prefers the persistent `$APPIMAGE` path (set by
  the AppImage runtime) and falls back to `current_exe()` otherwise, so the
  autostart `Exec=` targets the `.AppImage` file itself.

## 2026-10-22 — Release profile and lighter tracker tick

- **Release profile.** `[profile.release]` now uses `lto = "fat"`,
  `codegen-units = 1` and `strip = true` for smaller shipped binaries and
  cross-crate inlining, at the cost of longer release builds in CI.
  `panic` stays `unwind`: the daemon turns a panicked API thread into an error
  through `JoinHandle::join`, which `abort` would replace with a crash.
- **Tracker tick.** The worker reads the sampling gate (asleep, locked, paused)
  under the control lock instead of cloning the whole `Status`, keeps one status
  copy per tick for tray change detection, and writes `computer_capabilities`
  only when it changed. Behaviour is unchanged.
- **Day tracks.** `model::tracks` indexes tags by id and computes each tag path
  once per load, shared as `Arc<str>` between segments of the same tag.
