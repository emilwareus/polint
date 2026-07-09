# Evaluation Plan: Local Semantic Store

Date: 2026-07-07

## Benchmark Goals

Prove that SQLite/rusqlite can serve as the local semantic store before relying
on it for Static Analysis 2.0 implementation.

The benchmark must answer:

- Can cold ingest handle large fact graphs on laptop/CI hardware?
- Are warm updates proportional to changed files and dependent summaries?
- Are graph queries fast enough for agent workflows?
- Is output deterministic across cold/warm/parallel runs?
- Does DB size remain acceptable?
- Does crash recovery preserve complete generations?

## Synthetic Dataset Generator

Generate deterministic datasets with parameters:

- file count;
- package count;
- symbols per file;
- references per symbol;
- call edges per function;
- unresolved edge ratio;
- summary payload size distribution;
- unknown/budget event ratio;
- cross-package edge ratio.

Use a fixed seed and store generator parameters in benchmark output.

## Workloads

Ingest:

- empty DB cold ingest;
- add new package;
- edit one hot file;
- edit one leaf file;
- delete package;
- schema migration.

Queries:

- used-by for high fan-in symbol;
- used-by for low fan-in symbol;
- callers/callees for high fan-in function;
- neighbors depth 1, 2, 3;
- path query with cycles;
- path query with no result;
- taint-like source-to-sink reachability;
- unknowns only;
- precision/provenance filters.

Payloads:

- summary manifest only;
- summary BLOB inline;
- summary payload adjacent file;
- mixed BLOB/adjacent layout.

## Metrics

- wall time p50/p95;
- peak RSS;
- DB file size;
- WAL peak size;
- query allocations if available;
- rows scanned where available from query plan;
- number of invalidated layers;
- number of recomputed summaries;
- output byte digest.

## Acceptance Targets

Initial provisional targets, to be tuned against real repos:

- common one-hop graph queries under 200 ms warm on 1M-symbol dataset;
- high fan-in used-by under 1 s warm on 1M-symbol dataset;
- bounded path queries return partial/budget status rather than exceed configured
  time or memory budgets;
- one-file warm update avoids rebuilding unrelated package summaries;
- deterministic output digest across all cold/warm/parallel permutations;
- crash tests leave a readable complete generation or a clear rebuild
  diagnostic.

## Comparison Points

Primary:

- SQLite normalized schema with covering indexes.

Secondary experiments:

- redb manual indexes for the same used-by/callers/neighbors workloads;
- SQLite recursive CTE versus Rust-loaded traversal;
- inline BLOB versus adjacent content-addressed files;
- Tantivy lexical side index rebuild cost.

Do not spend implementation effort on RocksDB, Kuzu, DuckDB, or vector engines
until SQLite fails a concrete benchmark.
