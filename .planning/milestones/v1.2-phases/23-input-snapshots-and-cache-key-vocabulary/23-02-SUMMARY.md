---
phase: 23-input-snapshots-and-cache-key-vocabulary
plan: 02
subsystem: analysis-kernel-cache
tags: [rust, cache-identity, input-snapshot, lifecycle, provider-manifest]

# Dependency graph
requires:
  - phase: 23-input-snapshots-and-cache-key-vocabulary
    provides: typed digest, key, cache stat, and provider metadata vocabulary from 23-01
  - phase: 20-private-analysis-kernel-facade
    provides: crate-private provider manifests and kernel boundary
provides:
  - crate-private deterministic InputSnapshot construction
  - source/config/rule/model/extension/provider schema identity components
  - Go lifecycle identity components with setup-missing gaps
  - TS/JS lifecycle identity components with resolver/source-set inputs
  - unsupported official tool invocation identity components
affects: [phase-23, phase-24, incremental-cache, lifecycle-cache-identity]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - deterministic serde snapshots with sorted source, lifecycle, and provider rows
    - explicit InputComponent status vocabulary for present, absent, unsupported, and setup-missing inputs
    - source text identity through typed digests without raw source or absolute path serialization

key-files:
  created:
    - crates/polint/src/analysis_kernel/incremental/input_snapshot.rs
  modified:
    - crates/polint/src/analysis_kernel/incremental/mod.rs

key-decisions:
  - "Keep InputSnapshot and lifecycle snapshot types crate-private under analysis_kernel::incremental."
  - "Use SourceFile.content_hash as SourceText digest identity and record filesystem metadata only as mtime_hint_present."
  - "Record official Go and TS/JS tool invocations as Unsupported when polint does not invoke those tools."
  - "Include provider language_scope and cache_policy in provider_manifest_digest inputs."

patterns-established:
  - "Input components carry name, status, typed digest, and sorted machine-path-free detail rows."
  - "Provider schema snapshots use sorted schema/input/output labels before digest construction."
  - "Lifecycle file components hash file contents but serialize only root-relative file names."

requirements-completed: [SAE-FND-04]

# Metrics
duration: 11m
completed: 2026-05-18
---

# Phase 23 Plan 02: Input Snapshots Summary

**Crate-private deterministic input snapshots for source, config, lifecycle, rule, model, extension, provider schema, and tool identity inputs.**

## Performance

- **Duration:** 11m
- **Started:** 2026-05-18T06:13:56Z
- **Completed:** 2026-05-18T06:24:59Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Added `InputSnapshot`, `FileSnapshot`, `InputComponent`, lifecycle snapshots, and provider schema rows under the internal incremental module.
- Covered source/config/rule/options/model/extension/provider manifest identity with deterministic pretty-JSON tests and no raw source/path/timestamp leakage.
- Added Go lifecycle components for module roots, lifecycle files, build tags, include-tests, package patterns, environment policy, unsupported tool invocation, and setup-missing files.
- Added TS/JS lifecycle components for package manifests, lockfiles, config files, resolver options, source-set membership, and unsupported tool invocation.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Input snapshot tests** - `f7501e7` (test)
2. **Task 1 GREEN: Source/config/rule/model/extension/provider snapshots** - `9b07628` (feat)
3. **Task 2 RED: Lifecycle snapshot tests** - `f0533fb` (test)
4. **Task 2 GREEN: Go and TS/JS lifecycle snapshots** - `5f3c991` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` - Defines deterministic crate-private input snapshot rows, lifecycle components, provider schema digest construction, and unit tests.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Registers `input_snapshot` and crate-private re-exports for later Phase 23/24 consumers.

## Decisions Made

- Snapshot construction is internal/test-facing only; no SDK, runner, crate-root public, CLI, or stable JSON contract was added.
- Lifecycle file components hash file bytes for identity but serialize only root-relative file names.
- Go setup gaps are represented as `SetupMissing` components instead of silently omitting affected files.
- Official tool invocations are `Unsupported` unless this code path actually invokes a tool; this plan added no process execution.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- A stale `#[expect(unused_imports)]` became unfulfilled after the input snapshot module consumed digest re-exports. Removed it before the Task 1 green commit.
- Initial lifecycle detail normalization stripped configured `./` prefixes from Go package patterns. Adjusted component detail normalization to preserve exact config values while still normalizing path separators.

## User Setup Required

None - no external service configuration required.

## Known Stubs

None.

## Verification

- `cargo test -p polint --lib input_snapshot::source_config_rule_model_extension --locked`
- `cargo test -p polint --lib input_snapshot::lifecycle --locked`
- `cargo test -p polint --lib input_snapshot --locked`
- `cargo fmt --all -- --check`
- Plan grep checks for required snapshot/lifecycle/provider fields passed.
- Tool process-term grep returned no matches in `input_snapshot.rs`.

## Next Phase Readiness

Phase 23 can continue with cache snapshot/key integration work. The input snapshot vocabulary is internal, deterministic, and now covers the required lifecycle and provider identity inputs without changing cache reuse behavior.

## Self-Check: PASSED

- Created files exist: `input_snapshot.rs`, updated `incremental/mod.rs`, and this summary.
- Commits exist: `f7501e7`, `9b07628`, `f0533fb`, and `5f3c991`.
- Final verification passed: `cargo test -p polint --lib input_snapshot --locked` and `cargo fmt --all -- --check`.

---
*Phase: 23-input-snapshots-and-cache-key-vocabulary*
*Completed: 2026-05-18*
