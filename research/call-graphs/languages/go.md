# Go Call Graphs

## Best OSS References

- `repos/golang-tools`: canonical Go implementation.
- `repos/go-callvis`: practical visualizer built on `golang.org/x/tools`.
- `repos/codeql/go/ql/lib`: production query/dataflow model for Go call nodes.

## Practical Pipeline

Go call graph construction in the inspected tooling is:

```python
def load_go_ssa(module_dir, patterns, build_tags, include_tests):
    cfg = PackagesConfig(
        mode="LoadAllSyntax",
        dir=module_dir,
        tests=include_tests,
        build_flags=tags_to_build_flags(build_tags),
    )
    initial = packages_load(cfg, patterns)
    if has_errors(initial):
        return setup_missing("go/packages load failed")

    prog, pkgs = ssautil_all_packages(initial, instantiate_generics=True)
    prog.build()
    return prog, pkgs
```

This matches `go-callvis/analysis.go`: `packages.Load`, `ssautil.AllPackages`, `prog.Build`, then `static`, `cha`, `rta`, or `vta`.

## Algorithms In x/tools

### Static

Source: `repos/golang-tools/go/callgraph/static/static.go`

Static call graph construction follows only `ssa.CallCommon.StaticCallee()`.

```python
def static_go_edges(program):
    for fn in all_functions(program):
        for instr in fn.instructions:
            if instr.is_call():
                callee = instr.common.static_callee()
                if callee:
                    emit_edge(fn, instr, callee, algorithm="static", confidence="exact")
                else:
                    emit_unresolved(fn, instr, reason="dynamic_go_call")
```

Good for direct calls. It misses interface dispatch and function values.

### CHA

Source: `repos/golang-tools/go/callgraph/cha/cha.go`

CHA resolves dynamic calls by assuming all address-taken functions and all concrete types may be reachable.

```python
def go_cha(program):
    funcs = all_functions(program)
    lazy_callees = build_cha_resolver(funcs)

    for fn in funcs:
        for call in call_sites(fn):
            if static_target(call):
                emit_edge(fn, call, static_target(call), "cha-static")
            else:
                for target in lazy_callees(call):
                    emit_edge(fn, call, target, "cha-possible")
```

Good for partial programs and libraries. Imprecise for interfaces.

### RTA

Source: `repos/golang-tools/go/callgraph/rta/rta.go`

RTA starts from roots, tracks reachable functions, address-taken functions, runtime concrete types, dynamic function calls, and interface invoke sites until fixpoint.

```python
def go_rta(roots):
    reachable = set()
    runtime_types = set()
    addr_taken_by_signature = defaultdict(set)
    dynamic_calls = []
    invoke_sites = []
    work = list(roots)

    while work:
        fn = work.pop()
        if fn in reachable:
            continue
        reachable.add(fn)

        for event in scan_function(fn):
            if event.kind == "static_call":
                add_reachable(event.target, work)
            elif event.kind == "make_interface_or_alloc":
                if add(runtime_types, event.type):
                    revisit(invoke_sites, work)
            elif event.kind == "function_value":
                addr_taken_by_signature[event.signature].add(event.function)
                revisit(dynamic_calls, work)
            elif event.kind == "dynamic_func_call":
                dynamic_calls.append(event.call)
                for target in addr_taken_by_signature[event.signature]:
                    add_edge(fn, event.call, target)
                    add_reachable(target, work)
            elif event.kind == "interface_call":
                invoke_sites.append(event.call)
                for typ in runtime_types:
                    target = method_for(typ, event.interface_method)
                    add_edge(fn, event.call, target)
                    add_reachable(target, work)

    return graph
```

Best for configured binaries/tests, not arbitrary libraries.

### VTA

Source: `repos/golang-tools/go/callgraph/vta/vta.go`, `vta/propagation.go`

VTA builds a type propagation graph, reduces SCCs, propagates type/function tokens, and resolves dynamic calls by intersecting propagated candidates with an initial graph.

```python
def go_vta(functions, initial_graph=None):
    if initial_graph is None:
        initial_graph = go_cha(functions.program)

    tpg = build_type_propagation_graph(functions)
    scc_graph = collapse_sccs(tpg)
    propagated = propagate_tokens_reverse_topological(scc_graph)

    for call in unresolved_calls(functions):
        possible = initial_graph.targets(call)
        from_flow = propagated[callee_or_receiver_node(call)]
        for target in intersect(possible, from_flow):
            emit_edge(call.enclosing, call, target, algorithm="vta")
```

This is the most precise x/tools option but is marked experimental.

## CodeQL Go Model

CodeQL separates syntax from resolution:

- `CallExpr` is syntax.
- `DataFlow::CallNode` exposes target queries.
- `getACalleeWithoutVirtualDispatch()` separates direct/function-value targets from virtual dispatch.
- `getACalleeIncludingExternals()` includes function values and interface dispatch.

Polint should copy the separation, not the QL implementation.

## Polint Recommendation For Go

1. Keep the current tree-sitter Go parser for cheap call-site facts.
2. Add optional `go/packages` + `go/ssa` setup for semantic call graph facts.
3. Implement provider modes:
   - `go.syntax`: call sites only.
   - `go.ssa.static`: high-confidence direct SSA calls.
   - `go.ssa.cha`: library/partial mode.
   - `go.ssa.rta`: configured main/test roots.
   - `go.ssa.vta`: experimental higher precision.
4. Reuse existing Go lifecycle config from the project instructions: module roots, package patterns, build tags, include tests.
5. Emit capability diagnostics for package loading failures and missing roots.

