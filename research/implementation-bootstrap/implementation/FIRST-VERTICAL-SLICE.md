# First Vertical Slice

This is the recommended implementation sequence.

## Phase 1: Internal Analysis Skeleton

Deliver:

- `analysis` module in `crates/polint/src/lib.rs`;
- `ids.rs`, `meta.rs`, `stable_key.rs`, `error.rs`;
- `SemanticStore` skeleton;
- unit tests for stable key determinism and ID ordering.

Gate:

- `cargo test -p polint analysis::` passes;
- no public SDK changes.

## Phase 2: Provider DAG

Deliver:

- `ProviderId`;
- `FactFamily`;
- provider dependency table;
- deterministic topological ordering;
- plan-to-provider mapping for internal semantic providers.

Gate:

- tests prove stable order independent of declaration insertion order;
- unsupported/missing provider inputs produce typed errors or diagnostics.

## Phase 3: Minimal MIR

Deliver:

- MIR body/block/op structs;
- Go lowering for one simple function fixture;
- TS/JS lowering for one simple function fixture;
- explicit unsupported operation facts.

Minimum supported syntax:

- literals;
- identifiers;
- assignments;
- returns;
- if branches;
- direct calls;
- member access as a place projection.

Gate:

- snapshot fixtures show MIR and unsupported nodes;
- no AST references escape lowering.

## Phase 4: Places

Deliver:

- `PlaceId`;
- `PlaceKey`;
- projections;
- indexes by owner function and root.

Gate:

- stable place keys are deterministic across repeated runs;
- places map back to spans/evidence.

## Phase 5: Direct Calls

Deliver:

- `CallSiteFact`;
- `DirectCallTargetFact`;
- direct/static target resolution when available;
- unresolved reason enum.

Gate:

- existing `FunctionFact.calls` remains untouched except possibly as legacy
  syntactic evidence;
- snapshots include unresolved calls rather than dropping them.

## Phase 6: P0 Domains

Deliver:

- reachability;
- constants;
- nullish/nilness;
- truthiness;
- simple reduced-product reductions.

Gate:

- lattice law tests;
- transfer tests for branch narrowing and assignment;
- no public domain SDK views.

## Phase 7: Direct Summaries

Deliver:

- `SummaryKey`;
- context-insensitive direct summary for function inputs, returns, direct calls,
  and external effects;
- dependency digest list.

Gate:

- summary invalidates when body/direct callee/domain version changes;
- recursive/SCC support can be basic but must be explicit about limits.

## Phase 8: Semantic Artifact Cache Keys

Deliver:

- deterministic artifact key encoder;
- cache schemas for MIR, places, direct calls, P0 domains, direct summaries;
- cache tests for source/config/rule/plan/provider/domain/extension changes.

Gate:

- no reliance on generic serializer ordering for keys.

## Phase 9: Extension Sinks

Deliver:

- internal sink traits/structs for call and summary model facts;
- merge validator;
- accepted/rejected/conflict diagnostics;
- test-only fake extension emitter.

Gate:

- extension facts have provenance;
- extension facts cannot silently suppress native exact facts;
- no dynamic loading yet.

## Phase 10: Debug Output And Promotion Review

Deliver:

- hidden or internal debug output for semantic facts;
- validation report against fixtures;
- decision on first public SDK view candidate.

Gate before public:

- docs under `docs/facts/`;
- temp-repo rule test;
- capability support diagnostics;
- cache digest regression;
- deterministic output tests.
