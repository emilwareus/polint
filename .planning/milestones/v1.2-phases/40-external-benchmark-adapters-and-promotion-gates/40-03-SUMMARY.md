---
phase: 40-external-benchmark-adapters-and-promotion-gates
plan: 03
subsystem: eval
tags: [rust, evaluation-harness, performance, cache-stats, markdown]
requires:
  - phase: 40-01
    provides: evaluation report schema
provides:
  - provider/cache/runtime projection for eval reports
  - grouped metric sections for scanner, graph, path, unknown, performance, suite-native, and adaptation metrics
  - deterministic Markdown renderer derived from eval JSON structs
affects: [phase-40, eval, promotion-gates]
tech-stack:
  added: []
  patterns: [volatile runtime stripping before hashing, JSON-first Markdown rendering]
key-files:
  created:
    - crates/polint/src/eval/performance.rs
    - crates/polint/src/eval/markdown.rs
  modified:
    - crates/polint/src/eval/report.rs
    - crates/polint/src/eval/metrics.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/mod.rs
key-decisions:
  - "Eval JSON remains canonical; Markdown is generated from normalized report structs."
  - "Runtime durations and memory observations serialize but are stripped before deterministic output hashing."
  - "Grouped metric sections are additive and preserve existing scalar metric fields."
patterns-established:
  - "KernelRunReport provider/cache rows project into eval performance summaries without exposing a public surface."
  - "Markdown tables must carry limitations and adapter-only/imported labels from JSON."
requirements-completed: [SAE-PROM-01]
duration: 9 min
completed: 2026-05-26
---

# Phase 40 Plan 03: Provider Cache Performance And Report Output Summary

**Provider/cache performance evidence and deterministic Markdown summaries over canonical eval JSON**

## Performance

- **Duration:** 9 min
- **Started:** 2026-05-26T07:23:50Z
- **Completed:** 2026-05-26T07:32:39Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added `EvalPerformanceReport` with provider rows, aggregate cache stats, demand query rows, and runtime summary fields.
- Extended `EvaluationRun` with optional performance evidence and kept output hashes stable when runtime durations change.
- Extended `MetricSummary` with grouped scanner, graph, path, unknown, performance, suite-native, and adaptation sections while preserving existing scalar fields.
- Added a deterministic Markdown renderer for comparison rows, scanner/graph/path metrics, provider/cache stats, adaptation metadata, and limitations.

## Task Commits

1. **Tasks 1-3: Performance projection, metric sections, and Markdown renderer** - `bc52b89` (`feat(40-03)`)

**Plan metadata:** this summary commit.

## Files Created/Modified

- `crates/polint/src/eval/performance.rs` - Provider/cache/demand/runtime report projection and volatile runtime stripping.
- `crates/polint/src/eval/markdown.rs` - JSON-derived Markdown report tables.
- `crates/polint/src/eval/report.rs` - Added performance metadata and grouped metric structs to deterministic reports.
- `crates/polint/src/eval/metrics.rs` - Populates grouped metric sections and unknown counts by status.
- `crates/polint/src/eval/observed.rs` and `crates/polint/src/eval/fixtures.rs` - Updated report construction for the extended schema.
- `crates/polint/src/eval/mod.rs` - Registers performance and Markdown modules.

## Decisions Made

- Performance rows carry optional timing/memory fields for reports, but `deterministic_output_hash` clears those volatile observations.
- Suite-native metrics live in a sorted map under metric sections so OWASP and future adapters can add metrics without changing base report logic.
- Markdown is intentionally derived output and does not introduce a separate source of truth.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

The new unknown-by-status section changed one existing metrics test expectation. The implementation was correct, so the expected `unknown_by_status` map was updated to include the synthetic `present` status used by that test row.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib eval::performance --locked` - passed, 3 tests
- `cargo test -p polint --lib eval::metrics --locked` - passed, 7 tests
- `cargo test -p polint --lib eval::markdown --locked` - passed, 3 tests
- `cargo test -p polint --lib eval::report --locked` - passed, 7 tests

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 40-04. Promotion gates can now inspect deterministic report metrics plus provider/cache evidence.

## Self-Check: PASSED

All plan tasks and acceptance criteria were implemented and verified. Markdown summaries are derived from normalized JSON and preserve limitations.

---
*Phase: 40-external-benchmark-adapters-and-promotion-gates*
*Completed: 2026-05-26*
