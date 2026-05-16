# Core Algorithms In Stripped-Down Pseudo-Code

This file shows the algorithms in Python-ish pseudo-code. The real
implementation should be Rust-native, use stable ids, deterministic ordering,
typed edge kinds, provenance, and bounded caches.

## Local Backward Slice

```python
def backward_slice(graph, criterion, edge_filter, budget):
    seen = OrderedSet([criterion])
    edges = OrderedSet()
    work = Stack([criterion])
    omitted = []

    while work:
        if budget.node_limit_exceeded(len(seen)):
            omitted.append(OmittedRegion(reason="node_limit"))
            break

        node = work.pop()
        for edge in graph.in_edges(node).sorted_by_stable_id():
            if not edge_filter(edge):
                continue

            edges.add(edge.id)
            if edge.src not in seen:
                seen.add(edge.src)
                work.push(edge.src)

    return SliceResult(nodes=seen, edges=edges, omitted=omitted)
```

Cost over a fixed graph: `O(V + E)` for the selected reachable region.

## Local Forward Slice

```python
def forward_slice(graph, criterion, edge_filter, budget):
    seen = OrderedSet([criterion])
    edges = OrderedSet()
    work = Stack([criterion])

    while work and not budget.exceeded():
        node = work.pop()
        for edge in graph.out_edges(node).sorted_by_stable_id():
            if edge_filter(edge):
                edges.add(edge.id)
                if edge.dst not in seen:
                    seen.add(edge.dst)
                    work.push(edge.dst)

    return SliceResult(nodes=seen, edges=edges, status=budget.status())
```

## Chop

```python
def chop(graph, source, sink, edge_filter, budget):
    forward = forward_slice(graph, source, edge_filter, budget.half())
    backward = backward_slice(graph, sink, edge_filter, budget.half())

    nodes = forward.nodes.intersection(backward.nodes)
    edges = OrderedSet()

    for edge in forward.edges.intersection(backward.edges):
        if edge.src in nodes and edge.dst in nodes:
            edges.add(edge)

    return SliceResult(nodes=nodes, edges=edges)
```

For context-sensitive interprocedural graphs, `forward_slice` and
`backward_slice` must carry path context. Do not use this naive form across raw
call/return edges.

## Thin Slice Edge Filter

```python
def thin_edge_filter(edge):
    if edge.kind in {"DataValue", "DataTaint", "ParameterIn", "ParameterOut"}:
        return True
    if edge.kind == "Summary" and edge.precision.not_unknown():
        return True
    return False
```

Thin slices should report omitted edge classes:

```python
omitted = {
    "control": count_filtered("Control"),
    "address": count_filtered("DataAddress"),
    "unknown": count_filtered("Unknown"),
}
```

This keeps the user-facing view small without pretending it is complete.

## Ranked Shortest Path

```python
def edge_cost(edge):
    cost = 1
    if edge.kind == "Unknown":
        cost += 20
    if edge.precision == "Heuristic":
        cost += 8
    if edge.provenance.kind == "AgentExtension" and not edge.provenance.validated:
        cost += 15
    if edge.kind == "Summary" and not edge.expandable:
        cost += 5
    if edge.kind == "ExplanationOnly":
        cost += 2
    return cost

def best_path(graph, starts, ends, edge_filter, budget):
    queue = PriorityQueue()
    best = {}
    prev = {}

    for start in starts:
        queue.push((0, start))
        best[start] = 0

    while queue and not budget.exceeded():
        cost, node = queue.pop_min()
        if node in ends:
            return reconstruct(prev, node)

        for edge in graph.out_edges(node).sorted_by_stable_id():
            if not edge_filter(edge):
                continue
            next_cost = cost + edge_cost(edge)
            if next_cost < best.get(edge.dst, INF):
                best[edge.dst] = next_cost
                prev[edge.dst] = (node, edge.id)
                queue.push((next_cost, edge.dst))

    return NoPath(status=budget.status())
```

Cost: `O(E log V)` for one weighted path. Use bounded k-path extraction only
after one path works and only with strict caps.

## Context-Matched Interprocedural Traversal

```python
def successors_with_context(graph, state):
    node, ctx = state.node, state.context

    for edge in graph.out_edges(node).sorted_by_stable_id():
        if edge.kind == "Call":
            if ctx.depth == ctx.max_depth:
                yield unknown_summary_state(edge, ctx)
            else:
                yield State(edge.dst, ctx.push(edge.call_site))

        elif edge.kind == "Return":
            if ctx.top() == edge.call_site:
                yield State(edge.dst, ctx.pop())
            else:
                continue  # reject unrealizable path

        elif edge.kind == "Summary":
            yield State(edge.dst, ctx)  # compressed callee behavior

        else:
            yield State(edge.dst, ctx)
```

The cache key must include `node`, `ctx`, graph version, query mode, edge filter,
and extension digest.

## Summary Edge Expansion

```python
def expand_edge(edge, expansion_store, budget):
    if edge.kind != "Summary":
        return [edge]

    expansion = expansion_store.lookup(edge.expandable)
    if expansion is None:
        return [OpaqueStep(edge, reason="no_expansion")]

    if budget.summary_depth_exceeded():
        return [OpaqueStep(edge, reason="summary_depth")]

    return expansion.path_or_slice
```

Do not require every summary to be expandable. Opaque summaries are acceptable
when they are labeled with precision and provenance.

## Evidence Bundle Assembly

```python
def build_evidence_bundle(diagnostic, query_result):
    labels = collect_primary_and_related_locations(diagnostic, query_result)
    paths = compress_and_rank_paths(query_result.paths)
    slices = summarize_slices(query_result.slices)
    unknowns = collect_unknowns(query_result)

    return EvidenceBundle(
        primary=diagnostic.primary_location,
        labels=labels,
        paths=paths,
        slices=slices,
        unknowns=unknowns,
        provenance=collect_provenance(query_result),
        replay_key=make_replay_key(diagnostic, query_result),
        status=query_result.status,
        precision=query_result.precision,
    )
```

## Extension Merge Validation

```python
def merge_extension_edge(edge, provider_policy, validation):
    if not referenced_nodes_exist(edge):
        return Reject("missing node")

    if not source_spans_exist(edge):
        return Reject("missing evidence span")

    if edge.claims_exact_semantic and not provider_policy.can_claim_exact:
        edge.precision = "DeclaredExternal"
        return AcceptWithPrecisionDowngrade(edge)

    if edge.suppresses_native_may_edge and not validation.has_fixture:
        return CandidateOnly("suppression needs fixture")

    if edge.adds_unbounded_expansion:
        return Reject("unbounded expansion")

    return Accept(edge)
```

Extension facts should be useful, but they must not silently rewrite the truth
model of the engine.
