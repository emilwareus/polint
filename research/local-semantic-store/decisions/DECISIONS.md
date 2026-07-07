# Decisions: Local Semantic Store

Date: 2026-07-07

## D1. Primary Local Store

Decision: use SQLite via rusqlite bundled as the primary mutable local semantic
store.

Rationale: graph queries, structured filters, migrations, and future search
manifests need an indexed relational store more than a pure key-value cache.

Confidence: high.

## D2. redb Role

Decision: keep redb as the pure-Rust fallback and possible content-addressed
summary cache, not the default v1 store.

Rationale: redb is strong for ordered KV and summary lookup, but graph/filter
queries would require too much manual indexing and migration logic.

Confidence: high.

## D3. Remote Registry

Decision: do not build the remote package-summary registry now.

Rationale: local and CI cache value must be proven first. Registry operations,
trust, distribution, signing, and public corpus management would distract from
the local product.

Confidence: high.

## D4. Registry-Ready Seam

Decision: preserve content-addressed package summaries, package/version
identity, schema versions, provenance, validation metadata, trust hooks, and
recompute-and-diff metadata.

Rationale: the local store should not block future registry work.

Confidence: high.

## D5. Graph Query Backend

Decision: use typed SQLite adjacency/evidence tables plus bounded SQL/Rust
queries. Do not use a graph database as v1 default.

Rationale: keeps one canonical store, avoids dual-store drift, and fits the
offline CLI.

Confidence: high.

## D6. Public Query Surface

Decision: future query surface should be purpose-built `polint graph` commands,
not a public raw graph API or query language.

Rationale: agent and human workflows need stable answers with evidence, not
exposure to unstable internal graph shapes.

Confidence: high.

## D7. Lexical Search

Decision: use Tantivy for first lexical search over stable semantic-store
documents.

Rationale: Rust-native full-text search is a better fit than overloading SQLite
FTS for all search needs.

Confidence: medium-high.

## D8. Vector Search

Decision: defer vector search to an experimental sqlite-vec side index behind a
store/search boundary.

Rationale: vector search needs explicit model, chunking, provenance, and
determinism decisions. It should not affect `polint check`.

Confidence: medium.

## D9. Existing Kernel Reuse

Decision: the semantic store must reuse provider manifests, `InputSnapshot`,
layer/query/summary keys, fact metadata, and validation gates.

Rationale: a parallel cache or graph ID system would fork the engine.

Confidence: high.
