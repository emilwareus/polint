---
phase: 40-external-benchmark-adapters-and-promotion-gates
plan: 02
subsystem: eval
tags: [rust, evaluation-harness, owasp, benchmark-adapter, scanner-metrics]
requires:
  - phase: 40-01
    provides: suite manifest, comparison, and adaptation schema
provides:
  - crate-private benchmark adapter trait
  - OWASP expected-results CSV parser and synthetic scorer
  - adapter-only suite manifests for OWASP Java and BenchmarkPython
affects: [phase-40, eval, external-benchmarks]
tech-stack:
  added: []
  patterns: [internal benchmark adapter trait, inline CSV parser tests, adapter-only language labeling]
key-files:
  created:
    - crates/polint/src/eval/adapter.rs
    - crates/polint/src/eval/external/mod.rs
    - crates/polint/src/eval/external/owasp.rs
    - research/evaluation-harness/suites/owasp-java.toml
    - research/evaluation-harness/suites/owasp-python.toml
  modified:
    - crates/polint/src/eval/mod.rs
key-decisions:
  - "OWASP Java/Python are represented as adapter-only suites until polint supports those languages."
  - "OWASP parser tests use tiny inline CSV fixtures, not copied benchmark files."
  - "OWASP-native metrics count trap/forbidden matches as false positives for suite-native scoring."
patterns-established:
  - "External adapters translate suite data into canonical eval rows without owning CLI behavior."
  - "Unsupported-language adapter validation is separated from polint baseline analysis claims."
requirements-completed: [SAE-PROM-01]
duration: 8 min
completed: 2026-05-26
---

# Phase 40 Plan 02: Adapter Trait And OWASP Expected Results Scoring Summary

**Internal benchmark adapter trait plus OWASP expected-results parsing and adapter-only scoring manifests**

## Performance

- **Duration:** 8 min
- **Started:** 2026-05-26T07:15:38Z
- **Completed:** 2026-05-26T07:23:50Z
- **Tasks:** 4
- **Files modified:** 6

## Accomplishments

- Added a crate-private `BenchmarkAdapter` trait with manifest loading, case enumeration, workspace preparation, observed-output normalization, suite-native metrics, and language-support reporting.
- Added an OWASP expected-results CSV parser preserving test name, category, CWE, vulnerability truth, and optional full category name.
- Added OWASP-native confusion metrics for TP, FN, TN, FP, TPR, FPR, precision, F-score, and TPR-minus-FPR from synthetic observed rows.
- Added OWASP Java and BenchmarkPython suite manifests under `research/evaluation-harness/suites/`, both marked `adapter_only`.

## Task Commits

1. **Tasks 1-4: Adapter trait, OWASP parser/scorer, and manifests** - `70c35f1` (`feat(40-02)`)

**Plan metadata:** this summary commit.

## Files Created/Modified

- `crates/polint/src/eval/adapter.rs` - Internal adapter trait and shared prepared/raw output structs.
- `crates/polint/src/eval/external/owasp.rs` - OWASP CSV parser, canonical expected diagnostic conversion, synthetic observed rows, adapter-only guard, and native metrics.
- `crates/polint/src/eval/external/mod.rs` - External adapter module namespace.
- `crates/polint/src/eval/mod.rs` - Registers adapter and external eval modules.
- `research/evaluation-harness/suites/owasp-java.toml` - Pinned adapter-only OWASP Java manifest.
- `research/evaluation-harness/suites/owasp-python.toml` - Pinned adapter-only BenchmarkPython manifest.

## Decisions Made

- Kept OWASP support as parser/scorer validation only; the manifests cannot be treated as polint baseline analysis while Java/Python are unsupported.
- Used small inline CSV test fixtures so no benchmark expected-results files or source content enter git history.
- Kept the adapter trait independent from any public or hidden CLI command so Phase 40 can wire execution later without freezing a public API.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

One test initially asserted a `suite_id` field on `EvaluationCase`, which does not exist in the current canonical model. The test was corrected to assert the loaded manifest identity instead.

## Verification

- `cargo fmt --all --check` - passed after formatting
- `cargo test -p polint --lib eval::adapter --locked` - passed, 2 tests
- `cargo test -p polint --lib eval::external::owasp --locked` - passed, 4 tests
- `cargo test -p polint --lib eval::suite --locked` - passed, 4 tests

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Ready for Plan 40-03. The report layer can now consume adapter metrics and suite-native metric maps from external adapters.

## Self-Check: PASSED

All plan tasks and acceptance criteria were implemented and verified. Unsupported Java/Python benchmark support is explicitly adapter-only.

---
*Phase: 40-external-benchmark-adapters-and-promotion-gates*
*Completed: 2026-05-26*
