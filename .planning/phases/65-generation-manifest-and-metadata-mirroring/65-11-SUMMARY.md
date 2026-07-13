---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 11
subsystem: analysis-kernel
tags: [dependency-index, typed-inputs, invalidation, cache-wire, serde]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 10
    provides: Closed purpose-checked dependency-input vocabulary staged outside the wire
provides:
  - Typed-only dependency graph endpoints for module graph, symbol graph, metrics, lifecycle, provider, and upstream inputs
  - Canonical single-vector dependency-index persistence with derived forward and reverse traversal indexes
  - Exact temporary typed schema pin with fail-closed v1 and forged-v1 handling
affects: [phase-65-query-dependencies, phase-65-store-dependency-edges, phase-65-final-v2-schema]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Typed dependency endpoints carry exact kind, stable key, digest purpose, and availability status"
    - "One canonical edge vector is serialized; directional indexes are reconstructed after validation"
    - "Intermediate wire shapes receive unique schema labels and reject older labels without compatibility rewriting"

key-files:
  created:
    - .planning/phases/65-generation-manifest-and-metadata-mirroring/65-11-SUMMARY.md
  modified:
    - crates/polint/src/analysis_kernel/incremental/dependency_input.rs
    - crates/polint/src/analysis_kernel/incremental/change_set.rs
    - crates/polint/src/analysis_kernel/incremental/dependency_index.rs
    - crates/polint/src/analysis_kernel/incremental/invalidation.rs
    - crates/polint/src/analysis_kernel/incremental/layer_cache.rs
    - crates/polint/src/analysis_kernel/incremental/quarantine.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/metrics.rs
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/symbol_graph/mod.rs

key-decisions:
  - "Publish the first typed cache-node shape only under polint-dependency-index-next-typed, while preserving v1 solely as an explicit rejection fixture"
  - "Represent syntax, module, and manifest-relative dependencies as typed UpstreamLayer endpoints instead of synthetic dependency-only LayerKey nodes"
  - "Persist canonical edges only and derive forward/reverse indexes so serialized dependency truth cannot diverge"
  - "Require DigestKind::AnalysisSettings at the general LayerKey constructor boundary"

patterns-established:
  - "Wire validation: typed input deserialization and change rows both reject kind/digest-purpose disagreement"
  - "Invalidation boundary: typed input nodes drive their linked dependents and never become cache actions themselves"
  - "Manifest boundary: layer-cache readers and writers consume the central temporary schema constant and fail closed on every other label"

requirements-completed: [META-01, META-04]

# Metrics
duration: 48min
completed: 2026-07-13
---

# Phase 65 Plan 11: Typed Dependency Wire Migration Summary

**Every real dependency producer now emits purpose-checked typed endpoints into one canonical dependency edge wire, guarded by an exact temporary schema label and fail-closed legacy handling.**

## Performance

- **Duration:** 48 min
- **Started:** 2026-07-13T16:38:31Z
- **Completed:** 2026-07-13T17:26:42Z
- **Tasks:** 1
- **Files modified:** 10 implementation files

## Accomplishments

- Added serde to the full 19-kind dependency-input vocabulary and made typed-node decoding reject digest-purpose mismatches before an index can be accepted.
- Migrated module graph, module topology, symbol graph, and metrics producers to exact typed source, lifecycle, analysis-setting, provider, provider-schema, toolchain, package/project, and upstream-layer endpoints with explicit availability states.
- Replaced three legacy string-bearing cache-node variants with `DependencyInput`, changed the private wire pin atomically to `polint-dependency-index-next-typed`, and added explicit and forged v1 rejection coverage.
- Made the dependency index serialize one sorted canonical edge vector, reconstruct both directional maps, and prove identical results across all 24 tested provider/source insertion orders.
- Tightened change-set digest agreement, dependent-only invalidation, quarantine classification, the manifest-relative sentinel, and the general analysis-settings constructor boundary.

## Task Commits

Each task was committed atomically:

1. **Task 1: Migrate producers, remove legacy variants, and stage the typed wire shape** - `9806e7a2` (feat)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/dependency_input.rs` - Serde-visible typed keys with exact labels, purpose validation, and exhaustive kind/status coverage.
- `crates/polint/src/analysis_kernel/incremental/dependency_index.rs` - Typed-only cache nodes, exact codecs, temporary schema pin, canonical edge persistence, and permutation tests.
- `crates/polint/src/analysis_kernel/incremental/change_set.rs` - Exact change-kind codecs and typed endpoint/change-digest agreement validation.
- `crates/polint/src/analysis_kernel/incremental/invalidation.rs` - Dependent traversal for typed inputs and fail-closed mismatched changes or schema labels.
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` - Typed manifest sentinel plus exact schema read/write validation and forged-v1 rejection.
- `crates/polint/src/analysis_kernel/incremental/quarantine.rs` - Exhaustive native and extension-influenced classification for every typed endpoint.
- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Strict `AnalysisSettings` purpose validation for general layer keys.
- `crates/polint/src/metrics.rs` - Typed metrics dependency producer and direct mapping assertions.
- `crates/polint/src/module_graph/mod.rs` - Typed module graph/topology inputs, lifecycle states, provider identities, and upstream layers.
- `crates/polint/src/symbol_graph/mod.rs` - Typed symbol graph inputs, lifecycle states, provider identities, and upstream layers.

## Decisions Made

- The temporary typed/pre-query wire is accepted only under its exact current constant. The final v2 label remains deferred until `QueryKey` reaches its final shape.
- Input status and digest purpose come from existing `InputSnapshot` and layer identities; producers do not invent parallel hashes or collapse absent, unsupported, and setup-missing states.
- Lifecycle snapshot components using the tool-invocation digest purpose become `ToolInvocation` endpoints; language lifecycle files and settings retain `LanguageLifecycle` identity.
- Rule code and options stay diagnostic-only. Analysis-layer producers emit requested-capability, model, extension, or setting edges only when their provider declarations actually contain those inputs.

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- The plan's literal `symbol_graph::tests` filter selects zero tests in the current module layout. The required command still passed, and the real `symbol_graph::symbol_graph_derivation` suite was run separately with 10/10 passing.
- Strict Clippy identified redundant digest clones after producer migration; the values were moved directly and the repository hook then passed with warnings denied.

## User Setup Required

None - this is a private analysis-kernel and cache-wire migration.

## Verification

- `cargo test -p polint --lib analysis_kernel::incremental --locked`: 149 passed.
- Focused producer suites: module graph 34 passed, symbol graph derivation 10 passed, and metrics 30 passed.
- Focused wire suites: dependency index 7 passed, layer cache 14 passed, invalidation 8 passed; dependency-input, change-set, and quarantine coverage also passed within the incremental suite.
- `cargo test -p polint --test cli --locked -- --test-threads=1`: 166 passed in 667.57 seconds.
- `cargo check -p polint --all-features --locked`: passed.
- `scripts/conductor/git-hooks/pre-commit`: passed, including workspace/all-target/all-feature Clippy with `-D warnings`.
- Repository audit found no legacy string-bearing cache-node constructors, no `polint-dependency-index-2`, no CLI temporary-label pin, and no current-label duplication outside the central constant.
- Formatting and `git diff --check` passed; the task commit hook reran the full lint hook successfully.

## Next Phase Readiness

- Typed dependency edges are now the sole producer and consumer vocabulary, ready for the separate query-dependency input work.
- The intermediate label deliberately remains non-final; later plans must rotate it when the query wire changes and publish v2 only at the final coordinated boundary.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-13*

## Self-Check: PASSED

The task commit exists; all ten scoped implementation files and this summary exist; typed serde, exact codecs, deterministic canonical edges, producer mappings, v1 rejection, incremental and CLI compatibility, all-feature compilation, strict lint, repository hooks, and source/schema audits pass.
