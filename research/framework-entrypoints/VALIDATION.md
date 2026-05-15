# Validation, Accuracy, And Complexity Plan

## Principle

Validate facts before diagnostics.

A route, tool, task, callback, or lifecycle fact can be wrong even when no rule currently uses it. The harness should score the analysis substrate directly.

```text
expected framework facts
  -> observed framework facts
  -> strict/loose matching
  -> default-vs-extension delta
  -> diagnostics/path scoring later
```

## Ground Truth Shape

Use a canonical expected-fact schema:

```json
{
  "language": "typescript",
  "framework": "express",
  "kind": "http_route",
  "file": "src/server.ts",
  "handler": "getUser",
  "registration_span": {"line": 42, "column": 7},
  "method": "GET",
  "path": "/users/:id",
  "sources": [
    {"kind": "path_param", "name": "id"},
    {"kind": "query", "name": "*"},
    {"kind": "body", "name": "*"}
  ],
  "middleware": ["auth", "rateLimit"],
  "precision": "ExactStatic"
}
```

For MCP:

```json
{
  "language": "typescript",
  "framework": "mcp-typescript-sdk",
  "kind": "mcp_tool",
  "tool_name": "fetch_url",
  "handler": "fetchUrl",
  "request_source": "params.arguments",
  "return_boundary": "CallToolResult.content"
}
```

## Metrics

| Metric | Purpose |
|---|---|
| Entrypoint precision | Prevents over-modeling helper callbacks as externally reachable. |
| Entrypoint recall | Prevents hidden false negatives. |
| Binding accuracy | Ensures method/path/event/tool metadata matches the handler. |
| Source-object precision/recall | Checks query/body/header/path/MCP/CLI/queue sources. |
| Middleware/lifecycle edge recall | Checks ordering and guard edges. |
| Unknown rate | Measures unresolved dynamic framework behavior. |
| Unknown reduction | Measures agent/provider value. |
| Extension delta | Measures default mode vs agent-extended mode. |
| Cache determinism | Ensures cold/incremental/provider-order outputs match. |
| Provider cost | Runtime, memory, facts/sec, cache hit rate. |

## Matching Modes

Strict matching requires:

- same stable key;
- same language/framework/kind;
- same file/span;
- same target symbol;
- same route/tool/event metadata;
- same source kinds.

Loose matching allows:

- same callable and compatible metadata;
- source span within a small line window;
- equivalent path normalization;
- same method set even if order differs.

Report both. Strict catches implementation churn; loose catches real capability.

## Complexity Budgets

Default recognizers must be cheap enough for CI.

| Tier | Budget | Allowed Work |
|---|---|---|
| Tier 0 | Linear scan | Package/import tables, decorators/annotations/calls, direct request sources. |
| Tier 1 | Linear plus small maps | Intra-file component and registration tracking. |
| Tier 2 | Capped worklist | Inter-file router/controller builder summaries. |
| Tier 3 | Capped fixpoint | Middleware ordering, lifecycle composition, synthetic dispatch overlay. |
| Tier 4 | Opt-in | Type/points-to/callback/data-flow-assisted framework recovery. |

Every capped analysis must emit budget/truncation facts.

## External Benchmarks

External benchmarks are useful but incomplete for this track.

| Suite | Use |
|---|---|
| CodeQL query tests and framework libraries | Reference taxonomy for framework facts and source/sink APIs. |
| SecBench.js | JS/Node taint behavior and executable package-level security cases. |
| RealVuln | Python real-app scanner outcomes once Python exists. |
| OWASP Benchmark | Diagnostic scorecard discipline and Java/Python web cases. |
| SecuriBench Micro | Java servlet taint and source/sink microcases. |
| DroidBench/FlowDroid | Lifecycle/callback methodology. |
| gosec samples | Go security rule/sample behavior; not enough for route recall. |
| Pysa tests/generators | Python generated model and Django URL/view modeling. |
| NodeGoat/Juice Shop/Spring PetClinic | Real-app smoke scans, not gold truth without hand labels. |

## Native Fixture Matrix

### Go

- `net/http` default mux and explicit mux.
- `http.Server{Handler:nil}` and explicit handler.
- Go 1.22 path patterns and `Request.PathValue`.
- chi `Use`, `With`, `Group`, `Route`, `Mount`, `URLParam`.
- gorilla/mux fluent `Path/Methods/HandlerFunc`, `Subrouter`, `Vars`, `MatcherFunc`.
- gin nested groups, handler chains, `Abort`, `Next`, `Param`, `Query`, `BindJSON`.
- echo `Pre`, `Use`, `Group`, route middleware, `WrapHandler`, `Bind`, `Context.Set/Get`, `Redirect`, `File`.

### TS/JS

- Express app/router, mounted routers, middleware arrays, `app.route`, `next("route")`, CJS/ESM imports.
- Fastify `route` object, verb shorthand, hooks, plugin `register` prefixes, async return vs `reply.send`.
- Nest controller prefix + method route, param decorators, guard/interceptor/pipe, custom param decorator.
- MCP TS `registerTool`, `tool`, `registerResource`, `registerPrompt`, `setRequestHandler` for `tools/call`.
- EventEmitter `on/once/emit` as later callback graph fixture.

### Python

Future adapter fixtures:

- Flask blueprint prefix, `MethodView`, `add_url_rule`, hooks.
- Django nested `include`, `path`, `re_path`, `View.as_view`, string view path.
- FastAPI `APIRouter`, `include_router`, dependencies, background tasks, lifespan.
- Celery `@app.task`, `@shared_task`, beat schedule.
- Click/Typer command groups and console script entrypoints.

### Java/JVM

Future adapter fixtures:

- Spring MVC class/method annotations, composed annotations, request parameter binding.
- Servlet `HttpServlet`, filters, listeners.
- JAX-RS `@Path`, sub-resources, `@PathParam`, `@QueryParam`.
- DI graph with `@Component`, `@Bean`, qualifiers, profiles.
- JUnit 4/5 tests and lifecycle hooks.
- Android manifest components and lifecycle callbacks.

## Validation Gates

Each provider output should pass:

1. Schema validation.
2. Stable key normalization.
3. Span exists and belongs to the source digest.
4. Target symbol exists or synthetic target is justified.
5. Metadata is internally consistent.
6. Precision does not exceed provider ceiling.
7. Extension trust status is correct.
8. Merge conflicts become diagnostics.

Extension facts additionally require:

- fixture expected facts;
- provider source digest in cache key;
- deterministic output hash;
- default-vs-extension delta report.

## Reporting Format

A framework provider report should include:

```text
Provider: builtin.ts.express
Produced:
  entrypoints: 31
  trust_boundaries: 94
  dispatch_edges: 47
  unresolved: 6
Precision:
  ExactStatic: 23
  ResolvedStatic: 8
  Conservative: 2
Unknowns:
  dynamic route string: 3
  unresolved mounted router: 2
  unsupported wrapper: 1
Cost:
  runtime_ms: 18
  peak_bytes: ...
Cache:
  layer_digest: ...
  hit: false
```

## Acceptance Criteria For First Implementation

- Native fixtures cover Go `net/http` + one router and TS/JS Express.
- `Entrypoints<'_>` returns real facts with precision/status.
- Dynamic/unresolved patterns are visible.
- Cache keys include framework provider input digests.
- Extension provider can add facts through `EntrypointSink`.
- Extension facts cannot suppress native facts.
- `git diff` of expected/observed facts is stable across runs.
