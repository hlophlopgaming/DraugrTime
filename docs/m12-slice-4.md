# M12 design slice 4 — Tags and Rules

Implemented PLAN.md slice 4 only. Settings, tray/autostart and motion remain slices 5–6.
The adaptive shared rail from the preceding fix remains in use for every language.

## What changed

Tags now has its own header search and one primary New tag button, a collapsible tree
table with subtree statistics, and the selected tag editor in the right panel. The
flat list and old metadata dialog are no longer the production entry point. Parent,
name, color, archive, merge, subtree deletion and Undo use storage validation and
transactions. Delete states its impact and needs a fresh confirmation when that impact
changes. Search restores matching descendants and their ancestors, even in collapsed
branches. A new comma-separated branch can be created without adding a time range.

Rules now has an ordered table with drag handles, square on/off switches, field badges,
monospace expressions, target tags and actual matched time. The right editor owns the
field segments, regex, ignore case, tag picker, seven-day preview, highlighted samples,
30-day application option and Cancel/Save. Invalid drafts get a red underline and cannot
be saved. Old numeric priority controls, date range inputs, stacked editor, permanent
Refresh preview and separate Apply toolbar are gone. Save is the only primary action
in the editor. Existing tags always win; history application is confirmed, checked
again in its write transaction, scoped to the selected rule and undoable.

No additional dependency, migration, network operation or idle timer was introduced.
The chart caches geometry by values, language, theme, color and layout. Metadata loads
run outside view(); late loads cannot restore a hidden screen. Closing releases metadata,
chart and ordinary preview caches while keeping unsaved fields. A pending application
confirmation retains its original checked snapshot. A mouse-release subscription exists
only during dragging. All new copy has translations in all eight supported languages.
Text fields now use the shared 35 px approximate height instead of the old 27 px default.

## Screenshots

These are the actual Iced widgets rendered with the app's tiny-skia renderer, bundled
fonts and an isolated in-memory history. They are not native Wayland window captures.
Each desktop image is exactly 1280×820. Mockups are on the left, current UI on the right;
the original reference pixels are preserved.

| Screen | Dark | Light |
|---|---|---|
| Tags | [Comparison](screenshots/m12-tags-comparison-dark.png) | [Comparison](screenshots/m12-tags-comparison-light.png) |
| Rules | [Comparison](screenshots/m12-rules-comparison-dark.png) | [Comparison](screenshots/m12-rules-comparison-light.png) |
| Tags, narrow German, 700×820 | [Dark](screenshots/m12-tags-de-dark-700x820.png) | [Light](screenshots/m12-tags-de-light-700x820.png) |
| Rules, narrow Russian, 700×820 | [Dark](screenshots/m12-rules-ru-dark-700x820.png) | [Light](screenshots/m12-rules-ru-light-700x820.png) |
| Tags, full narrow layout, 700×1600 | [Dark](screenshots/m12-tags-de-dark-700x1600.png) | [Light](screenshots/m12-tags-de-light-700x1600.png) |
| Rules, full narrow layout, 700×1600 | [Dark](screenshots/m12-rules-ru-dark-700x1600.png) | [Light](screenshots/m12-rules-ru-light-700x1600.png) |

The screenshot suite also covers English/Russian/German in both themes at all three
sizes. On narrow windows the main table alone scrolls horizontally; the panel follows
below in the outer vertical scroll. Panel scrollbars reserve room when needed instead
of covering the fields. Table rows and action groups can grow/wrap for long translations.

## Differences from the artwork and reasons

1. **Actual data:** names, hierarchy, colors, durations, sample strings, chart values,
   ordering and row counts come from a generic test history, rather than copying the
   artwork's invented work. Time labels follow the app's localized duration format.
   The table sorts tag paths alphabetically; no persistent custom tag order exists.
2. **Existing engine fields:** URL replaces Path, and Any is disabled with a tooltip.
   Neither filesystem-path observations nor any-field matching exists in this slice.
3. **ActivityWatch:** its category import button is disabled and explains the later
   task. Export saves `draugr-time-rules-v1` JSON with tags and rules. AW interoperability
   requires the separate PLAN.md task; this file is not advertised as AW compatible.
4. **Editing tags:** Save/Cancel appear only while a tag draft is dirty. Custom HEX
   editing opens from the small ellipsis by Color; arbitrary existing colors survive.
   New branches initially accept their comma-separated name; parent/color become
   editable after creation. Merge and delete reveal explicit confirmation controls.
   These preserve existing capabilities and avoid silent destructive actions.
5. **Rule actions:** small up/down and Delete rule controls appear under the preview.
   Up/down supplies a keyboard alternative to dragging; deleting preserves tagged
   history. History application reveals a confirmation before writing. Errors and
   empty/loading states are real, rather than mock data.
6. **Suggestions:** real untagged applications are shown as direct draft shortcuts
   immediately under the list, rather than the artwork's static “See suggestions” link.
   They do not write rules or history until the user chooses and saves a tag.
7. **Shared shell:** the right panel uses the spec's 344 px width (the Rules artwork
   draws roughly 360 px). Navigation expands for translated labels at the user's
   request, with the same icon/label spacing and REC control as slices 1–3. The privacy
   footer remains on both panels and takes vertical space above the window edge.
8. **Detailed geometry:** headings use the bundled font's metrics; header/card
   positions differ by a few pixels. Fields are approximately 35 px tall, dropdowns
   show their native arrow, table headers use the existing caption token, and rows
   grow when localized content needs extra lines. Color swatches use the shared
   seven-color categorical palette/order instead of a second screen-only palette.
   Linked rules have keyboard focusable secondary rows, with Edit in Rules below
   them rather than beside the section title. Switches share the square component
   for boolean editor options as well as table rows. Suggestions and linked-rule
   outlines use existing card/control tokens. No new illustration/icon set is added;
   the search placeholder has no decorative magnifier and Import has no download icon.
9. **Narrow view:** the supplied artwork only covers desktop. The shared top
   navigation, wrapping actions, horizontally scrollable table and panel below it
   follow docs/design.md's narrow-window rule. The panel keeps its action footer
   reachable while its form scrolls. Explicit focus rings can add a border on focus.
10. **Motion and measurement:** animation work remains slice 6. These widget captures
    do not establish native lifecycle behavior or an unchanged idle-memory baseline.
    Prior Wayland teardown timeouts remain unresolved; no passing claim is made.

## Validation and remaining manual checks

Final checks: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.
`cargo test`: **256 passed, 0 failed, 12 ignored** across the workspace.
The explicit screenshot renderer passed separately.

Final command output is saved in [m12-management-validation.txt](screenshots/m12-management-validation.txt).
The explicit screenshot test is `cargo test -p draugr-time render_management_review_screenshots -- --ignored --test-threads=1`.
It renders actual widgets for both themes and three languages without reading personal history.

Regression tests cover subtree union/day clipping, atomic create/reparent/Undo/cycle
rejection, complete rule reorder validation, raw executable boundaries despite shared
app labels, checked selected-rule application, escaping suggested app names, tree
search/collapse, stale load rejection, export fields, changing delete impact, ignore-case
round trips, invalid patterns, Cancel, rule switches, immediate editor state after creating a nested branch, preserving stale-history errors
across background refresh, and releasing caches on close.
The ordinary suite also retains Day's 5,000-event fixture and keyboard/focus checks.

Manually verify native window close/reopen, drag capture and keyboard traversal on
Windows/macOS and a real Wayland session; use throwaway history for merge/delete/Undo
and 30-day application. Check the portal save dialog with Export. Native idle CPU/PSS
and very large personal histories were not remeasured in this slice. Live rule previews
retain the existing 100,000-event query cap and synchronous editor recalculation; a
long busy history can make editing pause. Statistics and coverage reads stream history,
but metadata refresh still scans all tag history and evaluates enabled rules separately.

Final Linux x86-64 build: `cargo build --release --workspace` passed.
Executable: `target/release/draugr-time` (21036912 bytes).
SHA256: `93014278d152fc3452ccd512bf9460df5565a1692c7cce937b1d166e91b62a7d`. Quit the previous instance through its tray before
launching this binary; closing the window alone leaves that instance running.
