//! Whole-repo performance runner.
//!
//! Drives a `polint check`-equivalent (and, when a review ref is supplied, the
//! diff-sizing for a `polint review`) over a checked-out repo and captures a
//! single [`CurvePoint`] via the shared measurement substrate: independently
//! timed cold/warm wall-clock and real OS peak RSS, the on-disk layer-cache
//! size, and the budget-exhaustion counters folded from the live `AnalysisDb`.
//!
//! Everything here is test-facing (`#[cfg(test)]`): it goes through the
//! capability-gated `AnalysisKernel::run` pipeline rather
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
///    with independently owned timed outputs so the cold database can be
///    released before the warm run starts;
/// 3. when `review_ref` is `Some`, derives the diff size (`diff_files`,
///    `diff_hunk_lines`) via [`crate::git::changeset_for_ref`] as the
///    review-kind (diff-gated) measurement — the analysis cost the review shares
///    with `check` is what the timing captures, and the diff gate itself is a
///    cheap reporting-layer filter;
/// 4. reads layer-cache and semantic-store sizes into [`StoreSizeBytes`]
///    without double-counting store bytes;
/// 5. folds budget-exhaustion telemetry from the live `AnalysisDb` (see
///    [`budget_counters`]).
pub(crate) fn run_repo_perf_point(
    repo_root: &Path,
    review_ref: Option<&str>,
) -> anyhow::Result<CurvePoint> {
    run_repo_perf_point_with_store_mode(repo_root, review_ref, SemanticStoreBenchMode::Disabled)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SemanticStoreBenchMode {
    Disabled,
    Enabled,
}

pub(crate) const STORE_DISABLED_FIXTURE_REPO_ID: &str = "polint-tiny-fixture";
pub(crate) const STORE_DISABLED_FIXTURE_VERSION: &str = "polint-tiny-fixture-1";
pub(crate) const STORE_DISABLED_FIXTURE_CHECK_SUITE_ID: &str = "polint-tiny-fixture-check";
pub(crate) const STORE_DISABLED_FIXTURE_REVIEW_SUITE_ID: &str = "polint-tiny-fixture-review";
pub(crate) const STORE_DISABLED_FIXTURE_FILE_COUNT: u64 = 2;
pub(crate) const STORE_DISABLED_FIXTURE_SOURCE_BYTES: u64 = 144;

const STORE_DISABLED_FIXTURE_GO_SOURCE: &[u8] =
    b"package app\n\nfunc handle() { helper() }\n\nfunc helper() { println(1) }\n";
const STORE_DISABLED_FIXTURE_TS_BASE_SOURCE: &[u8] =
    b"export function add(a: number, b: number): number {\n  return a + b;\n}\n";
const STORE_DISABLED_FIXTURE_TS_FINAL_SOURCE: &[u8] =
    b"export function add(a: number, b: number): number {\n  return a + b + 0;\n}\n";

pub(crate) fn store_disabled_fixture_digest() -> String {
    let go = std::str::from_utf8(STORE_DISABLED_FIXTURE_GO_SOURCE)
        .expect("committed fixture Go source is UTF-8");
    let ts_base = std::str::from_utf8(STORE_DISABLED_FIXTURE_TS_BASE_SOURCE)
        .expect("committed fixture base TypeScript source is UTF-8");
    let ts_final = std::str::from_utf8(STORE_DISABLED_FIXTURE_TS_FINAL_SOURCE)
        .expect("committed fixture final TypeScript source is UTF-8");
    crate::cache::stable_hash(&[STORE_DISABLED_FIXTURE_VERSION, go, ts_base, ts_final])
}

/// Reconstruct the exact two-source fixture used to record the committed
/// store-disabled check/review baselines and return its base commit.
///
/// Writing byte strings and disabling Git's automatic CRLF conversion keeps
/// the committed LF source shape identical on every CI platform. Both the
/// baseline regenerator and store-boundary gates use this helper so the
/// fixed reference cannot silently drift away from its measured workload.
pub(crate) fn write_store_disabled_fixture(repo_root: &Path) -> anyhow::Result<String> {
    let git = |args: &[&str]| -> anyhow::Result<std::process::Output> {
        let output = std::process::Command::new("git")
            .current_dir(repo_root)
            .args(args)
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(output)
    };
    let write_lf = |path: &Path, source: &[u8]| -> anyhow::Result<()> {
        anyhow::ensure!(
            !source.contains(&b'\r'),
            "store-disabled fixture source must use LF line endings"
        );
        std::fs::write(path, source)?;
        anyhow::ensure!(
            std::fs::read(path)? == source,
            "store-disabled fixture source changed while writing {}",
            path.display()
        );
        Ok(())
    };

    git(&["init", "--quiet"])?;
    git(&["config", "user.email", "t@example.com"])?;
    git(&["config", "user.name", "Test"])?;
    git(&["config", "commit.gpgsign", "false"])?;
    git(&["config", "core.autocrlf", "false"])?;
    std::fs::create_dir_all(repo_root.join("src"))?;
    write_lf(
        &repo_root.join("src/router.go"),
        STORE_DISABLED_FIXTURE_GO_SOURCE,
    )?;
    write_lf(
        &repo_root.join("src/util.ts"),
        STORE_DISABLED_FIXTURE_TS_BASE_SOURCE,
    )?;
    git(&["add", "-A"])?;
    git(&["commit", "--quiet", "-m", "base"])?;
    let base = git(&["rev-parse", "HEAD"])?;
    let base = String::from_utf8(base.stdout)
        .map_err(|error| anyhow::anyhow!("base commit was not UTF-8: {error}"))?
        .trim()
        .to_string();
    anyhow::ensure!(!base.is_empty(), "fixture base commit was empty");

    write_lf(
        &repo_root.join("src/util.ts"),
        STORE_DISABLED_FIXTURE_TS_FINAL_SOURCE,
    )?;
    git(&["add", "-A"])?;
    git(&["commit", "--quiet", "-m", "change"])?;

    let source_bytes =
        STORE_DISABLED_FIXTURE_GO_SOURCE.len() + STORE_DISABLED_FIXTURE_TS_FINAL_SOURCE.len();
    anyhow::ensure!(
        u64::try_from(source_bytes).unwrap_or(u64::MAX) == STORE_DISABLED_FIXTURE_SOURCE_BYTES,
        "store-disabled fixture source-byte contract drifted"
    );
    Ok(base)
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub(crate) struct PerfMeasurement {
    pub(crate) point: CurvePoint,
    pub(crate) cold_provider_evidence: ProviderRunEvidence,
    pub(crate) warm_provider_evidence: ProviderRunEvidence,
    pub(crate) cold_diagnostics_digest: String,
    pub(crate) warm_diagnostics_digest: String,
    pub(crate) semantic_store: Option<SemanticStoreRunEvidence>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) struct SemanticStoreRunEvidence {
    pub(crate) cold_ready: bool,
    pub(crate) warm_ready: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) struct ProviderRunEvidence {
    pub(crate) go_semantic_facts: Vec<(String, u64)>,
    pub(crate) provider_outputs: Vec<ProviderOutputEvidence>,
    pub(crate) requested_capabilities: Vec<RequestedCapabilityEvidence>,
    pub(crate) effective_capabilities: Vec<EffectiveCapabilityEvidence>,
    pub(crate) provider_diagnostics: Vec<ProviderDiagnosticEvidence>,
    pub(crate) capability_diagnostic_count: u64,
}

impl ProviderRunEvidence {
    pub(crate) fn go_semantic_fact_count(&self) -> u64 {
        self.go_semantic_facts.iter().map(|(_, count)| count).sum()
    }

    pub(crate) fn availability_projection(&self) -> ProviderAvailabilityEvidence {
        let mut provider_diagnostic_counts = BTreeMap::<String, u64>::new();
        for diagnostic in &self.provider_diagnostics {
            *provider_diagnostic_counts
                .entry(diagnostic.rule_id.clone())
                .or_default() += 1;
        }
        ProviderAvailabilityEvidence {
            go_semantic_facts: self.go_semantic_facts.clone(),
            provider_validations: self
                .provider_outputs
                .iter()
                .map(|provider| (provider.provider_id.clone(), provider.validation.clone()))
                .collect(),
            requested_capabilities: self.requested_capabilities.clone(),
            effective_capabilities: self.effective_capabilities.clone(),
            provider_diagnostic_counts: provider_diagnostic_counts.into_iter().collect(),
            capability_diagnostic_count: self.capability_diagnostic_count,
        }
    }
}

/// Clone- and platform-invariant provider availability evidence.
///
/// Provider output digests remain in [`ProviderRunEvidence`] for cold/warm
/// identity checks within one repository. They are intentionally excluded here
/// because workspace and toolchain identity can legitimately differ between
/// independently cloned fixtures or operating systems. The boundary gate locks
/// identical source shape/content before measurement and verifies the complete
/// stable fact digest after publication. The enabled post-run database no
/// longer owns fact metadata, so constructing a second path-normalized row
/// digest here would require adding a full scan inside the timed kernel path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProviderAvailabilityEvidence {
    pub(crate) go_semantic_facts: Vec<(String, u64)>,
    pub(crate) provider_validations: Vec<(String, String)>,
    pub(crate) requested_capabilities: Vec<RequestedCapabilityEvidence>,
    pub(crate) effective_capabilities: Vec<EffectiveCapabilityEvidence>,
    pub(crate) provider_diagnostic_counts: Vec<(String, u64)>,
    pub(crate) capability_diagnostic_count: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) struct ProviderOutputEvidence {
    pub(crate) provider_id: String,
    pub(crate) output_digest: String,
    pub(crate) validation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) struct RequestedCapabilityEvidence {
    pub(crate) capability: String,
    pub(crate) language: Option<String>,
    pub(crate) support_status: String,
    pub(crate) setup_status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) struct EffectiveCapabilityEvidence {
    pub(crate) capability: String,
    pub(crate) language: Option<String>,
    pub(crate) support_status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub(crate) struct ProviderDiagnosticEvidence {
    pub(crate) rule_id: String,
    pub(crate) message: String,
}

/// Compact proof retained between independently measured kernel runs.
///
/// A complete [`KernelOutput`] owns the analysis database and its source/fact
/// storage. Only this projection survives from cold to warm, keeping the warm
/// peak-RSS measurement representative of one live database.
#[derive(Clone, Debug, PartialEq, Eq)]
struct KernelRunEvidence {
    provider: ProviderRunEvidence,
    diagnostics_digest: String,
    store_ready: bool,
}

fn kernel_run_evidence(output: &KernelOutput) -> KernelRunEvidence {
    KernelRunEvidence {
        provider: provider_run_evidence(output),
        diagnostics_digest: digest_diagnostics(&output.diagnostics),
        store_ready: output.run_report.store_status()
            == &crate::analysis_kernel::StoreStatus::Ready,
    }
}

/// Measure cold and warm runs while retaining only compact evidence between
/// them. Cold errors stop before warm execution; warm errors remain independent.
fn measure_cold_then_warm_outputs<T, E>(
    mut run: impl FnMut() -> anyhow::Result<T>,
    mut project_evidence: impl FnMut(&T) -> E,
) -> anyhow::Result<(measure::ColdWarm, E, E, T)> {
    let (cold_timing, cold_result) = measure::measure_output(&mut run);
    let cold_output = cold_result?;
    let cold_evidence = project_evidence(&cold_output);
    drop(cold_output);

    let (warm_timing, warm_result) = measure::measure_output(&mut run);
    let warm_output = warm_result?;
    let warm_evidence = project_evidence(&warm_output);
    let timing = measure::TimedRun::cold_then_warm(cold_timing, warm_timing);
    Ok((timing, cold_evidence, warm_evidence, warm_output))
}

fn run_repo_perf_point_with_store_mode(
    repo_root: &Path,
    review_ref: Option<&str>,
    store_mode: SemanticStoreBenchMode,
) -> anyhow::Result<CurvePoint> {
    Ok(run_repo_perf_measurement_with_store_mode(repo_root, review_ref, store_mode)?.point)
}

fn run_repo_perf_measurement_with_store_mode(
    repo_root: &Path,
    review_ref: Option<&str>,
    store_mode: SemanticStoreBenchMode,
) -> anyhow::Result<PerfMeasurement> {
    require_repo_local_perf_cache()?;
    // Key the point by the repo directory name only; curve JSON must never leak
    // an absolute host path such as `/Users/` or `/home/`.
    let repo_id = repo_root
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".to_string());

    // `changeset_for_ref` passes the review ref as a fixed positional argument
    // to the git binary without invoking a shell.
    let (diff_files, diff_hunk_lines) = match review_ref {
        Some(target) => {
            let changeset = crate::git::changeset_for_ref(repo_root, target)?;
            let files = changeset.files.len() as u64;
            let hunk_lines: u64 = changeset
                .files
                .iter()
                .flat_map(|file| file.new_line_ranges.iter())
                // Count inclusive, non-empty ranges only: a degenerate or
                // inverted `end < start` entry contributes nothing. Widen to
                // u64 *before* the `+ 1` so a `u32::MAX`-wide range cannot
                // overflow-panic in a debug build.
                .filter(|(start, end)| end >= start)
                .map(|(start, end)| u64::from(end.saturating_sub(*start)) + 1)
                .sum();
            (files, hunk_lines)
        }
        None => (0, 0),
    };

    // Build compact proof evidence outside each timed kernel run. The complete
    // cold output is dropped before warm measurement, so peak RSS never includes
    // two simultaneously live analysis databases. Caching remains enabled, so
    // on-disk cache state still carries the intended cold-to-warm transition.
    let (timing, cold_evidence, warm_evidence, output) = measure_cold_then_warm_outputs(
        || run_check_kernel_with_store_mode(repo_root, store_mode),
        kernel_run_evidence,
    )?;
    let semantic_store =
        (store_mode == SemanticStoreBenchMode::Enabled).then_some(SemanticStoreRunEvidence {
            cold_ready: cold_evidence.store_ready,
            warm_ready: warm_evidence.store_ready,
        });

    // Derive repo size from the source set the pipeline loaded instead of a
    // separate eager whole-repo read.
    let files = output.db.files();
    let repo_file_count = files.len() as u64;
    let repo_source_bytes: u64 = files.iter().map(|file| file.source.len() as u64).sum();

    let cache_layout = crate::cache::CacheLayout::for_repo(repo_root);
    let semantic_store_bytes = dir_size_bytes(&cache_layout.semantic_store_dir())?;
    let cache_bytes = dir_size_bytes(cache_layout.root())?;
    let size = StoreSizeBytes {
        cache_bytes: checked_non_store_cache_bytes(cache_bytes, semantic_store_bytes)?,
        store_bytes: if store_mode == SemanticStoreBenchMode::Enabled {
            semantic_store_bytes
        } else {
            0
        },
    };

    let budget = budget_counters(&output.db);

    Ok(PerfMeasurement {
        point: CurvePoint {
            repo_id,
            repo_file_count,
            repo_source_bytes,
            diff_files,
            diff_hunk_lines,
            cold_wall_clock_ms: timing.cold_ms,
            warm_wall_clock_ms: timing.warm_ms,
            peak_rss_bytes: timing.peak_rss_bytes,
            peak_rss_delta_bytes: timing.peak_rss_delta_bytes,
            size,
            budget,
        },
        cold_provider_evidence: cold_evidence.provider,
        warm_provider_evidence: warm_evidence.provider,
        cold_diagnostics_digest: cold_evidence.diagnostics_digest,
        warm_diagnostics_digest: warm_evidence.diagnostics_digest,
        semantic_store,
    })
}

fn checked_non_store_cache_bytes(cache_bytes: u64, store_bytes: u64) -> anyhow::Result<u64> {
    cache_bytes.checked_sub(store_bytes).ok_or_else(|| {
        anyhow::anyhow!(
            "semantic-store bytes exceeded the containing cache size while collecting benchmark evidence"
        )
    })
}

fn provider_run_evidence(output: &KernelOutput) -> ProviderRunEvidence {
    use crate::core::CapabilitySupportStatus;

    let db = &output.db;
    let go_semantic_facts = vec![
        (
            "address_taken".to_string(),
            db.go_semantic_address_taken().len() as u64,
        ),
        (
            "callsites".to_string(),
            db.go_semantic_callsites().len() as u64,
        ),
        (
            "dynamic_dispatch".to_string(),
            db.go_semantic_dynamic_dispatch().len() as u64,
        ),
        (
            "functions".to_string(),
            db.go_semantic_functions().len() as u64,
        ),
        (
            "instantiated_types".to_string(),
            db.go_semantic_instantiated_types().len() as u64,
        ),
        (
            "method_sets".to_string(),
            db.go_semantic_method_sets().len() as u64,
        ),
        (
            "package_errors".to_string(),
            db.go_semantic_package_errors().len() as u64,
        ),
        (
            "packages".to_string(),
            db.go_semantic_packages().len() as u64,
        ),
        (
            "rta_edges".to_string(),
            db.go_semantic_rta_edges().len() as u64,
        ),
    ];
    let provider_outputs = output
        .run_report
        .provider_outputs
        .iter()
        .map(|provider| ProviderOutputEvidence {
            provider_id: provider.provider_id.clone(),
            output_digest: provider.output_digest.to_string(),
            validation: provider.validation.label().to_string(),
        })
        .collect();
    let requested_capabilities = output
        .run_report
        .input_snapshot
        .requested_capabilities
        .iter()
        .map(|capability| RequestedCapabilityEvidence {
            capability: capability.capability.clone(),
            language: capability
                .language
                .map(|language| language.label().to_string()),
            support_status: capability_support_status_label(&capability.support_status).to_string(),
            setup_status: capability.setup_status.label().to_string(),
        })
        .collect();
    let effective_capabilities = output
        .capability_support
        .entries()
        .iter()
        .map(|capability| EffectiveCapabilityEvidence {
            capability: capability.capability.clone(),
            language: capability
                .language
                .map(|language| language.label().to_string()),
            support_status: capability_support_status_label(&capability.status).to_string(),
        })
        .collect();
    let provider_diagnostics = output
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.rule_id.as_str(),
                "polint/go-semantic" | "polint/internal" | "polint/capability"
            )
        })
        .map(|diagnostic| ProviderDiagnosticEvidence {
            rule_id: diagnostic.rule_id.clone(),
            message: diagnostic.message.clone(),
        })
        .collect();
    let capability_diagnostic_count = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.rule_id == "polint/capability")
        .count() as u64;

    fn capability_support_status_label(status: &CapabilitySupportStatus) -> &'static str {
        match status {
            CapabilitySupportStatus::Supported => "supported",
            CapabilitySupportStatus::Unsupported => "unsupported",
            CapabilitySupportStatus::SetupMissing => "setup_missing",
        }
    }

    ProviderRunEvidence {
        go_semantic_facts,
        provider_outputs,
        requested_capabilities,
        effective_capabilities,
        provider_diagnostics,
        capability_diagnostic_count,
    }
}

/// Full libtest path of the child measurement entry [`tests::perf_child_measure_entry`].
/// The isolated point and measurement entry points re-invoke this binary
/// filtered to exactly this test; keep it in sync with the module path if the
/// test moves.
const CHILD_MEASURE_TEST: &str = "eval::bench::runner::tests::perf_child_measure_entry";
/// Env var carrying the repo path the child measurement entry should measure.
/// Presence of this var is what switches the child entry from a no-op into a
/// measuring run.
const CHILD_REPO_ENV: &str = "POLINT_PERF_CHILD_REPO";
/// Env var carrying the optional review ref (empty/absent == a `check` point).
const CHILD_REVIEW_ENV: &str = "POLINT_PERF_CHILD_REVIEW_REF";
/// Internal libtest-only mode selector, not a supported CLI/config contract.
const CHILD_SEMANTIC_STORE_ENV: &str = "POLINT_PERF_CHILD_SEMANTIC_STORE";
/// Stdout markers the child prints the serialized [`PerfMeasurement`] JSON
/// between, so the parent can extract it from libtest's own `--nocapture`
/// output.
const CHILD_POINT_BEGIN: &str = "<<<POLINT_PERF_POINT_BEGIN>>>";
const CHILD_POINT_END: &str = "<<<POLINT_PERF_POINT_END>>>";

/// Measure a single [`CurvePoint`] for `repo_root` in a dedicated child process
/// so its peak-memory fields are independent of work previously performed by
/// the parent test process.
///
/// `peak_rss_bytes` comes from the platform process peak-memory API
/// (`getrusage` on Unix and `K32GetProcessMemoryInfo` on Windows). It is a
/// process-global, monotonic, whole-lifetime high-water mark. When several
/// measurements share one process, the mark can saturate on an earlier run and
/// make later deltas collapse to allocator jitter. That makes the result depend
/// on process warm-up order rather than the measured workload.
///
/// Re-executing the measurement in a fresh child gives each run an independent
/// process high-water mark. The raw
/// [`ColdWarm::peak_rss_delta_bytes`](measure::ColdWarm) records only growth
/// beyond that child's pre-run high-water mark, so it can legitimately be zero
/// when libtest startup established a higher earlier peak. Same-host paired
/// gates therefore compare `peak_rss_bytes` from identical isolated children;
/// the raw delta remains useful evidence and committed references can still use
/// it when their measurement contexts match. The child is a private snapshot
/// of this test binary filtered to
/// [`tests::perf_child_measure_entry`], which records a [`PerfMeasurement`] and
/// prints it as JSON between [`CHILD_POINT_BEGIN`]/[`CHILD_POINT_END`] on
/// stdout, then exits. Capturing the executable once lets a multi-sample gate
/// reuse identical code even if another Cargo process replaces `current_exe`
/// while the parent test is still running. This wrapper returns only its [`CurvePoint`];
/// [`run_repo_perf_measurement_isolated_with_store_mode`] retains the child
/// evidence as well.
///
/// The committed store-disabled baselines are regenerated through this runner; a
/// measured run fed to `evaluate_regression_budget` MUST use the same
/// isolation for its raw peak-RSS delta to remain comparable.
pub(crate) fn run_repo_perf_point_isolated(
    repo_root: &Path,
    review_ref: Option<&str>,
) -> anyhow::Result<CurvePoint> {
    run_repo_perf_point_isolated_with_store_mode(
        repo_root,
        review_ref,
        SemanticStoreBenchMode::Disabled,
    )
}

pub(crate) fn run_repo_perf_point_isolated_with_store_mode(
    repo_root: &Path,
    review_ref: Option<&str>,
    store_mode: SemanticStoreBenchMode,
) -> anyhow::Result<CurvePoint> {
    Ok(
        run_repo_perf_measurement_isolated_with_store_mode(repo_root, review_ref, store_mode)?
            .point,
    )
}

pub(crate) fn run_repo_perf_measurement_isolated_with_store_mode(
    repo_root: &Path,
    review_ref: Option<&str>,
    store_mode: SemanticStoreBenchMode,
) -> anyhow::Result<PerfMeasurement> {
    IsolatedPerfRunner::capture()?.run_measurement(repo_root, review_ref, store_mode)
}

pub(crate) struct IsolatedPerfRunner {
    executable: tempfile::TempPath,
}

impl IsolatedPerfRunner {
    pub(crate) fn capture() -> anyhow::Result<Self> {
        let executable = std::env::current_exe().map_err(|error| {
            anyhow::anyhow!("locating test binary for isolated perf child: {error}")
        })?;
        Self::capture_from(&executable)
    }

    fn capture_from(executable: &Path) -> anyhow::Result<Self> {
        let parent = executable.parent().ok_or_else(|| {
            anyhow::anyhow!(
                "isolated perf child executable has no parent: {}",
                executable.display()
            )
        })?;
        let mut source_file = std::fs::File::open(executable).map_err(|error| {
            anyhow::anyhow!(
                "opening isolated perf child executable {}: {error}",
                executable.display()
            )
        })?;
        let source = source_file.metadata().map_err(|error| {
            anyhow::anyhow!(
                "reading isolated perf child executable {}: {error}",
                executable.display()
            )
        })?;
        anyhow::ensure!(
            source.is_file(),
            "isolated perf child executable is not a regular file: {}",
            executable.display()
        );
        let mut snapshot = tempfile::Builder::new()
            .prefix(".polint-perf-child-")
            .suffix(std::env::consts::EXE_SUFFIX)
            .tempfile_in(parent)
            .map_err(|error| {
                anyhow::anyhow!(
                    "creating isolated perf child snapshot beside {}: {error}",
                    executable.display()
                )
            })?;
        let copied = std::io::copy(&mut source_file, snapshot.as_file_mut()).map_err(|error| {
            anyhow::anyhow!(
                "snapshotting isolated perf child executable {}: {error}",
                executable.display()
            )
        })?;
        anyhow::ensure!(
            copied == source.len(),
            "isolated perf child executable changed while snapshotting: expected {} bytes, copied {copied}",
            source.len()
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;

            snapshot
                .as_file()
                .set_permissions(std::fs::Permissions::from_mode(0o500))
                .map_err(|error| {
                    anyhow::anyhow!("setting isolated perf child executable permissions: {error}")
                })?;
        }
        Ok(Self {
            executable: snapshot.into_temp_path(),
        })
    }

    pub(crate) fn run_point(
        &self,
        repo_root: &Path,
        review_ref: Option<&str>,
        store_mode: SemanticStoreBenchMode,
    ) -> anyhow::Result<CurvePoint> {
        Ok(self
            .run_measurement(repo_root, review_ref, store_mode)?
            .point)
    }

    pub(crate) fn run_measurement(
        &self,
        repo_root: &Path,
        review_ref: Option<&str>,
        store_mode: SemanticStoreBenchMode,
    ) -> anyhow::Result<PerfMeasurement> {
        require_repo_local_perf_cache()?;
        let mut command = std::process::Command::new(self.executable.as_os_str());
        command
            .arg("--exact")
            .arg(CHILD_MEASURE_TEST)
            .arg("--nocapture")
            .arg("--test-threads=1")
            .env(CHILD_REPO_ENV, repo_root)
            // Never let the child inherit the regenerator switch (it would re-enter
            // regeneration and recurse) or a stale review ref from an outer spawn.
            .env_remove("POLINT_WRITE_STORE_DISABLED_BASELINE")
            .env_remove(CHILD_REVIEW_ENV)
            .env_remove(CHILD_SEMANTIC_STORE_ENV);
        if let Some(reference) = review_ref {
            command.env(CHILD_REVIEW_ENV, reference);
        }
        if store_mode == SemanticStoreBenchMode::Enabled {
            command.env(CHILD_SEMANTIC_STORE_ENV, "enabled");
        }
        let output = command
            .output()
            .map_err(|error| anyhow::anyhow!("spawning isolated perf child: {error}"))?;
        if !output.status.success() {
            anyhow::bail!(
                "isolated perf child exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let json = stdout
            .split_once(CHILD_POINT_BEGIN)
            .and_then(|(_, rest)| rest.split_once(CHILD_POINT_END))
            .map(|(json, _)| json.trim())
            .ok_or_else(|| {
                anyhow::anyhow!("isolated perf child emitted no measurement; stdout: {stdout}")
            })?;
        serde_json::from_str(json)
            .map_err(|error| anyhow::anyhow!("parsing isolated perf child measurement: {error}"))
    }

    #[cfg(test)]
    fn executable_path(&self) -> &Path {
        self.executable.as_ref()
    }
}

/// Deterministic digest of the diagnostics a store-disabled (== current)
/// `polint check` produces over `repo_root`.
///
/// This is the diagnostics-parity marker a
/// [`StoreDisabledBaseline`](crate::eval::baseline::StoreDisabledBaseline)
/// records: enabling the durable store must not change the
/// diagnostics polint emits, so a later run can assert this digest is unchanged.
/// It is the FNV stable-hash over the sorted, canonical-JSON-serialized
/// diagnostics of the check-equivalent kernel run. Clean code (no diagnostics)
/// still yields a stable, non-empty digest (the hash of the empty set).
pub(crate) fn diagnostics_digest_for_repo(repo_root: &Path) -> anyhow::Result<String> {
    diagnostics_digest_for_repo_with_store_mode(repo_root, SemanticStoreBenchMode::Disabled)
}

pub(crate) fn diagnostics_digest_for_repo_with_store_mode(
    repo_root: &Path,
    store_mode: SemanticStoreBenchMode,
) -> anyhow::Result<String> {
    let output = run_check_kernel_with_store_mode(repo_root, store_mode)?;
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
/// exercises the persistent layer cache populated by the cold run.
fn run_check_kernel_with_store_mode(
    repo_root: &Path,
    store_mode: SemanticStoreBenchMode,
) -> anyhow::Result<KernelOutput> {
    require_repo_local_perf_cache()?;
    let loaded = crate::config::load_config(repo_root)?;
    let config_digest = crate::cache::keys::config_hash(&loaded);
    let rule_digest = crate::cache::keys::rule_hash(&[], None, &BTreeMap::new());
    let cache = crate::cache::Cache::default_for_repo(repo_root, true);
    let cache = if store_mode == SemanticStoreBenchMode::Enabled {
        cache.with_semantic_store_enabled_for_test()
    } else {
        cache
    };
    let plan = crate::analysis_plan::AnalysisPlan::full_pipeline_for_test();
    let output = AnalysisKernel::run(KernelInput {
        loaded: &loaded,
        cache: &cache,
        config_digest: &config_digest,
        rule_digest: &rule_digest,
        plan: &plan,
        parallel: true,
    })?;
    if store_mode == SemanticStoreBenchMode::Enabled
        && output.run_report.store_status() != &crate::analysis_kernel::StoreStatus::Ready
    {
        anyhow::bail!(
            "enabled semantic-store benchmark did not publish a complete generation: {:?}",
            output.run_report.store_status()
        );
    }
    Ok(output)
}

fn require_repo_local_perf_cache() -> anyhow::Result<()> {
    validate_perf_cache_override(std::env::var_os(crate::cache::POLINT_CACHE_DIR_ENV).as_deref())
}

fn validate_perf_cache_override(value: Option<&std::ffi::OsStr>) -> anyhow::Result<()> {
    if value.is_some_and(|value| !value.is_empty()) {
        let env_name = crate::cache::POLINT_CACHE_DIR_ENV;
        anyhow::bail!(
            "performance measurements require {env_name} to be unset so each fixture clone uses an independent repository-local cache"
        );
    }
    Ok(())
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
/// top-level directory is absent. Symlinks are not followed and directory
/// metadata is not counted, so the result reflects only the cache payload bytes.
/// Any error after traversal starts is propagated rather than converted into
/// plausible but incomplete benchmark evidence.
fn dir_size_bytes(dir: &Path) -> anyhow::Result<u64> {
    dir_size_bytes_inner(dir, true)
}

fn checked_size_total(total: u64, additional: u64, path: &Path) -> anyhow::Result<u64> {
    total.checked_add(additional).ok_or_else(|| {
        anyhow::anyhow!(
            "benchmark size total overflowed while accounting for {}",
            path.display()
        )
    })
}

fn dir_size_bytes_inner(dir: &Path, absent_root_is_zero: bool) -> anyhow::Result<u64> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if absent_root_is_zero && error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(0);
        }
        Err(error) => {
            return Err(anyhow::anyhow!(
                "reading benchmark size root {}: {error}",
                dir.display()
            ));
        }
    };
    let mut total = 0u64;
    for entry in entries {
        let entry = entry.map_err(|error| {
            anyhow::anyhow!("reading benchmark size entry in {}: {error}", dir.display())
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|error| {
            anyhow::anyhow!(
                "reading benchmark file type for {}: {error}",
                path.display()
            )
        })?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let child_total = dir_size_bytes_inner(&path, false)?;
            total = checked_size_total(total, child_total, &path)?;
        } else if file_type.is_file() {
            let metadata = entry.metadata().map_err(|error| {
                anyhow::anyhow!("reading benchmark metadata for {}: {error}", path.display())
            })?;
            total = checked_size_total(total, metadata.len(), &path)?;
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::process::Command;
    use std::rc::Rc;
    use tempfile::tempdir;

    #[test]
    fn cold_output_is_dropped_before_warm_measurement() {
        struct TrackedOutput(Rc<Cell<usize>>);

        impl Drop for TrackedOutput {
            fn drop(&mut self) {
                self.0.set(self.0.get() - 1);
            }
        }

        let live_outputs = Rc::new(Cell::new(0));
        let mut calls = 0;
        let (_, (), (), warm_output) = measure_cold_then_warm_outputs(
            || -> anyhow::Result<TrackedOutput> {
                assert_eq!(
                    live_outputs.get(),
                    0,
                    "the prior full output must be gone before each run"
                );
                calls += 1;
                live_outputs.set(live_outputs.get() + 1);
                Ok(TrackedOutput(Rc::clone(&live_outputs)))
            },
            |_| (),
        )
        .expect("measure tracked cold/warm outputs");

        assert_eq!(calls, 2);
        assert_eq!(live_outputs.get(), 1, "only warm output stays live");
        drop(warm_output);
        assert_eq!(live_outputs.get(), 0);
    }

    #[test]
    fn cold_measurement_error_stops_before_warm_execution() {
        let mut calls = 0;
        let error = measure_cold_then_warm_outputs(
            || -> anyhow::Result<()> {
                calls += 1;
                anyhow::bail!("cold failed")
            },
            |_| (),
        )
        .expect_err("cold failure must propagate");

        assert_eq!(calls, 1);
        assert!(error.to_string().contains("cold failed"));
    }

    #[test]
    fn warm_measurement_error_propagates_independently() {
        let mut calls = 0;
        let error = measure_cold_then_warm_outputs(
            || -> anyhow::Result<()> {
                calls += 1;
                if calls == 2 {
                    anyhow::bail!("warm failed");
                }
                Ok(())
            },
            |_| (),
        )
        .expect_err("warm failure must propagate");

        assert_eq!(calls, 2);
        assert!(error.to_string().contains("warm failed"));
    }

    #[test]
    fn dir_size_counts_nested_regular_files() {
        let root = tempdir().expect("size root");
        write(root.path(), "top.bin", "1234");
        write(root.path(), "nested/child.bin", "123456");

        assert_eq!(dir_size_bytes(root.path()).expect("size directory"), 10);
    }

    #[test]
    fn dir_size_total_overflow_is_an_error() {
        let path = Path::new("oversized-cache-entry");
        let error = checked_size_total(u64::MAX, 1, path)
            .expect_err("benchmark evidence must fail closed on size overflow");

        assert!(error.to_string().contains("overflowed"));
        assert!(error.to_string().contains("oversized-cache-entry"));
    }

    #[test]
    fn non_store_cache_size_underflow_is_an_error() {
        assert_eq!(
            checked_non_store_cache_bytes(10, 4).expect("nested store fits in cache"),
            6
        );
        let error = checked_non_store_cache_bytes(4, 10)
            .expect_err("inconsistent benchmark size evidence must fail closed");
        assert!(error.to_string().contains("exceeded the containing cache"));
    }

    #[test]
    fn perf_measurements_reject_a_shared_cache_override() {
        validate_perf_cache_override(None).expect("an unset override is isolated");
        validate_perf_cache_override(Some(std::ffi::OsStr::new("")))
            .expect("an empty override uses the repository-local default");

        let error = validate_perf_cache_override(Some(std::ffi::OsStr::new("shared-cache")))
            .expect_err("a shared cache override invalidates paired measurements");
        assert!(error.to_string().contains("POLINT_CACHE_DIR"));
        assert!(error.to_string().contains("repository-local cache"));
    }

    #[test]
    fn isolated_perf_runner_keeps_an_immutable_executable_snapshot() {
        let root = tempdir().expect("snapshot root");
        let source = root
            .path()
            .join(format!("source{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, b"original executable").expect("write source executable");

        let runner = IsolatedPerfRunner::capture_from(&source).expect("capture executable");
        std::fs::write(&source, b"replacement executable").expect("replace source executable");

        assert_ne!(runner.executable_path(), source);
        assert_eq!(
            std::fs::read(runner.executable_path()).expect("read executable snapshot"),
            b"original executable"
        );
    }

    #[test]
    fn dir_size_treats_only_an_absent_root_as_zero() {
        let root = tempdir().expect("size root");
        let absent = root.path().join("absent");
        assert_eq!(dir_size_bytes(&absent).expect("absent root"), 0);

        let regular_file = root.path().join("not-a-directory");
        std::fs::write(&regular_file, b"payload").expect("write regular file");
        let error = dir_size_bytes(&regular_file).expect_err("non-directory root must fail");
        assert!(error.to_string().contains("reading benchmark size root"));
    }

    #[cfg(unix)]
    #[test]
    fn dir_size_does_not_follow_symlinks() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("size root");
        let measured = root.path().join("measured");
        std::fs::create_dir(&measured).expect("create measured directory");
        let outside = root.path().join("outside.bin");
        std::fs::write(&outside, b"outside payload").expect("write symlink target");
        symlink(&outside, measured.join("linked.bin")).expect("create file symlink");

        assert_eq!(dir_size_bytes(&measured).expect("size directory"), 0);
    }

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

    /// Child-process entry for the isolated performance measurement functions.
    ///
    /// A normal `cargo test` run executes this as an immediate no-op because
    /// `POLINT_PERF_CHILD_REPO` is absent. When an isolated entry point
    /// re-invokes this binary it sets that var, so this FRESH process measures
    /// the repo and prints the [`PerfMeasurement`] as JSON between the shared
    /// markers, then `exit(0)`s before libtest prints its summary. Measuring in
    /// an independent process makes the absolute peak comparable between paired
    /// modes and prevents work in the parent test process from contaminating the
    /// raw `peak_rss_delta_bytes` evidence.
    #[test]
    fn perf_child_measure_entry() {
        use std::io::Write as _;
        let Some(repo) = std::env::var_os(super::CHILD_REPO_ENV) else {
            return;
        };
        let review = std::env::var(super::CHILD_REVIEW_ENV).ok();
        let store_mode = if std::env::var_os(super::CHILD_SEMANTIC_STORE_ENV).is_some() {
            super::SemanticStoreBenchMode::Enabled
        } else {
            super::SemanticStoreBenchMode::Disabled
        };
        let measurement = super::run_repo_perf_measurement_with_store_mode(
            Path::new(&repo),
            review.as_deref(),
            store_mode,
        )
        .expect("isolated perf child measurement");
        let json = serde_json::to_string(&measurement).expect("serialize child measurement");
        println!(
            "{}{json}{}",
            super::CHILD_POINT_BEGIN,
            super::CHILD_POINT_END
        );
        std::io::stdout().flush().ok();
        std::process::exit(0);
    }

    #[test]
    fn run_repo_perf_point_isolated_measures_in_a_fresh_child() {
        let repo = tempdir().unwrap();
        write_tiny_repo(repo.path());

        let point = run_repo_perf_point_isolated(repo.path(), None).unwrap();

        assert!(
            point.repo_file_count > 0,
            "isolated child must measure repo files: {point:?}"
        );
        assert!(
            point.cold_wall_clock_ms > 0,
            "isolated child must record cold wall-clock: {point:?}"
        );
        assert!(
            point.peak_rss_bytes > 0,
            "isolated child must record peak RSS: {point:?}"
        );
        assert_eq!(point.diff_files, 0, "no review ref -> no diff");
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

    mod semantic_store {
        use super::*;

        #[test]
        fn isolated_modes_report_real_store_bytes_and_equal_diagnostics_digest() {
            let isolated_runner =
                IsolatedPerfRunner::capture().expect("capture isolated test executable");
            let disabled_repo = tempdir().expect("disabled repo");
            write_tiny_repo(disabled_repo.path());
            let disabled = isolated_runner
                .run_point(disabled_repo.path(), None, SemanticStoreBenchMode::Disabled)
                .expect("disabled isolated measurement");
            let disabled_store_path =
                crate::cache::CacheLayout::for_repo(disabled_repo.path()).semantic_store_path();

            assert_eq!(disabled.size.store_bytes, 0);
            assert!(!disabled_store_path.exists());

            let enabled_repo = tempdir().expect("enabled repo");
            write_tiny_repo(enabled_repo.path());
            let enabled = isolated_runner
                .run_point(enabled_repo.path(), None, SemanticStoreBenchMode::Enabled)
                .expect("enabled isolated measurement");
            let enabled_store_path =
                crate::cache::CacheLayout::for_repo(enabled_repo.path()).semantic_store_path();

            assert!(enabled.size.store_bytes > 0);
            assert!(enabled_store_path.is_file());
            assert!(AnalysisKernel::semantic_store_schema_is_current_for_test(
                &enabled_store_path
            ));
            assert_eq!(
                serde_json::to_string(&enabled).expect("serialize point"),
                serde_json::to_string(&enabled).expect("serialize point again")
            );

            let digest_repo = tempdir().expect("digest repo");
            write_tiny_repo(digest_repo.path());
            let disabled_digest = diagnostics_digest_for_repo_with_store_mode(
                digest_repo.path(),
                SemanticStoreBenchMode::Disabled,
            )
            .expect("disabled digest");
            let enabled_digest = diagnostics_digest_for_repo_with_store_mode(
                digest_repo.path(),
                SemanticStoreBenchMode::Enabled,
            )
            .expect("enabled digest");
            assert_eq!(enabled_digest, disabled_digest);
        }
    }
}
