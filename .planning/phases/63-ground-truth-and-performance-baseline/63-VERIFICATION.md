---
phase: 63-ground-truth-and-performance-baseline
verified: 2026-07-09T00:00:00Z
status: passed
score: 5/5 must-haves verified
overrides_applied: 0
re_verification:
  # No — initial verification (no prior VERIFICATION.md existed)
---

# Phase 63: Ground Truth and Performance Baseline Verification Report

**Phase Goal:** The scale, latency, and accuracy problems become visible and gateable BEFORE any store code lands: baselines are recorded, curves are produced, and regression gates are wired so every later phase can prove it moved an outcome gate.
**Verified:** 2026-07-09
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth (roadmap Success Criteria) | Status | Evidence |
|---|----------------------------------|--------|----------|
| 1 | Pinned-commit manifest covers the locked repo set (grafana/hugo/excalidraw + Jelly + Go x/tools oracle suites + devloupe local-only, non-CI) | ✓ VERIFIED | 4 new TOML manifests exist and validate; `grafana-grafana-scale.toml` (commit `b587018...`, Go+TS, fast/nightly/release tiers), `gohugoio-hugo-scale.toml` (v0.140.0), `excalidraw-excalidraw-scale.toml` (v0.17.6), `devloupe-monorepo-local.toml` (`local_clone_policy = "allow_absolute"`, only a `research` tier, header comment `local-only`/`7.4`). `BENCHMARK-SUITE.md` indexes all four + the pre-existing `jelly-callgraph-micro.toml` and `go-x-tools-rta-callgraph.toml` oracle suites. `committed_evaluation_suite_manifests_parse_and_validate` passes (in `eval::` run). |
| 2 | Harness produces peak RSS, cold/warm wall-clock, cache/store size, budget-exhaustion telemetry as machine-readable curves vs repo size + diff size, plus markdown report | ✓ VERIFIED | `measure.rs` reads real `getrusage(RUSAGE_SELF).ru_maxrss` (per-OS normalized), cold/warm timing; `curve.rs` `CurvePoint` keyed by `repo_file_count`/`repo_source_bytes` and `diff_files`/`diff_hunk_lines` with `size`/`budget` fields (serde `deny_unknown_fields`); `runner.rs` drives real `AnalysisKernel::run` via `cold_then_warm`, derives diff size via `changeset_for_ref`, folds budget counters from real `SummaryStatus/CallTargetStatus/DomainStatus::BudgetExceeded` facts; `report.rs` emits byte-stable JSON + `## Benchmark Curves` markdown; `sweep.rs` assembles a real multi-point `CurveSeries` and writes `benchmark-curves.json` + `benchmark-report.md`. Tests green. |
| 3 | Store-disabled baselines for `polint check` and `polint review` recorded and committed | ✓ VERIFIED (fixture stand-in, documented) | `StoreDisabledBaseline` type in `baseline.rs` (own `STORE_DISABLED_BASELINE_SCHEMA_VERSION`, shared `BASELINE_SCHEMA_VERSION` untouched); committed `store-disabled-check.json` + `store-disabled-review.json` both `store_disabled: true`, non-empty `diagnostics_digest`, peak RSS + cold/warm. Measured against small git fixture (`repo_id = polint-tiny-fixture`) because large clones are absent here — plan-anticipated stub, env-gated regen `POLINT_WRITE_STORE_DISABLED_BASELINE`. `committed_store_disabled_baselines_load_and_validate` passes. |
| 4 | Persisted-graph recall/precision baseline recorded from Jelly + Go x/tools and appears in benchmark report (accuracy-visibility gate) | ✓ VERIFIED (null recall labeled, documented) | `GraphAccuracyBaseline`/`GraphAccuracyRow` (`recall`/`precision` as `Option<f64>`) in `report.rs`; committed `persisted-graph-accuracy.json` has both suite rows with pinned `suite_commit`, `recall`/`precision` present as explicit `null`, honest `reference` label naming the pre-store status + `POLINT_WRITE_GRAPH_BENCH` regenerator. `render_benchmark_report` wires the `## Persisted-Graph Accuracy Baseline` section. Regenerator in `external/mod.rs` reads real adapter `EvaluationRun` metrics when clones present. Tests green. |
| 5 | Regression-gate wiring exists: an over-budget later phase fails its gate rather than passing silently | ✓ VERIFIED | `gate.rs` `evaluate_regression_budget` compares measured `CurvePoint` vs `StoreDisabledBaseline` on peak-RSS and cold-wall-clock ratios; `BaselineThresholds` extended with `max_peak_rss_ratio` (1.20) + `max_cold_wall_clock_ratio` (1.25), serde-defaulted, exact-equality tested; `is_blocking` returns true on `Fail`; tests prove 1.25x RSS → Fail/blocking, within-budget → Pass, 1.30x cold → cold-check Fail, zero denominator → missing-baseline Fail (no panic). |

**Score:** 5/5 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `research/evaluation-harness/suites/grafana-grafana-scale.toml` | Pinned Go+TS scale manifest | ✓ VERIFIED | 40-char commit, `kind=performance`, `ignored_by_git=true`, `repo_relative_only` |
| `research/evaluation-harness/suites/gohugoio-hugo-scale.toml` | Go medium scale manifest | ✓ VERIFIED | Pinned commit, `source_url` gohugoio/hugo |
| `research/evaluation-harness/suites/excalidraw-excalidraw-scale.toml` | TS medium scale manifest | ✓ VERIFIED | Pinned commit, `source_url` excalidraw/excalidraw |
| `research/evaluation-harness/suites/devloupe-monorepo-local.toml` | Local-only non-CI reference | ✓ VERIFIED | `allow_absolute`, only `research` tier, `local-only`/`7.4` comment |
| `research/evaluation-harness/suites/BENCHMARK-SUITE.md` | Locked-set index | ✓ VERIFIED | All 4 repos + Jelly + Go x/tools oracle suites documented; devloupe labeled non-CI |
| `crates/polint/src/eval/bench/measure.rs` | getrusage peak RSS + cold/warm | ✓ VERIFIED | Real `libc::getrusage`, per-OS normalization, `peak_rss_bytes()>0` test |
| `crates/polint/src/eval/bench/curve.rs` | Curve-point telemetry types | ✓ VERIFIED | `CurvePoint`/`CurveSeries`, `deny_unknown_fields`, `Ord`, round-trip test |
| `crates/polint/src/eval/bench/runner.rs` | Whole-repo perf runner | ✓ VERIFIED | 360 lines; real kernel run, `changeset_for_ref`, budget-fact walk |
| `crates/polint/src/eval/bench/report.rs` | Curve JSON + markdown + graph accuracy | ✓ VERIFIED | Byte-stable JSON, required columns, graph-accuracy baseline |
| `crates/polint/src/eval/bench/sweep.rs` | Multi-point sweep entry-point | ✓ VERIFIED | Iterates committed manifests, skips absent checkouts, writes both artifacts |
| `crates/polint/src/eval/bench/gate.rs` | Regression-budget gate | ✓ VERIFIED | Fail/Pass verdict, `is_blocking`, zero-denominator guard |
| `crates/polint/src/eval/baseline.rs` | `StoreDisabledBaseline` + thresholds | ✓ VERIFIED | Distinct type/schema constant; RSS+cold ratio budgets |
| `research/evaluation-harness/baselines/store-disabled-check.json` | Committed check baseline | ✓ VERIFIED | `store_disabled:true`, non-empty digest |
| `research/evaluation-harness/baselines/store-disabled-review.json` | Committed review baseline | ✓ VERIFIED | `store_disabled:true`, parity digest |
| `research/evaluation-harness/baselines/persisted-graph-accuracy.json` | Pre-store graph accuracy | ✓ VERIFIED | Both suites, recall/precision keys present (null, labeled) |

### Key Link Verification

| From | To | Via | Status | Details |
|------|-----|-----|--------|---------|
| `measure.rs` | `libc::getrusage` | `RUSAGE_SELF ru_maxrss` | ✓ WIRED | Real FFI call, per-OS normalize |
| `eval/mod.rs` | `bench/mod.rs` | `pub(crate) mod bench` | ✓ WIRED | Line 12, crate-private |
| `runner.rs` | `measure.rs` | `cold_then_warm` + `peak_rss_bytes` | ✓ WIRED | Used around kernel run |
| `runner.rs` | `git/mod.rs` | `changeset_for_ref` | ✓ WIRED | Diff-size measurement for review |
| `report.rs` | `curve.rs` | `CurveSeries` JSON + markdown | ✓ WIRED | Serialized + rendered |
| `baseline.rs` | `curve.rs` | `StoreDisabledBaseline::from_curve_point` | ✓ WIRED | RSS+cold sourced from CurvePoint |
| `persisted-graph-accuracy.json` | `eval/external` | Jelly + Go x/tools adapter runs | ✓ WIRED | `from_runs` reads adapter metrics; `POLINT_WRITE_GRAPH_BENCH` regen |
| `gate.rs` | `baseline.rs` | reads `StoreDisabledBaseline` | ✓ WIRED | Reference denominator |
| `gate.rs` | `gates.rs` | `GateVerdict`/`GateCheck` | ✓ WIRED | Reuses existing verdict vocabulary |

### Data-Flow Trace (Level 4)

| Artifact | Data Variable | Source | Produces Real Data | Status |
|----------|---------------|--------|--------------------|--------|
| `runner.rs` CurvePoint | `budget`/`repo_file_count`/timing | `AnalysisKernel::run` output `db` facts + `getrusage` | Yes (real kernel + syscall) | ✓ FLOWING |
| `store-disabled-*.json` | peak RSS / cold/warm / digest | real `polint check`/`review` over fixture stand-in | Yes (fixture-scale, honest `repo_id`) | ✓ FLOWING (documented stand-in) |
| `persisted-graph-accuracy.json` | recall/precision | Jelly + Go x/tools adapters (gated) | Env-absent here → explicit `null`, env-gated regen | ⚠️ STATIC by design (labeled, accuracy-visibility honest) |

Note: the two ⚠️/stand-in rows are the plan-anticipated environment stubs called out in the verification brief (large scale-repo clones and the Jelly/Go x/tools graph clones + Go toolchain are absent in this environment). They are honestly labeled in-artifact and env-gated (`POLINT_WRITE_STORE_DISABLED_BASELINE`, `POLINT_WRITE_GRAPH_BENCH`), and are explicitly NOT counted as gaps per the accuracy-visibility gate (BENCH-04/CLI-07 intent).

### Behavioral Spot-Checks / Probe Execution

| Behavior | Command | Result | Status |
|----------|---------|--------|--------|
| Phase eval lib tests green | `cargo test -p polint --lib eval:: --locked` | `352 passed; 0 failed` (245.92s) | ✓ PASS |
| Public-surface leak gate green (bench stays pub(crate)) | `cargo test -p polint --test public_surface_leak --locked` | `5 passed; 0 failed` | ✓ PASS |
| No public `pub mod bench` leak | `grep -c "pub mod bench" crates/polint/src/lib.rs` | `0` | ✓ PASS |
| libc promoted to direct dep | `grep libc crates/polint/Cargo.toml` | `libc = "0.2"` | ✓ PASS |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| BENCH-01 | 63-01, 63-02 | Real-repo suite + peak RSS/cold-warm/cache-store/budget curves vs size | ✓ SATISFIED | Truths 1, 2 |
| BENCH-02 | 63-03 | Store-disabled check/review baseline recorded | ✓ SATISFIED | Truth 3 |
| BENCH-03 | 63-04 | Store-phase regression gates (fail-not-silent) | ✓ SATISFIED | Truth 5 |
| BENCH-04 | 63-03 | Persisted-graph recall/precision in benchmark report | ✓ SATISFIED | Truth 4 |

All four declared requirement IDs (BENCH-01..04) are accounted for. REQUIREMENTS.md maps the Ground Truth area to Phase 63 with no additional/orphaned IDs; the four items are marked `[x]` in REQUIREMENTS.md consistent with this phase.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| — | — | No `TBD`/`FIXME`/`XXX`/`TODO`/`todo!`/`unimplemented!`/placeholder in any phase-modified file | — | None |

The only `null` values (recall/precision in `persisted-graph-accuracy.json`) and `store_bytes = 0` / explicit-zero budget-counter decisions are honestly documented in-code and in the SUMMARYs as plan-anticipated, env-gated, non-stub decisions — not silent stubs.

### Human Verification Required

None. All truths are verifiable via committed artifacts, source inspection, and the green test suites. The two environment stubs (fixture-scale store-disabled baseline; null pre-store recall/precision) are plan-anticipated, honestly labeled, and reproducible via the documented env-gated regenerators when the large clones / Go toolchain are present. Per the brief they are recorded, not treated as gaps or human-decision items.

### Gaps Summary

No gaps. The phase goal is achieved: the measurement substrate (real getrusage RSS, cold/warm timing, curve types), the whole-repo runner + multi-point sweep, the committed store-disabled baselines, the pre-store graph-accuracy baseline surfaced in the benchmark report, and the enforceable +20% RSS / +25% cold regression gate all exist, are wired crate-privately under `eval::bench`, and are backed by passing tests. The scale/latency/accuracy problems are now visible and gateable before any store code lands, and the regression gate exposes a blocking signal every later phase (64+) can wire at its boundary.

---

_Verified: 2026-07-09_
_Verifier: Claude (gsd-verifier)_
