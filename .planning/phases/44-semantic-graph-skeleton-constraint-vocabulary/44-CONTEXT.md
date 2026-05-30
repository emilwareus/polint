# Phase 44: Semantic Graph Skeleton & Constraint Vocabulary - Context

**Gathered:** 2026-05-30
**Status:** Ready for planning
**Mode:** `/gsd:discuss-phase 44 --auto`

<domain>
## Phase Boundary

Phase 44 introduces the **architectural keystone** of v1.3: a single private `analysis::semantic_graph` that v1.2's isolated fact families (functions, callsites, scopes, places, abstract objects, modules, packages, calls, points-to, types) will eventually feed into as a **unified graph**, plus the **constraint vocabulary** that language frontends emit into it. It delivers exactly two requirements:

1. **GRAPH-01 — the graph skeleton.** A private `analysis::semantic_graph` module with typed `NodeKind` (function, callsite, scope, place, abstract object, module, package) and `EdgeKind` (call, member-of, alloc, flow), node/edge indexes, validation, a provider manifest entry, and a participating cache key. Every type is `pub(crate)`.
2. **GRAPH-02 — the constraint vocabulary.** A closed constraint enum (`CopyEdge`, `Alloc`, `FieldLoad`, `FieldStore`, `CallConstraint`, `ModelEdge`, `TypeConstraint`) that language frontends emit into the semantic graph, with constraint emission verified by **snapshot fixtures**.

This phase is **skeleton + vocabulary only**. It does **not** build the unified solver, `VecDeque` worklist, `SolverBudget`, or `SolverPolicy` trait (Phase 47, GRAPH-03), does **not** add `DerivedEdgeProvenance` to solver edges (Phase 47, GRAPH-04), does **not** rework `refined_calls::provider` to project over solver output (Phase 47, GRAPH-05), does **not** fold in `points_to::solver` as a sub-domain (Phase 47, GRAPH-03), and does **not** add the JS/TS inventory/scope/module work (Phase 45) or the Go sidecar (Phase 46) that will become the richest *producers* of nodes and constraints. Phase 44 establishes the **shared data structure and vocabulary** that all those later phases write into and read from. No new public SDK type is promoted (v1.3 discipline; the Phase 42 public-surface-leak gate still applies). The Phase 43 determinism gate (REACH-03, driven by `provider_manifests()`) is **inherited** and MUST stay green once the `polint.semantic_graph` provider registers.

</domain>

<decisions>
## Implementation Decisions

### Semantic Graph Module, Node/Edge Model & Identity (GRAPH-01)

- **D-01:** Add a new private module `analysis::semantic_graph` (`crates/polint/src/analysis/semantic_graph/`). Every type, enum, fact, index, and function is `pub(crate)`. Register it in `analysis/mod.rs` alongside the existing private analysis modules. This is the shared core the research doc calls the "single shared semantic graph / solver core" — its scope is deliberately language-agnostic; language-specific producers live in `src/go/`, `src/ts/`, and the per-language frontends of Phases 45/46.
- **D-02:** `NodeKind` is a **closed enum** (no `Other`/`Unknown`, no `#[non_exhaustive]`, pinned source order with explicit `#[repr(u8)]` ordinals — mirror the Phase 42 `IdentityCategory` / Phase 43 `RootKind` discipline so serde + `Ord` byte-stability is declaration-driven) with exactly the seven roadmap-named variants: `Function`, `Callsite`, `Scope`, `Place`, `AbstractObject`, `Module`, `Package`.
- **D-03:** `EdgeKind` is a **closed enum** with the four roadmap-named variants: `Call`, `MemberOf`, `Alloc`, `Flow`. Same `#[repr(u8)]` + pinned-order + byte-stability discipline as `NodeKind`.
- **D-04:** **Composition over duplication (MANDATORY, mirrors Phase 43 D-03).** A graph node **references** an existing v1.2 identity by ID rather than copying its data. Each `NodeKind` variant carries the corresponding existing newtype: `Function(FunctionId)` (`core::FunctionId`), `Callsite(CallSiteId)` (`analysis::ids::CallSiteId`), `Scope(ScopeId)` (`symbol_graph::semantic::ScopeId`), `Place(PlaceId)` (`analysis::ids::PlaceId`), `AbstractObject(ObjectTokenId)` (`analysis::ids::ObjectTokenId`), `Module(ModuleId)`, `Package(PackageId)` (`core::PackageId`). The planner confirms the exact module-node ID type from the existing module-graph facts. Do **not** invent parallel function/callsite/place identities.
- **D-05:** Introduce a run-local dense `SemanticNodeId` (and `SemanticEdgeId`) newtype in `analysis::ids` following the existing `pub(crate) struct XId(pub(crate) u64)` pattern. Dense IDs are assigned **only after sorting by stable key** (the v1.2 determinism rule, inherited from Phase 43 D-06). Do not invent a parallel ID scheme.
- **D-06:** Every node and edge carries a `stable_key: String` built with the **existing length-prefixed labeled-parts recipe** (`analysis_kernel::stable_key_from_parts` / `analysis::stable_key`), composed from `(node kind ordinal, referenced existing stable identity)` for nodes and `(edge kind ordinal, source node stable key, target node stable key)` for edges — **never** run-local IDs. This keeps the graph byte-stable across provider-order shuffles (the Phase 43 determinism gate will shuffle it).
- **D-07:** Carry the v1.2 status/precision/provenance vocabulary shape on nodes/edges where meaningful (e.g. a `precision` field rejecting `FactPrecision::Exact` for derived/heuristic edges, matching the reachability/identity precision-ceiling discipline). The planner picks whether nodes need full status/precision or only edges do; honest labels over fabricated certainty (no invented edges to inflate recall — the research doc's first-class precision rule).

### Constraint Vocabulary Design (GRAPH-02)

- **D-08:** Define the constraint vocabulary as a **closed enum** `ConstraintKind` (or `SemanticConstraintKind`) with exactly the seven roadmap-named variants: `CopyEdge`, `Alloc`, `FieldLoad`, `FieldStore`, `CallConstraint`, `ModelEdge`, `TypeConstraint`. Same `#[repr(u8)]` + pinned-order + byte-stability discipline (D-02). Each variant's payload references **semantic-graph node IDs / existing fact IDs** (e.g. `CopyEdge { dst: SemanticNodeId, src: SemanticNodeId }`, `CallConstraint` referencing a callsite node, `ModelEdge` referencing an adaptation model fact id, `TypeConstraint` referencing a type fact). The exact field shapes are a planner/researcher decision; the **variant set is fixed by the roadmap**.
- **D-09:** **Relationship to the existing `points_to::PointsToConstraintKind` (CRITICAL — naming/concept guard, mirrors Phase 43 D-02).** `analysis::points_to::facts::PointsToConstraintKind` (`AddressOf`, `Copy`, `CallReturn`, …) already exists as the **points-to sub-domain's** internal constraint language. The new GRAPH-02 `ConstraintKind` is the **unified, language-frontend-facing graph vocabulary** that sits *above* it. Phase 44 does **not** merge, rename, or delete `points_to::PointsToConstraintKind` — that folding is explicitly Phase 47 (GRAPH-03: "folds v1.2's `points_to::solver` in as a sub-domain"). Add a top-of-module doc comment in `semantic_graph/` distinguishing the two: GRAPH-02 = unified vocabulary emitted by frontends; `points_to` = one solver sub-domain's internal constraints. Do not conflate them. The planner may note the conceptual mapping (e.g. `CopyEdge` ↔ points-to `Copy`, `Alloc` ↔ points-to `AddressOf`) for Phase 47, but no code coupling is introduced now.
- **D-10:** Constraints are stored as a typed **fact family** (`ConstraintFact { id, kind, status, precision, stable_key }`) following the exact shape of `points_to::PointsToConstraintFact` — so the graph's constraint store is indexable, snapshot-serializable, and digest-participating like every other v1.2 fact family.

### Population Strategy & Snapshot Fixtures (GRAPH-01 + GRAPH-02)

- **D-11:** **Phase 44 emits a real-but-minimal graph from facts that already exist pre-Phase-45/46 — it is not a pure type-only stub.** GRAPH-02's "constraint emission is verified by snapshot fixtures" requires actual emission. The planner populates the skeleton from the **already-available** v1.2 fact families: functions/callsites/scopes/places/modules/packages → nodes; direct call edges (`analysis::calls`) → `Call` edges + `CallConstraint`; points-to/value facts (`analysis::points_to`, `analysis::values`) → `Alloc`/`CopyEdge`/`FieldLoad`/`FieldStore` constraints where the existing facts already express them; `member-of` edges from scope/place/object containment. `ModelEdge` has **no producer yet** (adaptation arrives Phase 49, ADAPT-01) — reserve the variant and emit zero `ModelEdge` constraints in Phase 44, documented explicitly (honest emptiness, not a placeholder). This proves the vocabulary end-to-end on a small input without waiting for the richer Phase 45/46 frontends.
- **D-12:** **Snapshot fixtures (GRAPH-02 acceptance).** Add native snapshot fixtures under `tests/eval-fixtures/semantic-graph/` (mirroring the `tests/eval-fixtures/identity/` and `tests/eval-fixtures/determinism/` precedents) covering at minimum one Go case and one TS/JS case, each asserting byte-stable serialized nodes, edges, and emitted constraints. Snapshots are **normalized + total-ordered** (sort by stable key) so they are byte-identical cross-platform (Linux + macOS) and across provider-order shuffles — consistent with the Phase 42/43 cross-platform byte-identical contract.
- **D-13:** The graph is **derived/aggregated, never authoritative over its sources** (composition over mutation, Phase 42/43 discipline). Building the graph does **not** mutate `analysis::calls`, `analysis::points_to`, or any upstream fact family — it references them by stable identity. Upstream snapshot fixtures stay byte-stable.

### Indexes, Validation, Provider Manifest & Cache Key (GRAPH-01)

- **D-14:** **Indexes.** Build deterministic indexes required for later solver consumption: nodes-by-`NodeKind`, edges-by-`EdgeKind`, forward adjacency (source node → outgoing edges), and constraints-by-`ConstraintKind`. The exact index set/representation is a planner choice, but indexes are built **after** dense-ID assignment and are order-independent given inputs.
- **D-15:** **Validation.** Add a `validate()` pass (mirroring `analysis::validate` / `points_to` validation) asserting structural invariants: every edge endpoint resolves to an existing node, every constraint references resolvable node/fact IDs, no duplicate stable keys, dense IDs are contiguous and stable-key-sorted, and the precision ceiling holds. Validation failures surface as structured facts/diagnostics, never silent drops.
- **D-16:** **Provider manifest placement.** Register a `polint.semantic_graph` provider in the kernel manifest (`analysis_kernel::provider`). Recommended slot: **after `polint.type_value_alias` and before `polint.refined_calls`** in `provider_order_for_test()` — by that point all node/constraint source families (calls, identity, abstract domains, entrypoints, reachability, type/value/alias) are available, and it sits exactly where Phase 47's GRAPH-05 refined-calls rework will later read solver/graph output. The planner confirms the slot against the dependency DAG; the ordering test in `analysis_kernel/provider.rs` must be updated to include `polint.semantic_graph`.
- **D-17:** **Cache key.** The `polint.semantic_graph` provider's cache key digests source files plus the **output digests of every provider it consumes** (`polint.calls`, `polint.identity`, `polint.abstract_domains`, `polint.entrypoints`, `polint.reachability`, `polint.type_value_alias`, symbol/module-graph digests) **and** the provider/schema version — following the established v1.2 digest recipe (Phase 43 D-19) so cache invalidation behaves identically. Both must-invalidate and must-preserve-hit behavior should be covered (CACHE-01 discipline, even though CACHE-01 itself is a later phase).

### Determinism Gate Inheritance (REACH-03 obligation)

- **D-18:** **Inherited acceptance gate (Phase 43 D-22/D-25).** The `polint.semantic_graph` provider auto-enrolls in the Phase 43 determinism harness because that harness is driven by `provider_manifests()`. Phase 44's verification MUST keep the determinism gate green (10 seeded provider-order shuffles → byte-identical normalized observed JSON) as a named acceptance criterion. This is the concrete mechanism behind GRAPH's byte-stability requirement; no per-phase edit to the gate harness is needed.

### Claude's Discretion

- The internal file layout of `analysis::semantic_graph/` (e.g. `facts.rs`, `nodes.rs`, `edges.rs`, `constraints.rs`, `index.rs`, `provider.rs`, `cache_key.rs`, `validate.rs`, `store.rs`, `debug.rs`, `build.rs`) is the planner's choice, provided visibility stays `pub(crate)` and digest discipline matches `analysis::calls`/`analysis::points_to`.
- The exact payload field shapes of each `NodeKind`, `EdgeKind`, and `ConstraintKind` variant (D-04, D-08) — planner/researcher decide field names and which existing IDs each references, provided the variant *sets* match the roadmap exactly and identities compose existing facts rather than duplicating them.
- Whether nodes carry full status/precision/provenance or only edges/constraints do (D-07) — planner picks the cleaner option, provided honest precision labeling holds and `Exact` is rejected for derived edges.
- The precise index set and representation (D-14) — planner decides, provided indexes are deterministic and built after stable-key-sorted dense-ID assignment.
- The exact provider slot for `polint.semantic_graph` (D-16) — planner confirms against the dependency DAG; the after-`type_value_alias`/before-`refined_calls` recommendation is the default unless the DAG dictates otherwise.
- Natural plan slicing: (1) module + `NodeKind`/`EdgeKind` + node/edge facts + dense IDs + stable keys + indexes; (2) `ConstraintKind` vocabulary + constraint fact family + emission from existing fact families + `ModelEdge` reservation; (3) provider/cache-key wiring + validation + snapshot fixtures + determinism-gate inheritance + public-surface-leak proof.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 44 goal, GRAPH-01/GRAPH-02 mapping, v1.3 milestone framing, no-public-SDK-promotion rule, the shared-semantic-graph keystone framing, and the surrounding GRAPH-03/04/05 / GO / JS / ADAPT phases that consume this graph downstream.
- `.planning/REQUIREMENTS.md` — GRAPH-01/GRAPH-02 requirement text (lines 25–26) plus GRAPH-03/04/05 (the solver, provenance, and refined-calls rework that read this graph), JS-03 (frontends emitting `CopyEdge` + `CallConstraint`), GO-02 (Go lowering to graph constraints), ADAPT-01 (the future `ModelEdge` producer). Phase→requirement map line 145.
- `.planning/PROJECT.md` — Product boundary, private-analysis-first milestone intent, public API discipline carried into v1.3, benchmark baselines (Go RTA 10% precision / 2.7% recall; Jelly 25% precision / 0.63% recall), the v1.3 "single shared semantic graph / solver core" keystone statement.
- `.planning/STATE.md` — Current v1.3 state, Phase 43 closeout, open repo-admin action T-42-04-10 (leak-gate branch protection).

### Immediate Upstream Phase Context (read first)

- `.planning/phases/43-reachability-roots-per-suite-scoring-mode/43-CONTEXT.md` — Closed-enum byte-stability discipline (`RootKind`), composition-over-mutation, stable-key recipe, dense-ID-after-sort rule, provider digest recipe (D-19), the **determinism gate** (REACH-03) this phase inherits, and the naming-collision guard pattern (D-02) that D-09 mirrors. Roots produced here become semantic-graph inputs in later phases.
- `.planning/phases/42-benchmark-identity-renderers-dedup-identity-taxonomy/42-CONTEXT.md` — `IdentityCategory` closed-enum + `#[repr(u8)]` byte-stability template, `polint.identity` provider, dedup total-order key, CRLF/render-time normalization, cross-platform byte-identical contract. `NodeKind`/`EdgeKind`/`ConstraintKind` follow this exact discipline.

### v1.3 Graph Engine Benchmark Research

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` — **Primary source.** "Architectural Recommendation" section (the shared code representation + solver core owning stable identities, MIR ops, CFG, scope/reference/binding, value/points-to constraints, call-graph constraints, model/adaptation constraints, provenance/precision/taxonomy), the constraint-construction complexity notes, and the "frontends add constraints, solvers derive edges" framing that GRAPH-01/02 implement.
- `research/call-graphs/FINAL-REPORT.md` — Layered call-graph conclusion, constraint-based solving framing, repo-local model provenance (motivates `ModelEdge`).
- `research/evaluation-harness/STANDARD.md` — Determinism requirements inherited from the Phase 43 gate.
- `research/evaluation-harness/decisions/decision-log.md` — Accumulated benchmark architecture decisions inherited from v1.2.

### Existing Implementation Touch Points

- `crates/polint/src/analysis/mod.rs` — Register `pub(crate) mod semantic_graph;` here alongside the existing private analysis modules.
- `crates/polint/src/analysis/ids.rs` — Run-local ID newtypes; add `SemanticNodeId`/`SemanticEdgeId` (and any constraint ID) here following the `pub(crate) struct XId(pub(crate) u64)` pattern. Existing `CallSiteId`, `PlaceId`, `ObjectTokenId` are node-reference IDs (D-04).
- `crates/polint/src/core/mod.rs` — `FunctionId` (line 130), `PackageId` (133), `SymbolId` (151); the node-reference identities for `Function`/`Package` nodes (D-04).
- `crates/polint/src/symbol_graph/semantic.rs` — `ScopeId` (line 10); the node-reference identity for `Scope` nodes (D-04).
- `crates/polint/src/analysis/points_to/{facts.rs,constraints.rs}` — **Read for the naming-collision guard (D-09).** `PointsToConstraintKind` (`AddressOf`/`Copy`/`CallReturn`/…) and `PointsToConstraintFact` are the **sub-domain** constraint language and the fact-family shape `ConstraintFact` mirrors. The GRAPH-02 vocabulary sits above this; Phase 47 folds `points_to::solver` in — Phase 44 must NOT merge or rename it.
- `crates/polint/src/analysis/calls/{facts.rs,store.rs}` — Direct call-site/target facts; the source for `Call` edges + `CallConstraint` emission (D-11). Reference by stable key; do not mutate.
- `crates/polint/src/analysis/values/facts.rs` & `crates/polint/src/analysis/access_paths/facts.rs` — Value/allocation/access-path facts; sources for `Alloc`/`CopyEdge`/`FieldLoad`/`FieldStore` constraint emission (D-11).
- `crates/polint/src/analysis/identity/` — Identity records (Phase 42); nodes may reference identity for stable naming.
- `crates/polint/src/analysis_kernel/provider.rs` — Provider manifest + `provider_order_for_test()` ordering (lines ~760–880). Add `polint.semantic_graph` after `polint.type_value_alias`, before `polint.refined_calls` (D-16); update the ordering assertions. This is also the `provider_manifests()` machinery the Phase 43 determinism gate reads to auto-enroll the new provider (D-18).
- `crates/polint/src/analysis/provider.rs` & `crates/polint/src/analysis/cache_key.rs` — Provider-output digest + cache-key recipe (D-17); follow the existing per-provider digest participation pattern.
- `crates/polint/src/analysis/{validate.rs,stable_key.rs,store.rs}` — Validation pass pattern (D-15), length-prefixed labeled-parts stable-key recipe (D-06), and store conventions to mirror.
- `crates/polint/tests/public_surface_leak.rs` — v1.3 leak gate (Phase 42). All new `semantic_graph` types stay `pub(crate)` and keep this green; do NOT extend `ALLOWED_PRELUDE`.
- `tests/eval-fixtures/{identity/,determinism/}` — Native fixture-tree precedents; add `tests/eval-fixtures/semantic-graph/` with Go + TS/JS snapshot fixtures (D-12).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::points_to::facts::PointsToConstraintFact` (`{ id, kind, status, precision, stable_key }`) is the **exact shape** the new `ConstraintFact` mirrors (D-10) — an indexable, snapshot-serializable, digest-participating constraint fact family already exists as a template.
- `analysis::points_to::PointsToConstraintKind` already enumerates `AddressOf`/`Copy`/`CallReturn`/field ops — the conceptual map for which existing points-to constraints will later fold under the GRAPH-02 `CopyEdge`/`Alloc`/`FieldLoad`/`FieldStore` umbrella (Phase 47), informing D-09's separation.
- Existing identity newtypes — `core::FunctionId`/`PackageId`/`SymbolId`, `analysis::ids::{CallSiteId, PlaceId, ObjectTokenId}`, `symbol_graph::semantic::ScopeId` — already provide stable identities for all seven `NodeKind` referents, so nodes compose existing IDs (D-04) rather than re-deriving them.
- `analysis::calls` (direct call-site/target facts) and `analysis::values`/`access_paths` (value/alloc/field facts) are the **already-available pre-Phase-45/46 producers** that let Phase 44 emit a real-but-minimal graph and prove the vocabulary end-to-end via snapshots (D-11).
- `analysis_kernel::provider::{provider_order_for_test, provider_manifests, ProviderOrderRow}` enumerate providers deterministically and (via the Phase 43 gate) auto-enroll new providers in the determinism shuffle (D-16, D-18).
- `analysis_kernel::stable_key_from_parts` / `analysis::stable_key` — the length-prefixed labeled-parts recipe reused for node/edge/constraint stable keys (D-06).

### Established Patterns

- **Closed-enum byte-stability:** Phase 42 `IdentityCategory` / Phase 43 `RootKind` use pinned source order + `#[repr(u8)]` so serde + `Ord` are declaration-driven and byte-stable. `NodeKind`, `EdgeKind`, and `ConstraintKind` all follow this (D-02, D-03, D-08).
- **Composition over mutation:** Phase 42 identity and Phase 43 reachability reference existing facts by ID/stable key rather than rewriting them. The semantic graph references all sources by stable identity and mutates none (D-04, D-13).
- **Dense IDs after sort:** v1.2/Phase 43 assign dense run-local IDs only after sorting by stable key. `SemanticNodeId`/`SemanticEdgeId` inherit this (D-05, D-06).
- **Provider digest participation:** every v1.2 provider digests source + config + upstream provider output digests; `polint.semantic_graph` follows the same recipe (D-17).
- **Honest status/precision:** unsupported/setup-missing inputs become explicit facts, never silent drops; precision ceiling rejects `Exact` for derived edges (D-07, D-15). `ModelEdge` is reserved-but-empty (no producer until Phase 49) rather than faked (D-11).
- **Cross-platform byte-identical proof + snapshot fixtures:** Phase 42 identity and Phase 43 determinism fixtures are the precedent for the new `tests/eval-fixtures/semantic-graph/` snapshots (D-12).
- **Naming-collision guard via top-of-module doc comment:** Phase 43 D-02 distinguished whole-program vs block-level "reachability"; Phase 44 D-09 distinguishes unified-graph `ConstraintKind` vs points-to-sub-domain `PointsToConstraintKind` the same way.

### Integration Points

- `analysis/mod.rs` gains `pub(crate) mod semantic_graph;`.
- `analysis_kernel` provider manifest + `provider_order_for_test()` gain `polint.semantic_graph` after `polint.type_value_alias`, before `polint.refined_calls` (D-16); ordering assertions updated.
- `analysis::ids` gains `SemanticNodeId`/`SemanticEdgeId` (and constraint ID).
- The graph provider consumes `polint.calls`, `polint.identity`, `polint.abstract_domains`, `polint.entrypoints`, `polint.reachability`, `polint.type_value_alias`, and symbol/module-graph outputs; emits node/edge/constraint facts; participates in the cache key (D-17) and the Phase 43 determinism gate (D-18).
- `crates/polint/tests/public_surface_leak.rs` must stay green with all new types `pub(crate)`.
- `tests/eval-fixtures/semantic-graph/` is the new snapshot fixture home (D-12).

</code_context>

<specifics>
## Specific Ideas

- The `ConstraintKind` ↔ `PointsToConstraintKind` separation is the single most important decision in this phase. Getting it wrong (merging/renaming points-to constraints now) would couple Phase 44 to Phase 47's solver-folding work and break the layered architecture the research doc prescribes. The top-of-module doc comment distinguishing the unified frontend vocabulary from the points-to sub-domain's internal language is **mandatory** (D-09).
- `ModelEdge` has **no producer until Phase 49 (ADAPT-01)**. Reserve the variant and emit zero `ModelEdge` constraints in Phase 44 — documented as honest emptiness, exactly like Phase 43 reserved `solver_step_count`/`budget_exceeded_reasons` defaulted-to-empty for the later solver (D-11). Do NOT fake a producer.
- GRAPH-02's "constraint emission verified by snapshot fixtures" means Phase 44 must emit *real* constraints from already-available facts (calls, values, access paths), not ship a type-only stub. The minimal Go + TS/JS snapshot fixtures are the acceptance artifact (D-11, D-12).
- Node identities must **compose** existing v1.2 IDs (`FunctionId`, `CallSiteId`, `ScopeId`, `PlaceId`, `ObjectTokenId`, module/package IDs), not duplicate them — the seven `NodeKind` referents all already have stable identities in the codebase (D-04).
- Recommended provider slot is after `polint.type_value_alias` and before `polint.refined_calls`: every node/constraint source is available by then, and it lands exactly where Phase 47's GRAPH-05 refined-calls rework will read graph/solver output — forward-compatible placement (D-16).

</specifics>

<deferred>
## Deferred Ideas

- **Unified solver core (`VecDeque` worklist, `SolverBudget`/`BudgetStatus`, `SolverPolicy` trait), folding `points_to::solver` in as a sub-domain** — Phase 47 (GRAPH-03). Phase 44 only defines the graph + vocabulary the solver consumes.
- **`DerivedEdgeProvenance` on solver-derived edges (contributing fact IDs, constraint kind, solver step) for `polint explain`** — Phase 47 (GRAPH-04). Phase 44 edges are aggregated from existing facts, not solver-derived.
- **`refined_calls::provider` rework to project over solver output (preserving `RefinedCallEdgeFact`)** — Phase 47 (GRAPH-05). Phase 44's provider slot is chosen to be forward-compatible with this rework (D-16).
- **JS/TS inventory, scope, bindings, module graph, and direct calls emitted as `CopyEdge` + `CallConstraint`** — Phase 45 (JS-01/02/03). The richest TS/JS constraint producer; writes into the Phase 44 vocabulary.
- **Go semantic frontend + sidecar lowering NDJSON facts to graph constraints** — Phase 46 (GO-01..04, esp. GO-02 `src/go/semantic/`). The richest Go constraint producer; writes into the Phase 44 vocabulary.
- **Adaptation model layer producing `ModelEdge` constraints (`analysis::adaptation/`, TOML model schema, validator confirming target symbols exist in the graph)** — Phase 49 (ADAPT-01). The `ModelEdge` variant is reserved-but-empty until then (D-11).
- **Per-family cache keys for sidecar binary digest, Go toolchain version, adaptation model files, solver budgets; budget enforcement surfaced as facts** — Phases 50/CACHE-01/CACHE-02. Phase 44's cache key follows the v1.2 recipe but does not yet digest sidecar/toolchain/model/budget inputs (no such inputs exist yet).
- **Public SDK promotion of any v1.3 semantic-graph view** — explicitly out of v1.3 per ROADMAP.md (SDK-FUT-01 requires two-milestone benchmark stability); revisit at milestone close.

### Reviewed Todos (not folded)

None — `todo.match-phase 44` returned 0 matches.

</deferred>

---

*Phase: 44-Semantic Graph Skeleton & Constraint Vocabulary*
*Context gathered: 2026-05-30*
