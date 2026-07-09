---
phase: 63-ground-truth-and-performance-baseline
plan: 02
subsystem: testing
tags: [benchmark, performance, curve-series, perf-runner, sweep, budget-exhaustion, diff-size]

# Dependency graph
requires:
  - phase: 63-ground-truth-and-performance-baseline
    plan: 01
    provides: CurvePoint/CurveSeries telemetry, measure::cold_then_warm + peak_rss_bytes, pinned scale suite manifests
provides:
  - Whole-repo perf runner (run_repo_perf_point) driving polint check + review over a checked-out repo into a CurvePoint
  - Deterministic curve-series JSON emission (write_curve_series) + markdown benchmark report (render_curve_markdown / render_benchmark_report)
  - Benchmark sweep entry-point (run_benchmark_sweep) assembling a real multi-point CurveSeries over the locked scale suite + diff-size sweep
affects: [63-03, 63-04, curves, baseline-report, regression-gates]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Perf measurement drives the capability-gated AnalysisKernel::run (PERF-01 discipline) wrapped in measure::cold_then_warm; repo size read from the loaded source set, not a separate eager whole-repo read"
    - "Budget-exhaustion counters folded from live AnalysisDb *::BudgetExceeded statuses (summary/call-target/domain), the same fact families analysis_kernel::debug counts"
    - "Sweep parameterized over the per-point measurer (run_sweep_with) so emission determinism is testable in isolation from inherently-volatile timing/RSS"

key-files:
  created:
    - crates/polint/src/eval/bench/runner.rs
    - crates/polint/src/eval/bench/report.rs
    - crates/polint/src/eval/bench/sweep.rs
  modified:
    - crates/polint/src/eval/bench/mod.rs
    - crates/polint/src/eval/markdown.rs

key-decisions:
  - "run_repo_perf_point times ONE capability-gated kernel run (the analysis cost check and review share); the review diff gate is a cheap reporting-layer filter, so the review measurement adds diff-size fields (via changeset_for_ref) rather than a second, differently-priced pipeline"
  - "Budget-counter field mapping: budget_exceeded <- SummaryStatus::BudgetExceeded (summary facts+events); tokens_exhausted <- CallTargetStatus::BudgetExceeded (token/points-to budget at call resolution); iteration_capped <- DomainStatus::BudgetExceeded (domain solver iteration/round cap)"
  - "store_bytes is an explicit 0 (documented) until the durable semantic store lands in Phase 64"
  - "Benchmark sweep skips (does not fail) absent large-repo checkouts, so it is runnable in CI without the multi-GB clones; emission determinism is asserted via an injected deterministic measurer because real cold/warm timing and peak RSS are inherently volatile"

requirements-completed: [BENCH-01]

# Metrics
duration: 40min
completed: 2026-07-09
---

# Phase 63 Plan 02: Whole-Repo Perf Runner + Curve Report + Sweep Summary

**A whole-repo perf runner drives `polint check`+`review` over a checked-out repo into a measured CurvePoint (cold/warm, peak RSS, diff size, cache size, budget exhaustion), a deterministic curve-series JSON + markdown benchmark report render it, and a sweep entry-point assembles a real multi-point CurveSeries over the locked scale suite and diff-size axes.**

## Performance
- **Duration:** ~40 min
- **Tasks:** 3
- **Files modified:** 5 (3 created, 2 modified)

## Accomplishments
- `run_repo_perf_point(repo_root, review_ref)` measures a single `CurvePoint` by driving the capability-gated `AnalysisKernel::run` (the `polint check` equivalent) inside `measure::cold_then_warm`, deriving repo size from the loaded source set (no eager whole-repo read), sizing the review diff via `crate::git::changeset_for_ref`, reading the on-disk `.polint/cache` size, and folding budget-exhaustion counters from the live `AnalysisDb`.
- `write_curve_series` emits byte-stable pretty JSON (sorted points), and `render_curve_markdown` renders a "## Benchmark Curves" table with Peak RSS, cold/warm ms, diff files/hunk lines, cache/store bytes, and budget-exceeded columns; `markdown::render_benchmark_report` composes a report header over the curve table.
- `run_benchmark_sweep(output_dir)` iterates the committed grafana/hugo/excalidraw scale manifests, measures a baseline + fixed diff-size sweep per present checkout into one multi-point `CurveSeries`, skips absent large clones, and writes both `benchmark-curves.json` and `benchmark-report.md`.

## Task Commits
1. **Task 1: Whole-repo perf runner driving check + review** - `ff8630e0` (feat)
2. **Task 2: Curve JSON emission + markdown benchmark report** - `9a4e597f` (feat)
3. **Task 3: Benchmark sweep entry-point over the locked scale suite** - `948429a8` (feat)

## Files Created/Modified
- `crates/polint/src/eval/bench/runner.rs` - `run_repo_perf_point` whole-repo perf runner + budget-counter fold + on-disk cache sizing (created)
- `crates/polint/src/eval/bench/report.rs` - `write_curve_series` (deterministic JSON) + `render_curve_markdown` (curve table) (created)
- `crates/polint/src/eval/bench/sweep.rs` - `run_benchmark_sweep` locked-suite + diff-size sweep into a multi-point CurveSeries (created)
- `crates/polint/src/eval/bench/mod.rs` - registered `runner`, `report`, `sweep` submodules (modified)
- `crates/polint/src/eval/markdown.rs` - added `render_benchmark_report` benchmark-report entry-point without changing `render_markdown` (modified)

## Decisions Made
- **Single-run timing for check/review:** `run_repo_perf_point` times one capability-gated kernel run — the analysis cost `check` and `review` share. The review diff gate is a cheap reporting-layer filter over the same facts, so the review measurement contributes diff-size fields (`diff_files`, `diff_hunk_lines`) derived from `changeset_for_ref` rather than a second, differently-priced pipeline. This keeps the measurement honest (the dominant analysis cost) and avoids double-counting.
- **Budget-counter field mapping (explicit sources):** `KernelRunReport` does not expose budget/token/iteration counters, so they are folded from the live `AnalysisDb` by walking the same `*::BudgetExceeded` fact families `analysis_kernel::debug` counts. `budget_exceeded` sums `SummaryStatus::BudgetExceeded` over summary facts and events; `tokens_exhausted` counts `CallTargetStatus::BudgetExceeded` (the token/points-to budget surfaced at call resolution); `iteration_capped` counts `DomainStatus::BudgetExceeded` over domain observations and events (the domain solver's iteration/round cap). All three are sourced from reachable pipeline signals — none is a silent zero-stub. On a fixture with no budget exhaustion these are naturally 0, which is the truthful value.
- **`store_bytes` explicit 0:** the durable semantic store lands in Phase 64; `store_bytes` is set to 0 with an in-code comment and this note, per the plan's explicit-zero-decision instruction.
- **Sweep determinism vs. volatile measurement:** real cold/warm wall-clock and peak RSS are inherently volatile, so the `benchmark-curves.json` emitted by a real measured sweep is NOT byte-identical across runs (the magnitudes change). The sweep loop (`run_sweep_with`) is therefore parameterized over the per-point measurer: the byte-identical-across-reruns determinism test injects a deterministic measurer (isolating emission determinism, which is what `write_curve_series` guarantees), while a separate test drives the real `run_repo_perf_point` over a git fixture repo to prove the sweep assembles >= 2 measured points and writes both non-empty artifacts, and a third test drives the real `run_benchmark_sweep` to prove absent large-repo checkouts are skipped, not failed.

## Deviations from Plan
None - plan executed exactly as written. The budget-counter field mapping and the `store_bytes = 0` decision were both anticipated by the plan (it explicitly required documenting the counter sources and any explicit-zero decision), and are recorded above rather than as deviations.

## Threat Flags
None — no new security surface beyond the plan's threat register. Mitigations in place: `changeset_for_ref` passes refs as fixed positional args to the git binary (no shell, T-63-02-01); measurement reuses the existing config-load workspace root with no new path joining (T-63-02-02); large-repo measurement is intended workload with budget counters recorded rather than unbounded expansion (T-63-02-03, accepted); and curve JSON/markdown key by `repo_id` (the checkout directory name) with tests asserting no `/Users/` or `/home/` absolute paths leak (T-63-02-04).

## Known Stubs
- `StoreSizeBytes.store_bytes` is a deliberate `0` until the durable semantic store lands in Phase 64 (documented decision above and in-code). Not a UI-facing stub; the curve schema field is present and populated once the store exists.

## Verification
- `cargo test -p polint --lib eval::bench::runner --locked` — 2 passed.
- `cargo test -p polint --lib eval::bench::report --locked` — 2 passed.
- `cargo test -p polint --lib eval::bench::sweep --locked` — 3 passed.
- `cargo test -p polint --lib eval::bench --locked` — 13 passed; `eval::suite` — 11 passed.
- `cargo build -p polint --locked` — clean.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` — clean.
- `cargo test -p polint --test public_surface_leak --locked` — 5 passed (bench stays `pub(crate)`).

## Next Phase Readiness
- The perf runner produces measured `CurvePoint`s and the sweep assembles a real multi-point `CurveSeries` with deterministic JSON + markdown emission — the substrate Plan 03 (baselines) and Plan 04 (regression gates) consume. No blockers.

---
*Phase: 63-ground-truth-and-performance-baseline*
*Completed: 2026-07-09*

## Self-Check: PASSED
All three created files exist on disk and all three task commits (ff8630e0, 9a4e597f, 948429a8) are in git history.
