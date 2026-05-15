# Go Data-Flow Notes

## Summary

Go is the best first language for serious native semantic data flow after the common substrate exists. The language is statically typed, has well-defined packages, and the main precision challenges are interfaces, function values, pointers, heap fields, closures, goroutines, channels, reflection, `unsafe`, cgo, and build tags.

The call-graph research matters directly: interprocedural Go data flow needs direct/static calls, concrete receiver method calls, interface dispatch, and function-value calls. Missing call edges cause false negatives; extra interface candidates cause false positives and state growth.

## OSS References

- Go taint: `repos/go-taint/check.go`, `repos/go-taint/walk_ssa.go`, `repos/go-taint/callgraphutil/*`.
- CodeQL Go: `repos/codeql/go/ql/lib/semmle/go/dataflow/*`.
- gosec: `repos/gosec/analyzers/*`.
- NilAway: `repos/nilaway/*`.

## Required Facts

```python
GoFacts(
    packages,
    files,
    imports,
    build_tags,
    functions,
    methods,
    receiver_types,
    interfaces,
    method_sets,
    declarations,
    references,
    lexical_scopes,
    call_sites,
    call_edges,
    cfgs,
    places,
    assignments,
    loads,
    stores,
    address_of,
    deref,
    composite_literals,
    type_assertions,
    closures,
    free_vars,
    goroutines,
    channels,
    reflection_markers,
    unsafe_markers,
)
```

## Local Flow

Start with SSA-like local places:

```python
for instr in function.instructions:
    match instr:
        case Assign(lhs, rhs):
            edge(value(rhs), place(lhs), "assignment")

        case Store(addr, value):
            edge(value, place(addr), "store")

        case Load(addr):
            edge(place(addr), result(instr), "load")

        case FieldAddr(base, field):
            edge(place(base), place(base, field), "field_address")

        case Phi(inputs):
            for input in inputs:
                edge(input, result(instr), "phi")
```

## Interprocedural Flow

Use summaries first:

```python
Summary(
    param_to_return,
    param_to_param,
    param_to_field,
    receiver_to_return,
    receiver_mutation,
    global_effects,
    channel_send_receive,
    unknown_effects,
)
```

Apply call summaries:

```python
for call in function.calls:
    targets = call_graph.targets(call)

    if not targets:
        emit_havoc(call, reason="unresolved_go_call")
        continue

    for target in targets:
        summary = summaries[target]
        bind_actuals_to_formals(call, summary)
        bind_returns(call, summary)
        bind_receiver_mutations(call, summary)
```

## Heap And Access Paths

Use bounded access paths:

```python
Place(param("r"), [".Body"])
Place(local("user"), [".Profile", ".Email"])
Place(local("items"), ["[*]"])
```

For v1, do:

- field-sensitive locals;
- bounded struct fields;
- wildcard for arrays/slices/maps;
- address/deref modeled conservatively;
- unknown pointer aliases produce conservative edges with `precision = conservative`.

Later, add allocation-site points-to for:

- interface values;
- pointer fields;
- escaping locals;
- closures;
- heap objects.

## Goroutines And Channels

Do not attempt precise happens-before in v1.

First model:

```python
go f(x)        -> edge(x, spawned_call.arg0, "goroutine_arg")
ch <- x        -> edge(x, channel(ch), "channel_send")
y := <-ch      -> edge(channel(ch), y, "channel_receive")
```

If channel identity is unknown, emit `UnknownEffect` or a wildcard channel place.

## Reflection, Unsafe, Cgo

Emit explicit unknown facts:

```python
reflect.Value.Call        -> unknown_call
MethodByName(nonliteral)  -> unknown_method
unsafe.Pointer           -> pointer_havoc
cgo call                 -> external_call
```

Literal reflection can be modeled later:

```python
MethodByName("ServeHTTP") -> candidate method named ServeHTTP
```

## Recommended Go Milestones

1. Syntax/local places and intraprocedural flow.
2. Direct calls and summary application.
3. Concrete receiver methods and package-level import binding.
4. Interface method candidates from method sets.
5. Conservative interface dispatch with explicit ambiguity.
6. Allocation-site points-to for interfaces and function values.
7. Goroutine/channel approximate flow.
8. Reflection and framework models.

## Polint Decision

For Go, the first strong version should be:

- intraprocedural flow;
- direct interprocedural summaries;
- method-set/interface-aware call graph;
- bounded access paths;
- explicit unknown/havoc for reflection, unsafe, cgo, unresolved calls, goroutines, and channels.

This gives useful repo-policy checks without requiring full Go compiler-equivalent semantics on day one.

