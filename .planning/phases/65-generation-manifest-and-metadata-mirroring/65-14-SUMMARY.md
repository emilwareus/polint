---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 14
subsystem: analysis-kernel
tags: [query-identity, dependency-index, run-metadata, deterministic-digests, semantic-store]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 13
    provides: Complete structured validation events retained after authoritative fact validation
provides:
  - Mandatory sorted typed query dependency declarations with exact invalidation edges
  - Complete telemetry-free ValidatedRunMetadata for one finalized kernel run
  - Canonical workspace, config, family, run, and generation identities with integrity validation
affects: [phase-65-store-commit-plan, phase-65-dependency-schema-v2, phase-65-generation-persistence]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Declare query invalidation inputs on QueryKey and derive edges from that declaration only"
    - "Project semantic run metadata into owned canonical rows before store planning"
    - "Compose GenerationIdentity from seven sorted telemetry-free semantic family digests"

key-files:
  created:
    - .planning/phases/65-generation-manifest-and-metadata-mirroring/65-14-SUMMARY.md
  modified:
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/incremental/demand.rs
    - crates/polint/src/analysis_kernel/incremental/dependency_index.rs
    - crates/polint/src/analysis_kernel/incremental/quarantine.rs
    - crates/polint/src/analysis_kernel/incremental/mod.rs
    - crates/polint/src/analysis_kernel/incremental/run_report.rs
    - crates/polint/src/analysis/demand/query.rs
    - crates/polint/src/analysis/summaries/closure.rs
    - crates/polint/src/analysis/summaries/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "QueryKey owns only requested capability, analysis setting, model, extension code, and extension-declared inputs that the query actually reads"
  - "The SCC closure query declares filtered capability state and direct-summary settings directly while upstream provider effects remain layer digests"
  - "RunIdentity covers workspace, full config, input snapshot, and provider manifests; GenerationIdentity adds provider, layer, summary, query, fact, dependency, and validation families"
  - "ValidatedRunMetadata retains InputSnapshot v2 but excludes cache outcomes, counters, durations, timestamps, and mtime hints from all semantic identities"

patterns-established:
  - "Exact query frontier: parameter, declared typed inputs, upstream layers, budget, and precision are the complete query dependency edge set"
  - "Store handoff privacy: private owned rows expose borrowed analysis-kernel-only accessors and no connection, SQL, payload, or raw relational identity"
  - "Canonical integrity: every family is sorted and unique, provider/schema references agree, required validation events attest their inputs, and identities are recomputed before handoff"

requirements-completed: [STORE-04, META-01, META-04]

# Metrics
duration: 41min
completed: 2026-07-13
---

# Phase 65 Plan 14: Exact Query Dependencies and Validated Run Metadata Summary

**Queries now declare their exact typed invalidation frontier, and one deterministic telemetry-free validated-run object carries every semantic family needed for store-only planning.**

## Performance

- **Duration:** 41 min
- **Started:** 2026-07-13T18:46:18Z
- **Completed:** 2026-07-13T19:26:52Z
- **Tasks:** 2
- **Files modified:** 10 implementation files

## Accomplishments

- Added mandatory `QueryDependencyInputs` to `QueryKey`, migrated every constructor and fixture, and made the real SCC closure query declare only the requested capabilities and direct-summary setting it reads.
- Rotated the temporary dependency-index wire label to `polint-dependency-index-next-query-inputs`, rejected v1 and the superseded temporary shape, and derived query edges exactly from declared inputs plus parameter, layer, budget, and precision identity.
- Added `ValidatedRunMetadata` with owned canonical provider manifest/output/layer/query/fact/dependency/validation families, an explicit empty summary-key slot, borrowed sibling-store accessors, and one canonical dependency index.
- Added `CanonicalRunIdentities` with the complete workspace/config/input/manifest/run identity boundary and seven telemetry-free generation family digests.
- Added fail-closed integrity validation for schemas, canonical order and uniqueness, provider/schema/output relationships, absolute paths, validation completeness and digest consistency, dependency assembly, and identity recomputation.
- Proved 24 construction permutations converge, query dependencies are exact, payload digests survive, forbidden bodies and raw IDs are absent, and cache/mtime/status/duration changes preserve every semantic identity.

## Task Commits

Each task was committed atomically:

1. **Task 1: Make QueryDependencyInputs mandatory and migrate every producer/fixture** - `c16632dd` (feat)
2. **Task 2: Assemble the complete validated-run handoff and semantic identities** - `4ca98ef1` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Mandatory canonical `QueryDependencyInputs` ownership and `QueryKey` identity.
- `crates/polint/src/analysis_kernel/incremental/demand.rs` - Telemetry-free semantic query projection plus the explicit dependency-free test factory migration.
- `crates/polint/src/analysis_kernel/incremental/dependency_index.rs` - Exact query dependency edges, temporary schema rotation, and fail-closed stale-label coverage.
- `crates/polint/src/analysis_kernel/incremental/quarantine.rs` - Explicit typed dependency fixture migration.
- `crates/polint/src/analysis_kernel/incremental/mod.rs` - Curated internal query and validated-run vocabulary re-exports.
- `crates/polint/src/analysis_kernel/incremental/run_report.rs` - Complete canonical validated-run handoff, identities, integrity validation, and deterministic regression coverage.
- `crates/polint/src/analysis/demand/query.rs` - Mandatory dependency input in the shared demand key builder.
- `crates/polint/src/analysis/summaries/closure.rs` - Exact real SCC query declarations and stable budget identity.
- `crates/polint/src/analysis/summaries/provider.rs` - Input snapshot and provider inputs supplied to the SCC producer.
- `crates/polint/src/analysis_kernel/mod.rs` - Provider input propagation and real post-finalization handoff coverage.

## Decisions Made

- `QueryDependencyInputs` is a closed collection of the five query-relevant typed input families. It has no production default or empty compatibility constructor; only fixtures that explicitly read nothing use an empty collection.
- The SCC query records filtered `calls`, `control_flow`, and `dataflow` capability rows plus the `polint.direct_summaries` analysis setting. It does not claim model or extension inputs it does not read, while direct-summary and calls outputs remain upstream layer digests.
- Query dependency assembly trusts the `QueryKey` declaration rather than rescanning `InputSnapshot`. An unreferenced sibling input therefore preserves the key, edge set, and reuse action.
- `RunIdentity` uses the existing workspace/full-config/input-snapshot/provider-manifest purposes. `GenerationIdentity` adds the existing provider-output, layer, summary, query, fact-metadata, dependency, and validation-event purposes in sorted order.
- `ValidatedRunMetadata` owns the complete input snapshot for later first-class mirroring, while the input identity uses `InputSnapshot::semantic_digest`; mtime hints and rendered details remain outside identity.
- Provider cache statistics and query cache status/duration never enter the canonical output/query rows. Generation telemetry remains outside the handoff rather than being normalized into semantic fields.
- Layer-manifest dependency sources are expanded to their actual `LayerKey` before joining exact query edges in the handoff's single canonical `DependencyIndex`.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The final constructor ownership audit initially found a direct `QueryKey::new` in the new run-report fixture and a regex-visible accessor return type. The fixture now uses the existing explicit dependency-free test factory and assigns its declared typed inputs directly; an internal transparent alias keeps the literal constructor audit exact. The final audit reports only the six authorized constructor files.

## User Setup Required

None - this is a private analysis-kernel and store-planning boundary with no CLI, configuration, SDK, or generated-skill change.

## Verification

- All 164 `analysis_kernel::incremental` tests passed.
- All 8 demand-query tests and all 11 SCC-closure tests passed.
- All 12 validated-run report tests passed, including 24 permutations, telemetry invariance, exact query edges, payload retention, forbidden-field absence, and integrity rejection.
- All 3 semantic-store kernel/parity tests passed, including construction from a real post-validation finalized run.
- `cargo check -p polint --all-features --locked` passed without warnings.
- `cargo fmt --all -- --check` passed.
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` passed both directly and in the repository pre-commit hook.
- Repository-wide constructor audit reports exactly `keys.rs`, `demand.rs`, `dependency_index.rs`, `quarantine.rs`, `analysis/demand/query.rs`, and `analysis/summaries/closure.rs`; debug and eval contain zero constructors.
- `DEPENDENCY_INDEX_SCHEMA` is exactly `polint-dependency-index-next-query-inputs`; v1 and `polint-dependency-index-next-typed` are rejected, and no final v2 label is published.
- Shipped-code chronology and exact forbidden content/body/blob/path/SQL/raw-ID audits returned zero findings; `git diff --check` passed.

## Next Phase Readiness

- Plan 15 can publish the final dependency-index v2 label and map this complete handoff into one deterministic SQL-free `StoreCommitPlan` without scanning roots, configuration, registries, caches, or provider state.
- The owned families and borrowed accessors expose every STORE-04/META-01 source needed for normalized store rows, and the integrity validator provides the required rejection precondition for planning.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-13*

## Self-Check: PASSED

Both task commits and this summary exist; the final constructor file set and temporary schema label are exact; every required semantic family, validation event, dependency edge, and identity is present and canonical; payload digests remain while forbidden bodies and raw IDs are absent; all focused, incremental, semantic-store, compilation, formatting, strict-lint, and hook gates pass.
