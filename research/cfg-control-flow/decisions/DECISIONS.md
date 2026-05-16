# CFG Decision Log

## D1. Build Native CFG Facts

Decision: build native polint-owned CFG facts instead of exposing or depending directly on external implementations.

Rationale:

- matches full native implementation goal;
- keeps public SDK stable;
- allows cross-language fact shape;
- supports provenance, precision, extension merges, and cache keys.

Rejected:

- expose Oxc CFG directly;
- expose Go SSA blocks directly;
- use CodeQL/Joern/Soot/WALA as runtime dependencies.

## D2. Use Operation Nodes And Basic Blocks

Decision: support both operation-level nodes and basic-block facts.

Rationale:

- operation nodes preserve source/evaluation order and diagnostic anchors;
- basic blocks make reachability/dominance/data-flow efficient;
- CodeQL, Oxc, Go SSA, Checker, LLVM, Soot/WALA all validate this split.

## D3. Keep CFG, Call Graph, And Framework Dispatch Separate

Decision: local CFG edges must not be polluted with call graph edges or framework dispatch edges.

Rationale:

- intraprocedural control flow has different semantics than interprocedural call/return;
- framework lifecycle edges need provenance and precision;
- rules need to know whether an edge is local execution, call relation, or synthetic lifecycle dispatch.

## D4. Compute Dominance/Postdominance As Derived Facts

Decision: CFG providers emit nodes/blocks/edges; analysis layer computes reachability, dominators, postdominators, and control dependence.

Rationale:

- one shared implementation;
- graph-view and precision policies are centralized;
- derived facts can be cached independently;
- providers stay simpler.

## D5. Use Artificial Unified Exits For Postdominance

Decision: postdominance must be computed with an explicit synthetic exit per graph view.

Rationale:

- multiple returns, throws, panics, infinite loops, and exceptional exits otherwise produce ambiguous/incorrect postdominators;
- SootUp source caveat around multiple tails validates the risk;
- control dependence depends on correct postdominance.

## D6. Start With Simple Dominator Algorithm

Decision: first implementation may use a deterministic simple/Cooper-style algorithm before optimizing.

Rationale:

- function CFGs are usually small;
- easier to validate;
- complexity can be upgraded after benchmarks;
- correctness matters more than early micro-optimization.

Upgrade path:

- Lengauer-Tarjan or Semi-NCA;
- bitset acceleration;
- incremental recomputation by changed function.

## D7. Make Exceptional/Cleanup Edges First-Class

Decision: `finally`, `defer`, `with`, resource close, monitor exit, throw/panic, implicit exceptions, await rejection, and yield/await suspend markers need explicit edge/node kinds.

Rationale:

- hidden exceptional flow causes false positives and false negatives;
- mature tools explicitly model these constructs;
- path evidence needs to explain cleanup/exceptional behavior.

## D8. Do Not Claim Exact Async Scheduling In CFG

Decision: `await`/`yield`/goroutine/promise constructs produce suspend/spawn/lifecycle facts, not full scheduler interleaving CFG.

Rationale:

- local CFG is not the right layer for concurrent/event-loop scheduling;
- future effects/lifecycle analysis can consume these facts;
- avoids false precision.

## D9. Extension Overlay Is Additive First

Decision: repo-local Rust providers can add CFG-adjacent facts through sinks, but cannot directly mutate native graph storage.

Rationale:

- protects native semantic facts;
- enables agent-authored improvements;
- keeps conflict/validation/cache behavior tractable.

Allowed first:

- no-return summaries;
- guard summaries;
- cleanup summaries;
- synthetic/generated nodes;
- extension-overlay edges.

Not allowed first:

- silent deletion of native edges;
- relabeling heuristic facts as exact;
- replacing provider graph internals.

## D10. Public SDK Later

Decision: keep CFG internal until Go and TS/JS fixtures, docs, cache keys, and capability diagnostics are stable.

Rationale:

- public SDK is a liability;
- early graph shape will evolve;
- typed views should be promoted only with honest docs.
