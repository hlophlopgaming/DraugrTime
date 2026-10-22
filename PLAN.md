# PLAN.md — Open-source, cross-platform ManicTime alternative

> Working name: Draugr Time (do **not** use "ManicTime" in the name — it is a trademark).

## 0. Positioning

ActivityWatch already exists as an open-source cross-platform tracker (MPL-2.0). What it lacks compared to ManicTime:

- a proper **timeline** with manual time tagging (projects/tasks),
- good "how much did I work on X" reports,
- solid **Wayland** support.

Decision: build our own project, but keep the **event format compatible** with ActivityWatch so we can import its data and reuse its browser extensions.

---

## 1. Scope

### MVP
1. Automatic tracking: active application, window title, computer state (active / AFK / asleep / off). Distinguish confirmed OS states from tracking failures and periods without recording; investigate reliable platform evidence in M4.5.
2. **Desktop app with a ManicTime-style Day view**: timeline with tracks (Tags, Computer usage, Applications, Documents/URLs), range selection and a summary panel (see §5).
3. **Tags**: select a range on the timeline → assign a project/task. This is the core ManicTime feature.
4. Auto-tagging rules (regex on app / title / URL).
5. Reports and stats for day/week; CSV/JSON export.
6. Fully local, offline, private by default.

### Out of MVP
Screenshots, multi-device sync, server edition, mobile clients.

---

## 2. Tech stack

| Layer | Choice | Reason |
|---|---|---|
| Core / daemon | **Rust** | Single binary per OS, low overhead, good OS bindings |
| GUI | **Iced** (Rust, tiny-skia) | Native cross-platform UI and canvas, no webview |
| Storage | **SQLite** (rusqlite or sqlx) | Local, single file, easy backups |
| Timeline | Canvas/SVG (vis-timeline or custom d3) | Needs zoom and range selection |
| CI | GitHub Actions, matrix windows/macos/ubuntu | Agent can verify builds on all OSes |
| License | GNU GPLv3 |

---

## 3. Architecture

One desktop executable contains the Iced window, native watchers, background
tracking, SQLite storage and the localhost API. No separate daemon executable
is required. Closing the window hides it to the tray and keeps tracking; Quit
stops the entire application. M7 adds a user-facing CLI for status, reports,
export and import, alongside the existing developer diagnostics.
Data defaults to the OS application-data directory. Settings can select another
folder for the next launch; existing history remains in its original folder.

- **Watchers** live behind traits, with a mock implementation for tests:

```rust
pub trait WindowWatcher {
    fn active_window(&self) -> Result<Option<WindowInfo>>;
}

pub trait IdleWatcher {
    fn idle_seconds(&self) -> Result<u64>;
}

pub struct WindowInfo {
    pub app: String,       // executable / bundle id
    pub title: String,
    pub pid: Option<u32>,
}
```

### Data model (heartbeat approach, ActivityWatch-compatible)

```sql
CREATE TABLE events (
    id        INTEGER PRIMARY KEY,
    bucket    TEXT NOT NULL,      -- 'window' | 'afk' | 'web'
    start_ts  INTEGER NOT NULL,   -- unix ms, UTC
    end_ts    INTEGER NOT NULL,
    data      TEXT NOT NULL       -- JSON
);
CREATE INDEX idx_events_bucket_start ON events(bucket, start_ts);

CREATE TABLE tags (
    id        INTEGER PRIMARY KEY,
    name      TEXT NOT NULL,
    color     TEXT,
    parent_id INTEGER REFERENCES tags(id)
);

CREATE TABLE tag_ranges (
    id       INTEGER PRIMARY KEY,
    tag_id   INTEGER NOT NULL REFERENCES tags(id),
    start_ts INTEGER NOT NULL,
    end_ts   INTEGER NOT NULL,
    note     TEXT
);

CREATE TABLE rules (
    id       INTEGER PRIMARY KEY,
    field    TEXT NOT NULL,       -- 'app' | 'title' | 'url'
    pattern  TEXT NOT NULL,       -- regex
    tag_id   INTEGER NOT NULL REFERENCES tags(id),
    priority INTEGER NOT NULL DEFAULT 0
);
```

**Heartbeat merging:** watchers send a heartbeat every N seconds. If `data` equals the last event's data in the same bucket and the gap is `< pulsetime`, extend `end_ts`; otherwise insert a new event.

---

## 4. Platform specifics

| OS | Active window | Idle time |
|---|---|---|
| **Windows** | `GetForegroundWindow` + `GetWindowTextW` + `GetWindowThreadProcessId` → exe name | `GetLastInputInfo` |
| **macOS** | `NSWorkspace.frontmostApplication`; title via Accessibility API (requires permission) | `CGEventSourceSecondsSinceLastEventType` |
| **Linux X11** | `_NET_ACTIVE_WINDOW` + `_NET_WM_NAME` (x11rb) | XScreenSaver extension |
| **Linux Wayland** | No universal API. GNOME: shell extension exposing D-Bus; KDE: KWin script; Sway/Hyprland: compositor IPC | `ext-idle-notify-v1`, or `org.freedesktop.ScreenSaver` / logind `IdleHint` |

**Browser URLs:** window titles are not enough, so a browser extension (Firefox + Chromium) sends the active tab's URL/title to the daemon on `localhost`.

---

## 5. Application UI (modeled on ManicTime)

The desktop app is the product. Background tracking is embedded in the desktop process; M7 adds a CLI for terminal use and scripting. The UI follows ManicTime's proven model: **one main "Day view" where you see the timeline, select time and tag it**, plus a few supporting screens. Copy the interaction model, not the look or branding.

### Design principles
- **Desktop-dense, not mobile-airy.** Lots of information on one screen, compact rows, small margins.
- **Keyboard-first.** ←/→ previous/next day, `T` today, `+`/`-` zoom, `Enter` tag the selection, `Ctrl/Cmd+F` search.
- **Selection drives everything.** Selecting a range on any track updates the summary panel and enables tagging.
- **Never silent.** If tracking is paused, blocked by permissions or unsupported (Wayland), it is visible on screen and in the tray.
- Light and dark themes following the OS. Local time zone in the UI; UTC everywhere else.

### Window layout

```
┌──────────────────────────────────────────────────────────────────────┐
│ ◀  Tue, 13 Oct 2026  ▶  [Today]  [📅]       🔍 Search   ● Tracking ▾│  toolbar
├────────┬─────────────────────────────────────────────────────────────┤
│ Day    │  08    09    10    11    12    13    14    15    16    17  │  time ruler
│ Week   │ Tags      ██Project A██      ░░░░  ██Client B████          │
│ Reports│ Computer  ████████████  ▒▒▒  ███████████████████  ▒▒       │  active / away / off
│ Tags   │ Apps      ██▓▓██░░████        ██▓▓▓▓██████░░████            │  colored per app
│ Rules  │ Documents ▏▏▏ ▏▏▏▏ ▏ ▏▏         ▏▏▏▏ ▏▏▏ ▏▏▏▏               │  titles / URLs
│        ├─────────────────────────────────────────────────────────────┤
│        │ Selection 10:12–11:47 (1h 35m)        [Tag selection ⏎]     │
│        │ ┌ Tags ┬ Applications ┬ Documents ┐                          │  summary panel
│        │ │ Project A   1h 10m  ██████████  74%                         │
│ ⚙      │ │ (untagged)    25m   ███         26%                         │
└────────┴─────────────────────────────────────────────────────────────┘
```

### Screens

**1. Day view (main screen, ManicTime's "Day view")**
- Toolbar: previous/next day, Today, date picker, search, tracking status pill (Tracking / Paused / Away / Error) with a dropdown: **Don't track for** 5 / 10 / 15 / 30 / 60 min (timed pauses only, see §5.10).
- Timeline tracks, top to bottom: **Tags**, **Computer usage** (active / AFK / asleep / off, plus explicit recording gaps and diagnostic `Unobserved`), **Applications** (stable color per app), **Documents** (window titles, URLs from M9). Track order and visibility configurable.
- Zoom from whole day down to minutes; scroll horizontally; hover shows app, title, start–end, duration.
- Click a block to select it; drag to select any range; Shift-click to extend. Double-click an app block selects its continuous span.
- **Summary panel** under the timeline with tabs *Tags / Applications / Documents*: duration and share for the current selection, or the whole day if nothing is selected. Clicking a row highlights its blocks on the timeline.
- A draggable horizontal divider between the timeline and summary adjusts panel heights; Alt+Up/Down provides keyboard resizing. Keep minimum pane heights and independent scrolling, and retain the ratio while the application is running (M5.6).
- Day totals in the corner: active time, away time, tagged vs untagged.

**2. Tagging (popup on the Day view)**
- `Enter` or "Tag selection" opens a small popup: text field with autocomplete from existing tags and recent ones.
- Hierarchical tags typed with commas, as in ManicTime: `Client B, Website, Design`.
- Optional note. Tagging an already-tagged range asks: replace, or split around it.
- Edit/move/resize/delete tag blocks directly on the Tags track; undo with `Ctrl/Cmd+Z`.

**3. "While you were away" prompt**
- When the user returns after away time ≥ threshold (default 15 min), a small non-modal window: *"You were away 10:30–11:15 (45 min). What were you doing?"* → quick tag (recent tags as buttons), ignore, or "don't ask for gaps shorter than…".
- Answers create tag ranges on the away period (meetings, calls, lunch).

**4. Week view**
- Seven days stacked as compact timelines; click a day to open it in the Day view. Weekly totals per tag.

**5. Reports**
- Date range (day / week / month / custom), group by *tag / application / document*, optional filter by tag.
- Table with durations + one bar chart. "Timesheet" layout: tags × days grid with daily totals.
- Separate **Day overview**, inspired by the [supplied Chronoscope reference](docs/references/day-overview-reference.png): date navigation at the top; a large two-level donut chart on the left; a grouped legend with durations on the right; a compact full-day timeline below. Keep the existing Day view for selection and tag editing.
- Inner ring: projects/tags, using the top-level tag in each hierarchy. Outer ring: applications within each project/tag group. Use the tag color and related shades for its applications; show active untagged time as an explicit group. The center shows total active time for the selected local day.
- Chart slices partition observed active time. Split intervals at tag/application boundaries and clip to the local day's UTC bounds; a parent and its descendant must not count the same time twice. For overlapping unrelated tags, use the most recently created range (highest range ID) for that overlap, and explain this assignment in the overview. If active time has no observed application, retain it as an explicit "Unknown application" slice.
- Hovering a slice or legend row shows duration and share of total active time; clicking highlights the corresponding timeline intervals. Provide the same information and highlighting through keyboard-accessible legend controls. Aggregate tiny slices into "Other" within each group, with a way to inspect the included applications. The legend stacks below the chart in narrow windows; support both themes.
- The bottom timeline distinguishes tagged/untagged activity, AFK, sleep/off, pause, recording gaps and diagnostic Unobserved. Inactive or unrecorded time stays visible there and is excluded from the active-time donut. Empty/zero-active-time days show an explanation instead of a misleading chart.
- Export CSV / JSON; copy as text.

**6. Tags manager**
- Tree of tags (client → project → task), rename (updates history), merge, color, archive, delete with confirmation showing affected time.

**7. Auto-tag rules**
- List of rules: field (app / title / URL), pattern, tag, priority, enabled.
- **Live preview**: "this rule would tag 3h 20m in the last 7 days", with sample matches.
- "Apply to history" for a date range, with a confirmation and undo.

**8. Settings**
- *Language*: detect and follow the OS language by default; let the user select a language or return to "System default" in Settings, save the choice across restarts, and fall back to English when a translation is unavailable.
- *Tracking*: away threshold, away prompt on/off, start with system, start minimized to tray.
- *Privacy*: excluded apps (not recorded at all), title masking per app, "Don't track for" shortcuts, "delete data for range".
- *Data*: database location, backup now / automatic backups, import (ActivityWatch history and categories), export (Draugr Time archive, ActivityWatch categories).
- *Status & permissions*: per-backend status (Windows / macOS Accessibility / X11 / Wayland GNOME/KDE/wlroots / browser extension) with a fix-it hint for each problem.
- *About*: version, license, link to the repo.

**9. First-run onboarding (3–4 steps)**
- What is tracked and that it never leaves the machine → grant permissions (macOS Accessibility; on Wayland: install the GNOME extension / KWin script or explain wlroots) → done, open Day view.
- Recording starts as soon as the app opens; there is no separate "Start" step.
- On first launch, while autostart is off, ask once in a small dialog: *"Start Draugr Time with your computer? Tracking only works while the app is running."* → **Yes** / **No**. "No" is remembered and never asked again; the choice stays editable in Settings → Tracking → Start with system.

**10. System tray / menu bar**
- Icon shows state: tracking / paused / away / error.
- Menu: open Draugr Time, current activity, today's active time, **tag the last N minutes** (quick tag), **Don't track for…** 5 / 10 / 15 / 30 / 60 min, Settings, Quit.
- **Timed pauses only.** "Don't track for…" is the user's way to say *I'm AFK, don't record this*. Picking a preset stops recording at once, with no confirmation, and recording resumes by itself when the time ends. There is no "pause until resumed" in the tray or the window: people forget to turn tracking back on and lose hours of history. While paused, the tray icon and the status pill show the paused state and the remaining time, with **Resume now** and **+5 / +15 min**. The paused range appears on the Computer usage track as a manual pause. The recording schedule (§5.8) and excluded apps remain the way to stop recording for longer.
- Closing the main window keeps tracking in the background.

### Empty and error states (required on every screen)
- No data yet → short explanation + "tracking started at 09:41".
- Daemon not running → banner with "Start tracking" button.
- Backend unsupported or permission missing → banner linking to Settings → Status & permissions.

### UI tech decisions (to be recorded in `docs/decisions.md` in M3.5)
- Framework: Iced in Rust with the tiny-skia renderer; no remote assets.
- Timeline rendered on the native Iced **canvas**; native controls for everything else.
- UI uses in-process tracking commands and snapshots from the runtime. The UI never issues SQL directly; the localhost API remains available for local integrations.
- Rust tests for summary math and selection; native smoke/runtime/lifecycle checks with isolated fixture data.

---

## 6. Milestones

Each milestone = one focused AI session. Do not start the next until the acceptance criteria pass.

### M0 — Skeleton
- Cargo workspace: `core`, `watchers`, `daemon`, `app` (Iced).
- CI matrix on 3 OSes: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.
- ✅ Empty Iced window builds in CI on Windows, macOS and Linux.

### M1 — Storage & heartbeat logic
- SQLite schema + migrations; `heartbeat(bucket, data, ts, pulsetime)`.
- ✅ Unit tests: identical events merge; window change splits; gap > pulsetime splits.

### M2 — Watchers: Windows, X11, macOS
- Trait implementations + `MockWatcher`.
- ✅ `tracker debug` CLI prints the current window and idle seconds every second.

### M3 — Daemon & local API
- Polling loop; AFK logic (idle > 5 min → `away`, configurable).
- REST: `GET /events?bucket&from&to`, CRUD for `/tags`, `/tag_ranges`, `/rules`, plus `GET /status` (tracking state, per-backend status and errors) and `POST /pause` / `POST /resume`.
- ✅ Integration test with `MockWatcher`: a simulated 10-minute scenario produces the expected events.
- Wayland backends: GNOME (D-Bus + extension), KDE (KWin script), wlroots (IPC).
- ✅ Wayland graceful fallback: if no backend is available, `/status` reports it and the UI (M3.5) clearly says so instead of silently recording nothing.

### M3.5 — App shell
- Use Iced and the Rust desktop build (see §5 "UI tech decisions"); record architecture decisions in `docs/decisions.md`.
- Main window with sidebar navigation (Day, Week, Reports, Tags, Rules, Settings) and placeholder screens.
- Toolbar with date navigation and the tracking status pill; Pause / Resume.
- **Tray / menu bar icon** with state and the menu from §5.10 (quick tag may be disabled until M5). Closing the window keeps tracking.
- Settings → Status & permissions screen showing live `/status`.
- Empty and error states: daemon not running, backend unsupported, permission missing.
- ✅ App launches on all 3 OSes in CI; on fixture/mock daemon data the status pill, tray state and Status screen reflect tracking / paused / error; pause from tray is reflected in the window and vice versa.

### M4 — Day view & timeline
- Day view from §5.1: tracks Tags, Computer usage, Applications, Documents; zoom (day ↔ minutes), scroll, hover details; stable per-app colors.
- Range selection (click, drag, Shift-click) and the **summary panel** (Tags / Applications / Documents tabs) for the selection or the whole day.
- Day totals; keyboard navigation between days.
- ✅ Renders fixture data with 5 000+ events smoothly; selection math (durations, shares) covered by tests; summary updates on selection change.

### M4.5 — Computer state evidence and diagnostic gaps
- First investigate which local OS signals and retained system history can establish sleep/resume, shutdown/boot, session lock/unlock and user idle time on Windows, macOS and Linux (X11 / Wayland). Document sources, permissions, retention limits, restart recovery and cases that cannot be determined reliably. No network access is required.
- Define separate dimensions: computer/session state (awake, asleep, off, locked where detectable), user activity (active / AFK), and tracking health (recording, paused, stopped, unavailable). A focused application is observed context, not proof that the user is active; a locked session is not proof of shutdown.
- Show active use and its observed application, AFK, and evidence-backed sleep/off intervals distinctly on the timeline. Keep `Unobserved` as a diagnostic state when collection is expected to work but cannot observe activity (backend failure, missing permissions, unavailable window details). Preserve an explicit reason and the affected backend; a missing window must not discard a valid idle/computer-state observation.
- Represent manual pause, tracker not running and gaps with insufficient evidence separately. Never infer AFK, sleep or shutdown merely from a heartbeat gap or application exit. After a crash or power loss, reconstruct only what reliable local evidence supports; retain the rest as an unknown recording gap, rather than mislabelling it as a watcher failure.
- After the investigation, record the chosen evidence and interval-boundary rules in `docs/decisions.md`, then implement supported states behind platform traits with mocks. Preserve Unix UTC milliseconds, use new migrations if persistence changes, and define how power/session events flush the buffered live tails.
- ✅ Research deliverable: a per-platform capability table with source references, confidence/limitations, required permissions and manual verification steps. Unsupported distinctions remain explicit.
- ✅ Tests first: MockWatcher / mock lifecycle scenarios cover active application → AFK → return, focus unavailable with valid idle samples, lock/unlock, suspend/resume, evidenced shutdown/boot, pause/resume, tracker restart/crash and an unexplained recording gap. Verify interval boundaries, day totals, absence of invented work across sleep/off, and diagnostic `Unobserved` reasons independently of the known computer state.
- ✅ UI/API retain the state and its evidence/reason; real supported-platform checks verify sleep, wake, reboot and backend failure. Run this milestone before M5 so the away prompt can distinguish AFK from sleep/off and collection gaps.

### M5 — Tags (core feature)
- Tagging popup from §5.2: autocomplete, comma-separated hierarchy, note; replace/split on overlap; edit, resize, move, delete on the Tags track; undo.
- **"While you were away" prompt** from §5.3, using M4.5 state classification: AFK is distinct from sleep/off, manual pause and collection gaps.
- Tags manager screen (§5.6).
- Tray "tag the last N minutes" enabled.
- ✅ Tested: total time per tag per day is correct, including overlaps, splits and day boundaries; away prompt creates the right tag range.

### M5.5 — Desktop memory usage
- Linux tray uses StatusNotifierItem over D-Bus through the existing tray-icon ksni backend; folder selection uses xdg-desktop-portal. Remove GTK3/libappindicator dependencies and their event loop.
- Closing the window releases day data, summaries, search cache, icon handles and tag metadata; invalidate pending loads and reload on reopen. Preserve unsaved edits and undo. On Linux/glibc, trim freed allocator arenas after the window renderer is destroyed.
- Build lowercase search strings only during search, share cached strings by their Arc identity and discard the cache when search is cleared. Verify incremental tail replacement.
- Load typed tag metadata and only ranges overlapping the selected day using an indexed query and a new migration. Keep recent-tag suggestions, overlap detection outside the day and whole-history delete confirmation correct through focused queries.
- Pass icon bytes directly without base64 or a process-lifetime image cache. Use typed runtime status and a dedicated day-loading worker; limit the SQLite page cache to 512 KiB per connection.
- ✅ Tests first: close/reopen and stale loads, lazy search and changed tails, tag query boundaries/migration, recent tags/global delete confirmation, overlap outside the viewed day, raw icon handling and tray updates without GUI ticks. Run fmt, clippy and tests.
- ✅ Compare PSS for tray-only startup and open/close cycles on Linux; document results and limitations. Manually verify tray menus and portal folder selection on X11/Wayland, plus close/reopen and tray behavior on Windows/macOS.
- Implemented and checked locally: [memory measurements and remaining platform checks](docs/desktop-memory.md).

### M5.6 — Resizable Day panels
- Add a draggable horizontal divider between the existing activity timeline and summary panel. Resize their heights within the current window; keep fixed track heights and allow vertical scrolling when the timeline pane is smaller than its content.
- Use Iced's existing PaneGrid, with visible hover/drag feedback, a vertical-resize cursor and minimum pane heights. Alt+Up/Down offers keyboard resizing. Preserve selection, zoom, filters and the split ratio across date changes and close/reopen within the same process.
- ✅ Tests first: real widget drag emits a resize event, both panes stay usable at extreme ratios, and resizing leaves activity data/selection unchanged. Verify keyboard resizing and retained layout after data cleanup/reopen; run fmt, clippy and tests.
- ✅ Native fixture smoke/lifecycle checks and manual divider drag/scroll/selection checks on Linux X11/Wayland, Windows and macOS. This milestone changes only the existing Day layout; the separate M7 overview remains planned.
- Implemented with native mouse-event regression tests; Linux/Wayland fixture smoke, runtime and close/reopen checks passed. Manual pointer drag/scroll/selection checks on real Windows/macOS/X11 sessions remain.

### M6 — Auto-tagging rules
- Regex rules on app/title/url with priorities; "apply to history" action.
- Rules screen (§5.7) with live preview and undo for "apply to history".
- ✅ Tests for rule matching and priority resolution; preview totals match the applied result.
- Interface localization: English, Russian, German, French, Spanish, Portuguese, Simplified Chinese and Japanese; translate screens, dialogs, tray menus and user-facing errors. Bundle translations locally.
- Detect the OS language on first launch and whenever "System default" is selected. Language selection in Settings (§5.8) overrides automatic detection, persists across restarts and offers a return to "System default"; use English when the detected language or a translation is unsupported.
- ✅ Tests cover language selection, persistence and fallback; manually verify both languages on all screens and in the tray, including long labels and pluralized durations.

- Implemented locally: [M6 usage, tests and remaining platform/language checks](docs/m6.md).

### M7 — Day overview, Week view, reports, export & CLI
- Week view (§5.4) and Reports screen (§5.5) with timesheet layout.
- Separate Day overview from §5.5: two-level project/tag → application donut, active-time total, interactive duration legend, date navigation and compact full-day timeline, following the supplied visual reference. Preserve the existing Day view for editing.
- Draw the overview with the existing Iced canvas and native controls; reuse interval/summary logic and local data. Keep aggregate computation independent of rendering and share it with report/CLI outputs for the same grouping and time basis.
- CSV/JSON export; ActivityWatch import (Settings → Data).
- User-facing CLI: status, reports for a selected date range grouped by tag/application/document, CSV/JSON export and ActivityWatch import, without opening a GUI window. Reuse the same business logic as the desktop UI.
- CLI supports `--help`, machine-readable JSON output, errors on stderr and nonzero exit codes on failure. All operations remain local; timestamps in machine-readable output use Unix UTC milliseconds.
- ✅ Export → import round-trip test; report totals equal the sum of Day view totals for the same range.
- ✅ Tests first for the overview: project/application totals and slice shares sum to observed active time; hierarchy and overlapping tags do not double-count; untagged and unknown-application time remain explicit. Cover cross-midnight intervals, local days with DST changes, AFK/sleep/off/pause/recording gaps and empty days. Overview, timeline selection and equivalent report/CLI totals agree.
- ✅ Overview renders 5 000+ fixture events smoothly; hover/click/keyboard highlighting, small-slice expansion, both themes and narrow-window layout receive native UI checks on supported platforms.
- ✅ CLI integration tests cover help, report/export parity with the UI on fixtures, import, invalid arguments and failure exit codes; commands run without a graphical session on Linux and receive manual terminal verification on Windows/macOS.

- Implemented locally: [M7 behavior, verification and remaining platform checks](docs/m7.md).

### M8 — Reserved
- Wayland scope and acceptance criteria moved to M3 at the user's request.

### M9 — Browser extension (dropped)
- Not needed: the active browser window title already names the page on the Documents track.
- Firefox/Chromium extensions moved to **Later**; revisit only if there is a real request.

### M10 — Release 1.0
- **Application icon everywhere:** embedded in the Windows `.exe`; `.desktop` file and hicolor icons on Linux; `.icns` on macOS.
- **CLI:** built into `draugr-time` on Linux/macOS (`draugr-time report ...`); separate `draugr-time-cli.exe` on Windows.
- **First-run onboarding (1–2 screens, §5.9):** what is recorded and where it is stored (nothing leaves the machine); permission hints (macOS Accessibility, GNOME/KDE Wayland bridge); "Start recording".
- **Minimal privacy settings (§5.8):** excluded applications (never recorded), delete data for a selected range, recording schedule (e.g. weekdays 9–19).
- **README:** drop "Early development", describe downloads and usage.
- ✅ First launch from a release archive on each OS goes through onboarding to a working Day view; the app shows its icon in the window, tray and file manager/app menu.

- Implemented: [M10 behavior, verification and real-OS checks](docs/m10.md).

### M11 — Release 1.1: installers
- In the app’s About screen, add a clickable link to the GitHub repository and credit its creator, **hlophlopgaming**, with a link to the GitHub profile.
- Windows: installer (Inno Setup or MSI) that adds `draugr-time-cli` to PATH, plus the portable zip.
- Linux: AppImage (Flatpak deferred to Later).
- macOS: `.app` bundle in a DMG.
- Release CI builds and attaches every installer.
- ✅ Fresh install on each OS goes through onboarding to a working Day view without touching a terminal; uninstall leaves history in place unless the user deletes it.

### M11.1 — Title masking
- Settings → Privacy gains a **Masked applications** list next to Excluded apps (§5.8). Same matching as exclusions: exact application IDs or executable paths, semicolon-separated, ASCII case insensitive.
- Time for a masked application is recorded as usual, but its window title is replaced with the fixed placeholder `[masked]` in the daemon before it reaches storage, rules or current-activity status. Exclusion wins when an application is in both lists.
- Applies to new activity only; existing history is not rewritten. Settings files without the new list keep loading.
- ✅ Tests: masked titles never reach storage or status while time is kept; exclusion wins over masking; other apps keep titles; the list persists and older settings files load.

### M12 — After 1.1 (no fixed dates)
- **Design improvements per [`docs/design.md`](docs/design.md)** (dark and light themes, motion spec, mockups). Do it in slices, one per session, in this order:
  1. Theme tokens (`Palette` dark/light), local fonts, shared components (docs/design.md → Components), layout shell (rail with REC under Settings, main column, right panel, narrow-window behaviour), and removing/relocating the old controls listed in “What moves or disappears”.
  2. Day view: summary strip, spaced timeline tracks with two-line labels, collapsible Applications/Documents swimlanes, hover tooltips, selection action bar, full-width 00–24 "Activity by hour", "Whole day" panel with Apps/Docs/Tags tabs, filter and highlight-on-timeline.
  3. Week view: seven two-lane day rows, timesheet, "This week" panel.
  4. Tags and Rules screens: tree table + tag editor; ordered rule list + editor with live preview.
  5. Settings sections, tray right-click menu, first-launch autostart dialog.
  6. Motion pass: animations from the Motion table, reduced-motion support.
  - ✅ Each slice: both themes match the mockups at 1280×820 and stay usable at narrow width; text contrast ≥ 4.5:1; keyboard access and focus rings; no idle ticking or CPU/memory regression (`docs/desktop-memory.md`); Day view smooth with 5 000+ fixture events; screenshots of both themes added under `docs/screenshots/`.
- Backups: back up now and automatic backups.
- Notes on tagged ranges.
- Keyboard shortcuts, "tag the last N minutes", compact mini window.
- Documents: full file path where the OS/application exposes it.
- **"Don't track for…" timed pauses (§5.10).** Replace the current tray items "Pause until resumed" and "Pause for 15 minutes" with presets 5 / 10 / 15 / 30 / 60 min in the tray menu and the window's status pill; remove the indefinite pause from the UI (the localhost API and CLI may keep `POST /pause` without a duration for scripts). Show remaining time, Resume now and +5 / +15 min while paused.
  - ✅ Tests: each preset stops recording immediately and resumes at the right time, including across sleep/resume and app restart during a pause; extending and resuming early work; the pause is stored as a manual pause, not AFK; tray and window stay in sync.
- **Autostart prompt on first launch (§5.9).** Ask once whether to start with the system; reuse the existing Settings autostart toggle; never ask again after an answer.
  - ✅ Tests: shown only on first launch with autostart off; "Yes" enables autostart via the existing startup backend and reports its errors; "No" persists; older settings files load without showing it again to existing users.
- **ActivityWatch categories import / export.** ActivityWatch has no manual tags; its equivalent is *categories* (hierarchical name, regex, color), exported from its Settings → Categories as JSON. Map them both ways to Draugr Time's hierarchical tags and auto-tagging rules (M6):
  - Import: each category becomes a tag with the same hierarchy and color plus a regex rule; show a preview before importing ("these rules would tag 34 h in the last 7 days") and reuse "apply to history" with undo. Existing tags with the same path are reused, not duplicated.
  - Export: tags with their rules are written as an ActivityWatch categories file that ActivityWatch's categories screen accepts.
  - ActivityWatch matches a category regex against application and title together; imported rules match either field. ActivityWatch fields with no Draugr Time equivalent (for example the category score) are dropped with a note in the import summary.
  - Manual tag ranges have no place in ActivityWatch and are not exported there; document this.
  - Verify the exact ActivityWatch categories JSON against its source and a real export before implementing.
  - ✅ Tests: round trip Draugr Time → ActivityWatch format → Draugr Time keeps hierarchy, colors, regexes and case sensitivity; invalid regexes and unknown rule types are reported, not silently skipped; import is atomic.

### Later
Git repository and branch detection for terminals and IDEs (moved out of M12: Draugr Time is not aimed at programmers, and full file paths cover most of it; revisit for a 2.0 only if users ask). Screenshots track (as in ManicTime), DB encryption, multi-device sync (CRDT or self-hosted server), MCP server on top of the local API, browser extensions (former M9), Linux Flatpak packaging (deferred from M11).

---

## 7. Risks

- **macOS permissions** (Accessibility / Screen Recording): needs clear onboarding, otherwise titles come back empty.
- **Wayland**: the most labor-intensive part; scheduled in M3 before the timeline UI.
- **Timeline performance**: thousands of events per day; render on canvas and aggregate when zoomed out.
- **DB growth**: index on `start_ts`, optional aggregation of old data.
- **Privacy**: the tracker sees everything. Nothing leaves the machine by default — state this in the README.
