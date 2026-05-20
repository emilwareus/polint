---
phase: 29-local-cfg-and-control-dependence
plan: 01
subsystem: static-analysis-engine
tags: [rust, cfg, control-flow, private-api, stable-keys]

requires:
  - phase: 21-provenance-precision-and-validation-metadata
    provides: stable-key metadata and internal fact-family vocabulary
  - phase: 28-private-semantic-mir-and-place-identity
    provides: MIR bodies, MIR operations, and private semantic analysis patterns
provides:
  - crate-private CFG module registration
  - dense CFG ID newtypes separated from persistent stable keys
  - private CFG fact contracts for functions, nodes, blocks, edges, derived rows, and unsupported control flow
  - normalized CFG output storage
  - AnalysisDb CFG storage accessors and metadata attachment
affects: [phase-29, phase-30-direct-calls, phase-31-domains]

tech-stack:
  added: []
  patterns: [crate-private contracts, deterministic normalization, metadata-backed internal facts]

key-files:
  created:
    - crates/polint/src/analysis/cfg/mod.rs
    - crates/polint/src/analysis/cfg/ids.rs
    - crates/polint/src/analysis/cfg/facts.rs
    - crates/polint/src/analysis/cfg/store.rs
  modified:
    - crates/polint/src/analysis/mod.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/core/mod.rs

key-decisions:
  - "Keep CFG contracts crate-private with no SDK, runner, CLI, or docs promotion."
  - "Use run-local dense IDs only as handles; persistent CFG identity is carried by stable keys."
  - "Preserve duplicate CFG rows during normalization so later validation can report conflicts deterministically."

patterns-established:
  - "CfgOutput::normalized sorts each CFG fact family deterministically without deduplicating."
  - "CFG metadata uses producer/layer id polint.cfg and compact structural payload fields."
  - "Unsupported control-flow rows carry construct, source evidence, status, precision, and conservative action."

requirements-completed: [SAE-SEM-04]

duration: 18 min
completed: 2026-05-20
---

# Phase 29 Plan 01: Private CFG Contracts and Storage Summary

**Private CFG fact/storage contracts with AnalysisDb metadata-backed storage**

## Performance

- **Duration:** 18 min
- **Completed:** 2026-05-20
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added the crate-private `analysis::cfg` module with CFG ID, fact, and output storage modules.
- Added dense CFG ID newtypes for functions, nodes, blocks, edges, derived rows, and unsupported control flow.
- Added CFG fact contracts for functions, operation nodes, basic blocks, typed edges, reachability, dominators, postdominators, control dependence, and unsupported control flow.
- Added deterministic `CfgOutput::normalized` storage that preserves duplicate rows for later validation.
- Added internal CFG `FactFamily` labels plus `AnalysisDb::replace_cfg_facts` and crate-private accessors.

## Task Commits

1. **Tasks 1-3:** `c38c5b1` feat - private CFG contracts and storage.

## Files Created/Modified

- `crates/polint/src/analysis/cfg/mod.rs` - Crate-private CFG module tree.
- `crates/polint/src/analysis/cfg/ids.rs` - Dense run-local CFG ID newtypes.
- `crates/polint/src/analysis/cfg/facts.rs` - CFG fact structs, graph views, statuses, precision, and edge/node/block kinds.
- `crates/polint/src/analysis/cfg/store.rs` - `CfgOutput` container and deterministic normalization tests.
- `crates/polint/src/analysis/mod.rs` - Private CFG module registration.
- `crates/polint/src/analysis_kernel/metadata.rs` - Internal CFG fact-family labels.
- `crates/polint/src/core/mod.rs` - CFG storage vectors, replacement path, accessors, and metadata helpers.

## Decisions Made

- Kept CFG entirely internal: no SDK, runner, CLI, public JSON, README, or docs promotion.
- Stored CFG facts directly on `AnalysisDb` for this first plan so later provider wiring can replace rows atomically.
- Mapped partial, unknown, unsupported, conservative, and heuristic CFG states to lower-confidence metadata instead of claiming exact coverage.

## Deviations from Plan

- Combined the three Wave 1 tasks into one atomic code commit because the contracts, storage, and `AnalysisDb` metadata path are tightly coupled.
- Ran the valid aggregate filter `cargo test -p polint --lib analysis::cfg --locked` instead of the plan's multi-filter Cargo commands.

## Issues Encountered

- The new private storage is intentionally unused until later Phase 29 plans wire the provider and language lowerers, so the focused test currently emits dead-code warnings for the new CFG storage path.

## Verification

- `cargo test -p polint --lib analysis::cfg --locked` passed.
- `cargo fmt --all -- --check` passed.

## Known Stubs

None.

## Threat Flags

None.

## User Setup Required

None.

## Next Phase Readiness

Plan 29-02 can now implement the CFG builder and derived reachability, dominator, postdominator, and control-dependence analysis over stable internal CFG fact rows.

## Self-Check: PASSED

- Verified created files exist.
- Verified the task commit exists in git history.
- Verified CFG contracts remain crate-private.

---
*Phase: 29-local-cfg-and-control-dependence*
*Completed: 2026-05-20*
