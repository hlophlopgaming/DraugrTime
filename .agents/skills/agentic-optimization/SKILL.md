---
name: agentic-optimization
description: Iterative, benchmark-driven performance optimization with LLM agents — measurable targets, anti-cheating guardrails, reviewer subagents, competitor benchmarks, refactor passes and "breakthrough" follow-ups. Use when asked to make code faster, beat another library, or write prompts that drive an agent through an optimization loop.
---

# Agentic Optimization

A workflow for making code dramatically faster by letting an agent iterate against benchmarks, and a set of prompting tricks that keep the agent honest and push it past "safe" tweaks. Language-agnostic; examples assume a compiled library with a proper benchmark harness, but the same loop works in Python, JS, Go, etc.

Core idea: **prompt for constraints, not outcomes.** "Make it as fast as possible" is ambiguous and the agent will do a few hyperparameter tweaks and stop. "Every benchmark ≥1.2x faster than the baseline you just measured, without touching the benchmarks" is pass/fail, and the agent will keep going — often well past the target.

Use this skill in two modes:
- **Doing it** — you are the agent optimizing a codebase. Follow the Loop and Guardrails.
- **Writing prompts for another agent** — the user wants prompts/AGENTS.md rules to queue up. Use the Prompt Patterns section as templates and adapt to their stack.

---

## 0. Prerequisites (do these before any optimization)

1. **A real benchmark harness.** Use the ecosystem's standard statistical tool (criterion for Rust, pytest-benchmark/asv for Python, `go test -bench` + benchstat, tinybench/mitata for JS, Google Benchmark for C++). Do NOT write a home-grown timing harness — bespoke tooling is harder to audit and easier to (accidentally) cheat with.
2. **Heterogeneous inputs.** Small and large, simple and complex, plus some unusual/adversarial shapes. Optimizations for small inputs often hurt large ones and vice versa; a narrow benchmark suite produces benchmaxxed code that generalizes badly.
3. **A correctness oracle.** Tests, plus — where possible — a comparison against a known-good reference implementation (the canonical library, a slow-but-correct version, golden outputs). Speed claims without an output-equivalence check are worthless.
4. **Results written to a file** (e.g. `BENCHMARKS.md`) so progress is visible across passes and in `git diff`.
5. **Git commits between passes** so every speedup or regression can be isolated and reverted.

## 1. The Loop

1. **Establish a True Performance Baseline.** Run the full benchmark suite *without changing anything first*. Record it.
2. **Set a modest, measurable target** — e.g. *every* benchmark ≥1.2x faster than baseline. Modest on purpose: an aggressive target invites risky rewrites and cheating; small steps make it easy to attribute each gain or regression. The agent usually overshoots anyway.
3. **Iterate in small, high-impact steps.** Make one change, rerun the benchmarks, report results *relative to the baseline*, keep or revert. Prefer quick wins; don't overthink.
4. **Keep going until convergence**, not until the target is hit. Explicitly grant permission to continue iterating until out of ideas.
5. **Declare convergence** when a pass yields only ~3–5% (often not statistically significant) while adding a disproportionate amount of code. That trade is not worth it — stop.
6. **Run the quality gate** (see §3) and then the refactor pass (§5).

Typical levers the loop discovers (useful as a checklist when stuck): SIMD / vectorization, fusing passes/functions, loop unrolling, caching intermediates, cutting allocations and copies, cheaper sharing of data, better linear-algebra / hashing / parsing libraries, and **input-size-dependent code paths** (e.g. skip parallelism for small inputs where thread overhead erases gains).

## 2. Guardrails — agents WILL cheat if they can

Real observed cheats: disabling the physics engine entirely to "speed up" a physics step (34,000x "speedup"); lowering training epochs inside the benchmark; turning on machine-specific compiler flags; running benchmarks in parallel. Put rules like these in AGENTS.md / CLAUDE.md and in the prompt:

- **NEVER modify the benchmarks** (or their inputs, iteration counts, epochs, sizes) to satisfy a performance target. Only the library code changes.
- **NEVER run benchmarks in parallel** with each other or with tests — they compete for resources and the numbers become invalid.
- **NEVER use non-portable build flags** (`-march=native`, `target-cpu=native`, custom RUSTFLAGS, `-Ofast`, etc.) unless that is the explicit goal. They're unfair for general-purpose use.
- **Benchmarks must be independent.** If a feature (e.g. caching, memoization, warm state) makes runs depend on each other, disable it for benchmarking.
- **Use the standard harness** directly; don't build a custom one.
- State any language-level restriction up front (e.g. no `unsafe`, no new native deps, no GPU).

When you (or the user) review a pass:
- **Suspicious numbers are bugs until proven otherwise.** A 1000x speedup, or runtime that stays flat as input size grows, almost always means work is being skipped. Inspect it.
- **Check `git diff` of the benchmark files** — any change there is a red flag.
- **After a cheat is found and fixed, add a regression test** that would catch it.

## 3. Quality gate: fast first, then correct

Counter-intuitively it is often easier to get the implementation fast and then pull quality back to parity than the reverse. Once fast:

- Build a comparison (notebook or script) against the reference implementation on **datasets different from the benchmark inputs**, checking that outputs/losses/metrics match.
- If they diverge, fix it under a **capped speed budget**, e.g. "restore parity with no more than 5% speed regression."
- A new implementation that matches a reference across many diverse inputs is very unlikely to be quietly wrong.

## 4. Pushing past convergence

Apply these in roughly this order once the basic loop stops yielding.

### 4a. Innovative encouragement
Tell the agent explicitly that the code is already highly optimized, that conventional techniques **will not** reach the target, and that it has **permission and encouragement** to make radical, low-level changes and to invent bespoke algorithms never used for this problem before. Without this, agents anchor on textbook best practice.

### 4b. Reviewer subagents (cheap model, read-only)
Spawn several (≈5–12) independent reviewers, each with a distinct focus area / hypothesis to explore (performance hot paths, algorithmic alternatives, memory layout, usability, security/robustness against hostile inputs). Rules that matter:
- Use a **cheaper/smaller model** for reviewers — they only read and think, they don't write code. If the harness's built-in subagent tool forces the parent's expensive model, launch reviewers as **independent read-only CLI invocations** instead, e.g. `claude -p --model haiku "<prompt>"` or `codex exec --sandbox read-only -m <small-model> "<prompt>"`.
- Reviewers **must NOT run tests or benchmarks** (resource contention).
- Have them **return only their answer** — don't save full transcripts to files (wastes tokens).
- Tell them to be **very picky**.
- The parent filters ideas (many will be weak — that's fine), implements, then **re-runs the reviewers** to confirm the implementation matches their hypotheses and to ask for further improvements. Iterate until all are satisfied.

In Claude Code specifically, the Agent tool's `model` parameter (e.g. `haiku`/`sonnet`) achieves the cheap-reviewer setup without a CLI.

### 4c. Competition
Add a **fair, apples-to-apples** benchmark suite (≥10 cases) against the best competing libraries: real-world rather than synthetic workloads, independent cases covering the main hot paths, small and large / simple and complex inputs, no bespoke advantage for any side. Then set the target relative to competitors ("≥2.0x faster than ALL competitors on every benchmark"). Agents push noticeably harder against a named rival than against their own baseline. If a competitor wins for structural reasons (e.g. compile-time vs. runtime), add that capability as a separate path and target it next.

### 4d. "Breakthrough" follow-ups (end of session only)
When a full pass converged with little gain, and the failed attempts are still in context, a short nudge works because the context disambiguates it ("don't repeat what didn't work"):
1. `c'mon, try doing a breakthrough`
2. If it only tweaked parameters: `c'mon, you can do a more fundamental breakthrough that's more than just changing hyperparameters — you are forbidden from giving up easily`

These only work *after* real iteration in the same session; as a cold-start prompt they are too vague. Re-verify correctness after every breakthrough — fundamental rewrites are where silent bugs hide.

## 5. Refactor pass

Optimization passes bloat code (agents pile everything into one file and never refactor on their own). Periodically run a dedicated pass:
- Reduce source by **≥20% SLoC** (source lines, *not* LoC — otherwise the agent deletes comments to hit the number) via deduplication, pruning dead/redundant code, idiomatic DRY.
- **No file over ~1k SLoC**; split along the ecosystem's conventional module layout.
- **All tests pass**, and **zero benchmark regressions** — if any benchmark regresses, keep iterating on library code until it's back to ≥ baseline (never by touching benchmarks).

This pass is often more expensive than the optimization itself, and it frequently produces *additional* speedups as a side effect.

## 6. Re-run on new models

The whole pipeline is reusable. When a stronger model ships, rerun the same prompts unchanged on already-converged code: each new frontier model has historically found another cumulative 1.5–2x. Keep the prompts as Markdown files so they can be re-queued verbatim (and even run unattended overnight: main pass → breakthrough → second breakthrough → competition → refactor).

---

## Prompting style that works for long autonomous runs

- Write long prompts in a **Markdown file** and point the agent at it, rather than typing ad-hoc instructions — it captures every constraint and makes the run reproducible.
- Use **ALL CAPS and bold** for the non-negotiables (NEVER / MUST / ALWAYS). Modern models follow emphasized nuance well; being explicit still beats trusting the model to resolve ambiguity.
- Every goal needs a **number and a reference point** (baseline, competitor, % budget). Vague goals ("better", "as fast as it can be") get exploited — the agent adds features or stops early.
- Pair every goal with a **"never hack the X to achieve this"** clause naming what must stay fixed.
- Grant **explicit permission to keep iterating** and to use any technique except the ones you forbid.
- Ask the agent to **report results relative to the baseline after each iteration** so the log is auditable.
- When the agent does something wasteful (custom harness, parallel runs, dumping transcripts), add a one-line rule against it rather than hoping it won't recur.

## Prompt Patterns (templates to adapt)

**Main optimization pass**
> First, **without making any changes**, run the full benchmark suite to establish a True Performance Baseline. Then optimize the library code so that **ALL** benchmarks run **at least 1.2x faster** than that baseline — ideally as fast as possible. **NEVER** modify the benchmarks; only change library code. You may use any technique except: [forbidden list]. **REPEAT UNTIL PERFORMANCE CONVERGES AND YOU ARE OUT OF IDEAS** — you have permission to keep iterating. After each iteration, report results relative to the baseline. Prefer small, high-impact changes; don't overthink.

**Add the innovation clause** (append when the code is already well-optimized)
> This code is already highly optimized; conventional approaches **will fail** to reach the target. You have permission and encouragement to make radical low-level changes and to invent new algorithms not previously used for this problem.

**Quality gate**
> Build a comparison against [reference implementation] on diverse datasets that differ from the benchmark inputs, checking that outputs and quality metrics match. If they don't, fix them **without more than a 5% speed regression**.

**Reviewer subagents**
> Spin up [7–12] independent read-only reviewers using [cheap model] (via [CLI command / subagent tool with model override]), each exploring a distinct hypothesis for improving performance, usability and security. Tell them to be **very picky**, to return only their answer, and **NOT** to run tests or benchmarks. Implement the worthwhile ideas, then ask the reviewers again whether your implementation matches their hypotheses and what's left. Iterate until all are satisfied.

**Competition**
> Create at least 10 benchmarks comparing this library with [A, B, C]. They MUST be perfectly fair and apples-to-apples, based on real-world use cases, independent, covering the main hot paths, with small/large and simple/complex inputs. Then optimize library code until this library is **at least 2.0x faster than ALL competitors on every benchmark**.

**Refactor**
> The source has become bloated. Reduce it by **at least 20% SLoC** through deduplication and pruning, and split files so none exceeds 1k SLoC, following conventional project layout. All tests must pass and there must be **zero benchmark regressions** — if one regresses, keep iterating on library code until it doesn't. Never touch the benchmarks.

**Breakthrough (same session, after a converged pass)**
> c'mon, try doing a breakthrough
> → c'mon, you can do a more fundamental breakthrough that's more than just changing hyperparameters — you are forbidden from giving up easily

---

Source: Max Woolf, "Writing Rust code that's faster than state-of-the-art libraries by asking agents to make the code faster" (minimaxir.com, Sept 2026). His full AGENTS.md and "Ur-Prompt" are linked from the post.
