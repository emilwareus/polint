# Core Algorithms

## Layered Solver Strategy

```python
def analyze_repo(repo):
    sem = build_semantic_index(repo)
    modules = build_module_graph(repo)
    cfgs = build_cfgs(repo, sem)

    places = build_places(repo, sem, cfgs)
    types = build_type_facts(repo, sem, modules, places)
    values = build_value_facts(repo, sem, cfgs, places, types)
    local_flow = build_local_flow(cfgs, places, types, values)
    summaries = build_summaries(repo, sem, places, types, values, local_flow)

    # Points-to is not mandatory for every rule.
    pt = lazy_points_to_provider(places, values, summaries)
    aliases = alias_provider_stack(places, types, values, local_flow, pt)
    return FactWorld(sem, modules, cfgs, places, types, values, local_flow, summaries, pt, aliases)
```

## Places And Access Paths

```python
def place_for_expr(expr):
    match expr:
        case Name(id):
            return local_or_global_place(id)
        case Attribute(base, attr):
            base_place = place_for_expr(base)
            return field_place(base_place, FieldKey.name(attr))
        case Subscript(base, key):
            base_place = place_for_expr(base)
            key_model = index_key_model(key)
            return index_place(base_place, key_model)
        case This() | Self():
            return receiver_place(current_function)
        case ImportBinding(module, name):
            return module_export_place(module, name)
        case _:
            return temporary_place(expr)
```

Rules:

- access paths must be bounded;
- dynamic keys produce `UnknownKey` or `DynamicKey` facts;
- source spans are evidence, not identity;
- module/package context participates in identity.

## Local Type/Value Flow

```python
def transfer(stmt, state):
    match stmt:
        case Assign(lhs, rhs):
            value = eval_abstract(rhs, state)
            place = place_for_expr(lhs)
            state.type[place] = value.type
            state.value[place] = value
            state.bound[place] = True
            invalidate_alias_narrowing(place, state)

        case IfCondition(cond, assume=True):
            state = narrow_by_condition(cond, assume, state)

        case Call(target, args):
            summary = summary_for_call(target, state)
            apply_summary(summary, args, state)

        case Return(expr):
            state.return_values.add(eval_abstract(expr, state))

    return state
```

## Guard/Narrowing Pattern

```python
def narrow_by_condition(cond, assume, state):
    match cond:
        case IsNone(place):
            return refine_nullness(place, is_null=assume, state=state)
        case Typeof(place, name):
            return refine_typeof(place, name, assume, state)
        case InstanceOf(place, ctor):
            return refine_instance(place, ctor, assume, state)
        case IsInstance(place, py_type):
            return refine_python_isinstance(place, py_type, assume, state)
        case InProperty(prop, place):
            return refine_property_presence(place, prop, assume, state)
        case Equality(left, right):
            return refine_equality(left, right, assume, state)
        case And(a, b):
            return narrow_by_condition(b, assume, narrow_by_condition(a, assume, state))
        case Or(a, b):
            return join(
                narrow_by_condition(a, assume, state),
                narrow_by_condition(b, assume, state),
            )
        case Not(inner):
            return narrow_by_condition(inner, not assume, state)
        case _:
            return state.with_unknown_narrowing(cond)
```

## Andersen-Style Points-To Constraints

### Constraint Generation

```python
def emit_constraints(stmt):
    match stmt:
        case Assign(x, AddressOf(o)):
            emit(AddressOf(dst=var(x), object=object_token(o)))

        case Assign(x, y):
            emit(Copy(dst=var(x), src=var(y)))

        case Assign(x, Deref(y)):
            emit(Load(dst=var(x), pointer=var(y)))

        case Store(Deref(x), y):
            emit(Store(pointer=var(x), src=var(y)))

        case Assign(x, Field(y, f)):
            emit(FieldLoad(dst=var(x), base=var(y), field=f))

        case Assign(Field(x, f), y):
            emit(FieldStore(base=var(x), field=f, src=var(y)))

        case Call(dst, callee, args):
            for target in possible_targets(callee):
                summary = summary_for(target)
                instantiate_summary(summary, dst, args)
```

### Worklist Solver

```python
def solve(constraints, budget):
    pt = Bitsets()
    copy_edges = Graph()
    load_edges = MultiMap()
    store_edges = MultiMap()
    field_loads = MultiMap()
    field_stores = MultiMap()

    for c in constraints:
        match c:
            case AddressOf(dst, obj):
                pt[dst].add(obj)
                enqueue(dst, obj)
            case Copy(dst, src):
                copy_edges.add(src, dst)
            case Load(dst, pointer):
                load_edges[pointer].append(dst)
            case Store(pointer, src):
                store_edges[pointer].append(src)
            case FieldLoad(dst, base, field):
                field_loads[base].append((field, dst))
            case FieldStore(base, field, src):
                field_stores[base].append((field, src))

    collapse_copy_sccs(copy_edges)
    initialize_copy_deltas(pt, copy_edges)

    while queue and budget.remaining():
        var, delta = queue.pop_delta()

        for dst in copy_edges.successors(var):
            if pt[dst].add_all(delta):
                queue.push(dst, delta)

        for dst in load_edges[var]:
            for obj in delta:
                add_dynamic_copy_edge(object_contents(obj), dst, pt, queue)

        for src in store_edges[var]:
            for obj in delta:
                add_dynamic_copy_edge(src, object_contents(obj), pt, queue)

        for field, dst in field_loads[var]:
            for obj in delta:
                add_dynamic_copy_edge(object_field(obj, field), dst, pt, queue)

        for field, src in field_stores[var]:
            for obj in delta:
                add_dynamic_copy_edge(src, object_field(obj, field), pt, queue)

    if not budget.remaining():
        mark_unknown("points-to budget exhausted")

    return pt
```

## Alias Query

```python
def may_alias(a, b):
    answer = alias(a, b)
    return answer in {MayAlias, MustAlias, PartialAlias, Unknown}

def alias(a, b):
    if a == b:
        return MustAlias("same stable place")

    if definitely_disjoint_by_scope(a, b):
        return NoAlias("scope/ownership")

    if definitely_disjoint_by_type(a, b):
        return NoAlias("disjoint type sets")

    ext = extension_alias_fact(a, b)
    if ext and ext.validated:
        return ext.answer

    pts_a = points_to(a)
    pts_b = points_to(b)

    if pts_a.unknown or pts_b.unknown:
        return Unknown("missing points-to")

    if pts_a.is_disjoint(pts_b):
        return NoAlias("disjoint points-to sets")

    if pts_a.same_singleton(pts_b) and mutation_model_supports_identity(a, b):
        return MustAlias("same singleton object token")

    if partial_overlap(pts_a, pts_b):
        return PartialAlias("overlapping aggregate/object fields")

    return MayAlias("conservative overlap")
```

## Context Sensitivity

Start context-insensitive. Add selective context sensitivity later:

```python
ContextKey =
    Empty
  | CallString(last_k_call_sites)
  | ReceiverObject(object_token)
  | ReceiverType(type_id)
  | ExtensionProvided(key)
```

Rules:

- context keys are part of cache keys;
- depth is budgeted;
- context sensitivity can be requested by providers, not globally enabled;
- default mode should not explode on large repos.

## Sparse Flow-Sensitive Future

Dense flow-sensitive points-to:

```text
Pt[var, program_point]
```

is expensive. Prefer sparse representation later:

```text
MemoryDef / MemoryUse / MemoryPhi
  -> def-use memory graph
  -> alias-aware clobber/value-flow queries
  -> sparse points-to refinements
```

Pseudo:

```python
def sparse_query(use):
    def_node = nearest_memory_def(use)
    while def_node:
        if alias(def_node.place, use.place).may_overlap():
            return def_node
        def_node = previous_memory_def(def_node)
    return Unknown("no clobber found within budget")
```

This should come after the first points-to provider and summary system.
