---
phase: 29-local-cfg-and-control-dependence
plan: 03
subsystem: static-analysis-engine
tags: [rust, cfg, provider, cache-key, validation, debug]

requires:
  - phase: 29-local-cfg-and-control-dependence
    plan: 01
    provides: private CFG fact contracts and storage
  - phase: 29-local-cfg-and-control-dependence
    plan: 02
    provides: shared CFG builder and derived analyses
provides:
  - private `polint.cfg` provider slot
  - CFG provider manifest and run-report row
  - CFG cache-key vocabulary
  - CFG structural validation
  - test-only CFG debug JSON
affects: [phase-29, phase-30-direct-calls, phase-31-domains]

tech-stack:
  added: []
  patterns: [private provider wiring, deterministic provider digests, internal validation, test-only debug rows]

key-files:
  created:
    - crates/polint/src/analysis/cfg/provider.rs
    - crates/polint/src/analysis/cfg/cache_key.rs
    - crates/polint/src/analysis/cfg/validate.rs
  modified:
    - crates/polint/src/analysis/cfg/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/validation.rs
    - crates/polint/src/analysis_kernel/debug.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml

key-decisions:
  - "Run `polint.cfg` after `polint.semantic_mir` and before `polint.metrics`."
  - "Accept an empty CFG provider output until language lowering plans populate real graph rows."
  - "Keep CFG validation and debug output crate-private/test-facing with no SDK, runner, CLI, or public JSON surface."

patterns-established:
  - "CFG output digests include provider/schema, input snapshot lifecycle/config/model/extension/tool slots, semantic MIR output, syntax outputs, and CFG stable keys."
  - "CFG layer keys include graph-view parameters and absent extension/model/toolchain slots."
  - "CFG debug rows use compact payload fragments and repo-relative paths."

requirements-completed: []

duration: 34 min
completed: 2026-05-20
---

# Phase 29 Plan 03: CFG Provider, Cache Key, Validation, and Debug Summary

**Private `polint.cfg` provider wiring with deterministic identity, validation, and test-only visibility**

## Performance

- **Duration:** 34 min
- **Completed:** 2026-05-20
- **Tasks:** 3
- **Files modified:** 12

## Accomplishments

- Added `analysis::cfg::provider` with `derive_cfg_with_cache_stats`, deterministic output digesting, and empty-output support until language lowerers populate CFG rows.
- Added `polint.cfg` to provider manifests between `polint.semantic_mir` and `polint.metrics`, including CFG inputs, outputs, schema `cfg-facts-1:1`, and setup-aware precision ceiling.
- Wired `AnalysisKernel::run` to execute CFG after semantic MIR and record a run-report provider row with output digest/cache stats.
- Added CFG layer-key vocabulary and tests covering source, lifecycle, config, syntax output, semantic MIR output, graph-view parameters, and absent extension/model/toolchain slots.
- Added CFG structural validation and test-only CFG debug JSON rows.
- Updated eval provider-order invariants and fixture expectations for the new provider slot.

## Task Commits

1. **Tasks 1-3:** `86d9aa3` feat - CFG provider, cache-key, validation, debug wiring.

## Files Created/Modified

- `crates/polint/src/analysis/cfg/provider.rs` - Private CFG provider shell, output digest, and tests.
- `crates/polint/src/analysis/cfg/cache_key.rs` - CFG provider parameter digest.
- `crates/polint/src/analysis/cfg/validate.rs` - CFG structural validation.
- `crates/polint/src/analysis_kernel/provider.rs` - `polint.cfg` manifest and provider order tests.
- `crates/polint/src/analysis_kernel/mod.rs` - Kernel execution slot and run-report test.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - `LayerKind::Cfg`, `cfg_layer_key`, and key tests.
- `crates/polint/src/analysis_kernel/validation.rs` - CFG validation hook and validation tests.
- `crates/polint/src/analysis_kernel/debug.rs` - Test-only CFG debug rows and tests.
- `crates/polint/src/eval/observed.rs`, `crates/polint/src/eval/fixtures.rs`, `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml` - Provider-order fixture alignment.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Replaced invalid multi-filter Cargo commands**
- **Found during:** Verification
- **Issue:** Plan verification listed multiple Cargo test filters in one command, which Cargo rejects.
- **Fix:** Ran equivalent single-filter commands for `cfg_provider`, `provider_order`, `kernel_run_report_cfg_row_carries_output_digest`, `cfg_layer_key`, `analysis_kernel::validation::cfg`, and `analysis_kernel::debug::cfg_debug_json`.
- **Files modified:** None
- **Verification:** Replacement commands passed.
- **Committed in:** N/A

**2. [Rule 1 - Bug] Updated eval provider-order invariants**
- **Found during:** `provider_order` verification
- **Issue:** Eval tests and the provider-order fixture still expected `polint.metrics` at index 7.
- **Fix:** Inserted `polint.cfg` at index 7 and moved `polint.metrics` to index 8 in eval assertions and fixture TOML.
- **Files modified:** `crates/polint/src/eval/observed.rs`, `crates/polint/src/eval/fixtures.rs`, `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml`
- **Verification:** `cargo test -p polint --lib provider_order --locked` passed.
- **Committed in:** `86d9aa3`

**3. [Rule 2 - Missing Critical] Tightened setup-aware precision ceiling validation**
- **Found during:** CFG validation work
- **Issue:** Generic setup-aware provider ceiling logic allowed `FactPrecision::Exact`, while CFG and semantic MIR providers must not claim exact precision.
- **Fix:** Changed setup-aware ceiling validation to reject `Exact`; existing semantic/topology validation tests still pass.
- **Files modified:** `crates/polint/src/analysis_kernel/validation.rs`
- **Verification:** `cargo test -p polint --lib analysis_kernel::validation --locked` passed.
- **Committed in:** `86d9aa3`

---

**Total deviations:** 3 auto-fixed (1 Rule 1, 1 Rule 2, 1 Rule 3)
**Impact on plan:** Deviations completed required provider-order integration and strengthened precision honesty; no public surface was added.

## Issues Encountered

- The CFG provider intentionally emits empty output until Go/TS lowering plans populate real CFG rows.
- The plan’s multi-filter Cargo commands were translated to valid single-filter commands.

## Verification

- `cargo test -p polint --lib cfg_provider --locked` passed.
- `cargo test -p polint --lib provider_order --locked` passed.
- `cargo test -p polint --lib kernel_run_report_cfg_row_carries_output_digest --locked` passed.
- `cargo test -p polint --lib cfg_layer_key --locked` passed.
- `cargo test -p polint --lib analysis_kernel::validation::cfg --locked` passed.
- `cargo test -p polint --lib analysis_kernel::debug::cfg_debug_json --locked` passed.
- `cargo test -p polint --lib analysis_kernel::validation --locked` passed.
- `cargo test -p polint --lib analysis::cfg --locked` passed.
- `cargo fmt --all -- --check` passed.

## Known Stubs

- `derive_cfg_output` currently returns empty `CfgOutput`; Plans 29-04 and 29-05 are responsible for Go and TS/JS lowering into this provider.

## Threat Flags

None.

## User Setup Required

None.

## Next Phase Readiness

Plan 29-04 can now add Go CFG lowering into a real `polint.cfg` provider slot with validation, cache identity, run-report metadata, and debug visibility already wired.

## Self-Check: PASSED

- Verified created files exist.
- Verified the task commit exists in git history.
- Verified targeted tests and formatting pass.

---
*Phase: 29-local-cfg-and-control-dependence*
*Completed: 2026-05-20*
