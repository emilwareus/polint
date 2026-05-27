# Phase 37: Refined Call Graph Providers - Context

**Gathered:** 2026-05-24
**Status:** Ready for planning
**Mode:** `$gsd-discuss-phase 37 --auto`

<domain>
## Phase Boundary

Phase 37 delivers private opt-in refined call graph providers over the call, entrypoint, summary, extension, and type/value/place/alias substrates that now exist. It should add refined call edge facts or a clearly separated refined edge layer that consumes direct call sites/targets, framework dispatch edges, function/value/type facts, receiver type information, summaries, validated extension/model facts, and bounded points-to sets. The result should make direct-versus-refined call graph behavior measurable and traceable without promoting a public `CallGraph<'_>` or broad SDK query surface.

This phase does **not** replace the existing `polint.calls` provider, does not make whole-program points-to mandatory for normal `polint check`, does not add local-plus-summary data-flow facts, does not add slicing/evidence bundles, does not add benchmark adapters, and does not promote public advanced query views. Phase 38 consumes refined calls for data-flow, Phase 39 consumes them for evidence/path explanations, Phase 40 measures external benchmark quality, and Phase 41 decides which validated public views and agent ergonomics are supportable.

</domain>

<decisions>
## Implementation Decisions

### Provider Shape and Scope

- **D-01:** Add Phase 37 as a refined provider layer over existing `call_sites`, `call_targets`, and `unresolved_calls`; do not rewrite the direct calls provider or mutate direct edges into refined edges in place.
- **D-02:** Keep the refined layer crate-private and internal. It may add new fact families such as `RefinedCallEdgeFact`, `RefinedCallGraphNodeFact`, `RefinedUnresolvedCallFact`, `CallGraphView`, or equivalent names, but any stable public SDK/CLI view remains deferred to Phase 41.
- **D-03:** Refined edges must preserve the original `CallSiteId` and direct target relationship where applicable. They should carry a refinement source that explains whether the edge came from framework dispatch, receiver/type filtering, function-token/value facts, summary projection, points-to sets, or extension/model facts.
- **D-04:** Do not collapse direct, framework, summary-assisted, points-to, and extension/model edges into a single unlabelled graph. Every edge needs algorithm, status, precision, provenance, confidence/validation, reason, and input evidence.
- **D-05:** Provider output must be deterministic: normalize by stable keys, assign run-local dense IDs only after sorting, assign metadata, validate before use, and include output digest/cache participation consistent with prior v1.2 providers.

### Algorithm Tiers and Budgets

- **D-06:** First-tier refined providers should be opt-in or demand/query scoped, not automatically expensive for every baseline check. Direct call facts remain the default always-on call substrate.
- **D-07:** Implement cheap high-value refinements first: framework dispatch edges from Phase 35, function-object/value facts from Phase 36, receiver type/value facts, type-filtered method candidates, direct summary-assisted return/callee hints, extension-provided call model facts, and bounded points-to refinements where Phase 36 facts already exist.
- **D-08:** Go refinements should initially use existing function, symbol/reference, receiver, type/value, direct call, entrypoint, and summary facts. If planning adds Go CHA/RTA-style logic, it must be explicit about roots, module lifecycle, build tags, test inclusion, setup-missing states, and cache digest inputs.
- **D-09:** TS/JS refinements should initially use function-token/value facts, object/class/module allocation facts, import/export binding facts, framework dispatch facts, call-return facts, and bounded property/points-to information. Dynamic property keys, `eval`, proxies/accessors, `call/apply/bind`, unresolved imports, and missing setup must stay explicit unresolved or unknown rows.
- **D-10:** Budgeting is part of the fact contract. Refined providers must report `BudgetExceeded`, `SetupMissing`, `Unsupported`, `Ambiguous`, and `Unknown` statuses instead of silently truncating or promoting uncertain edges as resolved.
- **D-11:** The provider should expose tier filters internally: direct-only, direct-plus-framework, direct-plus-type/value/function-token, summary-assisted, points-to-assisted, extension/model, and all accepted refined edges. Planning may choose the exact enum names.

### Extension and Model Integration

- **D-12:** Repo-local extension/model contributions must flow through the Phase 34 typed extension sink boundary and validation/quarantine machinery. Do not trust extension-produced targets as native direct edges.
- **D-13:** Extension/model edges should use distinct provenance such as `Extension`, `Model`, or `RepoModel` and carry extension id/provider id/model id when available. Default-vs-extended eval output must show changed edges and unresolved reduction or an explicit no-change result.
- **D-14:** Extension facts can add refined target edges, framework dispatch edges, function/API summaries, no-alias/must-alias evidence, receiver/type hints, or synthetic callable targets only if validation binds them to existing stable facts or explicitly declared synthetic identities.
- **D-15:** Extension/model conflicts with native facts or with other extensions should produce validation diagnostics or quarantine rather than overriding native facts silently.
- **D-16:** Extension precision ceilings apply to refined calls. Generated-unvalidated or heuristic extension facts cannot be surfaced as exact refined edges.

### Graph Materialization and Query Shape

- **D-17:** Materialized graph views should remain internal/test-facing in Phase 37. They can support internal indexes by caller, callee, site, incoming/outgoing edges, unresolved reason, algorithm, provenance, and tier, but should not become a supported `polint::sdk` view yet.
- **D-18:** Graph materialization is a view over normalized edge facts, not the primary source of truth. The stored facts should remain small and stable; graph indexes can be rebuilt from facts.
- **D-19:** The graph view should make direct-versus-refined differences explicit, including added edges, removed/filtered candidate edges if any, unresolved calls that remain unresolved, budget-exceeded areas, and extension/model deltas.
- **D-20:** Refined providers must avoid creating dependency cycles: direct call facts feed direct summaries; direct summaries, type/value/alias facts, entrypoints, and extension facts feed refined calls; refined calls feed later data-flow and evidence phases.

### Validation, Debug, Evaluation, and Public Boundary

- **D-21:** Validation must check dangling call-site/function/symbol/body/place/type/value/allocation/points-to/entrypoint/extension references, invalid spans, duplicate stable keys, impossible precision claims, missing provenance, malformed synthetic targets, extension precision violations, and cycles in derived graph indexes.
- **D-22:** Debug snapshots should report call sites, direct edges, refined edges, graph deltas, unresolved counts, budget counts, algorithm/status/provenance distributions, and default-vs-extended differences. They must avoid raw source bodies, absolute paths, parser object IDs, timestamps, and nondeterministic ordering.
- **D-23:** Eval fixtures must include at least one direct-versus-refined comparison for Go and TS/JS, framework dispatch refinement, function-token/value refinement, receiver/type filtering, bounded points-to refinement or explicit budgeted unknown, extension/model-improved refinement, cold/warm/no-cache determinism, and public no-leak coverage.
- **D-24:** Success criteria require precision and status on every emitted refined edge, provenance for dynamic dispatch and framework edges, and explicit unresolved/budget-exceeded statuses.
- **D-25:** Public no-leak proof must cover normal `polint check --format json`, CLI help, SDK exports, runner surface, README, and docs/facts. Private provider ids, refined graph debug vocabulary, preview SDK names, and internal graph schemas must not leak unless intentionally promoted in Phase 41.

### The Agent's Discretion

- The planner may decide exact fact names and module layout, such as `analysis::refined_calls`, `analysis::call_graph`, or submodules under `analysis::calls`, provided the direct provider remains intact and visibility stays crate-private.
- The planner may split work across contracts/store, provider/cache/manifest wiring, Go refinements, TS/JS refinements, extension/model integration, graph view indexes, and validation/debug/eval/no-leak proof.
- The planner may decide whether to introduce a new provider manifest `polint.refined_calls` or fold the refined layer into a follow-on manifest near `polint.calls`, provided provider order is deterministic and avoids cycles.
- The planner may choose whether the first graph view is only test-facing debug JSON or a crate-private query API consumed by later phases, provided no public API is promoted.
- The planner may defer heavyweight Go RTA/VTA or broad points-to refinement if the phase still proves opt-in refined providers over the completed type/value/alias and framework substrates with explicit unknowns.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Roadmap and Requirements

- `.planning/ROADMAP.md` — Phase 37 goal, SAE-PREC-02 mapping, research references, and success criteria.
- `.planning/REQUIREMENTS.md` — SAE-PREC-02 requirement text and v1.2 boundaries.
- `.planning/PROJECT.md` — Product boundaries, private-analysis-first milestone intent, and public API discipline.
- `.planning/STATE.md` — Current Phase 37 focus, accumulated context, and recent Phase 36 closeout status.

### Call Graph Research

- `research/call-graphs/FINAL-REPORT.md` — Layered call graph conclusion, algorithm/cost/accuracy caveats, unresolved facts, repo-local model provenance, and research-driven defaults.
- `research/call-graphs/RECOMMENDED_IMPLEMENTATION.md` — Native call graph architecture, fact model, tier defaults, accuracy reporting, and product goals.
- `research/call-graphs/VALIDATION.md` — Source-validated claims, metrics caveats, residual uncertainty, and bootstrap-integration validation.
- `research/call-graphs/implementation/BOOTSTRAP-INTEGRATION.md` — Revised private semantic-bootstrap path, `analysis::calls` placement, dependency direction, and public-view deferral.

### Upstream Phase Decisions

- `.planning/phases/36-p0-type-value-place-alias-substrate/36-CONTEXT.md` — Type/value/place/access-path/points-to/alias facts, extension precision ceiling, alias provider stack, and explicit deferral of refined call graphs to Phase 37.
- `.planning/phases/35-framework-entrypoints-and-trust-boundaries/35-CONTEXT.md` — Entrypoints, trust boundaries, framework dispatch edges, unresolved framework facts, and extension overlay rules consumed by refined calls.
- `.planning/phases/34-rust-extension-provider-sink/34-CONTEXT.md` — Repo-local extension host, typed sinks, validation, precision ceilings, cache quarantine, and default-vs-extended eval evidence.
- `.planning/phases/33-demand-queries-and-summary-scc-cache/33-CONTEXT.md` — Demand query, summary SCC cache, extension-aware quarantine, and query trace substrate for optional expensive analysis.
- `.planning/phases/32-summary-kernel-and-direct-summaries/32-CONTEXT.md` — Direct summaries, control/call/memory/TITO facts, and summary metadata consumed by summary-assisted refined calls.
- `.planning/phases/30-direct-call-facts/30-CONTEXT.md` — Direct call-site/target/unresolved fact model that Phase 37 must layer on top of rather than replace.

### Existing Implementation

- `crates/polint/src/analysis/calls/facts.rs` — Existing `CallSiteFact`, `CallTargetFact`, `UnresolvedCallFact`, status, algorithm, precision, and provenance vocabulary.
- `crates/polint/src/analysis/calls/provider.rs` — Direct call provider extraction, direct-target resolution, unresolved filtering, output digest, and cache input pattern.
- `crates/polint/src/analysis/entrypoints/facts.rs` — Entrypoint, trust boundary, framework dispatch edge, and unresolved framework facts.
- `crates/polint/src/analysis/types/facts.rs` — Type and narrowed type facts consumed by receiver/type refinements.
- `crates/polint/src/analysis/values/facts.rs` — Function/object/class/module/call-return value facts consumed by function-token/value refinements.
- `crates/polint/src/analysis/access_paths/`, `crates/polint/src/analysis/points_to/`, and `crates/polint/src/analysis/aliases/` — Access-path, points-to, and alias facts/query stack used for bounded refined dispatch.
- `crates/polint/src/analysis/extensions/sinks.rs` — Extension fact family labels, precision/confidence/status, payload validation, and type/value/alias sink baseline.
- `crates/polint/src/analysis_kernel/provider.rs` — Provider manifest/order/schema vocabulary and current provider order through `polint.type_value_alias`.
- `crates/polint/src/analysis_kernel/debug.rs`, `crates/polint/src/analysis_kernel/validation.rs`, and `crates/polint/src/eval/` — Debug, validation, and eval patterns for new private fact families.
- `tests/eval-fixtures/` — Native fixture suite and provider-order expectations that new refined call fixtures must extend.
- `AGENTS.md` and `docs/API-VISIBILITY-PLAN.md` — Public API visibility discipline and supported rule-author surface boundaries.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets

- `analysis::calls::facts` already contains the core call vocabulary: call sites, targets, unresolved calls, `CallAlgorithm::FunctionTokenFlow`, `GoCha`, `GoRta`, `GoVta`, `PointsTo`, `SummaryAssisted`, `FrameworkModel`, `RepoModel`, and explicit unresolved reasons.
- `analysis::calls::provider` already normalizes direct call output and digests source/config/lifecycle/upstream provider inputs. Refined calls should copy this digest discipline rather than invent another cache model.
- `analysis::entrypoints::facts::FrameworkDispatchEdgeFact` already models synthetic framework dispatch edges that Phase 37 can translate or project into refined call edges.
- `analysis::types`, `analysis::values`, `analysis::access_paths`, `analysis::points_to`, and `analysis::aliases` provide the precision substrate Phase 37 needs for receiver filtering, function-token flow, bounded points-to, and alias-aware uncertainty.
- `analysis::extensions::sinks` already validates type/value/alias extension fact families and preserves extension precision/status/provenance. Refined call model facts should follow the same shape.
- `analysis_kernel::provider` currently orders `polint.type_value_alias` after extensions and before metrics. A new refined-call provider likely belongs after `polint.type_value_alias` and before metrics, unless planning finds a stronger split.

### Established Patterns

- New v1.2 analysis families stay crate-private until validated and deliberately promoted.
- Provider output follows extract/refine -> normalize -> output digest -> store -> metadata refresh -> validate -> debug/eval.
- Cache identities include provider/schema/config/lifecycle/upstream output digests, model/extension/tool sentinels, and provider parameters/budgets.
- Unknown, unsupported, setup-missing, ambiguous, rejected, and budget-exceeded states are first-class facts.
- Public no-leak tests protect normal CLI JSON/help, SDK exports, runner behavior, README, and docs/facts from private analysis vocabulary.
- Extension-influenced facts must be distinguishable, precision-ceiling gated, and quarantine-aware.

### Integration Points

- Add or extend provider manifests in `analysis_kernel::provider` with refined-call inputs after `call_sites`, `call_targets`, `unresolved_calls`, `summary_*`, `entrypoints`, `dispatch_edges`, `type_facts`, `value_facts`, `access_paths`, `points_to_sets`, `alias_answers`, and extension facts.
- Add store/index support in `AnalysisDb` for refined edge rows and optional graph views, keeping graph indexes rebuildable from facts.
- Extend validation with refined-call reference checks and precision/status/provenance rules.
- Extend debug JSON and eval observation with direct-versus-refined edge deltas, unresolved reductions, algorithm/status/provenance counts, and budget reporting.
- Add fixtures under `tests/eval-fixtures/` that exercise Go, TS/JS, framework dispatch, extension/model refinement, and cold/warm/no-cache determinism.

</code_context>

<specifics>
## Specific Ideas

- Start with a vertical refined edge that maps Phase 35 framework dispatch edges onto call-site/target facts with `FrameworkModel` provenance and explicit trigger metadata.
- Add a TS/JS function-token fixture where a function value is assigned, passed, or returned, and a previously unresolved function-value call becomes a refined ambiguous/resolved edge.
- Add a Go receiver/interface fixture where receiver type/value evidence narrows candidate method targets while unresolved or setup-missing interface dispatch remains explicit.
- Use Phase 36 points-to sets only when already produced within budget. If the set is absent, over budget, or unsupported, emit `BudgetExceeded` or `Unknown` refined rows rather than falling back to broad exactness.
- Add an extension/model fixture that contributes one validated refined call target and one rejected malformed target, proving default-vs-extended edge delta and quarantine/validation behavior.
- Keep internal graph view filters simple at first: by caller, callee, site, status, algorithm, provenance, and tier.

</specifics>

<deferred>
## Deferred Ideas

- Local plus summary-projected data-flow facts, source/sink/sanitizer/barrier model sinks, budgets, unknown/havoc facts, and query-scoped path search: Phase 38.
- Slicing, ranked paths, evidence bundles, summary expansion handles, and diagnostic evidence rendering: Phase 39.
- External benchmark adapters and promotion gates for precision claims: Phase 40.
- Public `Calls<'_>` / `CallGraph<'_>` SDK query views, bounded query builders, stable public JSON, and agent ergonomics: Phase 41.
- Broad Python/Java call graph parity and heavyweight context-sensitive points-to algorithms: future milestone after Go and TS/JS prove the model.

</deferred>

---

*Phase: 37-refined-call-graph-providers*
*Context gathered: 2026-05-24*
