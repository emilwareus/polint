---
phase: 39-slicing-paths-and-evidence-bundles
plan: 02
subsystem: static-analysis-engine
tags: [rust, evidence, slicing, data-flow, cfg]

requires:
  - phase: 39-01-private-evidence-substrate
    provides: private evidence rows, store, and provider lifecycle
provides:
  - Data-flow to evidence graph projection
  - Control-dependence to evidence edge projection
  - Local thin/full backward slice traversal with explicit omitted-region reporting
affects: [phase-39-paths, phase-39-rendering, phase-39-eval]

tech-stack:
  added: []
  patterns: [evidence projection, bounded local traversal]

key-files:
  created:
    - crates/polint/src/analysis/evidence/query.rs
    - crates/polint/src/analysis/slicing/mod.rs
    - crates/polint/src/analysis/slicing/local.rs
  modified:
    - crates/polint/src/analysis/evidence/mod.rs
    - crates/polint/src/analysis/evidence/provider.rs
    - crates/polint/src/analysis/evidence/store.rs
    - crates/polint/src/analysis/mod.rs

key-decisions:
  - "Projected data-flow nodes/edges into evidence rows using existing stable keys as provenance."
  - "Projected CFG control-dependence rows as full-slice-only Control evidence edges."
  - "Made thin slices value-oriented and explicit about filtered control/address/unknown evidence."

patterns-established:
  - "Thin backward slices include DataValue, Summary, and Model evidence only."
  - "Full local slices include control, address, call/return, alias, model, summary, and unknown evidence."
  - "Filtered/budgeted traversal emits omitted-region metadata and partial status."

requirements-completed: [SAE-PREC-04]

duration: 9min
completed: 2026-05-25
---

# Phase 39-02: Local Evidence Graph And Thin Full Slice Queries Summary

**Local data-flow and control evidence can now be sliced with thin/full traversal modes**

## Performance

- **Duration:** 9 min
- **Started:** 2026-05-25T14:13:34Z
- **Completed:** 2026-05-25T14:22:15Z
- **Tasks:** 2
- **Files modified:** 7

## Accomplishments

- Extended `polint.evidence` provider to project data-flow facts into evidence nodes/edges with source fact stable keys.
- Added control-dependence evidence projection from CFG facts into `EvidenceEdgeKind::Control` edges.
- Added evidence query helpers and `analysis::slicing::local` with bounded backward/forward traversal types.
- Added tests for data-flow projection, control projection, thin/full subset behavior, direct producer inclusion, control inclusion, and omitted-region reporting.

## Task Commits

1. **Tasks 1-2: Local evidence graph projection and slice traversal** - `0e99c1e` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/evidence/provider.rs` - Projects data-flow and CFG control facts into evidence output.
- `crates/polint/src/analysis/evidence/query.rs` - Private query helpers over evidence store indexes.
- `crates/polint/src/analysis/evidence/store.rs` - Incoming/outgoing edge indexes and private query accessors.
- `crates/polint/src/analysis/slicing/local.rs` - Local slice query vocabulary and traversal.
- `crates/polint/src/analysis/slicing/mod.rs` - Slicing module entrypoint.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib analysis::evidence::provider --locked` - passed
- `cargo test -p polint --lib analysis::slicing::local --locked` - passed
- `cargo test -p polint --lib analysis::evidence --locked` - passed
- `cargo clippy -p polint --lib --locked -- -D warnings` - passed

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- Initial provider projection test assumed normalized node order. The assertion was corrected to check stable-key provenance independent of order.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Wave 3 can build bounded source-to-sink paths, chops, and deterministic ranking on top of the evidence graph and local traversal primitives.

---
*Phase: 39-slicing-paths-and-evidence-bundles*
*Completed: 2026-05-25*
