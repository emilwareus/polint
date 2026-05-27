# Phase 30: Direct Call Facts - Context

**Gathered:** 2026-05-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 30 delivers the first internal direct-call fact layer for Go and TypeScript/JavaScript. It should record call sites, directly resolved call targets, unresolved-call evidence, call indexes, provider/debug metadata, and fixtures for direct functions, methods, constructors, member calls, function-value calls, and unsupported dynamic forms.

This phase does not promote a public `CallGraph<'_>` or broad whole-program call graph API. Refined call graph providers such as Go CHA/RTA/VTA, TS/JS function-token flow, framework entrypoints, repo-local call models, summaries, data flow, and public query views remain later phases.

</domain>

<decisions>
## Implementation Decisions

### Internal Boundary And Provider Placement
- **D-01:** Add call facts as a crate-private `analysis::calls` layer following the existing `analysis::mir`, `analysis::places`, and `analysis::cfg` patterns. Keep call fact types, stores, validators, debug snapshots, and eval observation internal by default.
- **D-02:** Use an internal provider identity such as `polint.calls` or an equivalent manifest-owned direct-call provider. It should run after semantic MIR, symbols/references, module/topology facts, and current CFG facts are available, and before future direct summaries or refined call providers consume call facts.
- **D-03:** Do not expand legacy `FunctionFact.calls: Vec<String>` into the semantic call substrate. Existing string call hints may remain for compatibility, but Phase 30 direct calls should be modeled as new normalized internal fact families.
- **D-04:** Keep provider scheduling native and static for this phase. Do not introduce a public call-provider trait or plugin-style graph registry before native fact contracts, validation, and cache behavior exist.

### Call Fact Shape And Identity
- **D-05:** Model at minimum `CallSiteFact`, `CallTargetFact`, and unresolved-call evidence, either as a dedicated unresolved fact or as target facts with unresolved statuses. Unresolved calls must be countable, queryable, cacheable, and attributable.
- **D-06:** Reuse existing MIR call operations and `CallSiteId` where appropriate, but add any missing IDs such as `CallTargetId` as crate-private dense handles. Persistent identity belongs in stable keys and metadata, not run-local IDs.
- **D-07:** Derive call-site facts from MIR call operations and place identity. A call site should carry language, file, caller function, owning symbol when known, MIR body/op, span, call syntax kind, callee shape, receiver when known, arguments, result place, status, and stable key.
- **D-08:** Derive call-target facts from semantic references, symbols, resolved imports, module/topology context, and MIR/place evidence. A target fact should carry site, caller, target function or symbol when known, edge kind, algorithm, status, reason, provider/model provenance fields as needed, and metadata-backed precision/confidence.
- **D-09:** Stable keys should be deterministic and unrelated-file-stable. Call-site keys should include language, file stable key, caller stable key, span, callee shape, MIR operation stable key or same-span ordinal, and call kind. Target keys should include call-site stable key, algorithm, target stable key or unresolved reason, provider/schema identity, and model identity when present.
- **D-10:** Build deterministic indexes in the store, initially with `BTreeMap` or sorted vectors: sites by caller, targets by site, outgoing by function/symbol, incoming by target symbol/function, and unresolved by reason/status.

### Direct Resolution Semantics
- **D-11:** Phase 30 default resolution is direct/binding/static only. Use algorithms such as syntax-only call-site extraction, direct reference resolution, import binding, constructor/static member binding, and direct method/member binding where existing semantic references make the target precise enough.
- **D-12:** Do not implement Go CHA/RTA/VTA, TypeScript/JavaScript function-token flow, points-to, summary-assisted targets, framework dispatch, or repo-local call models in this phase. Leave those as explicit later-provider tiers.
- **D-13:** Go direct calls should cover named functions and directly resolvable concrete methods where existing symbol/reference evidence is sufficient. Interface dispatch, function values, reflection, goroutines as spawned interprocedural edges, and setup-missing package data should produce unresolved or unsupported statuses instead of placeholder targets.
- **D-14:** TypeScript/JavaScript direct calls should cover lexical/import-bound function calls, class constructors, direct static/member calls where semantic references identify the target, and CommonJS/ESM binding cases already represented by imports/references. Dynamic property calls, callable values, `eval`, proxies/getters/setters, dynamic imports, decorators, `call`/`apply`/`bind`, and framework dispatch should be unresolved/unsupported unless a precise direct reference exists.
- **D-15:** Every unsupported or dynamic form must preserve source evidence and a specific reason. Missing targets are not silent omissions; they are first-class precision/status data.

### Validation, Cache, Debug, And Evaluation
- **D-16:** Extend metadata and validation for all new call fact families. Validation should catch dangling body/op/place/function/symbol references, duplicate stable keys, invalid spans, malformed target rows, targets without matching sites, contradictory statuses, missing unresolved reasons, and provider precision-ceiling violations.
- **D-17:** Cache and output digests must include provider/schema version, source/config/lifecycle inputs, semantic MIR output digest, symbol/reference and module/topology output digests where used, upstream syntax digests, selected direct-call provider parameters, and absent extension/model/toolchain slots. Full persistent reuse may be implemented now or deferred only if deterministic output digests and future-fit keys are present.
- **D-18:** Add internal debug snapshots with counts by language, call kind, algorithm, status, unresolved reason, and provider. Snapshots must avoid raw source bodies, raw AST dumps, absolute paths, parser allocation IDs, timestamps, and run-local dense IDs as identity.
- **D-19:** Add deterministic eval fixtures for Go and TS/JS covering direct functions, methods, constructors, member calls, imported calls, function values as unresolved/unknown, unsupported dynamic calls, setup-sensitive cases, and precise statuses.
- **D-20:** Add public no-leak proof. Public CLI JSON/help, `polint inspect`, `polint test`, SDK exports, README/docs, and external temp-repo rules must not expose or advertise private call internals unless a deliberate preview/debug gate is included and documented as unstable.

### Public Capability Contract
- **D-21:** Keep public `CallGraph<'_>` and the `call_graph` capability unsupported in Phase 30. Capability diagnostics should remain honest and should not imply public call facts are available.
- **D-22:** Do not add public `docs/facts/calls.md` or SDK query docs for direct calls in this phase unless the implementation intentionally promotes a preview-gated or supported surface with external-consumer tests. Internal research/debug docs are fine.

### the agent's Discretion
- The planner may choose the exact file split, such as `analysis/calls/{mod,facts,store,direct,unresolved,cache_key,provider,validate}.rs`, as long as visibility stays crate-private.
- The planner may decide whether unresolved calls are represented as separate facts, target facts with unresolved status, or both, provided querying, debug counters, validation, and cache identity remain straightforward.
- The planner may choose whether the first implementation derives call sites and direct targets in one provider pass or two internal subpasses, provided output metadata and validation remain deterministic.
- The planner may defer persistent warm-run restoration, Go SSA static targets, TS/JS function-token flow, framework recognizers, repo model sinks, synthetic entrypoints, public `Calls<'_>`, public `CallGraph<'_>`, and benchmark precision/recall metrics if Phase 30 success criteria are satisfied honestly.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 30 goal, success criteria, research refs, and neighboring v1.2 phase boundaries.
- `.planning/REQUIREMENTS.md` - `SAE-SEM-05` requirement for direct call-site, target, unresolved-call facts, call indexes, debug snapshots, and unsupported public whole-program graph views.
- `.planning/PROJECT.md` - Product value, public API discipline, reliability, truthfulness, performance, and v1.2 substrate-first constraints.
- `.planning/STATE.md` - Current milestone state and accumulated Phase 20-29 decisions.
- `research/ROADMAP.md` - Source implementation sequence; Phase 30 maps to the direct-call facts slice after CFG and before abstract domains.

### Call Graph Research
- `research/call-graphs/FINAL-REPORT.md` - Research conclusion that call graphs are layered approximation families, unresolved calls must be first-class, and public views should wait for validation gates.
- `research/call-graphs/RECOMMENDED_IMPLEMENTATION.md` - Recommended native call-facts architecture, direct/binding default tier, provider counters, fact model, and later algorithm ladder.
- `research/call-graphs/implementation/BOOTSTRAP-INTEGRATION.md` - Revised bootstrap path: implement internal `analysis::calls` consuming MIR and `PlaceId`, with call-site/target/unresolved facts, store indexes, cache keys, debug snapshots, and delayed public SDK views.
- `research/call-graphs/STANDARD.md` - Normalized terminology, precision/status expectations, graph tiers, and future SDK shape.
- `research/call-graphs/VALIDATION.md` - Validation evidence, corrections, confidence levels, and open questions for call graph work.
- `research/call-graphs/languages/go.md` - Go-specific static, interface, RTA/VTA, lifecycle, and unresolved-call considerations.
- `research/call-graphs/languages/typescript-javascript.md` - TS/JS-specific direct binding, value-flow, dynamic call, module, and framework-call considerations.

### Upstream Phase Decisions
- `.planning/phases/29-local-cfg-and-control-dependence/29-CONTEXT.md` - CFG contracts, provider/cache/debug/eval precedent, explicit deferral of interprocedural call edges, and public no-leak requirements.
- `.planning/phases/28-private-semantic-mir-and-place-identity/28-CONTEXT.md` - Semantic MIR/place contracts, call-shaped operation evidence, direct-target deferral, unsupported semantics, cache identity, and public no-leak requirements.
- `.planning/phases/27-layered-module-package-topology-graph/27-CONTEXT.md` - Internal topology and semantic-aware import-to-package facts that can inform imported call target resolution.
- `.planning/phases/26-semantic-index-deepening/26-CONTEXT.md` - Internal semantic references/symbols, unknown/status vocabulary, stable semantic keys, generated hooks, and no broad public semantic API.
- `.planning/phases/24-persistent-layer-cache-for-existing-cheap-facts/24-CONTEXT.md` - Layer cache stale-safety, dependency indexes, deterministic payload restore, and public cache no-leak proof.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-CONTEXT.md` - Typed digest, layer/query/summary/diagnostic key vocabulary and provider output metadata.
- `.planning/phases/21-provenance-precision-and-validation-metadata/21-CONTEXT.md` - Fact metadata sidecar, stable-key merge validation, provider precision ceilings, and deterministic debug JSON.
- `.planning/phases/20-private-analysis-kernel-facade/20-CONTEXT.md` - Private kernel boundary, provider manifest discipline, and behavior-preserving provider order.

### Source Surfaces To Inspect
- `crates/polint/src/analysis/mod.rs` - Existing private analysis module boundary and current `cfg`, `mir`, and place module layout.
- `crates/polint/src/analysis/ids.rs` - Current semantic ID newtypes including `CallSiteId`; likely home or precedent for `CallTargetId`.
- `crates/polint/src/analysis/mir/op.rs` - `MirOperationKind::Call`, `MirValue::CallReturn`, `UnsupportedDomain::Calls`, and call-shaped MIR evidence.
- `crates/polint/src/analysis/mir/lower_go.rs` - Go MIR call operation lowering and deterministic call-site IDs.
- `crates/polint/src/analysis/mir/lower_ts.rs` - TS/JS MIR call operation lowering and dynamic-call evidence.
- `crates/polint/src/analysis/places.rs` - Place identity model for receiver, callee, argument, and return-place relationships.
- `crates/polint/src/analysis/cfg/facts.rs` - CFG call-site node precedent and status/precision vocabulary.
- `crates/polint/src/analysis/cfg/provider.rs` - Recent provider/digest/debug pattern for deriving and storing internal analysis facts.
- `crates/polint/src/analysis/provider.rs` - Semantic MIR provider merge, output digest, cache stats, and `AnalysisDb::replace_semantic_mir` precedent.
- `crates/polint/src/core/mod.rs` - `AnalysisDb` storage/metadata refresh patterns, `SEMANTIC_MIR_PROVIDER_ID`, `CFG_PROVIDER_ID`, and internal accessors.
- `crates/polint/src/analysis_kernel/mod.rs` - Provider execution order, provider output metadata, validation, and run report integration.
- `crates/polint/src/analysis_kernel/provider.rs` - Provider manifest schema, existing `polint.semantic_mir`/`polint.cfg` manifests, schema labels, and precision ceilings.
- `crates/polint/src/analysis_kernel/metadata.rs` - Fact family, precision, validation status, stable key, and metadata helpers to extend.
- `crates/polint/src/analysis_kernel/validation.rs` - Provider precision-ceiling and missing-metadata validation to extend.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Layer key and digest patterns, especially semantic MIR and CFG cache-key inputs.
- `crates/polint/src/analysis_kernel/debug.rs` - Test-only metadata/debug JSON patterns for adding call rows.
- `crates/polint/src/eval/model.rs`, `crates/polint/src/eval/observed.rs`, and `tests/eval-fixtures/` - Internal eval fixture categories, observed fact extraction, and existing placeholder `call_graph` precedent.
- `crates/polint/src/sdk/facts.rs`, `crates/polint/src/sdk/mod.rs`, and `crates/polint/src/analysis_plan.rs` - Existing public `CallGraph<'_>`/`call_graph` capability behavior that should remain unsupported.
- `crates/polint/src/graph/mod.rs` - Existing legacy graph helpers; do not mistake `FunctionFact.calls` or placeholder graph rendering for the new direct-call fact substrate.

### API And Visibility
- `AGENTS.md` - Public API visibility, Rust best-practice usage, rule-authoring platform contract, Go lifecycle contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.
- `docs/facts/symbols-and-references.md`, `docs/facts/imports.md`, and `docs/facts/module-graph.md` - Existing supported fact docs to keep aligned if public behavior is touched; Phase 30 should not add public call fact docs unless promotion is intentional.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `analysis::mir` already emits `MirOperationKind::Call { site, callee, arguments, return_place }` for Go and TS/JS, with deterministic `CallSiteId` values derived from source positions.
- `analysis::places` already provides crate-private place identities that can model callee, receiver, argument, and return places without leaking parser AST nodes.
- `analysis::cfg` already treats MIR calls as `CfgNodeKind::CallSite`, giving Phase 30 a recent provider/store/validation/debug/eval pattern and proof that call operations are normalized before this phase.
- `AnalysisDb::replace_semantic_mir` and `AnalysisDb::replace_cfg_facts` show the current pattern for storing private analysis facts and refreshing metadata sidecars.
- `analysis_kernel::provider`, `metadata`, `validation`, `debug`, and `incremental::keys` already provide provider manifest, precision ceiling, debug JSON, stable-key, and digest vocabulary patterns.
- `symbol_graph::semantic`, `symbol_graph::go`, and `symbol_graph::ts` provide semantic reference/symbol rows that direct target resolution should consume where available.
- Existing SDK fact exports include `CallGraph<'_>` as a reserved view, and `analysis_plan.rs` currently reports `call_graph` as unsupported. This is the public contract to preserve.

### Established Patterns
- New static-analysis internals stay `pub(crate)` and are test/eval-facing until deliberately promoted.
- Provider outputs are deterministic, sorted, metadata-backed, and validated before rules run.
- Unknown, unsupported, setup-missing, partial, dynamic, and heuristic states are explicit facts/statuses, not hidden logs or fake exact targets.
- Cache and debug payloads avoid raw source text, raw ASTs, absolute paths, timestamps, parser allocation IDs, and run-local dense IDs as persistent identity.
- Public no-leak proof uses CLI JSON/help, inspect/test JSON, docs/source-surface checks, and external temp-repo rule behavior through `polint::sdk::prelude::*` and `polint::runner::run_cli`.
- Go lifecycle constraints stay in `.polint.toml`; direct-call work must not introduce repository lifecycle side files or hidden per-analyzer config files.

### Integration Points
- Add `analysis::calls` and connect it to the private kernel provider sequence.
- Extend internal IDs, fact families, metadata refresh, validation, provider manifests, provider output reports, cache/output digests, debug JSON, and eval observation with call-site/target/unresolved rows.
- Consume existing MIR call operations and place identities as the source of call-site shape.
- Consume semantic references, symbol rows, resolved imports, and topology facts for direct target resolution.
- Preserve existing public `call_graph` unsupported capability diagnostics until a later promotion phase validates public query views.

</code_context>

<specifics>
## Specific Ideas

- Auto mode selected the research-driven default: complete internal call-site coverage, direct/binding target resolution only, and first-class unresolved-call rows.
- Public `CallGraph<'_>` remains unsupported even though internal call facts exist, because Phase 41 is the promotion point for validated SDK query views and agent ergonomics.
- Direct call facts should be useful to Phase 31 abstract domains and Phase 32 direct summaries without creating a cycle where summaries are needed to resolve direct calls.

</specifics>

<deferred>
## Deferred Ideas

- Go SSA static/RTA/VTA providers, Go interface CHA, and root-sensitive whole-program reachability belong to later refined call graph phases.
- TypeScript/JavaScript function-token flow, callback flow, `call`/`apply`/`bind`, framework router dispatch, and broader value-flow target resolution belong to later precision/refined-provider phases.
- Repo-local call graph models, framework entrypoints, synthetic targets, extension sinks, and trust-boundary dispatch edges belong to later extension/entrypoint phases.
- Public `Calls<'_>`, public `CallGraph<'_>`, docs under `docs/facts/calls.md`, and stable query builders belong to promotion phases after fixtures and benchmark gates justify them.

</deferred>

---

*Phase: 30-direct-call-facts*
*Context gathered: 2026-05-21*
