# Standard Research Vocabulary

This file defines a common structure for comparing frameworks, papers, implementations, and future polint providers.

## Implementation Profile

Each implementation should be described with:

| Field | Meaning |
|---|---|
| Name | Tool/library/framework being studied. |
| Language scope | Go, TS/JS, Java/JVM, Python, Android, MCP, multi-language. |
| Boundary kinds | HTTP, CLI, tests, jobs, lifecycle, callbacks, protocol tools, serverless, generated dispatch. |
| Model source | Static code, config files, annotations, decorators, manifests, dynamic traces, generated models, manual tables. |
| Output facts | Entrypoints, routes, handlers, sources, sinks, call edges, points-to, lifecycle edges, summaries. |
| Precision strategy | Exact, conservative, heuristic, dynamic under-approximation, manually curated. |
| Extension strategy | Built-in code, declarative models, generated models, user/agent Rust provider. |
| Validation method | Unit fixtures, external benchmark, dynamic trace, manual review, mutation, expected facts. |
| Complexity drivers | Number of calls, decorators, routes, framework components, type facts, points-to state, fixpoint iterations. |
| Limits | Dynamic registration, reflection, generated code, missing type info, runtime imports, plugin systems. |

## Fact Model

Use this normalized vocabulary in reports.

### Boundary

A boundary is a point where external, framework, or protocol control reaches application code, or where application data crosses back out through a protocol or privileged action.

Examples:

- HTTP request to route handler.
- MCP tool invocation.
- CLI command callback.
- Queue message consumer.
- Test runner invoking a test.
- Framework calling lifecycle hook.
- Serverless platform invoking a handler.
- External web content returned through MCP-visible output.

### Entrypoint

An entrypoint is a callable reachable from a boundary through framework/protocol dispatch.

An entrypoint is not always a source. For example, a scheduled cleanup job is an entrypoint but may not be attacker-controlled. A route handler is both an entrypoint and usually a request trust boundary.

### Registration

A registration is source evidence that binds a framework/protocol trigger to a callable.

Examples:

- `app.get("/x", handler)`
- `router.HandleFunc("/x", handler)`
- `@GetMapping("/x")`
- `@app.route("/x")`
- `server.registerTool("x", ..., handler)`
- `@click.command()`

### Lifecycle

Lifecycle is ordered framework behavior around entrypoints.

Examples:

- Express middleware before route handler.
- Flask `before_request` and `after_request`.
- FastAPI dependency resolution.
- Spring filter/controller/interceptor.
- Android activity lifecycle.
- Test setup/teardown hooks.

### Synthetic Dispatch Edge

A synthetic dispatch edge is an analysis edge representing framework invocation, not a source-code call.

It must carry edge kind, provenance, and precision.

Examples:

```text
HTTP request "/owners/{ownerId}" -> Spring controller method
MCP "tools/call" request with name "fetch" -> registered tool callback
Express router registration -> handler
pytest runner -> test function
```

### Trust Boundary Source

A source is a value supplied by a boundary.

Examples:

- HTTP query/body/header/cookie/path parameter.
- MCP request arguments.
- CLI args/env/stdin.
- Queue message payload.
- External content returned by network/file/database before crossing into MCP output.

### Output Boundary / Sink

A boundary sink is a protocol-visible return or privileged operation.

Examples:

- HTTP response body/header/cookie/status/redirect.
- MCP tool result content.
- Prompt/tool output passed to an LLM agent.
- Shell command argument.
- Filesystem path or write.
- SQL query string.
- Network request URL.

## Precision Labels

| Label | Definition |
|---|---|
| `ExactStatic` | Fully static evidence and target resolved. |
| `ResolvedStatic` | Static evidence with nontrivial symbol/import/alias resolution. |
| `ComposedStatic` | Static evidence composed across routers/controllers/middleware/modules. |
| `Conservative` | Sound-ish over-approximation in a constrained context. |
| `Heuristic` | Useful guess with meaningful false-positive risk. |
| `RuntimeDerived` | Learned from dynamic observation; usually under-approximate. |
| `AgentAsserted` | Emitted by repo-local provider before validation raises trust. |
| `ValidatedExtension` | Extension fact passed schema, referential, and fixture validation. |
| `Unsupported` | Known pattern not modeled. |
| `Unknown` | Behavior exists but target/metadata cannot be resolved. |

## Accuracy Metrics

Evaluate at fact level before diagnostic level.

| Metric | Formula |
|---|---|
| Entrypoint precision | matched predicted entrypoints / all predicted entrypoints |
| Entrypoint recall | matched expected entrypoints / all expected entrypoints |
| Binding accuracy | exact route/event/tool + handler match / matched entrypoints |
| Source precision | matched predicted source objects / all predicted source objects |
| Source recall | matched expected source objects / all expected source objects |
| Lifecycle edge recall | matched expected lifecycle/middleware edges / expected lifecycle edges |
| Unknown rate | unresolved facts / recognized framework registrations |
| Extension delta | extended score - default score |
| Cost | runtime, peak RSS, facts/sec, provider time, cache hit rate |

Use strict and loose matching:

- Strict: exact stable key, span, symbol, and metadata.
- Loose: same callable/file and compatible route/tool metadata within a line window.

## Pseudo-Code Style

Use Python-ish pseudo-code for algorithm reports:

```python
def recover_entrypoints(project):
    components = discover_framework_components(project)
    registrations = scan_registrations(project, components)
    graph = compose_framework_graph(components, registrations)
    return validate_and_emit(graph)
```

Keep pseudo-code implementation-neutral. The Rust implementation should use typed IDs, stable keys, deterministic iteration, and validation sinks.
