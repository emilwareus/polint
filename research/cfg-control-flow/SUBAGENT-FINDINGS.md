# Subagent Findings

Six parallel research agents investigated CFG/control-dependence from independent angles. This file records the consolidated results.

## Go

Conclusion:

- `go/ssa` is the best Go CFG substrate.
- `go/cfg` and `ctrlflow` are useful syntactic/lifecycle references.
- compiler SSA is a design reference only.

Key findings:

- `go/ssa` exposes `Function`, `BasicBlock`, `Preds`, `Succs`, dominator helpers, and instructions such as `Panic`, `RunDefers`, `Defer`, `Go`, `Select`, and `Recover`.
- `go/cfg` does not model enough abnormal flow for high-capability analysis.
- Polint should compute postdominators and control dependence itself.
- `go` statements are spawn facts, not intraprocedural CFG edges.
- `panic/recover` and `defer` require explicit precision modes.

## TypeScript / JavaScript

Conclusion:

- Oxc is the best Rust-native starting point.
- TypeScript flow nodes are for narrowing, not a general CFG.
- ESLint code paths are useful for rule ergonomics.
- CodeQL JS is the best semantic coverage reference.

Key findings:

- Oxc already has `ControlFlowGraph`, `BasicBlock`, typed edges, error harnesses, and finalizers.
- TypeScript’s binder/checker builds and walks `FlowNode`s lazily for reference-specific type information.
- ESLint tracks code-path segments through forks, joins, loops, returns, throws, and `finally`.
- CodeQL JS documents imprecision around `finally` and conservatively models implicit exception edges.

## Python

Conclusion:

- Build a CodeQL-style source CFG.
- Keep Pyright/mypy type narrowing separate.
- Use Pyre for fixpoint/data-flow architecture.
- Use CPython bytecode as semantic reference only.

Key findings:

- CodeQL Python maps AST nodes to zero/one/many `ControlFlowNode`s and exposes `BasicBlock`s.
- Pyright models finally/context-manager/narrowing with flow nodes such as `PreFinallyGate`, `PostFinally`, and `PostContextManager`.
- Pyre has explicit CFG nodes and weak-topological fixpoint analysis.
- CPython bytecode changes across releases and should not be public source-level CFG.

## Java / JVM

Conclusion:

- Java needs source CFG and bytecode CFG as separate capabilities.
- Checker Framework is the best source-level reference.
- Soot/SootUp/WALA/OPAL are the best bytecode/JVM references.

Key findings:

- Java abrupt completion, `finally`, try-with-resources, synchronized, lambdas, and `invokedynamic` require dedicated modeling.
- Soot `ExceptionalUnitGraph` is a canonical exceptional CFG implementation.
- WALA has SSA CFG, exploded CFG, exception-pruned views, and control dependence.
- SootUp has explicit normal/exceptional successor APIs but a postdominator caveat around multiple tails.
- Polint must use artificial exits for postdominance.

## Language-Neutral IR

Conclusion:

- Copy LLVM/MLIR block/terminator/dominator discipline.
- Copy CodeQL typed query surfaces.
- Copy Joern/CPG layering internally, not as public API.
- Copy Semgrep’s honesty about path sensitivity.

Key findings:

- Basic blocks plus explicit terminators are a proven core.
- Exceptional flow must be a first-class edge.
- Path evidence needs feasibility labels.
- Public SDK should remain typed and narrow.

## Benchmarks And Validation

Conclusion:

- There is no universal CFG ground-truth benchmark.
- Use micro fixtures, semantic rule oracles, differential checks, and external corpora.

Key findings:

- CodeQL tests are the closest query/fact oracle.
- ESLint, TypeScript, Pyright, Go SSA, Soot/WALA, and Checker Framework can be differential references.
- OWASP, Juliet, DroidBench, SecBench.js, Test262, and Pyre/Pysa tests are better for end-to-end flow/diagnostic validation than exact CFG shape.
- PR gates should enforce graph invariants and fixture snapshots.
