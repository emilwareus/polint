//! Semantic-store regression-budget gates.
//!
//! Committed store-disabled artifacts preserve fixture, digest, and complete
//! reproduction context. Their numeric values are historical/informational
//! unless every context field matches; general CI never uses them as portable
//! numeric normalization sources.
//!
//! The supported-boundary authenticity smoke compares store-enabled runs with
//! fresh same-host disabled controls while pinning rich semantic evidence. A
//! separate Linux scale gate restores a substantially larger generated working
//! set without coupling scale coverage to those exact evidence vectors. Paired
//! numeric gates compare isolated absolute process peaks and apply the locked
//! 1.20 RSS/1.25 cold ratios with the approved 16 MiB/50 ms absolute noise
//! floors. Raw within-run high-water growth remains available as informational
//! evidence but does not select a different blocking policy when a process's
//! earlier high-water mark makes one of them zero.
//!
//! A store change whose measured run regresses scale or latency past the budget
//! fails its gate rather than passing silently ([`is_blocking`] returns `true`
//! on Fail).
//! This is the fail-not-silent mechanism the scale/latency outcome gates rely on.
//!
//! Everything here stays `pub(crate)` under `eval::bench`; it is the
//! crate-internal validation infrastructure, not a public CLI surface.

use crate::eval::baseline::{BaselineThresholds, StoreDisabledBaseline};
use crate::eval::bench::curve::CurvePoint;
use crate::eval::gates::{GateCheck, GateVerdict};

/// Absolute peak-RSS noise floor in bytes. A run may exceed its baseline RSS
/// measurement by up to this many bytes before it Fails, even when that exceeds
/// the +20% ratio. Without it, a small baseline makes the ratio tolerance a
/// fraction of a megabyte, so ordinary allocator jitter would Fail a run that
/// did not regress. The locked +20% ratio still governs any baseline large
/// enough that `baseline * 1.20` exceeds `baseline + this floor`.
pub(crate) const PEAK_RSS_ABS_FLOOR_BYTES: u64 = 16 * 1024 * 1024;

/// Absolute cold-wall-clock noise floor in milliseconds. A run may exceed
/// the baseline cold wall-clock by up to this many milliseconds before it Fails,
/// even when that exceeds the +25% ratio. A +25% ratio on a 20 ms baseline is
/// only 5 ms — below scheduling jitter — so without this floor the gate would
/// emit false Fails on sub-second baselines. The locked +25% ratio still governs
/// any baseline large enough that `baseline * 1.25` exceeds `baseline + floor`.
pub(crate) const COLD_WALL_CLOCK_ABS_FLOOR_MS: u64 = 50;

/// The outcome of comparing a measured run against the store-disabled baseline.
///
/// `verdict` aggregates the per-metric `checks` via `.max()` over the shared
/// `GateVerdict` ordering (`Pass < Warn < Fail`), matching the vocabulary in
/// `eval::gates`. This regression gate emits only `Pass` or `Fail`: there is no
/// soft-warn band for a locked budget, so the aggregate is effectively
/// Pass-or-Fail. The `Warn` tier exists in the shared enum but is intentionally
/// unused here.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RegressionGateReport {
    pub(crate) verdict: GateVerdict,
    pub(crate) checks: Vec<GateCheck>,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SemanticStoreBoundaryReport {
    pub(crate) regression: RegressionGateReport,
    pub(crate) disabled_control: CurvePoint,
    pub(crate) measured: CurvePoint,
    pub(crate) diagnostics_digest: String,
    pub(crate) samples: Vec<SemanticStoreBoundarySample>,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
struct CommittedStoreDisabledCheckReport {
    regression: RegressionGateReport,
    measured: CurvePoint,
    diagnostics_digest: String,
}

#[cfg(all(test, target_os = "linux"))]
#[derive(Clone, Debug, PartialEq)]
struct SemanticStoreScaleReport {
    regression: RegressionGateReport,
    disabled: CurvePoint,
    enabled: CurvePoint,
    diagnostics_digest: String,
    cold_ready: bool,
    warm_ready: bool,
    fingerprint: crate::analysis_kernel::SemanticStoreBoundaryFingerprint,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SemanticStoreBoundarySample {
    pub(crate) disabled: CurvePoint,
    pub(crate) enabled: CurvePoint,
    pub(crate) regression: RegressionGateReport,
    pub(crate) provider_evidence: crate::eval::bench::runner::ProviderRunEvidence,
    pub(crate) cold_ready: bool,
    pub(crate) warm_ready: bool,
    pub(crate) fingerprint: crate::analysis_kernel::SemanticStoreBoundaryFingerprint,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SemanticStoreBoundaryShape {
    repo_file_count: u64,
    repo_source_bytes: u64,
    diff_files: u64,
    diff_hunk_lines: u64,
}

#[cfg(test)]
const REPRESENTATIVE_BOUNDARY_SHAPE: SemanticStoreBoundaryShape = SemanticStoreBoundaryShape {
    repo_file_count: 20,
    repo_source_bytes: 5_709,
    diff_files: 0,
    diff_hunk_lines: 0,
};

#[cfg(test)]
const SUPPORTED_BOUNDARY_FILE_PAIRS: usize = 10;
#[cfg(test)]
const FUNCTIONS_PER_SUPPORTED_BOUNDARY_FILE: usize = 1;
#[cfg(test)]
const REPRESENTATIVE_FUNCTION_COUNT: u64 =
    (SUPPORTED_BOUNDARY_FILE_PAIRS * FUNCTIONS_PER_SUPPORTED_BOUNDARY_FILE * 2) as u64;
#[cfg(test)]
const REPRESENTATIVE_BOUNDARY_REPO_ID: &str = "semantic-store-supported-boundary";
#[cfg(test)]
const REPRESENTATIVE_BOUNDARY_SUITE_ID: &str = "semantic-store-supported-boundary-check";
#[cfg(all(test, target_os = "linux"))]
const SCALE_GATE_FILE_PAIRS: usize = 256;
#[cfg(all(test, target_os = "linux"))]
const SCALE_GATE_FUNCTION_FILE_PAIRS: usize = 192;
#[cfg(all(test, target_os = "linux"))]
const SCALE_GATE_FUNCTIONS_PER_FILE: usize = 1;
#[cfg(all(test, target_os = "linux"))]
const SCALE_GATE_PACKAGE_COUNT: u64 = 1;
#[cfg(all(test, target_os = "linux"))]
// The Go SSA package inventory includes one synthetic `init` alongside the
// fixture's explicitly declared functions.
const SCALE_GATE_GO_FUNCTION_COUNT: u64 =
    (SCALE_GATE_FUNCTION_FILE_PAIRS * SCALE_GATE_FUNCTIONS_PER_FILE + 2) as u64
        + SCALE_GATE_PACKAGE_COUNT;
#[cfg(all(test, target_os = "linux"))]
const SCALE_GATE_REPO_FILE_COUNT: u64 = (SCALE_GATE_FILE_PAIRS * 2 + 2) as u64;
#[cfg(all(test, target_os = "linux"))]
const SCALE_GATE_REPO_SOURCE_BYTES: u64 = 19_732;
#[cfg(all(test, target_os = "linux"))]
const MIN_SCALE_GATE_STORE_BYTES: u64 = 15 * 1024 * 1024;
#[cfg(all(test, target_os = "linux"))]
const MIN_SCALE_GATE_FACT_COUNT: u64 = 25_000;
#[cfg(all(test, target_os = "linux"))]
const MIN_SCALE_GATE_STABLE_FACT_STORAGE_BYTES: u64 = 22 * 1024 * 1024;
#[cfg(all(test, target_os = "linux"))]
const MIN_SCALE_GATE_FACT_LOGICAL_BYTES: u64 = 16 * 1024 * 1024;
#[cfg(all(test, target_os = "linux"))]
const MIN_SCALE_GATE_SEMANTIC_LOGICAL_BYTES: u64 = 160 * 1024 * 1024;
#[cfg(all(test, target_os = "linux"))]
const MIN_SCALE_GATE_PLANNED_ROWS: u64 = 40_000;
#[cfg(all(test, target_os = "linux"))]
const SCALE_GATE_INPUT_FILE_COUNTS_BY_LANGUAGE: &[(&str, u64)] =
    &[("go", 257), ("typescript", 257)];
#[cfg(all(test, target_os = "linux"))]
const SCALE_GATE_FUNCTION_COUNTS_BY_PROVIDER: &[(&str, u64)] =
    &[("polint.go.syntax", 194), ("polint.ts.syntax", 193)];
#[cfg(test)]
const REPRESENTATIVE_GO_SEMANTIC_FACTS: &[(&str, u64)] = &[
    ("address_taken", 0),
    ("callsites", 29),
    ("dynamic_dispatch", 0),
    ("functions", 20),
    ("instantiated_types", 0),
    ("method_sets", 0),
    ("package_errors", 0),
    ("packages", 10),
    ("rta_edges", 0),
];

#[cfg(test)]
impl From<&CurvePoint> for SemanticStoreBoundaryShape {
    fn from(point: &CurvePoint) -> Self {
        Self {
            repo_file_count: point.repo_file_count,
            repo_source_bytes: point.repo_source_bytes,
            diff_files: point.diff_files,
            diff_hunk_lines: point.diff_hunk_lines,
        }
    }
}

#[cfg(test)]
fn validate_semantic_store_boundary_pair_shape(
    disabled: &CurvePoint,
    enabled: &CurvePoint,
) -> anyhow::Result<()> {
    validate_semantic_store_pair_shape(disabled, enabled)?;
    validate_supported_semantic_store_boundary_shape(SemanticStoreBoundaryShape::from(disabled))
}

#[cfg(test)]
fn validate_supported_semantic_store_boundary_shape(
    measured: SemanticStoreBoundaryShape,
) -> anyhow::Result<()> {
    if measured != REPRESENTATIVE_BOUNDARY_SHAPE {
        anyhow::bail!(
            "semantic-store boundary fixture shape drifted: measured={measured:?}, expected={REPRESENTATIVE_BOUNDARY_SHAPE:?}"
        );
    }
    Ok(())
}

#[cfg(test)]
fn validate_semantic_store_pair_shape(
    disabled: &CurvePoint,
    enabled: &CurvePoint,
) -> anyhow::Result<()> {
    let disabled_shape = SemanticStoreBoundaryShape::from(disabled);
    let enabled_shape = SemanticStoreBoundaryShape::from(enabled);
    if disabled_shape != enabled_shape {
        anyhow::bail!(
            "semantic-store modes measured different workload shapes: disabled={disabled_shape:?}, enabled={enabled_shape:?}"
        );
    }
    Ok(())
}

#[cfg(test)]
fn validate_provider_availability_pair(
    disabled: &crate::eval::bench::runner::PerfMeasurement,
    enabled: &crate::eval::bench::runner::PerfMeasurement,
) -> anyhow::Result<crate::eval::bench::runner::ProviderRunEvidence> {
    if disabled.cold_provider_evidence != disabled.warm_provider_evidence {
        anyhow::bail!(
            "store-disabled cold/warm provider evidence drifted: cold={:?}, warm={:?}",
            disabled.cold_provider_evidence,
            disabled.warm_provider_evidence
        );
    }
    if enabled.cold_provider_evidence != enabled.warm_provider_evidence {
        anyhow::bail!(
            "store-enabled cold/warm provider evidence drifted: cold={:?}, warm={:?}",
            enabled.cold_provider_evidence,
            enabled.warm_provider_evidence
        );
    }
    if disabled.cold_diagnostics_digest != disabled.warm_diagnostics_digest {
        anyhow::bail!("store-disabled cold/warm diagnostics digest drifted");
    }
    if enabled.cold_diagnostics_digest != enabled.warm_diagnostics_digest {
        anyhow::bail!("store-enabled cold/warm diagnostics digest drifted");
    }
    if disabled.warm_diagnostics_digest != enabled.warm_diagnostics_digest {
        anyhow::bail!(
            "semantic-store modes changed diagnostics: disabled={}, enabled={}",
            disabled.warm_diagnostics_digest,
            enabled.warm_diagnostics_digest
        );
    }
    let disabled_availability = disabled.warm_provider_evidence.availability_projection();
    let enabled_availability = enabled.warm_provider_evidence.availability_projection();
    if disabled_availability != enabled_availability {
        anyhow::bail!(
            "semantic-store modes observed different provider availability: disabled={disabled_availability:?}, enabled={enabled_availability:?}"
        );
    }
    Ok(enabled.warm_provider_evidence.clone())
}

#[cfg(test)]
fn validate_provider_evidence_pair(
    disabled: &crate::eval::bench::runner::PerfMeasurement,
    enabled: &crate::eval::bench::runner::PerfMeasurement,
) -> anyhow::Result<crate::eval::bench::runner::ProviderRunEvidence> {
    let evidence = validate_provider_availability_pair(disabled, enabled)?;
    let go_semantic_facts_match = evidence.go_semantic_facts.len()
        == REPRESENTATIVE_GO_SEMANTIC_FACTS.len()
        && evidence
            .go_semantic_facts
            .iter()
            .zip(REPRESENTATIVE_GO_SEMANTIC_FACTS)
            .all(|((family, measured), (expected_family, expected))| {
                family == expected_family && measured == expected
            });
    if !go_semantic_facts_match {
        anyhow::bail!(
            "semantic-store boundary Go semantic fact vector drifted: measured={:?}, expected={REPRESENTATIVE_GO_SEMANTIC_FACTS:?}",
            evidence.go_semantic_facts
        );
    }
    validate_fully_available_provider_evidence(&evidence)?;
    Ok(evidence)
}

#[cfg(test)]
fn validate_fully_available_provider_evidence(
    evidence: &crate::eval::bench::runner::ProviderRunEvidence,
) -> anyhow::Result<()> {
    let availability = evidence.availability_projection();
    let expected_provider_validations =
        crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| (manifest.id.to_string(), "native_trusted".to_string()))
            .collect::<Vec<_>>();
    if availability.provider_validations != expected_provider_validations
        || !availability.provider_diagnostic_counts.is_empty()
        || evidence.capability_diagnostic_count != 0
        || evidence.requested_capabilities.is_empty()
        || evidence.effective_capabilities.is_empty()
        || evidence
            .requested_capabilities
            .iter()
            .any(|row| row.support_status != "supported" || row.setup_status == "setup_missing")
        || evidence
            .effective_capabilities
            .iter()
            .any(|row| row.support_status != "supported")
    {
        anyhow::bail!(
            "semantic-store boundary provider setup/capability evidence was not fully available: {evidence:?}"
        );
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
fn validate_scale_go_semantic_workload(
    evidence: &crate::eval::bench::runner::ProviderRunEvidence,
) -> anyhow::Result<()> {
    let fact_count = |expected_family: &str| {
        evidence
            .go_semantic_facts
            .iter()
            .find_map(|(family, count)| (family == expected_family).then_some(*count))
    };
    anyhow::ensure!(
        fact_count("functions") == Some(SCALE_GATE_GO_FUNCTION_COUNT)
            && fact_count("packages") == Some(SCALE_GATE_PACKAGE_COUNT)
            && fact_count("package_errors") == Some(0),
        "semantic-store scale fixture did not exercise its complete Go semantic workload: {:?}",
        evidence.go_semantic_facts
    );
    Ok(())
}

#[cfg(test)]
fn semantic_store_comparison_baseline(
    disabled_control: &CurvePoint,
    disabled_diagnostics_digest: &str,
) -> StoreDisabledBaseline {
    StoreDisabledBaseline::from_curve_point(
        REPRESENTATIVE_BOUNDARY_REPO_ID,
        REPRESENTATIVE_BOUNDARY_SUITE_ID,
        disabled_control,
        disabled_diagnostics_digest,
    )
}

#[cfg(test)]
const ISOLATED_ABSOLUTE_RSS_METRIC: &str = "peak_rss_isolated_absolute_ratio";

#[cfg(test)]
fn evaluate_paired_semantic_store_regression_budget(
    disabled_control: &CurvePoint,
    enabled: &CurvePoint,
    disabled_diagnostics_digest: &str,
    enabled_diagnostics_digest: &str,
) -> RegressionGateReport {
    let comparison_baseline =
        semantic_store_comparison_baseline(disabled_control, disabled_diagnostics_digest);
    let thresholds = BaselineThresholds::default();
    let rss_check = isolated_absolute_rss_budget_check(
        disabled_control.peak_rss_bytes,
        enabled.peak_rss_bytes,
        thresholds.max_peak_rss_ratio,
        PEAK_RSS_ABS_FLOOR_BYTES,
    );
    evaluate_regression_budget_with_rss_check(
        &comparison_baseline,
        enabled,
        &thresholds,
        Some(enabled_diagnostics_digest),
        rss_check,
    )
}

#[cfg(test)]
pub(crate) fn evaluate_supported_semantic_store_boundary(
    repo_root: &std::path::Path,
) -> anyhow::Result<SemanticStoreBoundaryReport> {
    use std::process::Command;

    use crate::analysis_kernel::AnalysisKernel;
    use crate::eval::bench::runner::{
        IsolatedPerfRunner, SemanticStoreBenchMode, diagnostics_digest_for_repo_with_store_mode,
    };

    let mut samples = Vec::with_capacity(2);
    let mut expected_diagnostics_digest = None;
    let mut expected_provider_evidence = None;
    let isolated_runner = IsolatedPerfRunner::capture()?;

    // Each mode gets its own clean clone and the same one-run, store-disabled
    // cache priming. This keeps layer/tool inputs equivalent without letting a
    // disabled measurement warm the exact repository later measured enabled.
    // The two accepted samples reverse mode order to balance process and host
    // warm-up effects. Each child reports both its raw within-run high-water growth
    // and its isolated process high-water mark. The paired gate uses the latter
    // consistently because a pre-run libtest high-water mark can legitimately
    // make the raw delta zero even when the analysis used substantial memory.
    for enabled_first in [false, true] {
        let sample = (|| -> anyhow::Result<SemanticStoreBoundarySample> {
            let pair = tempfile::tempdir()?;
            let disabled_root = pair.path().join("disabled");
            let enabled_root = pair.path().join("enabled");
            for target in [&disabled_root, &enabled_root] {
                let output = Command::new("git")
                    .args(["clone", "--quiet", "--no-local"])
                    .arg(repo_root)
                    .arg(target)
                    .output()?;
                if !output.status.success() {
                    anyhow::bail!(
                        "cloning semantic-store boundary fixture failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }

            // Prime in measurement order so the first-measured mode has the
            // less-recently touched clone in both counterbalanced samples.
            let (disabled_prime, enabled_prime) = if enabled_first {
                let enabled_prime = diagnostics_digest_for_repo_with_store_mode(
                    &enabled_root,
                    SemanticStoreBenchMode::Disabled,
                )?;
                let disabled_prime = diagnostics_digest_for_repo_with_store_mode(
                    &disabled_root,
                    SemanticStoreBenchMode::Disabled,
                )?;
                (disabled_prime, enabled_prime)
            } else {
                let disabled_prime = diagnostics_digest_for_repo_with_store_mode(
                    &disabled_root,
                    SemanticStoreBenchMode::Disabled,
                )?;
                let enabled_prime = diagnostics_digest_for_repo_with_store_mode(
                    &enabled_root,
                    SemanticStoreBenchMode::Disabled,
                )?;
                (disabled_prime, enabled_prime)
            };
            if disabled_prime != enabled_prime {
                anyhow::bail!("independently primed boundary fixtures changed diagnostics");
            }
            if let Some(expected) = &expected_diagnostics_digest {
                if expected != &disabled_prime {
                    anyhow::bail!("boundary diagnostics changed across identical fixture clones");
                }
            } else {
                expected_diagnostics_digest = Some(disabled_prime.clone());
            }

            let (disabled, enabled) = if enabled_first {
                let enabled = isolated_runner.run_measurement(
                    &enabled_root,
                    None,
                    SemanticStoreBenchMode::Enabled,
                )?;
                let disabled = isolated_runner.run_measurement(
                    &disabled_root,
                    None,
                    SemanticStoreBenchMode::Disabled,
                )?;
                (disabled, enabled)
            } else {
                let disabled = isolated_runner.run_measurement(
                    &disabled_root,
                    None,
                    SemanticStoreBenchMode::Disabled,
                )?;
                let enabled = isolated_runner.run_measurement(
                    &enabled_root,
                    None,
                    SemanticStoreBenchMode::Enabled,
                )?;
                (disabled, enabled)
            };
            validate_semantic_store_boundary_pair_shape(&disabled.point, &enabled.point)?;
            let provider_evidence = validate_provider_evidence_pair(&disabled, &enabled)?;
            let provider_availability = provider_evidence.availability_projection();
            if let Some(expected) = &expected_provider_evidence {
                if expected != &provider_availability {
                    anyhow::bail!(
                        "provider availability changed across identical boundary samples: expected={expected:?}, measured={provider_availability:?}"
                    );
                }
            } else {
                expected_provider_evidence = Some(provider_availability);
            }

            let semantic_store = enabled.semantic_store.ok_or_else(|| {
                anyhow::anyhow!("enabled semantic-store child omitted readiness evidence")
            })?;

            let enabled_digest = enabled.warm_diagnostics_digest.clone();
            if enabled_digest != disabled_prime {
                anyhow::bail!("enabling the semantic store changed diagnostics");
            }
            let regression = evaluate_paired_semantic_store_regression_budget(
                &disabled.point,
                &enabled.point,
                &disabled_prime,
                &enabled_digest,
            );
            let store_path =
                crate::cache::CacheLayout::for_repo(&enabled_root).semantic_store_path();
            let fingerprint = AnalysisKernel::semantic_store_boundary_fingerprint_for_test(
                &store_path,
            )
            .map_err(|()| anyhow::anyhow!("active semantic-store generation was not authentic"))?;
            Ok(SemanticStoreBoundarySample {
                disabled: disabled.point,
                enabled: enabled.point,
                regression,
                provider_evidence,
                cold_ready: semantic_store.cold_ready,
                warm_ready: semantic_store.warm_ready,
                fingerprint,
            })
        })()?;
        samples.push(sample);
    }

    let disabled_control = aggregate_curve_points(samples.iter().map(|sample| &sample.disabled))?;
    let measured = aggregate_curve_points(samples.iter().map(|sample| &sample.enabled))?;
    let disabled_diagnostics_digest = expected_diagnostics_digest
        .ok_or_else(|| anyhow::anyhow!("semantic-store boundary produced no diagnostics digest"))?;
    let regression = evaluate_paired_semantic_store_regression_budget(
        &disabled_control,
        &measured,
        &disabled_diagnostics_digest,
        &disabled_diagnostics_digest,
    );
    Ok(SemanticStoreBoundaryReport {
        regression,
        disabled_control,
        measured,
        diagnostics_digest: disabled_diagnostics_digest,
        samples,
    })
}

#[cfg(all(test, target_os = "linux"))]
fn evaluate_semantic_store_scale_boundary(
    repo_root: &std::path::Path,
) -> anyhow::Result<SemanticStoreScaleReport> {
    use std::process::Command;

    use crate::analysis_kernel::AnalysisKernel;
    use crate::eval::bench::runner::{
        IsolatedPerfRunner, SemanticStoreBenchMode, diagnostics_digest_for_repo_with_store_mode,
    };

    (|| -> anyhow::Result<SemanticStoreScaleReport> {
        let isolated_runner = IsolatedPerfRunner::capture()?;
        let pair = tempfile::tempdir()?;
        let disabled_root = pair.path().join("disabled");
        let enabled_root = pair.path().join("enabled");
        for target in [&disabled_root, &enabled_root] {
            let output = Command::new("git")
                .args(["clone", "--quiet", "--no-local"])
                .arg(repo_root)
                .arg(target)
                .output()?;
            if !output.status.success() {
                anyhow::bail!(
                    "cloning semantic-store scale fixture failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        // Give both clones identical cache priming without publishing a store.
        // Enabled is measured first, so prime it first as well. The disabled
        // clone is then the more recently touched control.
        let enabled_prime = diagnostics_digest_for_repo_with_store_mode(
            &enabled_root,
            SemanticStoreBenchMode::Disabled,
        )?;
        let disabled_prime = diagnostics_digest_for_repo_with_store_mode(
            &disabled_root,
            SemanticStoreBenchMode::Disabled,
        )?;
        anyhow::ensure!(
            disabled_prime == enabled_prime,
            "independently primed scale fixtures changed diagnostics"
        );

        // Measure enabled first so host/process warm-up cannot make the store
        // path look artificially cheaper than its disabled control. Both runs
        // use their isolated absolute process peaks for the blocking RSS check;
        // their raw within-run high-water growth remains in the returned points.
        let enabled = isolated_runner.run_measurement(
            &enabled_root,
            None,
            SemanticStoreBenchMode::Enabled,
        )?;
        let disabled = isolated_runner.run_measurement(
            &disabled_root,
            None,
            SemanticStoreBenchMode::Disabled,
        )?;
        validate_semantic_store_pair_shape(&disabled.point, &enabled.point)?;
        let provider_evidence = validate_provider_availability_pair(&disabled, &enabled)?;
        validate_fully_available_provider_evidence(&provider_evidence)?;
        validate_scale_go_semantic_workload(&provider_evidence)?;
        let semantic_store = enabled.semantic_store.ok_or_else(|| {
            anyhow::anyhow!("enabled scale child omitted semantic-store readiness evidence")
        })?;
        let store_path = crate::cache::CacheLayout::for_repo(&enabled_root).semantic_store_path();
        let fingerprint = AnalysisKernel::semantic_store_boundary_fingerprint_for_test(&store_path)
            .map_err(|()| {
                anyhow::anyhow!("active scale semantic-store generation was not authentic")
            })?;

        let enabled_digest = enabled.warm_diagnostics_digest.clone();
        anyhow::ensure!(
            enabled_digest == disabled_prime,
            "enabling the semantic store changed scale-fixture diagnostics"
        );
        let regression = evaluate_paired_semantic_store_regression_budget(
            &disabled.point,
            &enabled.point,
            &disabled_prime,
            &enabled_digest,
        );
        Ok(SemanticStoreScaleReport {
            regression,
            disabled: disabled.point,
            enabled: enabled.point,
            diagnostics_digest: enabled_digest,
            cold_ready: semantic_store.cold_ready,
            warm_ready: semantic_store.warm_ready,
            fingerprint,
        })
    })()
}

#[cfg(test)]
fn evaluate_committed_store_disabled_check_boundary(
    baseline_path: &std::path::Path,
) -> anyhow::Result<CommittedStoreDisabledCheckReport> {
    use crate::eval::bench::runner::{
        STORE_DISABLED_FIXTURE_CHECK_SUITE_ID, STORE_DISABLED_FIXTURE_FILE_COUNT,
        STORE_DISABLED_FIXTURE_REPO_ID, STORE_DISABLED_FIXTURE_SOURCE_BYTES,
        SemanticStoreBenchMode, diagnostics_digest_for_repo_with_store_mode,
        run_repo_perf_point_isolated_with_store_mode, write_store_disabled_fixture,
    };

    let baseline = StoreDisabledBaseline::load(baseline_path)?;
    anyhow::ensure!(
        baseline.repo_id == STORE_DISABLED_FIXTURE_REPO_ID
            && baseline.suite_id == STORE_DISABLED_FIXTURE_CHECK_SUITE_ID,
        "committed store-disabled check baseline names a different fixture: repo_id={}, suite_id={}",
        baseline.repo_id,
        baseline.suite_id
    );

    let fixture = tempfile::tempdir()?;
    write_store_disabled_fixture(fixture.path())?;

    // Match the committed reference's cache state without creating the
    // semantic store before its cold measurement. The post-measurement enabled
    // digest then proves that store publication preserved output. A zero raw
    // RSS delta is valid when child startup established the process peak.
    let disabled_digest = diagnostics_digest_for_repo_with_store_mode(
        fixture.path(),
        SemanticStoreBenchMode::Disabled,
    )?;
    anyhow::ensure!(
        disabled_digest == baseline.diagnostics_digest,
        "store-disabled diagnostics drifted from the committed fixed reference: measured={disabled_digest}, committed={}",
        baseline.diagnostics_digest
    );
    let measured = run_repo_perf_point_isolated_with_store_mode(
        fixture.path(),
        None,
        SemanticStoreBenchMode::Enabled,
    )?;
    anyhow::ensure!(
        measured.repo_file_count == STORE_DISABLED_FIXTURE_FILE_COUNT
            && measured.repo_source_bytes == STORE_DISABLED_FIXTURE_SOURCE_BYTES
            && measured.diff_files == 0
            && measured.diff_hunk_lines == 0,
        "committed store-disabled fixture shape drifted: measured={measured:?}"
    );

    let enabled_digest = diagnostics_digest_for_repo_with_store_mode(
        fixture.path(),
        SemanticStoreBenchMode::Enabled,
    )?;
    anyhow::ensure!(
        enabled_digest == disabled_digest,
        "enabling the semantic store changed committed-fixture diagnostics: disabled={disabled_digest}, enabled={enabled_digest}"
    );
    let regression = evaluate_regression_budget(
        &baseline,
        &measured,
        &BaselineThresholds::default(),
        Some(&enabled_digest),
    );
    Ok(CommittedStoreDisabledCheckReport {
        regression,
        measured,
        diagnostics_digest: enabled_digest,
    })
}

#[cfg(test)]
fn aggregate_curve_points<'a>(
    points: impl IntoIterator<Item = &'a CurvePoint>,
) -> anyhow::Result<CurvePoint> {
    use crate::eval::bench::curve::{BudgetExhaustionCounters, StoreSizeBytes};

    let points = points.into_iter().collect::<Vec<_>>();
    let first = points
        .first()
        .copied()
        .ok_or_else(|| anyhow::anyhow!("cannot aggregate an empty boundary sample"))?;
    if points.iter().any(|point| {
        point.repo_file_count != first.repo_file_count
            || point.repo_source_bytes != first.repo_source_bytes
            || point.diff_files != first.diff_files
            || point.diff_hunk_lines != first.diff_hunk_lines
    }) {
        anyhow::bail!("semantic-store boundary fixture shape changed between samples");
    }

    let mean = |value: fn(&CurvePoint) -> u64| -> u64 {
        let total = points
            .iter()
            .map(|point| u128::from(value(point)))
            .sum::<u128>();
        u64::try_from(total / points.len() as u128).unwrap_or(u64::MAX)
    };
    Ok(CurvePoint {
        repo_id: "semantic-store-supported-boundary".to_string(),
        repo_file_count: first.repo_file_count,
        repo_source_bytes: first.repo_source_bytes,
        diff_files: first.diff_files,
        diff_hunk_lines: first.diff_hunk_lines,
        cold_wall_clock_ms: mean(|point| point.cold_wall_clock_ms),
        warm_wall_clock_ms: mean(|point| point.warm_wall_clock_ms),
        peak_rss_bytes: mean(|point| point.peak_rss_bytes),
        peak_rss_delta_bytes: mean(|point| point.peak_rss_delta_bytes),
        size: StoreSizeBytes {
            cache_bytes: mean(|point| point.size.cache_bytes),
            store_bytes: mean(|point| point.size.store_bytes),
        },
        budget: BudgetExhaustionCounters {
            budget_exceeded: points
                .iter()
                .map(|point| point.budget.budget_exceeded)
                .max()
                .unwrap_or_default(),
            tokens_exhausted: points
                .iter()
                .map(|point| point.budget.tokens_exhausted)
                .max()
                .unwrap_or_default(),
            iteration_capped: points
                .iter()
                .map(|point| point.budget.iteration_capped)
                .max()
                .unwrap_or_default(),
        },
    })
}

/// Evaluate a measured [`CurvePoint`] against a store-disabled
/// [`StoreDisabledBaseline`] on the two locked regression budgets: peak RSS
/// (`max_peak_rss_ratio`) and cold wall-clock (`max_cold_wall_clock_ratio`). The
/// manual historical comparison can supply a committed reference when its
/// recorded context is understood. Same-host paired gates use their paired
/// evaluator instead so the RSS check consistently compares isolated absolute
/// process peaks.
///
/// The peak-RSS budget compares the raw within-run high-water growth in
/// `peak_rss_delta_bytes`, not the process-wide absolute `peak_rss_bytes`. This
/// generic path is suitable only when the measured and baseline process
/// contexts match; the paired same-host gates use isolated absolute peaks
/// instead because a child's raw growth can legitimately be zero.
///
/// Produces one [`GateCheck`] per budget; each Fails if the measured/baseline
/// ratio exceeds its budget, else Passes. A zero baseline denominator is an
/// explicit Fail ("missing baseline") rather than a divide-by-zero panic.
///
/// When `measured_diagnostics_digest` is `Some`, a diagnostics-parity check is
/// added and Fails if it differs from the baseline's `diagnostics_digest` — the
/// store must not change the diagnostics polint emits.
/// Callers without a measured digest pass `None`; callers that provide one opt
/// into the parity check.
///
/// The baseline `diagnostics_digest` is CHECK-scoped for both the check and
/// review baselines (see [`StoreDisabledBaseline::diagnostics_digest`]), so a
/// caller that opts into the parity check MUST pass a check-scoped measured
/// digest; a review-scoped (diff-subset) digest would spuriously fail.
pub(crate) fn evaluate_regression_budget(
    baseline: &StoreDisabledBaseline,
    measured: &CurvePoint,
    thresholds: &BaselineThresholds,
    measured_diagnostics_digest: Option<&str>,
) -> RegressionGateReport {
    let rss_check = ratio_budget_check(
        "peak_rss_delta_ratio",
        measured.peak_rss_delta_bytes,
        baseline.peak_rss_delta_bytes,
        thresholds.max_peak_rss_ratio,
        PEAK_RSS_ABS_FLOOR_BYTES,
    );
    evaluate_regression_budget_with_rss_check(
        baseline,
        measured,
        thresholds,
        measured_diagnostics_digest,
        rss_check,
    )
}

fn evaluate_regression_budget_with_rss_check(
    baseline: &StoreDisabledBaseline,
    measured: &CurvePoint,
    thresholds: &BaselineThresholds,
    measured_diagnostics_digest: Option<&str>,
    rss_check: GateCheck,
) -> RegressionGateReport {
    let mut checks = vec![
        rss_check,
        ratio_budget_check(
            "cold_wall_clock_ratio",
            measured.cold_wall_clock_ms,
            baseline.cold_wall_clock_ms,
            thresholds.max_cold_wall_clock_ratio,
            COLD_WALL_CLOCK_ABS_FLOOR_MS,
        ),
    ];
    if let Some(measured_digest) = measured_diagnostics_digest {
        checks.push(digest_parity_check(
            &baseline.diagnostics_digest,
            measured_digest,
        ));
    }
    let verdict = checks
        .iter()
        .map(|check| check.verdict)
        .max()
        .unwrap_or(GateVerdict::Pass);
    RegressionGateReport { verdict, checks }
}

/// Whether a report is blocking. True exactly when the verdict is
/// [`GateVerdict::Fail`], so an over-budget run cannot pass silently.
pub(crate) fn is_blocking(report: &RegressionGateReport) -> bool {
    report.verdict == GateVerdict::Fail
}

/// Build a "measured must not exceed its budget" check with an absolute noise
/// floor. The measured value may exceed the baseline by up to the larger
/// of the ratio budget (`baseline * budget`) and an absolute tolerance
/// (`baseline + abs_floor`) before it Fails. The floor keeps the gate robust to
/// ms/MB jitter against a small baseline, while the locked ratio still governs
/// any baseline whose ratio headroom already exceeds the floor. A zero baseline
/// denominator is a Fail with a "missing baseline" observation rather than a
/// divide-by-zero.
fn ratio_budget_check(
    metric: &str,
    measured: u64,
    baseline: u64,
    budget: f64,
    abs_floor: u64,
) -> GateCheck {
    if baseline == 0 {
        return GateCheck {
            metric: metric.to_string(),
            observed: "missing baseline (0 denominator)".to_string(),
            threshold: format!("<= {budget:.4}"),
            verdict: GateVerdict::Fail,
        };
    }
    let ratio = measured as f64 / baseline as f64;
    // Effective ceiling: the larger of the ratio budget and the absolute floor,
    // so a small baseline still tolerates `abs_floor` of jitter.
    let allowed = (baseline as f64 * budget).max(baseline as f64 + abs_floor as f64);
    GateCheck {
        metric: metric.to_string(),
        observed: format!("{ratio:.4}"),
        threshold: format!("<= {budget:.4} (or within +{abs_floor} absolute floor)"),
        verdict: if measured as f64 > allowed {
            GateVerdict::Fail
        } else {
            GateVerdict::Pass
        },
    }
}

#[cfg(test)]
fn isolated_absolute_rss_budget_check(
    disabled: u64,
    enabled: u64,
    budget: f64,
    abs_floor: u64,
) -> GateCheck {
    if disabled == 0 || enabled == 0 {
        return GateCheck {
            metric: ISOLATED_ABSOLUTE_RSS_METRIC.to_string(),
            observed: format!("measurement unavailable (disabled={disabled}, enabled={enabled})"),
            threshold: format!(
                "both > 0 and <= {budget:.4} (or within +{abs_floor} absolute floor)"
            ),
            verdict: GateVerdict::Fail,
        };
    }
    ratio_budget_check(
        ISOLATED_ABSOLUTE_RSS_METRIC,
        enabled,
        disabled,
        budget,
        abs_floor,
    )
}

/// Build a diagnostics-parity check: the store must not change
/// the diagnostics polint emits, so a measured digest that differs from the
/// baseline's `diagnostics_digest` is a Fail. Only evaluated when the caller
/// supplies a measured digest.
fn digest_parity_check(baseline_digest: &str, measured_digest: &str) -> GateCheck {
    let same = baseline_digest == measured_digest;
    GateCheck {
        metric: "diagnostics_digest_parity".to_string(),
        observed: if same {
            "match".to_string()
        } else {
            format!("changed ({measured_digest})")
        },
        threshold: format!("== {baseline_digest}"),
        verdict: if same {
            GateVerdict::Pass
        } else {
            GateVerdict::Fail
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::bench::curve::{BudgetExhaustionCounters, CurvePoint, StoreSizeBytes};

    /// A baseline with round numbers so the test ratios are exact.
    fn baseline() -> StoreDisabledBaseline {
        StoreDisabledBaseline {
            schema_version: crate::eval::baseline::STORE_DISABLED_BASELINE_SCHEMA_VERSION
                .to_string(),
            store_disabled: true,
            repo_id: "polint-tiny-fixture".to_string(),
            suite_id: "polint-tiny-fixture-check".to_string(),
            peak_rss_bytes: 120_000_000,
            peak_rss_delta_bytes: 100_000_000,
            cold_wall_clock_ms: 1000,
            warm_wall_clock_ms: 500,
            diagnostics_digest: "digest".to_string(),
            measurement_context: None,
        }
    }

    /// A measured point at `rss_ratio`x both baseline RSS fields and
    /// `cold_ratio`x baseline cold wall-clock. Generic tests consume the raw
    /// delta while paired semantic-store tests consume the isolated peak.
    fn measured(rss_ratio: f64, cold_ratio: f64) -> CurvePoint {
        let base = baseline();
        CurvePoint {
            repo_id: base.repo_id.clone(),
            repo_file_count: 2,
            repo_source_bytes: 256,
            diff_files: 0,
            diff_hunk_lines: 0,
            cold_wall_clock_ms: (base.cold_wall_clock_ms as f64 * cold_ratio) as u64,
            warm_wall_clock_ms: base.warm_wall_clock_ms,
            peak_rss_bytes: (base.peak_rss_bytes as f64 * rss_ratio) as u64,
            peak_rss_delta_bytes: (base.peak_rss_delta_bytes as f64 * rss_ratio) as u64,
            size: StoreSizeBytes::default(),
            budget: BudgetExhaustionCounters::default(),
        }
    }

    #[test]
    fn over_budget_peak_rss_fails_and_is_blocking() {
        // 1.25x peak RSS exceeds the +20% (1.20) budget.
        let report = evaluate_regression_budget(
            &baseline(),
            &measured(1.25, 1.0),
            &BaselineThresholds::default(),
            None,
        );
        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(is_blocking(&report));
        assert!(report.checks.iter().any(|check| {
            check.metric == "peak_rss_delta_ratio" && check.verdict == GateVerdict::Fail
        }));
    }

    #[test]
    fn within_budget_run_passes_and_is_not_blocking() {
        // 1.10x peak RSS and 1.15x cold wall-clock are both within budget.
        let report = evaluate_regression_budget(
            &baseline(),
            &measured(1.10, 1.15),
            &BaselineThresholds::default(),
            None,
        );
        assert_eq!(report.verdict, GateVerdict::Pass);
        assert!(!is_blocking(&report));
        assert!(
            report
                .checks
                .iter()
                .all(|check| check.verdict == GateVerdict::Pass)
        );
    }

    #[test]
    fn over_budget_cold_wall_clock_fails_the_cold_check() {
        // 1.30x cold wall-clock exceeds the +25% (1.25) budget, even with peak
        // RSS within budget.
        let report = evaluate_regression_budget(
            &baseline(),
            &measured(1.05, 1.30),
            &BaselineThresholds::default(),
            None,
        );
        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(is_blocking(&report));
        assert!(report.checks.iter().any(|check| {
            check.metric == "cold_wall_clock_ratio" && check.verdict == GateVerdict::Fail
        }));
        // The peak-RSS check stays a Pass — the two budgets are independent.
        assert!(report.checks.iter().any(|check| {
            check.metric == "peak_rss_delta_ratio" && check.verdict == GateVerdict::Pass
        }));
    }

    #[test]
    fn ratio_exactly_at_budget_passes() {
        // Exactly at the budget (1.20x RSS, 1.25x cold) is not "exceeds" — Pass.
        let report = evaluate_regression_budget(
            &baseline(),
            &measured(1.20, 1.25),
            &BaselineThresholds::default(),
            None,
        );
        assert_eq!(report.verdict, GateVerdict::Pass);
        assert!(!is_blocking(&report));
    }

    #[test]
    fn small_baseline_within_absolute_floor_passes_despite_ratio_breach() {
        // A tiny baseline: 20 ms cold, ~1 MB peak-RSS delta. The naive ratio
        // budgets (+25% cold = 5 ms, +20% RSS ~= 0.2 MB) would Fail on ordinary
        // jitter; the absolute floors exempt these sub-threshold deltas.
        let mut base = baseline();
        base.cold_wall_clock_ms = 20;
        base.peak_rss_delta_bytes = 1_000_000;

        let mut point = measured(1.0, 1.0);
        // Jitter far beyond the ratio budgets but inside the absolute floors.
        point.cold_wall_clock_ms = 40; // +20 ms < COLD_WALL_CLOCK_ABS_FLOOR_MS
        point.peak_rss_delta_bytes = 5_000_000; // +4 MB < PEAK_RSS_ABS_FLOOR_BYTES

        let report =
            evaluate_regression_budget(&base, &point, &BaselineThresholds::default(), None);
        assert_eq!(report.verdict, GateVerdict::Pass);
        assert!(!is_blocking(&report));
    }

    #[test]
    fn zero_baseline_denominator_fails_rather_than_panicking() {
        // A zero baseline peak-RSS delta is a missing-baseline failure, not a panic.
        let mut base = baseline();
        base.peak_rss_delta_bytes = 0;
        let report = evaluate_regression_budget(
            &base,
            &measured(1.0, 1.0),
            &BaselineThresholds::default(),
            None,
        );
        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(is_blocking(&report));
        assert!(report.checks.iter().any(|check| {
            check.metric == "peak_rss_delta_ratio"
                && check.observed.contains("missing baseline")
                && check.verdict == GateVerdict::Fail
        }));
    }

    #[test]
    fn changed_diagnostics_digest_fails_the_parity_check() {
        // A within-budget run whose diagnostics digest differs from the baseline
        // is a parity Fail: the store must not change the diagnostics polint
        // emits. The parity check is only added when a measured digest is
        // supplied.
        let report = evaluate_regression_budget(
            &baseline(),
            &measured(1.0, 1.0),
            &BaselineThresholds::default(),
            Some("a-different-digest"),
        );
        assert_eq!(report.verdict, GateVerdict::Fail);
        assert!(is_blocking(&report));
        assert!(report.checks.iter().any(|check| {
            check.metric == "diagnostics_digest_parity" && check.verdict == GateVerdict::Fail
        }));
    }

    #[test]
    fn matching_diagnostics_digest_passes_the_parity_check() {
        // The same digest as the baseline passes; a `None` measured digest adds
        // no parity check at all (the default).
        let base = baseline();
        let with_digest = evaluate_regression_budget(
            &base,
            &measured(1.0, 1.0),
            &BaselineThresholds::default(),
            Some(&base.diagnostics_digest),
        );
        assert_eq!(with_digest.verdict, GateVerdict::Pass);
        assert!(with_digest.checks.iter().any(|check| {
            check.metric == "diagnostics_digest_parity" && check.verdict == GateVerdict::Pass
        }));

        let without_digest = evaluate_regression_budget(
            &base,
            &measured(1.0, 1.0),
            &BaselineThresholds::default(),
            None,
        );
        assert!(
            without_digest
                .checks
                .iter()
                .all(|check| check.metric != "diagnostics_digest_parity"),
            "no parity check is added when the measured digest is absent"
        );
    }

    #[test]
    fn same_platform_baseline_uses_control_metrics_and_representative_identity() {
        let mut control = measured(0.75, 2.5);
        control.warm_wall_clock_ms = 777;

        let comparison = semantic_store_comparison_baseline(&control, "paired-disabled-digest");

        assert_eq!(comparison.repo_id, REPRESENTATIVE_BOUNDARY_REPO_ID);
        assert_eq!(comparison.suite_id, REPRESENTATIVE_BOUNDARY_SUITE_ID);
        assert_eq!(comparison.diagnostics_digest, "paired-disabled-digest");
        assert_eq!(
            comparison.peak_rss_delta_bytes,
            control.peak_rss_delta_bytes
        );
        assert_eq!(comparison.cold_wall_clock_ms, control.cold_wall_clock_ms);
        assert_eq!(comparison.warm_wall_clock_ms, control.warm_wall_clock_ms);
    }

    #[test]
    fn same_platform_baseline_preserves_zero_rss_for_generic_rejection() {
        let mut control = measured(0.75, 2.5);
        control.peak_rss_bytes = 0;
        control.peak_rss_delta_bytes = 0;

        let comparison = semantic_store_comparison_baseline(&control, "paired-disabled-digest");

        assert_eq!(comparison.peak_rss_bytes, 0);
        assert_eq!(comparison.peak_rss_delta_bytes, 0);
        let report = evaluate_regression_budget(
            &comparison,
            &measured(1.0, 1.0),
            &BaselineThresholds::default(),
            None,
        );
        assert!(is_blocking(&report));
        assert!(report.checks.iter().any(|check| {
            check.metric == "peak_rss_delta_ratio"
                && check.observed.contains("missing baseline")
                && check.verdict == GateVerdict::Fail
        }));
        assert_eq!(comparison.cold_wall_clock_ms, control.cold_wall_clock_ms);
    }

    #[test]
    fn per_sample_reports_expose_regression_hidden_by_aggregate_mean() {
        let disabled = [measured(1.0, 1.0), measured(1.0, 1.0)];
        let enabled = [measured(2.0, 2.0), measured(0.4, 0.4)];
        let disabled_aggregate =
            aggregate_curve_points(disabled.iter()).expect("aggregate disabled controls");
        let enabled_aggregate =
            aggregate_curve_points(enabled.iter()).expect("aggregate enabled samples");
        let aggregate_report = evaluate_paired_semantic_store_regression_budget(
            &disabled_aggregate,
            &enabled_aggregate,
            "digest",
            "digest",
        );
        assert!(
            !is_blocking(&aggregate_report),
            "the ratio of means demonstrates the masking condition"
        );

        let sample_reports = disabled
            .iter()
            .zip(&enabled)
            .map(|(disabled, enabled)| {
                evaluate_paired_semantic_store_regression_budget(
                    disabled, enabled, "digest", "digest",
                )
            })
            .collect::<Vec<_>>();
        assert!(is_blocking(&sample_reports[0]));
        assert!(!is_blocking(&sample_reports[1]));
    }

    fn representative_boundary_point() -> CurvePoint {
        let mut point = measured(1.0, 1.0);
        point.repo_file_count = REPRESENTATIVE_BOUNDARY_SHAPE.repo_file_count;
        point.repo_source_bytes = REPRESENTATIVE_BOUNDARY_SHAPE.repo_source_bytes;
        point.diff_files = REPRESENTATIVE_BOUNDARY_SHAPE.diff_files;
        point.diff_hunk_lines = REPRESENTATIVE_BOUNDARY_SHAPE.diff_hunk_lines;
        point
    }

    fn complete_provider_evidence() -> crate::eval::bench::runner::ProviderRunEvidence {
        use crate::eval::bench::runner::{
            EffectiveCapabilityEvidence, ProviderOutputEvidence, ProviderRunEvidence,
            RequestedCapabilityEvidence,
        };

        ProviderRunEvidence {
            go_semantic_facts: REPRESENTATIVE_GO_SEMANTIC_FACTS
                .iter()
                .map(|(family, count)| ((*family).to_string(), *count))
                .collect(),
            provider_outputs: crate::analysis_kernel::AnalysisKernel::provider_manifests()
                .iter()
                .map(|manifest| ProviderOutputEvidence {
                    provider_id: manifest.id.to_string(),
                    output_digest: format!("provider_output:{}", manifest.id),
                    validation: "native_trusted".to_string(),
                })
                .collect(),
            requested_capabilities: vec![RequestedCapabilityEvidence {
                capability: "dataflow".to_string(),
                language: None,
                support_status: "supported".to_string(),
                setup_status: "ready".to_string(),
            }],
            effective_capabilities: vec![EffectiveCapabilityEvidence {
                capability: "dataflow".to_string(),
                language: None,
                support_status: "supported".to_string(),
            }],
            provider_diagnostics: Vec::new(),
            capability_diagnostic_count: 0,
        }
    }

    fn provider_measurement(
        evidence: crate::eval::bench::runner::ProviderRunEvidence,
    ) -> crate::eval::bench::runner::PerfMeasurement {
        crate::eval::bench::runner::PerfMeasurement {
            point: representative_boundary_point(),
            cold_provider_evidence: evidence.clone(),
            warm_provider_evidence: evidence,
            cold_diagnostics_digest: "digest".to_string(),
            warm_diagnostics_digest: "digest".to_string(),
            semantic_store: None,
        }
    }

    #[test]
    fn boundary_pair_accepts_complete_equal_provider_evidence() {
        let disabled = provider_measurement(complete_provider_evidence());
        let enabled = provider_measurement(complete_provider_evidence());

        let evidence = validate_provider_evidence_pair(&disabled, &enabled)
            .expect("complete equal provider evidence must pass");

        assert_eq!(evidence.go_semantic_fact_count(), 59);
    }

    #[test]
    fn boundary_pair_rejects_cold_warm_provider_drift() {
        let mut disabled = provider_measurement(complete_provider_evidence());
        disabled.warm_provider_evidence.provider_outputs[0].output_digest =
            "provider_output:warm-drift".to_string();
        let enabled = provider_measurement(complete_provider_evidence());

        let error = validate_provider_evidence_pair(&disabled, &enabled)
            .expect_err("cold/warm provider drift must fail");

        assert!(
            error
                .to_string()
                .contains("cold/warm provider evidence drifted")
        );
    }

    #[test]
    fn boundary_pair_accepts_clone_specific_provider_output_digests() {
        let disabled = provider_measurement(complete_provider_evidence());
        let mut enabled_evidence = complete_provider_evidence();
        enabled_evidence.provider_outputs[0].output_digest =
            "provider_output:independent-clone".to_string();
        let enabled = provider_measurement(enabled_evidence);

        validate_provider_evidence_pair(&disabled, &enabled)
            .expect("clone-specific provider digests are not availability drift");
    }

    #[test]
    fn boundary_pair_rejects_disabled_enabled_provider_drift() {
        let disabled = provider_measurement(complete_provider_evidence());
        let mut enabled_evidence = complete_provider_evidence();
        enabled_evidence.go_semantic_facts[0].1 += 1;
        let enabled = provider_measurement(enabled_evidence);

        let error = validate_provider_evidence_pair(&disabled, &enabled)
            .expect_err("store-mode provider drift must fail");

        assert!(
            error
                .to_string()
                .contains("different provider availability")
        );
    }

    #[test]
    fn boundary_pair_rejects_provider_capability_and_diagnostic_drift() {
        use crate::eval::bench::runner::ProviderDiagnosticEvidence;

        let baseline = complete_provider_evidence();
        let mut variants = Vec::new();

        let mut provider_drift = baseline.clone();
        provider_drift.provider_outputs[0].validation = "provider_failed".to_string();
        variants.push(("provider validation", provider_drift));

        let mut capability_drift = baseline.clone();
        capability_drift.effective_capabilities[0].support_status = "unsupported".to_string();
        variants.push(("effective capability", capability_drift));

        let mut diagnostic_drift = baseline.clone();
        diagnostic_drift
            .provider_diagnostics
            .push(ProviderDiagnosticEvidence {
                rule_id: "polint/go-semantic".to_string(),
                message: "setup changed".to_string(),
            });
        variants.push(("provider diagnostic", diagnostic_drift));

        for (label, variant) in variants {
            let disabled = provider_measurement(baseline.clone());
            let enabled = provider_measurement(variant);
            let error = validate_provider_evidence_pair(&disabled, &enabled)
                .expect_err("cross-clone availability drift must fail");
            assert!(
                error
                    .to_string()
                    .contains("different provider availability"),
                "{label} drift produced the wrong error: {error}"
            );
        }
    }

    #[test]
    fn boundary_pair_rejects_identical_provider_failure() {
        let mut evidence = complete_provider_evidence();
        let provider = evidence
            .provider_outputs
            .iter_mut()
            .find(|provider| provider.provider_id == "polint.go.semantic")
            .expect("Go semantic provider evidence");
        provider.validation = "provider_failed".to_string();
        let disabled = provider_measurement(evidence.clone());
        let enabled = provider_measurement(evidence);

        let error = validate_provider_evidence_pair(&disabled, &enabled)
            .expect_err("matching provider-failed states must not pass");

        assert!(error.to_string().contains("not fully available"));
    }

    #[test]
    fn boundary_pair_rejects_identical_non_capability_provider_diagnostics() {
        use crate::eval::bench::runner::ProviderDiagnosticEvidence;

        for rule_id in ["polint/go-semantic", "polint/internal"] {
            let mut evidence = complete_provider_evidence();
            evidence
                .provider_diagnostics
                .push(ProviderDiagnosticEvidence {
                    rule_id: rule_id.to_string(),
                    message: "provider failed".to_string(),
                });
            let disabled = provider_measurement(evidence.clone());
            let enabled = provider_measurement(evidence);

            let error = validate_provider_evidence_pair(&disabled, &enabled)
                .expect_err("matching provider diagnostics must not pass");
            assert!(
                error.to_string().contains("not fully available"),
                "{rule_id} produced the wrong error: {error}"
            );
        }
    }

    #[test]
    fn boundary_pair_rejects_zero_go_semantic_facts() {
        let mut evidence = complete_provider_evidence();
        for (_, count) in &mut evidence.go_semantic_facts {
            *count = 0;
        }
        let disabled = provider_measurement(evidence.clone());
        let enabled = provider_measurement(evidence);

        let error = validate_provider_evidence_pair(&disabled, &enabled)
            .expect_err("empty Go semantic output must fail");

        assert!(
            error
                .to_string()
                .contains("Go semantic fact vector drifted")
        );
    }

    #[test]
    fn boundary_pair_rejects_setup_missing_provider_evidence() {
        let mut evidence = complete_provider_evidence();
        evidence.requested_capabilities[0].setup_status = "setup_missing".to_string();
        let disabled = provider_measurement(evidence.clone());
        let enabled = provider_measurement(evidence);

        let error = validate_provider_evidence_pair(&disabled, &enabled)
            .expect_err("setup-missing evidence must fail");

        assert!(error.to_string().contains("not fully available"));
    }

    #[test]
    fn boundary_pair_rejects_missing_capability_evidence() {
        for clear_requested in [true, false] {
            let mut evidence = complete_provider_evidence();
            if clear_requested {
                evidence.requested_capabilities.clear();
            } else {
                evidence.effective_capabilities.clear();
            }
            let disabled = provider_measurement(evidence.clone());
            let enabled = provider_measurement(evidence);

            let error = validate_provider_evidence_pair(&disabled, &enabled)
                .expect_err("missing capability evidence must fail");
            assert!(error.to_string().contains("not fully available"));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn scale_workload_requires_zero_valued_fact_families_to_be_present() {
        let mut evidence = complete_provider_evidence();
        evidence.go_semantic_facts = vec![
            ("functions".to_string(), SCALE_GATE_GO_FUNCTION_COUNT),
            ("packages".to_string(), SCALE_GATE_PACKAGE_COUNT),
        ];

        let error = validate_scale_go_semantic_workload(&evidence)
            .expect_err("missing package_errors evidence must fail");
        assert!(error.to_string().contains("complete Go semantic workload"));
    }

    #[test]
    fn boundary_pair_rejects_capability_diagnostics() {
        let mut evidence = complete_provider_evidence();
        evidence.capability_diagnostic_count = 1;
        let disabled = provider_measurement(evidence.clone());
        let enabled = provider_measurement(evidence);

        let error = validate_provider_evidence_pair(&disabled, &enabled)
            .expect_err("capability diagnostics must fail");

        assert!(error.to_string().contains("not fully available"));
    }

    #[test]
    fn paired_rss_gate_uses_isolated_absolute_peaks_when_raw_deltas_are_zero() {
        let mut disabled = representative_boundary_point();
        let mut enabled = measured(1.1, 1.0);
        enabled.repo_file_count = REPRESENTATIVE_BOUNDARY_SHAPE.repo_file_count;
        enabled.repo_source_bytes = REPRESENTATIVE_BOUNDARY_SHAPE.repo_source_bytes;
        disabled.peak_rss_delta_bytes = 0;
        enabled.peak_rss_delta_bytes = 0;

        let report = evaluate_paired_semantic_store_regression_budget(
            &disabled, &enabled, "digest", "digest",
        );

        assert!(!is_blocking(&report), "{report:#?}");
        assert!(report.checks.iter().any(|check| {
            check.metric == ISOLATED_ABSOLUTE_RSS_METRIC && check.verdict == GateVerdict::Pass
        }));
        assert!(
            report
                .checks
                .iter()
                .all(|check| check.metric != "peak_rss_delta_ratio")
        );
    }

    #[test]
    fn paired_rss_gate_blocks_an_over_budget_absolute_peak_when_raw_deltas_are_zero() {
        let mut disabled = representative_boundary_point();
        let mut enabled = measured(2.0, 1.0);
        disabled.peak_rss_delta_bytes = 0;
        enabled.peak_rss_delta_bytes = 0;

        let report = evaluate_paired_semantic_store_regression_budget(
            &disabled, &enabled, "digest", "digest",
        );

        assert!(is_blocking(&report), "{report:#?}");
        assert!(report.checks.iter().any(|check| {
            check.metric == ISOLATED_ABSOLUTE_RSS_METRIC && check.verdict == GateVerdict::Fail
        }));
    }

    #[test]
    fn paired_rss_gate_fails_when_either_isolated_absolute_peak_is_unavailable() {
        for zero_enabled in [false, true] {
            let mut disabled = representative_boundary_point();
            let mut enabled = representative_boundary_point();
            if zero_enabled {
                enabled.peak_rss_bytes = 0;
            } else {
                disabled.peak_rss_bytes = 0;
            }

            let report = evaluate_paired_semantic_store_regression_budget(
                &disabled, &enabled, "digest", "digest",
            );

            assert!(is_blocking(&report), "{report:#?}");
            assert!(report.checks.iter().any(|check| {
                check.metric == ISOLATED_ABSOLUTE_RSS_METRIC
                    && check.observed.contains("measurement unavailable")
                    && check.verdict == GateVerdict::Fail
            }));
        }
    }

    #[test]
    fn boundary_pair_rejects_mode_dependent_workload_shape() {
        let disabled = representative_boundary_point();
        let mut enabled = disabled.clone();
        enabled.repo_file_count += 1;

        let error = validate_semantic_store_boundary_pair_shape(&disabled, &enabled)
            .expect_err("disabled and enabled modes must measure the same workload");

        assert!(error.to_string().contains("different workload shapes"));
    }

    #[test]
    fn boundary_pair_rejects_shared_workload_shape_drift() {
        let mut disabled = representative_boundary_point();
        disabled.repo_source_bytes += 1;
        let enabled = disabled.clone();

        let error = validate_semantic_store_boundary_pair_shape(&disabled, &enabled)
            .expect_err("both modes drifting together must still reject the locked fixture");

        assert!(error.to_string().contains("fixture shape drifted"));
    }

    mod semantic_store_boundary {
        use std::fmt::Write as _;
        use std::path::{Path, PathBuf};
        use std::process::Command;

        use super::*;

        fn workspace_root() -> PathBuf {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("workspace root")
                .to_path_buf()
        }

        fn git(root: &Path, args: &[&str]) {
            let output = Command::new("git")
                .current_dir(root)
                .args(args)
                .output()
                .expect("git invocation");
            assert!(
                output.status.success(),
                "git {args:?} failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        const REPRESENTATIVE_PROVIDER_GENERATIONS: u64 = 23;
        const REPRESENTATIVE_REQUESTED_CAPABILITIES: u64 = 8;
        const REPRESENTATIVE_CAPABILITY_REQUESTERS: u64 = 8;
        const REPRESENTATIVE_LAYERS: u64 = 6;
        const REPRESENTATIVE_SUMMARIES: u64 = 0;
        const REPRESENTATIVE_QUERIES: u64 = 30;
        const REPRESENTATIVE_FACTS: u64 = 12_843;
        const REPRESENTATIVE_DIAGNOSTICS: u64 = 0;
        const REPRESENTATIVE_DEPENDENCY_EDGES: u64 = 578;
        const REPRESENTATIVE_VALIDATION_EVENTS: u64 = 20;
        const REPRESENTATIVE_PLANNED_ROWS: u64 = 14_687;
        const REPRESENTATIVE_STABLE_FACT_STORAGE_BYTES: u64 = 24_720_824;
        const REPRESENTATIVE_FACT_LOGICAL_BYTES: u64 = 18_713_396;
        const REPRESENTATIVE_SEMANTIC_LOGICAL_BYTES_WITHOUT_VARIABLE_GO_TOOL_DETAILS: u64 =
            19_943_943;
        const REPRESENTATIVE_GO_TOOL_DETAIL_OCCURRENCES: u64 = 2;
        const REPRESENTATIVE_FACT_DIGEST: &str = "fact_metadata:8236cc13a46afdf5";
        const REPRESENTATIVE_DIAGNOSTICS_DIGEST: &str = "cbf29ce484222325";
        const REPRESENTATIVE_GIT_ATTRIBUTES: &str = "* text eol=lf\n";
        const MIN_STABLE_FACT_STORAGE_BYTES: u64 = 23 * 1024 * 1024;
        const MAX_STABLE_FACT_STORAGE_BYTES: u64 = 25 * 1024 * 1024;
        const MIN_REPRESENTATIVE_STORE_BYTES: u64 = 10 * 1024 * 1024;
        const MAX_REPRESENTATIVE_STORE_BYTES: u64 = 13 * 1024 * 1024;
        const REPRESENTATIVE_INPUT_FILE_COUNTS_BY_LANGUAGE: &[(&str, u64)] =
            &[("go", 10), ("typescript", 10)];
        const REPRESENTATIVE_FUNCTION_COUNTS_BY_PROVIDER: &[(&str, u64)] =
            &[("polint.go.syntax", 10), ("polint.ts.syntax", 20)];
        const REPRESENTATIVE_FACT_COUNTS_BY_FAMILY: &[(&str, u64)] = &[
            ("AccessPath", 160),
            ("Alias", 9),
            ("AliasAnswer", 65),
            ("BasicBlock", 120),
            ("BranchObligation", 20),
            ("CallSite", 8),
            ("CallTarget", 40),
            ("CfgControlDependence", 40),
            ("CfgDominator", 230),
            ("CfgEdge", 280),
            ("CfgFunction", 30),
            ("CfgNode", 290),
            ("CfgPostDominator", 270),
            ("CfgReachability", 120),
            ("ComplexityMetric", 20),
            ("DataFlowEdge", 536),
            ("DataFlowNode", 434),
            ("Definition", 98),
            ("DomainObservation", 6_630),
            ("EvidenceEdge", 576),
            ("EvidenceNode", 514),
            ("Export", 20),
            ("FileMetric", 20),
            ("Function", 30),
            ("FunctionMetric", 20),
            ("GeneratedSymbol", 20),
            ("Import", 18),
            ("ImportToPackage", 18),
            ("MirBody", 30),
            ("MirOperation", 170),
            ("ModuleEdge", 48),
            ("ModuleNode", 40),
            ("NarrowedType", 30),
            ("Package", 10),
            ("Place", 160),
            ("PointsToConstraint", 100),
            ("PointsToSet", 40),
            ("Reference", 337),
            ("RefinedCallEdge", 82),
            ("RepoTopologyOverlay", 28),
            ("Resolution", 375),
            ("ResolvedImport", 18),
            ("Scope", 100),
            ("SemanticImport", 18),
            ("SourceFile", 20),
            ("SourceSet", 20),
            ("StableExport", 20),
            ("StringLiteral", 9),
            ("SummaryCall", 30),
            ("SummaryControl", 30),
            ("SummaryMemory", 30),
            ("SummaryTito", 30),
            ("Symbol", 99),
            ("TopologyPackage", 1),
            ("Type", 190),
            ("Value", 140),
            ("WorkspaceRoot", 2),
        ];
        const REPRESENTATIVE_FACT_COUNTS_BY_PROVIDER: &[(&str, u64)] = &[
            ("polint.abstract_domains", 6_630),
            ("polint.calls", 48),
            ("polint.cfg", 1_380),
            ("polint.data_flow", 970),
            ("polint.direct_summaries", 120),
            ("polint.evidence", 1_090),
            ("polint.go.syntax", 49),
            ("polint.metrics", 60),
            ("polint.module_graph", 157),
            ("polint.module_topology", 18),
            ("polint.refined_calls", 82),
            ("polint.semantic_mir", 360),
            ("polint.source", 20),
            ("polint.symbol_graph", 1_096),
            ("polint.ts.syntax", 38),
            ("polint.type_value_alias", 725),
        ];

        fn assert_count_vector(actual: &[(String, u64)], expected: &[(&str, u64)], label: &str) {
            let actual = actual
                .iter()
                .map(|(name, count)| (name.as_str(), *count))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{label} drifted");
        }

        fn go_scale_source(file: usize) -> String {
            let mut source = format!("package pkg{file}\n\n");
            let has_next_package = file + 1 < SUPPORTED_BOUNDARY_FILE_PAIRS;
            if has_next_package {
                writeln!(
                    source,
                    "import \"example.com/polintgate/src/pkg{}\"\n",
                    file + 1
                )
                .expect("format representative Go import");
            }
            for member in 0..FUNCTIONS_PER_SUPPORTED_BOUNDARY_FILE {
                let call = if has_next_package {
                    format!("pkg{}.Step", file + 1)
                } else {
                    "Step".to_string()
                };
                writeln!(
                    source,
                    "func Step(value int, limit int) int {{\n\tcandidate := value + limit\n\tif candidate > limit {{\n\t\tcandidate = {call}(candidate-limit, limit)\n\t}} else {{\n\t\tcandidate = {call}(candidate+1, limit)\n\t}}\n\treturn candidate\n}}\n"
                )
                .expect("format representative Go function");
                debug_assert_eq!(member, 0, "the representative package has one function");
            }
            source
        }

        fn ts_scale_source(file: usize) -> String {
            let mut source = String::new();
            let has_next_module = file + 1 < SUPPORTED_BOUNDARY_FILE_PAIRS;
            if has_next_module {
                writeln!(
                    source,
                    "import {{ step as nextStep }} from \"../mod{}/scale\";\n",
                    file + 1
                )
                .expect("format representative TypeScript import");
            }
            for member in 0..FUNCTIONS_PER_SUPPORTED_BOUNDARY_FILE {
                let call = if has_next_module { "nextStep" } else { "step" };
                writeln!(
                    source,
                    "export function step(value: number, limit: number): number {{\n  let candidate = value + limit;\n  if (candidate > limit) {{\n    candidate = {call}(candidate - limit, limit);\n  }} else {{\n    candidate = {call}(candidate + 1, limit);\n  }}\n  return candidate;\n}}\n"
                )
                .expect("format representative TypeScript function");
                debug_assert_eq!(member, 0, "the representative module has one function");
            }
            source
        }

        fn representative_source_bytes() -> u64 {
            (0..SUPPORTED_BOUNDARY_FILE_PAIRS)
                .map(|file| go_scale_source(file).len() + ts_scale_source(file).len())
                .map(|bytes| u64::try_from(bytes).expect("fixture source length fits u64"))
                .sum()
        }

        fn representative_semantic_logical_bytes(
            toolchain_version_detail: &str,
            host_target_detail: &str,
        ) -> u64 {
            let variable_tool_detail_bytes = toolchain_version_detail
                .len()
                .checked_add(host_target_detail.len())
                .and_then(|bytes| u64::try_from(bytes).ok())
                .expect("variable Go tool identity length fits u64");
            REPRESENTATIVE_SEMANTIC_LOGICAL_BYTES_WITHOUT_VARIABLE_GO_TOOL_DETAILS
                + REPRESENTATIVE_GO_TOOL_DETAIL_OCCURRENCES * variable_tool_detail_bytes
        }

        #[test]
        fn semantic_logical_bytes_account_for_variable_go_tool_identity_width() {
            for (toolchain_version_detail, host_target_detail, expected) in [
                (
                    "toolchain_version=go1.25.12",
                    "host_target=linux/amd64",
                    19_944_043,
                ),
                (
                    "toolchain_version=go1.25.12",
                    "host_target=darwin/arm64",
                    19_944_045,
                ),
                (
                    "toolchain_version=go1.25.12",
                    "host_target=windows/amd64",
                    19_944_047,
                ),
                (
                    "toolchain_version=go1.26.2",
                    "host_target=darwin/arm64",
                    19_944_043,
                ),
            ] {
                assert_eq!(
                    representative_semantic_logical_bytes(
                        toolchain_version_detail,
                        host_target_detail,
                    ),
                    expected,
                    "unexpected semantic-byte contract for {toolchain_version_detail} {host_target_detail}"
                );
            }
        }

        #[test]
        fn supported_boundary_source_contract_is_pinned() {
            let source_bytes = representative_source_bytes();
            assert_eq!(
                source_bytes, REPRESENTATIVE_BOUNDARY_SHAPE.repo_source_bytes,
                "update the exact source-byte contract after intentional fixture changes"
            );
            assert!(
                REPRESENTATIVE_FUNCTION_COUNT * 1024 <= source_bytes * 5,
                "supported-boundary fixture exceeds five functions per KiB"
            );
        }

        #[test]
        fn supported_boundary_fixture_commits_stable_source_line_endings() {
            let repo = tempfile::tempdir().expect("semantic-store fixture repo");
            write_supported_boundary_fixture(repo.path());

            let committed = Command::new("git")
                .current_dir(repo.path())
                .args(["show", "HEAD:.gitattributes"])
                .output()
                .expect("read committed fixture attributes");
            assert!(
                committed.status.success(),
                "read committed fixture attributes: {}",
                String::from_utf8_lossy(&committed.stderr)
            );
            assert_eq!(committed.stdout, REPRESENTATIVE_GIT_ATTRIBUTES.as_bytes());

            let attributes = Command::new("git")
                .current_dir(repo.path())
                .args([
                    "check-attr",
                    "eol",
                    "--",
                    "src/pkg0/scale.go",
                    "src/mod0/scale.ts",
                    "go.mod",
                ])
                .output()
                .expect("resolve fixture source attributes");
            assert!(
                attributes.status.success(),
                "resolve fixture source attributes: {}",
                String::from_utf8_lossy(&attributes.stderr)
            );
            let attributes =
                String::from_utf8(attributes.stdout).expect("git check-attr output must be UTF-8");
            assert!(attributes.contains("src/pkg0/scale.go: eol: lf"));
            assert!(attributes.contains("src/mod0/scale.ts: eol: lf"));
            assert!(attributes.contains("go.mod: eol: lf"));
        }

        fn write_supported_boundary_fixture(root: &Path) {
            git(root, &["init", "--quiet"]);
            git(root, &["config", "user.email", "t@example.com"]);
            git(root, &["config", "user.name", "Test"]);
            git(root, &["config", "commit.gpgsign", "false"]);
            git(root, &["config", "core.autocrlf", "false"]);
            std::fs::write(root.join(".gitattributes"), REPRESENTATIVE_GIT_ATTRIBUTES)
                .expect("write fixture line-ending contract");
            std::fs::create_dir_all(root.join("src")).expect("create source directory");
            std::fs::write(
                root.join("go.mod"),
                "module example.com/polintgate\n\ngo 1.25\n",
            )
            .expect("write Go module");
            for index in 0..SUPPORTED_BOUNDARY_FILE_PAIRS {
                std::fs::create_dir_all(root.join(format!("src/pkg{index}")))
                    .expect("create representative Go package");
                std::fs::create_dir_all(root.join(format!("src/mod{index}")))
                    .expect("create representative TypeScript module");
                std::fs::write(
                    root.join(format!("src/pkg{index}/scale.go")),
                    go_scale_source(index),
                )
                .expect("write representative Go fixture");
                std::fs::write(
                    root.join(format!("src/mod{index}/scale.ts")),
                    ts_scale_source(index),
                )
                .expect("write representative TypeScript fixture");
            }
            git(root, &["add", "-A"]);
            git(root, &["commit", "--quiet", "-m", "base"]);
        }

        #[cfg(target_os = "linux")]
        fn write_scale_gate_fixture(root: &Path) {
            git(root, &["init", "--quiet"]);
            git(root, &["config", "user.email", "t@example.com"]);
            git(root, &["config", "user.name", "Test"]);
            git(root, &["config", "commit.gpgsign", "false"]);
            git(root, &["config", "core.autocrlf", "false"]);
            std::fs::write(root.join(".gitattributes"), REPRESENTATIVE_GIT_ATTRIBUTES)
                .expect("write scale fixture line-ending contract");
            std::fs::write(
                root.join("go.mod"),
                "module example.com/polint-scale-gate\n\ngo 1.25\n",
            )
            .expect("write scale fixture Go module");
            std::fs::create_dir_all(root.join("src")).expect("create scale source directory");
            std::fs::write(
                root.join("src/router.go"),
                "package app\n\nfunc handle() { helper() }\n\nfunc helper() { println(1) }\n",
            )
            .expect("write scale Go root");
            std::fs::write(
                root.join("src/util.ts"),
                "export function add(a: number, b: number): number {\n  return a + b;\n}\n",
            )
            .expect("write scale TypeScript root");

            for index in 0..SCALE_GATE_FILE_PAIRS {
                let mut go_source = String::from("package app\n\n");
                let mut ts_source = String::new();
                let functions = if index < SCALE_GATE_FUNCTION_FILE_PAIRS {
                    SCALE_GATE_FUNCTIONS_PER_FILE
                } else {
                    0
                };
                for member in 0..functions {
                    let symbol = index * SCALE_GATE_FUNCTIONS_PER_FILE + member;
                    writeln!(go_source, "func scale_{symbol}() int {{ return {symbol} }}")
                        .expect("format scale Go function");
                    writeln!(
                        ts_source,
                        "export function scale{symbol}(): number {{ return {symbol}; }}"
                    )
                    .expect("format scale TypeScript function");
                }
                std::fs::write(root.join(format!("src/scale_{index:04}.go")), go_source)
                    .expect("write scale Go fixture");
                std::fs::write(root.join(format!("src/scale_{index:04}.ts")), ts_source)
                    .expect("write scale TypeScript fixture");
            }
            git(root, &["add", "-A"]);
            git(root, &["commit", "--quiet", "-m", "base"]);
        }

        #[test]
        fn committed_store_disabled_artifacts_match_fixture_contract() {
            use crate::eval::bench::runner::{
                STORE_DISABLED_FIXTURE_CHECK_SUITE_ID, STORE_DISABLED_FIXTURE_REPO_ID,
                STORE_DISABLED_FIXTURE_REVIEW_SUITE_ID, STORE_DISABLED_FIXTURE_VERSION,
                store_disabled_fixture_digest,
            };

            let fixture_digest = store_disabled_fixture_digest();
            for (filename, suite_id) in [
                (
                    "store-disabled-check.json",
                    STORE_DISABLED_FIXTURE_CHECK_SUITE_ID,
                ),
                (
                    "store-disabled-review.json",
                    STORE_DISABLED_FIXTURE_REVIEW_SUITE_ID,
                ),
            ] {
                let baseline_path = workspace_root()
                    .join("research/evaluation-harness/baselines")
                    .join(filename);
                let baseline = StoreDisabledBaseline::load(&baseline_path)
                    .unwrap_or_else(|error| panic!("load {}: {error:#}", baseline_path.display()));
                assert_eq!(baseline.repo_id, STORE_DISABLED_FIXTURE_REPO_ID);
                assert_eq!(baseline.suite_id, suite_id);
                let context = baseline
                    .measurement_context
                    .as_ref()
                    .expect("validated committed artifact must carry measurement context");
                assert_eq!(context.fixture_version, STORE_DISABLED_FIXTURE_VERSION);
                assert_eq!(context.fixture_digest, fixture_digest);
            }
        }

        #[test]
        #[ignore = "manual historical comparison; numeric values are nonportable across measurement contexts"]
        fn informational_nonportable_committed_numeric_reference() {
            let baseline_path = workspace_root()
                .join("research/evaluation-harness/baselines/store-disabled-check.json");
            let fixed = evaluate_committed_store_disabled_check_boundary(&baseline_path)
                .expect("evaluate committed store-disabled check boundary");

            eprintln!(
                "informational committed store-disabled comparison: files={} source_bytes={} rss_delta={} cold_ms={} digest={} verdict={:?} checks={:?}",
                fixed.measured.repo_file_count,
                fixed.measured.repo_source_bytes,
                fixed.measured.peak_rss_delta_bytes,
                fixed.measured.cold_wall_clock_ms,
                fixed.diagnostics_digest,
                fixed.regression.verdict,
                fixed.regression.checks,
            );
        }

        #[cfg(target_os = "linux")]
        #[test]
        #[ignore = "runs as the dedicated serialized Linux scale performance gate"]
        fn generated_scale_store_enabled_measurement_passes_paired_budget() {
            let repo = tempfile::tempdir().expect("semantic-store scale fixture repo");
            write_scale_gate_fixture(repo.path());

            let scale = evaluate_semantic_store_scale_boundary(repo.path())
                .expect("evaluate generated semantic-store scale boundary");

            eprintln!(
                "semantic-store scale boundary: files={} source_bytes={} rss_absolute_ratio={:.4} cold_ratio={:.4} disabled_rss_peak={} enabled_rss_peak={} disabled_rss_delta={} enabled_rss_delta={} cold_ms={} store_bytes={} fact_count={} stable_fact_bytes={} stable_fact_limit={} fact_logical_bytes={} semantic_logical_bytes={} planned_rows={} input_languages={:?} function_providers={:?} digest={} checks={:?}",
                scale.enabled.repo_file_count,
                scale.enabled.repo_source_bytes,
                scale.enabled.peak_rss_bytes as f64 / scale.disabled.peak_rss_bytes as f64,
                scale.enabled.cold_wall_clock_ms as f64 / scale.disabled.cold_wall_clock_ms as f64,
                scale.disabled.peak_rss_bytes,
                scale.enabled.peak_rss_bytes,
                scale.disabled.peak_rss_delta_bytes,
                scale.enabled.peak_rss_delta_bytes,
                scale.enabled.cold_wall_clock_ms,
                scale.enabled.size.store_bytes,
                scale.fingerprint.fact_count,
                scale.fingerprint.stable_fact_storage_bytes,
                scale.fingerprint.stable_fact_storage_limit_bytes,
                scale.fingerprint.fact_logical_bytes,
                scale.fingerprint.semantic_logical_bytes,
                scale.fingerprint.planned_semantic_row_count,
                scale.fingerprint.input_file_counts_by_language,
                scale.fingerprint.function_counts_by_provider,
                scale.diagnostics_digest,
                scale.regression.checks,
            );

            assert_eq!(scale.enabled.repo_file_count, SCALE_GATE_REPO_FILE_COUNT);
            assert_eq!(scale.disabled.repo_file_count, SCALE_GATE_REPO_FILE_COUNT);
            assert_eq!(
                scale.enabled.repo_source_bytes, SCALE_GATE_REPO_SOURCE_BYTES,
                "enabled scale-fixture source shape drifted: {scale:#?}"
            );
            assert_eq!(
                scale.disabled.repo_source_bytes, SCALE_GATE_REPO_SOURCE_BYTES,
                "disabled scale-fixture source shape drifted: {scale:#?}"
            );
            assert!(
                scale.enabled.size.store_bytes >= MIN_SCALE_GATE_STORE_BYTES,
                "scale gate must exercise a durable projection larger than the authenticity smoke: {scale:#?}"
            );
            assert_eq!(
                scale.fingerprint.generation_count, 1,
                "warm scale reuse must not publish a second generation: {scale:#?}"
            );
            assert_eq!(
                scale.fingerprint.input_file_count, SCALE_GATE_REPO_FILE_COUNT,
                "published scale input count drifted from the measured fixture: {scale:#?}"
            );
            assert_count_vector(
                &scale.fingerprint.input_file_counts_by_language,
                SCALE_GATE_INPUT_FILE_COUNTS_BY_LANGUAGE,
                "scale input-language counts",
            );
            assert_count_vector(
                &scale.fingerprint.function_counts_by_provider,
                SCALE_GATE_FUNCTION_COUNTS_BY_PROVIDER,
                "scale function-provider counts",
            );
            assert!(
                scale.fingerprint.fact_count >= MIN_SCALE_GATE_FACT_COUNT,
                "scale gate did not retain its minimum fact workload: {scale:#?}"
            );
            assert!(
                scale.fingerprint.stable_fact_storage_bytes
                    >= MIN_SCALE_GATE_STABLE_FACT_STORAGE_BYTES,
                "scale gate did not retain its minimum charged stable-fact workload: {scale:#?}"
            );
            assert!(
                scale.fingerprint.stable_fact_storage_bytes
                    < scale.fingerprint.stable_fact_storage_limit_bytes,
                "scale gate exceeded the stable-fact handoff cap: {scale:#?}"
            );
            assert!(
                scale.fingerprint.fact_logical_bytes >= MIN_SCALE_GATE_FACT_LOGICAL_BYTES,
                "scale gate did not retain its minimum fact-logical workload: {scale:#?}"
            );
            assert!(
                scale.fingerprint.semantic_logical_bytes >= MIN_SCALE_GATE_SEMANTIC_LOGICAL_BYTES,
                "scale gate did not retain its minimum semantic-logical workload: {scale:#?}"
            );
            assert!(
                scale.fingerprint.planned_semantic_row_count >= MIN_SCALE_GATE_PLANNED_ROWS,
                "scale gate did not retain its minimum planned-row workload: {scale:#?}"
            );
            assert!(
                !scale.fingerprint.canonical_fact_digest.is_empty()
                    && !scale.fingerprint.canonical_generation_digest.is_empty(),
                "scale gate must retain canonical fact and generation identities: {scale:#?}"
            );
            assert!(scale.cold_ready, "enabled cold scale run was not Ready");
            assert!(scale.warm_ready, "enabled warm scale run was not Ready");
            assert!(!scale.diagnostics_digest.is_empty());
            assert_eq!(scale.enabled.budget, BudgetExhaustionCounters::default());
            assert_eq!(scale.disabled.budget, BudgetExhaustionCounters::default());
            assert!(!is_blocking(&scale.regression), "{scale:#?}");
            for metric in [
                ISOLATED_ABSOLUTE_RSS_METRIC,
                "cold_wall_clock_ratio",
                "diagnostics_digest_parity",
            ] {
                assert!(
                    scale.regression.checks.iter().any(|check| {
                        check.metric == metric && check.verdict == GateVerdict::Pass
                    }),
                    "missing passing scale check {metric}: {scale:#?}"
                );
            }
        }

        #[test]
        #[ignore = "runs as the dedicated serialized cross-platform authenticity smoke"]
        fn supported_boundary_authenticity_smoke_passes_paired_budget() {
            let repo = tempfile::tempdir().expect("semantic-store fixture repo");
            write_supported_boundary_fixture(repo.path());

            let boundary = evaluate_supported_semantic_store_boundary(repo.path())
                .expect("evaluate supported semantic-store boundary");

            eprintln!(
                "semantic-store boundary aggregate: files={} source_bytes={} rss_absolute_ratio={:.4} cold_ratio={:.4} disabled_rss_peak={} enabled_rss_peak={} disabled_rss_delta={} enabled_rss_delta={} cold_ms={} store_bytes={} checks={:?}",
                boundary.measured.repo_file_count,
                boundary.measured.repo_source_bytes,
                boundary.measured.peak_rss_bytes as f64
                    / boundary.disabled_control.peak_rss_bytes as f64,
                boundary.measured.cold_wall_clock_ms as f64
                    / boundary.disabled_control.cold_wall_clock_ms as f64,
                boundary.disabled_control.peak_rss_bytes,
                boundary.measured.peak_rss_bytes,
                boundary.disabled_control.peak_rss_delta_bytes,
                boundary.measured.peak_rss_delta_bytes,
                boundary.measured.cold_wall_clock_ms,
                boundary.measured.size.store_bytes,
                boundary.regression.checks
            );
            for (index, sample) in boundary.samples.iter().enumerate() {
                eprintln!(
                    "semantic-store boundary sample {index}: rss_absolute_ratio={:.4} cold_ratio={:.4} disabled_rss_delta={} enabled_rss_delta={} disabled={:?} enabled={:?} checks={:?} fingerprint={:?}",
                    sample.enabled.peak_rss_bytes as f64 / sample.disabled.peak_rss_bytes as f64,
                    sample.enabled.cold_wall_clock_ms as f64
                        / sample.disabled.cold_wall_clock_ms as f64,
                    sample.disabled.peak_rss_delta_bytes,
                    sample.enabled.peak_rss_delta_bytes,
                    sample.disabled,
                    sample.enabled,
                    sample.regression.checks,
                    sample.fingerprint,
                );
            }
            assert_eq!(
                boundary.measured.repo_file_count, REPRESENTATIVE_BOUNDARY_SHAPE.repo_file_count,
                "semantic-store boundary fixture file count drifted: {boundary:#?}"
            );
            assert_eq!(
                boundary.measured.repo_source_bytes,
                REPRESENTATIVE_BOUNDARY_SHAPE.repo_source_bytes,
                "semantic-store boundary fixture source bytes drifted: {boundary:#?}"
            );
            assert!(
                (MIN_REPRESENTATIVE_STORE_BYTES..=MAX_REPRESENTATIVE_STORE_BYTES)
                    .contains(&boundary.measured.size.store_bytes),
                "semantic-store boundary must exercise a material durable projection: {boundary:#?}"
            );
            assert_eq!(boundary.samples.len(), 2, "{boundary:#?}");
            for sample in &boundary.samples {
                assert!(
                    !is_blocking(&sample.regression),
                    "one semantic-store boundary sample exceeded the locked regression budget: {sample:#?}"
                );
                assert!(
                    sample.cold_ready,
                    "enabled cold run was not Ready: {sample:#?}"
                );
                assert!(
                    sample.warm_ready,
                    "enabled warm run was not Ready: {sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.generation_count, 1,
                    "warm reuse must not publish a second generation: {sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.input_file_count, sample.enabled.repo_file_count,
                    "published input count drifted from the measured fixture: {sample:#?}"
                );
                assert_count_vector(
                    &sample.fingerprint.input_file_counts_by_language,
                    REPRESENTATIVE_INPUT_FILE_COUNTS_BY_LANGUAGE,
                    "published input-language counts",
                );
                assert_eq!(
                    sample.fingerprint.requested_capability_count,
                    REPRESENTATIVE_REQUESTED_CAPABILITIES,
                    "published requested-capability count drifted: {sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.capability_requester_count,
                    REPRESENTATIVE_CAPABILITY_REQUESTERS,
                    "published capability-requester count drifted: {sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.provider_generation_count,
                    REPRESENTATIVE_PROVIDER_GENERATIONS,
                    "published provider-generation count drifted: {sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.layer_count, REPRESENTATIVE_LAYERS,
                    "{sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.summary_count, REPRESENTATIVE_SUMMARIES,
                    "{sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.query_count, REPRESENTATIVE_QUERIES,
                    "{sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.fact_count, REPRESENTATIVE_FACTS,
                    "{sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.diagnostic_count, REPRESENTATIVE_DIAGNOSTICS,
                    "{sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.dependency_edge_count, REPRESENTATIVE_DEPENDENCY_EDGES,
                    "{sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.validation_event_count, REPRESENTATIVE_VALIDATION_EVENTS,
                    "{sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.planned_semantic_row_count, REPRESENTATIVE_PLANNED_ROWS,
                    "{sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.stable_fact_storage_bytes,
                    REPRESENTATIVE_STABLE_FACT_STORAGE_BYTES,
                    "charged stable-fact storage drifted: {sample:#?}"
                );
                assert!(
                    (MIN_STABLE_FACT_STORAGE_BYTES..=MAX_STABLE_FACT_STORAGE_BYTES)
                        .contains(&sample.fingerprint.stable_fact_storage_bytes),
                    "supported boundary left the charged stable-fact band: {sample:#?}"
                );
                assert!(
                    sample.fingerprint.stable_fact_storage_bytes
                        < sample.fingerprint.stable_fact_storage_limit_bytes,
                    "supported boundary exceeded the stable-fact handoff cap: {sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.fact_logical_bytes, REPRESENTATIVE_FACT_LOGICAL_BYTES,
                    "fact logical bytes drifted: {sample:#?}"
                );
                assert_eq!(
                    sample.fingerprint.semantic_logical_bytes,
                    representative_semantic_logical_bytes(
                        &sample.fingerprint.go_toolchain_version_detail,
                        &sample.fingerprint.go_host_target_detail,
                    ),
                    "semantic logical bytes drifted: {sample:#?}"
                );
                assert_count_vector(
                    &sample.fingerprint.function_counts_by_provider,
                    REPRESENTATIVE_FUNCTION_COUNTS_BY_PROVIDER,
                    "published function-provider counts",
                );
                assert_count_vector(
                    &sample.fingerprint.fact_counts_by_family,
                    REPRESENTATIVE_FACT_COUNTS_BY_FAMILY,
                    "published fact-family counts",
                );
                assert_count_vector(
                    &sample.fingerprint.fact_counts_by_provider,
                    REPRESENTATIVE_FACT_COUNTS_BY_PROVIDER,
                    "published fact-provider counts",
                );
                assert_eq!(
                    sample.fingerprint.canonical_fact_digest, REPRESENTATIVE_FACT_DIGEST,
                    "canonical fact digest drifted: {sample:#?}"
                );
                assert!(
                    !sample.fingerprint.canonical_generation_digest.is_empty(),
                    "published generation must retain its canonical identity: {sample:#?}"
                );
                assert_eq!(
                    sample.enabled.budget,
                    BudgetExhaustionCounters::default(),
                    "enabled boundary exhausted an analysis budget: {sample:#?}"
                );
                assert_eq!(
                    sample.disabled.budget,
                    BudgetExhaustionCounters::default(),
                    "disabled control exhausted an analysis budget: {sample:#?}"
                );
                assert!(
                    (MIN_REPRESENTATIVE_STORE_BYTES..=MAX_REPRESENTATIVE_STORE_BYTES)
                        .contains(&sample.enabled.size.store_bytes),
                    "semantic-store sample did not exercise a material durable projection: {sample:#?}"
                );
            }
            assert_eq!(
                boundary.diagnostics_digest, REPRESENTATIVE_DIAGNOSTICS_DIGEST,
                "semantic-store boundary diagnostics drifted: {boundary:#?}"
            );
            assert!(!is_blocking(&boundary.regression), "{boundary:#?}");
            for metric in [
                ISOLATED_ABSOLUTE_RSS_METRIC,
                "cold_wall_clock_ratio",
                "diagnostics_digest_parity",
            ] {
                assert!(
                    boundary.regression.checks.iter().any(|check| {
                        check.metric == metric && check.verdict == GateVerdict::Pass
                    }),
                    "missing passing check {metric}: {boundary:#?}"
                );
            }
        }
    }
}
