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
pub(crate) fn evaluate_regression_budget(
    baseline: &StoreDisabledBaseline,
    measured: &CurvePoint,
    thresholds: &BaselineThresholds,
) -> RegressionGateReport {
    let checks = vec![
        ratio_budget_check(
            "peak_rss_delta_ratio",
            measured.peak_rss_delta_bytes,
            baseline.peak_rss_delta_bytes,
            thresholds.max_peak_rss_ratio,
        ),
        ratio_budget_check(
            "cold_wall_clock_ratio",
            measured.cold_wall_clock_ms,
            baseline.cold_wall_clock_ms,
            thresholds.max_cold_wall_clock_ratio,
        ),
    ];
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

/// Build a "measured/baseline ratio must not exceed `budget`" check. A zero
/// baseline denominator is a Fail with a "missing baseline" observation rather
/// than a divide-by-zero (threat T-63-04-02).
fn ratio_budget_check(metric: &str, measured: u64, baseline: u64, budget: f64) -> GateCheck {
    if baseline == 0 {
        return GateCheck {
            metric: metric.to_string(),
            observed: "missing baseline (0 denominator)".to_string(),
            threshold: format!("<= {budget:.4}"),
            verdict: GateVerdict::Fail,
        };
    }
    let ratio = measured as f64 / baseline as f64;
    GateCheck {
        metric: metric.to_string(),
        observed: format!("{ratio:.4}"),
        threshold: format!("<= {budget:.4}"),
        verdict: if ratio > budget {
            GateVerdict::Fail
        } else {
            GateVerdict::Pass
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
        );
        assert_eq!(report.verdict, GateVerdict::Pass);
        assert!(!is_blocking(&report));
    }

    #[test]
    fn zero_baseline_denominator_fails_rather_than_panicking() {
        // A zero baseline peak-RSS delta is a missing-baseline Fail, not a panic
        // (threat T-63-04-02).
        let mut base = baseline();
        base.peak_rss_delta_bytes = 0;
        let report =
            evaluate_regression_budget(&base, &measured(1.0, 1.0), &BaselineThresholds::default());
        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(is_blocking(&report));
        assert!(report.checks.iter().any(|check| {
            check.metric == "peak_rss_delta_ratio"
                && check.observed.contains("missing baseline")
                && check.verdict == GateVerdict::Fail
        }));
    }
}
