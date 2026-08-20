//! Per-golden-case cost records written when a check is asked to measure.
//!
//! The characterization harness sets [`COST_PATH_ENV`] to a sidecar path before
//! invoking `polint check`. The rules-host check path wraps analysis in
//! timed measurement and writes wall-clock + peak RSS so the harness can gate
//! regressions without reimplementing OS RSS capture.
//!
//! Crate-private (not under `runner` / `sdk` / CLI source trees) so the public
//! surface leak gate does not see eval harness path markers in product code.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::measure::TimedRun;

/// Env var naming the JSON file overwritten with one cost record for a check.
pub(crate) const COST_PATH_ENV: &str = "POLINT_GOLDEN_COST_PATH";

const SCHEMA_VERSION: &str = "polint-golden-cost-1";

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
