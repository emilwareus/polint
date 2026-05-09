---
phase: 11-capability-driven-analysis-plan
plan: 01
subsystem: core-analysis
tags: [rust, sdk, capabilities, diagnostics, deterministic-digest]

requires:
  - phase: 03-core-facts-and-diagnostics
    provides: Core Rule, RuleCtx, diagnostics, and deterministic runner contracts
  - phase: 07-cache-and-performance
    provides: stable_hash and deterministic rule option encoding
provides:
  - Internal AnalysisPlan construction for enabled rules
  - Stable analysis plan digest over rules, options, capabilities, support, and setup rows
  - SDK-visible CapabilitySupportView through RuleCtx
  - Unsupported reserved capability diagnostics
affects: [phase-11, runner, adapters, cache, explain-plan, sdk]

tech-stack:
  added: []
  patterns:
    - crate-private planner with narrow SDK support view
    - length-prefixed digest inputs with stable_hash
    - TDD RED/GREEN commits for core API changes

key-files:
  created:
    - crates/polint/src/analysis_plan.rs
  modified:
    - crates/polint/src/core/mod.rs
    - crates/polint/src/sdk/mod.rs
    - crates/polint/src/lib.rs

key-decisions:
  - "Keep AnalysisPlan crate-private and expose only CapabilitySupport, CapabilitySupportStatus, and CapabilitySupportView through the SDK prelude."
  - "Treat cfg, call_graph, coverage_facts, and test_suite_metrics as unsupported reserved capabilities in Phase 11."
  - "Use deterministic length-prefixed strings plus stable_hash for the plan digest instead of serde JSON output."

patterns-established:
  - "RuleCtx::new remains the compatibility constructor and creates an empty CapabilitySupportView."
  - "run_rules delegates to run_rules_with_capability_support so future runner integration can pass the resolved plan view without widening the public rule API."
  - "AnalysisPlan::from_rules sorts enabled planned rules and aggregates capabilities through BTree collections before digesting."

requirements-completed: [PLAN-01, PLAN-04]

duration: 8min
completed: 2026-05-09
---

# Phase 11 Plan 01: Internal AnalysisPlan Contract Summary

**Internal capability planning with stable digests, reserved-capability diagnostics, and a narrow SDK support view through RuleCtx**

## Performance

- **Duration:** 8 min
- **Started:** 2026-05-09T07:39:26Z
- **Completed:** 2026-05-09T07:47:35Z
- **Tasks:** 2
- **Files modified:** 4

## Accomplishments

- Added `CapabilitySupportStatus`, `CapabilitySupport`, and `CapabilitySupportView`, with `RuleCtx::capability_support()` exposing only the read-only support view.
- Added crate-private `run_rules_with_capability_support(...)` while preserving `run_rules(...)` and `RuleCtx::new(...)` compatibility.
- Implemented internal `AnalysisPlan` construction with deterministic rule filtering, capability aggregation, stable digest generation, support view generation, and `polint/capability` diagnostics for unsupported reserved capabilities.

## Task Commits

1. **Task 1 RED: capability support tests** - `3acb443` (test)
2. **Task 1 GREEN: capability support view** - `95fae45` (feat)
3. **Task 2 RED: analysis plan tests** - `ec6d259` (test)
4. **Task 2 GREEN: internal AnalysisPlan** - `942a6c5` (feat)

_Note: Both tasks were marked TDD, so each produced RED and GREEN commits._

## Files Created/Modified

- `crates/polint/src/analysis_plan.rs` - Internal `AnalysisPlan`, planned rule/capability/setup rows, support view construction, diagnostics, digest, and unit tests.
- `crates/polint/src/core/mod.rs` - SDK-facing capability support types, `RuleCtx` support accessor, and plan-aware runner plumbing.
- `crates/polint/src/sdk/mod.rs` - SDK prelude exports for the support view types only.
- `crates/polint/src/lib.rs` - Crate-private `analysis_plan` module registration.

## Decisions Made

- `AnalysisPlan` remains internal to `crates/polint`; rule authors get only `CapabilitySupport*` through `polint::sdk::prelude::*`.
- Reserved future capabilities are explicit `Unsupported` rows, not silently accepted empty facts.
- `test_suite_metrics` includes the required hint: `Use go_tests for current Go test evidence.`
- The digest input is manually encoded and length-prefixed before `stable_hash`; serde JSON is not used as digest input.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The Rust skill chapter path in the system skill cache was absent; the repo-local skill references were loaded instead. No implementation impact.
- A dead-code warning appeared for required `AnalysisPlan::empty()` and `setup_checks()` accessors until tests exercised them. The tests now cover both API points.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None.

## Next Phase Readiness

Plan 11-02 can now construct an `AnalysisPlan` before adapter execution, pass `plan.support_view()` into `run_rules_with_capability_support`, and thread `plan.digest()` into cache/adapter behavior.

## Self-Check: PASSED

- Found `crates/polint/src/analysis_plan.rs`.
- Found `.planning/phases/11-capability-driven-analysis-plan/11-01-SUMMARY.md`.
- Found commits `3acb443`, `95fae45`, `ec6d259`, and `942a6c5`.

---
*Phase: 11-capability-driven-analysis-plan*
*Completed: 2026-05-09*
