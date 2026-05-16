# Validation Plan

The goal is to ensure summaries improve capability without hiding uncertainty or
creating false precision.

## Validation Levels

| Level | What is validated |
|---|---|
| Schema | Payload shape, domain version, required fields, allowed enum values. |
| Identity | Subject resolves; signature hash matches; source span exists. |
| Access path | Roots and path components are valid for the language/signature. |
| Precision | Provider is allowed to claim the requested precision. |
| Merge | Extension merge mode is allowed and conflicts are detected. |
| Fixture | Expected facts/paths/diagnostics pass on controlled examples. |
| Benchmark | Default-vs-extended deltas are measured. |
| Cache | Summary invalidates when source/setup/config/model/extension/deps change. |

## Required Fixture Families

### Control

- direct no-return call;
- panic/throw/raise/reject;
- normal plus exceptional return;
- async/await/yield;
- defer/finally/cleanup;
- callback invoked now versus stored for later.

### Calls

- direct function call;
- method receiver call;
- function-valued parameter;
- callback passed through wrapper;
- dynamic import/reflection unresolved;
- framework synthetic dispatch.

### Data Flow

- argument to return;
- argument field to return field;
- receiver mutation;
- argument to sink;
- sanitizer/barrier;
- by-side-effect propagation;
- recursive TITO.

### Memory/External Effects

- read/write receiver;
- read/write parameter field;
- global/module mutation;
- file/network/env/process/db/log effects;
- unknown external call;
- missing dependency setup.

### Resource/Typestate

- acquire/release;
- open/close;
- lock/unlock;
- transaction begin/commit/rollback;
- awaitable created/awaited/unawaited;
- project-specific state transition via extension.

## Extension Validation Matrix

| Extension behavior | Required checks |
|---|---|
| Adds may call edge | selector, target exists or synthetic target declared, fixture if framework edge. |
| Adds flow summary | source/target access paths valid, flow kind valid, evidence span. |
| Adds sanitizer | sanitizer kind scoped; cannot erase unrelated taint kinds. |
| Adds no-return | source evidence or fixture; conflicts with observed returns become diagnostics. |
| Replaces native summary | high trust, fixture coverage, explicit approval. |
| Suppresses summary edge | diagnostic-only by default; cannot suppress internal may facts without strict validation. |

## Cache Invalidations To Test

- source body changes;
- function signature changes;
- called function summary changes;
- language version changes;
- Go build tags/module roots/package patterns change;
- `go.mod`, `go.sum`, `go.work` change;
- `tsconfig` or dependency declaration changes;
- extension crate source changes;
- model data changes;
- summary domain version changes;
- provider version changes;
- budget/widening configuration changes.

## Accuracy Metrics

Measure default and extension-enabled modes separately.

| Metric | Meaning |
|---|---|
| Summary precision | exact/local/summary/heuristic/unknown distribution. |
| Unknown rate | unresolved and setup-missing summaries per KLOC/function. |
| Call coverage | resolved call edges and unresolved call sites. |
| Flow recall | expected source-to-sink/TITO paths found. |
| Flow precision | unexpected paths reported. |
| Effect recall | expected external/memory/resource effects found. |
| Effect precision | spurious effects reported. |
| Cache hit rate | warm-run reuse by summary domain. |
| Extension delta | improvement from validated agent summaries. |
| Runtime/memory | cold and warm summary computation cost. |

## Monotonicity Tests

For may analyses, adding a higher-precision provider must not silently drop
required may-behavior unless the domain explicitly changes from over-approximate
to a validated exact/refined mode.

Test:

```python
default = run_analysis(mode="default")
extended = run_analysis(mode="extended")

assert required_may_edges(default) <= required_may_edges(extended) or has_refinement_proof()
assert unknowns_resolved_by_extension_have_evidence(extended)
```

This is important because call graph and framework model research shows
"higher precision" systems can accidentally lose edges.

## Validation Verdicts

Use explicit verdicts:

- `PASS`: summary/model can influence analysis.
- `WARN`: summary/model can be stored and reported, but precision downgraded.
- `BLOCK`: summary/model cannot influence analysis.
- `QUARANTINE`: keep as candidate for agent review only.
