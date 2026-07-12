---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 05
subsystem: analysis-kernel-testing
tags: [input-snapshot, analysis-plan, capabilities, provider-fixtures, compatibility-removal]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 04
    provides: First audited half of InputSnapshot fixtures migrated to the plan-aware constructor
provides:
  - Every InputSnapshot run-input caller now passes a concrete AnalysisPlan
  - No digest-only compatibility constructor or internal helper can bypass the plan boundary
  - Non-empty requested-capability ordering and support/setup status coverage
affects: [phase-65-input-snapshot-schema, phase-65-store-commit-plan, metadata-mirroring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Snapshot construction retains the borrowed AnalysisPlan through rule-component derivation"
    - "Capability-free provider fixtures share named empty-plan helpers with explicit identity-source assertions"

key-files:
  created: []
  modified:
    - crates/polint/src/analysis_kernel/incremental/input_snapshot.rs
    - crates/polint/src/analysis/types/provider.rs
    - crates/polint/src/analysis/refined_calls/provider.rs
    - crates/polint/src/analysis/entrypoints/provider.rs
    - crates/polint/src/analysis/reachability/provider.rs
    - crates/polint/src/analysis/domains/provider.rs
    - crates/polint/src/analysis/summaries/provider.rs
    - crates/polint/src/analysis/calls/provider.rs
    - crates/polint/src/analysis/cfg/provider.rs

key-decisions:
  - "Direct provider-unit fixtures with no rule demand use named empty plans and prove both zero requested-capability rows and the canonical absent requirements identity"
  - "The sole snapshot construction path borrows AnalysisPlan until the rule-options component derives the plan digest"
  - "Non-empty snapshot coverage locks deterministic capability order plus supported and unsupported status preservation without changing the v1 wire schema"

patterns-established:
  - "No run-input fixture may substitute an opaque plan digest for a concrete AnalysisPlan"
  - "Shared empty-plan fixture helpers centralize capability-source assertions for repeated provider digest tests"

requirements-completed: [META-01, META-04]

# Metrics
duration: 17min
completed: 2026-07-12
---

# Phase 65 Plan 05: Plan-Aware Snapshot Constructor Closure Summary

**All remaining snapshot fixtures now carry concrete analysis plans, and the capability-erasing digest-only constructor has been removed without changing the v1 serialized snapshot.**

## Performance

- **Duration:** 17 min
- **Started:** 2026-07-12T20:27:00Z
- **Completed:** 2026-07-12T20:44:26Z
- **Tasks:** 1
- **Files modified:** 9

## Accomplishments

- Migrated the final 17 legacy constructor sites across input-snapshot, type/value/alias, refined-call, entrypoint, reachability, abstract-domain, direct-summary, calls, and CFG fixtures.
- Removed the cfg(test) digest-only constructor and changed the remaining internal construction helpers to borrow `AnalysisPlan` through rule-component derivation.
- Consolidated repeated entrypoint, calls, and CFG fixtures behind asserted empty-plan helpers while retaining direct named-plan assertions in the other provider suites.
- Added non-empty capability-source coverage for deterministic ordering, supported versus unsupported status, setup status, policy-query version presence, and the unchanged `polint-input-snapshot-1` wire shape.

## Task Commits

1. **Task 1: Migrate remaining callers and remove the old constructor** - `6bfbbc96` (refactor)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` - Removed the digest-only seam, retained the borrowed plan through private helpers, migrated local fixtures, and expanded non-empty capability-source coverage.
- `crates/polint/src/analysis/types/provider.rs` - Type/value/alias provider fixtures now use an asserted empty plan.
- `crates/polint/src/analysis/refined_calls/provider.rs` - Solver-projection snapshot fixtures now carry an asserted empty plan.
- `crates/polint/src/analysis/entrypoints/provider.rs` - Five entrypoint fixture sites now share one asserted empty-plan snapshot helper.
- `crates/polint/src/analysis/reachability/provider.rs` - Reachability provider fixtures now use an asserted empty plan.
- `crates/polint/src/analysis/domains/provider.rs` - Abstract-domain provider fixtures now use an asserted empty plan.
- `crates/polint/src/analysis/summaries/provider.rs` - Direct-summary provider fixtures now use an asserted empty plan.
- `crates/polint/src/analysis/calls/provider.rs` - Calls digest and derivation fixtures now share one asserted empty-plan helper.
- `crates/polint/src/analysis/cfg/provider.rs` - Three CFG digest fixtures now share one asserted empty-plan helper instead of arbitrary plan strings.

## Decisions Made

- Kept direct provider-unit fixtures capability-free rather than inferring rule demand from the provider under test; each helper proves the canonical empty requested-capability state.
- Kept the plan borrowed down to `rule_components`, so no private helper accepts a substitute plan digest that could later become another compatibility seam.
- Used a mixed supported/unsupported non-empty plan to prove deterministic requested-capability ordering and status preservation before the serialized schema migration.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The prescribed input-snapshot, entrypoint, calls, and CFG `::tests` filters select zero tests because those files organize coverage under named modules. The exact commands passed, and the concrete named suites were run separately with 18 snapshot, 8 entrypoint, 4 calls, and 6 CFG tests passing.

## User Setup Required

None - this is a private constructor and fixture migration with no external configuration or service requirements.

## Verification

- Prescribed plan filters: input snapshot passed with 0 selected; entrypoints passed with 0 selected; calls passed with 0 selected; CFG passed with 0 selected; refined calls passed 2 tests.
- Concrete migrated suites: input snapshot source/config 8 passed; lifecycle 10 passed; entrypoints 8 passed; calls 4 passed; CFG 6 passed; refined-call solver projection 5 passed; types 5 passed; reachability 8 passed; abstract domains 2 passed; direct summaries 2 passed.
- `cargo check -p polint --all-features --locked`: passed.
- `make lint`: passed `cargo fmt --all -- --check` and strict workspace/all-target/all-feature Clippy with warnings denied.
- Acceptance scans: zero `InputSnapshot::from_run_inputs(` calls; zero `fn from_run_inputs(` definitions; zero inline `AnalysisPlan::empty().digest()` expressions.
- Capability proof: every named empty plan asserts zero requested rows and the canonical absent analysis-requirements digest; the non-empty calls/call-graph plan asserts sorted rows plus supported, unsupported, setup, and policy-version state.
- Schema and source-policy proof: `INPUT_SNAPSHOT_SCHEMA_VERSION` remains `polint-input-snapshot-1`, the v1 wire test excludes identity-source fields, and no delivery-history chronology was added to shipped Rust code.
- Threat review: removing the compatibility seam prevents capability/settings state from being replaced by an opaque digest; no public API, network, authentication, SQL, persistent file-write, source-body, or payload surface was introduced.

## Next Phase Readiness

- Every snapshot producer now supplies the semantic source objects needed by the upcoming v2 field/schema migration.
- No compatibility constructor remains to erase requested capability or analysis-setting state.
- Ready for Plan 65-06 with no blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-12*

## Self-Check: PASSED

All nine planned source files and this summary exist; task commit `6bfbbc96` is present; every prescribed and concrete focused suite, all-feature compilation, formatting, strict-Clippy, constructor-boundary, empty/non-empty plan, schema, and shipped-comment check listed above passes.
