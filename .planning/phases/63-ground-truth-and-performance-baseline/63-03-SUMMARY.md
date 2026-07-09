---
phase: 63-ground-truth-and-performance-baseline
plan: 03
subsystem: testing
tags: [benchmark, baseline, store-disabled, peak-rss, cold-warm, diagnostics-parity, graph-accuracy, recall, precision]

# Dependency graph
requires:
  - phase: 63-ground-truth-and-performance-baseline
    plan: 01
    provides: CurvePoint telemetry (peak RSS, cold/warm), getrusage substrate, pinned Jelly/Go x/tools oracle suite manifests
  - phase: 63-ground-truth-and-performance-baseline
    plan: 02
    provides: run_repo_perf_point (check + review CurvePoint), render_benchmark_report, run_benchmark_sweep
provides:
  - Distinct StoreDisabledBaseline type (own schema constant polint-store-disabled-baseline-0) carrying peak RSS + cold/warm wall-clock + store_disabled marker + diagnostics-parity digest, with from_curve_point/write/load/validate
  - diagnostics_digest_for_repo (FNV stable-hash over sorted check-equivalent diagnostics) — the parity marker source
  - Committed store-disabled-check.json / store-disabled-review.json reference baselines
  - GraphAccuracyBaseline/GraphAccuracyRow built from external-adapter EvaluationRuns + render_graph_accuracy_markdown + write/load
  - Committed persisted-graph-accuracy.json (pre-store Jelly + Go x/tools reference) surfaced in the benchmark report
affects: [63-04, regression-gates, store-milestone, phase-64, CLI-07]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Store-disabled == current polint: the pre-store reference baseline is a DISTINCT artifact type (own schema constant) from EvalBaseline so Plan 04's evaluate_regression_budget can hard-depend on its exact shape without EvalBaseline drift"
    - "Diagnostics-parity marker: a deterministic FNV stable-hash over the sorted, canonical-JSON diagnostics of the check-equivalent kernel run; clean code still yields a stable non-empty digest"
    - "Accuracy baseline reads recall/precision/graph_edges off existing adapter EvaluationRuns (no scoring reimplemented); recall/precision emitted even as null so the row is structurally complete when the gated clones are absent"
    - "Env-gated regenerators (POLINT_WRITE_STORE_DISABLED_BASELINE, POLINT_WRITE_GRAPH_BENCH) make committed artifacts reproducible without dirtying the tree on normal runs"

key-files:
  created:
    - research/evaluation-harness/baselines/store-disabled-check.json
    - research/evaluation-harness/baselines/store-disabled-review.json
    - research/evaluation-harness/baselines/persisted-graph-accuracy.json
  modified:
    - crates/polint/src/eval/baseline.rs
    - crates/polint/src/eval/bench/runner.rs
    - crates/polint/src/eval/bench/report.rs
    - crates/polint/src/eval/markdown.rs
    - crates/polint/src/eval/bench/sweep.rs
    - crates/polint/src/eval/external/mod.rs

key-decisions:
  - "StoreDisabledBaseline is a separate type with its own STORE_DISABLED_BASELINE_SCHEMA_VERSION constant; the shared BASELINE_SCHEMA_VERSION stays 'polint-eval-baseline-0' (EvalBaseline::validate asserts exact equality, so touching it would break existing artifacts) — matches the Plan 04 contract"
  - "Store-disabled baselines were measured against a small git fixture stand-in (repo_id polint-tiny-fixture) because the large scale clones are absent in the executor; documented and regenerable via POLINT_WRITE_STORE_DISABLED_BASELINE"
  - "Review baseline records the SAME diagnostics_digest as check — the diagnostics-parity marker: review shares the analysis, so it must agree with check"
  - "persisted-graph-accuracy.json commits recall/precision as null (unmeasured) because the gated Jelly/Go x/tools clones + Go toolchain are absent here; suite_id/suite_commit are pinned from the committed manifests, and the env-gated regenerator (POLINT_WRITE_GRAPH_BENCH) rewrites real numbers when the clones are present"

patterns-established:
  - "Pre-store reference baselines carry an explicit pre-store label (StoreDisabledBaseline.store_disabled marker; GraphAccuracyBaseline.reference string) so a future run can assert the store did not change diagnostics or accuracy"
  - "Committed baseline load()/validate() + a load-and-assert test are the tamper guard (threat T-63-03-01)"

requirements-completed: [BENCH-02, BENCH-04]

# Metrics
duration: 50min
completed: 2026-07-09
---

# Phase 63 Plan 03: Store-Disabled and Persisted-Graph Reference Baselines Summary

**A distinct StoreDisabledBaseline type records the pre-store peak-RSS / cold-warm-latency / diagnostics-parity reference for `polint check` and `polint review`, and a GraphAccuracyBaseline records the pre-store Jelly + Go x/tools recall/precision baseline that now renders in the benchmark report — the fixed references the Phase 64+ regression gates measure against.**

## Performance

- **Duration:** ~50 min
- **Started:** 2026-07-09
- **Completed:** 2026-07-09
- **Tasks:** 2
- **Files modified:** 9 (3 created, 6 modified)

## Accomplishments

- Added a DISTINCT `StoreDisabledBaseline` type in `eval/baseline.rs` with its own `STORE_DISABLED_BASELINE_SCHEMA_VERSION = "polint-store-disabled-baseline-0"` constant (the shared `BASELINE_SCHEMA_VERSION` is untouched), carrying `store_disabled`, `repo_id`, `suite_id`, `peak_rss_bytes`, `cold_wall_clock_ms`, `warm_wall_clock_ms`, and `diagnostics_digest`, plus `from_curve_point` / `write` / `load` / `validate`. This is the exact shape Plan 04's `evaluate_regression_budget(baseline: &StoreDisabledBaseline, ...)` hard-depends on.
- Added `diagnostics_digest_for_repo` to the bench runner: a deterministic FNV stable-hash over the sorted, canonical-JSON-serialized diagnostics of the check-equivalent kernel run — the diagnostics-parity marker (Phase 64's store must not change it). Clean code still yields a stable, non-empty digest.
- Committed `store-disabled-check.json` and `store-disabled-review.json`, measured against a small git fixture stand-in through the real `polint check` / `polint review` pipeline, both with `store_disabled = true` and a non-empty parity digest; review carries the same digest as check (parity).
- Added `GraphAccuracyBaseline`/`GraphAccuracyRow` in `bench/report.rs` built from the external-adapter `EvaluationRun`s (reads `metrics.recall`/`precision`/`graph_edges_*` off the runs; no scoring reimplemented) with deterministic `write`/`load` and a `render_graph_accuracy_markdown` "## Persisted-Graph Accuracy Baseline" section.
- Extended `render_benchmark_report` to append the accuracy section (BENCH-04 "appears in the benchmark report") and wired the sweep to load the committed baseline into `benchmark-report.md`.
- Committed `persisted-graph-accuracy.json` as the pre-store reference for the Jelly micro and Go x/tools RTA suites (pinned `suite_commit`s), plus an env-gated regenerator that rewrites real recall/precision when the gated clones are present.

## Task Commits

1. **Task 1: StoreDisabledBaseline type + committed check/review baselines** - `3a597500` (feat)
2. **Task 2: Pre-store graph accuracy baseline + benchmark-report section** - `b11c7c61` (feat)

## Files Created/Modified

- `crates/polint/src/eval/baseline.rs` - `StoreDisabledBaseline` type + own schema constant + from_curve_point/write/load/validate + round-trip/tamper/committed-load/regenerator tests (modified)
- `crates/polint/src/eval/bench/runner.rs` - `diagnostics_digest_for_repo` + `digest_diagnostics` FNV parity-marker helper (modified)
- `crates/polint/src/eval/bench/report.rs` - `GraphAccuracyBaseline`/`GraphAccuracyRow` + write/load + `render_graph_accuracy_markdown` + tests (modified)
- `crates/polint/src/eval/markdown.rs` - `render_benchmark_report` now appends the accuracy section (Option<&GraphAccuracyBaseline>) + test (modified)
- `crates/polint/src/eval/bench/sweep.rs` - loads the committed accuracy baseline into `benchmark-report.md` (modified)
- `crates/polint/src/eval/external/mod.rs` - `POLINT_WRITE_GRAPH_BENCH` block now also regenerates `persisted-graph-accuracy.json` from the real runs (modified)
- `research/evaluation-harness/baselines/store-disabled-check.json` - committed store-disabled `polint check` reference (created)
- `research/evaluation-harness/baselines/store-disabled-review.json` - committed store-disabled `polint review` reference (created)
- `research/evaluation-harness/baselines/persisted-graph-accuracy.json` - committed pre-store Jelly + Go x/tools recall/precision reference (created)

## Decisions Made

- **Distinct type, untouched shared constant:** `StoreDisabledBaseline` is a separate type with its own schema constant per the Plan 04 contract; `BASELINE_SCHEMA_VERSION` stays `"polint-eval-baseline-0"` so existing `EvalBaseline` artifacts keep validating.
- **Fixture stand-in for store-disabled measurement:** the large scale clones (grafana/hugo/excalidraw) are absent in the executor, so the committed store-disabled baselines were measured against a small go+ts git fixture (`repo_id = "polint-tiny-fixture"`, `suite_id` = the check/review command marker). This is the plan's explicit absent-clone fallback; values are honest fixture-scale and regenerable via `POLINT_WRITE_STORE_DISABLED_BASELINE`. Phase 64 should re-baseline against the real scale repo when present.
- **Diagnostics-parity marker across check/review:** review records the same `diagnostics_digest` as check because review shares the analysis; the marker records that agreement.
- **Null recall/precision in the accuracy baseline (documented):** the gated Jelly/Go x/tools clones and Go toolchain are not present in the executor and the plan's own external test early-returns without them, so full-suite recall/precision cannot be measured here. The committed artifact records both suite rows with pinned `suite_id`/`suite_commit` and `recall`/`precision` keys present as `null`, explicitly labeled pre-store; the env-gated regenerator (`POLINT_WRITE_GRAPH_BENCH`) rewrites the real measured numbers when a maintainer runs it with the clones present.

## Deviations from Plan

### Auto-fixed / scope adjustments

**1. [Rule 2 - Reproducibility] Wired the accuracy regenerator into `external/mod.rs`**
- **Found during:** Task 2
- **Issue:** The plan lists Task 2 files as `bench/report.rs`, `markdown.rs`, and the JSON, but the only place the real Jelly/Go `EvaluationRun`s are produced is the existing `POLINT_WRITE_GRAPH_BENCH`-gated block in `external/mod.rs`. Making the committed artifact reproducible (plan intent + threat T-63-03-02) required hooking the regenerator there.
- **Fix:** Added a small block in the existing gated test that builds `GraphAccuracyBaseline::from_runs(&[&go, &jelly])` and writes the committed JSON — reusing the runs already computed, not reimplementing scoring.
- **Files modified:** `crates/polint/src/eval/external/mod.rs`
- **Committed in:** `b11c7c61`

**2. [Rule 3 - Blocking] `render_benchmark_report` signature extended**
- **Found during:** Task 2
- **Issue:** The benchmark report needed to carry the accuracy section, but `render_benchmark_report` took only a `CurveSeries`.
- **Fix:** Added an `Option<&GraphAccuracyBaseline>` parameter (appends the section when present), updated the one existing test and the sweep caller (the sweep loads the committed baseline).
- **Files modified:** `crates/polint/src/eval/markdown.rs`, `crates/polint/src/eval/bench/sweep.rs`
- **Committed in:** `b11c7c61`

**Total deviations:** 2 (1 reproducibility hook, 1 blocking signature change). No architectural changes; delivered types/paths match the plan and the Plan 04 contract.

## Known Stubs

- `persisted-graph-accuracy.json` records `recall`/`precision` as `null` for both suites because the gated Jelly/Go x/tools clones are absent in the executor (documented decision above). This is an intentional, labeled pre-store placeholder: the structure, pinned commits, and rendering are complete, and the env-gated regenerator (`POLINT_WRITE_GRAPH_BENCH`) fills real numbers when the clones are present. Not a UI-facing stub. Phase 64 / CLI-07 work should regenerate it against the real oracle suites.

## Threat Flags

None — the new surface (two committed store-disabled baselines + one accuracy baseline, all crate-private `pub(crate)` types) is covered by the plan's threat register. Mitigations in place: `load()`/`validate()` + load-and-assert tests reject tampered/incomplete store-disabled files (T-63-03-01); `repo_id`/`suite_id` recorded honestly and the fixture stand-in + null-metric decisions documented here (T-63-03-02); tests assert no `/Users/` or `/home/` substrings in the committed JSON and the local devloupe reference stays uncommitted (T-63-03-03).

## Verification

- `cargo test -p polint --lib eval::baseline --locked` — 7 passed.
- `cargo test -p polint --lib eval::bench::report --locked` — 4 passed.
- `cargo test -p polint --lib eval::bench --locked` — 15 passed; `eval::markdown` — 5 passed; `eval::external` — 46 passed; `eval::bench::sweep` — 3 passed.
- `cargo build -p polint --locked` — clean.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` — clean (pre-commit hook).
- `cargo test -p polint --test public_surface_leak --locked` — 5 passed (new types stay `pub(crate)`).
- The three committed baseline JSON files exist and load/validate.

## Next Phase Readiness

- `StoreDisabledBaseline` (exact Plan 04 shape) and the committed check/review baselines are ready for Plan 04's `evaluate_regression_budget`. No blockers.
- The pre-store accuracy baseline renders in the benchmark report; a maintainer with the Jelly/Go clones present should run `POLINT_WRITE_GRAPH_BENCH` to replace the null recall/precision with measured numbers, and Phase 64 should re-baseline the store-disabled references against a real scale repo.

---
*Phase: 63-ground-truth-and-performance-baseline*
*Completed: 2026-07-09*

## Self-Check: PASSED

All three committed baseline JSON files exist on disk, `eval/baseline.rs` carries the `StoreDisabledBaseline` type, and both task commits (3a597500, b11c7c61) are in git history.
