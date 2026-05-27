---
phase: 21-provenance-precision-and-validation-metadata
plan: 03
subsystem: analysis-kernel
tags: [rust, metadata, provenance, validation, stable-keys, diagnostics]

requires:
  - phase: 21-provenance-precision-and-validation-metadata
    provides: Complete internal metadata sidecar coverage for current kernel fact families
provides:
  - Deterministic stable-key owner tracking and idempotent duplicate metadata inserts
  - Stable-key conflict recording exposed through crate-private metadata validation
  - Kernel metadata validation for missing metadata, conflicts, references, spans, and precision ceilings
  - Pre-rule validation diagnostics appended inside AnalysisKernel::run before KernelOutput returns
affects: [analysis-kernel, core-facts, validation, future-evaluation-harness]

tech-stack:
  added: []
  patterns:
    - BTree-backed stable-key ownership and conflict reports keyed by fact family
    - Crate-private validation pass returning deterministic polint/internal diagnostics

key-files:
  created:
    - crates/polint/src/analysis_kernel/validation.rs
  modified:
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/core/mod.rs

key-decisions:
  - "Stable-key ownership is keyed by (FactFamily, stable_key); conflicting payloads keep existing fact rows but become deterministic validation diagnostics."
  - "Metadata validation runs after metrics derivation and before KernelOutput is returned to rule execution."
  - "Provider precision ceilings allow lower-confidence precision labels while flagging syntax providers that claim Exact or SetupAware output."

patterns-established:
  - "Validation diagnostics use polint/internal with evidence keys that name the family, fact ref, field, stable key, producer, precision, or ceiling involved."
  - "Reference validation covers every current core fact foreign-key field from core/mod.rs through explicit field labels."

requirements-completed: [SAE-FND-02]

duration: 14m
completed: 2026-05-17
---

# Phase 21 Plan 03: Metadata Merge Validation Summary

**Deterministic metadata stable-key conflict tracking and pre-rule kernel validation diagnostics**

## Performance

- **Duration:** 14m
- **Started:** 2026-05-17T07:30:39Z
- **Completed:** 2026-05-17T07:45:04Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Changed `FactMetaStore` insertion to return `Inserted`, `Idempotent`, or `Conflict` while preserving existing fact rows for compatibility.
- Added BTree-backed stable-key owners and conflict records so duplicate stable keys with conflicting payload digests are reported deterministically.
- Added `analysis_kernel::validation` with checks for missing metadata, stable-key conflicts, span bounds/coherence, reference integrity across all current fact foreign keys, and provider precision ceilings.
- Wired validation into `AnalysisKernel::run` after metrics derivation and before `KernelOutput` is returned.

## Task Commits

1. **Task 1: Make metadata stable-key insertion deterministic** - `7174404` (test), `101f277` (feat)
2. **Task 2: Validate metadata before KernelOutput reaches rules** - `3de52b6` (test), `5464b02` (feat), `b6f39f3` (refactor)

_Note: Both tasks followed TDD with red test commits before implementation commits._

## Files Created/Modified

- `crates/polint/src/analysis_kernel/validation.rs` - Crate-private metadata validator and focused unit tests.
- `crates/polint/src/analysis_kernel/metadata.rs` - Stable-key owner/conflict tracking and idempotent insert semantics.
- `crates/polint/src/analysis_kernel/mod.rs` - Registers validation and appends validation diagnostics before returning kernel output.
- `crates/polint/src/core/mod.rs` - Adapts metadata recording to the new insert result API.

## Decisions Made

- Stable-key conflicts are diagnostic-producing metadata state in this phase, not destructive merges, preserving existing rule-facing fact vectors per compatibility requirements.
- Validation diagnostics remain internal `polint/internal` rows with deterministic evidence rather than adding any SDK, runner, or CLI metadata surface.
- Setup-aware providers may emit exact resolved facts where their own analysis proves them, while syntax-ceiling providers are prevented from claiming higher precision.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Refactored span validation helper for clippy**
- **Found during:** Task 2 final verification
- **Issue:** `cargo clippy -p polint --lib --all-features --locked -- -D warnings` rejected the initial `check_span` helper for too many arguments.
- **Fix:** Introduced a small `SpanCheck` parameter struct to keep the helper lint-clean without changing behavior.
- **Files modified:** `crates/polint/src/analysis_kernel/validation.rs`
- **Verification:** `cargo test -p polint --lib metadata_validation --locked`; `cargo clippy -p polint --lib --all-features --locked -- -D warnings`
- **Committed in:** `b6f39f3`

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** The fix was limited to the new validator helper shape and did not expand scope.

## Known Stubs

None. Stub scan matches were formatter strings used to construct deterministic evidence and metadata payloads.

## Issues Encountered

- Parallel Cargo verification briefly waited on package/artifact locks; final verification was run sequentially and passed.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib stable_key_conflict --locked`
- `cargo test -p polint --lib metadata_validation --locked`
- `cargo test -p polint --lib analysis_kernel --locked`
- `cargo test -p polint --test cli kernel_delegation_preserves_existing_rule_facts --locked`
- `cargo clippy -p polint --lib --all-features --locked -- -D warnings`
- `cargo fmt --all -- --check`

## Next Phase Readiness

Plan 21-04 can build debug/inspection proof on top of validated metadata coverage without changing rule-author APIs.

## Self-Check: PASSED

- Found summary file: `.planning/phases/21-provenance-precision-and-validation-metadata/21-03-SUMMARY.md`
- Found created file: `crates/polint/src/analysis_kernel/validation.rs`
- Found task commits: `7174404`, `101f277`, `3de52b6`, `5464b02`, `b6f39f3`

---
*Phase: 21-provenance-precision-and-validation-metadata*
*Completed: 2026-05-17*
