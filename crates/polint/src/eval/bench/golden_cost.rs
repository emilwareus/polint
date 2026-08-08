//! Per-golden-case cost records written when a check is asked to measure.
//!
//! The characterization harness sets [`COST_PATH_ENV`] to a sidecar path before
//! invoking `polint check`. The rules-host check path wraps analysis in
//! [`super::measure::TimedRun`] and writes wall-clock + peak RSS so the harness
//! can gate regressions without reimplementing OS RSS capture.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::measure::TimedRun;

/// Env var naming the JSON file overwritten with one cost record for a check.
pub(crate) const COST_PATH_ENV: &str = "POLINT_GOLDEN_COST_PATH";

const SCHEMA_VERSION: &str = "polint-golden-cost-1";

/// Initial per-case regression budget: fail when measured exceeds 1.20× baseline
/// (unless within the absolute noise floors below).
pub(crate) const MAX_COST_RATIO: f64 = 1.20;

/// Absolute wall-clock noise floor (ms) for golden cost gating.
pub(crate) const RUNTIME_ABS_FLOOR_MS: u64 = 50;

/// Absolute peak-RSS noise floor (bytes) for golden cost gating.
pub(crate) const PEAK_RSS_ABS_FLOOR_BYTES: u64 = super::gate::PEAK_RSS_ABS_FLOOR_BYTES;

/// Committed (or freshly measured) wall-clock and peak RSS for one golden case.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GoldenCostRecord {
    pub(crate) schema_version: String,
    pub(crate) wall_clock_ms: u64,
    pub(crate) peak_rss_bytes: u64,
    pub(crate) peak_rss_delta_bytes: u64,
}

impl GoldenCostRecord {
    pub(crate) fn from_timed_run(timing: TimedRun) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            wall_clock_ms: timing.elapsed_ms,
            peak_rss_bytes: timing.peak_rss_bytes,
            peak_rss_delta_bytes: timing.peak_rss_delta_bytes,
        }
    }

    pub(crate) fn to_json_line(&self) -> anyhow::Result<String> {
        let mut json = serde_json::to_string(self)?;
        json.push('\n');
        Ok(json)
    }
}

/// Run `check`, capturing wall-clock + peak RSS via [`TimedRun`], and write the
/// record when [`COST_PATH_ENV`] is set. Ordinary CLI runs leave the env unset
/// and pay only the env-var lookup.
pub(crate) fn run_with_optional_cost<F>(check: F) -> anyhow::Result<u8>
where
    F: FnOnce() -> anyhow::Result<u8>,
{
    let Some(path) = std::env::var_os(COST_PATH_ENV) else {
        return check();
    };

    let mut outcome = None;
    let mut check = Some(check);
    let timing = TimedRun::measure(|| {
        if let Some(run) = check.take() {
            outcome = Some(run());
        }
    });
    write_timed_run(Path::new(&path), timing)?;
    outcome.expect("cost-recorded check must produce an outcome")
}

pub(crate) fn write_timed_run(path: &Path, timing: TimedRun) -> anyhow::Result<()> {
    let record = GoldenCostRecord::from_timed_run(timing);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
            anyhow::anyhow!("create golden cost dir {}: {error}", parent.display())
        })?;
    }
    let json = record.to_json_line()?;
    std::fs::write(path, json)
        .map_err(|error| anyhow::anyhow!("write golden cost {}: {error}", path.display()))?;
    Ok(())
}

/// Compare measured costs against a committed baseline. Returns `Ok(())` when
/// both wall-clock and peak RSS stay within [`MAX_COST_RATIO`] (or the absolute
/// floors). Returns a human-readable failure string otherwise.
pub(crate) fn budget_failure(
    case_id: &str,
    baseline: &GoldenCostRecord,
    measured: &GoldenCostRecord,
) -> Option<String> {
    let mut failures = Vec::new();
    if let Some(msg) = metric_over_budget(
        "wall_clock_ms",
        measured.wall_clock_ms,
        baseline.wall_clock_ms,
        MAX_COST_RATIO,
        RUNTIME_ABS_FLOOR_MS,
    ) {
        failures.push(msg);
    }
    if let Some(msg) = metric_over_budget(
        "peak_rss_bytes",
        measured.peak_rss_bytes,
        baseline.peak_rss_bytes,
        MAX_COST_RATIO,
        PEAK_RSS_ABS_FLOOR_BYTES,
    ) {
        failures.push(msg);
    }
    if failures.is_empty() {
        return None;
    }
    Some(format!(
        "golden cost budget exceeded for `{case_id}`:\n  {}",
        failures.join("\n  ")
    ))
}

fn metric_over_budget(
    metric: &str,
    measured: u64,
    baseline: u64,
    budget: f64,
    abs_floor: u64,
) -> Option<String> {
    if baseline == 0 {
        return Some(format!(
            "{metric}: missing baseline (0 denominator); measured={measured}"
        ));
    }
    let allowed = (baseline as f64 * budget).max(baseline as f64 + abs_floor as f64);
    if (measured as f64) > allowed {
        let ratio = measured as f64 / baseline as f64;
        Some(format!(
            "{metric}: measured={measured} baseline={baseline} ratio={ratio:.4} \
             (budget <= {budget:.2} or +{abs_floor} absolute)"
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_record_round_trips_json() {
        let record = GoldenCostRecord::from_timed_run(TimedRun {
            elapsed_ms: 12,
            peak_rss_bytes: 34,
            peak_rss_delta_bytes: 5,
        });
        let json = record.to_json_line().unwrap();
        let parsed: GoldenCostRecord = serde_json::from_str(json.trim_end()).unwrap();
        assert_eq!(parsed, record);
        assert_eq!(parsed.schema_version, SCHEMA_VERSION);
    }

    #[test]
    fn write_timed_run_creates_sidecar_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("case.cost.json");
        write_timed_run(
            &path,
            TimedRun {
                elapsed_ms: 9,
                peak_rss_bytes: 100,
                peak_rss_delta_bytes: 40,
            },
        )
        .unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        let parsed: GoldenCostRecord = serde_json::from_str(raw.trim_end()).unwrap();
        assert_eq!(parsed.wall_clock_ms, 9);
        assert_eq!(parsed.peak_rss_bytes, 100);
        assert_eq!(parsed.peak_rss_delta_bytes, 40);
    }

    #[test]
    fn budget_passes_within_ratio() {
        let baseline = GoldenCostRecord {
            schema_version: SCHEMA_VERSION.to_string(),
            wall_clock_ms: 1_000,
            peak_rss_bytes: 100_000_000,
            peak_rss_delta_bytes: 50_000_000,
        };
        let measured = GoldenCostRecord {
            schema_version: SCHEMA_VERSION.to_string(),
            wall_clock_ms: 1_100,
            peak_rss_bytes: 110_000_000,
            peak_rss_delta_bytes: 55_000_000,
        };
        assert!(budget_failure("examples/demo/json", &baseline, &measured).is_none());
    }

    #[test]
    fn budget_fails_over_twenty_percent() {
        let baseline = GoldenCostRecord {
            schema_version: SCHEMA_VERSION.to_string(),
            wall_clock_ms: 10_000,
            peak_rss_bytes: 200_000_000,
            peak_rss_delta_bytes: 100_000_000,
        };
        let measured = GoldenCostRecord {
            schema_version: SCHEMA_VERSION.to_string(),
            wall_clock_ms: 13_000,
            peak_rss_bytes: 200_000_000,
            peak_rss_delta_bytes: 100_000_000,
        };
        let failure = budget_failure("examples/demo/json", &baseline, &measured)
            .expect("1.30x wall clock must fail");
        assert!(failure.contains("wall_clock_ms"), "{failure}");
        assert!(failure.contains("examples/demo/json"), "{failure}");
    }

    #[test]
    fn timed_run_measure_feeds_cost_record() {
        // If this stops calling measure::TimedRun, golden cost sidecars lose
        // their only RSS instrumentation path.
        let timing = TimedRun::measure(|| {
            let touch: Vec<u64> = (0..5_000).collect();
            std::hint::black_box(&touch);
        });
        let record = GoldenCostRecord::from_timed_run(timing);
        assert!(record.peak_rss_bytes > 0);
        assert_eq!(record.schema_version, SCHEMA_VERSION);
    }
}
