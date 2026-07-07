# Architecture Research: Static Analysis 2.0 Integration

**Project:** polint
**Researched:** 2026-07-07
**Scope:** v2.0 local semantic store, summaries, query foundations, search boundary, and validation integration.
**Overall confidence:** HIGH. Grounded in the existing private analysis kernel, provider DAG, incremental keys, fact metadata, validation gates, and locked local-semantic-store research.

## Architecture Recommendation

Add a private SQLite/rusqlite-backed `SemanticStore` under `analysis_kernel`, not under `sdk`, `runner`, language adapters, or public CLI modules. The store is a durable local index for validated facts, summaries, graph adjacency/evidence rows, query foundations, and search manifests. It is not a public SDK and not a replacement for `AnalysisDb` during the first v2.0 implementation.

Keep `AnalysisDb` as the in-memory assembly and rule-execution working set. The existing provider DAG remains the source of truth for computing facts. The semantic store is populated from the same provider outputs, `FactMeta`, `InputSnapshot`, `LayerKey`, `SummaryKey`, `QueryKey`, and `ProviderManifest` vocabulary after validation. Do not create a second fact-family registry, a second stable-ID scheme, or SQL-facing provider APIs.

Commit through a `StoreCommitPlan` built during the current kernel run. Initially, commit the full validated run generation after `validation::validate_fact_metadata` so the current global validators remain authoritative. Model each provider output as its own `provider_generation` row inside that run generation, but expose only complete generations to readers. Later, split into provider-level commits only after per-provider validators can prove the same invariants family by family.

Store failures should be controlled internal diagnostics and skipped persistence for `polint check` unless a future command explicitly requires a readable store. Public behavior should not change while the foundation lands. Future `polint graph` and search surfaces read from complete store generations through typed private query services and map results to stable JSON envelopes. They must never expose SQL, table names, raw row IDs, parser IDs, or provider internals.

## Module Plan

### New Modules

| Module | Responsibility | Boundary |
|--------|----------------|----------|
| `crates/polint/src/analysis_kernel/store/mod.rs` | Private facade: `SemanticStore`, `StoreReader`, `StoreWriter`, `StoreCommitPlan`, open modes, and store diagnostics. | `pub(crate)` only. No `rusqlite` types escape. |
| `analysis_kernel/store/connection.rs` | Opens SQLite connections with WAL, foreign keys, bounded busy timeout, read-only mode, and transaction helpers. | Internal. |
| `analysis_kernel/store/migrations.rs` | Numbered SQL migrations through `PRAGMA user_version`; future-schema and rebuild diagnostics. | Internal. |
| `analysis_kernel/store/schema.rs` | Private schema constants and prepared statement helpers. | Internal; table names are non-contract. |
| `analysis_kernel/store/generation.rs` | `store_manifest`, `input_snapshot`, `provider_generation`, active/complete generation selection, stale generation retention. | Internal. |
| `analysis_kernel/store/ingest.rs` | Extracts validated rows from `AnalysisDb` plus `FactMetaStore`; writes typed fact/index rows with required metadata. | Internal. |
| `analysis_kernel/store/summaries.rs` | Persists `summary_manifest`, `summary_payload`, `summary_dependency`, and `summary_projection` using `SummaryKey`. | Internal; future registry seam only. |
| `analysis_kernel/store/graph.rs` | Maintains typed node/edge adjacency, reverse adjacency, edge evidence, unknown regions, and budget events. | Internal graph/query substrate. |
| `analysis_kernel/store/query.rs` | Internal used-by, neighbors, callers, callees, path, and taint query foundation keyed by `QueryKey`. | Private result structs; future CLI maps them. |
| `analysis_kernel/store/payloads.rs` | Content-addressed payload abstraction for large summaries/evidence. Start with manifest indirection; benchmark SQLite BLOBs vs adjacent payload files. | Internal. |
| `analysis_kernel/store/search_manifest.rs` | Stable document IDs and Tantivy/vector manifest metadata. Search indexes are derived from store IDs and never facts. | Internal. |
| `analysis_kernel/store/validation.rs` | Store-specific validation: schema, referential integrity, row metadata, generation completeness, no-leak checks, query parity helpers. | Internal. |

### Modified Modules

| Module | Change |
|--------|--------|
| `Cargo.toml` | Add `rusqlite` with `bundled`. Add Tantivy only when lexical search starts. |
| `analysis_kernel/mod.rs` | Open the store from existing cache configuration, accumulate a `StoreCommitPlan`, run validation, commit complete generations, and include private store stats in `KernelRunReport`. |
| `analysis_kernel/provider.rs` | Keep manifests as the provider identity source. Add provider schema/version entries only when new persisted families require them. Do not expose store handles to providers. |
| `analysis_kernel/incremental/keys.rs` | Reuse `LayerKey`, `SummaryKey`, and `QueryKey`. Add missing private `LayerKind` variants for persisted v2.0 providers rather than inventing store-specific keys. |
| `analysis_kernel/incremental/layer_cache.rs` | Do not replace initially. Mirror layer manifests/dependencies into SQLite so invalidation has one vocabulary, then migrate reads only after parity is proven. |
| `analysis_kernel/metadata.rs` | Reuse `FactFamily` and `FactMeta`. Add new families only for real analysis facts, not for store bookkeeping rows. |
| `analysis_kernel/validation.rs` | Keep current fact validators first. Add store commit-plan validation and row-count/digest parity checks before committing. |
| `analysis/summaries/` | Expose crate-private canonical summary serialization/projection helpers consumed by `store::summaries`; keep refinement overlays run-local unless fully keyed. |
| `analysis/data_flow/` and `analysis/evidence/` | Route persisted path/evidence rows through store indexes only after internal query fixtures prove unknown/budget preservation. |
| `cli/` | Add no public command in the first store phase. Later add `polint graph` as a typed envelope renderer over `store::query`, not SQL. |

## Data Flow

```text
LoadedConfig + rules + source files
        |
        v
AnalysisKernel::run
        |
        v
existing provider DAG -> AnalysisDb + FactMeta + ProviderOutputMeta
        |
        v
validation::validate_fact_metadata and family validators
        |
        v
StoreCommitPlan
        |
        v
SQLite write transaction:
  store_manifest
  input_snapshot
  provider_generation
  layer_entry / layer_dependency
  typed facts + FactMeta metadata
  summary manifests/payload references
  graph adjacency/evidence/unknown/budget indexes
  validation_event
        |
        v
complete generation visible to read-only query/search commands
```

Provider execution should not query SQL in the first implementation. The first store write is an after-the-fact durable index over the already validated run. Once summary reuse is enabled, `analysis/summaries` may ask the store for validated summaries matching `SummaryKey` before recomputing. The summary reader must return `hit`, `miss`, `stale`, `unknown_schema`, or `invalid` explicitly; it must not silently substitute placeholder summaries.

Graph query flow should be:

```text
selector or stable ID
 -> store::query resolves stable node(s)
 -> indexed SQL fetch for xref/neighbors/callers/callees
 -> bounded recursive CTE only for small deterministic traversals
 -> Rust scoped traversal for path/taint queries
 -> result includes precision, status, provenance, unknowns, budgets, evidence IDs
 -> future CLI renders stable JSON envelope
```

Search flow should stay separate:

```text
complete store generation
 -> SearchCorpus over stable store document IDs
 -> Tantivy lexical index manifest
 -> search result IDs
 -> store lookup for spans/evidence/provenance
```

Search returns candidates. It does not create trusted analysis facts. Vector search stays deferred behind explicit model, chunker, dimension, metric, normalization, and source-digest lockfiles.

## Build Order

1. **Store facade and migrations**
   - Add `analysis_kernel::store` skeleton, rusqlite bundled dependency, migrations, manifest table, controlled diagnostics, and no-op integration tests.
   - No provider behavior change and no public surface.

2. **Run manifest and generation tracking**
   - Persist `InputSnapshot`, provider manifests, `ProviderOutputMeta`, layer entries, dependencies, validation events, and store stats.
   - Prove cold/warm byte-stable `KernelRunReport` normalization with store enabled and disabled.

3. **Validated semantic index ingest**
   - Persist source files, packages/modules/source sets, imports/resolutions, symbols/definitions/references, and `FactMeta` metadata.
   - Add internal xref tests for definition/reference/used-by. Keep CLI private.

4. **Summary persistence**
   - Persist canonical direct/SCC summaries with `SummaryKey`, payload digest, dependency summary digests, precision/status/provenance, and validation status.
   - Enable warm summary reuse only after from-scratch parity tests pass.
   - Preserve package/version summary manifests for future registry import/export, but build no remote registry.

5. **Graph adjacency and evidence indexes**
   - Persist direct calls, refined calls, reachability, data-flow edges, evidence bundles/paths/slices, unknown regions, and budget events.
   - Add bounded traversal over store-backed scoped graphs with deterministic ordering.

6. **Internal query engine**
   - Implement `used-by`, `neighbors`, `callers`, `callees`, `path`, and `taint` in `store::query`.
   - Key expensive query results/traces with `QueryKey`; expose status values `complete`, `partial`, `not_found`, `unknown`, `budget_exceeded`, `unsupported`, and `setup_missing`.

7. **Public graph CLI promotion**
   - Only after fixtures, determinism, benchmark, no-leak, and documentation gates pass.
   - CLI renders stable JSON/human envelopes over private query results. No SQL, Cypher, raw CFG/MIR IDs, or provider IDs.

8. **Lexical search boundary**
   - Add `SearchCorpus` and Tantivy index manifest over stable store document IDs.
   - Search can ship after graph query IDs are stable; vector search remains deferred.

9. **Pruning, compaction, and crash hardening**
   - Add stale-generation pruning, WAL/checkpoint behavior, payload cleanup, and recovery tests before treating store reuse as default for large repos.

## Validation Hooks

| Gate | Required Checks |
|------|-----------------|
| Migration | Empty DB, previous schema, idempotent migration/refusal, future-schema diagnostic, safe rebuild path. |
| Commit | Store writes only after existing fact validation passes; incomplete generations are invisible; old generation remains readable after failure. |
| Identity | Every fact-like row has fact family, stable key, provider ID/schema, source/package identity, digest inputs, precision, confidence/status, provenance, validation status, layer/input key, and generation. |
| Parity | Cold build, warm build, partial invalidation, process restart, randomized insertion order, and different Rayon worker counts produce byte-identical normalized query JSON. |
| Query correctness | Fixtures for used-by, cross-file refs, cross-package imports, direct/refined calls, cycles, path budget exceeded, taint barrier, summary boundary, extension edge, and unknown-preserving not-found cases. |
| Crash recovery | Kill during transaction, payload write, migration, WAL checkpoint, and search rebuild. Expected: old complete generation or new complete generation, never mixed rows. |
| Public boundary | Tests or static checks proving `rusqlite`, SQL strings, table names, raw row IDs, provider generation IDs, and store payload formats do not appear in `sdk`, public docs, check JSON, or promoted CLI envelopes. |
| Benchmarks | Ingest and query p50/p95 on 100k/500k/1M+ row scales; DB/WAL size, RSS, pruning/vacuum cost, recursive CTE vs Rust traversal, BLOB vs payload-file tradeoff. |

## Risks

| Risk | Mitigation |
|------|------------|
| Invalid facts become durable because current validation is global. | Start with full-run commit after global validation. Split to provider transactions only when per-provider validators are complete. |
| Store becomes a second cache/invalidation system. | Reuse `InputSnapshot`, `LayerKey`, `SummaryKey`, `QueryKey`, dependency indexes, provider manifests, and `FactMeta`; do not add parallel identity vocabularies. |
| Public API leaks through convenience exports. | Keep store modules `pub(crate)`, return typed private structs, and add no-leak tests before any CLI promotion. |
| Unknown or budget-exceeded regions collapse into empty query results. | Persist `unknown_region` and `budget_event`; require query envelopes to include status/provenance and unknowns by default. |
| SQLite write contention or partial graph visibility. | Single writer, WAL, bounded busy timeout, read-only query connections, transaction-scoped generation commits, complete-generation filter. |
| Summary reuse hides precision loss or poisoning. | Store precision/confidence/provenance, validate summary manifests, recompute-and-diff locally, keep remote registry deferred. |
| DB size and payload growth hurt large repos. | Keep payload indirection, benchmark BLOB vs files, add pruning/vacuum, and avoid storing source bodies by default. |
| Search is mistaken for semantic truth. | Search indexes only stable store document IDs and returns candidates; graph facts remain the validation path. |
| `polint check` reliability regresses on store corruption. | Treat store corruption as controlled diagnostic plus rebuild/skip path unless a command specifically requires store-backed results. |
