# Incremental Benchmark Plan

## Goal

Measure whether the incremental engine is correct, useful, and honest.

Correctness comes first:

- no stale facts;
- deterministic cache keys;
- correct invalidation after source/config/rule/extension changes;
- extension quarantine when needed.

Performance comes second:

- warm no-op runs should reuse most cacheable layers;
- body-only edits should avoid module-wide churn;
- public API edits should invalidate real dependents;
- summary-equal edits should stop at the summary boundary;
- extension edits should not blow away unrelated native syntax caches.

## Metrics

Record per run:

```text
cold wall time
warm wall time
peak memory
files parsed
layers reused
layers verified
layers recomputed
layers dropped
layers quarantined
queries executed
query cache hits
summary SCCs recomputed
summary SCCs backdated
diagnostics reused
diagnostics refreshed
dependency edges recorded
cache bytes read/written
```

Record per layer:

```text
layer kind
provider id/version
input count
dependency edge count
output digest
output bytes
hit/miss/recompute/quarantine
reason
```

## Edit Scenarios

### No-op Warm Run

Run the same analysis twice.

Expected:

- all safe cacheable layers reused;
- no diagnostics change;
- no provider recompute except non-cacheable layers;
- output deterministic.

### Function Body Edit

Change implementation without changing imports, exports, or public signature.

Expected:

- changed file syntax/local facts recompute;
- module graph reused;
- dependent public symbol facts reused;
- local summary recomputed;
- callers untouched if summary digest unchanged.

### Public API Edit

Change exported function signature or class/member shape.

Expected:

- public API digest changes;
- dependent references/call resolution invalidated;
- downstream summaries and diagnostics recompute as needed.

### Import Shape Edit

Add/remove/change import.

Expected:

- import layer changes;
- module graph/import ownership recomputes for affected file/package;
- dependent resolution facts recompute.

### Lifecycle Edit

Change `go.mod`, `go.sum`, `tsconfig`, package lockfile, Python package metadata,
or JVM classpath/build file.

Expected:

- affected language lifecycle digest changes;
- language-owned import/module/type facts invalidated;
- unrelated language layers stay reusable where possible.

### Rule Option Edit

Change a rule option in `.polint.toml`.

Expected:

- diagnostics for that rule recompute;
- syntax/import/module/symbol layers remain reused;
- cache stats explain the narrow invalidation.

### Extension Code Edit

Change a Rust extension provider.

Expected:

- extension digest changes;
- extension output and merged layers quarantined/recomputed;
- native layers reused;
- diagnostics depending on extension facts refreshed or downgraded.

### Model File Edit

Change an agent-generated source/sink/summary/framework model file.

Expected:

- affected extension/model layers recompute;
- unrelated extension outputs reused;
- validation status participates in cache action.

### Summary SCC Edit

Change a function inside a recursive call SCC.

Expected:

- affected SCC recomputed;
- equal summaries backdated;
- changed summaries propagate to callers;
- benchmark reports SCC closure size.

## Fixture Sizes

Use tiers:

| Tier | Size | Purpose |
|---|---:|---|
| Tiny | 5-20 files | Deterministic correctness tests. |
| Small | 100-500 files | Common repo behavior. |
| Medium | 1k-5k files | Cache and dependency-index stress. |
| Large | 10k+ files | Optional local benchmark, not mandatory CI. |

## External Benchmarks To Reuse

Reuse existing language/tool benchmarks where possible:

- TypeScript projects with realistic `tsconfig` and package lockfiles;
- Go module/workspace fixtures from gopls/go tools patterns;
- Python import graph fixtures inspired by Pyright/Pyrefly/Pyre;
- call/data-flow benchmarks already cataloged in the evaluation-harness
  research;
- polint-native fixtures for extension/model invalidation because external
  benchmarks do not cover this product-specific behavior.

## Pass Criteria For First Implementation

- no-op warm run reuses syntax/import/module/symbol layers;
- body-only edits do not invalidate module graph;
- public API edits invalidate real dependents;
- rule option edits do not invalidate syntax/import/module layers;
- extension code/model edits quarantine dependent facts;
- provider/schema version change drops old cache;
- cache output is deterministic across repeated runs;
- benchmark report includes reasons for recompute/quarantine.
