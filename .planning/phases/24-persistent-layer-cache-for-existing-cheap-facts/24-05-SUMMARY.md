---
phase: 24-persistent-layer-cache-for-existing-cheap-facts
plan: 05
subsystem: analysis-kernel-cache
tags: [rust, layer-cache, eval, cli, stale-safety, public-api]

requires:
  - phase: 24-persistent-layer-cache-for-existing-cheap-facts
    provides: "Plans 24-01 through 24-04 added the cache vocabulary and persisted syntax, module graph, symbol graph, and metrics layers."
provides:
  - "Real-provider native eval coverage for cold, warm, disabled, and import-edited layer-cache runs"
  - "LayerCacheStore stale-entry, digest, dependency-index, traversal, and symlink-escape hardening"
  - "CLI regression coverage proving layer-cache internals stay out of public JSON, help, SDK, runner, and crate-root surfaces"
affects: [analysis-kernel, eval, cli-tests, cache-layout, layer-cache]

tech-stack:
  added: []
  patterns:
    - "Native eval fixtures can request an explicit AnalysisPlan to exercise derived providers through real KernelRunReport output."
    - "Layer-cache manifests are validated before payload reads and never provide filesystem paths."
    - "Public-boundary CLI tests use temp external rule packs importing only polint::sdk::prelude::* and polint::runner::run_cli."

key-files:
  created:
    - tests/eval-fixtures/cache/layer-cache/expected.polint-eval.toml
    - tests/eval-fixtures/cache/layer-cache/repo/.polint.toml
    - tests/eval-fixtures/cache/layer-cache/repo/goapp/go.mod
    - tests/eval-fixtures/cache/layer-cache/repo/goapp/payment.go
    - tests/eval-fixtures/cache/layer-cache/repo/web/package.json
    - tests/eval-fixtures/cache/layer-cache/repo/web/tsconfig.json
    - tests/eval-fixtures/cache/layer-cache/repo/web/src/app.ts
  modified:
    - crates/polint/src/analysis_kernel/incremental/layer_cache.rs
    - crates/polint/src/eval/observed.rs
    - crates/polint/src/eval/fixtures.rs
    - crates/polint/tests/cli.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/analysis_kernel/incremental/dependency_index.rs
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/metrics.rs
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/symbol_graph/mod.rs

key-decisions:
  - "Layer-cache eval uses an explicit capability-requesting AnalysisPlan so Go syntax, TS/JS syntax, module graph, symbol graph, and metrics providers all run through real cache paths."
  - "LayerCacheStore rejects invalid manifests before payload reads, including dependency-index schema drift and derived-layer manifests without dependency rows."
  - "Layer-cache internals remain test/eval-facing only; public JSON, CLI help, SDK, runner, and crate-root surfaces are guarded by integration tests."
  - "The public cache status contract includes the managed layers category but still does not expose layer-cache internals or provider stats."

patterns-established:
  - "Eval layer-cache observations are emitted as deterministic layer_cache.provider.* and layer_cache.aggregate.* rows from KernelRunReport."
  - "Derived layer cache reads validate manifest schema, key identity, digest shape, validation label, dependency-index schema, and dependency-row presence before reuse."
  - "External rule public-boundary tests should assert public SDK imports and rule macro usage instead of internal capabilities or manual Rule implementations."

requirements-completed: [SAE-FND-05]

duration: 28min
completed: 2026-05-18
---

# Phase 24 Plan 05: Layer Cache Proof Summary

**Real-provider layer-cache proof with stale-entry hardening and public no-leak regression coverage**

## Performance

- **Duration:** 28 min
- **Started:** 2026-05-18T11:52:36Z
- **Completed:** 2026-05-18T12:20:34Z
- **Tasks:** 3
- **Files modified:** 17

## Accomplishments

- Added a mixed Go and TypeScript native eval fixture that proves cold misses/writes, warm hits/verified reuse, disabled-cache bypasses, and import-edit invalidation across all Phase 24 layers.
- Hardened `LayerCacheStore` against stale, corrupt, wrong-schema, wrong-digest, unsupported-validation, missing-dependency, traversal, and symlink-escape cache entries.
- Added CLI public-boundary coverage proving layer-cache internals and stats do not leak through `polint check --format json`, CLI help, SDK, runner, or crate-root exports.
- Brought the full Phase 24 cache code through workspace tests, clippy with `-D warnings`, and format verification.

## Task Commits

Each task was committed atomically:

1. **Task 1 RED: Add failing layer cache eval fixture** - `a1821c4` (test)
2. **Task 1 GREEN: Implement layer cache eval fixture** - `83cbef8` (feat)
3. **Task 2 RED: Add failing layer cache stale safety tests** - `9a27cb5` (test)
4. **Task 2 GREEN: Harden layer cache stale reads** - `d0178a0` (fix)
5. **Task 3 RED: Add failing layer cache public boundary test** - `79fa2bf` (test)
6. **Task 3 GREEN: Prove layer cache internals stay private** - `79e188a` (test)
7. **Verification auto-fix: Update cache CLI expectations for layers** - `9e1c61f` (fix)
8. **Verification auto-fix: Satisfy layer cache clippy verification** - `da4d09a` (fix)

## Files Created/Modified

- `crates/polint/src/eval/observed.rs` - Emits deterministic provider and aggregate layer-cache invariant rows from crate-private `KernelRunReport`.
- `crates/polint/src/eval/fixtures.rs` - Adds the layer-cache native fixture runner, pass labeling, disabled-cache checks, and category coverage.
- `tests/eval-fixtures/cache/layer-cache/expected.polint-eval.toml` - Defines expected real-provider cold/warm/import-edit/disabled-cache observations.
- `tests/eval-fixtures/cache/layer-cache/repo/**` - Provides the mixed Go and TS fixture repository.
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` - Validates manifest schema, dependency-index schema, digests, labels, managed paths, symlinks, and derived dependency rows.
- `crates/polint/tests/cli.rs` - Adds public no-leak coverage and updates cache CLI expectations for the public `layers` category.
- `crates/polint/src/analysis_kernel/mod.rs`, `crates/polint/src/analysis_kernel/incremental/dependency_index.rs`, `crates/polint/src/analysis_kernel/incremental/keys.rs`, `crates/polint/src/metrics.rs`, `crates/polint/src/module_graph/mod.rs`, `crates/polint/src/symbol_graph/mod.rs` - Mechanical clippy cleanup required by full-plan verification.

## Decisions Made

- Layer-cache eval coverage requests specific capabilities through `AnalysisPlan::from_capability_names_for_test` rather than depending on an empty/default plan, because derived providers only run when capabilities require their facts.
- Derived cache manifests must contain dependency rows and the current dependency-index schema before reuse; missing or stale dependency metadata is treated as invalid and recomputed.
- Public cache status may list `layers` as a managed cache category, but public output still excludes provider-output stats, layer-cache manifests, dependency indexes, run reports, and internal marker names.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated cache CLI expectations for managed layers**
- **Found during:** Full workspace verification after Task 3
- **Issue:** `cache_status_reports_structured_cache_layout` still expected three managed categories, and `capability_change_changes_cache_entries` switched between two syntax-level fact views that no longer produced a new derived layer entry.
- **Fix:** Expected the public `layers` cache category and changed the capability-change test to request `ModuleGraphFacts<'_>` so the layer cache produces a new derived entry.
- **Files modified:** `crates/polint/tests/cli.rs`
- **Verification:** `cargo test -p polint --test cli cache_status_reports_structured_cache_layout --locked`, `cargo test -p polint --test cli capability_change_changes_cache_entries --locked`, and full workspace tests passed.
- **Committed in:** `9e1c61f`

**2. [Rule 3 - Blocking] Fixed clippy warnings in Phase 24 cache/provider code**
- **Found during:** Full clippy verification
- **Issue:** `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` rejected redundant clones, obsolete lint expectations, one `sort_by` form, and test-only clone patterns.
- **Fix:** Removed redundant digest clones, used `sort_by_key`, removed unfulfilled `too_many_arguments` expectations, kept the still-valid symbol-graph expectation, and cleaned dependency-index test helpers.
- **Files modified:** `crates/polint/src/analysis_kernel/mod.rs`, `crates/polint/src/analysis_kernel/incremental/dependency_index.rs`, `crates/polint/src/analysis_kernel/incremental/keys.rs`, `crates/polint/src/metrics.rs`, `crates/polint/src/module_graph/mod.rs`, `crates/polint/src/symbol_graph/mod.rs`
- **Verification:** `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, `cargo test -p polint --lib eval --locked`, `cargo test --workspace --all-features --locked`, and `cargo fmt --all -- --check` passed.
- **Committed in:** `da4d09a`

---

**Total deviations:** 2 auto-fixed (2 blocking verification issues)
**Impact on plan:** Both fixes were required to satisfy the plan's full verification gate. No public API expansion or architectural change was introduced.

## Issues Encountered

- The first full workspace test run exposed stale CLI cache expectations after the `layers` managed category became part of the public cache status output; fixed in `9e1c61f`.
- The first clippy run exposed warning-level cleanup in Phase 24 cache/provider code; fixed in `da4d09a`.

## Verification

- `cargo test -p polint --lib eval_layer_cache_fixture_passes --locked` - passed
- `cargo test -p polint --lib eval_native_fixture_suite_covers_required_categories --locked` - passed
- `cargo test -p polint --lib layer_cache --locked` - passed
- `cargo test -p polint --lib stale --locked` - passed
- `cargo test -p polint --test cli layer_cache_internals_stay_private --locked` - passed
- `cargo test -p polint --test cli input_snapshots_stay_internal --locked` - passed
- `cargo test -p polint --lib eval --locked` - passed
- `cargo test --workspace --all-features --locked` - passed
- `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` - passed
- `cargo fmt --all -- --check` - passed

## Known Stubs

None. Stub scan found no plan-introduced placeholders; matches were existing CLI fixture literals or intentional empty TOML/config arrays.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

Phase 24 now has end-to-end proof for existing cheap-fact layer persistence, stale-safety, and public-boundary constraints. Follow-on work can build new cached layers against the same manifest validation, dependency-index, and public no-leak patterns.

## Self-Check: PASSED

- Created summary and fixture files exist.
- Task and verification auto-fix commits exist: `a1821c4`, `83cbef8`, `9a27cb5`, `d0178a0`, `79fa2bf`, `79e188a`, `9e1c61f`, `da4d09a`.

---
*Phase: 24-persistent-layer-cache-for-existing-cheap-facts*
*Completed: 2026-05-18*
