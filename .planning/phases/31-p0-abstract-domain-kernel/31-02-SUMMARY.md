---
phase: 31-p0-abstract-domain-kernel
plan: 02
subsystem: analysis
tags: [rust, abstract-interpretation, solver, transfer, domains, deterministic-results]

requires:
  - phase: 31-p0-abstract-domain-kernel
    provides: Crate-private lattice contracts, P0 slots, and deterministic product state from Plan 31-01.
  - phase: 29-local-cfg-and-control-dependence
    provides: Private CFG functions, blocks, nodes, and edges consumed by the local solver.
  - phase: 30-direct-call-facts
    provides: Private call-site, target, and unresolved-call rows consumed by conservative call transfer.
provides:
  - Deterministic local domain solver over private MIR, CFG, and call rows.
  - MIR operation and true/false edge transfer effects for P0 abstract domains.
  - Cursor-style domain results for function entry, block entry/exit, before/after operation, place observations, and top events.
affects: [phase-31, phase-32, phase-33, analysis-kernel, abstract-domains, summaries]

tech-stack:
  added: []
  patterns:
    - BTreeMap/BTreeSet-backed local worklist and result materialization.
    - Crate-private transfer context over polint-owned MIR/CFG/call facts only.

key-files:
  created:
    - crates/polint/src/analysis/domains/solver.rs
    - crates/polint/src/analysis/domains/transfer.rs
    - crates/polint/src/analysis/domains/results.rs
  modified:
    - crates/polint/src/analysis/domains/mod.rs
    - crates/polint/src/analysis/domains/state.rs

key-decisions:
  - "Keep solver, transfer, and result cursor APIs crate-private under analysis::domains with no SDK, runner, CLI, README, or docs/facts promotion."
  - "Materialize result identity and iteration through stable keys while using run-local IDs only for cursor lookup within a run."
  - "Treat calls, unsupported operations, dynamic writes, widening, and iteration budgets as explicit top/unknown events or states rather than silent certainty."

patterns-established:
  - "LocalDomainSolver: sorted function/block/operation/edge inputs with a BTreeSet worklist."
  - "TransferCx: borrowed private fact context for MIR operations, CFG branch assumptions, unsupported rows, and unresolved-call evidence."
  - "DomainResults: cursor-style crate-private result access with deterministic stable digest parts."

requirements-completed: [SAE-INT-01]

duration: 14 min
completed: 2026-05-21
---

# Phase 31 Plan 02: Deterministic Local Solver And Transfers Summary

**Deterministic local abstract interpreter with conservative MIR/call transfer and stable result cursors**

## Performance

- **Duration:** 14 min
- **Started:** 2026-05-21T11:06:41Z
- **Completed:** 2026-05-21T11:20:47Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added `LocalDomainSolver`, `SolverInput`, `SolverPolicy`, `SolverBudget`, `SolverResult`, and block-state materialization.
- Added `TransferCx`, operation transfer, branch-edge transfer, call/unsupported/dynamic-write havoc, and transfer monotonicity coverage.
- Added `DomainResults` cursor access for entry, block, before/after operation, block exit, stable iterators, place observations, and unknown/top events.
- Verified deterministic shuffled-row solving, widening fuel, budget top states, literal P0 slots, unreachable block behavior, and repeated solve digests.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add deterministic worklist solver** - `084130f` (test), `0cc7aab` (feat)
2. **Task 2: Implement MIR and edge transfer effects** - `fda3385` (test), `fd1bf0a` (feat)
3. **Task 3: Wire transfer into result cursor semantics** - `46dfb82` (test), `16d5bf9` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/domains/solver.rs` - Deterministic local solver, sorted row handling, worklist policy, widening/budget statuses, and solver tests.
- `crates/polint/src/analysis/domains/transfer.rs` - MIR operation transfer, branch assumptions, conservative call/unsupported/dynamic-write handling, and monotonicity tests.
- `crates/polint/src/analysis/domains/results.rs` - Stable-key result materialization and cursor-style state/observation access.
- `crates/polint/src/analysis/domains/state.rs` - Product-state ordering, top marking, and observed-place helpers used by solver/results/transfer.
- `crates/polint/src/analysis/domains/mod.rs` - Registers private solver, transfer, and results modules.

## Decisions Made

- Solver and transfer remain private implementation details; no public capability, SDK fact view, CLI output, README, or docs/facts surface was promoted.
- Transfers consume only polint-owned MIR, CFG, call, and unsupported rows. No parser AST, raw source, tree-sitter node, or Oxc object dependency was introduced.
- Branch assumptions use the private MIR predicate handle and only apply local P0 refinements that the current MIR evidence can support.
- Calls do not apply summaries in this plan. Resolved calls keep argument initializedness and mark return values unknown; unresolved/setup/budget-sensitive calls havoc affected places with explicit top reasons.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added ProductState helper surface for solver and transfer correctness**
- **Found during:** Task 1 (Add deterministic worklist solver)
- **Issue:** The existing product state had join/widen/digest behavior but lacked crate-private ordering, top-marking, and observed-place helpers needed to prove transfer monotonicity, materialize budget/top states, and expose place observations without duplicating state internals.
- **Fix:** Added `ProductState::leq`, `mark_reachability_top`, `mark_place_top`, and `observed_places`.
- **Files modified:** `crates/polint/src/analysis/domains/state.rs`
- **Verification:** `cargo test -p polint --lib analysis::domains --locked`
- **Committed in:** `0cc7aab`

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** The helper methods are crate-private and directly support the planned solver, transfer, and result cursor behavior. No public surface or architectural scope changed.

## Issues Encountered

None beyond the auto-fixed ProductState helper gap listed above.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib analysis::domains --locked` - passed
- `cargo fmt --all -- --check` - passed
- `rg -n "tree_sitter|oxc::|raw_source|source_text" crates/polint/src/analysis/domains || true` - no matches

## Known Stubs

None. Stub scan found no placeholder data or unwired surfaces. Regex hits were false positives inside deterministic `format!` strings.

## Next Phase Readiness

Ready for Plan 31-03 to publish domain facts/store metadata and provider/cache wiring over these private local results.

## Self-Check: PASSED

- Created files exist on disk.
- Task commits exist in git history.

---
*Phase: 31-p0-abstract-domain-kernel*
*Completed: 2026-05-21*
