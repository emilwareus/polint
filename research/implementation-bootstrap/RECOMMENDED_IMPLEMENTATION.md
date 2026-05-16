# Recommended Implementation

## Goal

Implement the first native semantic-analysis vertical slice in Rust while
preserving polint's public API discipline and leaving room for the later
state-of-the-art analyses already researched.

The first slice should prove that polint can:

- normalize language ASTs into a semantic operation layer;
- assign stable identities to places and call sites;
- run local abstract domains;
- produce direct summaries;
- cache semantic artifacts correctly;
- accept extension-emitted candidate facts through validated sinks.

## Module Layout

Add a new internal module:

```rust
// crates/polint/src/lib.rs
pub(crate) mod analysis;
```

Recommended tree:

```text
crates/polint/src/analysis/
  mod.rs
  ids.rs
  meta.rs
  stable_key.rs
  error.rs
  store.rs
  provider.rs
  schedule.rs
  validate.rs
  cache_key.rs
  mir/
    mod.rs
    body.rs
    op.rs
    lower_go.rs
    lower_ts.rs
  places.rs
  calls.rs
  domains/
    mod.rs
    lattice.rs
    reachability.rs
    constants.rs
    nullish.rs
    truthiness.rs
    product.rs
  summaries/
    mod.rs
    key.rs
    direct.rs
    store.rs
  extensions/
    mod.rs
    sink.rs
    merge.rs
    manifest.rs
```

Keep every item `pub(crate)` unless it must cross the public SDK boundary.

## Data Model

### IDs

Use dense IDs for in-memory handles:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirBodyId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct MirOpId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct PlaceId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct CallSiteId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct SummaryId(pub u64);
```

Use stable keys for cache/provenance:

```rust
pub(crate) struct StableFactKey(String);
pub(crate) struct StableKeyParts<'a> {
    family: &'static str,
    language: Language,
    file_key: &'a str,
    owner_key: Option<&'a str>,
    span: Option<&'a Span>,
    local_key: &'a str,
}
```

Do not use dense IDs as persistent identity for summaries or extension facts.

### Metadata

Use sidecar metadata instead of bloating every fact:

```rust
pub(crate) struct FactMeta {
    pub(crate) stable_key: StableFactKey,
    pub(crate) producer: Producer,
    pub(crate) status: FactStatus,
    pub(crate) precision: Precision,
    pub(crate) confidence: Confidence,
    pub(crate) evidence: EvidenceId,
}
```

Attach metadata by fact family:

```rust
pub(crate) struct FactStore<T, Id> {
    facts: Vec<T>,
    meta: Vec<FactMeta>,
    // indexes live in family-specific stores
}
```

Do not make public SDK rules handle `FactMeta` directly at first. Query views can
later expose precision/status methods.

### Semantic Store

Add one semantic owner:

```rust
pub(crate) struct SemanticStore {
    mir: MirStore,
    places: PlaceStore,
    calls: CallStore,
    domains: DomainStore,
    summaries: SummaryStore,
}
```

`AnalysisDb` can own:

```rust
semantic: Option<SemanticStore>
```

or, if that creates too much churn, expose a separate `AnalysisSession` that
borrows `AnalysisDb` and owns semantic artifacts during a run. Prefer
`AnalysisDb` ownership only if cache/reporting integration remains simple.

## MIR Contract

MIR must be polint-owned and language-normalized. It must not borrow from
tree-sitter or Oxc AST lifetimes.

First operation subset:

```rust
pub(crate) enum MirOpKind {
    Assign { dst: PlaceId, src: Rvalue },
    Read { place: PlaceId },
    Call { site: CallSiteId },
    Return { value: Option<Operand> },
    Branch { condition: Operand },
    Literal { value: LiteralValue },
    Unsupported { reason: UnsupportedReason },
}
```

First terminator subset:

```rust
pub(crate) enum Terminator {
    Goto(BlockId),
    If { condition: Operand, then_block: BlockId, else_block: BlockId },
    Return,
    Unreachable,
    Unsupported,
}
```

Rules:

- every function with a known body gets a `MirBody`;
- unsupported constructs become explicit `Unsupported` facts;
- MIR spans always point to source facts;
- lowering is per-file and deterministic;
- no parser AST references escape the lowering function.

## Place Identity

Use a language-normalized access path:

```rust
pub(crate) enum PlaceRoot {
    Local { function: FunctionId, name: String },
    Parameter { function: FunctionId, index: u32, name: Option<String> },
    Global { symbol: Option<SymbolId>, name: String },
    Temporary { body: MirBodyId, ordinal: u32 },
    Unknown,
}

pub(crate) enum Projection {
    Field(String),
    Index(Option<ConstValue>),
    Deref,
    AwaitResult,
    CallReturn(CallSiteId),
}

pub(crate) struct PlaceKey {
    root: PlaceRoot,
    projections: Vec<Projection>,
}
```

Start deterministic and simple. Do not add points-to precision to `PlaceKey`
itself; points-to facts refine relationships between places later.

## Direct Call Facts

Replace the semantic role of `FunctionFact.calls` with:

```rust
pub(crate) struct CallSiteFact {
    id: CallSiteId,
    caller: FunctionId,
    body: MirBodyId,
    op: MirOpId,
    span: Span,
    callee_syntax: String,
    kind: CallKind,
    receiver: Option<PlaceId>,
    arguments: Vec<PlaceId>,
}

pub(crate) struct DirectCallTargetFact {
    call_site: CallSiteId,
    target: Option<FunctionId>,
    target_symbol: Option<SymbolId>,
    status: ResolutionStatus,
    reason: Option<UnresolvedCallReason>,
}
```

This supports:

- direct call graph edges;
- unresolved call reporting;
- summary dependencies;
- data-flow argument/return mapping;
- agent-authored call-model extensions.

## Provider Scheduling

Start with a simple enum-backed provider DAG:

```rust
pub(crate) enum ProviderId {
    Mir,
    Places,
    DirectCalls,
    P0Domains,
    DirectSummaries,
}

pub(crate) struct ProviderSpec {
    id: ProviderId,
    requires: &'static [FactFamily],
    produces: &'static [FactFamily],
    version: &'static str,
}
```

Run providers through a deterministic topological order. Native execution can
use a `match ProviderId` initially:

```rust
match provider {
    ProviderId::Mir => mir::build(session)?,
    ProviderId::Places => places::build(session)?,
    ProviderId::DirectCalls => calls::build(session)?,
    ProviderId::P0Domains => domains::run_p0(session)?,
    ProviderId::DirectSummaries => summaries::build_direct(session)?,
}
```

This is boring, but it is debuggable and fast. Introduce trait objects only when
runtime extension loading requires it.

## Errors And Diagnostics

Use `thiserror` for kernel errors:

```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum AnalysisError {
    #[error("missing required fact family {family}")]
    MissingFactFamily { family: &'static str },

    #[error("invalid fact emitted by {provider}: {reason}")]
    InvalidFact { provider: &'static str, reason: String },

    #[error("semantic cache entry has incompatible schema {schema}")]
    CacheSchemaMismatch { schema: String },
}
```

Convert user-facing setup/unsupported states into diagnostics, not panics.
Keep `anyhow` in CLI/runner/setup glue where broad context is more useful than
typed recovery.

## Cache Keys

Add semantic artifact keys:

```rust
pub(crate) struct AnalysisArtifactKey<'a> {
    artifact: &'static str,
    schema: &'static str,
    language: Option<Language>,
    source_digest: Option<&'a str>,
    config_digest: &'a str,
    rule_digest: &'a str,
    plan_digest: &'a str,
    provider_versions: &'a [(&'static str, &'static str)],
    semantic_schema: &'static str,
    dependency_digests: &'a [&'a str],
    extension_manifest_digest: Option<&'a str>,
}
```

First artifacts:

- `mir-body`;
- `place-index`;
- `direct-calls`;
- `p0-domain-state`;
- `direct-summary`.

Cache invalidation must include:

- source content hash;
- language lifecycle inputs;
- config hash;
- rule/options hash where analysis is rule-requested;
- plan digest;
- provider version;
- semantic schema version;
- domain version/reduction graph/widening policy;
- extension manifest digest;
- dependency summary digests for summaries.

## Extension Sinks

Do not implement dynamic extension loading first. Implement the typed sink and
merge validator first.

Initial sinks:

```rust
pub(crate) struct ExtensionSinks<'a> {
    calls: &'a mut CallModelSink,
    summaries: &'a mut SummaryModelSink,
}
```

Validation rules:

- extension facts must reference existing stable keys or explicitly create
  synthetic stable keys;
- extension facts cannot silently replace native exact facts;
- suppressive facts require a conflict/review status;
- every accepted fact carries extension provenance;
- rejected facts create diagnostics with evidence.

This gives agents a target contract without committing to loading mechanics.

## Tests Before Public Views

Before exposing any new fact view:

- unit tests for stable key determinism;
- builder tests for duplicate/collision diagnostics;
- provider DAG ordering tests;
- cache digest regression tests;
- MIR fixture snapshots;
- direct call snapshots with unresolved reasons;
- domain lattice law tests with `proptest`;
- summary dependency invalidation tests;
- extension merge accept/reject/conflict tests;
- temp-repo test using only public SDK imports after view promotion.

## Public SDK Promotion Rule

Public SDK views are allowed only when all are true:

1. internal fact family has stable IDs and metadata;
2. docs exist under `docs/facts/`;
3. unsupported and heuristic behavior is documented;
4. fixture snapshots are deterministic;
5. cache digest tests exist;
6. an external-rule temp repo consumes the view through `polint::sdk::prelude::*`;
7. capability diagnostics are correct when the view is unavailable.

## Sequencing

Recommended coding order:

1. `analysis::{ids, meta, stable_key, error}`.
2. `analysis::store::SemanticStore` behind internal API.
3. provider registry and scheduler, with tests only.
4. MIR lowering for one Go fixture and one TS fixture.
5. place extraction.
6. direct call extraction.
7. P0 local domains.
8. direct summaries.
9. semantic artifact cache keys.
10. extension sink/merge validation.
11. docs and debug output.
12. only then public SDK views.

## Non-Goals For The First PR

- no full call graph solver;
- no IFDS/IDE solver;
- no points-to solver;
- no external extension loading;
- no public semantic SDK view;
- no dependency on Salsa or Datalog engines;
- no performance micro-optimization before baseline benchmarks;
- no `unsafe`.
