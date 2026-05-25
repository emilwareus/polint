# Phase 38: Local Plus Summary-Projected Data Flow - Context

**Gathered:** 2026-05-25
**Status:** Ready for planning
**Mode:** `$gsd-discuss-phase 38 --auto`

<domain>
## Phase Boundary

Phase 38 delivers private internal data-flow facts over the existing semantic MIR, place, CFG, direct/refined calls, direct summaries, framework trust-boundary, extension, type/value/access-path/points-to/alias, and demand-query substrates. It should add a local value-flow graph, direct-call interprocedural projection, summary-projected edges, source/sink/sanitizer/barrier model facts, explicit unknown/havoc and budget facts, cache/debug/eval support, and a bounded query-scoped path search surface for internal consumers.

This phase does **not** promote a public `DataFlow<'_>` SDK view, does not add full slicing/evidence bundles, does not add external benchmark adapters, and does not make whole-program IFDS/IDE or all-pairs source-to-sink path enumeration mandatory for normal `polint check`. Phase 39 consumes these facts for slicing, path explanation, and evidence bundles; Phase 40 measures benchmark/promotion quality; Phase 41 decides which validated query views and agent ergonomics become public.

</domain>

<decisions>
## Implementation Decisions

### Fact Family Scope and Shape

- **D-01:** Add Phase 38 as a new private `analysis::data_flow` family, not as a taint-only engine and not by widening the public SDK placeholder. Initial internal fact families should include `DataFlowNodeFact`, `DataFlowEdgeFact`, model/source/sink/sanitizer/barrier facts, unknown/havoc facts or events, and compact query/path result rows if needed for test-facing path search.
- **D-02:** Use stable keys as persistent identity and run-local dense IDs only as handles. Node and edge facts must normalize by stable key before storage, metadata assignment, debug output, eval observation, and cache digesting.
- **D-03:** Nodes should anchor to existing internal facts where possible: file, function, MIR body/op, CFG node, place, symbol/reference, call site, summary key, trust boundary, and extension/model identity. Do not introduce a parallel place, CFG, call, or summary identity system.
- **D-04:** Edges should distinguish local value edges, field/property/access-path edges, call argument binding, call return binding, receiver flow, return flow, summary-projected flow, source injection, sink reachability, sanitizer/barrier effects, additional model steps, unknown/havoc edges, and budget-truncated edges.
- **D-05:** Data-flow facts need the same metadata discipline as earlier v1.2 providers: producer id, fact family, stable key, precision/status/confidence/validation, provenance, input evidence, output digest, cache participation, and deterministic validation diagnostics.

### Local Flow Graph Semantics

- **D-06:** The first tier is local value flow over semantic MIR and places. It should cover assignments, reads/writes, parameters, receivers, locals, globals/module values, literals, temporaries, call arguments, call returns, return values, field/property/index access paths, and known call side effects where summaries already expose them.
- **D-07:** Local graph construction should be conservative around unsupported MIR operations, unknown writes, dynamic property/index access, reflection-like behavior, async/generator gaps, panics/throws/cleanup gaps, and setup-missing language facts. These cases emit explicit unknown/havoc rows rather than silently omitting flow or claiming exactness.
- **D-08:** Use the Phase 36 access-path and points-to substrate for low-depth field/property precision, but keep depth and fanout bounded. If an access path, alias answer, or points-to set is absent, unsupported, or over budget, emit an explicit unknown/budget row instead of broad exact flow.
- **D-09:** Taint is a query/domain over general value-flow facts. The core provider should build generic value-flow edges that later source/sink queries can interpret; it should not hard-code one security rule's semantics into the base graph.

### Interprocedural and Summary Projection

- **D-10:** Direct-call interprocedural edges are allowed by default where direct or accepted refined call facts bind call sites to targets. Argument-to-parameter, receiver-to-receiver, return-to-call-return, and summary event projection should preserve call-site identity and call edge provenance.
- **D-11:** Summary-projected edges consume the existing direct summary and summary SCC cache substrate. They should project TITO, return, memory-touch, call/effect, and unknown summary events into compact data-flow edges without expanding every callee path eagerly.
- **D-12:** Missing, unknown, unsupported, setup-missing, or budget-exceeded summaries must produce unknown/havoc flow rows with provenance. Do not treat missing summaries as no-flow.
- **D-13:** Avoid dependency cycles: data flow may consume direct/refined call facts and direct/closed summaries, but refined calls must not start depending on Phase 38 data-flow facts in the same phase.
- **D-14:** Whole-program IFDS/IDE, context-sensitive all-path tabulation, broad points-to refinement, and path-ranking evidence are deferred. The first global layer is summary-projected and query-scoped, not eager all-pairs reachability.

### Source, Sink, Sanitizer, Barrier, and Model Facts

- **D-15:** Source facts should initially consume Phase 35 trust boundaries and extension/model facts. HTTP path/query/body/header/cookie, MCP arguments/resource URIs, CLI args/flags/env/stdin, queue payloads, and external-return boundaries should map to data-flow source nodes with provenance and precision.
- **D-16:** Sink, sanitizer, barrier, and additional-flow-step facts should enter through validated native recognizers only where already scoped, or through the Phase 34 extension/model sink boundary. Repo-local models are additive and validation-gated; they cannot delete native facts.
- **D-17:** Sanitizers and barriers should be represented as facts/edges that queries can honor, not as destructive removal of base value-flow edges. Debug/eval should be able to show both the underlying flow and the sanitizer/barrier reason that stopped a query.
- **D-18:** Extension/model contributions need model/provider ids, binding evidence, precision ceilings, accepted/rejected status, and default-vs-extended delta reporting. Unvalidated or heuristic model facts cannot be surfaced as exact flow.
- **D-19:** Conflicting model facts, dangling bindings, invalid access paths, impossible precision claims, or unsupported source/sink declarations should produce validation diagnostics or quarantine rather than silently changing flow.

### Budgets, Unknowns, and Query-Scoped Path Search

- **D-20:** Budgeting is part of the fact contract. Track at least local edge budget, access-path depth/fanout, summary projection depth, interprocedural call depth, path search max nodes/edges/depth, and model expansion limits.
- **D-21:** Budget outcomes must be deterministic and visible through status rows such as `BudgetExceeded`, `Unknown`, `Unsupported`, `SetupMissing`, `Havoc`, or equivalent names. Truncation must never look like a clean no-flow result.
- **D-22:** Query-scoped path search should assemble paths on demand from compact local, interprocedural, summary, model, sanitizer, barrier, and unknown/havoc edges. It should return bounded paths and explicit truncation/unknown markers rather than storing every possible path.
- **D-23:** Path search should preserve enough call-site context to avoid obvious impossible argument/return paths, but full evidence rendering, slicing, ranking, SARIF/JSON evidence bundles, and diagnostic path UX belong to Phase 39.
- **D-24:** The first query API remains crate-private/test-facing. It may support source-to-sink, node-to-node, and summary-expanded path lookups for fixtures and later Phase 39 consumers, but no stable CLI/SDK contract is promoted here.

### Provider Placement, Cache, and Integration

- **D-25:** Add a provider such as `polint.data_flow` after `polint.refined_calls` and before `polint.metrics`, unless planning finds a stronger split into local/model/query providers. The provider order must remain deterministic and avoid cycles.
- **D-26:** Provider inputs should include source/config/lifecycle, symbols/references, semantic imports, MIR bodies/ops/places, CFG nodes/edges/control dependence, calls/refined calls/unresolved calls, domains, summaries/SCC cache, entrypoints/trust boundaries/dispatch, type/value/access-path/points-to/alias facts, and extension/model facts.
- **D-27:** Cache identity must include provider/schema version, upstream provider output digests, language lifecycle/config digests, rule/model/extension/tool sentinels or digests, budget/precision settings, and absent sentinels for unsupported future inputs.
- **D-28:** Store indexes should support lookup by node, edge kind, place, call site, function/body, source/sink/model id, provenance/status, and compact path query seeds. Indexes are rebuildable from facts and are not the primary persisted identity.

### Validation, Debug, Evaluation, and Public Boundary

- **D-29:** Validation must check dangling references to files/functions/MIR bodies/ops/places/CFG nodes/call sites/refined edges/summaries/trust boundaries/models, invalid spans, duplicate stable keys, malformed source/sink/sanitizer/barrier bindings, impossible precision/status combinations, missing provenance, and path/query rows that exceed declared limits without budget markers.
- **D-30:** Debug snapshots should report node/edge counts by language, edge kind, algorithm/tier, source/sink/sanitizer/barrier/model status, unknown/havoc counts, summary-projected counts, budget counts, query path truncation, and default-vs-extended deltas. They must avoid raw source bodies, absolute paths, parser object IDs, timestamps, and nondeterministic ordering.
- **D-31:** Eval fixtures must cover local flow, parameter/return flow, direct-call interprocedural flow, summary-projected flow, source/sink reachability, sanitizer and barrier behavior, missing-summary unknown/havoc behavior, extension-added flow/model facts, rejected malformed model facts, false-positive traps, cold/warm/no-cache determinism, and deterministic budget handling.
- **D-32:** Public no-leak proof must cover normal `polint check --format json`, CLI help, SDK exports, runner surface, README, and `docs/facts`. Private data-flow provider ids, internal query JSON, source/sink model schemas, and preview `DataFlow` semantics must not leak unless Phase 41 promotes them intentionally.

### The Agent's Discretion

- The planner may choose exact module layout such as `analysis/data_flow/{facts,store,local,direct_calls,summary_edges,models,query,paths,validate,debug,cache_key}.rs`, provided visibility stays crate-private.
- The planner may split execution into local facts/store, provider/cache wiring, model facts, interprocedural projection, query path search, validation/debug/eval, and public-boundary proof.
- The planner may decide whether source/sink/sanitizer/barrier facts live in one model module or separate native/extension model stores, provided provenance and validation stay explicit.
- The planner may decide exact enum names for edge kinds, algorithms, statuses, precision labels, and budget reasons, provided the vocabulary represents local, direct-call, summary, model, sanitizer/barrier, unknown/havoc, and budgeted cases.
- The planner may keep Phase 38 path search minimal if it proves bounded query-scoped reachability for fixtures and leaves rich evidence rendering to Phase 39.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 38 goal, SAE-PREC-03 mapping, research references, and success criteria.
- `.planning/REQUIREMENTS.md` — SAE-PREC-03 requirement text and v1.2 boundaries.
- `.planning/PROJECT.md` — Product boundaries, private-analysis-first milestone intent, and public API discipline.
- `.planning/STATE.md` — Current milestone state and accumulated v1.2 decisions.

### Data-Flow Research

- `research/data-flow/FINAL-REPORT.md` — Research conclusions on native data-flow, source/sink modeling, summaries, path search, precision/cost caveats, and agent-extensible modeling.
- `research/data-flow/RECOMMENDED_IMPLEMENTATION.md` — Recommended internal data-flow architecture, fact substrate, local graph, summaries, model facts, query/path engine, and accuracy reporting.
- `research/data-flow/implementation/BOOTSTRAP-INTEGRATION.md` — Revised Phase 38 bootstrap path, dependency direction, internal fact model, and public SDK deferral.

### Upstream Phase Decisions

- `.planning/phases/37-refined-call-graph-providers/37-CONTEXT.md` — Refined call edges, direct-versus-refined deltas, provider order, extension/model edge validation, and explicit deferral of data flow to Phase 38.
- `.planning/phases/36-p0-type-value-place-alias-substrate/36-CONTEXT.md` — Type/value/access-path/points-to/alias facts, alias statuses, precision ceilings, and access-path/points-to budget behavior consumed by data flow.
- `.planning/phases/35-framework-entrypoints-and-trust-boundaries/35-CONTEXT.md` — Trust boundaries, entrypoints, framework dispatch, and extension overlays that Phase 38 consumes as data-flow sources and model inputs.
- `.planning/phases/34-rust-extension-provider-sink/34-CONTEXT.md` — Repo-local extension host, typed sinks, validation, precision ceilings, quarantine, and default-vs-extended eval evidence.
- `.planning/phases/33-demand-queries-and-summary-scc-cache/33-CONTEXT.md` — Demand-query layer, summary SCC cache, extension-aware quarantine, and query trace/debug substrate.
- `.planning/phases/32-summary-kernel-and-direct-summaries/32-CONTEXT.md` — Direct summaries, TITO, memory-touch, control/call effects, summary events, and unknown summary behavior.
- `.planning/phases/31-p0-abstract-domain-kernel/31-CONTEXT.md` — Abstract-domain statuses, budgets, local solver discipline, and explicit top/unknown event pattern.
- `.planning/phases/30-direct-call-facts/30-CONTEXT.md` — Direct call-site/target/unresolved fact model and call indexes that Phase 38 projects interprocedurally.
- `.planning/phases/29-local-cfg-and-control-dependence/29-CONTEXT.md` — CFG, reachability, dominance, postdominance, control dependence, and unsupported control-flow facts.
- `.planning/phases/28-private-semantic-mir-and-place-identity/28-CONTEXT.md` — Semantic MIR operations, place identity, unknown/havoc conservative actions, and stable identity rules.

### Existing Implementation

- `crates/polint/src/analysis/mir/` — MIR body/op/value model, unknown values, unsupported semantics, and conservative actions such as havoc.
- `crates/polint/src/analysis/places.rs` — Stable place facts, roots, projections, status, and stable-key conventions.
- `crates/polint/src/analysis/cfg/` — CFG nodes/edges, control dependence, reachability, validation, and debug/eval patterns.
- `crates/polint/src/analysis/calls/` — Direct call-site/target/unresolved facts and call indexes.
- `crates/polint/src/analysis/refined_calls/` — Refined call edge facts, tiers, provider/cache/digest pattern, direct-plus-framework/type/value/summary/extension refinements, and eval fixtures.
- `crates/polint/src/analysis/summaries/` — Summary domains, TITO flow roots/kinds, summary events, SCC closure, provider/cache, validation, and debug infrastructure.
- `crates/polint/src/analysis/entrypoints/` — Entrypoint, trust boundary, framework dispatch, unresolved framework facts, and source-kind vocabulary.
- `crates/polint/src/analysis/types/`, `crates/polint/src/analysis/values/`, `crates/polint/src/analysis/access_paths/`, `crates/polint/src/analysis/points_to/`, and `crates/polint/src/analysis/aliases/` — Type/value/access-path/points-to/alias substrate for low-depth and budgeted precision.
- `crates/polint/src/analysis/demand/` — Demand context, query, trace, SCC, and quarantine support for bounded expensive analyses.
- `crates/polint/src/analysis/extensions/sinks.rs` — Extension fact candidate shape, precision/confidence/status, binding references, evidence, payload labels, and validation model for repo-local contributions.
- `crates/polint/src/analysis_kernel/provider.rs` — Provider manifest/order/schema vocabulary; Phase 38 should add `polint.data_flow` after `polint.refined_calls` unless planning intentionally splits it.
- `crates/polint/src/analysis_kernel/debug.rs`, `crates/polint/src/analysis_kernel/validation.rs`, and `crates/polint/src/eval/` — Debug, validation, eval observation, deterministic matching, fixture taxonomy, and no-leak proof patterns.
- `tests/eval-fixtures/` — Native fixture suite that Phase 38 must extend with data-flow fixtures and provider-order expectations.
- `AGENTS.md` and `docs/API-VISIBILITY-PLAN.md` — Public API visibility discipline and supported rule-author surface boundaries.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::mir` already emits operation/value evidence and conservative actions such as `HavocAffectedPlaces`; data-flow unknown/havoc rows should reuse this signal rather than invent unrelated unsupported semantics.
- `analysis::places::PlaceFact` and the Phase 36 access-path modules provide the stable identity anchors for local value-flow nodes and field/property/index precision.
- `analysis::summaries::facts` already reserves `SummaryDomainKind::DataFlowTito`, `FlowKind::{Value, BySideEffect, Taint, Barrier, Sanitizer}`, and `FlowRoot::{Param, Receiver, Return}`. Phase 38 can project these into data-flow edges.
- `analysis::entrypoints::facts::TrustBoundaryFact` provides a concrete source-kind vocabulary for HTTP, MCP, CLI, env/stdin, queue, and external-return sources.
- `analysis::refined_calls` already records accepted direct/refined edges with tiers, status, precision, provenance, confidence, and input stable keys. Data flow should consume these rather than recomputing call targets.
- `analysis::types`, `analysis::values`, `analysis::access_paths`, `analysis::points_to`, and `analysis::aliases` provide budgeted precision facts that can refine local and interprocedural flow without making broad points-to mandatory.
- `analysis::demand` already gives query/trace/quarantine scaffolding for bounded expensive work; query-scoped data-flow path search should fit this shape.
- The eval harness already has unknown and budget-exceeded status accounting plus refined-calls fixture taxonomy. Phase 38 should add data-flow area coverage rather than creating another eval path.

### Established Patterns

- New v1.2 analysis providers stay crate-private until validation and promotion phases justify public SDK/CLI exposure.
- Provider output follows extract/build -> normalize -> output digest -> store -> metadata refresh -> validate -> debug/eval.
- Cache identities include provider/schema/config/lifecycle/upstream output digests, extension/model/tool sentinels or digests, and budget/parameter digests.
- Unknown, unsupported, setup-missing, ambiguous, rejected, havoc, and budget-exceeded states are first-class facts and never hidden as no-result.
- Extension/model facts are additive, validation-gated, precision-ceiling gated, quarantine-aware, and reported with default-vs-extended deltas.
- Public no-leak tests protect `polint check` JSON/help, SDK exports, runner behavior, README, and `docs/facts` from private internal vocabulary.

### Integration Points

- Register `polint.data_flow` in `analysis_kernel::provider` and kernel execution after refined calls and before metrics, with schema such as `data-flow-facts-1`.
- Extend `AnalysisDb` with a `DataFlowStore` and replace/accessor methods for nodes, edges, model facts, unknown/havoc rows, and optional query result rows.
- Extend `FactFamily`, metadata assignment, provider output digest reporting, validation, debug JSON, eval observation, and no-leak tests for the new data-flow families.
- Add cache-key parameter structs for data-flow budgets and precision tiers, including access-path depth, interprocedural depth, summary projection depth, path search limits, and model expansion limits.
- Add eval fixtures under `tests/eval-fixtures/data-flow/` for local flow, interprocedural summary projection, source/sink/sanitizer/barrier behavior, extension model deltas, false-positive traps, and budget determinism.

</code_context>

<specifics>
## Specific Ideas

- Start with a minimal vertical: parameter -> local assignment -> return value in Go and TS/JS, producing deterministic `DataFlowNodeFact` and `DataFlowEdgeFact` rows plus debug/eval snapshots.
- Add a direct-call fixture where caller argument nodes project to callee parameter nodes and callee return projects back to call-return nodes through direct/refined call facts.
- Add a summary-projection fixture using existing `summary_tito` facts so Phase 38 proves scalable interprocedural flow without expanding all callee internals eagerly.
- Add a trust-boundary source fixture from Phase 35, such as Express request body/query or MCP tool arguments, and a model sink fixture with one sanitizer and one barrier.
- Add an extension/model fixture where a repo-local model contributes one source/sink/additional-step fact and one rejected malformed binding, proving validation, provenance, quarantine, and default-vs-extended deltas.
- Add false-positive trap fixtures: sanitized flow, barriered flow, missing summary as unknown/havoc rather than no-flow, dynamic property as unknown, and deterministic budget-exceeded path truncation.

</specifics>

<deferred>
## Deferred Ideas

- Rich slicing, thin/full slices, chops, ranked paths, summary expansion handles, SARIF/JSON evidence bundles, and diagnostic evidence rendering: Phase 39.
- External benchmark adapters, precision/recall promotion gates, memory/runtime/cache metrics, and default-vs-extension benchmark reports: Phase 40.
- Public `DataFlow<'_>` SDK view, bounded query builders, stable public JSON, and agent ergonomics: Phase 41.
- Full IFDS/IDE backend, high-k context sensitivity, broad heap/object sensitivity, all-pairs source-to-sink path materialization, and broad Python/Java data-flow parity: future work after Go and TS/JS validate the internal substrate.

</deferred>

---

*Phase: 38-local-plus-summary-projected-data-flow*
*Context gathered: 2026-05-25*
