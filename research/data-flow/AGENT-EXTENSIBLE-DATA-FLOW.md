# Agent-Extensible Data Flow

Date: 2026-05-15

## Core Shift

Classic data-flow tools assume that source, sink, sanitizer, framework, lifecycle, and summary models must be shipped by the analyzer vendor or discovered generically. That assumption limits precision for repo-specific policies because every organization has its own wrappers, trust boundaries, validators, service clients, generated code, and internal frameworks.

polint should use a different model:

```text
native data-flow substrate
  + explicit unknown/havoc facts
  + repo-local data-flow models
  + validation fixtures
  + provenance-aware SDK queries
```

The native engine remains deterministic and fully implemented inside polint. Agents do not replace the solver. Agents author model inputs and rules that the engine validates against symbols, calls, CFG nodes, places, spans, and call graph facts.

## What The Native Default Should Do

The default engine should provide:

1. local CFG and operation facts;
2. sparse value-flow edges;
3. low-depth access paths with explicit widening;
4. direct-call interprocedural parameter, return, and receiver edges;
5. summary facts where call graph precision is sufficient;
6. unknown/havoc facts for unresolved calls, reflection, dynamic property access, missing lifecycle setup, and unsupported constructs;
7. path evidence with algorithm, precision, and provenance.

This makes the engine useful even before a repository has custom models.

## What Agents Can Add

Agents can write repo-local models for:

- sources, such as request bodies, headers, cookies, queue messages, CLI args, env vars, secrets stores, MCP tool inputs, and generated client responses;
- sinks, such as SQL execution, shell execution, file writes, network calls, template rendering, logging, telemetry, auth decisions, and agent tool outputs;
- sanitizers and barriers, such as validators, schema parsers, escaping functions, permission checks, and allowlist guards;
- additional flow steps, such as builder APIs, fluent chains, serialization wrappers, framework context bags, and dependency-injection lookups;
- function summaries, such as "arg 0 flows to return" or "receiver.field flows to return";
- entrypoints and trust boundaries, such as HTTP routes, jobs, tests, MCP tools, and generated RPC handlers;
- call graph models needed to make interprocedural flow possible.

These models are more complex than a generic linter config. That is acceptable because the main user is an AI agent that can inspect the repository and maintain the model with tests.

## Model Shape

```python
DataFlowModel(
    id="repo.auth_and_sql",
    language="typescript",
    scope=["src/**/*.ts"],
    sources=[
        Source(call="ctx.request.body", label="user_input"),
        Source(call="tool.input", label="mcp_input"),
    ],
    sinks=[
        Sink(call="db.query", label="sql_execution"),
        Sink(call="shell.exec", label="command_execution"),
    ],
    sanitizers=[
        Sanitizer(call="validateSqlParams", removes=["user_input"]),
        Sanitizer(call="z.object(...).parse", removes=["user_input"]),
    ],
    barriers=[
        Barrier(condition="isAdmin(user)", blocks=["privilege_sensitive"]),
    ],
    additional_steps=[
        FlowStep(from_="builder.set(name, value).receiver", to="builder.execute().args"),
    ],
    summaries=[
        Summary(function="normalizeUser", param_to_return=[0]),
        Summary(function="Sql.where", receiver_to_return=True),
    ],
    validation=[
        Fixture("fixtures/sql/sanitized"),
        Fixture("fixtures/sql/unsafe"),
    ],
)
```

Model-produced facts should carry:

```text
provenance = "repo_model"
model_id = "repo.auth_and_sql"
validation_status = "validated" | "unvalidated" | "failed"
confidence = "high" | "medium" | "low"
```

Rules must be able to include or exclude model facts and inspect their provenance in path evidence.

## Extension Workflow

```python
native = run_native_data_flow(repo)

unknowns = native.unknowns(reason=[
    "unresolved_call",
    "framework_lifecycle",
    "missing_source_model",
    "missing_sink_model",
])

for gap in unknowns:
    model = agent_proposes_repo_model(gap)
    if model.binds_to_symbols_and_spans() and model.has_fixture_or_explicit_assumption():
        add_model(model)

extended = run_data_flow(repo, models=repo_models)
compare_default_to_extended(native, extended)
```

The product loop should make these questions visible:

- Which unknown/havoc facts remain?
- Which source-to-sink paths appeared only because of a repo model?
- Which paths disappeared because a sanitizer or barrier model was added?
- Which models are unvalidated?
- Which model added too many paths or too much runtime?

## Data Flow Depends On Call Graph Models

Interprocedural data flow cannot be better than the call graph it uses. The call graph research therefore becomes a required input:

- unresolved calls create unknown/havoc data-flow edges;
- repo-local call graph models can turn unknown edges into validated summary hops;
- route, decorator, DI, and generated-client models usually create both call edges and data-flow entrypoints;
- path evidence must show when a step crosses a model-produced call edge.

Data-flow models and call graph models should share provenance, validation status, and cache digest handling.

## Product Consequences

This changes the implementation priorities:

1. Build the data-flow substrate before specialized vulnerability rules.
2. Treat sources, sinks, sanitizers, barriers, summaries, and additional flow steps as modelable facts.
3. Store model identity on nodes, edges, summaries, and path steps.
4. Emit unknown/havoc facts whenever the engine lacks lifecycle, call graph, or language support.
5. Make default-vs-extended path deltas a first-class debug view.
6. Require fixtures for high-confidence model promotion.
7. Keep the public SDK typed and stable; keep solver internals private.

## Evaluation Metrics

Benchmarks should report default and extended results separately:

```text
default_sources
default_sinks
default_paths
default_unknown_havoc
model_sources_added
model_sinks_added
model_sanitizers_added
model_summaries_added
model_paths_added
model_paths_pruned
model_unknown_havoc_reduced
paths_by_model_id
precision_default
recall_default
precision_extended
recall_extended
runtime_delta_ms
memory_delta_bytes
```

The result is not a weaker analysis than classic static analysis. It is a higher-ceiling architecture: native algorithms provide reliable facts, while agents add validated repo-specific semantics where universal tools must either guess or give up.
