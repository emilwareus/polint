# Source Index: Local Semantic Store

Date: 2026-07-07

This track relies mostly on official documentation and implementation source
code rather than academic papers, because the decision is an embedded storage
and product architecture choice.

## SQLite Official Documentation

Source URL: https://www.sqlite.org/features.html

Publisher / project: SQLite

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: SQLite feature overview for embedded, zero-configuration,
transactional relational storage.

## SQLite WAL

Source URL: https://sqlite.org/wal.html

Publisher / project: SQLite

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Write-ahead logging behavior and concurrency model.

## SQLite Query Planner

Source URL: https://www.sqlite.org/queryplanner.html

Publisher / project: SQLite

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Index planning and multi-column query behavior relevant to used-by,
neighbors, and structured filters.

## SQLite Recursive CTEs

Source URL: https://sqlite.org/lang_with.html

Publisher / project: SQLite

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Recursive common table expressions for bounded graph traversal.

## SQLite FTS5

Source URL: https://sqlite.org/fts5.html

Publisher / project: SQLite

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: candidate

Short note: Full-text search option. Still prefer Tantivy for the first lexical
search layer because Tantivy is Rust-native and search-specialized.

## SQLite JSON1

Source URL: https://sqlite.org/json1.html

Publisher / project: SQLite

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: candidate

Short note: JSON support. Useful for small metadata blobs, not core query
columns.

## rusqlite Documentation

Source URL: https://docs.rs/rusqlite/

Publisher / project: rusqlite

Publication or revision date: current docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Rust SQLite binding and feature set.

## redb Documentation

Source URL: https://docs.rs/redb

Publisher / project: redb

Publication or revision date: current docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Pure-Rust ACID embedded database and fallback candidate.

## Tantivy Repository And Documentation

Source URL: https://github.com/quickwit-oss/tantivy

Publisher / project: Tantivy / Quickwit

Publication or revision date: repository commit indexed in REPO-INDEX, accessed
2026-07-07

Source type: source code / official docs

Status: summarized

Short note: Rust full-text search engine candidate for lexical search.

## sqlite-vec Repository And Documentation

Source URL: https://github.com/asg017/sqlite-vec

Publisher / project: sqlite-vec

Publication or revision date: repository commit indexed in REPO-INDEX, accessed
2026-07-07

Source type: source code / official docs

Status: summarized

Short note: SQLite vector extension candidate for experimental local embedding
search.

## Glean Documentation

Source URL: https://glean.software/docs/introduction/

Publisher / project: Glean

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Typed fact store and derived predicate architecture reference.

## CodeQL Documentation

Source URL: https://codeql.github.com/docs/codeql-overview/about-codeql/

Publisher / project: GitHub CodeQL

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Code-as-data, query packs, path evidence, and model-pack reference.

## SCIP Documentation

Source URL: https://scip-code.org/docs.html

Publisher / project: SCIP

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Symbol/occurrence interchange identity reference.

## Sourcegraph Code Search And Navigation

Source URL: https://sourcegraph.com/docs/code-search/queries

Publisher / project: Sourcegraph

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Query/filter UX and honest precision product reference.

## Semgrep Taint Documentation

Source URL: https://docs.semgrep.dev/writing-rules/data-flow/taint-mode/overview

Publisher / project: Semgrep

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Source/sink/sanitizer/barrier vocabulary for user-facing taint
queries and future rule promotion.

## Kuzu Documentation And Repository

Source URL: https://kuzudb.github.io/docs/

Repository URL: https://github.com/kuzudb/kuzu

Publisher / project: Kuzu

Publication or revision date: current online docs and repository page,
accessed 2026-07-07

Source type: official docs / source repository

Status: summarized

Short note: Embedded property graph database and Cypher-style query comparison
point. Rejected as v1 default; keep as optional later experiment/reference.

## SurrealDB Embedded Rust Documentation

Source URL: https://surrealdb.com/docs/languages/rust/embedding

Publisher / project: SurrealDB

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Embedded multi-model/document/graph option. Rejected as v1 default
because the store surface is broader than polint needs.

## DuckDB Rust And Recursive CTE Documentation

Source URL: https://duckdb.org/docs/current/clients/rust.html

Related URL: https://duckdb.org/docs/current/sql/query_syntax/with

Publisher / project: DuckDB

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Embedded analytics engine and recursive CTE reference. Rejected as
primary fact store because mutable graph-neighbor workloads are not its core
fit.

## petgraph Documentation

Source URL: https://docs.rs/petgraph/

Publisher / project: petgraph

Publication or revision date: current docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Rust graph algorithm library candidate for scoped in-memory path
queries loaded from SQLite indexes.

## datafrog Documentation

Source URL: https://docs.rs/datafrog/

Publisher / project: datafrog

Publication or revision date: current docs, accessed 2026-07-07

Source type: official docs

Status: candidate

Short note: Datalog-like relation engine reference for future derived
reachability/taint layers. Not a persistence layer.

## Joern Code Property Graph Documentation

Source URL: https://docs.joern.io/code-property-graph/

Publisher / project: Joern

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Code property graph and traversal design reference for future
security analysis. Not a v1 dependency.

## Kythe Documentation

Source URL: https://kythe.io/docs/kythe-overview.html

Publisher / project: Kythe

Publication or revision date: current online docs, accessed 2026-07-07

Source type: official docs

Status: summarized

Short note: Cross-language graph/schema and verifier reference. Not a v1
dependency.
