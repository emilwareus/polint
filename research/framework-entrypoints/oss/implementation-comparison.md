# OSS And Research Implementation Comparison

## Summary Table

| System | What It Models | Strength | Limit For polint |
|---|---|---|---|
| CodeQL | Framework libraries, route handlers, request/response nodes, sources/sinks, model data. | Best concrete industrial reference for multi-language framework facts. | Not a dependency; QL/database architecture is too large for first polint core. |
| Pysa/Pyre | Python taint models, generated Django/GraphQL models, summaries. | Strong model-generation and test discipline. | Runtime model generators can execute app code; Python-only. |
| FlowDroid | Android lifecycle, callbacks, dummy main, taint. | Strong lifecycle-aware static analysis reference. | Android/JVM-specific; heavy architecture. |
| CGMiner | Dynamic callback summaries for libraries. | Great callback summary insight and validation data. | Dynamic under-approximation; not a default static approach. |
| F4F | Generated framework specs for web taint analysis. | Clean split between framework model generation and taint engine. | Java web focus; old framework set. |
| AutoWeb | Mutation-based inference of framework entry/call/points-to relations. | Strong evidence for relation-based framework semantics. | Requires runnable apps and dynamic mutation. |
| Semgrep | Pattern rules, taint syntax, framework rules. | Excellent ergonomics and pragmatic source/sink vocabulary. | Pattern rules alone are not a precise analysis substrate. |
| MCP-BiFlow | MCP entrypoint recovery and bidirectional taint. | Best current protocol-boundary reference for AI-agent tool servers. | Recent research; not a general engine. |
| Soot/WALA/OPAL | JVM call graphs, points-to, IR, framework analysis. | Strong algorithm references. | JVM-focused and too heavy as polint dependencies. |
| Jelly/TAJS | JS call graph/points-to/abstract interpretation. | Important research baselines for dynamic JS. | Too heavy for v1; use as later tier reference. |

## CodeQL

CodeQL's framework libraries are the most useful concrete implementation reference.

Observed local examples:

- JavaScript Express model: route setup, handlers, request/response nodes, middleware traversal.
- JavaScript Fastify model: route object/shorthand registration, plugin registration, hooks, request/reply sources.
- JavaScript Nest model: controller methods, parameter decorators, request inputs, pipes, middleware.
- Go stdlib HTTP model: `Handle`, `HandleFunc`, `ServeMux`, request handlers guarded by paths.
- Go model data YAML: source/sink/summary tables for net/http, chi, gin, echo, mux.
- Java Spring model: controller annotations, request mapping annotations, request parameters, tainted inputs.
- Python Flask model: app/blueprint/request/response/view abstractions.

polint should copy the architectural lesson:

```text
framework-specific recognizer
  -> normalized HTTP/server concepts
  -> source/sink/summary facts
```

Do not copy the implementation architecture wholesale. polint needs Rust-native typed facts and repo-local Rust providers.

## Pysa/Pyre

Pysa's model generators show why generated models matter. The Django REST API generator finds views and taints parameters. GraphQL generators recover resolver functions. Tests check generated model text.

polint should copy:

- generator/test discipline;
- source/sink/summary vocabulary;
- expected/unexpected generated-model checks.

polint should avoid:

- executing arbitrary app imports by default;
- treating generated models as validated without fixture evidence.

## FlowDroid And CGMiner

FlowDroid shows lifecycle-aware taint analysis needs synthetic entrypoint construction. Android apps do not have a normal `main`; the analysis creates lifecycle/callback dispatch.

CGMiner shows library callback edges can be summarized once and reused. It distinguishes transfer methods and triggering API calls, which matters for flow-sensitive analysis.

polint should copy:

- lifecycle facts separate from normal call sites;
- callback registration and trigger facts;
- synthetic dispatch overlay;
- explicit data-flow mappings for callback parameters.

polint should avoid:

- Android-specific lifecycle hardcoding as the general model;
- treating dynamic callback summaries as complete.

## F4F And AutoWeb

F4F and AutoWeb are the closest research fit for web frameworks.

F4F:

```text
application code + config
  -> framework behavior spec
  -> taint engine consumes spec
```

AutoWeb:

```text
dynamic relation observation
  -> configuration mutation
  -> minimal sufficient/necessary config sets
  -> entry/call/points-to specs
```

polint should copy:

- framework relations as explicit facts;
- generated specs outside the core solver;
- validation before analysis consumption.

polint should adapt:

- AI agents can write repo-local Rust providers for project-specific wrappers.
- Default recognizers do not need to solve all frameworks universally.

## Semgrep

Semgrep is a strong ergonomics reference. Its taint vocabulary of sources, sinks, sanitizers, propagators, and exactness options maps well to rule author expectations.

polint should copy:

- simple rule author vocabulary;
- explicit source/sink/sanitizer concepts;
- framework-aware rule packs as usability evidence.

polint should avoid:

- making YAML pattern rules the advanced analysis extension surface;
- expecting pattern matching alone to recover lifecycle/call graph/data flow.

## MCP-BiFlow

MCP-BiFlow is directly relevant to polint's AI-agent thesis.

Key ideas:

- MCP tools/resources/prompts are protocol entrypoints.
- Request-side propagation starts from decoded MCP arguments.
- Return-side propagation treats protocol-visible output as a sink/boundary.
- Entry recovery must handle direct declaration, explicit registration, and protocol-level dispatch.

polint should copy:

- MCP as a first-class boundary family.
- Bidirectional trust-boundary framing.
- Tool-scoped path evidence.

polint should adapt:

- Make MCP facts available to repo-local rules and future data flow.
- Let agents add project-specific MCP wrapper providers.
