---
phase: 65-generation-manifest-and-metadata-mirroring
scope: r1-r2
depth: deep
status: clean
iteration: 2
diff_base: f3f4612f
files_reviewed: 8
files_reviewed_list:
  - crates/polint/src/analysis_kernel/incremental/run_manifest.rs
  - crates/polint/src/analysis_kernel/incremental/digest.rs
  - crates/polint/src/analysis_kernel/incremental/mod.rs
  - crates/polint/src/analysis_kernel/store/migrations.rs
  - crates/polint/src/analysis_kernel/store/connection.rs
  - crates/polint/src/analysis_kernel/store/generation.rs
  - crates/polint/src/analysis_kernel/store/mod.rs
  - crates/polint/src/analysis_kernel/store/tests.rs
findings:
  critical: 0
  warning: 0
  info: 0
  total: 0
reviewed_at: 2026-07-29
---

# Phase 65 R1-R2 Code Review

## Verdict

Clean. The three iteration-1 findings are closed by commits `48c5cce2`,
`00979854`, and `27506a52`. A deep re-review of the full current contents of
the eight scoped files found no new critical or warning issues and no R1
regression.

## Scope

Reviewed the full `f3f4612f..27506a52` implementation history and current
source, including the iteration-1 fixes rather than relying on
`65-REVIEW-FIX.md`. The review retraced canonical manifest construction,
schema authentication, migration/open policy, generation publication,
one-snapshot active reads and matching, rollback behavior, workspace ownership,
typed error mapping, and tamper tests.

The three fix commits change only
`incremental/run_manifest.rs`, `store/migrations.rs`, and `store/tests.rs`.
The combined R1-R2 implementation remains within the same eight-file private
boundary and does not add provider families, public CLI/config/SDK/output
surface, normal-kernel publication/reuse wiring, or CI workflow changes.

## Iteration-1 Finding Closure

### CR-01 — Closed: complete generations now require the singleton active row

**Evidence:** `crates/polint/src/analysis_kernel/store/migrations.rs:642-695`,
`crates/polint/src/analysis_kernel/store/connection.rs:43-60`,
`crates/polint/src/analysis_kernel/store/connection.rs:107-130`, and
`crates/polint/src/analysis_kernel/store/tests.rs:315-392`

`validate_manifest_rows` now compares `EXISTS(complete generation)` with
`EXISTS(active_generation)` and rejects any mismatch. The exact active-table
shape and lifecycle validation already bound the present case to one singleton
row pointing at a complete generation; manifest ownership validation binds
that selection to an existing manifest. Empty and pending-only stores remain
valid because both existence predicates are false.

The invariant is checked by writer preflight and again inside every immediate
mutation transaction. Active reads and manifest matching initialize/open the
store and then validate the current schema inside their read transaction before
reading selected truth. Deleting the active row can therefore no longer become
normal absence or bypass the workspace-ownership check.

The regression publishes workspace A, reserves a candidate, deletes the active
row, and proves active read, exact match, maintenance, same-workspace
publication, and second-workspace publication all return the typed
`RebuildNeeded(InvalidSchema)` outcome without changing the complete or pending
rows.

### CR-02 — Closed: manifest index authentication is bounded and streamed

**Evidence:** `crates/polint/src/analysis_kernel/store/migrations.rs:537-596`
and `crates/polint/src/analysis_kernel/store/tests.rs:394-436`

The validator first obtains index cardinality with scalar `count(*)` and
rejects unless it is exactly one. It then decodes only that sole row's bounded
metadata. Index columns are read as a stream: exactly the first two names are
decoded and compared with `generation_id` and `relative_path`, while a third
row is checked only for existence. No attacker-controlled index or column
catalog is collected into a Rust vector.

The real writer-open regression installs 128 extra indexes, receives the typed
invalid-schema/rebuild result, and proves the catalog is preserved rather than
mutated.

### WR-01 — Closed: portable absolute/prefixed paths are rejected

**Evidence:**
`crates/polint/src/analysis_kernel/incremental/run_manifest.rs:320-339` and
`crates/polint/src/analysis_kernel/incremental/run_manifest.rs:418-545`

Canonical source validation now rejects native `Prefix`/`RootDir` components,
portable ASCII drive prefixes such as `C:` (including drive-relative forms),
and leading POSIX or Windows separators before applying slash/dot
normalization. Build and stored-decode tests cover POSIX absolute,
drive-absolute, drive-relative, UNC, and verbatim-prefix spellings.

The checks are limited to actual root/prefix forms. Ordinary canonical relative
names such as `src/app.ts`, `src/a.ts`, and `src/b.go` still construct,
round-trip, sort, encode, and decode successfully, so the fix does not reject
the valid repo-relative path contract.

## Regression Review

- Publication still writes the header and sources, reads and recomputes the
  stored identity, completes the supplied reserved handle, and rotates the
  singleton selection in one immediate transaction.
- Every publication failure seam still rolls back the candidate manifest,
  leaves it pending, and preserves the prior active manifested generation
  across reopen.
- Active handle, header preflight, source preflight, row decode, and exact
  comparison still share one validated read transaction/snapshot.
- Header/source storage classes, counts, scalar lengths, aggregate payload
  bytes, checked numeric conversions, closed labels, canonical ordering,
  ownership, and run identity remain authenticated before trust.
- Exact empty-v2 migration, populated-v2 refusal before persistent policy,
  transactional revalidation, current-v3 idempotence, future-schema refusal,
  and malformed-schema preservation remain intact.
- Disabled entry points still return before path creation, workspace
  canonicalization, manifest construction, or SQLite access.
- Production visibility remains crate-private, and the fixes introduce no
  delivery-history comments or supported-surface widening.

## Verification

- `cargo test -p polint --lib analysis_kernel::incremental::run_manifest::tests --locked -- --nocapture`
  — 8 passed.
- `cargo test -p polint --lib analysis_kernel::store::tests --locked -- --nocapture`
  — 37 passed.
- `cargo test -p polint --lib analysis_kernel::store::migrations::tests --locked -- --nocapture`
  — 21 passed.
- `cargo test -p polint --test public_surface_leak --locked -- --nocapture`
  — 7 passed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity --locked -- --nocapture`
  — 1 passed with byte-identical JSON and exit semantics.
- `make lint` — passed, including workspace/all-target/all-feature Clippy with
  warnings denied and formatting validation.
- `git diff --check f3f4612f..HEAD -- <eight reviewed files>` — passed.

No product source, test, fix report, backup artifact, or commit was modified by
this review.

---

_Reviewed: 2026-07-29_
_Reviewer: gsd-code-reviewer_
_Depth: deep_
