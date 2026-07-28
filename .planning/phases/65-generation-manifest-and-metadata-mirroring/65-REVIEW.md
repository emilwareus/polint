---
phase: 65-generation-manifest-and-metadata-mirroring
scope: r1-only
depth: standard
status: issues_found
diff_base: 9f3bdc56
files_reviewed: 6
findings:
  critical: 1
  warning: 1
  info: 0
  total: 2
reviewed_at: 2026-07-28
---

# Phase 65 R1 Code Review

## Scope

Reviewed exactly the six implementation and test files committed after
`9f3bdc56`:

- `crates/polint/src/analysis_kernel/mod.rs`
- `crates/polint/src/analysis_kernel/store/connection.rs`
- `crates/polint/src/analysis_kernel/store/generation.rs`
- `crates/polint/src/analysis_kernel/store/migrations.rs`
- `crates/polint/src/analysis_kernel/store/mod.rs`
- `crates/polint/src/analysis_kernel/store/tests.rs`

This review applies the locked R1 boundary. Deferred manifests, providers,
fact/metadata families, dependency indexes, production kernel wiring, and the
sub-five-minute CI follow-up are not treated as implementation defects.

## Findings

### CR-01 — Persistent triggers bypass schema authentication and can make reservation publish truth

**Severity:** Critical

**Evidence:**

- `crates/polint/src/analysis_kernel/store/migrations.rs:176` authenticates the
  current schema through table SQL, indexes, the declared foreign key, current
  rows, and `foreign_key_check`, but it never inventories persistent triggers
  attached to the three owned tables.
- `crates/polint/src/analysis_kernel/store/migrations.rs:229` queries only
  `sqlite_master` rows with `type = 'table'`; the index checks at lines 267-339
  likewise cannot detect a trigger.
- `crates/polint/src/analysis_kernel/store/generation.rs:80` inserts a pending
  row and returns its handle without authenticating that the row is still
  pending and no active relationship was created.
- `crates/polint/src/analysis_kernel/store/connection.rs:141` commits schema
  authentication before the separate lifecycle transaction begins, so even a
  new trigger check would still have an authentication/use gap under a
  concurrent writer unless the operation binds them to one protected snapshot.

An exact claimed-v2 database can add this persistent behavior without changing
any table SQL, index, foreign-key declaration, or initially validated row:

1. An `AFTER INSERT ON generations` trigger changes `NEW.generation_id` from
   `pending` to `complete`.
2. The same trigger inserts singleton `1` into `active_generation` with
   `required_status = 'complete'`.
3. `reserve()` inserts what it believes is pending and commits.
4. The reserved generation is now complete and active even though publication
   was never called.

This was reproduced against SQLite 3.51 with foreign keys enabled: the reserve
insert returned handle `1`, after which `generations` contained
`1|complete`, `active_generation` contained `1|1|complete`, and
`foreign_key_check` remained clean.

**Impact:**

The store accepts a claimed-current malformed/tampered database and violates
the core R1 rule that reservation is durable but unreadable until explicit
publication. This directly defeats D-05 through D-07 and the HIGH-blocking
threats T-65-01-01 and T-65-01-02.

**Actionable fix:**

Reject persistent triggers on `_polint_schema_migrations`, `generations`, and
`active_generation` as part of exact schema-v2 authentication. Bind that
authentication to each lifecycle mutation's immediate transaction (or verify
an authenticated schema cookie/inventory inside that transaction) so another
writer cannot install schema behavior between validation and use. Also
authenticate the post-insert reservation state before commit. Add a
claimed-current trigger fixture proving that reserve is refused without
changing lifecycle rows or active truth.

### WR-01 — `active_generation` does not initialize an absent or supported-prior store

**Severity:** Warning

**Evidence:**

- `crates/polint/src/analysis_kernel/store/mod.rs:153` prepares the path and
  opens only a read-only connection for `active_generation`; the method has no
  encoded or documented prerequisite that `maintain` was called first.
- `crates/polint/src/analysis_kernel/store/connection.rs:86` opens the database
  with `SQLITE_OPEN_READ_ONLY`.
- `crates/polint/src/analysis_kernel/store/migrations.rs:129` treats schema
  versions 0 and 1 as supported during preflight, so a valid v1 database passes
  `open_read_only` without receiving migration 2.
- `crates/polint/src/analysis_kernel/store/generation.rs:151` then queries
  `active_generation`, which does not exist in v0/v1 and maps to
  `StoreStatus::Skipped(OpenFailed)`. An absent database fails even earlier at
  the read-only open.
- The fresh-store test at
  `crates/polint/src/analysis_kernel/store/tests.rs:50` calls
  `SemanticStore::maintain` before the read, so it does not exercise the facade
  entry point on a genuinely absent or exact-v1 store.

**Impact:**

The three lifecycle facade methods have inconsistent initialization semantics:
reservation initializes/migrates through `open_writer`, while the read method
fails unless another operation happened first. A valid Phase 64 database can
therefore produce `OpenFailed` instead of the valid initial `None`, weakening
the claimed v1 compatibility and the “fresh store reads `None`” contract.

**Actionable fix:**

Either initialize/migrate absent and supported-prior stores before opening the
read-only lifecycle view, or encode an initialized/current-store session in the
type API so the prerequisite cannot be skipped. Preserve the disabled-before-I/O
guard and bounded busy mapping. Add facade-level tests that call
`active_generation` directly on (a) an absent owned path and (b) an exact
schema-v1 fixture and assert the intended current-schema `None` result.

## Verified Strengths

- Publication changes the pending row and singleton selection in one immediate
  transaction; operation errors and all five injected seams drop the
  transaction before commit.
- Under the authenticated declared schema, the composite
  `(generation_id, required_status)` foreign key prevents a pending row from
  satisfying active selection.
- Active reads follow the explicit singleton relationship and do not infer
  truth from `MAX(id)`, timestamps, insertion order, or recency.
- The 250 ms busy policy, disabled-before-I/O guard, private visibility, and
  raw-SQL/rusqlite boundary are preserved.
- New source comments explain enduring boundaries and contain no delivery
  phase/plan history.
- No public CLI/config/SDK/output or CI workflow file is in the reviewed diff.

## Verification Run

- `cargo test -p polint --lib analysis_kernel::store::migrations::tests --locked`
  — 16 passed.
- `cargo test -p polint --lib analysis_kernel::store::tests::generation_lifecycle --locked`
  — 9 passed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity --locked`
  — 1 passed.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p polint --lib --tests --all-features --locked -- -D warnings`
  — passed.
- `git diff --check 9f3bdc56 -- <reviewed files>` — passed.

The passing suite does not cover either finding above.
