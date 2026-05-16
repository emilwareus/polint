# Parallel Research Findings

Six parallel research agents were used to split the work by ecosystem and algorithm family.

## Python Agent

Key findings:

- Ty/Ruff is the most relevant Rust-native Python implementation reference for polint.
- The core pattern is places, use-def, reachability constraints, narrowing constraints, and type relations.
- Pyrefly validates a module-centric Rust architecture with flow types and recursive placeholders.
- Pyright validates a mature reference-specific flow/narrowing architecture.
- Pysa/CodeQL show that Python interprocedural precision comes from summaries and models.

Recommendation:

```text
Python = module graph + places + local narrowing + function/class/module value facts + summaries first; points-to second.
```

## TypeScript / JavaScript Agent

Key findings:

- TypeScript compiler is a type/narrowing engine, not a points-to engine.
- Oxc is the right Rust-native parser/semantic substrate already aligned with polint.
- TAJS and Jelly are the stronger references for JS abstract interpretation and points-to/call-graph style analysis.
- CodeQL JS proves the value of type tracking and API modeling for query authors.

Recommendation:

```text
TS/JS = Oxc semantic facts + TypeScript-style narrowing + object/function tokens + property/access-path flow + extension summaries.
```

## Go Agent

Key findings:

- `go/types`, `go/packages`, `go/ssa`, and `x/tools/go/callgraph` are the official oracles.
- Current `x/tools` includes static, CHA, RTA, and VTA call graph packages.
- Current `x/tools` snapshot inspected here does not include a `go/pointer` package.
- VTA is a practical middle tier between cheap CHA/RTA and heavier points-to.

Recommendation:

```text
Go = native type/method/interface facts validated against Go tools; VTA-like type/function propagation before Andersen.
```

## Java/JVM Agent

Key findings:

- JVM tools converge on CHA -> RTA -> VTA/type propagation -> Andersen/Spark/Doop/WALA -> context sensitivity.
- Doop's relational model and Soot/WALA/Spark options show how points-to precision policies should be separated from the fact model.
- Reflection, class loaders, Android/framework lifecycle, and incomplete classpaths require explicit model facts.

Recommendation:

```text
Java/JVM = use the JVM precision ladder as the general polint ladder; make reflection/framework models extension-provided.
```

## Neutral Compiler Agent

Key findings:

- LLVM AliasAnalysis is a provider-stack query interface.
- MemorySSA and SVF show that sparse flow-sensitive memory/value-flow should come after simpler facts.
- Rust borrow checking/Polonius is not general alias analysis, but shows the value of ownership facts and relational solvers.
- rust-analyzer validates incremental semantic database design.

Recommendation:

```text
Alias = query service over providers; sparse flow-sensitive refinements later.
```

## Algorithm Agent

Key findings:

- Andersen is the best baseline inclusion solver, but must be bounded and engineered with bitsets/SCC/deltas.
- Steensgaard is too coarse as the default.
- IFDS/IDE is for finite data-flow problems, not general heap points-to.
- Demand-driven and selective context sensitivity should be future precision tiers.

Recommendation:

```text
Implement type/value/place/local flow first, then bounded Andersen, then selective context/sparse refinement.
```
