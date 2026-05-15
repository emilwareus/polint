# Validation Report

Date: 2026-05-15

This file records the validation pass over the CFG/control-dependence research.

## What Was Validated

| Area | Validation performed | Result |
|---|---|---|
| Repository snapshots | Cloned and sparse-checked out key OSS implementations under `research/cfg-control-flow/repos/`. | Passed. Directory is covered by existing `.gitignore` rule `research/*/repos/`. |
| Go claims | Searched cloned `golang-tools` for `Function`, `BasicBlock`, `Panic`, `RunDefers`, `Defer`, `builder.cond`, `builder.selectStmt`, and `BasicBlock.Dominates`. | Passed. Claims are grounded in source paths listed in `REPO-INDEX.md`. |
| TS claims | Searched TypeScript compiler for `FlowFlags`, `FlowNode`, `FlowSwitchClause`, `FlowReduceLabel`, binder flow creation, and checker flow-node evaluation. | Passed. |
| Oxc claims | Searched `oxc_cfg`/`oxc_semantic` for `EdgeType`, `ControlFlowGraph`, `BasicBlock`, error harnesses, finalizers, and throw/break/continue handling. | Passed. |
| ESLint claims | Searched code-path analysis for code paths, segments, forks, joins, try/finally contexts, breaks, continues, returns, and throws. | Passed. |
| CodeQL claims | Searched CodeQL JS/Python/Java libraries for `ControlFlowNode`, `BasicBlock`, successors, dominance, postdominance, exceptional successors, and finally imprecision notes. | Passed. |
| Python claims | Searched Pyright, Pyre, mypy, CPython for flow nodes, finally/context-manager constructs, explicit CFG, fixpoint, binder, and bytecode flowgraph/codegen. | Passed. |
| Java/JVM claims | Searched Soot, SootUp, WALA, Checker Framework for exceptional CFGs, normal/exceptional successors, dominance/postdominance, control dependence, try/finally/resources/synchronized handling. | Passed. |
| Language-neutral claims | Cloned LLVM/MLIR/Joern/Semgrep source subsets and checked primary docs. | Passed at architecture level. |
| Paper artifacts | Downloaded foundational/control-flow papers and primary docs snapshots under `papers/`. | Passed with caveat: some PDFs are public mirrors; DOI/official pages are preferred in prose. |
| Subagent review | Integrated six parallel research agents: Go, TS/JS, Python, Java/JVM, language-neutral IR, and benchmarks. | Passed. Findings converged on layered source CFG + block graph + explicit exceptional edges + derived dominance/control dependence. |

## Important Corrections Made During Validation

- Java CodeQL sparse checkout initially targeted the wrong path. Correct path is `java/ql/lib/semmle/code/java/ControlFlowGraph.qll` and `java/ql/lib/semmle/code/java/controlflow`.
- SootUp postdominance was not treated as a solved authority; source inspection found a caveat about multiple tail blocks. The recommendation now requires artificial unified exits.
- TypeScript flow nodes are described as narrowing/reachability infrastructure, not a reusable CFG API.
- `go/cfg` is described as syntactic/lightweight, not the Go state-of-the-art substrate.
- CPython bytecode is described as semantic reference only because Python bytecode is CPython-specific and unstable across releases.
- Promise scheduling, goroutine interleavings, Java bytecode/source equivalence, and Python dynamic execution are explicitly excluded from first-slice exact claims.

## Accuracy Confidence

| Claim | Confidence | Reason |
|---|---|---|
| Operation nodes + basic blocks + typed edges are the right common substrate. | High | Repeated across Go SSA, Oxc, CodeQL, Checker, Soot/WALA/OPAL, LLVM/MLIR. |
| Dominance/postdominance should be derived facts. | High | Standard in compiler and query systems; easier to cache and validate separately. |
| Control dependence should be computed from postdominance. | High | Classic Ferrante/Ottenstein/Warren model; WALA/OPAL validate industrial use. |
| First implementation can use simple dominator algorithms. | Medium | Likely fine for function-sized graphs; must be benchmarked. |
| Oxc can back TS/JS first slice. | Medium-High | Source has CFG support, but coverage must be validated. |
| Go tree-sitter provider can match Go SSA precision immediately. | Low | Semantic package loading/SSA-grade lowering is needed for exact claims. |
| Exact async/scheduler behavior belongs in CFG. | Low | Should be modeled as lifecycle/effects layer, not local CFG. |

## Remaining Open Questions

- Whether to implement TS/JS CFG by translating Oxc CFG output or by driving the shared builder from Oxc AST visitors.
- How soon to add reason-carrying `finally` continuations versus conservative finalizer nodes.
- Whether first public `Cfg<'_>` should expose operation nodes, blocks, or both.
- What the first extension sink should allow: no-return summaries only, or synthetic edges too.
- How much semantic Go package loading should be required before claiming Go CFG capability.

## Validation Recommendation

Before implementing public SDK support, create fixture snapshots for Go and TS/JS and run differential checks against:

- Go SSA / `go/cfg`;
- Oxc CFG;
- ESLint code-path tests;
- CodeQL JS/Go control-flow tests.

Only then promote `Cfg<'_>` from internal facts to public rule-author surface.
