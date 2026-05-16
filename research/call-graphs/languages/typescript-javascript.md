# JavaScript And TypeScript Call Graphs

## Best OSS References

- `repos/jelly`: strongest current practical JS/TS reference.
- `repos/codeql/javascript/ql/lib`: production query/dataflow model.
- `repos/tajs`: abstract interpretation reference.
- `repos/wala/cast/js`: field-based JavaScript call graph reference.

## Why JS/TS Is Hard

Dynamic features make exact static call graphs impractical at scale:

- functions are first-class values;
- methods are property reads;
- property names may be computed;
- modules can be loaded dynamically;
- prototypes and globals can be mutated;
- `bind`, `call`, `apply`, decorators, proxies, `eval`, and framework registration change call behavior;
- TypeScript types are compile-time information, not runtime values.

The best practical output is an explicitly approximate graph with uncertainty.

## Direct Extraction

```python
def js_direct_calls(files):
    for call in ast_call_like_nodes(files):
        emit_call_site(call)

        if is_identifier(call.callee):
            target = resolve_lexical_or_import(call.callee)
            if target:
                emit_edge(call, target, algorithm="binding", confidence="high")
            else:
                emit_unresolved(call, reason="unknown_identifier")

        elif is_member_call(call.callee):
            prop = static_property_name(call.callee.property)
            if prop is None:
                emit_unresolved(call, reason="computed_property")
            else:
                emit_possible_member_call(call, receiver=call.callee.object, property=prop)

        elif is_new_expression(call):
            emit_constructor_call_site(call)

        else:
            emit_unresolved(call, reason="dynamic_callee")
```

This is the right first tier for polint using Oxc.

## Jelly Pattern

Sources:

- `repos/jelly/src/analysis/astvisitor.ts`
- `repos/jelly/src/analysis/operations.ts`
- `repos/jelly/src/analysis/fragmentstate.ts`

Jelly models calls as constraints over tokens:

- `FunctionToken` represents functions.
- `ObjectToken`, `NativeObjectToken`, `AccessPathToken`, and related tokens represent objects and external/dynamic values.
- `Operations.callFunction` registers the call site and creates constraints.
- `Operations.callFunctionTokenBound` registers call edges and binds arguments/results.
- `FragmentState.registerCallEdge` records both function-to-function and call-to-function edges.

Simplified:

```python
def jelly_style_call(call):
    caller = enclosing_function_or_module(call)
    callee_var = expression_var(call.callee)
    arg_vars = [expression_var(arg) for arg in call.args]
    result_var = expression_var(call)

    register_call_site(call, caller, callee_var)

    def on_callee_token(token):
        if token.is_function:
            register_call_edge(call, caller, token.function)
            bind_arguments(arg_vars, token.function.parameters)
            bind_return(token.function.return_var, result_var)

            if call.is_new:
                instance = new_object_token(token.function)
                add_token(instance, result_var)
                add_token(instance, this_var(token.function))

        elif token.is_native_require:
            load_module(call.first_string_arg, result_var)

        elif token.is_access_path:
            register_external_call(call, caller)
            mark_args_escaping(arg_vars)

    add_for_all_tokens_constraint(callee_var, on_callee_token)
```

This is the best JS/TS model to study for a future high-precision provider.

## CodeQL JS/TS Pattern

Sources:

- `repos/codeql/javascript/ql/lib/semmle/javascript/dataflow/Nodes.qll`
- `repos/codeql/javascript/ql/lib/semmle/javascript/dataflow/internal/CallGraphs.qll`

CodeQL exposes:

- `InvokeNode.getCalleeNode()`
- `InvokeNode.getACallee()`
- `InvokeNode.getACallee(int imprecision)`
- `InvokeNode.isImprecise()`
- `InvokeNode.isIncomplete()`
- `InvokeNode.isUncertain()`

Simplified:

```python
def codeql_js_style(call, dataflow):
    value = dataflow.abstract_value(call.callee_node)

    for fn in value.function_values:
        imprecision = imprecision_from_flow_source(fn)
        emit_edge(call, fn, algorithm="value_flow", confidence=confidence(imprecision))

    if value.has_non_function_from_global_flow:
        mark_imprecise(call, reason="global_flow")

    if value.has_incompleteness_not_global:
        mark_incomplete(call, reason=value.incompleteness)

    if ts_static_resolved_callee(call):
        emit_edge(call, ts_static_resolved_callee(call), algorithm="ts_static")
```

The key lesson is not QL syntax. It is explicit imprecision and incompleteness.

## TAJS Pattern

Sources:

- `repos/tajs/src/dk/brics/tajs/analysis/FunctionCalls.java`
- `repos/tajs/src/dk/brics/tajs/solver/CallGraph.java`

TAJS uses abstract interpretation:

```python
def abstract_interpret_call(state, call_node):
    function_value = state.read(call_node.function_register)
    this_value = state.read_this(call_node)
    args = [state.read(arg) for arg in call_node.args]

    for abstract_function in function_value.object_labels:
        callee_context = make_context(abstract_function, this_value, args)
        edge_state = transfer_call_state(state, abstract_function, args, this_value)
        call_graph.add_target(call_node, state.context, abstract_function.entry, callee_context, edge_state)
```

This is powerful but too heavy for a first polint JS/TS implementation.

## WALA JS Pattern

WALA JS field-based call graph construction builds a flow graph of functions, properties, and objects, then extracts call edges when function vertices reach call sites.

```python
def field_based_js(files):
    flow = FlowGraph()

    for stmt in js_statements(files):
        if stmt.assigns_function:
            flow.add(stmt.function_token, stmt.target_var)
        elif stmt.property_write:
            flow.add(stmt.value_var, property_node(stmt.property))
        elif stmt.property_read:
            flow.add(property_node(stmt.property), stmt.target_var)
        elif stmt.call:
            flow.add(stmt.callee_expr_var, call_callee_node(stmt.call))

    solve_flow(flow)

    for call in calls:
        for fn in reached_functions(call_callee_node(call)):
            emit_edge(call, fn, algorithm="field_based")
```

Useful as an algorithm reference, less attractive as a dependency.

## Polint Recommendation For JS/TS

1. Start with Oxc AST and `oxc_resolver`.
2. Emit `CallSite` facts for:
   - `f()`
   - `obj.m()`
   - `this.m()`
   - `super.m()`
   - `new C()`
   - dynamic `import()`
   - CommonJS `require()`
   - JSX component calls if React support matters.
3. Resolve only high-confidence local/import bindings first.
4. Add a small function-token propagation pass:
   - function declarations/expressions/arrow functions;
   - imports/exports;
   - local assignments;
   - object literal methods;
   - class/static methods;
   - simple callbacks;
   - simple `bind`, `call`, `apply`.
5. Mark dynamic property, proxy, eval, unknown require/import, broad prototype mutation, and framework reflection as unresolved or low-confidence.
6. Do not claim sound JS/TS call graph coverage.

