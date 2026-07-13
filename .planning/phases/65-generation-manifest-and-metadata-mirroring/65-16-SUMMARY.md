---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 16
subsystem: database
tags: [sqlite, migrations, semantic-store, typed-codecs, generation-lifecycle]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 15
    provides: Validated normalized StoreCommitPlan rows and final dependency-index schema
provides:
  - Strict atomic semantic-store schema v2 with 33 normalized metadata tables
  - Typed canonical relational codecs and semantic row ordering contracts
  - Identity-selected generation lifecycle, recovery, and sanitized failure constraints
  - Dynamic future-schema fixtures and mutation-free fail-closed schema validation
affects: [phase-65-generation-writer, semantic-store-publication, generation-recovery]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Keep raw SQLite and migration vocabulary inside the private store boundary"
    - "Use relational IDs only as handles; persist canonical identities and select active state through the manifest pointer"
    - "Validate every current-schema table, column, index, trigger, foreign key, and lifecycle invariant before use"

key-files:
  created:
    - crates/polint/src/analysis_kernel/store/schema.rs
  modified:
    - crates/polint/src/analysis_kernel/store/migrations.rs
    - crates/polint/src/analysis_kernel/store/connection.rs
    - crates/polint/src/analysis_kernel/store/mod.rs
    - crates/polint/src/analysis_kernel/store/tests.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/mod.rs

key-decisions:
  - "Schema v2 adds exactly 33 private tables beside the sole migration marker and replaces marker 1 with marker 2 atomically"
  - "Repeated generation identities are legal retry attempts distinguished by reservation ordinal; only store_manifest.active_generation_id selects the active complete generation"
  - "Layer and query result references retain canonical ProviderOutput digests, while only layer payload digests use LayerOutput"
  - "Deterministic generation statistics exclude cache outcomes, durations, timestamps, and mtime hints; optional mtime telemetry is stored separately"

patterns-established:
  - "Strict schema preflight: exact column sets plus required indexes/triggers, forbidden-name rejection, foreign-key checking, and lifecycle validation"
  - "Closed failure audit: CommitAttemptFailed with three reasons and eleven stages, attached only to inactive failed attempts"
  - "Canonical decode: relational readers delegate labels to owning parse or serde contracts and reject wrong-purpose digests"

requirements-completed: [STORE-04, STORE-05, META-01, META-04]

# Metrics
duration: 48min
completed: 2026-07-13
---

# Phase 65 Plan 16: Strict Semantic Store Schema v2 Summary

**A private normalized SQLite v2 boundary now mirrors every validated semantic metadata family with canonical codecs, explicit generation publication state, and atomic fail-closed migration behavior.**

## Performance

- **Duration:** 48 min
- **Started:** 2026-07-13T20:33:30Z
- **Completed:** 2026-07-13T21:21:51Z
- **Tasks:** 1
- **Files modified:** 7 implementation files

## Accomplishments

- Raised the store to schema v2 with 33 normalized tables covering the manifest, generations, input snapshots and children, provider metadata, layers, summaries, exact query dependencies, fact sidecars, one edge relation, validation events, deterministic statistics, separate telemetry, and sanitized failures.
- Made empty and v1 migrations atomic and idempotent, retained exactly one current marker, proved a mid-DDL failure restores the exact v1 shape, and made current/future/invalid preflight mutation-free and fail closed.
- Added typed codecs for input, capability, provider, layer, query-dependency, fact, validation, and dependency-edge vocabularies, including wrong-label, wrong-purpose digest, and path rejection.
- Enforced pristine, recoverable, and active-complete lifecycle shapes without newest-row selection; retry identities may repeat, and active publication always follows the manifest's explicit generation handle.
- Kept semantic statistics independent of telemetry and prohibited source/body/blob/raw-graph schema names while requiring fact and layer payload digests.

## Task Commits

Each task was committed atomically:

1. **Task 1: Add schema v2, codecs, lifecycle states, and dynamic future pins** - `68537729` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/store/schema.rs` - Canonical relational codecs, semantic ordering clauses, lifecycle validator, and closed failure vocabulary.
- `crates/polint/src/analysis_kernel/store/migrations.rs` - Atomic v1-to-v2 DDL, strict shape validation, lifecycle triggers, edge indexes, and migration/constraint tests.
- `crates/polint/src/analysis_kernel/store/connection.rs` - Dynamic future-schema fixtures and strict current-schema probes.
- `crates/polint/src/analysis_kernel/store/mod.rs` - Private schema module registration and enduring rebuild invariant.
- `crates/polint/src/analysis_kernel/store/tests.rs` - Dynamic future-version recovery expectations.
- `crates/polint/src/analysis_kernel/mod.rs` - Dynamic kernel parity fixture and enduring internal run-report lint reason.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Crate-private canonical CacheNodeKind re-export and enduring internal-boundary lint reasons.

## Decisions Made

- The store owns one singleton manifest. Binding is immutable, generations must match its workspace, and an active pointer can reference only a same-workspace complete generation with no failure event.
- Reservation ordinal is retry identity within a canonical generation identity, not a publication selector. Relational handles and insertion order have no selection semantics.
- Dependency endpoints stay first-class columns: input identities are decomposed, while layer/query/summary endpoints use generation-scoped foreign-key handles. Both endpoint directions have dedicated indexes.
- Current-schema validation compares exact column sets, requires every lifecycle trigger and edge index, rejects forbidden table or column names globally, runs foreign-key checks, and validates the persisted manifest state.
- Provider cache counters, demand cache outcomes, runtime durations, timestamps, and mtime hints do not enter generation statistics or selection. File mtime presence remains optional telemetry only.

## Deviations from Plan

### Approved Bounded Scope Adjustment

**1. Re-exported CacheNodeKind through the existing crate-private incremental vocabulary seam**

- **Found during:** Task 1 canonical codec implementation
- **Issue:** `CacheNodeKind` had the required canonical label/parser implementation but was trapped behind the private `incremental::dependency_index` child module, so the sibling store module could not name it without duplicating labels.
- **Fix:** With explicit approval, added `CacheNodeKind` to the existing `pub(crate)` incremental re-export and updated touched lint reasons to describe enduring internal boundaries.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/mod.rs`
- **Verification:** Schema codec tests, all-feature compilation, privacy audit, and strict workspace Clippy passed; no public re-export was introduced.
- **Committed in:** `68537729`

---

**Total deviations:** 1 approved private-scope adjustment
**Impact on plan:** The adjustment preserves one canonical parser and adds no public API, CLI, SDK, or backend exposure.

## Issues Encountered

- SQLite resolves a referenced table when inserting the singleton manifest, so the manifest row insert had to occur after creation of `generations`; the surrounding migration transaction preserved atomicity.
- Canonical producers distinguish aggregate identities from row payloads: layer outputs and query results/layer dependencies use `ProviderOutput`, while layer payload content uses `LayerOutput`. Focused schema assertions now lock those purposes.
- Strict Clippy identified a collapsible digest-kind check and the intentionally explicit failure-reason suffixes. The check was simplified and the required closed vocabulary received a narrow enduring lint expectation.

## User Setup Required

None - this is a private database boundary with no new configuration, CLI, SDK, or external service.

## Verification

- Migration suite: 18 passed, covering empty/v1/current/future/invalid states, sole-marker replacement, exact mid-DDL rollback, strict shape tampering, lifecycle constraints, digest/path rejection, stats separation, and edge indexes.
- Connection suite: 2 passed, covering dynamic future refusal without mutation and strict current-shape probing.
- Schema codec/lifecycle suite: 9 passed, covering every persisted vocabulary, negative labels, typed input dependency purpose, semantic ordering, and legal/illegal manifest shapes.
- Store facade and connection-policy suite: 14 passed serially.
- Kernel store-mode parity: 1 passed with byte-identical JSON and exit semantics.
- `cargo check -p polint --all-features --locked` passed without warnings.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed across the workspace and all example rule crates, including the pre-commit hook.
- Source audits confirmed raw SQL/rusqlite remains private, no schema module is exported, dynamic future fixtures use `CURRENT_SCHEMA_VERSION + 1`, and touched shipped-code comments contain no delivery chronology.

## Next Phase Readiness

- The generation writer can map `StoreCommitPlan` rows directly into normalized v2 tables without defining new semantic identities or label vocabularies.
- Reservation, failure, completion, activation, and transaction-commit stages now have closed codes and database lifecycle constraints ready for atomic publication.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-13*

## Self-Check: PASSED

Implementation commit `68537729` exists; schema v2 has the approved 33-table inventory and one marker; every plan row family and dependency schema is represented; canonical codecs, lifecycle states, sanitized failures, strict shape validation, payload-digest/privacy constraints, dynamic future fixtures, focused suites, all-feature compilation, formatting, strict Clippy, source audits, and the commit hook all pass.
