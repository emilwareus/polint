---
phase: 39-slicing-paths-and-evidence-bundles
plan: 01
subsystem: static-analysis-engine
tags: [rust, evidence, analysis-kernel, provider, metadata]

requires:
  - phase: 38-local-plus-summary-projected-data-flow
    provides: data-flow facts and provider ordering anchor
provides:
  - Private evidence fact vocabulary and dense id families
  - Normalized evidence store with reference validation and indexes
  - Evidence provider manifest, cache identity, empty deterministic provider output, and kernel ordering
affects: [phase-39-slicing, diagnostics, eval-fixtures, provider-order]

tech-stack:
  added: []
  patterns: [crate-private fact provider, normalized store, kernel metadata family]

key-files:
  created:
    - crates/polint/src/analysis/evidence/mod.rs
    - crates/polint/src/analysis/evidence/facts.rs
    - crates/polint/src/analysis/evidence/store.rs
    - crates/polint/src/analysis/evidence/cache_key.rs
    - crates/polint/src/analysis/evidence/provider.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/analysis/mod.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/observed.rs
    - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml

key-decisions:
  - "Kept evidence internals crate-private under analysis::evidence with no SDK exports."
  - "Placed polint.evidence after polint.data_flow and before polint.metrics."
  - "Started with deterministic empty provider output so later slices can add materialization without changing provider lifecycle."

patterns-established:
  - "Evidence rows use stable keys for truth/replay and dense run-local ids only as handles."
  - "Evidence store validates references before and after normalization."
  - "Evidence provider cache identity includes query budget, ranking, renderer mode, and upstream output digests."

requirements-completed: [SAE-PREC-04]

duration: 18min
completed: 2026-05-25
---

# Phase 39-01: Private Evidence Fact Contracts Store And Provider Wiring Summary

**Private evidence substrate wired into the analysis kernel after data flow with deterministic empty output**

## Performance

- **Duration:** 18 min
- **Started:** 2026-05-25T13:55:00Z
- **Completed:** 2026-05-25T14:13:34Z
- **Tasks:** 3
- **Files modified:** 14

## Accomplishments

- Added private evidence node, edge, bundle, path, slice, unknown, omitted-region, replay-key, status, precision, provenance, validation, budget, ranking, and renderer contracts.
- Added `EvidenceStore` normalization and validation with dense id reassignment, stable key duplicate rejection, dangling reference rejection, and indexes for later query plans.
- Added `polint.evidence` provider manifest and kernel execution after `polint.data_flow`, with deterministic cache identity and empty output refresh.
- Updated eval provider-order expectations to include `polint.evidence` before metrics.

## Task Commits

1. **Tasks 1-3: Private evidence substrate, store, cache key, provider wiring** - `5363b1e` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/evidence/facts.rs` - Private evidence row and enum vocabulary.
- `crates/polint/src/analysis/evidence/store.rs` - Normalized evidence store, validation, indexes, and tests.
- `crates/polint/src/analysis/evidence/cache_key.rs` - Evidence provider parameter/input digest helpers.
- `crates/polint/src/analysis/evidence/provider.rs` - Initial deterministic evidence provider.
- `crates/polint/src/core/mod.rs` - Evidence storage, accessors, metadata refresh, and metadata mapping.
- `crates/polint/src/analysis_kernel/provider.rs` and `mod.rs` - Provider manifest/order and kernel invocation.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib analysis::evidence --locked` - passed
- `cargo test -p polint --lib provider_order --locked` - passed
- `cargo clippy -p polint --lib --locked -- -D warnings` - passed
- SDK no-leak grep over `crates/polint/src/sdk` and runner paths found no evidence API export.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Provider-order eval tests initially failed because expected invariants did not include `polint.evidence`; fixed the fixture and assertion to match the new kernel order.
- Initial compile exposed a test-only import path mistake and unused import; fixed before commit.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Wave 2 can build local evidence graph materialization and thin/full local slice queries on top of the private evidence rows, store, and provider ordering created here.

---
*Phase: 39-slicing-paths-and-evidence-bundles*
*Completed: 2026-05-25*
