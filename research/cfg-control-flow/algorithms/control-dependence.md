# Algorithm: Dominators, Postdominators, And Control Dependence

## Dominators

For the first implementation, use a deterministic simple algorithm. Function CFGs are usually small enough, and this version is easy to validate.

```python
def reachable_nodes(graph, entry):
    seen = set()
    stack = [entry]
    while stack:
        n = stack.pop()
        if n in seen:
            continue
        seen.add(n)
        for succ in reversed(sorted(graph.successors(n))):
            stack.append(succ)
    return seen
```

```python
def dominators(graph, entry):
    nodes = reachable_nodes(graph, entry)
    dom = {n: set(nodes) for n in nodes}
    dom[entry] = {entry}

    changed = True
    while changed:
        changed = False
        for n in sorted(nodes):
            if n == entry:
                continue
            preds = [p for p in graph.predecessors(n) if p in nodes]
            if not preds:
                new = {n}
            else:
                new = set(nodes)
                for p in preds:
                    new &= dom[p]
                new.add(n)
            if new != dom[n]:
                dom[n] = new
                changed = True

    return dom
```

Immediate dominators:

```python
def immediate_dominators(dom):
    idom = {}
    for n, ds in dom.items():
        strict = ds - {n}
        candidates = []
        for d in strict:
            if all(d == other or d not in dom[other] for other in strict):
                candidates.append(d)
        idom[n] = only(candidates)
    return idom
```

Upgrade path: replace with Lengauer-Tarjan or Semi-NCA if benchmarks show the simple algorithm is too slow.

## Postdominators

Postdominators are dominators on a reversed graph with a synthetic unified exit.

```python
def add_unified_exit(cfg, view):
    g = cfg.selected_view(view).copy()
    exit = g.new_synthetic_node("UnifiedExit")

    for node in g.nodes:
        if g.is_normal_exit(node):
            g.add_edge(node, exit, "ExitNormal")
        if view.includes_exceptional_exits and g.is_exceptional_exit(node):
            g.add_edge(node, exit, "ExitExceptional")
        if view.treats_infinite_loops_as_exits and g.is_infinite_loop_tail(node):
            g.add_edge(node, exit, "SyntheticLoopExit", precision="Conservative")

    return g, exit
```

```python
def postdominators(cfg, view):
    g, exit = add_unified_exit(cfg, view)
    reverse = g.reversed()
    return dominators(reverse, exit)
```

The exit policy is part of the cache key and precision of the derived fact.

## Control Dependence

Classic edge-based control dependence:

```python
def controls(edge, node, postdom):
    a = edge.from_block
    b = edge.to_block
    return postdom.dominates(node, b) and not postdom.strictly_dominates(node, a)
```

Practical tree-walk algorithm:

```python
def control_dependence(cfg, postdom, ipdom):
    facts = []
    for edge in cfg.block_edges():
        a = edge.from_block
        b = edge.to_block

        # If B postdominates A, this edge does not create dependence.
        if postdom.dominates(b, a):
            continue

        stop = ipdom.get(a)
        runner = b
        while runner is not None and runner != stop:
            facts.append(ControlDependence(
                controller=a,
                controlled=runner,
                via_edge=edge.id,
                edge_kind=edge.kind,
                precision=edge.precision.join(postdom.precision),
            ))
            runner = ipdom.get(runner)

    return dedupe_and_sort(facts)
```

Complexity:

```text
O(E * h)
```

where `h` is postdominator-tree height. This is acceptable for the first version. If materialized dependence facts become too large, use output-sensitive queries inspired by Bilardi/Pingali APT.

## Dominance Frontier

Dominance frontiers are useful for later SSA and sparse data flow:

```python
def dominance_frontier(graph, idom):
    frontier = {n: set() for n in graph.nodes}
    for b in graph.nodes:
        preds = graph.predecessors(b)
        if len(preds) < 2:
            continue
        for p in preds:
            runner = p
            while runner is not None and runner != idom[b]:
                frontier[runner].add(b)
                runner = idom[runner]
    return frontier
```

Do not expose dominance frontier publicly at first. Keep it internal for future SSA/value-flow construction.

## Precision Notes

Control dependence is only as precise as:

- the selected CFG view;
- the artificial exit policy;
- exceptional/finally/defer modeling;
- unreachable node handling;
- extension overlays included in the view.

Every `ControlDependenceFact` should store:

```text
cfg_view
postdominator_version
controller block
controlled block
controlling edge
edge kind
precision
provenance
unsupported notes
```
