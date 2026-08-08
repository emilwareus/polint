//! Curve-point telemetry types keyed by repo size and diff size (BENCH-01).
//!
//! These types are the durable measurement contract every later plan
//! consumes: each `CurvePoint` keys a single measurement by repo size
//! (`repo_file_count`, `repo_source_bytes`) and diff size (`diff_files`,
//! `diff_hunk_lines`) so downstream can plot cost curves versus size, and it
//! carries cold/warm wall-clock, real peak RSS, cache/store size, and
//! budget-exhaustion counters as first-class fields. All types are
//! `pub(crate)`; no public/SDK/CLI surface is introduced.

use serde::{Deserialize, Serialize};

/// Wire schema version for a serialized [`CurveSeries`].
pub(crate) const CURVE_SCHEMA_VERSION: &str = "polint-bench-curve-0";

/// Budget-exhaustion counters recorded (not silently swallowed) during a run.
/// These make honest the fact that a measurement may have hit an internal
/// budget/iteration ceiling rather than analyzing everything.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct BudgetExhaustionCounters {
    /// Number of times a solver/analysis budget was exceeded.
    pub(crate) budget_exceeded: u64,
    /// Number of times a token/points-to budget was exhausted.
    pub(crate) tokens_exhausted: u64,
    /// Number of times an iteration/round cap was hit.
    pub(crate) iteration_capped: u64,
}

/// Cache and store size (bytes) observed for a measurement.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct StoreSizeBytes {
    /// In-memory/on-disk layer-cache size in bytes.
    pub(crate) cache_bytes: u64,
    /// Durable semantic-store size in bytes.
    pub(crate) store_bytes: u64,
}

/// A single measurement point keyed by repo size and diff size.
///
/// `Ord` is derived so a `Vec<CurvePoint>` sorts deterministically. Field order
/// is the sort key order: repo identity first, then repo size, then diff size,
/// then the measured costs.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct CurvePoint {
    /// Stable repo/suite identifier this point was measured against.
    pub(crate) repo_id: String,
    /// Repo size: number of analyzed source files.
    pub(crate) repo_file_count: u64,
    /// Repo size: total analyzed source bytes.
    pub(crate) repo_source_bytes: u64,
    /// Diff size: number of changed files (0 for a whole-repo cold measurement).
    pub(crate) diff_files: u64,
    /// Diff size: number of changed hunk lines.
    pub(crate) diff_hunk_lines: u64,
    /// Cold (first-run) wall-clock in milliseconds.
    pub(crate) cold_wall_clock_ms: u64,
    /// Warm (second-run) wall-clock in milliseconds.
    pub(crate) warm_wall_clock_ms: u64,
    /// Real OS peak RSS in bytes (from `getrusage`): the process-wide monotonic
    /// high-water mark. Reporting only — the regression gate compares
    /// `peak_rss_delta_bytes`, which is not confounded by allocations made by
    /// whatever process hosts the measurement.
    pub(crate) peak_rss_bytes: u64,
    /// Run-attributable peak-RSS growth in bytes (the delta above the pre-run
    /// high-water mark). This is the confound-free metric the regression gate
    /// compares. Serde-defaulted so a curve serialized before this field existed
    /// still deserializes.
    #[serde(default)]
    pub(crate) peak_rss_delta_bytes: u64,
    /// Cache/store size for this measurement.
    pub(crate) size: StoreSizeBytes,
    /// Budget-exhaustion counters for this measurement.
    pub(crate) budget: BudgetExhaustionCounters,
}

/// A deterministically-ordered series of [`CurvePoint`]s with a schema version.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct CurveSeries {
    pub(crate) schema_version: String,
    pub(crate) points: Vec<CurvePoint>,
}

impl CurveSeries {
    /// Build an empty series stamped with the current schema version.
    pub(crate) fn new() -> Self {
        Self {
            schema_version: CURVE_SCHEMA_VERSION.to_string(),
            points: Vec::new(),
        }
    }

    /// Sort points into deterministic order (by the derived `CurvePoint` `Ord`).
    pub(crate) fn sort(&mut self) {
        self.points.sort();
    }
}

impl Default for CurveSeries {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_point(repo: &str, files: u64) -> CurvePoint {
        CurvePoint {
            repo_id: repo.to_string(),
            repo_file_count: files,
            repo_source_bytes: files * 1024,
            diff_files: 2,
            diff_hunk_lines: 40,
            cold_wall_clock_ms: 7400,
            warm_wall_clock_ms: 4600,
            peak_rss_bytes: 1_000_000_000,
            peak_rss_delta_bytes: 800_000_000,
            size: StoreSizeBytes {
                cache_bytes: 4096,
                store_bytes: 8192,
            },
            budget: BudgetExhaustionCounters {
                budget_exceeded: 1,
                tokens_exhausted: 0,
                iteration_capped: 3,
            },
        }
    }

    #[test]
    fn curve_series_round_trips_byte_identically_after_sorting() {
        let mut series = CurveSeries::new();
        // Insert out of sorted order.
        series.points.push(sample_point("zeta", 200));
        series.points.push(sample_point("alpha", 100));
        series.points.push(sample_point("alpha", 50));
        series.sort();

        let json = serde_json::to_string(&series).unwrap();
        let mut decoded: CurveSeries = serde_json::from_str(&json).unwrap();
        decoded.sort();
        let reencoded = serde_json::to_string(&decoded).unwrap();

        assert_eq!(
            json, reencoded,
            "sorted CurveSeries must round-trip byte-identically"
        );
        assert_eq!(decoded, series);
        assert_eq!(decoded.schema_version, CURVE_SCHEMA_VERSION);
    }

    #[test]
    fn curve_types_reject_unknown_fields() {
        let raw = r#"{
            "schema_version": "polint-bench-curve-0",
            "points": [],
            "unexpected": true
        }"#;
        assert!(serde_json::from_str::<CurveSeries>(raw).is_err());
    }

    #[test]
    fn curve_points_sort_deterministically() {
        let mut points = [sample_point("beta", 3), sample_point("alpha", 9)];
        points.sort();
        assert_eq!(points[0].repo_id, "alpha");
        assert_eq!(points[1].repo_id, "beta");
    }
}
