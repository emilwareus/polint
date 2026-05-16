# Native Rust Architecture Path

## Internal Modules

Recommended internal module split:

```text
analysis/
  domain/
    lattice.rs
    product.rs
    transfer.rs
    reduction.rs
    widening.rs
    validation.rs
  ir/
    op.rs
    place.rs
    predicate.rs
    cfg.rs
  solver/
    worklist.rs
    wto.rs
    results.rs
    cursor.rs
  summaries/
    key.rs
    store.rs
    projection.rs
    scc.rs
  extensions/
    manifest.rs
    sinks.rs
    validation.rs
    merge.rs
  facts/
    nilness.rs
    constants.rs
    strings.rs
    ranges.rs
    shapes.rs
    typestate.rs
```

This is a domain-oriented view of shared internal modules, not a mandate to
duplicate owners from other research tracks. `PlaceId` and abstract values
belong to the type/value/place substrate; summary keys/stores belong to the
effects-summary kernel; MIR/CFG belongs to the CFG/control-flow substrate.
Abstract interpretation contributes domain transfer, reduction, validation, and
summary payload logic over those shared IDs.

Keep this under crate-internal visibility until typed SDK views are mature.

## Core Types

```rust
pub(crate) struct AnalysisKey {
    pub language: LanguageId,
    pub package: PackageId,
    pub callable: Option<CallableId>,
    pub domain: Option<DomainId>,
    pub domain_version: Option<DomainVersion>,
    pub context: Option<ContextKey>,
    pub file_digest: Hash,
    pub semantic_digest: Hash,
    pub config_digest: Hash,
    pub setup_digest: Hash,
    pub extension_digest: Hash,
    pub dependency_digest: Hash,
}

pub(crate) struct DomainFact<T> {
    pub value: T,
    pub precision: Precision,
    pub status: FactStatus,
    pub provenance: ProvenanceId,
    pub evidence: SmallVec<[EvidenceId; 2]>,
}
```

All derived facts should have sidecar metadata. Do not bake full provenance into
each small lattice value; store compact event ids during transfer and attach
expanded provenance at fact/summary boundaries to keep transfer cheap.

Minimum provenance events:

```text
assigned
assumed
reduced
widened
narrowed
invalidated
forgotten
summary-applied
extension-declared
unsupported-semantics
budgeted-to-top
```

Events need deterministic compression so explanations are useful without
turning every join into a large provenance graph.

## Place Model

Use interned places:

```rust
pub(crate) struct Place {
    pub owner: PlaceOwner,
    pub root: PlaceRoot,
    pub projections: SmallVec<[Projection; 4]>,
    pub precision: PlacePrecision,
}

pub(crate) enum PlaceRoot {
    Local(LocalId),
    Param(u16),
    Receiver,
    Return,
    Global(SymbolId),
    Allocation(AllocId),
    Unknown,
}

pub(crate) enum Projection {
    Field(SymbolId),
    Property { key: PropKey, precision: KeyPrecision },
    Index { key: IndexKey, precision: KeyPrecision },
    Deref,
    Variant(VariantId),
    Dynamic,
}
```

This is required for initializedness, nullness, shape, typestate, alias, and
data-flow domains to agree on identity. `Local`, `Param`, `Receiver`, and
`Return` are scoped to a callable/context. Dynamic property/index projections,
unknown calls, and alias escapes must map to explicit invalidation regions.

## Extension Merge Policies

Every fact family declares a merge policy:

| Policy | Use |
|---|---|
| `Join` | Additive facts such as extra possible constants, sources, sinks. |
| `MeetForPrecision` | Guard refinements that must be stronger but still conservative. |
| `ConservativeTopOnConflict` | Conflicting value facts where safe fallback is unknown. |
| `RejectConflict` | Facts where conflict indicates invalid extension/model. |

Never use last-writer-wins.

## Cache Keys

Cache keys include:

- source file digest;
- parser and adapter version;
- semantic operation schema version;
- domain id and version;
- domain reduction graph version;
- widening policy version;
- config digest;
- rule/options digest when rule-requested domains change behavior;
- extension manifest/source/artifact/Cargo.lock/toolchain/target/features
  digests;
- extension validation digest and validation schema version;
- merge policy version;
- budget/limit config;
- external model data digests;
- language lifecycle setup digest;
- dependent summary digests.

Changing `leq`, `join`, `widen`, transfer behavior, reduction order, or merge
policy is cache-breaking. For recursive summaries, cache the converged SCC
summary identity, not intermediate iteration states.

## Validation Harness In The Kernel

Add a native validation mode:

```text
polint internal validate-domain
polint ext test
```

Validation should run:

- lattice law checks;
- transfer monotonicity samples;
- serialization round trips;
- stable digest checks;
- deterministic parallel repeated runs;
- conflict merge tests;
- cache invalidation tests.

This is not optional. Without it, agent-authored analysis extensions will be too
powerful to trust.

## Public API Discipline

Internal domain traits should not be public SDK. Expose curated typed views only
after the domain has stable semantics:

```rust
pub struct Nilness<'a> { /* opaque */ }
pub struct Constants<'a> { /* opaque */ }
pub struct StringValues<'a> { /* opaque */ }
```

Each view should expose query methods and metadata:

```rust
fn nilness(&self, place: impl IntoPlace) -> Fact<NilnessValue>;
fn precision(&self, fact: FactId) -> Precision;
fn evidence(&self, fact: FactId) -> Evidence;
```

Rules should not call other rules or inspect raw domain stores.

## First Implementation Milestone

Build one vertical slice:

```text
Go + TS/JS semantic ops
  -> CFG locations
  -> places
  -> reachability/nilness/truthiness/constants
  -> local facts
  -> direct function summaries
  -> internal typed views
  -> one temp-repo rule fixture
  -> one extension guard fixture
```

This slice proves the kernel, validation, cache, extension, and rule-facing path
without waiting for every advanced domain.
