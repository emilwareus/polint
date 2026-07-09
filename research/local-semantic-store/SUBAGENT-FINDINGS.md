# Subagent Findings: Local Semantic Store

Date: 2026-07-07

Six dedicated research threads were run and then reviewed against the current
repository architecture.

## Embedded Store

Finding: SQLite/rusqlite is the best primary mutable local store. redb is the
best pure-Rust fallback. RocksDB is overkill for v1, sled should be avoided for
durable facts, LMDB/heed is viable but less ergonomic, and mmap/rkyv shards are
better as future immutable package-summary artifacts.

Reviewer action: accepted. This changes the earlier Static Analysis 2.0 Q13
decision from `redb first` to `SQLite/rusqlite first` because the product now
requires graph queries, structured filters, and search-index manifests.

## Local Graph Query Layer

Finding: use SQLite as the canonical persisted graph and summary store. Add
typed adjacency/evidence tables for used-by, callers/callees, neighbors, module
boundaries, and per-file invalidation. Use recursive CTEs for bounded CLI
queries and scoped Rust traversal or a small Datalog/datafrog layer for
path-heavy reachability and taint. Keep Kuzu optional/experimental, not v1.

Reviewer action: accepted with one constraint: Datalog/datafrog is a possible
derived-query engine later, not part of the first store milestone.

## Vector And Semantic Search

Finding: build the search boundary now, not a vector database. Use Tantivy for
lexical search in phase 1, sqlite-vec experimentally in phase 2, and consider
Qdrant Edge/LanceDB/USearch only if real product traces require filtered ANN or
hybrid sparse+dense search.

Reviewer action: accepted. Vector search stays outside deterministic `check`
until model/chunker/provenance lockfiles are explicit.

## Existing Code-Intelligence Architectures

Finding: copy Glean's typed immutable facts and derived views, CodeQL's
evidence-rich query/model-pack ideas, Salsa's deterministic query layering,
SCIP's symbol identity, Sourcegraph's precision-labeled navigation/search UX,
and Semgrep's rule ergonomics. Do not import Kythe/Joern/stack-graphs as v1
dependencies.

Reviewer action: accepted. This reinforces typed SDK views and a private store,
not a public raw graph or query language.

## Product And CLI Query Surface

Finding: future query surface should be a purpose-built read-only
`polint graph` family: used-by, neighbors, callers, callees, path, taint. It
should return stable JSON envelopes with precision, provenance, status, nodes,
edges, paths, findings, unknowns, and summary data. Do not expose SQL, Cypher,
QL, raw CFG/MIR, solver internals, or CI gating semantics through graph queries.

Reviewer action: accepted. `polint check` and `polint review` remain the core
products; `polint graph` is the exploratory understanding surface.

## Repo Architecture Explorer

Finding: current polint already has the correct spine: provider DAG,
`InputSnapshot`, layer/query/summary keys, dependency index, layer cache,
quarantine, fact metadata, validation gates, deterministic module/symbol graph
patterns, and bounded query statuses. The store should plug into this spine and
stay crate-private.

Reviewer action: accepted. The implementation plan now explicitly reuses these
existing concepts and rejects a parallel cache/graph ID system.

## Convergence

All threads converge on the same high-level design:

- SQLite/rusqlite primary local semantic store;
- typed adjacency/evidence tables for graph queries;
- no remote registry now, registry-ready manifests later;
- no public raw graph or query language yet;
- Tantivy lexical search first;
- vector search experimental and provenance-locked;
- store private, SDK/CLI results public only after promotion.
