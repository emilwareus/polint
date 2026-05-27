---
phase: 29-local-cfg-and-control-dependence
plan: 04
subsystem: static-analysis-engine
tags: [rust, cfg, go, semantic-mir, control-flow]

requires:
  - phase: 29-local-cfg-and-control-dependence
    plan: 01
    provides: private CFG fact contracts and storage
  - phase: 29-local-cfg-and-control-dependence
    plan: 02
    provides: shared CFG builder and derived analyses
  - phase: 29-local-cfg-and-control-dependence
    plan: 03
    provides: private CFG provider slot, validation, cache identity, and debug output
provides:
  - Go semantic-MIR-to-CFG lowering
  - provider merge path for Go CFG output
  - Go CFG tests for branches, loops, short-circuiting, returns, abrupt control, spawn/defer/panic, and unsupported rows
affects: [phase-29, phase-30-direct-calls, phase-31-domains]

tech-stack:
  added: []
  patterns: [private Go CFG lowering, conservative CFG unsupported facts, shared derived-analysis provider path]

key-files:
  created:
    - crates/polint/src/analysis/cfg/lower_go.rs
  modified:
    - crates/polint/src/analysis/cfg/mod.rs
    - crates/polint/src/analysis/cfg/builder.rs
    - crates/polint/src/analysis/cfg/provider.rs

key-decisions:
  - "Lower Go CFG from private semantic MIR rows and do not depend on raw tree-sitter AST objects."
  - "Keep reachability, dominators, postdominators, and control dependence in the shared provider path rather than language lowering."
  - "Represent Go-specific spawn, defer, panic, select, goto, fallthrough, and unsupported semantics with typed CFG edges or unsupported control-flow rows."

patterns-established:
  - "Go branch rows lower into conservative conditional, loop, or short-circuit shapes based on MIR/source evidence."
  - "Returns add explicit return edges to the normal exit and leave following MIR operations in unreachable blocks."
  - "Unsupported MIR rows affecting CFG become UnsupportedControlFlowFact rows with conservative precision/status."

requirements-completed: []

duration: 28 min
completed: 2026-05-20
---

# Phase 29 Plan 04: Go CFG Lowering Summary

**Private Go CFG lowering over semantic MIR**

## Performance

- **Duration:** 28 min
- **Completed:** 2026-05-20
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added `analysis::cfg::lower_go::lower_go_cfg` and wired the CFG provider to consume it before shared derived analyses run.
- Lowered Go MIR branches, returns, calls, regular operations, and unsupported rows into deterministic CFG nodes, blocks, edges, and unsupported-control-flow facts.
- Added conservative edge handling for loop, short-circuit, spawn, defer, recover, panic, unknown/select, goto, and fallthrough evidence.
- Added a builder accessor for exceptional exit blocks so panic edges can target the exceptional exit when present.
- Added focused Go CFG tests covering straight-line return, branch/loop/short-circuit edges, return-tail reachability structure, and abrupt/unsupported constructs.

## Task Commits

1. **Tasks 1-2:** `601c468` feat - Go CFG lowering and provider integration.

## Files Created/Modified

- `crates/polint/src/analysis/cfg/lower_go.rs` - Go semantic-MIR-to-CFG lowerer and tests.
- `crates/polint/src/analysis/cfg/mod.rs` - Registered the private Go CFG module.
- `crates/polint/src/analysis/cfg/builder.rs` - Added exceptional-exit block lookup.
- `crates/polint/src/analysis/cfg/provider.rs` - Replaced empty CFG output with Go CFG lowering output.

## Deviations from Plan

### Auto-fixed Issues

None.

## Issues Encountered

- Existing Go MIR exposes coarse branch rows rather than structured statement subgraphs, so the first CFG lowerer uses conservative source/MIR evidence to classify branches as conditional, loop, or short-circuit.
- Some Go-specific constructs remain unsupported by MIR as precise control shapes; the CFG lowerer preserves them as typed edges where possible and `UnsupportedControlFlowFact` rows where precision is not proven.

## Verification

- `cargo test -p polint --lib analysis::cfg::lower_go --locked` passed.
- `cargo test -p polint --lib go_cfg_abrupt_and_unsupported --locked` passed.
- `cargo test -p polint --lib cfg_provider --locked` passed.
- `cargo test -p polint --lib analysis::cfg --locked` passed.
- `cargo test -p polint --lib analysis_kernel::validation::cfg --locked` passed.
- `cargo fmt --all -- --check` passed.
- Acceptance `rg` checks for Go CFG lowering, no derived-analysis calls inside `lower_go.rs`, abrupt/unsupported evidence, and no lifecycle file writes passed.

## Known Stubs

- Go CFG lowering is conservative over the current MIR shape; richer Go structured CFG can improve once MIR carries explicit statement nesting, labels, and terminator targets.
- `select`, `goto`, and `fallthrough` are surfaced as unsupported or conservative CFG evidence until precise MIR lowering supports target-aware modeling.

## Threat Flags

None.

## User Setup Required

None.

## Next Phase Readiness

Plan 29-05 can add TS/JS CFG lowering through the same private provider and shared derived-analysis path.

## Self-Check: PASSED

- Verified created files exist.
- Verified the task commit exists in git history.
- Verified targeted tests, CFG validation tests, formatting, and acceptance searches pass.

---
*Phase: 29-local-cfg-and-control-dependence*
*Completed: 2026-05-20*
