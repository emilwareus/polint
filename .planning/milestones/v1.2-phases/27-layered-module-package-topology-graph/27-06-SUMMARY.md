---
phase: 27-layered-module-package-topology-graph
plan: 06
subsystem: analysis-kernel
tags: [rust, eval, module-topology, fixtures, layer-cache]

requires:
  - phase: 27-05
    provides: "Private module topology provider, topology facts, and cache layer participation"
provides:
  - "Internal eval observation for private module topology fact families and uncertainty statuses"
  - "Native module-topology fixture covering Go module roots, TS workspace packages, source sets, requirements, edges, overlays, and cache reuse"
  - "Regression coverage proving polint.module_topology participates in layer-cache invalidation"
affects: [analysis-kernel, eval, module-graph, layer-cache, phase-27]

tech-stack:
  added: []
  patterns:
    - "Eval observes topology through crate-private AnalysisDb accessors and normalized stable-key payload fragments"
    - "Native fixtures prove cold, warm, package.json edit, and go.mod edit cache behavior"

key-files:
  created:
    - tests/eval-fixtures/module-topology/core/expected.polint-eval.toml
    - tests/eval-fixtures/module-topology/core/repo/.polint.toml
    - tests/eval-fixtures/module-topology/core/repo/services/api/go.mod
    - tests/eval-fixtures/module-topology/core/repo/services/api/go.sum
    - tests/eval-fixtures/module-topology/core/repo/services/api/main.go
    - tests/eval-fixtures/module-topology/core/repo/web/package.json
    - tests/eval-fixtures/module-topology/core/repo/web/package-lock.json
    - tests/eval-fixtures/module-topology/core/repo/web/src/app.ts
    - tests/eval-fixtures/module-topology/core/repo/web/src/app.test.ts
    - tests/eval-fixtures/module-topology/core/repo/web/generated/client.generated.ts
  modified:
    - crates/polint/src/eval/model.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/metrics.rs
    - crates/polint/src/eval/matcher.rs
    - crates/polint/src/eval/report.rs
    - crates/polint/src/eval/mod.rs
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - tests/eval-fixtures/cache/layer-cache/expected.polint-eval.toml

key-decisions:
  - "Kept topology eval observation crate-private and test-facing, with no SDK, runner, CLI, or public crate-root topology API."
  - "Represented topology expected rows through stable keys, status labels, precision labels, and compact payload fragments instead of raw source or absolute paths."
  - "Updated existing layer-cache expectations so polint.module_topology is part of the managed provider cache proof."

patterns-established:
  - "Topology eval rows are emitted from AnalysisDb family accessors rather than by exposing topology through RuleCtx or public SDK facts."
  - "Module-topology fixtures exercise cold/warm/edit passes against a copied temp repository and assert layer-cache invariants as eval rows."

requirements-completed: [SAE-SEM-02]

duration: 17min
completed: 2026-05-19
---

# Phase 27 Plan 06: Module Topology Eval Fixture Summary

**Private topology facts are now verified through native eval rows, uncertainty scoring, and cold/warm/edit cache invariants.**

## Performance

- **Duration:** 17 min
- **Started:** 2026-05-19T13:21:43Z
- **Completed:** 2026-05-19T13:38:15Z
- **Tasks:** 2
- **Files modified:** 21

## Accomplishments

- Added internal eval observation for all seven topology fact families: workspace roots, topology packages, source sets, dependency requirements, resolved dependency edges, import-to-package edges, and repo topology overlays.
- Added topology status labels for resolved, external, missing lockfiles, unsupported, dynamic, ambiguous, undeclared, and outside-workspace cases, with unknown-like statuses contributing to eval unknown metrics.
- Added `module-topology-core`, a native fixture that proves Go monorepo and TS workspace topology facts plus module graph and module topology cache participation across cold, warm, package.json edit, and go.mod edit passes.
- Updated existing provider/cache regressions so `polint.module_topology` is included in the workspace-wide cache proof.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Teach internal eval to observe topology rows** - `8381a0d` (test)
2. **Task 1 GREEN: Teach internal eval to observe topology rows** - `bad9d4d` (feat)
3. **Task 2 RED: Add module topology native fixture and cache invalidation proof** - `317790d` (test)
4. **Task 2 GREEN: Add module topology native fixture and cache invalidation proof** - `0b21d50` (feat)
5. **Deviation fix: Workspace clippy verification** - `4e5ff84` (fix)
6. **Deviation fix: Module topology cache regressions** - `41e94b7` (fix)

## Files Created/Modified

- `crates/polint/src/eval/model.rs` - Added module-topology fixture area, topology fact families, and topology uncertainty statuses.
- `crates/polint/src/eval/observed.rs` - Emits normalized topology observed facts from private `AnalysisDb` accessors.
- `crates/polint/src/eval/fixtures.rs` - Runs the native module-topology fixture through cold, warm, package.json edit, and go.mod edit passes.
- `crates/polint/src/eval/metrics.rs`, `crates/polint/src/eval/matcher.rs`, `crates/polint/src/eval/report.rs` - Scores and reports the new topology unknown-like statuses.
- `crates/polint/src/eval/mod.rs` - Added focused topology eval tests.
- `tests/eval-fixtures/module-topology/core/` - Added the fixture repository and expected eval assertions.
- `tests/eval-fixtures/cache/layer-cache/expected.polint-eval.toml` - Updated cache expectations for the module topology provider.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` and `crates/polint/src/analysis_kernel/mod.rs` - Updated regression expectations for the module topology provider.
- `crates/polint/src/module_graph/mod.rs` - Documented existing high-arity internal helpers for clippy.

## Decisions Made

- Kept topology verification internal. The plan proves topology behavior through eval rows without promoting a public topology SDK, runner, CLI, or crate-root API.
- Used stable-key and compact payload fragments for eval evidence so reports stay deterministic and do not include raw source, absolute paths, or timestamps.
- Treated `polint.module_topology` as part of the existing provider cache matrix once the fixture exposed stale expectations.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Documented existing module graph helper argument shapes for clippy**
- **Found during:** Plan-level clippy verification
- **Issue:** Workspace clippy with `-D warnings` rejected existing internal helper functions in `module_graph/mod.rs` for high argument counts.
- **Fix:** Added local `#[expect(clippy::too_many_arguments)]` annotations with reasons on the affected internal helpers.
- **Files modified:** `crates/polint/src/module_graph/mod.rs`
- **Verification:** `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- **Committed in:** `4e5ff84`

**2. [Rule 3 - Blocking] Updated stale module topology provider/cache regression expectations**
- **Found during:** Plan-level workspace test verification
- **Issue:** Existing provider manifest and layer-cache expected rows did not yet include the `polint.module_topology` provider added by Phase 27.
- **Fix:** Updated provider-manifest expectations, module graph layer key schema expectation, and layer-cache expected aggregate/provider rows.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/keys.rs`, `crates/polint/src/analysis_kernel/mod.rs`, `tests/eval-fixtures/cache/layer-cache/expected.polint-eval.toml`
- **Verification:** Focused failing tests plus full workspace tests
- **Committed in:** `41e94b7`

---

**Total deviations:** 2 auto-fixed (2 Rule 3)
**Impact on plan:** Both fixes were required to complete the plan-level verification; no public topology surface was added.

## Issues Encountered

- Initial module-topology expected rows were stricter than the actual normalized topology evidence for generated source-set status and external import precision. The expected fixture was corrected during the TDD green pass.
- No authentication gates or external service setup were encountered.

## Known Stubs

None. Stub scan only found fixture literals, test names, and formatting strings that do not block the plan goal.

## Threat Flags

None. The plan reused existing fixture copy and kernel execution paths and did not add network endpoints, auth paths, external file-access surfaces, or public topology APIs.

## Verification

- `cargo test -p polint --lib eval::topology_rows --locked`
- `cargo test -p polint --lib eval::fixtures::module_topology_core --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
- `cargo fmt --all -- --check`

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 27 now has internal eval proof for topology facts, uncertainty status scoring, Go monorepo support, TS workspace topology, overlays, import classification, and cache participation. Plan 27-07 can rely on these rows without exposing topology as public SDK or CLI surface.

## Self-Check: PASSED

- Created summary and fixture files verified present.
- Task commits verified present: `8381a0d`, `bad9d4d`, `317790d`, `0b21d50`, `4e5ff84`, `41e94b7`.
- Plan verification commands passed.

---
*Phase: 27-layered-module-package-topology-graph*
*Completed: 2026-05-19*
