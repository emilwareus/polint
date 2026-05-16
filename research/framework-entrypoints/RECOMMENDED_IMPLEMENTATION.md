# Recommended Implementation: Native Framework Boundary Layer

## Goal

Implement a full native Rust framework boundary layer that supports multiple languages and lets users or agents extend analysis accuracy with Rust code.

The goal is not a generic config-only model system. The advanced extension surface should be Rust providers that read typed fact views and emit validated facts.

```text
repo-local Rust provider
  -> typed input views
  -> narrow fact sinks
  -> kernel validation
  -> deterministic merge
  -> SDK views
```

## Non-Goals

- Do not depend on CodeQL, Pysa, FlowDroid, Semgrep, Soot, WALA, or external analyzers at runtime.
- Do not expose raw ASTs, parser internals, or `AnalysisDb` to users.
- Do not make declarative TOML/YAML config the primary extension mechanism.
- Do not put framework dispatch directly into the base call graph.
- Do not let extensions delete or suppress native facts in the first implementation.

## Architecture

Add a provider DAG layer inside the analysis kernel:

```text
SourceFiles
  -> Syntax
  -> Imports / Modules
  -> Symbols / References
  -> CallSites
  -> FrameworkComponents
  -> Entrypoints / TrustBoundaries
  -> FrameworkDispatchOverlay
  -> CallGraph / Reachability
  -> CFG / DataFlow
  -> Rules
```

Framework providers declare:

```rust
ProviderManifest {
    id: "builtin.ts.express",
    version: "0.1.0",
    schema: "entrypoints.v1",
    inputs: ["source_files", "imports", "symbols", "references"],
    outputs: ["entrypoints", "trust_boundaries", "framework_dispatch"],
    language_scope: ["typescript", "javascript"],
    deterministic: true,
    precision_ceiling: "ResolvedStatic",
    cache_inputs: ["package_json", "tsconfig", "provider_version"],
}
```

Provider output should never be inserted directly into the main database:

```text
provider output
  -> schema validation
  -> span/symbol/reference validation
  -> precision ceiling check
  -> stable key normalization
  -> deterministic merge
  -> fact layer digest
```

## Internal Fact Sketch

```rust
struct EntrypointFact {
    stable_key: StableFactKey,
    language: Language,
    framework: FrameworkId,
    kind: EntrypointKind,
    target: TargetRef,
    registration: EvidenceRef,
    trigger: TriggerMetadata,
    trust_boundary: Option<TrustBoundaryId>,
    lifecycle: Vec<LifecycleRef>,
    precision: Precision,
    confidence: Confidence,
    validation: ValidationStatus,
}

enum EntrypointKind {
    HttpRoute,
    HttpMiddleware,
    McpTool,
    McpResource,
    McpPrompt,
    CliCommand,
    Test,
    Job,
    QueueConsumer,
    ServerlessHandler,
    LifecycleCallback,
    EventListener,
    GeneratedDispatch,
}

struct TrustBoundaryFact {
    stable_key: StableFactKey,
    entrypoint: EntrypointId,
    source_kind: SourceKind,
    expression: TargetRef,
    access_path: Option<AccessPath>,
    protocol: Option<Protocol>,
    precision: Precision,
}

struct FrameworkDispatchEdgeFact {
    stable_key: StableFactKey,
    from: DispatchSource,
    to: TargetRef,
    edge_kind: FrameworkEdgeKind,
    guard: Option<RouteOrEventGuard>,
    order: Option<OrderingMetadata>,
    precision: Precision,
}
```

## Repo-Local Rust Extension Surface

Use two user-facing surfaces:

| Surface | Reads Facts | Emits Facts | Reports Diagnostics |
|---|---:|---:|---:|
| `#[polint::rule]` | Yes | No | Yes |
| `#[polint::provider]` | Yes | Yes, through sinks | Provider diagnostics only |

Provider example shape:

```rust
use polint::sdk::prelude::*;

#[polint::provider(
    id = "acme.api_routes",
    outputs(entrypoints, trust_boundaries),
    inputs(symbols, references, resolved_imports)
)]
fn acme_routes(
    ctx: &mut ProviderCtx<'_>,
    symbols: Symbols<'_>,
    refs: References<'_>,
    imports: ResolvedImports<'_>,
    out: &mut EntrypointSink<'_>,
) -> ProviderResult {
    for call in refs.calls_to("acme::router::route") {
        let Some(handler) = resolve_handler(call.arg(2), &symbols) else {
            out.unresolved(call.span(), "handler argument is dynamic");
            continue;
        };

        out.http_route(HttpRouteSpec {
            method: call.literal_arg(0),
            path: call.literal_arg(1),
            handler,
            registration: call.span(),
            precision: Precision::ResolvedStatic,
        });
    }

    Ok(())
}
```

The macro syntax can evolve. The core rule is stable: providers read typed views and emit facts through typed sinks.

## Merge Semantics

Default merge is additive set union by normalized stable key.

Rules:

- Identical facts merge provenance.
- Native exact facts cannot be deleted by extensions.
- Exact/exact conflicts produce `polint/model` diagnostics.
- Weak extension facts do not shadow native exact facts.
- Generated and agent-authored facts start as `AgentAsserted` or `GeneratedUnvalidated`.
- Sanitizers, barriers, suppressions, and negative facts require stricter validation than entrypoints.
- Merge order is deterministic by layer, provider id, provider version, stable key.

## Cache Keys

Layer cache keys must be smaller and more precise than the current whole-plan parser cache.

```text
entrypoint layer key =
    symbol/reference layer digest
  + framework provider id/version/schema
  + provider source digest for repo-local Rust provider
  + language lifecycle digest
  + relevant manifests and config digests
  + model activation manifest digest
  + absence dependencies
```

Examples of absence dependencies:

- No `package.json` workspace found.
- No `tsconfig.json` matched this file.
- No `go.mod` under selected module roots.
- No `.polint/extensions/acme_routes` provider enabled.
- No route file matching `apps/**/routes.ts`.

Absence can affect results, so it must affect caches.

## First Vertical Slice

### Phase 1: Internal Facts And SDK View

Add internal facts:

- `EntrypointFact`
- `TrustBoundaryFact`
- `FrameworkDispatchEdgeFact`
- `UnresolvedFrameworkFact`

Add SDK view:

- `Entrypoints<'_>`

Expose query methods:

```rust
entrypoints.all()
entrypoints.by_kind(EntrypointKind::HttpRoute)
entrypoints.for_file(path)
entrypoints.routes()
entrypoints.trust_boundaries()
entrypoints.unresolved()
```

Document limits under `docs/facts/entrypoints.md`.

### Phase 2: Built-In Go Recognizers

Implement syntactic/high-confidence models for:

- `net/http`: `Handle`, `HandleFunc`, `ServeMux`, `ListenAndServe`, `http.Server{Handler}`.
- chi: `Use`, `With`, `Group`, `Route`, `Mount`, verbs, `URLParam`.
- gin: `Engine`, `RouterGroup`, `Use`, `Group`, verbs, handler chain, `Param`, `Query`, `Bind`.
- echo: `Pre`, `Use`, `Group`, verbs, route middleware, `Param`, `QueryParam`, `Bind`.
- gorilla/mux: `HandleFunc`, fluent `Methods/Path/PathPrefix`, `Subrouter`, `Vars`.

Start with Tier 1 and selected Tier 2. Mark dynamic wrappers as unresolved.

### Phase 3: Built-In TS/JS Recognizers

Implement syntactic/high-confidence models for:

- Express: `express()`, `Router()`, `app.use`, `router.use`, verbs, `all`, `route`, middleware arrays.
- Fastify: `fastify()`, `route`, verb shorthands, `register` prefix, `addHook`.
- MCP TypeScript SDK: `registerTool`, `tool`, `registerResource`, `registerPrompt`, `setRequestHandler("tools/call")`.
- Nest: decorator metadata where Oxc can recover class/method/parameter decorators with import identity.

Do not attempt complete JS points-to first. Use import identity, local aliases, shallow access paths, and explicit unknowns.

### Phase 4: Repo-Local Provider Prototype

Support `.polint/extensions/<name>` Rust provider crates through a process-isolated handshake.

First sink:

- `EntrypointSink`

Provider fixture command:

```text
polint test-extension .polint/extensions/acme_routes
```

Expected output:

- JSON expected/observed facts.
- Unknown reduction.
- Added facts by precision.
- Cache digest report.
- Validation diagnostics.

### Phase 5: Call Graph And Data Flow Integration

Call graph consumes `FrameworkDispatchEdgeFact` only when requested:

```text
SyntheticRoot
  -> EntrypointFact
  -> FrameworkDispatchEdgeFact
  -> Handler symbol
```

Data flow consumes `TrustBoundaryFact` and `FrameworkSourceFact` only when requested:

```text
HTTP request body
MCP request args
CLI args/env/stdin
queue payload
external resource return
```

Rules can request just `Entrypoints<'_>` without paying for full call graph/data flow.

## Accuracy Policy

Each recognizer must emit:

- high-confidence facts where evidence is direct;
- unresolved facts where evidence is incomplete;
- provider diagnostics where setup is missing;
- precision labels for every fact;
- no fake placeholder facts.

Do not report "all routes discovered" unless validation proves the claim for that fixture. Prefer:

```text
polint recovered 31 high-confidence routes, 4 dynamic route registrations, and 2 unresolved handler wrappers.
```

## Why This Avoids Building Into Corners

- Entrypoints are independently valuable.
- The facts feed call graph and data flow later without forcing either algorithm now.
- Repo-local Rust providers prove the extension model on a constrained output family.
- Validation/merge/cache logic gets exercised before sanitizer or global data-flow facts can create serious false negatives.
- The public SDK remains typed and stable while internal provider machinery can evolve.
