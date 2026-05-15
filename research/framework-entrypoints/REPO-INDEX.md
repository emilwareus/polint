# Repository Index

Third-party repositories were cloned under `research/framework-entrypoints/repos/`, which is gitignored. They are local research artifacts, not vendored dependencies.

## Framework And Tool Implementations

| Repository | Commit | Why It Was Studied | Key Local Evidence |
|---|---:|---|---|
| <https://github.com/go-chi/chi> | `a54874f` | Go router registration, middleware, subroutes, mounted handlers. | `mux.go`, `chain.go`, `context.go` |
| <https://github.com/gorilla/mux> | `db9d1d0` | Go fluent route builders, subrouters, route vars, middleware wrapping. | `mux.go`, `route.go`, `middleware.go` |
| <https://github.com/gin-gonic/gin> | `5f4f964` | Go handler chains, route groups, request context sources, abort/next semantics. | `routergroup.go`, `gin.go`, `context.go` |
| <https://github.com/labstack/echo> | `d17c907` | Go pre/use middleware, group prefixes, binding order, context sources, file/redirect sinks. | `echo.go`, `group.go`, `route.go`, `bind.go`, `context.go` |
| <https://github.com/expressjs/express> | `f873ac2` | JS app/router creation, route methods, middleware, route chaining, mounted apps. | `lib/application.js`, `lib/express.js`, examples |
| <https://github.com/fastify/fastify> | `1c49974` | JS route object registration, hooks, plugins, prefix composition. | `lib/route.js`, `lib/plugin-override.js`, `fastify.js` |
| <https://github.com/nestjs/nest> | `067a071` | TS decorators, controllers, parameter decorators, DI, guards/interceptors/pipes, microservice patterns. | `packages/common/decorators`, `sample/*` |
| <https://github.com/pallets/flask> | `9fcd34c` | Python decorators, blueprints, URL rules, request lifecycle hooks, class-based views. | `src/flask/sansio/scaffold.py`, `app.py`, `views.py` |
| <https://github.com/django/django> | `da6567d` | Python URLconf, includes, class-based views, middleware, project templates. | `django/urls/conf.py`, `django/urls/resolvers.py`, `views/generic/base.py` |
| <https://github.com/fastapi/fastapi> | `ecace74` | Python path operation decorators, routers, dependencies, lifespan, background tasks. | `fastapi/applications.py`, `fastapi/routing.py`, `dependencies/utils.py` |
| <https://github.com/encode/starlette> | `e935b6b` | ASGI routing, lifespan, middleware, background tasks. | `starlette/routing.py`, `applications.py` |
| <https://github.com/celery/celery> | `8dace6d` | Python task registration, worker/job entrypoints, signals. | task and app modules |
| <https://github.com/pallets/click> | `63daae2` | Python CLI command/group decorators and callback registration. | command/core decorators |
| <https://github.com/spring-projects/spring-framework> | `2f458f9` | Java annotations, request mapping, filters/interceptors, DI/lifecycle. | `spring-webmvc`, `spring-context`, annotations |
| <https://github.com/semgrep/semgrep> | `2940ecd` | Pattern/rule ergonomics, taint vocabulary, framework-specific pragmatic modeling. | rule and analysis implementation |
| <https://github.com/secure-software-engineering/FlowDroid> | `73cee57` | Android lifecycle/dummy-main/callback modeling reference. | `soot-infoflow-android`, `entryPointCreators`, `callbacks` |
| <https://github.com/modelcontextprotocol/typescript-sdk> | `22595b9` | MCP TS registration and protocol dispatch: tools, resources, prompts, request handlers. | `packages/server/src/server/mcp.ts` |
| <https://github.com/modelcontextprotocol/python-sdk> | `161834d` | MCP Python decorators/managers and low-level protocol handlers. | `src/mcp/server/mcpserver/server.py`, `tools/tool_manager.py` |

## Real Application Smoke References

These are not gold benchmarks by themselves. They are useful smoke references for normal app layout and wiring.

| Repository | Commit | Why It Was Studied |
|---|---:|---|
| <https://github.com/OWASP/NodeGoat> | `c5cb68a` | Express app with middleware and routers. |
| <https://github.com/juice-shop/juice-shop> | `07b0c19` | Large TypeScript/Express security training app with many routes and middleware chains. |
| <https://github.com/spring-projects/spring-petclinic> | `c7ee170` | Spring MVC controller annotations and route composition in a canonical app. |

## Existing Cross-Research Repositories Reused

| Repository | Existing Folder | Why It Matters Here |
|---|---|---|
| GitHub CodeQL | `research/data-flow/repos/codeql` | Best concrete framework model source for JS/TS, Go, Python, Java. |
| Pyre/Pysa | `research/evaluation-harness/repos/pyre-check` | Python model generators, Django URL/view source modeling, generated model tests. |
| gosec | `research/evaluation-harness/repos/gosec` | Go security fixture corpus and analyzer behavior reference. |
| SecBench.js | `research/evaluation-harness/repos/SecBench.js` | JS/Node executable vulnerability benchmark. |
| DroidBench | `research/evaluation-harness/repos/DroidBench` | Lifecycle/callback/data-flow fixture methodology. |
| SecuriBench Micro | `research/evaluation-harness/repos/securibench-micro` | Java servlet/web taint microbenchmarks. |

## Key Source Observations

### Go

- chi exposes route and middleware registration through `Use`, `With`, `Group`, `Route`, `Mount`, `Handle`, `HandleFunc`, `Method`, and HTTP verb methods in `mux.go`.
- gorilla/mux splits route constraints across fluent `Route` builders: `Path`, `PathPrefix`, `Methods`, `Host`, `Queries`, `Schemes`, `Subrouter`, `Handler`, and `HandlerFunc`.
- gin route groups and handlers are variadic handler chains. Middleware continuation differs from Express/Echo because gin's engine drives the chain and `Abort` stops it.
- echo has pre-routing middleware, post-routing middleware, route-specific middleware, groups, wrapping of stdlib handlers, and explicit binding order.

### TS/JS

- Express app and router APIs normalize around method calls and middleware arrays, but mounted routers and `app.route()` require composition.
- Fastify route registration includes object-form `route`, verb shorthands, hooks, plugin registration, and prefix propagation.
- Nest pushes entrypoint recovery into decorator metadata and DI/module structure.
- MCP TypeScript SDK exposes protocol boundary callbacks through both high-level registration APIs and low-level `setRequestHandler` dispatch.

### Python

- Flask's `route` decorator delegates to `add_url_rule`; blueprints defer setup and prefix endpoints.
- Django URLconf recursion and class-based `as_view` are the main entrypoint discovery shape.
- FastAPI/Starlette combine route decorators, routers, dependencies, background tasks, and lifespan.
- Pysa's Django model generation shows a high-precision but side-effect-prone runtime import approach.

### Java/JVM

- Spring MVC route recovery is annotation closure plus class/method composition.
- DI, reflection, generated code, servlet filters, tests, and Android lifecycle require separate fact families, not a single call graph algorithm.
