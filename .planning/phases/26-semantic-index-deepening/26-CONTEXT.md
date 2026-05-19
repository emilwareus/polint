# Phase 26: Semantic Index Deepening - Context

**Gathered:** 2026-05-19
**Status:** Ready for planning
**Mode:** auto-selected recommended defaults

<domain>
## Phase Boundary

Phase 26 delivers `SAE-SEM-01`: deepen the semantic index with scopes, richer imports, resolution facts, aliases, generated-symbol hooks, unresolved/ambiguous rows, and stable export identities for Go and TS/JS. It should build on the current internal module graph, symbol graph, provenance metadata, layer cache, and rule inspect/test foundation while preserving existing public behavior.

This phase must not jump ahead to the Phase 27 topology graph, Phase 28 MIR/place identity, Phase 29 CFG/control dependence, Phase 30 direct call facts, extension-provider activation, broad semantic export, or public advanced SDK/query promotion. Public `Symbols<'_>` and `References<'_>` behavior may be deepened only where existing contracts require it; new fact families such as public `Scopes<'_>` or richer public `Imports<'_>` should stay internal unless the planner proves a narrow, documented promotion is necessary to satisfy Phase 26 success criteria.

</domain>

<decisions>
## Implementation Decisions

### Semantic Provider Boundary
- **D-01:** Keep semantic-index work private and provider-owned. The normal rule-author surface remains typed SDK views plus existing `polint check`, `inspect`, and `test` surfaces; do not add a public generic semantic graph API in this phase.
- **D-02:** Deepen the existing `polint.symbol_graph` path first, because it already owns symbols, definitions, references, stable keys, Go/TS language extraction, layer cache restore, and capability support. Introduce a new internal `semantic` submodule only if it reduces coupling for scopes/imports/aliases/resolution instead of becoming a second graph stack.
- **D-03:** Prefer language-owned providers for Go and TS/JS semantics that emit normalized polint facts. The shared layer should normalize identity, status, metadata, and validation; it should not force one generic lookup algorithm across both languages.
- **D-04:** Preserve the current kernel provider order unless the planner finds a low-risk reason to split semantic providers: source, Go syntax, TS syntax, module graph, symbol/semantic graph, metrics. Any split must update provider manifests, run reports, cache keys, and no-leak tests together.
- **D-05:** Existing module graph and symbol graph facts are inputs, not throwaway code. Reuse `ModuleGraphBuilder`, `SymbolGraphBuilder`, stable-key helpers, layer payload patterns, and `AnalysisDb::replace_*` restore paths wherever practical.

### Fact Model Deepening
- **D-06:** Add or deepen internal fact families for lexical/package/module scopes, declarations, richer imports/exports/reexports, aliases, resolution steps, generated/synthetic symbols, unresolved references, ambiguous candidates, and stable export identities.
- **D-07:** Every emitted symbol/reference/import/resolution row needs deterministic stable identity separate from run-local IDs. Stable keys should prefer language, repo/package/module context, file path where appropriate, declaration path or object path, namespace/kind, and generated-symbol discriminators; spans are evidence, not primary identity.
- **D-08:** Model scopes explicitly enough to support shadowing fixtures and enclosing-scope lookup. Every emitted local reference should be tied to an enclosing scope where practical, and every emitted symbol should have an owning scope or an explicit reason that scope ownership is unsupported.
- **D-09:** Keep import and export facts richer than today: Go aliases/dot/blank imports; TS/JS static imports, exports, reexports, default/namespace imports, type-only where known, CommonJS as conservative facts, and dynamic expressions as dynamic/unsupported rather than exact.
- **D-10:** Add alias/reexport closure as a bounded deterministic fixpoint or equivalent typed relation helper. Cycles must terminate with bounded diagnostics/status rows, not hang or silently drop facts.
- **D-11:** Generated symbols and generated-symbol hooks should exist as validated internal rows with provenance and stable identity. Do not implement full repo-local extension activation yet; that belongs to the extension-provider phase.

### Resolution And Unknown Handling
- **D-12:** Unknowns are data. Unresolved, ambiguous, setup-missing, unsupported, dynamic, generated, and external states must be visible in internal fixtures/debug output and, where existing public symbol/reference facts expose status, remain visible to rule authors.
- **D-13:** Reference resolution should follow an explainable ladder: lexical lookup, import/export/alias lookup, package/module lookup, language-specific member/field support where available, generated/provider hints where available, then unresolved or ambiguous. The implementation does not need to make every step exact in this phase.
- **D-14:** Capability/setup failures should stay deterministic `polint/capability` or controlled internal diagnostics with docs paths and actionable hints. Do not run rules with placeholder semantic facts when setup required for requested capabilities is missing.
- **D-15:** Family-specific precision/status fields and Phase 21 metadata must stay honest. Shared metadata should summarize provenance, precision, confidence, validation, and stable keys without making heuristic or setup-sensitive facts look exact.
- **D-16:** TS/JS should remain Oxc-backed for the first deepening pass. Use Oxc semantics for lexical symbols and references, then add richer import/export/reexport/alias handling conservatively. Do not require a TypeScript compiler sidecar in Phase 26.
- **D-17:** Go should keep using the current Go lifecycle and `go/packages`/`go/types` sidecar path for typed symbols where setup exists. Monorepo module-root behavior must follow the Go analysis lifecycle contract and never require generated repository files.

### Cache, Validation, And Fixtures
- **D-18:** Semantic layer cache keys must include provider/schema versions, source content digests, parser/language adapter versions, module/package lifecycle inputs, behavior-affecting config, upstream syntax/module output digests, extension digest placeholders, and semantic provider parameters.
- **D-19:** Stable export identities and semantic stable keys must survive deterministic cache restore. Cache payloads must store normalized facts and metadata, not raw source text, parser ASTs, absolute paths, timestamps as identity, or transient run-local IDs.
- **D-20:** Extend the internal eval/native fixture approach with semantic-index fixtures for resolved, ambiguous, unresolved, generated, alias, import/export, cross-file references, shadowing, and stable-key/cache-restore behavior.
- **D-21:** Fixture assertions should cover both strict and practical loose matching where useful: strict for stable key/file/span/role/target, loose for semantic target compatibility after formatting/parser recovery.
- **D-22:** Validation should fail closed on stable-key conflicts, referential-integrity failures, invalid spans, provider precision-ceiling violations, malformed generated facts, and cache payload/schema drift.
- **D-23:** Public compatibility tests must prove existing `polint check --format json`, SDK views, runner behavior, inspect/test surfaces, and docs do not leak internal semantic providers, metadata rows, layer cache internals, raw language-tool output, or eval schemas.

### Public Surface And Documentation
- **D-24:** Do not promote broad public semantic commands, public provider inspection, public cache/query debug commands, SCIP/Kythe export, or generic graph APIs in Phase 26.
- **D-25:** Public fact docs should be updated only where existing public `Symbols<'_>` / `References<'_>` behavior changes or where limits need clearer wording. If new fact families stay internal, document them in planning/research/eval context rather than implying user support.
- **D-26:** If the planner decides a narrow public SDK addition is unavoidable, it must include docs under `docs/facts/`, macro capability derivation, external-consumer temp-repo proof, stable JSON/no-leak tests, and explicit precision/heuristic limits.
- **D-27:** Keep examples and repo-local rule tests on `polint::sdk::prelude::*` plus `polint::runner::run_cli`; do not make examples import `core`, `symbol_graph`, `module_graph`, `analysis_kernel`, cache, eval, or parser adapters.

### the agent's Discretion
- The planner may choose whether the internal module layout remains under `symbol_graph` or introduces `crates/polint/src/semantic/` with submodules such as `scope`, `import`, `alias`, `resolution`, and `validation`.
- The planner may sequence work across multiple plans, such as semantic model/provider manifest changes, scopes/declarations, richer imports/exports/aliases, resolution ladder/unknowns, cache/validation, and fixture/docs/public-boundary proof.
- The planner may defer TS declaration merging, TS compiler-backed type resolution, exact CommonJS semantics, exact generated-code modeling, SCIP/Kythe export, xref search indexes, and public `Scopes<'_>` / richer `Imports<'_>` until later phases if Phase 26 success criteria are satisfied honestly.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 26 goal, success criteria, research refs, and neighboring phase boundaries.
- `.planning/REQUIREMENTS.md` - `SAE-SEM-01` requirement plus v1.2 out-of-scope and promotion constraints.
- `.planning/PROJECT.md` - Product value, public API discipline, truthfulness, reliability, performance, and current milestone target.
- `.planning/STATE.md` - Current milestone state and accumulated decisions; note it may lag the latest completed phase records.
- `research/ROADMAP.md` - Source implementation sequence if broader v1.2 ordering needs cross-checking.

### Semantic Index Research
- `research/semantic-index/FINAL-REPORT.md` - Executive decision for native language-owned semantic providers, stable identity, explicit unknowns, and typed SDK/export sequencing.
- `research/semantic-index/RECOMMENDED_IMPLEMENTATION.md` - Target internal semantic module shape, fact families, provider stack, stable identity layers, alias/reexport fixpoint, resolution ladder, SDK path, language order, and cache-key inputs.
- `research/semantic-index/VALIDATION.md` - Fixture taxonomy, semantic metrics, external oracle guidance, extension validation, and acceptance gates before public SDK expansion.

### Prior Phase Decisions
- `.planning/phases/12-resolved-imports-and-module-relationships/12-CONTEXT.md` - Module graph provider pattern, setup-aware import status, typed SDK boundary, and unresolved/dynamic/unsupported import handling.
- `.planning/phases/13-symbols-and-references/13-CONTEXT.md` - Symbol/reference fact model, stable IDs, precision/status fields, Go/TS extraction strategy, and external-consumer proof expectations.
- `.planning/phases/20-private-analysis-kernel-facade/20-CONTEXT.md` - Private kernel boundary, provider manifests, explicit provider order, and no public provider surface.
- `.planning/phases/21-provenance-precision-and-validation-metadata/21-CONTEXT.md` - Fact metadata sidecar, stable-key ownership, provider precision ceilings, merge validation, and debug JSON no-public-surface decision.
- `.planning/phases/22-internal-evaluation-harness-mvp/22-CONTEXT.md` - Internal eval harness, deterministic expected/observed JSON, fixture model, and no public eval CLI/schema.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-CONTEXT.md` - Input snapshots, typed layer/query/summary/diagnostic keys, provider output metadata, and cache digest vocabulary.
- `.planning/phases/24-persistent-layer-cache-for-existing-cheap-facts/24-CONTEXT.md` - Layer cache persistence, dependency indexes, stale-reuse safeguards, and public no-leak proof.
- `.planning/phases/25-rule-manifest-inspect-and-test-skeleton/25-CONTEXT.md` - Public inspect/test boundary, external-consumer rule-host proof, and no broad fact/query/debug promotion.

### Source Surfaces To Inspect
- `crates/polint/src/analysis_kernel/mod.rs` - Kernel execution order, provider output reporting, input snapshots, cache stats, and metadata validation integration.
- `crates/polint/src/analysis_kernel/provider.rs` - Current provider manifests for source, Go syntax, TS syntax, module graph, symbol graph, and metrics.
- `crates/polint/src/analysis_kernel/metadata.rs` - Shared fact metadata vocabulary and stable-key helpers from Phase 21.
- `crates/polint/src/analysis_kernel/validation.rs` - Metadata validation and provider precision-ceiling enforcement.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Layer key identity vocabulary that semantic layer cache keys should reuse.
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` - Persistent layer cache store/read/write behavior and stale/corrupt handling.
- `crates/polint/src/module_graph/model.rs` - Current resolved import and module graph builder model.
- `crates/polint/src/module_graph/go.rs` and `crates/polint/src/module_graph/ts.rs` - Language-specific import resolution inputs to reuse or deepen.
- `crates/polint/src/symbol_graph/model.rs` - Current symbol/reference builder, stable-key insertion, unresolved/ambiguous/setup-missing rows, and layer payload.
- `crates/polint/src/symbol_graph/stable_id.rs` - Stable symbol/definition/reference identity helpers.
- `crates/polint/src/symbol_graph/go.rs` - Current Go sidecar integration, lifecycle setup handling, and typed symbol/reference conversion.
- `crates/polint/src/symbol_graph/ts.rs` - Current Oxc semantic extraction, import alias handling, export-name collection, and TS reference conversion.
- `crates/polint/src/core/mod.rs` - Current public fact structs and status/precision enums for imports, module graph, symbols, definitions, and references.
- `crates/polint/src/sdk/facts.rs` - Existing typed fact-view boundary for any public behavior changes.
- `tests/eval-fixtures/` - Internal eval fixture layout to extend for semantic-index validation.
- `crates/polint/tests/cli.rs` and `crates/polint/tests/common/mod.rs` - Public no-leak, temp-repo, and external-consumer integration patterns.

### API And Visibility
- `AGENTS.md` - Public API visibility, Rust best-practice usage, rule-authoring platform contract, Go lifecycle contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.
- `docs/facts/symbols-and-references.md` - Existing public symbol/reference fact docs that may need updates if public behavior changes.
- `docs/facts/imports.md` and `docs/facts/module-graph.md` - Existing import/module docs to keep aligned with public behavior.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `AnalysisKernel::run` already builds an `InputSnapshot`, runs source/Go/TS/module/symbol/metrics providers, records provider outputs, validates metadata, and keeps `KernelRunReport` crate-private.
- `ProviderManifest` already names the current `polint.symbol_graph` provider with inputs from source files, packages, imports, resolved imports, module nodes/edges, and functions, and outputs symbols, definitions, and references.
- `SymbolGraphBuilder` already creates stable symbol/definition/reference keys, records ambiguous/unresolved/setup-missing/unsupported references, emits deterministic diagnostics, and serializes layer payloads.
- `symbol_graph::ts` already uses Oxc parser/semantic data, stable sorted symbol iteration, import alias summaries, export-name collection, and reference conversion.
- `symbol_graph::go` already uses the Go lifecycle contract and sidecar-backed `go/packages`/`go/types` extraction with setup-missing capability outputs.
- `ModuleGraphBuilder` already models resolved import rows, file/package/module/external nodes, dependency edges, and resolution status/precision.
- `analysis_kernel::metadata` and validation already provide the sidecar metadata vocabulary that new semantic facts should participate in.
- The Phase 24 layer cache already persists normalized module graph, symbol graph, and metrics payloads with dependency edges and verified reuse counters.
- The Phase 22 eval fixture layout already supports deterministic expected/observed assertions for kernel, provenance, cache, and extension-style invariants.

### Established Patterns
- New internals should be `pub(crate)` and test/eval-facing until explicitly promoted through `sdk`, `runner`, or documented CLI contracts.
- Provider outputs are deterministic vectors with sorted intermediate maps, normalized paths, no raw source-text cache payloads, and no absolute path/timestamp identity.
- Public uncertainty is preferred over fake precision: setup-missing, unresolved, ambiguous, dynamic, unsupported, heuristic, and external states should remain visible where a public fact family already exposes them.
- Language-specific setup stays in language lifecycle/config modules, especially Go module-root inference and explicit `[languages.go]` lifecycle inputs.
- Public rule-author proof uses temp repos and `polint::sdk::prelude::*`, not direct imports from internal modules.
- Public JSON/help output must not leak internal provider manifests, metadata rows, eval schemas, layer-cache internals, or parser/sidecar raw output.

### Integration Points
- Extend or split the symbol graph provider to produce scope, import/export, alias, resolution, generated-symbol, and stable export identity facts without widening public surface by default.
- Update provider manifests and layer keys if semantic subproviders become separate provider rows or schema versions.
- Attach metadata to all new facts and validate stable keys, referential integrity, span bounds, precision ceilings, and conflict behavior before facts reach rule execution.
- Extend cache payloads/restore paths so semantic stable keys and export identities survive warm runs.
- Extend eval fixtures for semantic-index taxonomy and add public compatibility/no-leak tests around existing CLI/SDK surfaces.
- Update fact docs only for supported public behavior changes and keep heuristic/setup limits explicit.

</code_context>

<specifics>
## Specific Ideas

- Auto-selected default: treat Phase 26 as the first serious semantic-index vertical slice, not as a broad public query promotion phase.
- Auto-selected default: scopes and resolution facts can be internal first; the planner should only expose public views after fixture evidence and docs are ready.
- Auto-selected default: stable export identities should be SCIP/Kythe-inspired but polint-owned internally. Do not make SCIP/Kythe the storage model or ship semantic export now.
- Auto-selected default: alias/reexport closure should be a small deterministic typed relation/fixpoint helper, not a full Datalog dependency.
- Auto-selected default: use external tools such as TypeScript/gopls as validation oracles only when useful; do not make language servers runtime dependencies for polint's native engine.

</specifics>

<deferred>
## Deferred Ideas

- Phase 27 layered module/package/topology graph, including workspace roots, package/project/source-set topology, declared requirements, lockfile/tool edges, and topology overlays.
- Phase 28 MIR/place identity, Phase 29 CFG/control dependence, and Phase 30 direct call facts.
- Real repo-local extension/provider activation, extension merge, extension quarantine, and generated semantic overlays beyond internal hooks.
- Public `Scopes<'_>`, richer public `Imports<'_>`, semantic export commands, public `polint facts`/`polint unknowns`/`polint explain`, and broad SDK query builders.
- TypeScript compiler sidecar, exact declaration merging, full CommonJS semantics, xref/name index optimization, SCIP/Kythe export, and benchmark-driven compact indexes.

</deferred>

---

*Phase: 26-semantic-index-deepening*
*Context gathered: 2026-05-19*
