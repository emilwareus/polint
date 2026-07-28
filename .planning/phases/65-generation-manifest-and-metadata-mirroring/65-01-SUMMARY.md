---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 01
subsystem: database
tags: [sqlite, migrations, generations, atomic-publication, rollback]

# Dependency graph
requires:
  - phase: 64-store-foundation-and-boundary-proof
    provides: Private SQLite boundary, migration policy, writer lease, recovery, and public no-leak proof
provides:
  - Exact schema-v2 pending/complete generation lifecycle with one complete-only active relationship
  - Opaque store-local reservation, atomic publication/selection, and explicit active-generation reads
  - Deterministic rollback and reopen proof at every publication seam
affects: [phase-65-r2-r6, semantic-store, durable-metadata, store-recovery]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Reserve durable pending state separately, then complete and select the same handle in one immediate transaction"
    - "Read durable truth only through an explicit authenticated singleton relationship"
    - "Exercise transaction rollback with finite cfg(test)-only failure seams and typed reopen snapshots"

key-files:
  created:
    - crates/polint/src/analysis_kernel/store/generation.rs
  modified:
    - crates/polint/src/analysis_kernel/store/migrations.rs
    - crates/polint/src/analysis_kernel/store/connection.rs
    - crates/polint/src/analysis_kernel/store/mod.rs
    - crates/polint/src/analysis_kernel/store/tests.rs
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "Schema v2 adds only generations plus a complete-only singleton active relationship; the integer handle is relational, not semantic identity"
  - "Publication updates pending to complete and replaces the active relationship in one immediate transaction; reads never infer newest state"
  - "Failure injection is finite, deterministic, and test-only, with no environment variable, feature, global lock, or production mode"
  - "The sub-five-minute CI target remains deferred and the CI workflow is unchanged"

patterns-established:
  - "Pending rows are durable but unreadable until the exact handle is completed and selected atomically"
  - "Malformed, future, pending, dangling, and invalid lifecycle state is a typed refusal rather than inferred truth"
  - "Lifecycle fixture snapshots expose typed ordered handles/statuses while keeping SQL and scalar IDs inside the store subtree"

requirements-completed: []

# Metrics
duration: 38min
completed: 2026-07-28
---

# Phase 65 Plan 01: Generation Lifecycle R1 Summary

**A private schema-v2 generation primitive now reserves opaque pending handles, atomically completes and explicitly selects the same handle, and preserves prior durable truth across every tested publication failure.**

This completes only restart slice R1. Phase 65 is not complete: R2-R6 remain
open, and STORE-04, STORE-05, META-01, and META-04 remain open.

## Performance

- **Duration:** ~38 min
- **Started:** 2026-07-28T19:53:33+02:00
- **Completed:** 2026-07-28T20:31:05+02:00
- **Tasks:** 3
- **Implementation/test files modified:** 6
- **Bounded implementation delta:** 1,427 additions, 86 deletions

## Accomplishments

- Advanced the private store to exact schema version 2 with only a positive generation handle, closed `pending`/`complete` status, and one singleton relationship constrained to complete generations.
- Preserved exact v0/v1 migration support, authenticated current schema shape and contents strictly, and refused malformed or future state without mutation.
- Added opaque reservation, same-handle atomic completion/selection, explicit relationship-based reads, typed invalid-transition handling, and disabled-before-I/O lifecycle guards.
- Proved rollback at all five finite publication seams: before update, after update, before selection, after selection, and before commit.
- Proved successful reopen, A-to-B rotation, newer pending C remaining unreadable, first-publication failure preserving `None`, relational rejection of pending selection, and byte/exit parity for public check behavior.
- Kept production kernel wiring, public CLI/config/SDK/output, providers, facts, metadata families, and `.github/workflows/ci.yml` unchanged.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add and strictly authenticate the minimal schema-v2 lifecycle** - `344cba59` (feat)
2. **Task 2: Implement opaque reservation, atomic publication, and explicit active reads** - `159f4cac` (feat)
3. **Task 3: Prove rollback, reopen, exact selection, and public-behavior parity** - `92500234` (test)

**Plan metadata:** `9f3bdc56` (docs: bounded R1 plan)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/store/generation.rs` - Opaque handle, lifecycle operations, relationship authentication, and cfg(test)-only failure seams.
- `crates/polint/src/analysis_kernel/store/migrations.rs` - Ordered schema-v2 migration plus exact shape, constraint, marker, contents, and foreign-key validation.
- `crates/polint/src/analysis_kernel/store/connection.rs` - Existing-policy immediate transaction/read callbacks, read preflight, relative future fixture, and typed lifecycle snapshots.
- `crates/polint/src/analysis_kernel/store/mod.rs` - Crate-private lifecycle facade with hard disabled guards and typed error mapping.
- `crates/polint/src/analysis_kernel/store/tests.rs` - Lifecycle, constraint, reopen, failure matrix, and relative future-version tests using independent temporary stores.
- `crates/polint/src/analysis_kernel/mod.rs` - Test-only future-schema parity expectation derived from the current schema version.

## Decisions Made

- Used the composite `(generation_id, required_status)` foreign key so the database itself prevents a pending generation from satisfying the active relationship.
- Kept reservation in its own durable immediate transaction and publication in a later immediate transaction, allowing failed candidates to remain pending while the prior active relationship survives.
- Validated the exact candidate before selection and the exact active relationship before commit; zero-row, repeated, and unknown publications return typed rejection.
- Queried only the singleton active relationship joined to its complete generation. No `MAX(id)`, timestamp, insertion-order, or recency inference exists.
- Kept `GenerationHandle` relational and opaque: no serialization, display, timestamps, hashes, semantic identity, or public constructor/accessor was added.

## Deviations from Plan

### Sequencing Adjustment

**1. Added the initial lifecycle acceptance tests during Task 2**

- **Found during:** Task 2 verification.
- **Issue:** The Task 2 command targeted `store::tests::generation_lifecycle`; waiting until Task 3 to create that already-planned test module would have made Task 2's required verification empty.
- **Fix:** Added the five basic lifecycle/disabled/constraint tests to the plan-listed `store/tests.rs` in Task 2, then extended the same module with rollback and reopen proof in Task 3.
- **Impact:** Commit sequencing only. The final file set, behavior, and scope match the plan; no extra file or product surface was introduced.
- **Committed in:** `159f4cac`

**Total deviations:** 1 sequencing adjustment; zero product-scope deviations.

## Issues Encountered

- Activating the previously retained read-only connection path made two old `dead_code` lint expectations stale; they were removed once the lifecycle legitimately used that path.
- The full workspace command completed successfully, but Cargo reported several pre-existing evaluation and CLI integration tests running longer than 60 seconds. The new R1 lifecycle suite completed in 0.07 seconds, adds no global serialization, and changes no timeout or CI workflow.

## Verification

- `cargo test -p polint --lib analysis_kernel::store::migrations::tests --locked`: 16 passed in 0.03 s.
- `cargo test -p polint --lib analysis_kernel::store::tests::generation_lifecycle --locked`: 9 passed in 0.07 s.
- `cargo test -p polint --lib analysis_kernel::store::tests --locked`: 23 passed in 0.32 s.
- `cargo test -p polint --lib analysis_kernel::tests::semantic_store_check_parity --locked`: 1 passed; JSON bytes and exit semantics remained identical across all store modes.
- `make lint`: passed.
- `cargo test --workspace --all-features --locked`: passed.
  - polint library: 2,495 passed, 2 intentional ignores.
  - CLI integration: 166 passed.
  - public-surface integration: 7 passed.
  - polint-bench: 2 passed.
  - polint-macros: 11 passed.
  - example crates and doctests: all passed.
- `git diff --check`: passed.
- Scope audit from the pre-code plan commit: exactly six implementation/test files, 1,427 added lines, one lifecycle schema family, zero provider families, and no CI/public-surface file changes.

## User Setup Required

None - the lifecycle remains private and unwired from normal production kernel execution.

## Next Phase Readiness

- R1 is complete and provides the crash-safe publication primitive needed by later Phase 65 slices.
- R2-R6 must still define and implement manifest/input identity, provider/fact mirroring, dependency/invalidation relationships, metadata queries, and broader integration before Phase 65 can complete.
- STORE-04, STORE-05, META-01, and META-04 remain open; this plan contributes toward them but does not satisfy them.
- The sub-five-minute CI objective remains a separately tracked follow-up, explicitly deferred by the user; no workflow or branch-protection changes were made.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-28 (R1 only; phase remains open)*

## Self-Check: PASSED

The six bounded implementation/test files and this summary exist, all three
task commits are in history, focused and workspace verification passed, and no
Phase 65 or requirement completion marker was written.
