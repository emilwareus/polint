# Core Data-Flow Algorithms

This file explains the main algorithm families in stripped-down Python-ish pseudocode.

## 1. Classic Iterative CFG Data Flow

Use for local facts such as reaching definitions, liveness, initialized variables, constant propagation, nilness, and simple taint inside one function.

```python
def solve_forward(cfg, bottom, entry_state, transfer, join):
    in_state = {block: bottom() for block in cfg.blocks}
    out_state = {block: bottom() for block in cfg.blocks}
    in_state[cfg.entry] = entry_state

    worklist = deque([cfg.entry])

    while worklist:
        block = worklist.popleft()
        new_out = transfer(block, in_state[block])

        if new_out != out_state[block]:
            out_state[block] = new_out

            for succ in block.successors:
                joined = join(in_state[succ], new_out)
                if joined != in_state[succ]:
                    in_state[succ] = joined
                    worklist.append(succ)

    return in_state, out_state
```

Polint lesson: implement this first for intraprocedural facts. It is easy to test, easy to explain, and gives rule authors immediate value.

## 2. Transfer Function With Guards

For path-sensitive-ish local precision, store branch facts without claiming full path sensitivity.

```python
def transfer_statement(state, stmt):
    match stmt:
        case Assign(lhs, rhs):
            state[place(lhs)] = eval_abstract(rhs, state)

        case If(cond):
            true_state = assume(state, cond)
            false_state = assume(state, not_(cond))
            return Branch(true_state, false_state)

        case Call(target, args):
            state = apply_call_effects(state, target, args)

    return state
```

Polint lesson: expose guard provenance on edges and paths. Do not expose path sensitivity as exact unless the solver proves feasibility.

## 3. SSA Sparse Value Flow

Dense propagation visits every CFG node. Sparse flow visits definition/use relations.

```python
def build_sparse_flow(ssa_function):
    graph = ValueFlowGraph()

    for instr in ssa_function.instructions:
        for used in instr.uses:
            graph.add_edge(def_of(used), instr.result, kind="use")

        if instr.kind == "phi":
            for input_value in instr.inputs:
                graph.add_edge(input_value, instr.result, kind="phi")

        if instr.kind == "store":
            graph.add_edge(instr.value, heap_place(instr.address), kind="store")

        if instr.kind == "load":
            graph.add_edge(heap_place(instr.address), instr.result, kind="load")

    return graph
```

Polint lesson: lower each language into a small native "places and operations" model. Do not expose SSA internals as public SDK.

## 4. Taint Propagation

Taint is a may-flow problem: if any source reaches a sink without a barrier/sanitizer, report a path.

```python
def local_taint(function, sources, sinks, sanitizers):
    tainted = set()
    paths = PathStore()

    for node in function.flow_nodes:
        if matches_source(node, sources):
            tainted.add(node)
            paths.start(node)

    changed = True
    while changed:
        changed = False

        for edge in function.flow_edges:
            if edge.from_node not in tainted:
                continue

            if matches_sanitizer(edge, sanitizers):
                continue

            if edge.to_node not in tainted:
                tainted.add(edge.to_node)
                paths.extend(edge)
                changed = True

            if matches_sink(edge.to_node, sinks):
                emit_path(paths.path_to(edge.to_node))
```

Polint lesson: make source/sink/sanitizer definitions rule-provided or config-provided. The engine provides flow facts and path search.

## 5. Bounded Access Paths

Access paths track fields/properties without modeling an unbounded heap.

```python
MAX_DEPTH = 4

def normalize_place(base, selectors):
    selectors = selectors[:MAX_DEPTH]
    if len(selectors) == MAX_DEPTH and has_more_selectors():
        selectors[-1] = WILDCARD
    return Place(base, tuple(selectors))

def field_write(state, obj, field, value):
    dst = normalize_place(obj.base, obj.selectors + [field])
    state[dst] = state[value]

def field_read(state, obj, field):
    src = normalize_place(obj.base, obj.selectors + [field])
    if src in state:
        return state[src]
    return join_parent_or_unknown(obj)
```

Polint lesson: access-path depth must be configurable and included in cache digests. Unknown/wildcard paths must be visible in provenance.

## 6. Function Summaries

Summaries are the bridge from local flow to interprocedural flow.

```python
FunctionSummary(
    function,
    param_to_return,
    param_to_param,
    param_to_field,
    receiver_to_return,
    receiver_mutations,
    global_reads,
    global_writes,
    sources_returned,
    sinks_reached,
    sanitizers_applied,
    unknown_effects,
)
```

```python
def summarize_function(function, local_flow, callees):
    summary = empty_summary(function)

    for param in function.params:
        for ret in function.returns:
            if local_flow.reaches(param, ret):
                summary.param_to_return.add(param, ret)

    for call in function.calls:
        callee_summary = callees.get(call.target)

        if callee_summary is None:
            summary.unknown_effects.add(call)
            continue

        summary.compose(call, callee_summary)

    return summary
```

Polint lesson: store summaries as compact facts, not all transitive paths. Path reconstruction can be demand-driven.

## 7. Interprocedural Summary Fixed Point

Pysa-style summary propagation iterates over the call dependency graph.

```python
summaries = {fn: empty_summary(fn) for fn in all_functions}
worklist = reverse_topological_functions(call_graph)

while worklist:
    fn = worklist.pop()
    old = summaries[fn]
    new = summarize_function(fn, local_flow[fn], summaries)

    if new != old:
        summaries[fn] = widen_if_needed(old, new)
        for caller in call_graph.callers(fn):
            worklist.add(caller)
```

Polint lesson: this is the right first interprocedural engine. It composes naturally with the planned call-graph facts.

## 8. IFDS

IFDS solves finite distributive subset problems by reachability in an exploded supergraph.

```python
PathEdge(start_node, fact_at_start, program_node, fact_at_node)

def ifds_solve(icfg, flow_functions, seeds):
    reached = set()
    worklist = deque()

    for seed_node, zero_fact in seeds:
        edge = PathEdge(seed_node, zero_fact, seed_node, zero_fact)
        reached.add(edge)
        worklist.append(edge)

    while worklist:
        edge = worklist.popleft()
        n = edge.program_node
        d = edge.fact_at_node

        for successor in icfg.successors(n):
            for d2 in flow_functions.normal(n, successor)(d):
                propagate(edge.start_node, edge.fact_at_start, successor, d2)

        if icfg.is_call(n):
            for callee_entry in icfg.callees(n):
                for d2 in flow_functions.call(n, callee_entry)(d):
                    propagate(edge.start_node, edge.fact_at_start, callee_entry, d2)

        if icfg.is_exit(n):
            for call_site in icfg.callers_of_exit(n):
                for return_site in icfg.return_sites(call_site):
                    for d2 in flow_functions.return_(call_site, n, return_site)(d):
                        propagate(edge.start_node, edge.fact_at_start, return_site, d2)
```

Polint lesson: IFDS is useful once we have a stable ICFG and finite data-flow facts. It should be internal; SDK users should see paths and precision, not tabulation edges.

## 9. IDE

IDE extends IFDS by attaching edge functions over values.

```python
def compose_edge_value(old_value, edge_function):
    return edge_function(old_value)

def join_values(left, right):
    return lattice_join(left, right)
```

Use for constants, typestate, small taint kinds with transforms, or source/sink kinds where an edge changes the value.

Polint lesson: keep IDE for later. Start with simpler summary fixed points and finite taint paths.

## 10. Abstract Interpretation

Use for constants, ranges, nullness, initializedness, and path predicates.

```python
class Domain:
    def bottom(self): ...
    def join(self, a, b): ...
    def widen(self, old, new): ...
    def transfer(self, stmt, state): ...

def abstract_interpret(function, domain):
    return solve_forward(
        cfg=function.cfg,
        bottom=domain.bottom,
        entry_state=domain.entry(function),
        transfer=domain.transfer_block,
        join=domain.join,
    )
```

Polint lesson: separate value domains from taint/reachability. Do not force all data-flow questions into a source-sink model.

## 11. Points-To Assisted Data Flow

Heap-sensitive data flow needs alias information.

```python
def points_to(program):
    pts = defaultdict(set)
    constraints = collect_constraints(program)
    worklist = deque(constraints)

    while worklist:
        constraint = worklist.popleft()

        if constraint.kind == "addr":
            changed = pts[constraint.var].add(constraint.alloc)

        if constraint.kind == "copy":
            changed = union_into(pts[constraint.dst], pts[constraint.src])

        if constraint.kind == "load":
            for obj in pts[constraint.base]:
                changed = union_into(pts[constraint.dst], heap[obj, constraint.field])

        if constraint.kind == "store":
            for obj in pts[constraint.base]:
                changed = union_into(heap[obj, constraint.field], pts[constraint.src])

        if changed:
            worklist.extend(dependents(constraint))

    return pts
```

Polint lesson: do not start here for every language. Add it after local flow, summaries, and call graph facts exist.

## 12. Datalog / Semi-Naive Fixed Point

Represent facts as relations and rules as joins.

```python
Reachable(x, y) :- Edge(x, y).
Reachable(x, z) :- Reachable(x, y), Edge(y, z).
```

Semi-naive evaluation only joins against newly added facts:

```python
delta_reachable = edge_facts
reachable = set(delta_reachable)

while delta_reachable:
    new_delta = set()

    for (x, y) in delta_reachable:
        for z in edge_successors[y]:
            if (x, z) not in reachable:
                new_delta.add((x, z))

    reachable |= new_delta
    delta_reachable = new_delta
```

Polint lesson: a native Rust relational engine is a strong fit for internal facts. Do not expose Datalog as the rule-authoring API initially.

## 13. Demand-Driven Path Search

Rules usually ask for a small set of source/sink pairs.

```python
def reaches(source, sink, graph, budget):
    queue = deque([(source, [])])
    seen = set([source])

    while queue and budget.remaining():
        node, path = queue.popleft()

        if node == sink:
            return path

        for edge in graph.outgoing(node):
            if not edge_allowed(edge):
                continue
            if edge.to_node not in seen:
                seen.add(edge.to_node)
                queue.append((edge.to_node, path + [edge]))

    return None
```

Polint lesson: combine precomputed local/summaries with demand-driven path queries. AI-authored rules should be able to ask targeted questions cheaply.

## 14. Unknown Calls And Havoc

Dynamic languages and missing setup must not silently drop flow.

```python
def apply_unknown_call(state, call):
    for arg in call.args:
        emit_edge(arg, UnknownEffect(call), kind="unknown_call")

    if call.may_return_unknown:
        emit_edge(UnknownEffect(call), call.return_value, kind="havoc_return")

    for mutable in mutable_args(call):
        emit_edge(UnknownEffect(call), mutable, kind="havoc_mutation")

    record_reason(call, "unknown_callee")
```

Polint lesson: unknown/havoc facts are part of the product. Rule authors can choose whether to include or exclude them.

