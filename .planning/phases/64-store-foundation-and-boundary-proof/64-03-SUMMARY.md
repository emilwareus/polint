---
phase: 64-store-foundation-and-boundary-proof
plan: 03
subsystem: analysis-kernel
tags: [kernel, telemetry, parity, benchmark, sqlite]

# Dependency graph
requires:
  - phase: 64-store-foundation-and-boundary-proof
    plan: 02
    provides: Typed store maintenance, connection policy, contention, and recovery outcomes
  - phase: 63-ground-truth-and-performance-baseline
    provides: Isolated CurvePoint runner and deterministic diagnostics digest
provides:
  - Post-validation/finalization store maintenance with private KernelRunReport telemetry
  - Six-mode byte-identical JSON and exit-code parity proof
  - Test-only isolated store-enabled benchmark mode with actual non-double-counted store bytes
affects: [phase-64-plan-04, semantic-store, benchmarks, public-boundary]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Store maintenance runs after fact validation/finalization and cannot alter kernel outputs"
    - "Internal benchmark mode crosses the child boundary through a libtest-only environment key"

key-files:
  created: []
  modified:
    - crates/polint/src/cache/mod.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis_kernel/store/mod.rs
    - crates/polint/src/analysis_kernel/store/connection.rs
    - crates/polint/src/eval/bench/runner.rs
    - crates/polint/src/eval/performance.rs

key-decisions:
  - "StoreStatus is re-exported only crate-internally from analysis_kernel so KernelRunReport can retain typed telemetry without exposing the facade or rusqlite"
  - "All production Cache constructors remain store-disabled; only cfg(test) kernel and benchmark paths can enable maintenance"
  - "cache_bytes excludes semantic-store directory bytes so enabled measurements report each byte exactly once"

patterns-established:
  - "Behavior-neutral side effect: validated output is finalized first, then maintenance status is appended only to private telemetry"
  - "Parity fixtures compare rendered bytes and exit threshold, not merely diagnostic counts"

requirements-completed: [STORE-06, STORE-07, PERF-03, PROD-01]

# Metrics
duration: 16min
completed: 2026-07-10
---

# Phase 64 Plan 03: Kernel Integration and Behavior Parity Summary

**Validated kernel runs now record private store status after finalization, while six store states produce identical check JSON/exit semantics and the isolated harness measures real enabled-store bytes.**

## Performance

- **Duration:** ~16 min
- **Started:** 2026-07-10T10:33:18Z
- **Completed:** 2026-07-10T10:49:28Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Hooked `SemanticStore::maintain` strictly after `validate_fact_metadata` and `finish_all_fact_meta_insertions`, recording only typed `StoreStatus` in the private run report.
- Proved disabled full-kernel runs create no semantic-store directory or database, while enabled runs create a valid current schema after facts validate.
- Ran the same real kernel/render/exit helper through disabled, ready, corrupt, future, invalid, and contended states; all produced byte-identical normalized JSON and identical exit codes with distinct internal statuses.
- Added an isolated child benchmark mode that enables the test-only cache seam, reports actual store-directory bytes, excludes those bytes from `cache_bytes`, and preserves the disabled diagnostics digest.

## Task Commits

Each task was committed atomically:

1. **Task 1: Hook post-validation maintenance and private telemetry** - `e8747240` (feat)
2. **Task 2: Prove six-mode check behavior parity** - `693f84c8` (test)
3. **Task 3: Measure explicit enabled-store overhead and bytes** - `7584073b` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/mod.rs` - Post-finalization hook, status test accessor, ordering/disabled/parity fixtures.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Private `store_status` telemetry field.
- `crates/polint/src/cache/mod.rs` - Production-consumed private path/activation accessors; test enablement remains cfg-only.
- `crates/polint/src/analysis_kernel/store/mod.rs`, `connection.rs` - Typed test fixtures/guards and current-schema validation without exposing SQLite handles.
- `crates/polint/src/eval/bench/runner.rs` - Store bench mode, child protocol, real size accounting, and digest helper.
- `crates/polint/src/eval/performance.rs` - Internal run-report fixture updated for typed disabled status.

## Decisions Made

- Re-exported only the raw-free `StoreStatus` at crate visibility from `analysis_kernel`; the store module, facade, connections, SQL, row IDs, and errors remain inaccessible outside their intended internal boundary.
- Used a dedicated `POLINT_PERF_CHILD_SEMANTIC_STORE` key only in cfg(test) code and explicitly removed inherited values before child selection, preventing accidental recursive/product activation.
- Calculated `store_bytes` from `CacheLayout::semantic_store_dir()` and subtracted the same directory from aggregate cache bytes, preventing footprint double-counting.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Build] Scoped pre-integration dead-code expectations to future-only helpers**

- **Found during:** Task 1 pre-commit lint.
- **Issue:** Removing the module-wide pre-integration expectation correctly exposed read-only/rebuild helpers that Phase 64 establishes but later phases consume.
- **Fix:** Added narrow, reasoned non-test expectations only to those future-only helper items; integrated maintenance paths remain warning-clean without suppression.
- **Files modified:** `crates/polint/src/analysis_kernel/store/mod.rs`, `connection.rs`
- **Verification:** Full workspace clippy passed with `-D warnings`.
- **Committed in:** `e8747240`

**2. [Rule 3 - Blocking] Updated the existing performance test fixture for the new run-report constructor**

- **Found during:** Task 1 compilation.
- **Issue:** The sole synthetic `KernelRunReport::new` caller outside the kernel needed the new typed status argument.
- **Fix:** Passed `StoreStatus::Disabled`, matching the fixture's no-store behavior.
- **Files modified:** `crates/polint/src/eval/performance.rs`
- **Verification:** Workspace lint and plan test suite passed.
- **Committed in:** `e8747240`

---

**Total deviations:** 2 auto-fixed (1 build/lint, 1 blocking compile ripple)
**Impact on plan:** Both were required consequences of replacing the temporary foundation state with the integrated typed contract; no public or product scope changed.

## Issues Encountered

None beyond the auto-fixed integration ripples above.

## User Setup Required

None - production activation remains disabled and no CLI/config/documented environment contract was added.

## Verification

- `cargo test -p polint --lib analysis_kernel::tests::semantic_store --locked -- --test-threads=1` - 3 matching tests passed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity --locked -- --test-threads=1` - parity test passed.
- `cargo test -p polint --lib eval::bench::runner::tests::semantic_store --locked -- --test-threads=1` - isolated size/digest test passed.
- `cargo fmt --all -- --check` - passed.
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings` - passed.

## Next Phase Readiness

- Plan 64-04 can run the outside-consumer leak proof and apply the locked Phase 63 regression budgets to a real isolated store-enabled measurement.
- No blockers.

---
*Phase: 64-store-foundation-and-boundary-proof*
*Completed: 2026-07-10*

## Self-Check: PASSED

The kernel/report/benchmark integration files and this summary exist, and commits `e8747240`, `693f84c8`, and `7584073b` are in git history.
