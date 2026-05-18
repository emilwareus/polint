# Phase 24: Persistent Layer Cache for Existing Cheap Facts - Context

**Gathered:** 2026-05-18
**Status:** Ready for planning

<domain>
## Phase Boundary

Phase 24 turns the Phase 23 cache vocabulary and kernel run-report instrumentation into a real persistent layer cache for existing cheap fact layers. The phase should persist reusable outputs for current parse/syntax, imports, module graph, symbol/reference, and metrics layers with conservative invalidation, deterministic hit/miss reporting, dependency tracking, and stale-reuse safeguards.

This phase must not add public cache/query APIs, cache diagnostics as the primary artifact, implement arbitrary demand-query or summary caching, build extension quarantine semantics beyond placeholder counters, or promote SDK/CLI inspection surfaces. Phase 25 owns public rule manifest/inspect/test loops. Later v1.2 phases own semantic deepening, demand queries, summaries, extension sinks, and public SDK/query promotion.

</domain>

<decisions>
## Implementation Decisions

### Layer Cache Boundary
- **D-01:** Auto-selected default: cache existing cheap provider layers only. Start with the current manifest-owned providers and their outputs: `polint.go.syntax`, `polint.ts.syntax`, `polint.module_graph`, `polint.symbol_graph`, and `polint.metrics`.
- **D-02:** Treat source files as input snapshot identity, not a separate reusable layer payload unless the planner finds an existing low-churn source metadata artifact worth manifesting. Raw source text must not be persisted in new layer payloads.
- **D-03:** Do not cache diagnostics first. Diagnostics remain downstream rule products until fact-layer digests, dependency indexes, and evidence identity are stable enough for later phases.
- **D-04:** Keep all new cache-layer types and helpers crate-private under `crates/polint/src/analysis_kernel/incremental/`, `crates/polint/src/cache/`, or another existing private namespace. Do not expose them through `polint::sdk`, `polint::runner`, crate-root exports, documented CLI output, or public JSON.
- **D-05:** Preserve the behavior of current `polint check`, SDK fact views, ignore handling, diagnostics rendering, and existing cache compatibility unless stale or invalid cache data is intentionally treated as a controlled miss/recompute.

### Layer Identity And Dependencies
- **D-06:** Use Phase 23 `LayerKey`, `Digest`, `InputSnapshot`, `ProviderOutputMeta`, provider manifests, and `KernelRunReport` as the canonical identity substrate. Do not introduce a parallel stringly typed layer identity model.
- **D-07:** Layer keys must include provider id/version, schema, parameters, config, lifecycle/toolchain where relevant, input digests, dependency-layer digests, and extension digest placeholders. Variable digest lists must remain canonically sorted.
- **D-08:** Record a versioned crate-private dependency index for cached layers. The first useful form can store forward and reverse edges between input components, provider layers, and dependency layer digests; if the index schema changes, rebuild/drop it rather than migrating aggressively.
- **D-09:** Start with explicit dependency edges derived from provider manifests and actual snapshot/key inputs. Add shape classification only where the current code already makes it cheap and deterministic. If classification is missing or uncertain, recompute.
- **D-10:** Module graph and symbol graph cache entries must depend on import facts, source/package/function inputs as appropriate, lifecycle/config digests, provider/schema identity, and upstream layer output digests. Metrics must depend on source/function inputs and relevant upstream syntax/function layer digests.

### Invalidation Policy
- **D-11:** Fail closed. A cache entry may be reused only when its manifest, schema, provider version, key inputs, dependency index, and output digest validation all match the current run.
- **D-12:** Unrelated rule code edits must not invalidate syntax layers unless the rule digest or options genuinely affect provider parameters. This is a required success criterion and should have a regression test.
- **D-13:** Import shape, language lifecycle, config, provider/schema, source text, and dependency-layer output changes must invalidate affected module and symbol layers. When the engine cannot prove a narrower invalidation boundary, recompute the broader dependent layer.
- **D-14:** Corrupt, stale, mismatched, unsupported, or deserialization-failing cache entries should be counted as invalid reads or misses and recomputed. Normal analysis must not crash because of stale on-disk layer cache data.
- **D-15:** Keep extension quarantine counters explicit but do not implement real extension-aware quarantine semantics in Phase 24. Extension influence belongs to later extension/provider phases.

### Persistence And Rollout
- **D-16:** Build on the existing `.polint/cache` layout and `CacheLayout` where practical. Add layer-specific manifest/blob locations rather than replacing the current file-fact cache in one step.
- **D-17:** Prefer content-addressed or digest-named payloads plus a manifest written last. Writes should be atomic enough for normal local/CI use: write a temporary payload, write or rename into place, then publish the manifest after the payload is durable enough for the existing cache standard.
- **D-18:** Migrate provider layers incrementally. Syntax providers can bridge from the existing file-fact cache path; derived providers such as module graph, symbol graph, and metrics should gain layer manifests/payloads once their dependencies and output digests are explicit.
- **D-19:** Persist only normalized, deterministic fact-layer payloads and metadata. Do not persist absolute machine paths, raw source text, timestamps as identity, temp roots, nondeterministic map order, or transient run IDs as cache truth.
- **D-20:** If low-churn implementation requires keeping the existing file-fact cache for Go/TS syntax while adding a layer manifest around it, that is acceptable as a first step, provided Phase 24 success criteria are still proven.

### Verification And Observability
- **D-21:** Cache stats must report deterministic per-layer and aggregate hits, misses, recomputes, writes, disabled bypasses, invalid evicted reads, and verified reuse where Phase 24 implements verified layer reuse. Counters stay crate-private/test-facing unless a later phase promotes them.
- **D-22:** Extend internal eval observations or kernel test helpers to prove layer cache behavior without changing public `polint check --format json`.
- **D-23:** Required tests: cold/warm runs show deterministic misses then hits; unrelated rule edits do not invalidate syntax layers; import edits invalidate module/symbol layers; config/lifecycle edits invalidate affected layers; corrupt/stale entries fail safely; public JSON does not leak layer cache internals.
- **D-24:** Add focused unit tests for manifest serialization, dependency-index ordering, invalidation planning, key equality, atomic write/read fallback, and stale-entry handling. Add temp-repo or native eval fixtures where cross-provider behavior matters.
- **D-25:** Keep verification tied to the real existing providers and facts, not synthetic-only fixtures. Synthetic helpers are fine for unit tests, but at least one end-to-end proof should exercise Go/TS syntax plus derived layers.

### the agent's Discretion
- The planner may choose exact type names such as `LayerCacheManifest`, `LayerCacheStore`, `DependencyIndex`, `ChangeSet`, or `InvalidationPlan`.
- The planner may decide whether the first implementation reads cached derived layers before or after lightweight validation of source snapshots, as long as stale reuse fails closed.
- The planner may split the phase into syntax layer persistence, derived layer manifests, invalidation/dependency index, and eval/public-boundary proof.
- The planner may leave more precise semantic change classification for later phases if Phase 24 still recomputes conservatively and satisfies the stated success criteria.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Phase Scope
- `.planning/ROADMAP.md` - Phase 24 goal, success criteria, research refs, phase order, and v1.2 milestone guardrails.
- `.planning/REQUIREMENTS.md` - `SAE-FND-05` requirement and milestone out-of-scope constraints.
- `.planning/PROJECT.md` - Product constraints, current milestone target, public API discipline, reliability, truthfulness, and performance requirements.
- `.planning/STATE.md` - Current milestone state and accumulated decisions. Note that it may lag phase 23 completion; prefer phase summaries for the latest phase 23 implementation details.
- `research/ROADMAP.md` - Source implementation sequence if broader roadmap cross-checking is needed.

### Research
- `research/incremental-query-engine/RECOMMENDED_IMPLEMENTATION.md` - Primary guidance for persistent layer cache, dependency index, change-set classification, invalidation planning, cache manifests, and conservative reuse.
- `research/analysis-kernel/RECOMMENDED_IMPLEMENTATION.md` - Kernel provider manifests, layer cache keys, provider output metadata, validation/merge gates, and internal boundary guidance.
- `research/semantic-index/RECOMMENDED_IMPLEMENTATION.md` - Future semantic-layer identity and symbol/reference cache expectations. Use as future-fit guidance, not scope expansion.

### Prior Phase Decisions
- `.planning/phases/07-cache-and-performance/07-CONTEXT.md` - Existing cache discipline: source-free payloads, stale-hit intolerance, disabled-cache semantics, and deterministic parallelism.
- `.planning/phases/20-private-analysis-kernel-facade/20-CONTEXT.md` - Private kernel boundary, provider manifests, provider order, and no public provider surface.
- `.planning/phases/21-provenance-precision-and-validation-metadata/21-CONTEXT.md` - Fact metadata, stable keys, validation, provider identity, and merge conflict discipline.
- `.planning/phases/22-internal-evaluation-harness-mvp/22-CONTEXT.md` - Internal eval harness, native fixture model, deterministic output hashing, and no public eval surface.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-CONTEXT.md` - Cache vocabulary boundary, input snapshot coverage, key semantics, provider output metadata, and verification strategy.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-01-SUMMARY.md` - `Digest`, `LayerKey`, query/summary/diagnostic keys, `CacheStats`, and provider metadata were added.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-02-SUMMARY.md` - `InputSnapshot` construction and lifecycle/input component identity were added.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-03-SUMMARY.md` - Existing Go/TS file-fact cache access now reports internal `CacheStats`.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-04-SUMMARY.md` - `KernelRunReport` now carries input snapshots, provider outputs, and aggregate stats.
- `.planning/phases/23-input-snapshots-and-cache-key-vocabulary/23-05-SUMMARY.md` - Native eval fixture and public no-leak proof cover Phase 23 snapshot/key/provider invariants.

### API And Visibility
- `AGENTS.md` - Public API visibility, Rust skill usage, rule-authoring platform contract, Go lifecycle contract, and GSD workflow requirements.
- `docs/API-VISIBILITY-PLAN.md` - Visibility tightening and supported API boundary guidance.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/polint/src/analysis_kernel/mod.rs` - `AnalysisKernel::run` now constructs an `InputSnapshot`, executes providers in manifest order, records provider outputs, and attaches a crate-private `KernelRunReport`.
- `crates/polint/src/analysis_kernel/provider.rs` - Provider manifests define ids, inputs, outputs, schema versions, language scopes, precision ceilings, and cache policies for the six current providers.
- `crates/polint/src/analysis_kernel/incremental/digest.rs` - Typed digest construction and deterministic identity helpers from Phase 23.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - `LayerKey`, `LayerKind`, `QueryKey`, `SummaryKey`, and `DiagnosticKey`; Phase 24 should consume `LayerKey` instead of inventing a new identity shape.
- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` - Deterministic source/config/rule/model/extension/provider/lifecycle snapshot rows and input component status vocabulary.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - `KernelRunReport`, provider output digest construction, manifest-derived dependency inputs, and aggregate cache stats.
- `crates/polint/src/analysis_kernel/incremental/stats.rs` - `CacheStats` counters and `ProviderOutputMeta` rows, including future counters for verified reuse and quarantine.
- `crates/polint/src/cache/mod.rs` - Current disk-backed JSON cache, `CacheKey`, status-aware read/write helpers, `CacheLayout`, and cache status/clean/prune layout. This is the likely base for layer cache persistence.
- `crates/polint/src/go/adapter.rs` and `crates/polint/src/ts/adapter.rs` - Existing syntax providers now expose stats-returning wrappers while keeping diagnostic-only compatibility wrappers.
- `crates/polint/src/module_graph/`, `crates/polint/src/symbol_graph/`, and `crates/polint/src/metrics.rs` - Current derived providers that need layer output digests, persistence, and conservative invalidation.
- `crates/polint/src/eval/observed.rs`, `crates/polint/src/eval/fixtures.rs`, and `tests/eval-fixtures/cache/input-snapshots/` - Internal eval observation and fixture patterns that can be extended for layer cache behavior.
- `crates/polint/tests/cli.rs` - Existing public-boundary, temp-repo, cache, and no-leak integration test patterns.

### Established Patterns
- Internals stay crate-private and test/eval-facing until a later phase deliberately promotes a CLI, SDK, or JSON contract.
- Provider order is currently explicit and behavior-preserving: source, Go syntax, TS/JS syntax, module graph, symbol graph, metrics.
- Current Go/TS cache behavior is file-fact oriented and already counts hit/miss/write/disabled/invalid outcomes internally.
- Phase 23 snapshots and run reports intentionally exclude raw source text, absolute paths, timestamps as identity, and public output exposure.
- Fact metadata already separates run-local IDs from stable keys and provider output digests summarize deterministic fact metadata rows.
- Existing cache invalid reads are treated as misses/evictions rather than normal-run crashes.

### Integration Points
- Add layer cache read/write planning around `AnalysisKernel::run`, before and after each current provider executes.
- Use provider manifests to construct layer cache identities and dependency edges.
- Reuse `InputSnapshot` and `ProviderOutputMeta` to decide reuse and report stats.
- Extend `CacheLayout` with layer cache directories/manifests while preserving current analysis and rules-target directories.
- Feed Go/TS adapter cache stats and new layer stats into `KernelRunReport`.
- Add eval observed rows for layer-cache cold/warm/invalidation behavior without changing public `polint check` output.

</code_context>

<specifics>
## Specific Ideas

- Auto-selected default: implement the first persistent layer cache as a conservative local cache, not a red-green daemon or arbitrary query engine.
- Auto-selected default: syntax layers should survive unrelated rule edits; if a current compatibility key includes rule digest too broadly, Phase 24 should introduce a layer key that separates provider inputs from downstream diagnostic/rule inputs.
- Auto-selected default: derived layer reuse is allowed only when upstream layer output digests and lifecycle/config dependencies match.
- Auto-selected default: stale reuse is worse than recompute. Prefer broader recomputation and explicit stats over clever shape pruning that is not yet proven.
- Auto-selected default: all observability remains internal/test/eval-facing in this phase.

</specifics>

<deferred>
## Deferred Ideas

- Public rule manifest, `polint inspect rule --format json`, and `polint test` fixture runner - Phase 25.
- Deeper semantic index, richer import resolution, aliases, stable export identities, and topology graph deepening - Phases 26 and 27.
- Demand-query caching, summary SCC cache, query traces, abstract domains, summaries, and interprocedural reuse - later v1.2 phases.
- Real repo-local extension provider activation and extension-aware cache quarantine - Phase 34 and related later phases.
- Public SDK query views, public cache/query debug commands, or agent-facing advanced analysis contracts - Phase 41 or another explicit promotion phase.

</deferred>

---

*Phase: 24-persistent-layer-cache-for-existing-cheap-facts*
*Context gathered: 2026-05-18*
