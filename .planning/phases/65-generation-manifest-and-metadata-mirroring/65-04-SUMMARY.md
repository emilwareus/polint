---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 04
subsystem: analysis-kernel-testing
tags: [input-snapshot, analysis-plan, capabilities, provider-fixtures, cache-fixtures]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 03
    provides: Plan-aware input-snapshot constructor and typed capability/settings identity sources
provides:
  - First audited half of InputSnapshot fixtures migrated to the plan-aware constructor
  - Exact requested-capability and analysis-requirement source assertions for non-empty plans
  - Canonical absent capability-source assertions for direct provider-unit fixtures
affects: [phase-65-input-snapshot-migration, phase-65-store-commit-plan, metadata-mirroring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Capability-bearing fixtures borrow their existing AnalysisPlan and assert its typed identity sources"
    - "Capability-free provider fixtures name one empty plan and prove its canonical absent requirement identity"

key-files:
  created: []
  modified:
    - crates/polint/src/metrics.rs
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/go/semantic/provider.rs
    - crates/polint/src/analysis/provider.rs
    - crates/polint/src/analysis/identity/provider.rs
    - crates/polint/src/analysis/semantic_graph/provider.rs
    - crates/polint/src/analysis/solver/provider.rs

key-decisions:
  - "Preserve exact non-empty analysis plans for capability/cache fixtures; use named, asserted empty plans only for direct provider fixtures with no capability demand"

patterns-established:
  - "Snapshot fixture helpers validate requested-capability rows and analysis-requirement identity before constructing the v1 snapshot"
  - "Empty-plan helpers assert both zero requested capabilities and the canonical absent AnalysisRequirements digest"

requirements-completed: [META-01, META-04]

# Metrics
duration: 9min
completed: 2026-07-12
---

# Phase 65 Plan 04: Plan-Aware Input Snapshot Fixture Migration Summary

**Seventeen graph, metric, semantic, identity, and solver fixture sites now reach InputSnapshot through exact AnalysisPlan values with typed capability-source proofs and no v1 wire-schema change.**

## Performance

- **Duration:** 9 min
- **Started:** 2026-07-12T20:14:52Z
- **Completed:** 2026-07-12T20:24:12Z
- **Tasks:** 1
- **Files modified:** 8

## Accomplishments

- Migrated every legacy InputSnapshot constructor use in the eight-file audited slice to from_run_inputs_with_plan.
- Preserved the existing non-empty metric, module-graph, module-topology, and symbol-graph plans unchanged and asserted their exact requested-capability rows and analysis-requirements digest.
- Replaced arbitrary plan strings and inline empty-plan digests in direct Go semantic, semantic MIR, identity, semantic-graph, and solver fixtures with named empty plans that prove zero capability rows and the canonical absent analysis-requirements identity.
- Kept INPUT_SNAPSHOT_SCHEMA_VERSION at polint-input-snapshot-1 and introduced no production, serialized, public API, CLI, config, or generated-skill change.

## Task Commits

1. **Task 1: Migrate graph, metric, semantic, identity, and solver fixtures** - c899c8f7 (test)

## Files Created/Modified

- crates/polint/src/metrics.rs - Metric cache fixtures now borrow their requested metrics plan and validate its identity sources.
- crates/polint/src/module_graph/mod.rs - Module graph and module topology fixtures share plan-aware constructors with exact capability-source assertions.
- crates/polint/src/symbol_graph/mod.rs - Symbol cache and semantic payload fixtures carry the existing symbols/references plan through the plan-aware seam.
- crates/polint/src/go/semantic/provider.rs - Direct provider fixtures use one canonical empty-plan snapshot helper.
- crates/polint/src/analysis/provider.rs - The semantic MIR digest fixture replaces its arbitrary plan string with an asserted empty plan.
- crates/polint/src/analysis/identity/provider.rs - Three identity-provider fixture sites share one asserted empty-plan helper.
- crates/polint/src/analysis/semantic_graph/provider.rs - Semantic-graph provider fixtures prove their empty capability source state.
- crates/polint/src/analysis/solver/provider.rs - Solver provider fixtures prove their empty capability source state.

## Decisions Made

- Existing non-empty AnalysisPlan values remain the source of truth for fixtures that exercise requested capabilities or cache behavior.
- Direct provider-unit fixtures that do not model a rule capability use a named empty plan and assert both sides of the identity split instead of fabricating provider-derived capabilities.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The prescribed symbol_graph::tests filter selected zero tests because symbol coverage is organized under named modules. The concrete symbol_graph::symbol_graph_derivation and symbol_graph::semantic_layer_payload suites were run separately and passed 12 tests.

## User Setup Required

None - this migration changes private test fixtures only.

## Verification

- Prescribed plan chain: metrics 29 passed; module graph 34 passed; symbol_graph::tests completed with 0 selected; Go semantic 6 passed; identity 8 passed; solver 17 passed.
- Concrete migrated-call coverage: symbol derivation 9 passed; symbol semantic payload 3 passed; module topology 3 passed; semantic MIR 3 passed; semantic graph provider 10 passed.
- cargo check -p polint --all-features --locked: passed.
- make lint: passed cargo fmt --all -- --check and workspace/all-target/all-feature Clippy with warnings denied.
- Task commit hook: passed make lint without warning suppression or hook bypass.
- Acceptance scans: no InputSnapshot::from_run_inputs call or inline AnalysisPlan::empty().digest() remains in the eight files; plan-aware calls borrow AnalysisPlan; every empty helper has explicit zero-capability and absent-requirements assertions.
- Schema proof: INPUT_SNAPSHOT_SCHEMA_VERSION remains polint-input-snapshot-1.
- Threat review: no new network, authentication, SQL, persistent file-write, source-body, or public surface was introduced.

## Next Phase Readiness

- The first audited constructor slice is complete; the remaining InputSnapshot consumers can follow the same exact-plan versus asserted-empty-plan split.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-12*

## Self-Check: PASSED

All eight planned source files and this summary exist; task commit c899c8f7 is present; every prescribed and concrete focused suite, all-feature compilation, formatting, strict workspace Clippy, schema assertion, and constructor-boundary scan listed above passes.
