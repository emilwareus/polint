# Python Call Graphs

## Best OSS References

- `repos/pycg`: research-grade Python call graph baseline.
- `repos/jarvis`: newer application-centered Python call graph research.
- `repos/pyan`: practical AST/symtable/import graph implementation.
- `repos/codeql/python/ql/lib`: mature production dataflow/type-tracking implementation.
- `repos/pyre-check`: typed Python/Pysa call graph data model and builder.

## Python Is Heuristic

Static Python call graph construction is inherently approximate because of:

- dynamic imports and import-time side effects;
- rebinding and monkey patching;
- decorators and factories;
- descriptors, properties, metaclasses, `__getattr__`, and `__getattribute__`;
- higher-order functions and callbacks;
- `eval`, `exec`, reflection, and generated modules;
- protocol calls like `__iter__`, `__next__`, `__enter__`, `__exit__`, `__call__`.

Any polint Python call graph must say it is heuristic.

## PyCG Pattern

Sources:

- `repos/pycg/pycg/pycg.py`
- `repos/pycg/pycg/processing/cgprocessor.py`

PyCG runs a pre-pass, then a fixed-point post-pass, then emits call edges.

```python
def pycg_style(entrypoints):
    managers = create_managers_for_imports_scopes_defs_classes_modules()

    preprocessor_scan(entrypoints, managers)
    complete_definitions(managers)

    previous_state = None
    while previous_state != extract_state(managers):
        previous_state = extract_state(managers)
        reset_scope_counters(managers)
        postprocessor_scan(entrypoints, managers)
        complete_definitions(managers)

    call_graph = CallGraph()
    cgprocessor_scan(entrypoints, managers, call_graph)
    return call_graph
```

Call resolution:

```python
def pycg_visit_call(call, current_function):
    visit_args(call.args)
    visit(call.func)

    names = retrieve_call_names(call)
    if not names:
        if external_attribute(call.func):
            emit_external_edge(current_function, full_external_name(call.func))
        elif builtin_name(call.func):
            emit_external_edge(current_function, "builtin." + call.func.id)
        else:
            emit_unresolved(call, reason="unknown_python_call")
        return

    for pointer in names:
        definition = definitions.get(pointer)
        if definition.is_function:
            emit_edge(current_function, definition)
        elif definition.is_class:
            for init in find_class_function(definition, "__init__"):
                emit_edge(current_function, init)
        elif definition.is_external:
            emit_external_edge(current_function, definition)
```

Good baseline for assignment/name/return propagation.

## JARVIS Pattern

Sources:

- `repos/jarvis/Jarvis/tool/Jarvis/jarvis.py`
- `repos/jarvis/Jarvis/tool/Jarvis/processing/extProcessor.py`

JARVIS stores AST nodes and traverses reachable functions from configured or inferred module entries.

```python
def jarvis_style(entrypoints):
    parse_modules_and_create_defs()
    module_entries = discover_module_entry_functions()

    for entry in module_entries:
        analyze_local_function(entry)

def push_stack(definition):
    if definition in call_stack:
        maybe_emit_recursive_edge(definition)
        return

    emit_edge(current_namespace, definition)
    call_stack.push(definition)
    node = node_manager.get(definition.namespace)
    visit(node)
    call_stack.pop()
```

The interesting part is row-sensitive resolution and recursive traversal from application entries.

## Pyan Pattern

Source: `repos/pyan/pyan/analyzer.py`

Pyan is not a precise call graph, but it is a strong source for Python syntax, `symtable`, imports, and MRO handling.

```python
def pyan_style(files):
    root = infer_root(files)
    modules = map_files_to_modules(files, root)

    for module in modules:
        symtable_data = symtable(module.source)
        prescan_definitions(module.ast, symtable_data)

    compute_mro_for_classes()

    for module in modules:
        visit_ast(module.ast)
        record_defines_edges()
        record_uses_edges_for_calls_attrs_names()

    resolve_import_aliases()
    expand_unknowns()
    cull_inherited_noise()
```

Polint should copy the scope/import engineering, not treat Pyan as exact.

## CodeQL Python Pattern

Source: `repos/codeql/python/ql/lib/semmle/python/dataflow/new/internal/DataFlowDispatch.qll`

CodeQL models explicit call types:

- plain function;
- normal method;
- static method;
- class method;
- method as plain function;
- class construction;
- instance `__call__`.

```python
def codeql_python_style(call):
    if function_tracker_flows_to(call.function):
        emit(call, tracked_function, kind="plain_function")

    if attr_read_flows_to(call.function):
        for cls in possible_classes(attr_read.receiver):
            target = mro_lookup(cls, attr_read.name)
            emit(call, target, kind=method_kind(target))

    if class_tracker_flows_to(call.function):
        for target in [mro_lookup(cls, "__new__"), mro_lookup(cls, "__init__")]:
            emit(call, target, kind="class_construction")

    if class_instance_tracker_flows_to(call.function):
        emit(call, mro_lookup(cls, "__call__"), kind="instance_call")

    if no_target:
        emit_unresolved(call, reason="dynamic_python_callable")
```

This is the best target model for a high-quality future Python provider.

## Pyre/Pysa Pattern

Sources:

- `repos/pyre-check/source/interprocedural/callGraph.ml`
- `repos/pyre-check/source/interprocedural/callGraphBuilder.ml`

Pyre/Pysa has a strong data model:

- normal call targets;
- `__new__` targets;
- `__init__` targets;
- decorated targets;
- higher-order parameters;
- shim targets;
- unresolved reason;
- recognized-call markers.

```python
def pysa_style(call, type_environment, override_graph):
    callee_type = type_of(call.callee)
    callee_kind = classify_callee(callee_type, call.callee)

    targets = resolve_targets_from_type(callee_type, callee_kind, override_graph)
    constructor_targets = resolve_constructor_targets(call, callee_type)
    higher_order = find_callable_arguments(call.args, type_environment)

    if not targets and not constructor_targets:
        return CallCallees.unresolved(reason="unknown_type_or_dynamic")

    return CallCallees(
        call_targets=targets,
        new_targets=constructor_targets.new,
        init_targets=constructor_targets.init,
        higher_order_parameters=higher_order,
        unresolved=False,
    )
```

The implementation notes it is tuned for taint analysis, but the data shape is excellent.

## Polint Recommendation For Python

Python should come after the existing Go and TS/JS adapters mature. When added:

1. Start with Pyan-style AST + `symtable` extraction:
   - modules;
   - imports;
   - definitions;
   - classes;
   - inheritance;
   - scopes;
   - call sites.
2. Add PyCG-style fixed-point points-to:
   - name points-to;
   - return points-to;
   - assignment edges;
   - argument binding;
   - constructor edges.
3. Add MRO-based dispatch for:
   - `self`;
   - `cls`;
   - `super`;
   - `__new__`;
   - `__init__`;
   - `__call__`;
   - common protocols.
4. Model uncertainty explicitly:
   - `dynamic_import`;
   - `unknown_attribute`;
   - `decorator_unknown`;
   - `higher_order_unknown`;
   - `eval_or_exec`;
   - `metaclass_or_descriptor_unknown`.

