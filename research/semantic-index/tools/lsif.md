# LSIF

## What It Is

LSIF, the Language Server Index Format, is a graph format that stores precomputed LSP request results such as definitions, references, hovers, and document symbols. It is historically important but should not guide polint's internal semantic-index architecture.

Primary inspected files:

- `_specifications/lsif/0.6.0/specification.md`
- `_specifications/lsif/0.5.0/specification.md`

## Index Shape

Core objects:

- graph vertices and edges;
- document vertices;
- range vertices;
- resultSet vertices;
- hover/definition/reference/declaration result vertices;
- `contains`, `next`, `textDocument/definition`, `textDocument/references`, and `item` edges;
- monikers for cross-project identity.

LSIF does not define a symbol database. It dumps enough graph structure to answer LSP-like navigation queries.

## Algorithm

```python
def lsif_lookup_definition(graph, document, position):
    ranges = graph.contains(document)
    innermost = find_innermost_range(ranges, position)
    result_set = follow_next_edge(innermost)
    definition_result = graph.out_edge(result_set, "textDocument/definition")
    return graph.item_targets(definition_result)
```

## Accuracy

LSIF accuracy depends entirely on the language server/indexer that emitted the graph. It stores answers; it does not compute semantic truth.

The important weakness is structural:

- graph IDs are brittle as an internal storage model;
- LSP-result shape is less natural than symbol/occurrence facts;
- semantic identity is indirect through monikers and result graph topology.

SCIP's design explicitly addresses several practical LSIF pain points.

## Complexity

Export and lookup costs depend on graph size:

```text
O(vertices + edges)
```

Lookups traverse range/resultSet/result edges. Storage and memory overhead can be high due to graph adjacency and many small vertices/edges.

## Strengths

- Directly represents LSP navigation answers.
- Useful historical reference.
- Monikers are relevant to cross-project identity.

## Weaknesses

- Not a semantic model.
- Awkward for internal facts.
- Graph shape makes debugging and compatibility harder than document/occurrence schemas.
- Superseded in practice by SCIP in several modern code-intelligence contexts.

## Polint Implications

Copy:

- the idea that precomputed navigation answers can be exported;
- moniker-like cross-project identity concepts.

Avoid:

- internal LSIF graph storage;
- exposing LSP-result graph semantics as the SDK;
- confusing "precomputed jump-to-definition results" with semantic facts.

Recommended role:

```text
Historical reference only.
Prefer SCIP/Kythe concepts for export and internal stable identity.
```
