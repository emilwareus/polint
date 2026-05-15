# Core Call Graph Algorithms

This file uses Python-ish pseudocode to show the algorithms without tying them to a specific implementation language.

## Precision Ladder

| Tier | Algorithm | Best Use | Main Risk |
|---:|---|---|---|
| 0 | Syntactic call sites | Fast repo-local facts | No target resolution. |
| 1 | Name/import binding | Direct calls and module-local references | Needs language-specific scoping. |
| 2 | CHA | OO dispatch baseline | Huge over-approximation. |
| 3 | RTA | Closed-world applications | Needs entrypoints and complete lifecycle. |
| 4 | VTA/type propagation | Better receiver precision | Needs assignment/type-flow graph. |
| 5 | Points-to / CFA | Higher-order and object dispatch | Cost and memory growth. |
| 6 | Context-sensitive points-to | High precision Java/JVM-like analysis | Context explosion. |
| 7 | Query/dataflow/CPG systems | Security and policy queries | Quality depends on all upstream facts. |

## Tier 0: Syntactic Call Sites

```python
def emit_call_sites(files):
    sites = []
    for file in files:
        for call in parse(file).call_expressions():
            sites.append(CallSite(
                id=fresh_id(),
                file=file.path,
                span=call.span,
                enclosing_callable=enclosing_callable(call),
                callee_syntax=source_text(call.callee),
                receiver_syntax=receiver_text(call),
                argument_syntax=[source_text(arg) for arg in call.args],
            ))
    return sites
```

Use this for all languages. It is cheap and useful even when resolution is impossible.

## Tier 1: Name And Import Binding

```python
def build_scope_graph(files):
    scopes = ScopeGraph()
    for file in files:
        module_scope = scopes.scope_for_module(file)

        for decl in file.declarations:
            scopes.define(module_scope, decl.name, decl.symbol_id)

        for imp in file.imports:
            target_scope = resolve_import_scope(imp)
            scopes.add_import_edge(module_scope, target_scope)

        for block in file.blocks:
            block_scope = scopes.new_scope(parent=enclosing_scope(block))
            for decl in block.declarations:
                scopes.define(block_scope, decl.name, decl.symbol_id)

    return scopes

def resolve_identifier_call(site, scopes):
    if not is_identifier(site.callee_syntax):
        return []
    scope = scope_at(site.span)
    return scopes.resolve(site.callee_syntax, scope)
```

This tier turns `foo()` into a symbol when lexical and import semantics are enough.

## Tier 2: CHA

Class Hierarchy Analysis resolves virtual calls by considering every concrete subtype of the declared receiver type.

```python
def cha_targets(call, hierarchy):
    if call.is_static_or_special:
        return {call.declared_target}

    receiver_type = call.declared_receiver_type
    targets = set()
    for cls in hierarchy.subtypes_including_self(receiver_type):
        if cls.is_abstract:
            continue
        method = hierarchy.dispatch(cls, call.method_signature)
        if method:
            targets.add(method)
    return targets

def cha_call_graph(entrypoints, calls_by_function, hierarchy):
    graph = Graph()
    work = list(entrypoints)
    seen = set()

    while work:
        fn = work.pop()
        if fn in seen:
            continue
        seen.add(fn)

        for call in calls_by_function[fn]:
            for target in cha_targets(call, hierarchy):
                if graph.add_edge(fn, call, target, algorithm="cha"):
                    work.append(target)

    return graph
```

CHA is a good baseline for Java and library-style Go interface analysis, but it over-approximates heavily.

## Tier 3: RTA

Rapid Type Analysis restricts CHA targets to instantiated classes discovered from reachable code.

```python
def rta_call_graph(entrypoints, calls_by_function, allocations_by_function, hierarchy):
    graph = Graph()
    reachable = set(entrypoints)
    instantiated = set()
    work = list(entrypoints)
    deferred_virtual_sites = []

    def try_resolve(call):
        if call.is_static_or_special:
            return {call.declared_target}
        return {
            target for target in cha_targets(call, hierarchy)
            if target.owner_type in instantiated
        }

    while work:
        fn = work.pop()

        for cls in allocations_by_function[fn]:
            if cls not in instantiated:
                instantiated.add(cls)
                for old_call in deferred_virtual_sites:
                    work.append(old_call.enclosing_callable)

        for call in calls_by_function[fn]:
            if call.is_virtual:
                deferred_virtual_sites.append(call)

            for target in try_resolve(call):
                if graph.add_edge(fn, call, target, algorithm="rta"):
                    if target not in reachable:
                        reachable.add(target)
                        work.append(target)

    return graph
```

RTA is strong for closed-world binaries and tests. It needs configured entrypoints and lifecycle knowledge.

## Tier 4: VTA / Type Propagation

Variable Type Analysis propagates possible receiver types through assignments, calls, returns, fields, and containers.

```python
def build_type_flow_constraints(program):
    types = defaultdict(set)
    constraints = []

    for stmt in program.statements:
        if stmt.kind == "new":
            types[stmt.lhs].add(stmt.allocated_type)

        elif stmt.kind == "assign":
            constraints.append((stmt.rhs_var, stmt.lhs_var))

        elif stmt.kind == "call_arg":
            constraints.append((stmt.actual_var, stmt.formal_var))

        elif stmt.kind == "return":
            constraints.append((stmt.return_var, stmt.call_result_var))

        elif stmt.kind == "field_store":
            constraints.append((stmt.value_var, field_node(stmt.field_name)))

        elif stmt.kind == "field_load":
            constraints.append((field_node(stmt.field_name), stmt.lhs_var))

    return types, constraints

def solve_type_flow(types, constraints):
    dependents = index_dependents(constraints)
    work = list(constraints)

    while work:
        src, dst = work.pop()
        if union_into(types[dst], types[src]):
            work.extend(dependents[dst])

    return types

def vta_targets(call, types, hierarchy):
    if call.is_static_or_special:
        return {call.declared_target}

    return {
        hierarchy.dispatch(cls, call.method_signature)
        for cls in types[call.receiver_var]
    } - {None}
```

Go x/tools VTA is the best concrete source inspected for this tier.

## Tier 5: Andersen-Style Points-To

Inclusion-based points-to analysis tracks which abstract objects may flow to variables and fields.

```python
def andersen(constraints):
    pts = defaultdict(set)
    subset_edges = defaultdict(set)
    loads = []
    stores = []
    work = deque()

    for c in constraints:
        if c.kind == "addr":       # x = &o
            if add(pts[c.var], c.obj):
                work.append(c.var)
        elif c.kind == "copy":     # x = y
            subset_edges[c.src].add(c.dst)
        elif c.kind == "load":     # x = *y
            loads.append(c)
        elif c.kind == "store":    # *x = y
            stores.append(c)

    while work:
        var = work.popleft()

        for dst in subset_edges[var]:
            if union_into(pts[dst], pts[var]):
                work.append(dst)

        for load in loads_where_source_is(var):
            for obj in pts[var]:
                subset_edges[obj].add(load.dst)
                if union_into(pts[load.dst], pts[obj]):
                    work.append(load.dst)

        for store in stores_where_target_is(var):
            for obj in pts[var]:
                subset_edges[store.src].add(obj)
                if union_into(pts[obj], pts[store.src]):
                    work.append(obj)

    return pts
```

Call graph extraction:

```python
def points_to_edges(calls, pts, hierarchy):
    edges = []
    for call in calls:
        if call.is_function_value_call:
            for fn_obj in pts[call.callee_var]:
                edges.append(edge(call, fn_obj.function, "points_to"))

        elif call.is_virtual:
            for recv_obj in pts[call.receiver_var]:
                target = hierarchy.dispatch(recv_obj.type, call.method_signature)
                if target:
                    edges.append(edge(call, target, "points_to"))

    return edges
```

This is the basis of Soot Spark, WALA propagation builders, Doop, and Tai-e PTA-based call graphs.

## Tier 6: Context Sensitivity

Context sensitivity clones abstract analysis state by call strings, receiver objects, allocation sites, or receiver types.

```python
def next_context(policy, caller_context, call_site, receiver_object):
    if policy == "0-cfa":
        return ()
    if policy == "k-call":
        return (call_site,) + caller_context[:k - 1]
    if policy == "k-object":
        return (receiver_object.allocation_site,) + caller_context[:k - 1]
    if policy == "k-type":
        return (receiver_object.type,) + caller_context[:k - 1]

def context_sensitive_virtual_edges(call, caller_context, pts, hierarchy):
    for receiver_object in pts[(call.receiver_var, caller_context)]:
        callee_context = next_context(policy, caller_context, call, receiver_object)
        target = hierarchy.dispatch(receiver_object.type, call.method_signature)
        if target:
            yield edge(call, target, context=callee_context)
```

This is valuable for Java/JVM precision, but too expensive to make the default in polint.

## Tier 7: Query/Dataflow Graphs

CodeQL and Code Property Graph systems make call resolution a queryable relation over syntax, symbols, dataflow, type tracking, framework models, and summaries.

```python
def query_style_resolution(site, facts):
    emit_call_site(site)

    for target in direct_binding(site, facts):
        emit_edge(site, target, "bound", confidence="high")

    for target in type_or_value_flow_to_callee(site, facts):
        emit_edge(site, target, "dataflow", confidence="medium")

    for target in framework_or_reflection_model(site, facts):
        emit_edge(site, target, "heuristic", confidence="low")

    if no_targets(site):
        emit_unresolved(site, reason=best_unresolved_reason(site, facts))
```

The important lesson is architecture: call sites, callees, imprecision, and incompleteness are represented separately.

