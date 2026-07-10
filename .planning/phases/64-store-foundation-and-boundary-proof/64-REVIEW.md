---
phase: 64-store-foundation-and-boundary-proof
reviewed: 2026-07-10T14:16:02Z
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
  warning: 0
  info: 0
  total: 0
status: clean
---

# Phase 64 Code Review Report

**Depth:** standard
**Diff base:** `origin/main`
**Fix commits reviewed:** `b3b4823e`, `e9c11275`
**Status:** clean

## Result

No blocker, critical, warning, or informational findings remain in the fixed
15-file Phase 64 scope.

## Prior Warning Resolution

### WR-01 — Resolved: fresh-store migrations are serialized

- `open_writer` applies the connection policy and then acquires an immediate
  transaction before the migration runner re-reads `PRAGMA user_version`.
- Migration selection, SQL writes, version updates, current-schema validation,
  and commit all use that same transaction; the runner does not open a nested
  transaction.
- Any migration or validation error drops the still-owned transaction, so
  uncommitted schema work rolls back. A successful initialization commits before
  the writer is returned.
- Immediate-transaction contention is classified as the typed `Busy` outcome,
  which the store facade maps to `BusySkipped` within the 250 ms policy bound.
- The absent-store fixture opens two version-zero connections before the first
  initialization lease, holds that lease, proves the second initializer returns
  `Busy`, then releases and proves the second re-reads the committed schema as
  current. The final marker count is one and `integrity_check` is `ok`, covering
  the stale-v0 schedule that motivated the finding.
- Existing future-schema preflight still occurs before persistent pragma setup,
  and the in-transaction version check remains authoritative. Refused future
  fixtures retain their version, sentinel data, and original bytes.

### WR-02 — Resolved: WAL negotiation is required

- The value returned by `PRAGMA journal_mode = WAL` is retained and validated
  with ASCII case-insensitive comparison.
- `wal` in mixed case is accepted; every other successful result becomes the
  private typed `Policy` error.
- `Policy` maps to the controlled `Skipped(OpenFailed)` store status and cannot
  produce `Ready` or affect public diagnostics/exit behavior.
- Focused tests exercise both the mixed-case acceptance branch and the non-WAL
  rejection branch. The normal-filesystem policy test additionally proves the
  live writer reports WAL.

## Cross-File Review

- Store activation remains crate-private and test-only. All ordinary `Cache`
  constructors keep the store disabled, and the disabled kernel hook performs
  no store filesystem or SQLite I/O.
- Store maintenance remains after fact validation and metadata finalization.
  Its status is private run-report telemetry and is not an input to providers,
  rules, diagnostics, JSON rendering, or exit-code calculation.
- Corrupt and invalid stores are preserved for controlled rebuild; future stores
  are refused without replacement. Explicit rebuild remains restricted to the
  verified cache-owned path and rejects symlink targets/ancestors.
- Writer connections enforce foreign keys, WAL, a 250 ms busy timeout, and
  immediate transactions. Read-only connections use a separate
  `SQLITE_OPEN_READ_ONLY` handle and reject writes.
- Store and migration errors are typed and non-panicking in production paths.
  Transaction commit/drop behavior does not leave partial migrations.
- The six-mode kernel parity proof keeps normalized JSON bytes and exit semantics
  identical across disabled, ready, corrupt, future, invalid, and busy states.
- Rusqlite handles, SQL/table vocabulary, store status/configuration, and raw row
  identifiers remain absent from the supported SDK, runner, CLI, generated
  skill, examples, documentation surface, and public JSON. The external probe
  still imports only `polint::sdk::prelude::*`, whose allow-list remains 115
  names.
- The isolated Phase 64 measurement retains first-open/migration cost, separates
  store bytes from cache bytes, and passes the committed regression and
  diagnostics-digest gates.
- The fixed code follows the repository's Rust practices: narrow visibility,
  borrowed inputs, typed `Result` propagation, no production `unwrap`/`expect`,
  and no new lint suppression.

## Verification Performed

- `cargo test -p polint --lib analysis_kernel::store --locked -- --test-threads=1` — 23 passed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store --locked -- --test-threads=1` — 3 passed.
- `cargo test -p polint --test public_surface_leak --locked -- --test-threads=1` — 7 passed.
- `cargo test -p polint --lib eval::bench::gate::tests::phase_64 --locked -- --test-threads=1` — 1 passed.
- `cargo clippy -p polint --all-targets --all-features --locked -- -D warnings` — passed.
- `git diff --check origin/main...HEAD` — passed.

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
