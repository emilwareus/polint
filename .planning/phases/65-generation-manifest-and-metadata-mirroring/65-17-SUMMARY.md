---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 17
subsystem: database
tags: [sqlite, semantic-store, atomic-publication, generation-lifecycle, concurrency]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 16
    provides: Strict normalized schema v2, generation states, lifecycle triggers, and closed failure codecs
provides:
  - Atomic first-workspace binding and pending generation reservation
  - Complete normalized metadata publication with typed post-write validation and active-pointer activation
  - Trusted best-effort failure audit with exact pending-attempt revalidation
  - Active-complete typed reader with exact query dependencies and canonical DependencyIndex reconstruction
  - Telemetry-independent semantic trust and selection
affects: [phase-65-store-integration, semantic-store-publication, generation-recovery]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Use separate immediate transactions for atomic reservation and complete publication"
    - "Select durable truth only through the singleton active pointer, then independently validate complete semantic metadata"
    - "Treat telemetry as strict publication data but best-effort, non-gating active-read data"

key-files:
  created:
    - crates/polint/src/analysis_kernel/store/generation.rs
  modified:
    - crates/polint/src/analysis_kernel/store/connection.rs
    - crates/polint/src/analysis_kernel/store/mod.rs
    - crates/polint/src/analysis_kernel/store/tests.rs
    - crates/polint/src/analysis_kernel/store/commit_plan.rs

key-decisions:
  - "First binding and reservation commit together; a cross-workspace contender cannot mutate the store"
  - "A changed pointer is a stale reservation only when its current target has a valid complete same-workspace identity and no failure event"
  - "Only exact, payload-free pending attempts in unchanged trusted state may receive a closed failure audit"
  - "Active reads never use recency or insertion order and never fall back to an older complete generation"

patterns-established:
  - "Strict typed mirror: publication re-reads every normalized family and compares the reconstructed plan before completion"
  - "Rollback boundary injection: every logical write group, activation, and transaction commit has deterministic failure coverage"
  - "Telemetry isolation: malformed telemetry degrades to an empty telemetry view without changing semantic validation or selection"

requirements-completed: [STORE-04, STORE-05]

# Metrics
duration: 1h 51min
completed: 2026-07-14
---

# Phase 65 Plan 17: Atomic Generation Lifecycle Summary

**The private semantic store now binds, reserves, publishes, audits, and reads complete generations atomically while preserving the prior active truth across contention, stale writers, and every injected failure boundary.**

## Performance

- **Duration:** 1h 51 min
- **Started:** 2026-07-13T23:23:09+02:00
- **Completed:** 2026-07-14T01:14:12+02:00
- **Tasks:** 1
- **Files modified:** 5 implementation files

## Accomplishments

- Added a two-transaction writer: the first immediate transaction atomically binds a pristine store and inserts one exact pending reservation; the second validates the unchanged trusted state, writes every normalized family, strictly re-reads it, completes the parent, updates the active pointer, validates the active projection, and commits.
- Added a fail-closed active reader that starts only from the singleton pointer, independently requires a complete same-workspace generation with valid identities and no failure event, reconstructs every typed metadata family, and rebuilds the canonical dependency index from the single edge relation.
- Added separate best-effort auditing that revalidates schema, workspace, pointer, identity, ordinal, pending status, zero payload rows, and zero prior events before atomically persisting one closed failure event.
- Proved both first-binding race orders, same-workspace retries, stale reservations, all eleven failure stages, post-pointer rollback, missing and mismatched provider children, busy audit behavior, pending and failed isolation, active tampering, exact query inputs, no fallback, and corrupt telemetry isolation.

## Task Commits

Each task was committed atomically:

1. **Task 1: Bind/reserve atomically, publish completely, and read only active complete** - `14fc29d9` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/store/generation.rs` - Private reservation, publication, trusted audit, typed active reader, relational writers/readers, and deterministic failure controls.
- `crates/polint/src/analysis_kernel/store/connection.rs` - Private immediate-transaction, read-connection, schema-validation, and SQLite-classification seams.
- `crates/polint/src/analysis_kernel/store/mod.rs` - Private generation module plus sanitized workspace, stale, invalid-plan, commit-failure, and invalid-metadata outcomes.
- `crates/polint/src/analysis_kernel/store/tests.rs` - Real-kernel generation fixtures and focused concurrency, rollback, audit, tampering, telemetry, and typed round-trip coverage.
- `crates/polint/src/analysis_kernel/store/commit_plan.rs` - Store-private validation seam shared by publication and active reconstruction, with an enduring internal-boundary lint reason.

## Decisions Made

- Reservation ordinals remain retry identity only. They are validated as a contiguous sequence within one canonical generation identity but never select the active generation.
- A publication whose saved pointer changed returns a sanitized stale-reservation skip only after validating the new pointer target's complete header, same workspace, recomputed identities, and lack of failure events. The stale row remains pending and unaudited.
- Publication round trips telemetry strictly so the writer proves what it wrote. Active selection treats any telemetry query or codec failure as non-gating and returns an empty telemetry view while preserving the exact semantic plan and dependency index.
- SQLite errors attach follow-up audits only for trusted busy or other operational failures. Corrupt, future, invalid-schema, identity, workspace, and other untrusted states structurally carry no audit request.
- Provider child rows remain children of the pending parent. Complete status and activation occur only after the entire typed projection, including all child counts and relationships, matches the validated source plan.

## Deviations from Plan

### Approved Bounded Scope Adjustment

**1. Shared the normalized-plan validator with the sibling generation boundary**

- **Found during:** Task 1 publication preflight and active reconstruction
- **Issue:** `StoreCommitPlan::validate` was private to `commit_plan.rs`, but the sibling private generation module must validate before reservation and after typed reconstruction without duplicating the contract.
- **Fix:** With explicit approval, widened only `StoreCommitPlan::validate` to `pub(super)` and updated its non-test dead-code reason to cover both publication and active reads. The type, constructor boundary, SQL, connection types, and relational handles remain private.
- **Files modified:** `crates/polint/src/analysis_kernel/store/commit_plan.rs`
- **Verification:** All-feature compilation, strict workspace Clippy, bare-public and rusqlite-boundary audits, and the complete store suite passed.
- **Committed in:** `14fc29d9`

---

**Total deviations:** 1 approved private-scope adjustment
**Impact on plan:** The adjustment keeps one canonical validator within the existing private store boundary and adds no SDK, CLI, public crate, or backend exposure.

## Issues Encountered

- Persisted input-group labels sort lexically, while `StoreInputGroup` has a typed canonical order. The reader now keeps deterministic SQL ordering for stable scans, decodes the rows, and typed-sorts components and details before strict plan validation.
- Active telemetry initially shared the strict publication path. Splitting strict and best-effort telemetry policies ensured malformed telemetry cannot gate semantic trust or selection while preserving exact writer validation.
- A same-workspace writer can reserve against active A and become stale before its publication transaction begins. The validation path now distinguishes a verified newer active target from identity or schema corruption and leaves the stale attempt isolated and unaudited.
- Strict Clippy requested the `?` operator in one test-only reservation wrapper; the wrapper was simplified without changing behavior.

## User Setup Required

None - this remains a private store boundary with no new configuration, CLI, SDK, or external service.

## Verification

- First-binding race suite: 1 passed, forcing both A-wins and B-wins orderings within the bounded busy timeout.
- Generation lifecycle suite: 6 passed, covering retries, all eleven rollback boundaries, provider child validation, deterministic busy audit, stale publication, and invalid-plan preflight.
- Active-complete reader suite: 5 passed, covering full typed projection, exact query dependencies and edge reconstruction, null-active states, workspace mismatch, pending/failed isolation, corrupt telemetry, active tampering, and no fallback to an older complete generation.
- Complete store suite: 26 passed serially.
- `cargo check -p polint --all-features --locked` passed without warnings.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed across the workspace and all example rule crates, including the pre-commit hook.
- Source audits confirmed no delivery chronology or TODO markers in touched shipped code, no bare public API additions, rusqlite and connection types remain inside `analysis_kernel::store`, active selection contains no MAX/time/rowid/ID-order fallback, and failure rows contain only closed event/reason/stage codes.

## Next Phase Readiness

- The private lifecycle is ready to be connected to the production kernel/store execution path without changing the schema or normalized handoff contract.
- Atomic publication, typed active reads, trusted recovery residue, and telemetry separation are proven in isolation.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-14*

## Self-Check: PASSED

Implementation commit `14fc29d9` exists; atomic binding/reservation, complete publication, exact trusted audit, active-only typed reads, stale-writer isolation, telemetry-independent selection, all focused and store tests, all-feature compilation, formatting, strict Clippy, privacy/source audits, and the commit hook all pass. The single file-list deviation was explicitly approved and remains store-private.
