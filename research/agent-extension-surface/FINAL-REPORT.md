# Final Report: Agent Extension Surface

## Executive Summary

polint should become a framework where users and AI agents can write **Rust code that improves the analysis engine**, not just Rust code that reports diagnostics.

The recommended architecture is a two-surface model:

1. `#[polint::rule]` for diagnostics. Rules read typed fact views and report findings.
2. `#[polint::extension]` for analysis facts. Extensions read typed base facts and emit validated, provenance-labeled facts that later analyses and rules consume.

This is the path to maximum capability and tailored scan accuracy.

## Main Finding

The state of the art splits into two families:

- **Rule plugin systems** such as ESLint, Error Prone, Dylint, and OpenRewrite. These are good at custom checks and developer ergonomics.
- **Model extension systems** such as CodeQL Models-as-Data, Pysa model generators, Semgrep taint rules, and Joern overlays. These are good at improving the analyzer's understanding of frameworks, libraries, data flow, and program structure.

polint needs both. Existing polint rules cover the first family. The missing product surface is the second.

## Recommended Shape

Add repo-local Rust extension crates:

```text
.polint/extensions/<extension-name>/
  Cargo.toml
  src/main.rs
  tests/fixtures/
```

Compile and run them as process-isolated executables with a versioned protocol. They are Rust code, but they do not get loaded into the polint process as arbitrary dynamic libraries.

Extensions should emit only typed facts through controlled sinks:

- `EntrypointSink`
- `CallGraphSink`
- `DataFlowModelSink`
- `EffectSink`
- future `TypeHintSink`, `AliasHintSink`, `FrameworkModelSink`

The host validates every emitted fact before it enters the analysis database.

## Why This Beats Config

Configuration can only describe pre-decided shapes. Rust extensions can do actual repo analysis:

- inspect symbols and references;
- walk framework bootstrap code;
- bind handlers to routes;
- infer project-specific sources/sinks;
- generate summaries for wrappers and adapters;
- reject low-confidence matches;
- emit evidence and provenance;
- run tests and benchmark before/after accuracy.

Because the intended advanced user is an AI agent, a richer extension API is acceptable. The agent can read the repo, write Rust, run fixture tests, and iterate.

## Why Not Dynamic Libraries First

Dylint is the best Rust-code precedent, but it is not the right first runtime for polint:

- it depends on toolchain-qualified dynamic libraries;
- it uses rustc internals;
- it runs plugins in-process;
- it is shaped around Rust compiler lints, not multi-language facts;
- plugin crashes can take down the driver process.

polint should copy Dylint's repo-local Rust authoring story and version handshake, but use a process protocol for stability and isolation.

## Accuracy Impact

Extensions let polint replace hidden uncertainty with explicit, fixable integration tasks.

Default mode:

```text
parse -> imports -> symbols -> references -> syntactic calls -> conservative facts -> visible unknowns
```

Agent-extended mode:

```text
default facts
  + repo Fastify routes
  + project auth boundary sources
  + wrapper sanitizer summaries
  + queue/job entrypoints
  + generated client call edges
  -> better call graph, data flow, effects, and diagnostics
```

This is the central product advantage. polint does not need to perfectly auto-discover every customer framework. It needs a powerful way for agents to encode the framework knowledge they discover.

## Required Invariants

To keep this safe and maintainable:

- extensions cannot mutate `AnalysisDb` directly;
- extensions cannot access raw internal ASTs as the default API;
- emitted facts must bind to stable file/symbol/callsite IDs or declare synthetic IDs;
- every extension fact has provenance, precision, confidence, and evidence;
- cache keys include extension source, Cargo.lock, SDK version, protocol version, options, and input facts;
- failures become diagnostics, not host crashes;
- rule execution happens after extension facts are validated and merged.

## Implementation Order

1. Add extension discovery and process-host runner.
2. Add manifest/handshake protocol.
3. Add `EntrypointSink` first.
4. Add extension-aware capability planning.
5. Add fact validation and provenance.
6. Add extension fixture tests.
7. Add call graph and data-flow sinks.
8. Add extension delta reports.
9. Add agent-facing docs and scaffolding.

The first vertical slice should be small but real: a repo-local Rust extension emits HTTP entrypoints; a rule reads `Entrypoints<'_>`; the report shows provenance and the cache invalidates when extension code changes.

## Final Recommendation

Build polint as:

```text
native static-analysis engine
  + typed fact database
  + simple rule SDK
  + advanced Rust extension SDK
  + strict validation/provenance
  + default-vs-extended accuracy measurement
```

This makes polint less like a sealed universal analyzer and more like an agent-programmable analysis framework. That is the right architecture for "super good and tailored scan accuracy."
