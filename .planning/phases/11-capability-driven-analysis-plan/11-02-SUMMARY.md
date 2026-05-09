---
phase: 11-capability-driven-analysis-plan
plan: "02"
subsystem: core-analysis
tags: [rust, analysis-plan, cache, adapters, diagnostics]

requires:
  - phase: 11-01
    provides: internal AnalysisPlan contract and capability support view
  - phase: 07-cache-and-performance
    provides: file fact cache and deterministic cache keys
provides:
  - panic-contained RulePlanInputs reused for rule options, rule digest, and plan construction
  - child local-rule check path that builds the real plan before file loading
  - parent no-local-rule check path that passes an empty valid plan to adapters
  - Go and TS/JS plan-aware adapter entrypoints with unchanged bench-facing wrappers
  - plan_hash participation in stable file fact cache identity
affects: [11-03, capability-planning, go-adapter, ts-adapter, cache]

tech-stack:
  added: []
  patterns:
    - panic-contained planning snapshot before analysis file loading
    - crate-internal plan-aware adapter entrypoints behind public bench wrappers
    - plan digest as cache identity input

key-files:
  created:
    - .planning/phases/11-capability-driven-analysis-plan/11-02-SUMMARY.md
  modified:
    - crates/polint/src/analysis_plan.rs
    - crates/polint/src/runner/mod.rs
    - crates/polint/src/cli/mod.rs
    - crates/polint/src/go/adapter.rs
    - crates/polint/src/go/mod.rs
    - crates/polint/src/ts/adapter.rs
    - crates/polint/src/ts/mod.rs
    - crates/polint/src/cache/mod.rs
    - crates/polint/tests/cli.rs

key-decisions:
  - "Use RulePlanInputs as the single panic-contained rule metadata/capability snapshot for options, rule digest, and plan construction."
  - "Keep AnalysisPlan crate-private; bench-facing analyze_with_options wrappers construct AnalysisPlan::empty() internally."
  - "Include plan_hash in CacheKey::stable_id between rule_hash and cache version."
  - "Use an empty AnalysisPlan in parent CLI paths where no local rule host is loaded."

patterns-established:
  - "Child runner planning order: collect RulePlanInputs, derive options, derive rule digest, build AnalysisPlan, then load files."
  - "Production adapter calls use crate-internal analyze_with_plan_options while _bench keeps the old wrapper names."

requirements-completed: [PLAN-02, PLAN-03, PLAN-04]

duration: 16m 12s
completed: 2026-05-09
---

# Phase 11 Plan 02: Capability Plan Wiring Summary

**Capability-driven check planning now feeds adapter cache identity through a private AnalysisPlan digest**

## Performance

- **Duration:** 16m 12s
- **Started:** 2026-05-09T07:51:39Z
- **Completed:** 2026-05-09T08:07:51Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments

- Added `RulePlanInputs`, a safe rule metadata/capability snapshot that catches plan-time `meta()` and `capabilities()` panics and reuses the collected data for options, rule digests, and `AnalysisPlan` construction.
- Updated the child local-rule runner to build the real plan before `load_analysis_files`, include plan diagnostics, and pass the plan support view into rule execution.
- Added crate-internal Go and TS/JS `analyze_with_plan_options` entrypoints while keeping public bench-facing `analyze_with_options` wrappers unchanged.
- Added `plan_hash` to stable cache key identity so a changed plan digest invalidates stale file facts.
- Added regressions for plan-time metadata panic containment and plan-hash-sensitive cache keys.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Build and use the plan in check execution** - `4f34654` (`test`)
2. **Task 1 GREEN: Build and use the plan in check execution** - `efc001d` (`feat`)
3. **Task 2 RED: Pass plan digests into adapter cache keys** - `d9bb97f` (`test`)
4. **Task 2 GREEN: Pass plan digests into adapter cache keys** - `7d2e8dd` (`feat`)

Plan metadata is committed separately after state updates.

## Files Created/Modified

- `crates/polint/src/analysis_plan.rs` - added `RulePlanInputs`, panic-contained planning collection, plan construction from inputs, and plan-time panic tests.
- `crates/polint/src/runner/mod.rs` - switched child check execution to collect planning inputs before options, digests, plan construction, and file loading.
- `crates/polint/src/cli/mod.rs` - added empty-plan construction for parent CLI analysis paths and routed adapter calls through plan-aware internals.
- `crates/polint/src/go/adapter.rs` - added plan-aware Go adapter implementation and passed the plan digest into cache keys.
- `crates/polint/src/go/mod.rs` - re-exported the plan-aware Go adapter entrypoint as crate-internal while preserving `_bench` wrapper access.
- `crates/polint/src/ts/adapter.rs` - added plan-aware TS/JS adapter implementation and passed the plan digest into cache keys.
- `crates/polint/src/ts/mod.rs` - re-exported the plan-aware TS/JS adapter entrypoint as crate-internal while preserving `_bench` wrapper access.
- `crates/polint/src/cache/mod.rs` - added `plan_hash` to `CacheKey`, `stable_id`, and cache-key regression coverage.
- `crates/polint/tests/cli.rs` - added a temp-repo local-rule regression for controlled metadata panic diagnostics.

## Decisions Made

- The first rule metadata/capability collection in the child runner is `RulePlanInputs::collect`; later option, digest, and plan construction code reads only the collected snapshot.
- `AnalysisPlan` remains crate-private and is not exposed through `sdk`, crate root, or `_bench`.
- Bench-facing adapter wrappers keep their old public signatures and use `AnalysisPlan::empty()` internally.
- Parent CLI analysis uses `AnalysisPlan::empty()` because it has no local rule host rules to plan.
- Adapter cache identity uses the resolved plan digest through `plan_hash`, not an unrelated rule digest extension.

## Verification

- `cargo test -p polint --lib analysis_plan --locked`
- `cargo test -p polint --test cli check_contains_plan_time_rule_metadata_panic --locked`
- `cargo test -p polint --lib cache_key_changes_with_plan_hash --locked`
- `cargo check -p polint-bench --locked`
- `cargo fmt --all -- --check`
- Structural `rg` checks confirmed `RulePlanInputs`, `analyze_with_plan_options`, `plan.digest()`, parent empty plans, cache `plan_hash`, and preserved `_bench` wrapper exports.

## Deviations from Plan

None - plan executed within the requested files and behavior. A scoped warning cleanup was included with Task 2 so intentionally preserved bench wrappers and future plan accessors remain quiet in non-bench integration builds.

## Known Stubs

None. Stub-pattern scan found only intentional existing CLI test fixture literals such as `TODO` and empty rule arrays.

## Issues Encountered

- Moving production callers off `analyze_with_options` made those bench-facing wrappers unused in non-bench test builds. The wrappers were preserved as required and annotated narrowly to avoid warning noise.
- `roadmap update-plan-progress` collapsed the Phase 11 top-table row; the row was restored manually with the same 2/3 progress.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan construction now reaches adapters and cache keys without widening public API surface. Plan 11-03 can build on this by exposing or explaining plan artifacts internally while keeping rule-author APIs curated.

## Self-Check: PASSED

- Confirmed summary and key modified files exist.
- Confirmed task commits exist: `4f34654`, `efc001d`, `d9bb97f`, `7d2e8dd`.

---
*Phase: 11-capability-driven-analysis-plan*
*Completed: 2026-05-09*
