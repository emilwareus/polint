# Polint Incremental Query Engine Design

## Placement In The Existing Architecture

The incremental subsystem should live under the future private analysis module:

```text
crates/polint/src/analysis/incremental/
```

It should be consumed by providers and the analysis scheduler, not exposed
directly to rule authors.

Rules should continue to look like:

```rust
#[polint::rule]
fn rule(ctx: &mut RuleCtx<'_>, symbols: Symbols<'_>, calls: Calls<'_>) -> RuleResult {
    // rule logic
}
```

The engine decides whether `Symbols<'_>` and `Calls<'_>` came from fresh
computation, cache reuse, verified reuse, an extension provider, or a demand
query.

## Internal Contracts

### Provider Contract

Providers should become cache-aware without owning cache policy.

```rust
pub(crate) trait AnalysisProvider {
    fn id(&self) -> ProviderId;
    fn version(&self) -> ProviderVersion;
    fn output_layer(&self) -> LayerKind;

    fn collect_inputs(&self, cx: &ProviderInputCx<'_>) -> ProviderInputs;
    fn compute(&self, cx: &mut ProviderCx<'_>) -> ProviderResult;
}
```

`collect_inputs` must be deterministic and cheap. If collecting inputs requires
expensive analysis, that analysis should itself be a prior layer or query.

### Trace Contract

Every provider/query reads through a trace context.

```rust
pub(crate) struct TraceRecorder {
    edges: Vec<DependencyEdge>,
}

impl TraceRecorder {
    pub(crate) fn read_input(&mut self, input: InputKey, shape: ShapeKind);
    pub(crate) fn read_layer(&mut self, layer: LayerKey);
    pub(crate) fn read_query(&mut self, query: QueryKey);
    pub(crate) fn read_summary(&mut self, summary: SummaryKey);
    pub(crate) fn read_extension_input(&mut self, input: ExtensionInputKey);
}
```

Tracing must be low-overhead. Use dense IDs during a run and serialize stable
keys only for persisted manifests.

### Output Contract

Provider outputs need a digest that is stable across runs.

```rust
pub(crate) trait StableOutput {
    fn stable_digest(&self, hasher: &mut StableHasher);
}
```

Stable digests must not include allocation order, thread scheduling order, hash
map iteration order, or run-local IDs unless they are mapped to stable keys.

## Recommended Data Types

```rust
pub(crate) enum LayerKind {
    Source,
    Syntax,
    Imports,
    ModuleGraph,
    Symbols,
    References,
    SemanticMir,
    Cfg,
    DirectCalls,
    CallGraph,
    LocalDomains,
    Summaries,
    DataFlow,
    Alias,
    Evidence,
    Diagnostics,
}

pub(crate) enum ShapeKind {
    Text,
    Syntax,
    Imports,
    PublicApi,
    FrameworkBoundary,
    SummaryEffects,
    DiagnosticAnchors,
}

pub(crate) enum DependencyKind {
    ReadsInput,
    ReadsLayer,
    ReadsQuery,
    ReadsSummary,
    ReadsExtension,
    UsesToolInvocation,
}
```

## Persistent Cache Layout

Recommended on-disk structure:

```text
.polint/cache/
  v1/
    manifests/
      layer/
      summary/
      diagnostic/
    blobs/
      aa/
      bb/
    dependency-index/
    stats/
```

Use content-addressed blob paths:

```text
blobs/<first-two-hex>/<digest>.bin
```

Do not store absolute paths inside portable cache blobs unless they are
canonicalized relative to the repository root. Tool invocation metadata may need
absolute paths, but those should be part of a nonportable invocation digest and
marked as such.

## Provider Order For First Integration

Start with layers that already exist or are part of the semantic bootstrap:

1. source file snapshots;
2. syntax parse outputs;
3. import facts;
4. module/package topology;
5. symbol/reference facts;
6. semantic MIR;
7. direct calls;
8. local P0 domains;
9. direct summaries.

Do not start with global call graph/data-flow caches. They need the dependency
index and summary invalidation first.

## Query Families To Add First

After layer caching works:

```text
QueryKind::FunctionCfg
QueryKind::FunctionDefUse
QueryKind::DirectCallTargets
QueryKind::FunctionSummary
QueryKind::BoundedAlias
QueryKind::EvidencePath
```

These have natural small parameters and are good tests for dependency tracing.

Avoid a generic "run arbitrary graph query" public feature. Query families
should be explicit, typed, versioned, and benchmarked.

## Extension Integration

Extensions should never write directly to the cache. They produce candidate
facts through validated sinks:

```text
extension provider
  -> candidate facts
  -> schema validation
  -> referential validation
  -> precision ceiling
  -> validation fixture status
  -> merge
  -> output layer digest
  -> dependency edges include extension code/model/input digests
```

Extension outputs should be stored separately enough that native facts can be
reused when extension facts are quarantined.

Example:

```text
DirectCalls.Native
DirectCalls.Extension
DirectCalls.Merged
```

If `DirectCalls.Extension` is quarantined, the engine can still use
`DirectCalls.Native` and emit lower-precision diagnostics.

## Cache Explainability

Add internal debug output early, even if no public command is promoted:

```text
cache node: layer:Symbols
action: Verify
reason: source text changed, public API digest unchanged
dependencies checked: 142
result: Reuse
```

This is necessary for agent workflows. When an agent changes a model, it should
be able to understand why a call graph or diagnostic recomputed.

## Tests To Write

### Unit Tests

- stable digest ignores map iteration order;
- `LayerKey` canonicalizes digest lists;
- provider version changes drop cache;
- schema version changes drop cache;
- validation status participates in digest when requested;
- extension undeclared read quarantines output;
- equal recompute backdates summary.

### Integration Fixtures

- body edit preserves import/module/symbol layers;
- exported signature edit invalidates dependent references;
- `go.mod` edit invalidates Go module topology;
- `tsconfig` path alias edit invalidates TS import resolution;
- rule option edit invalidates diagnostics but not syntax;
- extension code edit quarantines extension-influenced facts;
- model file edit recomputes affected call/data-flow facts;
- summary-equal body edit does not propagate to callers;
- summary-changed body edit propagates to callers;
- diagnostic evidence path change refreshes diagnostic output.

### Benchmark Fixtures

- cold run;
- warm no-op run;
- single body edit;
- single public API edit;
- manifest/lockfile edit;
- extension model edit;
- rule option edit;
- large generated file edit;
- package boundary edit;
- summary SCC edit.

## Public SDK Implications

Do not expose cache or query mechanics first. The public SDK should expose typed
views and, later, limited metadata:

```rust
symbols.precision(symbol_id)
calls.edge_status(edge_id)
data_flow.path_provenance(path_id)
evidence.unknowns()
```

Rules should not decide whether to use a cache. The engine should make that
decision.

## Migration Strategy

1. Add types and stats with no cache behavior.
2. Wire existing providers to emit `ProviderOutputMeta`.
3. Store manifests for syntax/import/module/symbol layers.
4. Add dependency index and invalidation tests.
5. Add cache reuse for safe layers.
6. Add semantic bootstrap layers.
7. Add demand query memoization.
8. Add summary cache.
9. Add extension-aware quarantine.
10. Add diagnostic cache.

This sequence avoids changing the public API and keeps each step testable.
