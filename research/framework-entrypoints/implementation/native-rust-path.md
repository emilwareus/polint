# Native Rust Implementation Path

## Core Claim

polint can support full framework boundary recovery natively without embedding external analyzers. The implementation should borrow model shapes and algorithms from CodeQL, Pysa, FlowDroid, F4F, AutoWeb, Semgrep, and MCP-BiFlow, but it should not depend on those tools at runtime.

## Internal Modules

Suggested internal modules:

```text
crates/polint/src/kernel/
  provider.rs
  layers.rs
  provenance.rs
  validation.rs
  merge.rs

crates/polint/src/framework/
  facts.rs
  provider.rs
  go.rs
  ts.rs
  mcp.rs
  validate.rs

crates/polint/src/sdk/facts/entrypoints.rs
```

Keep these `pub(crate)` until the SDK view is ready. The public surface is `polint::sdk`.

## Provider Trait Sketch

```rust
pub(crate) trait AnalysisProvider {
    fn manifest(&self) -> ProviderManifest;
    fn run(&self, ctx: ProviderCtx<'_>, out: ProviderOutput<'_>) -> ProviderResult;
}

pub(crate) struct ProviderManifest {
    pub id: ProviderId,
    pub version: ProviderVersion,
    pub schema: SchemaVersion,
    pub inputs: Vec<FactFamily>,
    pub outputs: Vec<FactFamily>,
    pub language_scope: Vec<Language>,
    pub deterministic: bool,
    pub precision_ceiling: Precision,
    pub cache_inputs: Vec<CacheInputKind>,
}
```

Repo-local providers can use a serialized handshake with the same manifest shape.

## Sink API Sketch

```rust
pub struct EntrypointSink<'a> {
    inner: &'a mut ProviderOutputBuffer,
}

impl<'a> EntrypointSink<'a> {
    pub fn http_route(&mut self, spec: HttpRouteSpec) -> Result<(), EmitError>;
    pub fn mcp_tool(&mut self, spec: McpToolSpec) -> Result<(), EmitError>;
    pub fn cli_command(&mut self, spec: CliCommandSpec) -> Result<(), EmitError>;
    pub fn lifecycle_callback(&mut self, spec: LifecycleCallbackSpec) -> Result<(), EmitError>;
    pub fn unresolved(&mut self, spec: UnresolvedFrameworkSpec) -> Result<(), EmitError>;
}
```

The sink should attach provider metadata automatically. Providers should not manually forge provenance.

## Stable Keys

Stable keys should include semantic identity plus source anchors:

```text
entrypoint:<language>:<framework>:<kind>:<target_symbol>:<registration_anchor>:<trigger_hash>
trust-boundary:<entrypoint_key>:<source_kind>:<expression_anchor>:<access_path>
dispatch-edge:<from_key>:<to_key>:<edge_kind>:<guard_hash>
```

If a target symbol is unresolved, use a synthetic key derived from registration span and expression hash, and mark status unresolved.

## Validation Rules

At minimum:

```rust
fn validate_entrypoint(fact: &EntrypointFact, db: &AnalysisDb) -> ValidationResult {
    require_span_exists(fact.registration.span)?;
    require_target_resolves_or_synthetic(fact.target, db)?;
    require_framework_known_or_extension_declared(fact.framework)?;
    require_precision_within_provider_ceiling(fact)?;
    require_trigger_metadata_matches_kind(fact)?;
    Ok(())
}
```

Extension providers cannot emit `ExactStatic` by default. They can earn `ValidatedExtension` through fixtures.

## Scheduler

Provider scheduling should be demand driven by requested SDK views:

```text
rule capabilities
  + extension provider outputs
  -> demanded fact families
  -> provider closure
  -> topological execution
  -> validation/merge after each layer
```

Entrypoints should depend on syntax/imports/symbols/references. Call graph should depend on call sites, symbols/references, and optional framework dispatch. Data flow should depend on CFG, call graph, summaries, and trust-boundary facts.

## Built-In Provider Tiers

### Go

Tier 1:

- Recognize `net/http` direct registrations.
- Track route calls in same function.
- Emit request source APIs from table.

Tier 2:

- Router variable tracking across simple helper functions.
- chi group/route callbacks.
- mux subrouters and fluent chains.
- gin/echo groups.

Tier 3:

- Middleware order and abort/next semantics.
- Framework dispatch overlay.
- Context flow facts.

### TS/JS

Tier 1:

- Recognize Express/Fastify/MCP imports and local factory calls.
- Track app/router/server variables in same module.
- Emit route/tool facts for literal registrations.

Tier 2:

- Mounted routers across imports.
- Middleware arrays and `app.route()`.
- Fastify plugin prefixes.
- MCP protocol-level dispatch specialization.

Tier 3:

- Nest decorators and DI/module composition.
- EventEmitter/callback graph.
- Next file-based routes/server actions.

## Native Extension Workflow

Agent workflow:

```text
polint check --explain-unknowns
  -> agent inspects unresolved framework facts
  -> agent creates .polint/extensions/acme_routes
  -> provider emits EntrypointFact and TrustBoundaryFact
  -> polint test-extension validates expected facts
  -> activation manifest records provider digest
  -> polint check shows default-vs-extension delta
```

Activation manifest sketch:

```toml
[[extensions]]
id = "acme.api_routes"
path = ".polint/extensions/acme_routes"
enabled = true
validated_with = ["fixtures/acme-routes.expected.json"]
precision_ceiling = "ValidatedExtension"
```

This manifest is not the model. The Rust provider is the model. The manifest controls lifecycle, validation, and cache keys.

## Public SDK Timing

Do not expose `Entrypoints<'_>` until:

- facts are populated by at least Go and TS/JS providers;
- docs describe precision and limits;
- capability planning blocks unsupported views honestly;
- JSON output can show facts for debugging;
- cache keys include provider inputs;
- extension facts are validated and merged deterministically.

## First Rule Examples

Useful rules once `Entrypoints<'_>` exists:

- "All HTTP routes under `/admin` require auth middleware."
- "MCP tool handlers must not return raw external content without a sanitizer or allowlist."
- "No route handler may be registered from a test-only module."
- "Every public API route must have an owner tag in a repo-local route model."
- "CLI commands that read env/secrets must not call network sinks."

These rules prove the product value before global taint analysis exists.
