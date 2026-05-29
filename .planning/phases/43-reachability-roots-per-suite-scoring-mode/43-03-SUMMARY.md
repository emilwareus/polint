---
phase: 43-reachability-roots-per-suite-scoring-mode
plan: 03
subsystem: testing
tags: [determinism, eval, reachability, provider-manifests, ci, byte-identity, solver-reserved]

# Dependency graph
requires:
  - phase: 43-reachability-roots-per-suite-scoring-mode
    provides: analysis::reachability module + ReachabilityRootFact + CallReachabilityFact marking + polint.reachability provider (Plan 01) and the populated reachable-graph marking + ScoringMode (Plan 02)
  - phase: 42-identity-substrate
    provides: frozen MetricSummary discipline + MetricSections #[serde(default)] section pattern + the Linux+macOS leak-gate CI analog
provides:
  - "reserved SolverMetricSection { solver_step_count: u64, budget_exceeded_reasons: Vec<String> } as a #[serde(default)] sibling on MetricSections (NOT MetricSummary) — defaulted to 0/empty for Phase 47+"
  - "eval::determinism_gate: N=10 seeded-permutation byte-identical normalized-observed-JSON gate driven by provider_manifests() with per-fixture reachable-graph marking invariants"
  - "Go + TS/JS determinism fixtures (root + direct call + >=1 unreachable call) under tests/eval-fixtures/determinism/"
  - "fast-CI determinism-gate job on ubuntu-latest + macos-latest, fail-fast: false (independent passes, no averaging)"
  - "documented phases 44-54 per-phase inheritance obligation in the gate file and CI job"
affects: [44-marking-traversal, 45-per-suite-scoring-mode, 46-determinism-gate, 47-solver, reachability, scoring, determinism]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Reserved JSON shape on a #[serde(default)] MetricSections section so byte-identity gates stay stable when a later phase populates values"
    - "Parametric determinism harness driven by provider_manifests() for near-zero-maintenance auto-enrollment of future providers"
    - "In-tree SplitMix64 seeded Fisher-Yates shuffle for permutation seeds (no rand dependency, threat T-43-03-SC)"
    - "Order-independent normalized-observed-JSON byte-identity proven by feeding permuted observed rows through the live normalize_run + deterministic_output_hash path"

key-files:
  created:
    - crates/polint/src/eval/determinism_gate.rs
    - tests/eval-fixtures/determinism/go_reachable/repo/main.go
    - tests/eval-fixtures/determinism/go_reachable/repo/go.mod
    - tests/eval-fixtures/determinism/go_reachable/repo/.polint.toml
    - tests/eval-fixtures/determinism/go_reachable/expected.polint-eval.toml
    - tests/eval-fixtures/determinism/ts_reachable/repo/src/app.ts
    - tests/eval-fixtures/determinism/ts_reachable/repo/package.json
    - tests/eval-fixtures/determinism/ts_reachable/repo/.polint.toml
    - tests/eval-fixtures/determinism/ts_reachable/expected.polint-eval.toml
  modified:
    - crates/polint/src/eval/report.rs
    - crates/polint/src/eval/metrics.rs
    - crates/polint/src/eval/mod.rs
    - crates/polint/src/eval/fixtures.rs
    - .github/workflows/ci.yml

key-decisions:
  - "SolverMetricSection lives on MetricSections (#[serde(default)] sibling of categorized_failures), NOT on the frozen MetricSummary; metric_summary_layout_unchanged is left untouched (D-23)"
  - "The gate permutes the OBSERVED row-insertion order + the provider-enumeration order and asserts byte-identity through the live normalize_run path, rather than reordering internal kernel scheduling (which is fixed); the kernel itself is already deterministic, so this is the realistic permutation surface the gate controls"
  - "The shuffled provider set is sourced directly from provider_manifests() (no hand-written provider-name array) so phases 44-54 auto-enroll; the gate asserts shuffled count == provider_manifests().len()"
  - "The Go fixture's unreachable mark is produced from a call site in an unreachable function (orphan -> orphanHelper); this needs only tree-sitter call SITES, not resolved targets, so the gate passes WITHOUT a Go toolchain — matching the leak-gate CI analog which does not set up Go"

patterns-established:
  - "Pattern: reserve a future phase's JSON fields NOW on a defaulted MetricSections section so the milestone-wide byte-identity gate never breaks merely because the section appeared (Phase 47+ populates values, not shape)"
  - "Pattern: determinism gate = N seeded permutations of (provider order, row order) -> byte-identical normalized observed JSON, parametric over provider_manifests() for auto-enrollment"

requirements-completed: [REACH-03]

# Metrics
duration: 19min
completed: 2026-05-29
---

# Phase 43 Plan 03: Determinism Gate, Reserved Solver Fields & Cross-Platform CI Summary

**N=10 seeded-permutation byte-identical determinism gate driven by `provider_manifests()` over Go + TS/JS reachable-graph fixtures, the reserved `solver_step_count`/`budget_exceeded_reasons` JSON shape on a `#[serde(default)]` `MetricSections` section, a fast-CI Linux+macOS `determinism-gate` job, and the documented phases 44-54 inheritance obligation.**

## Performance

- **Duration:** ~19 min
- **Started:** 2026-05-29T16:24:43Z
- **Completed:** 2026-05-29T16:44Z
- **Tasks:** 4
- **Files modified/created:** 5 modified + 9 created

## Accomplishments

- **Task 1 — reserved solver fields:** Added `SolverMetricSection { solver_step_count: u64, budget_exceeded_reasons: Vec<String> }` as a `#[serde(default)]` sibling of `categorized_failures` on `MetricSections` (copying the `CategorizedFailureSection` derive shape), defaulted to `0`/`[]` for Phase 47+. The reserved section surfaces in the observed/report JSON via `From<ComputedMetrics> for MetricSummary`. `MetricSummary` and its `metric_summary_layout_unchanged` layout-lock are untouched; older v1.2/Phase-42 JSON (no `solver` section) still deserializes.
- **Task 2 — parametric determinism gate:** Added `eval/determinism_gate.rs` — for each fixture it observes the kernel once, then under 10 distinct seeded permutations of the provider-enumeration order AND the observed row-insertion order asserts the normalized observed JSON (via the live `normalize_run` + `deterministic_output_hash` path) is byte-identical. The provider set is sourced from `provider_manifests()` (asserted equal to `provider_manifests().len()`, D-22 auto-enrollment). Per-fixture invariants confirm each fixture yields >=1 root, >=1 direct call site, and >=1 `in_reachable_graph = false` mark (D-24). Two fixtures (`go_reachable`, `ts_reachable`) each carry a root + reachable call + unreachable call.
- **Task 3 — cross-platform fast CI:** Added a `determinism-gate` CI job modeled exactly on `leak-gate` — `matrix.os: [ubuntu-latest, macos-latest]`, `fail-fast: false`, running `cargo test -p polint --lib eval::determinism_gate --locked`. Both platforms pass independently (no averaging, D-24); the job comment documents the phases 44-54 inheritance precondition; `leak-gate` is byte-unchanged.
- **Task 4 — leak gate stays green:** Verified `SolverMetricSection` and the determinism-gate harness are `pub(crate)`/test-facing (no bare `pub`), the v1.3 leak gate passes 5/5, `ALLOWED_PRELUDE` is byte-unchanged from Phase 42, and clippy reports zero `unreachable_pub` for the eval additions.

## Task Commits

Each task was committed atomically:

1. **Task 1: Reserve solver_step_count / budget_exceeded_reasons on MetricSections** — `df9f0c9` (feat)
2. **Task 2: Parametric determinism-gate harness + Go/TS fixtures** — `36dab50` (feat)
3. **Task 3: Fast-CI Linux + macOS determinism-gate job** — `fb2d860` (ci)
4. **Task 4: Public-surface-leak gate stays green** — `ca599ef` (test, empty verification commit)

_TDD note: Tasks 1-2 are co-located test+implementation units (the typed `SolverMetricSection` and the gate harness must exist for their tests to compile), so each landed as a single `feat` commit with its tests rather than separate RED/GREEN commits — matching the Plan 01/02 convention._

## Files Created/Modified

- `crates/polint/src/eval/report.rs` — `SolverMetricSection` struct + `#[serde(default)] solver` field on `MetricSections`; default-values, serde round-trip, backward-compat, and destructure layout-lock tests
- `crates/polint/src/eval/metrics.rs` — default `solver` in `From<ComputedMetrics> for MetricSummary` so the report JSON surfaces the reserved section; import update
- `crates/polint/src/eval/determinism_gate.rs` — N=10 seeded-permutation byte-identity harness + reachable-graph marking invariants + D-25 inheritance doc
- `crates/polint/src/eval/mod.rs` — register `pub(crate) mod determinism_gate`
- `crates/polint/src/eval/fixtures.rs` — `evaluation_run_for_fixture_with_observed_for_test` test-facing helper feeding caller-supplied observed rows through the normalized-run path
- `tests/eval-fixtures/determinism/go_reachable/` — Go fixture: `func main` root, `main -> reachable` (reachable), `orphan -> orphanHelper` (unreachable)
- `tests/eval-fixtures/determinism/ts_reachable/` — TS fixture: exported `entry` root, `entry -> usedHelper` (reachable), `orphanFn -> orphanHelper` (unreachable)
- `.github/workflows/ci.yml` — `determinism-gate` job (ubuntu+macos, fail-fast false), phases 44-54 inheritance comment

## Decisions Made

- **Reserved fields on `MetricSections`, never on the frozen `MetricSummary`.** D-23 is explicit: the layout-locked `MetricSummary` destructure test stays untouched; extensions ride a `#[serde(default)]` section. Reserving the shape now keeps the N=10 byte-identity gate stable when Phase 47+ emits real values.
- **Permutation surface is observed-row + provider-enumeration order, not internal kernel scheduling.** The kernel runs providers in a fixed DAG order and is already internally deterministic; the realistic, controllable permutation the gate exercises is the order rows are collected/inserted before normalization plus the provider-enumeration order, both fed through the live `normalize_run`/`deterministic_output_hash` path. This generalizes `eval_report_normalization_makes_json_order_independent` and `solver.rs`'s single-provider seeded shuffle to the N=10 multi-provider case.
- **Gate driven by `provider_manifests()`, never a hand-written list.** The gate asserts the shuffled provider count equals `provider_manifests().len()`, so a newly registered solver provider (phases 44-54) auto-enrolls with no harness edit (D-22). `grep '"polint\.'` in the gate file is zero — no hardcoded provider names.
- **Go fixture passes without a Go toolchain.** The unreachable mark comes from a call site inside an unreachable function (`orphan`), which only needs tree-sitter call sites + the auto-discovered `main` root — not Go-sidecar-resolved targets. Verified by running the Go gate with `go` removed from `PATH`. This matches the `leak-gate` CI analog, which does not set up Go, so both ubuntu and macos CI runs are in the same (no-Go) state and stay cross-platform byte-consistent.

## Deviations from Plan

None — plan executed exactly as written. The only adjustments were lint-driven (auto-formatting via `cargo fmt` and rewording the gate's doc comment to satisfy clippy's `doc_lazy_continuation` lint), both of which are mechanical and fully within the planned files; no behavior or scope changed.

## Issues Encountered

- The plan's `<verify>` for Task 3 used `python3 -c "import yaml; ..."`, but PyYAML is not installed locally and pip is externally-managed (PEP 668). Validated the identical assertions (`fail-fast: false`, `matrix.os == {ubuntu-latest, macos-latest}`, the cargo step invokes `eval::determinism_gate`, `leak-gate` unchanged) with Ruby's built-in YAML instead — equivalent verification, CI itself will use whatever Python it has.
- The full native fixture-suite coverage test (`eval_native_fixture_suite_covers_required_categories`) auto-discovers every fixture under `tests/eval-fixtures/`, including the two new determinism fixtures. It ran green (175s) — the `facts`-area fixtures fall through to `run_native_fixture_for_test` with empty expected rows, so `false_negatives = forbidden_hits = runtime_budget_failed = 0`.

## User Setup Required

None — no external service configuration. The determinism fixtures are checked-in test inputs; the CI job uses only the existing Rust toolchain action (no Go/Node setup, no new dependencies).

## Threat Flags

None — no new network endpoints, auth paths, file-read surface, or schema changes at trust boundaries. The reserved solver JSON fields are mitigated by the backward-compat `#[serde(default)]` test (T-43-03-02); the fixtures are copied into a temp dir by the existing symlink-rejecting eval adapter (T-43-03-03); all additions are `pub(crate)`/test-facing and the leak gate stays green (T-43-03-04); no package-manager installs (T-43-03-SC, in-tree SplitMix64 RNG).

## Next Phase Readiness

- The determinism gate is wired and inherited: phases 44-54 get coverage for free because the provider set is driven by `provider_manifests()`, and the gate file + CI job document the per-phase obligation to keep the fixtures green.
- The `solver_step_count` / `budget_exceeded_reasons` JSON shape is reserved and defaulted, so Phase 47+ can populate real solver-step counts and budget-exceeded reasons without changing the observed-JSON shape or breaking the byte-identity gate.
- ROADMAP success criteria 3 and 4 for Phase 43 are met: the determinism gate fixture passes (10 shuffled runs -> byte-identical observed JSON, identical reserved solver step counts = 0 and budget reasons = empty), and the gate is wired so every subsequent solver phase inherits it.

---
*Phase: 43-reachability-roots-per-suite-scoring-mode*
*Completed: 2026-05-29*

## Self-Check: PASSED

All created files exist on disk (`determinism_gate.rs`, both fixture trees, `43-03-SUMMARY.md`) and all four task commits (df9f0c9, 36dab50, fb2d860, ca599ef) are present in git history.
