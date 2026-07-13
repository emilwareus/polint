---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 10
subsystem: analysis-kernel
tags: [dependency-inputs, invalidation, typed-digests, stable-codecs, non-wire]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 09
    provides: Scoped production analysis identities and exact declared-input boundaries
provides:
  - Closed 19-kind dependency-input vocabulary with stable labels and typed decode failures
  - Purpose-checked constructors carrying stable key, typed digest, and explicit input status
  - Crate-private non-wire staging seam that leaves the v1 dependency-index contract unchanged
affects: [phase-65-typed-cache-nodes, phase-65-query-dependencies, phase-65-store-dependency-edges]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Typed dependency endpoints validate each existing digest purpose before construction"
    - "New invalidation vocabulary remains non-wire until producers and consumers migrate together"

key-files:
  created:
    - crates/polint/src/analysis_kernel/incremental/dependency_input.rs
    - .planning/phases/65-generation-manifest-and-metadata-mirroring/65-10-SUMMARY.md
  modified:
    - crates/polint/src/analysis_kernel/incremental/mod.rs

key-decisions:
  - "Reuse existing purpose-separated digests instead of adding a parallel digest kind or wire identity"
  - "Keep SearchManifest as reserved vocabulary backed by the existing dependency digest purpose, with no producer"
  - "Accept both GoLifecycle and TsJsLifecycle for the shared LanguageLifecycle endpoint"

patterns-established:
  - "Constructor boundary: every dependency-input kind rejects unrelated digest purposes before a key can exist"
  - "Availability boundary: present, absent, unsupported, and setup-missing remain explicit states on every endpoint kind"

requirements-completed: [META-01, META-04]

# Metrics
duration: 11min
completed: 2026-07-13
---

# Phase 65 Plan 10: Typed Dependency Input Vocabulary Summary

**A closed, purpose-checked dependency-input vocabulary now covers all metadata invalidation classes without changing any serialized cache node or v1 schema.**

## Performance

- **Duration:** 11 min
- **Started:** 2026-07-13T16:15:42Z
- **Completed:** 2026-07-13T16:26:45Z
- **Tasks:** 1
- **Files modified:** 2 implementation files

## Accomplishments

- Added all 19 required `InputDependencyKind` variants with exact snake-case labels, exhaustive parsing, and typed unknown-label rejection.
- Added canonical `InputDependencyKey` constructors that retain stable keys, typed digests, and all four explicit `InputComponentStatus` states while rejecting purpose mismatches.
- Registered a crate-private seam while proving `CacheNode`, dependency-index schema v1, layer manifests, and every producer remain unchanged and unable to emit typed nodes.

## Task Commits

Each task was committed atomically:

1. **Task 1: Define typed dependency inputs without changing any wire shape** - `5a3003a4` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/dependency_input.rs` - Closed input kinds, exact codecs, purpose-checked constructors, typed errors, and focused tests.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Narrow crate-private module and re-export seam.

## Decisions Made

- Existing digest purposes remain authoritative: source, config, lifecycle, tool, analysis setting/requirement, upstream layer, summary, query, budget, extension, model, and rule endpoints each validate against their current typed purpose.
- Vocabulary without a dedicated digest variant reuses the closest canonical identity: package/project uses workspace identity, provider schema uses provider-manifest identity, extension-declared input uses extension-code identity, and the reserved search-manifest endpoint uses dependency identity.
- The shared language-lifecycle constructor accepts both current language-specific lifecycle purposes and rejects every unrelated purpose.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - all changes are private, non-wire analysis-kernel vocabulary.

## Verification

- `cargo test -p polint --lib analysis_kernel::incremental::dependency_input::tests --locked`: 4 passed.
- `cargo test -p polint --lib analysis_kernel::incremental::dependency_index::tests --locked`: 4 passed.
- Negative construction audit found no `CacheNode::DependencyInput` or typed-node construction.
- Non-wire audit found none of the serialization markers forbidden by the plan in `dependency_input.rs`.
- Schema audit confirmed `DEPENDENCY_INDEX_SCHEMA` remains exactly `polint-dependency-index-1`.
- `cargo check -p polint --all-features --locked`: passed.
- `cargo fmt --all -- --check`: passed.
- `make lint`: passed, including workspace/all-target/all-feature Clippy with warnings denied.
- The task commit hook reran `make lint` and passed.

## Next Phase Readiness

- Plan 11 can add the first typed cache-node wire shape using this closed vocabulary and its purpose checks.
- No producers, consumers, manifests, or v1 serialized fixtures were migrated early.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-13*

## Self-Check: PASSED

The task commit and both implementation files exist; all 19 kind labels and constructors, all four statuses, every mismatch path, the v1 wire guard, formatting, all-feature compilation, strict lint, and commit hooks pass.
