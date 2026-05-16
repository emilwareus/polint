# Language-Neutral IR And Query-System Lessons

## Executive Take

LLVM, MLIR, CodeQL, Joern/CPG, Semgrep, and compiler IRs all point to the same core design:

```text
basic blocks + typed terminators/edges + dominance
```

But polint should not become LLVM, MLIR, CodeQL, or Joern. It should copy the representation lessons while keeping a native Rust, source-facing, typed fact API.

## LLVM Lessons

LLVM functions are CFGs of basic blocks ending in terminators. Dominator and postdominator trees are standard analysis products. Exception handling is explicit: `invoke` has a normal destination and unwind destination, and landing pads/catch/cleanup pads shape exceptional flow.

Polint takeaways:

- every block should end in an explicit control-transfer concept;
- exceptional flow cannot be hidden;
- dominators/postdominators are derived analyses;
- compiler IR is too low-level for direct source diagnostics.

## MLIR Lessons

MLIR supports SSACFG regions with blocks, block arguments, terminators, and successors. It also distinguishes structured control-flow dialects (`scf`) from branch-based control-flow dialects (`cf`).

Polint takeaways:

- keep raw CFG edges plus high-level source/construct metadata;
- later value-flow can use edge-carried/block-argument ideas without exposing them first;
- structured constructs should not be erased when diagnostics need them.

## CodeQL Lessons

CodeQL exposes query-friendly typed classes:

- `ControlFlowNode`
- `BasicBlock`
- successors/predecessors;
- dominance/reachability;
- guard/control-flow helpers;
- path query infrastructure.

Polint takeaways:

- public API should be typed and query-oriented;
- one AST node can map to many flow nodes;
- basic blocks are performance facts, not the only source anchor;
- path evidence should be a first-class product feature.

## Joern / CPG Lessons

Joern’s Code Property Graph layers AST, CFG, data-flow, call graph, dominators, CDG, and PDG into a single queryable property graph.

Polint takeaways:

- layering is correct;
- exposing everything as one graph database is not the right public SDK for polint;
- internal cross-fact indexes can borrow CPG ideas;
- typed SDK views should remain narrow and stable.

## Semgrep Lessons

Semgrep is useful less as a CFG authority and more as a product-honesty reference. Its docs explicitly discuss limitations around path sensitivity and soundness. That matters because CFG facts will feed diagnostics users trust.

Polint takeaways:

- be explicit about path sensitivity;
- label heuristics and unsupported behavior;
- path evidence should not imply solver-checked feasibility unless a solver checked it.

## Recommended Polint Internal IR

Use a source-level operation/block model:

```text
Operation node:
  source anchor
  operation kind
  owning function/body
  precision
  provenance

Basic block:
  ordered operation range
  predecessor edges
  successor edges
  reachability

Edge:
  source node/block
  target node/block
  kind
  label/condition
  precision
  provenance
```

Keep graph storage private. A dense arena plus sorted adjacency lists is likely better than exposing `petgraph`. `petgraph` can remain an internal helper if it proves useful, but public facts should be stable typed IDs.

## Derived Facts

Derived facts should be computed over selected graph views:

- reachability;
- SCCs and loops;
- dominators;
- postdominators;
- dominance frontier;
- control dependence;
- later SSA/value-flow.

Each derived fact must remember which graph view and precision mode produced it.

## Path Evidence

Every diagnostic path should carry:

```text
path id
function/body id
ordered node ids
ordered edge ids
source spans
edge kinds
guard labels
feasibility status
precision notes
missing model notes
```

Feasibility statuses:

- `unchecked`
- `syntactic`
- `solver_checked`
- `infeasible_pruned`
- `unknown_due_to_unsupported`

Do not call a path “possible” if it is merely graph-reachable through known-infeasible `finally` merging.

## Final Tooling Decision

Copy LLVM/MLIR block discipline, CodeQL typed query surfaces, Joern layering, and Semgrep honesty. Do not adopt any of them as the public representation or runtime dependency.
