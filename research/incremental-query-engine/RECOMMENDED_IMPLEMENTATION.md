# Recommended Implementation Path

## Goal

Implement a native incremental substrate that can support:

- fast repeated agent edits;
- reliable cache reuse;
- explicit invalidation for files, configs, lifecycle inputs, rules, models, and
  Rust extensions;
- demand queries for expensive graph/data-flow/alias/evidence facts;
- future red-green daemon mode;
- future relation/fixpoint sub-engine.

The first version should be conservative. It is better to recompute too much
than to reuse a stale fact. Precision can improve after cache behavior is
measured.

## Non-Goals For The First Version

- Do not expose query internals as the public SDK.
- Do not adopt Salsa as a hard dependency for the entire kernel.
- Do not build a Datalog database as the cache.
- Do not require a daemon/watch server for correctness.
- Do not try to incrementally update every recursive relation in v1.
- Do not trust extension outputs without declared inputs and validation status.

## Module Layout

Add:

```text
crates/polint/src/analysis/incremental/
  mod.rs
  digest.rs
  input_snapshot.rs
  keys.rs
  change_set.rs
  dependency_index.rs
  invalidation.rs
  layer_cache.rs
  query.rs
  query_trace.rs
  summary_cache.rs
  diagnostic_cache.rs
  extension_inputs.rs
  stats.rs
```

Keep it `pub(crate)` until the rule SDK has a deliberate public query story.

## Phase 0: Cache Vocabulary And Instrumentation

Add types before changing provider behavior.

```rust
pub(crate) struct Digest([u8; 32]);

pub(crate) enum DigestKind {
    SourceText,
    SyntaxShape,
    ImportShape,
    PublicApiShape,
    SummaryShape,
    ProviderOutput,
    RuleOptions,
    ExtensionCode,
    ModelFile,
    ToolInvocation,
}

pub(crate) struct CacheStats {
    pub(crate) hits: u64,
    pub(crate) misses: u64,
    pub(crate) verified_reuse: u64,
    pub(crate) recomputes: u64,
    pub(crate) quarantines: u64,
}
```

Every provider should be able to emit:

```rust
pub(crate) struct ProviderOutputMeta {
    pub(crate) provider_id: ProviderId,
    pub(crate) provider_version: ProviderVersion,
    pub(crate) schema_version: SchemaVersion,
    pub(crate) output_digest: Digest,
    pub(crate) precision: Precision,
    pub(crate) validation: ValidationStatus,
    pub(crate) dependencies: Vec<DependencyEdge>,
    pub(crate) stats: CacheStats,
}
```

This phase can ship with no persistent reuse yet. The point is to make cache
inputs visible.

## Phase 1: InputSnapshot

Create a canonical snapshot at the start of each run.

```rust
pub(crate) struct InputSnapshot {
    pub(crate) run_id: RunId,
    pub(crate) root: RepoRoot,
    pub(crate) files: FileSnapshotTable,
    pub(crate) config: ConfigSnapshot,
    pub(crate) language_lifecycle: LanguageLifecycleSnapshot,
    pub(crate) toolchains: ToolchainSnapshot,
    pub(crate) rules: RuleSnapshot,
    pub(crate) extensions: ExtensionSnapshot,
    pub(crate) models: ModelSnapshot,
}
```

File records should include:

```rust
pub(crate) struct FileSnapshot {
    pub(crate) file_key: StableFileKey,
    pub(crate) path: NormalizedPath,
    pub(crate) language: Option<Language>,
    pub(crate) text_digest: Digest,
    pub(crate) size_bytes: u64,
    pub(crate) mtime_hint: Option<SystemTime>,
    pub(crate) overlay_kind: OverlayKind,
}
```

Do not use mtime as proof of content equality. It is only a discovery hint.

Language lifecycle inputs should include:

- Go: module roots, `go.mod`, `go.sum`, `go.work`, build tags, include-tests
  flag, package patterns, Go version, relevant environment policy.
- TypeScript/JavaScript: `tsconfig`/`jsconfig`, package manifests, lockfiles,
  resolver options, module kind, target, path aliases, source set membership.
- Python: package roots, `pyproject.toml`, lockfiles, virtualenv/interpreter
  identity when used, import path policy.
- Java/JVM: classpath/module path, build files, source sets, Java version,
  annotation processor/generated-source policy.

Official language tools may be invoked later, but their invocation digest and
tool version must become provider inputs.

## Phase 2: Key Types

Define typed keys. Do not use stringly typed global hashes internally.

```rust
pub(crate) struct LayerKey {
    pub(crate) layer: LayerKind,
    pub(crate) provider: ProviderId,
    pub(crate) provider_version: ProviderVersion,
    pub(crate) schema_version: SchemaVersion,
    pub(crate) params_digest: Digest,
    pub(crate) lifecycle_digest: Digest,
    pub(crate) config_digest: Digest,
    pub(crate) toolchain_digest: Option<Digest>,
    pub(crate) input_digests: Vec<Digest>,
    pub(crate) dependency_layer_digests: Vec<Digest>,
    pub(crate) extension_digests: Vec<Digest>,
}

pub(crate) struct QueryKey {
    pub(crate) query_kind: QueryKind,
    pub(crate) query_version: QueryVersion,
    pub(crate) params_digest: Digest,
    pub(crate) layer_digests: Vec<Digest>,
    pub(crate) budget_digest: Digest,
    pub(crate) precision_tier: PrecisionTier,
}

pub(crate) struct SummaryKey {
    pub(crate) callable: CallableStableKey,
    pub(crate) summary_domain: SummaryDomainId,
    pub(crate) summary_version: SummaryVersion,
    pub(crate) body_shape_digest: Digest,
    pub(crate) dependency_summary_digest: Digest,
    pub(crate) extension_digest: Option<Digest>,
}

pub(crate) struct DiagnosticKey {
    pub(crate) rule: RuleId,
    pub(crate) rule_version: RuleVersion,
    pub(crate) rule_code_digest: Digest,
    pub(crate) options_digest: Digest,
    pub(crate) requested_view_digests: Vec<Digest>,
    pub(crate) evidence_digest: Option<Digest>,
}
```

Use canonical sorting for variable-length digest lists. A cache key must not
depend on nondeterministic traversal order.

## Phase 3: Persistent Layer Cache

Start with layer caching, not arbitrary query caching.

```rust
pub(crate) struct LayerCacheManifest {
    pub(crate) key: LayerKey,
    pub(crate) output_digest: Digest,
    pub(crate) created_by_polint: Version,
    pub(crate) dependencies: Vec<DependencyEdge>,
    pub(crate) precision: Precision,
    pub(crate) validation: ValidationStatus,
    pub(crate) warnings: Vec<CacheWarning>,
    pub(crate) stats: LayerStats,
}
```

Use content-addressed blobs for large payloads and atomic writes:

```text
write tmp file
fsync if configured
rename tmp -> final
write manifest last
```

The first cacheable layers should be:

1. parse/syntax facts;
2. import facts;
3. module/package topology facts;
4. symbol/reference facts;
5. semantic bootstrap facts after the bootstrap lands.

Do not cache diagnostics first. Diagnostics are downstream products and are
harder to validate correctly until fact-layer digests are stable.

## Phase 4: DependencyIndex

Record dependencies while providers run.

```rust
pub(crate) struct DependencyEdge {
    pub(crate) from: CacheNode,
    pub(crate) to: CacheNode,
    pub(crate) kind: DependencyKind,
    pub(crate) required_shape: ShapeKind,
}

pub(crate) enum CacheNode {
    Input(InputKey),
    Layer(LayerKey),
    Query(QueryKey),
    Summary(SummaryKey),
    Diagnostic(DiagnosticKey),
    Extension(ExtensionKey),
    ToolInvocation(ToolInvocationKey),
}
```

Persist reverse edges for cached layers and summaries:

```rust
pub(crate) struct DependencyIndex {
    forward: Map<CacheNode, Vec<DependencyEdge>>,
    reverse: Map<CacheNode, Vec<DependencyEdge>>,
}
```

This index should be versioned. If its schema changes, rebuild it instead of
trying to migrate aggressively.

## Phase 5: ChangeSet Classification

Classify edits by semantic impact.

```rust
pub(crate) enum ChangeKind {
    ContentOnly,
    SyntaxShape,
    ImportShape,
    PublicApiShape,
    ModuleTopology,
    Lifecycle,
    Toolchain,
    RuleCode,
    RuleOptions,
    ExtensionCode,
    ExtensionDeclaredInput,
    ModelFile,
    ProviderVersion,
    Unknown,
}
```

Example:

```text
function body edit
  ContentOnly + maybe SummaryShape

changed exported function signature
  ContentOnly + SyntaxShape + PublicApiShape + SummaryShape

changed go.mod
  Lifecycle + ModuleTopology

changed .polint extension crate
  ExtensionCode + dependent facts quarantine/recompute
```

When classification is uncertain, choose a broader change kind.

## Phase 6: Invalidation Planner

Turn a `ChangeSet` into an `InvalidationPlan`.

```rust
pub(crate) enum InvalidationAction {
    Reuse(CacheNode),
    Verify(CacheNode, VerifyReason),
    Recompute(CacheNode, RecomputeReason),
    Drop(CacheNode, DropReason),
    Quarantine(CacheNode, QuarantineReason),
}

pub(crate) struct InvalidationPlan {
    pub(crate) actions: Vec<InvalidationAction>,
    pub(crate) affected_nodes: Vec<CacheNode>,
    pub(crate) stats: InvalidationStats,
}
```

Rules:

- source text change invalidates text-dependent layers for that file;
- unchanged import shape should preserve module topology;
- unchanged public API shape should preserve dependent symbol resolution where
  language rules allow it;
- lifecycle changes invalidate language-owned layers for affected roots;
- rule option changes invalidate only diagnostics/evidence that use the rule;
- extension code changes quarantine facts emitted or influenced by that
  extension until validation passes;
- provider/schema changes drop old outputs.

## Phase 7: Demand Query Engine

Add a small native query engine for expensive facts.

```rust
pub(crate) trait Query {
    type Params;
    type Output;

    const KIND: QueryKind;
    const VERSION: QueryVersion;

    fn key(cx: &QueryCx<'_>, params: &Self::Params) -> QueryKey;
    fn execute(cx: &mut QueryCx<'_>, params: Self::Params) -> QueryResult<Self::Output>;
}
```

The query context records reads:

```rust
impl QueryCx<'_> {
    pub(crate) fn read_layer(&mut self, key: LayerKey) -> LayerRef;
    pub(crate) fn read_query<Q: Query>(&mut self, params: Q::Params) -> Q::Output;
    pub(crate) fn read_summary(&mut self, key: SummaryKey) -> SummaryRef;
    pub(crate) fn read_extension_input(&mut self, key: ExtensionInputKey);
}
```

First query families:

- one function CFG/control-dependence view;
- one function def-use/data-dependence view;
- direct call target query;
- one function direct summary;
- evidence/path query for one diagnostic;
- bounded alias query for one place pair.

Persist query results only after measuring reuse value. In-run memoization is
useful immediately.

## Phase 8: Summary SCC Cache

Summaries are the scaling boundary, so they need special handling.

First version:

```text
build call/SCC graph
find affected functions
expand to affected SCC closure
recompute summaries for affected SCCs
compare old/new summary digests
backdate equal summaries
propagate only changed summary digests to callers
```

Later version:

```text
for monotone IDFA domains
  use IncIDFA-like update inside affected SCC
  avoid resetting all facts to bottom/top
```

Summary equality must include:

- domain payload digest;
- precision;
- validation status;
- extension/model provenance relevant to downstream consumers;
- timeout/budget status;
- unknown facts.

## Phase 9: Extension-Aware Caching

Add extension input tracking before allowing extensions to influence expensive
analysis layers.

```rust
pub(crate) struct ExtensionSnapshot {
    pub(crate) crate_digest: Digest,
    pub(crate) api_version: ApiVersion,
    pub(crate) declared_outputs: Vec<LayerKind>,
    pub(crate) declared_inputs: Vec<ExtensionInputSpec>,
    pub(crate) model_files: Vec<ModelFileSnapshot>,
    pub(crate) validation_digest: Digest,
    pub(crate) precision_ceiling: Precision,
}
```

Policy:

- declared input changed: recompute affected extension output;
- extension code changed: quarantine previous extension-influenced facts;
- validation fixture changed: downgrade/quarantine until rerun;
- undeclared read observed: fail closed and invalidate broad extension output;
- precision ceiling changed: invalidate facts whose precision/status depended on
  it.

## Phase 10: Diagnostic Cache

Only cache diagnostics after fact layers and query traces are reliable.

Diagnostic fingerprints should include:

```text
rule id
rule version
rule options
primary stable source anchor
semantic subject key if available
diagnostic kind
relevant fact/query digest
evidence digest when emitted
```

Diagnostics should be dropped if:

- their primary source anchor disappears;
- any required fact view is recomputed to a different digest;
- evidence path changes in a way that affects truthfulness;
- rule code/options change;
- extension validation status changes.

## Phase 11: Watch/Daemon Red-Green Mode

After batch correctness works, add revisions:

```rust
pub(crate) struct Revision(u64);

pub(crate) struct Memo<T> {
    value: T,
    dependencies: Vec<CacheNode>,
    changed_at: Revision,
    verified_at: Revision,
    durability: Durability,
}
```

Durability classes:

| Durability | Examples |
|---|---|
| Low | source text, open buffers, generated temp inputs. |
| Medium | config, manifests, lockfiles, rule options, extension/model source. |
| High | stdlib facts, vendored dependency facts, toolchain metadata. |

Provider/schema version changes override durability and force invalidation.

## Phase 12: Relation/Fixpoint Sub-Engine

Only build this when benchmarks show the need.

Candidate users:

- transitive reachability;
- context-limited call graph expansion;
- data-flow step closure;
- points-to propagation;
- summary dependency propagation;
- slice/evidence graph reachability.

Start with semi-naive deltas and explicit indexes before considering
differential dataflow. Differential-style traces are powerful, but memory cost
can be high and the implementation is nontrivial.

## Acceptance Gates

Do not promote this as a stable internal substrate until:

- cache keys are deterministic across repeated runs;
- source body edits do not invalidate unrelated module/symbol layers;
- public API edits do invalidate dependents;
- rule option edits do not invalidate parser facts;
- extension edits quarantine dependent facts;
- lifecycle changes invalidate affected language layers;
- summaries backdate equal outputs;
- cache manifests are ignored after schema/provider version changes;
- benchmark output reports hit/miss/recompute/quarantine counts;
- tests prove no cloned repos under `research/*/repos/` are committed.
