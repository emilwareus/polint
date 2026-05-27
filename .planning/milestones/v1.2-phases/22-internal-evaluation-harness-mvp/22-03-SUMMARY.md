---
phase: 22-internal-evaluation-harness-mvp
plan: "03"
subsystem: evaluation-harness
tags: [rust, eval, fixtures, analysis-kernel, deterministic-json, internal-api]

requires:
  - phase: 22-internal-evaluation-harness-mvp
    provides: Canonical eval model and deterministic report hashing from Plan 22-01
  - phase: 22-internal-evaluation-harness-mvp
    provides: Generic matcher and metric aggregation from Plan 22-02
provides:
  - Crate-private native fixture manifest loading with path traversal defenses
  - Real-kernel observed item collection for native fixtures
  - First in-repo provider-order kernel fixture with deterministic output hash checks
affects: [22-04-baseline-management, evaluation-harness, promotion-gates]

tech-stack:
  added: []
  patterns:
    - crate-private fixture loading and execution under eval with no public CLI, SDK, runner, or crate-root surface
    - temp-dir fixture repo copies before kernel execution to avoid mutating checked-in fixture content
    - slash-normalized relative paths for fixture manifests and observed diagnostics
    - provider-order invariants sourced from AnalysisKernel provider manifests

key-files:
  created:
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/src/eval/observed.rs
    - tests/eval-fixtures/README.md
    - tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml
    - tests/eval-fixtures/kernel/provider-order/repo/.polint.toml
    - tests/eval-fixtures/kernel/provider-order/repo/src/app.ts
    - .planning/phases/22-internal-evaluation-harness-mvp/22-03-SUMMARY.md
  modified:
    - crates/polint/src/eval/mod.rs

key-decisions:
  - "Keep native fixture loading, observation, and execution crate-private/test-facing under eval."
  - "Copy fixture repos into temporary directories before calling AnalysisKernel::run so checked-in fixture content is not mutated."
  - "Use AnalysisKernel::provider_manifests() as the source of provider-order observed invariants instead of duplicating expected fixture text."
  - "Record runtime budget pass/fail while keeping exact observed elapsed milliseconds out of deterministic output hashes."

patterns-established:
  - "Native fixtures use tests/eval-fixtures/<area>/<case>/repo plus expected.polint-eval.toml."
  - "Fixture-owned relative paths reject absolute paths, parent-dir traversal, Windows drive prefixes, and leading slash/backslash input."
  - "Observed metadata fact rows come from metadata_debug_json_for_test and are normalized before matching."

requirements-completed: [SAE-FND-03]

duration: 12 min
completed: 2026-05-17
---

# Phase 22 Plan 03: Native Fixture Runner Summary

**Native fixture execution against real AnalysisKernel output with deterministic provider-order evidence**

## Performance

- **Duration:** 12 min
- **Started:** 2026-05-17T17:00:11Z
- **Completed:** 2026-05-17T17:12:24Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added `eval::fixtures` with TOML fixture manifest loading, schema validation, repo path containment checks, relative path normalization, optional runtime budgets, and a crate-private test runner.
- Added `eval::observed` with real `AnalysisKernel::run` execution over copied fixture repos, normalized diagnostics, provider-order invariants, metadata debug fact rows, and runtime budget observations.
- Added the first native kernel fixture, `tests/eval-fixtures/kernel/provider-order`, asserting the six current provider IDs in deterministic execution order.
- Documented the fixture directory contract and the rule that external benchmark content must not be committed under `tests/eval-fixtures`.

## Task Commits

Each TDD step was committed atomically:

1. **Task 1 RED: Add failing native fixture manifest tests** - `1f3516d` (test)
2. **Task 1 GREEN: Implement native fixture manifest loading** - `d6fbd96` (feat)
3. **Task 2 RED: Add failing real-kernel observed item tests** - `09eec80` (test)
4. **Task 2 GREEN: Collect observed items from real kernel fixtures** - `b2ea134` (feat)
5. **Task 3 RED: Add failing native fixture runner test** - `63275fb` (test)
6. **Task 3 GREEN: Wire native fixture runner** - `d992db8` (feat)

**Plan metadata:** pending final docs commit.

## Files Created/Modified

- `crates/polint/src/eval/fixtures.rs` - Native fixture manifest contracts, path safety, fixture loading tests, and test-facing fixture runner.
- `crates/polint/src/eval/observed.rs` - Real-kernel observation path, temp repo copy logic, normalized observed rows, and observed-item tests.
- `crates/polint/src/eval/mod.rs` - Registers crate-private fixture and observed modules.
- `tests/eval-fixtures/README.md` - Native fixture layout and repository-boundary documentation.
- `tests/eval-fixtures/kernel/provider-order/expected.polint-eval.toml` - First native kernel fixture manifest with six provider-order invariants.
- `tests/eval-fixtures/kernel/provider-order/repo/.polint.toml` - Minimal fixture repo config.
- `tests/eval-fixtures/kernel/provider-order/repo/src/app.ts` - Minimal TypeScript source used by the kernel fixture.

## Decisions Made

- Kept all fixture APIs crate-private and test-facing; no public CLI, SDK, runner, or crate-root surface was added.
- Ran kernel fixtures from copied temporary repos so `.polint/` cache writes and generated files never touch fixture-owned source directories.
- Rejected symlinks during fixture repo copying so fixture content cannot escape the declared repo tree during recursive copy.
- Compared provider-order expectations against observed rows generated from `AnalysisKernel::provider_manifests()`, preserving one source of truth for current provider IDs.
- Preserved deterministic hashing by matching runtime budget pass/fail while excluding exact observed durations from output hashes through the existing report normalization path.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Reject symlinks in native fixture repo copies**

- **Found during:** Task 2
- **Issue:** Copying fixture repos recursively without an explicit symlink guard could allow fixture content to escape the fixture-owned tree.
- **Fix:** Added symlink rejection to the observed fixture copy path before real kernel execution.
- **Files modified:** `crates/polint/src/eval/observed.rs`
- **Commit:** `b2ea134`

## Issues Encountered

None unresolved. The only stub-scan hits were intentional `exclude = []` fixture config values, including one embedded in an observed-kernel unit test.

## Verification

- `cargo test -p polint --lib eval_fixture_manifest --locked`
- `cargo test -p polint --lib eval_observed_kernel --locked`
- `cargo test -p polint --lib analysis_kernel --locked`
- `cargo test -p polint --lib eval_native_fixture_runner --locked`
- `cargo test -p polint --lib eval --locked`
- `cargo clippy -p polint --lib --all-features --locked -- -D warnings`
- `cargo test --workspace --all-features --locked`
- `cargo fmt --all -- --check`

## Known Stubs

None. Stub scan found no TODO/FIXME markers, placeholder text, or hardcoded empty values that flow to user-visible output. The `exclude = []` matches are intentional minimal fixture configuration, not incomplete data flow.

## Threat Flags

None. The new file-system surface is the planned native fixture loader/runner, remains crate-private/test-facing, constrains fixture paths to fixture-owned directories, rejects traversal and symlink escape, and executes only copied in-repo fixture content.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Plan 22-04 can build baseline management on top of executable native fixtures, real-kernel observation, deterministic matching, and stable provider-order evidence from Plans 22-01 through 22-03.

## Self-Check: PASSED

- Found created files: `crates/polint/src/eval/fixtures.rs`, `crates/polint/src/eval/observed.rs`, `tests/eval-fixtures/README.md`, the provider-order fixture files, and this summary.
- Found task commits: `1f3516d`, `d6fbd96`, `09eec80`, `b2ea134`, `63275fb`, and `d992db8`.

---
*Phase: 22-internal-evaluation-harness-mvp*
*Completed: 2026-05-17*
