# Phase 45 Pattern Map

**Generated:** 2026-05-31
**Phase:** 45 - JS/TS Inventory, Scope, Bindings, Module Graph & Direct Calls

## Closest Analogs

| New Area | Closest Existing Analog | Why It Matters |
|----------|-------------------------|----------------|
| `crates/polint/src/ts/inventory/` | `crates/polint/src/ts/adapter.rs` | Existing Oxc parser, `SourceType`, span conversion, function/class/call/import extraction, TS syntax cache. |
| `crates/polint/src/ts/scope/` | `crates/polint/src/symbol_graph/ts.rs` | Existing `oxc_semantic::SemanticBuilder`, scopes, references, aliases, imports, exports, stable keys. |
| TS module binding bridge | `crates/polint/src/module_graph/ts.rs` and `crates/polint/src/module_graph/model.rs` | Existing `oxc_resolver`, package/workspace/tsconfig alias handling, module nodes and resolved imports. |
| Semantic constraint projection | `crates/polint/src/analysis/semantic_graph/build.rs` and `constraints.rs` | Existing `CopyEdge` / `CallConstraint` vocabulary, stable-key recipe, normalized graph output. |
| Provider/cache/validation wiring | `crates/polint/src/analysis/semantic_graph/provider.rs`, `cache_key.rs`, `validate.rs` | Existing derived provider, digest, validation, and store patterns from Phase 44. |
| Public-boundary proof | `crates/polint/tests/public_surface_leak.rs` | v1.3 leak gate; new TS inventory/scope types must remain private. |

## Reusable Patterns

- Oxc span conversion is centralized through `span_from_byte_range` / `span_from_oxc`; new inventory code should not hand-roll line/column math.
- Oxc semantic extraction in `symbol_graph::ts` already sorts files and semantic rows deterministically. New scope/binding facts should follow the same stable-key and sorted-output discipline.
- Module graph code already owns ESM/CJS/tsconfig/package resolution; new binding code should consume `ResolvedImportFact` and `ModuleNodeId` references rather than resolving paths again.
- Semantic graph rows are derived/projection facts. Dense IDs are assigned after stable-key sorting and should never appear in persistent digest payloads.
- Unsupported/dynamic JS behavior is represented as explicit unresolved or unsupported facts, not guessed edges.

## Integration Notes

- New private modules should be declared from `crates/polint/src/ts/mod.rs`.
- If new fact families are stored in `AnalysisDb`, add replace/store paths mirroring existing analysis families and keep visibility `pub(crate)`.
- If the planner introduces a provider for TS inventory/scope/binding, it should run after TS syntax/symbol/module graph inputs and before `polint.semantic_graph` consumes constraints, or it should be invoked by the semantic graph provider as an internal projection with explicit cache inputs.
- Every new fixture should cover both fact shape and semantic graph constraint output; direct binding is not complete until `CopyEdge` and `CallConstraint` rows are visible in a snapshot.

