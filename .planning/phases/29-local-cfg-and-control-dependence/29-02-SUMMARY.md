---
phase: 29-local-cfg-and-control-dependence
plan: 02
subsystem: static-analysis-engine
tags: [rust, cfg, graph, dominance, control-dependence]

requires:
  - phase: 29-local-cfg-and-control-dependence
    plan: 01
    provides: private CFG fact contracts and normalized storage
provides:
  - shared crate-private CFG builder
  - read-only CFG graph view
  - reachability derivation
  - dominator and postdominator derivation
  - control-dependence derivation
affects: [phase-29, phase-30-direct-calls, phase-31-domains]

tech-stack:
  added: []
  patterns: [deterministic graph construction, iterative dominance, postdominance-based control dependence]

key-files:
  created:
    - crates/polint/src/analysis/cfg/builder.rs
    - crates/polint/src/analysis/cfg/graph.rs
    - crates/polint/src/analysis/cfg/derived.rs
  modified:
    - crates/polint/src/analysis/cfg/mod.rs

key-decisions:
  - "Drive language CFG lowering through one shared builder rather than duplicating graph construction per language."
  - "Derive reachability, dominators, postdominators, and control dependence from selected graph views instead of storing language-authored derived rows."
  - "Use a synthetic unified exit for postdominance and preserve controlling edge evidence on control-dependence facts."

patterns-established:
  - "CfgGraph exposes sorted block successors and predecessors over a selected view."
  - "CfgBuilder emits virtual entry/exit nodes, deterministic blocks, operation nodes, and typed edges."
  - "Control dependence uses postdominator edge-walk semantics and deduplicates structurally identical rows."

requirements-completed: [SAE-SEM-04]

duration: 24 min
completed: 2026-05-20
---

# Phase 29 Plan 02: CFG Builder and Derived Analyses Summary

**Shared CFG construction plus deterministic reachability, dominance, postdominance, and control-dependence derivation**

## Performance

- **Duration:** 24 min
- **Completed:** 2026-05-20
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added `CfgBuilder` for deterministic crate-private CFG construction over MIR body/operation IDs and source spans.
- Added `CfgGraph` as a read-only selected-view helper for sorted nodes, blocks, edges, successors, predecessors, entry block, and synthetic exit block lookup.
- Added reachability derivation that walks sorted successors and excludes unreachable blocks from dominator facts.
- Added iterative dominator and postdominator derivation, with postdominance using a synthetic unified exit policy.
- Added control-dependence derivation from postdominators while preserving controlling edge id, edge kind, view, precision, and status.

## Task Commits

1. **Tasks 1-3:** `ab0312b` feat - CFG builder, graph view, and derived analyses.

## Files Created/Modified

- `crates/polint/src/analysis/cfg/builder.rs` - Shared CFG builder and construction tests.
- `crates/polint/src/analysis/cfg/graph.rs` - Read-only graph view and sorted predecessor/successor tests.
- `crates/polint/src/analysis/cfg/derived.rs` - Reachability, dominator, postdominator, and control-dependence algorithms and tests.
- `crates/polint/src/analysis/cfg/mod.rs` - Private module registration for builder, graph, and derived analysis.

## Decisions Made

- Kept all builder/graph/derived APIs `pub(crate)` and did not add SDK, runner, CLI, public JSON, README, or docs surface.
- Used simple deterministic set-based algorithms for function-local graphs, matching the research recommendation for the first implementation slice.
- Represented postdominance through selected graph views and a synthetic unified exit so later validation can attach precision notes for unsupported exits.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Replaced invalid multi-filter Cargo commands**
- **Found during:** Verification
- **Issue:** Cargo accepts only one test filter before `--`; commands like `cargo test ... analysis::cfg::builder analysis::cfg::graph` are invalid.
- **Fix:** Ran equivalent single-filter commands for `analysis::cfg::builder`, `analysis::cfg::graph`, `analysis::cfg::derived`, plus aggregate `analysis::cfg`.
- **Files modified:** None
- **Verification:** All replacement commands passed.
- **Committed in:** N/A

**2. [Rule 1 - Bug] Corrected postdominator virtual-exit predecessor handling**
- **Found during:** Task 2 tests
- **Issue:** Exit blocks were not treated as having the synthetic unified exit as a predecessor in the reversed graph.
- **Fix:** Added virtual-exit predecessor handling for selected real exit blocks.
- **Files modified:** `crates/polint/src/analysis/cfg/derived.rs`
- **Verification:** `cargo test -p polint --lib analysis::cfg::derived --locked` passed.
- **Committed in:** `ab0312b`

**3. [Rule 1 - Bug] Wired return test blocks to the normal exit**
- **Found during:** Task 2 tests
- **Issue:** The multiple-return postdominator fixture expected normal-exit postdominance but did not connect return blocks to the normal exit block.
- **Fix:** Added `CfgBuilder::normal_exit_block()` and used return edges in the fixture.
- **Files modified:** `crates/polint/src/analysis/cfg/builder.rs`, `crates/polint/src/analysis/cfg/derived.rs`
- **Verification:** `cargo test -p polint --lib analysis::cfg::derived --locked` passed.
- **Committed in:** `ab0312b`

---

**Total deviations:** 3 auto-fixed (2 Rule 1, 1 Rule 3)
**Impact on plan:** Deviations improved algorithm correctness and verification fidelity; no public surface was added.

## Issues Encountered

- Focused tests still emit dead-code warnings from the Plan 29-01 `AnalysisDb` CFG storage path because provider wiring is scheduled for later Phase 29 plans.
- Parallel Cargo test commands briefly contended on package/artifact locks; all relevant tests still completed successfully.

## Verification

- `cargo test -p polint --lib analysis::cfg::builder --locked` passed.
- `cargo test -p polint --lib analysis::cfg::graph --locked` passed.
- `cargo test -p polint --lib analysis::cfg::derived --locked` passed.
- `cargo test -p polint --lib analysis::cfg --locked` passed.
- `cargo fmt --all -- --check` passed.
- Acceptance `rg` checks for builder, graph, derived facts, and control-dependence fields passed.

## Known Stubs

None.

## Threat Flags

None.

## User Setup Required

None.

## Next Phase Readiness

Plan 29-03 can wire `polint.cfg` provider execution, cache identity, validation, and debug output on top of the private builder and derived-analysis substrate.

## Self-Check: PASSED

- Verified created files exist.
- Verified the task commit exists in git history.
- Verified tests and formatting pass.

---
*Phase: 29-local-cfg-and-control-dependence*
*Completed: 2026-05-20*
