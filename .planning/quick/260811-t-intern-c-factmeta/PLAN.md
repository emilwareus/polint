# Quick Plan: T-INTERN-C FactMeta StableKeyId

## Goal
Migrate `FactMeta.stable_key`, conflict types, and `stable_key_owners` from `String` to `StableKeyId` with no dual path. Structural gate: `stable_key: String` count in `crates/polint/src` = 0.

## Scope
- `analysis_kernel/metadata.rs` — FactMeta, StableKeyConflict, FactMetaInsert::Conflict, owner map, store/tests
- `core/metadata.rs` — fact_meta_from_parts / fact_meta_from_stable_key* / extension/adaptation/topology helpers
- Producers/consumers: core/db, validation/debug, summary/topology/calls/entrypoints/domains/refined_calls/data_flow/extensions/unknown_taxonomy, tests/fixtures
- Debug JSON: MetadataDebugFields owns `stable_key_text` with serde rename to `stable_key`
- Conflict emission: sort on resolved text (not StableKeyId Ord / BTree iteration)

## Out of scope
- Solver densification
- Public API widening / leak allowlist changes
- Golden baseline regeneration

## Validation
Focused metadata/conflict/core/validation/debug tests; cargo check/fmt/clippy; structural 0; public_surface_leak; determinism_gate; golden once (retry once for cost-only).
