---
phase: 64-store-foundation-and-boundary-proof
plan: 02
subsystem: database
tags: [sqlite, wal, concurrency, recovery, symlink-safety]

# Dependency graph
requires:
  - phase: 64-store-foundation-and-boundary-proof
    plan: 01
    provides: Private store facade, cache-owned path, and strict schema-v1 migrations
provides:
  - Writer connections with foreign keys, WAL, 250 ms busy timeout, and immediate leases
  - Independent read-only SQLite connections and bounded typed contention fallback
  - Non-destructive corrupt/future/invalid handling with explicit verified-cache rebuild
affects: [phase-64-plan-03, semantic-store, kernel-integration, concurrency]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Raw SQLite connections and transactions are encapsulated in connection.rs; callers receive typed classifications"
    - "The cache root is a trust anchor and every store-relative existing component is checked for symlinks and containment"

key-files:
  created:
    - crates/polint/src/analysis_kernel/store/connection.rs
    - crates/polint/src/analysis_kernel/store/tests.rs
  modified:
    - crates/polint/src/analysis_kernel/store/mod.rs
    - crates/polint/src/repo_fs.rs

key-decisions:
  - "Writer contention is bounded at 250 ms and maps SQLITE_BUSY/SQLITE_LOCKED to BusySkipped"
  - "maintain never deletes corrupt, invalid, or future stores; replacement requires the explicit exact-path rebuild helper"
  - "The configured cache root is the managed trust anchor, avoiding rejection of platform-level symlinks above that root while rejecting symlinks inside it"

patterns-established:
  - "Single-writer lease: BEGIN IMMEDIATE is acquired and consumed wholly inside the private connection module"
  - "Recovery is classification-first: inspect/preserve during normal maintenance, verify ownership, then explicitly replace only rebuildable state"

requirements-completed: [STORE-02, STORE-03, STORE-07, STORE-08, VAL-02]

# Metrics
duration: 13min
completed: 2026-07-10
---

# Phase 64 Plan 02: Connection, Contention, and Recovery Summary

**The private store now serializes writers through bounded immediate leases, supports independent read-only access, and preserves untrusted database state until an exact cache-owned rebuild is requested.**

## Performance

- **Duration:** ~13 min
- **Started:** 2026-07-10T10:20:09Z
- **Completed:** 2026-07-10T10:33:18Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments

- Enforced `foreign_keys=ON`, WAL journal mode, and a 250 ms busy timeout for writer connections; read-only access uses a separate `SQLITE_OPEN_READ_ONLY` handle that cannot write.
- Proved two independent writers serialize deterministically: the loser reports busy in under one second, then succeeds after lease release with one unchanged bootstrap marker and `integrity_check=ok`.
- Mapped corrupt and invalid stores to rebuild-needed, future schemas to non-destructive skip, and unsafe/open failures to stable typed statuses without panics or public diagnostics.
- Added exact-path, canonical-containment, regular-file, and no-symlink checks before explicit rebuild; outside and symlink targets remain untouched.

## Task Commits

Each task was committed atomically:

1. **Task 1: Implement private connection policy and maintain outcomes** - `29af8e5a` (feat)
2. **Task 2: Prove bounded single-writer contention** - `48d84f4f` (test)
3. **Task 3: Add controlled recovery and explicit owned rebuild** - `7fb4a510` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/store/connection.rs` - Writer/read-only opens, pragma policy, error classification, immediate lease, and private test probes.
- `crates/polint/src/analysis_kernel/store/tests.rs` - Connection, contention, corrupt/future/invalid, rebuild, and symlink fixtures.
- `crates/polint/src/analysis_kernel/store/mod.rs` - Disabled-first maintenance facade, typed mapping, path preparation, and explicit rebuild.
- `crates/polint/src/repo_fs.rs` - Managed-root containment and component-level symlink verification.

## Decisions Made

- Used a fixed 250 ms busy timeout: it is long enough for normal short transactions but deterministically below the one-second fallback contract.
- Kept the held-transaction test helper test-only and opaque; even internal tests cannot inspect or pass around a raw `rusqlite::Transaction` field.
- Treated the declared cache root as the ownership anchor. Components below it are checked lexically and canonically, so managed-path symlinks are rejected without misclassifying operating-system path aliases above the cache root.
- Made explicit rebuild idempotent for healthy/current stores and non-destructive for future, busy, unsafe, disabled, or open-failed states.

## Deviations from Plan

None - plan executed as written.

## Issues Encountered

None.

## User Setup Required

None - connection policy and recovery remain private and test-enabled only.

## Verification

- `cargo test -p polint --lib analysis_kernel::store::tests --locked -- --test-threads=1` - 11 passed.
- `cargo test -p polint --lib analysis_kernel::store::migrations::tests --locked` - 7 passed.
- `cargo fmt --all -- --check` - passed.
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings` - passed.
- Full workspace lint passed in all three task pre-commit hooks.

## Next Phase Readiness

- Plan 64-03 can invoke `SemanticStore::maintain` after kernel validation/finalization and carry only `StoreStatus` in its private run report.
- No blockers.

---
*Phase: 64-store-foundation-and-boundary-proof*
*Completed: 2026-07-10*

## Self-Check: PASSED

Connection and recovery source files plus this summary exist, and commits `29af8e5a`, `48d84f4f`, and `7fb4a510` are present in git history.
