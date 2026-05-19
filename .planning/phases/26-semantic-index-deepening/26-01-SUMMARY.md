---
phase: 26-semantic-index-deepening
plan: 01
subsystem: analysis-kernel
tags: [rust, semantic-index, symbol-graph, metadata, provider-manifest]

requires:
  - phase: 20-private-analysis-kernel-facade
    provides: Private provider manifests and fixed provider order
  - phase: 21-provenance-precision-and-validation-metadata
    provides: Fact metadata sidecar, stable keys, and validation vocabulary
provides:
  - Internal semantic fact contracts for scopes, semantic imports, exports, aliases, resolution rows, generated symbols, and stable export identities
  - AnalysisDb storage and metadata refresh path for semantic rows
  - Symbol graph provider manifest outputs for semantic row families
affects: [semantic-index, symbol-graph, analysis-db, metadata, provider-manifests]

tech-stack:
  added: []
  patterns: [crate-private semantic rows, deterministic stable-key sorting, metadata-backed internal facts]

key-files:
  created:
    - crates/polint/src/symbol_graph/semantic.rs
  modified:
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/analysis_kernel/provider.rs

key-decisions:
  - "Keep semantic index rows crate-private under symbol_graph::semantic with no SDK, runner, CLI, or crate-root public surface."
  - "Use polint.symbol_graph as producer/layer id for semantic metadata rows."
  - "Assign semantic run-local IDs by sorted stable keys while keeping stable keys separate from IDs."

patterns-established:
  - "Semantic rows expose crate-private computed_stable_key helpers backed by stable_key_from_parts."
  - "AnalysisDb replacement APIs own deterministic ID reassignment, index rebuilds, and metadata refresh together."

requirements-completed: [SAE-SEM-01]

duration: 12min
completed: 2026-05-19
---

# Phase 26 Plan 01: Semantic Index Contracts Summary

**Crate-private semantic index substrate with deterministic stable keys, AnalysisDb storage, metadata rows, and provider manifest outputs**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-19T05:41:52Z
- **Completed:** 2026-05-19T05:53:53Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added internal semantic fact structs, ID newtypes, status enums, generated-symbol hooks, and stable export identities under `symbol_graph::semantic`.
- Added `AnalysisDb::replace_semantic_index_facts` with deterministic stable-key ordering, run-local ID reassignment, crate-private accessors, metadata refresh, and missing-metadata detection.
- Expanded the existing `polint.symbol_graph` provider manifest to schema `symbol-graph-facts-2:2` and declared semantic outputs without changing provider order.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add crate-private semantic fact contracts** - `0cfa57d` (test), `aaf7fc5` (feat)
2. **Task 2: Store semantic rows inside AnalysisDb with metadata families** - `96485a9` (test), `ff32ca5` (feat)
3. **Task 3: Update symbol provider manifest for semantic outputs** - `a72c1fe` (test), `c511bb9` (feat)

_Note: TDD tasks have separate red test and green implementation commits._

## Files Created/Modified

- `crates/polint/src/symbol_graph/semantic.rs` - Internal semantic fact contracts, status/kind enums, stable-key helpers, and focused unit tests.
- `crates/polint/src/symbol_graph/mod.rs` - Crate-private semantic module registration.
- `crates/polint/src/core/mod.rs` - Semantic row storage, replacement API, accessors, deterministic ID/index rebuilds, metadata refresh, and storage tests.
- `crates/polint/src/analysis_kernel/metadata.rs` - Semantic `FactFamily` variants and labels.
- `crates/polint/src/analysis_kernel/provider.rs` - Symbol graph schema v2 and semantic provider output declarations.

## Decisions Made

- Kept all new semantic rows and accessors `pub(crate)` to avoid promoting a public semantic graph API.
- Reused `polint.symbol_graph` as the semantic producer/layer id because Phase 26 deepens the existing symbol graph path rather than splitting provider order.
- Mapped semantic status to metadata precision/confidence explicitly: resolved/generated rows are setup-aware high confidence, ambiguous/unresolved rows are medium confidence, dynamic/cycle/unsupported rows are low confidence, external rows are setup-aware medium confidence, and setup-missing rows are high-confidence setup-missing.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added semantic FactFamily variants during Task 1**
- **Found during:** Task 1 (semantic stable-key constructors)
- **Issue:** The planned semantic stable-key constructors had to call `stable_key_from_parts`, which requires concrete `FactFamily` variants. Waiting until Task 2 would have forced incorrect placeholder families or non-compiling constructors.
- **Fix:** Added the semantic `FactFamily` variants and labels alongside the model, then consumed them fully in Task 2 metadata storage.
- **Files modified:** `crates/polint/src/analysis_kernel/metadata.rs`, `crates/polint/src/symbol_graph/semantic.rs`
- **Verification:** `cargo test -p polint --lib symbol_graph::semantic --locked`
- **Committed in:** `aaf7fc5`

---

**Total deviations:** 1 auto-fixed (Rule 2)
**Impact on plan:** No scope expansion; the change was required for correct semantic stable-key construction.

## Issues Encountered

- Initial storage test expected human-name ordering, but the contract is stable-key ordering. The test data was adjusted to assert deterministic stable-key behavior without changing implementation semantics.

## Known Stubs

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 26-02 can build language-owned semantic producers on top of the internal row contracts and `AnalysisDb` replacement boundary added here.

## Self-Check: PASSED

- Created/modified files exist.
- Task commits found: `0cfa57d`, `aaf7fc5`, `96485a9`, `ff32ca5`, `a72c1fe`, `c511bb9`.

---
*Phase: 26-semantic-index-deepening*
*Completed: 2026-05-19*
