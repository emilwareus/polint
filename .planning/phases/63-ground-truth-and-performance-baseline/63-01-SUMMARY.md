---
phase: 63-ground-truth-and-performance-baseline
plan: 01
subsystem: testing
tags: [benchmark, performance, getrusage, peak-rss, curve-telemetry, suite-manifest, libc]

# Dependency graph
requires:
  - phase: 40-external-benchmark-adapters-and-promotion-gates
    provides: SuiteManifest schema (polint-eval-suite-1), SuiteKind::Performance, LocalClonePolicy, validate_suite_path
provides:
  - Pinned-commit CI scale suite manifests for grafana/grafana, gohugoio/hugo, excalidraw/excalidraw
  - Local-only, non-CI devloupe monorepo reference manifest (allow_absolute, research tier only)
  - BENCHMARK-SUITE.md index of the full locked repo set (scale + Jelly/Go x/tools oracles + local devloupe)
  - crate-private eval::bench substrate — real OS peak RSS (getrusage), cold/warm wall-clock timing
  - CurvePoint/CurveSeries telemetry types keyed by repo size + diff size with cache/store size and budget-exhaustion counters
affects: [63-02, 63-03, 63-04, curves, baseline-report, regression-gates, store-milestone]

# Tech tracking
tech-stack:
  added: [libc (promoted to direct dep of polint)]
  patterns:
    - "Real OS peak-RSS capture via getrusage(RUSAGE_SELF).ru_maxrss, per-OS normalized (macOS bytes / Linux kilobytes)"
    - "Curve-point telemetry keyed by repo size and diff size, serde deny_unknown_fields + derived Ord for deterministic ordering"
    - "Performance suites declared as pinned-commit local_clone manifests; local-only refs use allow_absolute + research-only tier to stay out of CI"

key-files:
  created:
    - research/evaluation-harness/suites/grafana-grafana-scale.toml
    - research/evaluation-harness/suites/gohugoio-hugo-scale.toml
    - research/evaluation-harness/suites/excalidraw-excalidraw-scale.toml
    - research/evaluation-harness/suites/devloupe-monorepo-local.toml
    - research/evaluation-harness/suites/BENCHMARK-SUITE.md
    - crates/polint/src/eval/bench/mod.rs
    - crates/polint/src/eval/bench/measure.rs
    - crates/polint/src/eval/bench/curve.rs
  modified:
    - crates/polint/Cargo.toml
    - crates/polint/src/eval/mod.rs
    - Cargo.lock

key-decisions:
  - "polint owns its lint table (unsafe_code forbid->deny, all other workspace lints mirrored) so one audited getrusage FFI opts in via #[allow(unsafe_code)]; unsafe stays denied crate-wide otherwise"
  - "Pinned commits are real release tags: grafana v11.4.0, hugo v0.140.0, excalidraw v0.17.6"
  - "devloupe is local-only/non-CI: research-tier-only + allow_absolute so CI fast/nightly/release never resolve it"

patterns-established:
  - "getrusage-based peak RSS is the single source of the previously-never-populated peak_rss_bytes schema field"
  - "Curve telemetry types are pub(crate) under eval::bench with no SDK/CLI surface"

requirements-completed: [BENCH-01]

# Metrics
duration: 25min
completed: 2026-07-09
---

# Phase 63 Plan 01: Ground Truth and Performance Baseline Substrate Summary

**Pinned-commit real-repo scale suite manifests plus a getrusage-backed measurement substrate (real peak RSS, cold/warm wall-clock) and curve-point telemetry keyed by repo size + diff size.**

## Performance

- **Duration:** ~25 min
- **Started:** 2026-07-09
- **Completed:** 2026-07-09
- **Tasks:** 2
- **Files modified:** 11 (8 created, 3 modified)

## Accomplishments
- Four `kind = "performance"` suite manifests: three CI scale repos pinned to real 40-char release-tag commits, plus a local-only/non-CI devloupe reference.
- `BENCHMARK-SUITE.md` documents the complete locked set in one index (scale repos + Jelly and Go x/tools recall oracles + local-only devloupe), with clone instructions and pinned commits.
- New crate-private `eval::bench` submodule measuring real OS peak RSS via `getrusage` and cold/warm wall-clock, finally populating the long-declared but never-set `peak_rss_bytes`.
- `CurvePoint`/`CurveSeries` telemetry types keyed by repo size and diff size, carrying cold/warm timing, peak RSS, cache/store size, and budget-exhaustion counters, with `deny_unknown_fields` and derived `Ord` for byte-identical deterministic serialization.

## Task Commits

1. **Task 1: Pinned-commit real-repo suite manifests + benchmark-suite index** - `b548e05e` (feat)
2. **Task 2: Measurement substrate — OS peak RSS, cold/warm timing, curve-point telemetry types** - `96e13a38` (feat)

## Files Created/Modified
- `research/evaluation-harness/suites/grafana-grafana-scale.toml` - Go+TS CI scale suite pinned to grafana v11.4.0 (`b587018...`)
- `research/evaluation-harness/suites/gohugoio-hugo-scale.toml` - Go CI scale suite pinned to hugo v0.140.0 (`3f35721...`)
- `research/evaluation-harness/suites/excalidraw-excalidraw-scale.toml` - TS CI scale suite pinned to excalidraw v0.17.6 (`f164071...`)
- `research/evaluation-harness/suites/devloupe-monorepo-local.toml` - Local-only/non-CI reference (allow_absolute, research tier only, ~1GB / cold 7.4s / warm 4.6s)
- `research/evaluation-harness/suites/BENCHMARK-SUITE.md` - Index of the full locked repo set
- `crates/polint/src/eval/bench/mod.rs` - bench submodule root
- `crates/polint/src/eval/bench/measure.rs` - getrusage peak RSS + TimedRun + cold_then_warm
- `crates/polint/src/eval/bench/curve.rs` - CurvePoint/CurveSeries/StoreSizeBytes/BudgetExhaustionCounters telemetry types
- `crates/polint/Cargo.toml` - libc direct dep + owned lint table
- `crates/polint/src/eval/mod.rs` - `pub(crate) mod bench;`
- `Cargo.lock` - records polint's new direct libc edge (no version change)

## Decisions Made
- Pinned each CI repo to a real release-tag commit fetched from the upstream remote (grafana v11.4.0, hugo v0.140.0, excalidraw v0.17.6) so the benchmark is reproducible.
- devloupe declared as local-only/non-CI via a research-only tier and `local_clone_policy = "allow_absolute"`, so CI fast/nightly/release runs never resolve it (threat T-63-01-02 mitigation).
- Public manifests keep `local_clone_policy = "repo_relative_only"` so `validate_suite_path` rejects traversal at parse time (threat T-63-01-01 mitigation).

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Workspace `unsafe_code = "forbid"` blocked the mandated getrusage FFI**
- **Found during:** Task 2 (measurement substrate)
- **Issue:** The plan mandates `getrusage`/`ru_maxrss` via libc (a `key_link` and acceptance criterion), which requires an `unsafe` block. The workspace sets `unsafe_code = "forbid"` (inherited by polint via `[lints] workspace = true`), and `forbid` cannot be relaxed by a per-item `#[allow]` — the crate failed to compile with `-F unsafe-code`.
- **Fix:** Replaced polint's `[lints] workspace = true` with an explicit lint table that mirrors every workspace lint verbatim (`unreachable_pub = "deny"` and all clippy lints preserved) but downgrades ONLY `unsafe_code` from `forbid` to `deny`. Added a single per-item `#[allow(unsafe_code, reason = "...")]` on `peak_rss_bytes()`. Unsafe therefore stays denied crate-wide with exactly one greppable, audited exception; the workspace-wide `forbid` still applies to every other crate.
- **Files modified:** `crates/polint/Cargo.toml`, `crates/polint/src/eval/bench/measure.rs`
- **Verification:** `cargo build -p polint --locked` and `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` pass; public-surface-leak grep (`pub mod bench` in lib.rs = 0) unaffected.
- **Committed in:** `96e13a38` (Task 2 commit)

**2. [Rule 3 - Blocking] Promoting libc to a direct dep required a Cargo.lock edge update**
- **Found during:** Task 2 (measurement substrate)
- **Issue:** `cargo build --locked` refused to record polint's new direct `libc` dependency edge even though the resolved version (0.2.186, already a vetted transitive dep) is unchanged.
- **Fix:** Ran `cargo build -p polint --offline` once to add the single edge line to `Cargo.lock` (no version change, no new package downloaded — threat T-63-01-SC preserved).
- **Files modified:** `Cargo.lock`
- **Verification:** `git diff --stat Cargo.lock` shows a single-line insertion; subsequent `--locked` builds/tests pass.
- **Committed in:** `96e13a38` (Task 2 commit)

**3. [Rule 1 - Bug] clippy `useless_vec` in a bench unit test**
- **Found during:** Task 2 (pre-commit `make lint`)
- **Issue:** A test built a fixed two-element `vec!` that clippy flagged as `useless_vec` under `-D warnings`.
- **Fix:** Changed `vec![...]` to a `[...]` array (still sortable).
- **Files modified:** `crates/polint/src/eval/bench/curve.rs`
- **Verification:** Full workspace clippy passes with `-D warnings`.
- **Committed in:** `96e13a38` (Task 2 commit)

---

**Total deviations:** 3 auto-fixed (2 blocking, 1 bug)
**Impact on plan:** All auto-fixes were necessary to compile and land the plan's mandated getrusage-based measurement. The lint-table change is the notable one — it narrowly relaxes a security-posture invariant for one audited FFI while preserving `deny`-by-default for the whole crate and the workspace `forbid` for every other crate. No scope creep; the delivered files and contracts match the plan.

## Issues Encountered
- None beyond the deviations above. Both plan verification tests pass: `eval::suite::tests::committed_evaluation_suite_manifests_parse_and_validate` and `eval::bench::` (6 tests).

## Threat Flags
None — the new surface (performance suite manifests + crate-private measurement types) is covered by the plan's threat register (T-63-01-01/02/03/SC), and all mitigations are in place: repo-relative validation for public manifests, allow_absolute + research-only tier for the local devloupe reference, and no new registry package.

## User Setup Required
None - no external service configuration required. Developers who want to run the CI scale suites clone each pinned repo into its repo-relative `checkout.path` per `BENCHMARK-SUITE.md`.

## Next Phase Readiness
- The measurement substrate (real peak RSS, cold/warm timing) and curve-point telemetry contracts are ready for the downstream Phase 63 plans (curves, baseline report, regression gates) to consume.
- No blockers. Note for reviewers: polint now owns its lint table; future workspace lint additions must be mirrored into `crates/polint/Cargo.toml` until/unless the FFI is isolated into a dedicated leaf crate.

---
*Phase: 63-ground-truth-and-performance-baseline*
*Completed: 2026-07-09*

## Self-Check: PASSED

All created files exist on disk and both task commits (b548e05e, 96e13a38) are in git history.
