# Phase 12: Resolved Imports and Module Relationships - Context

**Gathered:** 2026-05-11
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 12 adds the first project-wide relationship layer on top of the existing Go and TypeScript/JavaScript syntax facts. It should resolve syntactic imports into setup-aware facts, build an internal module/dependency graph, and expose narrow typed SDK views that repo-local rules can consume for architecture linting.

This phase must not implement symbols/references, call graphs, CFG, coverage import, test metrics, dataflow, taint analysis, Python, or Java. Those are later phases that should depend on this module relationship foundation instead of bypassing it.

</domain>

<decisions>
## Implementation Decisions

### Public SDK And Capability Boundary
- **D-01:** Add real capabilities for resolved import facts and module graph facts. Suggested names are `resolved_imports` and `module_graph`.
- **D-02:** Expose the rule-author surface as typed SDK views, not broad `RuleCtx` fact access. Suggested views are `ResolvedImports<'_>` and `ModuleGraphFacts<'_>`, exported through `polint::sdk::prelude::*`.
- **D-03:** Raw resolver output from `oxc_resolver`, `go list`, package metadata, or any future resolver stays internal. Public facts should use polint-owned IDs, paths, module/package names, dependency kinds, and status enums.
- **D-04:** The derive macro should map SDK fact-view parameters to capabilities. Do not add manual capability examples or compatibility shims for this phase.
- **D-05:** Public status/precision types should be honest and forward-compatible. Use explicit states such as `Resolved`, `External`, `Unresolved`, `SetupMissing`, `Dynamic`, and `Unsupported`; avoid implying exact semantic coverage.

### Provider Placement And Data Flow
- **D-06:** Implement resolved imports and module relationships as a project-wide derived fact provider, likely under a new internal `module_graph` module. Do not bury project-wide resolution inside per-file Go or TS parser adapters.
- **D-07:** Run the provider after Go and TS/JS syntax adapters have populated/restored syntax facts and before rules execute. The provider should receive the `AnalysisDb`, loaded config, and `AnalysisPlan`/support information.
- **D-08:** Store resolved import and module graph facts in `AnalysisDb` as ID-based records. Link back to existing syntax import facts and file/package facts instead of cloning source text or exposing AST nodes.
- **D-09:** Keep the provider deterministic: stable input ordering, sorted edges, `BTreeMap`/sorted intermediate maps where ordering matters, and deterministic diagnostics.

### Resolution Semantics And Uncertainty
- **D-10:** Emit one resolved-import record for every syntactic import the adapters already harvest, even when resolution fails or setup is missing.
- **D-11:** Do not drop uncertain relationships. Unresolved, dynamic, unsupported, and setup-missing imports should remain visible to rules and agents.
- **D-12:** Setup failures should become capability/setup diagnostics when the requested capability needs them. They must not panic, crash the run, or masquerade as parser errors.
- **D-13:** Rules should be able to distinguish local project edges from external dependency edges. Standard library dependencies and package-manager dependencies should not be confused with missing local files.

### Language Resolver Scope
- **D-14:** TypeScript/JavaScript resolution should use `oxc_resolver` for practical project-aware behavior: relative imports, package metadata, extensions, and `tsconfig` concepts such as `baseUrl` and `paths` where available.
- **D-15:** TypeScript/JavaScript dynamic imports with string literals may be treated as resolvable imports when the syntax facts can identify the string. Dynamic expressions should be reported as `Dynamic`.
- **D-16:** Go resolution should use Go package/module metadata where setup is available, such as `go list -json ./...` or an equivalent library-backed integration. It should map Go import paths to local package/file nodes where possible.
- **D-17:** If Go tooling or module setup is unavailable, emit setup-aware unresolved facts and diagnostics instead of falling back to fake precision.
- **D-18:** Do not add TypeScript type checking, Go type checking, Go SSA, or interprocedural semantic analysis in Phase 12.

### Graph Storage And Query Shape
- **D-19:** The internal graph may use `petgraph`, but public SDK APIs must not expose `petgraph` types or raw graph internals.
- **D-20:** Start with query methods that architecture rules actually need: imports for a file, unresolved imports for a file, module/package node for a file, outgoing dependencies, incoming dependencies, local/external classification, and dependency status.
- **D-21:** Add reachability or path queries only if they are small, deterministic, and directly useful for Phase 12 tests. More advanced graph algorithms can wait for call graph/dataflow phases.
- **D-22:** Do not add a project-level graph cache in the first implementation. Recompute the graph from cached syntax facts, then add a project cache later only if profiling shows it matters.

### External-Consumer Proof
- **D-23:** Add at least one temp-repo style test where `.polint/rules` imports only `polint::sdk::prelude::*`, requests the new typed views through `#[polint::rule]`, runs `polint check --format json`, and asserts diagnostics from real resolved facts.
- **D-24:** Add focused tests for TypeScript/JavaScript resolution, Go resolution with setup, Go setup-missing behavior, deterministic graph output, and macro capability derivation.
- **D-25:** Tests must prove unsupported or setup-missing capabilities produce deterministic capability diagnostics and do not execute with placeholder facts.

### the agent's Discretion
- The agent may choose exact internal type names, module/file layout, and query method names as long as the public rule-author surface remains narrow and documented.
- The agent may decide whether resolver setup is configured through existing language config maps first or through new explicit config fields, as long as config that affects behavior participates in deterministic digests.
- The agent may defer project-level caching, advanced graph algorithms, and extra convenience queries if the core MOD-01 through MOD-04 requirements are met.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 12 goal, success criteria, and sequencing within the v1.1 capability roadmap.
- `.planning/REQUIREMENTS.md` - MOD-01 through MOD-04 acceptance requirements and explicit out-of-scope items.
- `.planning/PROJECT.md` - Product positioning, long-term graph-analysis direction, Rust/performance constraints, truthfulness requirement, and public API discipline.
- `docs/roadmap/12_RESOLVED_IMPORTS_MODULE_GRAPH_ARCHITECTURE.md` - Primary architecture document for this phase.

### Capability Roadmap
- `docs/ANALYSIS-ROADMAP.md` - Human-facing sequence from module relationships through symbols, calls, CFG, coverage, and later dataflow.
- `docs/CAPABILITY-FULFILLMENT-RESEARCH.md` - Capability families, adapter contract, verification expectations, and quality gates.
- `docs/roadmap/00_ROADMAP.md` - Human roadmap index and capability sequencing.
- `docs/roadmap/04_ENTRY_4_RESOLVED_IMPORTS_MODULE_GRAPH.md` - Human roadmap entry for resolved imports/module graph.

### Existing Phase Decisions
- `.planning/phases/03-core-facts-and-diagnostics/03-CONTEXT.md` - `AnalysisDb`, typed IDs, deterministic diagnostics, `Rule`, `RuleCtx`, and capability-contract baseline.
- `.planning/phases/04-go-adapter/04-CONTEXT.md` - Go syntax-first adapter decisions and why Go semantic sidecars were deferred until after syntax facts stabilized.
- `.planning/phases/05-typescript-adapter/05-CONTEXT.md` - Oxc syntax adapter decisions and prior deferral of production module resolution.
- `.planning/phases/06-sdk-and-example-rules/06-CONTEXT.md` - SDK/prelude public surface and external-consumer example discipline.
- `.planning/phases/07-cache-and-performance/07-CONTEXT.md` - Cache correctness, source-free payloads, deterministic parallelism, and performance constraints.
- `.planning/phases/08-ci-output-and-graph-commands/08-CONTEXT.md` - Prior syntactic graph command boundaries and DOT/CI output discipline.
- `.planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md` - `AnalysisPlan`, capability support, setup diagnostics, and plan digest decisions.

### Coding Rules
- `AGENTS.md` - Public API visibility, Rust skill usage, rule-authoring platform contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/core/mod.rs` - Owns `AnalysisDb`, `Capabilities`, `RuleCtx`, typed facts, and rule execution. Phase 12 should add internal fact storage here or in tightly scoped submodules.
- `crates/polint/src/analysis_plan.rs` - Existing capability support model. Phase 12 should make `resolved_imports` and `module_graph` supported only when real providers and public views exist.
- `crates/polint/src/sdk/facts.rs` and `crates/polint/src/sdk/prelude.rs` - Existing typed fact view pattern. Add the new views here rather than exposing internals.
- `crates/polint-macros/src/lib.rs` - Maps fact-view parameter types to capabilities. It needs new mappings for the Phase 12 views.
- `crates/polint/src/runner/mod.rs` - `analyze_and_run` is the integration point after syntax adapters and before rule execution.
- `crates/polint/src/go/adapter.rs` and `crates/polint/src/ts/adapter.rs` - Existing syntax fact producers. Phase 12 should consume their import facts rather than rewriting parsing.
- `crates/polint/src/cache/keys.rs` - Existing digest pattern for config/rule/plan inputs. New behavior-affecting config must participate in deterministic digests.
- `crates/polint/tests/cli.rs` - Temp-repo/local-rule-host tests provide the proof pattern for outside rule authors.

### Established Patterns
- Public rule-author APIs belong under `polint::sdk` and `polint::runner`; crate-root analysis internals should stay private or `pub(crate)`.
- Fact views should borrow from `AnalysisDb` and avoid cloning large source strings or AST data.
- Syntax parser caches are per-file and source-free. Project-wide module resolution should be a separate derived step.
- Unsupported future capabilities should fail clearly or emit deterministic diagnostics; placeholder facts are not acceptable.
- JSON output for agents and CI must be deterministic.

### Integration Points
- Extend `Capabilities` with `resolved_imports` and `module_graph`.
- Extend `AnalysisDb` with module nodes, resolved import records, dependency edges, and lookup indexes.
- Add the project-wide module graph provider and call it from the runner after syntax facts are available.
- Extend SDK fact views and macro capability derivation.
- Add docs under `docs/facts/` describing resolved import/module graph limits and heuristic/setup-sensitive behavior.

</code_context>

<specifics>
## Specific Ideas

- The long-term product direction is a repo-local quality-control platform that agents can consume, not another generic linter. Phase 12 should make codebase structure explicit enough for architecture rules and future agent reasoning.
- Resolved imports/module relationships are the right next step because symbols, call graphs, CFG, dataflow, and taint analysis all become more useful when file/package/module boundaries are already modeled.
- The implementation should optimize for maintainability and honest precision first. It is acceptable to be setup-sensitive and incomplete if uncertainty is visible as data.

</specifics>

<deferred>
## Deferred Ideas

- Symbol and reference indexing - Phase 13.
- Direct and resolved call graph facts - Phase 14.
- CFG facts for Go and TS/JS - Phase 15.
- Coverage fact import - Phase 16.
- Test suite metrics - Phase 17.
- Python and Java adapters - Phases 18 and 19.
- Interprocedural dataflow, taint analysis, type-aware queries, graph database export, and advanced agent query surfaces - future milestones after the foundational graph is stable.
- Project-level graph caching - defer until profiling proves recomputation from cached syntax facts is too expensive.

</deferred>

---

*Phase: 12-resolved-imports-and-module-relationships*
*Context gathered: 2026-05-11*
