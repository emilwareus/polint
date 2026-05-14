# Phase 13: Symbols and References - Context

**Gathered:** 2026-05-12
**Status:** Ready for planning
**Mode:** auto-selected recommended defaults

<domain>
## Phase Boundary

Phase 13 delivers stable symbol, definition, and reference facts through the
public SDK. It must satisfy SYM-01 through SYM-04 by adding public typed fact
views, stable polint-owned IDs, precision/status fields, TS/JS facts from Oxc
semantic data, and Go facts from typed package information where setup exists.

This phase creates the identity layer needed by later call graph, CFG, dataflow,
coverage, and agent navigation work. It does not implement call graph, CFG,
dataflow, broad lifecycle/plugin APIs, TypeScript type-checker sidecars, Go SSA,
or whole-program analysis beyond what is necessary to expose symbols and
references honestly.

</domain>

<decisions>
## Implementation Decisions

### Public SDK And Capability Boundary
- **D-01:** Add real capabilities named `symbols` and `references`; `references` depends on symbol identity internally.
- **D-02:** Expose rule-author access through typed SDK views such as `Symbols<'_>` and `References<'_>`, exported through `polint::sdk::prelude::*`.
- **D-03:** Add a separate `Definitions<'_>` view only if it materially improves ergonomics. Otherwise expose definitions through `Symbols::definition` and `Symbols::definitions`.
- **D-04:** Keep Oxc scoping IDs, Go `types.Object`, package loader output, sidecar JSON, raw AST nodes, and internal indexes out of the public API.
- **D-05:** Extend the `#[polint::rule]` macro mapping so canonical SDK view parameters derive `symbols` and `references` capabilities. Do not add manual `impl Rule` examples, compatibility shims, or public escape hatches.

### Fact Model And Precision
- **D-06:** Model symbols, definitions, and references as distinct facts. Do not collapse declarations into references.
- **D-07:** Use stable polint-owned IDs for `SymbolId`, `DefinitionId`, and `ReferenceId`; symbol IDs must be semantic digests, not vector positions.
- **D-08:** Store enough stable-key/debug input during early versions to diagnose collisions and unexpected ID churn.
- **D-09:** Facts must carry explicit precision and resolution status. At minimum support exact semantic, exact local, module-linked, heuristic, unresolved, ambiguous, setup-missing, and unsupported cases where relevant.
- **D-10:** Public facts should expose honest uncertainty. Setup-missing, unresolved, ambiguous, unsupported, and heuristic facts remain visible to rules and agents instead of becoming empty arrays or crashes.

### Derivation Pipeline
- **D-11:** Implement symbols/references as a cross-file derived analysis stage, likely under `crates/polint/src/symbol_graph/`, following the Phase 12 `module_graph` provider pattern.
- **D-12:** Run syntax adapters first, derive module graph when symbols/references need resolved module context, then derive symbol/reference facts before rules execute.
- **D-13:** The symbol provider may request module graph derivation internally. Rule authors should not have to request `ModuleGraphFacts<'_>` just to get useful symbol references.
- **D-14:** The derivation stage should return diagnostics plus capability support overrides, matching the `ModuleGraphDerivation` pattern.
- **D-15:** Keep internal indexes outside public fact structs so SDK queries can be fast without exposing storage details.

### Language Semantics
- **D-16:** For TS/JS, use `oxc_semantic` first. This phase should produce exact local lexical symbols/references and module-linked import/export references where existing module graph facts allow it.
- **D-17:** Do not claim exact TS cross-file member/property/type-checker resolution in Phase 13. Leave a future TypeScript compiler sidecar path open for max-precision mode.
- **D-18:** For Go, use a small Go sidecar backed by `golang.org/x/tools/go/packages` and `go/types` for typed package information where setup exists.
- **D-19:** Go package-level symbols should prefer `objectpath`-style stable identity where possible; local symbols may include file path, lexical owner chain, name, and position.
- **D-20:** If Go tooling or project setup is unavailable, report `symbols`/`references` as setup-missing for Go while keeping existing syntax-level facts available.

### Lifecycle And Future Analysis Fit
- **D-21:** Keep Phase 13 lifecycle implementation minimal. Do not ship a broad public lifecycle/plugin API just because research sketched one.
- **D-22:** Implement only the lifecycle/analyzer configuration hooks needed for real Phase 13 symbol setup, if any, most likely Go package patterns/build tags/tests mode and TS project roots/aliases/known globals.
- **D-23:** Internally avoid architecture that would block a future `AnalysisManifest`, `AnalysisUnit`, command declaration model, or typed fact-provider lifecycle.
- **D-24:** Later call graph, CFG, dataflow, coverage, and test metrics should consume symbol/reference identity and lifecycle products rather than inventing separate setup systems.
- **D-25:** Lifecycle/fact-provider drafts, when introduced later, must go through typed builders with origin, precision, source/digest inputs, and validation. They must not receive raw mutable `AnalysisDb` access.

### Cache, Setup, And Diagnostics
- **D-26:** Any config, sidecar version, toolchain version, build tags, package patterns, TS project roots, resolver aliases, or rule options that can affect symbol/reference facts must participate in deterministic cache digests.
- **D-27:** Go symbol cache granularity should be package/setup oriented, not falsely per-file, because typed package facts depend on package context.
- **D-28:** Symbol/reference setup failures should produce deterministic capability diagnostics with actionable hints and docs paths. They must not be parser diagnostics, panics, or silent unsupported facts.
- **D-29:** Provider outputs must be deterministic: stable sorting, stable hashing, reproducible diagnostics, and no randomized hashers.

### External-Consumer Proof
- **D-30:** Add temp-repo style tests where `.polint/rules` imports only `polint::sdk::prelude::*`, requests typed symbol/reference views through `#[polint::rule]`, runs `polint check --format json`, and asserts diagnostics from real facts.
- **D-31:** Minimum proof should include TS local definitions/references, TS unresolved names, TS import alias/module-linked references, Go package function definitions/calls, Go method selector references, setup-missing Go behavior, macro capability mapping, and stable ID/cache restore behavior.
- **D-32:** Document new public facts under `docs/facts/`, including precision limits and heuristic/setup behavior.

### the agent's Discretion
- The agent may choose exact file/module names inside `symbol_graph`, exact enum variant names, and exact query method names as long as the public SDK surface remains narrow, documented, and consistent with existing fact views.
- The agent may sequence implementation across multiple Phase 13 plans, for example core facts/capabilities first, TS/JS semantic extraction next, Go sidecar next, and external-consumer/cache proof last.
- The agent may defer broad lifecycle APIs, graph database storage, TS compiler sidecar work, call graph/CFG/dataflow facts, and advanced symbol queries if SYM-01 through SYM-04 are met honestly.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 13 goal, requirements, and success criteria.
- `.planning/REQUIREMENTS.md` - SYM-01 through SYM-04 and sequencing into later call graph, CFG, coverage, test metrics, and dataflow work.
- `.planning/PROJECT.md` - Product positioning, Rust/performance constraints, truthfulness requirements, and long-term graph-analysis direction.
- `.planning/phases/13-symbols-and-references/13-RESEARCH.md` - Primary research and architecture recommendation for symbols, references, lifecycle staging, stable IDs, language engines, and verification plan.

### Prior Phase Decisions
- `.planning/phases/03-core-facts-and-diagnostics/03-CONTEXT.md` - `AnalysisDb`, typed IDs, deterministic diagnostics, `Rule`, `RuleCtx`, and capability-contract baseline.
- `.planning/phases/04-go-adapter/04-CONTEXT.md` - Go syntax-first adapter decisions, heuristic wording, and deferral of semantic Go sidecars until after syntax facts stabilized.
- `.planning/phases/05-typescript-adapter/05-CONTEXT.md` - Oxc TS/JS adapter decisions and earlier deferral of project-level semantic resolution.
- `.planning/phases/06-sdk-and-example-rules/06-CONTEXT.md` - SDK/prelude public surface, external-consumer examples, and rule-authoring helper discipline.
- `.planning/phases/07-cache-and-performance/07-CONTEXT.md` - Cache correctness, source-free payloads, deterministic parallelism, and performance constraints.
- `.planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md` - Internal `AnalysisPlan`, capability support, setup diagnostics, and plan digest decisions.
- `.planning/phases/12-resolved-imports-and-module-relationships/12-CONTEXT.md` - Derived provider pattern, module graph sequencing, setup-missing support, and typed SDK view boundary.

### API And Visibility
- `AGENTS.md` - Public API visibility, Rust skill usage, rule-authoring platform contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.

### External Technical References From Research
- Oxc semantic docs/source - local symbol/reference extraction source of truth.
- Go `go/packages`, `go/types`, and `objectpath` docs - Go typed package and stable object identity source of truth.
- Sourcegraph SCIP / LSIF - design inspiration for symbol occurrences and roles, not a public format dependency.
- Bazel aspects, Pants plugin API, CodeQL extraction setup, Joern CPG overlays, LLVM pass invalidation, CodeQL dataflow, and rust-analyzer incrementality references listed in `13-RESEARCH.md` - lifecycle and layered-analysis design inputs.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/core/mod.rs` - Owns `AnalysisDb`, current fact structs/IDs, `Capabilities`, `CapabilitySupport`, `RuleCtx`, and rule execution.
- `crates/polint/src/sdk/facts.rs` - Existing typed fact view pattern and `FactView` trait used by the macro.
- `crates/polint/src/sdk/mod.rs` - Public prelude export surface for supported rule-author facts.
- `crates/polint-macros/src/lib.rs` - Maps canonical fact view parameter types to capability methods.
- `crates/polint/src/analysis_plan.rs` - Internal requested-capability planning, support status, setup diagnostics, and plan digest model.
- `crates/polint/src/module_graph/` - Closest existing pattern for a setup-aware cross-file derived provider with diagnostics and support overrides.
- `crates/polint/src/runner/mod.rs` - `analyze_and_run` integration point after syntax adapters and before derived metrics/rule execution.
- `crates/polint/src/go/adapter.rs` and `crates/polint/src/ts/adapter.rs` - Existing syntax fact producers and plan-aware adapter entrypoints.
- `crates/polint/tests/cli.rs` - Temp-repo/local-rule-host integration test pattern for proving outside rule-author behavior.

### Established Patterns
- Public rule-author APIs belong under `polint::sdk` and `polint::runner`; internals should stay private or `pub(crate)`.
- Fact views borrow from `AnalysisDb` and should avoid cloning source text or exposing parser objects.
- Project-wide providers run after syntax fact harvest and before rules execute.
- Setup-missing and unsupported capability states are public data and capability diagnostics, not hidden failures.
- Machine-readable CLI output must remain deterministic and free from human prelude text.
- Cache keys include configuration/rule/plan inputs when they affect harvested facts.

### Integration Points
- Extend `Capabilities`, capability planning/support, macro mapping, SDK prelude exports, `AnalysisDb` storage, and fact docs for symbols/references.
- Add `symbol_graph::derive_requested_symbols` after module graph derivation and before metrics/rules.
- Teach module graph derivation to run when symbols/references require module context.
- Add TS/JS semantic extraction using existing Oxc dependencies and existing source/span conversion patterns.
- Add a Go sidecar or sidecar integration boundary that converts typed Go facts into polint-owned symbol/reference facts.
- Add external-consumer CLI tests that prove public SDK imports only.

</code_context>

<specifics>
## Specific Ideas

- Recommended implementation order: core fact model/capabilities/SDK views, TS/JS Oxc semantic extraction, Go typed sidecar extraction, cache/setup diagnostics, then external-consumer tests and docs.
- Recommended directory shape from research:
  - `crates/polint/src/symbol_graph/mod.rs`
  - `crates/polint/src/symbol_graph/model.rs`
  - `crates/polint/src/symbol_graph/ts.rs`
  - `crates/polint/src/symbol_graph/go.rs`
  - `crates/polint/src/symbol_graph/query.rs`
  - `crates/polint/src/symbol_graph/stable_id.rs`
- Keep lifecycle research as architectural guidance, not Phase 13 public API scope unless a narrow analyzer configuration hook is required for Go or TS setup.

</specifics>

<deferred>
## Deferred Ideas

- Broad public lifecycle API with `ScanLifecycle`, `AnalysisManifest`, command declarations, typed fact providers, and custom provider builders.
- TypeScript compiler sidecar for project/type-checker-level member/property/declaration-file resolution.
- Go SSA, call graph algorithms, CHA/RTA/pointer analysis, and interprocedural summaries.
- Public call graph, CFG, dataflow, coverage, test metric, Python, and Java fact families beyond the identity layer needed by symbols/references.
- Project-level symbol graph cache beyond deterministic fact/cache participation required for Phase 13 correctness.

</deferred>

---

*Phase: 13-symbols-and-references*
*Context gathered: 2026-05-12*
