# Phase 12: Resolved Imports and Module Relationships - Discussion Log

**Command:** `$gsd-discuss-phase 12 --auto`
**Date:** 2026-05-11
**Mode:** auto
**Interactive prompts:** none

## Inputs Reviewed

- Phase 12 in `.planning/ROADMAP.md`
- MOD-01 through MOD-04 in `.planning/REQUIREMENTS.md`
- Product direction in `.planning/PROJECT.md`
- Existing roadmap docs under `docs/roadmap/`
- Existing Phase 03, 04, 05, 06, 07, 08, and 11 context files
- Current code structure around `AnalysisDb`, `Capabilities`, SDK fact views, macros, adapters, runner, and cache keys
- Rust/project rules in `AGENTS.md`

## Auto-Resolved Gray Areas

### Public SDK Boundary
- **Question:** Should resolved imports/module graph be exposed as raw resolver data, broad `RuleCtx` accessors, or typed SDK views?
- **[auto] Selected:** Typed SDK views. Add `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` through the SDK/prelude and derive capabilities from rule function parameters.
- **Reason:** This matches the rule-authoring platform contract and keeps external consumers insulated from resolver/library churn.

### Capability Naming And Support
- **Question:** Should this phase reuse syntax `imports`, add new capabilities, or keep capabilities unsupported until later?
- **[auto] Selected:** Add real `resolved_imports` and `module_graph` capabilities when the provider and public fact views exist.
- **Reason:** Syntax imports and resolved imports have different precision/setup requirements. Architecture rules need to request the richer facts explicitly.

### Provider Placement
- **Question:** Should resolution live in Go/TS adapters, the runner, or a project-wide derived provider?
- **[auto] Selected:** Add a project-wide derived provider after syntax adapters and before rule execution.
- **Reason:** Module relationships are cross-file and cross-language orchestration data. Keeping them out of per-file parser adapters preserves cache boundaries and maintainability.

### Resolution Failure Semantics
- **Question:** Should unresolved imports be omitted, turned into diagnostics only, or kept as facts with status?
- **[auto] Selected:** Keep one resolved-import record per syntactic import, with explicit status and reason.
- **Reason:** Agents and architecture rules need to see uncertainty. Omitting failed resolution would hide important codebase structure and setup gaps.

### TypeScript/JavaScript Resolver Scope
- **Question:** Should TS/JS resolution be syntactic only, use `oxc_resolver`, or require TypeScript type checking?
- **[auto] Selected:** Use `oxc_resolver` for project-aware import resolution; do not add TypeScript type checking in Phase 12.
- **Reason:** `oxc_resolver` fits the existing Rust/Oxc stack and covers the module-resolution problem without expanding into type semantics.

### Go Resolver Scope
- **Question:** Should Go resolution stay syntax-only, shell out to Go package metadata, or add full Go semantic analysis?
- **[auto] Selected:** Use Go package/module metadata where setup is available, such as `go list -json ./...`; do not add Go type checking or SSA.
- **Reason:** Package metadata is enough for module/package relationships and avoids pulling future call graph/dataflow concerns into this phase.

### Graph Storage And Public Queries
- **Question:** Should the graph be exposed directly, hidden entirely, or exposed through narrow query methods?
- **[auto] Selected:** Keep internal graph storage private and expose narrow borrowed query methods for architecture rules.
- **Reason:** Public graph internals would be a semver burden. Query methods can evolve around real rule-author needs.

### Caching
- **Question:** Should Phase 12 introduce a project-level graph cache immediately?
- **[auto] Selected:** No project-level cache in the first implementation. Recompute from cached syntax facts and add project cache only if profiling requires it.
- **Reason:** Correctness and deterministic behavior matter more than speculative cache complexity at this point.

### Test Strategy
- **Question:** What proof is required before treating this as a real capability?
- **[auto] Selected:** Unit/model tests, language resolver fixtures, deterministic output tests, macro capability tests, and at least one temp-repo external-rule test through `polint check --format json`.
- **Reason:** The capability should be proven the way users and agents will actually consume it.

## Locked Outcome

Phase 12 should deliver an honest, setup-aware module relationship foundation. It should be useful immediately for repo-local architecture linting, while staying deliberately narrower than symbols, call graphs, CFG, dataflow, or taint analysis.

## Follow-On

The workflow should advance to Phase 12 planning after this context is committed.
