# Recommended Implementation: Local Semantic Store

Date: 2026-07-07

## Implementation Thesis

Implement a private `analysis_kernel` persistence backend named conceptually
`SemanticStore`, backed by SQLite/rusqlite. It should be an internal substrate
used by providers, layer caches, summary caches, evidence builders, and future
query commands.

It must not become a public SDK. Public users get typed SDK views, policy query
objects, diagnostics/evidence bundles, and later stable CLI JSON envelopes.

## Crate Shape

Recommended internal module shape:

```text
crates/polint/src/analysis_kernel/store/
  mod.rs
  connection.rs
  migrations.rs
  schema.rs
  ids.rs
  ingest.rs
  query.rs
  graph.rs
  payloads.rs
  search_manifest.rs
  tests.rs
```

The exact path can change, but the ownership boundary should not: the store is
inside the analysis kernel, not in `sdk`, `runner`, or language adapters.

## Dependency

Add `rusqlite` with the `bundled` feature when implementation starts.

Rationale:

- avoids relying on system SQLite versions;
- keeps the CLI offline and single-process;
- unlocks SQLite features consistently across macOS, Linux, and CI;
- avoids the RocksDB/Kuzu/native-graph database footprint.

Use a narrow wrapper so `rusqlite` does not leak through provider APIs.

## Store Lifecycle

Store path should be under polint's existing cache directory, with workspace and
configuration identity encoded in the path or manifest.

Use:

- `PRAGMA journal_mode = WAL` for local concurrency;
- `PRAGMA foreign_keys = ON`;
- `PRAGMA user_version` for schema migrations;
- `busy_timeout` with bounded retries;
- write transactions for provider/layer commits;
- read-only connections for query commands when possible.

All writes should be atomic at a layer/provider generation boundary. A killed
process must leave either the old generation or the new generation, never a
mixed graph.

## Identity Model

Every persisted row must be keyed by stable semantic identity, not parser IDs or
iteration order.

Identity inputs:

- schema version;
- provider ID and provider schema version;
- toolchain/frontend version;
- language/project/module identity;
- repo-relative normalized path;
- source digest or package/version digest;
- relevant config digest;
- extension/model digest when applicable;
- stable semantic key material from existing symbol/module/MIR/fact builders.

Reuse existing concepts:

- `InputSnapshot`;
- `LayerKey`;
- `QueryKey`;
- `SummaryKey`;
- `DiagnosticKey`;
- `FactFamily`;
- `FactMeta`;
- provider manifests and precision ceilings;
- validation gates.

Do not create a second fact-family registry or second stable-ID vocabulary.

## Schema Families

Start with minimal tables that are enough for summaries and graph queries. Keep
payload tables separate from index tables.

Core metadata:

- `store_manifest`
- `schema_migrations`
- `provider_generation`
- `input_snapshot`
- `layer_entry`
- `layer_dependency`
- `validation_event`

Identity and topology:

- `package`
- `source_set`
- `source_file`
- `module`
- `dependency`

Semantic index:

- `symbol`
- `definition`
- `reference`
- `import`
- `export`
- `resolution`

Graph and evidence indexes:

- `node`
- `edge`
- `edge_evidence`
- `adjacency_forward`
- `adjacency_reverse`
- `unknown_region`
- `budget_event`

Summaries:

- `summary_manifest`
- `summary_payload`
- `summary_dependency`
- `summary_projection`

Search manifests:

- `search_index_manifest`
- `search_document_manifest`
- `embedding_index_manifest`

The first schema should favor explicit tables over opaque JSON. JSON is fine for
small extensible metadata fields, not for core join/filter columns.

## Graph Query Execution

Use SQLite indexes for the common local questions:

- symbol by selector or stable ID;
- references by target symbol;
- callers by callee function;
- callees by caller function;
- imports by module/package;
- one-hop and bounded-depth neighbors;
- summary dependencies by digest;
- unknown/budget regions by path or provider.

Use recursive CTEs only for bounded queries with explicit limits, deterministic
ordering, and cycle guards.

For path-heavy queries, load a scoped subgraph from SQLite into a Rust traversal
layer, likely `petgraph` or a custom deterministic adjacency structure. This
keeps path evidence, budgets, unknown propagation, and ranking easier to audit
than deeply nested SQL.

For taint/data-flow reachability, prefer existing demand-query and data-flow
status semantics:

- `Found`
- `NotFound`
- `Unknown`
- `BudgetExceeded`

Do not silently translate unknowns into no path.

## CLI Query Surface

Do not expose SQL. Add a future `polint graph` facade once enough facts exist.

The first commands should be purpose-built:

- `used-by`
- `neighbors`
- `callers`
- `callees`
- `path`
- `taint`

The CLI should return deterministic JSON arrays sorted by stable ID and source
span. Human output can be a rendering of the same result envelope.

See [implementation/QUERY-CLI-SKETCH.md](implementation/QUERY-CLI-SKETCH.md).

## Search

Do not block the first semantic-store implementation on search.

When search starts:

1. build a `SearchCorpus` abstraction over stable store document IDs;
2. write a Tantivy index for lexical queries;
3. store the Tantivy manifest in SQLite;
4. keep index rebuilds deterministic by content digest;
5. add sqlite-vec only as an experimental side index with an explicit embedding
   lockfile.

Search results must point back to store stable IDs and evidence spans. Search
does not create facts by itself.

## Remote Registry Seam

Even though the registry is deferred, the store must preserve:

- package/version identity;
- package source provenance;
- summary digest;
- summary schema version;
- provider/toolchain/config/model digests;
- validation status and validation diagnostics;
- trust status placeholder;
- recompute-and-diff metadata.

The eventual registry can distribute immutable summary manifests and payloads.
The local SQLite store imports, validates, indexes, and can recompute them. The
registry never becomes the only source of truth.

## Migration And Compatibility

Use numbered SQL migrations with tests:

- migration applies to empty DB;
- migration applies to prior schema;
- migration is idempotent or refuses with a clear error;
- unknown future schema returns a controlled diagnostic;
- downgrade is not required, but rebuild must be clear and safe.

Because this is an internal store, table schema is not public API. CLI JSON
envelopes and SDK views are public once promoted.

## Sequencing

1. Add the store facade and SQLite connection/migration skeleton.
2. Persist store manifest, input snapshots, provider generations, and layer
   entries without changing analysis behavior.
3. Persist semantic index facts: files, symbols, references, imports,
   resolutions, and fact metadata.
4. Add xref queries for definitions/references/used-by internally.
5. Persist direct call/refined call edge indexes.
6. Persist summary manifests and payload digests.
7. Add bounded graph query internals and fixtures.
8. Add hidden/unstable `polint graph` experiments only after result envelopes
   and precision/status behavior are ready.
9. Add Tantivy lexical search as a side index.
10. Add sqlite-vec experiments only after embeddings have a lockfile and
    deterministic invalidation story.
