# Python Framework Entrypoint Research

Python is future-facing for polint until a Python adapter exists. The design still matters because Python frameworks stress decorators, runtime imports, URLconf recursion, ASGI/WSGI, tasks, CLI commands, tests, and runtime model generation.

## Core Recommendation

Build an entrypoint/lifecycle graph before whole-program Python call graph.

```text
imports/bindings/decorators
  -> framework objects
  -> route/task/cli/test registrations
  -> lifecycle/trust-boundary facts
  -> optional runtime-derived model tier
```

Do not execute application code by default.

## Parser Tiers

| Tier | Approach | Notes |
|---|---|---|
| Tier 0 | `tree-sitter-python` or Rust parser | Fast, error tolerant, static only. |
| Tier 1 | repo import resolution and package roots | Handles `src` layout and pyproject metadata. |
| Tier 2 | CPython `ast`/`symtable` sidecar or native equivalent | More exact binding/type info, but extra lifecycle complexity. |
| Tier 3 | opt-in runtime discovery | Django URLconf/FastAPI route tables/Celery app import. Risky and side-effect-prone. |

## Flask

Recognize:

- `Flask(...)`
- `Blueprint(...)`
- `@app.route`
- `@bp.route`
- `add_url_rule`
- `register_blueprint(..., url_prefix=...)`
- `MethodView.as_view`
- request hooks: `before_request`, `after_request`, `teardown_request`, `errorhandler`

Pseudo-code:

```python
def recover_flask(module):
    for stmt in module.statements:
        if assigns(stmt, call("Flask")):
            bind_component(stmt.lhs, kind="flask_app")

        if assigns(stmt, call("Blueprint")):
            bind_component(stmt.lhs, kind="flask_blueprint")

        if decorated_function(stmt, decorator_method("route")):
            component = resolve_decorator_receiver(stmt.decorator)
            emit_route(component, path=decorator_arg(0), handler=stmt.function)

        if method_call(stmt, "add_url_rule"):
            emit_route(stmt.recv, path=stmt.arg(0), handler=kwarg(stmt, "view_func"))
```

## Django

Recognize:

- `urlpatterns`
- `path`
- `re_path`
- `include`
- string view paths
- `View.as_view`
- middleware list from settings
- `ROOT_URLCONF`

Pseudo-code:

```python
def recover_django(project):
    roots = find_urlconf_roots(project)
    worklist = list(roots)

    while worklist:
        mod = worklist.pop()
        for item in parse_urlpatterns(mod):
            if item.is_include:
                worklist.append(resolve_include(item))
            else:
                handler = resolve_view(item.view)
                emit_route(framework="django", path=item.route, handler=handler)
```

Runtime import of URLconf can be precise, as Pysa demonstrates, but it can execute application code. Make it opt-in.

## FastAPI / Starlette

Recognize:

- `FastAPI(...)`
- `APIRouter(...)`
- decorators `@app.get`, `@router.post`, etc.
- `add_api_route`
- `include_router(..., prefix=..., dependencies=...)`
- `Depends`
- lifespan and deprecated `on_event`
- `BackgroundTasks`

Pseudo-code:

```python
def recover_fastapi(module):
    for call in module.calls:
        if call.target in ["FastAPI", "APIRouter"]:
            bind_component(call.result)

        if method_call(call, FASTAPI_ROUTE_METHODS):
            emit_route(call.recv, method_from_call(call), call.arg(0), decorated_function(call))

        if method_call(call, "include_router"):
            emit_router_mount(parent=call.recv, child=call.arg(0), prefix=kwarg(call, "prefix"))
```

## Jobs, CLI, Tests

Celery:

- `@app.task`
- `@shared_task`
- `beat_schedule`
- autodiscovery facts, marked unresolved if dynamic.

Click/Typer:

- `@click.command`
- `@click.group`
- `group.add_command`
- `console_scripts` entrypoints.

pytest/unittest:

- test functions/classes;
- fixtures and `conftest.py`;
- parametrization;
- unittest `TestCase` methods.

## Recommended Future Facts

- `PythonEntrypoints<'_>`
- `PythonRoutes<'_>`
- `PythonLifecycle<'_>`
- `PythonJobs<'_>`
- `PythonCli<'_>`
- `PythonTests<'_>`
- `PythonFixtures<'_>`
- `PythonModels<'_>`

## Limits

- dynamic imports;
- monkey patching;
- runtime-generated routes;
- decorator factories;
- framework plugins;
- settings-dependent Django behavior;
- side effects during runtime discovery.

Unknowns must be explicit.
