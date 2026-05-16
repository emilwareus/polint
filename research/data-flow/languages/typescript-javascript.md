# TypeScript / JavaScript Data-Flow Notes

## Summary

TypeScript and JavaScript need complete syntactic coverage, honest dynamic-feature handling, and bounded value/object flow. Exact whole-program flow is not realistic in general because of dynamic property access, prototype mutation, `eval`, decorators, dependency injection, framework callbacks, and runtime module loading.

The right product promise is complete call-site and local-flow coverage, progressively resolved interprocedural flow, and visible unknowns.

## OSS References

- CodeQL JS/TS: `repos/codeql/javascript/ql/lib/semmle/javascript/dataflow/*`.
- Semgrep: `repos/semgrep/src/tainting/*`, `repos/semgrep/src/il/IL.ml`.
- OpenGrep: `repos/opengrep/src/tainting/*`, `repos/opengrep/src/analyzing/Dataflow_core.ml`.
- TypeScript compiler control flow: `repos/TypeScript/src/compiler/types.ts`, `binder.ts`, `checker.ts`.
- YASA-UAST: `repos/YASA-UAST/*`.

## Required Facts

```python
TsJsFacts(
    files,
    modules,
    imports,
    exports,
    commonjs_require,
    dynamic_imports,
    scopes,
    bindings,
    references,
    declarations,
    functions,
    arrows,
    classes,
    methods,
    object_literals,
    property_reads,
    property_writes,
    computed_properties,
    calls,
    new_expressions,
    callbacks,
    closures,
    captures,
    async_await,
    promise_chains,
    jsx,
    decorators,
    eval_markers,
)
```

## Local Flow

Model assignments, destructuring, object fields, arrays, returns, and calls.

```python
for stmt in function.body:
    match stmt:
        case "const x = y":
            edge(value(y), place("x"), "assignment")

        case "obj.k = v":
            edge(value(v), place("obj", ".k"), "property_write")

        case "x = obj.k":
            edge(place("obj", ".k"), place("x"), "property_read")

        case "return x":
            edge(value(x), return_value(function), "return")

        case "const f = () => body":
            edge(function_token(f), place("f"), "function_value")
```

## Object Shapes

Use shape-like cells similar to Semgrep's taint shapes.

```python
Shape(
    direct_taint,
    fields = {
        ".name": Shape(...),
        ".email": Shape(...),
        "[*]": Shape(...),
    },
    unknown_fields,
)
```

Computed property policy:

```python
obj["literal"] = v  -> field ".literal"
obj[k] = v          -> field "[*]" and mark unknown_property_write
```

## Function-Token Flow

This is needed for callbacks and higher-order calls.

```python
tokens = defaultdict(set)

for function_declaration in functions:
    tokens[name(function_declaration)].add(FunctionToken(function_declaration))

for assignment in assignments:
    add_constraint(tokens[assignment.rhs] <= tokens[assignment.lhs])

for call in calls:
    for token in tokens[call.callee]:
        emit_call_edge(call, token.function, "value_flow")
```

## Framework And Library Models

Add small, versioned models:

```python
Array.map(callback):
    edge(array_element(receiver), callback.param0, "callback_arg")
    edge(callback.return_value, array_element(result), "callback_return")

Promise.then(callback):
    edge(promise_value(receiver), callback.param0, "promise_then_arg")
    edge(callback.return_value, promise_value(result), "promise_then_return")

express.get(path, handler):
    mark_entrypoint(handler)
    mark_source(handler.param0, "http_request")
```

## Dynamic Features

Emit explicit unknowns:

```python
eval(code)                 -> unknown_code_execution
Function(code)             -> unknown_code_execution
obj[dynamic]               -> unknown_property
Proxy                      -> proxy_havoc
decorator                  -> decorator_havoc unless modeled
dynamic import(path)       -> unknown_module when path nonliteral
require(path)              -> unknown_module when path nonliteral
prototype mutation         -> prototype_havoc
```

## TypeScript Type Hints

Without the TS compiler, annotations should be treated as hints:

```python
if receiver_has_annotation("UserService"):
    candidate_precision = "type_hint"
else:
    candidate_precision = "unknown"
```

Do not label TS hints as exact semantic type information unless a native type-checker-quality model exists.

## Recommended TS/JS Milestones

1. Oxc AST local call/data-flow extraction.
2. Lexical scope and binding facts.
3. ES module and CommonJS direct import/export flow.
4. Intraprocedural assignments, destructuring, returns, object fields, arrays.
5. Function-token flow for callbacks.
6. Direct interprocedural summaries.
7. Framework models for Express, React handlers, test runners, and common Node APIs.
8. Bounded object-shape refinement.
9. Optional type-hint enrichment.

## Polint Decision

For JS/TS, the engine should prioritize useful, explainable, bounded analysis:

- local value/object flow;
- direct module-linked interprocedural summaries;
- function-token/callback propagation;
- explicit dynamic unknowns;
- rule-configurable framework models.

Do not promise exact whole-program JS/TS data flow.

