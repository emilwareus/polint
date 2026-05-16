# Rule Authoring Evaluation Plan

## Goal

Measure whether humans and AI agents can author correct polint rules, models,
and provider extensions with tight feedback.

This is not only runtime benchmarking. It is product capability benchmarking.

## Metrics

For generated rules:

```text
time to first compiling rule
number of compile/test repair iterations
positive fixture pass
negative fixture pass
snapshot stability
diagnostic precision
false positives in fixture set
false negatives in fixture set
runtime and cache behavior
capability diagnostics quality
```

For models:

```text
number of matched model rows
dead/stale model rows
default-vs-modeled fact delta
default-vs-modeled diagnostic delta
precision/recall delta where ground truth exists
rejected model count and reason
runtime delta
```

For provider extensions:

```text
protocol failures
fact validation failures
determinism across runs
fact snapshot stability
consumer rule pass
failure-mode diagnostics
cache invalidation behavior
runtime delta
```

## Benchmark Scenarios

### Simple Syntactic Policy

Example: no raw color literals in TSX.

Expected:

- generated rule uses `StringLiterals<'_>` or JSX facts;
- one positive and one negative fixture;
- no advanced model/provider required.

### Architecture Boundary Policy

Example: UI layer cannot import server-only modules.

Expected:

- rule uses `Imports<'_>` or `ResolvedImports<'_>`/`ModuleGraphFacts<'_>`;
- manifest shows resolved-import capability when needed;
- setup missing blocks rule with capability diagnostic.

### Source-To-Sink Policy

Example: user input reaches process execution.

Expected:

- rule uses future `DataFlow<'_>` builder;
- source/sink facts may come from model pack;
- diagnostic includes evidence path;
- test asserts path summary.

### Framework Entrypoint Model

Example: custom router registers HTTP handlers.

Expected:

- model or provider emits entrypoint facts;
- default-vs-extended diff shows unknowns removed;
- consumer rule sees `Entrypoints<'_>`;
- extension facts carry provenance.

### Summary Model

Example: wrapper function sanitizes HTML.

Expected:

- declarative summary/sanitizer model;
- default mode reports or remains unknown;
- modeled mode suppresses false positive only with evidence;
- stale/invalid model row is rejected.

### Fix/Rewrite Scenario

Future example: replace raw literal with token.

Expected:

- structured fix;
- applicability set;
- dry-run output;
- before/after snapshot;
- conflict handling.

## Test Corpus

Use:

- current polint examples;
- new tiny fixture repos under `.polint/tests`;
- selected external evaluation-harness benchmarks for source/sink policies;
- synthetic provider/model fixtures for extension behavior.

## Pass Criteria

The authoring system passes the first gate when:

- `polint new-rule` creates a compiling rule with positive and negative tests;
- `polint test` produces deterministic JSON snapshots;
- `polint inspect rule` explains capability derivation;
- `polint facts sample` gives bounded facts useful to an agent;
- `polint unknowns` produces actionable unknowns;
- a model pack test proves default-vs-modeled delta;
- a provider fixture proves emitted facts are deterministic and validated.

## Failure Signals

Treat these as design failures:

- agents edit internal modules instead of public SDK rule code;
- generated rules request broad `RuleCtx` fact access;
- tests only assert "some diagnostic exists";
- model packs change diagnostics without a fact/model delta explanation;
- provider extensions can emit facts that do not bind to stable IDs;
- capability diagnostics do not tell the author what to change;
- snapshots are nondeterministic.
