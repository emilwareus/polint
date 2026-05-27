---
phase: 20-private-analysis-kernel-facade
plan: "01"
subsystem: core-analysis
tags: [rust, analysis-kernel, runner, cli, capabilities]

requires:
  - phase: 11-capability-driven-analysis-plan
    provides: AnalysisPlan, RulePlanInputs, plan digests, and capability support view
  - phase: 12-resolved-imports-and-module-relationships
    provides: module graph derivation and provider support overlays
  - phase: 13-symbols-and-references
    provides: symbol graph derivation and setup-aware capability support
provides:
  - Crate-private AnalysisKernel facade for existing provider orchestration
  - KernelInput and KernelOutput boundary around loaded config, cache, digests, plan, diagnostics, db, and support view
  - Runner delegation through AnalysisKernel before rule execution
  - Parent/no-local-rule CLI delegation through AnalysisKernel
  - Temp-repo public SDK proof that metrics and symbol facts remain visible through polint check
affects: [20-02, analysis-kernel, runner, cli, provider-manifests]

tech-stack:
  added: []
  patterns:
    - crate-private kernel facade owns eager provider execution order
    - runner and CLI retain rule selection, rule options, ignores, filtering, rendering, and rule execution
    - provider support overlays merge inside the kernel before rules see the final support view

key-files:
  created:
    - crates/polint/src/analysis_kernel/mod.rs
    - .planning/phases/20-private-analysis-kernel-facade/20-01-SUMMARY.md
  modified:
    - crates/polint/src/lib.rs
    - crates/polint/src/runner/mod.rs
    - crates/polint/src/cli/mod.rs
    - crates/polint/tests/cli.rs

key-decisions:
  - "Keep AnalysisKernel, KernelInput, and KernelOutput crate-private and expose no new SDK, crate-root public, or CLI surface."
  - "Preserve the existing eager provider order inside AnalysisKernel: source loading, Go syntax, TS/JS syntax, module graph, symbol graph, metrics."
  - "Keep rule selection, options, rule execution, ignores, filtering, rendering, and exit behavior outside the kernel."

patterns-established:
  - "AnalysisKernel::run accepts borrowed KernelInput and returns owned KernelOutput."
  - "KernelOutput.capability_support is the module-then-symbol overlay of the static plan support view."
  - "Behavior-preservation tests use temp-repo local rules importing only polint::sdk::prelude::*."

requirements-completed: [SAE-FND-01]

duration: 9 min
completed: 2026-05-16
---

# Phase 20 Plan 01: Private Kernel Facade Summary

**Private AnalysisKernel facade now owns the existing source, syntax, module, symbol, and metrics provider sequence without changing rule-facing behavior**

## Performance

- **Duration:** 9 min
- **Started:** 2026-05-16T19:51:47Z
- **Completed:** 2026-05-16T20:01:12Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments

- Added `crates/polint/src/analysis_kernel/mod.rs` with crate-private `AnalysisKernel`, `KernelInput<'_>`, and `KernelOutput`.
- Moved the existing eager provider orchestration order behind `AnalysisKernel::run`: source loading, Go syntax, TS/JS syntax, module graph, symbol graph, and metrics.
- Updated child local-rule runner and parent/no-local-rule CLI analysis paths to delegate provider execution through the kernel while keeping rule execution and reporting outside the kernel.
- Added a temp-repo external rule test that imports only `polint::sdk::prelude::*` and proves `FileMetrics<'_>`, `FunctionMetrics<'_>`, `ComplexityMetrics<'_>`, `Symbols<'_>`, and `References<'_>` remain visible after delegation.

## Task Commits

Each TDD step was committed atomically:

1. **Task 1 RED: Add failing tests for analysis kernel facade** - `c47e089` (test)
2. **Task 1 GREEN: Implement private analysis kernel facade** - `6aaa49b` (feat)
3. **Task 2 RED: Add failing delegation fact-preservation test** - `3041bd2` (test)
4. **Task 2 GREEN: Delegate analysis paths through kernel** - `8f7e723` (feat)

**Plan metadata:** pending final docs commit.

## Files Created/Modified

- `crates/polint/src/analysis_kernel/mod.rs` - Crate-private kernel facade, input/output types, provider orchestration, and unit tests.
- `crates/polint/src/lib.rs` - Registers `analysis_kernel` as a crate-private module.
- `crates/polint/src/runner/mod.rs` - Delegates provider execution through `AnalysisKernel::run` and then runs rules with kernel capability support.
- `crates/polint/src/cli/mod.rs` - Delegates parent/no-local-rule analysis through `AnalysisKernel::run`.
- `crates/polint/tests/cli.rs` - Adds behavior-preservation coverage for rule-visible derived metrics and symbol facts through the public SDK.

## Decisions Made

- Kept the kernel strictly crate-private and did not add provider inspection, public CLI, or SDK exposure in this plan.
- Preserved the existing provider order exactly rather than introducing scheduler or manifest-driven execution ahead of Plan 20-02.
- Kept the parent/no-local-rule path's empty `run_rules` call after the kernel so existing warning-free build behavior remains unchanged.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The Task 2 behavior-only integration test would already pass against the old inline orchestration. To make the RED phase meaningful, the same test also asserts that both runner and CLI source paths contain `AnalysisKernel::run` and `KernelInput`, matching the plan's structural acceptance criteria.

## Verification

- `cargo test -p polint --lib analysis_kernel --locked`
- `cargo test -p polint --test cli kernel_delegation_preserves_existing_rule_facts --locked`
- `cargo test -p polint --test cli symbol_reference_cache_and_setup --locked`
- `cargo fmt --all -- --check`
- Structural `rg` checks confirmed module registration, facade types, provider calls inside the kernel, no rule/reporting concerns inside the kernel, kernel delegation in runner and CLI, no direct provider orchestration left in runner, and the new behavior-preservation test.

## Known Stubs

None. Stub-pattern scan hits only existing CLI fixture literals and intentional TOML empty arrays; no new stubbed behavior was introduced by this plan.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 20-02 can add internal provider manifests and deterministic provider-order inspection on top of the private kernel boundary. SAE-FND-01's facade/delegation portion is complete here; provider manifest coverage remains the next plan's scope.

## Self-Check: PASSED

- Confirmed `crates/polint/src/analysis_kernel/mod.rs` and this summary exist.
- Confirmed task commits exist: `c47e089`, `6aaa49b`, `3041bd2`, and `8f7e723`.

---
*Phase: 20-private-analysis-kernel-facade*
*Completed: 2026-05-16*
