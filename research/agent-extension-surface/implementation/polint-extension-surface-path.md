# Polint Extension Surface Path

This is the implementation-oriented version of the research recommendation.

## Target User Experience

Rules stay simple:

```rust
use polint::sdk::prelude::*;

#[polint::rule(
    id = "repo/no-secret-logs",
    description = "Do not log secret values",
    severity = "error"
)]
fn no_secret_logs(
    ctx: &mut RuleCtx<'_>,
    dataflow: DataFlow<'_>,
    effects: Effects<'_>,
) -> RuleResult {
    // Read final engine facts and report diagnostics.
    Ok(())
}
```

Extensions are more powerful:

```rust
use polint::extension::prelude::*;

#[polint::extension(
    id = "repo.fastify-model",
    version = "0.1.0",
    description = "Fastify routes and trust boundaries for this repository"
)]
fn extension() -> Extension {
    Extension::new()
        .provider(fastify_entrypoints)
        .provider(fastify_call_edges)
        .provider(repo_trust_boundaries)
}

#[polint::provider(
    id = "fastify-entrypoints",
    inputs = [Symbols, References, Calls],
    outputs = [Entrypoints]
)]
fn fastify_entrypoints(
    ctx: &mut ExtensionCtx<'_>,
    symbols: Symbols<'_>,
    references: References<'_>,
    calls: Calls<'_>,
    out: &mut EntrypointSink,
) -> ExtensionResult {
    for route_call in calls.match_callee("fastify.route") {
        if let Some(handler) = route_call.argument_symbol("handler") {
            out.entrypoint(handler)
                .kind(EntrypointKind::HttpRoute)
                .framework("fastify")
                .evidence(route_call.span())
                .precision(Precision::ValidatedByFixture)
                .emit();
        }
    }
    Ok(())
}
```

## Repository Layout

Recommended:

```text
.polint/
  rules/
    Cargo.toml
    src/main.rs
  extensions/
    repo-fastify-model/
      Cargo.toml
      src/main.rs
      tests/fixtures/
  cache/
    rules-target/
    extensions-target/
    analysis/
    derived/
```

Why not `.polint/models/`? Because the user explicitly wants code, not config. Use "extensions" for executable Rust code. Extensions can emit model facts.

## Crate Split

Add these public modules/crates carefully:

- `polint::extension`: stable extension authoring surface.
- `polint::extension::prelude`: extension macros, context, provider traits, typed sinks.
- `polint::extension_protocol`: versioned wire protocol, internal at first if possible.
- `polint::sdk`: existing rule-authoring surface.

Keep parser adapters, `AnalysisDb`, raw graph storage, and solver internals private.

## First Public Types

```rust
pub struct ExtensionManifest {
    pub id: ExtensionId,
    pub version: String,
    pub sdk_version: String,
    pub description: String,
    pub trust: ExtensionTrust,
    pub deterministic: bool,
}

pub struct ProviderManifest {
    pub id: ProviderId,
    pub inputs: Vec<FactFamily>,
    pub outputs: Vec<FactFamily>,
    pub phase: AnalysisPhase,
    pub budget: ProviderBudget,
}

pub struct FactProvenance {
    pub origin: FactOrigin,
    pub extension_id: Option<ExtensionId>,
    pub provider_id: Option<ProviderId>,
    pub precision: Precision,
    pub confidence: Confidence,
    pub validation: ValidationStatus,
    pub evidence: Vec<EvidenceRef>,
}
```

## First Sinks

Start with fact families that immediately improve call graphs and data flow:

```rust
EntrypointSink
CallGraphSink
DataFlowModelSink
EffectSink
```

Do not start with a generic graph mutation sink. It is too powerful and too hard to validate.

## Phase 1: Process-Isolated Extension Host

Build a local extension runner similar to the existing repo-local rule host, but with a different purpose.

Implementation pieces:

- discover `.polint/extensions/*/Cargo.toml`;
- build with `cargo run --manifest-path`;
- set `POLINT_CACHE_DIR`;
- use `CARGO_TARGET_DIR=.polint/cache/extensions-target`;
- perform a protocol handshake;
- run providers by phase;
- capture stdout JSON or JSONL;
- convert failures to `polint/capability` or `polint/extension` diagnostics.

This mirrors the existing `.polint/rules` operational model while keeping rule and extension semantics separate.

## Phase 2: Protocol And Fact Sinks

Use a versioned protocol:

```json
{
  "schema": "polint-extension-request-v1",
  "command": "run_provider",
  "host": {
    "polint_version": "x.y.z",
    "protocol_version": "1",
    "repo_root": "."
  },
  "provider": "fastify-entrypoints",
  "inputs": {
    "symbols": "...",
    "references": "...",
    "calls": "..."
  }
}
```

Responses:

```json
{
  "schema": "polint-extension-response-v1",
  "status": "ok",
  "facts": [],
  "diagnostics": [],
  "metrics": {
    "runtime_ms": 12,
    "emitted_facts": 8
  }
}
```

JSON is easiest to debug first. If performance requires it later, add a binary format under the same semantic protocol.

## Phase 3: Validation And Merge

All emitted facts go through validation:

- every referenced file ID exists;
- every referenced symbol/callsite exists;
- synthetic symbols have stable keys and source evidence;
- spans are valid;
- access paths are valid;
- output fact family matches declared provider output;
- confidence/precision is present;
- facts are deterministic after sorting.

Only accepted facts enter the analysis database.

## Phase 4: Extension-Aware Capability Planning

Enhance capability planning:

```text
rules request facts
  -> planner asks which native analyzers and extension providers can supply them
  -> unsupported facts produce capability diagnostics
  -> extension setup gaps block dependent rules
```

The current `CapabilitySupportView` is the right seed. Extend it so rows can say:

- native supported;
- extension supported;
- extension failed;
- unsupported;
- setup missing;
- validation failed.

## Phase 5: Extension Test Harness

Add a command:

```bash
polint extension test
```

The fixture format should assert facts, not only diagnostics:

```toml
[[expected.entrypoints]]
symbol = "src/server.ts#handler:getUser"
kind = "http_route"
route = "GET /users/:id"

[[unexpected.call_edges]]
from = "src/server.ts#call:dynamic"
to = "src/admin.ts#deleteAll"
```

Also add a delta report:

```bash
polint extension diff --extension repo-fastify-model
```

It should show:

- new resolved call edges;
- removed unresolved calls;
- new entrypoints;
- new source/sink models;
- new data-flow paths;
- runtime delta;
- validation failures.

## Phase 6: Agent Workflow

The agent workflow should be explicit:

```text
polint explain unknowns
  -> agent inspects repo conventions
  -> agent scaffolds extension
  -> agent writes providers
  -> polint extension test
  -> polint extension diff
  -> activate extension in CI
```

The extension is code-reviewed Rust, not hidden prompt state.

## Phase 7: Optional Trusted In-Process Runtime

Only after the process protocol is stable, consider an in-process runtime for built-in or trusted extensions. This can optimize hot paths but must not be the first public extension mechanism.

## Acceptance Criteria

The first implementation is successful when:

- a repo-local extension can add an HTTP entrypoint fact;
- a rule can consume the resulting `Entrypoints<'_>` view;
- extension failure does not crash polint;
- emitted facts are provenance-labeled;
- cache keys change when extension code changes;
- fixture tests can assert expected and unexpected facts;
- `polint extension diff` shows default vs extended analysis delta.
