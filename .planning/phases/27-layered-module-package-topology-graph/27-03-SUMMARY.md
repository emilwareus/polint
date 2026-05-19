---
phase: 27-layered-module-package-topology-graph
plan: 03
subsystem: analysis-kernel
tags: [rust, module-graph, typescript, javascript, package-json, lockfile-evidence]

requires:
  - phase: 27-layered-module-package-topology-graph
    provides: Internal topology contracts and Go topology row patterns from Plans 01 and 02
provides:
  - Static package.json parser for package identity, workspaces, packageManager, exports/imports, scripts-adjacent evidence, and dependency sections
  - Static package-lock.json parser for selected package evidence from lockfileVersion 2/3 packages
  - TS/JS topology collector for workspace roots, package rows, source sets, package-manager evidence, tsconfig overlays, declared requirements, and lockfile evidence
affects: [module-graph, topology-facts, ts-js-analysis, dependency-evidence, future-cache-validation]

tech-stack:
  added: []
  patterns:
    - Static JS package-manager parsing through serde_json only, with unsupported rows for malformed or unsupported inputs
    - TS/JS declared requirements and selected lockfile evidence emitted as separate topology fact families
    - Unsupported pnpm/Yarn/Bun lockfiles represented as explicit unsupported evidence, not exact selections

key-files:
  created:
    - crates/polint/src/module_graph/formats/package_json.rs
    - crates/polint/src/module_graph/formats/package_lock.rs
  modified:
    - crates/polint/src/module_graph/formats/mod.rs
    - crates/polint/src/module_graph/ts.rs
    - crates/polint/src/module_graph/topology.rs

key-decisions:
  - "Represent package-manager and tsconfig evidence as internal repo topology overlay rows until a dedicated manager-evidence fact family is introduced."
  - "Treat package-lock.json packages as exact lockfile-selected rows while marking pnpm, Yarn, and Bun lockfile presence as unsupported evidence."
  - "Use workspace: dependency ranges to override the dependency-section kind with RequirementKind::Workspace."

patterns-established:
  - "TS/JS source-set classification is deterministic per discovered TS-family file and covers source, test, generated, and vendor contexts."
  - "Static JS topology collection reads checked-in manifests and lockfiles only; it never executes npm, pnpm, Yarn, Bun, or node_modules traversal."

requirements-completed: [SAE-SEM-02]

duration: 16 min
completed: 2026-05-19
---

# Phase 27 Plan 03: TS/JS Package Topology Summary

**Deterministic TS/JS workspace, package, source-set, declared dependency, and lockfile-evidence topology from static manifests**

## Performance

- **Duration:** 16 min
- **Started:** 2026-05-19T12:17:58Z
- **Completed:** 2026-05-19T12:33:40Z
- **Tasks:** 3
- **Files modified:** 5

## Accomplishments

- Added static `package.json` and `package-lock.json` readers for package identity, workspaces, dependency sections, package-manager metadata, and selected package evidence.
- Added `collect_ts_topology` for JS workspace roots, JS package rows, package-manager/lockfile evidence overlays, tsconfig evidence overlays, and TS/JS source-set classification.
- Added TS/JS declared dependency requirement rows and separate resolved dependency evidence rows for package-lock selections, unsupported lockfile presence, and missing lockfiles.

## Task Commits

1. **Task 1: Parse package.json and package-lock topology inputs** - `3eb29f5` (test), `dd035ce` (feat)
2. **Task 2: Collect TS/JS package, workspace, and source-set topology** - `7d2ff5c` (test), `44aec5d` (feat)
3. **Task 3: Emit TS/JS declared and selected dependency rows** - `d956b2b` (test), `eabb037` (feat), `dede2ee` (refactor), `86bf5a7` (refactor)

_Note: TDD tasks have separate red test and green implementation commits._

## Files Created/Modified

- `crates/polint/src/module_graph/formats/package_json.rs` - Static package.json parser, dependency-section rows, workspace parsing, evidence preservation, and parser tests.
- `crates/polint/src/module_graph/formats/package_lock.rs` - Static package-lock parser for lockfileVersion schema labels and selected package rows.
- `crates/polint/src/module_graph/formats/mod.rs` - Registers JS static manifest parser modules.
- `crates/polint/src/module_graph/ts.rs` - TS/JS topology collector, source-set classification, manager/tsconfig overlays, declared requirements, and lockfile evidence rows.
- `crates/polint/src/module_graph/topology.rs` - Adds TS/JS topology enum variants required by the collector and tests.

## Decisions Made

- Package-manager and tsconfig evidence is stored as internal `RepoTopologyOverlayFact` rows using `SourceOfTruthDirectory` until a more specific fact family lands.
- Declared `package.json` requirements are separate from package-lock selected versions and actual import usage.
- Unsupported pnpm/Yarn/Bun lockfiles are recognized by filename with unsupported precision/status rather than parsed or treated as exact.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Added TS/JS topology enum vocabulary**
- **Found during:** Task 2 (TS/JS topology collector)
- **Issue:** Existing topology enums did not include the TS/JS variants required by the plan acceptance criteria, including `JsWorkspace`, `JsPackage`, `Dev`, `Bundled`, `Workspace`, `LockfileSelected`, and `SourceOfTruthDirectory`.
- **Fix:** Extended crate-private topology enums without adding SDK, runner, CLI, or public docs surface.
- **Files modified:** `crates/polint/src/module_graph/topology.rs`
- **Verification:** `cargo test -p polint --lib module_graph::ts::topology --locked`; `cargo test -p polint --lib module_graph::ts::dependency_topology --locked`
- **Committed in:** `7d2ff5c` and `dd035ce`

---

**Total deviations:** 1 auto-fixed (1 missing critical)
**Impact on plan:** The enum additions were required to represent the planned TS/JS topology rows. No public topology surface was promoted.

## Issues Encountered

- The first Task 2 test placement compiled under `module_graph::ts::tests::topology`, which the plan filter did not select. Topology tests were added under top-level `module_graph::ts::topology`, then the duplicate nested copy was removed.
- `cargo fmt --all -- --check` required rustfmt-only changes after the larger TS implementation; these were committed separately.
- Parallel Cargo verification briefly waited on package/artifact locks; all targeted reruns completed successfully.

## Known Stubs

None. Stub-scan found only an existing test fixture string containing `{}` (`export const tokens = {};`), not a placeholder or UI-flowing stub.

## Threat Flags

None. New trust-boundary handling matches the plan threat model: repository JSON/text inputs are parsed statically, malformed JSON returns unsupported rows, unsupported lockfile formats do not claim exactness, and raw lockfile bodies are not stored in topology facts.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib module_graph::formats::package_json --locked`
- `cargo test -p polint --lib module_graph::formats::package_lock --locked`
- `cargo test -p polint --lib module_graph::ts::topology --locked`
- `cargo test -p polint --lib module_graph::ts::dependency_topology --locked`
- `cargo fmt --all -- --check`
- Acceptance scans for parser symbols, dependency-section names, package-manager/lockfile recognition, source-set variants, no package-manager execution, and unsupported lockfile labels passed.

## Next Phase Readiness

Plan 27-04 can build import-to-package classification on TS/JS package ownership, source-set context, declared requirement rows, package-lock selected evidence, and explicit unsupported/missing lockfile uncertainty.

## Self-Check: PASSED

- Found created files: `crates/polint/src/module_graph/formats/package_json.rs`, `crates/polint/src/module_graph/formats/package_lock.rs`.
- Found modified files: `crates/polint/src/module_graph/ts.rs`, `crates/polint/src/module_graph/topology.rs`, `crates/polint/src/module_graph/formats/mod.rs`.
- Found summary file: `.planning/phases/27-layered-module-package-topology-graph/27-03-SUMMARY.md`.
- Found task commits: `3eb29f5`, `dd035ce`, `7d2ff5c`, `44aec5d`, `d956b2b`, `eabb037`, `dede2ee`, `86bf5a7`.

---
*Phase: 27-layered-module-package-topology-graph*
*Completed: 2026-05-19*
