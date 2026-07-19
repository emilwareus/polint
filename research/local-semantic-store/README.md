# Local Semantic Store

Date: 2026-07-07

## Research Question

What embedded, offline storage and query architecture should polint use for
Static Analysis 2.0 facts, summaries, graph edges, evidence, text search, and
future vector search?

The store must support the product direction in
`research/static-analysis-2.0/README.md`: `polint check` for repo-local Rust
rules, `polint review` for agentic review, and a future local `polint graph`
surface for questions like used-by, neighbors, callers, callees, paths, taint
reachability, impact, and semantic search.

## Short Answer

Use **SQLite through `rusqlite` with the bundled SQLite feature** as the primary
mutable local semantic store.

Do not use an external graph database, remote registry, Datalog/QL runtime, or
vector database as the first implementation. Build a polint-owned storage
facade over normalized SQLite tables, stable IDs, schema migrations,
provenance/precision/status columns, content-addressed payloads, and covering
indexes for the expected graph/query workloads.

Layer additional engines only where they are clearly the right tool:

- **petgraph or a small internal traversal library** for scoped path-heavy
  graph queries loaded from SQLite.
- **Tantivy** for lexical search over symbols, summaries, evidence bundles, and
  documentation-like text.
- **sqlite-vec** only as an experimental offline vector side index after the
  core store and lexical search have shipped.
- **immutable content-addressed shards** later for package-summary export and a
  possible remote registry, not as the first mutable store.

## Why This Matters

Earlier Static Analysis 2.0 summary-store research leaned toward `redb` for a
pure-Rust local cache. That was reasonable when the primary requirement was
content-addressed summary lookup.

The product requirement changed: the local CLI should eventually make the
program graph queryable, including structured filters and semantic search. That
moves the dominant workload from key-value lookup to indexed relational and
graph-adjacent queries. SQLite is the better default for that shape because it
gives polint a durable single-file store, transactions, migrations, query
planning, multi-column indexes, recursive CTEs for bounded traversals, FTS
options, JSON support, and mature operational behavior without requiring a
daemon.

## Scope

Included:

- local persistence for analysis facts, summaries, graph edges, evidence, and
  invalidation metadata;
- queryable local graph and future `polint graph` CLI shape;
- lexical and vector search technology choices;
- remote-registry seams without building the registry now;
- integration with the existing analysis kernel, layer cache, provider DAG, and
  validation gates.

Excluded from the first implementation:

- remote package-summary registry;
- public raw graph API;
- Cypher, QL, SPARQL, or another public query language;
- mandatory daemon or server process;
- live embedding generation during deterministic `polint check`;
- replacing `polint check` or `polint review` with graph queries.

## Status

The storage technology decision remains locked, but the first Phase 65 delivery
attempt was abandoned as an oversized implementation. Restart work must follow
the bounded slices and review disposition rules in the restart documents rather
than the original all-at-once sequencing.

Retained decisions:

- primary local store: SQLite/rusqlite bundled;
- graph backend: typed adjacency/evidence tables plus bounded Rust/SQL queries;
- search: Tantivy first, sqlite-vec later behind an experimental boundary;
- pure-Rust fallback: redb, if SQLite distribution proves unacceptable;
- remote registry: deferred, but schema must preserve registry-ready seams.

See:

- [RESTART-PLAN.md](RESTART-PLAN.md)
- [IDENTITY-READINESS.md](IDENTITY-READINESS.md)
- [REVIEW-FINDINGS-TRIAGE.md](REVIEW-FINDINGS-TRIAGE.md)
- [FINAL-REPORT.md](FINAL-REPORT.md)
- [RECOMMENDED_IMPLEMENTATION.md](RECOMMENDED_IMPLEMENTATION.md)
- [RESEARCH-ANALYSIS.md](RESEARCH-ANALYSIS.md)
- [VALIDATION.md](VALIDATION.md)
- [REPO-INDEX.md](REPO-INDEX.md)
- [PAPER-INDEX.md](PAPER-INDEX.md)
