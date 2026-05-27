---
phase: 30-direct-call-facts
plan: 03
subsystem: analysis
tags: [rust, analysis-kernel, call-facts, validation, debug-json]

requires:
  - phase: 30-direct-call-facts
    provides: "Plan 01 crate-private call fact contracts, CallStore indexes, and AnalysisDb storage"
  - phase: 30-direct-call-facts
    provides: "Plan 02 private polint.calls provider slot and provider manifest precision ceiling"
provides:
  - "Crate-private validate_calls hook for call sites, targets, unresolved calls, references, spans, duplicate stable keys, statuses, and precision ceilings"
  - "Test-only calls debug JSON report with safe rows, aggregate counts, and D-10 index count evidence"
  - "Targeted validation/debug tests covering outgoing, incoming, and unresolved call-store accessors"
affects: [analysis-kernel, direct-calls, future-call-extraction, eval-observation]

tech-stack:
  added: []
  patterns: ["metadata-backed internal validation", "test-only safe debug snapshots", "D-10 index evidence by accessor name"]

key-files:
  created:
    - crates/polint/src/analysis/calls/validate.rs
  modified:
    - crates/polint/src/analysis/calls/mod.rs
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/analysis_kernel/debug.rs

key-decisions:
  - "Call validation remains crate-private under analysis::calls and is invoked from metadata validation after CFG validation."
  - "Calls debug snapshots stay behind cfg(test) and expose relative paths, stable keys, spans, statuses, precision, compact payload labels, counts, and index evidence only."
  - "Exact metadata precision from polint.calls is rejected because call facts are setup-aware/conservative internal rows, not public exact facts."

patterns-established:
  - "Malformed call rows fail closed through polint/internal diagnostics with family, stable_key, field, and reason evidence."
  - "Call debug reports use aggregate counts and stable-key references instead of raw source, AST dumps, absolute paths, or dense IDs as identity."

requirements-completed: [SAE-SEM-05]

duration: 12min
completed: 2026-05-21
---

# Phase 30 Plan 03: Calls Validation and Debug Snapshots Summary

**Crate-private call fact validation plus test-only safe call debug rows, counts, and D-10 index evidence**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-21T08:10:51Z
- **Completed:** 2026-05-21T08:23:02Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added `validate_calls(db, diagnostics)` and wired it into `validate_fact_metadata` after CFG validation.
- Added deterministic `polint/internal` diagnostics for malformed call sites, targets, unresolved rows, duplicate stable keys, invalid spans, dangling references, contradictory statuses, missing unresolved reasons, and exact precision claims.
- Added test-only `calls` debug JSON with `sites`, `targets`, `unresolved`, `index_counts`, and `counts` covering language, call kind, algorithm, status, unresolved reason, and provider.

## Task Commits

1. **Task 1 RED:** `0f74f30` test(30-03): add failing call validation tests
2. **Task 1 GREEN:** `472af88` feat(30-03): validate call facts
3. **Task 2 RED:** `7b2135f` test(30-03): add failing call debug JSON tests
4. **Task 2 GREEN:** `29194f3` feat(30-03): add call debug snapshots

## Files Created/Modified

- `crates/polint/src/analysis/calls/validate.rs` - Internal call fact validation and validation-focused tests.
- `crates/polint/src/analysis/calls/mod.rs` - Registered the private validation module.
- `crates/polint/src/analysis_kernel/validation.rs` - Hooked call validation into metadata validation and added plan-targeted validation tests.
- `crates/polint/src/analysis_kernel/debug.rs` - Added test-only call debug report rows, aggregate counts, and D-10 index count tests.

## Decisions Made

- Kept validation and debug surfaces crate-private/test-facing; no SDK, runner, CLI, docs, or public call graph surface was promoted.
- Used redundant validation over stored call rows even though `CallStore` already rejects target/unresolved rows without sites before indexing.
- Counted D-10 index evidence by non-empty accessor groups so snapshots prove the accessors are wired without exposing raw index internals.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The Task 1 RED fixture initially used an outdated symbol insertion helper and the wrong test module path. The fixture was corrected before the RED commit so the committed failing test targeted call validation behavior.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib analysis_kernel::validation::calls --locked`
- `cargo test -p polint --lib analysis_kernel::debug::calls_debug_json --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None.

## Threat Flags

None - validation/debug trust-boundary changes were covered by the plan threat model.

## Next Phase Readiness

Plan 30-04 can add direct call extraction on top of validated call rows and safe debug snapshots without promoting public call graph APIs.

## Self-Check: PASSED

- Verified all created/modified key files exist.
- Verified all task commit hashes exist in git history.

---
*Phase: 30-direct-call-facts*
*Completed: 2026-05-21*
