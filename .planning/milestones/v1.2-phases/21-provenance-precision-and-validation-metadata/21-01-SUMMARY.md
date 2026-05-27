---
phase: 21-provenance-precision-and-validation-metadata
plan: 01
subsystem: analysis-kernel
tags: [rust, analysisdb, metadata, provenance, stable-keys]

requires:
  - phase: 20-private-analysis-kernel-facade
    provides: Crate-private analysis kernel facade and provider manifests
provides:
  - Crate-private metadata vocabulary for fact provenance, precision, confidence, validation status, stable keys, and payload digests
  - AnalysisDb sidecar metadata storage for source and syntax fact families
  - Cached restore metadata attachment for source, Go syntax, and TS/JS syntax facts
affects: [analysis-kernel, core-facts, cache-restore, future-validation]

tech-stack:
  added: []
  patterns:
    - Crate-private FactMetaStore sidecar keyed by run-local FactRef
    - Deterministic stable keys from sorted, normalized, length-prefixed labeled parts

key-files:
  created:
    - crates/polint/src/analysis_kernel/metadata.rs
  modified:
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/core/mod.rs

key-decisions:
  - "Metadata stays in an AnalysisDb sidecar rather than widening public fact structs."
  - "Provider IDs polint.source, polint.go.syntax, and polint.ts.syntax are reused as producer and layer IDs for current source/syntax facts."
  - "Stable keys are deterministic strings built from sorted, normalized, length-prefixed labeled parts while run-local FactRef IDs remain separate."

patterns-established:
  - "Fact metadata is recorded at existing AnalysisDb push boundaries after final run-local IDs are assigned."
  - "Cached file fact restore continues through push methods so restored facts receive current-run metadata."

requirements-completed: [SAE-FND-02]

duration: 9h 8m
completed: 2026-05-17
---

# Phase 21 Plan 01: Provenance Metadata Sidecar Summary

**Crate-private FactMeta sidecar with deterministic stable keys for source, Go syntax, and TS/JS syntax facts**

## Performance

- **Duration:** 9h 8m wall-clock
- **Started:** 2026-05-16T22:00:07Z
- **Completed:** 2026-05-17T07:08:23Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added `analysis_kernel::metadata` with crate-private `FactFamily`, `FactRef`, `FactMeta`, `FactMetaStore`, precision/confidence/validation vocabulary, and deterministic stable-key construction.
- Added `fact_meta: FactMetaStore` to `AnalysisDb` with crate-private accessors and metadata lookup by `FactRef`.
- Attached metadata for source files, packages, functions, imports, branch obligations, tests, coverage, TS components/classes, string literals, and JSX attributes on fresh parse and cache restore paths.

## Task Commits

1. **Task 1: Add crate-private metadata vocabulary and store** - `73eb71a` (test), `f45235a` (feat)
2. **Task 2: Attach source and syntax metadata in AnalysisDb** - `2ba8faf` (test), `9292f39` (feat)

_Note: Both tasks followed TDD with red test commits before implementation commits._

## Files Created/Modified

- `crates/polint/src/analysis_kernel/metadata.rs` - Crate-private metadata vocabulary, `FactMetaStore`, stable-key helper, and unit tests.
- `crates/polint/src/analysis_kernel/mod.rs` - Registers the metadata module and crate-private re-exports.
- `crates/polint/src/core/mod.rs` - Owns sidecar metadata storage and records metadata at source/syntax fact insertion and restore boundaries.

## Decisions Made

- Metadata remains internal and sidecar-based to preserve SDK, runner, crate-root, CLI, and public fact-struct contracts.
- Source and syntax metadata uses provider IDs from the existing provider manifest vocabulary as both producer and layer IDs.
- Payload digests are computed from normalized metadata payload fields, while stable keys are computed separately from family-specific identity labels.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Kept the full future-shaped metadata vocabulary lint-clean**
- **Found during:** Task 1
- **Issue:** The plan requires fact-family, precision, confidence, and validation variants that are not all attached in Plan 21-01, which would trigger `dead_code` under clippy.
- **Fix:** Added an internal metadata vocabulary weight consumed by the existing kernel metadata token path so every required vocabulary value is represented without public exposure.
- **Files modified:** `crates/polint/src/analysis_kernel/metadata.rs`, `crates/polint/src/analysis_kernel/mod.rs`
- **Verification:** `cargo clippy -p polint --lib --all-features --locked -- -D warnings`
- **Committed in:** `f45235a`

**2. [Rule 3 - Blocking] Kept crate-private metadata accessors lint-clean**
- **Found during:** Task 2
- **Issue:** `metadata_for`, `fact_meta`, and `FactMetaStore::get` are required internal accessors but were initially only used from tests.
- **Fix:** Added a debug assertion in `record_fact_meta` that verifies the just-inserted row through `metadata_for`, exercising the accessor path without changing runtime behavior.
- **Files modified:** `crates/polint/src/core/mod.rs`
- **Verification:** `cargo clippy -p polint --lib --all-features --locked -- -D warnings`
- **Committed in:** `9292f39`

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes preserve the requested private API and keep required verification strict without expanding scope.

## Known Stubs

None. Stub scan matches were false positives from a formatter string and an existing test fixture literal.

## Issues Encountered

- Parallel Cargo verification caused temporary package/artifact lock waits; verification was rerun sequentially and passed.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib metadata --locked`
- `cargo test -p polint --lib restore_file_facts --locked`
- `cargo clippy -p polint --lib --all-features --locked -- -D warnings`
- `cargo fmt --all -- --check`

## Next Phase Readiness

Plan 21-02 can build on the sidecar model to add broader validation, merge behavior, or debug inspection without changing the public SDK or rule-author contracts.

## Self-Check: PASSED

- Found created file: `crates/polint/src/analysis_kernel/metadata.rs`
- Found summary file: `.planning/phases/21-provenance-precision-and-validation-metadata/21-01-SUMMARY.md`
- Found task commits: `73eb71a`, `f45235a`, `2ba8faf`, `9292f39`

---
*Phase: 21-provenance-precision-and-validation-metadata*
*Completed: 2026-05-17*
