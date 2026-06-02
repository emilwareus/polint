# Phase 47: Unified Solver Core & Derived-Edge Provenance - Context

**Gathered:** 2026-06-02
**Status:** Ready for planning
**Mode:** `/gsd:discuss-phase 47 --auto`

<domain>
## Phase Boundary

Phase 47 builds **the heart of v1.3**: a single private deterministic `analysis::solver`
that consumes the Phase 44 constraint vocabulary (`semantic_graph::constraints::ConstraintKind`)
and derives edges, with explicit budgets, per-language policy scaffolding, and full provenance
on every derived edge. It delivers exactly **two** requirements:

1. **GRAPH-03 — unified solver core.** A private `analysis::solver` with a deterministic
   `VecDeque` worklist, explicit `SolverBudget` / `BudgetStatus`, and per-language `SolverPolicy`
   trait scaffolding; v1.2's `points_to::solver` is **folded in as a sub-domain** (the first
   `SolverPolicy` implementation), preserving its existing fixpoint behavior byte-identically.
2. **GRAPH-04 — derived-edge provenance.** Every solver-derived edge carries a
   `DerivedEdgeProvenance` (contributing fact IDs totally ordered by stable ID, constraint kind,
   solver step) consumable by `polint explain`; a property test asserts that deleting any
   contributing fact invalidates the derived edge.

This phase is **the solver core + provenance only**. It explicitly does **NOT**:

- Rework `refined_calls::provider` to project over solver output — that is **GRAPH-05, Phase 52**
  (confirmed against `.planning/REQUIREMENTS.md` line 119; do NOT pull it into Phase 47 scope).
  Phase 47 only emits solver output into a provider slot; Phase 52 reads it.
- Implement the Go RTA driver (`solver::go_rta`) — **Phase 48 (GO-05)**. Phase 47 ships only the
  `SolverPolicy` trait scaffolding; the Go policy is an honest stub until Phase 48.
- Implement the JS/TS function-token propagation driver (`solver::ts_tokens`) — **Phase 49 (JS-04)**.
  Same: TS policy is an honest stub until Phase 49.
- Build the unknown taxonomy consolidation or the `polint inspect unknowns` CLI — **Phase 52 (TAX-01)**.

No new public SDK type is promoted (v1.3 discipline; the Phase 42 public-surface-leak gate still
applies). The Phase 43 determinism gate (REACH-03, driven by `provider_manifests()`) is **inherited**
and MUST stay green once the solver provider registers — 10 seeded provider-order shuffles →
byte-identical normalized observed JSON.

</domain>

<decisions>
## Implementation Decisions

### Solver Core & Folding `points_to::solver` In (GRAPH-03)

- **D-01:** Add a new private module `analysis::solver` (`crates/polint/src/analysis/solver/`).
  Every type, enum, fact, index, and function is `pub(crate)`. Register it in `analysis/mod.rs`
  alongside the existing private analysis modules. This is the "single shared solver core" the
  research doc prescribes; language-specific drivers (`go_rta`, `ts_tokens`) live as sub-modules /
  `SolverPolicy` impls under it in Phases 48/49.
- **D-02:** The solver core owns a **deterministic `VecDeque` worklist** following the exact pattern
  already proven in `points_to::solver` (`crates/polint/src/analysis/points_to/solver.rs` — see the
  `queue: VecDeque<(PtVarId, BTreeSet<ObjectTokenId>)>` worklist + `BTreeMap`/`BTreeSet`
  accumulation). Determinism comes from BTree-ordered accumulation + dense IDs assigned only after
  stable-key sort (the v1.2 rule inherited from Phase 43/44). The core's worklist drives a
  single fixpoint per run.
- **D-03:** **Fold `points_to::solver` in as the FIRST sub-domain, by composition not rewrite
  (MANDATORY).** The existing points-to fixpoint becomes the first `SolverPolicy` implementation
  inside the unified core. Preserve its current behavior: existing points-to snapshot + determinism
  fixtures MUST stay byte-identical (no recall/precision regression on the points-to sub-domain).
  The planner decides whether to physically relocate the fixpoint engine into `solver/` or have the
  core invoke the existing engine as a registered sub-domain — either is acceptable provided
  (a) the unified core owns the worklist/budget/policy abstraction and (b) the points-to fixtures do
  not change byte-for-byte. The points-to **constraint vocabulary** (`points_to::facts`,
  `::constraints`, `::vars`) stays where it is; only the solving engine participates in the fold.
- **D-04:** **Naming-collision guard (mirrors Phase 44 D-09, Phase 43 D-02).** Add a top-of-module
  doc comment in `solver/` distinguishing: the unified `analysis::solver` core (consumes the
  GRAPH-02 `ConstraintKind` vocabulary, emits derived edges with provenance) vs. the
  `points_to` sub-domain's internal `PointsToConstraintKind`/`PtVarId` language. The unified core
  sits *above* the points-to sub-domain. Do not conflate them.

### Budget Model & `SolverPolicy` Scaffolding (GRAPH-03)

- **D-05:** Introduce unified `SolverBudget` and `BudgetStatus` types at the solver core,
  generalizing the existing `PointsToBudget` (`max_steps`, `max_objects_per_var`,
  `max_dynamic_vars`) and `PointsToBudgetStatus` (`WithinBudget` / `BudgetExceeded`). The unified
  budget carries the cross-domain knobs (e.g. `max_steps`, bounded outer-iteration cap) plus a
  channel for per-sub-domain knobs. `PointsToBudget`/`PointsToBudgetStatus` become a sub-domain
  projection/mapping of the unified types — the planner picks whether to alias or wrap, provided
  points-to fixtures stay byte-identical.
- **D-06:** **Budget exhaustion surfaces as facts, never silent drops (honest precision, Phase 44
  discipline).** Mirror the existing `PointsToStatus::BudgetExceeded` / `PointsToBudgetStatus::BudgetExceeded`:
  when the solver hits a budget ceiling it emits an explicit `BudgetExceeded` signal consumable
  downstream (the Phase 52 unknown taxonomy will categorize it), rather than dropping precision
  silently. Precision ceiling holds: derived edges reject `FactPrecision::Exact`.
- **D-07:** **`SolverPolicy` trait scaffolding only — Go/TS policies are honest stubs.** Phase 47
  defines the `SolverPolicy` trait and ships exactly ONE real implementation (the points-to
  sub-domain, D-03). The per-language Go (`go_rta`) and TS (`ts_tokens`) policies are
  scaffolding/stubs reserved for Phases 48/49 — documented as honest emptiness exactly like Phase 44
  reserved the `ModelEdge` variant with zero producers. Do NOT fake a Go/TS driver here.

### Derived-Edge Provenance (GRAPH-04)

- **D-08:** Define a `pub(crate) struct DerivedEdgeProvenance` carried on every solver-derived edge,
  with three fields per the roadmap: (1) **contributing fact IDs totally ordered by stable ID**
  (the v1.2 total-order rule — sort by stable key, mirroring Phase 42 dedup / Phase 44 dense-ID
  discipline), (2) the **constraint kind** that produced the edge (reuse the existing
  `ConstraintKind` discriminant / `kind_str()` from `semantic_graph::constraints`), and (3) the
  **solver step** (monotonic `u64` worklist step counter). The exact fact-ID newtype is a planner
  decision, provided it references existing stable identities (composition over duplication).
- **D-09:** **Invalidation property test (GRAPH-04 acceptance).** Add a property test asserting that
  **deleting any single contributing fact invalidates the derived edge** — re-running the solver
  without that fact must not produce the same derived edge. This proves provenance is sound and
  load-bearing, not decorative.
- **D-10:** **`polint explain` consumption.** `DerivedEdgeProvenance` is consumable by the existing
  `polint explain` command surface (`crates/polint/src/cli/mod.rs` → `explain(...)`). Phase 47 wires
  explain to surface, for a derived edge, its contributing facts + constraint kind + solver step.
  This is NOT a new public CLI surface (the only new public CLI surface in v1.3 is `polint inspect
  unknowns`, Phase 52) — it extends the existing private/explain plumbing. All provenance types
  stay `pub(crate)`.

### Dependency Contract & Cycle Detection (GRAPH-03 Success Criterion 4)

- **D-11:** Document the solver's dependency contract explicitly as a module-level doc comment:
  **closed input set** (the solver consumes a fixed snapshot of upstream facts/constraints and never
  re-reads mutated state mid-run), **single fixpoint per run** (one worklist drain to convergence),
  and **bounded outer iterations** (an explicit cap enforced via `SolverBudget`, surfaced as
  `BudgetStatus`/`BudgetExceeded` when hit — never an unbounded loop).
- **D-12:** **Cycle-detection fixture.** Add a fixture proving **no solver↔summary loop is admitted**:
  function/procedure summaries are an *input* to the solver, never re-fed into the same fixpoint as
  they are produced. The fixture demonstrates that a constraint set which would create a
  solver→summary→solver cycle is detected/rejected (or bounded) rather than diverging. This is the
  concrete mechanism behind "closed input set / single-fixpoint-per-run."

### Provider Wiring, Determinism Gate & Leak Gate (GRAPH-03 SC2/SC5)

- **D-13:** **Register a private `polint.solver` provider** in the kernel manifest
  (`analysis_kernel::provider`), slotted **after `polint.semantic_graph` and before
  `polint.refined_calls`** — the exact slot Phase 44 reserved (D-16 of 44-CONTEXT) for "where
  Phase 47's solver/graph output is read." The provider consumes the `polint.semantic_graph`
  constraint vocabulary (+ the upstream fact families the points-to sub-domain needs) and emits
  derived-edge + `DerivedEdgeProvenance` facts. Update `provider_order_for_test()` and all ordering
  assertions to include `polint.solver`. The planner confirms the slot against the dependency DAG.
  **Note (memory):** adding a provider touches ~7 provider-order snapshot assertions — run the full
  `cargo test -p polint` and update every snapshot site.
- **D-14:** **Determinism gate inheritance (Phase 43 D-22/D-25, Phase 44 D-18).** The `polint.solver`
  provider auto-enrolls in the Phase 43 determinism harness because that harness is driven by
  `provider_manifests()`. Phase 47 verification MUST keep the gate green: 10 seeded provider-order
  shuffles → byte-identical normalized observed JSON, as a named acceptance criterion. No per-phase
  edit to the gate harness is needed; the solver's BTree accumulation + sort-then-assign-dense-IDs
  discipline (D-02) is what makes this hold.
- **D-15:** **Cache key.** The `polint.solver` provider's cache key digests the output digests of
  every provider it consumes (at minimum `polint.semantic_graph` plus the points-to source families)
  **and** the provider/schema version **and the solver budgets** — following the established v1.2
  digest recipe (Phase 43 D-19 / Phase 44 D-17). Budgets participate so a budget change invalidates
  downstream (forward-compatible with CACHE-01/02, Phase 53).
- **D-16:** **Public-surface-leak gate stays green (Phase 42).** All new `solver` types
  (`SolverBudget`, `BudgetStatus`, `SolverPolicy`, `DerivedEdgeProvenance`, derived-edge facts, the
  provider) stay `pub(crate)`. Do NOT extend `ALLOWED_PRELUDE` in
  `crates/polint/tests/public_surface_leak.rs`. This is a named acceptance criterion (GRAPH-03 SC5).

### Claude's Discretion

- Internal file layout of `analysis::solver/` (e.g. `mod.rs`, `core.rs`/`engine.rs`, `budget.rs`,
  `policy.rs`, `provenance.rs`, `facts.rs`, `provider.rs`, `cache_key.rs`, `validate.rs`,
  `points_to_domain.rs`) is the planner's choice, provided visibility stays `pub(crate)` and digest
  discipline matches `analysis::semantic_graph` / `analysis::points_to`.
- Whether to physically relocate the points-to fixpoint engine into `solver/` or invoke it as a
  registered sub-domain in place (D-03) — planner picks the cleaner option, provided points-to
  fixtures stay byte-identical and the unified core owns the worklist/budget/policy abstraction.
- Exact field shapes/newtypes of `SolverBudget`, `BudgetStatus`, `SolverPolicy`, and
  `DerivedEdgeProvenance` (D-05, D-07, D-08) — planner/researcher decide, provided: the budget
  generalizes the existing points-to knobs, the policy trait ships exactly one real impl, and
  provenance carries the three roadmap-named fields with contributing facts total-ordered by stable ID.
- Whether to alias or wrap `PointsToBudget`/`PointsToBudgetStatus` as a sub-domain projection (D-05).
- Exact `polint.solver` provider slot (D-13) — planner confirms against the DAG; the
  after-`semantic_graph`/before-`refined_calls` recommendation is the default unless the DAG dictates
  otherwise.
- Natural plan slicing (planner confirms): (1) `analysis::solver` core — `VecDeque` worklist +
  `SolverBudget`/`BudgetStatus` + `SolverPolicy` trait + folding points-to in as the first sub-domain;
  (2) `DerivedEdgeProvenance` on derived edges + `polint explain` consumption + deletion property test;
  (3) provider/cache-key wiring + dependency-contract doc + cycle-detection fixture + determinism-gate
  inheritance + public-surface-leak proof.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 47 goal + 5 Success Criteria (the precise acceptance contract),
  GRAPH-03/GRAPH-04 mapping, "heart of v1.3" framing, and the downstream Phases 48/49/52 that consume
  the solver core. Confirms Phase 48 (Go RTA) and Phase 49 (TS tokens) are parallel-eligible after 47.
- `.planning/REQUIREMENTS.md` — GRAPH-03 (line 27) + GRAPH-04 (line 28) requirement text;
  **GRAPH-05 is line 29 / Phase 52 (line 119) — explicitly OUT of Phase 47 scope.** Phase→requirement
  map line 148 (Phase 47 = GRAPH-03, GRAPH-04, 2 reqs).
- `.planning/PROJECT.md` — Product boundary, private-analysis-first discipline, public-API discipline
  carried into v1.3, benchmark baselines, the "single shared semantic graph / solver core" keystone.
- `.planning/STATE.md` — Current v1.3 state (Phase 46 complete), open repo-admin action T-42-04-10
  (leak-gate branch protection).

### Immediate Upstream Phase Context (read first)

- `.planning/phases/44-semantic-graph-skeleton-constraint-vocabulary/44-CONTEXT.md` — **Primary
  upstream.** Defines the `ConstraintKind` vocabulary the solver consumes, the
  `ConstraintKind` ↔ `PointsToConstraintKind` separation (D-09), the reserved provider slot for
  Phase 47 (D-16: after `type_value_alias`/`semantic_graph`, before `refined_calls`), the closed-enum
  byte-stability + composition + dense-ID-after-sort + provider-digest + determinism-gate discipline
  Phase 47 inherits wholesale, and the deferred items that name Phase 47's work.
- `.planning/phases/43-reachability-roots-per-suite-scoring-mode/43-CONTEXT.md` — The **determinism
  gate** (REACH-03) Phase 47 inherits, the dense-ID-after-sort rule, provider digest recipe (D-19),
  and the naming-collision-guard pattern (D-02).
- `.planning/phases/42-benchmark-identity-renderers-dedup-identity-taxonomy/42-CONTEXT.md` —
  `IdentityCategory` closed-enum `#[repr(u8)]` byte-stability template, dedup total-order key
  (the rule provenance contributing-fact ordering follows), cross-platform byte-identical contract,
  and the public-surface-leak gate.

### v1.3 Graph Engine Benchmark Research

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` — **Primary architecture source.**
  "Architectural Recommendation" (shared code representation + solver core owning stable identities,
  constraints, provenance/precision/taxonomy), the "frontends add constraints, solvers derive edges"
  framing, and the budget/precision-first rules GRAPH-03/04 implement.
- `research/call-graphs/FINAL-REPORT.md` — Layered call-graph + constraint-based solving framing.
- `research/evaluation-harness/STANDARD.md` — Determinism requirements inherited from the Phase 43 gate.
- `research/evaluation-harness/decisions/decision-log.md` — Accumulated benchmark architecture decisions.

### Existing Implementation Touch Points

- `crates/polint/src/analysis/points_to/solver.rs` — **The engine Phase 47 folds in (D-03).** Existing
  `PointsToBudget` (`max_steps`/`max_objects_per_var`/`max_dynamic_vars`), `PointsToSolveResult`,
  the `VecDeque` worklist + `BTreeMap`/`BTreeSet` fixpoint, `step_budget_ok`, and the determinism
  test at the bottom. The unified `SolverBudget`/`BudgetStatus` (D-05) generalize these.
- `crates/polint/src/analysis/points_to/facts.rs` — `PointsToBudgetStatus` (`WithinBudget`/`BudgetExceeded`),
  `PointsToStatus::BudgetExceeded` — the honest budget-exhaustion-as-fact pattern (D-06).
- `crates/polint/src/analysis/points_to/{constraints.rs,vars.rs,store.rs}` — the points-to sub-domain
  constraint vocabulary/vars/store that stay in place while the engine folds in.
- `crates/polint/src/analysis/semantic_graph/constraints.rs` — `ConstraintKind` (`CopyEdge`/`Alloc`/
  `FieldLoad`/`FieldStore`/`CallConstraint`/`ModelEdge`/`TypeConstraint`), its `kind_str()` and
  node-reference accessors. **This is the solver's input vocabulary.** Note the documented
  `ConstraintKind` ↔ `PointsToConstraintKind` conceptual map at the top of the file.
- `crates/polint/src/analysis/semantic_graph/{facts.rs,store.rs,provider.rs,cache_key.rs,validate.rs}` —
  `NodeKind`/`EdgeKind`/`SemanticNodeId`, the `SemanticGraphStore` + deterministic indexes the solver
  reads, and the provider/cache-key/validate patterns to mirror for `polint.solver`.
- `crates/polint/src/analysis/mod.rs` — register `pub(crate) mod solver;`.
- `crates/polint/src/analysis/ids.rs` — run-local dense ID newtypes (`pub(crate) struct XId(pub(crate) u64)`);
  add any solver/derived-edge/provenance IDs here.
- `crates/polint/src/analysis_kernel/provider.rs` — provider manifest + `provider_order_for_test()`
  (~lines 250–940 + 999–1366); add `polint.solver` after `polint.semantic_graph` (line 658) and before
  `polint.refined_calls` (line 689); update ALL ordering/snapshot assertions (~7 sites — see memory
  `polint-kernel-provider-snapshot-sites`). Also the `provider_manifests()` machinery that auto-enrolls
  the provider in the Phase 43 determinism gate (D-14).
- `crates/polint/src/analysis/refined_calls/facts.rs` — `RefinedCallEdgeFact` (the v1.2 contract Phase 52
  preserves when it reworks refined_calls to project over solver output). **Read for awareness only —
  do NOT rework refined_calls in Phase 47.**
- `crates/polint/src/cli/mod.rs` — the `explain(...)` command entry (`Command::…Explain → explain(...)`);
  the consumer surface for `DerivedEdgeProvenance` (D-10). Extend the existing private plumbing; add no
  new public CLI surface.
- `crates/polint/src/analysis/{validate.rs,stable_key.rs,store.rs}` — validation-pass pattern (D-12),
  length-prefixed labeled-parts stable-key recipe (provenance total-order, D-08), store conventions.
- `crates/polint/tests/public_surface_leak.rs` — v1.3 leak gate (Phase 42). All new `solver` types stay
  `pub(crate)`; do NOT extend `ALLOWED_PRELUDE` (D-16).
- `tests/eval-fixtures/{semantic-graph/,determinism/,identity/}` — native fixture-tree precedents;
  cycle-detection fixture (D-12) and provenance/determinism fixtures live alongside these.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::points_to::solver` already implements a **deterministic `VecDeque` worklist + budget +
  fixpoint** with a determinism unit test — this is both the engine to fold in (D-03) and the proven
  template for the unified core's worklist (D-02). The hard determinism work is already done; Phase 47
  generalizes and lifts it, it does not invent it from scratch.
- `PointsToBudget` / `PointsToBudgetStatus` / `PointsToStatus::BudgetExceeded` are the exact shapes
  `SolverBudget` / `BudgetStatus` generalize, including the honest "budget exhaustion as a fact"
  pattern (D-05, D-06).
- `semantic_graph::constraints::ConstraintKind` (Phase 44) is the **solver's input vocabulary**, with
  `kind_str()` and node-reference accessors already present — and a documented conceptual map to
  `PointsToConstraintKind` that tells the planner how points-to constraints relate to the unified
  vocabulary.
- `semantic_graph::store::SemanticGraphStore` already provides deterministic, byte-stable indexes
  (nodes-by-kind, edges-by-kind, adjacency, constraints-by-kind) the solver consumes.
- `analysis_kernel::provider::{provider_order_for_test, provider_manifests}` enumerate providers
  deterministically and auto-enroll new providers in the Phase 43 determinism shuffle (D-13, D-14).
- `analysis_kernel::stable_key_from_parts` / `analysis::stable_key` — length-prefixed labeled-parts
  recipe reused for derived-edge stable keys and provenance total-ordering (D-08).
- `polint explain` command (`cli/mod.rs`) — existing surface to extend for provenance consumption (D-10).

### Established Patterns

- **Closed-enum byte-stability** (Phase 42/43/44): pinned source order + `#[repr(u8)]` so serde + `Ord`
  are declaration-driven. Any new solver enum (`BudgetStatus`, etc.) follows this.
- **Composition over rewrite** (Phase 42/43/44): the solver references upstream facts/constraints by
  stable identity and folds the points-to engine in without rewriting its observable behavior (D-03).
- **Dense IDs after sort** (v1.2/Phase 43/44): dense run-local IDs assigned only after stable-key sort —
  the mechanism behind solver determinism (D-02, D-14).
- **Provider digest participation** (every v1.2 provider): `polint.solver` digests source + budgets +
  upstream provider output digests (D-15).
- **Honest status/precision** (Phase 44): budget exhaustion → explicit `BudgetExceeded` fact, never a
  silent drop; derived edges reject `FactPrecision::Exact`; Go/TS policies reserved-but-stubbed, not
  faked (D-06, D-07).
- **Naming-collision guard via top-of-module doc comment** (Phase 43 D-02, Phase 44 D-09): distinguish
  the unified solver core from the points-to sub-domain (D-04).
- **Determinism gate auto-enrollment** (Phase 43/44 D-18): provider auto-enrolls via
  `provider_manifests()`; 10-shuffle byte-identical observed JSON is the named acceptance criterion (D-14).

### Integration Points

- `analysis/mod.rs` gains `pub(crate) mod solver;`.
- `analysis_kernel` provider manifest + `provider_order_for_test()` gain `polint.solver` after
  `polint.semantic_graph`, before `polint.refined_calls`; ~7 ordering/snapshot assertions updated.
- `analysis::ids` gains solver / derived-edge / provenance IDs.
- The solver provider consumes `polint.semantic_graph` (+ points-to source families); emits derived-edge
  + provenance facts; participates in the cache key (D-15) and the Phase 43 determinism gate (D-14).
- `cli/mod.rs` `explain(...)` extended to surface `DerivedEdgeProvenance` (no new public surface).
- `crates/polint/tests/public_surface_leak.rs` stays green with all new types `pub(crate)`.
- Cycle-detection + provenance-deletion fixtures live under `tests/eval-fixtures/` / unit + property tests.

</code_context>

<specifics>
## Specific Ideas

- **Scope discipline is the single most important guardrail this phase.** The ROADMAP narrative and
  Phase 44's deferred section both informally describe GRAPH-05 (refined_calls rework) as "Phase 47,"
  but the authoritative `.planning/REQUIREMENTS.md` maps GRAPH-05 → **Phase 52**. Phase 47 = GRAPH-03 +
  GRAPH-04 only. Do NOT rework `refined_calls::provider`; only emit solver output into the reserved
  provider slot so Phase 52 can read it. Likewise the Go RTA driver (Phase 48) and TS token driver
  (Phase 49) are out — Phase 47 ships only `SolverPolicy` scaffolding + the points-to sub-domain impl.
- **Folding points-to is composition, not a rewrite.** The acceptance bar is that existing points-to
  snapshot + determinism fixtures stay byte-identical after the fold (D-03). If a points-to fixture
  changes, the fold was done wrong.
- **Provenance must be load-bearing, not decorative.** The deletion property test (D-09) is the proof:
  remove any contributing fact → the derived edge must not survive. Contributing facts are totally
  ordered by stable ID (the Phase 42 dedup total-order rule), so provenance is itself byte-stable.
- **The provider slot was pre-reserved.** Phase 44 D-16 deliberately placed `polint.semantic_graph`
  before `polint.refined_calls` "exactly where Phase 47's solver output is read." Use that slot for
  `polint.solver`; it is forward-compatible with Phase 52's GRAPH-05 rework.
- **Adding a provider is a known snapshot-update chore.** Memory `polint-kernel-provider-snapshot-sites`:
  expect ~7 provider-order snapshot assertions; run the full `cargo test -p polint`.

</specifics>

<deferred>
## Deferred Ideas

- **`refined_calls::provider` rework to project over solver output (preserving `RefinedCallEdgeFact`
  for `data_flow`/`evidence`/SDK)** — **GRAPH-05, Phase 52.** Phase 47's provider slot is forward-
  compatible; Phase 47 does not touch refined_calls.
- **Go RTA driver** (`solver::go_rta`: reachability fixpoint from roots, address-taken tracking, dynamic
  dispatch by signature, interface invoke by method-set, `solver_config.go.*` knobs) — **GO-05, Phase 48.**
  Phase 47 ships only the `SolverPolicy` Go stub.
- **JS/TS function-token propagation driver** (`solver::ts_tokens`: token propagation through copy/
  call/return constraints, per-variable token cap, `"too-many-tokens"` sentinel, `solver_config.js.*`
  knobs, `BitSet`/`RoaringBitmap`) — **JS-04, Phase 49.** Phase 47 ships only the `SolverPolicy` TS stub.
- **JS/TS object/property/prototype/`this` model & driver** — **JS-05, Phase 50.**
- **Adaptation model layer producing `ModelEdge` constraints** — **ADAPT-01/02, Phase 51.** Still no
  `ModelEdge` producer in Phase 47 (reserved-but-empty since Phase 44).
- **Unknown-taxonomy consolidation + `polint inspect unknowns --format json`** (the only new public CLI
  surface in v1.3) — **TAX-01, Phase 52.** Phase 47's `BudgetExceeded` facts feed it but the taxonomy
  is built later.
- **Cache-key participation for sidecar binary digest / Go toolchain version / adaptation model files /
  solver budgets across all fact families, with must-invalidate + must-preserve-hit fixtures** —
  **CACHE-01/02, Phase 53.** Phase 47's cache key digests solver budgets but the cross-family
  consolidation sweep is later.
- **Public SDK promotion of any solver view** — explicitly out of v1.3 per ROADMAP (SDK-FUT-01 requires
  two-milestone benchmark stability). All solver types stay `pub(crate)`.

### Reviewed Todos (not folded)

None — `todo.match-phase 47` returned 0 matches.

</deferred>

---

*Phase: 47-Unified Solver Core & Derived-Edge Provenance*
*Context gathered: 2026-06-02*
