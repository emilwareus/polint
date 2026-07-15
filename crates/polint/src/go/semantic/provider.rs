use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{CacheStats, Digest, DigestKind, InputSnapshot};
use crate::cache::keys::AnalysisSettingsScope;
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, Span};
use crate::diagnostics::{Diagnostic, TextRange};
use crate::go::lifecycle::{GoAnalysisConfig, go_files};
use crate::go::semantic::cache_key::{
    GoSemanticCacheInputs, go_semantic_input_digest, go_semantic_provider_parameter_digest,
};
use crate::go::semantic::client::{GoSemanticClient, GoSemanticClientError, GoSemanticClientRun};
use crate::go::semantic::diagnostics::{
    GoSemanticDiagnosticCategory, category_for_package_error, category_for_timeout,
    category_for_unsupported_go_version,
};
use crate::go::semantic::lower::lower_go_semantic;
use crate::go::semantic::process::{GoSemanticProcessError, GoSemanticToolPreparation};
use crate::go::semantic::store::{GoSemanticFactsOutput, StructuralDuplicateReport};

const REQUESTED_CAPABILITIES: &[&str] = &["calls", "control_flow", "dataflow"];

#[derive(Debug, Clone, Default)]
pub(crate) struct GoSemanticProviderRunOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome,
    pub(crate) output_digest: Option<Digest>,
}

pub(crate) fn derive_go_semantic_with_cache_stats(
    db: &mut AnalysisDb,
    loaded: &LoadedConfig,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    go_syntax_output_digest: Digest,
    tool_preparation: &GoSemanticToolPreparation,
) -> GoSemanticProviderRunOutput {
    derive_go_semantic_with_runner(
        db,
        loaded,
        input_snapshot,
        manifest,
        go_syntax_output_digest,
        |config| match tool_preparation {
            GoSemanticToolPreparation::Ready(frontend) => {
                GoSemanticClient::new(loaded.root.clone()).run_prepared(config, frontend)
            }
            GoSemanticToolPreparation::SetupMissing {
                process_error: Some(error),
                ..
            } => Err(GoSemanticClientError::Process(error.clone())),
            GoSemanticToolPreparation::SetupMissing { reason, .. }
            | GoSemanticToolPreparation::NotInvoked { reason } => Err(
                GoSemanticClientError::Process(GoSemanticProcessError::CommandUnavailable(
                    format!("Go semantic frontend was not prepared: {reason}"),
                )),
            ),
        },
    )
}

fn derive_go_semantic_with_runner(
    db: &mut AnalysisDb,
    loaded: &LoadedConfig,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    go_syntax_output_digest: Digest,
    runner: impl FnOnce(&GoAnalysisConfig) -> Result<GoSemanticClientRun, GoSemanticClientError>,
) -> GoSemanticProviderRunOutput {
    debug_assert_eq!(manifest.id, "polint.go.semantic");
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    let files = go_files(db);
    if files.is_empty() {
        return store_output(
            db,
            input_snapshot,
            manifest,
            StoreOutputParts {
                go_syntax_output_digest,
                output: GoSemanticFactsOutput::default(),
                lifecycle: default_lifecycle(),
                digest_inputs: EmptyDigestInputs::default().into(),
                cache_stats,
                diagnostics: Vec::new(),
                execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Skipped,
            },
        );
    }

    let config = match GoAnalysisConfig::from_loaded_files(loaded, &files) {
        Ok(config) => config,
        Err(error) => {
            return store_output(
                db,
                input_snapshot,
                manifest,
                StoreOutputParts {
                    go_syntax_output_digest,
                    output: GoSemanticFactsOutput::default(),
                    lifecycle: default_lifecycle(),
                    digest_inputs: EmptyDigestInputs::from_lifecycle_error(error.reason()).into(),
                    cache_stats,
                    diagnostics: vec![setup_missing_diagnostic(error.reason())],
                    execution:
                        crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed,
                },
            );
        }
    };

    if !config.files_without_module_root.is_empty() {
        let diagnostics = if go_module_roots_configured(loaded) {
            vec![setup_missing_diagnostic(
                "some Go files are not under a configured go.mod module root.",
            )]
        } else {
            Vec::new()
        };
        return store_output(
            db,
            input_snapshot,
            manifest,
            StoreOutputParts {
                go_syntax_output_digest,
                output: GoSemanticFactsOutput::default(),
                lifecycle: config,
                digest_inputs: EmptyDigestInputs::from_lifecycle_error(
                    "files outside module roots",
                )
                .into(),
                cache_stats,
                diagnostics,
                execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed,
            },
        );
    }

    let missing_roots = config.missing_module_roots(&loaded.root);
    if !missing_roots.is_empty() {
        return store_output(
            db,
            input_snapshot,
            manifest,
            StoreOutputParts {
                go_syntax_output_digest,
                output: GoSemanticFactsOutput::default(),
                lifecycle: config,
                digest_inputs: EmptyDigestInputs::from_lifecycle_error("missing module roots")
                    .into(),
                cache_stats,
                diagnostics: vec![setup_missing_diagnostic(&format!(
                    "configured Go module roots are missing go.mod: {}.",
                    missing_roots.join(", ")
                ))],
                execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed,
            },
        );
    }

    let run = match runner(&config) {
        Ok(run) => run,
        Err(error) => {
            return store_output(
                db,
                input_snapshot,
                manifest,
                StoreOutputParts {
                    go_syntax_output_digest,
                    output: GoSemanticFactsOutput::default(),
                    lifecycle: config,
                    digest_inputs: EmptyDigestInputs::from_client_error(&error).into(),
                    cache_stats,
                    diagnostics: vec![client_error_diagnostic(error)],
                    execution:
                        crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed,
                },
            );
        }
    };

    let lowered = match lower_go_semantic(db, &run.output) {
        Ok(output) => output,
        Err(error) => {
            return GoSemanticProviderRunOutput {
                diagnostics: vec![provider_error_diagnostic(error.to_string())],
                cache_stats,
                execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed,
                output_digest: None,
            };
        }
    };
    let diagnostics = package_error_diagnostics(&lowered);
    let digest_inputs = DigestInputs {
        sidecar_digest: run.frontend_digest,
        go_version: run.output.go_version,
        x_tools_version: run.output.x_tools_version,
    };
    store_output(
        db,
        input_snapshot,
        manifest,
        StoreOutputParts {
            go_syntax_output_digest,
            output: lowered,
            lifecycle: config,
            digest_inputs,
            cache_stats,
            diagnostics,
            execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Succeeded,
        },
    )
}

#[derive(Debug, Clone)]
struct DigestInputs {
    sidecar_digest: String,
    go_version: String,
    x_tools_version: String,
}

#[derive(Debug, Clone, Default)]
struct EmptyDigestInputs {
    reason: String,
}

impl EmptyDigestInputs {
    fn from_lifecycle_error(reason: &str) -> Self {
        Self {
            reason: format!("lifecycle:{reason}"),
        }
    }

    fn from_client_error(error: &GoSemanticClientError) -> Self {
        Self {
            reason: format!("client:{error}"),
        }
    }
}

impl From<EmptyDigestInputs> for DigestInputs {
    fn from(inputs: EmptyDigestInputs) -> Self {
        Self {
            sidecar_digest: format!("not-run:{}", inputs.reason),
            go_version: "not-run".to_string(),
            x_tools_version: "not-run".to_string(),
        }
    }
}

struct StoreOutputParts {
    go_syntax_output_digest: Digest,
    output: GoSemanticFactsOutput,
    lifecycle: GoAnalysisConfig,
    cache_stats: CacheStats,
    diagnostics: Vec<Diagnostic>,
    digest_inputs: DigestInputs,
    execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome,
}

fn store_output(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    parts: StoreOutputParts,
) -> GoSemanticProviderRunOutput {
    let output = parts.output.normalized();
    match db.replace_go_semantic_facts(output) {
        // `replace_go_semantic_facts` returns the resilience report: the count of malformed
        // RTA-signal harvest rows it dropped (FIX 3) and the duplicate STRUCTURAL rows it
        // collapsed keep-first (FIX-08). Surface a counted diagnostic for each so a systematic
        // frontend regression (e.g. every method_set losing its stable_key, or an emitter
        // double-emitting one structural key) is OBSERVABLE rather than silently
        // under-resolving — or, in the structural case, no longer catastrophically zeroing all
        // Go RTA repo-wide.
        Ok(report) => {
            // Compute the digest after store-time resilience has run, over the rows that were
            // actually persisted. This keeps the output digest aligned with the certified DB
            // state even when invalid harvest rows are dropped or duplicate structural rows are
            // collapsed keep-first.
            let stored_output = db.go_semantic_facts_output();
            let output_digest = go_semantic_output_digest(
                manifest,
                input_snapshot,
                &parts.go_syntax_output_digest,
                &parts.digest_inputs,
                &parts.lifecycle,
                &stored_output,
            );
            let mut diagnostics = parts.diagnostics;
            if report.dropped_harvest_rows > 0 {
                diagnostics.push(dropped_harvest_rows_diagnostic(report.dropped_harvest_rows));
            }
            diagnostics.extend(structural_duplicate_diagnostics(
                &report.structural_duplicates,
            ));
            GoSemanticProviderRunOutput {
                diagnostics,
                cache_stats: parts.cache_stats,
                execution: parts.execution,
                output_digest: (parts.execution
                    == crate::analysis_kernel::incremental::ProviderExecutionOutcome::Succeeded)
                    .then_some(output_digest),
            }
        }
        Err(error) => GoSemanticProviderRunOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats: parts.cache_stats,
            execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed,
            output_digest: None,
        },
    }
}

fn go_semantic_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    go_syntax_output_digest: &Digest,
    digest_inputs: &DigestInputs,
    lifecycle: &GoAnalysisConfig,
    output: &GoSemanticFactsOutput,
) -> Digest {
    let cache_inputs = GoSemanticCacheInputs {
        sidecar_digest: digest_inputs.sidecar_digest.clone(),
        go_version: digest_inputs.go_version.clone(),
        x_tools_version: digest_inputs.x_tools_version.clone(),
        upstream_digest: go_syntax_output_digest.to_string(),
        lifecycle: lifecycle.clone(),
    };
    let input_digest = go_semantic_input_digest(&cache_inputs);
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", go_semantic_provider_parameter_digest()),
        format!(
            "analysis_settings={}",
            input_snapshot.analysis_settings_digest(AnalysisSettingsScope::GoSemantic)
        ),
        format!(
            "requested_capabilities={}",
            input_snapshot.analysis_requirements_digest_for(REQUESTED_CAPABILITIES)
        ),
        format!("go_syntax={go_syntax_output_digest}"),
        format!("input_digest={input_digest}"),
    ];
    parts.extend(
        input_snapshot
            .go_lifecycle
            .components
            .iter()
            .map(go_lifecycle_identity_part),
    );
    parts.extend(output.packages.iter().map(|package| {
        format!(
            "package={} id={} path={} name={} module={} files={}",
            package.stable_key,
            package.package_id,
            package.package_path,
            package.package_name,
            package.module_path,
            package.files.join(",")
        )
    }));
    parts.extend(output.functions.iter().map(|function| {
        format!(
            "function={} package={} qualified={} kind={:?} file={} span={}",
            function.stable_key,
            function.package_path,
            function.qualified,
            function.kind,
            function.relative_file.as_deref().unwrap_or(""),
            option_span_part(function.span.as_ref())
        )
    }));
    parts.extend(output.callsites.iter().map(|callsite| {
        format!(
            "callsite={} package={} caller={} static={} status={:?} file={} span={}",
            callsite.stable_key,
            callsite.package_path,
            callsite.caller,
            callsite.static_callee.as_deref().unwrap_or(""),
            callsite.status,
            callsite.relative_file.as_deref().unwrap_or(""),
            option_span_part(callsite.span.as_ref())
        )
    }));
    parts.extend(output.method_sets.iter().map(|method_set| {
        format!(
            "method_set={} package={} type={} methods={}",
            method_set.stable_key,
            method_set.package_path,
            method_set.type_name,
            method_set.methods.join(",")
        )
    }));
    // FIX 4: the three RTA-signal harvest families (`instantiated_types`, `address_taken`,
    // `dynamic_dispatch`) change the RTA-resolved derived edges but were NOT folded here, so
    // a Go edit touching ONLY them changed neither this digest nor the downstream solver
    // digest — a stale-edge false-cache-hit risk once a persistent cache lands. Fold each
    // row's content (mirroring `method_sets`); the output is `parts.sort()`-ed below, so
    // insertion order does not matter. The instantiated_type FILTER is the RTA discriminant,
    // so its membership must invalidate; the address-taken set drives func-value resolution;
    // the dynamic-dispatch discriminant decides which callees a callsite resolves to.
    parts.extend(output.instantiated_types.iter().map(|instantiated_type| {
        format!(
            "instantiated_type={} package={} type={}",
            instantiated_type.stable_key,
            instantiated_type.package_path,
            instantiated_type.type_name
        )
    }));
    parts.extend(output.address_taken.iter().map(|address_taken| {
        format!(
            "address_taken={} package={} function={}",
            address_taken.stable_key, address_taken.package_path, address_taken.function
        )
    }));
    parts.extend(output.dynamic_dispatch.iter().map(|dynamic_dispatch| {
        format!(
            "dynamic_dispatch={} package={} caller={} callsite={} interface={} method={} signature={}",
            dynamic_dispatch.stable_key,
            dynamic_dispatch.package_path,
            dynamic_dispatch.caller,
            dynamic_dispatch.callsite_stable_key,
            dynamic_dispatch.interface_type.as_deref().unwrap_or(""),
            dynamic_dispatch.method.as_deref().unwrap_or(""),
            dynamic_dispatch.signature.as_deref().unwrap_or("")
        )
    }));
    parts.extend(output.rta_edges.iter().map(|edge| {
        format!(
            "rta_edge={} package={} caller={} callee={} kind={}",
            edge.stable_key, edge.package_path, edge.caller, edge.callee, edge.edge_kind
        )
    }));
    parts.extend(output.package_errors.iter().map(|package_error| {
        format!(
            "package_error={} package={} message={}",
            package_error.stable_key, package_error.package_path, package_error.message
        )
    }));
    if output.packages.is_empty()
        && output.functions.is_empty()
        && output.callsites.is_empty()
        && output.method_sets.is_empty()
        && output.instantiated_types.is_empty()
        && output.address_taken.is_empty()
        && output.dynamic_dispatch.is_empty()
        && output.rta_edges.is_empty()
        && output.package_errors.is_empty()
    {
        parts.push("go_semantic_output=empty".to_string());
    }
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "go_semantic_output", &refs)
}

fn go_lifecycle_identity_part(
    component: &crate::analysis_kernel::incremental::InputComponent,
) -> String {
    format!(
        "go_lifecycle:{}:{}:{}",
        component.name,
        component.status.label(),
        component.digest
    )
}

fn option_span_part(span: Option<&Span>) -> String {
    span.map(span_part).unwrap_or_else(|| "none".to_string())
}

fn go_module_roots_configured(loaded: &LoadedConfig) -> bool {
    loaded.config.languages.go.contains_key("module_roots")
        || loaded.config.languages.go.contains_key("module_root")
}

fn span_part(span: &Span) -> String {
    format!(
        "{}:{}..{}:{}@{}..{}",
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col,
        span.start_byte,
        span.end_byte
    )
}

fn package_error_diagnostics(output: &GoSemanticFactsOutput) -> Vec<Diagnostic> {
    output
        .package_errors
        .iter()
        .map(|error| {
            category_diagnostic(
                category_for_package_error(),
                format!(
                    "package {} failed to load: {}",
                    error.package_path, error.message
                ),
            )
        })
        .collect()
}

fn setup_missing_diagnostic(reason: &str) -> Diagnostic {
    category_diagnostic(category_for_package_error(), reason.to_string())
}

/// Observable signal that malformed RTA-signal harvest rows were dropped (FIX 3). Routed
/// through the same diagnostic channel the provider uses for setup/diagnostic messages so
/// a systematic frontend regression surfaces loudly instead of being swallowed into
/// repo-wide under-resolution. NOT fatal (the row-resilience contract, FINDING B) — just
/// visible.
fn dropped_harvest_rows_diagnostic(dropped: usize) -> Diagnostic {
    category_diagnostic(
        category_for_package_error(),
        format!(
            "{dropped} Go RTA-signal rows dropped (invalid identity/stable_key); \
             interface/func-value dispatch may under-resolve."
        ),
    )
}

/// Observable signal that duplicate STRUCTURAL rows (packages/functions/method_sets) were
/// collapsed keep-first (FIX-08). A SINGLE duplicate structural stable key used to make
/// `validate_unique` reject the whole output, leaving the DB with ZERO Go facts and driving
/// RTA to zero edges repo-wide (recurring 3×); the store now collapses the duplicate and
/// surfaces this counted diagnostic so the emitter regression is loud instead of catastrophic.
/// A "conflicting" duplicate (same stable_key, DIFFERING rows) can only come from a
/// stable-key-recipe bug, so its message is ESCALATED. Empty (no diagnostics) on a clean run.
fn structural_duplicate_diagnostics(report: &StructuralDuplicateReport) -> Vec<Diagnostic> {
    if report.total() == 0 {
        return Vec::new();
    }
    let families = [
        ("package", report.packages),
        ("function", report.functions),
        ("method_set", report.method_sets),
    ];
    families
        .into_iter()
        .filter(|(_, dropped)| *dropped > 0)
        .map(|(family, dropped)| {
            let message = if report.conflicting {
                format!(
                    "conflicting duplicate Go {family} stable key — {dropped} row(s) collapsed \
                     keep-first; possible identity-recipe bug (rows differ for one stable key)."
                )
            } else {
                format!(
                    "{dropped} duplicate Go {family} stable key row(s) collapsed keep-first \
                     (byte-identical double-emit); facts preserved."
                )
            };
            category_diagnostic(category_for_package_error(), message)
        })
        .collect()
}

fn client_error_diagnostic(error: GoSemanticClientError) -> Diagnostic {
    let category = match &error {
        GoSemanticClientError::Process(GoSemanticProcessError::Timeout(_)) => {
            category_for_timeout()
        }
        GoSemanticClientError::Process(GoSemanticProcessError::VersionUnsupported(_)) => {
            category_for_unsupported_go_version()
        }
        GoSemanticClientError::Process(_) | GoSemanticClientError::Protocol(_) => {
            category_for_package_error()
        }
    };
    category_diagnostic(category, error.to_string())
}

fn category_diagnostic(category: GoSemanticDiagnosticCategory, message: String) -> Diagnostic {
    Diagnostic::warning(
        "polint/go-semantic",
        "<workspace>",
        TextRange::point(1, 1),
        format!("{}: {message}", category.as_str()),
    )
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        format!("Go semantic provider failed: {message}"),
    )
}

fn default_lifecycle() -> GoAnalysisConfig {
    GoAnalysisConfig {
        module_roots: vec![".".to_string()],
        package_patterns: vec!["./...".to_string()],
        build_tags: Vec::new(),
        include_tests: true,
        offline: false,
        files_without_module_root: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{
        Digest, DigestKind, InputComponent, InputComponentStatus, InputSnapshot,
    };
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{AnalysisDb, Language};
    use crate::go::semantic::protocol::decode_ndjson_str;
    use std::path::PathBuf;

    #[test]
    fn lifecycle_identity_uses_canonical_lowercase_status_labels() {
        for status in [
            InputComponentStatus::Present,
            InputComponentStatus::Absent,
            InputComponentStatus::Unsupported,
            InputComponentStatus::SetupMissing,
        ] {
            let component = InputComponent {
                name: "go.tool_invocation".to_string(),
                status,
                digest: Digest::from_parts(DigestKind::ToolInvocation, "test", &[status.label()]),
                detail: Vec::new(),
            };
            let identity = go_lifecycle_identity_part(&component);

            assert!(identity.contains(&format!(":{}:", status.label())));
            assert!(!identity.contains(&format!(":{status:?}:")));
        }
    }

    #[test]
    fn output_identity_uses_only_declared_go_semantic_inputs() {
        let temp = tempfile::tempdir().expect("tempdir");
        let manifest = crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.go.semantic")
            .expect("Go semantic manifest");
        let syntax = Digest::absent(DigestKind::ProviderOutput, "go_syntax");
        let digest_inputs: DigestInputs = EmptyDigestInputs::default().into();
        let lifecycle = default_lifecycle();
        let output = GoSemanticFactsOutput::default();

        crate::analysis::provider::scoped_identity_test_support::assert_provider_identity(
            temp.path(),
            crate::cache::keys::AnalysisSettingsScope::GoSemantic,
            true,
            true,
            false,
            |snapshot| {
                super::go_semantic_output_digest(
                    manifest,
                    snapshot,
                    &syntax,
                    &digest_inputs,
                    &lifecycle,
                    &output,
                )
            },
        );
    }

    #[test]
    fn provider_lowers_and_stores_fake_sidecar_output() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.test/app\n\ngo 1.25\n",
        )
        .expect("write go.mod");
        std::fs::write(
            temp.path().join("main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("write main.go");
        let loaded = load_config(temp.path()).expect("config loads");
        let mut db = AnalysisDb::new();
        db.add_source_file(
            PathBuf::from("main.go"),
            "main.go".to_string(),
            Language::Go,
            "package main\nfunc main() {}\n".into(),
            "hash".to_string(),
        );
        let input_snapshot = input_snapshot_for(&loaded, &db);
        let manifest = crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.go.semantic")
            .expect("manifest exists");

        let output = derive_go_semantic_with_runner(
            &mut db,
            &loaded,
            &input_snapshot,
            manifest,
            Digest::absent(DigestKind::ProviderOutput, "polint.go.syntax"),
            |_config| {
                let output = decode_ndjson_str(
                    r#"{"schema":"polint-go-semantic-2","kind":"session_begin","go_version":"go1.25.0","x_tools_version":"v0.45.0"}
{"schema":"polint-go-semantic-2","kind":"package","package_id":"example.test/app","package_path":"example.test/app","files":["main.go"],"stable_key":"pkg"}
{"schema":"polint-go-semantic-2","kind":"session_end"}
"#,
                )
                .expect("protocol decodes");
                Ok(GoSemanticClientRun {
                    output,
                    frontend_digest: "sidecar".to_string(),
                })
            },
        );

        assert_eq!(
            output.execution,
            crate::analysis_kernel::incremental::ProviderExecutionOutcome::Succeeded
        );
        assert!(output.output_digest.is_some());
        assert_eq!(db.go_semantic_packages().len(), 1);
    }

    #[test]
    fn provider_surfaces_a_diagnostic_when_invalid_harvest_rows_are_dropped() {
        // FIX 3 (LOW): dropping invalid RTA-signal harvest rows must be OBSERVABLE. The
        // sidecar emits one discriminant-less dynamic_dispatch row (no interface_type, no
        // method, no signature); the store drops it (row-resilience, FINDING B) but the
        // provider must surface a counted diagnostic so a systematic frontend regression is
        // visible rather than silently under-resolving repo-wide.
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.test/app\n\ngo 1.25\n",
        )
        .expect("write go.mod");
        std::fs::write(
            temp.path().join("main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("write main.go");
        let loaded = load_config(temp.path()).expect("config loads");
        let mut db = AnalysisDb::new();
        db.add_source_file(
            PathBuf::from("main.go"),
            "main.go".to_string(),
            Language::Go,
            "package main\nfunc main() {}\n".into(),
            "hash".to_string(),
        );
        let input_snapshot = input_snapshot_for(&loaded, &db);
        let manifest = go_semantic_manifest();

        let output = derive_go_semantic_with_runner(
            &mut db,
            &loaded,
            &input_snapshot,
            manifest,
            Digest::absent(DigestKind::ProviderOutput, "polint.go.syntax"),
            |_config| {
                let output = decode_ndjson_str(
                    r#"{"schema":"polint-go-semantic-2","kind":"session_begin","go_version":"go1.25.0","x_tools_version":"v0.45.0"}
{"schema":"polint-go-semantic-2","kind":"package","package_id":"example.test/app","package_path":"example.test/app","files":["main.go"],"stable_key":"pkg"}
{"schema":"polint-go-semantic-2","kind":"dynamic_dispatch","package_id":"example.test/app","package_path":"example.test/app","caller":"example.test/app.main","callsite_stable_key":"cs","stable_key":"dd|bad"}
{"schema":"polint-go-semantic-2","kind":"session_end"}
"#,
                )
                .expect("protocol decodes");
                Ok(GoSemanticClientRun {
                    output,
                    frontend_digest: "sidecar".to_string(),
                })
            },
        );

        assert_eq!(
            output.execution,
            crate::analysis_kernel::incremental::ProviderExecutionOutcome::Succeeded
        );
        assert!(output.output_digest.is_some());
        // The discriminant-less row was dropped, so the DB holds zero dynamic-dispatch rows.
        assert!(db.go_semantic_dynamic_dispatch().is_empty());
        // The drop is surfaced as a diagnostic mentioning the count of dropped RTA-signal
        // rows (observable, not silent).
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("RTA-signal")
                    && diagnostic.message.contains("dropped")
                    && diagnostic.message.contains('1')
            }),
            "a dropped harvest row must surface a counted diagnostic: {:#?}",
            output.diagnostics
        );

        let expected_digest = go_semantic_output_digest(
            manifest,
            &input_snapshot,
            &Digest::absent(DigestKind::ProviderOutput, "polint.go.syntax"),
            &DigestInputs {
                sidecar_digest: "sidecar".to_string(),
                go_version: "go1.25.0".to_string(),
                x_tools_version: "v0.45.0".to_string(),
            },
            &default_lifecycle(),
            &db.go_semantic_facts_output(),
        );
        assert_eq!(
            output.output_digest,
            Some(expected_digest),
            "the go.semantic digest must certify the stored output after dropped-row cleanup"
        );
    }

    #[test]
    fn provider_surfaces_a_diagnostic_when_duplicate_structural_rows_are_collapsed() {
        // FIX-08 (CATASTROPHIC, recurring 3×): a SINGLE duplicate stable key in a structural
        // family (here two byte-identical `function` rows) used to make `validate_unique` →
        // Err → the provider stored ZERO Go facts → RTA derived zero edges repo-wide. The
        // store now collapses the duplicate keep-first BEFORE validation, so the valid facts
        // SURVIVE; the provider surfaces a counted diagnostic so the emitter regression is
        // OBSERVABLE rather than silently swallowed.
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.test/app\n\ngo 1.25\n",
        )
        .expect("write go.mod");
        std::fs::write(
            temp.path().join("main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("write main.go");
        let loaded = load_config(temp.path()).expect("config loads");
        let mut db = AnalysisDb::new();
        db.add_source_file(
            PathBuf::from("main.go"),
            "main.go".to_string(),
            Language::Go,
            "package main\nfunc main() {}\n".into(),
            "hash".to_string(),
        );
        let input_snapshot = input_snapshot_for(&loaded, &db);
        let manifest = go_semantic_manifest();

        let output = derive_go_semantic_with_runner(
            &mut db,
            &loaded,
            &input_snapshot,
            manifest,
            Digest::absent(DigestKind::ProviderOutput, "polint.go.syntax"),
            |_config| {
                // TWO byte-identical `function` rows with the SAME stable_key `fn|main`.
                let output = decode_ndjson_str(
                    r#"{"schema":"polint-go-semantic-2","kind":"session_begin","go_version":"go1.25.0","x_tools_version":"v0.45.0"}
{"schema":"polint-go-semantic-2","kind":"package","package_id":"example.test/app","package_path":"example.test/app","files":["main.go"],"stable_key":"pkg"}
{"schema":"polint-go-semantic-2","kind":"function","package_id":"example.test/app","package_path":"example.test/app","name":"main","qualified":"example.test/app.main","stable_key":"fn|main"}
{"schema":"polint-go-semantic-2","kind":"function","package_id":"example.test/app","package_path":"example.test/app","name":"main","qualified":"example.test/app.main","stable_key":"fn|main"}
{"schema":"polint-go-semantic-2","kind":"session_end"}
"#,
                )
                .expect("protocol decodes");
                Ok(GoSemanticClientRun {
                    output,
                    frontend_digest: "sidecar".to_string(),
                })
            },
        );

        // The duplicate did NOT zero the fact set: a digest was assigned and the function
        // survives EXACTLY ONCE (keep-first).
        assert!(output.output_digest.is_some());
        assert_eq!(db.go_semantic_functions().len(), 1);
        assert_eq!(
            db.go_semantic_functions()[0].qualified,
            "example.test/app.main"
        );
        // The collapse is surfaced as a diagnostic (observable, not silent). It is a benign
        // byte-identical double-emit, so it must NOT be flagged as "conflicting".
        assert!(
            output.diagnostics.iter().any(|diagnostic| {
                diagnostic.message.contains("duplicate")
                    && diagnostic.message.contains("function")
                    && diagnostic.message.contains("collapsed")
            }),
            "a collapsed structural duplicate must surface a diagnostic: {:#?}",
            output.diagnostics
        );
        assert!(
            !output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("conflicting")),
            "a byte-identical double-emit must NOT be flagged conflicting: {:#?}",
            output.diagnostics
        );

        let expected_digest = go_semantic_output_digest(
            manifest,
            &input_snapshot,
            &Digest::absent(DigestKind::ProviderOutput, "polint.go.syntax"),
            &DigestInputs {
                sidecar_digest: "sidecar".to_string(),
                go_version: "go1.25.0".to_string(),
                x_tools_version: "v0.45.0".to_string(),
            },
            &default_lifecycle(),
            &db.go_semantic_facts_output(),
        );
        assert_eq!(
            output.output_digest,
            Some(expected_digest),
            "the go.semantic digest must certify the stored output after duplicate collapse"
        );
    }

    #[test]
    fn provider_silently_stores_empty_output_for_inferred_missing_module_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("write main.go");
        let loaded = load_config(temp.path()).expect("config loads");
        let mut db = AnalysisDb::new();
        db.add_source_file(
            PathBuf::from("main.go"),
            "main.go".to_string(),
            Language::Go,
            "package main\nfunc main() {}\n".into(),
            "hash".to_string(),
        );
        let input_snapshot = input_snapshot_for(&loaded, &db);
        let manifest = go_semantic_manifest();

        let output = derive_go_semantic_with_runner(
            &mut db,
            &loaded,
            &input_snapshot,
            manifest,
            Digest::absent(DigestKind::ProviderOutput, "polint.go.syntax"),
            |_config| panic!("missing inferred module root should not run sidecar"),
        );

        assert_eq!(
            output.execution,
            crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed
        );
        assert!(output.output_digest.is_none());
        assert!(output.diagnostics.is_empty());
        assert!(db.go_semantic_packages().is_empty());
    }

    #[test]
    fn provider_warns_for_explicit_module_root_mismatch() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join(".polint.toml"),
            "[workspace]\ninclude = [\"**/*.go\"]\n\n[languages.go]\nmodule_roots = [\"module\"]\n",
        )
        .expect("write config");
        std::fs::write(
            temp.path().join("main.go"),
            "package main\nfunc main() {}\n",
        )
        .expect("write main.go");
        let loaded = load_config(temp.path()).expect("config loads");
        let mut db = AnalysisDb::new();
        db.add_source_file(
            PathBuf::from("main.go"),
            "main.go".to_string(),
            Language::Go,
            "package main\nfunc main() {}\n".into(),
            "hash".to_string(),
        );
        let input_snapshot = input_snapshot_for(&loaded, &db);
        let manifest = go_semantic_manifest();

        let output = derive_go_semantic_with_runner(
            &mut db,
            &loaded,
            &input_snapshot,
            manifest,
            Digest::absent(DigestKind::ProviderOutput, "polint.go.syntax"),
            |_config| panic!("explicit mismatch should not run sidecar"),
        );

        assert_eq!(
            output.execution,
            crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed
        );
        assert!(output.output_digest.is_none());
        assert_eq!(output.diagnostics.len(), 1);
        assert!(
            output.diagnostics[0]
                .message
                .contains("configured go.mod module root")
        );
    }

    fn input_snapshot_for(loaded: &LoadedConfig, db: &AnalysisDb) -> InputSnapshot {
        let empty_plan = AnalysisPlan::empty();
        assert!(empty_plan.requested_capability_snapshots().is_empty());
        let identity_sources = InputSnapshot::identity_sources_from_plan(loaded, &empty_plan);
        assert!(identity_sources.requested_capabilities.is_empty());
        assert_eq!(
            identity_sources.analysis_requirements_identity,
            Digest::absent(DigestKind::AnalysisRequirements, "requested_capabilities")
        );

        InputSnapshot::from_run_inputs_with_plan(
            loaded,
            db,
            "config",
            "rules",
            &empty_plan,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        )
    }

    fn go_semantic_manifest() -> &'static ProviderManifest {
        crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.go.semantic")
            .expect("manifest exists")
    }

    /// Compute `go_semantic_output_digest` for `output` with fixed (irrelevant-to-this-test)
    /// provider/lifecycle inputs, so a test can isolate the effect of the OUTPUT row content.
    fn output_digest_for(output: &GoSemanticFactsOutput) -> Digest {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let db = AnalysisDb::new();
        let input_snapshot = input_snapshot_for(&loaded, &db);
        go_semantic_output_digest(
            go_semantic_manifest(),
            &input_snapshot,
            &Digest::absent(DigestKind::ProviderOutput, "polint.go.syntax"),
            &DigestInputs {
                sidecar_digest: "sidecar".to_string(),
                go_version: "go1.25.0".to_string(),
                x_tools_version: "v0.45.0".to_string(),
            },
            &default_lifecycle(),
            output,
        )
    }

    /// FIX 4 part 1: the three RTA-signal harvest families
    /// (`instantiated_types` / `address_taken` / `dynamic_dispatch`) must each be FOLDED into
    /// `go_semantic_output_digest`. A Go edit that changes ONLY one of them changes the
    /// RTA-resolved edges, so the go.semantic output digest MUST change too (otherwise a
    /// persistent cache could serve stale RTA edges, WR-06). Before the fix only `method_sets`
    /// was folded, so each of these mutations left the digest unchanged.
    #[test]
    fn rta_harvest_families_participate_in_go_semantic_output_digest() {
        use crate::go::semantic::facts::{
            GoSemanticAddressTakenFact, GoSemanticAddressTakenId, GoSemanticDynamicDispatchFact,
            GoSemanticDynamicDispatchId, GoSemanticInstantiatedTypeFact,
            GoSemanticInstantiatedTypeId, GoSemanticRtaEdgeFact, GoSemanticRtaEdgeId,
        };

        // A non-empty base so the emptiness sentinel is not what carries the signal.
        let base = GoSemanticFactsOutput {
            instantiated_types: vec![GoSemanticInstantiatedTypeFact {
                id: GoSemanticInstantiatedTypeId(0),
                stable_key: "inst|pkg.Dog".to_string(),
                package_id: "pkg".to_string(),
                package_path: "pkg".to_string(),
                type_name: "pkg.Dog".to_string(),
            }],
            address_taken: vec![GoSemanticAddressTakenFact {
                id: GoSemanticAddressTakenId(0),
                stable_key: "at|pkg.handler".to_string(),
                package_id: "pkg".to_string(),
                package_path: "pkg".to_string(),
                function: "pkg.handler".to_string(),
            }],
            dynamic_dispatch: vec![GoSemanticDynamicDispatchFact {
                id: GoSemanticDynamicDispatchId(0),
                stable_key: "dd|pkg.main".to_string(),
                package_id: "pkg".to_string(),
                package_path: "pkg".to_string(),
                caller: "pkg.main".to_string(),
                callsite_stable_key: "cs|pkg.main".to_string(),
                interface_type: Some("pkg.Speaker".to_string()),
                method: Some("Speak".to_string()),
                signature: None,
            }],
            ..GoSemanticFactsOutput::default()
        }
        .normalized();
        let base_digest = output_digest_for(&base);

        // (a) Adding an instantiated type (the RTA rapid-type filter) changes the digest.
        let mut changed_instantiated = base.clone();
        changed_instantiated
            .instantiated_types
            .push(GoSemanticInstantiatedTypeFact {
                id: GoSemanticInstantiatedTypeId(0),
                stable_key: "inst|pkg.Cat".to_string(),
                package_id: "pkg".to_string(),
                package_path: "pkg".to_string(),
                type_name: "pkg.Cat".to_string(),
            });
        assert_ne!(
            base_digest,
            output_digest_for(&changed_instantiated.normalized()),
            "an instantiated_type change must invalidate the go.semantic output digest"
        );

        // (b) Adding an address-taken function (func-value candidate set) changes the digest.
        let mut changed_address_taken = base.clone();
        changed_address_taken
            .address_taken
            .push(GoSemanticAddressTakenFact {
                id: GoSemanticAddressTakenId(0),
                stable_key: "at|pkg.other".to_string(),
                package_id: "pkg".to_string(),
                package_path: "pkg".to_string(),
                function: "pkg.other".to_string(),
            });
        assert_ne!(
            base_digest,
            output_digest_for(&changed_address_taken.normalized()),
            "an address_taken change must invalidate the go.semantic output digest"
        );

        // (c) Changing the dynamic-dispatch discriminant (the invoked method) changes it.
        let mut changed_dispatch = base.clone();
        changed_dispatch.dynamic_dispatch[0].method = Some("Bark".to_string());
        assert_ne!(
            base_digest,
            output_digest_for(&changed_dispatch.normalized()),
            "a dynamic_dispatch discriminant change must invalidate the go.semantic output digest"
        );

        // (d) Adding a direct x/tools RTA edge changes it.
        let mut changed_rta_edge = base;
        changed_rta_edge.rta_edges.push(GoSemanticRtaEdgeFact {
            id: GoSemanticRtaEdgeId(0),
            stable_key: "rta|main|init1".to_string(),
            package_id: "pkg".to_string(),
            package_path: "pkg".to_string(),
            caller: "main".to_string(),
            callee: "init$1".to_string(),
            edge_kind: "dynamic function call".to_string(),
        });
        assert_ne!(
            base_digest,
            output_digest_for(&changed_rta_edge.normalized()),
            "an rta_edge change must invalidate the go.semantic output digest"
        );
    }
}
