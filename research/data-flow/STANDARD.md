# Standard Research Vocabulary

Use this vocabulary when comparing data-flow implementations.

## Core Objects

```python
DataFlowNode(
    id,
    language,
    file,
    span,
    enclosing_callable,
    symbol_id,
    reference_id,
    cfg_node_id,
    kind,              # parameter, local, field, property, literal, call_arg, call_return, return_value, unknown
    place,             # optional Place abstraction
    value,             # optional Value abstraction
    precision,
    status,
    provenance,
    model_id,
    validation,
)

Place(
    base,              # local, param, global, receiver, temp, return, unknown
    access_path,       # .field, ["key"], [*], deref, address, element, wildcard
    max_depth,
)

DataFlowEdge(
    id,
    from_node,
    to_node,
    kind,              # assignment, read, write, field_read, field_write, arg_bind, return, call_return, capture, phi, havoc
    guard,
    call_edge_id,
    precision,
    status,
    provenance,
    model_id,
    validation,
)

DataFlowPath(
    source,
    sink,
    nodes,
    edges,
    status,
    precision,
    provenance,
    model_ids,
    explanation,
)
```

## Precision Labels

- `exact_local`: value-preserving local flow inside one function.
- `exact_semantic`: precise for the modeled language semantics.
- `summary`: crosses a function summary.
- `module_linked`: crosses import/export/package/module facts.
- `conservative`: intentionally over-approximated to avoid missing flows.
- `heuristic`: useful rule-specific approximation with known gaps.
- `repo_model`: produced by a repo-local model that bound to native facts.
- `unknown`: insufficient setup or unsupported semantics.

## Status Labels

- `proven`: the engine found a modeled path.
- `partial`: the engine found a path but crossed incomplete setup or unsupported features.
- `ambiguous`: multiple possible targets, definitions, or aliases exist.
- `unresolved`: the engine knows a data-flow operation exists but cannot resolve it.
- `setup_missing`: lifecycle input such as package roots, classpath, dependency metadata, or module resolution is missing.
- `unsupported`: feature known but not modeled.
- `model_unvalidated`: produced by a repo-local model that has not passed validation fixtures.

## Algorithm Families

- `iterative_cfg`: classic monotone data-flow equations over a CFG.
- `ssa_sparse`: sparse value-flow over def-use/value IDs.
- `ifds`: interprocedural finite distributive subset analysis.
- `ide`: IFDS generalized with edge functions over values.
- `summary`: function summaries propagated to a fixed point.
- `abstract_interpretation`: lattice domains, joins, transfer functions, widening.
- `points_to`: allocation/object abstraction used to resolve aliases and dispatch.
- `datalog`: relation-based fixed point with semi-naive evaluation.
- `cfl_reachability`: balanced call/return or field-sensitive graph reachability.
- `heuristic_model`: explicit manually modeled propagation, barriers, or framework behavior.
- `repo_model`: agent-authored or rule-author-authored repository model for sources, sinks, sanitizers, summaries, entrypoints, or additional steps.

## Standard Implementation Template

Each inspected implementation should be described with:

1. **What it builds**: local flow, global flow, taint paths, summaries, abstract states, points-to sets, or CPG/DDG edges.
2. **Inputs**: source, AST, bytecode, SSA, CFG, call graph, type checker, package loader, classpath, dependency graph.
3. **IR shape**: AST-attached flow nodes, SSA, Jimple, CPG, generic AST/IL, Datalog facts, or typed relation facts.
4. **Algorithm family**: iterative CFG, IFDS/IDE, abstract interpretation, sparse value-flow, Datalog, summary fixed point, points-to, or heuristic.
5. **Interprocedural model**: none, call graph traversal, summaries, tabulation, demand-driven, context-sensitive, or framework entrypoints.
6. **Heap/access-path model**: locals only, bounded fields/properties, field-insensitive heap, field-sensitive heap, object-sensitive heap, wildcard/havoc.
7. **Dynamic feature handling**: reflection, `eval`, dynamic import, decorators, monkeypatching, unknown calls, framework callbacks.
8. **Uncertainty model**: missing edges, explicit unknown facts, conservative propagation, warnings, diagnostics, precision labels.
9. **Extension model**: whether repo-local sources, sinks, sanitizers, barriers, summaries, entrypoints, or additional steps can be supplied externally.
10. **Cost profile**: expected speed, memory, whole-program requirements, incremental viability.
11. **Polint lesson**: what to copy, what to avoid.

## Minimal Provider Interface

```python
class DataFlowProvider:
    language: str
    algorithm: str

    def required_inputs(self) -> list[str]:
        ...

    def available(self, repo_context) -> bool:
        ...

    def emit_local_flow(self, function_facts) -> list[DataFlowEdge]:
        ...

    def emit_summaries(self, local_flow, call_graph) -> list[FunctionSummary]:
        ...

    def resolve_interprocedural(self, summaries, call_graph) -> list[DataFlowEdge]:
        ...

    def query_paths(self, query) -> list[DataFlowPath]:
        ...

    def diagnostics(self) -> list[CapabilityDiagnostic]:
        ...
```

This interface keeps cheap local facts separate from expensive interprocedural path search.

Repo-local model providers should feed the same node, edge, summary, and path facts, with explicit `model_id`, provenance, and validation status.
