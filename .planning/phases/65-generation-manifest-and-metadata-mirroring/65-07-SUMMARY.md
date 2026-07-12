---
phase: 65-generation-manifest-and-metadata-mirroring
plan: 07
subsystem: analysis-kernel
tags: [layer-cache, analysis-settings, syntax, invalidation, determinism]

# Dependency graph
requires:
  - phase: 65-generation-manifest-and-metadata-mirroring
    plan: 06
    provides: Typed full-config and provider-scoped analysis-setting rows in InputSnapshot v2
provides:
  - Purpose-checked analysis-settings identity for syntax, graph, metrics, topology, and extension LayerKeys
  - Go and TS syntax layer/file-cache identity independent of full config, rule, and plan digests
  - Production cache mutation coverage for rule-only preservation and provider-setting invalidation
affects: [phase-65-semantic-layer-identities, phase-65-dependency-vocabulary, phase-65-store-commit-plan]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Provider caches consume only their declared analysis-settings projection while durable snapshots retain full manifest identity"
    - "Purpose-specific LayerKey constructors reject full-config digests at the identity boundary"

key-files:
  created: []
  modified:
    - crates/polint/src/analysis_kernel/incremental/keys.rs
    - crates/polint/src/analysis_kernel/incremental/input_snapshot.rs
    - crates/polint/src/analysis_kernel/mod.rs
    - crates/polint/src/go/adapter.rs
    - crates/polint/src/ts/adapter.rs
    - crates/polint/src/module_graph/mod.rs
    - crates/polint/src/symbol_graph/mod.rs
    - crates/polint/src/metrics.rs
    - crates/polint/src/analysis/extensions/cache_key.rs

key-decisions:
  - "Removed the unused file-cache-to-LayerKey bridge instead of preserving a path that could import full config, rule, or plan identity"
  - "Retained compatibility rule and plan parameters only at existing private/bench seams, explicitly marking them non-identity"
  - "Kept full config, rule, and plan identities in InputSnapshot and run metadata while production syntax calls consume scoped GoSyntax/TsSyntax digests"

patterns-established:
  - "Scoped identity: the same provider settings digest feeds both a structural LayerKey and its dependency metadata"
  - "Mutation matrices: prove owned setting misses, unowned sibling hits, and rule-only layer plus file-cache hits"

requirements-completed: [META-01, META-04]

# Metrics
duration: 1h 4min
completed: 2026-07-12
---

# Phase 65 Plan 07: Provider-Scoped Layer and Syntax Identity Summary

**Structural LayerKeys and Go/TS syntax caches now depend on declared provider settings instead of opaque full-config, rule, or plan identity.**

## Performance

- **Duration:** 1h 4min
- **Started:** 2026-07-12T21:27:00Z
- **Completed:** 2026-07-12T22:31:02Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments

- Renamed the LayerKey identity slot to `analysis_settings_digest`, added purpose-checked constructors for the migrated structural families, removed the unused legacy file-cache bridge, and enumerated the intentionally deferred semantic/general seams.
- Routed module topology, module graph, symbol graph, metrics, and extension keys through provider-scoped settings; key construction and dependency metadata now consume the same scoped digest.
- Routed production Go and TS syntax analysis through `GoSyntax`/`TsSyntax` snapshot settings and removed rule/full-plan identity from both syntax layers and legacy per-file fact keys.
- Added real cache mutation matrices covering severity, files, allow-files, max, custom settings, unrelated solver settings, relevant parser settings, and direct file-cache hits.

## Task Commits

1. **Task 1: Migrate LayerKey and graph/metric producers to scoped settings** - `803da530` (feat)
2. **Task 2: Preserve syntax hits across rule-only changes** - `d612d81b` (fix)

## Files Created/Modified

- `crates/polint/src/analysis_kernel/incremental/keys.rs` - Purpose-specific settings constructors, renamed identity slot, removed bridge, and bounded remaining seams.
- `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs` - Crate-private typed accessor for complete provider settings rows.
- `crates/polint/src/analysis_kernel/incremental/invalidation.rs` - Renamed LayerKey settings comparison.
- `crates/polint/src/analysis_kernel/incremental/layer_cache.rs` - Renamed settings validation and invalidation reason construction.
- `crates/polint/src/analysis_kernel/mod.rs` - Production Go/TS calls consume snapshot-scoped syntax settings while full identities remain in snapshot/run metadata.
- `crates/polint/src/go/adapter.rs` - Scoped syntax LayerKey and file CacheKey construction with rule/plan identity excluded.
- `crates/polint/src/go/tests.rs` - Go rule-only preservation, owned-setting invalidation, sibling preservation, and file-cache hit coverage.
- `crates/polint/src/ts/adapter.rs` - Scoped syntax LayerKey and file CacheKey construction with rule/plan identity excluded.
- `crates/polint/src/ts/tests.rs` - TS rule-only preservation, owned-setting invalidation, sibling preservation, and file-cache hit coverage.
- `crates/polint/src/module_graph/mod.rs` - Module topology/graph scoped settings in keys and dependency metadata.
- `crates/polint/src/symbol_graph/mod.rs` - Symbol graph scoped settings in keys and dependency metadata.
- `crates/polint/src/metrics.rs` - Metrics scoped settings in keys and dependency metadata.
- `crates/polint/src/analysis/extensions/cache_key.rs` - Extension settings projection and raw RuleOptions purpose rejection.

## Decisions Made

- Removed `LayerKey::from_existing_file_cache` because it had no production caller and necessarily carried broader `CacheKey` identity than a structural provider declares.
- Used a typed `InputSnapshot::analysis_settings_digest(scope)` accessor that fails closed on incomplete or wrong-purpose rows instead of exposing callers to ad hoc row searches.
- Preserved the externally compiled bench helper signature, but renamed its identity argument and made the compatibility rule parameter explicitly non-identity; internal plan-aware seams likewise retain a plan parameter without hashing it.
- Left semantic MIR/CFG/calls/domain/direct-summary and general-constructor migrations explicit for Plans 08-11 rather than silently treating full config as provider settings.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added the scoped settings accessor to InputSnapshot**

- **Found during:** Task 1 production producer migration.
- **Issue:** Producers needed a purpose-validating lookup for the complete provider settings rows introduced by Plan 06, but `input_snapshot.rs` was not listed in this plan's frontmatter.
- **Fix:** Added one crate-private accessor that requires a complete row and verifies `DigestKind::AnalysisSettings`; no public surface changed.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/input_snapshot.rs`
- **Verification:** All-feature compilation, strict Clippy, focused key/producer suites, and the public-surface probe pass.
- **Committed in:** `803da530`

**2. [Rule 3 - Blocking] Updated LayerKey comparison consumers after the field rename**

- **Found during:** Task 1 LayerKey identity rename.
- **Issue:** Cache validation and invalidation directly compare the renamed field, so leaving those unlisted files unchanged made the migration fail to compile.
- **Fix:** Mechanically renamed the two crate-private comparisons and their settings-specific invalidation reason; behavior and visibility remain unchanged.
- **Files modified:** `crates/polint/src/analysis_kernel/incremental/invalidation.rs`, `crates/polint/src/analysis_kernel/incremental/layer_cache.rs`
- **Verification:** Focused producer suites, all-feature compilation, strict workspace Clippy, and public-surface probes pass.
- **Committed in:** `803da530`

---

**Total deviations:** 2 auto-fixed (2 blocking implementation dependencies)
**Impact on plan:** Scope expanded only to the three mechanically required private files. No public API, CLI, rule-author surface, or persistence behavior widened.

## Issues Encountered

- The plan's exact `symbol_graph::tests` filter selects zero tests because symbol coverage is split across named submodules. The broader `symbol_graph` filter passed 99 concrete tests.
- The first Task 2 commit attempt was correctly stopped by strict Clippy for two mirrored redundant clones. Both were removed and the unchanged hook passed on retry.

## User Setup Required

None - cache identity changes are internal and require no repository configuration or external service.

## Verification

- LayerKey constructors and bounded-seam source audit: 27 passed.
- Module graph/topology suite: 34 passed; concrete symbol graph suite: 99 passed; metrics suite: 29 passed.
- Go-focused suite: 75 passed; TS-focused suite: 186 passed; extension cache-key suite: 3 passed.
- Mutation coverage proves five rule-only config changes preserve both syntax-layer and direct per-file cache hits, unrelated solver settings preserve hits, and owned parser settings miss for each language.
- `cargo check -p polint --all-features --locked`: passed.
- `cargo test -p polint --test public_surface_leak --locked`: 7 passed.
- `cargo fmt --all -- --check`: passed.
- Both task commit hooks passed `make lint`, including strict workspace/all-target/all-feature Clippy with warnings denied.
- Acceptance audit finds no production `input_snapshot.config.digest`, `from_existing_file_cache`, adapter `config_hash`, adapter `plan.digest()`, or syntax `DigestKind::Config`; full config/rule/plan identities remain in snapshot and run metadata.

## Next Phase Readiness

- Semantic provider migrations in Plans 08-09 can reuse the purpose-checked constructor and snapshot accessor pattern.
- Dependency-vocabulary and general-constructor cleanup in Plans 10-11 has an explicit source-tested seam inventory.
- No blockers.

---
*Phase: 65-generation-manifest-and-metadata-mirroring*
*Completed: 2026-07-12*

## Self-Check: PASSED

All thirteen implementation files and this summary exist; commits `803da530` and `d612d81b` are present; every planned focused suite, concrete symbol suite, mutation matrix, all-feature check, formatting gate, strict Clippy hook, acceptance audit, and public-surface probe listed above passes.
