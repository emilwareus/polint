# Python Data-Flow Notes

## Summary

Python data flow should be honest, bounded, and model-driven. The language is dynamic enough that exact whole-program flow is not realistic without strong assumptions. The best practical systems combine import/name binding, local flow, function summaries, type annotations as hints, MRO/class modeling, and explicit unknowns for dynamic behavior.

Pysa is the most useful production reference: summaries for sources, sinks, and TITO are iterated over the call dependency graph to a fixed point.

## OSS References

- Pysa: `repos/pyre-check/source/interprocedural_analyses/taint/*`.
- CodeQL Python: `repos/codeql/python/ql/lib/semmle/python/dataflow/new/*`.
- Doop Python analysis: `repos/doop/souffle-logic/python/*`.

## Required Facts

```python
PythonFacts(
    modules,
    imports,
    relative_imports,
    package_inits,
    functions,
    classes,
    methods,
    decorators,
    annotations,
    scopes,
    globals,
    nonlocals,
    comprehensions,
    assignments,
    attributes,
    subscripts,
    calls,
    returns,
    yields,
    async_await,
    exceptions,
    context_managers,
    class_mro,
    dynamic_feature_markers,
)
```

## Local Flow

```python
for stmt in function.body:
    match stmt:
        case Assign(lhs, rhs):
            edge(value(rhs), place(lhs), "assignment")

        case AttributeAssign(obj, attr, value):
            edge(value, place(obj, "." + attr), "attribute_write")

        case AttributeRead(obj, attr):
            edge(place(obj, "." + attr), result(stmt), "attribute_read")

        case Return(value):
            edge(value, return_value(function), "return")

        case Yield(value):
            edge(value, yield_value(function), "yield")
```

## Summary Model

Pysa-style summaries:

```python
Summary(
    sources_returned,
    sinks_reached_by_param,
    tito_param_to_return,
    tito_param_to_param,
    attribute_writes,
    unknown_callees,
)
```

Fixed point:

```python
while changed:
    for function in reverse_call_order:
        new_summary = analyze(function, summaries_of_callees)
        changed |= update(summary[function], new_summary)
```

## Attribute And Object Flow

Use bounded attributes:

```python
Place(local("request"), [".GET", "[*]"])
Place(local("user"), [".profile", ".email"])
```

Treat unknown attribute writes as object havoc:

```python
setattr(obj, dynamic_name, value) -> edge(value, place(obj, ".*"), "unknown_attribute_write")
getattr(obj, dynamic_name)        -> edge(place(obj, ".*"), result, "unknown_attribute_read")
```

Literal `getattr(obj, "name")` can resolve to `.name`.

## Calls

```python
direct local/imported function     -> high precision
class constructor                  -> class __init__ + instance value
bound method with inferred class   -> medium/high precision
MRO-known method                   -> medium precision
decorated function                 -> decorated_unknown unless modeled
dynamic callable                   -> unknown_call
```

## Dynamic Features

Emit explicit unknowns:

```python
eval
exec
getattr nonliteral
setattr nonliteral
__getattr__
__getattribute__
importlib
monkeypatching
decorators
metaclasses
dynamic class creation
```

Unknown should be contagious but visible:

```python
if value.precision == "unknown":
    path.precision = join_precision(path.precision, "unknown")
```

## Framework Models

Useful first models:

- Flask route params and request object.
- Django views, request, ORM persistence, template rendering.
- FastAPI route params and Pydantic models.
- Click/Typer CLI input.
- `subprocess`, `os.system`, SQL libraries, templating sinks.

## Recommended Python Milestones

1. AST local flow.
2. module/import/name binding.
3. direct calls and summaries.
4. attribute access paths.
5. class and MRO basics.
6. annotations as hints.
7. common framework source/sink/sanitizer models.
8. decorators/metaclasses as modeled unknowns.

## Polint Decision

Python should follow Go and TS/JS. It can become very useful, but only if unsupported dynamic behavior is visible in the SDK and diagnostics.

