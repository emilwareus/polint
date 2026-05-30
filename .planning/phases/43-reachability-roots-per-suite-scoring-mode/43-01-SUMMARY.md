---
phase: 43-reachability-roots-per-suite-scoring-mode
plan: 01
subsystem: api
tags: [reachability, roots, static-analysis, provider, call-graph, kernel, polint]

# Dependency graph
requires:
  - phase: 35-framework-entrypoints-and-trust-boundaries
    provides: EntrypointFact substrate (kind/precision/status vocabulary) bridged into roots
  - phase: 42-identity-substrate
    provides: identity provider five-phase pipeline + output-digest + sort-then-assign-dense-IDs determinism pattern
  - phase: 30-direct-call-facts
    provides: direct-call edge set CallReachabilityFact marks will consume in Plan 02
provides:
  - "analysis::reachability private module: whole-program reachability-from-roots (provider polint.reachability)"
  - "ReachabilityRootFact + closed RootKind/RootStatus/RootPrecision/RootProvenance enums + ReachabilityRootId newtype"
  - "discover_reachability_roots: Go main/init, Go/TS exported, Test/FrameworkEntrypoint bridge, configured .polint.toml roots"
  - "polint.reachability provider slotted immediately after polint.entrypoints with PrecisionCeiling::SetupAware"
  - "[reachability] roots config section on PolintConfig"
  - "CallReachabilityFact marks shape (populated by Plan 02 marking traversal)"
affects: [44-marking-traversal, 45-per-suite-scoring-mode, 46-determinism-gate, reachability, scoring]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Reachability roots derived from existing facts only (no new parsing) mirroring identity extract pattern"
    - "Loss-free entrypoint->root bridge inheriting precision/status with originating_entrypoint composition"
    - "Configured-unresolvable roots become RootStatus::Unresolved (honest, never silent drop)"

key-files:
  created:
    - crates/polint/src/analysis/reachability/mod.rs
    - crates/polint/src/analysis/reachability/facts.rs
    - crates/polint/src/analysis/reachability/discover.rs
    - crates/polint/src/analysis/reachability/store.rs
    - crates/polint/src/analysis/reachability/validate.rs
    - crates/polint/src/analysis/reachability/cache_key.rs
    - crates/polint/src/analysis/reachability/provider.rs
    - crates/polint/src/analysis/reachability/debug.rs
  modified:
    - crates/polint/src/analysis/ids.rs
    - crates/polint/src/analysis/mod.rs
    - crates/polint/src/config/mod.rs
    - crates/polint/src/core/mod.rs
    - crates/polint/src/analysis_kernel/provider.rs
    - crates/polint/src/analysis_kernel/mod.rs

key-decisions:
  - "RootKind/status/precision/provenance use pinned declaration order + serde rename for byte-stability; no #[repr(u8)] (43-PATTERNS D-04 correction matching the established EntrypointKind convention)"
  - "Configured root [reachability] section lives in crates/polint/src/config/mod.rs (no polint-config crate exists); configured roots passed to discovery as &[String] from LoadedConfig since InputSnapshot only carries digests"
  - "Module list trimmed to files this plan creates (no traverse.rs; that is a Plan 02 deliverable) to keep the build compiling"
  - "Configured-unresolvable roots carry a sentinel FunctionId; the provider filters them out of the referentially-validated store while discovery still reports them"

patterns-established:
  - "Pattern: whole-program reachability module carries a mandatory D-02 doc comment distinguishing it from the block-level polint.domain.reachability abstract domain"
  - "Pattern: provider output digest folds in every upstream provider output digest + config + per-root stable payloads, with empty-output sentinel"

requirements-completed: [REACH-01]

# Metrics
duration: 90min
completed: 2026-05-29
---

# Phase 43 Plan 01: Reachability Roots & Provider Summary

**Private whole-program `analysis::reachability` module with typed root facts, root discovery from existing Go/TS facts + the entrypoint bridge + configured `.polint.toml` roots, and the `polint.reachability` provider slotted immediately after `polint.entrypoints` with a SetupAware precision ceiling.**

## Performance

- **Duration:** ~90 min
- **Started:** 2026-05-29T13:30Z (approx)
- **Completed:** 2026-05-29T15:30Z
- **Tasks:** 4
- **Files modified/created:** 14 source files (+ 4 eval-fixture goldens)

## Accomplishments
- Created the private `analysis::reachability` module (8 submodules) with the mandatory D-02 whole-program vs block-level distinction doc comment.
- `ReachabilityRootFact` (13 D-03 fields composing v1.2 IDs by reference) + closed `RootKind` (6 pinned variants) + `RootStatus`/`RootPrecision`/`RootProvenance` mirroring `EntrypointStatus`/`EntrypointPrecision` loss-lessly.
- `discover_reachability_roots` projects existing facts only (zero parsing): Go `main`/`init`, Go/TS `Exported`, the `Test`/`FrameworkEntrypoint` entrypoint bridge (inheriting precision/status, carrying `originating_entrypoint`), and configured `.polint.toml` roots (unresolvable → `RootStatus::Unresolved`, never dropped).
- `polint.reachability` provider runs the identity-style five-phase pipeline and is spliced into `PROVIDER_MANIFESTS` and the kernel execution order **immediately after** `polint.entrypoints` with `PrecisionCeiling::SetupAware`.
- Public-surface-leak gate stays green; `ALLOWED_PRELUDE` byte-unchanged; every new item is `pub(crate)`.

## Task Commits

1. **Task 1: Reachability fact shape, IDs, and config input** - `2292e6a` (feat)
2. **Task 2: Root discovery, store, validation, and cache key** - `13686e5` (feat)
3. **Task 3: Provider, output digest, and kernel manifest/order splice** - `b273b7d` (feat)
4. **Task 4: Public-surface-leak gate stays green** - `bbc0e4c` (test, empty verification commit)

_TDD note: Tasks 1-3 are co-located test+implementation units; the typed facts must exist for the byte-stability tests to compile, so each landed as a single `feat` commit with its tests rather than separate RED/GREEN commits._

## Files Created/Modified
- `crates/polint/src/analysis/reachability/mod.rs` - module root + D-02 distinction doc comment
- `crates/polint/src/analysis/reachability/facts.rs` - ReachabilityRootFact, closed enums, stable-key recipe, CallReachabilityFact marks
- `crates/polint/src/analysis/reachability/discover.rs` - root discovery from existing facts (Go main/init, exported, entrypoint bridge, configured)
- `crates/polint/src/analysis/reachability/store.rs` - ReachabilityProviderOutput + ReachabilityStore (total-order normalize + dangling-ref rejection)
- `crates/polint/src/analysis/reachability/validate.rs` - validate_reachability + reject_exact_precision ceiling
- `crates/polint/src/analysis/reachability/cache_key.rs` - schema label + frozen parameter digest + trip-wire
- `crates/polint/src/analysis/reachability/provider.rs` - derive_reachability_with_cache_stats + output digest + for_test helper
- `crates/polint/src/analysis/reachability/debug.rs` - cfg(test) root/mark debug renderer
- `crates/polint/src/analysis/ids.rs` - ReachabilityRootId newtype + assert_small_id_contract roster
- `crates/polint/src/analysis/mod.rs` - register reachability module
- `crates/polint/src/config/mod.rs` - [reachability] roots config section
- `crates/polint/src/core/mod.rs` - reachability storage fields + replace_reachability_facts/accessors
- `crates/polint/src/analysis_kernel/provider.rs` - REACHABILITY_SCHEMA + manifest after entrypoints + order tests/golden
- `crates/polint/src/analysis_kernel/mod.rs` - wire reachability provider after entrypoints + expected-order test lists

## Decisions Made
- **No `#[repr(u8)]` on reachability enums.** Followed the 43-PATTERNS.md D-04 correction: byte-stability comes from pinned declaration order + derived `Ord` + serde rename, matching the existing `EntrypointKind`/`IdentityCategory` convention. The literal D-04 wording was not followed where it conflicted with the observed codebase pattern.
- **Configured roots passed as `&[String]` to discovery.** `InputSnapshot` carries only digest components, not the structured config, so the kernel reads `input.loaded.config.reachability.roots` and passes them down; the config still rides on `input_snapshot.config.digest` for cache identity (D-19).
- **Configured-unresolvable roots use a sentinel `FunctionId`.** Discovery emits them as honest `RootStatus::Unresolved`/`RootProvenance::Configured` rows; the provider filters them out of the referentially-validated store (their target is by definition not a real function) while keeping them reportable.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Trimmed module list to files this plan creates**
- **Found during:** Task 1 (module root creation)
- **Issue:** The plan's `<action>` listed `traverse` in the `pub(crate) mod {...}` declaration, but `traverse.rs` is a Plan 02 deliverable (43-PATTERNS.md "No Analog Found"). Declaring a nonexistent module fails the build.
- **Fix:** Declared only the modules each task actually creates (facts in Task 1; discover/store/validate/cache_key/debug in Task 2; provider in Task 3). `traverse` will be added in Plan 02.
- **Files modified:** crates/polint/src/analysis/reachability/mod.rs
- **Verification:** `cargo build -p polint` succeeds at each task boundary.
- **Committed in:** 2292e6a / 13686e5 / b273b7d

**2. [Rule 2 - Missing Critical] Added AnalysisDb reachability storage + accessors**
- **Found during:** Task 2 (validate/debug need stored facts)
- **Issue:** `validate_reachability` and `debug.rs` read `db.reachability_roots()`, and the provider needs `db.replace_reachability_facts(...)`; these did not exist on `AnalysisDb`.
- **Fix:** Added `reachability_roots`/`reachability_marks` storage fields, `replace_reachability_facts` (with referential validation via `ReachabilityStore::from_output`), and `reachability_roots()`/`reachability_marks()` accessors, plus the `CallReachabilityFact` marks shape, following the `replace_entrypoint_facts` precedent (with `#[allow(dead_code, reason=...)]` for not-yet-fully-wired methods, matching the codebase Phase-34 precedent).
- **Files modified:** crates/polint/src/core/mod.rs, crates/polint/src/analysis/reachability/facts.rs
- **Verification:** store/validate/debug/provider tests all pass.
- **Committed in:** 13686e5 / b273b7d

**3. [Rule 1 - Bug] Fixed three eval/kernel goldens + 4 eval-fixture TOMLs for the new provider order**
- **Found during:** Task 3 (full-suite run after the manifest splice)
- **Issue:** Splicing `polint.reachability` after `polint.entrypoints` shifted indices in every hardcoded provider-order expectation: `analysis_kernel/mod.rs` (2 lists), `run_report.rs`, `eval/observed.rs`, `eval/fixtures.rs`, plus 4 `expected.polint-eval.toml` fixtures.
- **Fix:** Inserted `polint.reachability` after `polint.entrypoints` and renumbered subsequent `provider_order.N` indices in every expectation and the provider_order_report golden.
- **Files modified:** analysis_kernel/mod.rs, analysis_kernel/incremental/run_report.rs, eval/observed.rs, eval/fixtures.rs, tests/eval-fixtures/{kernel/provider-order, type-value-alias/{extension-precision,ts-js-core,go-core}}/expected.polint-eval.toml
- **Verification:** `cargo test -p polint --lib` 1668/1668 green; CLI integration 140/140 green.
- **Committed in:** b273b7d

---

**Total deviations:** 3 auto-fixed (1 blocking, 1 missing-critical, 1 bug/golden-update).
**Impact on plan:** All auto-fixes were necessary to make the plan's provider-order splice land cleanly. No scope creep — the goldens were explicitly named as Task 3 update targets in the plan.

## Issues Encountered
- `push_function`/`replace_entrypoint_facts` reassign dense IDs on insert, so two discovery tests initially asserted pre-store IDs. Fixed the test expectations to read the actual stored IDs (test-expectation bug, not implementation bug).
- The pre-commit `make lint` gate denies warnings (`-D warnings`) and builds `--all-targets` (so `analysis/mod.rs`'s `cfg(not(test))` dead-code `expect` does not cover test builds). Resolved by exercising the new `as_str` label helpers in co-located tests and using the codebase `#[allow(dead_code, reason=...)]` precedent for the not-yet-wired DB methods.

## User Setup Required
None - no external service configuration required. The `[reachability] roots` config section is optional and defaults to empty.

## Next Phase Readiness
- Roots are now discoverable as typed `ReachabilityRootFact` rows and the `polint.reachability` provider runs in the kernel after `polint.entrypoints`.
- Plan 02 can add `traverse.rs` (BFS/DFS marking over direct-call edges) to populate the `CallReachabilityFact` marks slot (already wired through store/digest/debug, currently always empty).
- Plan 03's determinism gate inherits the sort-then-assign-dense-IDs + output-digest byte-stability proven here.

---
*Phase: 43-reachability-roots-per-suite-scoring-mode*
*Completed: 2026-05-29*

## Self-Check: PASSED

All 8 created reachability source files exist, SUMMARY.md exists, and all 4 task commits (2292e6a, 13686e5, b273b7d, bbc0e4c) are present in git history.
