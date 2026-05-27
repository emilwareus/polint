---
phase: 40-external-benchmark-adapters-and-promotion-gates
plan: 05
subsystem: eval
tags: [rust, evaluation-harness, external-benchmarks, tier-runner, smoke-suites]
requires:
  - phase: 40-02
    provides: external benchmark adapter trait
  - phase: 40-03
    provides: grouped metrics and deterministic report output
  - phase: 40-04
    provides: native promotion gates
provides:
  - deterministic suite tier selection
  - internal suite run planning and normalized report construction
  - SecBench.js supported-language smoke adapter
  - gosec supported-language smoke adapter
affects: [phase-40, eval, external-benchmarks]
tech-stack:
  added: []
  patterns: [deterministic case selection, absent-clone limitations, suite-owned workspace path checks]
key-files:
  created:
    - crates/polint/src/eval/runner.rs
    - crates/polint/src/eval/tiers.rs
    - crates/polint/src/eval/external/secbench_js.rs
    - crates/polint/src/eval/external/gosec.rs
    - research/evaluation-harness/suites/secbench-js-smoke.toml
    - research/evaluation-harness/suites/gosec-samples.toml
  modified:
    - crates/polint/src/eval/mod.rs
    - crates/polint/src/eval/external/mod.rs
key-decisions:
  - "Fast/nightly/release suite tiers select deterministic case ids from pinned source commits and seeds."
  - "Supported-language smoke adapters tolerate absent local clones and report setup gaps as limitations."
  - "Runner workspace path joining rejects absolute paths, parent escapes, and symlink escapes."
requirements-completed: []
duration: 9 min
completed: 2026-05-26
---

# Phase 40 Plan 05: Supported Language Smoke Suites And Tier Runner Summary

**Internal tier runner plus supported-language SecBench.js and gosec smoke adapter shapes**

## Performance

- **Duration:** 9 min
- **Started:** 2026-05-26T07:40:00Z
- **Completed:** 2026-05-26T07:48:14Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- Added deterministic `all` and `sample:balanced:N` tier selection with seed, source commit, selected case ids, and limitations recorded.
- Added an internal suite runner plan/report path that disables polint analysis for adapter-only suites and builds normalized eval reports for selected cases.
- Added workspace path safety checks so benchmark case preparation cannot use absolute paths, parent-directory escapes, or symlink escapes out of the suite root.
- Added SecBench.js and gosec supported-language smoke adapters that enumerate local clones when present and skip gracefully when clones are absent.
- Added pinned suite manifests for SecBench.js commit `bc3156219138` and gosec commit `de65614d10a6`.

## Task Commits

1. **Tasks 1-3: Tier runner, SecBench.js smoke adapter, and gosec sample adapter** - `2abe791` (`feat(40-05)`)

**Plan metadata:** this summary commit.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib eval::runner --locked` - passed, 4 tests
- `cargo test -p polint --lib eval::tiers --locked` - passed, 3 tests
- `cargo test -p polint --lib eval::external::secbench_js --locked` - passed, 3 tests
- `cargo test -p polint --lib eval::external::gosec --locked` - passed, 3 tests

## User Setup Required

None for unit tests. Actual external benchmark execution still requires local clones under `research/evaluation-harness/repos/`, which remain gitignored.

## Next Phase Readiness

Ready for Plan 40-06. The adaptation prompt artifacts can now reference concrete supported-language smoke suites and deterministic tier selections.

## Self-Check: PASSED

All plan tasks and acceptance criteria were implemented and verified. External benchmark source content was not committed.

---
*Phase: 40-external-benchmark-adapters-and-promotion-gates*
*Completed: 2026-05-26*
