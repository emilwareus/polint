# Repository Index

Third-party repositories were cloned under `research/cfg-control-flow/repos/`. That directory is gitignored.

## Cloned Repositories

| Repository | Local path | Why inspected | Key files/directories |
|---|---|---|---|
| `golang/tools` | `repos/golang-tools` | Public Go CFG and SSA packages. | `go/cfg`, `go/ssa`, `go/analysis/passes/ctrlflow`, `go/analysis/passes/buildssa` |
| `golang/go` | `repos/golang-go` | Compiler SSA/dominator/block-layout reference. | `src/cmd/compile/internal/ssa`, `src/cmd/compile/internal/ssagen` |
| `microsoft/TypeScript` | `repos/typescript` | TypeScript flow nodes, narrowing, reachability, compiler tests. | `src/compiler/binder.ts`, `src/compiler/checker.ts`, `src/compiler/types.ts`, `tests/cases/compiler` |
| `oxc-project/oxc` | `repos/oxc` | Rust-native JS/TS CFG and semantic builder. | `crates/oxc_cfg`, `crates/oxc_semantic`, `crates/oxc_ast`, `crates/oxc_linter` |
| `eslint/eslint` | `repos/eslint` | Code-path analysis and rule ergonomics. | `lib/linter/code-path-analysis`, `tests/lib/linter/code-path-analysis` |
| `github/codeql` | `repos/codeql` | Query-facing CFG APIs across JS, Python, Java, Go, C/C++. | `javascript/ql/lib/semmle/javascript/CFG.qll`, `python/ql/lib/semmle/python/Flow.qll`, `java/ql/lib/semmle/code/java/ControlFlowGraph.qll`, `java/ql/lib/semmle/code/java/controlflow` |
| `microsoft/pyright` | `repos/pyright` | Python flow-node/narrowing engine. | `packages/pyright-internal/src/analyzer/codeFlowTypes.ts`, `binder.ts`, `codeFlowEngine.ts`, tests samples |
| `facebook/pyre-check` | `repos/pyre-check` | Explicit Python CFG and fixpoint/data-flow architecture. | `source/analysis/cfg.ml`, `cfg.mli`, `fixpoint.ml`, `source/interprocedural*` |
| `python/cpython` | `repos/cpython` | Python bytecode CFG/codegen semantic reference. | `Python/flowgraph.c`, `Python/codegen.c`, `Python/bytecodes.c` |
| `python/mypy` | `repos/mypy` | Python binder/type narrowing and reachability reference. | `mypy/binder.py`, `mypy/checker.py`, `mypy/reachability.py` |
| `soot-oss/soot` | `repos/soot` | Mature JVM/Jimple exceptional CFG and PDG. | `src/main/java/soot/toolkits/graph`, `src/main/java/soot/toolkits/graph/pdg` |
| `soot-oss/SootUp` | `repos/sootup` | Modern Soot control-flow graph APIs. | `sootup.core/src/main/java/sootup/core/graph` |
| `wala/WALA` | `repos/wala` | JVM SSA CFG, exploded CFG, exception-pruned CFG, CDG. | `core/src/main/java/com/ibm/wala/cfg`, `core/src/main/java/com/ibm/wala/ssa`, `core/src/main/java/com/ibm/wala/ipa/cfg` |
| `typetools/checker-framework` | `repos/checker-framework` | Source-level Java CFG and dataflow. | `dataflow/src/main/java/org/checkerframework/dataflow/cfg` |
| `opalj/opal` | `repos/opal` | JVM bytecode/TAC CFG, dominance, control dependence. | `OPAL/br/src/main/scala`, `OPAL/ai/src/main/scala`, `OPAL/tac/src/main/scala` |
| `llvm/llvm-project` | `repos/llvm-project` | Basic blocks, terminators, dominators, MLIR regions/blocks. | `llvm/include/llvm/IR`, `llvm/include/llvm/Analysis`, `llvm/include/llvm/Support/GenericDomTreeConstruction.h`, `mlir/include/mlir/IR`, `mlir/docs` |
| `joernio/joern` | `repos/joern` | CPG, CFG/CDG/PDG layering, data-flow engine. | `semanticcpg`, `dataflowengineoss`, `x2cpg`, `docs` |
| `semgrep/semgrep` | `repos/semgrep` | Data-flow/path-sensitivity honesty and IL/CFG lessons. | `src/analyzing`, `src/il`, `src/tainting`, `cli/src/semgrep` |
| `cs-au-dk/TAJS` | `repos/tajs` | JavaScript flow graph and abstract interpretation reference. | `src/dk/brics/tajs/flowgraph`, `src/dk/brics/tajs/js2flowgraph`, `src/dk/brics/tajs/analysis` |
| `cs-au-dk/jelly` | `repos/jelly` | Modern JS/TS call graph/points-to constraints and pragmatic async/module modeling. | `src`, `docs`, `test` |

## Source Path Validation Notes

Direct source inspection with `rg` confirmed:

- Go SSA defines `Function`, `BasicBlock`, `Panic`, `RunDefers`, `Defer`, and builder methods such as `cond` and `selectStmt`.
- TypeScript defines `FlowFlags`, `FlowNode`, `FlowSwitchClause`, `FlowReduceLabel`, binder creation helpers, and checker flow-node evaluation.
- Oxc defines `EdgeType`, `ControlFlowGraph`, `BasicBlock`, error harnesses, finalizers, and throw/break/continue append methods.
- ESLint code-path analysis models forks, joins, returned/thrown paths, try/finally contexts, loops, breaks, continues, returns, and throws.
- CodeQL exposes JS `ControlFlowNode`, Python `ControlFlowNode`/`BasicBlock`, and Java control-flow/dominance predicates.
- Pyright defines `FlowFlags`, `PreFinallyGate`, `PostFinally`, `PostContextManager`, `NarrowForPattern`, and reachability/type-flow evaluation.
- Soot, SootUp, WALA, Checker Framework, OPAL, and CPython all expose explicit normal/exceptional/cleanup CFG code paths relevant to this research.

## Repositories Considered But Not Central

| Repository/tool | Decision |
|---|---|
| Babel | Useful parser/traversal reference, but no canonical production CFG. Not cloned for this pass because Oxc/ESLint/TypeScript/CodeQL/TAJS cover the JS CFG spectrum better. |
| JavaParser | Useful Java parser/symbol solver, but not a CFG/control-dependence authority. Not cloned for this pass. |
| Jedi/astroid/pylint | Useful Python inference/linter ergonomics, but not state-of-the-art CFG. Not cloned for this pass. |
| Test262/OWASP/Juliet/DroidBench/SecBench.js | Important evaluation corpora; kept in benchmark recommendations rather than cloned here. |
