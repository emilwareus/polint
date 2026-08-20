//! Retained-bytes-per-LOC CI gate over the committed scale-corpus artifact.
//!
//! Peak RSS from a successful isolated `full_pipeline` scale measurement is the
//! available retained-memory proxy (ASTs are not retained; process RSS bounds
//! the live `AnalysisDb` working set). Bytes/LOC is `peak_rss_bytes.div_ceil(loc)`
//! on each `ok` row. The CI ceiling is an absolute bytes/LOC budget.
//!
//! Suites that OOM, error, or skip are **not** treated as under-ceiling success.
//! The metric is computed only from successful (`ok`) rows. When the artifact
//! has zero successful rows, the gate **fails closed** — it does not Pass and
//! does not invent a bytes/LOC number.

#![cfg(test)]

use crate::eval::bench::gate::{RegressionGateReport, is_blocking};
use crate::eval::bench::scale_corpus::{
    SCALE_CORPUS_ARTIFACT_REL, SCALE_CORPUS_SCHEMA_VERSION, ScaleCorpusLimits, ScaleCorpusRepo,
    ScaleCorpusRun, ScaleCorpusStatus, expected_suite_ids, workspace_root,
};
use crate::eval::gates::{GateCheck, GateVerdict};

/// Absolute retained-bytes-per-LOC ceiling enforced in CI (bytes).
///
/// Locked above the review's ~5.6 KiB/LOC structural estimate so the first
/// successful scale measurements can land without a false Fail, and low enough
/// that a gross memory blow-up on an `ok` row still fails the build.
pub(crate) const RETAINED_BYTES_PER_LOC_CEILING: u64 = 16 * 1024;

/// One successful suite's retained-bytes-per-LOC sample.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RetainedBytesSample {
    pub(crate) suite_id: String,
    pub(crate) loc: u64,
    pub(crate) peak_rss_bytes: u64,
    pub(crate) bytes_per_loc: u64,
}

/// Aggregate over successful scale-corpus rows only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RetainedBytesMetric {
    pub(crate) samples: Vec<RetainedBytesSample>,
    /// Worst-case bytes/LOC among successful samples (`None` when empty).
    pub(crate) max_bytes_per_loc: Option<u64>,
    pub(crate) oom_or_error_count: usize,
    pub(crate) skipped_count: usize,
}

/// Peak RSS ÷ LOC for one successful row. Returns `None` for non-ok rows or
/// missing/zero cost columns (never synthesizes a value from an OOM).
pub(crate) fn retained_bytes_per_loc(repo: &ScaleCorpusRepo) -> Option<RetainedBytesSample> {
    if repo.status != ScaleCorpusStatus::Ok {
        return None;
    }
    let peak_rss_bytes = repo.peak_rss_bytes.filter(|bytes| *bytes > 0)?;
    if repo.loc == 0 {
        return None;
    }
    Some(RetainedBytesSample {
        suite_id: repo.suite_id.clone(),
        loc: repo.loc,
        peak_rss_bytes,
        bytes_per_loc: peak_rss_bytes.div_ceil(repo.loc),
    })
}

/// Collect successful samples only. OOM/error/skip rows contribute to counters
/// but never to `max_bytes_per_loc`.
pub(crate) fn aggregate_retained_bytes(run: &ScaleCorpusRun) -> RetainedBytesMetric {
    let mut samples = Vec::new();
    let mut oom_or_error_count = 0;
    let mut skipped_count = 0;
    for repo in &run.repos {
        match repo.status {
            ScaleCorpusStatus::Ok => {
                if let Some(sample) = retained_bytes_per_loc(repo) {
                    samples.push(sample);
                }
            }
            ScaleCorpusStatus::Oom | ScaleCorpusStatus::Error => {
                oom_or_error_count += 1;
            }
            ScaleCorpusStatus::SkippedAfterFailure => {
                skipped_count += 1;
            }
        }
    }
    let max_bytes_per_loc = samples.iter().map(|sample| sample.bytes_per_loc).max();
    RetainedBytesMetric {
        samples,
        max_bytes_per_loc,
        oom_or_error_count,
        skipped_count,
    }
}

/// Enforce the absolute retained-bytes-per-LOC ceiling against a scale-corpus run.
///
/// Fail closed when there are no successful samples: CI must not claim the
/// budget is met when every measured suite OOM'd or was skipped.
pub(crate) fn evaluate_retained_bytes_ceiling(
    run: &ScaleCorpusRun,
    ceiling_bytes_per_loc: u64,
) -> RegressionGateReport {
    let metric = aggregate_retained_bytes(run);
    let check = match metric.max_bytes_per_loc {
        None => GateCheck {
            metric: "retained_bytes_per_loc".to_string(),
            observed: format!(
                "no successful scale-corpus rows (oom_or_error={}, skipped={}); \
                 cannot certify under ceiling",
                metric.oom_or_error_count, metric.skipped_count
            ),
            threshold: format!("≤ {ceiling_bytes_per_loc} bytes/LOC from ok rows only"),
            verdict: GateVerdict::Fail,
        },
        Some(observed) if observed > ceiling_bytes_per_loc => GateCheck {
            metric: "retained_bytes_per_loc".to_string(),
            observed: format!(
                "{observed} bytes/LOC (max over {} ok row(s); oom_or_error={}, skipped={})",
                metric.samples.len(),
                metric.oom_or_error_count,
                metric.skipped_count
            ),
            threshold: format!("≤ {ceiling_bytes_per_loc} bytes/LOC"),
            verdict: GateVerdict::Fail,
        },
        Some(observed) => GateCheck {
            metric: "retained_bytes_per_loc".to_string(),
            observed: format!(
                "{observed} bytes/LOC (max over {} ok row(s); oom_or_error={}, skipped={})",
                metric.samples.len(),
                metric.oom_or_error_count,
                metric.skipped_count
            ),
            threshold: format!("≤ {ceiling_bytes_per_loc} bytes/LOC"),
            verdict: GateVerdict::Pass,
        },
    };
    RegressionGateReport {
        verdict: check.verdict,
        checks: vec![check],
    }
}

mod tests {
    use super::*;

    fn ok_repo(suite_id: &str, loc: u64, peak_rss_bytes: u64) -> ScaleCorpusRepo {
        ScaleCorpusRepo {
            suite_id: suite_id.to_string(),
            status: ScaleCorpusStatus::Ok,
            source_commit: "a".repeat(40),
            checkout_commit: Some("b".repeat(40)),
            repo_id: suite_id.to_string(),
            loc,
            repo_file_count: None,
            repo_source_bytes: None,
            peak_rss_bytes: Some(peak_rss_bytes),
            peak_rss_delta_bytes: Some(peak_rss_bytes / 2),
            cold_wall_clock_ms: Some(100),
            warm_wall_clock_ms: Some(80),
            failure_detail: None,
        }
    }

    fn oom_repo(suite_id: &str, loc: u64) -> ScaleCorpusRepo {
        ScaleCorpusRepo {
            suite_id: suite_id.to_string(),
            status: ScaleCorpusStatus::Oom,
            source_commit: "c".repeat(40),
            checkout_commit: Some("d".repeat(40)),
            repo_id: suite_id.to_string(),
            loc,
            repo_file_count: None,
            repo_source_bytes: None,
            peak_rss_bytes: None,
            peak_rss_delta_bytes: None,
            cold_wall_clock_ms: None,
            warm_wall_clock_ms: None,
            failure_detail: Some(format!("isolated perf child SIGKILL; LOC attempted={loc}")),
        }
    }

    fn run_with(repos: Vec<ScaleCorpusRepo>) -> ScaleCorpusRun {
        ScaleCorpusRun {
            schema_version: SCALE_CORPUS_SCHEMA_VERSION.to_string(),
            command: "make scale-corpus-run".to_string(),
            limits: ScaleCorpusLimits {
                workload: "isolated_perf_full_pipeline".to_string(),
                note: "test fixture".to_string(),
            },
            repos,
        }
    }

    #[test]
    fn ceiling_constant_is_locked() {
        // Silently loosening the CI memory budget must fail this test.
        assert_eq!(RETAINED_BYTES_PER_LOC_CEILING, 16 * 1024);
    }

    #[test]
    fn ok_row_bytes_per_loc_uses_ceil_division() {
        let repo = ok_repo("tiny", 1000, 5_600_000);
        let sample = retained_bytes_per_loc(&repo).expect("ok row yields sample");
        assert_eq!(sample.bytes_per_loc, 5600);
        let uneven = ok_repo("uneven", 3, 10);
        assert_eq!(
            retained_bytes_per_loc(&uneven).unwrap().bytes_per_loc,
            4,
            "10/3 must ceil to 4 so the metric never under-reports"
        );
    }

    #[test]
    fn oom_row_never_produces_a_sample() {
        let repo = oom_repo("blown", 86_527);
        assert!(retained_bytes_per_loc(&repo).is_none());
    }

    #[test]
    fn aggregate_uses_only_successful_rows() {
        let run = run_with(vec![
            ok_repo("small", 10_000, 40 * 1024 * 1024),
            oom_repo("large", 1_000_000),
        ]);
        let metric = aggregate_retained_bytes(&run);
        assert_eq!(metric.samples.len(), 1);
        assert_eq!(metric.oom_or_error_count, 1);
        assert_eq!(metric.skipped_count, 0);
        assert_eq!(
            metric.max_bytes_per_loc,
            Some((40 * 1024 * 1024u64).div_ceil(10_000))
        );
    }

    #[test]
    fn under_ceiling_ok_rows_pass() {
        let run = run_with(vec![ok_repo(
            "fit",
            10_000,
            10_000 * (RETAINED_BYTES_PER_LOC_CEILING - 1),
        )]);
        let report = evaluate_retained_bytes_ceiling(&run, RETAINED_BYTES_PER_LOC_CEILING);
        assert!(!is_blocking(&report), "{report:#?}");
        assert_eq!(report.verdict, GateVerdict::Pass);
    }

    #[test]
    fn over_ceiling_ok_row_fails() {
        let run = run_with(vec![ok_repo(
            "fat",
            10_000,
            10_000 * (RETAINED_BYTES_PER_LOC_CEILING + 1),
        )]);
        let report = evaluate_retained_bytes_ceiling(&run, RETAINED_BYTES_PER_LOC_CEILING);
        assert!(is_blocking(&report), "{report:#?}");
        assert_eq!(report.verdict, GateVerdict::Fail);
    }

    #[test]
    fn no_successful_rows_fail_closed() {
        let run = run_with(vec![
            oom_repo("excalidraw", 86_527),
            ScaleCorpusRepo {
                suite_id: "hugo".to_string(),
                status: ScaleCorpusStatus::SkippedAfterFailure,
                source_commit: "e".repeat(40),
                checkout_commit: Some("f".repeat(40)),
                repo_id: "hugo".to_string(),
                loc: 198_514,
                repo_file_count: None,
                repo_source_bytes: None,
                peak_rss_bytes: None,
                peak_rss_delta_bytes: None,
                cold_wall_clock_ms: None,
                warm_wall_clock_ms: None,
                failure_detail: Some("skipped after hard failure".to_string()),
            },
        ]);
        let report = evaluate_retained_bytes_ceiling(&run, RETAINED_BYTES_PER_LOC_CEILING);
        assert!(is_blocking(&report), "{report:#?}");
        assert!(
            report.checks.iter().any(|check| {
                check.metric == "retained_bytes_per_loc"
                    && check.verdict == GateVerdict::Fail
                    && check.observed.contains("no successful")
            }),
            "fail-closed must not pretend under-ceiling success: {report:#?}"
        );
    }

    #[test]
    fn committed_scale_corpus_artifact_is_gated() {
        let expected = expected_suite_ids().expect("inventory manifests must load");
        let path = workspace_root().join(SCALE_CORPUS_ARTIFACT_REL);
        let run = ScaleCorpusRun::load(&path, &expected).unwrap_or_else(|error| {
            panic!(
                "committed scale-corpus artifact at {} must load: {error}",
                path.display()
            )
        });
        let report = evaluate_retained_bytes_ceiling(&run, RETAINED_BYTES_PER_LOC_CEILING);
        let metric = aggregate_retained_bytes(&run);

        // Wire check: deleting evaluate_retained_bytes_ceiling from this test
        // leaves the committed artifact ungated. The call above must stay.
        assert!(
            !report.checks.is_empty(),
            "retained-bytes gate must emit at least one check"
        );
        assert_eq!(report.checks[0].metric, "retained_bytes_per_loc");

        match metric.max_bytes_per_loc {
            Some(observed) => {
                assert!(
                    !is_blocking(&report),
                    "successful scale rows must stay ≤ {RETAINED_BYTES_PER_LOC_CEILING} \
                     bytes/LOC; observed={observed}: {report:#?}"
                );
                assert!(
                    observed <= RETAINED_BYTES_PER_LOC_CEILING,
                    "observed {observed} exceeds locked ceiling"
                );
            }
            None => {
                // Current committed artifact: every suite OOM'd or was skipped.
                // Fail closed — CI must not claim the memory ceiling is met.
                assert!(
                    is_blocking(&report),
                    "zero ok rows must fail closed, not Pass: {report:#?}"
                );
                assert!(
                    metric.oom_or_error_count > 0,
                    "empty samples without an OOM/error attempt would be silent skip"
                );
                eprintln!(
                    "retained_bytes_per_loc: FAIL CLOSED — no successful scale-corpus \
                     rows (oom_or_error={}, skipped={}); ceiling {} bytes/LOC stands; \
                     regenerate with `make scale-corpus-run` after a suite completes",
                    metric.oom_or_error_count, metric.skipped_count, RETAINED_BYTES_PER_LOC_CEILING
                );
            }
        }
    }
}
