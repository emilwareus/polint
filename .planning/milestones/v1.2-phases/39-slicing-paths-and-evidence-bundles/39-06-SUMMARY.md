---
phase: 39-slicing-paths-and-evidence-bundles
plan: 06
subsystem: static-analysis-engine
tags: [rust, evidence, extensions, validation, eval]

requires:
  - phase: 39-03-bounded-paths
    provides: bounded paths and ranking
  - phase: 39-04-summary-context
    provides: summary expansion and context traversal
provides:
  - Extension evidence candidate rows
  - Extension evidence merge verdicts and validation
  - Eval/debug rows for extension evidence deltas
affects: [phase-39-eval]

tech-stack:
  added: []
  patterns: [validation-gated extension merge, precision downgrade, eval delta rows]

key-files:
  created:
    - crates/polint/src/analysis/evidence/validate.rs
  modified:
    - crates/polint/src/analysis/evidence/facts.rs
    - crates/polint/src/analysis/evidence/mod.rs
    - crates/polint/src/eval/observed.rs

key-decisions:
  - "Extension evidence candidates are internal, serialized rows with extension id, provider id, endpoints, source span, claimed status/precision, expansion/replay data, native anchors, and evidence labels."
  - "Merge verdicts are accepted, accepted-with-precision-downgrade, candidate-only, and rejected."
  - "Exact extension claims are downgraded unless native anchors validate them; invalid endpoints, spans, and unbounded expansion are rejected."

patterns-established:
  - "Native evidence edges are counted and retained when extension evidence is merged."
  - "Candidate/rejected extension evidence remains observable but cannot strengthen diagnostics."
  - "Eval delta rows expose accepted, downgraded, candidate-only, rejected, native-edge count, and representative reasons without absolute paths or raw source."

requirements-completed: [SAE-PREC-04]

duration: 8min
completed: 2026-05-25
---

# Phase 39-06: Extension Evidence Merge And Validation Summary

**Extension evidence is now validation-gated and visible as deterministic deltas**

## Performance

- **Duration:** 8 min
- **Started:** 2026-05-25T14:42:35Z
- **Completed:** 2026-05-25T14:50:31Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added internal extension evidence candidate and merge verdict facts.
- Added `analysis::evidence::validate` with deterministic validation for endpoints, spans, exact precision claims, expansion keys, and replay keys.
- Added merge delta accounting that keeps native may-edges visible while recording accepted, downgraded, candidate-only, and rejected extension evidence.
- Added eval/debug delta row generation for extension evidence merge results.
- Added tests for deterministic sorting/serialization, invalid endpoint/span rejection, exact-claim downgrade, native-anchor acceptance, native edge retention, candidate-only status, and eval row privacy.

## Task Commits

1. **Tasks 1-3: Extension evidence validation and eval deltas** - `985b133` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/evidence/facts.rs` - Extension evidence candidate, merge fact, verdict, and reason rows.
- `crates/polint/src/analysis/evidence/validate.rs` - Validation and merge delta logic.
- `crates/polint/src/analysis/evidence/mod.rs` - Registers evidence validation module.
- `crates/polint/src/eval/observed.rs` - Adds deterministic eval rows for extension evidence deltas.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib analysis::evidence::validate --locked` - passed
- `cargo test -p polint --lib analysis::evidence --locked` - passed
- `cargo test -p polint --lib eval --locked` - passed
- `cargo clippy -p polint --lib --locked -- -D warnings` - passed

## Deviations from Plan

- The existing extension subsystem already validates generic extension facts. This plan added the evidence-specific validation layer instead of replacing the generic extension sink path.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Wave 6 can finish Phase 39 with evidence debug/eval fixtures and public-boundary proof.

---
*Phase: 39-slicing-paths-and-evidence-bundles*
*Completed: 2026-05-25*
