---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 13
subsystem: analysis-kernel
tags: [validation, structured-events, diagnostics, run-metadata, deterministic-digests]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 12
    provides: Exact current-run layer metadata retained in the private kernel report
provides:
  - Closed exact validation event kind and status codecs with typed unknown-label rejection
  - Deterministic structured evidence for all 19 authoritative validation stages plus global validation
  - Explicit FactValidationReport and private KernelRunReport event handoff with diagnostics unchanged
affects: [phase-65-validated-run-handoff, phase-65-store-commit-plan, phase-65-validation-storage]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Count each validator's diagnostic delta in the same pass instead of interpreting rendered messages"
    - "Attest global validation with the ordered semantic digests of every required stage event"
    - "Use an explicit structured report boundary and migrate every caller without Vec compatibility traits"

key-files:
  created:
    - .planning/phases/65-generation-manifest-and-metadata-mirroring/65-13-SUMMARY.md
  modified:
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis/calls/validate.rs
    - crates/polint/src/analysis/entrypoints/validate.rs
    - crates/polint/src/eval/performance.rs

key-decisions:
  - "Derive stage status solely from the structured issue count and digest only kind, status, and count"
  - "Build the global digest from the stable ordered child-event vocabulary and semantic child digests"
  - "Retain validation events after unchanged diagnostic extension and before finalized facts reach later persistence"
  - "Permit an empty event vector only in the synthetic eval report where the authoritative validator did not run"

patterns-established:
  - "One-pass evidence: each authoritative validator emits exactly one stage event without parsing diagnostics"
  - "Closed completeness: ValidationEventKind::ALL is the canonical 20-event order including GlobalFactValidation"
  - "Diagnostic compatibility: existing diagnostics keep their construction order, final sort, bytes, and policy behavior"

requirements-completed: [STORE-04, META-01]

# Metrics
duration: 18min
completed: 2026-07-13
---

# Phase 65 Plan 13: Structured Validation Events Summary

**The authoritative fact validator now returns unchanged diagnostics alongside deterministic structured trust events for every validation stage and one global outcome.**

## Performance

- **Duration:** 18 min
- **Started:** 2026-07-13T18:14:29Z
- **Completed:** 2026-07-13T18:32:46Z
- **Tasks:** 1
- **Files modified:** 6 implementation files

## Accomplishments

- Added `FactValidationReport`, `ValidationEvent`, a closed 20-kind vocabulary, passed/failed status, exact snake-case codecs, typed unknown-label errors, and canonical event ordering.
- Wrapped all 19 existing top-level validator calls in same-pass issue counting, then appended a global event whose digest attests the ordered stage outcomes without consuming rendered messages.
- Preserved diagnostic construction and sorting exactly while moving the structured events through the private `KernelRunReport` after validation and fact finalization.
- Migrated every original direct caller found by the repository-wide audit: 27 validation test calls, three calls validator tests, one entrypoint validator test, and production kernel execution.
- Kept the eval-only synthetic report honest with an explicit empty vector because that fixture does not execute the authoritative validator.
- Added passing/failing determinism, exact status/count/digest, full event serde round-trip, complete stable order, and unknown-label rejection coverage.

## Task Commits

Each task was committed atomically:

1. **Task 1: Emit events and migrate the complete caller set** - `ef65a92d` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/validation.rs` - Structured validation report, complete closed event vocabulary, same-pass event construction, codecs, caller migrations, and focused fixtures.
- `crates/polint/src/analysis_kernel/mod.rs` - Exact report/diagnostic/event/finalization order and real-kernel event-retention assertions.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Private validation-event ownership in the kernel run handoff.
- `crates/polint/src/analysis/calls/validate.rs` - Explicit diagnostics-field migration for all three direct validator callers.
- `crates/polint/src/analysis/entrypoints/validate.rs` - Explicit diagnostics-field migration for the integrated validator caller.
- `crates/polint/src/eval/performance.rs` - Explicit empty validation events for a synthetic report that runs no validator.

## Decisions Made

- Stage events use the diagnostic vector length delta observed around each validator invocation. This counts structured validator output at its source and does not interpret message text or evidence rendering.
- A stage digest contains only the closed event kind, derived status, and issue count. The global digest additionally includes each ordered child kind, status, count, and digest so completeness and order are attested.
- `ValidationEventKind::ALL` is the canonical required order for later integrity checks: the 19 current stages in execution order followed by `GlobalFactValidation`.
- Event identity excludes rendered text, source/body/path data, cache status and counters, timestamps, durations, relational handles, and persistence concepts.
- The production sequence remains explicit: produce the report, extend unchanged diagnostics, retain events, finalize fact metadata, and only then perform store maintenance.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - this is private analysis-kernel trust metadata with no CLI, config, SDK, or generated-skill surface.

## Verification

- Repository-wide pre-edit audit found 33 textual matches: one definition and 32 direct callers across the six declared files; every caller migrated explicitly with no `IntoIterator`, `Deref`, or implicit Vec seam.
- Validator suite: 30 passed, including all event codec, stable-order, positive/negative, diagnostic, and digest fixtures.
- Calls validator suite: 5 passed.
- Entrypoints validator suite: 9 passed.
- Semantic-store kernel suite: 3 passed, including enabled/disabled private event retention and byte-identical store-mode behavior.
- Eval observed-kernel suite: 13 passed.
- `cargo check -p polint --all-features --locked`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings`: passed.
- Repository pre-commit hook passed, including workspace/all-target/all-feature Clippy with warnings denied.
- `git diff --check` passed; only the six declared implementation files changed before summary creation.

## Next Phase Readiness

- The private run report now owns complete authoritative validation evidence, ready for Plan 14 to assemble the validated-run handoff and canonical semantic identities.
- The closed canonical event order gives later store-plan integrity checks an exact required-event set without diagnostic parsing.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-13*

## Self-Check: PASSED

The task commit exists; all six scoped implementation files and this summary exist; the report contains exactly 19 stage events plus the global event in canonical order; every original direct caller migrated explicitly; positive and negative fixtures prove stable status/count/digest behavior and unchanged diagnostics; all focused suites, all-feature compilation, fmt, strict lint, and the repository hook pass.
