# Go Framework Entrypoint Research

## Core Recommendation

Use two layers:

```text
Go framework facts
  -> optional synthetic dispatch overlay
```

Do not bake routes directly into the base call graph. Emit `GoRoute`, `GoEntrypoint`, `GoMiddlewareEdge`, `GoRequestSource`, `GoResponseSink`, `GoContextFlow`, and `GoFrameworkDispatchEdge` facts. Reachability/data-flow views can opt into the overlay.

## Fact Families

| Fact | Meaning |
|---|---|
| `GoEntrypoint` | HTTP handler, middleware, framework handler, CLI command, test, job. |
| `GoRoute` | Framework, router value, methods, path pattern, params, constraints, handler, middleware chain, order. |
| `GoMiddlewareEdge` | Middleware callable, next placeholder, calls-next, may-abort, pre/post regions. |
| `GoRequestSource` | Path/query/form/body/header/cookie/client IP/context/args/env/stdin sources. |
| `GoResponseSink` | Body/header/status/cookie/redirect/file/path writes. |
| `GoContextFlow` | `context.WithValue/Value`, gin/echo context storage, chi/mux route context. |
| `GoFrameworkDispatchEdge` | Synthetic router/listener/middleware to handler edge with route constraints. |

## net/http

Recognize:

- `http.Handle`
- `http.HandleFunc`
- `(*http.ServeMux).Handle`
- `(*http.ServeMux).HandleFunc`
- `http.ListenAndServe`
- `http.Server{Handler: ...}`
- `ServeHTTP(http.ResponseWriter, *http.Request)` implementers
- Go 1.22+ `Request.PathValue`

Pseudo-code:

```python
def recover_net_http(calls, types):
    for call in calls:
        if call.target in ["net/http.Handle", "net/http.HandleFunc"]:
            emit_route(default_mux(), call.arg(0), call.arg(1))

        if call.method in ["Handle", "HandleFunc"] and receiver_type(call) == "net/http.ServeMux":
            emit_route(call.receiver, call.arg(0), call.arg(1))

        if call.target == "net/http.ListenAndServe":
            handler = call.arg(1)
            if handler is nil:
                emit_dispatch(default_mux())
            else:
                emit_dispatch(handler)

    for typ in types:
        if implements_serve_http(typ):
            emit_entrypoint(kind="http_handler", target=typ.method("ServeHTTP"))
```

## chi

Recognize:

- `chi.NewRouter`
- `Use`
- `With`
- `Group`
- `Route`
- `Mount`
- `Handle`
- `HandleFunc`
- `Method`
- HTTP verb methods
- `chi.URLParam`

Important semantics:

- `Use` mutates router middleware before route registration.
- `With` creates inline middleware for following route registration.
- `Group` passes a router to a callback.
- `Route` creates a subrouter under a prefix.
- `Mount` dispatches to another handler/router under a wildcard.

Pseudo-code:

```python
def recover_chi(function):
    env = RouterEnv()

    for stmt in function.statements:
        if assigns(stmt, call("chi.NewRouter")):
            env.bind_router(stmt.lhs)

        if method_call(stmt, "Use"):
            env.router(stmt.recv).add_global_middleware(stmt.args)

        if method_call(stmt, "With"):
            env.bind_inline_router(stmt.result, stmt.recv, stmt.args)

        if method_call(stmt, "Route"):
            sub = env.create_subrouter(prefix=literal(stmt.arg(0)))
            analyze_callback(stmt.arg(1), sub)

        if method_call(stmt, "Mount"):
            emit_mount(stmt.recv, literal(stmt.arg(0)), stmt.arg(1))

        if method_call(stmt, CHI_ROUTE_METHODS):
            emit_route(stmt.recv, method_from_call(stmt), stmt.arg(0), stmt.arg(1))
```

## gorilla/mux

Recognize:

- `mux.NewRouter`
- `Router.Handle`
- `Router.HandleFunc`
- `Router.Methods`
- `Router.Path`
- `Router.PathPrefix`
- `Route.Handler`
- `Route.HandlerFunc`
- `Route.Methods`
- `Route.Path`
- `Route.PathPrefix`
- `Route.Subrouter`
- `Router.Use`
- `mux.Vars`

Important semantics:

- Routes are ordered.
- Constraints are added through fluent builder calls.
- Subrouters inherit route matchers/prefixes.
- `mux.Vars(r)` is the path parameter source.

Pseudo-code:

```python
def recover_mux(calls):
    builders = {}

    for call in calls:
        if call.target == "mux.NewRouter":
            bind_router(call.result)

        if call.method in ["Handle", "HandleFunc", "Path", "PathPrefix", "Methods"]:
            route = route_builder_for(call)
            builders[call.result] = update_constraints(route, call)

        if call.method in ["Handler", "HandlerFunc"]:
            route = builders[call.receiver]
            route.handler = resolve_handler(call.arg(0))
            emit_route(route)

        if call.method == "Subrouter":
            bind_subrouter(call.result, builders[call.receiver])
```

## gin

Recognize:

- `gin.New`
- `gin.Default`
- `Engine.Use`
- `RouterGroup.Group`
- `RouterGroup.Use`
- verb methods
- `Any`
- `Match`
- `Static`, `StaticFS`, `StaticFile`
- `Context.Param`, `Query`, `PostForm`, `GetHeader`, `Bind*`, `ShouldBind*`, `JSON`, `String`

Important semantics:

- Handlers are variadic chains.
- Gin engine drives the chain with `c.Next`.
- `Abort` stops remaining handlers.
- Middleware does not have to explicitly call next to continue.

Pseudo-code:

```python
def recover_gin(calls):
    for call in calls:
        if call.target in ["gin.New", "gin.Default"]:
            bind_group(call.result, prefix="", middleware=[])

        if call.method == "Group":
            parent = group(call.receiver)
            bind_group(call.result, prefix=parent.prefix + literal(call.arg(0)),
                       middleware=parent.middleware + handler_args_after_path(call))

        if call.method == "Use":
            group(call.receiver).middleware.extend(call.args)

        if call.method in GIN_ROUTE_METHODS:
            route_handlers = call.args[1:]
            emit_route(call.receiver, method_from_call(call), call.arg(0), route_handlers)
```

## echo

Recognize:

- `echo.New`
- `Echo.Pre`
- `Echo.Use`
- `Echo.Group`
- route verbs
- `Any`, `Match`, `Add`, `AddRoute`, `RouteNotFound`
- `WrapHandler`
- `WrapMiddleware`
- `Context.Param`, `QueryParam`, `FormValue`, `Bind`, `JSON`, `String`, `Redirect`, `File`

Important semantics:

- `Pre` middleware runs before routing.
- `Use` middleware runs after routing.
- Route/group middleware wraps handlers.
- Middleware must call `next(c)` to continue.
- Binding order includes path params, query for GET/DELETE/HEAD, then body.

Pseudo-code:

```python
def recover_echo(calls):
    for call in calls:
        if call.target == "echo.New":
            bind_echo(call.result)

        if call.method == "Group":
            parent = component(call.receiver)
            bind_group(call.result, prefix=parent.prefix + literal(call.arg(0)),
                       middleware=parent.middleware + call.args[1:])

        if call.method in ECHO_ROUTE_METHODS:
            emit_route(
                component=call.receiver,
                method=method_from_call(call),
                path=call.arg(0),
                handler=call.arg(1),
                middleware=call.args[2:],
            )
```

## Precision Tiers

| Tier | Meaning |
|---|---|
| Tier 0 | Table-driven source/sink/summary facts by package/type/method. |
| Tier 1 | Intra-function router variable tracking and literal handler registration. |
| Tier 2 | Interprocedural builder summaries for helpers returning routers/handlers. |
| Tier 3 | Middleware control semantics, context store flows, dispatch overlay. |
| Tier 4 | Dynamic route strings, custom matchers, interface routers, custom routers, job frameworks. |

## Benchmark Fixtures

Required:

- stdlib default mux and explicit mux.
- `Server{Handler:nil}`.
- Go 1.22 path patterns and `PathValue`.
- chi `Use/With/Group/Route/Mount/URLParam`.
- mux fluent chains and `Subrouter`.
- gin nested groups, `Abort`, `Next`, binding.
- echo pre/use/group, wrap handlers, binding, context set/get.

CLI:

Start generic:

- `main`
- `os.Args`
- environment variables
- stdin and scanners
- test entrypoints

Add Cobra later:

- `Command{Run, RunE, PreRunE, PersistentPreRunE}`
- `AddCommand`
- `Execute`
- flag sources
