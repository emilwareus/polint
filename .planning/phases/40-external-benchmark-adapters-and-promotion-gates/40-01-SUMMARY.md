---
phase: 40-external-benchmark-adapters-and-promotion-gates
plan: 01
subsystem: eval
tags: [rust, evaluation-harness, benchmarks, adaptation, competitors]
requires:
  - phase: 22-internal-evaluation-harness-mvp
    provides: internal eval model, matcher, metrics, deterministic report hashing
provides:
  - suite manifest schema for native, supported-language, and adapter-only benchmark suites
  - evaluation modes and comparison rows for competitor, baseline, and adapted results
  - adaptation record schema with prompt hashes and repo-relative changed artifacts
affects: [phase-40, phase-41, eval, benchmark-promotion]
tech-stack:
  added: []
  patterns: [crate-private eval schema extension, repo-relative benchmark artifact validation]
key-files:
  created:
    - crates/polint/src/eval/suite.rs
    - crates/polint/src/eval/adaptation.rs
    - crates/polint/src/eval/competitors.rs
  modified:
    - crates/polint/src/eval/mod.rs
    - crates/polint/src/eval/model.rs
    - crates/polint/src/eval/report.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/observed.rs
key-decisions:
  - "Keep benchmark suite, comparison, and adaptation metadata crate-private under eval."
  - "Represent imported scanner results separately from locally reproduced scanner and polint runs."
  - "Require adapted runs to carry prompt path, prompt hash, allowed inputs, and forbidden inputs."
patterns-established:
  - "Benchmark manifests validate local paths as repo-relative unless an explicit local-clone policy allows absolute paths."
  - "EvaluationRun owns optional comparison/adaptation metadata and keeps deterministic output hashing over benchmark claims."
requirements-completed: [SAE-PROM-01]
duration: 20 min
completed: 2026-05-26
---

# Phase 40 Plan 01: Eval Suite Manifest Comparison And Adaptation Schema Summary

**Crate-private benchmark schema for suite manifests, three-way comparison rows, and auditable adaptation records**

## Performance

- **Duration:** 20 min
- **Started:** 2026-05-26T07:09:20Z
- **Completed:** 2026-05-26T07:15:38Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- Added `SuiteManifest` and related tier/support/checkout/scoring types for native fixtures, supported-language smoke suites, adapter-only suites, and future research/release suites.
- Added `EvaluationMode` plus comparison rows that distinguish imported scanner results, locally reproduced scanner results, polint baseline, polint adapted, and adapter-only records.
- Added `AdaptationRecord` with prompt path/hash, allowed/forbidden inputs, changed artifact digests, notes path, commands, and final adapted report path.
- Extended `EvaluationRun` with optional suite manifest, comparison rows, adaptation metadata, and limitations while preserving deterministic report hashing.

## Task Commits

1. **Tasks 1-3: Suite manifest, comparison rows, and adaptation records** - `2ed256e` (`feat(40-01)`)

**Plan metadata:** this summary commit.

## Files Created/Modified

- `crates/polint/src/eval/suite.rs` - Suite manifest, tier, checkout, expected-source, scoring, selector, and path-validation types.
- `crates/polint/src/eval/adaptation.rs` - Adaptation record, prompt hash helper, budget, changed artifact, and validation helpers.
- `crates/polint/src/eval/competitors.rs` - Benchmark comparison row, product identity, imported/local/polint/adapter-only result sources, and validation.
- `crates/polint/src/eval/model.rs` - Added internal `EvaluationMode`.
- `crates/polint/src/eval/report.rs` - Embedded comparison/adaptation metadata in deterministic reports and added report tests.
- `crates/polint/src/eval/fixtures.rs` and `crates/polint/src/eval/observed.rs` - Updated internal test report construction for the extended schema.

## Decisions Made

- Kept all new benchmark metadata under crate-private `eval` modules; no public CLI, SDK, runner, or docs surface was promoted.
- Used explicit `AdapterOnly` and `SuiteLanguageSupport::AdapterOnly` states so unsupported Java/Python suite adapters cannot imply real polint analysis.
- Made adapted-run validation require both allowed and forbidden input lists, preventing score-only adaptation records without audit context.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

Cargo serialized the focused test commands on its package/artifact locks when they were launched concurrently. The tests passed after Cargo completed its normal locking behavior.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib eval::suite --locked` - passed, 4 tests
- `cargo test -p polint --lib eval::adaptation --locked` - passed, 4 tests
- `cargo test -p polint --lib eval::report --locked` - passed, 7 tests

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 40-02 and Plan 40-03. The base report schema now has the suite, comparison, and adaptation fields those plans need.

## Self-Check: PASSED

All plan tasks and acceptance criteria were implemented and verified. Summary and production changes are ready for the GSD metadata commit.

---
*Phase: 40-external-benchmark-adapters-and-promotion-gates*
*Completed: 2026-05-26*
