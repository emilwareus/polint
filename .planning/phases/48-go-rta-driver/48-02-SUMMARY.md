---
phase: 48-go-rta-driver
plan: 02
subsystem: api
tags: [go, rta, call-graph, solver, semantic-graph, dispatch, budget, cache-key, config]

# Dependency graph
requires:
  - phase: 48-go-rta-driver (Plan 01)
    provides: "Three crate-private GoSemantic* RTA-signal facts (address-taken, instantiated-type, dynamic-dispatch) + their AnalysisDb accessors, plus Phase 46 method-sets/callsites and Phase 43 reachability roots"
  - phase: 47-unified-solver-core-derived-edge-provenance
    provides: "SolverPolicy trait + GoRtaPolicy stub, SolverEngine reserved seam, DerivedEdgeFact + DerivedEdgeProvenance, SolverBudget/BudgetStatus/PointsToSubBudget, polint.solver provider slot + cache key, derive_edges CopyEdge closure, precision ceiling"
  - phase: 44-private-semantic-graph-skeleton
    provides: "ConstraintKind::CallConstraint vocabulary + SemanticNodeId function/callsite nodes the RTA edges connect"
provides:
  - "A second real SolverPolicy (GoRtaPolicy): RTA fixpoint over a closed GoRtaInputs snapshot — reachability-from-roots, interface-invoke-by-method-set + func-value-by-signature dispatch filtered by the instantiated-type set, iterated to convergence under a budget"
  - "RTA-resolved Go call edges emitted as DerivedEdgeFacts (caller-node -> callee-node) in the unified vocabulary with DerivedEdgeProvenance; never exact, worst-trust status/precision, honest-unresolved preserved"
  - "Production routed through SolverEngine::run_to_solver_output: points-to CopyEdge closure + Go RTA edges converge into one SolverOutput under one SolverBudget (points-to output byte-identical)"
  - "GoRtaSubBudget channel on SolverBudget + [solver].go config table threaded into it; Go knobs + go_rta_fixpoint_v1 algo-version participate in the polint.solver cache key"
affects: [phase-52-refined-calls-projection, phase-54-bench-promotion-gate, GO-05, go_rta, solver, oracle-rta-scoring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Composition-over-rewrite seam realization: a new SolverPolicy joins the engine via run_to_solver_output without touching derive_edges, so points-to derived-edge output stays byte-identical"
    - "RTA = CHA filtered by the instantiated runtime-type set seeded from roots; the instantiated-type method-set membership is the load-bearing dispatch filter"
    - "Reconstruct-the-builder-stable-key bridge: GoRtaInputs::from_db re-derives semantic_graph's function/callsite node-key recipe to map Go `qualified`/callsite identities to already-built SemanticNodeIds without coupling to private builder internals"
    - "Whole-program-reachable rapid-type set: the instantiated/address-taken sets are the sidecar's whole-reachable-program harvest (no per-function attribution available); the RTA discriminant is preserved via the instantiated-type filter at dispatch resolution"

key-files:
  created:
    - "crates/polint/src/analysis/solver/go_rta/mod.rs"
    - "crates/polint/src/analysis/solver/go_rta/inputs.rs"
    - "crates/polint/src/analysis/solver/go_rta/fixpoint.rs"
    - "crates/polint/src/analysis/solver/go_rta/dispatch.rs"
  modified:
    - "crates/polint/src/analysis/solver/budget.rs"
    - "crates/polint/src/analysis/solver/cache_key.rs"
    - "crates/polint/src/analysis/solver/policy.rs"
    - "crates/polint/src/analysis/solver/engine.rs"
    - "crates/polint/src/analysis/solver/provider.rs"
    - "crates/polint/src/analysis/solver/mod.rs"
    - "crates/polint/src/config/mod.rs"
    - "crates/polint/src/analysis_kernel/mod.rs"

key-decisions:
  - "RTA identity domain = Go `qualified` function-name strings + `type_name` strings (the Go-frontend fact vocabulary); edge endpoints map to SemanticNodeIds via a qualified->node index reconstructed from the semantic-graph function-node stable-key recipe in GoRtaInputs::from_db."
  - "The instantiated-type and address-taken sets are seeded WHOLE (the sidecar's reachable-program rapid-type harvest) rather than per-function-attributed, because Plan 1's facts carry no containing-function attribution and over-filtering would drop real RTA targets; the RTA discriminant (interface invoke resolves ONLY to callees whose receiver type is instantiated) is preserved end-to-end at dispatch resolution. Documented in fixpoint.rs module docs."
  - "Interface invoke resolves a candidate callee = a concrete method function whose normalized receiver type is in the instantiated set AND whose method-set declares the method; pointer receivers (`*pkg.T`) are normalized (single leading `*` stripped) to match the value type-name used by the instantiated-type/method-set facts."
  - "RTA edges are floored at PointsToPrecision::Heuristic (-> FactPrecision::Heuristic) because dynamic dispatch is an over-approximation; status is Present; the store's derived_edge_precision_ceiling remains the hard never-exact gate (D-08)."
  - "Engine composition: added SolverEngine::run_to_solver_output(copy_edge_constraints) which computes the points-to closure via the UNCHANGED derive_edges, drives registered policies via run(), concatenates points-to + policy derived_edges, worst-case-combines budget_status, and normalizes — NOT a parallel free function (D-02). derive_edges itself was not modified, so points-to byte-identity holds."
  - "engine weakest_status/weakest_precision/status_rank/precision_rank promoted to pub(crate) so the Go RTA dispatch resolver reuses the SAME severity ordering for worst-trust edge status (D-09) rather than minting a parallel ranking."

patterns-established:
  - "GoRtaSubBudget { address_taken_threshold: 256, max_candidates_per_callsite: 128, max_rta_rounds: 32 } mirrors PointsToSubBudget; [solver].go config keys (address_taken_threshold / max_candidates_per_callsite / max_rta_rounds) are Option<usize> overlaid onto the defaults via SolverConfig::to_go_sub_budget."
  - "BudgetExceeded latches on any of: round cap (max_rta_rounds), worklist-step cap (max_outer_iterations), per-callsite candidate cap (max_candidates_per_callsite), or address-taken-set > address_taken_threshold — edges resolved before the cap keep their honest status (review finding #R1); no unbounded loop."
  - "Deletion-invalidation extends to RTA edges: each edge's stable_key embeds its provenance witness (callsite + dispatch + method-set + instantiated-type contributing facts), so deleting any contributing fact does not reproduce that edge fact."

requirements-completed: [GO-05]

# Metrics
duration: 90min
completed: 2026-06-02
---

# Phase 48 Plan 02: Go RTA Driver Summary

**The reserved `GoRtaPolicy` stub is now a real RTA driver: a `analysis::solver::go_rta` fixpoint resolves Go interface-invoke (by instantiated-type method-set) and func-value (by signature) dispatch into `DerivedEdgeFact` call edges, routed through the reserved `SolverEngine` seam so points-to and Go RTA converge into one byte-identical-points-to `SolverOutput` under one config-driven `SolverBudget`.**

## Performance

- **Duration:** ~90 min
- **Started:** 2026-06-02
- **Completed:** 2026-06-02
- **Tasks:** 3
- **Files modified:** 12 (4 created, 8 modified)

## Accomplishments
- New crate-private `analysis::solver::go_rta` module (mod/inputs/fixpoint/dispatch) with the mandatory D-04 naming-collision guard distinguishing the unified derived-edge vocabulary from the Go-frontend fact vocabulary.
- `solve_go_rta` runs the reachability ⊗ dispatch fixpoint: seeded from Phase 43 roots, interface-invoke resolved by intersecting the invoked method with the method-sets of instantiated runtime types, func-value resolved by address-taken signature match, iterated to a fixed point under explicit caps.
- RTA edges are `DerivedEdgeFact`s (caller fn-node -> callee fn-node) reusing `PointsToStatus`/`PointsToPrecision`, carrying `DerivedEdgeProvenance` (callsite + dispatch + method-set + instantiated-type contributing facts), never exact (Heuristic ceiling), worst-trust status — no parallel Go edge family (D-04).
- Production routes through `SolverEngine::run_to_solver_output` (D-02): the unchanged `derive_edges` points-to closure + the Go RTA policy edges merge into one normalized `SolverOutput` under one budget; `points_to_via_engine_equals_solve_points_to` + `derive_edges_is_shuffle_stable` + the points-to/determinism fixtures stay byte-identical.
- `GoRtaSubBudget` + `[solver].go` config table wired and threaded through the kernel; Go knobs + `go_rta_fixpoint_v1` algo-version fold into the `polint.solver` cache key (all three locked trip-wire tests updated in the same edit); runaway dispatch latches the existing `BudgetExceeded` diagnostic (D-13).

## Task Commits

Each task was committed atomically:

1. **Task 1: GoRtaSubBudget + [solver] config table + cache-key digest** - `df0b5bba` (feat)
2. **Task 2: go_rta RTA fixpoint + dispatch resolver emitting DerivedEdgeFacts** - `057dba4d` (feat)
3. **Task 3: Extend PolicyOutcome, replace the stub, route through SolverEngine, thread config budget** - `ff7bfd43` (feat)

**Plan metadata:** committed with STATE/ROADMAP/REQUIREMENTS updates (docs: complete plan)

## Files Created/Modified
- `crates/polint/src/analysis/solver/go_rta/mod.rs` - Module root: D-04 naming-collision guard + RTA-model docs; `pub(crate) mod` list + `solve_go_rta`/`GoRtaInputs` re-exports.
- `crates/polint/src/analysis/solver/go_rta/inputs.rs` - `GoRtaInputs` closed snapshot + `from_db`: joins Go-frontend facts to the already-built semantic-graph function/callsite nodes (reconstructs the builder node-key recipe), builds BTree-keyed reachable/instantiated/address-taken/method-set/dispatch structures; `from_db` integration test.
- `crates/polint/src/analysis/solver/go_rta/fixpoint.rs` - `solve_go_rta`: bounded reachability ⊗ dispatch worklist mirroring `points_to::solver`; global monotonic step counter; round/step/candidate/address-taken caps; dedup + `normalized()`; 7 unit tests.
- `crates/polint/src/analysis/solver/go_rta/dispatch.rs` - `resolve_callsite`: interface-invoke-by-method-set (instantiated filter) + func-value-by-signature; emits provenance-bearing DerivedEdgeFacts; reuses engine worst-trust helpers; 2 unit tests.
- `crates/polint/src/analysis/solver/budget.rs` - `GoRtaSubBudget` hung on `SolverBudget.go`; existing defaults (10_000 / 64 / points_to) byte-identical; go-defaults unit test.
- `crates/polint/src/analysis/solver/cache_key.rs` - `budget_parts` + `go_rta_fixpoint_v1` algo-version include the Go knobs (D-12); all three locked trip-wire tests updated.
- `crates/polint/src/analysis/solver/policy.rs` - `PolicyOutcome.derived_edges` channel (D-03); `empty()` stays empty; real `GoRtaPolicy::new(GoRtaInputs)`; positive go_rta test + TS-stub-stays-empty test.
- `crates/polint/src/analysis/solver/engine.rs` - `run_to_solver_output` composition (D-02) + `combine_budget_status`; `weakest_*`/`*_rank` promoted to `pub(crate)`; updated stub-drive test.
- `crates/polint/src/analysis/solver/provider.rs` - Routes through `SolverEngine` over `[PointsToPolicy, GoRtaPolicy, TsTokensPolicy]`; output digest folds `budget.go.*`; existing budget-exceeded/validation/cycle path unchanged.
- `crates/polint/src/analysis/solver/mod.rs` - Registered `pub(crate) mod go_rta`.
- `crates/polint/src/config/mod.rs` - `SolverConfig { go: SolverGoConfig }` registered as `[solver]`; `to_go_sub_budget` mapper; default + override unit tests.
- `crates/polint/src/analysis_kernel/mod.rs` - Threads `[solver].go` config into `SolverBudget.go` at the solver call site (mirrors the reachability provider's config reach).

## Decisions Made
- **Identity domain + node mapping.** The RTA fixpoint operates over Go `qualified` function strings and `type_name` strings (the Go-frontend vocabulary). Edge endpoints map to `SemanticNodeId`s via `GoRtaInputs::from_db`, which reconstructs `semantic_graph::build`'s function-node and callsite-node stable-key recipes and looks them up in the already-built `db.semantic_nodes()`. This keeps the mapping a composition over the public node set (no coupling to the builder's private helpers) and is proven by a `from_db` integration test against a real `AnalysisDb`.
- **Whole-reachable rapid-type set.** Plan 1's instantiated-type and address-taken facts carry no per-function attribution, and the sidecar harvested them over the already-reachable SSA program. So the fixpoint seeds the instantiated/address-taken sets whole and preserves the RTA discriminant at dispatch resolution (interface invoke resolves ONLY to callees whose receiver type is instantiated). Over-filtering by reachability would drop real RTA targets. Documented in `fixpoint.rs`.
- **Precision floor.** RTA-resolved dynamic edges claim `PointsToPrecision::Heuristic` (over-approximation), never exact; the store's `derived_edge_precision_ceiling` stays the hard gate.
- **Engine composition shape.** Chose `run_to_solver_output(copy_edge_constraints)` over a parallel free function (D-02 mandate). It calls the UNCHANGED `derive_edges` for the points-to closure, drives registered policies via `run()`, concatenates edges, worst-case-combines budget status, and `normalized()`s. `derive_edges` was not touched, so points-to byte-identity holds.
- **Registered policy set.** The provider registers `PointsToPolicy` + `GoRtaPolicy` + `TsTokensPolicy` so the unified solver genuinely drives all sub-domains under one budget; points-to edges still come exclusively from `derive_edges` (PointsToPolicy contributes no channel edges), and TsTokensPolicy stays an honest empty stub (Phase 49).

## Deviations from Plan

None - plan executed exactly as written.

All three tasks landed as specified. The transient `#[cfg_attr(not(test), allow(dead_code))]` on `SolverConfig::to_go_sub_budget` (Task 1) and `#[allow(unused_imports)]` on the `go_rta` module re-exports (Task 2) were intentional in-plan bridges to keep the `clippy -D warnings` pre-commit hook green between tasks; both were removed in Task 3 once the kernel/provider/policy wiring consumed them. This is normal intra-plan sequencing, not a scope deviation.

## Issues Encountered
- **Pre-commit hook is `clippy --workspace --all-targets --all-features --locked -- -D warnings` (plus `cargo fmt --check`).** Each commit had to be warning-free. Resolved by: running `cargo fmt` before each commit; adding a `from_db` integration test in `inputs.rs` (rather than a blanket `#[allow(dead_code)]`) so the whole input-construction chain is genuinely exercised in non-test builds once consumed; and collapsing a nested `if let`/`if` flagged by `clippy::collapsible_if`.
- **Test-construction bug (self-caught).** An early `interface_scenario` test helper built `methods_by_receiver` but omitted it from the `GoRtaInputs` struct literal, so interface dispatch resolved nothing. Fixed by passing the field; all interface/deletion tests then passed.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plan 3 (verification) can now add: the iteration-cap fixture (D-14), the x/tools RTA native fixtures (D-15), and the polyglot Go+TS canary (D-16), and assert the recall lift + non-regression. The `BudgetExceeded` signal, the engine routing, and the determinism/leak gates are all in place.
- Phase 52 (GRAPH-05) can read the RTA-derived edges from the `polint.solver` slot to project `refined_calls`; the `polint.solver` provider-order slot is unchanged.
- `oracle-rta` scoring (Phase 43 D-17) consumes the richer RTA edge set behind the existing reachable-graph marking contract.

## Self-Check: PASSED

- SUMMARY: `.planning/phases/48-go-rta-driver/48-02-SUMMARY.md` — FOUND
- Commits: `df0b5bba` (Task 1), `057dba4d` (Task 2), `ff7bfd43` (Task 3) — all FOUND
- Key created files (go_rta/{mod,inputs,fixpoint,dispatch}.rs) — all FOUND
- `cargo test -p polint` green: 1941 lib + 140 integration + 5 public_surface_leak + 1 doc, 0 failures; `points_to_via_engine_equals_solve_points_to`, `derive_edges_is_shuffle_stable`, `provider_manifests_list_solver_between_semantic_graph_and_refined_calls`, the determinism gate (10-shuffle byte-identical, Go + TS), and the public-surface-leak gate all green.

---
*Phase: 48-go-rta-driver*
*Completed: 2026-06-02*
