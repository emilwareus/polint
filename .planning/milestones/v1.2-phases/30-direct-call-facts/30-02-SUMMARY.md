---
phase: 30-direct-call-facts
plan: 02
subsystem: analysis
tags: [rust, analysis-kernel, call-facts, cache-identity, eval-fixtures]

requires:
  - phase: 30-direct-call-facts
    provides: "Plan 01 crate-private call fact contracts, CallOutput, CallStore, and AnalysisDb call fact storage"
  - phase: 29-local-cfg-and-control-dependence
    provides: "polint.cfg provider slot and CFG output digest consumed by calls"
provides:
  - "Private polint.calls provider manifest after CFG and before metrics"
  - "Recompute-only calls provider shell publishing deterministic empty CallOutput"
  - "calls-facts-1 provider output digest and future-fit calls layer key vocabulary"
  - "Provider-order eval fixture updated with private polint.calls row"
affects: [analysis-kernel, direct-calls, layer-cache, eval-fixtures, future-call-resolution]

tech-stack:
  added: []
  patterns: ["manifest-owned private provider slot", "provider output digest over stable row payloads", "future-fit layer key vocabulary"]

key-files:
  created:
    - crates/polint/src/analysis/calls/provider.rs
    - crates/polint/src/analysis/calls/cache_key.rs
  modified:
    - crates/polint/src/analysis/calls/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml

key-decisions:
  - "polint.calls remains crate-private and manifest-owned, with no SDK, runner, CLI, or public call graph promotion."
  - "The calls provider runs after polint.cfg and before polint.metrics so direct calls can consume CFG/MIR context before metrics remain unchanged."
  - "Calls cache identity includes semantic MIR, CFG, symbol graph, module topology, syntax, lifecycle, config, parameters, and absent extension/model/toolchain slots."

patterns-established:
  - "Recompute-only private providers can still publish deterministic provider output digests before persistent cache restore is added."
  - "Call output identity hashes stable keys, statuses, algorithms, reasons, precision, and compact labels rather than dense IDs or raw source."

requirements-completed: [SAE-SEM-05]

duration: 8 min
completed: 2026-05-21
---

# Phase 30 Plan 02: Calls Provider Slot and Cache Identity Summary

**Private polint.calls provider shell with deterministic output digest and future-fit calls layer keys**

## Performance

- **Duration:** 8 min
- **Started:** 2026-05-21T07:58:55Z
- **Completed:** 2026-05-21T08:07:10Z
- **Tasks:** 2
- **Files modified:** 9

## Accomplishments

- Added the private `polint.calls` manifest with `calls-facts-1:1`, inputs from source/functions/symbols/references/import topology/MIR/CFG, and outputs `call_sites`, `call_targets`, and `unresolved_calls`.
- Wired `AnalysisKernel::run` to execute `derive_calls_with_cache_stats` after `polint.cfg` and before `polint.metrics`, publishing an empty normalized `CallOutput` and recompute count `1`.
- Added `calls_provider_parameter_digest()` and `LayerKind::Calls` / `LayerKey::calls_layer_key` covering required upstream digests, lifecycle/config inputs, direct-call parameters, and absent extension/model/toolchain slots.
- Updated provider-order eval fixtures and tests so `polint.calls` appears as `provider_order.8` and metrics shifts to `provider_order.9`.

## Task Commits

1. **Task 1 RED:** `7689a5f` test(30-02): add failing test for calls provider slot
2. **Task 1 GREEN:** `e8e3db5` feat(30-02): wire private calls provider slot
3. **Task 2 RED:** `38870ba` test(30-02): add failing test for calls layer key
4. **Task 2 GREEN:** `8ead317` feat(30-02): add calls cache identity vocabulary

## Files Created/Modified

- `crates/polint/src/analysis/calls/provider.rs` - Calls provider shell, deterministic output digest, provider diagnostics, and digest tests.
- `crates/polint/src/analysis/calls/cache_key.rs` - Calls provider parameter digest vocabulary.
- `crates/polint/src/analysis/calls/mod.rs` - Registered `cache_key` and `provider` modules.
- `crates/polint/src/analysis_kernel/provider.rs` - Added `polint.calls` manifest, schema, and provider-order assertions.
- `crates/polint/src/analysis_kernel/mod.rs` - Runs calls provider after CFG and records its provider output metadata.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Added `LayerKind::Calls`, `calls_layer_key`, and layer-key tests.
- `crates/polint/src/eval/observed.rs` - Updated provider-order expected test rows.
- `crates/polint/src/eval/fixtures.rs` - Updated native fixture provider-order assertions.
- `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml` - Added `polint.calls` fixture expectation.

## Decisions Made

- Kept calls internals crate-private; public `call_graph` remains unsupported and no SDK/query/docs surface was promoted.
- Used a recompute-only provider shell for this plan; persistent restore can be added after extraction/resolution produce real rows.
- Included stable row payload fragments in the provider output digest and explicitly avoided raw source, AST dumps, absolute paths, timestamps, and run-local dense IDs as identity.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib provider_order --locked`
- `cargo test -p polint --lib calls_provider --locked`
- `cargo test -p polint --lib kernel_run_report_calls_row_carries_output_digest --locked`
- `cargo test -p polint --lib calls_layer_key --locked`
- `cargo test -p polint --lib calls_provider_parameters --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None.

## Threat Flags

None - new provider and cache identity trust-boundary changes were covered by the plan threat model.

## Next Phase Readiness

Ready for Plan 30-03 to populate call-site extraction on top of the private provider and cache identity established here.

## Self-Check: PASSED

- Verified created key files exist.
- Verified all task commit hashes exist in git history.
- Verified summary file exists.

---
*Phase: 30-direct-call-facts*
*Completed: 2026-05-21*
