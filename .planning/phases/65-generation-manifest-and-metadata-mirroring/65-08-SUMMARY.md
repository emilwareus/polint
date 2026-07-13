---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 08
subsystem: analysis-kernel
tags: [provider-identity, analysis-settings, capabilities, layer-cache, invalidation]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 07
    provides: Purpose-checked LayerKey settings constructors and bounded semantic-provider seams
provides:
  - Scoped settings and filtered requested-capability identity for the first semantic-provider slice
  - Purpose-checked semantic MIR, CFG, calls, and abstract-domain LayerKeys
  - Shared canonical type/value/alias cache and output input projection
  - Production mutation coverage for rule-only preservation and declared-input invalidation
affects: [phase-65-remaining-semantic-identities, phase-65-dependency-vocabulary, phase-65-store-commit-plan]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Semantic provider identity selects one typed settings scope plus only declared capability, lifecycle, model, extension, tool, and upstream rows"
    - "Provider capability projections hash analysis dependencies while excluding rule behavior and undeclared capability rows"

key-files:
  created:
    - .planning/phases/65-generation-manifest-and-metadata-mirroring/65-08-SUMMARY.md
  modified:
    - crates/polint/src/analysis/provider.rs
    - crates/polint/src/analysis/cfg/provider.rs
    - crates/polint/src/analysis/calls/provider.rs
    - crates/polint/src/analysis/domains/provider.rs
    - crates/polint/src/analysis/entrypoints/provider.rs
    - crates/polint/src/analysis/identity/provider.rs
    - crates/polint/src/go/semantic/provider.rs
    - crates/polint/src/analysis/types/cache_key.rs
    - crates/polint/src/analysis/types/provider.rs
    - crates/polint/src/analysis_kernel/incremental/input_snapshot.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs

key-decisions:
  - "Filtered each provider's requested-capability identity to calls, control_flow, and dataflow analysis-dependency rows instead of importing aggregate plan or rule behavior identity"
  - "Kept Identity and Entrypoints on typed absent settings rows because neither provider owns configurable analysis settings"
  - "Made type/value/alias cache and output digests consume one shared canonical input-parts projection"

patterns-established:
  - "Filtered capability boundary: declared capability/setup changes invalidate, while requesting-rule behavior and undeclared capabilities do not"
  - "Unavailable input boundary: provider identities retain component status together with its typed digest"

requirements-completed: [META-01, META-04]

# Metrics
duration: 23min
completed: 2026-07-13
---

# Phase 65 Plan 08: Semantic Provider Scoped Identity Summary

**Semantic MIR through type/value/alias providers now derive output and cache identity from typed provider settings and declared inputs instead of full repository config.**

## Performance

- **Duration:** 23 min
- **Started:** 2026-07-13T13:09:51Z
- **Completed:** 2026-07-13T13:32:40Z
- **Tasks:** 1
- **Files modified:** 11 implementation files

## Accomplishments

- Replaced all nine target builders' full `InputSnapshot.config` contributions with their exact `AnalysisSettingsScope`, filtered semantic trigger capabilities, and retained declared lifecycle/model/extension/tool/upstream identities.
- Closed the semantic MIR, CFG, calls, and abstract-domain purpose-typing seams by routing their specialized LayerKeys through `new_with_analysis_settings`.
- Added explicit Go semantic lifecycle status rows and made the type/value/alias parameter and output builders share one canonical input projection.
- Added a production mutation matrix in every target provider module proving five rule-only changes preserve identity despite changed `ConfigIdentity`, while relevant settings, capability/setup state, lifecycle state, declared model/extension/tool inputs, and upstream inputs invalidate as applicable.

## Task Commits

1. **Task 1: Migrate semantic provider output/cache digests** - `774e52e7` (feat)

## Files Created/Modified

- `crates/polint/src/analysis/provider.rs` - Semantic MIR scoped identity plus shared provider mutation harness.
- `crates/polint/src/analysis/cfg/provider.rs` - CFG settings/capability projection and mutation coverage.
- `crates/polint/src/analysis/calls/provider.rs` - Calls settings/capability projection and mutation coverage.
- `crates/polint/src/analysis/domains/provider.rs` - Abstract-domain settings/capability projection, typed LayerKey fixture, and mutation coverage.
- `crates/polint/src/analysis/entrypoints/provider.rs` - Typed absent settings, filtered capabilities, and declared-input coverage.
- `crates/polint/src/analysis/identity/provider.rs` - Typed absent settings and upstream/capability-only identity.
- `crates/polint/src/go/semantic/provider.rs` - Go-only scoped settings, filtered capabilities, and explicit lifecycle/setup rows.
- `crates/polint/src/analysis/types/cache_key.rs` - Canonical type/value/alias input-parts projection.
- `crates/polint/src/analysis/types/provider.rs` - Output identity reuses the canonical type/value/alias projection.
- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` - Crate-private filtered analysis-requirements accessor.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Purpose-checked semantic MIR, CFG, calls, and abstract-domain constructors.

## Decisions Made

- Provider capability identity uses only each provider slice's direct semantic triggers (`calls`, `control_flow`, `dataflow`). Transitive graph capabilities and unrelated requests remain represented by their producing upstream digests rather than widening every downstream key.
- The filtered projection consumes `analysis_dependency_digest`, which includes capability, support, setup, and policy-query state, but intentionally excludes requesting rule IDs and rule behavior.
- Existing declared lifecycle/model/extension/tool slots remain first-class, including absent, unsupported, and setup-missing status; no full snapshot or aggregate requirements digest is used as a shortcut.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Closed the explicitly required LayerKey seams outside the frontmatter file list**

- **Found during:** Task 1 source audit.
- **Issue:** The plan action requires semantic MIR, CFG, calls, and domain LayerKeys to reject full-config digests, but `analysis_kernel/incremental/keys.rs` was not listed under `files_modified`.
- **Fix:** Renamed those four private constructor inputs to analysis settings, routed them through the purpose-checking constructor, and updated the bounded-seam source test.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/keys.rs`
- **Verification:** LayerKey suite passed 35 tests; strict workspace Clippy and the source audit passed.
- **Committed in:** `774e52e7`

**2. [Rule 3 - Blocking] Added a filtered capability accessor outside the frontmatter file list**

- **Found during:** Task 1 capability projection implementation.
- **Issue:** Providers needed exact capability/setup analysis dependencies without importing `analysis_requirements_identity`, rule behavior, or the aggregate capability set.
- **Fix:** Added one crate-private accessor that filters requested rows by declared names, verifies `DigestKind::AnalysisRequirements`, and returns a typed absent digest when none are requested.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs`
- **Verification:** Eight production mutation suites prove declared/undeclared capability and rule-behavior boundaries; no public item was added.
- **Committed in:** `774e52e7`

---

**Total deviations:** 2 auto-fixed (2 blocking implementation dependencies)
**Impact on plan:** Expansion was limited to the two private files required by the plan's explicit LayerKey and accessor contract. No SDK, runner, CLI, persistence, or public API surface changed.

## Issues Encountered

- Five exact plan filters select zero tests because those provider suites use named modules rather than `tests`; the concrete provider filters passed all 75 relevant tests.
- Strict Clippy identified a redundant test-fixture clone and a single-element source-audit loop. Both were simplified before the normal commit hook passed.

## User Setup Required

None - provider identity changes are internal and require no configuration or external service.

## Verification

- Concrete semantic provider suites: 75 passed across semantic MIR, CFG, calls, domains, entrypoints, identity, Go semantic, and type/value/alias.
- Scoped production mutation tests: 8 passed, each covering rule-only preservation, unrelated settings/capabilities, declared capability/setup changes, and provider-specific declared inputs.
- LayerKey constructor and bounded-seam suite: 35 passed.
- Exact plan command chain: passed; the five module-name mismatches selected zero tests, while identity, Go semantic, and type suites selected 45 tests.
- `cargo check -p polint --all-features --locked`: passed.
- `cargo fmt --all -- --check`: passed.
- `make lint`: passed, including workspace/all-target/all-feature Clippy with warnings denied.
- Acceptance audit finds no production `input_snapshot.config.digest` in the nine target files and no remaining untyped MIR/CFG/calls/domain specialized LayerKey path.
- Public-surface audit found only new `pub(crate)` items, with the shared harness additionally gated by `cfg(test)`.

## Next Phase Readiness

- The remaining semantic providers can reuse the scoped-settings plus filtered-capability projection without importing full config or plan identity.
- Direct summaries remains the sole explicitly enumerated semantic LayerKey seam for its later slice.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-13*

## Self-Check: PASSED

All eleven implementation files and this summary exist; implementation commit `774e52e7` is present; focused provider and LayerKey suites, mutation matrices, all-feature compilation, formatting, strict Clippy, acceptance scans, and the private-visibility audit pass.
