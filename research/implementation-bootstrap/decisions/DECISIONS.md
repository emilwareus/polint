# Decisions

## D1: Start With A Private `analysis` Module

Status: recommended.

Do not add semantic facts directly into public SDK or into root public modules.
Start with `pub(crate) mod analysis;`.

Reason: public API is a liability, and the first semantic contracts need room to
change.

## D2: Keep `AnalysisDb` As Owner, Not Implementation Dump

Status: recommended.

`AnalysisDb` may own `SemanticStore`, but semantic implementation files should
live under `analysis/`.

Reason: `core/mod.rs` is already broad. MIR/domains/summaries/extensions would
make it too coupled.

## D3: Do Not Upgrade `FunctionFact.calls`

Status: recommended.

Create `CallSiteFact` and `DirectCallTargetFact`.

Reason: string call lists cannot support provenance, unresolved reasons,
argument/return mapping, or summaries.

## D4: Dense IDs Plus Stable Keys

Status: recommended.

Use dense `Copy` IDs for runtime storage and stable keys for cache/provenance.

Reason: this matches existing Rust style and avoids treating run-local indexes
as persistent identity.

## D5: Native Provider DAG Before Query Engine

Status: recommended.

Use a deterministic provider DAG and enum/native dispatch for the bootstrap.
Delay Salsa-like or Datalog-like engines.

Reason: the first slice needs measurable fact production and invalidation, not a
large query-runtime dependency.

## D6: Typed Errors In Kernel

Status: recommended.

Use `thiserror` for `AnalysisError`; keep `anyhow` at CLI/setup boundaries.

Reason: kernel callers need to distinguish missing input, invalid fact,
unsupported semantics, cache mismatch, and extension rejection.

## D7: Extension Sinks Before Extension Loading

Status: recommended.

Implement typed sinks and merge validation before dynamic extension loading.

Reason: validation rules are the real product contract. Loading mechanics can
evolve later.

## D8: No Public Views Until Validated

Status: recommended.

Do not expose behavior behind `Cfg<'_>`, `CallGraph<'_>`, `DataFlow<'_>`, or
new domain views until docs, fixtures, cache tests, and temp-repo tests exist.

Reason: public SDK facts are support contracts.
