---
phase: 27-layered-module-package-topology-graph
plan: 02
subsystem: analysis-kernel
tags: [rust, module-graph, go, topology, manifests, lockfile-evidence]

requires:
  - phase: 27-layered-module-package-topology-graph
    provides: Crate-private topology contracts, AnalysisDb topology storage, and module graph provider output labels from Plan 01
provides:
  - Static go.mod parser for module, go, require, replace, and exclude directives
  - Static go.work parser for use and replace directives
  - Go topology collector for module roots, packages, source sets, declared requirements, and go.sum checksum evidence
affects: [module-graph, go-lifecycle, topology-facts, dependency-evidence, future-cache-validation]

tech-stack:
  added: []
  patterns:
    - Static Go manifest parsing with unsupported rows instead of panics
    - Go topology collection via existing GoAnalysisConfig lifecycle normalization
    - Declared requirements and lockfile evidence emitted as separate topology fact families

key-files:
  created:
    - crates/polint/src/module_graph/formats/go_mod.rs
    - crates/polint/src/module_graph/formats/go_work.rs
  modified:
    - crates/polint/src/module_graph/formats/mod.rs
    - crates/polint/src/module_graph/go.rs
    - crates/polint/src/module_graph/topology.rs

key-decisions:
  - "Go module topology reuses GoAnalysisConfig::from_loaded so configured module_roots take precedence and nearest go.mod discovery remains centralized."
  - "go.mod requirements, replace/exclude directives, and go.sum checksum rows remain separate topology facts rather than import or DependsOn edges."
  - "Missing go.sum evidence for external requirements is represented as explicit MissingLockfile topology uncertainty."

patterns-established:
  - "Go source-set rows classify each discovered Go file as source, test, generated, vendor, or setup-missing using deterministic path/content checks."
  - "Static Go parser rows preserve source labels for go.mod, go.work, and go.sum-derived evidence."

requirements-completed: [SAE-SEM-02]

duration: 14 min
completed: 2026-05-19
---

# Phase 27 Plan 02: Go Module And Dependency Topology Summary

**Go monorepo module roots, static manifests, source sets, declared requirements, and go.sum evidence as internal topology rows**

## Performance

- **Duration:** 14 min
- **Started:** 2026-05-19T12:00:25Z
- **Completed:** 2026-05-19T12:14:19Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added static `go.mod` and `go.work` parsers for the planned directive set, including unsupported rows for malformed directives and no command execution.
- Added `collect_go_topology` for repository roots, inferred/configured Go module roots, covering checked-in go.work roots, Go module/package rows, and Go file source-set rows.
- Added declared requirement facts for require/replace/exclude directives and resolved dependency edge facts for go.sum checksum evidence and missing-lockfile uncertainty.

## Task Commits

1. **Task 1: Parse Go module and workspace manifests statically** - `68c34e7` (test), `15bc733` (feat)
2. **Task 2: Collect Go workspace roots, packages, and source sets** - `8ecacbc` (test), `cfe0cf1` (feat)
3. **Task 3: Emit Go declared requirements and lock evidence** - `a168524` (test), `2a29d2e` (feat)

_Note: TDD tasks have separate red test and green implementation commits._

## Files Created/Modified

- `crates/polint/src/module_graph/formats/go_mod.rs` - Static go.mod parser, manifest row structs, block/single-line directive handling, and parser tests.
- `crates/polint/src/module_graph/formats/go_work.rs` - Static go.work parser for use/replace directives and malformed directive tests.
- `crates/polint/src/module_graph/formats/mod.rs` - Registers Go static manifest parser modules.
- `crates/polint/src/module_graph/go.rs` - Go topology collector, source-set classification, declared requirement emission, go.sum evidence parsing, and topology tests.
- `crates/polint/src/module_graph/topology.rs` - Adds internal requirement/resolved-edge/status variants needed for Go dependency topology.

## Decisions Made

- Go topology root selection follows the existing lifecycle contract by using `GoAnalysisConfig::from_loaded`; this keeps explicit `[languages.go].module_roots` ahead of inferred nearest `go.mod` roots.
- Declared dependency data is represented as `DependencyRequirementFact` rows, while checksum evidence is represented as `ResolvedDependencyEdgeFact` rows; no declared requirement is collapsed into an import edge.
- Missing `go.sum` files for external requirements become explicit `MissingLockfile` resolved-edge uncertainty with unknown precision.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The initial Task 2 test placement compiled under `module_graph::go::tests::topology`, which the plan filter did not select. The tests were moved to `module_graph::go::topology_monorepo`, which is selected by both required topology filters.
- Parallel Cargo test invocations briefly waited on package/artifact locks; all targeted reruns completed successfully.

## Known Stubs

None. Stub-scan found only an existing format string in Go lifecycle test support (`-tags={}`), not a placeholder or UI-flowing stub.

## Threat Flags

None. New trust-boundary handling matches the plan threat model: repository manifests are parsed statically with unsupported rows for malformed directives, lifecycle root normalization is reused, and no repository go.work files are written.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib module_graph::formats::go_mod --locked`
- `cargo test -p polint --lib module_graph::formats::go_work --locked`
- `cargo test -p polint --lib module_graph::go::topology --locked`
- `cargo test -p polint --lib module_graph::go::topology_monorepo --locked`
- `cargo test -p polint --lib module_graph::go::dependency_topology --locked`
- `cargo fmt --all -- --check`
- Acceptance scans for parser symbols, no parser command execution, no Go topology go.work writes, and no declared-requirement/import collapse passed.

## Next Phase Readiness

Plan 27-03 can build on deterministic Go topology rows for the next language/package topology slice without needing public SDK or CLI promotion.

## Self-Check: PASSED

- Found created files: `crates/polint/src/module_graph/formats/go_mod.rs`, `crates/polint/src/module_graph/formats/go_work.rs`.
- Found modified files: `crates/polint/src/module_graph/go.rs`, `crates/polint/src/module_graph/topology.rs`.
- Found summary file: `.planning/phases/27-layered-module-package-topology-graph/27-02-SUMMARY.md`.
- Found task commits: `68c34e7`, `15bc733`, `8ecacbc`, `cfe0cf1`, `a168524`, `2a29d2e`.

---
*Phase: 27-layered-module-package-topology-graph*
*Completed: 2026-05-19*
