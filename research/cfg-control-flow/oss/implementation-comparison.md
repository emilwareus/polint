# OSS Implementation Comparison

| Tool | Level | CFG shape | Exceptional flow | Dominance/CD | Best lesson for polint |
|---|---|---|---|---|---|
| Go `go/cfg` | Source AST | Blocks over AST statements | Limited; panic/recover not analysis-grade | No core CD | Good lightweight syntactic baseline only. |
| Go `go/ssa` | Typed SSA/source | Functions, basic blocks, instructions | `Panic`, `Defer`, `RunDefers`, recover block concepts | Dominators available; CD must be derived | Best Go semantic reference. |
| Go compiler SSA | Compiler IR | Blocks, values, controls, edges | Compiler/runtime-oriented | Dominator algorithms | Data-structure and algorithm reference, not dependency. |
| Oxc | JS/TS AST/semantic | `ControlFlowGraph`, `BasicBlock`, typed edges | Error harnesses/finalizers | Reachability; derive CD in polint | Best Rust-native JS/TS substrate. |
| TypeScript compiler | TS checker flow | Backward `FlowNode` graph | Enough for narrowing/reachability | Not general CD | Keep narrowing separate from CFG. |
| ESLint | JS rule code paths | CodePath/CodePathSegment | Models try/finally and thrown paths for linting | Not CD substrate | Rule ergonomics reference. |
| CodeQL JS | Source query DB | `ControlFlowNode`, synthetic nodes, basic blocks | Explicit/conservative, documents finally imprecision | Dominance/guards | Best query API and honesty reference. |
| CodeQL Python | Source query DB | Many flow nodes per AST, basic blocks | Explicit exceptional successors | Dominance; limited postdom in adapter | Best Python source CFG design. |
| CodeQL Java | Source query DB | Source control-flow predicates | Exception modeling predicates | Dominance/postdominance predicates | Good Java query-level reference. |
| Pyright | Python type checker | Inverse flow-node graph | Finally/context-manager nodes | Reachability for type checking | Best Python narrowing reference. |
| Pyre/Pysa | Python analyzer | Explicit CFG nodes and fixpoint | Try/with/dispatch nodes | Fixpoint over CFG | Good data-flow architecture reference. |
| mypy | Python type checker | Binder frames, not CFG | Checker-state handling | No public CD | Narrowing/reachability reference only. |
| CPython | Bytecode compiler | Basic blocks and bytecode instructions | Exception tables, with/finally/yield/await lowering | Compiler optimizations | Semantic oracle, not source API. |
| Checker Framework | Java source | Source CFG nodes/blocks | Regular and exceptional exit, try/finally/TWR/sync | Dataflow substrate | Best Java source CFG reference. |
| Soot | JVM/Jimple | UnitGraph/BlockGraph | Mature `ExceptionalUnitGraph` | Postdominators, PDG | Canonical JVM exceptional CFG reference. |
| SootUp | JVM/Jimple | Modern CFG APIs | Normal and exceptional successors | Dominance/postdominance with caveats | Modern API shape; validate multi-exit postdom. |
| WALA | JVM SSA | SSACFG, ShrikeCFG, exploded CFG | Exceptional successors, exception-pruned views | ControlDependenceGraph | Multiple graph views and CD reference. |
| OPAL | JVM bytecode/TAC | CFG/TAC/SSA | AI-informed exception flow | Postdominance/CD | Bytecode precision and artificial exits. |
| LLVM | Compiler IR | Basic blocks + terminators | `invoke`/EH pads | Dominator/postdom trees | Block discipline and EH edge clarity. |
| MLIR | Compiler IR | SSACFG regions, block args | Dialect-specific | Dominance | Preserve structured and lowered views. |
| Joern/CPG | Multi-language graph DB | AST+CFG+CDG+PDG layers | Language-specific | CDG/PDG layers | Layering, not public API shape. |
| Semgrep | Source linter/SAST | Internal IL/flow | Conservative/limited | Not CD authority | Product honesty on path sensitivity. |
| TAJS | JS abstract interpreter | FlowGraph, BasicBlock, AbstractNode | Exception handlers and duplicates | Solver-oriented | JS abstract interpretation architecture. |
| Jelly | JS/TS research analyzer | Constraint/control-flow model | Pragmatic async/module handling | Call graph focus | Future call/points-to lessons. |

## Ranking By Usefulness For First Implementation

1. Oxc for TS/JS provider shape.
2. Go SSA for Go provider semantics.
3. CodeQL for typed query views and precision honesty.
4. ESLint for rule-author ergonomics.
5. LLVM/MLIR for block/terminator/dominator discipline.
6. Checker/Soot/WALA/OPAL for future Java.
7. Pyright/Pyre/CodeQL Python/CPython for future Python.

## What Not To Copy

- CodeQL database architecture as runtime dependency.
- Joern public graph database as SDK.
- TypeScript/Pyright flow nodes as a general CFG.
- `go/cfg` as exact Go CFG.
- bytecode CFG as source-level Java/Python diagnostics.
- Oxc internal IDs as public polint IDs.
- Soot/WALA JVM stacks as direct dependencies for a native Rust engine.
