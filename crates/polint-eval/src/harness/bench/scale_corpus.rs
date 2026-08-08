//! Scale-corpus measurement: LOC + peak RSS + wall-clock on the pinned suites.
//!
//! Uses the existing isolated perf runner (`run_repo_perf_point_isolated`).
//! Not every suite is required to complete `full_pipeline`: suites that OOM or
//! otherwise fail are recorded loudly in the committed artifact with the LOC
//! attempted. After the first hard failure, larger suites are marked
//! `skipped_after_failure` (still with LOC when the checkout is present) rather
//! than silently omitted.
//!
//! Absent checkouts fail the regenerator loudly when `POLINT_REQUIRE_BENCH_CORPUS`
//! is set. The committed artifact under
//! `research/evaluation-harness/baselines/scale-corpus-run.json` is what the
//! retained-bytes-per-LOC CI gate (`retained_bytes`) consumes: successful rows
//! supply peak-RSS/LOC samples; an artifact with no `ok` rows fails that gate
//! closed rather than claiming the memory ceiling is met.

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::eval::bench::runner::run_repo_perf_point_isolated;
use crate::eval::suite::SuiteManifest;

/// Wire schema for the committed scale-corpus measurement artifact.
pub(crate) const SCALE_CORPUS_SCHEMA_VERSION: &str = "polint-scale-corpus-run-0";

/// Relative path of the committed measurement artifact (repo-root relative).
pub(crate) const SCALE_CORPUS_ARTIFACT_REL: &str =
    "research/evaluation-harness/baselines/scale-corpus-run.json";

/// Golden-corpus inventory that lists the scale suite manifests to measure.
const GOLDEN_CORPUS_INPUTS_REL: &str = "tests/golden-corpus/inputs.toml";

/// Documented measurement ceiling: isolated `full_pipeline` perf child. Suites
/// that exceed host memory are published as `oom`, not required to succeed.
const WORKLOAD: &str = "isolated_perf_full_pipeline";

/// Source extensions counted toward published LOC (physical lines).
const LOC_EXTENSIONS: &[&str] = &["go", "ts", "tsx", "js", "jsx", "mjs", "cjs"];

/// Directory name components skipped while counting LOC.
const LOC_SKIP_DIR_NAMES: &[&str] = &[
    ".git",
    "node_modules",
    "vendor",
    "dist",
    "build",
    "target",
    ".next",
    "coverage",
];

/// Outcome of one scale-suite measurement attempt.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ScaleCorpusStatus {
    /// Isolated perf child completed; cost columns are present.
    Ok,
    /// Child was killed by the OS (typically SIGKILL / signal 9) under memory pressure.
    Oom,
    /// Child failed for a non-OOM reason.
    Error,
    /// Not measured because an earlier suite already failed hard.
    SkippedAfterFailure,
}

/// One measured (or loudly failed) scale repository.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ScaleCorpusRepo {
    pub(crate) suite_id: String,
    pub(crate) status: ScaleCorpusStatus,
    /// Manifest `source_commit` as declared (may be an annotated-tag object SHA).
    pub(crate) source_commit: String,
    /// Peeled commit SHA actually checked out (`HEAD`), when available.
    pub(crate) checkout_commit: Option<String>,
    pub(crate) repo_id: String,
    /// Physical lines of Go/TS/JS source under the checkout (see LOC rules).
    pub(crate) loc: u64,
    pub(crate) repo_file_count: Option<u64>,
    pub(crate) repo_source_bytes: Option<u64>,
    pub(crate) peak_rss_bytes: Option<u64>,
    pub(crate) peak_rss_delta_bytes: Option<u64>,
    pub(crate) cold_wall_clock_ms: Option<u64>,
    pub(crate) warm_wall_clock_ms: Option<u64>,
    /// Human-readable failure detail when `status` is not `ok` (never silent).
    pub(crate) failure_detail: Option<String>,
}

/// Documented measurement limits published beside the rows.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ScaleCorpusLimits {
    pub(crate) workload: String,
    pub(crate) note: String,
}

/// Committed scale-corpus run summary.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ScaleCorpusRun {
    pub(crate) schema_version: String,
    /// Reproduction command (never a host-local path).
    pub(crate) command: String,
    pub(crate) limits: ScaleCorpusLimits,
    pub(crate) repos: Vec<ScaleCorpusRepo>,
}

impl ScaleCorpusRun {
    pub(crate) fn validate(&self, expected_suite_ids: &[String]) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema_version == SCALE_CORPUS_SCHEMA_VERSION,
            "unsupported scale-corpus schema_version `{}`; expected `{SCALE_CORPUS_SCHEMA_VERSION}`",
            self.schema_version
        );
        anyhow::ensure!(
            self.limits.workload == WORKLOAD,
            "limits.workload must be `{WORKLOAD}`, got `{}`",
            self.limits.workload
        );
        anyhow::ensure!(
            !self.limits.note.is_empty(),
            "limits.note must be non-empty"
        );
        for repo in &self.repos {
            anyhow::ensure!(
                repo.loc > 0,
                "{}: loc must be published (> 0)",
                repo.suite_id
            );
            anyhow::ensure!(
                repo.source_commit.len() == 40,
                "{}: source_commit must be a full SHA",
                repo.suite_id
            );
            match repo.status {
                ScaleCorpusStatus::Ok => {
                    anyhow::ensure!(
                        repo.peak_rss_bytes.is_some_and(|v| v > 0),
                        "{}: ok rows require peak_rss_bytes > 0",
                        repo.suite_id
                    );
                    anyhow::ensure!(
                        repo.cold_wall_clock_ms.is_some_and(|v| v > 0),
                        "{}: ok rows require cold_wall_clock_ms > 0",
                        repo.suite_id
                    );
                    anyhow::ensure!(
                        repo.failure_detail.is_none(),
                        "{}: ok rows must not carry failure_detail",
                        repo.suite_id
                    );
                }
                ScaleCorpusStatus::Oom
                | ScaleCorpusStatus::Error
                | ScaleCorpusStatus::SkippedAfterFailure => {
                    anyhow::ensure!(
                        repo.failure_detail
                            .as_ref()
                            .is_some_and(|detail| !detail.is_empty()),
                        "{}: non-ok status requires non-empty failure_detail (loud, not silent)",
                        repo.suite_id
                    );
                }
            }
        }
        for expected in expected_suite_ids {
            anyhow::ensure!(
                self.repos.iter().any(|repo| repo.suite_id == *expected),
                "missing suite_id `{expected}` in scale-corpus artifact"
            );
        }
        Ok(())
    }

    pub(crate) fn load(path: &Path, expected_suite_ids: &[String]) -> anyhow::Result<Self> {
        let raw = fs::read_to_string(path)?;
        let run: Self = serde_json::from_str(&raw)?;
        run.validate(expected_suite_ids)?;
        Ok(run)
    }

    pub(crate) fn write(&self, path: &Path, expected_suite_ids: &[String]) -> anyhow::Result<()> {
        self.validate(expected_suite_ids)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(self)?;
        fs::write(path, format!("{body}\n"))?;
        Ok(())
    }
}

pub(crate) fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root is two levels above crates/polint")
        .to_path_buf()
}

fn default_limits() -> ScaleCorpusLimits {
    ScaleCorpusLimits {
        workload: WORKLOAD.to_string(),
        note: "Publishes suites that complete under host memory; OOM/error is a \
               recorded result with LOC attempted. Grafana-scale full_pipeline is not \
               required. Measurement stops after the first hard failure."
            .to_string(),
    }
}

/// Repo-relative suite manifest paths from the golden-corpus inventory.
fn scale_suite_manifest_rels() -> anyhow::Result<Vec<String>> {
    let path = workspace_root().join(GOLDEN_CORPUS_INPUTS_REL);
    let raw = fs::read_to_string(&path)?;
    let value: toml::Value = toml::from_str(&raw)?;
    let schema = value
        .get("schema_version")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    anyhow::ensure!(
        schema == "polint-golden-corpus-inputs-1",
        "unsupported golden-corpus schema in {}: {schema}",
        path.display()
    );
    let manifests = value
        .get("scale_suite_manifests")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            anyhow::anyhow!("{GOLDEN_CORPUS_INPUTS_REL}: missing scale_suite_manifests")
        })?;
    let mut rels = Vec::with_capacity(manifests.len());
    for entry in manifests {
        let rel = entry.as_str().ok_or_else(|| {
            anyhow::anyhow!(
                "{GOLDEN_CORPUS_INPUTS_REL}: scale_suite_manifests entry must be a string"
            )
        })?;
        anyhow::ensure!(!rel.is_empty(), "empty scale_suite_manifests entry");
        rels.push(rel.to_string());
    }
    anyhow::ensure!(
        !rels.is_empty(),
        "{GOLDEN_CORPUS_INPUTS_REL}: scale_suite_manifests must be non-empty"
    );
    Ok(rels)
}

fn load_scale_manifests() -> anyhow::Result<Vec<(String, SuiteManifest)>> {
    let root = workspace_root();
    let mut loaded = Vec::new();
    for rel in scale_suite_manifest_rels()? {
        let path = root.join(&rel);
        let raw = fs::read_to_string(&path)?;
        let manifest: SuiteManifest = toml::from_str(&raw)?;
        manifest.validate()?;
        loaded.push((rel, manifest));
    }
    loaded.sort_by(|left, right| left.1.id.0.cmp(&right.1.id.0));
    Ok(loaded)
}

pub(crate) fn expected_suite_ids() -> anyhow::Result<Vec<String>> {
    Ok(load_scale_manifests()?
        .into_iter()
        .map(|(_, manifest)| manifest.id.0)
        .collect())
}

fn count_loc(repo_root: &Path) -> anyhow::Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![repo_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if LOC_SKIP_DIR_NAMES.iter().any(|skip| *skip == name) {
                    continue;
                }
                stack.push(path);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            let ext = path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !LOC_EXTENSIONS.iter().any(|wanted| *wanted == ext) {
                continue;
            }
            let Ok(bytes) = fs::read(&path) else {
                continue;
            };
            if bytes.is_empty() {
                continue;
            }
            let newlines = bytes.iter().filter(|byte| **byte == b'\n').count() as u64;
            total = total.saturating_add(if bytes.last() == Some(&b'\n') {
                newlines
            } else {
                newlines + 1
            });
        }
    }
    Ok(total)
}

fn git_stdout(repo_root: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(repo_root)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "git {} failed in {}: {}",
        args.join(" "),
        repo_root.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase())
}

fn checkout_commit(repo_root: &Path) -> anyhow::Result<String> {
    git_stdout(repo_root, &["rev-parse", "HEAD"])
}

fn peel_commit(repo_root: &Path, pin: &str) -> anyhow::Result<String> {
    git_stdout(repo_root, &["rev-parse", &format!("{pin}^{{commit}}")])
}

fn looks_like_oom(detail: &str) -> bool {
    let lower = detail.to_ascii_lowercase();
    lower.contains("signal: 9")
        || lower.contains("sigkill")
        || lower.contains("killed")
        || lower.contains("out of memory")
}

/// Repo-relative checkout paths from the inventory manifests that are absent
/// on disk.
pub(crate) fn missing_scale_checkouts() -> anyhow::Result<Vec<String>> {
    let root = workspace_root();
    let mut missing = Vec::new();
    for (_rel, manifest) in load_scale_manifests()? {
        let repo_root = root.join(&manifest.checkout.path);
        if !repo_root.exists() {
            missing.push(manifest.checkout.path);
        }
    }
    Ok(missing)
}

/// Measure inventory scale suites in ascending LOC order. Stops attempting
/// isolated perf after the first OOM/error; remaining suites are recorded as
/// `skipped_after_failure` with LOC still published.
pub(crate) fn measure_scale_corpus() -> anyhow::Result<ScaleCorpusRun> {
    let root = workspace_root();
    let missing = missing_scale_checkouts()?;
    if !missing.is_empty() {
        anyhow::bail!(
            "scale checkout(s) missing: {}; run `make fetch-scale-repos`",
            missing.join(", ")
        );
    }

    let mut prepared = Vec::new();
    for (rel, manifest) in load_scale_manifests()? {
        let source_commit = manifest
            .source_commit
            .clone()
            .ok_or_else(|| anyhow::anyhow!("{rel}: missing source_commit"))?
            .to_ascii_lowercase();
        anyhow::ensure!(
            source_commit.len() == 40,
            "{rel}: source_commit must be a full SHA"
        );
        let repo_root = root.join(&manifest.checkout.path);
        let loc = count_loc(&repo_root)?;
        let suite_id = manifest.id.0.clone();
        anyhow::ensure!(loc > 0, "{suite_id}: LOC count was zero");
        let head = checkout_commit(&repo_root)?;
        let peeled = peel_commit(&repo_root, &source_commit)?;
        anyhow::ensure!(
            head == peeled,
            "{suite_id}: HEAD {head} != peeled pin {peeled} (manifest {source_commit}); \
             run `make fetch-scale-repos`"
        );
        prepared.push((suite_id, source_commit, head, repo_root, loc));
    }
    prepared.sort_by_key(|entry| entry.4);

    let mut repos = Vec::new();
    let mut hard_failure: Option<String> = None;
    for (suite_id, source_commit, head, repo_root, loc) in prepared {
        if let Some(prior) = &hard_failure {
            let detail = format!(
                "skipped after hard failure on `{prior}`; LOC counted but isolated \
                 perf not attempted (stops before larger suites)"
            );
            eprintln!("SKIP-AFTER-FAILURE {suite_id} loc={loc}: {detail}");
            repos.push(ScaleCorpusRepo {
                suite_id,
                status: ScaleCorpusStatus::SkippedAfterFailure,
                source_commit,
                checkout_commit: Some(head),
                repo_id: repo_root
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                loc,
                repo_file_count: None,
                repo_source_bytes: None,
                peak_rss_bytes: None,
                peak_rss_delta_bytes: None,
                cold_wall_clock_ms: None,
                warm_wall_clock_ms: None,
                failure_detail: Some(detail),
            });
            continue;
        }

        eprintln!("measuring {suite_id} @ {head} (loc={loc}) via isolated perf child…");
        match run_repo_perf_point_isolated(&repo_root, None) {
            Ok(point) => {
                eprintln!(
                    "  done {suite_id}: peak_rss_mib={:.1} cold_ms={} warm_ms={}",
                    point.peak_rss_bytes as f64 / (1024.0 * 1024.0),
                    point.cold_wall_clock_ms,
                    point.warm_wall_clock_ms
                );
                repos.push(ScaleCorpusRepo {
                    suite_id,
                    status: ScaleCorpusStatus::Ok,
                    source_commit,
                    checkout_commit: Some(head),
                    repo_id: point.repo_id.clone(),
                    loc,
                    repo_file_count: Some(point.repo_file_count),
                    repo_source_bytes: Some(point.repo_source_bytes),
                    peak_rss_bytes: Some(point.peak_rss_bytes),
                    peak_rss_delta_bytes: Some(point.peak_rss_delta_bytes),
                    cold_wall_clock_ms: Some(point.cold_wall_clock_ms),
                    warm_wall_clock_ms: Some(point.warm_wall_clock_ms),
                    failure_detail: None,
                });
            }
            Err(error) => {
                let detail = format!("{error:#}");
                let status = if looks_like_oom(&detail) {
                    ScaleCorpusStatus::Oom
                } else {
                    ScaleCorpusStatus::Error
                };
                eprintln!("  FAIL {suite_id} status={status:?} loc={loc}: {detail}");
                hard_failure = Some(suite_id.clone());
                repos.push(ScaleCorpusRepo {
                    suite_id,
                    status,
                    source_commit,
                    checkout_commit: Some(head),
                    repo_id: repo_root
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("unknown")
                        .to_string(),
                    loc,
                    repo_file_count: None,
                    repo_source_bytes: None,
                    peak_rss_bytes: None,
                    peak_rss_delta_bytes: None,
                    cold_wall_clock_ms: None,
                    warm_wall_clock_ms: None,
                    failure_detail: Some(detail),
                });
            }
        }
    }

    repos.sort_by(|left, right| left.suite_id.cmp(&right.suite_id));
    let expected = expected_suite_ids()?;
    let run = ScaleCorpusRun {
        schema_version: SCALE_CORPUS_SCHEMA_VERSION.to_string(),
        command: "make scale-corpus-run".to_string(),
        limits: default_limits(),
        repos,
    };
    run.validate(&expected)?;
    Ok(run)
}

mod tests {
    use super::*;

    #[test]
    fn committed_scale_corpus_artifact_loads_and_validates() {
        let expected = expected_suite_ids().expect("inventory manifests must load");
        let path = workspace_root().join(SCALE_CORPUS_ARTIFACT_REL);
        let run = ScaleCorpusRun::load(&path, &expected).unwrap_or_else(|error| {
            panic!(
                "committed scale-corpus artifact at {} must load: {error}\n\
                 regenerate with `make scale-corpus-run`",
                path.display()
            )
        });
        assert_eq!(run.repos.len(), expected.len());
        for suite_id in &expected {
            let repo = run
                .repos
                .iter()
                .find(|repo| repo.suite_id == *suite_id)
                .unwrap_or_else(|| panic!("missing suite {suite_id}"));
            assert!(repo.loc > 0, "{suite_id}: loc must be published");
            match repo.status {
                ScaleCorpusStatus::Ok => {
                    assert!(
                        repo.peak_rss_bytes.is_some_and(|v| v > 0),
                        "{suite_id}: ok requires peak_rss_bytes"
                    );
                }
                ScaleCorpusStatus::Oom
                | ScaleCorpusStatus::Error
                | ScaleCorpusStatus::SkippedAfterFailure => {
                    assert!(
                        repo.failure_detail
                            .as_ref()
                            .is_some_and(|detail| !detail.is_empty()),
                        "{suite_id}: non-ok must be loud"
                    );
                }
            }
        }
        // At least one row must be an honest measurement attempt result (ok or oom/error),
        // not an all-skipped placeholder.
        assert!(
            run.repos.iter().any(|repo| matches!(
                repo.status,
                ScaleCorpusStatus::Ok | ScaleCorpusStatus::Oom | ScaleCorpusStatus::Error
            )),
            "artifact must record at least one measurement attempt (ok/oom/error)"
        );
    }

    #[test]
    fn inventory_declares_exactly_three_ci_scale_suites() {
        let ids = expected_suite_ids().expect("inventory manifests must load");
        assert_eq!(
            ids,
            vec![
                "excalidraw-excalidraw-scale".to_string(),
                "gohugoio-hugo-scale".to_string(),
                "grafana-grafana-scale".to_string(),
            ]
        );
    }

    #[test]
    fn count_loc_counts_physical_lines_and_skips_vendor_trees() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("src/a.go"), "package a\n\nfunc A() {}\n").unwrap();
        fs::write(
            root.join("src/b.ts"),
            "export const x = 1;\nexport const y = 2;\n",
        )
        .unwrap();
        fs::write(
            root.join("node_modules/pkg/ignored.ts"),
            "export const huge = 1;\n".repeat(1000),
        )
        .unwrap();
        assert_eq!(count_loc(root).unwrap(), 3 + 2);
    }

    #[test]
    fn looks_like_oom_detects_sigkill() {
        assert!(looks_like_oom("isolated perf child exited with signal: 9"));
        assert!(looks_like_oom("process Killed by SIGKILL"));
        assert!(!looks_like_oom("compilation failed"));
    }

    #[test]
    fn missing_scale_checkouts_are_skip_or_require_loud() {
        let missing = missing_scale_checkouts().expect("manifests must load");
        if missing.is_empty() {
            eprintln!(
                "scale checkouts present; publish via `make scale-corpus-run` \
                 (POLINT_WRITE_SCALE_CORPUS=1)"
            );
            return;
        }
        let message = format!(
            "SKIP scale_corpus: missing checkout(s) {}; run `make fetch-scale-repos` \
             (set POLINT_REQUIRE_BENCH_CORPUS=1 to fail instead of skip)",
            missing.join(", ")
        );
        if std::env::var_os("POLINT_REQUIRE_BENCH_CORPUS").is_some() {
            panic!("{message}");
        }
        eprintln!("{message}");
    }

    /// Env-gated regenerator. Requires `make fetch-scale-repos` first.
    #[test]
    fn regenerate_scale_corpus_run() {
        if std::env::var_os("POLINT_WRITE_SCALE_CORPUS").is_none() {
            return;
        }
        let missing = missing_scale_checkouts().expect("manifests must load");
        if !missing.is_empty() {
            let message = format!(
                "SKIP regenerate_scale_corpus_run: missing checkout(s) {}; run \
                 `make fetch-scale-repos` (set POLINT_REQUIRE_BENCH_CORPUS=1 to fail \
                 instead of skip)",
                missing.join(", ")
            );
            if std::env::var_os("POLINT_REQUIRE_BENCH_CORPUS").is_some() {
                panic!("{message}");
            }
            eprintln!("{message}");
            return;
        }
        let run = measure_scale_corpus().expect("scale corpus measurement");
        let expected = expected_suite_ids().expect("inventory");
        let path = workspace_root().join(SCALE_CORPUS_ARTIFACT_REL);
        run.write(&path, &expected)
            .expect("write scale-corpus artifact");
        eprintln!("wrote {} ({} repos)", path.display(), run.repos.len());
        for repo in &run.repos {
            eprintln!(
                "  {} status={:?} loc={} peak_rss={:?} cold_ms={:?} detail={:?}",
                repo.suite_id,
                repo.status,
                repo.loc,
                repo.peak_rss_bytes,
                repo.cold_wall_clock_ms,
                repo.failure_detail
            );
        }
    }
}
