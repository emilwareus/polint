# Validation Plan: Local Semantic Store

Date: 2026-07-07

## Required Before Implementation Is Considered Sound

The store decision is locked, but the implementation must prove the details
with fixtures and benchmarks.

## Storage Microbenchmarks

Create synthetic and real-repo datasets at several sizes:

- 100k symbols, 500k references, 1M edges;
- 1M symbols, 5M references, 10M edges;
- 5M symbols, 25M references, 50M edges, if hardware permits.

Measure:

- cold ingest time;
- warm update time after editing 1 file, 10 files, and 1 package;
- DB size;
- WAL size during ingest;
- peak RSS;
- query latency p50/p95 for used-by, callers, callees, neighbors, and filters;
- bounded path query latency and budget behavior;
- pruning and vacuum behavior after deleting a package or branch of files.

Compare:

- SQLite BLOB payloads versus adjacent content-addressed payload files;
- recursive CTE traversal versus Rust-loaded scoped traversal;
- key indexes with and without covering columns.

## Determinism Tests

For the same source/config/toolchain:

- cold build then query;
- warm build then query;
- partial invalidation then query;
- process restart then query;
- randomized provider/file insertion order then query;
- different Rayon worker counts then query.

Expected result: byte-identical normalized JSON for all stable query outputs.

## Crash And Recovery Tests

Simulate process termination:

- during ingest transaction;
- during summary payload write;
- during migration;
- during WAL checkpoint;
- during search index rebuild.

Expected result:

- old generation remains readable or new generation is complete;
- no mixed graph;
- clear diagnostic if rebuild is needed;
- no silent invalid output.

## Query Correctness Fixtures

Build small fixture repos for:

- used-by on local symbol;
- cross-file reference;
- cross-package import;
- direct call;
- refined call;
- unresolved dynamic call with unknown status;
- caller/callee cycles;
- path query with cycle;
- taint path with sanitizer/barrier;
- budget-exceeded path query;
- summary boundary path segment;
- extension-provided edge with provenance.

Expected result: stable node/edge/path IDs, correct precision/status, and no
unknown hidden as not-found.

## Product UAT Scenarios

1. SQL injection investigation:
   - agent finds a SQL sink;
   - asks whether user-controlled data reaches it;
   - graph query returns found path, unknown, or budget-exceeded with evidence.

2. API impact:
   - agent asks what uses a function or export before refactoring;
   - query returns direct references, callers, module boundaries, and unknowns.

3. Architecture boundary:
   - agent asks whether code in one package calls another forbidden layer;
   - query returns paths and provenance suitable for promoting into a rule.

4. Similar sanitizer discovery:
   - lexical search finds candidate validators;
   - later vector search suggests similar code;
   - agent verifies before converting to source/sink/barrier model rows.

## Search Validation

Tantivy:

- stable document IDs;
- deterministic rebuild from store manifest;
- search result points back to store facts;
- no absolute path leakage;
- schema migration test.

sqlite-vec experiment:

- embedding lockfile includes model digest, chunker version, dimensions, metric,
  normalization, and source digest;
- no live embedding inference in `polint check`;
- result ranking is marked heuristic unless verified by graph facts.

## Gates

Do not promote a query command to public CLI until:

- JSON schema is documented;
- deterministic tests pass;
- unknown/budget behavior is visible;
- fixtures cover at least Go and TS/JS where facts exist;
- benchmark data shows acceptable latency on a large real repo;
- docs clearly state precision limits.
