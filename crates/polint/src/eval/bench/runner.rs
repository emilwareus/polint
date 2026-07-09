//! Whole-repo performance runner (BENCH-01, success criteria 2).
//!
//! Drives a `polint check`-equivalent (and, when a review ref is supplied, the
//! diff-sizing for a `polint review`) over a checked-out repo and captures a
//! single [`CurvePoint`] via the Plan 01 measurement substrate: cold/warm
//! wall-clock and real OS peak RSS from [`measure::cold_then_warm`], the on-disk
//! layer-cache size, and the budget-exhaustion counters folded from the live
//! `AnalysisDb`.
//!
//! Everything here is test-facing (`#[cfg(test)]`): it goes through the
//! capability-gated `AnalysisKernel::run` pipeline (PERF-01 discipline) rather
//! than eagerly reading the whole repo, and it exposes no public/SDK/CLI
//! surface. The per-point measurement is aggregated into a multi-point
//! [`CurveSeries`](crate::eval::bench::curve::CurveSeries) by the sweep
//! entry-point.

#![cfg(test)]

use std::collections::BTreeMap;
use std::path::Path;

use crate::analysis_kernel::{AnalysisKernel, KernelInput, KernelOutput};
use crate::core::AnalysisDb;
use crate::eval::bench::curve::{BudgetExhaustionCounters, CurvePoint, StoreSizeBytes};
use crate::eval::bench::measure;

/// Measure a single [`CurvePoint`] for `repo_root`.
///
/// The measurement:
/// 1. derives repo size (`repo_file_count`, `repo_source_bytes`) from the source
///    set the capability-gated pipeline actually loads — no separate whole-repo
///    eager read;
/// 2. drives the `polint check` equivalent through [`AnalysisKernel::run`],
///    wrapped in [`measure::cold_then_warm`] so the cold and warm wall-clock and
///    the real OS peak RSS are captured;
/// 3. when `review_ref` is `Some`, derives the diff size (`diff_files`,
///    `diff_hunk_lines`) via [`crate::git::changeset_for_ref`] as the
///    review-kind (diff-gated) measurement — the analysis cost the review shares
///    with `check` is what the timing captures, and the diff gate itself is a
///    cheap reporting-layer filter;
/// 4. reads the on-disk `.polint/cache` size into [`StoreSizeBytes`] (the durable
///    store lands in Phase 64, so `store_bytes` is an explicit 0 here);
/// 5. folds budget-exhaustion telemetry from the live `AnalysisDb` (see
///    [`budget_counters`]).
pub(crate) fn run_repo_perf_point(
    repo_root: &Path,
    review_ref: Option<&str>,
) -> anyhow::Result<CurvePoint> {
    // Key the point by the repo directory name only — never an absolute host
    // path (threat T-63-02-04: curve JSON must not leak `/Users/` or `/home/`).
    let repo_id = repo_root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".to_string());

    // Diff size for the review measurement. `changeset_for_ref` passes the ref as
    // a fixed positional arg to the git binary (no shell) — threat T-63-02-01.
    let (diff_files, diff_hunk_lines) = match review_ref {
        Some(target) => {
            let changeset = crate::git::changeset_for_ref(repo_root, target)?;
            let files = changeset.files.len() as u64;
            let hunk_lines: u64 = changeset
                .files
                .iter()
                .flat_map(|file| file.new_line_ranges.iter())
                .map(|(start, end)| u64::from(end.saturating_sub(*start) + 1))
                .sum();
            (files, hunk_lines)
        }
        None => (0, 0),
    };

    // Drive the check-equivalent kernel run twice (cold then warm) and keep the
    // warm run's output for the fact walk. Caching is enabled so the warm run
    // exercises the persistent layer cache and populates `.polint/cache`.
    let mut last: Option<anyhow::Result<KernelOutput>> = None;
    let timing = measure::cold_then_warm(|| {
        last = Some(run_check_kernel(repo_root));
    });
    let output = last.expect("cold_then_warm runs the closure at least once")?;

    // Repo size from the source set the pipeline loaded — not a separate eager
    // whole-repo read (PERF-01 discipline).
    let files = output.db.files();
    let repo_file_count = files.len() as u64;
    let repo_source_bytes: u64 = files.iter().map(|file| file.source.len() as u64).sum();

    let size = StoreSizeBytes {
        cache_bytes: dir_size_bytes(&repo_root.join(".polint").join("cache")),
        // The durable semantic store lands in Phase 64; explicitly 0 until then.
        store_bytes: 0,
    };

    let budget = budget_counters(&output.db);

    Ok(CurvePoint {
        repo_id,
        repo_file_count,
        repo_source_bytes,
        diff_files,
        diff_hunk_lines,
        cold_wall_clock_ms: timing.cold_ms,
        warm_wall_clock_ms: timing.warm_ms,
        peak_rss_bytes: timing.peak_rss_bytes,
        size,
        budget,
    })
}

/// Deterministic digest of the diagnostics a store-disabled (== current)
/// `polint check` produces over `repo_root`.
///
/// This is the diagnostics-parity marker a
/// [`StoreDisabledBaseline`](crate::eval::baseline::StoreDisabledBaseline)
/// records (BENCH-02): the durable store landing in Phase 64 must not change the
/// diagnostics polint emits, so a later run can assert this digest is unchanged.
/// It is the FNV stable-hash over the sorted, canonical-JSON-serialized
/// diagnostics of the check-equivalent kernel run. Clean code (no diagnostics)
/// still yields a stable, non-empty digest (the hash of the empty set).
pub(crate) fn diagnostics_digest_for_repo(repo_root: &Path) -> anyhow::Result<String> {
    let output = run_check_kernel(repo_root)?;
    Ok(digest_diagnostics(&output.diagnostics))
}

fn digest_diagnostics(diagnostics: &[crate::diagnostics::Diagnostic]) -> String {
    let mut rows: Vec<String> = diagnostics
        .iter()
        .map(|diagnostic| serde_json::to_string(diagnostic).unwrap_or_default())
        .collect();
    // Sort so the digest is independent of diagnostic emission order.
    rows.sort();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    crate::cache::stable_hash(&refs)
}

/// Drive one `polint check`-equivalent run through the capability-gated kernel.
///
/// This mirrors `crate::eval::observed::run_kernel_for_repo_for_test`: it loads
/// the repo config, requests the full pipeline (so the deep providers whose
/// budget counters we fold actually run), and enables caching so the warm run
/// of [`measure::cold_then_warm`] exercises the persistent layer cache.
fn run_check_kernel(repo_root: &Path) -> anyhow::Result<KernelOutput> {
    let loaded = crate::config::load_config(repo_root)?;
    let config_digest = crate::cache::keys::config_hash(&loaded);
    let rule_digest = crate::cache::keys::rule_hash(&[], None, &BTreeMap::new());
    let cache = crate::cache::Cache::default_for_repo(repo_root, true);
    let plan = crate::analysis_plan::AnalysisPlan::full_pipeline_for_test();
    AnalysisKernel::run(KernelInput {
        loaded: &loaded,
        cache: &cache,
        config_digest: &config_digest,
        rule_digest: &rule_digest,
        plan: &plan,
        parallel: true,
    })
}

/// Fold budget-exhaustion telemetry out of the live `AnalysisDb`.
///
/// `KernelRunReport` does not expose budget/token/iteration counters; they live
/// as per-fact `*::BudgetExceeded` statuses. This walks the same fact families
/// `analysis_kernel::debug` counts, mapping each reachable budget source to the
/// [`BudgetExhaustionCounters`] field it most directly evidences:
/// - `budget_exceeded`: summary facts/events with `SummaryStatus::BudgetExceeded`
///   (the solver/summary budget ceiling);
/// - `tokens_exhausted`: call targets with `CallTargetStatus::BudgetExceeded`
///   (the token/points-to budget surfaced at call resolution);
/// - `iteration_capped`: abstract-domain observations/events with
///   `DomainStatus::BudgetExceeded` (the domain solver's iteration/round cap).
fn budget_counters(db: &AnalysisDb) -> BudgetExhaustionCounters {
    use crate::analysis::calls::facts::CallTargetStatus;
    use crate::analysis::domains::facts::DomainStatus;
    use crate::analysis::summaries::facts::SummaryStatus;

    let budget_exceeded = db
        .summary_facts()
        .iter()
        .filter(|fact| fact.status == SummaryStatus::BudgetExceeded)
        .count() as u64
        + db.summary_events()
            .iter()
            .filter(|event| event.status == SummaryStatus::BudgetExceeded)
            .count() as u64;

    let tokens_exhausted = db
        .call_targets()
        .iter()
        .filter(|target| target.status == CallTargetStatus::BudgetExceeded)
        .count() as u64;

    let iteration_capped = db
        .abstract_domain_observations()
        .iter()
        .filter(|observation| observation.status == DomainStatus::BudgetExceeded)
        .count() as u64
        + db.abstract_domain_events()
            .iter()
            .filter(|event| event.status == DomainStatus::BudgetExceeded)
            .count() as u64;

    BudgetExhaustionCounters {
        budget_exceeded,
        tokens_exhausted,
        iteration_capped,
    }
}

/// Total size in bytes of all regular files under `dir` (recursive), or 0 if the
/// directory is absent. Symlinks are not followed and directory metadata is not
/// counted, so the result reflects only the cache payload bytes.
fn dir_size_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    let mut total = 0u64;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            total = total.saturating_add(dir_size_bytes(&entry.path()));
        } else if file_type.is_file()
            && let Ok(metadata) = entry.metadata()
        {
            total = total.saturating_add(metadata.len());
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::tempdir;

    /// Returns `true` when `git` is usable; git-dependent tests skip otherwise.
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

    /// Write a small but non-trivial Go + TS repo so the full pipeline does
    /// observable, measurable (> 0 ms) work.
    fn write_tiny_repo(dir: &Path) {
        write(
            dir,
            "src/router.go",
            "package app\n\nfunc handle() { helper() }\n\nfunc helper() { println(1) }\n",
        );
        write(
            dir,
            "src/util.ts",
            "export function add(a: number, b: number): number {\n  return a + b;\n}\n\nexport function twice(n: number): number {\n  return add(n, n);\n}\n",
        );
    }

    #[test]
    fn run_repo_perf_point_measures_tiny_repo() {
        let repo = tempdir().unwrap();
        write_tiny_repo(repo.path());

        let point = run_repo_perf_point(repo.path(), None).unwrap();

        assert!(
            point.cold_wall_clock_ms > 0,
            "cold wall-clock must be measurable and > 0: {point:?}"
        );
        assert!(
            point.peak_rss_bytes > 0,
            "peak RSS must be measurable and > 0: {point:?}"
        );
        assert!(
            point.repo_file_count > 0,
            "repo file count must be > 0: {point:?}"
        );
        assert!(point.repo_source_bytes > 0, "repo source bytes must be > 0");
        // No review ref -> no diff.
        assert_eq!(point.diff_files, 0);
        assert_eq!(point.diff_hunk_lines, 0);
        // repo_id is the directory name, never an absolute host path.
        assert!(!point.repo_id.contains('/'));
        assert!(!point.repo_id.starts_with("/Users"));
    }

    #[test]
    fn run_repo_perf_point_captures_review_diff_size() {
        if !git_available() {
            eprintln!("skipping review diff-size test; `git` not on PATH");
            return;
        }
        let repo = tempdir().unwrap();
        let dir = repo.path();
        git(dir, &["init", "--quiet"]);
        git(dir, &["config", "user.email", "t@example.com"]);
        git(dir, &["config", "user.name", "Test"]);
        git(dir, &["config", "commit.gpgsign", "false"]);

        write_tiny_repo(dir);
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "--quiet", "-m", "base"]);
        let base = {
            let out = Command::new("git")
                .current_dir(dir)
                .args(["rev-parse", "HEAD"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };

        // Introduce a diff on the working side.
        write(
            dir,
            "src/util.ts",
            "export function add(a: number, b: number): number {\n  return a + b + 0;\n}\n\nexport function twice(n: number): number {\n  return add(n, n);\n}\n",
        );
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "--quiet", "-m", "change"]);

        let point = run_repo_perf_point(dir, Some(&base)).unwrap();

        assert!(
            point.diff_files > 0,
            "review diff must record at least one changed file: {point:?}"
        );
        assert!(
            point.diff_hunk_lines > 0,
            "review diff must record changed hunk lines: {point:?}"
        );
        assert!(point.cold_wall_clock_ms > 0);
        assert!(point.peak_rss_bytes > 0);
    }
}
