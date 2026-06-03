# Phase 48: Go RTA Driver - Context

**Gathered:** 2026-06-02
**Status:** Ready for planning
**Mode:** `/gsd:discuss-phase 48 --auto`

<domain>
## Phase Boundary

Phase 48 makes the **reserved `GoRtaPolicy` stub real**. Phase 47 shipped the unified
`analysis::solver` core with a `SolverPolicy` trait and exactly one real impl
(`PointsToPolicy`); the Go RTA policy was an *honest stub* (`id() == "go_rta"`,
`solve()` returns `PolicyOutcome::empty()`). This phase replaces that stub with a
hand-rolled **Rapid Type Analysis (RTA) driver** over the Phase 44 constraint
vocabulary, lifting Go x/tools RTA recall from the current baseline (10% precision /
2.7% recall) toward the 70–90% algorithmic ceiling while holding precision as a
first-class target. It delivers exactly **one** requirement: **GO-05**.

Concretely, `analysis::solver::go_rta` must implement, over a closed snapshot:

1. **Reachable functions from roots** — seed from the Phase 43 `ReachabilityRootFact`
   set and expand the reachable function set as dispatch is resolved.
2. **Address-taken function tracking** — the set of functions whose address is taken
   (function values, method values, closures) — an RTA dispatch input.
3. **Dynamic call sites by signature** — unresolved-dynamic callsites matched to
   candidate callees by call signature.
4. **Runtime types through interfaces** — the "rapid type" set of concrete types
   actually instantiated/converted-to-interface in the reachable program.
5. **Interface invoke by method-set** — interface dispatch resolved by intersecting
   the interface method with the method-sets of instantiated runtime types.
6. **Fixed-point iteration** — iterate reachability ⊗ instantiated-types ⊗ dispatch to
   convergence under an explicit budget.

This phase is the **Go driver only**. It explicitly does **NOT**:

- Implement the JS/TS function-token propagation driver (`solver::ts_tokens`) — **Phase 49
  (JS-04)**. Phase 48 leaves `TsTokensPolicy` an honest stub. (Phases 48 and 49 are
  parallel-eligible: drivers share the solver core but their iteration logic and fixtures
  are independent.)
- Rework `refined_calls::provider` to project over solver output — **GRAPH-05, Phase 52**.
  Phase 48 emits RTA-derived edges into the existing `polint.solver` provider slot; Phase 52
  reads them.
- Build the unknown-taxonomy consolidation or `polint inspect unknowns --format json` —
  **TAX-01, Phase 52**. Phase 48's `BudgetExceeded` signals feed it but the taxonomy is later.
- Implement the JS/TS object/property/prototype/`this` model — **JS-05, Phase 50** — or the
  adaptation-model `ModelEdge` producer — **ADAPT-01/02, Phase 51**.
- Add Go VTA / type-flow refinement above RTA — **PREC-FUT-01** (out of v1.3).
- Enforce the BENCH-01 promotion gate (per-suite precision floors, F-score β=0.5, the
  canary as a hard gate) — **Phase 54**. Phase 48 *adds* the polyglot canary fixture and
  proves non-regression, but the hard promotion gate is wired in Phase 54.

No new public SDK type is promoted (v1.3 discipline). All `go_rta` types stay `pub(crate)`;
the Phase 42 public-surface-leak gate and the Phase 43 determinism gate (auto-enrolled via
`provider_manifests()`) MUST stay green.

</domain>

<decisions>
## Implementation Decisions

### Driver Integration Seam — How `go_rta` Plugs Into the Solver Core (GO-05)

- **D-01:** Add a new sub-module `analysis::solver::go_rta`
  (`crates/polint/src/analysis/solver/go_rta/`). Every type, enum, fact, and function is
  `pub(crate)`. It is the second real `SolverPolicy` implementation (after `PointsToPolicy`),
  replacing the Phase 47 `GoRtaPolicy` stub in `solver::policy`. Add a top-of-module doc
  comment per the D-04 naming-collision discipline distinguishing the unified solver's
  derived-edge vocabulary from Go-frontend fact vocabulary.
- **D-02:** **Route production through the reserved `SolverEngine` seam (MANDATORY
  direction).** Phase 47's `engine.rs` module docs explicitly reserve this: *"when the Go
  RTA and TS token drivers register as policies, production will route through the engine so
  multiple sub-domains converge under one budget."* Phase 48 realizes that: make
  `GoRtaPolicy::solve()` perform the RTA fixpoint and have the `polint.solver` provider drive
  registered policies through `SolverEngine::run()` rather than calling only the free
  `derive_edges` (the `CopyEdge` closure). The points-to `CopyEdge` derivation
  (`derive_edges`) and the Go RTA derivation must both flow into the same `SolverOutput`
  under one `SolverBudget`. **Points-to derived-edge output and its fixtures MUST stay
  byte-identical** — the seam change is additive (composition, not rewrite). The exact
  composition (engine that aggregates per-policy `SolverOutput`s vs. a thin orchestration
  wrapper) is a planner/researcher decision provided (a) points-to output is byte-identical,
  (b) the engine owns the single-fixpoint-per-run / bounded-outer-iteration contract, and
  (c) the determinism gate stays green.
- **D-03:** **Extend `PolicyOutcome` to carry derived edges.** Today `PolicyOutcome` only
  carries `points_to: Option<PointsToSolveResult>`. The Go RTA policy produces
  `DerivedEdgeFact`s (call edges), so `PolicyOutcome` gains a channel for a policy's derived
  edges (e.g. `derived_edges: Vec<DerivedEdgeFact>` or a `SolverOutput` fragment). Honest
  stubs continue to return the empty outcome. Keep the existing points-to field so the D-03
  (Phase 47) fold stays byte-identical. Exact field shape is planner's discretion.
- **D-04:** **RTA call edges are `DerivedEdgeFact`s in the existing vocabulary.** A resolved
  Go call edge is `caller-function-node -> callee-function-node` as a `DerivedEdgeFact`
  (`source`/`target` are `SemanticNodeId`s) reusing the shared `PointsToStatus`/
  `PointsToPrecision` status/precision vocabulary. Do NOT mint a parallel Go edge fact family.
  Each edge carries `DerivedEdgeProvenance` (D-08 of Phase 47): contributing fact IDs
  (callsite + method-set + instantiated-type facts, total-ordered by stable ID), the
  producing `ConstraintKind` (the `CallConstraint` that obligated the dispatch), and the
  solver step. The deletion-invalidation property (Phase 47 D-09) extends to RTA edges:
  deleting a contributing instantiated-type/method-set/callsite fact must not reproduce the
  same derived edge.

### RTA Input Signals — Frontend-Extension Boundary (GO-05 Success Criteria 1)

- **D-05:** **Phase 48 extends the Go frontend to emit the RTA-required SSA signals — this is
  in-scope, not Phase 46 creep.** The Phase 46 frontend emits functions, method-sets, and
  callsites (`GoSemanticCallStatus::{ResolvedStatic, UnresolvedDynamic, Unsupported}`) but
  emits **no address-taken facts and no instantiated-concrete-type ("rapid type") facts**.
  GO-05's named mechanisms ("address-taken function tracking", "runtime types through
  interfaces") *cannot* be implemented without these inputs, so surfacing them is part of
  delivering the driver. **This is feasible as a natural extension:** the sidecar already
  builds an SSA program (`ssautil.AllPackages(...)` + `prog.Build()`,
  `emit.go:99-100`) and already walks `fn.Blocks` / `block.Instrs`
  (`emit.go:247-248`). Phase 48 adds emission of:
  1. **Address-taken functions** — from `*ssa.MakeClosure`, function-valued globals/params,
     and method-value references in the instruction walk.
  2. **Instantiated runtime types** — the concrete types passed to `*ssa.MakeInterface`
     (plus `*ssa.Alloc`/`*ssa.New`/composite-literal types as the planner/researcher confirm
     against x/tools RTA semantics) — the RTA "rapid type" set.
  3. **Dynamic-callsite dispatch detail** — for each `UnresolvedDynamic` callsite, the
     interface type + invoked method name (or func-value signature) needed for method-set
     matching. Today the callsite fact carries `caller`/`static_callee`/`status` but not the
     interface/method discriminant.
  New facts stay crate-private, length-prefixed/stable-keyed from official Go identities
  (Phase 46 D-12/D-13), validated, and participate in the cache key. **Exact fact shapes and
  which SSA instruction families to harvest are a planner/researcher decision** grounded in
  x/tools RTA — keep the surface minimal and honest (emit `Unsupported`/unresolved rows
  rather than fabricating matchable identities, Phase 46 D-15).
- **D-06:** **RTA algorithm = CHA filtered by the instantiated-type set, seeded from roots.**
  Resolve each dynamic interface callsite to the set of reachable callees whose receiver type
  (a) is in the current instantiated runtime-type set AND (b) has the invoked method in its
  method-set (Phase 46 `GoSemanticMethodSetFact`). Address-taken functions whose signature
  matches a func-value callsite are candidate callees for that site. This is RTA proper (the
  instantiated-type filter is what distinguishes it from coarse CHA and is what lifts recall
  without flooding precision). Newly-reached functions contribute new instantiated types and
  new callsites — hence the fixed point.
- **D-07:** **Reuse Phase 43 roots + reachable-graph marking as the seed and the output
  contract.** Seed the reachable set from `ReachabilityRootFact.target_function`. Phase 43
  D-18 explicitly documented that *"Phases 47/48 replace the [direct-call] edge set with
  solver-derived edges behind this same marking contract"* — so Phase 48's RTA-derived edges
  become the richer edge set the reachable-graph BFS/DFS walks, and `oracle-rta` scoring
  (Phase 43 D-17: score only edges whose source is reachable-from-roots) consumes them. Do
  NOT re-invent roots or the marking fact family.

### Precision & Honesty Posture (GO-05; inherits Phase 47 D-06)

- **D-08:** **Derived RTA edges never claim exact precision (D-06 ceiling inherited).** An
  RTA-resolved interface/dynamic edge is an over-approximation; it claims at most
  `SetupAware`/`Heuristic` via the existing `derived_edge_precision_ceiling` (which is
  asserted to never return `FactPrecision::Exact`). A statically-resolved call
  (`ResolvedStatic` callsite) may be the most precise tier the ceiling allows but still not
  exact. Unresolved-after-RTA dispatch stays an honest unresolved/`Unknown` signal, never a
  fabricated edge — matching the project's "no edge flooding to inflate recall" discipline.
- **D-09:** **Worst-trust provenance discipline carries over.** Where an RTA edge is justified
  by multiple contributing facts (callsite + method-set + instantiated-type), its
  status/precision is the weakest across the adopted derivation and its provenance justifies
  that status (Phase 47 engine review findings #4/#R2). No laundering an unresolved/budget
  hop into a confident edge.

### `solver_config.go.*` Surface & Budget Channel (GO-05 Success Criteria 3)

- **D-10:** **Add a `[solver]` config table with a `go` sub-table.** No solver config surface
  exists today (`SolverBudget` is constructed from defaults in the provider). Add a `[solver]`
  table to `crates/polint/src/config/mod.rs` (beside the Phase 43 `ReachabilityConfig`
  `[reachability]` table), exposing per-language `solver_config.go.*` knobs — at minimum an
  **address-taken threshold** (the roadmap's named example) and the RTA iteration/dispatch
  caps. Keep it `.polint.toml` config surface (permitted under v1.3 discipline; NOT SDK
  promotion). Exact knob names/shape are planner/researcher discretion; keep minimal and
  honest.
- **D-11:** **Thread Go knobs through a new `GoRtaSubBudget` channel on `SolverBudget`**,
  mirroring the existing `PointsToSubBudget` (`budget.rs`). `SolverBudget` already carries
  cross-domain `max_steps` + `max_outer_iterations`; add `go: GoRtaSubBudget` for the
  Go-specific caps (address-taken threshold, per-callsite candidate cap, RTA round cap). The
  `SolverBudget::default()` for existing fields MUST stay byte-identical (10_000 / 64) so
  points-to fixtures do not change. Config values map into the budget; absence falls back to
  defaults.
- **D-12:** **Go RTA knobs participate in the `polint.solver` cache key.** The solver cache key
  already digests the `SolverBudget` (Phase 47 D-15); the new Go sub-budget rides along, plus
  the new Go-frontend fact digests (address-taken / instantiated-type / dispatch facts) the
  driver consumes. A Go-knob change must invalidate downstream (forward-compatible with
  CACHE-01/02, Phase 53).

### Budget Exhaustion — Runaway Dispatch (GO-05 Success Criteria 2)

- **D-13:** **Reuse the unified `BudgetStatus::BudgetExceeded` honest signal — no new enum.**
  When the RTA fixpoint hits a ceiling (outer-iteration cap, per-callsite candidate explosion,
  or the Go sub-budget threshold), latch run-level `BudgetStatus::BudgetExceeded` and surface
  it as a provider diagnostic — never a silent precision drop, never an unbounded loop
  (Phase 47 D-06/D-11). Edges fully derived before the cap was hit keep their honest status
  (the `derive_edges` R1 discipline: exhaustion costs the edges never reached, signalled
  run-level, not a downgrade of already-derived edges).
- **D-14:** **Iteration-cap fixture is a named acceptance artifact.** Add a fixture whose
  interface-dispatch graph is large/cyclic enough to exceed a deliberately-tight RTA cap, and
  assert `BudgetExceeded` is emitted (observable in the solver output / `solver_step_count` /
  `budget_exceeded_reasons` JSON reserved by Phase 43 D-23) rather than dropped. This is the
  GO-05 success-criterion-2 proof.

### Verification & Acceptance (GO-05 Success Criteria 2/3/4)

- **D-15:** **Native Go x/tools RTA fixture coverage proves benchmark-grade edges.** Reuse the
  existing `go-x-tools-rta-callgraph` suite (`scoring_mode = "oracle-rta"`, Phase 43 D-16) and
  its adapter (`crates/polint/src/eval/external/go_x_tools_callgraph.rs`). Add native
  fixtures under `tests/eval-fixtures/` exercising interface dispatch, method values/closures
  (address-taken), and dynamic calls, and assert RTA produces the expected reachable-only
  edges. Measure recall lift toward the 70–90% ceiling while precision holds (the
  hard per-suite floor lands in Phase 54; Phase 48 demonstrates the lift).
- **D-16:** **Add the polyglot Go+TS canary fixture (it does not exist yet).** Success
  criterion 3 requires a polyglot Go+TS canary exercising cross-language non-regression: with
  the Go RTA driver active and the TS policy still a stub, a mixed Go+TS fixture must show Go
  edges resolved and TS behavior unchanged (no cross-language interference through the shared
  solver core). This canary is *added* here; Phase 54 (BENCH-01) later promotes it to a hard
  gate.
- **D-17:** **Determinism + leak gates stay green (named acceptance criteria).** The
  `polint.solver` provider is already auto-enrolled in the Phase 43 determinism harness via
  `provider_manifests()`; the new Go RTA derivation must keep the 10-shuffle byte-identical
  observed JSON green (BTree-ordered accumulation + dense-IDs-after-stable-key-sort, D-02 of
  Phase 47). All new `go_rta` types stay `pub(crate)`; do NOT extend `ALLOWED_PRELUDE` in
  `crates/polint/tests/public_surface_leak.rs`.

### Claude's Discretion

- Internal file layout of `analysis::solver/go_rta/` (e.g. `mod.rs`, `rta.rs`/`fixpoint.rs`,
  `dispatch.rs`, `instantiated_types.rs`, `address_taken.rs`, `budget.rs`) — planner's choice,
  provided visibility stays `pub(crate)` and digest discipline matches the rest of `solver/`.
- The exact `PolicyOutcome` extension shape and how `SolverEngine` aggregates per-policy
  derived edges into one `SolverOutput` (D-02/D-03) — planner picks the cleaner composition,
  provided points-to output stays byte-identical and the engine owns the budget/fixpoint
  contract.
- Which SSA instruction families the frontend harvests for instantiated types and
  address-taken funcs, and the exact new Go-frontend fact/constraint shapes (D-05) —
  planner/researcher decide against x/tools RTA semantics; keep minimal, honest, stable-keyed.
- The exact `solver_config.go.*` knob names and `GoRtaSubBudget` fields (D-10/D-11) —
  planner/researcher decide; the address-taken threshold is the roadmap-named default.
- Natural plan slicing (planner confirms): (1) Go-frontend RTA-signal emission (address-taken
  + instantiated types + dynamic-callsite dispatch detail) + lowering; (2) `go_rta` RTA
  fixpoint policy + `PolicyOutcome`/`SolverEngine` production routing + `GoRtaSubBudget` +
  `[solver]` config + cache-key; (3) verification — iteration-cap fixture, x/tools RTA native
  fixtures, polyglot Go+TS canary, determinism + leak gates.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 48 goal ("lifting Go x/tools RTA recall toward the 70–90%
  algorithmic ceiling while holding precision") + the **4 Success Criteria** (the acceptance
  contract), GO-05 mapping, and the Phase 48 ↔ Phase 49 parallel-eligibility note.
- `.planning/REQUIREMENTS.md` — **GO-05 (line 37)** requirement text (the six named RTA
  mechanisms); phase→requirement map (line 124 GO-05 → Phase 48; line 149 parallel with
  Phase 49). Note BENCH-01 (line 57, Phase 54) owns the hard promotion gate + canary gate;
  PREC-FUT-01 (line 65, Go VTA) is out of v1.3.
- `.planning/PROJECT.md` — v1.3 milestone goal (raise Go RTA + Jelly recall from <3% to
  >25–30% with precision first-class); current baselines **Go x/tools RTA 10% precision /
  2.7% recall** (line 73); the "single shared semantic graph / solver core with
  language-specific frontends" keystone; private-analysis-first + no-public-SDK-promotion
  discipline.
- `.planning/STATE.md` — current v1.3 state (Phase 47 complete, ready for Phase 48), open
  repo-admin action T-42-04-10 (leak-gate branch protection).

### Immediate Upstream Phase Context (read first)

- `.planning/phases/47-unified-solver-core-derived-edge-provenance/47-CONTEXT.md` — **Primary
  upstream.** The unified `analysis::solver` core, `SolverPolicy` trait, the `GoRtaPolicy`
  honest stub Phase 48 replaces, `SolverBudget`/`BudgetStatus`/`PointsToSubBudget`,
  `DerivedEdgeFact` + `DerivedEdgeProvenance` (+ the deletion property test), the reserved
  `SolverEngine` multi-policy seam, the `polint.solver` provider slot + cache key + determinism
  inheritance, and the precision ceiling (never exact).
- `.planning/phases/46-go-semantic-frontend-sidecar/46-CONTEXT.md` — The `polint-go-frontend`
  sidecar + `go::semantic` lowering Phase 48 extends. D-18 (interface/dynamic dispatch left
  unresolved *for Phase 48*), D-12/D-13 (official Go identities + stable keys), D-15 (honest
  representation, no fabricated identities), D-19 (candidate/private fact families allowed),
  the NDJSON protocol + cache-input discipline, and the "keep an unresolved interface-dispatch
  fixture for Phase 48" specific.
- `.planning/phases/43-reachability-roots-per-suite-scoring-mode/43-CONTEXT.md` — The
  `ReachabilityRootFact` seed set, the reachable-graph **marking contract** Phase 48 plugs its
  RTA edges into (D-18 names Phases 47/48 as the edge-set replacement), the `oracle-rta`
  scoring semantics (D-17), the `[reachability]` config precedent (D-13) the `[solver]` table
  sits beside, the reserved `solver_step_count`/`budget_exceeded_reasons` JSON (D-23), and the
  inherited determinism gate (D-20–D-25).

### v1.3 Graph Engine Benchmark Research

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` — **Primary architecture +
  motivation source.** Go x/tools RTA baseline, the algorithmic recall ceiling, the
  frontend→shared-graph→solver framing, and precision-first targets.
- `research/call-graphs/FINAL-REPORT.md` — Layered call-graph + constraint-based solving;
  RTA/reachability framing, unresolved/dynamic edge handling, repo-local provenance.
- `research/type-alias-points-to/SUBAGENT-FINDINGS.md` — Official Go tooling authority
  (`go/types`, `go/packages`, `go/ssa`, x/tools callgraph/RTA) — the reference for what
  `MakeInterface`/address-taken/method-set data the frontend can extract.
- `research/cfg-control-flow/SUBAGENT-FINDINGS.md` — `go/ssa` as the Go CFG/semantic substrate
  (the SSA instruction families the frontend walks).
- `research/evaluation-harness/STANDARD.md` — Determinism requirements inherited from the
  Phase 43 gate.

### Existing Implementation Touch Points

- `crates/polint/src/analysis/solver/policy.rs` — `SolverPolicy` trait, `PolicyOutcome`, and
  the `GoRtaPolicy` stub (`id()=="go_rta"`, `solve()→empty()`) Phase 48 makes real (D-01/D-03).
- `crates/polint/src/analysis/solver/engine.rs` — `SolverEngine`/`SolverRunResult`/`derive_edges`;
  the reserved multi-policy seam (module docs lines 18–28) Phase 48 routes production through
  (D-02), and the worst-trust/per-source-budget/global-monotonic-step discipline RTA reuses.
- `crates/polint/src/analysis/solver/budget.rs` — `SolverBudget`/`BudgetStatus`/`PointsToSubBudget`;
  add `GoRtaSubBudget` here (D-11). Keep defaults byte-identical.
- `crates/polint/src/analysis/solver/facts.rs` — `DerivedEdgeFact` + `derived_edge_precision_ceiling`
  (never exact, D-08); RTA call edges are this family (D-04).
- `crates/polint/src/analysis/solver/provenance.rs` — `DerivedEdgeProvenance`/`ContributingFact`
  + deletion-invalidation; RTA edges attach this (D-04).
- `crates/polint/src/analysis/solver/{provider.rs,cache_key.rs,store.rs,validate.rs}` — the
  `polint.solver` provider (today calls `derive_edges` directly), cache key (digests budget +
  upstream), `SolverOutput::normalized()` stable-key sort, validation. Wire Go RTA + the new
  sub-budget/fact digests here (D-02/D-12).
- `crates/polint/src/go/semantic/facts.rs` — `GoSemanticFunctionFact`/`GoSemanticCallsiteFact`
  (`GoSemanticCallStatus::UnresolvedDynamic` = the dispatch RTA resolves) /
  `GoSemanticMethodSetFact` (type→methods, the method-set input). **No address-taken or
  instantiated-type fact exists — Phase 48 adds them (D-05).**
- `crates/polint/src/go/semantic/{lower.rs,provider.rs,cache_key.rs,validate.rs}` — Go
  lowering into semantic-graph constraints; extend to lower the new RTA-signal facts (D-05).
- `crates/polint/go-sidecar/polint-go-frontend/internal/semantic/emit.go` — the sidecar emitter.
  **Already builds SSA (`ssautil.AllPackages` + `prog.Build()`, lines 99–100) and walks
  `fn.Blocks`/`block.Instrs` (lines 247–248)** — the natural place to harvest `*ssa.MakeInterface`
  instantiated types + `*ssa.MakeClosure`/func-value address-taken signals + dynamic-callsite
  dispatch detail (D-05).
- `crates/polint/src/analysis/reachability/{facts.rs,traverse.rs,provider.rs}` —
  `ReachabilityRootFact` (RTA seed, D-07) + the reachable-graph traversal/marking RTA edges
  feed (Phase 43 D-18 contract).
- `crates/polint/src/analysis/semantic_graph/constraints.rs` — `ConstraintKind`
  (`CallConstraint { callsite }` is the dispatch obligation RTA resolves; `TypeConstraint`);
  the solver's input vocabulary.
- `crates/polint/src/config/mod.rs` — config root + `ReachabilityConfig` (`[reachability]`
  table, line ~46). Add the `[solver]` table with the `go` sub-table here (D-10).
- `crates/polint/src/analysis/ids.rs` — run-local dense ID newtypes; add any Go RTA / new
  Go-frontend fact IDs here.
- `crates/polint/src/analysis_kernel/provider.rs` — provider manifest + `provider_manifests()`
  (determinism auto-enrollment) + `provider_order_for_test()` (≈7 snapshot sites if a provider
  slot changes — see memory `polint-kernel-provider-snapshot-sites`).
- `crates/polint/src/eval/external/go_x_tools_callgraph.rs` (671 lines) +
  `research/evaluation-harness/suites/go-x-tools-rta-callgraph.toml` (`scoring_mode =
  "oracle-rta"`) — the RTA benchmark adapter + suite for D-15.
- `crates/polint/tests/public_surface_leak.rs` — v1.3 leak gate; all `go_rta` types stay
  `pub(crate)` (D-17).
- `tests/eval-fixtures/{determinism/,...}` — native fixture-tree precedent; the iteration-cap
  fixture (D-14), x/tools RTA fixtures (D-15), and the new polyglot Go+TS canary (D-16) live
  here.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::solver` (Phase 47) already provides the deterministic worklist core,
  `SolverBudget`/`BudgetStatus`, the `SolverPolicy` trait + `SolverEngine` **reserved
  precisely for this driver**, `DerivedEdgeFact` + `DerivedEdgeProvenance` (with the deletion
  property test), the precision ceiling, the `polint.solver` provider + cache key, and
  determinism-gate auto-enrollment. Phase 48 fills in the Go policy and routes production
  through the engine — it does not build solver infrastructure from scratch.
- The `polint-go-frontend` sidecar **already has the full SSA program** (`ssautil.AllPackages`
  + `prog.Build()`) and **already iterates SSA instructions** (`fn.Blocks`/`block.Instrs`),
  so address-taken + `MakeInterface` runtime-type extraction is an additive walk over data
  that is already in hand — the high-cost SSA build is done.
- `GoSemanticMethodSetFact` (type→methods) is the method-set input for interface-invoke
  matching; `GoSemanticCallsiteFact` already separates `ResolvedStatic` vs `UnresolvedDynamic`
  — the unresolved set is exactly what RTA targets.
- `ReachabilityRootFact` + the Phase 43 reachable-graph marking contract are the RTA seed and
  the output sink (D-18 named Phases 47/48 as the edge-set replacement).
- `PointsToSubBudget` is the structural template for `GoRtaSubBudget`; `ReachabilityConfig`
  (`[reachability]`) is the template for the `[solver]` config table.
- `eval::external::go_x_tools_callgraph` + the `oracle-rta` suite + the reserved
  `solver_step_count`/`budget_exceeded_reasons` JSON fields give Phase 48 its benchmark and
  its budget-signal observability for free.

### Established Patterns

- **Composition over rewrite** (Phase 42–47): the Go RTA policy joins the engine without
  rewriting points-to; points-to output stays byte-identical.
- **Honest status/precision + no edge flooding** (Phase 44/46/47): unresolved-after-RTA
  dispatch stays unresolved; derived edges reject exact; budget exhaustion is an explicit
  `BudgetExceeded` signal, never a silent drop or an unbounded loop.
- **Dense IDs after stable-key sort + BTree accumulation** (v1.2/Phase 43/47): the mechanism
  that keeps the 10-shuffle determinism gate green when the new provider derivation lands.
- **Official Go identities + length-prefixed stable keys** (Phase 46 D-12/D-13): new
  RTA-signal facts key off `go/types`/`ssa.Function` identity, never run-local order.
- **Config is `.polint.toml`, not SDK** (Phase 43 D-13): the `[solver]` table is permitted
  config surface, not public promotion.
- **Provider cache-key digests budget + upstream + version** (every v1.2/v1.3 provider): the
  Go sub-budget + new Go-frontend fact digests ride the `polint.solver` cache key.

### Integration Points

- `solver::policy` gains a real Go RTA policy (stub removed); `solver::engine` aggregates Go
  RTA + points-to derived edges into one `SolverOutput` under one budget; `solver::provider`
  routes through the engine.
- `solver::budget` gains `GoRtaSubBudget`; `config/mod.rs` gains the `[solver]`/`go` table;
  the solver cache key absorbs both.
- `go/semantic` (+ the sidecar `emit.go`) gains address-taken + instantiated-type +
  dynamic-callsite-dispatch facts and their lowering.
- `analysis::reachability` consumes the richer RTA edge set behind the Phase 43 marking
  contract; `oracle-rta` scoring reads it.
- `tests/eval-fixtures/` gains the iteration-cap fixture, x/tools RTA native fixtures, and the
  polyglot Go+TS canary; the determinism + leak gates stay green; `public_surface_leak.rs`
  unchanged.

</code_context>

<specifics>
## Specific Ideas

- **The single most load-bearing decision is D-05 (frontend-extension boundary).** RTA without
  an instantiated-type set is just CHA and will not lift recall toward the 70–90% ceiling. The
  good news the scout surfaced: the sidecar already owns the SSA program and instruction walk,
  so emitting `MakeInterface` runtime types + address-taken funcs is additive, not a new
  pipeline. The planner/researcher must confirm the exact x/tools-RTA-faithful instruction set
  to harvest.
- **Honor the reserved seam (D-02).** Phase 47 deliberately built `SolverEngine` + `SolverPolicy`
  and documented that Phase 48/49 route production through it. Do not bolt a parallel
  `derive_go_rta_edges` free function alongside `derive_edges` and skip the engine — that would
  abandon the multi-sub-domain-under-one-budget design the core was built for. The acceptance
  bar: points-to derived-edge output stays byte-identical after the seam change.
- **`oracle-rta` scoring already filters to reachable-from-roots edges (Phase 43 D-17).** Get
  the reachable seed/marking wiring right or the suite's recall silently misreads — Phase 43
  warned that getting RTA-vs-Jelly scoring filtering backwards tanks a suite.
- **The polyglot Go+TS canary does not exist yet (D-16).** It must be created in this phase; it
  is *added* here and *promoted to a hard gate* in Phase 54 (BENCH-01). Don't assume a fixture
  is already present.
- **Adding/altering a provider slot is a known snapshot chore** (memory
  `polint-kernel-provider-snapshot-sites`): if the provider wiring touches order, expect ~7
  provider-order snapshot assertions and run the full `cargo test -p polint`. (Phase 48 likely
  reuses the existing `polint.solver` slot rather than adding one, but the frontend-fact
  provider wiring may still touch ordering.)

</specifics>

<deferred>
## Deferred Ideas

- **JS/TS function-token propagation driver** (`solver::ts_tokens`) — **JS-04, Phase 49**
  (parallel-eligible with Phase 48). Phase 48 leaves `TsTokensPolicy` a stub.
- **JS/TS object/property/prototype/`this` model & driver** — **JS-05, Phase 50**.
- **Adaptation-model layer producing `ModelEdge` constraints** — **ADAPT-01/02, Phase 51**.
- **`refined_calls::provider` rework to project over solver output** (preserving
  `RefinedCallEdgeFact`) — **GRAPH-05, Phase 52**. Phase 48 only emits RTA edges into the
  provider slot.
- **Unknown-taxonomy consolidation + `polint inspect unknowns --format json`** (the only new
  public CLI surface in v1.3) — **TAX-01, Phase 52**. Phase 48's `BudgetExceeded` signals feed
  it; the taxonomy is built later.
- **Go VTA / type-flow refinement above RTA** — **PREC-FUT-01** (out of v1.3).
- **Hard benchmark promotion gate** (per-suite precision floors Go ≥60%, F-score β=0.5,
  per-language deltas, the canary as a *hard* gate, the final public-API leak CI gate) —
  **BENCH-01, Phase 54**. Phase 48 adds the canary + demonstrates the recall lift; Phase 54
  enforces the floors.
- **Cross-family cache + solver-budget consolidation sweep** — **CACHE-01/02, Phase 53**.
- **Public SDK promotion of any solver/call-graph view** — out of v1.3 (SDK-FUT-01 requires
  two-milestone benchmark stability). All `go_rta` types stay `pub(crate)`.

### Reviewed Todos (not folded)

None — `todo.match-phase 48` returned 0 matches.

</deferred>

---

*Phase: 48-Go RTA Driver*
*Context gathered: 2026-06-02*
