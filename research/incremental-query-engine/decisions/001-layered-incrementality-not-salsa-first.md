# Decision 001: Build Layered Native Incrementality Before Salsa-First Queries

## Status

Recommended.

## Context

Polint needs incremental analysis for a multi-language, agent-extensible static
analysis engine. The analysis kernel research already calls for typed fact
layers, provenance, extension merges, validation, cache keys, and invalidation.
This research deepens the cache/query part.

Candidates:

1. Use Salsa as the core engine immediately.
2. Build a Datalog/Souffle-like relation engine first.
3. Build only file/module-level cache invalidation.
4. Build a native layered incremental substrate, then add demand queries and
   relation engines where measured.

## Decision

Choose option 4.

Polint should first implement:

```text
InputSnapshot
LayerKey
LayerCacheManifest
DependencyIndex
ChangeSet
InvalidationPlan
CacheStats
```

Then add:

```text
QueryKey
QueryTrace
in-run memoization
summary SCC cache
diagnostic cache
daemon/watch red-green mode
relation/fixpoint sub-engine
```

## Rationale

### Salsa-first is too constraining now

Salsa is excellent for demand queries, but polint's first problem is broader
than query memoization. It needs persistent batch caches, extension digests,
validation status, official tool invocation keys, language lifecycle digests,
and layer-level provenance. These should be explicit native concepts first.

### Datalog-first is the wrong top-level abstraction

Recursive relations matter for call graphs, data flow, and summaries. They do
not cover source snapshots, lifecycle config, extension validation, diagnostic
fingerprints, official tool invocation metadata, or public SDK ergonomics.

### File/module-only incrementality is too weak long term

Module-level invalidation is good for syntax, imports, symbols, and package
metadata. It does not scale to one alias query, one path explanation, one
function summary, or one rule-requested data-flow query.

### Native layered incrementality fits the product

Polint's differentiator is agent-authored Rust extensions that improve analysis
accuracy. Cache correctness must include extension code, extension inputs,
validation status, precision ceilings, and model files. That requires a native
contract regardless of the query backend.

## Consequences

Positive:

- avoids freezing public SDK around a query framework;
- keeps cache manifests understandable;
- supports batch mode before daemon mode;
- lets cheap layers stay simple;
- lets expensive views become demand-driven later;
- supports extension quarantine and provenance;
- keeps future Salsa or relation backends possible.

Negative:

- more native infrastructure to build;
- must implement dependency tracing and invalidation carefully;
- less immediate reuse of a mature query crate;
- requires discipline to avoid ad hoc caches.

## Revisit Criteria

Revisit the decision if:

- the native demand query engine starts replicating most of Salsa badly;
- benchmarks show query-level red-green behavior is needed earlier than
  expected;
- relation volumes make simple SCC recompute too expensive;
- extension cache validation becomes too complex without a stronger transaction
  engine.

Possible later choices:

- adopt Salsa for the demand query subsystem only;
- use a Souffle-like or custom semi-naive relation sub-engine;
- build a DICE-inspired transaction/projection layer;
- add Differential Dataflow-like traces for selected recursive relations.
