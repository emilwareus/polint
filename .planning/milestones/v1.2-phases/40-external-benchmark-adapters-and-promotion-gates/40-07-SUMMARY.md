---
phase: 40-external-benchmark-adapters-and-promotion-gates
plan: 07
subsystem: eval
tags: [rust, evaluation-harness, competitor-baselines, regression-gates]
requires:
  - phase: 40-03
    provides: report output and performance sections
  - phase: 40-05
    provides: supported-language smoke suites and tier runner
  - phase: 40-06
    provides: adaptation delta records
provides:
  - competitor baseline record validation
  - known competitor row constructors
  - normalized eval baseline read/write/compare logic
  - baseline artifact policy
affects: [phase-40, eval, baselines]
tech-stack:
  added: []
  patterns: [source-cited competitor records, normalized baseline comparisons, thresholded regression gates]
key-files:
  created:
    - crates/polint/src/eval/baseline.rs
    - research/evaluation-harness/baselines/README.md
  modified:
    - crates/polint/src/eval/competitors.rs
    - crates/polint/src/eval/mod.rs
key-decisions:
  - "Imported competitor rows require citation metadata; locally reproduced rows require version, command, config/artifact, and suite commit/version metadata."
  - "Baseline comparisons reject adapter-only and competitor modes as real polint analysis results."
  - "Regression gates compare normalized reports for precision, recall, false-positive traps, output-hash drift, runtime, cache misses, and rejected facts."
requirements-completed: []
duration: 5 min
completed: 2026-05-26
---

# Phase 40 Plan 07: Competitor Baseline Records And Promotion Gate Baselines Summary

**Competitor result records plus normalized baseline regression comparison gates**

## Performance

- **Duration:** 5 min
- **Started:** 2026-05-26T08:01:15Z
- **Completed:** 2026-05-26T08:06:21Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Strengthened competitor comparison rows with product/version, suite commit, metric, limitation, citation, and local reproduction validation.
- Added known comparison row constructors for Semgrep, CodeQL, gosec, and suite-native reference results without hardcoded scores.
- Added deterministic competitor row sorting by suite/product/version/source.
- Added `EvalBaseline` read/write/validate logic over normalized `EvaluationRun` reports.
- Added baseline regression gates for precision drop, recall drop, runtime overhead, new false-positive traps, output-hash drift, cache-miss delta, and rejected-fact delta.
- Documented baseline artifact policy for committed summaries versus ignored large/raw outputs.

## Task Commits

1. **Tasks 1-3: Competitor records, baseline comparison, and artifact policy** - `63f2a4a` (`feat(40-07)`)

**Plan metadata:** this summary commit.

## Verification

- `rg -n "Do not commit|source commit|license" research/evaluation-harness/baselines/README.md` - passed
- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib eval::competitors --locked` - passed, 6 tests
- `cargo test -p polint --lib eval::baseline --locked` - passed, 3 tests
- `cargo test -p polint --lib eval::gates --locked` - passed, 3 tests

## User Setup Required

None.

## Next Phase Readiness

Ready for Plan 40-08. The final closeout can now verify hidden/internal boundaries and the complete Phase 40 evidence chain.

## Self-Check: PASSED

All plan tasks and acceptance criteria were implemented and verified. Unsupported adapter-only suites cannot be compared as real polint scanner baselines.

---
*Phase: 40-external-benchmark-adapters-and-promotion-gates*
*Completed: 2026-05-26*
