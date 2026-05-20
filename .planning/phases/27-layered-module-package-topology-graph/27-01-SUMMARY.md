---
phase: 27-layered-module-package-topology-graph
plan: 01
subsystem: analysis-kernel
tags: [rust, module-graph, topology, metadata, provider-manifest]

requires:
  - phase: 21-provenance-precision-and-validation-metadata
    provides: Fact metadata sidecar, stable-key ownership, and precision vocabulary
  - phase: 26-semantic-index-deepening
    provides: Semantic row storage patterns and provider manifest schema upgrades
provides:
  - Crate-private topology row contracts for roots, packages, source sets, requirements, resolved dependency edges, import-to-package edges, and overlays
  - AnalysisDb storage and replacement APIs with metadata for all topology row families
  - Module graph provider schema v2 outputs for base topology internals
affects: [module-graph, analysis-db, metadata, provider-manifests, public-boundary-tests]

tech-stack:
  added: []
  patterns:
    - Crate-private topology rows normalized by stable key with run-local ID reassignment
    - AnalysisDb replacement APIs refresh sidecar metadata per fact family
    - Provider manifests advertise internal outputs without SDK or CLI promotion

key-files:
  created:
    - crates/polint/src/module_graph/topology.rs
    - crates/polint/src/module_graph/formats/mod.rs
  modified:
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/analysis_kernel/metadata.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - docs/CAPABILITY-FULFILLMENT-RESEARCH.md
    - docs/facts/capability-plans.md
    - docs/roadmap/00_ROADMAP.md

key-decisions:
  - "Keep topology contracts crate-private under module_graph::topology with no SDK, runner, CLI, crate-root, or public docs promotion."
  - "Use polint.module_graph for base topology metadata and polint.module_topology for import-to-package metadata."
  - "Advertise only base topology outputs on the existing polint.module_graph provider; import_to_package_edges remains deferred to the later semantic-aware module topology pass."

patterns-established:
  - "TopologyOutput::normalized sorts every row family by stable_key, reassigns run-local IDs, and remaps internal topology references."
  - "AnalysisDb::replace_import_to_package_facts updates only import-to-package rows and metadata while preserving base topology rows."

requirements-completed: [SAE-SEM-02]

duration: 12 min
completed: 2026-05-19
---

# Phase 27 Plan 01: Internal Topology Contract Summary

**Crate-private topology facts with AnalysisDb storage, sidecar metadata, and module graph provider schema v2 outputs**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-19T11:44:12Z
- **Completed:** 2026-05-19T11:56:38Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments

- Added `module_graph::topology` with internal ID newtypes, seven topology row families, status/precision/kind enums, and deterministic normalization tests.
- Added `AnalysisDb` topology storage, crate-private accessors, full replacement, import-to-package-only replacement, and metadata refresh for every topology family.
- Updated `polint.module_graph` to schema `module-graph-facts-2:2` with base topology output labels while preserving the six-provider order and public API boundary.

## Task Commits

1. **Task 1: Add crate-private topology fact contracts** - `c3c08fb` (test), `0c891fe` (feat)
2. **Task 2: Store topology rows in AnalysisDb with metadata** - `9d87b73` (test), `4561c72` (feat)
3. **Task 3: Declare internal provider outputs without public promotion** - `ee33444` (test), `482600c` (feat)

_Note: TDD tasks have separate red test and green implementation commits._

## Files Created/Modified

- `crates/polint/src/module_graph/topology.rs` - Internal topology contracts, row normalization, ID remapping, and tests.
- `crates/polint/src/module_graph/formats/mod.rs` - Internal namespace for later static manifest parsers.
- `crates/polint/src/module_graph/mod.rs` - Registers crate-private topology and formats modules.
- `crates/polint/src/core/mod.rs` - Topology storage, replacement APIs, accessors, metadata refresh, and storage tests.
- `crates/polint/src/analysis_kernel/metadata.rs` - Topology `FactFamily` variants and labels.
- `crates/polint/src/analysis_kernel/provider.rs` - Module graph schema v2 outputs and provider boundary tests.
- `docs/CAPABILITY-FULFILLMENT-RESEARCH.md`, `docs/facts/capability-plans.md`, `docs/roadmap/00_ROADMAP.md` - Public-boundary wording cleanup for no-leak verification.

## Decisions Made

- Topology stays internal and crate-private; no SDK fact view, runner export, CLI command, crate-root export, or public topology docs were introduced.
- Base topology rows are attributed to `polint.module_graph`; import-to-package rows are attributed to `polint.module_topology` even though the later semantic-aware provider is not scheduled yet.
- Provider order remains `polint.source`, `polint.go.syntax`, `polint.ts.syntax`, `polint.module_graph`, `polint.symbol_graph`, `polint.metrics`.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Remapped topology references after ID reassignment**
- **Found during:** Task 2 (AnalysisDb topology replacement)
- **Issue:** Sorting rows by stable key and reassigning run-local IDs would leave package/source-set/requirement/overlay references pointing at stale IDs.
- **Fix:** `TopologyOutput::normalized` now records old-to-new ID maps and remaps internal topology references after every family is sorted.
- **Files modified:** `crates/polint/src/module_graph/topology.rs`
- **Verification:** `cargo test -p polint --lib module_graph::topology --locked`; `cargo test -p polint --lib topology_storage --locked`
- **Committed in:** `4561c72`

**2. [Rule 2 - Missing Critical] Removed existing public prose that violated topology no-leak checks**
- **Found during:** Task 3 (provider output public-boundary acceptance scan)
- **Issue:** Existing docs contained unsupported topology/fact-view wording that matched the plan's forbidden public surface scan.
- **Fix:** Removed the unsupported package fact-view entry and reworded broad "polint facts" prose without changing any public API.
- **Files modified:** `docs/CAPABILITY-FULFILLMENT-RESEARCH.md`, `docs/facts/capability-plans.md`, `docs/roadmap/00_ROADMAP.md`
- **Verification:** `rg -n "Packages<'_|Dependencies<'_|SourceSets<'_|RepoTopology<'_|polint topology|polint facts" crates/polint/src docs README.md` returned no matches.
- **Committed in:** `482600c`

---

**Total deviations:** 2 auto-fixed (2 missing critical)
**Impact on plan:** Both fixes were required to keep topology IDs internally correct and preserve the required private-only boundary. No public topology surface was added.

## Issues Encountered

- The `provider_manifests` filter did not select the new module graph topology test by name, so `module_graph_manifest` was run directly alongside the required provider manifest and provider order checks.
- Parallel Cargo verification briefly waited on package/artifact locks; reruns completed successfully.

## Known Stubs

None. Stub-scan matches were existing documentation prose using "placeholder" and an existing test fixture literal containing `{}`.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib module_graph::topology --locked`
- `cargo test -p polint --lib topology_storage --locked`
- `cargo test -p polint --lib provider_manifests --locked`
- `cargo test -p polint --lib provider_order --locked`
- `cargo fmt --all -- --check`
- Acceptance scans for crate-private topology contracts, metadata families, provider outputs, and public no-leak terms passed.

## Next Phase Readiness

Plan 27-02 can build language-specific topology collectors on the internal row contracts, AnalysisDb replacement APIs, and provider output labels added here.

## Self-Check: PASSED

- Found created files: `crates/polint/src/module_graph/topology.rs`, `crates/polint/src/module_graph/formats/mod.rs`.
- Found summary file: `.planning/phases/27-layered-module-package-topology-graph/27-01-SUMMARY.md`.
- Found task commits: `c3c08fb`, `0c891fe`, `9d87b73`, `4561c72`, `ee33444`, `482600c`.

---
*Phase: 27-layered-module-package-topology-graph*
*Completed: 2026-05-19*
