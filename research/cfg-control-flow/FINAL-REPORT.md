# Final Report: CFG And Control Dependence

## Executive Decision

Build a native, language-owned CFG fact layer before implementing serious type/value/alias analysis, function effects, sparse data flow, or high-confidence path evidence.

```text
semantic index + module graph
  -> function/body inventory
  -> language-owned operation CFG
  -> basic-block CFG
  -> typed normal/abrupt/exceptional edges
  -> reachability + dominators + postdominators
  -> control-dependence facts
  -> path evidence
  -> extension overlays with validation
```

The first mistake to avoid is treating CFG as “just a graph of statements.” The state of the art shows that useful CFGs are not merely AST traversal output. They are a carefully lowered representation of execution order, abrupt completion, exception/cleanup/finally behavior, short-circuit expression evaluation, and language-specific constructs. They are also intentionally incomplete in places, and mature tools label or isolate those precision choices.

The second mistake to avoid is making the CFG a public graph database. polint should expose typed SDK views, not raw Oxc nodes, Go SSA blocks, `petgraph`, Jimple, bytecode, or CodeQL-like database internals.

## Why CFG Is The Next Structural Layer

Call graphs answer “which callable might be invoked.” Data flow answers “which values might reach which use.” Neither can be accurate without local execution order.

CFG facts are needed for:

- unreachable-code and dead-branch rules;
- guarded-call and precondition rules;
- “must happen before” and “must be checked before use” rules;
- path evidence for diagnostics;
- postdominance-based control dependence;
- branch-sensitive data-flow transfer;
- local callsite placement and call-return flow;
- exceptional cleanup, `finally`, `defer`, `with`, and resource handling;
- future slicing and explanation.

CFG is also where polint’s product thesis matters. A generic analyzer will always miss some framework and codebase-specific behavior. polint can expose unknowns and allow repo-local Rust providers to add synthetic dispatch/lifecycle facts, model no-return APIs, or improve exceptional/cleanup summaries. But those extensions must not silently rewrite local language semantics. They need provenance, validation, precision ceilings, and merge rules.

## Core Finding

There is no single state-of-the-art CFG algorithm that works uniformly across Go, TS/JS, Python, and Java. The state of the art is a layered design:

| Layer | Purpose | Mature reference |
|---|---|---|
| Operation CFG | Preserve expression-level control and source anchors. | CodeQL `ControlFlowNode`, Oxc instructions, Checker Framework nodes, CPython codegen. |
| Basic-block CFG | Scalable graph queries and solver substrate. | LLVM `BasicBlock`, Go SSA `BasicBlock`, Soot/WALA/OPAL blocks, CodeQL `BasicBlock`. |
| Abrupt/exceptional edges | Model non-local transfers honestly. | Soot `ExceptionalUnitGraph`, WALA `SSACFG`, Checker CFG exceptional exit, Oxc `Error`/`Finalize`, CodeQL exception successors. |
| Derived dominance | Reachability structure and control guards. | LLVM dominator trees, Go SSA dominators, CodeQL dominance, OPAL postdominators. |
| Control dependence | Guard/dependence relation over postdominators. | Ferrante/Ottenstein/Warren PDG, WALA CDG, OPAL control-dependence utilities. |
| Path evidence | User-facing diagnostic explanation. | CodeQL path queries, LLVM analysis-printer tests, Semgrep path honesty. |

## Language Conclusions

### Go

Use `golang.org/x/tools/go/ssa` as the design reference and likely semantic target, not `go/cfg`.

`go/cfg` builds lightweight syntactic CFGs from `go/ast` block statements. It is useful for fast syntax-level reachability, but its own docs point users toward SSA when they need more precision. It does not model important abnormal flow such as panic/recover in a way that is sufficient for high-capability analysis.

`go/ssa` exposes `Function`, `BasicBlock`, predecessors/successors, dominator helpers, and explicit instructions such as `Panic`, `RunDefers`, `Defer`, `Go`, `Select`, and short-circuit lowering. For polint’s native implementation, the lesson is not “depend on Go SSA forever”; it is “copy the SSA-grade edge semantics and block model when implementing Go CFG facts.”

Important precision decisions:

- `go f()` is a spawn fact, not an intraprocedural CFG successor into `f`.
- `defer` and `RunDefers` must be first-class nodes/edges or the graph will lie around returns and panics.
- `panic/recover` needs explicit precision modes: normal-only, panic-exit, and recover-aware.
- `select` edges are nondeterministic branch edges, not ordered if/else edges.
- build tags, test variants, and module roots are lifecycle inputs and must be cache-keyed.

### TypeScript / JavaScript

Use Oxc as the Rust-native implementation reference, TypeScript/Pyright flow nodes as narrowing inspiration, ESLint as rule ergonomics inspiration, and CodeQL JS as semantic coverage reference.

Oxc already has an `oxc_cfg` crate with `ControlFlowGraph`, `BasicBlock`, typed `EdgeType`, error harnesses, finalizers, break/continue/throw handling, and semantic builder integration. It is young, so polint should own its fact model rather than expose Oxc internals directly.

TypeScript’s compiler builds `FlowNode` graphs in the binder and walks them lazily in the checker for reference-specific narrowing. That is excellent for a future `TypeNarrowing<'_>` fact layer, but it is not a general CFG API.

ESLint’s code-path analyzer is a useful reference for what rule authors expect: current segments, final segments, forks/joins, returned/thrown paths, and loop segments. But its code-path segment graph is not enough for dominance, control dependence, or serious data flow.

CodeQL JS explicitly documents `finally` imprecision and implicit exception edges from calls, `new`, property accesses, and `await` only where enclosing handlers make those edges relevant. That honesty should be copied.

Important precision decisions:

- `&&`, `||`, `??`, logical assignment, optional chaining, and ternary expressions must create expression-level control nodes.
- `try/finally` must model return/throw/break/continue through cleanup; a naive join introduces impossible paths.
- `await` is normal continuation plus possible rejection/throw; scheduler-level interleavings belong to a separate async/effects layer.
- promise chains are not CFG edges by default.
- dynamic `eval` and dynamic import targets should become unknown/unsupported facts, not exact edges.

### Python

Use a CodeQL-style source CFG as the rule-facing target, and Pyright/Pyre/mypy-style flow/narrowing as separate layers.

CodeQL Python is the best query-facing CFG reference: an AST node can map to zero, one, or many `ControlFlowNode`s, and `BasicBlock` is used for scalable reachability/dominance. This is essential for `try/finally`, `with`, boolean expressions, comprehensions, and pattern matching.

Pyright and mypy show how type narrowing should be kept separate from the general CFG. Pyright’s inverse flow graph with `PreFinallyGate`, `PostFinally`, `PostContextManager`, `NarrowForPattern`, and reachability caches is highly relevant, but it is reference-query driven. Pyre provides a clearer explicit CFG and fixpoint architecture, including dispatch/try/with nodes and synthetic assumptions.

CPython bytecode is an authority on runtime behavior but not an appropriate source-level rule API. The Python `dis` docs warn bytecode is CPython-specific and may change across releases. Use CPython `flowgraph.c`, `codegen.c`, and bytecode metadata as semantic reference only.

Important precision decisions:

- `with` must model `__enter__`, body, `__exit__`, and exception suppression.
- `finally` and exception groups need explicit synthetic nodes or cleanup edges.
- `yield`, `yield from`, `await`, async generators, `async with`, and `async for` are suspend/resume boundaries, not ordinary sequential statements.
- comprehensions need nested scope metadata and either subgraphs or expression-local CFGs.
- pattern matching has control flow and narrowing effects; protocol details should be marked heuristic unless modeled.

### Java / JVM

Support Java as two precision tiers when the adapter exists:

```text
java/source-cfg
java/bytecode-cfg
```

Java source and JVM bytecode diverge materially around `finally`, try-with-resources, suppressed exceptions, synchronized regions, lambdas, method references, class initialization, and compiler-generated paths.

Checker Framework is the best source CFG reference. It builds CFGs over `com.sun.source.tree` and explicitly models normal exit, exceptional exit, `try`, resources, `finally`, synchronized blocks, lambdas, and method-invocation exceptions.

Soot, SootUp, WALA, and OPAL are the best bytecode/JVM references. Soot’s `ExceptionalUnitGraph` distinguishes explicit throws, implicit exceptions, and side-effect-sensitive handler edges. WALA exposes SSA CFGs, exceptional successors, pruned CFG views, exploded CFGs, and a control-dependence graph. OPAL is especially useful for bytecode CFGs with artificial exits, postdominators, and control dependence.

Important precision decisions:

- source-level Java CFG should not claim bytecode-exact exceptional behavior.
- bytecode-level CFG should not claim perfect source construct recovery.
- `finally`, try-with-resources, and synchronized require dedicated modeling and fixtures before “exact” control dependence is advertised.
- lambdas and `invokedynamic` need call/lifecycle summaries, not just local CFG edges.

## Algorithm Conclusions

### CFG Construction

The right construction algorithm is an AST/IR lowering builder with an explicit control context stack:

- append operation nodes in source/evaluation order;
- split blocks at branch, join, loop, exception, cleanup, suspend, and abrupt-transfer boundaries;
- maintain break/continue/return/throw/defer/finally context stacks;
- add virtual entry and exit nodes;
- emit edge kinds and precision labels as facts;
- run invariants after construction.

This is linear in the lowered operation count plus emitted edges for normal constructs, with additional edges for exception/cleanup modeling.

### Dominance And Postdominance

Dominators and postdominators should be derived facts, not stored by CFG builders.

For a first native implementation, use a deterministic iterative or Cooper-Harvey-Kennedy-style algorithm over bitsets or sets. It is simple and likely fast enough for repo-local function CFGs. If benchmarks show it matters, implement Lengauer-Tarjan or Semi-NCA.

For postdominance:

- create a synthetic unified exit;
- add normal-return, exceptional-exit, panic/throw exit, and infinite-loop policies explicitly;
- run dominator computation on the reversed graph;
- include precision metadata in the result.

### Control Dependence

Use Ferrante/Ottenstein/Warren control dependence over postdominance:

```text
edge A -> B induces control dependence for nodes that postdominate B
but do not strictly postdominate A
```

Practical algorithm:

```text
for edge A -> B:
  if not postdominates(B, A):
    runner = B
    stop = ipostdom(A)
    while runner != stop:
      emit runner controlled_by A via edge A->B
      runner = ipostdom(runner)
```

This is `O(E * h)` with a simple tree walk, where `h` is postdominator-tree height. It can be optimized later with dominance-frontier-style methods or Bilardi/Pingali APT if control-dependence queries become a memory/time bottleneck.

## Product-Specific Shift: Agent-Extensible CFG

Classic static analyzers try to auto-model every framework and codebase convention. polint does not need to win that game by default. It needs to expose a truthful CFG substrate that agents can extend.

Allowed extension examples:

- add a no-return summary for a repo-specific fatal API;
- add framework dispatch/lifecycle overlays outside the local CFG;
- add generated-code source spans or synthetic operation nodes;
- mark a project-specific assertion function as a guard for path evidence;
- add exception/suppression summaries for known context managers or resource APIs;
- improve unresolved dynamic behavior with validated repo-local facts.

Unsafe extension examples:

- silently delete native edges;
- relabel heuristic edges as exact without validation;
- merge interprocedural call edges into intraprocedural CFG;
- suppress unknowns that would affect false negatives;
- expose parser internals as stable public IDs.

The extension model should be additive by default. Replacements and suppressions should require conflict diagnostics, preserved suppressed facts, validation fixtures, and precision ceilings.

## Final Recommendation

Implement CFG as the next research-backed engine layer, but start with a narrow vertical slice:

1. Internal CFG fact schema and invariant validator.
2. Go and TS/JS operation/block CFGs.
3. Normal, branch, loop, return, break, continue, throw/panic, finally/defer edge kinds.
4. Reachability and dominators/postdominators.
5. Control dependence behind an internal view.
6. Snapshot fixtures and differential comparisons against Go SSA/Oxc/ESLint/CodeQL.
7. Public `Cfg<'_>` only after the fact model is stable enough to document honestly.

This gives polint the structural substrate needed for the next deep research track: type, value, points-to, and alias analysis.
