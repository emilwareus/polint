---
phase: 20-private-analysis-kernel-facade
plan: "02"
subsystem: core-analysis
tags: [rust, analysis-kernel, provider-manifests, test-only-inspection]

requires:
  - phase: 20-private-analysis-kernel-facade
    provides: Crate-private AnalysisKernel facade and behavior-preserving provider sequence
provides:
  - Deterministic internal provider manifests for source, Go syntax, TS/JS syntax, module graph, symbol graph, and metrics providers
  - Behavior-preserving production manifest consumption through AnalysisKernel::provider_manifests
  - Test-only crate-private provider order and manifest report helpers
affects: [21-provenance-precision-validation-metadata, analysis-kernel, provider-metadata]

tech-stack:
  added: []
  patterns:
    - static borrowed manifest metadata for current eager providers
    - production metadata consumption that does not drive scheduling or cache identity
    - cfg(test) provider inspection helpers only

key-files:
  created:
    - crates/polint/src/analysis_kernel/provider.rs
    - .planning/phases/20-private-analysis-kernel-facade/20-02-SUMMARY.md
  modified:
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "Keep provider manifests crate-private and consume them only for behavior-preserving metadata consistency in this phase."
  - "Keep provider execution order as explicit AnalysisKernel::run calls; manifest dependency data remains deterministic test metadata only."
  - "Expose provider order inspection only through #[cfg(test)] crate-private helpers, with no SDK, runner, or CLI contract."

patterns-established:
  - "ProviderManifest rows use static borrowed slices and schema rows, avoiding allocation for production inspection."
  - "Provider order reports include only id, kind, language scope, inputs, and outputs."

requirements-completed: [SAE-FND-01]

duration: 9 min
completed: 2026-05-16
---

# Phase 20 Plan 02: Provider Manifests Summary

**Crate-private provider manifests and test-only provider order inspection for the existing AnalysisKernel sequence**

## Performance

- **Duration:** 9 min
- **Started:** 2026-05-16T20:05:32Z
- **Completed:** 2026-05-16T20:15:08Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `analysis_kernel::provider` with crate-private `ProviderManifest`, `ProviderKind`, `LanguageScope`, `CachePolicy`, `SchemaVersion`, and `PrecisionCeiling`.
- Registered six deterministic manifest rows in current kernel execution order: `polint.source`, `polint.go.syntax`, `polint.ts.syntax`, `polint.module_graph`, `polint.symbol_graph`, and `polint.metrics`.
- Made `AnalysisKernel::run` consume `AnalysisKernel::provider_manifests()` through a metadata token path that reads all manifest fields without changing scheduling, diagnostics, or cache keys.
- Added `#[cfg(test)]` crate-private provider order/report helpers and tests proving deterministic rows, path-stable output, and no SDK/runner/CLI exposure.

## Task Commits

Each TDD step was committed atomically:

1. **Task 1 RED: Add failing provider manifest tests** - `658e8c5` (test)
2. **Task 1 GREEN: Implement provider manifests** - `5169448` (feat)
3. **Task 2 RED: Add failing provider order inspection tests** - `4a19877` (test)
4. **Task 2 GREEN: Add test-only provider order inspection** - `c736168` (feat)

**Plan metadata:** pending final docs commit.

## Files Created/Modified

- `crates/polint/src/analysis_kernel/provider.rs` - Internal manifest model, concrete provider rows, cfg(test) order/report helpers, and focused unit tests.
- `crates/polint/src/analysis_kernel/mod.rs` - Crate-private manifest re-exports, `AnalysisKernel::provider_manifests`, and production metadata consumption.

## Decisions Made

- Manifest dependency data is deterministic metadata only in this phase; the existing eager provider calls remain the source of execution behavior.
- Cache policy rows describe current behavior (`NoCache`, existing Go/TS file fact cache schemas, and in-memory derived providers) but are not used for cache identity.
- Provider inspection is intentionally test-only. No public CLI command, SDK view, runner API, or crate-root public module was added.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None. The full verification suite passed after the planned implementation.

## Verification

- `cargo test -p polint --lib provider_manifests_cover_existing_kernel_providers --locked`
- `cargo test -p polint --lib provider_manifests --locked`
- `cargo test -p polint --lib provider_order --locked`
- `cargo test -p polint --lib provider_manifest_dependencies_are_deterministic_metadata --locked`
- `cargo test -p polint --lib provider_manifests_are_not_public_sdk_runner_or_cli_contract --locked`
- `cargo test -p polint --lib analysis_kernel --locked`
- `cargo test -p polint --test cli kernel_delegation_preserves_existing_rule_facts --locked`
- `cargo clippy -p polint --lib --all-features --locked -- -D warnings`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo test --workspace --all-features --locked`
- Structural `rg` checks confirmed manifest model types, both manifest accessors, all six provider ids, schema names, test-only helper shape, no manifest public exposure, and no phase-expansion terms in `provider.rs`.

## Known Stubs

None. Stub-pattern scan found no placeholder values or TODO/FIXME markers in the files created or modified by this plan.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 20 is complete. The private kernel facade and manifest/order-inspection foundation are ready for Phase 21 provenance, precision, validation, stable-key, and merge metadata work.

## Self-Check: PASSED

- Confirmed `crates/polint/src/analysis_kernel/provider.rs` and this summary exist.
- Confirmed task commits exist: `658e8c5`, `5169448`, `4a19877`, and `c736168`.

---
*Phase: 20-private-analysis-kernel-facade*
*Completed: 2026-05-16*
