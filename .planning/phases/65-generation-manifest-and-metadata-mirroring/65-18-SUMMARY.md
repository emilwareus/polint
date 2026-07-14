---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 18
subsystem: database
tags: [sqlite, semantic-store, invalidation, parity, telemetry]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 17
    provides: Atomic normalized generation publication and active-complete typed reconstruction
provides:
  - Enabled-only post-validation kernel commit through one private semantic-store facade
  - Persisted run-manifest and diagnostic nodes with typed dependency endpoints
  - Exact persisted invalidation and sibling-reuse coverage across metadata and status changes
  - Policy-neutral store outcomes with byte-identical check and review behavior
  - Zero materialization, path creation, and store I/O while disabled or using no-cache
affects: [phase-65-privacy-performance-gates, semantic-store-integration, metadata-invalidation]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Branch on store enablement before validated-handoff, plan, path, or I/O materialization"
    - "Persist result-boundary identities as normalized typed rows and reconstruct the existing dependency planner input"
    - "Keep store status, deterministic statistics, and telemetry private and policy-neutral"

key-files:
  created: []
  modified:
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/incremental/dependency_index.rs
    - crates/polint/src/analysis_kernel/store/mod.rs
    - crates/polint/src/analysis_kernel/store/commit_plan.rs
    - crates/polint/src/analysis_kernel/store/generation.rs
    - crates/polint/src/analysis_kernel/store/migrations.rs
    - crates/polint/src/analysis_kernel/store/tests.rs
    - crates/polint/tests/cli.rs

key-decisions:
  - "Run-manifest identity binds canonical RunIdentity to the complete ConfigIdentity; rule code and options bind only diagnostic nodes"
  - "Diagnostic requested views are a canonical set whose typed aggregate is part of normalized uniqueness while child rows remain first-class"
  - "The unreleased incomplete v2 shape is rejected without mutation and repaired only through the existing controlled rebuild path"
  - "Store-state coverage lives at the kernel/render boundary, while real check/review default-versus-no-cache parity proves the public command boundary"

patterns-established:
  - "Validated control plane: validation diagnostics and finalized facts precede the enabled-only store handoff"
  - "Persisted invalidation proof: commit, reopen, reconstruct typed edges, then feed the unchanged planner"
  - "Canonical result identities: aggregate identity belongs to the key model, never to a store-local hashing function"

requirements-completed: [STORE-04, STORE-05, META-01, META-04]

# Metrics
duration: 1h 37min
completed: 2026-07-14
---

# Phase 65 Plan 18: Validated Semantic Store Control Plane Summary

**Finalized kernel runs now enter the private semantic store only after authoritative validation, with normalized result identities, exact persisted invalidation, zero disabled work, and byte-identical public policy behavior.**

## Performance

- **Duration:** 1h 37 min
- **Started:** 2026-07-13T23:23:47Z
- **Completed:** 2026-07-14T01:00:52Z
- **Tasks:** 1
- **Files modified:** 17 implementation files

## Accomplishments

- Replaced maintenance-only kernel integration with an enabled-only validated-run commit after validation diagnostics are preserved and fact metadata is finalized. Disabled and no-cache paths now branch before handoff, plan, path, connection, or I/O work and prove zero materialization counters.
- Added private run-manifest and diagnostic cache nodes, normalized requested-view child rows, typed run-manifest/diagnostic edge handles, strict active reconstruction, and deterministic statistics without exposing any CLI, SDK, or public crate surface.
- Committed and reopened a complete metadata invalidation matrix covering source, workspace, provider, lifecycle, tool, config, layer, summary, query, budget, search, extension, model, rule, and exact query-declaration inputs. Referenced nodes invalidate precisely, unrelated siblings and analysis layers reuse, unchanged state reuses everything, and twenty order permutations remain identical.
- Proved Present-to-Absent, Absent-to-Present, Unsupported-to-SetupMissing, and SetupMissing-to-Unsupported transitions through persisted prior and next typed inputs, distinct semantic identities/indexes, and canonical change rows against the prior reconstructed index.
- Expanded private parity coverage across complete, mismatch, first-failure recovery, audited failed, pending, busy, future, invalid, corrupt, and unsafe store states, while real check and review commands remain byte- and exit-identical between default and `--no-cache` operation.

## Task Commits

Each task was committed atomically:

1. **Task 1: Commit real finalized runs and execute the persisted META-04 matrix** - `fdf1d63a` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/mod.rs` - Exact post-validation/finalization ordering, enabled-only handoff, zero-work counters, and store-state policy parity.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Validated-run construction independent of a report, run-manifest/diagnostic boundary edges, canonical identities, and private outcome retention.
- `crates/polint/src/analysis_kernel/incremental/dependency_index.rs` - Typed run-manifest cache node and stable key codec.
- `crates/polint/src/analysis_kernel/incremental/digest.rs` - Run identity decoding required by typed active reconstruction.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Canonical requested-view set and its single model-owned aggregate digest.
- `crates/polint/src/analysis_kernel/incremental/invalidation.rs` - Run-manifest invalidation classification through the existing planner.
- `crates/polint/src/analysis_kernel/store/mod.rs` - Sole parent-facing commit facade, sanitized private outcome/statistics, and test-only controls.
- `crates/polint/src/analysis_kernel/store/commit_plan.rs` - Normalized run-manifest/diagnostic rows, result-boundary validation, exact row accounting, and honest logical-byte families.
- `crates/polint/src/analysis_kernel/store/connection.rs` - Exact incomplete-v2 fixture and full user-defined SQLite schema snapshots for no-mutation recovery proof.
- `crates/polint/src/analysis_kernel/store/generation.rs` - Normalized result-boundary writers/readers, typed endpoint reconstruction, and strict round-trip checks.
- `crates/polint/src/analysis_kernel/store/migrations.rs` - Strict v2 result-boundary tables, uniqueness, typed endpoints, counts, logical bytes, and required-column checks.
- `crates/polint/src/analysis_kernel/store/schema.rs` - Run-manifest node label codec and closed failure-stage vocabulary.
- `crates/polint/src/analysis_kernel/store/tests.rs` - Persisted invalidation/status/order matrix, diagnostic uniqueness, schema recovery, telemetry, concurrency, and active-reader coverage.
- `crates/polint/tests/cli.rs` - Public check/review byte and exit parity plus private-marker/help/path leak checks.
- `crates/polint/src/eval/performance.rs` - Updated private report fixture construction.

## Decisions Made

- Full configuration changes target the run-manifest node through the complete `ConfigIdentity`; individual rule code/options inputs target diagnostic nodes and do not invalidate unrelated analysis layers.
- Requested-view membership is normalized as a sorted, deduplicated set in `DiagnosticKey`. Its canonical dependency digest participates in the diagnostic uniqueness boundary, while every requested view also persists as a normalized typed child row.
- Store failures remain sanitized private outcomes. They cannot modify the analysis database, capabilities, diagnostics, renderer bytes, or exit semantics, and deterministic statistics are returned only after the committed active generation exactly reconstructs the source plan.
- Telemetry counters, durations, cache statuses, and file-time hints remain outside semantic identities and selection. Diagnostic logical bytes count diagnostic rows only; run-manifest bytes belong to the input family.
- Because schema v2 is unreleased, a database with the exact older v2 table/index/statistics shape is invalid rather than incrementally patched. Maintenance preserves its full `sqlite_schema` unchanged, and only an explicit safe rebuild installs current v2.

## Deviations from Plan

### Approved Bounded Scope Adjustment

**1. Extended the private model and schema files required for a complete result boundary**

- **Found during:** Task 1 normalized persistence and active reconstruction
- **Issue:** The plan's initial file list did not include all private key, digest, commit-plan, connection, migration, schema, quarantine, and fixture call sites needed to add typed run-manifest/diagnostic identity without store-local hashing or partial schema support.
- **Fix:** With explicit approval, expanded only the private/internal implementation scope. Added canonical model-owned requested-view identity, strict normalized DDL and readers/writers, exhaustive private node handling, exact incomplete-v2 recovery, and the necessary internal fixture constructor update. No dependency, SDK, CLI, generated-skill, example, or public API surface was added.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/{digest,keys,mod,quarantine}.rs`, `crates/polint/src/analysis_kernel/store/{commit_plan,connection,migrations,schema}.rs`, `crates/polint/src/eval/performance.rs`
- **Verification:** Exact kernel/CLI gates, 33 store tests, 18 migration tests, strict workspace Clippy, public-surface scans, forbidden-payload scans, delivery-history scans, and the pre-commit lint hook all passed.
- **Committed in:** `fdf1d63a`

---

**Total deviations:** 1 approved private-scope adjustment
**Impact on plan:** The expansion was required to keep one canonical identity source and a truthful normalized schema. It remains entirely behind the existing private store boundary.

## Issues Encountered

- The first diagnostic uniqueness shape omitted requested-view membership, allowing two otherwise identical diagnostics to collide. Canonical requested-view set identity now participates in the normalized unique key, with both diagnostics attached to shared typed rule-code/options inputs and proven through active reconstruction.
- A first recovery fixture modeled only part of the incomplete v2 boundary. It was replaced with the exact earlier dependency-edge, statistics, and index shape, no result-boundary tables, and a complete `sqlite_schema` before/after comparison.
- Empty diagnostic vectors initially contributed JSON collection framing to diagnostic logical bytes. Row-wise accounting now reports zero diagnostic bytes for zero diagnostics and keeps the semantic total honest.
- Initial status coverage named destination states without mechanically consuming them. A transition helper now accepts persisted prior and next inputs, verifies identical kind/key/digest plus differing statuses, and creates the canonical prior-node change row.

## User Setup Required

None - the semantic store remains private and disabled by default, with no new configuration, CLI, SDK, or external service.

## Verification

- Exact required chain passed on the committed implementation: kernel semantic-store filter 5/5, exact all-state parity 1/1, persisted metadata matrix 5/5, full serial CLI 167/167 in 610.38 seconds, and `cargo check -p polint --all-features --locked`.
- Complete store suite passed 33/33 serially, including lifecycle, contention, active-reader, telemetry, status-transition, requested-view uniqueness, and controlled-rebuild coverage.
- Migration suite passed 18/18, and focused connection schema probes passed 2/2.
- `cargo fmt --all -- --check`, `git diff --check`, and `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed.
- The implementation commit's Conductor hook reran `make lint`, formatting, and strict workspace Clippy successfully.
- Source audits confirmed exact kernel ordering; no bare public additions; no parent references to commit-plan/SQL internals; no new delivery-history comments; no normalized JSON/blob/source-body fields; no store-local diagnostic identity hashing; and no private markers in help, output, or paths.

## Next Phase Readiness

- Ready for Plan 19 to close the phase with public-surface leak probes, real enabled-store performance gates, deterministic-identity checks, and full workspace verification.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-14*

## Self-Check: PASSED

Implementation commit `fdf1d63a` exists. The exact verification chain, complete store and migration suites, formatting, strict workspace Clippy, privacy/source/chronology audits, and commit hook all pass; the approved scope expansion remains private and the next plan has not been started.
