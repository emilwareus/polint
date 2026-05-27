---
phase: 39-slicing-paths-and-evidence-bundles
plan: 03
subsystem: static-analysis-engine
tags: [rust, evidence, slicing, paths, ranking]

requires:
  - phase: 39-02-local-evidence-graph
    provides: evidence graph projection, incoming/outgoing indexes, local slicing
provides:
  - Bounded source-to-sink evidence path search
  - Chop query reachability intersection
  - Deterministic display-only path ranking
affects: [phase-39-summary-expansion, phase-39-rendering, phase-39-eval]

tech-stack:
  added: []
  patterns: [bounded traversal, deterministic ranking, stable-key ordering]

key-files:
  created:
    - crates/polint/src/analysis/evidence/rank.rs
    - crates/polint/src/analysis/slicing/paths.rs
  modified:
    - crates/polint/src/analysis/evidence/mod.rs
    - crates/polint/src/analysis/slicing/mod.rs

key-decisions:
  - "Path search uses bounded BFS with max paths, nodes, edges, and depth controls."
  - "Chop queries intersect source-forward and sink-backward reachability without materializing all pairs."
  - "Ranking is display-only and ordered by unknown, validation, heuristic/model, opaque-summary, length, native/exact, direct-source-sink, then stable-key components."

patterns-established:
  - "Budget truncation reports omitted-region metadata and returns BudgetExceeded status."
  - "Numeric evidence IDs remain canonicalized by the store; tests assert stable-key behavior for externally meaningful ordering."
  - "Path ranking never mutates evidence status, precision, provenance, or reachability."

requirements-completed: [SAE-PREC-04]

duration: 6min
completed: 2026-05-25
---

# Phase 39-03: Bounded Path Chop And Ranking Queries Summary

**Evidence paths can now be bounded, chopped, and ranked deterministically**

## Performance

- **Duration:** 6 min
- **Started:** 2026-05-25T14:22:15Z
- **Completed:** 2026-05-25T14:28:53Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added `analysis::slicing::paths` with private path query, path result, budget, omitted-region, and chop query types.
- Implemented bounded source-to-sink path search with deterministic edge ordering and explicit budget truncation metadata.
- Implemented chop queries as the intersection of forward reachability from the source and backward reachability from the sink.
- Added `analysis::evidence::rank` with deterministic scoring for native/exact, unknown, unvalidated, heuristic/model, opaque-summary, length, and direct path signals.
- Added targeted tests for direct paths, max-path limits, truncation reporting, chop filtering, rank ordering, stable tie behavior, and non-mutating ranking.

## Task Commits

1. **Tasks 1-2: Bounded paths, chops, and ranking** - `c0657e9` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/evidence/rank.rs` - Display-only deterministic evidence path ranking.
- `crates/polint/src/analysis/slicing/paths.rs` - Bounded path search and chop queries.
- `crates/polint/src/analysis/evidence/mod.rs` - Registers the rank module.
- `crates/polint/src/analysis/slicing/mod.rs` - Registers the paths module.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib analysis::slicing::paths --locked` - passed
- `cargo test -p polint --lib analysis::evidence::rank --locked` - passed
- `cargo test -p polint --lib analysis::evidence --locked` - passed
- `cargo clippy -p polint --lib --locked -- -D warnings` - passed

## Deviations from Plan

None - plan executed as written.

## Issues Encountered

- Initial direct-path test asserted a numeric edge ID, but the evidence store canonicalizes IDs after stable-key sorting. The test now asserts the stable path key, which is the deterministic contract.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Wave 4 can add summary expansion and interprocedural context on top of bounded evidence paths and ranking.

---
*Phase: 39-slicing-paths-and-evidence-bundles*
*Completed: 2026-05-25*
