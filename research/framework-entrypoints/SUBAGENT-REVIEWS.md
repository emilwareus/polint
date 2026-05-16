# Subagent And Secondary Review Summary

This file records the parallel research waves used for this track.

## First Wave

### Architecture Review

Key recommendations:

- Use a provider DAG over typed fact layers.
- Normalize framework output into `EntrypointFact`, `LifecycleComponentFact`, `LifecycleTransitionFact`, `CallbackFact`, `SyntheticCallEdgeFact`, and flow model facts.
- Carry id, target symbol/file/span, model/provider id, provenance, precision, confidence/status, and evidence.
- Use additive deterministic merge; manual/project-reviewed facts outrank builtin/generated facts.
- Include schema, provider versions, model/provider code, config, lifecycle setup, source hashes, and sidecar/tool versions in cache keys.
- Feed entrypoints into call graph/data flow as synthetic edges only when those analyses request them.

Adjustment:

The subagent suggested a declarative model-pack DSL before Rust providers. The product requirement is stronger: users/agents should extend analysis with Rust code. The design therefore keeps normalized model facts but makes repo-local Rust providers the primary advanced extension surface. Declarative rows may still be useful as generated fixtures or data assets, not as the main product model.

### TS/JS Review

Key recommendations:

- Build layered facts: package entrypoints, detected frameworks, HTTP routes/middleware, framework dispatch, request inputs/response effects.
- Use Node/TS/Oxc resolution as foundation.
- Track framework instances with shallow access-path/alias propagation.
- Start with Express, Fastify, Nest, Next, Koa/Hapi later.
- CodeQL's JS framework libraries are the best concrete model source.
- Treat callbacks/events as registration-to-dispatch facts.
- Use cheap tiers first; bounded points-to/callback graph/dataflow later.

### Java/JVM Review

Key recommendations:

- Treat JVM framework support as staged entrypoint/modeling layer, not just Java call graph.
- Add annotation closure for Spring and JAX-RS.
- Model servlets, Spring MVC/WebFlux, JAX-RS, JUnit, scheduled jobs, Android lifecycle.
- Keep call graph tiers explicit: direct, CHA, RTA, points-to/context-sensitive.
- Use CodeQL/Soot/WALA/OPAL as references, not dependencies.
- Add facts for annotations, entrypoints, HTTP endpoints, DI graph, reflection, tests, Android lifecycle.

### Python Review

Key recommendations:

- Build Python around entrypoint/lifecycle graph first, not whole-program call graph.
- Parse Python natively first; runtime import discovery should be opt-in.
- Track imports, assignments, decorators, framework object instances.
- Model Flask, Django, FastAPI, aiohttp, Tornado, Celery, Click/Typer, tests.
- Treat ASGI/WSGI as separate entrypoints.
- Emit dynamic imports and setup gaps as facts, not failures.

### Evaluation Review

Key recommendations:

- Use external benchmarks for outcome truth and native fixtures for fact truth.
- Score entrypoint precision/recall, binding accuracy, source-object precision/recall, middleware/lifecycle edge recall, reachability recall, strict/loose matching, macro/micro averages.
- Use RealVuln, SecBench.js, OWASP Benchmark, DroidBench, SecuriBench Micro, CodeQL tests, gosec, Pysa, and real app smoke fixtures.
- Expect failures from indirect registration, dynamic routes, middleware ordering, lifecycle gaps, setup drift, partial analysis, over/under-modeling, benchmark overfit, and runtime-dependent discovery.

## Second Wave

### Skeptical Paper/Research Validation

Main correction:

Frame the result as "recover framework and protocol boundary facts with provenance," not "infer exact application behavior."

Well supported:

- F4F, AutoWeb, and CGMiner support a separate framework/lifecycle fact layer.
- Existing analysis-kernel, call-graph, data-flow, extension, and harness research all point to the same architecture.
- `Entrypoints<'_>` is the right first serious fact family.

Claims to qualify:

- Do not claim native Rust will outperform mature analyzers yet.
- Do not claim automatic framework inference is solved.
- Treat AI/LLM-generated models as heuristic until validated.
- Be careful with MCP and workflow precision claims.

### Native Rust Architecture Review

Key recommendations:

- Build the analysis kernel before call graph/data flow.
- Keep `#[polint::rule]` read-only; add `#[polint::provider]` for fact-emitting extensions.
- Extensions live as repo-local Rust crates under `.polint/extensions/<name>`.
- Providers declare id/version/schema, inputs, outputs, scope, determinism, budgets, precision ceilings, config digest inputs.
- Add sidecar fact metadata: layer, provider, stable key, validation, precision, confidence, evidence.
- Merge is normalized set union. No last-writer-wins.
- Sanitizer/barrier/negative facts need stricter validation than additive entrypoints.
- Move to layer cache keys and track absence dependencies.
- Reject public mutable graph, raw AST exposure, dynamic library plugins, external analysis engines as core dependencies, and starting with global data flow.

### Go Deep Dive

Key recommendations:

- Model stable Go framework facts plus optional synthetic dispatch overlay.
- Do not bake Go routes directly into base call graph.
- Add Go facts: entrypoints, routes, middleware edges, request sources, response sinks, context flows, framework dispatch edges.
- Start with typed symbol index; syntax-only is Tier 0, but real dispatch needs type facts.
- Model `net/http`, chi, gorilla/mux, gin, echo.
- Use CodeQL Go model tables as Tier 0 baseline for sources/sinks/summaries, but build route recovery natively.
- Start CLI with generic `main`, tests, `os.Args`, env, stdin; Cobra later.

## Final Synthesis

The subagents converged on one architecture:

```text
typed facts first
  + entrypoints/trust boundaries before full call graph/data flow
  + optional synthetic dispatch overlays
  + explicit unknowns
  + native Rust provider extensions
  + validation and provenance before merge
```

The final recommendation follows that convergence, with two product-specific constraints:

1. The advanced extension path is Rust provider code, not config as the main mechanism.
2. First implementation scope is Go and TS/JS because those are the currently supported polint languages.
