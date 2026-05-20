---
phase: 28-private-semantic-mir-and-place-identity
plan: 02
subsystem: static-analysis-engine
tags: [rust, mir, semantic-store, metadata, private-api]

requires:
  - phase: 28-private-semantic-mir-and-place-identity
    provides: private MIR/place row contracts and stable IDs from plan 28-01
  - phase: 21-provenance-precision-and-validation-metadata
    provides: internal fact metadata sidecar and stable-key validation vocabulary
provides:
  - crate-private SemanticStore for MIR bodies, MIR operations, places, and unsupported semantic rows
  - deterministic AnalysisDb::replace_semantic_mir restore path with run-local ID remapping
  - polint.semantic_mir metadata coverage for stored MIR/place/unsupported rows
  - source-surface tests proving no SDK, runner, docs, README, or bench leakage
affects: [phase-28, phase-29-cfg, phase-30-direct-calls, phase-31-domains]

tech-stack:
  added: []
  patterns: [crate-private semantic store, deterministic replacement, metadata refresh, source-surface boundary tests]

key-files:
  created:
    - crates/polint/src/analysis/store.rs
  modified:
    - crates/polint/src/analysis/mod.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/analysis_kernel/metadata.rs

key-decisions:
  - "Keep stored semantic MIR artifacts behind AnalysisDb crate-private accessors and SemanticStore rather than adding SDK or RuleCtx views."
  - "Use polint.semantic_mir as the internal producer/layer id and map stored MIR precision conservatively, never Exact."
  - "Treat public-boundary proof as source-surface tests over SDK, runner, docs, README, and _bench."

patterns-established:
  - "SemanticStore::from_output validates and remaps MIR/place/unsupported references before AnalysisDb publishes the replacement."
  - "AnalysisDb replacement paths refresh all associated metadata families immediately after successful replacement."
  - "Public-boundary source tests should search for concrete private tokens rather than generic substrings that collide with existing public names."

requirements-completed: [SAE-SEM-03]

duration: 12 min
completed: 2026-05-20
---

# Phase 28 Plan 02: Private Semantic MIR Store Summary

**Crate-private semantic MIR store with deterministic replacement, metadata coverage, and public-boundary guards**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-20T07:33:11Z
- **Completed:** 2026-05-20T07:44:39Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added `SemanticStore` for MIR bodies, operations, places, unsupported rows, and by-ID indexes.
- Added `AnalysisDb::replace_semantic_mir` plus crate-private borrowed semantic MIR accessors.
- Added `polint.semantic_mir` metadata refresh for `MirBody`, `MirOperation`, `Place`, and `UnsupportedSemantic`.
- Added source-surface tests that guard SDK, runner, README, docs, and `_bench` against private MIR leakage.

## Task Commits

1. **Task 1 RED:** `0de3576` test - failing semantic MIR storage tests.
2. **Task 1 GREEN:** `08f6206` feat - crate-private semantic MIR store and replacement API.
3. **Task 2 RED:** `73249fa` test - failing semantic MIR metadata tests.
4. **Task 2 GREEN:** `6f9a30a` feat - metadata refresh and precision mappings.
5. **Task 3 RED:** `ddc305a` test - failing public-boundary guard.
6. **Task 3 GREEN:** `47f9371` test - corrected and passing public-boundary guard.

## Files Created/Modified

- `crates/polint/src/analysis/store.rs` - New private `SemanticStore`, normalization, ID remapping, reference validation, and by-ID indexes.
- `crates/polint/src/analysis/mod.rs` - Registers `analysis::store` as crate-private.
- `crates/polint/src/core/mod.rs` - Stores semantic MIR output, exposes crate-private accessors, refreshes metadata, and adds storage/metadata/boundary tests.
- `crates/polint/src/analysis_kernel/metadata.rs` - Keeps active MIR/place fact-family labels without stale dead-code expectations.

## Decisions Made

- `replace_semantic_mir` returns `Result<(), AnalysisError>` so dangling internal references are rejected instead of silently accepted.
- Stored semantic rows use `FactPrecision::SetupAware`, `Heuristic`, `Unresolved`, or `Unsupported`; `FactPrecision::Exact` is intentionally never used for `polint.semantic_mir`.
- Boundary tests assert concrete private token absence, while still allowing existing public `AnalysisDb` bench re-export.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Narrowed overly broad public-boundary assertion**
- **Found during:** Task 3 (public boundary tests)
- **Issue:** The initial `_bench` test rejected any `analysis` substring, which falsely matched the existing public bench re-export of `AnalysisDb`.
- **Fix:** Changed the assertion to reject actual analysis module exports: `pub mod analysis` and `pub use crate::analysis`.
- **Files modified:** `crates/polint/src/core/mod.rs`
- **Verification:** `cargo test -p polint --lib semantic_mir_storage_public_boundary --locked` passed.
- **Committed in:** `47f9371`

---

**Total deviations:** 1 auto-fixed (1 Rule 1)
**Impact on plan:** The fix made the boundary guard precise without widening any public API.

## Issues Encountered

- `cargo test -p polint --lib semantic_mir_storage --locked` also runs the `semantic_mir_storage_public_boundary` tests because Cargo filters by substring. This is harmless and was reflected in final verification.

## Verification

- `cargo test -p polint --lib semantic_mir_storage --locked` passed.
- `cargo test -p polint --lib semantic_mir_metadata --locked` passed.
- `cargo test -p polint --lib semantic_mir_storage_public_boundary --locked` passed.
- `cargo fmt --all -- --check` passed.

## Known Stubs

None.

## Threat Flags

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 28 can now wire MIR lowering/providers into a deterministic private store with metadata already participating in missing-metadata checks and stable-key ownership. Later CFG, direct-call, and domain phases can consume crate-private MIR/place rows without adding public SDK or CLI surfaces.

## Self-Check: PASSED

- Verified created files exist.
- Verified task commits exist in git history.
- Verified stub scan found no plan-blocking stubs in modified files.
- Verified no new network endpoint, auth path, file-access boundary, or schema trust-boundary surface was introduced.

---
*Phase: 28-private-semantic-mir-and-place-identity*
*Completed: 2026-05-20*
