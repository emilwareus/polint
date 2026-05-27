# Phase 33: Demand Queries and Summary SCC Cache - Context

**Gathered:** 2026-05-22
**Status:** Ready for planning
**Refreshed:** 2026-05-22 via `$gsd-discuss-phase 33 --auto`

> Auto-refresh note: Phase 33 already had context and plans when this discussion
> was re-run. The existing decisions remain locked; no new gray areas or scope
> changes were introduced.

<domain>
## Phase Boundary

Phase 33 delivers the internal demand-query layer, summary SCC scheduling and cache, extension-aware cache quarantine, and query trace/debug output for expensive analyses. It wires the already-defined QueryKey and SummaryKey vocabulary into a working scheduling and caching substrate.

This phase does NOT add new providers, new fact families, or new analysis capabilities. It adds the infrastructure that makes existing summaries interprocedural (SCC-aware) and makes future expensive analyses (refined calls, data flow, slicing) demand-driven rather than eager. The already-reserved `direct_summaries_layer_key()` function in `keys.rs` gets activated. Extension-authored providers (Phase 34), framework entrypoints (Phase 35), refined call graphs (Phase 37), and data flow (Phase 38) are consumers of this infrastructure, not part of it.

</domain>

<decisions>
## Implementation Decisions

### Demand Query Scope and Activation

- **D-01:** Keep all existing providers (source, syntax, module graph, symbol graph, topology, semantic MIR, CFG, calls, abstract domains, direct summaries, metrics) eager in Phase 33. They run unconditionally during `AnalysisKernel::run` as they do today. The demand layer is new infrastructure for future consumers, not a rewrite of existing execution.
- **D-02:** The demand-query layer should support two modes of use: (a) in-run memoization where an expensive view is computed once and reused within the same kernel run, and (b) cross-run caching where QueryKey-addressed results persist to the layer cache and can be restored on subsequent runs when inputs haven't changed.
- **D-03:** Phase 33 should have one concrete demand-driven consumer: **summary SCC closure**. Direct summaries (Phase 32) are still computed eagerly per-function, but the interprocedural closure — applying callee summaries to improve caller summaries — is computed on demand per SCC. This gives a real test of the demand infrastructure without disrupting existing eager providers.

### SCC Discovery and Scheduling

- **D-04:** Build the summary call graph from Phase 30 direct call target facts. For each function with a direct summary, discover which other functions it calls. Compute SCCs (strongly connected components) using Tarjan's algorithm or equivalent from petgraph. Schedule SCC processing in reverse topological order (leaf callees first, then their callers).
- **D-05:** For non-recursive SCCs (single function, no self-call): compute the interprocedural summary in one pass by applying callee summaries to the local analysis. For recursive SCCs: iterate with widening until fixpoint, bounded by a configurable iteration budget. Budget exhaustion produces `BudgetExceeded` summaries, not silent convergence claims.
- **D-06:** Implement **backdating** for summary SCC results. After recomputing an SCC's summaries (due to a source change in one of its functions), compare the new summary digests against the previously cached digests. If equal, the SCC's callers do not need recomputation — the invalidation cascade stops at the SCC boundary where summaries didn't actually change.
- **D-07:** Store SCC-level summary results as a unit. One function edit within an SCC invalidates the entire SCC's cached fixpoint, but SCCs that depend on it are only invalidated if the SCC's summary output digests actually changed (backdating from D-06).

### Extension-Aware Cache Quarantine

- **D-08:** Implement quarantine as a cache-level concept. When an extension's code digest or manifest changes, all cache entries that include that extension's digest in their key are marked quarantined. Quarantined entries are not used as cache hits but are also not deleted — they can be reinstated if the extension reverts to a previously seen digest.
- **D-09:** Native facts are never quarantined. Quarantine affects only extension-contributed cache entries. This ensures that removing or breaking an extension degrades gracefully to the native analysis rather than producing no analysis.
- **D-10:** Extension quarantine is infrastructure for Phase 34 (extension/provider sink). Phase 33 should define the quarantine mechanism, cache-key participation rules, and validation gates, but extension providers don't exist yet. The mechanism should be exercised through test-only synthetic extension digests following the Phase 22 pattern.

### Layer Cache Activation for Direct Summaries

- **D-11:** Activate the already-reserved `direct_summaries_layer_key()` function in `analysis_kernel/incremental/keys.rs`. Wire it into the direct summaries provider so that summary output is persisted to and restored from the layer cache, following the established pattern from Phase 24 (syntax, imports, module graph, symbols, metrics layer caching).
- **D-12:** Summary layer cache identity must include all upstream provider output digests (semantic MIR, CFG, calls, abstract domains, symbol graph, module topology, syntax), plus provider/schema version, config/lifecycle inputs, and absent extension/model/toolchain slots. The reserved function already has this shape.

### Query Trace and Debug Output

- **D-13:** Query trace output should follow the established crate-private debug JSON pattern. For each demand query executed in a run, record: query kind, precision tier, input layer digests, cache hit/miss status, compute time, and result digest. This trace is for internal debugging and eval only.
- **D-14:** SCC scheduling debug output should include: number of SCCs discovered, SCC sizes, processing order, iteration counts for recursive SCCs, backdating events (which SCCs had unchanged digests), and total functions processed. This stays crate-private and test-facing.
- **D-15:** Add native eval fixtures proving: (a) demand query cache hit/miss behavior, (b) SCC scheduling order correctness, (c) backdating behavior when summaries don't change, (d) quarantine behavior with synthetic extension digests, and (e) deterministic cold/warm/no-cache three-way equality.

### Validation and Correctness

- **D-16:** Extend metadata validation for demand query results. Validate that query results have: valid query keys, consistent precision tiers, non-stale input digests, and correct provenance. Invalid query results should be recomputed, not silently served.
- **D-17:** SCC scheduling must be deterministic. When multiple SCCs are independent (no dependency between them), process them in a deterministic order derived from sorted stable keys of their member functions. This ensures reproducible analysis across runs.

### Claude's Discretion

- The planner may choose whether demand queries are implemented as a new module (`analysis::demand` or `analysis_kernel::demand`) or as extensions to the existing incremental module. The key constraint is that demand execution must compose with the existing layer cache.
- The planner may decide whether SCC discovery runs as a dedicated internal pass or is integrated into the summary provider's execution. The constraint is that SCC ordering must be available before interprocedural summary closure starts.
- The planner may decide whether to implement SCC-aware interprocedural summary improvement in Phase 33 or only the scheduling/caching infrastructure (with interprocedural closure wired in Phase 37/38 when refined call graphs and data flow need it). The recommended path is to implement at least one concrete SCC consumer to prove the infrastructure works.
- The planner may decide the exact plan split — e.g., (1) demand query infrastructure + layer cache activation, (2) SCC discovery and scheduling, (3) interprocedural summary closure, (4) quarantine mechanism, (5) validation/debug/eval/no-leak proof — as long as each plan is independently reviewable and compiling.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 33 goal, requirement mapping (SAE-INT-03), and success criteria.
- `.planning/REQUIREMENTS.md` — SAE-INT-03 requirement text and traceability.
- `.planning/PROJECT.md` — Public API discipline, current milestone goals, and active v1.2 boundaries.

### Incremental Query Engine Research

- `research/incremental-query-engine/RECOMMENDED_IMPLEMENTATION.md` — Demand query architecture, QueryKey semantics, layer cache invalidation, and memoization model.
- `research/incremental-query-engine/FINAL-REPORT.md` — Architectural rationale for native layered incremental engine over Salsa/Datalog/DICE.

### Effects and Summaries Research

- `research/effects-summaries/RECOMMENDED_IMPLEMENTATION.md` — Summary SCC scheduling, fixpoint computation, widening strategy, backdating, and callee summary application.
- `research/effects-summaries/FINAL-REPORT.md` — Summary domain algebra, direct vs interprocedural summaries, extension summary merge policy.
- `research/effects-summaries/VALIDATION.md` — Summary validation levels, SCC fixture families, cache invalidation tests, and accuracy metrics.

### Analysis Kernel Research

- `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md` — Provider scheduling, kernel facade, metadata validation, and provider dependency ordering.

### Upstream Phase Decisions

- `.planning/phases/32-summary-kernel-and-direct-summaries/32-CONTEXT.md` — Direct summary store, four core domains, builder pattern, cache identity, and explicit deferral of SCC closure to Phase 33.
- `.planning/phases/31-p0-abstract-domain-kernel/31-CONTEXT.md` — Domain solver, product state, lattice traits, and explicit deferral of interprocedural summaries to Phase 32-33.
- `.planning/phases/30-direct-call-facts/30-CONTEXT.md` — Direct call facts, target/unresolved model, store indexes — the call graph that SCC discovery builds on.

### Existing Implementation

- `crates/polint/src/analysis_kernel/incremental/keys.rs` — QueryKey (lines 82-89), SummaryKey (lines 90-98), and reserved `direct_summaries_layer_key()` (line 675+).
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` — LayerCacheStore, LayerCacheManifest, read/write/eviction infrastructure.
- `crates/polint/src/analysis_kernel/incremental/invalidation.rs` — InvalidationAction enum (Reuse, Verify, Recompute, Drop, Quarantine).
- `crates/polint/src/analysis_kernel/incremental/dependency_index.rs` — CacheNode enum including Query variant, forward/reverse dependency graphs.
- `crates/polint/src/analysis/summaries/` — SummaryStore, DirectSummaryBuilder, SummaryFact, SummaryDomainKind, provider, cache_key, validation, debug.
- `crates/polint/src/analysis/calls/` — Direct call target facts used for SCC discovery.
- `crates/polint/src/analysis_kernel/provider.rs` — Current eager provider execution order (12 providers, direct_summaries is #11).

### API and Visibility

- `AGENTS.md` — Public API visibility rules and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` — Visibility tightening and supported API boundary guidance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis_kernel::incremental::keys` already defines `QueryKey` with query_kind, query_version, parameter_digest, layer_digests, budget_digest, and precision_tier. Phase 33 activates this for real demand queries.
- `analysis_kernel::incremental::keys::direct_summaries_layer_key()` is fully implemented but marked `#[expect(dead_code)]`. Phase 33 activates it for summary layer cache persistence.
- `analysis_kernel::incremental::invalidation` already defines `InvalidationAction::Quarantine` for extension-aware cache entries.
- `analysis_kernel::incremental::dependency_index` already supports `CacheNode::Query` in the dependency graph alongside Layer, Summary, and Diagnostic nodes.
- `analysis::summaries::store::SummaryStore` supports querying by callable_stable_key, domain, and function — these indexes are sufficient for SCC-aware summary lookup.
- `analysis::calls` provides the direct call target graph needed for SCC discovery.
- `petgraph` is already a workspace dependency (used for module graph) and supports SCC computation via `tarjan_scc` or Kosaraju.

### Established Patterns

- New v1.2 infrastructure stays crate-private, uses run-local dense IDs plus stable keys, and is guarded by no-leak CLI tests.
- Provider outputs are normalized deterministically, validated before use, and exposed to eval through test-only debug JSON.
- Layer cache: manifest-first publish (payload → manifest), invalidation fails closed, schema version guards, dependency index in manifest.
- Cache identity includes provider/schema/config/lifecycle/upstream digests plus absent future extension/model/toolchain slots.
- Eval fixtures use the established TOML manifest format with deterministic cold/warm/no-cache three-way equality.

### Integration Points

- Activate `direct_summaries_layer_key()` in the summary provider to persist/restore summary layer output.
- Add demand query execution path alongside the existing eager provider path in `AnalysisKernel::run`.
- Build SCC discovery from `CallStore` target facts after `polint.calls` provider completes.
- Extend `KernelRunReport` with demand query trace metadata.
- Extend metadata validation with demand query result checks.
- Extend eval observation with SCC scheduling and demand query cache behavior.

</code_context>

<specifics>
## Specific Ideas

- The reserved `direct_summaries_layer_key()` function is the fastest win — activating it gives summary cache persistence immediately without any demand infrastructure, and that can be Plan 1.
- SCC discovery should use petgraph's `tarjan_scc` (or similar) over a directed graph built from direct call target facts. Each function with a direct summary is a node; each direct call target edge is an edge. Self-calls create single-function recursive SCCs.
- Backdating is the key performance optimization: when a source file changes, its function's summary gets recomputed. If the summary digest is unchanged (common for formatting-only edits), callers don't need recomputation. This is what makes incremental scanning fast in practice.
- Phase 33 demand queries provide the hook that Phase 37 (refined calls) and Phase 38 (data flow) will use to avoid eager whole-repo execution. Getting the demand query contract right here is load-bearing for the rest of v1.2.

</specifics>

<deferred>
## Deferred Ideas

- Extension-authored providers, typed sinks, and activation levels: Phase 34.
- Framework entrypoints, lifecycle, dispatch, and trust boundaries: Phase 35.
- Type/value/place/alias substrate: Phase 36.
- Refined call graph providers using demand queries: Phase 37.
- Full interprocedural data flow with summary-projected edges: Phase 38.
- Slicing, paths, and evidence bundles using demand queries: Phase 39.
- Benchmark adapters and promotion gates: Phase 40.
- Public SDK query views and agent ergonomics: Phase 41.

</deferred>

---

*Phase: 33-demand-queries-and-summary-scc-cache*
*Context gathered: 2026-05-22*
