# Phase 65 R2 Pattern Map

**Mapped:** 2026-07-28

**Purpose:** Concrete current-tree analogs for the private minimal audited run
manifest only.

## Locked Boundary

This map covers restart slice R2. R1's schema-v2 generation lifecycle is a
completed dependency and must be extended, not replaced.

R2 adds one private canonical payload containing only:

- a closed manifest schema version;
- a purpose-typed workspace/cache-owner digest;
- the complete config digest with a closed purpose;
- canonical source rows with normalized repo-relative path, closed language,
  source-content digest, and byte size;
- an authenticated source count; and
- a creation-independent run identity recomputed from those fields.

Rules, plans, providers, capabilities, lifecycle/tool identity, models,
extensions, layers, queries, summaries, facts, validation events, dependency
indexes, statistics, telemetry, timestamps, mtimes, and opaque JSON are absent.
R2 remains unwired from normal kernel persistence/reuse. It adds no public API,
configuration, CLI, output, diagnostic, CI, or SDK contract.

R2 completion does not complete Phase 65 or STORE-04, STORE-05, META-01, or
META-04. Plan and summary requirement lists remain empty; contributions are
tracked separately.

## Ownership Decision

The canonical projection belongs at the analysis-kernel boundary, alongside
the existing incremental identity vocabulary:

```text
analysis_kernel::incremental::run_manifest
        typed manifest construction / codec / equality / identity
                         |
                         v
analysis_kernel::store::generation
        SQL encode/decode / bounded preflight / atomic publication
```

`analysis_kernel::store` remains the sole owner of SQL, table names,
transactions, SQLite storage classes, and relational handles. It must not
define a competing store-only semantic identity. `InputSnapshot` is an input
witness for source fields, not the durable payload and not its codec.

## Likely File Set

The preferred implementation stays within eight product/test files. A separate
manifest-focused store test file may replace additions to `store/tests.rs`
without widening the production surface.

| File | Change and role | Current symbols to reuse | Constraint |
|---|---|---|---|
| `crates/polint/src/analysis_kernel/incremental/run_manifest.rs` | **Create:** private `RunManifest`, canonical source row, purpose-typed identities, closed codecs, canonical builder, exact comparison, and identity recomputation. | `Digest`/`DigestBuilder` length-prefixed construction; `FileSnapshot`; `Language`; `normalize_repo_relative`. | No serde/JSON/`Debug` identity input. Reject unknown language, noncanonical/duplicate path, wrong digest purpose, count mismatch, and noncanonical row order. |
| `crates/polint/src/analysis_kernel/incremental/mod.rs` | **Modify:** declare and narrowly re-export only the private manifest vocabulary required by the store and tests. | Existing curated crate-private re-exports. | Do not widen SDK, runner, CLI, or crate-root public surfaces. |
| `crates/polint/src/analysis_kernel/incremental/digest.rs` | **Modify if needed:** add only closed workspace/run purpose variants and byte-safe canonical builder support. | `DigestKind`, `DigestBuilder`, length-prefixed parts. | Do not use `debug_part`; keep all identity labels explicit and versioned. Do not add provider/layer purposes. |
| `crates/polint/src/analysis_kernel/store/migrations.rs` | **Modify:** schema version 3, exact empty-v2 migration, strict manifest-family catalog/content authentication, and populated-v2 refusal without mutation. | `MIGRATIONS`, `validate_supported_schema`, `validate_current_schema`, exact SQL comparison, FK/index/trigger/content validators. | One manifest schema family only. Every complete generation has exactly one valid header; pending generations have none; source rows belong to one header. |
| `crates/polint/src/analysis_kernel/store/generation.rs` | **Modify:** publish a supplied manifest with the same reserved handle and read the active handle plus decoded manifest in one transaction/snapshot. | `publish_transaction`, `active`, `require_complete`, `replace_selection`, `validate_selection`, finite test failure seams. | Manifest write, decode/recompute, completion, selection, and commit are one atomic publication. Handle-only active reads cannot bypass manifest authentication. |
| `crates/polint/src/analysis_kernel/store/connection.rs` | **Modify:** narrow read-transaction callback and typed manifest fixture support if needed. | `with_immediate_transaction`, `with_read_connection`, writer/read-only policy, error classifier, fixture snapshot. | Reuse WAL, foreign keys, busy timeout, preflight, and recovery mapping. No second connection policy or raw SQLite type above `store`. |
| `crates/polint/src/analysis_kernel/store/mod.rs` | **Modify:** private manifest-aware facade/result and disabled guard. | `SemanticStore`, `StoreConfig`, `prepare_generation_store`, `GenerationError` mapping. | Disabled mode returns before path validation, workspace canonicalization, manifest construction, or I/O. Keep normal `AnalysisKernel::run` unchanged. |
| `crates/polint/src/analysis_kernel/store/tests.rs` | **Modify:** round-trip, match/mismatch, migration, tamper, bounds, rollback, reopen, and disabled proof. | Independent temp stores, typed fixture snapshots, R1 lifecycle/failure matrix. | No global serialization; no required individual test over 60 seconds. |

Only if the canonical root encoder cannot live cleanly in the new manifest
module may a small private helper be added to `repo_fs.rs`. The plan should not
touch `Cargo.toml`, providers, rule/plan keys, `InputSnapshot` fields, public
docs, examples, or `.github/workflows/ci.yml`.

## Current Interfaces and Reuse Points

### Canonical inputs

- `InputSnapshot::files` is already sorted by `relative_path`; each
  `FileSnapshot` exposes the needed path, `Language`, source-text `Digest`, and
  byte size. The manifest constructor must copy only those approved fields and
  revalidate them rather than trust construction order.
- `InputSnapshot::config.digest` is not the approved durable value because the
  snapshot component includes broader status/detail vocabulary. R2 receives
  the already-computed complete `config_hash` string directly and wraps it in
  the fixed config purpose.
- `LoadedConfig::root` supplies the workspace root. Canonicalize and normalize
  it before hashing, encode platform path units losslessly, and persist only
  the resulting typed digest. Never use `display`, `to_string_lossy`, or
  `Debug` as identity input.
- `SourceFile::{relative_path, language, content_hash, source}` is the original
  source of the approved file fields. `FileSnapshot` is the narrowest existing
  projection and avoids reparsing or cloning source text.

### Digest vocabulary

`Digest` already separates kinds and uses explicit length-prefixed parts.
Extend that pattern with closed workspace/run purposes or an equivalent private
wrapper. The durable codec must expose explicit labels for every field:

```text
manifest-schema
workspace-purpose + workspace-value
config-purpose + config-value
source-count
for each source in normalized-path order:
  path + language + source-purpose + source-value + size
```

The stored run digest is supporting evidence only. Decode all fields,
reconstruct the same typed projection, recompute the identity, and compare
exact canonical fields before returning a match.

### Language codec

`Language` currently includes `Go`, `TypeScript`, `Tsx`, `JavaScript`, `Jsx`,
and `Unknown`. The durable codec should use explicit closed labels for supported
source languages. `Unknown` is not a trusted stored language and must be
rejected during construction and decode. Do not rely on serde rename behavior
as the SQLite contract.

### R1 publication primitive

`generation::publish_transaction` currently transitions the supplied pending
handle, validates completion, selects the same handle, validates selection, and
commits through `connection::with_immediate_transaction`. R2 should extend that
one transaction:

1. validate the in-memory canonical manifest;
2. within the same writer transaction, decode/authenticate the current active
   manifest when present and refuse before mutation if its workspace identity
   differs from the candidate;
3. write the header and owned source rows for the reserved handle;
4. authenticate storage and decode/recompute the just-written projection;
5. transition that exact handle from pending to complete;
6. validate complete-generation/manifest cardinality;
7. rotate and authenticate the singleton active relationship; and
8. commit.

Every error rolls the candidate back to its prior durable pending state and
preserves the previous active manifested generation.

`generation::active` currently opens one read-only connection but does not
explicitly begin a read transaction and returns only a handle. R2 needs one
read snapshot that selects the active complete handle and reads all header and
source rows before typed reconstruction. The handle-only method must delegate
to or be replaced by this authenticated path.

## Manifest Relational Shape

Names are discretionary, but one family should be equivalent to:

```sql
CREATE TABLE run_manifests (
    generation_id INTEGER PRIMARY KEY
        REFERENCES generations (generation_id),
    manifest_schema TEXT NOT NULL,
    workspace_purpose TEXT NOT NULL,
    workspace_digest TEXT NOT NULL,
    config_purpose TEXT NOT NULL,
    config_digest TEXT NOT NULL,
    source_count INTEGER NOT NULL CHECK (source_count >= 0),
    run_purpose TEXT NOT NULL,
    run_digest TEXT NOT NULL
);

CREATE TABLE run_manifest_sources (
    generation_id INTEGER NOT NULL,
    relative_path TEXT NOT NULL,
    language TEXT NOT NULL,
    source_purpose TEXT NOT NULL,
    source_digest TEXT NOT NULL,
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    PRIMARY KEY (generation_id, relative_path),
    FOREIGN KEY (generation_id)
        REFERENCES run_manifests (generation_id)
);
```

The header references the generation by relational handle, while committed
content validation enforces the status/cardinality invariant. This permits the
transaction to stage and authenticate the manifest while the owner is still
pending, then transition it to complete before selection and commit, in the
order required by D-16. The uncommitted staged header is invisible to readers.
Do not weaken this to an unowned scalar, and do not add a `complete`-only
immediate foreign key that makes the required write-before-complete sequence
impossible. The exact constraints may otherwise be stricter.

Do not persist ordinals. Source order is derived with `ORDER BY relative_path`
and authenticated again by canonical reconstruction. Header/source columns,
closed labels, keys, indexes, foreign keys, triggers, and current contents are
part of current-schema validation.

Schema-v3 content invariants include:

- pending generation: zero headers and zero owned source rows;
- complete generation: exactly one header;
- manifest header: belongs to the same complete generation;
- declared source count: exactly equals owned row count;
- source membership: unique canonical path and closed language/digest purpose;
- stored run identity: equals typed recomputation;
- active generation: joins one complete generation with one authenticated
  manifest; and
- no orphan/cross-owned header or source row.

## Migration Pattern

Version 2 is supported only when both `generations` and `active_generation` are
empty. Check this first through the existing read-only `preflight_schema`,
which runs before `open_uninitialized_writer` applies `synchronous` or
`journal_mode = WAL`; a populated v2 refusal must not persist a connection
policy change. Repeat the prerequisite inside the same immediate migration
transaction before executing any v3 DDL or marker/version update.

- Empty exact v2: add the two manifest tables and advance the single marker and
  `user_version` to 3.
- Populated exact v2: return the existing typed invalid-schema/rebuild-needed
  path with journal mode, catalog, marker, generation rows, and active
  relationship unchanged.
- v0/v1: ordered migrations may pass through an empty v2 to v3.
- v3: strict idempotent validation only.
- malformed/future: refuse before mutation.

Do not synthesize a manifest, delete or deactivate legacy generations, or
support a mixed manifestless current schema.

## Bounded Decode Pattern

SQLite values are untrusted even in a private cache. Before retrieving owned
text into large Rust allocations:

1. authenticate the current schema and relationships;
2. query `typeof(...)` for every scalar/column family;
3. preflight header cardinality and declared child count;
4. enforce explicit per-field byte limits;
5. enforce source-row and aggregate payload-byte limits including row overhead;
6. only then read rows in the same transaction;
7. decode closed purposes/languages and checked numeric conversions;
8. reconstruct the canonical projection; and
9. recompute and compare the run identity and exact fields.

Limits should be constants large enough for real repositories and small enough
to bound attacker-controlled work. Do not silently truncate, coerce SQLite
storage classes, allocate from an unchecked count, or accept partial payloads.

## Private Read Result

A narrow private result may be shaped like:

```rust
enum ManifestMatch {
    NoActiveManifest,
    Exact(GenerationHandle),
    Mismatch,
}
```

Malformed storage is an error/refusal rather than another mismatch. Exact
means the decoded stored manifest and requested manifest compare field for
field after both identities are recomputed. R2 returns no persisted provider
facts and changes no analysis result.

## Failure and Tamper Proof

Extend the finite cfg(test)-only publication seams to cover:

- before/after header write;
- during or after source writes;
- before/after stored decode and identity validation;
- before/after pending-to-complete transition;
- before/after singleton selection; and
- before commit.

For each seam: publish A with manifest M1; reserve B; fail publication of B
with M2; close all connections; reopen; authenticate A/M1 as the sole readable
active truth; prove B remains pending with no manifest rows.

Tamper tests should mutate one field while leaving the stored run digest
unchanged and require typed refusal for:

- manifest schema, every purpose label, every digest value, declared count,
  and run identity;
- source insertion and deletion;
- source path, language, purpose, digest, and byte-size update;
- storage-class substitution;
- oversized scalar/row/count/aggregate cases; and
- missing, extra, orphaned, or cross-owned relationships.

Normal mismatch tests separately prove exact workspace, config, source
insert/delete/update/path/language/digest/size changes are non-reusable rather
than corruption. Insertion order, worker order, timestamps, mtimes, durations,
and cache telemetry must not affect identity because the constructor and schema
cannot represent them.

## Three-Task Planning Fit

1. **Projection:** typed canonical manifest/codec/identity plus focused
   constructor and mutation tests.
2. **Schema and I/O:** exact v3 migration, bounded relational encode/decode,
   one-snapshot authenticated reads, and populated-v2 refusal.
3. **Atomic proof:** manifest-aware publication/match facade, rollback/reopen
   matrix, tamper/bounds tests, disabled parity, and existing test-suite checks.

This remains within three tasks, fifteen product/test files, 2,500 handwritten
added lines, one schema family, and zero provider families. If the work needs
provider trust, capability state, tool/environment certification, a general
dependency index, `InputSnapshot` redesign, production enablement, or CI
redesign, stop and split it into its owning later slice.
