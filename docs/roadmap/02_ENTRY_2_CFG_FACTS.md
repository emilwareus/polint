# Entry 2: CFG Facts

## Goal

Fulfill the CFG typed view with real intra-procedural control-flow graph
facts for Go and TS/JS first.

## Why

CFG facts unlock branch-shape rules, flow-sensitive checks, and future dataflow.
The current graph placeholder is not enough for rule authors.

## Difficulty

**L for TS/JS**, **XL for Go**, **L later for Python**, **L/XL later for Java**.

## What To Build

- `ControlFlowGraph`
- `BasicBlock`
- `CfgNodeId`
- `CfgEdge`
- `CfgEdgeKind`
- function-to-CFG query on the typed view

## Build Method

1. Add shared CFG graph types in the public SDK surface.
2. Store CFGs in `AnalysisDb` keyed by `FunctionId`.
3. Add a function-to-CFG query returning an optional `ControlFlowGraph`.
4. For TS/JS, adapt Oxc semantic CFG output into polint's graph model.
5. For Go, expand existing branch extraction into entry, exit, sequential,
   branch, loop, switch, defer, panic, and return edges.
6. Cache CFG facts only when requested by `AnalysisPlan`.
7. Document syntax-level precision and unsupported constructs.

## Done When

- Go and TS/JS functions can expose a real CFG.
- A generated external rule can consume the CFG typed view and query a function CFG.
- Docs explain precision limits.

## Later Languages

- Python can build CFGs from `ast` statements.
- Java can build CFGs from `JavacTask.parse()` tree scanners or JavaParser.
