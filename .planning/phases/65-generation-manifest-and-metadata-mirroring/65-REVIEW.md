---
phase: 65-generation-manifest-and-metadata-mirroring
scope: r1-only
depth: standard
status: clean
iteration: 3
diff_base: 9f3bdc56
files_reviewed: 6
resolved_findings:
  - CR-01
  - WR-01
  - WR-02
findings:
  critical: 0
  warning: 0
  info: 0
  total: 0
reviewed_at: 2026-07-28
---

# Phase 65 R1 Code Review

## Verdict

Clean. CR-01, WR-01, and WR-02 are closed, and the final adversarial pass found
no new actionable correctness, security, or quality issue in the reviewed R1
scope.

## Scope

Reviewed exactly the six R1 implementation and test files changed after
`9f3bdc56`, including repair commits `c01fa930`, `6de2949c`, `d1494824`, and
`1e795d9f`:

- `crates/polint/src/analysis_kernel/mod.rs`
- `crates/polint/src/analysis_kernel/store/connection.rs`
- `crates/polint/src/analysis_kernel/store/generation.rs`
- `crates/polint/src/analysis_kernel/store/migrations.rs`
- `crates/polint/src/analysis_kernel/store/mod.rs`
- `crates/polint/src/analysis_kernel/store/tests.rs`

The implementation remains within the six-file, private R1 boundary. Deferred
manifests, providers, fact/metadata families, dependency indexes, production
kernel wiring, and the sub-five-minute CI follow-up are outside this review.

## Closed Findings

### CR-01 — Persistent-trigger publication bypass

- Current-schema authentication inventories persistent triggers attached to
  `_polint_schema_migrations`, `generations`, and `active_generation`.
  `tbl_name = ? COLLATE NOCASE` follows SQLite identifier casing; quoting is
  removed from the catalog value, and a manual mixed-case quoted-target probe
  confirmed the trigger is found.
- Persistent triggers cannot target a main owned table from an attached
  database, and the store opens no attachments. Temporary triggers are
  connection-local and cannot be injected into the lifecycle writer by a
  second connection.
- Every lifecycle mutation starts `BEGIN IMMEDIATE`, authenticates the schema,
  performs the operation, authenticates it again, then commits. SQLite's
  single-writer lease prevents persistent trigger or schema DDL from
  interposing between validation and use.
- Reservation verifies that the returned row remains pending and has no active
  relationship before commit.
- The pre-opened-writer hostile-trigger test returns typed invalid schema and
  proves generation rows and active truth are unchanged.

### WR-01 — Direct active read initialization

- `active_generation` retains the disabled-before-path/I/O guard.
- Enabled reads initialize or migrate through the writer policy before opening
  the read-only view.
- Direct absent-store and exact-v1 calls produce a current schema, preserve v1
  sentinel data, and return the valid initial `None`.
- Held-writer contention maps to `BusySkipped` under the 250 ms bound.
- Future and malformed state remain typed refusals without lifecycle mutation.

### WR-02 — Legacy owned-name collision classification

- Version 0 and version 1 preflight now query the persistent schema catalog
  with `name = ? COLLATE NOCASE` across object types before running migration
  SQL.
- Quoted or case-varied tables, views, indexes, and triggers using an owned
  name are therefore rejected as invalid schema rather than falling through to
  a migration collision and `OpenFailed`.
- Migration tests prove a quoted uppercase v0 table and an uppercase v1 view
  return version-specific `InvalidSchema` with an unchanged catalog, version,
  and sentinel value.
- Facade tests prove the same fixtures return
  `RebuildNeeded(InvalidSchema)` and remain byte-identical after refusal.
- Exact v0 and v1 fixtures still migrate successfully, current reopen remains
  idempotent, and future schemas remain unchanged.

## Regression Review

- Reservation stays unreadable until same-handle publication.
- Completion and active rotation remain atomic, and every injected failure
  preserves the prior active selection across reopen.
- Active reads follow only the explicit singleton complete relationship; no
  recency or maximum-ID inference was introduced.
- Public diagnostics, JSON bytes, exit behavior, CLI/config/SDK surfaces, and
  production kernel wiring are unchanged.
- The reviewed diff is 1,781 added and 90 removed lines, below the locked 2,500
  handwritten-added-line budget.
- No delivery-history comments, unsafe production shortcuts, visibility
  widening, dependency changes, or CI workflow changes were introduced.

## Verification

- `cargo test -p polint --lib analysis_kernel::store::migrations::tests --locked -- --nocapture`
  — 19 passed.
- `cargo test -p polint --lib analysis_kernel::store::tests --locked -- --nocapture`
  — 29 passed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity --locked -- --nocapture`
  — 1 passed; public JSON bytes and exit semantics remain identical.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p polint --lib --tests --all-features --locked -- -D warnings`
  — passed.
- `git diff --check 9f3bdc56 -- <six reviewed files>` — passed.

No actionable findings remain.
