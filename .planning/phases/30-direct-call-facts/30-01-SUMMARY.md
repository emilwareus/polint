---
phase: 30-direct-call-facts
plan: 01
subsystem: analysis
tags: [rust, analysis-kernel, call-facts, metadata]

requires:
  - phase: 28-private-semantic-mir-and-place-identity
    provides: "MIR call-site handles, MIR operation IDs, and place identity"
  - phase: 29-local-cfg-and-control-dependence
    provides: "CFG storage and metadata patterns mirrored by call facts"
provides:
  - "Crate-private direct-call fact contracts for sites, targets, unresolved evidence, status, reason, algorithm, precision, and provenance vocabulary"
  - "CallTargetId dense run-local handle separate from stable keys"
  - "Deterministic CallOutput normalization and CallStore indexes for caller/site/outgoing/incoming/unresolved lookups"
  - "AnalysisDb call fact replacement, accessors, indexed helpers, and polint.calls metadata rows"
affects: [analysis, analysis-kernel, direct-calls, future-call-provider, summaries]

tech-stack:
  added: []
  patterns: ["crate-private fact families", "BTreeMap deterministic indexes", "metadata sidecar refresh"]

key-files:
  created:
    - crates/polint/src/analysis/calls/mod.rs
    - crates/polint/src/analysis/calls/facts.rs
    - crates/polint/src/analysis/calls/store.rs
  modified:
    - crates/polint/src/analysis/mod.rs
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/analysis_kernel/metadata.rs

key-decisions:
  - "Call facts remain crate-private under analysis::calls with no SDK, runner, CLI, or docs promotion."
  - "CallStore validates target and unresolved site references before publishing indexes."
  - "CALLS_PROVIDER_ID is polint.calls and call metadata uses compact status/kind/algorithm/reason/stable-key payload fragments."

patterns-established:
  - "CallOutput mirrors CfgOutput: normalize deterministic row order while preserving duplicates."
  - "AnalysisDb replacement owns both row slices and an optional CallStore index cache."

requirements-completed: [SAE-SEM-05]

duration: 17min
completed: 2026-05-21
---

# Phase 30 Plan 01: Direct Call Fact Foundation Summary

**Crate-private direct-call fact contracts with deterministic storage indexes and polint.calls metadata sidecars**

## Performance

- **Duration:** 17 min
- **Started:** 2026-05-21T07:37:06Z
- **Completed:** 2026-05-21T07:54:01Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added `CallTargetId` plus `CallSiteFact`, `CallTargetFact`, and `UnresolvedCallFact` contracts under private `analysis::calls`.
- Added deterministic `CallOutput` normalization and `CallStore` indexes for callers, sites, outgoing/incoming targets, and unresolved reason/status.
- Added `AnalysisDb::replace_call_facts`, call accessors/index helpers, and metadata families `CallSite`, `CallTarget`, and `UnresolvedCall` under `polint.calls`.

## Task Commits

1. **Task 1 RED:** `8753bb6` test(30-01): add failing test for call fact contracts
2. **Task 1 GREEN:** `d870c61` feat(30-01): implement call fact contracts
3. **Task 2 RED:** `6e65599` test(30-01): add failing test for call store indexes
4. **Task 2 GREEN:** `8a221d8` feat(30-01): implement deterministic call store
5. **Task 3 RED:** `938f710` test(30-01): add failing test for call fact storage metadata
6. **Task 3 GREEN:** `5342ead` feat(30-01): store call facts in analysis db

## Files Created/Modified

- `crates/polint/src/analysis/calls/facts.rs` - Private call fact structs and call vocabulary.
- `crates/polint/src/analysis/calls/store.rs` - Normalized call output, deterministic indexes, and dangling-site validation.
- `crates/polint/src/analysis/calls/mod.rs` - Private calls module root.
- `crates/polint/src/analysis/ids.rs` - Added `CallTargetId`.
- `crates/polint/src/analysis/mod.rs` - Registered `analysis::calls`.
- `crates/polint/src/core/mod.rs` - Added call storage, replacement/accessors, metadata refresh, and tests.
- `crates/polint/src/analysis_kernel/metadata.rs` - Added call fact family labels.

## Decisions Made

- Kept all new call fact surfaces `pub(crate)`; no public `Calls<'_>` or supported `CallGraph<'_>` behavior was added.
- Used `BTreeMap`/sorted vectors for deterministic indexes and preserved duplicate rows during normalization.
- Used existing metadata sidecar patterns with compact fragments only; no raw source, AST dumps, absolute paths, or run-local IDs are used as persistent identity.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Corrected call metadata test IDs**
- **Found during:** Task 3 (Store call facts in AnalysisDb with metadata families)
- **Issue:** The new metadata tests expected run ID `0` while helper rows used call site/target IDs starting at `1`, so the tests were checking absent metadata instead of the intended rows.
- **Fix:** Aligned Task 3 test fixtures to use call site/target ID `0` for metadata assertions and strengthened non-exact precision checks with `expect(...)`.
- **Files modified:** `crates/polint/src/core/mod.rs`
- **Verification:** `cargo test -p polint --lib call_fact_metadata --locked`
- **Committed in:** `5342ead`

---

**Total deviations:** 1 auto-fixed (Rule 1).
**Impact on plan:** Test-only correction needed for accurate verification; implementation scope stayed within the plan.

## Issues Encountered

None beyond the documented test fixture correction.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib analysis::calls::facts --locked`
- `cargo test -p polint --lib analysis::ids --locked`
- `cargo test -p polint --lib analysis::calls::store --locked`
- `cargo test -p polint --lib call_fact_storage --locked`
- `cargo test -p polint --lib call_fact_metadata --locked`
- `cargo test -p polint --lib analysis::calls --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None.

## Next Phase Readiness

Provider wiring can now publish normalized call rows into `AnalysisDb` without adding public call graph APIs.

## Self-Check: PASSED

- Verified all created/modified key files exist.
- Verified all task commit hashes exist in git history.

---
*Phase: 30-direct-call-facts*
*Completed: 2026-05-21*
