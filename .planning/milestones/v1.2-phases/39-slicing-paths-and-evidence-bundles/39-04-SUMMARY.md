---
phase: 39-slicing-paths-and-evidence-bundles
plan: 04
subsystem: static-analysis-engine
tags: [rust, evidence, slicing, summaries, interprocedural]

requires:
  - phase: 39-03-bounded-paths
    provides: bounded evidence paths, chops, deterministic ranking
provides:
  - Compressed summary evidence steps with expansion state
  - Summary edge expansion/opaque/model metadata from data-flow projection
  - Context-matched interprocedural evidence traversal
affects: [phase-39-rendering, phase-39-extension-merge, phase-39-eval]

tech-stack:
  added: []
  patterns: [summary compression, call-site stack matching, explicit unknown evidence]

key-files:
  created:
    - crates/polint/src/analysis/slicing/interprocedural.rs
  modified:
    - crates/polint/src/analysis/evidence/provider.rs
    - crates/polint/src/analysis/slicing/mod.rs
    - crates/polint/src/analysis/slicing/paths.rs

key-decisions:
  - "Summary-projected evidence edges now carry expandable, opaque, or external-model expansion state."
  - "Compressed summary steps are derived privately from evidence edges and include summary key, callable key, domain, endpoints, status, precision, provenance, and expansion."
  - "Interprocedural traversal pushes call-site context on call/parameter-in edges and requires matching pops on return/parameter-out edges."

patterns-established:
  - "Summary edges render compactly by default while preserving stable expansion keys or explicit opaque reasons."
  - "Mismatched call-site returns are rejected instead of producing false source-to-sink evidence."
  - "Unknown dynamic calls remain visible as unknown evidence edges in traversal results."

requirements-completed: [SAE-PREC-04]

duration: 7min
completed: 2026-05-25
---

# Phase 39-04: Summary Expansion And Interprocedural Context Summary

**Evidence paths now preserve summary expansion handles and call-site context**

## Performance

- **Duration:** 7 min
- **Started:** 2026-05-25T14:28:53Z
- **Completed:** 2026-05-25T14:35:20Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Updated evidence projection so summary-derived edges carry expandable, opaque, or external-model expansion metadata.
- Added compressed summary step extraction for default compact rendering with summary key, callable key, domain, endpoints, status, precision, provenance, and expansion state.
- Mapped call boundary data-flow edges to evidence call/parameter-in/parameter-out kinds for context-aware traversal.
- Added `analysis::slicing::interprocedural` with call-site stack matching, depth limits, omitted-region reporting, and visible unknown-edge reporting.
- Added tests for compressed summary steps, expandable keys, opaque summary reasons, matching caller reachability, mismatched return rejection, over-depth truncation, and unresolved dynamic calls.

## Task Commits

1. **Tasks 1-2: Summary expansion and interprocedural context** - `99f2b8d` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/evidence/provider.rs` - Summary expansion metadata and call-boundary evidence edge kinds.
- `crates/polint/src/analysis/slicing/interprocedural.rs` - Context-matched interprocedural traversal.
- `crates/polint/src/analysis/slicing/paths.rs` - Compressed summary step extraction.
- `crates/polint/src/analysis/slicing/mod.rs` - Registers the interprocedural slicing module.

## Verification

- `cargo fmt --all --check` - passed
- `cargo test -p polint --lib analysis::slicing::paths::summary --locked` - passed
- `cargo test -p polint --lib analysis::slicing::interprocedural --locked` - passed
- `cargo test -p polint --lib analysis::slicing::paths --locked` - passed
- `cargo test -p polint --lib analysis::evidence::provider --locked` - passed
- `cargo clippy -p polint --lib --locked -- -D warnings` - passed

## Deviations from Plan

None - plan executed as written.

## Issues Encountered

- Clippy flagged duplicate status branches in the first interprocedural result status implementation. The status selection was simplified and reverified.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Wave 5 can render diagnostic evidence bundles to JSON and SARIF using the compact summary and context-aware path data.

---
*Phase: 39-slicing-paths-and-evidence-bundles*
*Completed: 2026-05-25*
