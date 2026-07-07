# Final Report: Local Semantic Store

Date: 2026-07-07

## Recommendation

Static Analysis 2.0 should use **SQLite via `rusqlite` with bundled SQLite** as
the canonical embedded local semantic store.

This is a product and architecture decision, not only a storage-library
decision. The store is the local knowledge base behind:

- `polint check`: repo-local Rust policy rules over typed SDK views;
- `polint review`: agentic review with evidence, uncertainty, and diff focus;
- future `polint graph`: local graph exploration for used-by, neighbors,
  callers/callees, paths, impact, and taint-style questions;
- future local search: lexical first, vector/embedding search later.

`redb` remains the best pure-Rust fallback, but it is no longer the primary
recommendation. The new graph-query and structured-filter requirements make
SQLite's relational indexes, query planner, migrations, and recursive CTEs more
valuable than a simpler ordered key-value store.

## Product Decision

Do not build the remote registry of precomputed package summaries now.

Build an offline local store that is registry-ready:

- package and workspace identities are explicit;
- dependency summaries are content-addressed;
- schema versions are stored and migrated;
- every persisted fact has precision, status, provenance, provider, and
  validation metadata;
- payloads can be recomputed and diffed against stored summaries;
- trust hooks and signatures can be added later without changing the fact
  identity model.

The local store should be useful even if a remote registry never exists. The
registry is an optimization and distribution layer, not a core correctness
dependency.

## Architecture Decision

Use a **polint-owned store facade** over SQLite. The public product surface
should remain typed SDK views and future CLI query envelopes, not raw SQL,
SQLite tables, graph database APIs, or internal provider IDs.

The store should contain normalized tables for:

- source files, packages, modules, source sets, and dependency identities;
- symbols, definitions, references, imports, exports, and resolution facts;
- functions, call sites, direct/refined call edges, unresolved calls;
- MIR/CFG/data-flow/evidence summary identities where those families exist;
- summary manifests and content-addressed summary payloads;
- graph adjacency indexes for callers, callees, used-by, contains, imports,
  flows, evidence, and module-boundary relationships;
- query and layer-cache metadata keyed by existing `InputSnapshot`, `LayerKey`,
  `SummaryKey`, and provider schema/version digests.

Large immutable payloads can be stored either as SQLite BLOBs or adjacent
content-addressed files. The key decision is that the canonical mutable index is
SQLite, while large payload layout stays an implementation detail.

## Query Decision

Add a future read-only `polint graph` command family rather than a generic graph
shell:

- `polint graph used-by --at path:line:col`
- `polint graph neighbors --symbol <id> --edge refs,calls,flows --depth 1`
- `polint graph callers --symbol <id>`
- `polint graph callees --symbol <id>`
- `polint graph path --from <selector> --to <selector> --edge calls|dataflow|any`
- `polint graph taint --source <pattern> --sink <pattern> [--barrier <pattern>]`

Global flags should be JSON-first and agent-friendly:

- `--format human|json|jsonl`
- `--path <glob>`
- `--include-tests`
- `--min-precision exact|setup-aware|syntax|conservative|heuristic`
- `--provenance native,summary,extension,model,query,synthetic`
- `--unknowns include|only|exclude`
- `--max-depth`, `--max-paths`, `--limit`

The output should use one stable envelope:

```json
{
  "version": 1,
  "schema": "polint.graph.result",
  "command": "used-by",
  "query": {},
  "status": "complete",
  "precision": "setup-aware",
  "nodes": [],
  "edges": [],
  "paths": [],
  "findings": [],
  "unknowns": [],
  "summary": {}
}
```

Every node, edge, and path should carry stable IDs, relative paths only,
precision, status, provenance, and evidence links. Do not expose absolute
workspace paths, raw parser IDs, source bodies, or internal solver/provider IDs.

## Search Decision

Build a search boundary now, but do not make vector search part of the first
storage milestone.

Recommended sequence:

1. **Lexical search** with Tantivy over symbols, documentation-like summaries,
   diagnostics/evidence text, rule explanations, and selected code snippets.
2. **Experimental vector search** with sqlite-vec behind a feature/unstable
   command once the chunking, model provenance, lockfile, and deterministic
   rules are clear.
3. Promote Qdrant Edge, LanceDB, USearch, or another ANN engine only if actual
   product traces show Tantivy plus sqlite-vec cannot satisfy local agent
   exploration.

Embeddings must not be live nondeterministic inputs to `polint check`.
Embeddings belong in an explicit local index with model ID, model digest,
chunker version, dimensions, metric, normalization, and content digest.

## What To Copy From Existing Systems

Copy ideas, not whole systems:

- From Glean: typed facts, schema discipline, derived views, stacked/additional
  fact layers.
- From CodeQL: evidence-rich query results, path explanations, model rows for
  sources/sinks/summaries/barriers.
- From Salsa/rust-analyzer: deterministic query layering, stable inputs versus
  derived values, early cutoff principles.
- From SCIP/LSIF: stable symbol and occurrence identity for navigation
  interchange.
- From Sourcegraph: honest precision labels and navigation/search product UX.
- From Semgrep: approachable rule iteration and clear JSON/SARIF-style outputs.
- From Joern/Kythe/stack-graphs: design references for graph/query/navigation,
  not v1 dependencies.

## Rejected As V1 Defaults

- **Kuzu**: useful embedded graph database ideas, but the GitHub repository was
  observed as archived in 2025 and it introduces a second database and Cypher
  surface. Keep as a design reference or later experimental backend only.
- **SurrealDB**: too broad and async/multi-model for polint's deterministic
  static-analysis store.
- **DuckDB**: strong analytics engine, but less natural for mutable per-file
  fact updates and graph-neighbor workloads.
- **RocksDB**: powerful, but C++ build and tuning burden are too high for the
  first local CLI store.
- **sled**: avoid for durable v1 facts due maturity and maintenance risk.
- **LMDB/heed**: excellent read-heavy mmap KV, but manual indexes and mmap
  lifecycle make it less ergonomic than SQLite for evolving query surfaces.
- **Flat mmap/rkyv shards**: good future export/package-summary artifacts, not
  a mutable local database.
- **Cypher/QL/SPARQL public query language**: too much public API and support
  burden before facts and query patterns stabilize.

## Confidence

High confidence in SQLite/rusqlite as the v1 local semantic store choice.

Medium confidence in the exact BLOB versus adjacent-file payload layout. That
should be benchmarked with real summary sizes.

Medium confidence in Tantivy as the first search engine. It is a strong fit for
lexical search, but search should not block the first semantic-store milestone.

Low-to-medium confidence in sqlite-vec as the eventual vector engine. It is the
best small embedded experiment because it stays close to SQLite, but it is not a
decision to make vector search a stable core feature yet.
