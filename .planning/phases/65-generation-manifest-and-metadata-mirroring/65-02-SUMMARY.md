---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 02
subsystem: database
tags: [sqlite, run-manifest, canonical-identity, atomic-publication, tamper-resistance]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    provides: R1 opaque generation reservation, atomic completion/selection, and rollback-safe active truth
provides:
  - Private canonical workspace/config/source run-manifest projection with a versioned typed codec
  - Exact schema-v3 manifest header/source family with bounded reconstruction and empty-v2-only migration
  - Manifest-aware atomic publication, workspace ownership refusal, and one-snapshot exact matching
affects: [phase-65-r3-r6, semantic-store, durable-metadata, provider-mirroring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Persist explicit purpose-typed relational fields, then reconstruct and recompute identity before trust"
    - "Authenticate active workspace ownership inside the same immediate transaction before candidate mutation"
    - "Preflight storage classes, cardinality, scalar lengths, and aggregate size before bounded allocation"

key-files:
  created:
    - crates/polint/src/analysis_kernel/incremental/run_manifest.rs
  modified:
    - crates/polint/src/analysis_kernel/incremental/digest.rs
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/analysis_kernel/store/migrations.rs
    - crates/polint/src/analysis_kernel/store/connection.rs
    - crates/polint/src/analysis_kernel/store/generation.rs
    - crates/polint/src/analysis_kernel/store/mod.rs
    - crates/polint/src/analysis_kernel/store/tests.rs

key-decisions:
  - "The manifest contains only a hashed canonical workspace root, the complete config identity, canonical source rows, derived count, and recomputed run identity"
  - "Populated schema-v2 stores require rebuild instead of receiving synthesized legacy manifests; only empty v2 migrates to v3"
  - "Exact match compares fully decoded canonical fields, while the run digest is an authentication witness rather than sole equality"
  - "The sub-five-minute CI target remains deferred and the CI workflow is unchanged"

patterns-established:
  - "Complete and active generations are trustworthy only when their owned manifest decodes and recomputes exactly"
  - "Workspace ownership is checked before any candidate header, source, lifecycle, or selection mutation"
  - "Handle-only active reads delegate to the authenticated manifest read and discard only validated payload"

requirements-completed: []

# Metrics
duration: 40m
completed: 2026-07-28
---

# Phase 65 Plan 02: Run Manifest R2 Summary

**A private schema-v3 run manifest now authenticates canonical workspace, complete-config, and exact source membership across atomic generation publication and one-snapshot matching.**

This completes only restart slice R2. R1 and R2 are complete, while Phase 65
remains open with R3-R6 outstanding. STORE-04, STORE-05, META-01, and META-04
remain open.

## Performance

- **Duration:** ~40 min
- **Started:** 2026-07-28T21:00:59Z
- **Completed:** 2026-07-28T21:40:56Z
- **Tasks:** 3
- **Implementation/test files modified:** 8
- **Bounded implementation delta:** 1,883 additions, 85 deletions

## Accomplishments

- Added a private, versioned canonical manifest that losslessly hashes the canonical workspace root without retaining its raw path, wraps the complete config hash in a closed purpose, normalizes supported-language source rows, and derives a creation-independent identity.
- Advanced the store to exact schema v3 with one generation-owned manifest header/source family, strict catalog/content authentication, bounded typed reconstruction, and migration only from empty schema v2.
- Refused populated v2 before WAL/synchronous policy changes and repeated the refusal inside the migration transaction, with reopen proof that journal mode, schema, marker, generations, and selection remain unchanged.
- Bound manifest write/readback, identity recomputation, completion, cardinality authentication, active selection, and joined validation into one immediate publication transaction.
- Added one-snapshot active-manifest reads with distinct no-active, exact, mismatch, and malformed-refusal outcomes; handle-only reads cannot bypass manifest authentication.
- Proved different canonical workspaces sharing a store path cannot replace one another: ownership refusal occurs before candidate mutation and the original manifested active truth survives reopen.
- Expanded publication failure injection to eleven seams spanning header, sources, stored decode, completion, selection, and commit; populated A/M1 and B/M2 fixtures prove the prior manifest survives while the candidate remains pending with no header or source rows.
- Added semantic mismatch, header-scalar tamper, source membership/ownership/storage-class tamper, allocation-bound, disabled-before-I/O, public-surface, and byte/exit parity proof.
- Kept providers, normal kernel publication/reuse wiring, public CLI/config/SDK/output, and `.github/workflows/ci.yml` unchanged.

## Task Commits

Each task was committed atomically:

1. **Task 1: Define the private canonical manifest and versioned codec** - `06d9082b` (feat)
2. **Task 2: Add bounded schema-v3 manifest storage and migration refusal** - `5f73992e` (feat)
3. **Task 3: Bind atomic publication, exact matching, rollback, and tamper refusal** - `20d5a92c` (feat)

**Plan metadata:** `f3f4612f` (docs: bounded R2 plan)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/run_manifest.rs` - Canonical input projection, closed manifest vocabulary, typed SQL codec, and identity recomputation.
- `crates/polint/src/analysis_kernel/incremental/digest.rs` - Minimal closed workspace/run digest kinds and deterministic byte/integer field support.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Curated crate-private manifest exports.
- `crates/polint/src/analysis_kernel/store/migrations.rs` - Exact schema-v3 family, strict invariants, empty-v2-only migration, and populated-v2 no-mutation proof.
- `crates/polint/src/analysis_kernel/store/connection.rs` - Read transactions and typed manifest-aware reopen snapshots.
- `crates/polint/src/analysis_kernel/store/generation.rs` - Atomic manifest publication, ownership authentication, bounded decode, exact matching, and failure seams.
- `crates/polint/src/analysis_kernel/store/mod.rs` - Disabled-first manifest-aware private facade.
- `crates/polint/src/analysis_kernel/store/tests.rs` - Round-trip, mismatch, ownership, rollback, tamper, bounds, and disabled-mode tests.

## Decisions Made

- Used canonical platform path units only as input to a purpose-separated digest; no absolute workspace path is stored in memory after construction or persisted in SQLite.
- Stored source rows by canonical relative path with a composite key and no ordinal, so membership order is derived rather than ambient.
- Required exact field equality after typed decode and run-identity recomputation, preventing a matching digest from substituting for semantic equality.
- Treated a populated v2 store as rebuild-needed rather than inventing identity that the older schema never recorded.
- Used a 512 MiB production aggregate decode ceiling and a smaller test-only ceiling to exercise refusal deterministically without large fixtures.

## Deviations from Plan

### Sequencing Adjustment

**1. Kept temporary lint expectations between schema storage and facade wiring**

- **Found during:** Task 2 commit verification.
- **Issue:** The storage codec was intentionally not a production caller until Task 3, so strict dead-code linting rejected the intermediate atomic commit.
- **Fix:** Added narrowly scoped expectations for the unwired boundary, then removed them when Task 3 connected the private facade. The input constructor retains a scoped expectation because normal kernel publication remains deliberately disabled.
- **Impact:** Commit sequencing only; final behavior and scope match the plan.
- **Committed in:** `5f73992e`, `20d5a92c`

**Total deviations:** 1 sequencing-only adjustment; zero product-scope deviations.

## Issues Encountered

- The first Task 2 commit hook found an over-broad set of lint expectations; narrowing the expectations made the intermediate boundary lint-clean without weakening workspace lint policy.
- The full workspace suite passed, but Cargo reported several pre-existing evaluation and CLI integration tests taking longer than 60 seconds. Every new focused R2 test completed in well under one second, and no timeout, serialization, or CI workflow was changed.

## Verification

- `cargo test -p polint --lib analysis_kernel::incremental::run_manifest::tests --locked`: 7 passed.
- `cargo test -p polint --lib analysis_kernel::store::migrations::tests --locked`: 21 passed.
- `cargo test -p polint --lib analysis_kernel::store::tests::run_manifest_storage --locked`: 2 passed.
- `cargo test -p polint --lib analysis_kernel::store::tests::run_manifest --locked`: 6 passed.
- `cargo test -p polint --lib analysis_kernel::store::tests --locked`: 35 passed.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity --locked`: 1 passed; public JSON bytes and exit semantics remained identical.
- `cargo test -p polint --test public_surface_leak --locked`: 7 passed.
- `cargo fmt --all -- --check`: passed.
- `make lint`: passed.
- `cargo test --workspace --all-features --locked`: passed.
  - polint library: 2,519 passed, 2 intentional ignores.
  - CLI integration: 166 passed.
  - public-surface integration: 7 passed.
  - polint-bench: 2 passed.
  - polint-macros: 11 passed.
  - example crates and doctests: all passed.
- `git diff --check`: passed.
- Scope audit from the pre-code plan commit: exactly eight implementation/test files, 1,883 added lines, one manifest schema family, zero provider families, and no CI/public-surface/normal-kernel wiring changes.

## User Setup Required

None - manifest publication and matching remain private and unwired from normal production analysis.

## Next Phase Readiness

- R1 and R2 now provide crash-safe generations plus canonical run identity for later provider/fact mirroring.
- R3-R6 must still add provider families, dependency/invalidation relationships, metadata queries, and broader integration before Phase 65 can complete.
- STORE-04, STORE-05, META-01, and META-04 remain open; R2 contributes toward them without satisfying them.
- The sub-five-minute CI objective remains a separately tracked follow-up, explicitly deferred by the user.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-28 (R2 only; phase remains open)*

## Self-Check: PASSED

The eight bounded implementation/test files and this summary exist, all three
task commits are in history, focused and workspace verification passed, and no
Phase 65 or requirement completion marker was written.
