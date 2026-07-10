---
phase: 64-store-foundation-and-boundary-proof
reviewed: 2026-07-10T13:50:56Z
depth: standard
files_reviewed: 15
files_reviewed_list:
  - crates/polint/Cargo.toml
  - crates/polint/src/analysis_kernel/incremental/run_report.rs
  - crates/polint/src/analysis_kernel/mod.rs
  - crates/polint/src/analysis_kernel/store/connection.rs
  - crates/polint/src/analysis_kernel/store/migrations.rs
  - crates/polint/src/analysis_kernel/store/mod.rs
  - crates/polint/src/analysis_kernel/store/tests.rs
  - crates/polint/src/cache/mod.rs
  - crates/polint/src/eval/bench/gate.rs
  - crates/polint/src/eval/bench/runner.rs
  - crates/polint/src/eval/performance.rs
  - crates/polint/src/repo_fs.rs
  - crates/polint/tests/public_surface_leak.rs
  - tests/fixtures/public-surface-leak-probe/Cargo.lock
  - tests/fixtures/public-surface-leak-probe/src/lib.rs
findings:
  blocker: 0
  critical: 0
  warning: 2
  info: 0
  total: 2
status: issues_found
---

# Phase 64 Code Review Report

**Depth:** standard
**Diff base:** `origin/main`
**Status:** issues found

## Findings

### WR-01 — Fresh-store migrations run before the immediate writer lease

**Severity:** Warning
**Files:** `crates/polint/src/analysis_kernel/store/connection.rs:45-64`, `crates/polint/src/analysis_kernel/store/migrations.rs:38-63`, `crates/polint/src/analysis_kernel/store/mod.rs:99-106`, `crates/polint/src/analysis_kernel/store/tests.rs:93-102`

`open_writer` calls `apply_migrations` before `SemanticStore::maintain` calls
`try_writer_lease`. The migration runner reads `PRAGMA user_version` before
starting its own transaction, and `rusqlite::Connection::transaction()` uses a
deferred transaction by default. Two processes can therefore both observe an
absent store as version 0. If the first creates and commits the bootstrap table
before the second executes its migration SQL, the second receives
`SQLITE_ERROR: table _polint_schema_migrations already exists`, not
`SQLITE_BUSY`/`SQLITE_LOCKED`. `classify_sqlite_error` maps that to `Other`, and
the facade reports `Skipped(OpenFailed)` instead of the bounded
`BusySkipped`/serialized outcome required by D-07, D-08, and STORE-08.

The current contention test cannot catch this schedule because it initializes
the schema with `SemanticStore::maintain` before opening the two contending
connections. A two-connection SQL reproduction in this review confirmed that
the stale version-0 schedule ends with `SQLITE_ERROR` after the first migration
commits.

**Recommendation:** acquire `BEGIN IMMEDIATE` before migration writes and re-read
the schema version while that lease is held. Refactor migration execution so it
uses the already-held immediate transaction instead of opening a nested deferred
transaction. Add a deterministic absent-database contention fixture that forces
both connections through first-open initialization and asserts a typed bounded
outcome plus one valid bootstrap marker.

### WR-02 — Writer readiness does not verify that SQLite actually entered WAL mode

**Severity:** Warning
**File:** `crates/polint/src/analysis_kernel/store/connection.rs:54-64`

`PRAGMA journal_mode = WAL` returns the journal mode SQLite actually selected,
but `open_writer` discards that string with `let _: String`. SQLite may return
the previous mode without raising an error when a WAL transition is unavailable
(for example, when the VFS cannot support shared memory). The code then applies
migrations, acquires the lease, and can return `StoreStatus::Ready` even though
the D-07 connection policy is not in force. The policy test proves WAL on the
normal test filesystem only; it does not protect the production classification
path from a successful non-`wal` result.

**Recommendation:** retain and validate the returned mode case-insensitively.
Treat any non-`wal` result as a controlled open/policy failure rather than
returning `Ready`, and cover the result-validation branch with a focused test.

## Verified Behavior

- Store activation remains crate-private and every production `Cache`
  constructor leaves it disabled.
- The disabled kernel path computes a path only in memory and performs no store
  filesystem or SQLite I/O.
- Future and malformed current schemas are preflighted before persistent WAL
  setup; future fixture bytes, version, and sentinel data are preserved.
- Corrupt, invalid, future, unsafe-path, and established-schema contention paths
  map to controlled private statuses without affecting policy diagnostics.
- Store maintenance runs after fact validation/finalization and only enters the
  private `KernelRunReport` telemetry.
- The enabled/disabled/failure parity helper preserves rendered JSON bytes and
  exit semantics for its six covered modes.
- Rusqlite handles, SQL vocabulary, store types, and schema identifiers do not
  leak through SDK, runner, CLI, generated skill, examples, or public JSON.
- The SDK prelude remains exactly 115 names.
- The Phase 64 measurement includes first-open/migration cost after disabled
  analysis-cache priming and keeps semantic-store bytes separate from cache
  bytes.

## Verification Performed

- `cargo test -p polint --lib analysis_kernel::store --locked -- --test-threads=1` — 20 passed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store --locked -- --test-threads=1` — 3 passed.
- `cargo test -p polint --lib eval::bench::gate::tests::phase_64 --locked -- --test-threads=1` — 1 passed.
- `cargo test -p polint --test public_surface_leak --locked -- --test-threads=1` — 7 passed.
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings` — passed.
- Related workspace dependency inspection confirmed `rusqlite 0.40.1` with only
  the `bundled` feature and no public feature/re-export.

## Scope Notes

- The hard-link alias case remains a same-user/local-cache limitation rather
  than a cross-permission path escape; symlink targets and ancestors are
  rejected as required.
- The regression fixture is intentionally tiny and does not constitute a
  large-repository performance claim.

---
_Reviewed: 2026-07-10_
_Reviewer: Codex GSD code reviewer_
_Depth: standard_
