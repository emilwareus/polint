//! Curve-series JSON emission and markdown benchmark-report rendering (BENCH-01).
//!
//! Turns a measured [`CurveSeries`] into the two durable artifacts the baseline
//! (Plan 03) and regression gates (Plan 04) consume: a byte-stable, machine
//! readable JSON curve file and a human-readable markdown benchmark report whose
//! columns record peak RSS, cold/warm wall-clock, cache/store size, and
//! budget-exhaustion per curve point. Everything is `pub(crate)`; no
//! public/SDK/CLI surface is introduced. The rendering emits only `repo_id` and
//! size/cost fields — never an absolute host path (threat T-63-02-04).

#![cfg(test)]

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::eval::bench::curve::{CurvePoint, CurveSeries};
use crate::eval::report::EvaluationRun;

/// Wire schema version for a serialized [`GraphAccuracyBaseline`].
pub(crate) const GRAPH_ACCURACY_BASELINE_SCHEMA_VERSION: &str = "polint-graph-accuracy-baseline-0";

/// Write `series` to `path` as byte-stable pretty JSON.
///
/// Points are sorted into deterministic order first (by the derived
/// `CurvePoint` `Ord`), so two calls with the same logical content produce
/// byte-identical files. Mirrors the deterministic-JSON discipline of
/// `crate::eval::report::to_deterministic_json_pretty`.
pub(crate) fn write_curve_series(path: &Path, series: &CurveSeries) -> anyhow::Result<()> {
    let mut sorted = series.clone();
    sorted.sort();
    let json = serde_json::to_string_pretty(&sorted)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // A trailing newline keeps the file POSIX-clean and diff-stable.
    std::fs::write(path, format!("{json}\n"))?;
    Ok(())
}

/// Render `series` to a markdown "## Benchmark Curves" table.
///
/// Columns: Repo, Files, Source bytes, Diff files, Diff hunk lines, Cold ms,
/// Warm ms, Peak RSS (MiB), Peak RSS delta (bytes), Cache bytes, Store bytes,
/// Budget exceeded. Rows are sorted by `(repo_id, repo_file_count, diff_files)`
/// (the leading fields of the derived `CurvePoint` `Ord`), so the output is
/// deterministic.
///
/// "Peak RSS (MiB)" is the process-wide absolute high-water mark used by
/// isolated same-host paired gates. "Peak RSS delta (bytes)" is the raw
/// run-attributable `peak_rss_delta_bytes`; it remains visible as informational
/// evidence and can legitimately be zero when startup established the process
/// high-water mark.
pub(crate) fn render_curve_markdown(series: &CurveSeries) -> String {
    let mut sorted = series.clone();
    sorted.sort();

    let mut out = String::new();
    out.push_str("## Benchmark Curves\n\n");
    out.push_str(&format!("Schema: `{}`\n\n", sorted.schema_version));
    out.push_str(
        "| Repo | Files | Source bytes | Diff files | Diff hunk lines | Cold ms | Warm ms | Peak RSS (MiB) | Peak RSS delta (bytes) | Cache bytes | Store bytes | Budget exceeded |\n",
    );
    out.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    if sorted.points.is_empty() {
        out.push_str("| _none_ |  |  |  |  |  |  |  |  |  |  |  |\n");
    } else {
        for point in &sorted.points {
            out.push_str(&row(point));
        }
    }
    out.push('\n');
    out
}

fn row(point: &CurvePoint) -> String {
    format!(
        "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
        escape_cell(&point.repo_id),
        point.repo_file_count,
        point.repo_source_bytes,
        point.diff_files,
        point.diff_hunk_lines,
        point.cold_wall_clock_ms,
        point.warm_wall_clock_ms,
        bytes_to_mib(point.peak_rss_bytes),
        point.peak_rss_delta_bytes,
        point.size.cache_bytes,
        point.size.store_bytes,
        point.budget.budget_exceeded,
    )
}

fn bytes_to_mib(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

fn escape_cell(value: &str) -> String {
    // Neutralize both the column separator and any CR/LF: a `repo_id` derived
    // from a checkout directory name can contain newlines on Unix, which would
    // otherwise break the table structure or inject rows.
    value.replace('|', "\\|").replace(['\n', '\r'], " ")
}

/// One suite's pre-store recall/precision row in a [`GraphAccuracyBaseline`].
///
/// `recall`/`precision` are always emitted (even as `null`) so the row records
/// the metric structurally even when it is unmeasured in an environment lacking
/// the gated Jelly/Go x/tools benchmark clones.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct GraphAccuracyRow {
    pub(crate) suite_id: String,
    pub(crate) suite_commit: Option<String>,
    pub(crate) recall: Option<f64>,
    pub(crate) precision: Option<f64>,
    pub(crate) graph_edges_expected: u64,
    pub(crate) graph_edges_observed: u64,
    pub(crate) unknown_count: u64,
}

/// The committed pre-store persisted-graph recall/precision baseline (BENCH-04).
///
/// This is explicitly the pre-store reference: it records the recall/precision
/// the Jelly micro suite and
/// the Go x/tools RTA suite achieve today, sourced from the existing external
/// adapter runs (scoring is NOT reimplemented here — the rows read
/// `metrics.recall`/`precision`/`graph_edges_*` off the produced
/// [`EvaluationRun`]s). It keeps `polint graph` answers honest (the
/// accuracy-visibility gate) and provides a fixed accuracy reference.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct GraphAccuracyBaseline {
    pub(crate) schema_version: String,
    /// Explicit pre-store label (BENCH-04 acceptance).
    pub(crate) reference: String,
    pub(crate) rows: Vec<GraphAccuracyRow>,
}

impl GraphAccuracyBaseline {
    /// The explicit pre-store label recorded on every committed baseline.
    pub(crate) const PRE_STORE_REFERENCE: &'static str = "pre-store graph reference; recall/precision are sourced from the Jelly + Go x/tools callgraph adapter runs and regenerated with POLINT_WRITE_GRAPH_BENCH when the gated benchmark clones are present";

    /// Build the baseline from the external adapter [`EvaluationRun`]s, reading
    /// recall/precision/edge counts off each run (no scoring reimplemented).
    /// Rows are sorted by `suite_id` for deterministic emission.
    pub(crate) fn from_runs(runs: &[&EvaluationRun]) -> Self {
        let mut rows: Vec<GraphAccuracyRow> = runs
            .iter()
            .map(|run| GraphAccuracyRow {
                suite_id: run.suite_id.clone(),
                suite_commit: run
                    .suite_manifest
                    .as_ref()
                    .and_then(|manifest| manifest.source_commit.clone()),
                recall: run.metrics.recall,
                precision: run.metrics.precision,
                graph_edges_expected: run.metrics.graph_edges_expected,
                graph_edges_observed: run.metrics.graph_edges_observed,
                unknown_count: run.metrics.unknown_count,
            })
            .collect();
        rows.sort_by(|left, right| left.suite_id.cmp(&right.suite_id));
        Self {
            schema_version: GRAPH_ACCURACY_BASELINE_SCHEMA_VERSION.to_string(),
            reference: Self::PRE_STORE_REFERENCE.to_string(),
            rows,
        }
    }

    /// A clone with rows sorted deterministically by `suite_id`.
    fn sorted(&self) -> Self {
        let mut cloned = self.clone();
        cloned
            .rows
            .sort_by(|left, right| left.suite_id.cmp(&right.suite_id));
        cloned
    }
}

/// Write `baseline` to `path` as byte-stable pretty JSON (rows sorted by
/// `suite_id`) with a trailing newline. Two calls with the same logical content
/// produce byte-identical files.
pub(crate) fn write_graph_accuracy_baseline(
    path: &Path,
    baseline: &GraphAccuracyBaseline,
) -> anyhow::Result<()> {
    let sorted = baseline.sorted();
    let json = serde_json::to_string_pretty(&sorted)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, format!("{json}\n"))?;
    Ok(())
}

/// Load a committed [`GraphAccuracyBaseline`].
pub(crate) fn load_graph_accuracy_baseline(path: &Path) -> anyhow::Result<GraphAccuracyBaseline> {
    let raw = std::fs::read_to_string(path)?;
    let baseline: GraphAccuracyBaseline = serde_json::from_str(&raw)?;
    Ok(baseline)
}

/// Render `baseline` to a markdown "## Persisted-Graph Accuracy Baseline"
/// section. Columns: Suite, Commit, Recall, Precision, Edges expected, Edges
/// observed, Unknowns. Rows are sorted by `suite_id` (deterministic).
pub(crate) fn render_graph_accuracy_markdown(baseline: &GraphAccuracyBaseline) -> String {
    let sorted = baseline.sorted();
    let mut out = String::new();
    out.push_str("## Persisted-Graph Accuracy Baseline\n\n");
    out.push_str(&format!("_{}_\n\n", escape_cell(&sorted.reference)));
    out.push_str(
        "| Suite | Commit | Recall | Precision | Edges expected | Edges observed | Unknowns |\n",
    );
    out.push_str("|---|---|---:|---:|---:|---:|---:|\n");
    if sorted.rows.is_empty() {
        out.push_str("| _none_ |  |  |  |  |  |  |\n");
    } else {
        for row in &sorted.rows {
            out.push_str(&format!(
                "| `{}` | `{}` | {} | {} | {} | {} | {} |\n",
                escape_cell(&row.suite_id),
                escape_cell(row.suite_commit.as_deref().unwrap_or("-")),
                metric_cell(row.recall),
                metric_cell(row.precision),
                row.graph_edges_expected,
                row.graph_edges_observed,
                row.unknown_count,
            ));
        }
    }
    out.push('\n');
    out
}

fn metric_cell(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_string(), |value| format!("{value:.4}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::bench::curve::{BudgetExhaustionCounters, StoreSizeBytes};
    use tempfile::tempdir;

    fn point(repo: &str, files: u64, diff_files: u64) -> CurvePoint {
        CurvePoint {
            repo_id: repo.to_string(),
            repo_file_count: files,
            repo_source_bytes: files * 2048,
            diff_files,
            diff_hunk_lines: diff_files * 12,
            cold_wall_clock_ms: 7400,
            warm_wall_clock_ms: 4600,
            peak_rss_bytes: 3 * 1024 * 1024,
            peak_rss_delta_bytes: 1024 * 1024,
            size: StoreSizeBytes {
                cache_bytes: 4096,
                store_bytes: 0,
            },
            budget: BudgetExhaustionCounters {
                budget_exceeded: 2,
                tokens_exhausted: 1,
                iteration_capped: 3,
            },
        }
    }

    fn two_point_series() -> CurveSeries {
        let mut series = CurveSeries::new();
        // Insert out of sorted order to prove deterministic sorting.
        series.points.push(point("zeta", 200, 5));
        series.points.push(point("alpha", 100, 2));
        series
    }

    #[test]
    fn write_curve_series_is_byte_identical_across_calls() {
        let temp = tempdir().unwrap();
        let series = two_point_series();
        let first = temp.path().join("first.json");
        let second = temp.path().join("second.json");

        write_curve_series(&first, &series).unwrap();
        write_curve_series(&second, &series).unwrap();

        let first_bytes = std::fs::read(&first).unwrap();
        let second_bytes = std::fs::read(&second).unwrap();
        assert_eq!(
            first_bytes, second_bytes,
            "two writes of the same series must be byte-identical"
        );

        // And it must round-trip back into an equal (sorted) series.
        let decoded: CurveSeries =
            serde_json::from_slice(&first_bytes).expect("curve JSON round-trips");
        let mut expected = series;
        expected.sort();
        assert_eq!(decoded, expected);
    }

    #[test]
    fn render_curve_markdown_has_required_columns_and_one_row_per_point() {
        let markdown = render_curve_markdown(&two_point_series());

        for header in [
            "Peak RSS (MiB)",
            "Peak RSS delta (bytes)",
            "Cold ms",
            "Warm ms",
            "Diff files",
            "Budget",
            "Cache bytes",
        ] {
            assert!(
                markdown.contains(header),
                "markdown must contain `{header}` column header:\n{markdown}"
            );
        }

        // The raw `peak_rss_delta_bytes` (1 MiB in the fixture point) remains
        // visible without MiB rounding even though portable paired gates use
        // isolated absolute peaks.
        assert!(
            markdown.contains(&(1024 * 1024).to_string()),
            "markdown must render the raw peak-RSS delta bytes:\n{markdown}"
        );

        // Exactly one data row per point (a two-point series -> two data rows).
        let data_rows = markdown
            .lines()
            .filter(|line| line.starts_with("| `"))
            .count();
        assert_eq!(data_rows, 2, "two-point series must render two data rows");

        // Deterministic order: alpha sorts before zeta.
        let alpha = markdown.find("`alpha`").unwrap();
        let zeta = markdown.find("`zeta`").unwrap();
        assert!(alpha < zeta, "rows must be sorted by repo_id");

        // No absolute host paths leak into the report (threat T-63-02-04).
        assert!(!markdown.contains("/Users/"));
        assert!(!markdown.contains("/home/"));
    }
}

#[cfg(test)]
mod graph_accuracy_tests {
    use super::*;
    use crate::eval::model::EvaluationMode;
    use crate::eval::report::{MetricSections, MetricSummary};
    use tempfile::tempdir;

    /// Build a synthetic graph-accuracy [`EvaluationRun`] carrying only the
    /// fields `GraphAccuracyBaseline::from_runs` reads.
    fn graph_run(
        suite_id: &str,
        recall: f64,
        precision: f64,
        expected: u64,
        observed: u64,
        unknowns: u64,
    ) -> EvaluationRun {
        EvaluationRun {
            schema_version: crate::eval::report::EVALUATION_SCHEMA_VERSION.to_string(),
            suite_id: suite_id.to_string(),
            mode: EvaluationMode::PolintBaseline,
            suite_manifest: None,
            cases: Vec::new(),
            metrics: MetricSummary {
                true_positives: observed,
                false_positives: 0,
                false_negatives: expected.saturating_sub(observed),
                true_negatives: 0,
                unconfirmed: 0,
                false_positive_trap_hits: 0,
                forbidden_hits: 0,
                unknown_count: unknowns,
                facts_present: 0,
                facts_accepted: 0,
                facts_rejected: 0,
                graph_edges_expected: expected,
                graph_edges_observed: observed,
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
            performance: None,
            comparison_rows: Vec::new(),
            adaptation: None,
            adaptation_delta: None,
            limitations: Vec::new(),
            output_hash: "graph-accuracy-hash".to_string(),
        }
    }

    #[test]
    fn graph_accuracy_baseline_renders_one_row_per_suite_and_round_trips() {
        let jelly = graph_run("jelly-callgraph-micro", 0.42, 0.90, 120, 50, 7);
        let go = graph_run("go-x-tools-rta-callgraph", 0.80, 0.95, 60, 48, 2);
        // Pass out of sorted order to prove deterministic sorting.
        let baseline = GraphAccuracyBaseline::from_runs(&[&jelly, &go]);

        assert_eq!(baseline.rows.len(), 2);
        assert_eq!(baseline.rows[0].suite_id, "go-x-tools-rta-callgraph");
        assert_eq!(baseline.rows[1].suite_id, "jelly-callgraph-micro");

        let markdown = render_graph_accuracy_markdown(&baseline);
        assert!(markdown.contains("Persisted-Graph Accuracy"));
        assert!(markdown.contains("Recall"));
        assert!(markdown.contains("Precision"));
        let data_rows = markdown
            .lines()
            .filter(|line| line.starts_with("| `"))
            .count();
        assert_eq!(data_rows, 2, "one data row per suite");

        // The committed JSON round-trips byte-identically across two emissions.
        let temp = tempdir().unwrap();
        let first = temp.path().join("a.json");
        let second = temp.path().join("b.json");
        write_graph_accuracy_baseline(&first, &baseline).unwrap();
        write_graph_accuracy_baseline(&second, &baseline).unwrap();
        assert_eq!(
            std::fs::read(&first).unwrap(),
            std::fs::read(&second).unwrap(),
            "two writes of the same baseline must be byte-identical"
        );

        let loaded = load_graph_accuracy_baseline(&first).unwrap();
        assert_eq!(loaded, baseline.sorted());
    }

    #[test]
    fn committed_persisted_graph_accuracy_baseline_has_both_suites() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf();
        let path = root.join("research/evaluation-harness/baselines/persisted-graph-accuracy.json");
        let baseline = load_graph_accuracy_baseline(&path)
            .expect("committed persisted-graph-accuracy.json must load");

        assert_eq!(
            baseline.schema_version,
            GRAPH_ACCURACY_BASELINE_SCHEMA_VERSION
        );
        assert!(
            baseline.reference.contains("pre-store"),
            "committed baseline must carry the explicit pre-store label"
        );
        let suites: Vec<&str> = baseline
            .rows
            .iter()
            .map(|row| row.suite_id.as_str())
            .collect();
        assert!(
            suites.contains(&"jelly-callgraph-micro"),
            "Jelly suite row must be present"
        );
        assert!(
            suites.contains(&"go-x-tools-rta-callgraph"),
            "Go x/tools suite row must be present"
        );

        // Every row must be internally consistent: either fully measured (both
        // recall AND precision present) or a consistent null stub (both absent).
        // A half-measured row (one null, one not) is a corrupt baseline, not a
        // stub. A null stub is only legitimate under the explicit pre-store
        // reference label (asserted above), so a stub can be distinguished from
        // a real reference rather than silently mistaken for one.
        for row in &baseline.rows {
            assert_eq!(
                row.recall.is_none(),
                row.precision.is_none(),
                "row {} must be a consistent stub (both null) or fully measured \
                 (both present), never half-measured",
                row.suite_id
            );
            if row.recall.is_some() {
                // A measured row must carry a positive expected-edge denominator;
                // an all-zero measured row would be a mislabeled stub.
                assert!(
                    row.graph_edges_expected > 0,
                    "measured row {} must record a non-zero graph_edges_expected",
                    row.suite_id
                );
            }
        }

        // recall/precision keys are present in the committed JSON, and no
        // absolute host path leaks (threat T-63-03-03).
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("\"recall\""), "recall field must be present");
        assert!(
            raw.contains("\"precision\""),
            "precision field must be present"
        );
        assert!(!raw.contains("/Users/"));
        assert!(!raw.contains("/home/"));
    }
}
