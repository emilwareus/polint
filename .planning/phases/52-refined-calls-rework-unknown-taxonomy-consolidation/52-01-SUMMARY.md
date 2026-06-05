---
phase: 52-refined-calls-rework-unknown-taxonomy-consolidation
plan: 01
subsystem: analysis
tags: [rust, refined-calls, solver, cache, static-analysis]

requires:
  - phase: 47-unified-solver-derived-edges
    provides: solver-derived edges with provenance
provides:
  - solver-derived call edges projected into RefinedCallEdgeFact
  - refined-call cache identity that includes solver output digest
  - direct CallTargetFact compatibility floor with retired heuristic primary producers
affects: [data-flow, evidence, call-graph, unknown-taxonomy]

tech-stack:
  added: []
  patterns: [private projection over solver facts, digest-threaded provider dependency]

key-files:
  created: []
  modified:
    - crates/polint/src/analysis/refined_calls/provider.rs
    - crates/polint/src/analysis/refined_calls/cache_key.rs
    - crates/polint/src/analysis/refined_calls/store.rs
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "Solver-derived call edges are the canonical dynamic refined-call source."
  - "Direct CallTargetFact mirroring remains as a compatibility floor."
  - "Old heuristic refined-call producers are no longer appended by the primary provider path."

patterns-established:
  - "Project solver call_constraint edges only when source/target resolve to functions and provenance identifies a callsite."
  - "Assign refined-call dense IDs after stable-key normalization, not during producer append order."

requirements-completed: [GRAPH-05]

duration: 1h
completed: 2026-06-05T08:45:04Z
---

# Phase 52 Plan 01 Summary

**Solver-derived call edges now feed refined calls with provenance-preserving compatibility rows**

## Performance

- **Duration:** 1h
- **Started:** 2026-06-05T07:45:00Z
- **Completed:** 2026-06-05T08:45:04Z
- **Tasks:** 4
- **Files modified:** 4

## Accomplishments

- Added private projection from `DerivedEdgeFact` call-constraint rows into `RefinedCallEdgeFact`.
- Threaded the `polint.solver` output digest into `polint.refined_calls` cache identity.
- Removed old heuristic producer calls from the primary refined-call provider path while preserving direct target mirroring.
- Added provider tests for solver projection and solver-digest invalidation.

## Task Commits

1. **Begin phase execution state** - `66770815` (docs)
2. **Project solver derived edges** - `633b42bd` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/refined_calls/provider.rs` - Projects solver-derived call edges, maps status/precision conservatively, and records solver provenance keys.
- `crates/polint/src/analysis/refined_calls/cache_key.rs` - Updates deterministic provider parameters to describe solver projection and retired heuristic producers.
- `crates/polint/src/analysis/refined_calls/store.rs` - Removes obsolete append-order ID helper.
- `crates/polint/src/analysis_kernel/mod.rs` - Passes the solver output digest into refined-call derivation.

## Decisions Made

Followed the phase decisions: solver output is canonical for dynamic calls; direct call targets remain a compatibility floor; projection skips rows that cannot be mapped back to function endpoints and a contributing callsite.

## Deviations from Plan

None. The old heuristic modules remain available as crate-private implementation files and tests, but the primary provider path no longer calls them.

## Issues Encountered

The first provider test patch introduced a duplicate `tests` module name and exposed an obsolete `next_refined_call_id` helper. Both were fixed before commit.

## Verification

- `cargo test -p polint analysis::refined_calls::provider`
- `cargo check -p polint`
- Pre-commit hook: `cargo fmt --all -- --check`
- Pre-commit hook: `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`

## User Setup Required

None.

## Next Phase Readiness

Plan 52-03 can now add the private unknown taxonomy without needing data-flow or evidence changes first. Plan 52-02 should verify downstream consumers continue to read `RefinedCallEdgeFact` only.

---
*Phase: 52-refined-calls-rework-unknown-taxonomy-consolidation*
*Completed: 2026-06-05*
