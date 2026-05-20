---
phase: 28-private-semantic-mir-and-place-identity
plan: 04
subsystem: static-analysis-engine
tags: [rust, ts-js, mir, places, unsupported-semantics, private-api]

requires:
  - phase: 28-private-semantic-mir-and-place-identity
    provides: private MIR/place row contracts, SemanticStore foundation, and Go lowering from plans 28-01 through 28-03
  - phase: 05-typescript-adapter
    provides: Oxc TS/JS parsing and FunctionFact extraction patterns
provides:
  - crate-private TS/JS MIR lowering entrypoint
  - deterministic TS/JS MIR body rows for function declarations, variable-assigned arrows/functions, and class methods
  - TS/JS place rows for parameters, locals, globals, unknown roots, temporaries, call returns, properties, and indexes
  - TS/JS MIR operation rows for declarations, overwrites, projection mutations, branches, returns, reads, and call-shaped operations
  - structured unsupported semantic rows for dynamic TS/JS constructs and parser recovery
affects: [phase-28, phase-29-cfg, phase-30-direct-calls, phase-31-domains]

tech-stack:
  added: []
  patterns: [Oxc-local lowering, stable-key operation drafts, explicit dynamic-JS unsupported rows]

key-files:
  created:
    - crates/polint/src/analysis/mir/lower_ts.rs
  modified:
    - crates/polint/src/analysis/mir/mod.rs

key-decisions:
  - "Keep TS/JS MIR lowering crate-private under analysis::mir::lower_ts with no SDK, runner, CLI, docs, or public JSON surface."
  - "Use Oxc AST nodes only inside the lowering pass; emitted MIR/place rows contain polint-owned IDs, spans, stable keys, roots, projections, operations, and unsupported facts."
  - "Represent TS/JS calls only as MirOperationKind::Call shape evidence with call-return places; no direct target facts or call graph surface was added."

patterns-established:
  - "TS/JS lowering reparses source locally with Oxc using SourceType::from_path, then joins candidates back to existing FunctionFact rows by language, file, name, and span containment."
  - "Function-local lowering records parameter/local maps before lowering expressions so access paths can distinguish parameters, locals, globals, unknown roots, properties, and indexes deterministically."
  - "Dynamic TS/JS behavior emits UnsupportedSemanticFact rows with construct labels, source evidence, domains, conservative action, and stable keys."

requirements-completed: [SAE-SEM-03]

duration: 17 min
completed: 2026-05-20
---

# Phase 28 Plan 04: TS/JS MIR Lowering Summary

**TypeScript and JavaScript function bodies lower into deterministic private MIR, place, call-shape, and unsupported-semantics rows**

## Performance

- **Duration:** 17 min
- **Started:** 2026-05-20T08:06:04Z
- **Completed:** 2026-05-20T08:23:06Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `analysis::mir::lower_ts` with `pub(crate) fn lower_ts_mir(db: &AnalysisDb) -> MirOutput`.
- Registered `pub(crate) mod lower_ts;` from `analysis::mir::mod`.
- Lowered TS/JS function declarations, variable-assigned arrow/function expressions, and class methods into deterministic `MirBody` rows.
- Emitted place rows for parameters, locals, globals, unknown roots, temporaries, call returns, static properties, known indexes, and dynamic index projections.
- Emitted MIR operations for declaration bindings, overwrites, projection mutations, branch/control shapes, returns, reads, and call-shaped operations.
- Added unsupported rows for `eval`, `with`, `Proxy`, dynamic property keys, optional chaining, await/yield, async rejection gaps, getters/setters vocabulary, complex destructuring, spread/rest vocabulary, dynamic CommonJS require, JSX callback scheduling, and parser recovery.

## Task Commits

1. **Task 1 RED:** `128d658` test - failing TS MIR place lowering tests.
2. **Task 1 GREEN:** `806e178` feat - TS body/place lowering for functions, arrows, methods, projections, and stable identities.
3. **Task 2 RED:** `d2013fc` test - failing TS MIR operation and unsupported-semantics tests.
4. **Task 2 GREEN:** `0838cfb` feat - TS statement/call/control lowering and unsupported rows.

## Files Created/Modified

- `crates/polint/src/analysis/mir/lower_ts.rs` - New crate-private Oxc TS/JS to MIR/place lowering module with tests.
- `crates/polint/src/analysis/mir/mod.rs` - Registers `pub(crate) mod lower_ts;`.

## Decisions Made

- Kept TS/JS MIR lowering private and test-facing only in this plan.
- Reused the existing `PlaceTableBuilder` and stable-key helpers so dense `PlaceId` values remain run-local and stable keys remain persistent identity.
- Kept call lowering as shape evidence only; no direct target facts, call graph indexes, CFG edges, dominance, or postdominance behavior was added.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- One RED-test fixture initially mixed `&&` and `??` without parentheses, which Oxc correctly rejects. The fixture was corrected before the RED test commit so the committed failing tests reflected missing lowering behavior rather than parser invalid syntax.

## Verification

- `cargo test -p polint --lib analysis::mir::lower_ts::places --locked` passed.
- `cargo test -p polint --lib analysis::mir::lower_ts::operations --locked` passed.
- `cargo fmt --all -- --check` passed.
- Acceptance grep checks passed for module registration, Oxc/function/member lowering constructs, place roots/projections, assignment/call/unsupported rows, no direct-call/CFG scope creep strings, and no parser-node leakage in MIR/place row contracts.

## Known Stubs

None.

## Threat Flags

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 28 can now wire both Go and TS/JS MIR output into provider/cache/eval flows without changing the private MIR/place identity model or promoting public APIs.

## Self-Check: PASSED

- Verified created summary and TS/JS lowering files exist.
- Verified task commits exist in git history.
- Verified stub scan found no plan-blocking placeholder patterns in modified files.
- Verified no new network endpoint, auth path, file-access boundary, schema boundary, public CLI, SDK, or public JSON surface was introduced.

---
*Phase: 28-private-semantic-mir-and-place-identity*
*Completed: 2026-05-20*
