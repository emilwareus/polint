# Research Analysis: Accuracy, Complexity, And Tradeoffs

This file collects the deeper research claims behind `FINAL-REPORT.md`.

## Core Thesis

Data-flow precision is determined by the whole analysis stack:

```text
entrypoints
  + call graph
  + CFG
  + symbol/reference binding
  + place/access-path model
  + heap/points-to abstraction
  + summary abstraction
  + source/sink/sanitizer models
  + solver
```

A better solver cannot recover a missing entrypoint, missing call edge, missing source, or wrong sanitizer model.

## Product Thesis: Agent-Extensible Modeling

Classic data-flow systems try to ship enough framework and library models to be useful everywhere. polint should ship native defaults, but it should also treat repo-local models as a first-class part of the analysis lifecycle.

The research question is therefore not only:

```text
Which solver should we use?
```

It is:

```text
Which native facts, summaries, unknowns, and model hooks let agents safely improve data-flow precision for this repository?
```

The engine should measure two modes:

- **default mode:** native CFG, value-flow, access paths, direct-call flow, summaries, and unknown/havoc facts;
- **extended mode:** default mode plus validated repo-local sources, sinks, sanitizers, barriers, summaries, additional flow steps, entrypoints, trust boundaries, and call graph models.

The extended mode remains deterministic static analysis. Agent-authored models are inputs that bind to source spans and symbols; they are not trusted free-form conclusions.

## Algorithm Complexity Notes

### Monotone CFG worklist

For a finite-height lattice:

```text
cost ~= number_of_edge_relaxations * transfer_cost
worst ~= O(E * H * transfer_cost)
```

This is the most important v1 building block. It supports local reaching definitions, liveness-like facts, nilness/nullness, constant propagation, and local taint.

### Sparse value-flow

Sparse value-flow replaces repeated CFG propagation with def-use/value edges:

```text
build ~= O(N + def_use_edges)
reachability ~= O(V + E)
```

The cost moves to lowering language syntax into stable places and values. This is a good tradeoff for polint because rules usually ask value-flow questions, not arbitrary CFG-state questions.

### Function summaries

Summaries form a finite lattice:

```text
summary_size = param/return/field/source/sink/unknown relations
cost ~= iterations_until_fixed_point * (local_summary_cost + call_edges * compose_cost)
```

This is production-proven in Pysa-like systems and compositional taint systems. It is the first global layer polint should build.

### IFDS/IDE

IFDS tabulation:

```text
general IFDS ~= O(E * D^3)
locally separable IFDS ~= O(E * D)
```

The theorem gives precision for valid call/return paths, but only after a finite fact domain and ICFG are supplied. Access paths, aliases, heap abstractions, and framework models still determine real accuracy.

### Points-to

Andersen-style inclusion points-to is classically cubic in the worst case. Modern solvers use SCC collapsing, worklists, bitsets, BDDs, hybrid sets, on-the-fly call graph construction, and type filters to scale.

Practical precision knobs:

- field-sensitive vs field-based;
- flow-sensitive vs flow-insensitive;
- context-insensitive vs object/type/call-site-sensitive;
- heap abstraction by allocation site/type/access path;
- library modeling scope.

### Datalog and relational fixed points

Semi-naive Datalog evaluation can be very fast with good indexes and SCC scheduling, but recursive joins can still explode. It is excellent for internal experiments and whole-program derived facts; it is too much surface area for a public rule API in polint v1.

### Incrementality

IncIDFA's key result is not just speed. It proves from-scratch precision for generic monotone IDFA updates by processing affected SCCs carefully. That means polint should store dependency edges between:

- files;
- functions;
- summaries;
- call graph edges;
- data-flow edges;
- query results.

## Accuracy Failure Modes

| Failure mode | Effect | Required engine behavior |
|---|---|---|
| Missing call edge | False negative for interprocedural flow. | Unknown/havoc fact or capability diagnostic. |
| Over-broad call edge | False positives and state explosion. | Algorithm label and confidence/status. |
| Missing entrypoint | Entire reachable slice absent. | Domain pack/lifecycle diagnostics. |
| Missing source/sink | Vulnerability/policy not expressible. | Rule/config source-sink APIs; agent-proposed specs. |
| Over-broad source/sink | False positives. | Exact/non-exact match mode and provenance. |
| Missing sanitizer | False positives. | Sanitizer/barrier models with evidence. |
| Over-broad sanitizer | False negatives. | Conservative default and test fixtures. |
| Access path too shallow | Field/property false positives. | Configured depth and wildcard provenance. |
| Access path too deep | Memory/time blowup. | Caps, widening, and query budgets. |
| Dynamic feature ignored | Silent false negatives. | Unsupported/unknown diagnostics. |

## Research Implications For AI-Agent Rules

AI agents will be good at writing rules only if the engine exposes stable facts:

- "find this source" should bind to concrete source spans;
- "this sink" should bind to a call/reference fact;
- "this sanitizer" should bind to a function/call fact;
- "this flow exists" should include a path and precision label;
- "this flow may exist" should say which unresolved edge made it uncertain.

The SemTaint and AdaTaint direction is valuable, but the engine must stay deterministic. LLMs can propose facts; polint must validate, bind, cache, and display them.

For polint's product path, the most important agent workflow is gap closure:

```python
unknowns = dataflow.unknowns()
candidate_model = agent.inspects_repo_and_proposes_model(unknowns)

if candidate_model.binds_to_static_facts():
    run_fixture_validation(candidate_model)
    rerun_default_vs_extended_delta()
```

This turns uncertainty into work items. A generic analyzer can only say "unknown framework behavior." polint can say "this route wrapper, sanitizer, or generated client needs a repo-local model."

## Benchmark Harness Recommendation

Every data-flow provider should report:

```text
functions analyzed
CFG nodes/edges
data-flow nodes/edges
access-path count
summary count and average size
summary iterations
interprocedural edges
unknown/havoc edges
runtime
memory if available
cache hits/misses
query count and path-search time
repo models loaded
model sources/sinks/sanitizers/summaries added
unknown/havoc reduced by models
paths added by models
paths pruned by model sanitizers/barriers
model validation failures
```

Every curated fixture should report:

```text
expected source
expected sink
expected path
expected sanitizer/barrier
expected unknown if unsupported
precision
recall
false positives
false negatives
algorithm tier
language feature category
model id if expected path depends on a repo model
default-vs-extended precision
default-vs-extended recall
```

Without this harness, "more precise" will become an opinion rather than a measured product property.
