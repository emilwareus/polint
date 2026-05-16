# Research Analysis: Accuracy, Complexity, And Tradeoffs

This file collects the deeper research claims behind `FINAL-REPORT.md`.

## Core Thesis

Call graph construction is a precision/cost frontier, not a solved lookup problem. The same algorithm name can have different accuracy in different frameworks because support for lambdas, reflection, native methods, static initializers, module loaders, dynamic imports, decorators, callbacks, and framework entrypoints is implemented outside the core algorithm.

The architecture implication for polint is to store:

- algorithm name;
- provider name;
- repo model id, when an edge comes from an agent-authored model;
- precision tier;
- configured scope;
- validation status;
- unresolved reason;
- source evidence;
- runtime/memory counters;
- graph delta relative to lower tiers.

## Product Thesis: Native Defaults Plus Repo Models

The older static-analysis assumption is that the tool must fit every codebase with one generic implementation. polint can use a higher-ceiling model: native algorithms provide the reliable baseline, while agents author repo-local call graph models for internal frameworks and conventions.

This changes the research question from:

```text
Which universal algorithm gives the best graph?
```

to:

```text
Which native facts, algorithm tiers, unresolved facts, and model hooks let agents improve this repository's graph safely?
```

The engine should therefore evaluate two graphs:

- **default graph:** native syntax, binding, semantic providers, and unresolved facts;
- **extended graph:** default graph plus validated repo-local models.

The extended graph is not "less static." It is still produced by the native engine. The repo model is an input with provenance, validation status, and cache participation.

## Complexity Cheat Sheet

| Family | Cost model | Main precision knob | Main failure mode |
|---|---:|---|---|
| AST call-site scan | `O(N)` | Parser coverage | No target resolution. |
| Symbol/import binding | `O(N + R + I)` | Scope/import resolver quality | Dynamic exports/imports, shadowing, aliasing. |
| CHA | `O(C * subtypes)` after hierarchy build | Class/interface hierarchy completeness | Over-approximates dispatch; can still miss modern runtime features. |
| RTA | `O(reachable_methods + allocations + dispatch_checks)` | Entrypoints and allocation reachability | Incomplete roots or dynamic allocation mechanisms. |
| VTA/XTA/MTA | `O(constraint_edges * type_set_growth)` | Variable/field/type propagation granularity | More expensive; field/context choices alter soundness. |
| Andersen points-to | Worst-case cubic; optimized solvers are workload-dependent | Field/context/heap abstraction | Can hit memory/time walls, especially with libraries. |
| `k`-CFA | Often exponential-ish in practical context variants as `k` grows | Context depth | Context explosion. |
| Function-token flow | `O(flow_edges * function_token_set_growth)` | Object/property/callback modeling | Dynamic property names, `eval`, monkeypatching. |
| Demand-driven resolution | Proportional to query slice | Seed selection | Misses edges outside the slice. |

## Accuracy Lessons

### Micro-benchmarks are necessary but insufficient

Unimocg uses feature micro-benchmarks to show JVM feature support. This is useful because it reveals missing support for reflection, serialization, threads, native calls, class loading, and dynamic proxies. But Total Recall shows that micro-benchmark pass rates do not directly translate to real application edge recall.

Polint should keep both:

- small feature fixtures for every language feature;
- larger dynamic-baseline fixtures for precision/recall estimates.

### Graph size is not accuracy

Total Recall explicitly shows that smaller call graphs are not always more precise and larger call graphs are not always higher recall. A graph can be small because it missed true edges, or large because it conservatively modeled impossible edges.

Polint should therefore report graph size only as a cost/shape metric, never as a precision claim.

### Theoretical partial orders break in implementations

In theory, CHA should be less precise and at least as recall-oriented as RTA, and RTA should over-approximate more precise type/points-to variants. In practice, Java frameworks violate this due to feature-specific implementation gaps. A "more precise" implementation can add true edges that a "less precise" implementation misses.

Polint should test provider deltas. If `RTA` adds edges that `CHA` missed, that is not automatically wrong; it may mean the `CHA` provider lacks a feature model.

### Dynamic languages need scoped truth

Python and JavaScript papers show strong precision is possible only in scoped settings:

- PyCG: whole-program, high precision, moderate recall.
- JARVIS: application-centered, better scaling and accuracy for selected entrypoints.
- JS tools: no single tool dominates; combinations improve recall but lower precision.

Polint should let rules opt into scoped graph views:

```text
direct only
direct + module binding
direct + module binding + value-flow
include heuristic framework routes
include unresolved/havoc edges
```

## Benchmark Harness Recommendation

For every call graph provider, collect:

```text
files analyzed
functions discovered
call sites discovered
resolved edges
unresolved call sites
edges by algorithm
edges by status
runtime
peak memory if available
cache hits/misses
graph delta from previous tier
repo models loaded
repo model edges added
unresolved reduced by repo models
repo model validation failures
```

For curated fixtures, add:

```text
expected call sites
expected edges
expected unresolved reasons
precision
recall
false positives
false negatives
feature category
model id if expected edge is model-produced
default-vs-extended precision
default-vs-extended recall
```

This makes call graph development empirical instead of subjective.
