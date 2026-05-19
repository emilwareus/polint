---
phase: 27-layered-module-package-topology-graph
plan: 04
subsystem: analysis-kernel
tags: [rust, module-graph, topology, layer-cache, cache-identity]

requires:
  - phase: 27-layered-module-package-topology-graph
    provides: Internal topology row contracts plus Go and TS/JS topology collectors from Plans 01-03
  - phase: 24-persistent-layer-cache-for-existing-cheap-facts
    provides: Module graph layer cache key, payload, dependency-index, and restore patterns
provides:
  - Base topology derivation wired into normal module graph provider runs
  - Module graph layer payload schema v2 with normalized base topology row persistence and restore
  - Topology-aware module graph cache identity and dependency-index edges for manifests, lockfiles, workspace files, source-set, and overlay inputs
affects: [module-graph, topology-facts, layer-cache, cache-invalidation, dependency-index]

tech-stack:
  added: []
  patterns:
    - Merge Go and TS/JS topology collectors before storing base topology through AnalysisDb::replace_topology_facts
    - Persist normalized internal topology rows in module graph layer payloads while keeping import-to-package edges deferred
    - Hash checked-in topology manifests and lockfiles as module graph input digests with absent extension placeholders only

key-files:
  created: []
  modified:
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/module_graph/model.rs
    - crates/polint/src/module_graph/topology.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/core/mod.rs

key-decisions:
  - "Base topology is stored by the existing polint.module_graph provider immediately after resolved imports, module nodes, and module edges are replaced."
  - "Module graph layer payload schema v2 persists base topology rows but keeps import_to_package_edges out for the later semantic-aware topology pass."
  - "Topology cache identity hashes checked-in manifest, lockfile, workspace, and tsconfig files under topology-relevant roots while preserving absent-only extension handling."

patterns-established:
  - "Repo topology overlays include D-21 categories with explicit Unknown rows for unavailable authoritative ownership, architecture, deploy, and internal/public boundary evidence."
  - "Layer-cache topology payload validation rejects duplicate stable keys before restoring cached rows into AnalysisDb."

requirements-completed: [SAE-SEM-02]

duration: 14 min
completed: 2026-05-19
---

# Phase 27 Plan 04: Module Graph Topology Wiring Summary

**Base Go and TS/JS topology rows now flow through module graph derivation, cache payloads, and topology-aware invalidation**

## Performance

- **Duration:** 14 min
- **Started:** 2026-05-19T12:37:26Z
- **Completed:** 2026-05-19T12:51:49Z
- **Tasks:** 3
- **Files modified:** 6

## Accomplishments

- Added `derive_base_topology` to merge Go and TS/JS topology collectors, generate repo overlay rows, and store the result during normal module graph provider runs.
- Upgraded module graph layer payloads to `module-graph-facts-v2`, including workspace roots, topology packages, source sets, dependency requirements, resolved dependency edges, and repo topology overlays.
- Added topology input digesting for `go.mod`, `go.work`, `go.sum`, JS package/lock/workspace files, Bun/Yarn/pnpm files, and `tsconfig.json`, plus dependency-index edges with `ShapeKind::ModuleTopology`.

## Task Commits

1. **Task 1: Merge base topology rows into module graph derivation** - `188d9e9` (test), `93a5fb4` (feat)
2. **Task 2: Persist base topology in module graph layer payloads** - `52ea03b` (test), `15c11eb` (feat)
3. **Task 3: Add topology inputs to module graph cache identity** - `bc2885c` (test), `bace158` (feat)

_Note: TDD tasks have separate red test and green implementation commits._

## Files Created/Modified

- `crates/polint/src/module_graph/mod.rs` - Base topology derivation, repo overlay collection, topology cache restore, duplicate stable-key validation, topology input dependency edges, and tests.
- `crates/polint/src/module_graph/model.rs` - Module graph layer payload schema v2 and topology row families.
- `crates/polint/src/module_graph/topology.rs` - Serializable topology rows, exact D-21 overlay kind names, `TopologyStatus::Unknown`, and `TopologyOutput::merge`.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Topology input filename set, digest row collection, and key tests.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Re-exports topology input digest helpers for provider wiring.
- `crates/polint/src/core/mod.rs` - Updated internal topology test fixture to the renamed `OwnershipZone` overlay kind.

## Decisions Made

- The existing dependency-index vocabulary already had `ShapeKind::ModuleTopology`, so no dependency-index enum change was needed; module graph now emits new edges using that shape.
- Repo overlays use exact static facts for source-set evidence such as generated/test/internal paths and explicit `Unknown` rows where authoritative overlay evidence is absent.
- Cached topology rows are restored through `AnalysisDb::replace_topology_facts`, preserving the existing normalization and metadata refresh path.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The topology module's old `dead_code` lint expectation became stale once base topology was wired into derivation and payloads; it was removed as part of the Task 2 implementation.
- Parallel cargo invocations briefly waited on package/artifact locks; all targeted reruns completed successfully.

## Known Stubs

None. Stub-scan matched only the intentional absent extension placeholder test required by D-22.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib module_graph::base_topology --locked`
- `cargo test -p polint --lib module_graph_layer_cache_topology --locked`
- `cargo test -p polint --lib module_graph_layer_key_topology_inputs --locked`
- `cargo test -p polint --lib module_graph_layer_cache_rejects_duplicate_topology_stable_keys --locked`
- `cargo test -p polint --lib module_graph_layer_key_ignores_rule_digest_changes --locked`
- D-22 extension guard grep returned no matches for activation, merge, quarantine, accepted/rejected, or conflict semantics.
- `cargo fmt --all -- --check`

## Next Phase Readiness

Plan 27-05 can build import-to-package classification on cached base topology rows, declared requirements, source-set context, and topology-aware cache invalidation.

## Self-Check: PASSED

- Found summary file: `.planning/phases/27-layered-module-package-topology-graph/27-04-SUMMARY.md`.
- Found modified files: `crates/polint/src/module_graph/mod.rs`, `crates/polint/src/module_graph/model.rs`, `crates/polint/src/module_graph/topology.rs`, `crates/polint/src/analysis_kernel/incremental/keys.rs`, `crates/polint/src/analysis_kernel/incremental/mod.rs`, `crates/polint/src/core/mod.rs`.
- Found task commits: `188d9e9`, `93a5fb4`, `52ea03b`, `15c11eb`, `bc2885c`, `bace158`.

---
*Phase: 27-layered-module-package-topology-graph*
*Completed: 2026-05-19*
