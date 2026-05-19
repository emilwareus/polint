---
phase: 26-semantic-index-deepening
plan: 05
subsystem: analysis-kernel
tags: [rust, semantic-index, symbol-graph, layer-cache, stable-identity]

requires:
  - phase: 26-01
    provides: Internal semantic row contracts, AnalysisDb storage, and metadata refresh paths
  - phase: 26-04
    provides: Semantic closure, generated hooks, validation, and debug fixtures
  - phase: 24
    provides: Persistent layer cache identity, manifests, payload validation, and verified reuse counters
provides:
  - Semantic-aware symbol graph layer cache identity with provider parameter inputs
  - Symbol graph cache payloads containing normalized semantic rows
  - Cache-hit restore for symbols, definitions, references, semantic rows, and metadata refresh
  - Cold/warm stable export identity proof for TS/JS and Go symbol graph cache reuse
affects: [semantic-index, symbol-graph, analysis-kernel, layer-cache]

tech-stack:
  added: []
  patterns:
    - Semantic provider parameters are included in layer-key parameter digests
    - Symbol graph payload validation fails closed on malformed semantic rows
    - Cache restore goes through AnalysisDb replacement APIs for metadata refresh

key-files:
  created:
    - .planning/phases/26-semantic-index-deepening/26-05-SUMMARY.md
  modified:
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/symbol_graph/model.rs
    - crates/polint/src/symbol_graph/mod.rs

key-decisions:
  - "Keep semantic cache identity and payload restore crate-private under the existing symbol graph provider."
  - "Use schema symbol-graph-facts-2 for symbol graph layer payloads that include semantic_index rows."
  - "Reject malformed semantic cache payloads before reuse instead of restoring partial or placeholder semantic facts."

patterns-established:
  - "semantic_provider_parameter_digest records the enabled semantic families and closure/generated-hook parameters as deterministic provider inputs."
  - "SymbolGraphLayerPayload persists SemanticIndexOutput alongside symbol, definition, and reference rows."
  - "Warm cache restore calls replace_symbol_graph_facts and replace_semantic_index_facts so metadata is rebuilt after hits."

requirements-completed: [SAE-SEM-01]

duration: 13min
completed: 2026-05-19
---

# Phase 26 Plan 05: Semantic Cache Persistence Summary

**Semantic index rows now participate in symbol graph layer cache identity, payload persistence, validation, restore, and stable export warm-reuse proof**

## Performance

- **Duration:** 13 min
- **Started:** 2026-05-19T08:01:10Z
- **Completed:** 2026-05-19T08:14:39Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Added `semantic_provider_parameter_digest` and wired symbol graph cache identity to semantic provider parameters while preserving absent extension digests and upstream layer output digests.
- Extended `SymbolGraphLayerPayload` to schema `symbol-graph-facts-2` with `semantic_index` rows and cache-hit restore through both symbol and semantic `AnalysisDb` replacement APIs.
- Added payload validation for non-empty semantic stable keys, absolute path-like semantic fields, and conflicting duplicate stable export identities.
- Added cold/warm kernel tests proving stable export keys survive deterministic cache restore for TS/JS and Go when setup succeeds.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Semantic layer key tests** - `cf796e8` (test)
2. **Task 1 GREEN: Semantic layer key inputs** - `1ed3ef5` (feat)
3. **Task 2 RED: Semantic payload tests** - `91f948b` (test)
4. **Task 2 GREEN: Semantic payload persistence and restore** - `70f4f7b` (feat)
5. **Task 3: Stable export cache restore proof** - `76444b9` (test)
6. **Refactor: rustfmt cleanup** - `e22f0a9` (refactor)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Added semantic provider parameter digest and focused cache-key tests.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Re-exported the semantic parameter digest for symbol graph cache construction.
- `crates/polint/src/symbol_graph/model.rs` - Bumped symbol graph payload schema and added `semantic_index`.
- `crates/polint/src/symbol_graph/mod.rs` - Persisted/restored semantic rows, added validation, and added semantic cache restore tests.

## Decisions Made

- Kept the semantic cache work inside the existing `polint.symbol_graph` provider rather than adding a new provider or public surface.
- Treated absolute path-like strings in semantic path-bearing fields as invalid cache payload data.
- Used setup-aware cache tests for Go: when Go setup succeeds, stable exports must match cold/warm; otherwise setup-missing semantic evidence remains explicit.

## Deviations from Plan

None - plan scope was executed as written.

## Issues Encountered

- Task 3 proof tests passed immediately because Task 2 had already implemented the semantic restore behavior needed for warm cache reuse. The task was completed as a focused test-only proof commit.
- The acceptance grep for rule digest terms still finds pre-existing compatibility key tests in `keys.rs`; no new symbol-graph semantic layer key rule, raw path, timestamp, or transient runtime inputs were added.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib incremental::keys::symbol_graph_semantic_layer_key --locked`
- `cargo test -p polint --lib symbol_graph::semantic_layer_payload --locked`
- `cargo test -p polint --lib symbol_graph::semantic_cache_restore --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None.

## Threat Flags

None - the new cache payload trust-boundary work was covered by the plan threat model and validates schema, semantic stable keys, path-like fields, and duplicate stable export conflicts before reuse.

## Next Phase Readiness

Plan 26-06 can build on deterministic semantic cache identity and restore proof for broader semantic fixture coverage and public-boundary no-leak checks without changing the crate-private semantic surface.

## Self-Check: PASSED

- Found summary file: `.planning/phases/26-semantic-index-deepening/26-05-SUMMARY.md`
- Found commits: `cf796e8`, `1ed3ef5`, `91f948b`, `70f4f7b`, `76444b9`, `e22f0a9`

---
*Phase: 26-semantic-index-deepening*
*Completed: 2026-05-19*
