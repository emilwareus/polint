# Bootstrap Algorithms

## Provider DAG

Python-ish sketch:

```python
def schedule(requested_families, providers):
    needed = closure_over_requirements(requested_families, providers)
    graph = dependency_graph(needed)
    order = stable_toposort(graph, key=lambda provider: provider.name)
    return order
```

Complexity: `O(P + D)` for providers and dependency edges.

Validation:

- cycle detection;
- missing provider diagnostic;
- deterministic order test.

## Stable Fact Key

```python
def stable_key(family, language, file_key, owner_key, span, local_parts):
    parts = [
        tagged("family", family),
        tagged("language", language),
        tagged("file", file_key or "<none>"),
        tagged("owner", owner_key or "<none>"),
        tagged("span", span_key(span) if span else "<none>"),
    ]
    for name, value in local_parts:
        parts.append(tagged(name, normalize(value)))
    return length_prefix_join(parts)
```

Complexity: linear in key length.

Validation:

- path separator normalization;
- length-prefix collision avoidance for adjacent fields;
- stable hash tests.

## MIR Lowering

```python
def lower_function(ast_function):
    body = MirBody()
    for ast_node in walk_body(ast_function):
        match ast_node.kind:
            case "assignment":
                body.emit(Assign(place(ast_node.left), rvalue(ast_node.right)))
            case "return":
                body.emit(Return(operand(ast_node.value)))
            case "call":
                site = calls.add_site(ast_node)
                body.emit(Call(site))
            case "if":
                body.emit(Branch(operand(ast_node.condition)))
            case _:
                body.emit(Unsupported(reason(ast_node)))
    return body
```

Complexity: `O(AST nodes visited)`.

Validation:

- unsupported nodes preserved;
- spans round-trip;
- no parser references escape.

## Place Interning

```python
def intern_place(place_key):
    existing = places.by_key.get(place_key)
    if existing:
        return existing
    place_id = PlaceId(len(places.items))
    places.items.append(PlaceFact(place_id, place_key))
    places.by_key[place_key] = place_id
    return place_id
```

Complexity: `O(log P)` with `BTreeMap`.

Validation:

- deterministic key order;
- same source yields same place keys.

## Direct Calls

```python
def build_direct_calls(mir, symbols):
    for op in mir.ops:
        if op.kind != "Call":
            continue
        site = calls.site(op.site)
        target = resolve_static_target(site, symbols)
        if target:
            emit_target(site.id, target, status="resolved")
        else:
            emit_target(site.id, None, status="unresolved", reason=classify(site))
```

Complexity: `O(call_sites * lookup_cost)`.

Validation:

- unresolved reason emitted;
- direct static calls resolved;
- dynamic calls not guessed as exact.

## P0 Domains

```python
def solve_local_domain(cfg, domain):
    state_in = {cfg.entry: domain.bottom()}
    worklist = [cfg.entry]
    while worklist:
        block = worklist.pop()
        out = transfer_block(block, state_in[block], domain)
        for edge in cfg.successors(block):
            narrowed = apply_edge_condition(edge, out, domain)
            changed = join_into(state_in[edge.to], narrowed, domain)
            if changed:
                worklist.append(edge.to)
    return state_in
```

Complexity: bounded by `O(edges * iterations * transfer_cost)`.

Validation:

- lattice laws;
- transfer monotonicity;
- widening for loops when needed.

## Direct Summaries

```python
def summarize_function(body, p0_state, direct_calls):
    summary = Summary()
    summary.params = parameter_facts(body, p0_state)
    summary.returns = return_facts(body, p0_state)
    summary.calls = direct_call_effects(direct_calls)
    summary.external_effects = conservative_external_effects(body)
    summary.dependencies = digest_dependencies(body, direct_calls)
    return summary
```

Complexity: local body pass plus direct call dependency collection.

Validation:

- summary changes when body changes;
- summary changes when direct callee summary changes;
- unknown/havoc policy explicit.
