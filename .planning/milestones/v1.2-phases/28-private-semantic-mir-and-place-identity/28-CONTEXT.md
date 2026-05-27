# Phase 28: Private Semantic MIR and Place Identity - Context

**Gathered:** 2026-05-20
**Status:** Ready for planning
**Mode:** auto-selected recommended defaults

<domain>
## Phase Boundary

Phase 28 delivers `SAE-SEM-03`: a private semantic MIR and normalized place identity for Go and TS/JS function bodies. It should create the first owned operation/place layer that later CFG, direct-call, abstract-domain, summary, and data-flow phases can consume without reading parser ASTs.

This phase must not promote public MIR, CFG, call graph, data-flow, or broad query APIs. It must not implement Phase 29 dominance/control-dependence, Phase 30 direct call target facts, Phase 31 abstract domains, summary solving, extension loading, or public SDK query ergonomics. Existing `polint check`, `polint inspect rule`, `polint test`, `Symbols<'_>`, `References<'_>`, `ResolvedImports<'_>`, and `ModuleGraphFacts<'_>` behavior should remain compatible unless a narrow internal bug fix is required.

</domain>

<decisions>
## Implementation Decisions

### Internal Boundary And Provider Placement
- **D-01:** Add a new crate-private semantic analysis boundary, preferably `crates/polint/src/analysis/`, with `analysis::mir` and `analysis::places` as the Phase 28 focus. Register it from `lib.rs` as `pub(crate) mod analysis;` only. Do not expose it through `polint::sdk`, `polint::runner`, crate-root public exports, public CLI help, public JSON, or docs as a supported user feature.
- **D-02:** Keep Phase 28 provider behavior private and manifest-owned. A new internal provider such as `polint.semantic_mir` or an equivalent analysis provider should run after symbol and topology facts are available, and before later consumers need MIR. If the planner keeps it test/eval-triggered first, it must still update provider metadata, run reports, and validation paths enough that downstream phases can connect without another identity rewrite.
- **D-03:** Prefer a `SemanticStore` or `AnalysisSession` owned behind internal APIs instead of stretching `core::AnalysisDb` into the semantic engine. `AnalysisDb` may own or reference semantic artifacts only if cache/reporting integration remains simple and public SDK surfaces stay unchanged.
- **D-04:** Reuse existing internal patterns: small `Copy` ID newtypes, stable-key helpers, sidecar metadata, deterministic builders, normalized payloads, `AnalysisDb::replace_*` style restore paths, and private eval/no-leak tests.
- **D-05:** Do not make parser AST nodes, tree-sitter nodes, Oxc AST lifetimes, raw language-tool objects, or absolute source roots part of MIR or place identity. Lowering functions may use parser data locally, then emit polint-owned facts.

### MIR Shape And Lowering Subset
- **D-06:** Start with a small deterministic MIR slice for every discovered Go and TS/JS function body that has a known body. The first slice should cover declarations/binds, assignment/read/write, literals, identifiers, member/property access, index access, branches/conditions, returns, and call-shaped operations.
- **D-07:** Keep Phase 28 MIR language-normalized but not over-generalized. It should be rich enough for Phase 29 CFG and Phase 31 domains to build on, but it should not try to solve full type/value/alias precision, full interprocedural calls, framework dispatch, or language parity beyond Go and TS/JS.
- **D-08:** Represent bodies with stable body IDs, ordered operation/statement rows, source spans, owner function/package/module context, and explicit terminator or control-shape rows where needed for deterministic snapshots. Full CFG edges, dominance, postdominance, and control dependence belong to Phase 29.
- **D-09:** Calls should appear in MIR only as call-shaped operations and call-return place evidence needed for place identity. Direct target resolution, unresolved-call fact families, call indexes, and public call graph behavior remain Phase 30.
- **D-10:** Lowering must be deterministic across repeated runs: stable input order, canonical sort keys, deterministic temporary ordinals, deterministic stable-key construction, and no dependence on hash map iteration, parser allocation IDs, temp directories, or wall-clock data.

### Place Identity Model
- **D-11:** Use language-normalized access-path keys. Place roots should cover locals, parameters, globals, temporaries, call returns, and unknown roots. Projections should cover fields/properties, indexes where known, deref-like access where applicable, await/call-return projections where needed, and unknown projections when precision is not available.
- **D-12:** Dense `PlaceId` values are run-local handles only. Persistent identity should be stable `PlaceKey` strings or typed stable keys derived from language, file/function/symbol owner, root kind, root name or parameter index, deterministic temporary ordinal, projection sequence, and source evidence where needed.
- **D-13:** Do not encode alias, points-to, heap abstraction, or refined receiver precision directly into `PlaceKey`. Later Phase 36 type/value/place/alias facts should refine relationships between places without invalidating the Phase 28 identity contract.
- **D-14:** Place identity should distinguish declaration binding, overwrite, partial write, simultaneous assignment, mutation through projection, and unknown writes where practical. Go multi-assign and TS/JS destructuring can start conservative, but unsupported semantics must be explicit.
- **D-15:** Unknown roots and unsupported projections are valid data, not failure cases. They should be stable, status-labeled, and usable by downstream domains as conservative havoc/unknown inputs.

### Unsupported Semantics And Uncertainty
- **D-16:** Unsupported lowering must become explicit unsupported facts or controlled diagnostics, never silent omissions. Each unsupported row should include construct kind, source evidence, affected places when known, affected future domains when obvious, conservative action, precision/status downgrade, and producer metadata.
- **D-17:** Dynamic or setup-sensitive constructs should stay honest: JS/TS `eval`, proxies, getters/setters, dynamic property keys, optional chaining gaps, async/await gaps, complex destructuring, CommonJS edges, Go reflection, `unsafe`, goroutines, channels, deferred/panic/recover complexity, and parser recovery should become unsupported/unknown/status rows as appropriate.
- **D-18:** Parser errors and lowering gaps must not crash normal analysis. They should produce existing parser diagnostics, capability/setup diagnostics, or internal MIR unsupported diagnostics depending on the source of the problem.
- **D-19:** Precision should never be overstated. Semantic MIR rows produced from syntax-only evidence should not claim exact semantic coverage where language dynamics or missing setup make that untrue.

### Validation, Cache, And Fixtures
- **D-20:** Attach Phase 21 metadata to all new MIR/place families: producer/layer id, stable key, precision, validation status, confidence, source evidence, and deterministic payload digest participation.
- **D-21:** Add validation for duplicate stable keys, invalid spans, missing owner references, dangling place/body references, malformed projection chains, unsupported rows without evidence, provider precision-ceiling violations, and cache payload/schema drift once persisted.
- **D-22:** Cache identity should be future-fit even if Phase 28 does not fully persist every MIR artifact. Keys should include provider/schema versions, source digest, language lifecycle, config digest, analysis-plan/provider parameters, upstream syntax output digests, semantic/topology output digests where used, and absent extension/model/tool slots.
- **D-23:** Add deterministic snapshots through crate-private tests or the internal eval harness for at least one Go fixture and one TS/JS fixture. Fixtures should assert MIR/place shape directly, not only later domain outcomes.
- **D-24:** Public compatibility proof is required: public CLI JSON, help text, SDK exports, docs, and temp-repo rule behavior must not leak `analysis::mir`, `analysis::places`, `SemanticStore`, provider internals, raw parser output, cache internals, or eval schemas.

### the agent's Discretion
- The planner may choose exact module names and file split, such as `analysis/ids.rs`, `analysis/store.rs`, `analysis/stable_key.rs`, `analysis/provider.rs`, `analysis/validate.rs`, `analysis/mir/{body,op,lower_go,lower_ts}.rs`, and `analysis/places.rs`.
- The planner may decide whether MIR/place facts are stored in `AnalysisDb` sidecars, a `SemanticStore`, or an `AnalysisSession`, as long as deterministic validation, future cache identity, and downstream provider access stay straightforward.
- The planner may split implementation into multiple plans, such as: internal IDs/store/provider manifest, MIR model and Go lowering, TS/JS lowering, place table/stable keys, unsupported semantics/validation, eval/cache/no-leak proof.
- The planner may defer exact destructuring, exact optional chaining, Go `defer`/panic/recover semantics, async rejection paths, callback scheduling, closure allocation identity, heap/alias facts, direct target resolution, and public query views if Phase 28 success criteria are satisfied honestly.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 28 goal, success criteria, research refs, neighboring phase boundaries, and v1.2 milestone guardrails.
- `.planning/REQUIREMENTS.md` - `SAE-SEM-03` requirement plus v1.2 out-of-scope and promotion constraints.
- `.planning/PROJECT.md` - Product value, public API discipline, truthfulness, reliability, performance, and current milestone target.
- `.planning/STATE.md` - Current milestone state, accumulated decisions, and note that Phase 28 is ready for planning/execution.
- `research/ROADMAP.md` - Source implementation sequence; Phase 28 maps to research PR 9, after semantic index and topology and before CFG/calls/domains.

### MIR And Bootstrap Research
- `research/implementation-bootstrap/FINAL-REPORT.md` - Main decision to build a private deterministic typed semantic kernel and the first vertical slice order.
- `research/implementation-bootstrap/RECOMMENDED_IMPLEMENTATION.md` - Recommended `analysis` module layout, ID/store/provider/MIR/place/cache guidance, sequencing, and non-goals.
- `research/abstract-interpretation/implementation/MIR-CONTRACT.md` - Minimum MIR contract needed by future CFG/domains, including IDs, body shape, statements, terminators, expression facts, unsupported semantics, and validation expectations.

### Prior Phase Decisions
- `.planning/phases/20-private-analysis-kernel-facade/20-CONTEXT.md` - Private kernel boundary, provider manifests, behavior-preserving provider order, and no public provider surface.
- `.planning/phases/21-provenance-precision-and-validation-metadata/21-CONTEXT.md` - Fact metadata sidecar, stable-key ownership, provider precision ceilings, merge validation, and debug JSON no-public-surface decision.
- `.planning/phases/22-internal-evaluation-harness-mvp/22-CONTEXT.md` - Internal eval harness, deterministic expected/observed JSON, fixture model, and no public eval CLI/schema.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-CONTEXT.md` - Input snapshots, typed layer/query/summary/diagnostic keys, provider output metadata, and cache digest vocabulary.
- `.planning/phases/24-persistent-layer-cache-for-existing-cheap-facts/24-CONTEXT.md` - Layer cache persistence, dependency indexes, stale-reuse safeguards, and public no-leak proof.
- `.planning/phases/25-rule-manifest-inspect-and-test-skeleton/25-CONTEXT.md` - Public inspect/test boundary, external-consumer rule-host proof, and no broad fact/query/debug promotion.
- `.planning/phases/26-semantic-index-deepening/26-CONTEXT.md` - Internal semantic provider boundary, stable semantic keys, explicit unknowns, semantic import/export/resolution rows, and no public broad semantic API.
- `.planning/phases/27-layered-module-package-topology-graph/27-CONTEXT.md` - Internal topology layers, import-to-package facts, cache participation, and explicit deferral of MIR/place identity to Phase 28.

### Source Surfaces To Inspect
- `crates/polint/src/lib.rs` - Current public/private module boundary and place to add a crate-private `analysis` module.
- `crates/polint/src/analysis_kernel/mod.rs` - Current provider execution order, run report, input snapshot, provider output metadata, and validation integration.
- `crates/polint/src/analysis_kernel/provider.rs` - Provider manifest structure, provider order tests, schema labels, cache policies, and existing provider inputs/outputs.
- `crates/polint/src/analysis_kernel/metadata.rs` - Shared fact metadata vocabulary and stable-key helpers.
- `crates/polint/src/analysis_kernel/validation.rs` - Metadata validation and provider precision-ceiling enforcement to extend for MIR/place facts.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Layer/query/summary/diagnostic key vocabulary and semantic/topology cache-key patterns.
- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` - Source/config/lifecycle/toolchain/provider input components that MIR/cache keys should consume.
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` - Persistent layer cache store/read/write behavior and stale/corrupt handling.
- `crates/polint/src/core/mod.rs` - Current `AnalysisDb`, IDs, spans, language enum, functions, symbols, references, imports, and public fact structs.
- `crates/polint/src/go/adapter.rs` - Go syntax extraction and function body source/span inputs available before semantic MIR.
- `crates/polint/src/ts/adapter.rs` - TS/JS syntax extraction and function/source facts available before semantic MIR.
- `crates/polint/src/symbol_graph/model.rs` - Stable-key builder, deterministic symbol/reference storage, collision handling, and payload patterns to emulate.
- `crates/polint/src/symbol_graph/stable_id.rs` - Stable identity helpers for symbol/reference facts.
- `crates/polint/src/symbol_graph/go.rs` - Go sidecar/lifecycle setup handling and typed symbol/reference conversion that can inform MIR owner identity.
- `crates/polint/src/symbol_graph/ts.rs` - Oxc-backed semantic extraction, node/scoping usage, and language-specific sorting patterns.
- `crates/polint/src/symbol_graph/semantic.rs` - Phase 26 semantic rows, statuses, metadata, stable exports, and internal debug/eval helpers.
- `crates/polint/src/module_graph/topology.rs` and `crates/polint/src/module_graph/mod.rs` - Phase 27 topology facts and import-to-package context that may become MIR owner/package context.
- `tests/eval-fixtures/` - Internal eval fixture layout to extend for MIR/place snapshots.
- `crates/polint/tests/cli.rs` and `crates/polint/tests/common/mod.rs` - Public no-leak, temp-repo, cache, and external-consumer integration patterns.

### API And Visibility
- `AGENTS.md` - Public API visibility, Rust best-practice usage, rule-authoring platform contract, Go lifecycle contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.
- `docs/facts/symbols-and-references.md`, `docs/facts/imports.md`, and `docs/facts/module-graph.md` - Existing supported fact docs to keep aligned if public behavior is touched; Phase 28 should not add public MIR docs.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AnalysisKernel::run` already constructs an `InputSnapshot`, executes providers, records `ProviderOutputMeta`, aggregates diagnostics, validates metadata, and returns a crate-private `KernelRunReport`.
- `ProviderManifest` already has provider ids, inputs, outputs, schema versions, language scope, precision ceilings, and cache policy; this is the natural place to describe a private MIR/place provider.
- `analysis_kernel::metadata` and `validation` already provide FactFamily/stable-key/precision/validation machinery that MIR/place facts should join.
- `analysis_kernel::incremental` already provides `Digest`, `LayerKey`, `InputSnapshot`, `ProviderOutputMeta`, `CacheStats`, dependency indexes, invalidation, and layer-cache primitives.
- `symbol_graph::model` and `symbol_graph::stable_id` are the strongest local precedent for deterministic builders, stable keys, collision diagnostics, sorted output, and normalized payloads.
- `symbol_graph::ts` already uses Oxc parser/semantic data locally and emits normalized polint facts; Phase 28 should use the same no-AST-leak shape.
- `symbol_graph::go` already follows the Go lifecycle contract and setup-missing behavior; MIR lowering should preserve that monorepo/setup discipline.
- `module_graph::topology` and Phase 27 import-to-package facts provide package/source-set context that can help owner identity but should not become a public dependency.
- The eval harness and CLI tests already prove private internals through deterministic fixture observations and public no-leak checks.

### Established Patterns
- New internals stay `pub(crate)` and test/eval-facing until deliberately promoted.
- Provider output is deterministic, sorted, normalized, metadata-backed, and validated before rules run.
- Source payloads and cache payloads avoid raw source text, raw ASTs, absolute paths, timestamps as identity, and transient run-local IDs.
- Unknown/setup-missing/unsupported/dynamic/ambiguous states are explicit data, not hidden logs or fake exactness.
- Public rule-author proof uses `polint::sdk::prelude::*` and `polint::runner::run_cli`, never internal module imports.
- Existing public CLI, SDK, docs, and JSON must not leak internal provider names, metadata rows, eval schemas, cache internals, parser internals, or new MIR/place types.

### Integration Points
- Add a crate-private `analysis` module and internal semantic store/session APIs.
- Add MIR/place provider metadata and output summaries to the private kernel path or private test/eval provider path.
- Extend `FactFamily`/metadata/validation with MIR body, MIR operation/statement/terminator, place, and unsupported-semantic families where needed.
- Extend cache key vocabulary for MIR/place artifacts, even if persistence begins as internal snapshots rather than full warm-run reuse.
- Add Go and TS/JS lowering modules that consume existing file/function/symbol/topology facts and parser outputs without leaking AST lifetimes.
- Add eval fixtures and no-leak tests proving deterministic snapshots and public boundary preservation.

</code_context>

<specifics>
## Specific Ideas

- Auto-selected default: Phase 28 is the semantic bootstrap, not public query promotion.
- Auto-selected default: implement the first useful MIR/place slice for both Go and TS/JS instead of going deep on one language and leaving the other without a contract.
- Auto-selected default: keep MIR sufficiently shaped for later CFG/domains but do not implement full CFG or direct call target facts here.
- Auto-selected default: stable place identity should be access-path based and conservative; alias/points-to precision is a later layer.
- Auto-selected default: unsupported semantics should be visible in snapshots and metadata, because later analyses need to know where precision was intentionally lowered.

</specifics>

<deferred>
## Deferred Ideas

- Phase 29 local CFG, reachability, dominance, postdominance, control dependence, full exceptional/control edge modeling, and path-sensitive control facts.
- Phase 30 direct call-site target facts, unresolved-call facts, direct/static resolution, call indexes, and any public call graph discussion.
- Phase 31 P0 abstract-domain kernel and domain-law tests over MIR/CFG.
- Later summaries, demand queries, extension sinks, framework entrypoints, type/value/place/alias refinement, local/interprocedural data flow, slicing, evidence bundles, benchmark gates, and public SDK query views.
- Public `Mir<'_>`, `Places<'_>`, `Cfg<'_>`, `CallGraph<'_>`, `DataFlow<'_>`, `polint facts`, `polint mir`, `polint explain`, or other broad advanced analysis commands.

</deferred>

---

*Phase: 28-private-semantic-mir-and-place-identity*
*Context gathered: 2026-05-20*
