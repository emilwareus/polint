use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{CacheStats, Digest, DigestKind, InputSnapshot};
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
use crate::go::semantic::process::GoSemanticProcessError;
use crate::go::semantic::store::GoSemanticFactsOutput;

#[derive(Debug, Clone, Default)]
pub(crate) struct GoSemanticProviderRunOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

pub(crate) fn derive_go_semantic_with_cache_stats(
    db: &mut AnalysisDb,
    loaded: &LoadedConfig,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    go_syntax_output_digest: Digest,
) -> GoSemanticProviderRunOutput {
    derive_go_semantic_with_runner(
        db,
        loaded,
        input_snapshot,
        manifest,
        go_syntax_output_digest,
        |config| GoSemanticClient::new(loaded.root.clone()).run(config),
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
}

fn store_output(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    parts: StoreOutputParts,
) -> GoSemanticProviderRunOutput {
    let output = parts.output.normalized();
    let output_digest = go_semantic_output_digest(
        manifest,
        input_snapshot,
        &parts.go_syntax_output_digest,
        &parts.digest_inputs,
        &parts.lifecycle,
        &output,
    );
    match db.replace_go_semantic_facts(output) {
        Ok(()) => GoSemanticProviderRunOutput {
            diagnostics: parts.diagnostics,
            cache_stats: parts.cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => GoSemanticProviderRunOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats: parts.cache_stats,
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
        format!("config={}", input_snapshot.config.digest),
        format!("go_syntax={go_syntax_output_digest}"),
        format!("input_digest={input_digest}"),
    ];
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
        && output.package_errors.is_empty()
    {
        parts.push("go_semantic_output=empty".to_string());
    }
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "go_semantic_output", &refs)
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
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{AnalysisDb, Language};
    use crate::go::semantic::protocol::decode_ndjson_str;
    use std::path::PathBuf;

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
        let input_snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config",
            "rules",
            AnalysisPlan::empty().digest(),
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
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
                    r#"{"schema":"polint-go-semantic-1","kind":"session_begin","go_version":"go1.25.0","x_tools_version":"v0.45.0"}
{"schema":"polint-go-semantic-1","kind":"package","package_id":"example.test/app","package_path":"example.test/app","files":["main.go"],"stable_key":"pkg"}
{"schema":"polint-go-semantic-1","kind":"session_end"}
"#,
                )
                .expect("protocol decodes");
                Ok(GoSemanticClientRun {
                    output,
                    frontend_digest: "sidecar".to_string(),
                })
            },
        );

        assert!(output.output_digest.is_some());
        assert_eq!(db.go_semantic_packages().len(), 1);
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

        assert!(output.output_digest.is_some());
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

        assert!(output.output_digest.is_some());
        assert_eq!(output.diagnostics.len(), 1);
        assert!(
            output.diagnostics[0]
                .message
                .contains("configured go.mod module root")
        );
    }

    fn input_snapshot_for(loaded: &LoadedConfig, db: &AnalysisDb) -> InputSnapshot {
        InputSnapshot::from_run_inputs(
            loaded,
            db,
            "config",
            "rules",
            AnalysisPlan::empty().digest(),
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        )
    }

    fn go_semantic_manifest() -> &'static ProviderManifest {
        crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.go.semantic")
            .expect("manifest exists")
    }
}
