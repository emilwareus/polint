# Benchmark And Validation Plan

This research track should feed the evaluation harness with targeted fixtures and external-oracle comparisons.

## Validation Modes

### 1. Differential Oracle Fixtures

Use external tools during development, not at runtime:

- Ty/Ruff/Pyright/Pyrefly for Python type/narrowing expectations.
- TypeScript compiler for TS narrowing expectations.
- Oxc for TS/JS scopes/references/CFG frontend expectations.
- `go/types`, `go/ssa`, Go callgraph RTA/VTA for Go expectations.
- Doop/WALA/Soot/SVF-style examples for points-to/call graph expectations.

### 2. Native Invariant Fixtures

Validate internal facts independent of external tools:

- every place has stable owner/scope identity;
- every access path is bounded;
- every `NoAlias` has evidence;
- every `MustAlias` has stronger evidence than overlapping points-to;
- budget exhaustion emits `Unknown`;
- extension facts preserve provenance;
- extension conflicts are deterministic.

### 3. Default Versus Agent-Extended Metrics

For each fixture suite, report:

```text
default_precision
default_recall
default_unknown_count
extended_precision
extended_recall
extended_unknown_count
extension_fact_count
extension_runtime_delta
```

This is central to polint's product thesis.

## Initial Fixture Suites

### Python

- `isinstance`/`issubclass`/`TypeGuard`/`TypeIs` narrowing.
- Guard aliases and invalidation after assignment.
- TypedDict/dataclass-like field facts.
- Decorator-added routes/callbacks.
- Dynamic `getattr` unknown.
- Function object passed through registry.

### TS/JS

- `typeof`, strict equality, truthiness, `in`, `instanceof`, discriminated unions.
- Object literal property flow.
- Function object callback registration.
- Optional chaining/nullish coalescing.
- Computed property known string key versus unknown key.
- Dynamic import/eval unknown.

### Go

- interface dispatch with CHA/RTA/VTA difference.
- pointer receiver/value receiver.
- function values and closures.
- field load/store through pointers.
- generics/aliases selector cases.
- reflection unknown.

### Java/JVM

- CHA versus RTA versus VTA method targets.
- field-sensitive versus field-based points-to.
- reflection unknown and extension-mode model.
- dependency-injection summary.
- context-sensitive precision example.

## Metrics

- points-to set size distribution;
- call-target set size distribution;
- alias query answer distribution;
- `Unknown` count by reason;
- false `NoAlias` count in oracle fixtures;
- false `MustAlias` count;
- runtime and memory per provider;
- cache hit rate;
- invalidation breadth after source or extension change.
