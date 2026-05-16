# Recommended Implementation: Native Rust Analysis Extensions

## Recommendation

Add a first-class **analysis extension** system separate from rules.

Rules:

- read facts;
- report diagnostics;
- stay ergonomic and simple.

Extensions:

- read facts;
- emit new facts and models;
- improve downstream call graph, data flow, effects, and rule accuracy.

Use repo-local Rust code, compiled as process-isolated executables, not config files and not dynamic libraries.

## Public Product Model

```text
.polint/rules/          -> "what policy should we enforce?"
.polint/extensions/     -> "what does this repo mean?"
```

Example:

```text
.polint/extensions/fastify-routes/
  Cargo.toml
  src/main.rs
```

The extension can tell polint:

- these functions are HTTP entrypoints;
- this callback registration calls that handler;
- this request object is untrusted input;
- this validator sanitizes this field;
- this wrapper preserves taint from argument 0 to return value;
- this generated client method performs a network call.

## Why This Is The Right Path

The user goal is max capability and tailored scan accuracy. That requires repo-specific semantics. The best agent workflow is:

```text
agent sees unknowns
  -> reads repo framework code
  -> writes Rust extension
  -> tests expected facts
  -> measures default vs extended delta
  -> commits extension with the repo
```

This turns uncertainty into code, not prompt text.

## First Vertical Slice

Implement one real vertical slice before broadening.

### Fact Family

Start with `Entrypoints`.

Why:

- entrypoints are high leverage for call graphs and data flow;
- framework defaults are often wrong;
- easy to validate against symbols and spans;
- a visible product demo.

### New SDK View

```rust
pub struct Entrypoints<'a> {
    db: &'a AnalysisDb,
}

impl<'a> Entrypoints<'a> {
    pub fn all(self) -> impl Iterator<Item = &'a EntrypointFact>;
    pub fn for_kind(self, kind: EntrypointKind) -> impl Iterator<Item = &'a EntrypointFact>;
}
```

### New Extension Sink

```rust
pub struct EntrypointSink;

impl EntrypointSink {
    pub fn entrypoint(&mut self, target: SymbolId) -> EntrypointBuilder;
}
```

### Extension Example

```rust
#[polint::provider(
    id = "repo-routes",
    inputs = [Symbols, References, Calls],
    outputs = [Entrypoints]
)]
fn repo_routes(
    ctx: &mut ExtensionCtx<'_>,
    symbols: Symbols<'_>,
    calls: Calls<'_>,
    out: &mut EntrypointSink,
) -> ExtensionResult {
    for call in calls.by_callee_name("router.get") {
        if let Some(handler) = call.argument_symbol(1) {
            out.entrypoint(handler)
                .kind(EntrypointKind::HttpRoute)
                .framework("repo-router")
                .evidence(call.span())
                .precision(Precision::ValidatedByFixture)
                .emit();
        }
    }
    Ok(())
}
```

## Protocol Runtime

Use the same operational spirit as repo-local rules:

```bash
cargo run --manifest-path .polint/extensions/<name>/Cargo.toml -- handshake
cargo run --manifest-path .polint/extensions/<name>/Cargo.toml -- run-provider <provider>
```

The host should cache builds under:

```text
.polint/cache/extensions-target/
```

Initial wire format: JSON or JSONL.

Later optimization: binary protocol, but only after semantics stabilize.

## Extension Manifest

Every extension must declare:

```rust
ExtensionManifest {
    id: "repo.fastify",
    version: "0.1.0",
    sdk_version: "0.x",
    providers: vec![
        ProviderManifest {
            id: "routes",
            phase: AnalysisPhase::Entrypoints,
            inputs: vec![FactFamily::Symbols, FactFamily::References, FactFamily::Calls],
            outputs: vec![FactFamily::Entrypoints],
            deterministic: true,
            budget: ProviderBudget::default(),
        }
    ],
}
```

## Provenance

Every emitted fact gets provenance:

```rust
FactProvenance {
    origin: FactOrigin::Extension,
    extension_id: "repo.fastify",
    provider_id: "routes",
    precision: Precision::ValidatedByFixture,
    confidence: Confidence::High,
    evidence: vec![EvidenceRef::Span(route_call_span)],
}
```

Rules should be able to inspect provenance when needed.

Reports should preserve provenance in JSON so agents can understand why a path exists.

## Capability Planning

Extend the current capability model:

```text
requested fact family
  -> native provider?
  -> extension provider?
  -> unsupported?
  -> setup missing?
  -> validation failed?
```

If a rule requests a fact family that depends on a failed extension, do not run the rule with placeholder facts. Emit a capability diagnostic.

This matches current polint discipline.

## Validation Rules

Reject facts if:

- referenced symbol does not exist;
- referenced file does not exist;
- span is outside file bounds;
- output fact family was not declared;
- precision/provenance is missing;
- synthetic symbol lacks stable key and evidence;
- access path is malformed;
- fact conflicts with native facts without explicit allowed merge policy.

## Extension Testing

Add:

```bash
polint extension test
polint extension diff
```

`test` asserts emitted facts.

`diff` shows what the extension changes:

- entrypoints added;
- call edges added;
- unresolved calls resolved;
- sources/sinks added;
- data-flow paths added;
- runtime delta;
- validation failures.

This is essential. Without a diff, extensions become magic. With a diff, agents can iterate.

## Implementation Milestones

### Milestone 1: Extension Process Host

- Discover `.polint/extensions/*/Cargo.toml`.
- Build/run each extension with cached target dir.
- Add handshake command.
- Convert failures into diagnostics.

### Milestone 2: Entrypoint Facts

- Add `EntrypointFact`, `EntrypointKind`, `Entrypoints<'_>`.
- Add `EntrypointSink`.
- Add validation and provenance.
- Add one fixture extension.

### Milestone 3: Extension-Aware Planner

- Add extension output capabilities.
- Schedule extensions before dependent rules.
- Block dependent rules if extension setup fails.

### Milestone 4: Call Graph Sinks

- Add `CallGraphSink`.
- Allow extension call edges with precision labels.
- Measure unresolved-call deltas.

### Milestone 5: Data-Flow Model Sinks

- Add `DataFlowModelSink`.
- Support sources, sinks, sanitizers, barriers, summaries, and additional flow steps.
- Use CodeQL/Pysa vocabulary but Rust builders.

### Milestone 6: Agent Tooling

- `polint extension new <name>`.
- `polint explain unknowns`.
- `polint extension diff`.
- Generated docs for extension facts and provenance.

## What Not To Do First

- Do not load arbitrary Rust dylibs into polint.
- Do not expose `AnalysisDb` mutation.
- Do not expose raw AST mutation.
- Do not make a TOML/YAML model DSL the primary surface.
- Do not let extension facts bypass validation.
- Do not support remote extension fetching before local repo extensions are solid.

## The North Star

The engine should say:

```text
I know the generic program facts.
I know where I am uncertain.
I can accept repo-local Rust extensions that make me more accurate.
I can prove what changed and where those facts came from.
```

That is how polint becomes an agent-programmable analysis engine instead of another black-box analyzer.
