# Final Report: Framework, Lifecycle, And Entrypoint Modeling

## Executive Decision

Build a native framework boundary layer before full call graph and data flow.

```text
syntax/imports/symbols
  -> framework component discovery
  -> registration and lifecycle recovery
  -> normalized entrypoint/trust-boundary facts
  -> optional synthetic dispatch overlay
  -> call graph reachability and data-flow sources/sinks
```

The layer should recover framework/protocol boundary facts with provenance. It should not claim exact runtime behavior. Routes, decorators, middleware, jobs, CLI commands, MCP tools, and lifecycle callbacks are evidence-backed analysis facts, not magic call graph edges.

The first public-facing SDK view should be `Entrypoints<'_>` backed by real facts, validation, docs, and cache keys. Keep the provider internals private. Repo-local Rust analysis providers should be able to add validated framework facts through narrow sinks.

## Why This Matters

Call graphs and data flow fail quietly when entrypoints are wrong. A missing route handler, MCP tool callback, Celery task, Spring controller, test method, or CLI command hides every downstream call edge and every source-to-sink path reachable only from that boundary.

The research repeatedly shows the same pattern:

- Frameworks invoke application code through configuration, reflection, decorators, protocol dispatch, router tables, and generated dispatch.
- Generic call graph construction does not see those edges reliably.
- Mature analyzers handle this by modeling framework semantics as separate facts or models.
- Static-only models need explicit precision labels because dynamic strings, generated code, plugins, DI, and runtime registration cannot always be resolved.

F4F is the classic web-framework lesson: process application code and config to generate framework-behavior specifications, then let taint analysis consume those specs. AutoWeb pushes the same idea further by automatically inferring Java web framework relations through configuration mutation. CodeQL's framework libraries are the pragmatic industrial form of this idea. FlowDroid and CGMiner show the same pattern for lifecycle and callbacks. MCP-BiFlow shows the current AI-agent-era version: protocol-specific entrypoint recovery and trust-boundary taint semantics are required for MCP servers.

## The Product-Specific Shift

Traditional static analyzers assume the analyzer must be a black box that works across arbitrary codebases. polint has a different operating model: the advanced user can be an AI coding agent that inspects the repository and writes repo-local Rust analysis code.

That changes the implementation target:

```text
sane default recognizers
  + explicit unknowns
  + typed facts
  + provenance and precision
  + repo-local Rust providers
  + validation fixtures
  + default-vs-extended evaluation
```

polint does not need to perfectly auto-discover every framework convention by default. It needs a native substrate where default analysis exposes facts and uncertainty, then agents can add accurate repo-specific models without corrupting the engine.

Examples:

- Default recognizer sees `app.use("/api", router)` but cannot resolve a project-local router wrapper. It emits an unresolved framework registration fact.
- An agent reads the wrapper, writes `.polint/extensions/acme_routes`, and emits `EntrypointFact` and `FrameworkDispatchEdge` facts with fixture tests.
- The kernel validates spans, symbols, route metadata, source kinds, precision ceilings, cache keys, and merge semantics.
- The evaluation harness reports whether recall improved, whether false positives changed, and which unknowns disappeared.

## Core Fact Families

The normalized internal model should be language-neutral. Language providers can emit specialized facts, but the SDK should expose stable views.

| Fact | Purpose |
|---|---|
| `EntrypointFact` | Externally or framework-reachable callable: HTTP route, MCP tool, CLI command, test, job, queue consumer, lifecycle callback, serverless handler. |
| `FrameworkComponentFact` | App/router/controller/server/blueprint/module/bean/worker/test-suite object that owns registrations. |
| `RegistrationFact` | Source-level act that registers a handler, middleware, callback, task, route, resource, prompt, command, or hook. |
| `LifecycleFact` | Ordering and trigger facts such as startup, shutdown, before request, after request, teardown, middleware chain, async job, test lifecycle. |
| `TrustBoundaryFact` | How untrusted or external data enters: request params, body, headers, cookies, MCP args, CLI args, env, stdin, queue payloads, external resource returns. |
| `FrameworkSourceFact` | Concrete source expression or parameter with source kind and scope. |
| `FrameworkSinkBoundaryFact` | Protocol-visible output or privileged boundary: response, MCP return, file write, shell, network request, DB query, prompt/tool output. |
| `FrameworkDispatchEdgeFact` | Optional synthetic edge from framework/protocol root or registration site to handler/middleware/callback. |
| `UnresolvedFrameworkFact` | Explicit unknown: dynamic route, unknown wrapper, unresolved handler, missing setup, unsupported framework version, budget exceeded. |

Every fact needs:

```text
stable_key
run_id
language
file/span
target symbol or synthetic target
framework id and version evidence
kind
metadata
provenance
precision
confidence
validation status
provider id/version/schema
parents/evidence
```

## Precision Tiers

Use honest, comparable precision labels.

| Tier | Meaning | Typical Examples |
|---|---|---|
| `ExactStatic` | Literal registration and resolved handler in analyzed source. | `app.get("/x", handler)`, `@GetMapping("/x")`, `http.HandleFunc("/x", h)`. |
| `ResolvedStatic` | Registration is static enough, but required alias/import/symbol resolution. | Express router imported from another file, chi subrouter callback, Spring meta-annotation. |
| `ComposedStatic` | Multiple framework facts composed into one boundary. | Router prefixes, blueprint prefixes, Nest controller + method decorators, middleware stacks. |
| `Conservative` | Handler known but route/order/source metadata may over-approximate. | Dynamic route string, method array with partial literals, wildcard middleware. |
| `Heuristic` | Pattern is useful but not guaranteed by semantic evidence. | Naming convention, project wrapper guessed by shape. |
| `RuntimeDerived` | Learned from execution or generated model. | Pysa Django URL import, AutoWeb-style mutation output, dynamic callback summary. |
| `AgentAsserted` | Repo-local Rust provider asserted it. Requires validation before high-trust use. | Custom router provider, company job scheduler provider. |
| `Unsupported` | Recognized but not modeled. | Custom matcher, dynamic plugin system, reflection-heavy dispatch. |

Rules should be able to inspect precision, but normal ergonomic rules should default to high-confidence facts unless they opt into lower tiers.

## Accuracy And Complexity

The default recognizers should be cheap. Most first-tier framework recovery is a graph problem over files, calls, definitions, decorators, annotations, and registrations.

Approximate costs:

| Step | Complexity | Notes |
|---|---:|---|
| Framework import/dependency detection | `O(F + M)` | F files, M manifests/imports. |
| Registration scan | `O(C + D)` | C call expressions, D decorators/annotations. |
| Component graph construction | `O(V + E)` | Router/controller/app nodes and registration edges. |
| Prefix/middleware composition | `O(V + E)` with capped fixpoint | Dedupe by normalized route key. |
| Annotation/decorator closure | `O(A + R)` | A annotations/decorators, R meta/decorator relationships. |
| Intra-file alias tracking | `O(N)` per file | N AST nodes/tokens for shallow binding. |
| Inter-file builder summaries | `O(S * I)` capped | S summaries, I iterations; must budget and emit truncation facts. |
| Fact validation/merge | `O(P log P)` or `O(P)` with stable sort/index | P produced facts. |

Do not run expensive points-to or global data flow just to find first-level entrypoints. Let deeper tiers be requested by rules or extension providers.

## State Of The Art: What To Copy And What To Avoid

### Copy

- F4F's split: framework model generation outside the core taint engine.
- AutoWeb's relation vocabulary: entrypoints, points-to, call relations introduced by frameworks.
- CodeQL's framework libraries: normalized route handlers, request sources, response effects, model data.
- Pysa's generated model approach, but not unsafe runtime import as the default.
- FlowDroid's lifecycle/dummy-main idea, generalized as lifecycle facts and synthetic edges.
- CGMiner's callback summary insight: model registration-to-callback edges instead of analyzing entire libraries every time.
- MCP-BiFlow's protocol-aware entrypoint recovery and bidirectional trust-boundary framing.

### Avoid

- Baking every framework rule directly into the base call graph.
- Treating absence of facts as absence of behavior.
- Letting generated or agent-authored models suppress native facts without validation.
- Exposing raw parser ASTs or `AnalysisDb` as the extension API.
- Using external engines as core dependencies. Borrow algorithms and model shapes, but keep polint native.
- Claiming automatic framework inference is solved. AutoWeb and CGMiner are strong research, but their dynamic/mutation methods have scope limits and under-approximation risks.

## First Implementation Scope

Current polint supports Go and TS/JS. Start there.

Recommended first vertical slice:

1. Internal `EntrypointFact`, `TrustBoundaryFact`, `FrameworkDispatchEdgeFact`, and `UnresolvedFrameworkFact`.
2. SDK view `Entrypoints<'_>` with route/tool metadata and precision/status filters.
3. Go recognizers: `net/http`, chi, gin, echo, gorilla/mux basics.
4. TS/JS recognizers: Express, Fastify, selected Nest decorator metadata where Oxc can recover it, MCP TypeScript SDK tool/resource/prompt registration.
5. Native fixtures for expected facts, cache determinism, merge behavior, and extension delta.
6. Process-isolated repo-local Rust provider prototype that emits only entrypoint facts through an `EntrypointSink`.

Python and Java/JVM research should remain design input until adapters exist. They are important because they reveal decorators, annotations, DI, URLconf, lifecycle, generated dispatch, and runtime-model-generator patterns, but they should not block the first Go/TS implementation.

## Critical Risks

- Dynamic route strings and plugin systems can hide handlers. Emit explicit unknowns.
- Middleware order is framework-specific. Gin, Express, Koa, Echo, Fastify, and Spring differ materially.
- Agent-authored providers can be plausible but wrong. Require validation status and default-vs-extension reports.
- Cache keys must include framework model code, provider versions, manifests, language lifecycle config, and absence dependencies.
- Sanitizer/barrier facts are dangerous because bad ones create false negatives. Additive entrypoints are safer than negative/suppressing facts.
- Over-modeling entrypoints inflates false positives. Under-modeling hides findings. The evaluation harness must score both.

## Final Recommendation

Use framework/lifecycle modeling as the first serious consumer of the analysis kernel and extension surface.

This is the right first vertical slice because it is valuable before full call graph/data flow, it creates immediate agent-extensibility value, and it forces the kernel to solve provenance, precision, validation, cache keys, and merge semantics on a constrained fact family.
