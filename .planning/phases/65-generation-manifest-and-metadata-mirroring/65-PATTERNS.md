# Phase 65 R1 Pattern Map

**Mapped:** 2026-07-28

**Purpose:** Concrete current-main analogs for the private minimal generation
lifecycle only.

## Locked Boundary

This map covers restart slice R1, not the original all-at-once Phase 65 design.
The durable shape is limited to a store-local generation handle, the closed
`pending`/`complete` lifecycle, and one optional active selection. It must not
introduce a run manifest, workspace or input identity, providers, capabilities,
facts, layers, query/summary keys, dependency indexes, validation events,
statistics, language/tool metadata, or any public API/config/CLI/output change.

R1 completion does not complete Phase 65 or close STORE-04, STORE-05, META-01,
or META-04.

## Likely File Set

The preferred implementation is six files, five under the existing private
store subtree and one test-only expectation adjustment. A separate generation
test file may replace the additions to `store/tests.rs` if that keeps the
failure matrix clearer; it should not cause the production surface to spread.

| File | Change and role | Data flow | Closest current analog | Pattern to preserve |
|---|---|---|---|---|
| `crates/polint/src/analysis_kernel/store/generation.rs` | **Create:** own the typed relational handle, lifecycle SQL, active read, publication validation, and test-only failure seams. | `WriterConnection` → committed pending handle; pending handle + writer → complete-and-selected transaction; `ReadOnlyConnection` → `Option<active complete handle>`. | `connection.rs::try_initialize_writer` for protected transactions; `StoreConfig` for opaque private values; `StoreFixtureSnapshot` for typed test inspection. | Small `Copy + Eq` opaque handle with a private scalar; typed `Result`; no serialization, semantic hashing, timestamps, or raw ID accessor. |
| `crates/polint/src/analysis_kernel/store/mod.rs` | **Modify:** declare the lifecycle module and expose only narrow crate-private facade operations/outcomes and test helpers. Keep `maintain` and its disabled short circuit intact. | `StoreConfig` is checked before path work; owned path opens the existing connection policy; lower-level lifecycle errors map to private store outcomes. | `SemanticStore::maintain`, `map_connection_error`, and the existing `#[cfg(test)]` forwarding helpers. | No `rusqlite` type, SQL text, table name, or scalar row ID crosses `analysis_kernel::store`; use `pub(crate)` only where an intentional internal caller needs it and tighter visibility otherwise. |
| `crates/polint/src/analysis_kernel/store/connection.rs` | **Modify:** provide the narrow transaction/read access needed by the sibling lifecycle module; make future-schema fixtures derive from the current version. | Writer → `BEGIN IMMEDIATE` transaction → operation → commit; operation error or injected failure drops/rolls back the transaction. Reader remains separately opened read-only. | `try_initialize_writer`, `try_writer_lease`, `hold_initialization_lease`, `open_read_only`, and `classify_sqlite_error`. | Reuse WAL, foreign keys, 250 ms busy timeout, bounded busy mapping, and migration preflight. Do not add a second connection policy or leak `Connection`/`Transaction` above the store module. |
| `crates/polint/src/analysis_kernel/store/migrations.rs` | **Modify:** add schema version 2 as the sole R1 durable schema family and extend current-schema validation. | v0 applies v1 then v2; the exact Phase 64 v1 fixture applies only v2; v2 reopens without mutation; future/malformed v2 refuses before lifecycle access. | `MIGRATIONS`, `apply_migrations_in_transaction`, `preflight_schema`, and `validate_current_schema`. | One ordered transactional migration, `PRAGMA user_version` set only inside the migration transaction, one current bootstrap marker, exact shape/relationship validation, and no opportunistic repair. |
| `crates/polint/src/analysis_kernel/store/tests.rs` | **Modify:** add facade-level lifecycle, reopen, exact-selection, constraint, and failure-preservation tests with typed fixture snapshots. | Fresh temp store → reserve/publish/read/drop/reopen; failure matrix → reopen → authenticate old active relationship and candidate unreadability. | Existing `connection_policy`, `writer_contention`, and `recovery` modules. | Independent temporary databases, no process-global serialization, persisted relationship assertions rather than row counts alone, and every test below 60 seconds. |
| `crates/polint/src/analysis_kernel/mod.rs` | **Modify test code only:** make the existing future-schema parity expectation use a current-version-relative fixture value. Production `AnalysisKernel::run` remains unchanged. | Existing post-validation `SemanticStore::maintain` call and public-answer parity flow are untouched. | `semantic_store_check_parity::run_mode` and `CURRENT_SCHEMA_VERSION_FOR_TEST`. | Preserve disabled/ready/future/invalid/busy JSON and exit parity. Do not reserve or publish an empty generation from the kernel merely to make R1 code live. |

No R1 change is expected in `Cargo.toml`, `cache`, `incremental`,
`public_surface_leak.rs`, SDK/runner/CLI code, documentation, examples, or
`.github/workflows/ci.yml`.

## Concrete Lifecycle Shape

Names remain discretionary, but the planner should preserve a shape equivalent
to:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct GenerationHandle(NonZeroI64);

fn reserve(
    writer: &mut WriterConnection,
) -> Result<GenerationHandle, GenerationError>;

fn publish(
    writer: &mut WriterConnection,
    generation: GenerationHandle,
) -> Result<(), GenerationError>;

fn active(
    reader: &ReadOnlyConnection,
) -> Result<Option<GenerationHandle>, GenerationError>;
```

The scalar constructor and accessor stay inside the store module. The wrapper
is only a store-local relational handle: do not implement a semantic digest,
cache key, source/run/provider identity, `Serialize`, or public `Display`.
Passing the small handle by value is appropriate; connections and paths remain
borrowed.

The connection layer may use a store-internal callback or transaction guard
equivalent to:

```rust
fn with_immediate_transaction<T, E>(
    writer: &mut WriterConnection,
    operation: impl FnOnce(&Transaction<'_>) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<ConnectionError>;
```

`Transaction<'_>` may be visible to sibling code inside
`analysis_kernel::store`, but never to its callers. Production fallibility uses
typed `Result` and `?`; `expect`/`unwrap` remain test-only. If the not-yet-wired
lifecycle needs a lint expectation, follow the existing narrow
`cfg_attr(not(test), expect(dead_code, reason = "..."))` precedent and describe
the enduring private-boundary reason without phase/plan chronology.

## Transaction and Read Invariants

1. **Reservation is durable and separate.** In one immediate transaction,
   insert exactly one `pending` row, commit, and return its opaque handle.
2. **Publication uses that same handle.** In one later immediate transaction,
   update exactly one row from `pending` to `complete`; zero changed rows is a
   typed rejection, not success.
3. **Selection is part of publication.** Validate the completed candidate,
   replace the singleton active relationship with that exact handle, validate
   the relationship, then commit. Do not expose a general “activate arbitrary
   handle” production operation.
4. **Rollback preserves truth.** Any error after publication begins and before
   commit rolls the candidate back to `pending` and leaves the prior active
   pointer unchanged. A failed first publication leaves the valid initial
   `None` state.
5. **Reads follow the relationship.** `active()` reads only the singleton
   selection joined/authenticated against a `complete` generation. `None`
   means there is no pointer; a dangling, duplicate, pending, malformed, older,
   or future-schema pointer is a typed refusal, not `None`.
6. **No inferred selection.** No `MAX(id)`, insertion order, timestamp,
   recency, or “latest complete” query is permitted. A newer pending handle
   must not displace an older explicitly active handle, including after reopen.
7. **Closed lifecycle only.** R1 needs `pending` and `complete`; publication
   failure is represented by rollback, not a new durable failure/event family.

## Minimal Relational Constraint Pattern

The exact private names are discretionary. The schema needs the equivalent of:

```sql
CREATE TABLE generations (
    generation_id INTEGER PRIMARY KEY,
    status TEXT NOT NULL CHECK (status IN ('pending', 'complete')),
    UNIQUE (generation_id, status)
);

CREATE TABLE active_generation (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    generation_id INTEGER NOT NULL,
    required_status TEXT NOT NULL CHECK (required_status = 'complete'),
    FOREIGN KEY (generation_id, required_status)
        REFERENCES generations (generation_id, status)
);
```

The constant `required_status` column is relational enforcement, not a second
metadata family: together with the composite foreign key it prevents a pending
row from becoming active even if a future code path bypasses the normal
publication method. Equivalent trigger/constraint designs are acceptable only
if tests prove the same invariant. `AUTOINCREMENT`, timestamps, payload
columns, semantic IDs, and indexes for future metadata are unnecessary.

Version 2 should keep the Phase 64 single-marker invariant. Because version 1
stores one marker row, the version-2 migration should advance that row rather
than silently leave a v1 marker or accumulate an unrelated marker convention.
`validate_current_schema` must authenticate the lifecycle tables, columns,
checks/keys, foreign-key relationship, the singleton bound, and current
contents (including a foreign-key check), not merely their names.

The supported-prior fixture must be an exact Phase 64 v1 database. Current
future fixtures hard-code version 2 in `connection.rs`, `store/tests.rs`, and
the kernel parity expectation; after R1, derive the future value as
`CURRENT_SCHEMA_VERSION + 1` so version 2 is tested as the supported migration
source instead of accidentally remaining “future.”

## Failure-Injection Pattern

Use a deterministic, store-private, test-only hook or finite seam enum. Do not
add an environment variable, feature, global lock, public option, or
production-visible failure mode. Cover at least these boundaries:

- before the pending-to-complete update;
- immediately after the update;
- immediately before singleton selection;
- immediately after singleton selection;
- immediately before commit.

For every seam: publish A; reserve B; inject B’s failure; assert the typed
failure; authenticate that B is not active; drop every connection; reopen; and
assert A is still the sole selected complete generation. The fixture snapshot
should carry ordered `(typed handle, typed status)` rows plus the selected
handle so tests prove relationships and states, not just counts or
`user_version`.

Also prove:

- a fresh migrated store has no active generation;
- reservation alone does not create readable truth;
- successful publication returns/selects the reserved handle;
- a direct pending-selection attempt is rejected by the relational constraint;
- publishing B after A selects B exactly, while a newer unselected/pending C
  does not affect the read;
- close/reopen preserves the selection;
- the exact v1 → v2 migration succeeds and v2 reopen is idempotent;
- malformed current and future schemas remain typed private refusals;
- disabled lifecycle entry points return before path validation or I/O.

## Three-Task Planning Fit

1. **Schema:** version-2 migration, exact validation, v1/current/future fixtures.
2. **Lifecycle:** opaque handle, reservation, atomic publish/select, active
   read, and narrow connection transaction support.
3. **Proof:** lifecycle/reopen/failure matrix plus the test-only future-version
   parity adjustment.

This stays within the restart limits of three tasks, fifteen product/test
files, 2,500 handwritten added lines, one durable schema family, and zero
provider families. Discovery of any required identity, provider, runtime, or
validation contract is a stop-and-split condition.

## CI Execution Gate

Planning does not satisfy the implementation gate. The current required
workflow still runs full platform library tests; GitHub Actions run
`29752687999` had an approximately **9 minute 52 second** critical path in the
Windows library-test job. R1 requires a truthfully measured required
pull-request path at or below **5 minutes**, with no required individual test
above 60 seconds and no global serialization of ordinary correctness tests.

Therefore implementation must stop unless a separate prerequisite or human
split decision establishes that required path. A focused local store test
command is useful evidence but does not by itself make the current required
workflow sub-five-minute. CI redesign, workflow edits, branch-protection
administration, and timeout increases are explicitly outside this R1 file set.
