# Phase 23: Input Snapshots and Cache-Key Vocabulary - Context

**Gathered:** 2026-05-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 23 delivers the internal typed vocabulary and deterministic input snapshot instrumentation required for correct future layered cache invalidation. It must introduce internal `InputSnapshot`, `Digest`, `LayerKey`, `QueryKey`, `SummaryKey`, and `DiagnosticKey` concepts, record cache/provider metadata needed to explain invalidation inputs, and prove snapshot determinism across source, config, lifecycle, rule, model, and tool-invocation inputs.

This phase does not persist reusable layer outputs, change public SDK or CLI contracts, add a public inspect/test surface, or implement demand query/summary/extension caching. Phase 24 owns persistent layer cache behavior; Phase 25 owns public rule manifest/inspect/test loops; later v1.2 phases own demand queries, summaries, extension sinks, and red-green incrementality.

</domain>

<decisions>
## Implementation Decisions

### Vocabulary Boundary
- **D-01:** Keep the new cache-key and snapshot model crate-private/internal. Do not expose `InputSnapshot`, `Digest`, `LayerKey`, `QueryKey`, `SummaryKey`, `DiagnosticKey`, provider output metadata, or cache stats through `polint::sdk`, `runner`, crate-root exports, documented CLI output, or stable JSON schemas in this phase.
- **D-02:** Add typed vocabulary and instrumentation before changing cache reuse behavior. Existing file-fact cache behavior may be observed and instrumented, but Phase 23 must not implement persistent layered reuse, dependency-index invalidation, stale-reuse verification, or cache quarantine semantics.
- **D-03:** Prefer a future-shaped internal module boundary such as `crates/polint/src/analysis/incremental/` or an equivalent private namespace under the existing kernel/cache modules. Exact file placement is planner discretion, but it must preserve public API discipline and avoid stringly typed global hashes internally.
- **D-04:** The existing `crate::cache::CacheKey`, `Cache`, `CACHE_VERSION`, `config_hash`, `rule_hash`, and analysis-plan digest remain compatibility inputs. Phase 23 should bridge them into the typed vocabulary instead of deleting or broad-rewriting current cache behavior.

### Input Snapshot Coverage
- **D-05:** `InputSnapshot` should represent a coherent run input view, not a mutable filesystem read-through. It should cover discovered source files, normalized paths, language classification, source text digest, file size, optional mtime/discovery hints, loaded config digest, language lifecycle digests, toolchain/tool invocation digests where present, rule digests/options, extension/model digest placeholders, and provider/schema versions.
- **D-06:** Source text digests are authoritative; mtimes and other filesystem metadata may be stored only as hints and must not prove content equality.
- **D-07:** Include Go lifecycle vocabulary from the project contract: inferred or configured module roots, `go.mod`, `go.sum`, `go.work`, build tags, `include_tests`, package patterns, Go version/tool invocation identity where known, and relevant environment policy.
- **D-08:** Include TS/JS lifecycle vocabulary for resolver-affecting inputs: `tsconfig`/`jsconfig`, package manifests, lockfiles, resolver options, module kind/target/path aliases where available, source-set membership, and official tool invocation digest slots where present.
- **D-09:** Include rule and model digest vocabulary even if model files or extension providers are placeholders today. The shape should make Phase 34 extension cache participation and later model/summary invalidation possible without another identity rewrite.
- **D-10:** Snapshot serialization used in tests must be deterministic and must exclude absolute machine-local paths, timestamps as identity, temp roots, nondeterministic map order, and raw source text.

### Key Semantics
- **D-11:** `Digest` should be a typed value with explicit digest kind/context, deterministic construction helpers, stable display/serde behavior for internal snapshots, and canonical sorting for variable-length digest lists.
- **D-12:** `LayerKey` should encode layer kind, provider identity/version, schema version, parameter digest, lifecycle digest, config digest, optional toolchain digest, input digests, dependency-layer digests, and extension digests. Variable lists must be sorted canonically.
- **D-13:** `QueryKey`, `SummaryKey`, and `DiagnosticKey` should exist now as internal vocabulary even if their full consumers arrive later. They should model query kind/version/budget/precision, callable/domain/body/dependency summary identity, and rule code/options/requested view/evidence identity respectively.
- **D-14:** Provider manifests from Phase 20 should be one source of provider id, schema, cache policy, language scope, and precision ceiling. Phase 21 metadata should remain the source of fact provenance/stable-key truth, while Phase 23 adds input/output/cache identity around those facts.

### Provider Output Metadata And Stats
- **D-15:** Add internal provider output metadata that records provider id/version, schema version, output digest, precision/validation status, dependency edges or dependency inputs where available, and cache stats.
- **D-16:** Cache stats should start with deterministic counters such as hits, misses, recomputes, writes, bypasses/disabled, invalid/evicted reads, verified reuse, and quarantines where applicable. Counters may be zero for not-yet-implemented future behavior but should be explicit, not inferred from logs.
- **D-17:** Provider output metadata should attach to kernel/provider run results or a crate-private run report first. It should not alter public `polint check` JSON or rule-visible facts.
- **D-18:** Replace the Phase 20 synthetic provider-manifest consumption debt with real consumers where practical: snapshot/key construction, provider output metadata, or validation tests should read manifest metadata for a reason, rather than keeping metadata live through a dropped weight token.

### Verification Strategy
- **D-19:** Use focused crate-private unit tests for digest determinism, typed key equality, canonical list sorting, config/rule/lifecycle digest participation, and no absolute path/source-text leakage in snapshots.
- **D-20:** Use the Phase 22 native evaluation harness for at least one cache/snapshot fixture proving current cache behavior plus new input snapshot/key invariants without introducing Phase 24 persistent layer semantics.
- **D-21:** Add snapshot coverage for the roadmap-required input families: file text, config, Go lifecycle, TS/JS lifecycle, rule digests, model digests, and official tool invocation digests where present.
- **D-22:** Preserve existing `polint check`, SDK fact views, examples, ignore behavior, diagnostics rendering, cache read/write compatibility, and deterministic output. Public behavior should change only if an existing internal bug is surfaced as a controlled diagnostic.
- **D-23:** Tests should prove unsupported or absent lifecycle/tool/model inputs are represented explicitly as absent/unsupported/setup-missing identity components, not silently dropped or faked as exact.

### the agent's Discretion
- The planner may choose exact module/file names, type fields, digest labels, and serde/debug helper names.
- The planner may decide whether snapshot/key debug output is direct JSON snapshots, eval observed invariants, or both, as long as it remains internal/test-facing and deterministic.
- The planner may split planning by digest/key model, input snapshot construction, provider metadata/stats, and evaluation/test coverage.
- The planner may defer any expensive full lifecycle discovery that would belong to Phase 24 or Phase 27, provided Phase 23 still records deterministic vocabulary and explicit absent/setup-missing placeholders.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 23 goal, success criteria, research refs, and v1.2 phase order.
- `.planning/REQUIREMENTS.md` - `SAE-FND-04` requirement and milestone out-of-scope constraints.
- `.planning/PROJECT.md` - Current milestone target, public API discipline, cache substrate target, and behavior-preservation constraints.
- `.planning/STATE.md` - Current phase position and instruction to use referenced research before broad new research.
- `research/ROADMAP.md` - Source-of-truth static-analysis engine implementation sequence, if present in the checkout.

### Research
- `research/incremental-query-engine/FINAL-REPORT.md` - Snapshot-the-world model, typed cache identity, dependency edges, equality pruning, stale-cache risk, and first-version conservative strategy.
- `research/incremental-query-engine/RECOMMENDED_IMPLEMENTATION.md` - Recommended Phase 0/1/2 vocabulary: `Digest`, `CacheStats`, `ProviderOutputMeta`, `InputSnapshot`, `LayerKey`, `QueryKey`, `SummaryKey`, and `DiagnosticKey`.
- `research/module-graph/RECOMMENDED_IMPLEMENTATION.md` - Lifecycle/topology cache-key direction for module roots, package managers, manifests, lockfiles, Go/TS/JS source sets, and future cache-key sidecars.
- `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md` - Provider manifest, scheduling, metadata, and cache/layer strategy context relevant to replacing synthetic manifest consumption with real cache/snapshot consumers.
- `research/semantic-index/RECOMMENDED_IMPLEMENTATION.md` - Future semantic-layer stable identity and lifecycle/cache digest expectations; use as future-fit guidance, not Phase 23 scope expansion.

### Prior Phase Decisions
- `.planning/phases/07-cache-and-performance/07-CONTEXT.md` - Existing content-addressed cache key contract, source-free cache payload constraint, disabled-cache semantics, deterministic parallelism, and stale-hit intolerance.
- `.planning/phases/11-capability-driven-analysis-plan/11-CONTEXT.md` - Internal `AnalysisPlan`, plan digest, capability/setup diagnostics, and cache identity participation.
- `.planning/phases/20-private-analysis-kernel-facade/20-CONTEXT.md` - Private kernel boundary, provider manifests, provider order, no public provider surface, and Phase 23 cache vocabulary ownership.
- `.planning/phases/21-provenance-precision-and-validation-metadata/21-CONTEXT.md` - Internal fact metadata, stable keys, validation, provider identity, and prohibition on Phase 21 cache-key/layer-cache work.
- `.planning/phases/22-internal-evaluation-harness-mvp/22-CONTEXT.md` - Internal evaluation harness, native cache fixture boundary, deterministic output hashing, and no Phase 23/24 cache semantics in Phase 22.

### API And Visibility
- `AGENTS.md` - Public API visibility, Rust skill usage, rule-authoring platform contract, Go analysis lifecycle contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/cache/mod.rs` - Existing `CacheKey`, `Cache`, `CacheLayout`, cache status/clean/prune helpers, `stable_hash`, and tests for current file-key digest behavior.
- `crates/polint/src/cache/keys.rs` - Deterministic config/rule/options hashing that should become typed snapshot/key inputs rather than being replaced by ad hoc serde JSON.
- `crates/polint/src/analysis_kernel/mod.rs` - Private kernel facade currently receives `LoadedConfig`, `Cache`, config digest, rule digest, `AnalysisPlan`, and parallel flag; this is the natural integration point for snapshot construction and provider run metadata.
- `crates/polint/src/analysis_kernel/provider.rs` - Provider manifests for `polint.source`, `polint.go.syntax`, `polint.ts.syntax`, `polint.module_graph`, `polint.symbol_graph`, and `polint.metrics`; Phase 23 should use these for provider/schema/cache identity.
- `crates/polint/src/analysis_kernel/metadata.rs` and `crates/polint/src/analysis_kernel/debug.rs` - Existing provenance/stable-key/validation metadata and deterministic test-facing debug JSON; useful for linking provider output metadata to fact metadata without public exposure.
- `crates/polint/src/eval/` and `tests/eval-fixtures/` - Internal harness and native fixture layout from Phase 22; reuse for snapshot/key invariants and cache determinism proof.
- `crates/polint/tests/cli.rs` - Existing integration tests around cache behavior, capability digest changes, public JSON compatibility, and temp-repo fixtures.

### Established Patterns
- Internal analysis additions are crate-private and test-facing first; public SDK/CLI promotion waits for explicit later phases.
- Deterministic output is enforced by sorted rows/maps and explicit snapshot normalization, not by relying on incidental map or filesystem order.
- Cache correctness is conservative: stale reuse is worse than recompute, and invalid cache reads should miss/fallback rather than crash normal analysis.
- Existing adapter cache keys include file path/content, config digest, rule digest, plan digest, cache version, and schema. Phase 23 should type and extend this identity model rather than weakening it.
- Provider/fact metadata already separates run-local IDs from stable keys. Phase 23 should keep that separation and add input/output identity around providers.

### Integration Points
- Kernel input construction in runner/CLI analysis paths where config/rule/plan digests are already available.
- Source discovery and `AnalysisDb` loading where normalized root-relative paths, content hashes, and language classification are known.
- Go and TS/JS adapter entry points where per-file cache keys and schema names are built.
- Module graph and symbol graph providers where lifecycle/setup-sensitive inputs should become explicit snapshot components.
- Evaluation observed-item generation where internal snapshot/key invariants can be emitted as fixture observations.

</code_context>

<specifics>
## Specific Ideas

- Auto-selected default: keep Phase 23 as vocabulary/instrumentation only, because Phase 24 owns persistent layer reuse and stale-reuse safeguards.
- Auto-selected default: cover the full SAE-FND-04 input family now, including model/tool/extension placeholders, to avoid another cache identity rewrite when later phases land.
- Auto-selected default: provider output metadata should be consumed internally by kernel/eval tests first, not exposed through public check output.
- Auto-selected default: use native eval fixtures plus unit tests to prove determinism and non-leakage.

</specifics>

<deferred>
## Deferred Ideas

- Persistent layer cache with dependency indexes, change sets, hit/miss reporting for reusable layers, and stale-reuse safeguards - Phase 24.
- Public rule manifest, `polint inspect rule --format json`, and `polint test` fixture runner - Phase 25.
- Deeper semantic index and topology graph cache participation beyond vocabulary/placeholders - Phases 26 and 27.
- Demand queries, summary cache, extension-aware cache quarantine, repo-local extension provider sink, and red-green daemon behavior - later v1.2 phases.
- Public SDK query views or public cache/query debug contracts - Phase 41 or another explicit promotion phase.

</deferred>

---

*Phase: 23-input-snapshots-and-cache-key-vocabulary*
*Context gathered: 2026-05-18*
