# Research Analysis: Accuracy, Complexity, And Product Fit

## Evaluation Frame

CFG quality has to be judged by the analysis that consumes it. A CFG that is excellent for compiler optimization may be poor for source diagnostics. A CFG that is excellent for editor type narrowing may be poor for data-flow reachability. A CFG that is bytecode-exact may hide the source construct a rule author wants to report.

The useful evaluation dimensions are:

| Dimension | Question |
|---|---|
| Source fidelity | Can diagnostics point to the construct the user wrote? |
| Semantic fidelity | Does the graph model execution order and abrupt flow honestly? |
| Query scalability | Can rules ask reachability/dominance/control-dependence questions cheaply? |
| Language completeness | Are tricky constructs modeled or explicitly unsupported? |
| Extension fit | Can agents add repo-specific knowledge without corrupting native facts? |
| Cache fit | Can the graph and derived facts be invalidated precisely? |

## Algorithm Complexity

| Operation | Practical complexity | Notes |
|---|---:|---|
| Operation CFG construction | `O(N + E)` | N lowered operations, E emitted edges. Exception/finally duplication can increase E. |
| Basic-block construction | `O(N + E)` | Split at branch targets, joins, exits, handlers, cleanup boundaries. |
| Reachability | `O(N + E)` | Per selected view. |
| Dominators, simple iterative | Worst `O(N^2 * E)` depending on representation | Often fine for small function CFGs; deterministic and easy to validate. |
| Dominators, Lengauer-Tarjan | Near-linear `O(E alpha(E,N))` in common formulations | More complex; worth adding if benchmarks justify it. |
| Dominators, Semi-NCA | Usually fast, simple enough for production | LLVM uses a Semi-NCA family in generic dominator tree construction. |
| Postdominators | Dominator cost on reversed graph | Requires artificial exit and graph-view policy. |
| Control dependence, simple edge walk | `O(E * h)` | h is postdominator-tree height. Fine first. |
| Control dependence, APT/output-sensitive | `O(E)` preprocessing plus output-sensitive queries | Use if materialized CDG becomes too large. |
| SCC/loop detection | `O(N + E)` | Tarjan/Kosaraju or DFS backedge classification. |

## Accuracy Lessons By Tool

### Go `go/ssa`

Accuracy strengths:

- branch, loop, switch, select, short-circuit, defer, panic, recover, goroutine spawn, and SSA instruction order are explicit enough for analysis;
- dominator APIs exist;
- source positions remain available through instructions.

Accuracy limits:

- public Go SSA is not a stable cross-language polint API;
- panic/recover semantics still need a policy;
- goroutine interleavings are not local CFG;
- build tags/tests/module loading determine what code exists.

Product fit: high as a semantic reference for native Go CFG. Do not expose raw Go SSA.

### Oxc CFG

Accuracy strengths:

- Rust-native;
- already has `ControlFlowGraph`, `BasicBlock`, typed edges, error harnesses, finalizers, and semantic builder integration;
- good fit with current TS/JS parser stack.

Accuracy limits:

- young feature surface;
- needs fixture validation for all JS/TS constructs polint wants to claim;
- Oxc internal IDs should not become public polint IDs.

Product fit: high as implementation substrate/reference, with polint-owned facts.

### TypeScript/Pyright Flow Nodes

Accuracy strengths:

- excellent for flow-sensitive type narrowing;
- lazy backward evaluation avoids doing unnecessary work;
- explicitly models branch labels, loop labels, assignments, calls, conditions, finally gates, and pattern narrowing.

Accuracy limits:

- not a general CFG;
- reference-specific, checker-state-driven;
- unsuitable as a public graph API.

Product fit: high for future `TypeNarrowing<'_>`, medium/low for CFG.

### CodeQL

Accuracy strengths:

- strong query-facing API;
- `ControlFlowNode` and `BasicBlock` separation;
- dominance/reachability APIs;
- language libraries document limitations such as JS `finally` imprecision;
- source-level mapping is designed for queries.

Accuracy limits:

- database/extractor architecture is not embeddable as polint’s native engine;
- language implementations vary in precision;
- some postdominance/control-dependence APIs are absent or limited in specific languages.

Product fit: very high as SDK/query design reference, not a dependency.

### Soot/WALA/OPAL

Accuracy strengths:

- mature JVM exceptional CFGs;
- explicit normal/exceptional successors;
- bytecode/Jimple/SSA/TAC layers;
- WALA and OPAL have control-dependence utilities;
- Soot’s `ExceptionalUnitGraph` precision knobs are an important reference.

Accuracy limits:

- bytecode/source mapping is difficult;
- classpath and exception type precision materially affect results;
- Java source constructs may be desugared beyond recognition;
- direct dependency would violate native Rust goal.

Product fit: high as Java/JVM research reference, not an early dependency.

### Checker Framework

Accuracy strengths:

- source-level Java CFG;
- explicit regular and exceptional exit blocks;
- good modeling for `try`, `finally`, try-with-resources, synchronized, lambdas, method exceptions;
- designed for dataflow.

Accuracy limits:

- Java-specific and javac-integrated;
- conservative exception modeling;
- not suitable as polint core dependency.

Product fit: best source-level Java reference.

### CPython

Accuracy strengths:

- authoritative CPython bytecode/control-transfer implementation;
- `flowgraph.c` and `codegen.c` show how Python semantics lower to blocks and bytecode;
- exception tables, `with`, `yield`, `await`, and pattern matching are concrete.

Accuracy limits:

- bytecode is implementation-specific and unstable;
- source diagnostics need higher-level anchors;
- alternative Python implementations may differ.

Product fit: semantic reference only.

### TAJS/Jelly

Accuracy strengths:

- TAJS shows mature JavaScript flow-graph and abstract-interpretation architecture;
- Jelly shows modern JS/TS pragmatism around call graph, promises, modules, and access paths.

Accuracy limits:

- TAJS is older and archived;
- Jelly focuses more on call graph/points-to than CFG rule API;
- both make tradeoffs not appropriate for direct copying.

Product fit: useful for later JS abstract interpretation and call graph/data-flow choices.

## Soundness And Honesty

CFG facts should not use a binary exact/inexact story. Each graph view needs precision metadata:

- normal-only view can be exact for normal control but intentionally incomplete for exceptions;
- exception-conservative view may over-approximate;
- `finally` modeling may introduce infeasible paths if cleanup bodies are merged rather than duplicated or gated;
- async/yield facts may be source-correct but not schedule-correct;
- repo-local extension facts may be validated but still agent-asserted.

Rules should default to high-confidence facts, but advanced rules and agents should be able to inspect precision and unknown facts.

## Max-Capability Path

The highest-capability long-term architecture is not “make CFG perfect.” It is:

```text
CFG
  + source spans
  + semantic index
  + module graph
  + type/value/alias facts
  + call graph
  + data-flow summaries
  + effect summaries
  + extension overlays
  + path evidence
  + evaluation harness
```

CFG is the local execution skeleton. It must be precise enough to support future sparse value-flow and IFDS/IDE solvers, but it should not try to encode call graph, framework dispatch, or scheduler semantics inside the local graph.

## Main Risks

| Risk | Mitigation |
|---|---|
| Overclaiming exactness for exception/finally behavior | Use graph views, precision labels, and fixtures per construct. |
| Public API freezes bad graph IDs | Keep CFG internal first; expose typed views only after validation. |
| Extension edges corrupt native semantics | Additive layers, validation, conflict records, precision ceilings. |
| Dominator/control-dependence wrong with multiple exits | Always use artificial exits and explicit view policies. |
| Language providers diverge | Shared fact schema, invariant validator, and benchmark matrix. |
| CFG becomes a dumping ground for framework/call behavior | Keep framework dispatch, call graph, and lifecycle overlays separate but composable. |

## Research Confidence

Confidence is high for the architectural direction because independent mature systems converged on operation nodes, basic blocks, explicit exceptional edges, and derived dominance. Confidence is medium for exact first-slice language coverage because TS/JS and Go adapters must be implemented natively and validated against fixtures. Confidence is low for claiming exact async/interprocedural/scheduler behavior in early phases; those should remain separate future layers.
