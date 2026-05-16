# Phase 20: Private Analysis Kernel Facade - Context

**Gathered:** 2026-05-16
**Status:** Ready for planning
**Mode:** auto-selected recommended defaults

<domain>
## Phase Boundary

Phase 20 moves the current analysis orchestration behind a private kernel boundary and adds provider manifests for existing source, Go syntax, TS/JS syntax, module graph, symbol graph, and metrics providers. It must preserve current behavior while establishing the internal ownership boundary that later phases will extend.

This phase must not implement provenance metadata, validation gates, layer cache keys, a demand scheduler, MIR, CFG, call facts, summaries, data flow, or public graph/query APIs. Those belong to later phases in the v1.2 roadmap.

</domain>

<decisions>
## Implementation Decisions

### Kernel Boundary
- **D-01:** Add a private kernel facade for existing analysis orchestration. The likely shape is a new internal module such as `crates/polint/src/analysis_kernel/`, but exact file names are the planner's discretion.
- **D-02:** Keep the kernel crate-private (`pub(crate)`) and do not add supported public SDK or crate-root API surface in this phase.
- **D-03:** The kernel owns analysis provider execution and returns `AnalysisDb`, provider diagnostics, and final capability support. The runner remains responsible for CLI/reporting concerns, ignore filtering, rule selection, rule options, and rule execution.
- **D-04:** Preserve the current provider execution order inside the kernel: source loading, Go syntax, TS/JS syntax, module graph, symbol graph, metrics. Do not introduce a new scheduler yet.

### Provider Manifests
- **D-05:** Add provider manifests as internal metadata first, not as a full execution framework. They should document provider id, kind, inputs, outputs, language scope, cache policy, schema/version placeholders, and precision ceiling where practical.
- **D-06:** Initial manifests should cover `polint.source`, `polint.go.syntax`, `polint.ts.syntax`, `polint.module_graph`, `polint.symbol_graph`, and `polint.metrics`.
- **D-07:** Manifest dependency data should be deterministic and testable, but it does not need to drive provider scheduling in Phase 20.
- **D-08:** Avoid migrating cache identity or layer cache behavior here. Phase 23 owns typed cache-key vocabulary and Phase 24 owns persistent layer cache behavior.

### Inspection And Debugging
- **D-09:** Provider order inspection should be internal/test-facing by default, for example through a crate-private snapshot/report helper used by tests.
- **D-10:** Do not add a new stable public CLI command for provider inspection in Phase 20. A hidden/debug path is acceptable only if the planner finds an existing internal debug pattern that does not widen the supported CLI contract.
- **D-11:** Any machine-readable provider-order output used in tests must be deterministic and must not include timestamps, absolute machine paths, or nondeterministic ordering.

### Compatibility And Tests
- **D-12:** Existing `polint check` behavior and all current rule-facing facts must remain unchanged.
- **D-13:** Tests should prove that `runner::analyze_and_run` delegates analysis to the kernel while rule execution still sees the same facts, diagnostics, and capability support.
- **D-14:** Add focused unit tests for provider manifest contents and provider order. Keep integration tests focused on behavior preservation rather than expanding public CLI surface.
- **D-15:** Do not expose raw provider manifests to rule authors. Later public ergonomics belong to Phase 41 after promotion gates.

### the agent's Discretion
- The planner may choose exact internal type names, module layout, and helper boundaries.
- The planner may decide whether `KernelInput` borrows `LoadedConfig`, `Cache`, `AnalysisPlan`, and digest strings exactly as the research sketch suggests or wraps them differently to reduce churn.
- The planner may keep `load_analysis_files` inside the kernel or model it as the first source provider, as long as the runner no longer owns the full analysis sequence and behavior stays unchanged.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 20 goal, success criteria, phase order, and release gate expectations.
- `.planning/REQUIREMENTS.md` - `SAE-FND-01` acceptance requirement and milestone out-of-scope constraints.
- `.planning/PROJECT.md` - Public API discipline, truthfulness constraints, Rust/performance constraints, and current milestone target features.
- `research/ROADMAP.md` - Source-of-truth implementation sequence and guardrails.

### Research
- `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md` - Primary design for the private kernel facade, provider manifests, provider order, and later phases.
- `research/implementation-bootstrap/RECOMMENDED_IMPLEMENTATION.md` - Longer-term internal `analysis` module and semantic-store direction; use as future-fit guidance, not Phase 20 scope expansion.

### Prior Phase Decisions
- `.planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md` - Internal `AnalysisPlan`, capability support, setup diagnostics, and plan digest decisions.
- `.planning/phases/12-resolved-imports-and-module-relationships/12-CONTEXT.md` - Derived provider pattern, module graph sequencing, setup-missing support, and typed SDK view boundary.
- `.planning/phases/13-symbols-and-references/13-CONTEXT.md` - Symbol graph provider pattern, setup-aware support overrides, stable IDs, and public API discipline.

### API And Visibility
- `AGENTS.md` - Public API visibility, Rust skill usage, rule-authoring platform contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/runner/mod.rs` - Current `analyze_and_run` owns the analysis sequence that Phase 20 should move behind the kernel facade.
- `crates/polint/src/lib.rs` - Internal module registration point. New analysis kernel modules should be `pub(crate)`.
- `crates/polint/src/analysis_plan.rs` - Existing private planning model and capability support view that the kernel should consume, not replace.
- `crates/polint/src/module_graph/mod.rs` - Existing project-wide derived provider pattern with diagnostics and capability support overrides.
- `crates/polint/src/symbol_graph/mod.rs` - Existing setup-aware symbol provider pattern and support merging.
- `crates/polint/src/metrics.rs` - Existing derived metrics provider that should receive a manifest.
- `crates/polint/src/go/adapter.rs` and `crates/polint/src/ts/adapter.rs` - Existing syntax provider entrypoints that should be invoked by the kernel without behavior change.
- `crates/polint/src/fs/mod.rs` - Source loading currently happens before language providers and should be represented as the source provider.

### Established Patterns
- Public rule-author APIs live under `polint::sdk` and `polint::runner`; internals stay crate-private.
- Derived providers already return diagnostics and capability-support overrides rather than panicking or exposing raw tool output.
- Current provider order is deterministic and should remain stable in Phase 20.
- Machine-readable outputs used by tests should be deterministic and path-stable.
- Cache changes should be minimized in this phase because later foundation phases own cache-key vocabulary and layer persistence.

### Integration Points
- Replace the analysis body of `runner::analyze_and_run` with a call into the new private kernel facade.
- Let the kernel call `load_analysis_files`, Go analysis, TS/JS analysis, module graph derivation, symbol graph derivation, and metrics derivation.
- Merge module graph and symbol graph capability support in the same order as today.
- Add tests around provider manifest registration and provider order inspection.

</code_context>

<specifics>
## Specific Ideas

- Phase 20 should be deliberately boring: move orchestration, add manifests, prove no behavior changed.
- The provider manifest model should be future-shaped enough for later scheduling/metadata/cache phases, but it should not try to solve those phases now.
- If a choice creates tension between research purity and low-churn behavior preservation, prefer low churn in Phase 20 and leave expansion to later phases.

</specifics>

<deferred>
## Deferred Ideas

- Fact metadata side tables, provenance, precision, validation, stable-key enforcement, and merge gates - Phase 21.
- Evaluation harness fixtures and deterministic expected/observed JSON - Phase 22.
- Input snapshots, typed cache keys, and provider output metadata - Phase 23.
- Persistent layer cache and conservative invalidation - Phase 24.
- Demand scheduling and summary/query cache behavior - later foundation/interprocedural phases.
- MIR, places, CFG, direct calls, abstract domains, summaries, extensions, framework entrypoints, type/value/alias facts, data flow, and evidence - later v1.2 phases.

</deferred>

---

*Phase: 20-private-analysis-kernel-facade*
*Context gathered: 2026-05-16*
