---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 15
subsystem: analysis-kernel
tags: [dependency-index, semantic-store, commit-plan, deterministic-metadata, privacy]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 14
    provides: Complete telemetry-free ValidatedRunMetadata with exact typed query dependencies and canonical run identities
provides:
  - Final dependency-index v2 wire label with fail-closed stale and future schema handling
  - Private deterministic StoreCommitPlan covering every validated semantic metadata family
  - Typed completeness, reference, status, path, payload, identity, count, and dependency validation before storage work
affects: [phase-65-schema-v2, phase-65-generation-persistence, semantic-store-writer]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Accept only ValidatedRunMetadata at the private store-planning boundary and call its authoritative integrity validator first"
    - "Copy canonical semantic identities verbatim while deriving only deterministic counts and logical sizes"
    - "Keep normalized semantic rows and optional runtime telemetry in separate plan projections"

key-files:
  created:
    - crates/polint/src/analysis_kernel/store/commit_plan.rs
    - .planning/phases/65-generation-manifest-and-metadata-mirroring/65-15-SUMMARY.md
  modified:
    - crates/polint/src/analysis_kernel/incremental/dependency_index.rs
    - crates/polint/src/analysis_kernel/incremental/layer_cache.rs
    - crates/polint/src/analysis_kernel/incremental/invalidation.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis_kernel/store/mod.rs
    - crates/polint/tests/cli.rs

key-decisions:
  - "polint-dependency-index-2 is the sole current wire label; v1, both temporary labels, unknown, and future labels are rejected without compatibility decoding"
  - "StoreCommitPlan is visible only to its store parent and accepts only an owned ValidatedRunMetadata"
  - "The plan copies workspace, full-config, run, generation, and family identities; it introduces no store-local digest or hash identity"
  - "Provider counters, demand cache outcomes, durations, and file mtime hints do not enter semantic rows, digests, counts, or logical sizes"

patterns-established:
  - "Complete plan gate: no normalized plan is returned until required validation events, canonical rows, references, exact query declarations, edge endpoints, identities, counts, and paths pass together"
  - "One edge truth: the store plan retains one canonical typed dependency-edge vector and derives no separate forward or reverse copy"
  - "Metadata-only privacy: stable fact sidecars retain payload_digest while source, body, blob, and raw graph fields remain absent"

requirements-completed: [STORE-04, META-01, META-04]

# Metrics
duration: 48min
completed: 2026-07-13
---

# Phase 65 Plan 15: Final Dependency Schema and Private Store Commit Plan Summary

**The final typed dependency wire now fails closed across every stale shape, and one validated run deterministically becomes a complete private metadata-only store plan before any storage backend opens.**

## Performance

- **Duration:** 48 min
- **Started:** 2026-07-13T19:42:29Z
- **Completed:** 2026-07-13T20:30:31Z
- **Tasks:** 2
- **Files modified:** 7 implementation files

## Accomplishments

- Published `polint-dependency-index-2` as the single final label for typed endpoints, one canonical edge vector, and mandatory query dependency inputs.
- Made v1, both temporary labels, unknown labels, and a future label fail closed through direct decoding, layer-cache invalid-read eviction, and conservative invalidation.
- Added a private `StoreCommitPlan` with owned first-class rows for run identities, input snapshots and children, provider manifests and generations, layers, summaries, queries, fact sidecars, dependencies, validation events, and semantic statistics.
- Required the authoritative validated-run integrity check before projection, then added typed plan checks for successful required events, provider/schema references, stable uniqueness, exact query declarations, retained edge endpoints, explicit statuses, repository-relative paths, copied identities, row counts, logical sizes, and payload digests.
- Proved 24 source-order permutations converge, runtime telemetry changes preserve the semantic plan, the exact forbidden field set is absent, and the analysis-kernel parent cannot name the plan type or constructor.

## Task Commits

Each task was committed atomically:

1. **Task 1: Publish the final dependency-index schema and retire temporary labels** - `e94ef2ab` (feat)
2. **Task 2: Normalize and validate every semantic family without backend coupling** - `75080e88` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/dependency_index.rs` - Final v2 schema label, typed serialized-shape proof, and literal fail-closed stale/future matrix.
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` - Final schema pin and stale-manifest invalid-read/eviction coverage.
- `crates/polint/src/analysis_kernel/incremental/invalidation.rs` - Final schema guard and conservative stale-label invalidation coverage.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Analysis-kernel-private visibility for the authoritative handoff integrity validator.
- `crates/polint/src/analysis_kernel/store/mod.rs` - Private commit-plan module declaration.
- `crates/polint/src/analysis_kernel/store/commit_plan.rs` - Backend-independent normalized plan, typed validation errors, semantic statistics, telemetry separation, and focused proof suite.
- `crates/polint/tests/cli.rs` - Private final-schema pin and exact production-literal ownership assertion.

## Decisions Made

- The final dependency-index version is published only after typed query inputs became mandatory. No legacy or temporary shape is translated, inferred, or rewritten as current.
- Store planning consumes only `ValidatedRunMetadata`; it does not accept a root, configuration, registry, cache, connection, or provider state.
- Existing canonical keys, typed digests, stable fact identities, and exact dependency endpoints are copied. The plan computes only deterministic row counts and serialized logical sizes for statistics.
- Query rows retain the complete canonical `QueryKey`, normalized typed input children, layer-digest children, and their one canonical edge subset.
- File mtime hints live in a separate telemetry vector. Provider cache counters, demand cache status, durations, and timestamps are already absent from the validated handoff and therefore cannot affect plan equality or statistics.
- `StoreCommitPlan`, `StorePlanError`, and `StoreGenerationStats` remain `pub(super)` inside a private module, so the analysis-kernel parent and all downstream/public surfaces cannot name them.

## Deviations from Plan

### Auto-fixed Issues

**1. Widened the authoritative integrity method within the private analysis-kernel boundary**

- **Found during:** Task 2 (normalize and validate every semantic family)
- **Issue:** `ValidatedRunMetadata::validate_integrity` was private to `run_report.rs`, while the plan required `StoreCommitPlan::from_validated_run` to call that exact authoritative validator. Duplicating its logic in the store would create competing integrity rules.
- **Fix:** Changed only the method visibility to `pub(in crate::analysis_kernel)` and called it directly before projection. The type, fields, module, and method remain unavailable outside the private analysis kernel.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/run_report.rs`
- **Verification:** All-feature compilation, focused store tests, privacy source assertions, and strict workspace clippy passed.
- **Committed in:** `75080e88` (Task 2 commit)

---

**Total deviations:** 1 auto-fixed private scope adjustment
**Impact on plan:** The adjustment preserves one authoritative integrity implementation and adds no public API or product surface.

## Issues Encountered

- Provider output schema identity uses the canonical aggregate `name:version` label, while individual layer keys retain a declared schema name. Store validation now verifies the layer name against manifest schema children rather than incorrectly treating those distinct canonical fields as identical.
- Strict clippy identified the typed dependency endpoint as making `StorePlanError` too large. Boxing only that diagnostic payload kept the error typed and made every result path lint-clean without changing semantic data.
- A lint-suppression reason initially described delivery chronology. It was replaced with the enduring private-writer ownership invariant before the Task 2 commit was finalized.

## User Setup Required

None - this is a private analysis-kernel/store boundary with no CLI, configuration, SDK, generated-skill, or external-service change.

## Verification

- Dependency-index tests: 10 passed, including the final typed shape and all five rejected stale/unknown/future labels.
- Layer-cache tests: 16 passed, including invalid-read eviction for every rejected schema label.
- Invalidation tests: 8 passed, including conservative drop behavior for every rejected schema label.
- Store commit-plan tests: 6 passed, covering every normalized family, 24 permutations, the typed negative matrix, semantic/telemetry separation, payload retention, exact forbidden keys, and privacy.
- Full serial CLI suite: all 166 cases passed with `--test-threads=1`.
- `cargo check -p polint --all-features --locked` passed without warnings.
- `cargo fmt --all -- --check` and `git diff --check` passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed in both task pre-commit hooks, including after the Task 2 amendment.
- Repository-wide schema audit reports only dependency-index construction, layer-cache pinning, invalidation checking, the read-only incremental re-export, and CLI private assertions.
- Exact source audit found no backend query vocabulary, relational row identity, alternate hash, or forbidden source/body/blob/graph fields in `commit_plan.rs`.

## Next Phase Readiness

- Plan 16 can map the private normalized rows to strict schema v2 codecs and transactions without defining any new semantic identity or dependency vocabulary.
- Every required semantic family, explicit state, copied identity, payload digest, validation event, and dependency endpoint is available behind the private store facade.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-13*

## Self-Check: PASSED

Both task commits exist; the final v2 schema consumer set and rejected-label matrix are exact; the private constructor accepts only `ValidatedRunMetadata` and calls its authoritative validator; all normalized families, copied identities, counts, logical sizes, payload digests, statuses, and one edge set are present; semantic telemetry exclusion and parent unnameability are proved; all focused, serial CLI, compilation, formatting, strict-lint, source-audit, and hook gates pass.
