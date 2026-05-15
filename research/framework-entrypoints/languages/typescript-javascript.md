# TypeScript / JavaScript Framework Entrypoint Research

## Core Recommendation

Build layered facts, not a full JS points-to engine first.

```text
package/manifests
  -> resolved imports
  -> framework component instances
  -> route/tool/middleware registrations
  -> entrypoints/trust boundaries
  -> optional dispatch overlay
```

Use Oxc parsing/resolution and shallow access-path tracking. Add deeper points-to/callback graph only after the fact layer and validation exist.

## Fact Families

| Fact | Purpose |
|---|---|
| `PackageEntrypoint` | package `main`, `exports`, `bin`, scripts, workspaces, module type. |
| `JsFrameworkComponent` | Express app/router, Fastify server/plugin, Nest controller/module, MCP server. |
| `HttpRoute` | method, path, handler, registration, middleware, precision. |
| `HttpMiddleware` | middleware registration and order. |
| `McpEntrypoint` | tool/resource/prompt/request handler boundary. |
| `RequestInput` | request args/query/body/header/cookie/path/MCP arguments. |
| `ResponseEffect` | send/json/render/redirect/header/cookie/MCP result. |
| `FrameworkDispatch` | synthetic framework/protocol dispatch edge. |

## Package And Module Resolution

Use manifest and resolver facts because entrypoint identity depends on:

- `package.json` `type`;
- `main`;
- `exports`;
- conditional exports;
- `imports`;
- `bin`;
- workspace roots;
- TS `node16`, `nodenext`, and `bundler` resolution modes;
- CJS vs ESM imports.

Do not trust local names like `express` or `router` unless import identity or extension facts bind them.

## Express

Recognize:

- `express()`
- `express.Router()`
- `app.use`
- `router.use`
- `app.METHOD`, `router.METHOD`
- `all`
- `route(path).get/post/...`
- handler arrays
- mounted routers/apps
- `next("route")`

Pseudo-code:

```python
def recover_express(module):
    env = JsEnv()

    for stmt in module.statements:
        if assigns(stmt, call(imported("express"))):
            env.bind_component(stmt.lhs, kind="express_app")

        if assigns(stmt, call(member(imported("express"), "Router"))):
            env.bind_component(stmt.lhs, kind="express_router")

        if method_call(stmt, "use") and env.is_component(stmt.recv):
            path, handlers = parse_optional_path_then_handlers(stmt.args)
            emit_middleware_or_mount(stmt.recv, path, handlers)

        if method_call(stmt, EXPRESS_METHODS) and env.is_component(stmt.recv):
            path = literal_or_unknown(stmt.arg(0))
            handlers = flatten_handlers(stmt.args[1:])
            emit_route(stmt.recv, method_from_call(stmt), path, handlers)

        if method_call(stmt, "route") and env.is_component(stmt.recv):
            bind_route_builder(stmt.result, stmt.recv, stmt.arg(0))
```

Limits:

- path globs and regex need conservative metadata;
- mounted router prefixes require inter-file composition;
- dynamic handler arrays need unknown facts;
- wrappers need extension providers.

## Fastify

Recognize:

- `fastify()`
- `server.route({ method, url, handler })`
- verb shorthands `get`, `post`, etc.
- `addHook`
- `register(plugin, { prefix })`
- async return vs `reply.send`
- request/reply sources and effects

Pseudo-code:

```python
def recover_fastify(module):
    for call in module.calls:
        if call.target == imported("fastify"):
            bind_component(call.result, kind="fastify_server")

        if method_call(call, "route") and is_fastify(call.recv):
            opts = object_literal(call.arg(0))
            emit_route(call.recv, opts["method"], opts["url"], opts["handler"])

        if method_call(call, FASTIFY_METHODS) and is_fastify(call.recv):
            path, opts, handler = parse_fastify_shorthand(call.args)
            emit_route(call.recv, method_from_call(call), path, handler, hooks=opts.hooks)

        if method_call(call, "register") and is_fastify(call.recv):
            emit_plugin_registration(call.recv, call.arg(0), prefix=option(call.arg(1), "prefix"))
```

Limits:

- plugin scopes and encapsulation are nontrivial;
- hooks have lifecycle-specific ordering;
- dynamic plugin registration should be unresolved or conservative.

## NestJS

Recognize where Oxc can recover metadata:

- `@Controller`
- method decorators: `@Get`, `@Post`, `@Put`, `@Delete`, `@Patch`, `@All`, `@RequestMapping`
- parameter decorators: `@Param`, `@Query`, `@Headers`, `@Body`, `@UploadedFile`, `@UploadedFiles`
- `@UseGuards`, `@UseInterceptors`, `@UsePipes`
- `@Module`
- `@Injectable`
- microservice `@MessagePattern`, `@EventPattern`

Pseudo-code:

```python
def recover_nest(classes):
    controllers = []

    for cls in classes:
        ctrl = decorator(cls, "Controller")
        if not ctrl:
            continue

        prefix = decorator_path(ctrl)
        controllers.append(cls)

        for method in cls.methods:
            route_dec = first_route_decorator(method)
            if not route_dec:
                continue

            route = compose(prefix, decorator_path(route_dec))
            inputs = []
            for param in method.params:
                dec = request_param_decorator(param)
                if dec:
                    inputs.append(source_from_decorator(dec, param))

            emit_route(method_from_decorator(route_dec), route, method.symbol, inputs)
```

Limits:

- DI resolution and custom decorators need deeper semantic support.
- Decorator metadata must be tied to import identity, not just decorator names.
- Custom parameter decorators should become extension facts.

## MCP TypeScript SDK

Recognize:

- `server.registerTool(name, config, handler)`
- `server.tool(name, schema/config, handler)`
- `server.registerResource`
- `server.registerPrompt`
- `server.setRequestHandler("tools/call", handler)`
- schema-based arguments and protocol-visible returns

Pseudo-code:

```python
def recover_mcp_ts(module):
    for call in module.calls:
        if method_call(call, ["registerTool", "tool"]) and is_mcp_server(call.recv):
            emit_mcp_tool(
                name=literal_or_unknown(call.arg(0)),
                handler=last_function_arg(call),
                request_source="tool arguments",
                return_boundary="CallToolResult",
            )

        if method_call(call, "setRequestHandler") and is_mcp_server_or_protocol(call.recv):
            schema = request_schema(call.arg(0))
            if schema == "CallToolRequestSchema" or literal(call.arg(0)) == "tools/call":
                emit_protocol_dispatch(call.arg(1), protocol="tools/call")
```

MCP is especially important for polint because AI agents consume MCP outputs and invoke MCP tools. Return-side trust boundaries matter.

## Precision Tiers

| Tier | Meaning |
|---|---|
| Tier 0 | File conventions and syntactic registrations. |
| Tier 1 | Resolved import identity plus local aliases/chaining. |
| Tier 2 | Shallow inter-file access paths, router prefix composition, Fastify plugins, Nest decorators. |
| Tier 3 | Bounded points-to/callback/event graph and data flow. |

## Fixtures

Required:

- Express mounted router in another file.
- Express arrays of handlers and `app.route`.
- Express dynamic path fallback.
- Fastify route object and shorthand.
- Fastify nested `register({ prefix })`.
- Nest controller prefix + method route + param decorators.
- MCP `registerTool` and protocol-level dispatch.
- CJS and ESM import variants.
