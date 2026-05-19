# Phase 27: Layered Module/Package/Topology Graph - Context

**Gathered:** 2026-05-19
**Status:** Ready for planning
**Mode:** auto-selected recommended defaults

<domain>
## Phase Boundary

Phase 27 delivers `SAE-SEM-02`: expand module topology into workspace roots, packages/projects, source sets, declared requirements, lockfile/tool-resolved edges, import-to-package facts, and repo topology overlays for Go and TS/JS.

This phase should deepen the existing internal `polint.module_graph` substrate and its cache identity. It must not jump ahead to Phase 28 MIR/place identity, Phase 29 CFG/control dependence, Phase 30 direct call facts, extension-provider activation, broad topology query builders, or public SDK promotion of new topology views. Existing public `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` behavior must keep working unless a narrow, documented compatibility-preserving change is necessary.

</domain>

<decisions>
## Implementation Decisions

### Provider Boundary
- **D-01:** Deepen the existing internal `polint.module_graph` provider first. Keep the provider order behavior-preserving: source, Go syntax, TS/JS syntax, module graph, symbol graph, metrics, unless the planner finds a low-risk reason to split topology subproviders and updates manifests, cache keys, run reports, and tests together.
- **D-02:** Model topology as layered facts, not one generic graph. The internal layers should distinguish workspace roots, packages/projects, source sets, declared requirements, resolved dependency edges, import-to-package edges, and overlays.
- **D-03:** Keep new topology types crate-private/test/eval-facing by default. Do not expose package-manager internals, provider manifests, raw resolver output, cache internals, or broad graph APIs through `polint::sdk`, `polint::runner`, crate-root exports, public CLI JSON, or docs.

### Root And Package Model
- **D-04:** Root discovery is product-critical and should fail closed. Prefer explicit files and lifecycle config first, nearest-root discovery second, and heuristics only with explicit lower precision/status.
- **D-05:** Go monorepo support should follow the Go lifecycle contract: infer nearest `go.mod` roots, honor `[languages.go].module_roots`, support multi-module workspaces without writing generated repo files, and use temporary internal workspaces when needed.
- **D-06:** TS/JS package and workspace facts should recognize package manager signals deterministically: `package.json`, `packageManager`, `workspaces`, pnpm/Yarn/Bun workspace files, lockfiles, and tsconfig project/reference inputs where available.
- **D-07:** Source sets should be first-class internal facts for source, test, generated, vendor, and external contexts where known. Unknown generated/vendor/test status should remain explicit rather than being folded into normal source.

### Dependency Layers
- **D-08:** Keep declared requirements, selected/resolved dependency edges, and actual import usage separate. Do not collapse them into one `DependsOn` edge or treat a declared dependency as proof that source imports use it.
- **D-09:** Start with Go and TS/JS because Phase 27 is scoped to those adapters. Parse static manifests and lockfile/tool evidence natively where practical, but do not make package-manager execution or network access mandatory for normal scans.
- **D-10:** Lockfile/tool edges must record their source, schema/version where known, precision/status, and lifecycle inputs. Missing or stale lockfiles should produce explicit facts/diagnostics instead of fake exactness.
- **D-11:** Per-edge precision matters. The same package can have exact lockfile evidence, setup-aware resolver evidence, heuristic import evidence, or unknown/unsupported status in the same run.

### Import-To-Package Classification
- **D-12:** Add explicit internal import-to-package facts that bridge syntax imports, Phase 26 semantic import rows where useful, package/project/source-set ownership, and external package nodes.
- **D-13:** Import-to-package facts should distinguish source, test, generated, vendor, and external edges where known, with explicit unresolved, setup-missing, unsupported, dynamic, ambiguous, undeclared, or outside-workspace states.
- **D-14:** Preserve existing `ResolvedImportFact` and `ModuleNode` behavior for current SDK consumers. New richer topology rows can inform them internally, but public compatibility must be proven through temp-repo and no-leak tests.
- **D-15:** TS/JS resolution should remain Oxc/`oxc_resolver` backed for current import resolution, then add package/workspace ownership, package `exports`/`imports` awareness, tsconfig paths/project references, and package-manager layout facts conservatively.
- **D-16:** Go resolution should reuse the current Go lifecycle and sidecar package metadata, then add module-root/package/source-set/dependency facts without requiring a repository-root `go.mod`.

### Cache, Metadata, And Validation
- **D-17:** Topology facts must participate in Phase 21 metadata and Phase 24 layer cache identity. Cache keys should include provider/schema versions, source/import shapes, package/root/source-set facts, manifests/lockfiles/config, Go lifecycle, TS/JS lifecycle, upstream syntax/semantic output digests where used, and absent extension/toolchain placeholders.
- **D-18:** Cache payloads must store normalized topology facts and metadata, not raw source text, raw ASTs, absolute paths, timestamps as identity, package-manager private blobs, or transient run-local IDs.
- **D-19:** Validation should fail closed on invalid roots, path escapes, malformed manifests/lockfiles, referential-integrity failures, stable-key conflicts, precision-ceiling violations, unsupported dynamic build logic claiming exactness, and cache payload/schema drift.
- **D-20:** Extend internal eval/native fixtures for roots, packages, source sets, declared requirements, lockfile/tool edges, import-to-package classification, Go monorepos, TS/JS workspaces, cache invalidation, and public no-leak behavior.

### Topology Overlays And Future Extension Shape
- **D-21:** Repo topology overlays should be modeled as a private layer for ownership, architecture layers, deploy units, generated zones, test-only visibility, internal/public API boundaries, and source-of-truth directories.
- **D-22:** Phase 27 may design overlay facts and config-shaped inputs but should not implement real repo-local extension/provider activation or extension merge/quarantine semantics. Those belong to Phase 34 and related later phases.
- **D-23:** Overlay and package-manager uncertainty should be actionable data for agents and future extensions, not hidden logs. Examples include ambiguous workspace root, missing lockfile, undeclared dependency, generated source unknown, unsupported dynamic build script, and unresolved import target.

### Public Surface And Documentation
- **D-24:** Do not promote public `Packages<'_>`, `Dependencies<'_>`, `SourceSets<'_>`, `RepoTopology<'_>`, `polint facts`, `polint topology`, or broad query-builder APIs in Phase 27.
- **D-25:** Update public docs only where existing public resolved-import/module-graph behavior changes or limits need clearer wording. Internal topology docs and eval fixtures can describe new fact families without implying user support.
- **D-26:** If the planner decides a narrow public SDK addition is unavoidable, it must include docs under `docs/facts/`, macro capability derivation, external-consumer temp-repo proof, stable JSON/no-leak tests, and explicit precision/setup/heuristic limits.

### the agent's Discretion
- The planner may decide whether topology layers live inside the existing `module_graph` files or new internal submodules such as `discovery`, `manifest`, `lockfile`, `requirements`, `source_set`, `topology`, `validation`, and `cache_key`.
- The planner may split the phase into multiple plans, for example: internal fact/schema/provider manifest, Go module-root topology, TS/JS workspace/package topology, declared/resolved dependency layers, import-to-package classification, cache/validation/eval, and public-boundary/docs proof.
- The planner may defer Python, Java/JVM, Cargo, Maven/Gradle, Nx/Turborepo, exact Yarn PnP, exact npm/pnpm peer-context modeling, exact CommonJS semantics, dynamic build-tool execution, and public topology SDK/query views if Phase 27 success criteria are satisfied honestly for Go and TS/JS.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 27 goal, success criteria, research refs, and neighboring phase boundaries.
- `.planning/REQUIREMENTS.md` - `SAE-SEM-02` requirement plus v1.2 out-of-scope and promotion constraints.
- `.planning/PROJECT.md` - Product value, public API discipline, truthfulness, reliability, performance, and current milestone target.
- `.planning/STATE.md` - Current milestone state and accumulated decisions.
- `research/ROADMAP.md` - Source implementation sequence if broader v1.2 ordering needs cross-checking.

### Module Graph Research
- `research/module-graph/FINAL-REPORT.md` - Executive decision for layered topology facts, ecosystem-specific providers, explicit uncertainty, and private-first SDK promotion.
- `research/module-graph/RECOMMENDED_IMPLEMENTATION.md` - Recommended internal module layout, fact families, provider pipeline, Go/TS implementation phases, extension shape, and acceptance criteria.
- `research/module-graph/VALIDATION.md` - Parser/fact/tool/cache/extension validation layers, accuracy metrics, and regression gates.

### Prior Phase Decisions
- `.planning/phases/12-resolved-imports-and-module-relationships/12-CONTEXT.md` - Existing module graph provider, setup-aware resolved imports, SDK boundary, and unresolved/dynamic/unsupported status discipline.
- `.planning/phases/20-private-analysis-kernel-facade/20-CONTEXT.md` - Private kernel boundary, provider manifests, explicit provider order, and no public provider surface.
- `.planning/phases/21-provenance-precision-and-validation-metadata/21-CONTEXT.md` - Fact metadata sidecar, stable-key ownership, provider precision ceilings, merge validation, and debug JSON no-public-surface decision.
- `.planning/phases/22-internal-evaluation-harness-mvp/22-CONTEXT.md` - Internal eval harness, deterministic expected/observed JSON, fixture model, and no public eval CLI/schema.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-CONTEXT.md` - Input snapshots, typed layer/query/summary/diagnostic keys, provider output metadata, and cache digest vocabulary.
- `.planning/phases/24-persistent-layer-cache-for-existing-cheap-facts/24-CONTEXT.md` - Layer cache persistence, dependency indexes, stale-reuse safeguards, and public no-leak proof.
- `.planning/phases/25-rule-manifest-inspect-and-test-skeleton/25-CONTEXT.md` - Public inspect/test boundary, external-consumer rule-host proof, and no broad fact/query/debug promotion.
- `.planning/phases/26-semantic-index-deepening/26-CONTEXT.md` - Semantic import rows, internal semantic provider boundary, stable export identities, and explicit deferral of topology graph work to Phase 27.

### Source Surfaces To Inspect
- `crates/polint/src/module_graph/mod.rs` - Current module graph derivation, trigger capabilities, layer key/dependency edges, cache read/write path, and provider integration.
- `crates/polint/src/module_graph/model.rs` - Current `ModuleGraphBuilder`, `ModuleGraphLayerPayload`, resolved import drafts, and deterministic node/edge construction.
- `crates/polint/src/module_graph/go.rs` - Go package index, lifecycle setup handling, module ownership, and package-to-file mapping.
- `crates/polint/src/module_graph/ts.rs` - TS/JS resolver context, oxc_resolver integration, tsconfig path alias handling, and external/unresolved classification.
- `crates/polint/src/module_graph/query.rs` - Current graph query helper shape and deterministic traversal expectations.
- `crates/polint/src/core/mod.rs` - Existing public `PackageFact`, `ModuleNode`, `ModuleEdge`, `ResolvedImportFact`, status, precision, and reason enums.
- `crates/polint/src/sdk/facts.rs` - Existing `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` public view boundary to preserve.
- `crates/polint/src/analysis_kernel/provider.rs` - Current provider manifests, schema versions, provider order tests, and module/symbol graph inputs/outputs.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Layer key and dependency identity vocabulary to reuse for topology cache keys.
- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` - Source/config/lifecycle/toolchain/provider input components that topology facts should include.
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` - Persistent layer cache store/read/write behavior and stale/corrupt handling.
- `crates/polint/src/analysis_kernel/metadata.rs` - Shared fact metadata vocabulary and stable-key helpers.
- `crates/polint/src/analysis_kernel/validation.rs` - Metadata validation and referential-integrity checks to extend for topology facts.
- `crates/polint/src/symbol_graph/semantic.rs` - Phase 26 semantic import rows and status vocabulary that may feed import-to-package classification.
- `tests/eval-fixtures/` - Internal eval fixture layout to extend for topology validation.
- `crates/polint/tests/cli.rs` and `crates/polint/tests/common/mod.rs` - Public no-leak, temp-repo, cache, and external-consumer integration patterns.

### API And Visibility
- `AGENTS.md` - Public API visibility, Rust best-practice usage, rule-authoring platform contract, Go lifecycle contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.
- `docs/facts/imports.md` and `docs/facts/module-graph.md` - Existing public import/module graph docs to keep aligned if public behavior changes.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ModuleGraphBuilder` already creates deterministic file, package, module, and external nodes plus contains/imports/depends-on edges from existing syntax facts.
- `derive_requested_module_graph_with_cache_stats` already wraps the module graph provider with layer cache read/write, output digests, dependency edges, and capability support overrides.
- `module_graph_layer_key` and dependency edge helpers already include source/package/import shape, config, Go lifecycle, TS/JS lifecycle, provider schema, toolchain placeholder, and upstream syntax output digests.
- `GoPackageIndex` already uses the Go lifecycle config and sidecar package metadata, tracks setup-missing reasons, maps files to import paths, and seeds Go module/package ownership.
- `TsResolverContext` already builds a per-run resolver, maps resolved paths back to known files, collects tsconfig path aliases, and classifies dynamic/external/unresolved imports.
- `ProviderManifest` already names `polint.module_graph` with inputs `source_files`, `packages`, and `imports`, and outputs `resolved_imports`, `module_nodes`, and `module_edges`.
- `AnalysisDb::replace_module_graph_facts` and metadata replacement tests already prove module graph facts can be restored with provider metadata after cache hits.
- Phase 26 semantic debug/eval support already exposes semantic import/status rows internally, which can be used as context for richer import-to-package classification.

### Established Patterns
- New internals should be `pub(crate)` and test/eval-facing until deliberately promoted through `sdk`, `runner`, or documented CLI contracts.
- Provider outputs are normalized, deterministically sorted, and validated before rule execution.
- Public uncertainty is preferred over fake precision: setup-missing, unresolved, ambiguous, dynamic, unsupported, heuristic, external, undeclared, and unknown states should remain visible where relevant.
- Go lifecycle inputs live in `[languages.go]` and participate in deterministic digests; the engine must not write repository lifecycle files.
- Public rule-author proof uses temp repos and `polint::sdk::prelude::*`, not imports from internal modules.
- Public JSON/help output must not leak internal provider manifests, metadata rows, eval schemas, layer-cache internals, topology internals, or raw package-manager/resolver output.

### Integration Points
- Extend `module_graph` with internal topology fact families and schema/version updates.
- Update provider manifests if `polint.module_graph` outputs or subprovider boundaries change.
- Extend layer keys and dependency edges for manifest/lockfile/workspace/source-set/topology inputs.
- Extend metadata and validation for topology stable keys, referential integrity, path normalization, precision ceilings, and conflict handling.
- Extend cache payloads/restore paths to persist normalized topology facts.
- Extend eval fixtures and CLI tests to prove Go monorepo root inference, TS/JS deterministic workspace/package facts, import-to-package classification, cache digest participation, and public no-leak behavior.

</code_context>

<specifics>
## Specific Ideas

- Auto-selected default: Phase 27 should be the module graph deepening vertical slice, not a public topology promotion phase.
- Auto-selected default: exactness should come from static files and already-available tool metadata; missing lockfiles or unsupported dynamic build scripts are explicit unknowns, not silent gaps.
- Auto-selected default: declared requirements, resolved dependency selections, and actual source imports answer different questions and should remain separate.
- Auto-selected default: Go should be the highest-precision path first because the lifecycle model and module semantics are tractable.
- Auto-selected default: TS/JS support should prioritize deterministic workspaces/packages, dependency sections, tsconfig paths, package exports/imports, and lockfile evidence without depending on physical `node_modules`.

</specifics>

<deferred>
## Deferred Ideas

- Python, Java/JVM, Cargo, Maven/Gradle, Nx/Turborepo, Pants/Bazel, and broader monorepo/task graph support from the research roadmap.
- Real repo-local extension/provider activation, extension merge, extension conflict side tables, and extension-aware cache quarantine.
- Public `Packages<'_>`, `Dependencies<'_>`, `SourceSets<'_>`, `RepoTopology<'_>`, topology CLI commands, and broad SDK query builders.
- Exact npm/pnpm/Yarn/Bun peer-context modeling, exact Yarn PnP, external package-manager oracle execution in default scans, and dynamic build-tool execution.
- MIR/place identity, CFG/control dependence, direct call facts, abstract domains, summaries, dataflow, slicing, evidence bundles, benchmark promotion gates, and public advanced query ergonomics.

</deferred>

---

*Phase: 27-layered-module-package-topology-graph*
*Context gathered: 2026-05-19*
