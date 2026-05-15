# Evaluation Plan For Framework Entrypoints

## Tier A: Fast CI

Run on every PR touching framework providers, kernel facts, SDK views, or extension validation.

Fixtures:

- Go `net/http` direct registrations.
- Go chi basic route and middleware.
- TS/JS Express app/router route and middleware.
- TS/JS MCP `registerTool`.
- Extension provider fixture that adds one route and one unresolved fact.
- Cache determinism fixture.

Metrics:

- strict expected/observed fact diff;
- provider diagnostics;
- deterministic output hash;
- cache key changed only when expected.

## Tier B: Nightly

Add larger native fixtures:

- Go gin, echo, gorilla/mux.
- Express mounted routers, arrays, `app.route`.
- Fastify plugins/prefixes/hooks.
- MCP protocol-level `setRequestHandler`.
- Nest decorator basics.
- Real app smoke scans: NodeGoat, Juice Shop, Spring PetClinic as non-gold smoke references.

Metrics:

- strict and loose precision/recall for hand-labeled fixtures;
- unknown rate by framework;
- default-vs-extension delta;
- runtime and memory by provider.

## Tier C: Release/Research

Use external benchmark suites and hand-labeled real apps:

- SecBench.js for JS/Node security paths.
- OWASP Benchmark for scorecard methodology.
- RealVuln once Python exists.
- SecuriBench Micro once Java exists.
- DroidBench/FlowDroid methodology for lifecycle/callback validation.
- CodeQL tests as taxonomy/reference, not direct truth.

Metrics:

- diagnostic precision/recall/F-scores;
- path evidence quality;
- source/sink matching;
- reachability recall from entrypoints;
- false-positive trap hit rate;
- extension improvement and extension regressions.

## Gold Fixture Design

Each fixture should include:

```text
input repo
expected framework facts
expected unknowns
expected diagnostics, if any
expected provider report
cache inputs and expected invalidation behavior
```

Do not mix too many behaviors in one fixture. Small fixtures make precision failures explainable.

## Real App Smoke Checks

Real apps are useful for scale and layout, but they are not ground truth until labeled.

Smoke output should include:

- number of recognized frameworks;
- number of recovered entrypoints by kind;
- number of unresolved registrations;
- top unknown reasons;
- provider runtime and memory;
- no crashes on parse errors or unsupported syntax.

## Default vs Extension Evaluation

The product differentiator is extension improvement.

For each extension fixture:

```text
default mode:
  entrypoints recovered
  unknowns
  diagnostics

extension mode:
  entrypoints recovered
  unknowns resolved
  new conflicts
  new diagnostics
  runtime/cache cost
```

Report:

- new true positives;
- new false positives;
- unknowns resolved;
- validation failures;
- facts rejected;
- cache invalidated.

## First Benchmark Gate

Before exposing `Entrypoints<'_>` publicly:

- 100% pass on native fact fixtures.
- No nondeterministic fact ordering.
- Provider diagnostics are stable.
- Extension facts cannot suppress native facts.
- Unknown facts appear for dynamic patterns.
- Docs describe limits for Go and TS/JS.
