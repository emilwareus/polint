---
status: fixed
findings_in_scope:
  - CR-01
  - WR-01
  - WR-02
fixed:
  - CR-01
  - WR-01
  - WR-02
skipped: []
iteration: 2
---

# Phase 65 R1 Code Review Fix

All three findings across the two `65-REVIEW.md` iterations are fixed. The
iteration-2 review confirms CR-01 and WR-01 remain resolved, and WR-02 is fixed
by the latest repair. The changes remain inside the
private R1 lifecycle/store boundary: five existing files under
`crates/polint/src/analysis_kernel/store/` changed, with no public API, CLI,
configuration, provider, metadata-family, kernel-wiring, or CI workflow
changes.

## CR-01 — Fixed

**Commit:** `c01fa930` — `fix(65): authenticate lifecycle mutations`

- **Identifier hardening:** `d1494824` —
  `fix(65): authenticate trigger table names`

- Exact schema authentication now rejects every persistent trigger attached to
  `_polint_schema_migrations`, `generations`, or `active_generation`, including
  trigger definitions that reference an owned table with different identifier
  casing.
- Every lifecycle mutation authenticates the current schema after acquiring its
  immediate transaction and authenticates it again after the operation, before
  commit. The immediate writer lease prevents a concurrent schema writer from
  changing persistent behavior between authentication and use.
- Reservation verifies that the returned handle still names a pending row and
  has no active relationship before commit.
- The regression fixture opens the lifecycle writer first, installs a hostile
  claimed-v2 `AFTER INSERT` trigger through another connection, and then calls
  reservation. Reservation returns the typed invalid-schema outcome, with the
  generation rows and active relationship unchanged.
- Migration coverage separately proves claimed-current schemas are rejected
  when any of the three owned tables has a persistent trigger.

## WR-01 — Fixed

**Commit:** `6de2949c` — `fix(65): initialize generation reads`

- `SemanticStore::active_generation` keeps its disabled-before-path/I/O guard,
  then initializes or migrates through the existing writer policy before
  opening the lifecycle read-only view.
- Calling the facade directly on an absent owned path creates the exact current
  schema and returns `None`.
- Calling it directly on the exact schema-v1 fixture transactionally migrates
  to v2, preserves the sentinel data, and returns `None`.
- A held immediate writer lease maps the read-side initialization attempt to
  `GenerationError::Store(StoreStatus::BusySkipped)` within the existing
  bounded busy policy.
- Existing malformed/future typed refusal tests and the disabled zero-I/O test
  continue to pass.

## WR-02 — Fixed

**Commit:** `1e795d9f` — `fix(65): authenticate legacy owned names`

- Pre-migration v0/v1 validation now treats each owned identifier as a
  case-insensitive SQLite schema namespace. It queries all persistent
  `sqlite_master` object types rather than only exact-case tables.
- Version 0 refuses any object named `_polint_schema_migrations`,
  `generations`, or `active_generation`; version 1 first authenticates its exact
  bootstrap table and then applies the same collision check to the two
  lifecycle names.
- A quoted uppercase `"GENERATIONS"` table fixture returns
  `MigrationError::InvalidSchema { version: 0 }`. Its full schema catalog,
  version, and stored sentinel value remain unchanged.
- An exact-v1 fixture with an uppercase `"ACTIVE_GENERATION"` view returns
  `MigrationError::InvalidSchema { version: 1 }`. Its full schema catalog,
  version, and sentinel value remain unchanged.
- Facade-level versions of both fixtures return
  `StoreStatus::RebuildNeeded(StoreRebuildReason::InvalidSchema)` and remain
  byte-identical after refusal.
- Existing legitimate v0/v1 migration, current-schema reopen, and future-schema
  refusal coverage continues to pass.

## Verification

- `cargo test -p polint --lib analysis_kernel::store::migrations::tests --locked`
  — 19 passed, 0 failed, 0 ignored.
- `cargo test -p polint --lib analysis_kernel::store::tests --locked`
  — 29 passed, 0 failed, 0 ignored.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity --locked`
  — 1 passed, 0 failed, 0 ignored; JSON bytes and exit semantics remain
  identical across store modes.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p polint --lib --tests --all-features --locked -- -D warnings`
  — passed.
- All four repair commits passed the normal Conductor pre-commit hook,
  including
  `make lint` and workspace-wide all-target/all-feature strict clippy.
- `git diff --check` — passed.

## Scope and Completion

No finding was skipped. This fixes the reviewed R1 implementation only. Phase
65, STORE-04, STORE-05, META-01, and META-04 remain open, and the deferred
sub-five-minute CI follow-up is unchanged.
