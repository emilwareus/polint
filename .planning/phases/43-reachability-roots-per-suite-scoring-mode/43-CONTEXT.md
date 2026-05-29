# Phase 43: Reachability, Roots & Per-Suite Scoring Mode - Context

**Gathered:** 2026-05-29
**Status:** Ready for planning
**Mode:** `/gsd:discuss-phase 43 --auto`

<domain>
## Phase Boundary

Phase 43 turns v1.2's per-function call facts into a **whole-program reachability layer** and teaches the benchmark harness to **score each suite in the mode its oracle expects**, then locks both behind a **determinism gate** that every later solver phase inherits. Concretely it delivers three things:

1. **Reachability roots (REACH-01)** — explicit roots (`main`, `init`, exported symbols, tests, framework entrypoints, configured repo entrypoints) discovered from the v1.2 entrypoint substrate (`analysis::entrypoints`, Phase 35) plus Go/TS function and symbol facts, exposed as typed `pub(crate)` facts.
2. **Per-suite scoring mode + reachable-graph marking (REACH-02)** — a required `scoring_mode` field (`oracle-rta`, `oracle-jelly`, `whole-repo`) on every suite manifest (the gate fails when it is missing), and a reachable-graph computation over the available direct-call edges so unreachable direct calls **remain facts but are marked outside the reachable graph**.
3. **Determinism gate (REACH-03)** — a 10-shuffle gate proving byte-identical observed JSON, identical solver step counts, and identical budget-exceeded reasons across permuted provider order, wired so every subsequent solver-introducing phase (44–54) inherits it as an acceptance gate.

This phase does **not** build the shared semantic graph or constraint vocabulary (Phase 44), does **not** add JS/TS inventory/scope/module-graph (Phase 45) or the Go sidecar (Phase 46), and does **not** implement the unified solver or the Go RTA reachability fixpoint over solver edges (Phases 47/48). Phase 43's reachable set is computed over the **direct-call edge set from v1.2 `analysis::calls`** — the only edge family available pre-solver. Later phases swap in richer solver-derived edges behind the same marking contract. No new public SDK type is promoted (v1.3 discipline; the public-surface-leak gate from Phase 42 still applies).

</domain>

<decisions>
## Implementation Decisions

### Reachability Roots Module and Fact Shape (REACH-01)

- **D-01:** Add a new private module `analysis::reachability` (`crates/polint/src/analysis/reachability/`). Every type, fact, enum, and function is `pub(crate)`. Do not extend `analysis::entrypoints` (that stays the framework-entrypoint substrate) and do not extend `analysis::calls`.
- **D-02:** **Naming-collision guard (MANDATORY):** the existing `analysis::domains` abstract domain `polint.domain.reachability` is **block-level reachability inside one function body** (`Reachable`/`Unreachable`/`Ambiguous`). The new module is **whole-program reachability-from-roots** — a different concept. Use module `analysis::reachability` and provider id `polint.reachability`; never reuse `polint.domain.reachability`. Add a doc comment at the top of `reachability/mod.rs` stating the distinction so future readers do not conflate them.
- **D-03:** The root fact is a single type `ReachabilityRootFact` with fields: `id: ReachabilityRootId`, `kind: RootKind`, `language: Language`, `target_function: FunctionId`, `target_symbol: Option<SymbolId>`, `originating_entrypoint: Option<EntrypointId>` (set for `Test`/`FrameworkEntrypoint` roots bridged from Phase 35), `file: FileId`, `span: Span`, `precision: RootPrecision`, `provenance: RootProvenance`, `status: RootStatus`, `provider_id: String`, `stable_key: String`. Compose v1.2 IDs by reference — do not duplicate or rewrite entrypoint/call facts.
- **D-04:** `RootKind` is a **closed enum** (no `Other`/`Unknown`, no `#[non_exhaustive]`, pinned source order with explicit `#[repr(u8)]` ordinals — mirror the Phase 42 `IdentityCategory` discipline so serde + `Ord` byte-stability is declaration-driven): `Main`, `Init`, `Exported`, `Test`, `FrameworkEntrypoint`, `ConfiguredEntrypoint`.
- **D-05:** Reuse the v1.2 status/precision/provenance vocabulary shape. `RootStatus { Resolved, Partial, Unresolved, SetupMissing, Unsupported }`, `RootPrecision { ResolvedStatic, SetupAware, Heuristic, Conservative, Unknown }`, `RootProvenance { NativeDiscovery, EntrypointBridge, Configured }` — match the `EntrypointStatus`/`EntrypointPrecision`/`EntrypointProvenance` shapes from `analysis::entrypoints::facts` so the bridge is loss-free. The planner may reuse the entrypoint enums directly if that proves cleaner.
- **D-06:** Add `ReachabilityRootId` (and any other run-local IDs) to `analysis::ids` following the existing newtype pattern; do not invent a parallel ID scheme. Roots get dense IDs assigned **after** sorting by stable key (v1.2 determinism rule). The root `stable_key` is built from `(language, kind, function stable identity)` using the existing length-prefixed labeled-parts stable-key recipe — never run-local IDs.

### Root Discovery Sources and Per-Language Semantics (REACH-01)

- **D-07:** Discovery consumes **existing facts only** — no new parsing. Sources: Go function/package facts (for `Main`/`Init`/`Exported`), TS/JS symbol + module-graph export facts (for `Exported`), `analysis::entrypoints` facts (for `Test` and `FrameworkEntrypoint`), and `.polint.toml` configured roots (for `ConfiguredEntrypoint`). Honest labels: setup-missing inputs yield `SetupMissing` roots, not fabricated ones.
- **D-08:** **Go `Main`/`Init`:** `Main` = `func main` in a `package main` (derivable from existing Go function facts + package-name facts already used by the Phase 42 identity renderer). `Init` = every `func init` (any package; multiple allowed). Precision `ResolvedStatic` when the package/function facts are present.
- **D-09:** **Go `Exported`:** top-level exported (capitalized) functions and exported methods in non-`main` packages, from existing function/symbol facts.
- **D-10:** **TS/JS `Exported`:** functions reachable through `export`/re-export edges, sourced from the existing symbol-graph + module-graph export facts. Precision `SetupAware` (depends on resolver/tsconfig setup being present).
- **D-11:** **TS/JS `Main`/`Init`:** TS/JS has no intrinsic `main`/`init`. Do **not** synthesize them in Phase 43 — a TS/JS entry module is expressed as a `ConfiguredEntrypoint` (D-13) or arrives through the Phase 45 JS/TS work. Document this explicitly; do not emit empty/placeholder TS/JS `Main` roots.
- **D-12:** **`Test` and `FrameworkEntrypoint` bridge:** map `EntrypointKind::Test` entrypoint facts to `RootKind::Test`; map every other `EntrypointKind` (HttpRoute, HttpMiddleware, McpTool/Resource/Prompt, CliCommand, Job, QueueConsumer, ServerlessHandler, LifecycleCallback, EventListener, GeneratedDispatch) to `RootKind::FrameworkEntrypoint`. Each bridged root carries `originating_entrypoint` and inherits the entrypoint's precision/status so no signal is lost.
- **D-13:** **`ConfiguredEntrypoint`:** add a **minimal** `.polint.toml` configured-roots input (e.g. a `[reachability]` table with a `roots = ["pkg/path.Func", "src/x.ts#handler"]` list — exact shape is a planner/researcher decision). Each configured root resolves to a function via existing symbol facts; an unresolvable entry becomes a `RootStatus::Unresolved` root fact (honest, debuggable), never a silent drop. This is `.polint.toml` config surface, **not** SDK promotion, so it is permitted under v1.3 discipline. Configured-root inputs participate in the reachability cache key (D-19).

### Per-Suite Scoring Mode + Reachable-Graph Marking (REACH-02)

- **D-14:** Add a **required** `scoring_mode: ScoringMode` field to `eval::suite::SuiteManifest`. `ScoringMode` is a closed enum serialized to the exact spec strings via serde rename: `OracleRta` → `"oracle-rta"`, `OracleJelly` → `"oracle-jelly"`, `WholeRepo` → `"whole-repo"`. A test asserts the wire strings match byte-for-byte.
- **D-15:** **Gate-fails-if-missing** is satisfied two ways: (a) `SuiteManifest` already has `#[serde(deny_unknown_fields)]` and the field is non-`Option`, so a manifest lacking `scoring_mode` fails TOML deserialization; and (b) add an explicit `SuiteManifest::validate()` check plus a dedicated test that a manifest without `scoring_mode` is rejected with a clear error. Both layers ship — structural + explicit.
- **D-16:** Update all **four** existing suite manifests: `go-x-tools-rta-callgraph` → `oracle-rta`; `jelly-callgraph-micro` → `oracle-jelly`; `gosec-samples` → `whole-repo`; `secbench-js-smoke` → `whole-repo` (the security suites are not reachability-filtered call-graph suites; `whole-repo` is the correct default for them).
- **D-17:** **Mode semantics for scoring:**
  - `oracle-rta` — score only edges whose source function is in the reachable-from-roots set (the Go x/tools RTA oracle reports reachable-only edges). Edges outside the reachable graph are excluded from precision/recall but retained as facts marked unreachable.
  - `oracle-jelly` — Jelly enumerates functions/callsites across all modules independent of `main`-reachability; score against the full enumerated set. Reachability marking is recorded but does **not** filter scoring in this mode.
  - `whole-repo` — score everything; no reachability filtering.
- **D-18:** **Reachable-graph marking** is a **separate fact family**, not an in-place mutation of `analysis::calls`. Emit a `CallReachabilityFact` (or equivalently named marking fact) in `analysis::reachability`, keyed by the call-site **stable key**, carrying `in_reachable_graph: bool` and a compact root-path/reason. This composition mirrors the Phase 42 identity approach (reference existing facts by stable identity rather than rewriting them). The reachable set is computed by BFS/DFS from `ReachabilityRootFact.target_function` over **direct-call resolved-target edges** from `analysis::calls` — the only pre-solver edge set. Document that Phases 47/48 replace the edge set with solver-derived edges behind this same marking contract.
- **D-19:** **Provider wiring:** the `polint.reachability` provider slots **after `polint.entrypoints`** in the kernel manifest (it consumes entrypoints + direct calls + identity + symbol/module-graph facts). Its cache key digests source files, the `polint.calls` output digest, the `polint.entrypoints` output digest, the `polint.identity` output digest, symbol/module-graph digests, the configured-roots config input, and the provider/schema version — following the established v1.2 digest recipe so cache invalidation behaves identically. Precision ceiling rejects `FactPrecision::Exact` (reachability over direct calls is setup-aware/conservative, never exact).

### Determinism Gate Design + Cross-Phase Inheritance (REACH-03)

- **D-20:** The gate runs the eval observation **N = 10 times under shuffled (seeded, distinct) permutations** and asserts: (a) byte-identical **normalized observed JSON**, (b) identical **solver step counts**, and (c) identical **budget-exceeded reasons**. Byte-identity of the full normalized observed JSON transitively covers (b) and (c) once those fields exist.
- **D-21:** **What is shuffled:** the permutation surface is (1) provider **execution order** wherever the dependency DAG allows independent reordering, plus (2) provider **output row order** / input fact-insertion order (re-inserting facts in permuted order). Providers are already order-independent given their inputs (the v1.2 kernel contract); the gate *proves* it rather than assuming it. The exact permutation plumbing is a planner/researcher decision, but the **contract is fixed**: 10 seeded permutations → byte-identical normalized observed JSON. Anchor on the existing `analysis_kernel::provider::{provider_order_for_test, provider_manifests}` machinery.
- **D-22:** **Inheritance must be near-zero-maintenance.** Build the gate as a **parametric harness driven by `provider_manifests()`** so that when a later phase registers a new solver provider, it is automatically included in the shuffled set and the byte-identical assertion — no per-phase edit to the harness required. Place the harness at `crates/polint/src/eval/determinism_gate.rs` (or a `tests/` integration test), `pub(crate)`/test-facing.
- **D-23:** **Reserve the step-count / budget-reason JSON shape now.** Phase 43 has no solver yet, so add `solver_step_count` and `budget_exceeded_reasons` fields to the observed/report JSON defaulted to zero/empty (using the Phase 42 `#[serde(default)]` + frozen-`MetricSummary`-shape discipline, layout-locked by a destructure test). Later phases populate them without breaking the gate or downstream JSON consumers.
- **D-24:** Add at least one determinism fixture under `tests/eval-fixtures/determinism/` exercising a Go case and a TS/JS case that both have roots + direct calls + at least one unreachable call (so reachable-graph marking is exercised inside the gate). The gate runs in **fast CI on Linux + macOS**; both platforms must pass independently (no cross-platform averaging) — consistent with the Phase 42 cross-platform byte-identical contract.
- **D-25:** **Per-phase obligation (document in the gate file and verification convention):** every subsequent solver-introducing phase (44–54) MUST (a) ensure its new provider is part of the shuffled-provider set (automatic via D-22) and (b) keep the determinism-gate fixture green as a named acceptance gate in its verification. This is the REACH-03 "inherited by every subsequent solver phase" requirement.

### Claude's Discretion

- The internal file layout of `analysis::reachability/` (`facts.rs`, `provider.rs`, `discover.rs`, `traverse.rs`, `cache_key.rs`, `validate.rs`, `store.rs`, `debug.rs`) is the planner's choice, provided visibility stays `pub(crate)` and digest discipline matches `analysis::calls`/`analysis::entrypoints`.
- Whether `RootKind` reuses the existing `EntrypointPrecision`/`EntrypointStatus` enums directly or defines parallel `RootPrecision`/`RootStatus` (D-05) — planner picks the cleaner option, provided the entrypoint bridge is loss-free.
- The exact `.polint.toml` configured-roots schema (D-13) — planner/researcher decide field names and resolution rules; keep it minimal and honest.
- The precise provider-order permutation plumbing for the determinism gate (D-21) — planner/researcher decide; the 10-shuffle byte-identical contract is the fixed acceptance criterion.
- Whether reachable-graph marking is a standalone `CallReachabilityFact` family or a thin reachable-set index queried by stable key (D-18) — planner picks, provided `analysis::calls` is not mutated and snapshot fixtures stay byte-stable.
- The natural plan slicing: (1) reachability module + root facts + discovery + provider/cache; (2) reachable-set traversal + call marking + scoring-mode field on manifests + manifest updates + mode-aware scoring; (3) determinism gate harness + fixtures + CI wiring + inheritance contract.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 43 goal, REACH-01/02/03 mapping, success criteria, v1.3 milestone framing, no-public-SDK-promotion rule, parallel-phase notes.
- `.planning/REQUIREMENTS.md` — REACH-01/02/03 requirement text (lines 19–21) and dependency on the v1.2 entrypoint substrate; surrounding GRAPH-/GO-/JS- requirements that consume reachability downstream.
- `.planning/PROJECT.md` — Product boundary, private-analysis-first milestone intent, public API discipline carried into v1.3, benchmark baselines (Go RTA 10% precision / 2.7% recall; Jelly 25% precision / 0.63% recall).
- `.planning/STATE.md` — Current v1.3 state, Phase 42 closeout, accumulated decisions (esp. Phase 35 entrypoints + Phase 42 identity), open repo-admin action T-42-04-10 (leak-gate branch protection).

### Immediate Upstream Phase Context (read first)

- `.planning/phases/42-benchmark-identity-renderers-dedup-identity-taxonomy/42-CONTEXT.md` — Identity record shape (`IdentityRecord`, originating call IDs), closed-enum discipline (`IdentityCategory`), `polint.identity` provider placement, dedup total-order key, CRLF/render-time normalization, cross-platform byte-identical contract. Roots and reachability marking reference identity records and inherit this determinism discipline.

### v1.3 Graph Engine Benchmark Research

- `research/evaluation-harness/GRAPH-ENGINE-BENCHMARK-RESEARCH.md` — Two-suite scope (Go x/tools RTA + Jelly micro), what each oracle expects (RTA = reachable-only edges from roots; Jelly = whole-module enumeration), baseline numbers, capability gaps that motivate scoring modes + reachability.
- `research/evaluation-harness/FINAL-REPORT.md` — External-benchmark-first strategy, suite ranking, measurement model, tiers.
- `research/evaluation-harness/RECOMMENDED_IMPLEMENTATION.md` — Internal eval architecture, canonical model, matchers/metrics, native fixture adapter, default-vs-extension delta.
- `research/evaluation-harness/STANDARD.md` — Suite/case/adapter vocabulary, expected/observed model, result classes, **determinism requirements** (directly informs REACH-03).
- `research/evaluation-harness/decisions/decision-log.md` — Accumulated benchmark architecture decisions inherited from v1.2.
- `research/call-graphs/FINAL-REPORT.md` — Layered call-graph conclusion, roots/reachability framing, unresolved facts, repo-local model provenance.

### Upstream v1.2 Phase Decisions Carried Forward

- `.planning/milestones/v1.2-phases/35-framework-entrypoints-and-trust-boundaries/35-CONTEXT.md` — Entrypoint fact model (`EntrypointFact`, `EntrypointKind`, trust boundaries, dispatch), provider order (`polint.entrypoints` after `polint.direct_summaries`/SCC, before `polint.extensions`/`polint.refined_calls`). **The substrate REACH-01 roots derive from.**
- `.planning/milestones/v1.2-phases/30-direct-call-facts/30-CONTEXT.md` — Direct call-site/target/unresolved fact model and resolution rules; the reachable-set traversal walks these edges.
- `.planning/milestones/v1.2-phases/40-external-benchmark-adapters-and-promotion-gates/40-CONTEXT.md` — Suite manifest shape, deterministic JSON reports, tiered gates; `scoring_mode` extends this manifest.
- `.planning/milestones/v1.2-phases/41-public-sdk-query-views-and-agent-ergonomics/41-CONTEXT.md` — Final v1.2 public-surface decisions; v1.3 must not regress these (leak gate).

### Existing Implementation Touch Points

- `crates/polint/src/analysis/entrypoints/{facts.rs,provider.rs,recognizers_go.rs,recognizers_ts.rs,dispatch.rs,trust_boundaries.rs}` — `EntrypointFact`/`EntrypointKind`/precision/status/provenance enums; the bridge source for `Test`/`FrameworkEntrypoint` roots.
- `crates/polint/src/analysis/calls/{facts.rs,provider.rs,store.rs}` — Direct call-site/target/unresolved facts; the pre-solver edge set the reachable-graph traversal walks. Reachability marks these by stable key; it does not modify them.
- `crates/polint/src/analysis/identity/` — Identity records (Phase 42); roots and reachability marking reference identity by originating call IDs.
- `crates/polint/src/analysis/ids.rs` — Run-local ID newtypes; add `ReachabilityRootId` here following the pattern. Do not rewrite existing IDs.
- `crates/polint/src/analysis/domains/core.rs` (line ~106) — `polint.domain.reachability` **block-level** abstract domain. Read to confirm the naming-collision guard (D-02): the new whole-program module must NOT reuse this id.
- `crates/polint/src/analysis_kernel/provider.rs` — Provider manifest/order machinery (`provider_order_for_test`, `provider_manifests`, `ProviderOrderRow`); `polint.reachability` slots after `polint.entrypoints`. **Anchor for the determinism-gate permutation harness (D-21/D-22).**
- `crates/polint/src/eval/suite.rs` — `SuiteManifest` (`#[serde(deny_unknown_fields)]`), `SuiteScoring`, `validate()`; add `scoring_mode: ScoringMode` here (D-14/D-15).
- `crates/polint/src/eval/external/{go_x_tools_callgraph.rs,jelly_callgraph.rs}` — Adapters that read the manifests; mode-aware scoring (D-17) plugs into these / the runner/metrics path.
- `crates/polint/src/eval/{runner.rs,metrics.rs,report.rs,observed.rs,model.rs}` — Eval pipeline; mode-aware scoring filter, reserved `solver_step_count`/`budget_exceeded_reasons` fields (D-23), and the determinism gate live here.
- `research/evaluation-harness/suites/{go-x-tools-rta-callgraph.toml,jelly-callgraph-micro.toml,gosec-samples.toml,secbench-js-smoke.toml}` — The four manifests to update with `scoring_mode` (D-16).
- `tests/eval-fixtures/` — Native fixture tree; add `tests/eval-fixtures/determinism/` (D-24). `tests/eval-fixtures/identity/` (Phase 42) is the structural precedent.
- `crates/polint/tests/public_surface_leak.rs` — v1.3 leak gate (Phase 42). New reachability/scoring-mode types must stay `pub(crate)` and keep this green; do NOT extend `ALLOWED_PRELUDE`.
- `crates/polint-config/src/` — Config loading surface; the minimal configured-roots `.polint.toml` input (D-13) plugs in here.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::entrypoints::facts::{EntrypointFact, EntrypointKind, EntrypointPrecision, EntrypointStatus, EntrypointProvenance}` already model framework entrypoints, tests, and dispatch with `target_function: FunctionId`. REACH-01 bridges these into roots rather than re-discovering them — `Test` and all framework kinds come free.
- `analysis::calls::facts` (`CallSiteFact`, `CallTargetFact`, `UnresolvedCallFact`) already give resolved direct-call edges by stable key — the BFS/DFS edge set for the reachable-graph computation (D-18).
- `analysis::identity::IdentityRecord` (Phase 42) already carries package/module + container + originating call IDs, deduplicated and totally ordered — roots/reachability reference identity instead of recomputing names.
- `eval::suite::SuiteManifest` already uses `#[serde(deny_unknown_fields)]` and required fields, so adding a non-`Option` `scoring_mode` makes "gate fails if missing" structural (D-15).
- `analysis_kernel::provider::{provider_order_for_test, provider_manifests, ProviderOrderRow}` already enumerate providers deterministically and expose test-only order inspection — the anchor for the 10-shuffle determinism harness (D-21/D-22).
- Phase 31's `domains/solver.rs::deterministic_shuffled_rows_produce_byte_identical_result_digests` and Phase 42's dedup/render byte-identity tests are existing per-provider determinism precedents the gate generalizes across all providers.

### Established Patterns

- **Provider digest participation:** every v1.2 provider digests source, config, lifecycle, and upstream provider output digests; `polint.reachability` follows the same recipe (D-19) so cache invalidation behaves identically.
- **Closed-enum byte-stability:** Phase 42 `IdentityCategory` is a closed enum with pinned source order + `#[repr(u8)]` so serde + `Ord` are declaration-driven and byte-stable. `RootKind` and `ScoringMode` follow this (D-04, D-14).
- **Composition over mutation:** Phase 42 identity references existing call facts by ID rather than rewriting them. Reachability marking references call sites by stable key instead of mutating `analysis::calls` (D-18).
- **Determinism:** v1.2 providers sort by stable key and assign dense IDs only after sorting; roots inherit this (D-06). The new gate makes this a milestone-wide proven invariant rather than a per-provider one.
- **Honest status/precision:** unsupported/setup-missing inputs become explicit `SetupMissing`/`Unresolved` facts, never silent drops or fake exact claims (D-07, D-13); precision ceiling rejects `Exact` from setup-aware providers (D-19).
- **Cross-platform byte-identical proof:** existing CI gates verify Linux + macOS byte-identical reports; the determinism gate extends this (D-24).
- **Frozen report shape:** Phase 42 froze `MetricSummary` with a destructure layout-lock test and added new sections via `#[serde(default)]`; reserved `solver_step_count`/`budget_exceeded_reasons` follow the same discipline (D-23).

### Integration Points

- `analysis_kernel` provider manifest gains `polint.reachability` immediately after `polint.entrypoints` (D-19).
- `eval::suite::SuiteManifest` gains required `scoring_mode`; `eval::runner`/`eval::metrics` gain a mode-aware scoring filter that consults reachable-graph marking (D-17).
- `eval::report`/`eval::observed` reserve `solver_step_count` and `budget_exceeded_reasons` (defaulted) for later solver phases (D-23).
- The determinism-gate harness reads `provider_manifests()` so future solver providers auto-enroll (D-22).
- `analysis::ids` gains `ReachabilityRootId` (and any marking IDs); `crates/polint/tests/public_surface_leak.rs` must stay green with all new types `pub(crate)`.
- `crates/polint-config` gains a minimal configured-roots input feeding `ConfiguredEntrypoint` discovery (D-13).

</code_context>

<specifics>
## Specific Ideas

- The Go x/tools RTA oracle reports **reachable-from-roots** edges only, so `oracle-rta` scoring MUST filter by the reachable set; Jelly's micro oracle enumerates module-wide, so `oracle-jelly` MUST NOT filter — getting these two backwards would silently tank one suite's recall (D-17).
- The "gate fails if `scoring_mode` is missing" requirement is best proven by an explicit negative test (a manifest missing the field is rejected) on top of the structural `deny_unknown_fields` guarantee — don't rely on serde alone for the verification artifact (D-15).
- The determinism gate's value is **inheritance**: drive it off `provider_manifests()` so Phases 44–54 get coverage for free. A hand-maintained provider list would rot and the "inherited by every subsequent solver phase" requirement would quietly break (D-22, D-25).
- Reserve `solver_step_count` + `budget_exceeded_reasons` in the observed JSON now (defaulted, zero/empty) even though there is no solver yet — Phase 47+ populates them, and reserving them keeps the byte-identical gate stable across the milestone (D-23).
- The naming collision with the block-level `polint.domain.reachability` abstract domain is a real foot-gun; the top-of-module doc comment distinguishing whole-program-from-roots vs in-body-block reachability is mandatory (D-02).

</specifics>

<deferred>
## Deferred Ideas

- **Reachability fixpoint over solver-derived edges** — Phase 43 computes the reachable set over **direct-call** edges only. The RTA reachability fixpoint (address-taken tracking, dynamic dispatch by signature, interface invoke by method-set) is Phase 48 (`analysis::solver::go_rta`, GO-05); it reuses Phase 43's roots + marking contract.
- **Shared `analysis::semantic_graph` + `NodeKind`/`EdgeKind`/constraint vocabulary** — Phase 44 (GRAPH-01/02). Roots become graph inputs there.
- **JS/TS inventory, scope, bindings, module graph, direct calls as constraints** — Phase 45 (JS-01/02/03). A richer TS/JS entry/`main` notion may arrive here, superseding the Phase 43 `ConfiguredEntrypoint` workaround for TS/JS.
- **Go semantic frontend + sidecar** — Phase 46 (GO-01..04). Full Go module import-path qualification (deferred from Phase 42) lands here and may refine exported-root identity.
- **Unified solver core + `DerivedEdgeProvenance` + folding `points_to::solver`** — Phase 47 (GRAPH-03/04); inherits the Phase 43 determinism gate as an explicit acceptance criterion.
- **`solver_step_count` / `budget_exceeded_reasons` population** — fields are reserved in Phase 43 (D-23); first populated by the solver in Phase 47+.
- **Consolidated unknown taxonomy + `polint inspect unknowns --format json`** — Phase 52 (TAX-01); the only new public CLI surface in v1.3.
- **Per-suite precision floors, F-score β=0.5, polyglot canary, final leak gate** — Phase 54 (BENCH-01).
- **Public SDK promotion of any v1.3 type (incl. a future `Reachability<'_>` view)** — explicitly out of v1.3 per ROADMAP.md; revisit at milestone close after two-milestone benchmark stability.

### Reviewed Todos (not folded)

None — `todo.match-phase 43` returned 0 matches.

</deferred>

---

*Phase: 43-Reachability, Roots & Per-Suite Scoring Mode*
*Context gathered: 2026-05-29*
