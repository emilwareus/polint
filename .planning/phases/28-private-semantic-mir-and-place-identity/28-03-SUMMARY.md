---
phase: 28-private-semantic-mir-and-place-identity
plan: 03
subsystem: static-analysis-engine
tags: [rust, go, mir, places, unsupported-semantics, private-api]

requires:
  - phase: 28-private-semantic-mir-and-place-identity
    provides: private MIR/place row contracts and SemanticStore foundation from plans 28-01 and 28-02
  - phase: 04-go-adapter
    provides: tree-sitter Go parsing and FunctionFact body/span extraction patterns
provides:
  - crate-private Go MIR lowering entrypoint
  - deterministic Go MIR body rows for function and method bodies
  - Go place rows for parameters, receivers, locals, globals, temporaries, call returns, fields, indexes, and unknown roots
  - Go MIR operation rows for declarations, assignments, projection mutations, branches, returns, and call-shaped operations
  - structured unsupported semantic rows for Go concurrency, channels, reflection, unsafe, panic/recover, and parser recovery
affects: [phase-28, phase-29-cfg, phase-30-direct-calls, phase-31-domains]

tech-stack:
  added: []
  patterns: [tree-sitter-local lowering, stable-key operation drafts, run-local ID resolution after place normalization]

key-files:
  created:
    - crates/polint/src/analysis/mir/lower_go.rs
  modified:
    - crates/polint/src/analysis/mir/mod.rs

key-decisions:
  - "Keep Go MIR lowering crate-private under analysis::mir::lower_go with no SDK, runner, CLI, docs, or public JSON surface."
  - "Draft MIR operations against stable place keys, then resolve to run-local PlaceId values only after PlaceTableBuilder assigns deterministic dense IDs."
  - "Represent Go calls only as MirOperationKind::Call shape evidence and emit UnsupportedSemanticFact rows for dynamic/control constructs instead of direct-call facts."

patterns-established:
  - "Go lowering reparses source locally and lets tree-sitter nodes die inside lowering functions; emitted rows contain only polint-owned MIR/place data."
  - "Call-return and temporary places are part of place identity, while direct call target resolution remains deferred to Phase 30."
  - "Unsupported Go semantics include construct, source evidence, affected domains, conservative action, precision/status, and stable key."

requirements-completed: [SAE-SEM-03]

duration: 14 min
completed: 2026-05-20
---

# Phase 28 Plan 03: Go MIR Lowering Summary

**Go function and method bodies lower into deterministic private MIR, place, call-shape, and unsupported-semantics rows**

## Performance

- **Duration:** 14 min
- **Started:** 2026-05-20T07:48:22Z
- **Completed:** 2026-05-20T08:02:48Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `analysis::mir::lower_go` with `pub(crate) fn lower_go_mir(db: &AnalysisDb) -> MirOutput`.
- Lowered Go functions and methods into deterministic `MirBody` rows and stable place identities for receiver/parameter/local/global/temporary/call-return roots plus field and index projections.
- Added MIR operation lowering for declaration bindings, overwrites, projection mutations, simultaneous assignments, branch/control shapes, returns, reads, and call-shaped operations.
- Added structured unsupported rows for parser recovery, goroutines, defer, select, channel send/receive, reflection-like calls, `unsafe`, `panic`, and `recover`.

## Task Commits

1. **Task 1 RED:** `3bf2086` test - failing Go MIR place lowering tests.
2. **Task 1 GREEN:** `cc0f4c8` feat - Go body/place lowering for functions, methods, projections, and stable identities.
3. **Task 2 RED:** `84474e4` test - failing Go MIR operation and unsupported-semantics tests.
4. **Task 2 GREEN:** `653a82c` feat - Go statement/call/control lowering and unsupported rows.

## Files Created/Modified

- `crates/polint/src/analysis/mir/lower_go.rs` - New crate-private Go tree-sitter to MIR/place lowering module with tests.
- `crates/polint/src/analysis/mir/mod.rs` - Registers `pub(crate) mod lower_go;`.

## Decisions Made

- Kept all new lowering behavior crate-private and test-facing only.
- Used stable place keys while drafting operations so operation identity stays deterministic before dense `PlaceId` assignment.
- Kept Go calls as shape evidence only; no direct target facts, call graph indexes, CFG edges, dominance, or postdominance behavior was added.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The first RED attempt referenced the private Go adapter path directly; this was corrected to use the existing crate-private test re-export before committing the RED tests.
- Final verification commands briefly contended on Cargo file locks when launched together; Cargo serialized them and all checks passed.

## Verification

- `cargo test -p polint --lib analysis::mir::lower_go::places --locked` passed.
- `cargo test -p polint --lib analysis::mir::lower_go::operations --locked` passed.
- `cargo fmt --all -- --check` passed.
- Acceptance grep checks passed for module registration, Go parser/lowering constructs, place root/projection variants, assignment modes, call rows, unsupported rows, no direct-call/CFG scope creep strings, and no parser-node leakage in MIR/place row contracts.

## Known Stubs

None.

## Threat Flags

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 28 can now wire Go MIR output into provider/cache/eval flows and add the TS/JS lowering slice without changing the private MIR/place identity model.

## Self-Check: PASSED

- Verified created summary and Go lowering files exist.
- Verified task commits exist in git history.
- Verified stub scan found no plan-blocking placeholders; matches were Go fixture comparisons against empty strings.
- Verified no new network endpoint, auth path, file-access boundary, schema boundary, public CLI, SDK, or public JSON surface was introduced.

---
*Phase: 28-private-semantic-mir-and-place-identity*
*Completed: 2026-05-20*
