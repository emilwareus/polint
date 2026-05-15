# Agent-Extensible Call Graphs

Date: 2026-05-15

## Core Shift

Classic call graph research assumes the analyzer must infer as much as possible from source code, dependencies, and language semantics without project-specific help. That is the right assumption for a generic compiler, IDE indexer, or SaaS scanner.

polint has a different product shape. The primary users are AI coding agents and repo-local rule authors. That means the engine does not need to be a closed black box that discovers every framework convention by itself. It should be a native static-analysis framework with strong defaults and a deliberate extension surface for repo-specific call models.

The goal is therefore:

```text
native language semantics
  + explicit unresolved call facts
  + repo-local call graph models
  + validation fixtures
  + provenance-aware SDK queries
```

This does not replace static analysis. It changes where precision can come from. The native engine supplies stable syntax, symbol, scope, type, and call facts. Agents can then add narrowly scoped models for the specific repository, framework, generated code pattern, or internal platform layer that the generic engine cannot know.

## What The Native Default Should Do

The default engine should still be useful without any agent-authored model:

1. emit every syntactic call site;
2. resolve direct lexical, import, static, and constructor calls;
3. emit unresolved facts for dynamic dispatch, framework dispatch, generated clients, decorators, reflection, and missing setup;
4. run cheap language-specific algorithms where lifecycle inputs are available;
5. preserve algorithm, provider, confidence, and unresolved reason on every fact.

The default graph is the portable baseline. It should be deterministic, cacheable, and conservative about claims.

## What Agents Can Add

Agents can inspect the repository and write repo-local call graph models for patterns such as:

- web router registrations;
- dependency-injection containers;
- decorators and annotations;
- generated RPC clients and service stubs;
- MCP tool registration and invocation;
- test fixture lifecycle hooks;
- event buses, job queues, and callback registries;
- project-specific factory conventions;
- internal framework wrappers around HTTP, SQL, queues, metrics, or auth.

These are not arbitrary external libraries. They are native polint model inputs that bind back to parsed source facts.

## Model Shape

```python
CallGraphModel(
    id="repo.fastify.routes",
    language="typescript",
    scope=["src/server/**/*.ts"],
    evidence=[
        SourcePattern("server.get(path, handler)"),
        SymbolBinding("server", imported_from="@internal/http"),
    ],
    resolvers=[
        RouteRegistration(
            receiver_symbol="server",
            methods=["get", "post", "put", "delete"],
            handler_arg=1,
            entrypoint_kind="http_route",
        ),
    ],
    validation=[
        Fixture("fixtures/routes/basic"),
        ExpectedEdge("src/server/routes.ts:12", "getUserHandler"),
    ],
)
```

Model-produced facts should carry:

```text
provenance = "repo_model"
model_id = "repo.fastify.routes"
validation_status = "validated" | "unvalidated" | "failed"
confidence = "high" | "medium" | "low"
```

A model edge is never silently equivalent to a native binding edge. Rules and debug exports must be able to include or exclude model edges.

## Extension Workflow

```python
native_graph = run_native_call_graph(repo)

for unresolved in native_graph.unresolved_calls():
    if unresolved.reason in ["framework_dispatch", "dynamic_callback", "decorator"]:
        candidate_model = agent_inspects_repo(unresolved)
        if candidate_model.binds_to_static_facts():
            add_model(candidate_model)

extended_graph = run_call_graph(repo, models=repo_models)
validate_delta(native_graph, extended_graph, fixtures)
```

The important product loop is the delta:

- Which unresolved calls disappeared?
- Which edges were added?
- Which added edges are validated by fixtures?
- Which model assumptions remain unvalidated?
- How much runtime and memory did the model add?

## Examples

### TypeScript Router

```python
if call.matches("router.METHOD(path, handler)") and receiver_is_router(call.receiver):
    emit_edge(
        caller=synthetic_entrypoint("http_route", path_arg(call, 0)),
        target=symbol_of_arg(call, 1),
        kind="framework_dispatch",
        provenance="repo_model",
    )
```

### Python Decorator

```python
if function.has_decorator("@app.tool") or function.has_decorator("@mcp.tool"):
    emit_edge(
        caller=synthetic_entrypoint("mcp_tool", function.name),
        target=function.symbol,
        kind="decorator_registration",
        provenance="repo_model",
    )
```

### Java Dependency Injection

```python
for injection_site in fields_annotated("@Inject"):
    for implementation in repo_model.bindings(interface=injection_site.type):
        emit_edge(
            caller=call_site_enclosing_symbol(injection_site.use),
            target=implementation.method(call.method_name),
            kind="dependency_injection",
            provenance="repo_model",
        )
```

### Go Handler Registry

```python
if call.matches("registry.Register(name, handler)"):
    emit_edge(
        caller=synthetic_entrypoint("registered_handler", literal_arg(call, 0)),
        target=symbol_of_arg(call, 1),
        kind="handler_registration",
        provenance="repo_model",
    )
```

## Product Consequences

This changes the implementation priorities:

1. Native defaults should expose unknowns, not hide them.
2. Call graph facts need provenance and model identity from the first version.
3. The SDK should let rules select edge tiers: direct, semantic, heuristic, and repo-model.
4. Cache digests must include model files and model validation settings.
5. Debug output must show default-vs-extended graph deltas.
6. Model authoring must be testable with temp-repo fixtures.

## Evaluation Metrics

Every benchmark should distinguish default and extended graphs:

```text
default_call_sites
default_edges
default_unresolved
model_edges_added
model_unresolved_reduced
model_edges_by_model_id
model_validation_failures
precision_default
recall_default
precision_extended
recall_extended
runtime_delta_ms
memory_delta_bytes
```

This is the practical path to higher accuracy. polint can keep a native, deterministic engine while allowing agents to add repo-specific call semantics that a universal analyzer would either miss or over-generalize.
