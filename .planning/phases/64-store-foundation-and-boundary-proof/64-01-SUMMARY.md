---
phase: 64-store-foundation-and-boundary-proof
plan: 01
subsystem: database
tags: [sqlite, rusqlite, migrations, cache-layout, private-boundary]

# Dependency graph
requires:
  - phase: 63-ground-truth-and-performance-baseline
    provides: Locked store-disabled baseline and regression budgets
provides:
  - Bundled rusqlite dependency kept behind a crate-private module
  - Cache-owned semantic-store path and disabled-by-default activation state
  - Transactional, idempotent schema-v1 bootstrap migrations with strict future/invalid refusal
affects: [phase-64-plan-02, semantic-store, cache, migrations]

# Tech tracking
tech-stack:
  added: [rusqlite 0.40.1 with bundled SQLite]
  patterns:
    - "Store configuration crosses the boundary as typed path/state values; rusqlite and SQL remain private"
    - "PRAGMA user_version is checked before writes and updated inside the migration transaction"

key-files:
  created:
    - crates/polint/src/analysis_kernel/store/mod.rs
    - crates/polint/src/analysis_kernel/store/migrations.rs
  modified:
    - Cargo.toml
    - Cargo.lock
    - crates/polint/Cargo.toml
    - crates/polint/src/cache/mod.rs
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "Production cache constructors hard-code semantic-store activation off; only cfg(test) code can enable it during Phase 64"
  - "Schema v1 contains only _polint_schema_migrations bookkeeping, preserving Phase 65 ownership of manifest/generation/fact tables"
  - "Current-version databases must satisfy the bootstrap invariant; they are never silently repaired"

patterns-established:
  - "Disabled-first boundary: activation is inspected before filesystem metadata, directory creation, or SQLite open"
  - "Strict migrations: future versions are refused unchanged, current versions are validated, and supported upgrades are atomic"

requirements-completed: [STORE-01, STORE-02, PERF-03]

# Metrics
duration: 35min
completed: 2026-07-10
---

# Phase 64 Plan 01: Store Foundation and Migration Substrate Summary

**A disabled-by-default, cache-owned SQLite boundary now bootstraps schema v1 transactionally while refusing future and malformed stores without leaking database types.**

## Performance

- **Duration:** ~35 min
- **Started:** 2026-07-10T09:45:00Z
- **Completed:** 2026-07-10T10:20:09Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments

- Added the locked `rusqlite 0.40.1` dependency with bundled SQLite and no public feature or re-export.
- Derived `semantic-store/store.sqlite3` from the existing `CacheLayout`, while keeping every production cache disabled and proving config inspection is filesystem-free.
- Implemented ordered schema-v1 migrations with empty, explicit-v0, idempotent-current, future-version, and invalid-current fixtures.
- Preserved sentinel data through supported upgrades and proved future-version refusal leaves both `user_version` and data unchanged.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add the locked bundled rusqlite dependency** - `22000eee` (chore)
2. **Task 2: Add cache-owned private store configuration** - `e5693e0e` (feat)
3. **Task 3: Implement strict numbered store migrations** - `5e88e6ee` (feat)

## Files Created/Modified

- `Cargo.toml`, `Cargo.lock`, `crates/polint/Cargo.toml` - Locked bundled rusqlite dependency.
- `crates/polint/src/cache/mod.rs` - Cache-owned store path, disabled activation state, and zero-I/O tests.
- `crates/polint/src/analysis_kernel/mod.rs` - Private store module registration.
- `crates/polint/src/analysis_kernel/store/mod.rs` - Typed crate-private configuration and status vocabulary.
- `crates/polint/src/analysis_kernel/store/migrations.rs` - Transactional version runner, schema validation, and fixture matrix.

## Decisions Made

- Kept activation as a private `Cache` field initialized to false by every production constructor; Phase 64 integration tests use the only test-scoped enablement seam.
- Limited v1 to `_polint_schema_migrations(version)` plus `PRAGMA user_version = 1`; no later-phase persistence concepts were pulled forward.
- Treated a v1 database without its marker as invalid rather than recreating it, distinguishing corruption/tampering from an empty database.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Build] Removed a fulfilled lint expectation**

- **Found during:** Task 2 pre-commit lint.
- **Issue:** `CacheLayout::semantic_store_dir` was used by `semantic_store_path`, so its temporary `dead_code` expectation became unfulfilled under `-D warnings`.
- **Fix:** Removed only the obsolete expectation while retaining the private method and module-level pre-integration expectation.
- **Files modified:** `crates/polint/src/cache/mod.rs`
- **Verification:** Full workspace pre-commit lint passed.
- **Committed in:** `e5693e0e`

---

**Total deviations:** 1 auto-fixed (build/lint)
**Impact on plan:** No scope change; the fix aligned lint metadata with actual use.

## Issues Encountered

- The first SQLite-backed test build took several minutes because the large `polint` test binary had to be relinked; subsequent targeted tests completed immediately.

## User Setup Required

None - the store remains internally disabled and uses bundled SQLite.

## Verification

- `cargo test -p polint --lib cache::tests --locked` - 36 passed.
- `cargo test -p polint --lib analysis_kernel::store::migrations::tests --locked` - 7 passed.
- `cargo check -p polint --locked` - passed.
- `cargo fmt --all -- --check` - passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` - passed in each task's pre-commit hook.

## Next Phase Readiness

- Plan 64-02 can consume the typed facade and migration runner to add connection policy, deterministic contention handling, and safe recovery.
- No blockers.

---
*Phase: 64-store-foundation-and-boundary-proof*
*Completed: 2026-07-10*

## Self-Check: PASSED

Both store source files and this summary exist, and task commits `22000eee`, `e5693e0e`, and `5e88e6ee` are present in git history.
