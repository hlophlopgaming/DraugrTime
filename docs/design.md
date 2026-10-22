# Draugr Time — design spec (M12 redesign)

This is the source of truth for the M12 "Design improvements" work. It describes
the target look and behaviour of every main screen in both themes, plus motion.
Implement it in Iced with the existing tiny-skia renderer; do not add a web view.

## Reference

- Interactive mockups: the "Draugr Time — app design" canvas
  (<https://claude.ai/artifact/XZNceaHNMSHpaJGMZcqNDP>; private until the owner
  shares it). Page **Dark** and page **Light** hold the same screens:
  Day, Week, Tags, Rules, Settings, plus the tray right-click menu and the
  first-launch autostart dialog (dark only; derive light from the tokens below).
- Exported PNGs of each artboard go to `docs/design/` (`day-dark.png`,
  `day-light.png`, …). When the mockup and this text disagree, this text wins;
  when both are silent, follow the existing app.
- Copy the layout, hierarchy, spacing and colors. Data in the mockups is sample
  data: never ship it or build a demo mode from it.

## Principles

1. **The timeline is the product.** On Day it takes all remaining height. It
   becomes readable through spacing between tracks, not through thicker bars.
2. **Infographic, not toy.** Blocks on timelines and charts are plain
   rectangles: no rounded corners (radius 0; 2 px max on tag blocks and the
   selection), no gradients, no glow. Thin 1 px gaps separate neighbours.
3. **Untagged time is a call to action.** It is drawn as a dashed outline with
   "+ Tag", and the summary always offers "Review N gaps".
4. **Tracking is always visible and never silently off.** REC state sits in the
   rail; pauses are timed only ("Don't track for 5/10/15/30/60 min").
5. **Local and private, and it says so.** Panels end with "Stored on this
   computer only" and how many apps have masked titles.
6. One accent color. Per-app and per-tag colors carry meaning; everything else
   is neutral.

## Layout shell (all main screens)

- **Rail**, 72 px, left: logo (skull with crown, 36 px), then Day, Week, Tags,
  Rules, Settings (icon + 11 px label, 56×48–52 px targets), then the REC
  button directly under Settings. Active item: accent-dim background, accent
  text. Empty space below.
- **Main column**, fluid, 24 px padding, 16 px gaps between cards.
- **Right panel**, 344 px (300–380), side background, left 1 px border.
  Day: "Whole day"; Week: "This week"; Tags: selected tag; Rules: rule editor.
  Settings has a 220 px section list instead.
- Below 760 px wide: rail becomes a top bar, the right panel moves under the
  main column, wide tables scroll horizontally.

## Components

Build these once, in a shared module, and use them everywhere. Most of the
current UI is buttons with a solid accent fill; in the new design almost
nothing is.

| Component | Look | Use |
|---|---|---|
| **Primary button** | accent fill, accentInk text, 600, radius 9, height 36–40 | at most **one per panel/dialog**: "Tag", "Save rule", "+ New tag", "Open Draugr Time" |
| **Secondary button** | transparent, 1 px `line` border, text color, radius 8–9 | Export, Merge into…, Back up now |
| **Ghost button** | no border, no fill, text2; `hover` background on hover | ‹ › day arrows, ×, icon buttons |
| **Destructive** | dangerText on a dark-red tint, 1 px red border | Delete tag, Delete range |
| **Segmented control** | `side`/`bg` track with 3 px padding, radius 10; active option = `hover` fill + text 600, others text3 | Full day / Active hours / Fit selection; Apps / Docs / Tags |
| **Rail item** | 56 px wide, icon 18 px + 11 px label; active = accentDim fill + accent text | left rail only |
| **Card** | `surf` fill, 1 px `line`, radius 14, padding 16–20 | timeline, summaries, lists |
| **Summary strip** | one card split by 1 px dividers into cells: 10 px caps label, 17 px mono value, small bar or meta | top-right of Day and Week |
| **Chip** | 28 px high, 1 px border, square corners, × to remove | excluded / masked apps |
| **Switch** | square knob, accent track when on | Settings, Rules |
| **Text field / select** | `surf` fill, 1 px `line` border, radius 8, 34–38 px | search, filters, rule pattern |
| **Tooltip / popover** | `raised` fill, 1 px border, radius 10, soft shadow | hover cards, REC menu |
| **Status dot** | 7–9 px circle | recording state, backend status |

## What moves or disappears (from the current UI)

The redesign is not a reskin of today's layout. When implementing, remove or
relocate these controls instead of restyling them:

| Today | In the new design |
|---|---|
| Header buttons **Pause** and **15 min** | gone; the **REC** button at the bottom of the rail opens the "Don't track for" menu (5/10/15/30/60) |
| **Theme: System** button | Settings → General |
| **Quit** button | tray menu only |
| "Tracking" status pills in the header, footer and right panel | only the REC button in the rail (and the tray) |
| Date field "2026-10-23" + ‹ › + Today | inside the timeline card header: ‹ Today · Fri 23 Oct › (ghost arrows); Today appears only when another day is shown |
| Search field in the header | Ctrl/Cmd+F opens it; on Day the right panel has its own filter field |
| **Overview** / **Reports** buttons | Overview becomes the "Activity by hour" card and the summary strip; Reports lives in Week ("Open full report →") |
| **Undo tag action** button | no permanent button: after tagging show a small toast "Tagged 1h 30m · Undo" for 6 s; Ctrl/Cmd+Z always works |
| Four separate stat cards (Active, Away, Tagged, Untagged) | one summary strip in the header (Active with away, Tagged, Untagged + Review) |
| Right panel repeating the same totals | right panel is **Whole day** (Apps / Docs / Tags list) — no duplicated totals |
| "Whole day" summary under the timeline | moves to the right panel; the space under the timeline is Activity by hour |
| Divider "Drag divider to resize" | not needed: the timeline fills the height |
| "Pan across day" slider | horizontal scroll/drag on the timeline + range control; no slider |
| Long legend line | short legend (Active, Away, Untagged, Now) + hints on one line |
| Empty timeline between 00:00 and first activity | default range is **Active hours** (first to last activity, rounded to the hour) |

## Screens

### Day
- Header: "Timeline" (30 px, 600) + "First activity · last · span"; on the
  right a summary strip: Active (with away), Tagged (bar), Untagged
  (amber label, "Review N gaps →").
- Timeline card (fills the height): date navigator "‹ Today · Thu 22 Oct ›",
  highlight chip ("Highlighting Krita ×") when an app is focused, range
  segmented control (Full day / Active hours / Fit selection).
- Time axis on top (11 px mono), strong hour lines, faint half-hour lines,
  red "now" line with a time badge on the axis.
- Track label column 132 px: name (13 px, 600) + one meta line (11 px):
  "5h 03m tagged", "9h 17m active", "6 apps · 34 switches", "14 windows".
- Tracks, separated by 1 px lines, each with generous vertical space:
  - **Tags** — 34 px blocks with label; AUTO badge for rule-made tags;
    untagged = dashed outline "+ Tag".
  - **Computer** — 18 px; active = accent, away = amber diagonal hatching.
  - **Applications** — collapsed by default: one 26 px strip of app colors.
    Clicking the label (chevron) expands swimlanes: one 18 px row per top app
    (top 6, rest in "Other") with color square, name and total, and 10 px bars
    on a dotted guide line. Switching apps visibly jumps rows.
  - **Documents** — same pattern: 18 px strip, expands to lanes for the top 8
    windows/documents, colored by their app.
- Hover on any block: tooltip card (app color + name, window/document title,
  "09:12–09:41", duration). Masked titles render in italics. Hovered block gets
  a 1 px light outline.
- Selection: accent outline over all tracks plus a floating action bar under
  the tracks: range, duration, "Tag as [tag ▾] [Tag] Exclude ×".
- Legend line (Active, Away, Untagged, Now) and shortcut hints.
- **Activity by hour** card under the timeline, full width: 24 bars for
  00–24, active (accent) stacked with away (hatched); hour labels every hour,
  multiples of 3 brighter; one-line insight ("most focused 08–10 · longest
  stretch 1h 31m in Krita").
- Right panel "Whole day": tabs Apps / Docs / Tags, filter field, rows with
  chevron, app icon, name + share bar, time + %, and a "highlight on
  timeline" button; expanded rows list windows. Footer: privacy line.

### Week
- Summary: Active (+ per day), Tagged (bar), Untagged with review link.
- Seven day rows (64 px): day name + sessions, a two-lane timeline
  07:00–21:00 (tags on top 10 px, activity below 16 px), day total. Today is
  tinted; future days say "No data yet". Clicking a row opens that day.
- Timesheet table: tags × days in hours, total column, "Export CSV".
- Right panel: tag totals with bars and delta vs last week; most used apps.

### Tags
- Header with search and "+ New tag". Tree table: tag (indent per level,
  color square, chevron), this week (bar + time), all time, rules, last used.
  Archived tags are dimmed with an ARCHIVED badge.
- Right panel for the selected tag: path, name, parent, color swatches,
  7-day bar chart, linked rules (with "Edit in Rules →"), Merge into…,
  Archive, and a destructive Delete that states how much tagged time it removes.

### Rules
- Header: "Import ActivityWatch categories", Export, "+ New rule".
- List in priority order: number (drag handle), on/off switch, field
  (App / Title / Path / Any), regex (mono), target tag, matched time last 7 d.
  Invalid regex = red wavy underline and "error". A hint row offers suggested
  rules for today's untagged time.
- Right panel editor: field segmented control, pattern input, ignore case, tag,
  **live preview** ("3h 20m would be tagged · 1h 05m already has another tag"
  + sample windows with the match highlighted), optional apply to last 30 days
  (undoable), Cancel / Save.

### Settings
- Sections: General, Tracking, Privacy, Data & import, Status & permissions,
  About. Each section is one card of rows: label + one-line explanation on the
  left, control on the right.
- Tracking: start with the computer, away threshold, ask after breaks,
  "Don't track for" presets, repeat-last shortcut, recording schedule.
- Privacy: never record (chips), hide window titles (chips), delete range.
- Data & import: data folder, backups, ActivityWatch import (history,
  categories), export (archive, CSV, AW categories).
- Status: one row per backend with a status dot and a fix-it link.

### Tray and dialogs
- Tray right-click menu: status line (recording + today total + current
  window), Open Draugr Time, Tag the last…, **I'm away · don't track for ›**
  (5 / 10 / 15 / 30 min / 1 hour, last used highlighted, "turns back on by
  itself"), Settings, Quit.
- First launch: modal "Start Draugr Time with your computer?" with
  "Recording already started" badge, No, thanks / Yes, start with system,
  "Asked only once".

## Brand

Sources live in `logo/` and are generated by `scripts/render_logos.py` (one
geometry → SVG, PNG, ICO, ICNS). Edit the script, not the generated files.

- **Mark** (`draugr-time-mark.svg`, `blackDraugr128.png`, `whiteDraugr128.png`):
  a round skull with large round eye sockets, a small nose and a scalloped jaw,
  with a three-point crown floating above it. One colour, transparent
  background; black on light surfaces, white on dark ones (rail, About).
- **App icon** (`draugr-time-app.svg`, `appIcon128.png`, `.ico`, `.icns`):
  the mark on a dark rounded tile (#10141A, radius 28/128): bone skull
  (#ECE7DC), mint crown (#5FD4B8). Used for the window, installers,
  `.desktop` entries and the dock/taskbar.
- **Tray icon** (`draugr-time-tray.svg`, `tray32.png`): a separate glyph
  drawn for 16–32 px — a crown fused onto a clock ring with two hands. It is
  **one colour**, recoloured by state:
  - recording (tracking, AFK, locked, asleep): brand mint `#2BB596`;
  - not recording (paused / "Don't track for", stopped, error): red `#D55A46`;
  - starting: neutral grey `#828FA0`.
  The tooltip always states the state in words, so colour is never the only
  signal.
- Wordmark: "Draugr Time" in Inter Tight 600 next to the mark; caption
  "LOCAL ACTIVITY TRACKER" in 10 px caps, 0.18em tracking.

## Tokens

Use these names in code (one `Palette` per theme); never hard-code hex values
in widgets.

| Token | Dark | Light | Use |
|---|---|---|---|
| bg | #0D1015 | #F4F2EE | window background |
| side | #10141A | #FBFAF7 | rail, right panel |
| surf | #151A21 | #FFFFFF | cards |
| raised | #1B2129 | #F0EEE9 | popovers, selected rows |
| hover | #222A34 | #E9E6E0 | hover, active segment |
| line | #252D38 | #E3DFD7 | card borders |
| lineSoft | #1E252E | #ECE9E3 | separators, bar tracks |
| gridHour | #262E39 | #DDD9D1 | hour lines |
| gridHalf | #1A2028 | #EFECE7 | half-hour lines |
| text | #ECE7DC | #1A1F26 | primary text |
| text2 | #A7AFBA | #4E5764 | labels |
| text3 | #8E97A4 | #646C78 | meta text (≥ 4.5:1 on surf) |
| accent | #5FD4B8 | #2BB596 | fills: active, primary buttons |
| accentText | #5FD4B8 | #137A66 | accent used as text/links |
| accentDim | #1E3A35 | #DDF3EC | active rail item, badges |
| accentInk | #0B1F1A | #0B1F1A | text on accent and on app/tag colors |
| away | #D9A54A | #D9A54A | away hatching (with awayDim) |
| awayDim | #3A2E1A | #F4E2BD | hatching background |
| awayText | #D9A54A | #8F6210 | "Untagged" label |
| danger | #E0715F | #D55A46 | now line, destructive |
| dangerText | #F09484 | #B4412F | destructive text |

Categorical colors for apps and tags (same in both themes): #C792EA, #E7B65C,
#6CC6E8, #7C9CF0, #9BD67A, #E88A9B, then #5FD4B8. Assign per app stably; text
on them is accentInk.

Type: Inter Tight (UI) and JetBrains Mono (times, durations, axis), bundled
locally (no network fonts). Sizes: 30/20/18/15/13/12/11/10 px. Uppercase
10 px labels with 0.08em tracking for section captions.

Shape: cards radius 12–14, controls 7–10, rail items 12, timeline blocks 0.

## Motion

Motion explains state changes; it never decorates. Every animation is short,
interruptible and skipped when the OS asks for reduced motion (then changes are
instant).

| What | How | Duration | Easing |
|---|---|---|---|
| Expand/collapse Applications or Documents lanes | track height grows/shrinks; lanes fade in (opacity 0→1) and slide 4 px down; chevron rotates 0→90° | 260 ms (collapse 200 ms) | ease-out cubic (0.2, 0.7, 0.2, 1) |
| Expand/collapse a row in "Whole day" / Tags tree | children height + fade, chevron rotate | 180 ms | ease-out |
| Highlight an app on the timeline | other blocks fade to 16% opacity (lanes 30%) | 150 ms | linear |
| Hover tooltip | appear after 120 ms hover delay, fade + 2 px rise; hide instantly | 100 ms | ease-out |
| Selection | outline follows the pointer without easing; the action bar fades in after release | 120 ms | ease-out |
| Tag applied | new tag block grows from the selection start to its end | 200 ms | ease-out |
| Tab / segmented switch | active background slides between options | 160 ms | ease-in-out |
| Date change (‹ ›) | timeline content cross-fades; no sliding of the whole card | 140 ms | linear |
| Popovers, menus, tooltips | fade + 4 px offset from their anchor; close faster | open 140 ms, close 90 ms | ease-out |
| Pause menu / "Don't track" | REC button color changes accent→amber; remaining time counts every second | 200 ms | ease-in-out |
| Dialogs (autostart) | backdrop fade, dialog scale 0.98→1 + fade | 180 ms | ease-out |
| Theme switch | instant (no cross-fade) | — | — |
| Recording dot | static; no pulsing (pulsing costs CPU and distracts) | — | — |

Implementation rules for motion in Iced:
- Drive animations from a time subscription that exists **only while an
  animation is running and the window is visible**. Idle windows and the tray
  must not tick (see `docs/desktop-memory.md`).
- Interpolate in app state (start time, from, to); redraw only the affected
  widget/canvas layer. Cache timeline geometry and invalidate on data, zoom or
  theme changes, not on animation frames of unrelated widgets.
- Never animate while loading or recomputing data; show the final state.

## Accessibility

- Text contrast ≥ 4.5:1 (≥ 3:1 for 24 px+) in both themes; colors that must
  be told apart also differ in lightness. Never rely on color alone: away is
  hatched, untagged is dashed, masked titles are italic.
- All controls are reachable by keyboard; focus rings use accent at 2 px.
- Minimum target 28 px in dense lists, 36–44 px for primary controls.
- Tooltips have keyboard equivalents (focus a block → show the same card).
