# AGENTS.md — Instructions for AI coding agents

Read `PLAN.md` first. It defines the scope, stack, architecture and milestones.

## Rules

1. **One milestone per session.** Work only on the milestone you were given. Do not start the next one.
2. **Tests first.** Write tests for the acceptance criteria, then the implementation.
3. **Platform code only behind traits** (`WindowWatcher`, `IdleWatcher`, …). Business logic must be testable with `MockWatcher` on any OS.
4. **No schema changes without a migration.** Never edit an existing migration; add a new one.
5. **Timestamps:** unix milliseconds, UTC, everywhere in storage and API. Convert to local time only in the UI.
6. **Privacy:** no network calls except to `localhost`. No telemetry.
7. **Before finishing**, run and show results of:
   ```
   cargo fmt --check
   cargo clippy --all-targets -- -D warnings
   cargo test
   ```
8. **Record decisions** that affect architecture in `docs/decisions.md` (date, decision, reason), so future sessions don't redo them.
9. You usually cannot run code on Windows/macOS/Wayland. For platform-specific code, say explicitly what needs manual verification and how.

## Design rules (UI work)

Any change to screens, widgets or the tray follows [`docs/design.md`](docs/design.md) and the mockups it references.

1. **Match the spec, don't redesign.** Keep the layout, spacing, hierarchy and copy from `docs/design.md`. If something is missing or contradictory, pick the simplest option consistent with it and list the decision in your final report; do not invent new screens or features.
2. **Tokens only.** Colors, font sizes, radii and spacing come from one theme module (`Palette` for dark and light). No hex values or magic numbers in widget code. Both themes must work for every change; check contrast (≥ 4.5:1 for text).
3. **Timeline drawing.** Draw tracks on the existing Iced canvas. Blocks are plain rectangles (radius 0; 2 px max for tag blocks and selection), 1 px gaps, no gradients except the away hatching. Cache geometry; rebuild it only when data, zoom, theme or layout change.
4. **Motion.** Use the durations and easings from the Motion table. Animations run from a subscription that exists only while something is animating and the window is visible; no ticking when idle, hidden or in the tray. Respect the OS reduced-motion setting by skipping animations.
5. **Performance budget.** No regressions in idle CPU/memory (`docs/desktop-memory.md`). The Day view must stay smooth with 5 000+ events; animate at most the affected layer.
6. **Fonts and assets are local.** Bundle Inter Tight and JetBrains Mono (check licenses); no network fonts, icons or images.
7. **Accessibility.** Keyboard reachable controls, visible focus, information never carried by color alone (hatching, dashes, italics as in the spec).
8. **Localization.** Every new string goes through the existing localization for all supported languages; layouts must survive long German/Russian labels.
9. **Show your work.** For UI changes, attach screenshots of both themes at 1280×820 and at a narrow width, and compare them with the matching mockup in `docs/design/`.

## Prompt template

> Implement milestone **M_X** from PLAN.md.
> Acceptance criteria: [copy from PLAN.md].
> Write tests first, then code. Stay within this milestone.
> At the end run fmt, clippy and tests and show the output, and list anything that needs manual verification on a specific OS.
