# Research Analysis: Local Semantic Store

Date: 2026-07-07

## Evaluation Criteria

The store was evaluated against polint-specific constraints:

- offline local CLI, no required daemon;
- macOS/Linux/CI portability;
- low install friction;
- durable cache/fact storage;
- deterministic output across cold/warm/parallel runs;
- mutable per-file invalidation;
- indexed graph queries and structured filters;
- future text and vector search;
- provenance, precision, unknowns, and budget status as first-class data;
- remote package-summary registry enabled later, not built now.

## Storage Comparison

| Option | Fit | Main Strength | Main Problem | Decision |
|---|---|---|---|---|
| SQLite/rusqlite | High | Durable embedded relational store with mature indexes, migrations, WAL, CTEs, FTS/JSON options | Requires SQL schema discipline and one-writer design | Primary v1 store |
| redb | Medium | Pure Rust, ACID, MVCC, ordered B-tree tables | No query planner; graph/filter queries require manual secondary indexes | Pure-Rust fallback |
| RocksDB | Low-medium | Large write-heavy KV workloads | C++ build/tuning burden, no relational query layer | Reject for v1 |
| sled | Low | Simple Rust KV API | Durability/maintenance risk for core facts | Reject for durable facts |
| LMDB/heed | Medium | Very fast read-heavy mmap KV | Manual indexes, native/mmap lifecycle, map sizing | Possible niche fallback |
| Flat mmap/rkyv shards | Medium later | Immutable, content-addressed, registry-friendly | Not a mutable DB; hard schema evolution and pruning | Future export artifact |
| DuckDB | Medium | In-process analytics and columnar workflows | Less natural for mutable per-file graph facts | Not primary |

Conclusion: the moment graph and filter queries become product requirements,
SQLite dominates the first implementation because it handles both storage and
query ergonomics while staying embedded.

## Graph Query Comparison

| Option | Fit | Use | Decision |
|---|---|---|---|
| SQLite adjacency tables and recursive CTEs | High | Used-by, neighbors, callers/callees, bounded reachability | v1 default |
| Scoped Rust traversal over SQLite-loaded subgraphs | High | Path evidence, taint paths, budgets, unknown propagation | v1 path-heavy layer |
| Kuzu embedded graph DB | Medium-later | Exploratory Cypher/property graph work | Not v1; optional experiment only |
| SurrealDB embedded | Low-medium | Multi-model document+graph store | Too broad for v1 |
| RDF/Oxigraph/SPARQL | Low | Interoperable knowledge graph | Poor fit for source evidence paths |
| Datalog/datafrog | Medium-later | Derived relations, fixed-point reachability, taint | Derived layer, not persistence |
| CodeQL-like query runtime | Later | Expert variant analysis | Too much public API now |

Conclusion: persist the canonical graph in SQLite with typed edge tables. Use
Rust algorithms or a small Datalog relation engine for specific derived queries
only after the fact model is stable.

## Search Comparison

| Option | Fit | Use | Decision |
|---|---|---|---|
| Tantivy | High | Lexical/BM25/faceted search over symbols, docs, evidence, snippets | Phase 1 search |
| sqlite-vec | Medium | Small/medium local embedding side index close to SQLite | Experimental phase 2 |
| sqlite-vss | Low-medium | Faiss-backed vector extension | Avoid in core |
| LanceDB | Medium-later | Vector + FTS + columnar multimodal store | Later only |
| Qdrant Edge | Medium-later | Embedded vector DB with payload filters | Later if proven |
| USearch | Medium-later | Lightweight ANN sidecar | Fallback if sqlite-vec too slow |
| hnsw_rs | Low-medium | Pure Rust ANN primitive | Research spike only |
| DuckDB VSS | Low-medium | Vector joins and analytics | Not CLI core |

Conclusion: separate lexical search and vector search. Tantivy is stable enough
for lexical search. Vector search needs model/chunking/provenance decisions and
should not be part of deterministic `check`.

## Existing polint Architecture Fit

The current codebase already has the right concepts. The local semantic store
should reuse them instead of inventing a parallel cache system:

- `analysis_kernel::provider` has provider manifests with inputs, outputs,
  language scope, schema versions, cache policy, and precision ceiling.
- `analysis_kernel::incremental` already has `InputSnapshot`, `LayerKey`,
  `QueryKey`, `SummaryKey`, dependency indexes, invalidation, layer cache, and
  quarantine concepts.
- `analysis_kernel::metadata` already has `FactFamily`, `FactMeta`, precision,
  confidence, provenance, and validation status vocabulary.
- `analysis_kernel::validation` already gates fact metadata, stable keys,
  semantic indexes, topology, MIR, CFG, calls, domains, summaries, entrypoints,
  aliases, semantic graph, refined calls, and data flow.
- `symbol_graph` and `module_graph` use normalized paths, deterministic maps,
  stable keys, and private query helpers.
- `analysis/data_flow/query.rs` and `policy_queries.rs` already expose
  `Found`, `NotFound`, `Unknown`, and `BudgetExceeded` semantics.

The store should become the persistent backing for these layers, not a separate
product surface.

## Failure Modes To Design Against

- Query results differ between cold and warm runs because SQLite insertion order
  leaks into output. Mitigation: stable sort order in all public queries.
- Recursive CTE returns incomplete or duplicate paths around cycles. Mitigation:
  depth caps, visited sets, property tests, path-count budgets.
- Summary payloads bloat the DB. Mitigation: benchmark BLOBs versus adjacent
  content-addressed files and keep payload layout behind a facade.
- Rule-pack edits invalidate analysis layers unnecessarily. Mitigation: keep
  rule execution digests separate from analysis input/layer digests.
- Remote registry constraints pollute the local MVP. Mitigation: preserve
  content-addressed manifests and trust fields, but do not build networking.
- Vector search makes `check` nondeterministic. Mitigation: explicit embedding
  lockfiles and no live embedding inference in deterministic commands.
- SQL schema becomes accidental public API. Mitigation: expose typed SDK views
  and CLI JSON envelopes only.
- Search results are treated as facts. Mitigation: search returns candidates
  linked to evidence; analysis facts still come from providers and validation.

## Open Questions

- Should large summary payloads live as SQLite BLOBs, adjacent files, or a
  hybrid? This needs real payload-size benchmarks.
- How much graph traversal should stay in SQL versus Rust for path-heavy
  queries? Start simple and measure.
- What exact document schema should Tantivy index? Decide after the first graph
  and evidence result envelopes exist.
- What embedding model, chunker, and dimensions should vector search use? Defer
  until the lexical and graph query surfaces expose real search needs.
