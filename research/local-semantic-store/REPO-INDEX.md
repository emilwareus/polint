# Repository Index: Local Semantic Store

Date: 2026-07-07

Cloned repositories live under `research/local-semantic-store/repos/`, which is
gitignored.

## rusqlite

Repo: `https://github.com/rusqlite/rusqlite.git`

Commit: `77a171935741ce56da0bb67277068cb0c08aaa85`

Why inspected: primary Rust binding candidate for the SQLite-backed local
semantic store.

Key source paths:

- `src/lib.rs`
- `src/transaction.rs`
- `src/statement.rs`
- `src/blob/mod.rs`
- `src/pragma.rs`
- `src/backup.rs`
- `src/vtab/mod.rs`
- `src/load_extension_guard.rs`
- `README.md`

Relevant algorithms/domains: SQLite connection management, prepared statements,
transactions, BLOB access, pragmas, backup/restore, optional virtual table and
extension support.

Notes and caveats: choose the `bundled` feature for consistent SQLite features
across developer machines and CI. Keep rusqlite behind an internal wrapper.

## redb

Repo: `https://github.com/cberner/redb.git`

Commit: `b2da02d1017b827ae3a18f99cca1ae1b74b9520b`

Why inspected: previous leading candidate for pure-Rust summary/cache storage
and still the best pure-Rust fallback.

Key source paths:

- `src/db.rs`
- `src/transactions.rs`
- `src/table.rs`
- `src/multimap_table.rs`
- `src/tree_store/btree.rs`
- `src/transaction_tracker.rs`
- `README.md`

Relevant algorithms/domains: ACID embedded database, MVCC readers/writer,
ordered B-tree tables, multimap tables, range iteration.

Notes and caveats: good for content-addressed summary lookup. Less good as the
primary graph/query store because secondary indexes, migrations, filters, and
path queries all become app-owned logic.

## Tantivy

Repo: `https://github.com/quickwit-oss/tantivy.git`

Commit: `6b8bd7b8847c87686117a0e3a400bd6377afb8fd`

Why inspected: primary lexical-search candidate for symbols, evidence bundles,
summary text, code snippets, and future agent query workflows.

Key source paths:

- `src/index/index.rs`
- `src/indexer/index_writer.rs`
- `src/query/`
- `src/collector/`
- `src/fastfield/`
- `query-grammar/src/`
- `columnar/src/`
- `README.md`

Relevant algorithms/domains: full-text indexing, BM25-style search, query
parsing, collectors, facets/ranges/fast fields, incremental indexing.

Notes and caveats: strong lexical search fit, not a vector database. Search
documents should reference semantic-store stable IDs and digests.

## sqlite-vec

Repo: `https://github.com/asg017/sqlite-vec.git`

Commit: `04d28bd21773981e2d266bbf6aa4efbd011eb4f6`

Why inspected: closest-fit experimental vector side index because it stays in
the SQLite ecosystem.

Key source paths:

- `sqlite-vec.c`
- `sqlite-vec-diskann.c`
- `sqlite-vec-ivf.c`
- `sqlite-vec-ivf-kmeans.c`
- `site/api-reference.md`
- `site/compiling.md`
- `tests/test-general.py`
- `tests/test-insert-delete.py`
- `tests/test-metadata.py`
- `README.md`

Relevant algorithms/domains: SQLite vector extension, vector storage, metadata,
insert/delete behavior, ANN experiments.

Notes and caveats: useful for an experimental offline semantic-search slice.
Do not make it part of deterministic `check` or stable core storage yet.

## Local polint Source

Repo: current workspace.

Why inspected: ensure the recommendation fits existing architecture.

Key source paths:

- `crates/polint/src/analysis_kernel/mod.rs`
- `crates/polint/src/analysis_kernel/provider.rs`
- `crates/polint/src/analysis_kernel/incremental/`
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs`
- `crates/polint/src/analysis_kernel/incremental/keys.rs`
- `crates/polint/src/analysis_kernel/metadata.rs`
- `crates/polint/src/analysis_kernel/validation.rs`
- `crates/polint/src/module_graph/`
- `crates/polint/src/symbol_graph/`
- `crates/polint/src/analysis/data_flow/query.rs`
- `crates/polint/src/policy_queries.rs`

Relevant algorithms/domains: provider DAG, input snapshots, layer/query/summary
keys, fact metadata, validation, stable IDs, deterministic maps, bounded query
statuses.

Notes and caveats: the semantic store should plug into these existing concepts
instead of introducing a parallel cache, graph, or fact ID system.

## Non-Cloned Repositories Checked As Rejected Or Reference Options

These were inspected through official docs, GitHub pages, or `git ls-remote`
rather than cloned, because they are rejected v1 defaults or interoperability
references rather than implementation dependencies.

### Kuzu

Repo: `https://github.com/kuzudb/kuzu`

Observed commit: `89f0263cc7a1fd9c396d2c4953747a013556a7f9`

Why inspected: embedded property-graph database comparison point for local
graph queries.

Notes and caveats: useful design reference for graph workloads, but not the v1
default because it introduces a second database, native dependency surface, and
Cypher/product API pressure. The GitHub page was observed as archived in 2025.

### SCIP

Repo: `https://github.com/sourcegraph/scip`

Observed commit: `e01e97efac2f6b8c266b4d04825f1f1eab7b8f6c`

Why inspected: symbol/occurrence interchange reference.

Notes and caveats: useful for future import/export identity and navigation
interop. Not a storage engine.
