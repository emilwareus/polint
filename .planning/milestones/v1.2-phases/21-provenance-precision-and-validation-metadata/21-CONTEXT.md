# Phase 21: Provenance, Precision, and Validation Metadata - Context

**Gathered:** 2026-05-16
**Status:** Ready for planning
**Mode:** auto-selected recommended defaults

<domain>
## Phase Boundary

Phase 21 adds shared internal metadata for existing kernel-produced fact families: provenance, precision, confidence, validation status, stable keys, and deterministic merge validation. It extends the private kernel/fact substrate created in Phase 20 without changing rule-author ergonomics or promoting a new public analysis API.

This phase must not build the evaluation harness, typed layer cache keys, persistent layer cache, demand scheduler, extension/provider sink, MIR, CFG, call graph, summaries, data-flow, evidence bundles, or public SDK query promotion. Those belong to later v1.2 phases.

</domain>

<decisions>
## Implementation Decisions

### Metadata Storage
- **D-01:** Add metadata as crate-private sidecar storage, most likely under `analysis_kernel`, `AnalysisDb`, or a nested `FactMetaStore`. Do not inflate public fact structs with new provenance/confidence/validation fields in Phase 21.
- **D-02:** Model metadata around a stable `FactRef` concept such as `(fact_family, run_id)` mapped to `FactMeta`. Exact type names are planner discretion, but the model must support current dense in-run IDs and cross-run stable keys separately.
- **D-03:** Metadata must remain internal. Do not add SDK views, crate-root exports, runner contracts, or documented public CLI output for metadata in this phase.
- **D-04:** The planner may keep existing family-specific fields such as `ResolutionPrecision`, `SymbolPrecision`, stable symbol keys, and unresolved/status enums. Add shared metadata beside them instead of deleting or forcing a broad public model migration.

### Metadata Coverage
- **D-05:** Attach metadata to all existing kernel-produced fact families where practical in this phase: source files, packages, functions, imports, branch obligations, tests, coverage placeholders, TS/JS component/class/literal/JSX facts, resolved imports, module nodes/edges, symbols, definitions, references, and metrics facts.
- **D-06:** Debug JSON proof must cover at least files, imports, symbols, and references, matching the roadmap success criteria. Broader debug coverage is welcome only if it stays low-churn and internal.
- **D-07:** Use provider manifests from Phase 20 as the source of producer/layer identity for default native metadata: `polint.source`, `polint.go.syntax`, `polint.ts.syntax`, `polint.module_graph`, `polint.symbol_graph`, and `polint.metrics`.
- **D-08:** If full coverage for an obscure legacy placeholder would cause disproportionate churn, the implementation should add an explicit internal validation gap test or TODO tied to Phase 22 rather than silently omitting metadata.

### Truth Labels
- **D-09:** Introduce a shared crate-private vocabulary for provenance/producer, precision, confidence, and validation status. Keep it small enough for current providers but future-shaped for extensions.
- **D-10:** Recommended defaults: native provider provenance; exact or syntax precision for parser/source/metrics facts; setup-aware or family-mapped precision for module and symbol facts; high confidence for native trusted facts; lower confidence for setup-missing, unsupported, ambiguous, heuristic, or unresolved facts.
- **D-11:** Preserve family-specific precision/status truthfulness. Shared metadata should summarize or normalize existing precision/status, not overwrite honest family-level distinctions.
- **D-12:** Validation status should distinguish native trusted facts from schema/referential/span/stable-key validated facts and rejected/conflicting outputs. Exact enum names are planner discretion.

### Stable Keys And Merge Validation
- **D-13:** Add deterministic stable-key metadata for every metadata-covered fact family. Existing stable symbol/reference keys should be reused; families without stable keys need deterministic keys derived from normalized file paths, provider/family identity, spans, names, and existing stable fingerprints where appropriate.
- **D-14:** Separate run-local IDs from stable keys. Dense IDs may stay for in-memory joins, but metadata stable keys must be suitable for cache/evidence/debug use across runs.
- **D-15:** Duplicate identical stable keys with identical facts may collapse idempotently. Duplicate stable keys with conflicting facts must fail deterministically through controlled kernel diagnostics or internal errors, not "first writer wins" behavior.
- **D-16:** Fail closed on merge conflicts before affected facts reach rules where practical. If preserving behavior requires keeping existing facts in this phase, emit deterministic `polint/internal` diagnostics and add tests that pin the conflict behavior.
- **D-17:** Validation should include at least stable-key uniqueness, referential integrity for IDs/spans/targets, span bounds where source text is available, provider precision ceiling checks, deterministic ordering, and conflict diagnostics.

### Debug And Inspection
- **D-18:** Add deterministic crate-private/test-facing debug JSON helpers for provenance and metadata inspection. Do not promote a stable public CLI surface for metadata in Phase 21.
- **D-19:** Debug JSON must exclude timestamps, absolute machine paths, nondeterministic map order, and transient memory details.
- **D-20:** Include enough provider/family/stable-key/precision/confidence/validation fields in debug JSON for agents and future harness fixtures to explain where files, imports, symbols, and references came from.

### Compatibility And Tests
- **D-21:** Existing `polint check`, SDK fact views, examples, cache behavior, diagnostics rendering, ignore handling, and rule execution behavior must remain compatible unless a deterministic internal conflict is intentionally surfaced.
- **D-22:** Add focused tests for metadata presence, default metadata mapping, debug JSON determinism, stable-key uniqueness, conflicting duplicate behavior, and missing-metadata detection for new kernel provider outputs.
- **D-23:** Do not use Phase 21 to split cache keys or persist layer outputs. Phase 23 owns typed cache-key vocabulary and Phase 24 owns persistent layer cache behavior.
- **D-24:** Do not add extension authoring or extension merge APIs yet. Validation/merge code may be shaped so future extension-like outputs can pass through it, but extension activation belongs to Phase 34.

### the agent's Discretion
- The planner may choose exact module placement and type names, with a preference for keeping kernel-owned metadata under `crates/polint/src/analysis_kernel/` unless `AnalysisDb` needs direct sidecar ownership.
- The planner may split implementation into metadata model/default attachment first, then validation/merge/debug JSON second.
- The planner may decide whether `SourceFile.content_hash`, `BranchObligation.stable_fingerprint`, and existing symbol stable-key helpers are wrapped directly or normalized through a new stable-key helper layer.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 21 goal, success criteria, research refs, and v1.2 phase order.
- `.planning/REQUIREMENTS.md` - `SAE-FND-02` acceptance requirement and milestone out-of-scope constraints.
- `.planning/PROJECT.md` - Current milestone state, public API discipline, truthfulness constraints, and performance/reliability constraints.
- `research/ROADMAP.md` - Source-of-truth implementation sequence and Phase 21 row.

### Research
- `research/analysis-kernel/FINAL-REPORT.md` - Kernel decision, sidecar provenance, stable keys, validation-before-merge, explicit unknowns, and cache/layer strategy.
- `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md` - Primary implementation guidance for `FactMeta`, `FactRef`, metadata defaults, validation gates, provider outputs, and merge checks.
- `research/semantic-index/FINAL-REPORT.md` - Semantic-index metadata requirements: stable keys, provider identity, input dependencies, precision, confidence, validation status, and lifecycle/cache digest direction.

### Prior Phase Decisions
- `.planning/phases/20-private-analysis-kernel-facade/20-CONTEXT.md` - Kernel ownership boundary, provider manifests, provider order, and no public provider surface.
- `.planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md` - Internal `AnalysisPlan`, capability support, setup diagnostics, deterministic plan/cache digest discipline.
- `.planning/phases/12-resolved-imports-and-module-relationships/12-CONTEXT.md` - Setup-aware module graph facts, explicit unresolved/setup/unsupported states, deterministic provider output, and typed SDK boundary.
- `.planning/phases/13-symbols-and-references/13-CONTEXT.md` - Stable symbol/reference IDs, precision/status fields, setup-missing behavior, deterministic provider outputs, and future provider validation notes.

### API And Visibility
- `AGENTS.md` - Public API visibility, Rust skill usage, rule-authoring platform contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/analysis_kernel/mod.rs` - Current crate-private kernel facade and execution order. Phase 21 should extend this boundary rather than moving logic back into the runner.
- `crates/polint/src/analysis_kernel/provider.rs` - Provider manifests already define provider IDs, outputs, schema versions, cache policy, and precision ceilings for the six current providers.
- `crates/polint/src/core/mod.rs` - `AnalysisDb` owns current typed fact vectors and in-run IDs; it is the likely host or integration point for a sidecar metadata store.
- `crates/polint/src/symbol_graph/model.rs` and `crates/polint/src/symbol_graph/stable_id.rs` - Existing stable-key generation, collision diagnostics, duplicate checks, and deterministic ordering for symbols, definitions, and references.
- `crates/polint/src/module_graph/model.rs` - Existing deterministic module graph builder and resolution precision/status model.
- `crates/polint/src/diagnostics/mod.rs` - Existing deterministic diagnostics and fingerprinting helpers for controlled internal diagnostics.
- `crates/polint/src/go/adapter.rs` and `crates/polint/src/ts/adapter.rs` - Syntax providers that currently push cached fact payloads into `AnalysisDb`; metadata attachment must cover cache-restore and fresh-parse paths.
- `crates/polint/src/metrics.rs` - Derived metrics provider that should receive default metadata without changing rule-facing metric views.

### Established Patterns
- Internals stay `pub(crate)` unless deliberately promoted through `sdk`, `runner`, or documented CLI contracts.
- Current providers already use deterministic vectors, `BTreeMap`/sorted output, stable fingerprints, and controlled diagnostics instead of panics where possible.
- Setup-sensitive uncertainty is represented explicitly for module and symbol facts; metadata should preserve that honesty.
- Provider manifests are production-consumed only as internal metadata consistency today; scheduling and cache identity changes are later phases.

### Integration Points
- Attach default metadata when files/facts enter `AnalysisDb`, when module/symbol/metrics facts replace derived vectors, or at provider output merge boundaries if the planner introduces an internal `ProviderOutput`.
- Reuse `AnalysisKernel::provider_manifests()` for provider IDs and precision ceiling validation.
- Add debug JSON helpers behind crate-private/test-only APIs, then use tests to prove files/imports/symbols/references expose provenance deterministically.
- Generalize or complement symbol graph duplicate/collision validation so stable-key conflict behavior is shared by other fact families.

</code_context>

<specifics>
## Specific Ideas

- Treat this as the metadata substrate phase, not a behavior expansion phase.
- Prefer a sidecar store and deterministic reports over broad fact struct churn.
- Metadata should make uncertainty more inspectable, not make existing heuristic or setup-sensitive facts look exact.
- Validation should be strict enough to catch future extension-style mistakes, even though extension activation is out of scope.

</specifics>

<deferred>
## Deferred Ideas

- Internal evaluation harness fixtures and expected/observed JSON - Phase 22.
- Input snapshots, typed cache keys, provider output metadata for invalidation, and lifecycle/toolchain/rule/model digest vocabulary - Phase 23.
- Persistent layer cache, dependency indexes, change sets, cache stats, and stale reuse safeguards - Phase 24.
- Rule manifests, `polint inspect rule`, and `polint test` public fixture runner - Phase 25.
- Demand scheduler, extension sink, MIR, CFG, call graph, summaries, framework entrypoints, type/value/alias facts, data-flow, slicing/evidence, benchmark gates, and SDK query promotion - later v1.2 phases.

</deferred>

---

*Phase: 21-provenance-precision-and-validation-metadata*
*Context gathered: 2026-05-16*
