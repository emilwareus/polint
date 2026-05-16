# Validation Plan

The goal is to ensure slicing and path evidence improve trust and accuracy
without hiding uncertainty or creating false precision.

## Validation Levels

| Level | What is validated |
|---|---|
| Schema | Evidence nodes, edges, bundles, paths, slices, unknowns, replay keys. |
| Identity | Every rendered node maps to a stable id and source span when available. |
| Edge legality | Edge kind is valid for the source/target node kinds and analysis provider. |
| Context | Interprocedural paths respect call/return matching. |
| Summary | Summary edges have provenance, precision, and expansion/opaque status. |
| Extension | Agent-authored model edges pass validation and merge policy. |
| Renderer | Human, JSON, and SARIF renderers preserve required path information. |
| Determinism | Same inputs produce stable node ordering, path ranking, and fingerprints. |
| Cache | Query results invalidate when graph/config/model/provider inputs change. |
| Benchmark | Default and extension-enabled path/slice quality are measured separately. |

## Required Fixture Families

### Local Dependence

- variable assigned then used;
- reassignment and shadowing;
- field/index access paths;
- destructuring/multiple assignment;
- branch condition controlling sink;
- loop condition controlling use;
- exception/throw/panic/return edge;
- dead code or unreachable branch.

### Thin Versus Full Slices

- value producer without control context;
- control condition required for full explanation;
- base pointer/address dependence;
- heap/field dependence;
- exception dependence;
- summary expansion.

Assertions:

```text
thin_slice subset full_slice
thin_slice contains direct producers
full_slice contains selected control dependencies
omitted edges are reported when budget/filter removes them
```

### Source-To-Sink Paths

- direct local source to sink;
- sanitizer breaks path;
- barrier blocks one label but not another;
- multiple paths, deterministic ranking;
- source through field/index;
- source through return value;
- source through parameter mutation;
- source through closure/callback;
- source through framework model edge.

### Interprocedural Context

- two callers of one callee, only one source reaches sink;
- recursive call with bounded depth;
- callback passed through wrapper;
- unresolved dynamic call produces unknown;
- summary edge produces compressed path;
- summary edge expansion reproduces local callee path;
- mismatched call/return path is rejected.

### Extension Evidence

- extension adds source/sink model;
- extension adds summary edge;
- extension adds framework dispatch edge;
- extension resolves an unknown call;
- extension tries to suppress native may edge and is rejected or downgraded;
- extension references nonexistent span and is rejected;
- extension lacks fixture for high-trust claim and becomes candidate-only.

## Path Quality Metrics

Measure these separately for default mode and extension-enabled mode.

| Metric | Meaning |
|---|---|
| Path recall | Expected source-to-sink paths found. |
| Path precision | Reported paths that match expected feasible or acceptable abstract paths. |
| Slice recall | Expected relevant statements/facts included. |
| Slice size | Number of visible and hidden nodes. Lower is better when recall holds. |
| Thin/full ratio | Thin slice size divided by full slice size. |
| Unknown rate | Unknown/havoc/model-missing edges per path or diagnostic. |
| Summary opacity | Fraction of paths using opaque summaries. |
| Context errors | Mismatched call/return paths admitted. Target: zero in context-matched modes. |
| Ranking quality | Expected primary path appears in top 1/top 3. |
| Renderer loss | Information present internally but missing from JSON/SARIF/human output. |
| Runtime | Query latency by mode and graph size. |
| Memory | Evidence graph view size and cache size. |

## External Benchmarks To Consider

Use external suites as evidence, but do not depend on them alone.

- SliceBench from SliceMate for Java/Python slicing comparison.
- CodeNetSlice-style Java/Python slicing cases from recent neural slicing work.
- CodeQL query tests for path output shape and source-to-sink behavior.
- Joern data-flow examples for CPG-style `reachableByFlows`.
- Semgrep taint tests for trace ergonomics and sanitizer/propagator behavior.
- FlowDroid/SecuriBench Micro/DroidBench style source-to-sink suites after the
  data-flow engine is ready.
- Native polint fixtures for provenance, cache, extension merge, unknowns, and
  renderer invariants.

External benchmarks are not enough because polint's differentiator is validated
agent extension. Native fixtures must test default-vs-extension deltas.

## Determinism Tests

For every path query:

```python
first = run_query(seed=1)
second = run_query(seed=999)

assert first.paths == second.paths
assert first.replay_key == second.replay_key
assert first.omitted == second.omitted
```

Run under parallel provider scheduling and changed file iteration order.

## Cache Invalidation Tests

Invalidate path/slice query results when any of these change:

- source file content;
- semantic operation ids/version;
- CFG/control-dependence facts;
- def-use/data-dependence facts;
- call graph facts;
- summary facts;
- alias/points-to facts;
- source/sink/sanitizer/barrier models;
- extension crate source or compiled artifact;
- extension validation state;
- language setup: Go build tags/module roots/package patterns, `tsconfig`,
  classpath, Python environment metadata;
- query mode, edge filter, budget, ranking, renderer options;
- provider version or domain version.

## Negative Tests

- A path that enters a callee through call site A and returns to call site B must
  be rejected in context-matched mode.
- A sanitizer path must not be displayed as reaching the sink unless the query
  intentionally asks for blocked paths.
- An unvalidated extension must not suppress a native may path.
- A path with unknown/havoc edges must be labeled as such.
- SARIF rendering must not claim completeness if internal evidence was partial.
- A budget-truncated slice must show omitted-region metadata.

## Validation Verdicts

Use explicit verdicts:

- `PASS`: evidence can influence diagnostics and normal output.
- `WARN`: evidence can be reported but precision is downgraded.
- `CANDIDATE`: evidence can be shown for agent review but not used for a strong
  diagnostic claim.
- `BLOCK`: evidence is rejected and cannot influence diagnostics.

## Open Validation Gaps

- Need final decision on JSON evidence schema versioning.
- Need benchmark harness adapter for SliceBench or equivalent once source is
  available and licensing is reviewed.
- Need SARIF renderer tests against a real SARIF consumer.
- Need large-repo performance baselines after evidence graph view exists.
