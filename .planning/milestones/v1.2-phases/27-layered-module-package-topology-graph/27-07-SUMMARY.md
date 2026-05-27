---
phase: 27-layered-module-package-topology-graph
plan: 07
subsystem: analysis-kernel
tags: [rust, cli, public-boundary, module-topology, sdk-compatibility]

requires:
  - phase: 27-layered-module-package-topology-graph
    provides: Private topology facts, module topology provider, eval fixture proof, and cache participation from Plans 27-01 through 27-06
provides:
  - Public no-leak integration proof for module topology internals across check JSON, inspect rule JSON, polint test JSON, CLI help, SDK, runner, crate-root, and CLI source surfaces
  - External temp-repo compatibility proof for ResolvedImports and ModuleGraphFacts through polint::sdk::prelude::* and polint::runner::run_cli
  - Existing imports docs wording that keeps relationship facts supported while keeping richer topology internals out of SDK facts
affects: [module-graph, module-topology, sdk, cli, public-boundary-tests, docs]

tech-stack:
  added: []
  patterns:
    - Public-boundary CLI tests assert exact internal topology vocabulary is absent from check, inspect, test, help, SDK, runner, crate-root, and docs surfaces.
    - External rule-pack fixtures continue to prove typed fact-view compatibility using only the supported SDK prelude and runner entrypoint.

key-files:
  created:
    - .planning/phases/27-layered-module-package-topology-graph/27-07-SUMMARY.md
  modified:
    - crates/polint/tests/cli.rs
    - docs/facts/imports.md

key-decisions:
  - "Keep Phase 27 topology internals private and prove the boundary with public CLI JSON, help text, and source-surface assertions rather than adding any SDK topology view."
  - "Document ResolvedImports<'_> and ModuleGraphFacts<'_> as the supported relationship surfaces while explicitly leaving richer package/workspace topology internals outside SDK facts."

patterns-established:
  - "Module-topology public-boundary tests use a temp repo rule pack that imports only polint::sdk::prelude::* and registers through polint::runner::run_cli."
  - "No-leak assertions cover exact internal row names, provider IDs, cache schema names, and proposed SDK view names."

requirements-completed: [SAE-SEM-02]

duration: 5 min
completed: 2026-05-19
---

# Phase 27 Plan 07: Public Boundary Compatibility Summary

**Module topology internals remain private while public relationship fact views stay compatible for external rule packs.**

## Performance

- **Duration:** 5 min
- **Started:** 2026-05-19T13:41:37Z
- **Completed:** 2026-05-19T13:45:57Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments

- Added `module_topology_internals_stay_private`, a public-boundary integration test covering `polint check --format json`, `polint inspect rule --format json`, `polint test --format json`, CLI help, SDK, runner, crate-root, and CLI source surfaces.
- Proved an external temp-repo rule can still request `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` through `polint::sdk::prelude::*`, register through `polint::runner::run_cli`, and receive diagnostics.
- Updated `docs/facts/imports.md` with bounded wording that public relationship rules should use `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` while richer package/workspace topology internals are not SDK fact views.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Prove public compatibility and update existing docs only if needed** - `546ce5f` (test)
2. **Task 1 GREEN: Prove public compatibility and update existing docs only if needed** - `5a16ddc` (feat)

_Note: This was a TDD task, so the failing public-boundary test and the passing docs alignment are separate commits._

## Files Created/Modified

- `crates/polint/tests/cli.rs` - Adds the module-topology no-leak test, temp-repo rule fixture, CLI help assertions, public source-surface scan, and exact forbidden marker list.
- `docs/facts/imports.md` - Clarifies that `ResolvedImports<'_>` and `ModuleGraphFacts<'_>` remain the supported public relationship surfaces and topology internals are not SDK views.

## Decisions Made

- Kept the new coverage entirely in public compatibility tests and existing docs; no public topology SDK, runner, CLI, crate-root, command, or broad docs surface was introduced.
- Used exact internal marker assertions for row families, provider IDs, schema names, and proposed view names so future leaks fail loudly.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The first green attempt wrapped the docs sentence across a newline, so the exact test substring still failed. The docs sentence was kept literal and the focused test passed.
- Parallel Cargo invocations briefly waited on package/artifact locks during the final verification pass; all commands completed successfully.

## Known Stubs

None. Stub scan matched existing test fixture literals such as `exclude = []`, `rules = []`, and `TODO` policy fixtures in `crates/polint/tests/cli.rs`; none were introduced as runtime stubs or block this plan.

## Threat Flags

None. The plan added test-only temp-repo fixture files and a docs sentence; it did not introduce network endpoints, auth paths, repository-write lifecycle behavior, schema changes, or public topology APIs.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --test cli module_topology_internals_stay_private --locked`
- `cargo test -p polint --test cli external_rule_consumes_module_relationship_facts_through_public_sdk --locked`
- `rg -n "module_topology_internals_stay_private|workspace_roots|polint\\.module_topology|module-topology-facts" crates/polint/tests/cli.rs`
- `rg -n "Packages<'_|Dependencies<'_|SourceSets<'_|RepoTopology<'_|polint topology|polint facts" docs README.md crates/polint/src/cli crates/polint/src/sdk` returned no matches.
- `cargo fmt --all -- --check`

## Next Phase Readiness

Phase 27 now has internal topology derivation, cache/eval proof, and public-boundary compatibility coverage. SAE-SEM-02 can be marked complete for the current milestone.

## Self-Check: PASSED

- Found modified files: `crates/polint/tests/cli.rs`, `docs/facts/imports.md`.
- Found summary file: `.planning/phases/27-layered-module-package-topology-graph/27-07-SUMMARY.md`.
- Found task commits: `546ce5f`, `5a16ddc`.

---
*Phase: 27-layered-module-package-topology-graph*
*Completed: 2026-05-19*
