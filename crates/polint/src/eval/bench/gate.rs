//! Store-phase regression-budget gate (BENCH-03).
//!
//! This gate turns Plan 03's committed store-disabled baseline into an enforced
//! budget: it compares a measured [`CurvePoint`] against the fixed
//! [`StoreDisabledBaseline`] on peak RSS and cold wall-clock, and produces a
//! Fail verdict when either exceeds its locked budget (+20% peak RSS, +25% cold
//! wall-clock — REQUIREMENTS.md Locked Milestone Decisions, D).
//!
//! It is invoked at every store-phase boundary from Phase 64 onward: a store
//! change whose measured run regresses scale or latency past the budget fails
//! its gate rather than passing silently ([`is_blocking`] returns `true` on
//! Fail, which a later phase converts to a non-zero exit at its phase boundary).
//! This is the fail-not-silent mechanism the scale/latency outcome gates rely on
//! (threat T-63-04-03).
//!
//! Everything here stays `pub(crate)` under `eval::bench`; it is the
//! crate-internal gate later store phases invoke, not a public CLI surface.

use crate::eval::baseline::{BaselineThresholds, StoreDisabledBaseline};
use crate::eval::bench::curve::CurvePoint;
use crate::eval::gates::{GateCheck, GateVerdict};

/// Absolute peak-RSS noise floor (bytes, HI-03). A run may exceed the baseline
/// peak-RSS delta by up to this many bytes before it Fails, even when that
/// exceeds the +20% ratio. Without it, a small baseline delta makes the ratio
/// tolerance a fraction of a megabyte, so ordinary allocator jitter would Fail a
/// run that did not regress. The locked +20% ratio still governs any baseline
/// large enough that `baseline * 1.20` exceeds `baseline + this floor`.
pub(crate) const PEAK_RSS_ABS_FLOOR_BYTES: u64 = 16 * 1024 * 1024;

/// Absolute cold-wall-clock noise floor (milliseconds, HI-03). A run may exceed
/// the baseline cold wall-clock by up to this many milliseconds before it Fails,
/// even when that exceeds the +25% ratio. A +25% ratio on a 20 ms baseline is
/// only 5 ms — below scheduling jitter — so without this floor the gate would
/// emit false Fails on sub-second baselines. The locked +25% ratio still governs
/// any baseline large enough that `baseline * 1.25` exceeds `baseline + floor`.
pub(crate) const COLD_WALL_CLOCK_ABS_FLOOR_MS: u64 = 50;

/// The outcome of comparing a measured run against the store-disabled baseline.
///
/// `verdict` aggregates the per-metric `checks` via `.max()` over the shared
/// `GateVerdict` ordering (`Pass < Warn < Fail`), matching the vocabulary in
/// `eval::gates`. This regression gate emits only `Pass` or `Fail`: there is no
/// soft-warn band for a locked budget, so the aggregate is effectively
/// Pass-or-Fail. The `Warn` tier exists in the shared enum but is intentionally
/// unused here.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RegressionGateReport {
    pub(crate) verdict: GateVerdict,
    pub(crate) checks: Vec<GateCheck>,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Phase64BoundaryReport {
    pub(crate) regression: RegressionGateReport,
    pub(crate) measured: CurvePoint,
    pub(crate) diagnostics_digest: String,
}

#[cfg(test)]
fn phase_64_comparison_baseline(
    committed: &StoreDisabledBaseline,
    disabled_control: &CurvePoint,
) -> StoreDisabledBaseline {
    StoreDisabledBaseline::from_curve_point(
        &committed.repo_id,
        &committed.suite_id,
        disabled_control,
        &committed.diagnostics_digest,
    )
}

#[cfg(test)]
pub(crate) fn evaluate_phase_64_boundary(
    repo_root: &std::path::Path,
    baseline_path: &std::path::Path,
) -> anyhow::Result<Phase64BoundaryReport> {
    use crate::eval::bench::runner::{
        SemanticStoreBenchMode, diagnostics_digest_for_repo_with_store_mode,
        run_repo_perf_point_isolated_with_store_mode,
    };

    let committed_baseline = StoreDisabledBaseline::load(baseline_path)?;
    // Match the committed baseline generator's cache state without hiding the
    // first store open: prime analysis/toolchain caches with a disabled digest,
    // measure enabled mode against an absent store, then compute enabled digest
    // parity after the measured run.
    let _priming_digest =
        diagnostics_digest_for_repo_with_store_mode(repo_root, SemanticStoreBenchMode::Disabled)?;
    // The committed tiny-fixture artifact locks the fixture identity and
    // diagnostics marker, but its millisecond values have no host, target, or
    // toolchain provenance. Pair disabled and enabled isolated children on the
    // same runner so the locked ratios evaluate semantic-store overhead rather
    // than process-launch and filesystem differences between machines.
    let disabled_control = run_repo_perf_point_isolated_with_store_mode(
        repo_root,
        None,
        SemanticStoreBenchMode::Disabled,
    )?;
    let comparison_baseline = phase_64_comparison_baseline(&committed_baseline, &disabled_control);
    let measured = run_repo_perf_point_isolated_with_store_mode(
        repo_root,
        None,
        SemanticStoreBenchMode::Enabled,
    )?;
    let diagnostics_digest =
        diagnostics_digest_for_repo_with_store_mode(repo_root, SemanticStoreBenchMode::Enabled)?;
    let regression = evaluate_regression_budget(
        &comparison_baseline,
        &measured,
        &BaselineThresholds::default(),
        Some(&diagnostics_digest),
    );
    Ok(Phase64BoundaryReport {
        regression,
        measured,
        diagnostics_digest,
    })
}

/// Evaluate a measured [`CurvePoint`] against the committed store-disabled
/// [`StoreDisabledBaseline`] on the two locked regression budgets: peak RSS
/// (`max_peak_rss_ratio`) and cold wall-clock (`max_cold_wall_clock_ratio`).
///
/// The peak-RSS budget compares the **run-attributable** `peak_rss_delta_bytes`,
/// not the process-wide absolute `peak_rss_bytes`: the absolute high-water mark
/// also reflects allocations made by whatever process hosts the measurement, so
/// gating on it would confound unrelated memory with the analyzed run (HI-01).
///
/// Produces one [`GateCheck`] per budget; each Fails if the measured/baseline
/// ratio exceeds its budget, else Passes. A zero baseline denominator is an
/// explicit Fail ("missing baseline") rather than a divide-by-zero panic
/// (threat T-63-04-02).
///
/// When `measured_diagnostics_digest` is `Some`, a diagnostics-parity check is
/// added and Fails if it differs from the baseline's `diagnostics_digest` — the
/// store must not change the diagnostics polint emits (BENCH-03, LW-02). Phase
/// 63 records the marker but has no run that produces a measured digest to feed
/// the gate, so callers pass `None` until the store phase that first measures
/// one; the parity mechanism is wired and tested here so it is not dead surface.
///
/// The baseline `diagnostics_digest` is CHECK-scoped for both the check and
/// review baselines (see [`StoreDisabledBaseline::diagnostics_digest`]), so a
/// caller that opts into the parity check MUST pass a check-scoped measured
/// digest; a review-scoped (diff-subset) digest would spuriously Fail (LW-08).
pub(crate) fn evaluate_regression_budget(
    baseline: &StoreDisabledBaseline,
    measured: &CurvePoint,
    thresholds: &BaselineThresholds,
    measured_diagnostics_digest: Option<&str>,
) -> RegressionGateReport {
    let mut checks = vec![
        ratio_budget_check(
            "peak_rss_delta_ratio",
            measured.peak_rss_delta_bytes,
            baseline.peak_rss_delta_bytes,
            thresholds.max_peak_rss_ratio,
            PEAK_RSS_ABS_FLOOR_BYTES,
        ),
        ratio_budget_check(
            "cold_wall_clock_ratio",
            measured.cold_wall_clock_ms,
            baseline.cold_wall_clock_ms,
            thresholds.max_cold_wall_clock_ratio,
            COLD_WALL_CLOCK_ABS_FLOOR_MS,
        ),
    ];
    if let Some(measured_digest) = measured_diagnostics_digest {
        checks.push(digest_parity_check(
            &baseline.diagnostics_digest,
            measured_digest,
        ));
    }
    let verdict = checks
        .iter()
        .map(|check| check.verdict)
        .max()
        .unwrap_or(GateVerdict::Pass);
    RegressionGateReport { verdict, checks }
}

/// Whether a report is blocking (a real regression the later phase must convert
/// to a non-zero exit at its phase boundary). True exactly when the verdict is
/// [`GateVerdict::Fail`], so an over-budget run cannot pass silently
/// (threat T-63-04-03, BENCH-03).
pub(crate) fn is_blocking(report: &RegressionGateReport) -> bool {
    report.verdict == GateVerdict::Fail
}

/// Build a "measured must not exceed its budget" check with an absolute noise
/// floor (HI-03). The measured value may exceed the baseline by up to the LARGER
/// of the ratio budget (`baseline * budget`) and an absolute tolerance
/// (`baseline + abs_floor`) before it Fails. The floor keeps the gate robust to
/// ms/MB jitter against a small baseline, while the locked ratio still governs
/// any baseline whose ratio headroom already exceeds the floor. A zero baseline
/// denominator is a Fail with a "missing baseline" observation rather than a
/// divide-by-zero (threat T-63-04-02).
fn ratio_budget_check(
    metric: &str,
    measured: u64,
    baseline: u64,
    budget: f64,
    abs_floor: u64,
) -> GateCheck {
    if baseline == 0 {
        return GateCheck {
            metric: metric.to_string(),
            observed: "missing baseline (0 denominator)".to_string(),
            threshold: format!("<= {budget:.4}"),
            verdict: GateVerdict::Fail,
        };
    }
    let ratio = measured as f64 / baseline as f64;
    // Effective ceiling: the larger of the ratio budget and the absolute floor,
    // so a small baseline still tolerates `abs_floor` of jitter.
    let allowed = (baseline as f64 * budget).max(baseline as f64 + abs_floor as f64);
    GateCheck {
        metric: metric.to_string(),
        observed: format!("{ratio:.4}"),
        threshold: format!("<= {budget:.4} (or within +{abs_floor} absolute floor)"),
        verdict: if measured as f64 > allowed {
            GateVerdict::Fail
        } else {
            GateVerdict::Pass
        },
    }
}

/// Build a diagnostics-parity check (LW-02, BENCH-03): the store must not change
/// the diagnostics polint emits, so a measured digest that differs from the
/// baseline's `diagnostics_digest` is a Fail. Only evaluated when the caller
/// supplies a measured digest.
fn digest_parity_check(baseline_digest: &str, measured_digest: &str) -> GateCheck {
    let same = baseline_digest == measured_digest;
    GateCheck {
        metric: "diagnostics_digest_parity".to_string(),
        observed: if same {
            "match".to_string()
        } else {
            format!("changed ({measured_digest})")
        },
        threshold: format!("== {baseline_digest}"),
        verdict: if same {
            GateVerdict::Pass
        } else {
            GateVerdict::Fail
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::bench::curve::{BudgetExhaustionCounters, CurvePoint, StoreSizeBytes};

    /// A baseline with round numbers so the test ratios are exact.
    fn baseline() -> StoreDisabledBaseline {
        StoreDisabledBaseline {
            schema_version: crate::eval::baseline::STORE_DISABLED_BASELINE_SCHEMA_VERSION
                .to_string(),
            store_disabled: true,
            repo_id: "polint-tiny-fixture".to_string(),
            suite_id: "polint-tiny-fixture-check".to_string(),
            peak_rss_bytes: 120_000_000,
            peak_rss_delta_bytes: 100_000_000,
            cold_wall_clock_ms: 1000,
            warm_wall_clock_ms: 500,
            diagnostics_digest: "digest".to_string(),
        }
    }

    /// A measured point at `rss_ratio`x baseline peak-RSS delta and `cold_ratio`x
    /// baseline cold wall-clock. The gate compares the run-attributable delta, so
    /// the delta (not the absolute peak) is what carries the ratio.
    fn measured(rss_ratio: f64, cold_ratio: f64) -> CurvePoint {
        let base = baseline();
        CurvePoint {
            repo_id: base.repo_id.clone(),
            repo_file_count: 2,
            repo_source_bytes: 256,
            diff_files: 0,
            diff_hunk_lines: 0,
            cold_wall_clock_ms: (base.cold_wall_clock_ms as f64 * cold_ratio) as u64,
            warm_wall_clock_ms: base.warm_wall_clock_ms,
            peak_rss_bytes: base.peak_rss_bytes,
            peak_rss_delta_bytes: (base.peak_rss_delta_bytes as f64 * rss_ratio) as u64,
            size: StoreSizeBytes::default(),
            budget: BudgetExhaustionCounters::default(),
        }
    }

    #[test]
    fn over_budget_peak_rss_fails_and_is_blocking() {
        // 1.25x peak RSS exceeds the +20% (1.20) budget.
        let report = evaluate_regression_budget(
            &baseline(),
            &measured(1.25, 1.0),
            &BaselineThresholds::default(),
            None,
        );
        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(is_blocking(&report));
        assert!(report.checks.iter().any(|check| {
            check.metric == "peak_rss_delta_ratio" && check.verdict == GateVerdict::Fail
        }));
    }

    #[test]
    fn within_budget_run_passes_and_is_not_blocking() {
        // 1.10x peak RSS and 1.15x cold wall-clock are both within budget.
        let report = evaluate_regression_budget(
            &baseline(),
            &measured(1.10, 1.15),
            &BaselineThresholds::default(),
            None,
        );
        assert_eq!(report.verdict, GateVerdict::Pass);
        assert!(!is_blocking(&report));
        assert!(
            report
                .checks
                .iter()
                .all(|check| check.verdict == GateVerdict::Pass)
        );
    }

    #[test]
    fn over_budget_cold_wall_clock_fails_the_cold_check() {
        // 1.30x cold wall-clock exceeds the +25% (1.25) budget, even with peak
        // RSS within budget.
        let report = evaluate_regression_budget(
            &baseline(),
            &measured(1.05, 1.30),
            &BaselineThresholds::default(),
            None,
        );
        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(is_blocking(&report));
        assert!(report.checks.iter().any(|check| {
            check.metric == "cold_wall_clock_ratio" && check.verdict == GateVerdict::Fail
        }));
        // The peak-RSS check stays a Pass — the two budgets are independent.
        assert!(report.checks.iter().any(|check| {
            check.metric == "peak_rss_delta_ratio" && check.verdict == GateVerdict::Pass
        }));
    }

    #[test]
    fn ratio_exactly_at_budget_passes() {
        // Exactly at the budget (1.20x RSS, 1.25x cold) is not "exceeds" — Pass.
        let report = evaluate_regression_budget(
            &baseline(),
            &measured(1.20, 1.25),
            &BaselineThresholds::default(),
            None,
        );
        assert_eq!(report.verdict, GateVerdict::Pass);
        assert!(!is_blocking(&report));
    }

    #[test]
    fn small_baseline_within_absolute_floor_passes_despite_ratio_breach() {
        // A tiny baseline: 20 ms cold, ~1 MB peak-RSS delta. The naive ratio
        // budgets (+25% cold = 5 ms, +20% RSS ~= 0.2 MB) would Fail on ordinary
        // jitter; the absolute floors (HI-03) exempt these sub-threshold deltas.
        let mut base = baseline();
        base.cold_wall_clock_ms = 20;
        base.peak_rss_delta_bytes = 1_000_000;

        let mut point = measured(1.0, 1.0);
        // Jitter far beyond the ratio budgets but inside the absolute floors.
        point.cold_wall_clock_ms = 40; // +20 ms < COLD_WALL_CLOCK_ABS_FLOOR_MS
        point.peak_rss_delta_bytes = 5_000_000; // +4 MB < PEAK_RSS_ABS_FLOOR_BYTES

        let report =
            evaluate_regression_budget(&base, &point, &BaselineThresholds::default(), None);
        assert_eq!(report.verdict, GateVerdict::Pass);
        assert!(!is_blocking(&report));
    }

    #[test]
    fn zero_baseline_denominator_fails_rather_than_panicking() {
        // A zero baseline peak-RSS delta is a missing-baseline Fail, not a panic
        // (threat T-63-04-02).
        let mut base = baseline();
        base.peak_rss_delta_bytes = 0;
        let report = evaluate_regression_budget(
            &base,
            &measured(1.0, 1.0),
            &BaselineThresholds::default(),
            None,
        );
        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(is_blocking(&report));
        assert!(report.checks.iter().any(|check| {
            check.metric == "peak_rss_delta_ratio"
                && check.observed.contains("missing baseline")
                && check.verdict == GateVerdict::Fail
        }));
    }

    #[test]
    fn changed_diagnostics_digest_fails_the_parity_check() {
        // A within-budget run whose diagnostics digest differs from the baseline
        // is a parity Fail: the store must not change the diagnostics polint
        // emits (LW-02). The parity check is only added when a measured digest is
        // supplied.
        let report = evaluate_regression_budget(
            &baseline(),
            &measured(1.0, 1.0),
            &BaselineThresholds::default(),
            Some("a-different-digest"),
        );
        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(is_blocking(&report));
        assert!(report.checks.iter().any(|check| {
            check.metric == "diagnostics_digest_parity" && check.verdict == GateVerdict::Fail
        }));
    }

    #[test]
    fn matching_diagnostics_digest_passes_the_parity_check() {
        // The same digest as the baseline passes; a `None` measured digest adds
        // no parity check at all (the Phase 63 default).
        let base = baseline();
        let with_digest = evaluate_regression_budget(
            &base,
            &measured(1.0, 1.0),
            &BaselineThresholds::default(),
            Some(&base.diagnostics_digest),
        );
        assert_eq!(with_digest.verdict, GateVerdict::Pass);
        assert!(with_digest.checks.iter().any(|check| {
            check.metric == "diagnostics_digest_parity" && check.verdict == GateVerdict::Pass
        }));

        let without_digest = evaluate_regression_budget(
            &base,
            &measured(1.0, 1.0),
            &BaselineThresholds::default(),
            None,
        );
        assert!(
            without_digest
                .checks
                .iter()
                .all(|check| check.metric != "diagnostics_digest_parity"),
            "no parity check is added when the measured digest is absent"
        );
    }

    #[test]
    fn phase_64_same_platform_baseline_uses_control_metrics_and_committed_identity() {
        let committed = baseline();
        let mut control = measured(0.75, 2.5);
        control.warm_wall_clock_ms = 777;

        let comparison = phase_64_comparison_baseline(&committed, &control);

        assert_eq!(comparison.repo_id, committed.repo_id);
        assert_eq!(comparison.suite_id, committed.suite_id);
        assert_eq!(comparison.diagnostics_digest, committed.diagnostics_digest);
        assert_eq!(
            comparison.peak_rss_delta_bytes,
            control.peak_rss_delta_bytes
        );
        assert_eq!(comparison.cold_wall_clock_ms, control.cold_wall_clock_ms);
        assert_eq!(comparison.warm_wall_clock_ms, control.warm_wall_clock_ms);
    }

    mod phase_64 {
        use std::path::{Path, PathBuf};
        use std::process::Command;

        use super::*;

        fn workspace_root() -> PathBuf {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("workspace root")
                .to_path_buf()
        }

        fn git(root: &Path, args: &[&str]) {
            let output = Command::new("git")
                .current_dir(root)
                .args(args)
                .output()
                .expect("git invocation");
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        fn write_committed_baseline_fixture(root: &Path) {
            git(root, &["init", "--quiet"]);
            git(root, &["config", "user.email", "t@example.com"]);
            git(root, &["config", "user.name", "Test"]);
            git(root, &["config", "commit.gpgsign", "false"]);
            std::fs::create_dir_all(root.join("src")).expect("create source directory");
            std::fs::write(
                root.join("src/router.go"),
                "package app\n\nfunc handle() { helper() }\n\nfunc helper() { println(1) }\n",
            )
            .expect("write Go fixture");
            std::fs::write(
                root.join("src/util.ts"),
                "export function add(a: number, b: number): number {\n  return a + b;\n}\n",
            )
            .expect("write TS fixture");
            git(root, &["add", "-A"]);
            git(root, &["commit", "--quiet", "-m", "base"]);
            std::fs::write(
                root.join("src/util.ts"),
                "export function add(a: number, b: number): number {\n  return a + b + 0;\n}\n",
            )
            .expect("write changed TS fixture");
            git(root, &["add", "-A"]);
            git(root, &["commit", "--quiet", "-m", "change"]);
        }

        #[test]
        fn real_store_enabled_measurement_passes_locked_boundary() {
            let repo = tempfile::tempdir().expect("phase 64 fixture repo");
            write_committed_baseline_fixture(repo.path());
            let baseline_path = workspace_root()
                .join("research/evaluation-harness/baselines/store-disabled-check.json");

            let boundary = evaluate_phase_64_boundary(repo.path(), &baseline_path)
                .expect("evaluate real Phase 64 boundary");

            eprintln!(
                "phase64 boundary: rss_delta={} cold_ms={} store_bytes={} checks={:?}",
                boundary.measured.peak_rss_delta_bytes,
                boundary.measured.cold_wall_clock_ms,
                boundary.measured.size.store_bytes,
                boundary.regression.checks
            );

            assert!(boundary.measured.size.store_bytes > 0);
            assert!(!boundary.diagnostics_digest.is_empty());
            assert!(!is_blocking(&boundary.regression), "{boundary:#?}");
            for metric in [
                "peak_rss_delta_ratio",
                "cold_wall_clock_ratio",
                "diagnostics_digest_parity",
            ] {
                assert!(
                    boundary.regression.checks.iter().any(|check| {
                        check.metric == metric && check.verdict == GateVerdict::Pass
                    }),
                    "missing passing check {metric}: {boundary:#?}"
                );
            }
        }
    }
}
