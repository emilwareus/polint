//! Benchmark sweep entry-point (BENCH-01, success criteria 2 — curves vs size).
//!
//! Iterates the committed CI scale manifests (grafana, hugo, excalidraw),
//! measures one baseline curve-point per present checkout plus a small fixed
//! diff-size sweep (review against a few refs), assembles the results into a
//! single multi-point [`CurveSeries`] keyed by repo size AND diff size, and
//! writes both the machine-readable `benchmark-curves.json` and the human
//! readable `benchmark-report.md`.
//!
//! Absent large-repo checkouts are SKIPPED, not failed, mirroring
//! `eval::external::tests`, so the sweep is runnable in CI without the multi
//! gigabyte clones present. Everything is test-facing (`#[cfg(test)]`) and
//! `pub(crate)`; no public/SDK/CLI surface is introduced. (Plan 03 later
//! appends the persisted-graph accuracy section to `benchmark-report.md`.)

#![cfg(test)]

use std::path::{Path, PathBuf};

use crate::eval::bench::curve::{CurvePoint, CurveSeries};
use crate::eval::bench::runner::run_repo_perf_point;
use crate::eval::suite::SuiteManifest;

/// File names of the committed CI scale manifests the sweep iterates.
const SCALE_MANIFESTS: &[&str] = &[
    "grafana-grafana-scale.toml",
    "gohugoio-hugo-scale.toml",
    "excalidraw-excalidraw-scale.toml",
];

/// The fixed diff-size sweep: each present checkout is measured at the cold/warm
/// baseline (no diff) plus a review measurement against each of these refs. Bad
/// or unreachable refs (e.g. a shallow clone) are tolerated and skipped, so the
/// sweep never fails on a repo whose history is too short.
const REVIEW_REFS: &[&str] = &["HEAD~1", "HEAD~10"];

/// A single repo to sweep: its checkout root and the review refs to measure diff
/// sized points against (in addition to the no-diff baseline point).
struct SweepTarget {
    repo_root: PathBuf,
    review_refs: Vec<String>,
}

/// Run the benchmark sweep over the committed CI scale manifests, writing
/// `benchmark-curves.json` and `benchmark-report.md` under `output_dir` and
/// returning the assembled [`CurveSeries`].
///
/// Manifests whose `checkout.path` does not exist on disk are skipped (not
/// failed), so this is runnable without the large clones present — in that case
/// the returned series is empty but the two artifacts are still written.
pub(crate) fn run_benchmark_sweep(output_dir: &Path) -> anyhow::Result<CurveSeries> {
    let targets = committed_sweep_targets()?;
    run_sweep_with(&targets, output_dir, |root, review_ref| {
        run_repo_perf_point(root, review_ref)
    })
}

/// Resolve the committed scale manifests to [`SweepTarget`]s for the checkouts
/// that exist on disk. Manifests are loaded and validated; a manifest whose
/// `checkout.path` (repo-relative to the workspace root) is absent is skipped.
fn committed_sweep_targets() -> anyhow::Result<Vec<SweepTarget>> {
    let workspace_root = workspace_root();
    let suites_dir = workspace_root.join("research/evaluation-harness/suites");
    let mut targets = Vec::new();
    for file in SCALE_MANIFESTS {
        let manifest_path = suites_dir.join(file);
        let raw = std::fs::read_to_string(&manifest_path)?;
        let manifest: SuiteManifest = toml::from_str(&raw)?;
        manifest.validate()?;
        // checkout.path is repo-relative (repo_relative_only policy); resolve it
        // against the workspace root and skip if the clone is absent.
        let repo_root = workspace_root.join(&manifest.checkout.path);
        if repo_root.exists() {
            targets.push(SweepTarget {
                repo_root,
                review_refs: REVIEW_REFS.iter().map(|r| (*r).to_string()).collect(),
            });
        }
    }
    Ok(targets)
}

/// Core sweep loop, parameterized over the per-point measurement so tests can
/// drive it with a deterministic measurer (isolating the inherently-volatile
/// timing/RSS capture from the emission-determinism assertion).
///
/// For each target it measures one baseline point (no diff) and one point per
/// review ref (varying diff size). A baseline failure skips that repo; a review
/// point failure (e.g. a bad ref) skips just that point.
fn run_sweep_with<M>(
    targets: &[SweepTarget],
    output_dir: &Path,
    mut measure: M,
) -> anyhow::Result<CurveSeries>
where
    M: FnMut(&Path, Option<&str>) -> anyhow::Result<CurvePoint>,
{
    let mut series = CurveSeries::new();
    for target in targets {
        match measure(&target.repo_root, None) {
            Ok(point) => series.points.push(point),
            Err(error) => {
                tracing::warn!(
                    target: "polint::bench",
                    repo = %target.repo_root.display(),
                    %error,
                    "skipping repo whose baseline perf measurement failed"
                );
                continue;
            }
        }
        for review_ref in &target.review_refs {
            match measure(&target.repo_root, Some(review_ref.as_str())) {
                Ok(point) => series.points.push(point),
                Err(error) => tracing::warn!(
                    target: "polint::bench",
                    repo = %target.repo_root.display(),
                    review_ref = %review_ref,
                    %error,
                    "skipping review diff-size point (ref unreachable)"
                ),
            }
        }
    }
    series.sort();

    std::fs::create_dir_all(output_dir)?;
    let json_path = output_dir.join("benchmark-curves.json");
    let markdown_path = output_dir.join("benchmark-report.md");
    crate::eval::bench::report::write_curve_series(&json_path, &series)?;
    // Surface the committed pre-store persisted-graph accuracy baseline in the
    // report when it is present (BENCH-04 "appears in the benchmark report").
    let accuracy = crate::eval::bench::report::load_graph_accuracy_baseline(
        &workspace_root()
            .join("research/evaluation-harness/baselines/persisted-graph-accuracy.json"),
    )
    .ok();
    std::fs::write(
        &markdown_path,
        crate::eval::markdown::render_benchmark_report(&series, accuracy.as_ref()),
    )?;
    Ok(series)
}

/// Workspace root (two levels up from `crates/polint`).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root is two levels above crates/polint")
        .to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::bench::curve::{BudgetExhaustionCounters, StoreSizeBytes};
    use std::process::Command;
    use tempfile::tempdir;

    fn git_available() -> bool {
        Command::new("git")
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git invocation");
        assert!(
            status.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&status.stderr)
        );
    }

    fn write(dir: &Path, rel: &str, contents: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(path, contents).expect("write file");
    }

    /// A deterministic measurer: fixed CurvePoints keyed by (repo dir, diff),
    /// so emission determinism can be asserted without volatile timing/RSS.
    fn fixed_point(repo_root: &Path, review_ref: Option<&str>) -> anyhow::Result<CurvePoint> {
        let repo_id = repo_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "repo".to_string());
        let diff_files = u64::from(review_ref.is_some());
        Ok(CurvePoint {
            repo_id,
            repo_file_count: 42,
            repo_source_bytes: 86_016,
            diff_files,
            diff_hunk_lines: diff_files * 9,
            cold_wall_clock_ms: 1234,
            warm_wall_clock_ms: 567,
            peak_rss_bytes: 5 * 1024 * 1024,
            peak_rss_delta_bytes: 2 * 1024 * 1024,
            size: StoreSizeBytes {
                cache_bytes: 8192,
                store_bytes: 0,
            },
            budget: BudgetExhaustionCounters::default(),
        })
    }

    #[test]
    fn sweep_assembles_multipoint_series_and_writes_deterministic_artifacts() {
        // Two targets, each measured at baseline + one review ref -> 4 points,
        // varying by repo size (distinct repo_id) and diff size (0 vs 1). Uses a
        // deterministic measurer so re-running yields byte-identical JSON.
        let repo_a = tempdir().unwrap();
        let repo_b = tempdir().unwrap();
        let targets = vec![
            SweepTarget {
                repo_root: repo_a.path().join("alpha"),
                review_refs: vec!["HEAD~1".to_string()],
            },
            SweepTarget {
                repo_root: repo_b.path().join("beta"),
                review_refs: vec!["HEAD~1".to_string()],
            },
        ];

        let first_dir = tempdir().unwrap();
        let series = run_sweep_with(&targets, first_dir.path(), fixed_point).unwrap();
        assert!(
            series.points.len() >= 2,
            "sweep must assemble a multi-point series: {series:?}"
        );

        let json_path = first_dir.path().join("benchmark-curves.json");
        let markdown_path = first_dir.path().join("benchmark-report.md");
        let json = std::fs::read(&json_path).unwrap();
        let markdown = std::fs::read_to_string(&markdown_path).unwrap();
        assert!(!json.is_empty(), "benchmark-curves.json must be non-empty");
        assert!(
            markdown.contains("## Benchmark Curves"),
            "benchmark-report.md must render the curve section"
        );
        assert!(markdown.contains("Peak RSS"));

        // Re-running the sweep yields byte-identical benchmark-curves.json.
        let second_dir = tempdir().unwrap();
        run_sweep_with(&targets, second_dir.path(), fixed_point).unwrap();
        let json_again = std::fs::read(second_dir.path().join("benchmark-curves.json")).unwrap();
        assert_eq!(
            json, json_again,
            "re-running the sweep must produce byte-identical benchmark-curves.json"
        );
    }

    #[test]
    fn sweep_over_real_fixture_repo_measures_baseline_and_review_points() {
        if !git_available() {
            eprintln!("skipping real-fixture sweep test; `git` not on PATH");
            return;
        }
        // A real git fixture repo, measured with the real perf runner: baseline
        // (no diff) + one review ref = 2 measured points, without any large clone.
        let repo = tempdir().unwrap();
        let dir = repo.path();
        git(dir, &["init", "--quiet"]);
        git(dir, &["config", "user.email", "t@example.com"]);
        git(dir, &["config", "user.name", "Test"]);
        git(dir, &["config", "commit.gpgsign", "false"]);
        write(
            dir,
            "src/app.go",
            "package app\n\nfunc run() { step() }\n\nfunc step() {}\n",
        );
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "--quiet", "-m", "base"]);
        write(
            dir,
            "src/app.go",
            "package app\n\nfunc run() { step() }\n\nfunc step() { println(1) }\n",
        );
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "--quiet", "-m", "change"]);

        let targets = vec![SweepTarget {
            repo_root: dir.to_path_buf(),
            review_refs: vec!["HEAD~1".to_string()],
        }];
        let output = tempdir().unwrap();
        let series = run_sweep_with(&targets, output.path(), |root, review_ref| {
            run_repo_perf_point(root, review_ref)
        })
        .unwrap();

        assert!(
            series.points.len() >= 2,
            "baseline + one review ref must yield >= 2 measured points: {series:?}"
        );
        assert!(
            series.points.iter().any(|point| point.diff_files > 0),
            "the review point must record a non-zero diff size"
        );
        assert!(
            output.path().join("benchmark-curves.json").exists(),
            "benchmark-curves.json must be written"
        );
        assert!(
            std::fs::metadata(output.path().join("benchmark-report.md"))
                .unwrap()
                .len()
                > 0,
            "benchmark-report.md must be non-empty"
        );
    }

    #[test]
    fn sweep_entry_point_skips_absent_checkouts_without_failing() {
        // The committed scale checkouts are absent in CI; the real entry point
        // must still succeed, write both artifacts, and skip (not fail).
        let output = tempdir().unwrap();
        let series = run_benchmark_sweep(output.path()).unwrap();

        assert!(
            output.path().join("benchmark-curves.json").exists(),
            "benchmark-curves.json is written even when all checkouts are absent"
        );
        let markdown = std::fs::read_to_string(output.path().join("benchmark-report.md")).unwrap();
        assert!(markdown.contains("## Benchmark Curves"));
        // Determinism is inherited from write_curve_series; the point count
        // depends on which (if any) large clones are present on this host.
        let _ = series.points.len();
    }
}
