# Final Report: Rule SDK, Query Ergonomics, And AI-Agent Authoring

## Executive Decision

Build polint's public authoring surface around typed Rust rules, not a new query
language.

The recommended product model is:

```text
#[polint::rule]
  -> policy diagnostics over typed fact views

declarative model packs
  -> API/framework behavior facts under fixed engine semantics

process-isolated provider extensions
  -> repo-local Rust code that emits validated facts or extends analysis

summary domains
  -> reusable function/API behavior facts

fixture/eval harness
  -> proof that rules/models/providers improve results

agent inspect/explain/diff tools
  -> machine-readable feedback loop for Claude/Codex-style authors
```

Do not expose a raw graph DSL, Datalog shell, CodeQL clone, Semgrep YAML clone,
or broad `RuleCtx` fact database as the first primary API.

The public SDK should look like this:

```rust
#[polint::rule(
    id = "local/no-cross-layer-import",
    description = "UI code must not import server-only modules",
    severity = "error",
    docs = "docs/polint/no-cross-layer-import.md",
    precision = "resolved-imports"
)]
fn no_cross_layer_import(
    ctx: &mut RuleCtx<'_>,
    imports: Imports<'_>,
    modules: ModuleGraphFacts<'_>,
    options: NoCrossLayerImportOptions,
) -> RuleResult {
    for edge in modules.import_edges() {
        if options.forbids(edge) {
            ctx.diagnostic(edge.span(), "UI code imports a server-only module")
                .with_related(edge.target_span(), "server-only module")
                .emit();
        }
    }
    Ok(())
}
```

Macro-derived capabilities remain correct, but the macro must also generate an
inspectable manifest so agents can see what the rule requires and why it did or
did not run.

## The Main Insight

The product surface is not just "how do users write a rule." It is "how does an
AI agent discover an analysis gap, choose the right artifact, generate it, test
it, measure it, and explain it."

The core loop should be:

```text
inspect facts and unknowns
  -> classify the gap
  -> scaffold rule, model, summary, provider, or fixture
  -> compile and run real fixtures
  -> compare default vs extended facts/diagnostics
  -> accept only with evidence
```

This is the difference between an agent-native analysis framework and a prompt
library. The agent should write executable artifacts, and polint should validate
and measure those artifacts before any rule depends on them.

## What State Of The Art Shows

### CodeQL

CodeQL's best lesson is not "build QL." It is the split between:

- small query files;
- reusable libraries;
- query metadata;
- path diagnostics;
- model packs;
- test packs.

Production CodeQL security queries are often thin wrappers over library modules
that define sources, sinks, sanitizers, and flow configuration. Polint should
copy the thin-rule/thick-library split with typed Rust views and builders.

### Semgrep

Semgrep's best lesson is authoring velocity. Code-shaped patterns,
metavariables, taint mode, focus metavariables, inline rule tests, and autofix
snapshots make a user productive quickly.

Polint should copy the ergonomics:

- scaffold simple rules quickly;
- separate match range from diagnostic range;
- provide source/sink/sanitizer/propagator vocabulary;
- make rule tests trivial;
- require positive and negative fixtures.

But polint should not make YAML the main power surface. The goal is maximum
analysis capability through Rust-native facts and repo-local extensions.

### ESLint, typescript-eslint, Go analysis

These systems prove the value of:

- metadata as a real contract;
- structured diagnostics;
- option schemas;
- typed rule creators or explicit analyzer dependencies;
- fix/suggestion metadata;
- test harnesses.

Go `analysis` is especially relevant because it makes analyzer dependencies and
fact types explicit. Polint should infer those dependencies from typed
parameters for rule authors, then expose them through manifests.

### Joern And CPGQL

Joern shows the power of fluent graph traversal and path evidence. It also
shows why a raw graph API should not be the default public SDK. Most policy
authors should not stitch together AST, CFG, PDG, call, and data-flow edges by
hand.

Polint should expose typed relationship views:

```rust
calls.named("exec.Command")
flow.sources(user_input()).to(sink.argument(0)).paths().limit(5)
modules.paths_from(ui_package).to(server_package).limit(10)
```

Internally, the engine can be graph-shaped. Publicly, rules should see domain
queries with bounds, precision, and evidence.

### Pysa, CodeQL Model Packs, Semgrep Taint

These systems converge on the same modeling vocabulary:

- sources;
- sinks;
- sanitizers;
- barriers;
- barrier guards;
- propagators;
- summaries;
- access paths;
- labels/features;
- expected/unexpected model tests.

Polint should add model packs as declarative semantic facts. They are not
rules. They change the analyzer's knowledge so multiple rules can improve.

### Ruff, Clippy, OpenRewrite

Ruff and Clippy prove Rust-native analyzers can be disciplined, fast, and
snapshot-tested. OpenRewrite proves before/after transformation tests and dry
run diffs are central when fixes enter the product.

Polint should copy:

- preview/experimental gates;
- safe/unsafe fix applicability;
- committed snapshots;
- before/after fix tests;
- stable diagnostic IDs.

## Artifact Boundaries

The most important product boundary is the artifact selection matrix.

| Artifact | Use it when | Do not use it when |
|---|---|---|
| Rule | The policy can be expressed from existing typed facts and emits diagnostics. | You need to teach the engine new framework or data-flow semantics. |
| Model | You need to describe API behavior: source, sink, sanitizer, barrier, entrypoint, summary, propagator. | You need arbitrary code, resolver changes, or new transfer functions. |
| Provider extension | You need repo-local Rust code to emit new validated facts or extend analysis lifecycle/semantics. | A declarative model is enough. |
| Summary | You need reusable function/API semantics such as argument-to-return flow or side effects. | It is only one rule's final policy. |
| Fixture/benchmark | You need proof, regression coverage, or precision/runtime comparison. | Never. Every generated artifact should have one. |

This prevents a common failure: agents writing huge policy rules that secretly
rediscover sources, sinks, framework routes, and call graph edges in ad hoc code.

## Required SDK Shape

### Rules

Rules should be plain functions:

```rust
#[polint::rule(...)]
fn rule(ctx: &mut RuleCtx<'_>, view: SomeFacts<'_>, options: Options) -> RuleResult
```

Rules should not be:

- generic;
- async;
- raw AST visitors;
- broad database queries;
- manual capability declarations;
- unbounded graph traversals by default.

### RuleCtx

`RuleCtx` should stay narrow:

- diagnostic builders;
- source/path metadata;
- rule options;
- rule id and metadata;
- capability/setup status;
- future structured fixes and suggestions;
- future evidence helpers.

It should not expose:

- `analysis_db()`;
- raw parser ASTs;
- arbitrary fact lookup by string;
- mutable facts;
- extension sinks.

### Fact Views And Query Builders

Fact views should be typed and capability-derived:

```rust
Imports<'_>
ResolvedImports<'_>
ModuleGraphFacts<'_>
Symbols<'_>
References<'_>
Calls<'_>
CallGraph<'_>
DataFlow<'_>
Effects<'_>
Summaries<'_>
Evidence<'_>
```

Query builders should be domain-specific:

```rust
calls.named("exec.Command")
symbols.by_qualified_name("acme.auth.RequireAdmin")
flow.sources(kinds.user_input()).to(sink.argument(0)).paths().limit(5)
effects.for_function(function).contains(EffectKind::WritesFile)
modules.forbid_imports(ui_layer, server_layer)
```

Every potentially expensive query must have visible cost and bounds.

## Rule Manifests Are Mandatory

The `#[polint::rule]` macro should generate a manifest:

```text
rule id
description/docs/tags/severity
messages
option schema
requested fact views
derived capabilities
capability precision requirements
analysis families triggered
supported languages
fixability/applicability
stability: stable | preview | experimental
SDK version
rule code digest
known limitations
```

Agents need this manifest to debug compile errors, setup gaps, unsupported
capabilities, and expensive queries.

## Agent Tooling Is Required

Before promoting advanced rules/providers, add machine-readable commands:

```text
polint facts list --format json
polint facts sample --cap symbols --path src/foo.ts --limit 50
polint capabilities --format json
polint inspect rule local/no-cross-layer-import --format json
polint explain --rule local/no-cross-layer-import --format json
polint unknowns --cap calls|references|dataflow --format json
polint models explain <symbol-or-api> --format json
polint diff --extension <name> --format json
polint eval --suite <suite> --format json
```

Without these, Rust rules become too opaque for agents. With them, agents can
iterate like this:

```text
read unknowns
generate a model/provider/rule
run polint test
read structured failures
repair
run diff/eval
commit only when the delta is good
```

## Test Harness Decision

`polint test` should be a fixture runner, not just `cargo test`.

It should:

1. compile the local rule pack once;
2. create an isolated temp repo per case;
3. copy fixture files and case config;
4. run the real `polint check --format json` path;
5. assert inline expectations;
6. compare normalized JSON snapshots;
7. support fix before/after snapshots later;
8. support model/provider fact snapshots;
9. support `--bless`, `--rule`, `--case`, `--keep-temp`, `--jobs`, and
   `--no-cache`.

Default generated rule layout:

```text
.polint/
  rules/
    Cargo.toml
    src/main.rs
    src/no_raw_colors.rs
  tests/
    rules/no_raw_colors/basic/
      polint-test.toml
      src/example.ts
      expected.snap.json
```

Inline markers:

```ts
// polint-expect local/no-raw-colors: /raw color/
const color = "#ff00aa";

// polint-ok local/no-raw-colors
const color = tokens.brand.primary;
```

Snapshots remain necessary because inline markers do not cover severity,
related locations, evidence paths, model provenance, capability diagnostics, or
JSON schema stability.

## Accuracy And Risk

Typed Rust rules improve correctness and max capability, but they raise author
friction. The mitigation is tooling:

- `polint new-rule` must generate a compiling rule and fixture;
- macro errors must be direct and repairable;
- manifests must show capabilities and precision;
- `polint test` must be fast and exact;
- `polint explain` must show why a rule did not run;
- model/provider deltas must be visible.

The biggest correctness risk is hidden semantics in models and provider
extensions. The mitigation is strict validation:

- unknown symbols become diagnostics;
- impossible argument indexes are rejected;
- unsupported access paths are rejected;
- stale package/version applicability is visible;
- generated/heuristic provenance is retained;
- extension/model digests participate in caches.

## Implementation Sequence

1. Add rule manifest generation and `polint inspect rule`.
2. Add `polint facts list`, `facts sample`, `capabilities`, and `unknowns`.
3. Build `polint test` as a fixture runner with inline markers and JSON
   snapshots.
4. Improve `polint new-rule` to always generate tests.
5. Add typed diagnostic builders with message IDs, related locations, and
   future fix slots.
6. Add domain query builders for existing facts first: imports, modules,
   symbols, references, metrics.
7. Add preview-gated future builders: calls, data flow, effects, evidence.
8. Add declarative model packs for sources/sinks/sanitizers/summaries after the
   analysis families exist.
9. Add provider extension authoring and tests after the process protocol is
   stable.
10. Promote public docs only after temp-repo tests prove outside-agent usage via
    `polint::sdk::prelude::*`.

## Final Recommendation

Polint should optimize for:

```text
small typed Rust rules
  + strong generated manifests
  + low-friction fixtures
  + model/provider extension path
  + explainable facts and unknowns
  + evidence-backed diagnostics
  + default-vs-extended evaluation
```

This keeps simple policies simple, preserves the max-capability path for
advanced analysis, and gives AI agents the feedback loop they need to improve
repo-specific scan accuracy without turning polint into an unbounded query DSL.
