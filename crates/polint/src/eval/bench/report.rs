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

use crate::eval::bench::curve::{CurvePoint, CurveSeries};

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
/// Warm ms, Peak RSS (MiB), Cache bytes, Store bytes, Budget exceeded. Rows are
/// sorted by `(repo_id, repo_file_count, diff_files)` (the leading fields of the
/// derived `CurvePoint` `Ord`), so the output is deterministic.
pub(crate) fn render_curve_markdown(series: &CurveSeries) -> String {
    let mut sorted = series.clone();
    sorted.sort();

    let mut out = String::new();
    out.push_str("## Benchmark Curves\n\n");
    out.push_str(&format!("Schema: `{}`\n\n", sorted.schema_version));
    out.push_str(
        "| Repo | Files | Source bytes | Diff files | Diff hunk lines | Cold ms | Warm ms | Peak RSS (MiB) | Cache bytes | Store bytes | Budget exceeded |\n",
    );
    out.push_str("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    if sorted.points.is_empty() {
        out.push_str("| _none_ |  |  |  |  |  |  |  |  |  |  |\n");
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
        "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
        escape_cell(&point.repo_id),
        point.repo_file_count,
        point.repo_source_bytes,
        point.diff_files,
        point.diff_hunk_lines,
        point.cold_wall_clock_ms,
        point.warm_wall_clock_ms,
        bytes_to_mib(point.peak_rss_bytes),
        point.size.cache_bytes,
        point.size.store_bytes,
        point.budget.budget_exceeded,
    )
}

fn bytes_to_mib(bytes: u64) -> u64 {
    bytes / (1024 * 1024)
}

fn escape_cell(value: &str) -> String {
    value.replace('|', "\\|")
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
            "Peak RSS",
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
