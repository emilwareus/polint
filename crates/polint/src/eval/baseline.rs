use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::eval::bench::curve::CurvePoint;
use crate::eval::gates::{GateCheck, GateVerdict};
use crate::eval::model::EvaluationMode;
use crate::eval::report::{EvaluationRun, normalize_run};

pub(crate) const BASELINE_SCHEMA_VERSION: &str = "polint-eval-baseline-0";

/// Wire schema version for a serialized [`StoreDisabledBaseline`].
///
/// This is a DISTINCT constant from [`BASELINE_SCHEMA_VERSION`]: the
/// store-disabled baseline is its own artifact kind with its own shape, so
/// evolving it must never touch the shared `EvalBaseline` schema constant
/// (whose `validate()` asserts exact equality).
pub(crate) const STORE_DISABLED_BASELINE_SCHEMA_VERSION: &str = "polint-store-disabled-baseline-0";

/// The committed pre-store reference baseline for a `polint check` / `polint
/// review` run (BENCH-02).
///
/// The durable semantic store lands in Phase 64, so "store-disabled" is simply
/// current polint. This baseline is the fixed reference the Phase 64+ regression
/// gates (Plan 04's `evaluate_regression_budget`) compare against: it records the
/// real OS peak RSS and cold/warm wall-clock of the measured run, a
/// `store_disabled` marker, and a deterministic `diagnostics_digest` that is the
/// diagnostics-parity marker — the store must not change the diagnostics polint
/// emits, and this digest is what a later run asserts is unchanged.
///
/// This is intentionally a SEPARATE type from [`EvalBaseline`]: its shape is a
/// hard dependency of Plan 04 and must not drift with the accuracy-oriented
/// `EvalBaseline`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct StoreDisabledBaseline {
    pub(crate) schema_version: String,
    /// Always `true`: this baseline is the pre-store reference. `validate()`
    /// rejects a hand-edited `false`.
    pub(crate) store_disabled: bool,
    /// Stable repo/fixture identifier the baseline was measured against — never
    /// an absolute host path (threat T-63-03-03).
    pub(crate) repo_id: String,
    /// Suite/command context (e.g. the check vs review command marker).
    pub(crate) suite_id: String,
    /// Real OS peak RSS in bytes (from `getrusage`): the process-wide monotonic
    /// high-water mark. Reporting/reference only — the regression gate compares
    /// `peak_rss_delta_bytes`, which is not confounded by the host process's own
    /// allocations.
    pub(crate) peak_rss_bytes: u64,
    /// Run-attributable peak-RSS growth in bytes (the delta above the pre-run
    /// high-water mark). This is the confound-free metric the Phase 64+
    /// regression gate compares against. Serde-defaulted so a baseline
    /// serialized before this field existed still deserializes.
    #[serde(default)]
    pub(crate) peak_rss_delta_bytes: u64,
    /// Cold (first-run) wall-clock in milliseconds.
    pub(crate) cold_wall_clock_ms: u64,
    /// Warm (second-run) wall-clock in milliseconds.
    pub(crate) warm_wall_clock_ms: u64,
    /// Deterministic diagnostics/output digest — the parity marker.
    ///
    /// This is the whole-repo CHECK-scoped digest (the FNV hash over the sorted
    /// diagnostics of a `polint check`-equivalent kernel run), for BOTH the check
    /// and review baselines: `polint review` reuses the same check analysis here,
    /// so its parity marker is the same check digest, not a diff-scoped subset
    /// (LW-08). A store phase that computes a measured digest to feed
    /// `evaluate_regression_budget` MUST therefore supply a check-scoped digest
    /// (or `None`); a review-scoped subset would spuriously fail the parity check.
    pub(crate) diagnostics_digest: String,
}

impl StoreDisabledBaseline {
    /// Build a store-disabled baseline from a measured [`CurvePoint`] and the
    /// deterministic diagnostics digest of the same run.
    pub(crate) fn from_curve_point(
        repo_id: impl Into<String>,
        suite_id: impl Into<String>,
        point: &CurvePoint,
        diagnostics_digest: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: STORE_DISABLED_BASELINE_SCHEMA_VERSION.to_string(),
            store_disabled: true,
            repo_id: repo_id.into(),
            suite_id: suite_id.into(),
            peak_rss_bytes: point.peak_rss_bytes,
            peak_rss_delta_bytes: point.peak_rss_delta_bytes,
            cold_wall_clock_ms: point.cold_wall_clock_ms,
            warm_wall_clock_ms: point.warm_wall_clock_ms,
            diagnostics_digest: diagnostics_digest.into(),
        }
    }

    /// Enforce the store-disabled baseline invariants: its own schema constant,
    /// the pre-store marker, and a non-empty repo/suite/digest. A hand-edited or
    /// incomplete committed file fails this (threat T-63-03-01).
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema_version == STORE_DISABLED_BASELINE_SCHEMA_VERSION,
            "unsupported store-disabled baseline schema_version `{}`; expected `{STORE_DISABLED_BASELINE_SCHEMA_VERSION}`",
            self.schema_version
        );
        anyhow::ensure!(
            self.store_disabled,
            "store_disabled must be true; this artifact is the pre-store reference"
        );
        anyhow::ensure!(!self.repo_id.trim().is_empty(), "repo_id must not be empty");
        anyhow::ensure!(
            !self.suite_id.trim().is_empty(),
            "suite_id must not be empty"
        );
        anyhow::ensure!(
            !self.diagnostics_digest.trim().is_empty(),
            "diagnostics_digest (parity marker) must not be empty"
        );
        Ok(())
    }

    /// Write the baseline to `path` as deterministic pretty JSON with a trailing
    /// newline (POSIX-clean, diff-stable). Validates before writing.
    pub(crate) fn write(&self, path: &Path) -> anyhow::Result<()> {
        self.validate()?;
        let json = serde_json::to_string_pretty(self)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, format!("{json}\n"))?;
        Ok(())
    }

    /// Load and validate a committed store-disabled baseline.
    pub(crate) fn load(path: &Path) -> anyhow::Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        let baseline: StoreDisabledBaseline = serde_json::from_str(&raw)?;
        baseline.validate()?;
        Ok(baseline)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct EvalBaseline {
    pub(crate) schema_version: String,
    pub(crate) suite_id: String,
    pub(crate) suite_commit: Option<String>,
    pub(crate) mode: EvaluationMode,
    pub(crate) output_hash: String,
    pub(crate) run: EvaluationRun,
}

impl EvalBaseline {
    pub(crate) fn from_run(run: &EvaluationRun) -> anyhow::Result<Self> {
        ensure_real_polint_mode(run.mode)?;
        let run = normalize_run(run);
        Ok(Self {
            schema_version: BASELINE_SCHEMA_VERSION.to_string(),
            suite_id: run.suite_id.clone(),
            suite_commit: run
                .suite_manifest
                .as_ref()
                .and_then(|manifest| manifest.source_commit.clone()),
            mode: run.mode,
            output_hash: run.output_hash.clone(),
            run,
        })
    }

    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema_version == BASELINE_SCHEMA_VERSION,
            "unsupported baseline schema_version `{}`; expected `{BASELINE_SCHEMA_VERSION}`",
            self.schema_version
        );
        ensure_real_polint_mode(self.mode)?;
        anyhow::ensure!(
            !self.suite_id.trim().is_empty(),
            "suite_id must not be empty"
        );
        anyhow::ensure!(
            !self.output_hash.trim().is_empty(),
            "baseline output_hash must not be empty"
        );
        anyhow::ensure!(
            self.run.suite_id == self.suite_id,
            "baseline suite_id must match embedded run"
        );
        anyhow::ensure!(
            self.run.mode == self.mode,
            "baseline mode must match embedded run"
        );
        Ok(())
    }
}

/// Locked regression budget (REQUIREMENTS.md, "Locked Milestone Decisions", D):
/// a store-phase run may use at most +20% peak RSS versus the store-disabled
/// baseline until warm reuse lands (Phase 67). Revisable only with a recorded
/// decision — a silent edit here fails the default-value test (threat
/// T-63-04-01).
pub(crate) const DEFAULT_MAX_PEAK_RSS_RATIO: f64 = 1.20;

/// Locked regression budget (REQUIREMENTS.md, "Locked Milestone Decisions", D):
/// a store-phase run may use at most +25% cold wall-clock versus the
/// store-disabled baseline until warm reuse lands (Phase 67). Revisable only
/// with a recorded decision — a silent edit here fails the default-value test
/// (threat T-63-04-01).
pub(crate) const DEFAULT_MAX_COLD_WALL_CLOCK_RATIO: f64 = 1.25;

fn default_max_peak_rss_ratio() -> f64 {
    DEFAULT_MAX_PEAK_RSS_RATIO
}

fn default_max_cold_wall_clock_ratio() -> f64 {
    DEFAULT_MAX_COLD_WALL_CLOCK_RATIO
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct BaselineThresholds {
    pub(crate) max_precision_drop: f64,
    pub(crate) max_recall_drop: f64,
    pub(crate) warn_runtime_overhead_ratio: f64,
    pub(crate) fail_runtime_overhead_ratio: f64,
    pub(crate) max_new_false_positive_traps: u64,
    pub(crate) require_same_output_hash: bool,
    pub(crate) max_cache_miss_delta: i64,
    pub(crate) max_rejected_fact_delta: i64,
    /// Locked +20% peak-RSS regression budget vs the store-disabled baseline.
    /// A distinct signal from the eval-runner `*_runtime_overhead_ratio`
    /// fields, which gate the accuracy runner's observed runtime, not the
    /// whole-repo peak RSS. Serde-defaulted so thresholds serialized before
    /// this field still deserialize.
    #[serde(default = "default_max_peak_rss_ratio")]
    pub(crate) max_peak_rss_ratio: f64,
    /// Locked +25% cold-wall-clock regression budget vs the store-disabled
    /// baseline. Cold wall-clock is a distinct signal from the eval-runner
    /// runtime overhead, so this stays separate from the
    /// `*_runtime_overhead_ratio` fields. Serde-defaulted for backward-compatible
    /// deserialization.
    #[serde(default = "default_max_cold_wall_clock_ratio")]
    pub(crate) max_cold_wall_clock_ratio: f64,
}

impl Default for BaselineThresholds {
    fn default() -> Self {
        Self {
            max_precision_drop: 0.02,
            max_recall_drop: 0.02,
            warn_runtime_overhead_ratio: 1.10,
            fail_runtime_overhead_ratio: 1.25,
            max_new_false_positive_traps: 0,
            require_same_output_hash: false,
            max_cache_miss_delta: 0,
            max_rejected_fact_delta: 0,
            max_peak_rss_ratio: DEFAULT_MAX_PEAK_RSS_RATIO,
            max_cold_wall_clock_ratio: DEFAULT_MAX_COLD_WALL_CLOCK_RATIO,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct BaselineComparisonReport {
    pub(crate) suite_id: String,
    pub(crate) mode: EvaluationMode,
    pub(crate) verdict: GateVerdict,
    pub(crate) checks: Vec<GateCheck>,
}

pub(crate) fn write_baseline(path: &Path, run: &EvaluationRun) -> anyhow::Result<EvalBaseline> {
    let baseline = EvalBaseline::from_run(run)?;
    let json = serde_json::to_string_pretty(&baseline)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, json)?;
    Ok(baseline)
}

pub(crate) fn load_baseline(path: &Path) -> anyhow::Result<EvalBaseline> {
    let raw = std::fs::read_to_string(path)?;
    let baseline: EvalBaseline = serde_json::from_str(&raw)?;
    baseline.validate()?;
    Ok(baseline)
}

pub(crate) fn compare_to_baseline(
    baseline: &EvalBaseline,
    current: &EvaluationRun,
    thresholds: &BaselineThresholds,
) -> anyhow::Result<BaselineComparisonReport> {
    baseline.validate()?;
    ensure_real_polint_mode(current.mode)?;
    anyhow::ensure!(
        baseline.suite_id == current.suite_id,
        "baseline and current suite_id must match"
    );
    anyhow::ensure!(
        baseline.mode == current.mode,
        "baseline and current mode must match"
    );
    let current = normalize_run(current);
    let prior = &baseline.run;
    let checks = vec![
        max_f64_check(
            "precision_drop",
            option_drop(prior.metrics.precision, current.metrics.precision),
            thresholds.max_precision_drop,
        ),
        max_f64_check(
            "recall_drop",
            option_drop(prior.metrics.recall, current.metrics.recall),
            thresholds.max_recall_drop,
        ),
        runtime_overhead_check(prior, &current, thresholds),
        max_u64_check(
            "new_false_positive_traps",
            current
                .metrics
                .false_positive_trap_hits
                .saturating_sub(prior.metrics.false_positive_trap_hits),
            thresholds.max_new_false_positive_traps,
        ),
        output_hash_check(baseline, &current, thresholds.require_same_output_hash),
        max_i64_check(
            "cache_miss_delta",
            cache_misses(&current) - cache_misses(prior),
            thresholds.max_cache_miss_delta,
        ),
        max_i64_check(
            "rejected_fact_delta",
            current.metrics.facts_rejected as i64 - prior.metrics.facts_rejected as i64,
            thresholds.max_rejected_fact_delta,
        ),
    ];
    let verdict = checks
        .iter()
        .map(|check| check.verdict)
        .max()
        .unwrap_or(GateVerdict::Pass);
    Ok(BaselineComparisonReport {
        suite_id: current.suite_id,
        mode: current.mode,
        verdict,
        checks,
    })
}

pub(crate) fn deterministic_baseline_json(baseline: &EvalBaseline) -> anyhow::Result<String> {
    // Surface a serialization failure rather than swallowing it into a
    // valid-looking `"{}"`: this is the determinism/parity marker, so two
    // failed serializations must never compare equal and be mistaken for a
    // match.
    let mut normalized = baseline.clone();
    normalized.run = normalize_run(&normalized.run);
    normalized.output_hash = normalized.run.output_hash.clone();
    Ok(serde_json::to_string_pretty(&normalized)?)
}

fn ensure_real_polint_mode(mode: EvaluationMode) -> anyhow::Result<()> {
    anyhow::ensure!(
        matches!(
            mode,
            EvaluationMode::PolintBaseline | EvaluationMode::PolintAgentAdapted
        ),
        "adapter-only and competitor baselines cannot be compared as real polint analysis"
    );
    Ok(())
}

fn option_drop(prior: Option<f64>, current: Option<f64>) -> f64 {
    match (prior, current) {
        (Some(prior), Some(current)) if current < prior => prior - current,
        _ => 0.0,
    }
}

fn max_f64_check(metric: &str, observed: f64, threshold: f64) -> GateCheck {
    GateCheck {
        metric: metric.to_string(),
        observed: format!("{observed:.4}"),
        threshold: format!("<= {threshold:.4}"),
        verdict: if observed <= threshold {
            GateVerdict::Pass
        } else {
            GateVerdict::Fail
        },
    }
}

fn runtime_overhead_check(
    prior: &EvaluationRun,
    current: &EvaluationRun,
    thresholds: &BaselineThresholds,
) -> GateCheck {
    let ratio = runtime_ratio(prior, current).unwrap_or(1.0);
    GateCheck {
        metric: "runtime_overhead_ratio".to_string(),
        observed: format!("{ratio:.4}"),
        threshold: format!(
            "warn > {:.4}; fail > {:.4}",
            thresholds.warn_runtime_overhead_ratio, thresholds.fail_runtime_overhead_ratio
        ),
        verdict: if ratio > thresholds.fail_runtime_overhead_ratio {
            GateVerdict::Fail
        } else if ratio > thresholds.warn_runtime_overhead_ratio {
            GateVerdict::Warn
        } else {
            GateVerdict::Pass
        },
    }
}

fn runtime_ratio(prior: &EvaluationRun, current: &EvaluationRun) -> Option<f64> {
    let prior_ms = prior.performance.as_ref()?.runtime.observed_runtime_ms?;
    let current_ms = current.performance.as_ref()?.runtime.observed_runtime_ms?;
    if prior_ms == 0 {
        return None;
    }
    Some(current_ms as f64 / prior_ms as f64)
}

fn max_u64_check(metric: &str, observed: u64, threshold: u64) -> GateCheck {
    GateCheck {
        metric: metric.to_string(),
        observed: observed.to_string(),
        threshold: format!("<= {threshold}"),
        verdict: if observed <= threshold {
            GateVerdict::Pass
        } else {
            GateVerdict::Fail
        },
    }
}

fn max_i64_check(metric: &str, observed: i64, threshold: i64) -> GateCheck {
    GateCheck {
        metric: metric.to_string(),
        observed: observed.to_string(),
        threshold: format!("<= {threshold}"),
        verdict: if observed <= threshold {
            GateVerdict::Pass
        } else {
            GateVerdict::Fail
        },
    }
}

fn output_hash_check(
    baseline: &EvalBaseline,
    current: &EvaluationRun,
    require_same_output_hash: bool,
) -> GateCheck {
    let same = baseline.output_hash == current.output_hash;
    GateCheck {
        metric: "output_hash_changed".to_string(),
        observed: (!same).to_string(),
        threshold: format!("require_same={require_same_output_hash}"),
        verdict: if !require_same_output_hash || same {
            GateVerdict::Pass
        } else {
            GateVerdict::Fail
        },
    }
}

fn cache_misses(run: &EvaluationRun) -> i64 {
    run.performance
        .as_ref()
        .map_or(0, |performance| performance.cache.misses as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::bench::curve::{BudgetExhaustionCounters, CurvePoint, StoreSizeBytes};
    use crate::eval::performance::{CacheStatsSummary, EvalPerformanceReport, RuntimeStatsSummary};
    use crate::eval::report::to_deterministic_json_pretty;
    use crate::eval::report::{MetricSections, MetricSummary};

    #[test]
    fn baseline_round_trip_is_deterministic_and_path_normalized() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("baseline.json");
        let run = run(
            0.90,
            0.80,
            100,
            0,
            0,
            "hash-a",
            EvaluationMode::PolintBaseline,
        );

        let written = write_baseline(&path, &run).unwrap();
        let loaded = load_baseline(&path).unwrap();

        assert_eq!(written, loaded);
        assert_eq!(
            deterministic_baseline_json(&written).unwrap(),
            deterministic_baseline_json(&loaded).unwrap()
        );
        assert!(to_deterministic_json_pretty(&loaded.run).contains("baseline-suite"));
    }

    #[test]
    fn baseline_comparison_covers_pass_warning_and_fail() {
        let prior = EvalBaseline::from_run(&run(
            0.90,
            0.80,
            100,
            0,
            0,
            "hash-a",
            EvaluationMode::PolintBaseline,
        ))
        .unwrap();
        let pass = run(
            0.90,
            0.80,
            105,
            0,
            0,
            "hash-a",
            EvaluationMode::PolintBaseline,
        );
        let warn = run(
            0.90,
            0.80,
            115,
            0,
            0,
            "hash-a",
            EvaluationMode::PolintBaseline,
        );
        let fail = run(
            0.70,
            0.70,
            130,
            1,
            2,
            "hash-b",
            EvaluationMode::PolintBaseline,
        );
        let mut thresholds = BaselineThresholds {
            require_same_output_hash: true,
            ..BaselineThresholds::default()
        };

        assert_eq!(
            compare_to_baseline(&prior, &pass, &thresholds)
                .unwrap()
                .verdict,
            GateVerdict::Pass
        );
        assert_eq!(
            compare_to_baseline(&prior, &warn, &thresholds)
                .unwrap()
                .verdict,
            GateVerdict::Warn
        );
        thresholds.max_precision_drop = 0.05;
        assert_eq!(
            compare_to_baseline(&prior, &fail, &thresholds)
                .unwrap()
                .verdict,
            GateVerdict::Fail
        );
    }

    #[test]
    fn baseline_thresholds_carry_locked_regression_budgets() {
        // The locked +20% peak-RSS and +25% cold-wall-clock budgets
        // (REQUIREMENTS.md Locked Milestone Decisions, D). These are exact:
        // a silent loosening fails this assertion (threat T-63-04-01).
        let thresholds = BaselineThresholds::default();
        assert_eq!(thresholds.max_peak_rss_ratio, 1.20);
        assert_eq!(thresholds.max_cold_wall_clock_ratio, 1.25);
        // The distinct eval-runner runtime-overhead budgets are unchanged.
        assert_eq!(thresholds.warn_runtime_overhead_ratio, 1.10);
        assert_eq!(thresholds.fail_runtime_overhead_ratio, 1.25);
    }

    #[test]
    fn baseline_thresholds_deserialize_without_the_new_ratio_fields() {
        // A thresholds document serialized before the peak-RSS / cold-wall-clock
        // fields existed must still deserialize, defaulting to the locked budgets.
        let legacy = r#"{
            "max_precision_drop": 0.02,
            "max_recall_drop": 0.02,
            "warn_runtime_overhead_ratio": 1.10,
            "fail_runtime_overhead_ratio": 1.25,
            "max_new_false_positive_traps": 0,
            "require_same_output_hash": false,
            "max_cache_miss_delta": 0,
            "max_rejected_fact_delta": 0
        }"#;
        let thresholds: BaselineThresholds = serde_json::from_str(legacy).unwrap();
        assert_eq!(thresholds.max_peak_rss_ratio, 1.20);
        assert_eq!(thresholds.max_cold_wall_clock_ratio, 1.25);
    }

    #[test]
    fn adapter_only_baselines_cannot_be_compared_as_polint_analysis() {
        let run = run(0.90, 0.80, 100, 0, 0, "hash-a", EvaluationMode::AdapterOnly);

        assert!(EvalBaseline::from_run(&run).is_err());
    }

    fn sample_curve_point() -> CurvePoint {
        CurvePoint {
            repo_id: "polint-tiny-fixture".to_string(),
            repo_file_count: 2,
            repo_source_bytes: 256,
            diff_files: 0,
            diff_hunk_lines: 0,
            cold_wall_clock_ms: 42,
            warm_wall_clock_ms: 21,
            peak_rss_bytes: 128 * 1024 * 1024,
            peak_rss_delta_bytes: 96 * 1024 * 1024,
            size: StoreSizeBytes {
                cache_bytes: 4096,
                store_bytes: 0,
            },
            budget: BudgetExhaustionCounters::default(),
        }
    }

    #[test]
    fn store_disabled_baseline_round_trips_and_validates() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("store-disabled.json");
        let point = sample_curve_point();
        let baseline = StoreDisabledBaseline::from_curve_point(
            "polint-tiny-fixture",
            "polint-tiny-fixture-check",
            &point,
            "abc123digest",
        );

        baseline.write(&path).unwrap();
        let loaded = StoreDisabledBaseline::load(&path).unwrap();

        assert_eq!(baseline, loaded);
        assert!(loaded.store_disabled);
        assert_eq!(loaded.peak_rss_bytes, point.peak_rss_bytes);
        assert_eq!(loaded.peak_rss_delta_bytes, point.peak_rss_delta_bytes);
        assert_eq!(loaded.cold_wall_clock_ms, point.cold_wall_clock_ms);
        assert_eq!(loaded.warm_wall_clock_ms, point.warm_wall_clock_ms);
        assert_eq!(
            loaded.schema_version,
            STORE_DISABLED_BASELINE_SCHEMA_VERSION
        );
        // The shared EvalBaseline schema constant is a distinct, untouched value.
        assert_ne!(
            STORE_DISABLED_BASELINE_SCHEMA_VERSION,
            BASELINE_SCHEMA_VERSION
        );
    }

    #[test]
    fn store_disabled_baseline_validate_rejects_tampering() {
        let point = sample_curve_point();
        // store_disabled hand-edited to false must fail validation.
        let mut tampered =
            StoreDisabledBaseline::from_curve_point("repo", "suite", &point, "digest");
        tampered.store_disabled = false;
        assert!(tampered.validate().is_err());

        // Empty parity digest must fail validation.
        let mut empty_digest = StoreDisabledBaseline::from_curve_point("repo", "suite", &point, "");
        empty_digest.store_disabled = true;
        assert!(empty_digest.validate().is_err());
    }

    #[test]
    fn committed_store_disabled_baselines_load_and_validate() {
        let root = workspace_root();
        for name in ["store-disabled-check.json", "store-disabled-review.json"] {
            let path = root
                .join("research/evaluation-harness/baselines")
                .join(name);
            let baseline = StoreDisabledBaseline::load(&path)
                .unwrap_or_else(|error| panic!("committed baseline {name} must load: {error}"));
            assert!(
                baseline.store_disabled,
                "committed baseline {name} must be store_disabled == true"
            );
            assert!(
                !baseline.diagnostics_digest.trim().is_empty(),
                "committed baseline {name} must carry a non-empty diagnostics_digest"
            );
            // No absolute host paths may leak into a committed artifact
            // (threat T-63-03-03).
            let raw = std::fs::read_to_string(&path).unwrap();
            assert!(
                !raw.contains("/Users/"),
                "{name} must not leak /Users/ paths"
            );
            assert!(!raw.contains("/home/"), "{name} must not leak /home/ paths");
        }
    }

    /// Env-gated regenerator (`POLINT_WRITE_STORE_DISABLED_BASELINE`): measures a
    /// small git fixture through the real `polint check` / `polint review`
    /// pipeline and rewrites the two committed store-disabled baselines. Run once
    /// to refresh the committed reference:
    ///
    /// ```text
    /// POLINT_WRITE_STORE_DISABLED_BASELINE=1 cargo test -p polint --lib \
    ///   eval::baseline::tests::regenerate_committed_store_disabled_baselines --exact
    /// ```
    #[test]
    fn regenerate_committed_store_disabled_baselines() {
        if std::env::var_os("POLINT_WRITE_STORE_DISABLED_BASELINE").is_none() {
            return;
        }
        use crate::eval::bench::runner::{
            diagnostics_digest_for_repo, run_repo_perf_point_isolated,
        };
        use std::process::Command;

        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .current_dir(dir)
                .args(args)
                .output()
                .expect("git invocation");
            assert!(out.status.success(), "git {args:?} failed");
        };
        git(&["init", "--quiet"]);
        git(&["config", "user.email", "t@example.com"]);
        git(&["config", "user.name", "Test"]);
        git(&["config", "commit.gpgsign", "false"]);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(
            dir.join("src/router.go"),
            "package app\n\nfunc handle() { helper() }\n\nfunc helper() { println(1) }\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("src/util.ts"),
            "export function add(a: number, b: number): number {\n  return a + b;\n}\n",
        )
        .unwrap();
        git(&["add", "-A"]);
        git(&["commit", "--quiet", "-m", "base"]);
        let base = {
            let out = Command::new("git")
                .current_dir(dir)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        std::fs::write(
            dir.join("src/util.ts"),
            "export function add(a: number, b: number): number {\n  return a + b + 0;\n}\n",
        )
        .unwrap();
        git(&["add", "-A"]);
        git(&["commit", "--quiet", "-m", "change"]);

        // The digest run is in-process (it reads diagnostics, not RSS), so its
        // saturation of this process's peak-RSS high-water mark does not matter.
        // Each perf point, by contrast, is measured in its OWN fresh child so its
        // `peak_rss_delta_bytes` is run-attributable and order-independent rather
        // than a shared-process artifact — the review point in particular must
        // not collapse to allocator jitter just because it ran after check
        // (HI-01R). A Phase 64 measured run fed to the gate must use the same
        // `run_repo_perf_point_isolated` isolation.
        let digest = diagnostics_digest_for_repo(dir).unwrap();
        let check_point = run_repo_perf_point_isolated(dir, None).unwrap();
        let review_point = run_repo_perf_point_isolated(dir, Some(&base)).unwrap();

        let out_dir = workspace_root().join("research/evaluation-harness/baselines");
        StoreDisabledBaseline::from_curve_point(
            "polint-tiny-fixture",
            "polint-tiny-fixture-check",
            &check_point,
            digest.clone(),
        )
        .write(&out_dir.join("store-disabled-check.json"))
        .unwrap();
        StoreDisabledBaseline::from_curve_point(
            "polint-tiny-fixture",
            "polint-tiny-fixture-review",
            &review_point,
            // Intentionally the whole-repo CHECK digest, not a review-scoped one
            // (LW-08). `diagnostics_digest_for_repo` only runs `run_check_kernel`;
            // no review-scoped digest function exists, and a `polint review`
            // reuses the same check analysis (the diff gate is a cheap
            // reporting-layer filter over the same diagnostics). This baseline
            // therefore holds the check digest and MUST be compared against a
            // check-scoped measured digest (or `None`). Feeding a review-scoped
            // subset digest to the gate's `digest_parity_check` would spuriously
            // Fail; the field doc on `diagnostics_digest` records this contract.
            digest,
        )
        .write(&out_dir.join("store-disabled-review.json"))
        .unwrap();
    }

    fn workspace_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }

    fn run(
        precision: f64,
        recall: f64,
        runtime_ms: u64,
        trap_hits: u64,
        rejected_facts: u64,
        hash: &str,
        mode: EvaluationMode,
    ) -> EvaluationRun {
        EvaluationRun {
            schema_version: crate::eval::report::EVALUATION_SCHEMA_VERSION.to_string(),
            suite_id: "baseline-suite".to_string(),
            mode,
            suite_manifest: None,
            cases: Vec::new(),
            metrics: MetricSummary {
                true_positives: 8,
                false_positives: 1,
                false_negatives: 2,
                true_negatives: 9,
                unconfirmed: 0,
                false_positive_trap_hits: trap_hits,
                forbidden_hits: 0,
                unknown_count: 0,
                facts_present: 0,
                facts_accepted: 0,
                facts_rejected: rejected_facts,
                graph_edges_expected: 0,
                graph_edges_observed: 0,
                graph_edges_unconfirmed: 0,
                paths_expected: 0,
                paths_observed: 0,
                paths_unconfirmed: 0,
                runtime_budget_passed: 0,
                runtime_budget_failed: 0,
                precision: Some(precision),
                recall: Some(recall),
                f1: None,
                f2: None,
                f3: None,
                false_positive_rate: None,
                sections: MetricSections::default(),
            },
            performance: Some(EvalPerformanceReport {
                providers: Vec::new(),
                cache: CacheStatsSummary {
                    misses: rejected_facts,
                    ..CacheStatsSummary::default()
                },
                demand_queries: Vec::new(),
                runtime: RuntimeStatsSummary {
                    observed_runtime_ms: Some(runtime_ms),
                    peak_rss_bytes: None,
                },
                rss: Default::default(),
            }),
            comparison_rows: Vec::new(),
            adaptation: None,
            adaptation_delta: None,
            limitations: Vec::new(),
            output_hash: hash.to_string(),
        }
    }
}
