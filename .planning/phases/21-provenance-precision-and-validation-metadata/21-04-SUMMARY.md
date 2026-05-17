---
phase: 21-provenance-precision-and-validation-metadata
plan: 04
subsystem: analysis-kernel
tags: [rust, metadata, provenance, debug-json, public-api-compatibility]

requires:
  - phase: 21-provenance-precision-and-validation-metadata
    provides: Internal metadata sidecar coverage and validation diagnostics from Plans 21-01 through 21-03
provides:
  - Deterministic crate-private metadata debug JSON for files, imports, symbols, and references
  - Public-boundary proof that metadata/debug helpers stay out of SDK, runner, and crate-root public API
  - CLI compatibility proof that public check JSON remains deterministic and metadata-free for external rules
affects: [analysis-kernel, public-api-boundary, cli-compatibility, future-evaluation-harness]

tech-stack:
  added: []
  patterns:
    - Test-only debug reports serialized from typed structs with deterministic row sorting
    - External-rule compatibility checks use public SDK prelude imports only

key-files:
  created:
    - crates/polint/src/analysis_kernel/debug.rs
  modified:
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/tests/cli.rs

key-decisions:
  - "Metadata debug JSON remains behind cfg(test) and crate-private AnalysisKernel helpers, with no SDK, runner, or public CLI surface."
  - "Debug rows use SourceFile.relative_path and explicit row sorting by path/span/name/stable key/run id to avoid machine-local or transient details."
  - "Public compatibility is proven through a temp-repo external rule importing only polint::sdk::prelude::* and checking metadata-only keys stay out of public JSON."

patterns-established:
  - "Internal provenance inspection should be exposed through crate-private test helpers until a later phase deliberately promotes a public inspect/test contract."
  - "Metadata-only field names are guarded against accidental public JSON exposure in CLI tests."

requirements-completed: [SAE-FND-02]

duration: 11m
completed: 2026-05-17
---

# Phase 21 Plan 04: Metadata Debug JSON Summary

**Deterministic internal provenance debug JSON for core fact families with public CLI/API compatibility proof**

## Performance

- **Duration:** 11m
- **Started:** 2026-05-17T07:48:48Z
- **Completed:** 2026-05-17T07:59:44Z
- **Tasks:** 2
- **Files modified:** 3

## Accomplishments

- Added `analysis_kernel::debug` with test-only metadata debug JSON rows for files, imports, symbols, and references.
- Exposed `AnalysisKernel::metadata_debug_json_for_test` only under `#[cfg(test)]`, preserving the crate-private metadata boundary.
- Added public compatibility tests proving SDK/runner/lib surfaces do not export metadata helpers and `polint check --format json` remains deterministic and metadata-free.

## Task Commits

1. **Task 1: Add deterministic crate-private provenance debug JSON** - `b9cc571` (test), `b5ec2ee` (feat)
2. **Task 2: Prove no public exposure and public behavior compatibility** - `423d3e5` (test)

_Note: Task 1 followed TDD with a failing test commit. Task 2 was test-only; the new compatibility tests passed immediately because Task 1 and existing public boundaries already satisfied the requested behavior._

## Files Created/Modified

- `crates/polint/src/analysis_kernel/debug.rs` - Test-only deterministic metadata debug report and public-boundary tests.
- `crates/polint/src/analysis_kernel/mod.rs` - Registers the debug module and crate-private test helper under `#[cfg(test)]`.
- `crates/polint/tests/cli.rs` - Adds `kernel_metadata_preserves_public_check_behavior` external-rule compatibility coverage.

## Decisions Made

- Kept provenance debug output internal/test-facing only; no CLI, runner, crate-root, or SDK contract was added.
- Serialized debug reports from typed structs with top-level `files`, `imports`, `symbols`, and `references` keys.
- Included existing symbol/reference precision as `fact_precision` so metadata `precision` remains unambiguous.

## Deviations from Plan

None - plan executed exactly as written.

## Known Stubs

None. Stub scan matches were existing fixture strings, TOML empty arrays such as `exclude = []`, and configured `TODO` literal tests; none are new unwired behavior or placeholders.

## Threat Flags

None. The only security-relevant surfaces introduced were the planned internal/test-only debug JSON and public-boundary assertions covered by T-21-04-01 through T-21-04-04.

## Issues Encountered

- Task 2's TDD red step did not fail because the behavior under test was already satisfied after Task 1 and existing public API boundaries; no product-code change was needed.
- A few parallel Cargo test invocations briefly waited on package/artifact locks; final verification was run sequentially and passed.

## User Setup Required

None - no external service configuration required.

## Verification

- `cargo test -p polint --lib metadata_debug --locked`
- `cargo test -p polint --lib metadata_debug_helpers_are_not_public --locked`
- `cargo test -p polint --test cli kernel_metadata_preserves_public_check_behavior --locked`
- `cargo test -p polint --test cli kernel_delegation_preserves_existing_rule_facts --locked`
- `cargo test -p polint --lib analysis_kernel --locked`
- `cargo test --workspace --all-features --locked`
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`
- `cargo fmt --all -- --check`

## Next Phase Readiness

SAE-FND-02 is now complete across Phase 21. Phase 22 can consume deterministic internal debug JSON and metadata validation behavior as fixture input for the internal evaluation harness without promoting a public metadata surface.

## Self-Check: PASSED

- Found created file: `crates/polint/src/analysis_kernel/debug.rs`
- Found summary file: `.planning/phases/21-provenance-precision-and-validation-metadata/21-04-SUMMARY.md`
- Found task commits: `b9cc571`, `b5ec2ee`, `423d3e5`

---
*Phase: 21-provenance-precision-and-validation-metadata*
*Completed: 2026-05-17*
