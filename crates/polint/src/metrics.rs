use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheNode, CacheStats, DependencyEdge, DependencyKind, Digest, DigestKind, InputSnapshot,
    LayerCacheManifest, LayerCacheReadStatus, LayerCacheStore, LayerCacheWriteStatus, LayerKey,
    LayerKind, PrecisionTier, ShapeKind, dependency_layer_digest,
};
use crate::analysis_plan::AnalysisPlan;
use crate::cache::Cache;
use crate::core::{
    AnalysisDb, ComplexityMetricFact, FileId, FileMetricFact, FunctionFact, FunctionMetricFact,
    SourceFile, Span, is_synthetic_ts_js_module_function,
};
use crate::diagnostics::{Diagnostic, TextRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const METRIC_CAPABILITIES: &[&str] = &["file_metrics", "function_metrics", "complexity_metrics"];
const METRICS_LAYER_SCHEMA: &str = "metrics-facts-v1";

#[derive(Debug, Clone, Default)]
pub(crate) struct MetricsDerivation {
    pub(crate) cache_stats: CacheStats,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) output_digest: Option<Digest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MetricsLayerPayload {
    schema: String,
    file_metrics: Vec<FileMetricFact>,
    function_metrics: Vec<FunctionMetricFact>,
    complexity_metrics: Vec<ComplexityMetricFact>,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Compatibility wrapper remains for direct in-crate metrics derivation callers while the kernel uses the stats-returning cache path."
    )
)]
pub(crate) fn derive_requested_metrics(db: &mut AnalysisDb, plan: &AnalysisPlan) {
    let _ = derive_requested_metrics_uncached(db, plan);
}

pub(crate) fn derive_requested_metrics_with_cache_stats(
    db: &mut AnalysisDb,
    plan: &AnalysisPlan,
    cache: &Cache,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> MetricsDerivation {
    if !plan.requests_any_capability(METRIC_CAPABILITIES) {
        return MetricsDerivation::default();
    }

    let config_digest = input_snapshot.config.digest.clone();
    let layer_key = metrics_layer_key(
        db,
        manifest,
        config_digest.clone(),
        upstream_syntax_output_digests.clone(),
    );
    let store = cache.layer_cache_store();
    let mut cache_stats = CacheStats::default();
    let read = store
        .read_json_validated::<MetricsLayerPayload, _>(&layer_key, |payload, manifest| {
            validate_metrics_layer_payload(payload, manifest)
        });

    match read.status {
        LayerCacheReadStatus::Hit => {
            cache_stats.record_hit();
            cache_stats.record_verified_reuse();
            let payload = read
                .value
                .expect("layer cache hit should include metrics payload");
            restore_metrics_layer_payload(db, &payload);
            MetricsDerivation {
                cache_stats,
                diagnostics: Vec::new(),
                output_digest: read.output_digest,
            }
        }
        LayerCacheReadStatus::BypassedDisabled => {
            cache_stats.record_disabled_bypass();
            cache_stats.record_recompute();
            let mut derivation = derive_requested_metrics_uncached(db, plan);
            derivation.cache_stats = cache_stats;
            derivation
        }
        LayerCacheReadStatus::Miss | LayerCacheReadStatus::InvalidEvicted => {
            if read.status == LayerCacheReadStatus::Miss {
                cache_stats.record_miss();
            } else {
                cache_stats.record_invalid_evicted_read();
            }
            cache_stats.record_recompute();
            let mut derivation = derive_requested_metrics_uncached(db, plan);
            let payload = metrics_layer_payload(db);
            let dependencies = metrics_layer_dependency_edges(
                db,
                &layer_key,
                manifest,
                &upstream_syntax_output_digests,
                config_digest,
            );
            derivation.output_digest = write_metrics_layer_payload(
                &store,
                layer_key,
                &payload,
                dependencies,
                &mut cache_stats,
                &mut derivation.diagnostics,
            );
            derivation.cache_stats = cache_stats;
            derivation
        }
    }
}

pub(crate) fn metrics_layer_key(
    db: &AnalysisDb,
    manifest: &ProviderManifest,
    config_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> LayerKey {
    LayerKey::metrics_layer_key(
        manifest,
        metrics_source_text_digests(db),
        metrics_function_fact_digests(db),
        config_digest,
        upstream_syntax_output_digests,
        metrics_parameter_digest(),
    )
}

fn derive_requested_metrics_uncached(
    db: &mut AnalysisDb,
    plan: &AnalysisPlan,
) -> MetricsDerivation {
    if !plan.requests_any_capability(METRIC_CAPABILITIES) {
        return MetricsDerivation::default();
    }

    let mut function_counts = BTreeMap::<FileId, u32>::new();
    for function in db
        .functions()
        .iter()
        .filter(|function| !is_synthetic_ts_js_module_function(function))
    {
        let count = function_counts.entry(function.file).or_default();
        *count = count.saturating_add(1);
    }

    let file_metrics = db
        .files()
        .iter()
        .map(|file| FileMetricFact {
            file: file.id,
            language: file.language,
            line_count: line_count(&file.source),
            non_empty_line_count: non_empty_line_count(&file.source),
            byte_count: saturating_u32(file.source.len()),
            function_count: function_counts.get(&file.id).copied().unwrap_or_default(),
        })
        .collect::<Vec<_>>();

    let function_metrics = db
        .functions()
        .iter()
        .filter(|function| !is_synthetic_ts_js_module_function(function))
        .map(|function| FunctionMetricFact {
            function: function.id,
            file: function.file,
            name: function.name.clone(),
            span: function.span.clone(),
            language: function.language,
            line_count: span_line_count(&function.span),
            byte_count: function
                .span
                .end_byte
                .saturating_sub(function.span.start_byte),
        })
        .collect::<Vec<_>>();

    let complexity_metrics = db
        .functions()
        .iter()
        .filter(|function| !is_synthetic_ts_js_module_function(function))
        .map(|function| ComplexityMetricFact {
            function: function.id,
            file: function.file,
            name: function.name.clone(),
            span: function.span.clone(),
            language: function.language,
            cyclomatic_complexity: function.cyclomatic_complexity,
        })
        .collect::<Vec<_>>();

    db.replace_metric_facts(file_metrics, function_metrics, complexity_metrics);
    MetricsDerivation {
        cache_stats: CacheStats::default(),
        diagnostics: Vec::new(),
        output_digest: Some(metrics_output_digest_for_payload(
            &metrics_layer_payload(db),
            None,
        )),
    }
}

fn metrics_layer_dependency_edges(
    db: &AnalysisDb,
    key: &LayerKey,
    manifest: &ProviderManifest,
    upstream_syntax_output_digests: &[Digest],
    config_digest: Digest,
) -> Vec<DependencyEdge> {
    let from = CacheNode::Layer(key.clone());
    let mut edges = Vec::new();

    for file in sorted_files(db) {
        edges.push(dependency_edge(
            &from,
            CacheNode::Input(format!(
                "source:{}:{}",
                normalized_file_path(file),
                file.content_hash
            )),
            DependencyKind::SourceText,
            ShapeKind::Content,
        ));
    }

    for function in sorted_functions(db) {
        edges.push(dependency_edge(
            &from,
            CacheNode::Input(format!(
                "function:{}:{}:{}:{}:{}",
                db.path_for(function.file),
                function.name,
                function.span.start_byte,
                function.span.end_byte,
                language_cache_label(function.language)
            )),
            DependencyKind::Input,
            ShapeKind::Syntax,
        ));
    }

    edges.push(dependency_edge(
        &from,
        CacheNode::Input(format!("config:{}", config_digest)),
        DependencyKind::Config,
        ShapeKind::Unknown,
    ));
    edges.push(dependency_edge(
        &from,
        CacheNode::Input(format!(
            "provider_schema:{}:{}",
            manifest.id,
            manifest.primary_schema_label()
        )),
        DependencyKind::ProviderSchema,
        ShapeKind::ProviderVersion,
    ));
    edges.push(dependency_edge(
        &from,
        CacheNode::Input("toolchain:metrics:absent".to_string()),
        DependencyKind::Toolchain,
        ShapeKind::Toolchain,
    ));

    for (index, output_digest) in upstream_syntax_output_digests.iter().cloned().enumerate() {
        let (layer_kind, provider_id) = match index {
            0 => (LayerKind::GoSyntax, "polint.go.syntax"),
            1 => (LayerKind::TsSyntax, "polint.ts.syntax"),
            _ => (LayerKind::Extension, "polint.unknown_upstream"),
        };
        edges.push(dependency_edge(
            &from,
            CacheNode::Layer(upstream_layer_key(layer_kind, provider_id, output_digest)),
            DependencyKind::UpstreamLayer,
            ShapeKind::Output,
        ));
    }

    edges.sort();
    edges.dedup();
    edges
}

fn metrics_source_text_digests(db: &AnalysisDb) -> Vec<Digest> {
    sorted_files(db)
        .into_iter()
        .map(|file| {
            let parts = [
                normalized_file_path(file),
                file.content_hash.clone(),
                language_cache_label(file.language).to_string(),
            ];
            let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
            Digest::from_parts(DigestKind::SourceText, "metrics_source_text", &refs)
        })
        .collect()
}

fn metrics_function_fact_digests(db: &AnalysisDb) -> Vec<Digest> {
    sorted_functions(db)
        .into_iter()
        .map(|function| {
            let mut calls = function.calls.clone();
            calls.sort();
            calls.dedup();
            let parts = [
                db.path_for(function.file),
                function.name.clone(),
                function.span.start_byte.to_string(),
                function.span.end_byte.to_string(),
                function.span.start_line.to_string(),
                function.span.end_line.to_string(),
                function.is_test.to_string(),
                function.is_exported.to_string(),
                function.cyclomatic_complexity.to_string(),
                language_cache_label(function.language).to_string(),
                calls.join("\n"),
            ];
            let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "metrics_function_fact",
                &refs,
            )
        })
        .collect()
}

fn metrics_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "metrics_parameters",
        &[
            "output=file_metrics",
            "output=function_metrics",
            "output=complexity_metrics",
        ],
    )
}

fn metrics_layer_payload(db: &AnalysisDb) -> MetricsLayerPayload {
    let mut file_metrics = db.file_metrics().to_vec();
    let mut function_metrics = db.function_metrics().to_vec();
    let mut complexity_metrics = db.complexity_metrics().to_vec();
    sort_file_metrics(&mut file_metrics);
    sort_function_metrics(&mut function_metrics);
    sort_complexity_metrics(&mut complexity_metrics);

    MetricsLayerPayload {
        schema: METRICS_LAYER_SCHEMA.to_string(),
        file_metrics,
        function_metrics,
        complexity_metrics,
    }
}

fn restore_metrics_layer_payload(db: &mut AnalysisDb, payload: &MetricsLayerPayload) {
    let mut file_metrics = payload.file_metrics.clone();
    let mut function_metrics = payload.function_metrics.clone();
    let mut complexity_metrics = payload.complexity_metrics.clone();
    sort_file_metrics(&mut file_metrics);
    sort_function_metrics(&mut function_metrics);
    sort_complexity_metrics(&mut complexity_metrics);
    db.replace_metric_facts(file_metrics, function_metrics, complexity_metrics);
}

fn validate_metrics_layer_payload(
    payload: &MetricsLayerPayload,
    manifest: &LayerCacheManifest,
) -> bool {
    payload.schema == METRICS_LAYER_SCHEMA
        && manifest.output_digest == metrics_output_digest_for_payload(payload, Some(&manifest.key))
}

fn write_metrics_layer_payload(
    store: &LayerCacheStore,
    layer_key: LayerKey,
    payload: &MetricsLayerPayload,
    dependencies: Vec<DependencyEdge>,
    stats: &mut CacheStats,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Digest> {
    let payload_digest = match LayerCacheStore::payload_digest_for_json(payload) {
        Ok(digest) => digest,
        Err(error) => {
            diagnostics.push(cache_write_diagnostic("metrics layer", error));
            return None;
        }
    };
    let output_digest = metrics_output_digest(&layer_key, &payload_digest);
    let manifest = LayerCacheManifest::new(
        layer_key,
        output_digest.clone(),
        payload_digest,
        dependencies,
        PrecisionTier::Syntax,
        "native_trusted",
        Vec::new(),
    );

    match store.write_json(&manifest, payload) {
        Ok(LayerCacheWriteStatus::Written) => stats.record_write(),
        Ok(LayerCacheWriteStatus::BypassedDisabled) => stats.record_disabled_bypass(),
        Err(error) => diagnostics.push(cache_write_diagnostic("metrics layer", error)),
    }
    Some(output_digest)
}

fn cache_write_diagnostic(path: &str, error: anyhow::Error) -> Diagnostic {
    Diagnostic::warning(
        "internal/cache",
        path,
        TextRange::point(1, 1),
        format!("cache write failed: {error}"),
    )
}

fn metrics_output_digest_for_payload(
    payload: &MetricsLayerPayload,
    layer_key: Option<&LayerKey>,
) -> Digest {
    let payload_digest = LayerCacheStore::payload_digest_for_json(payload)
        .unwrap_or_else(|_| Digest::unsupported(DigestKind::LayerOutput, "metrics", "json"));
    if let Some(layer_key) = layer_key {
        metrics_output_digest(layer_key, &payload_digest)
    } else {
        Digest::from_parts(
            DigestKind::ProviderOutput,
            "metrics_layer_output",
            &[&payload_digest.to_string()],
        )
    }
}

fn metrics_output_digest(layer_key: &LayerKey, payload_digest: &Digest) -> Digest {
    let layer_key_json =
        serde_json::to_string(layer_key).unwrap_or_else(|_| "unserializable_layer_key".to_string());
    Digest::from_parts(
        DigestKind::ProviderOutput,
        "metrics_layer_output",
        &[&payload_digest.to_string(), &layer_key_json],
    )
}

fn sorted_files(db: &AnalysisDb) -> Vec<&SourceFile> {
    let mut files = db.files().iter().collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files
}

fn sorted_functions(db: &AnalysisDb) -> Vec<&FunctionFact> {
    let mut functions = db
        .functions()
        .iter()
        .filter(|function| !is_synthetic_ts_js_module_function(function))
        .collect::<Vec<_>>();
    functions.sort_by(|left, right| {
        (
            db.path_for(left.file),
            left.span.start_byte,
            left.span.end_byte,
            left.name.as_str(),
            left.id,
        )
            .cmp(&(
                db.path_for(right.file),
                right.span.start_byte,
                right.span.end_byte,
                right.name.as_str(),
                right.id,
            ))
    });
    functions
}

fn normalized_file_path(file: &SourceFile) -> String {
    crate::module_graph::paths::normalize_repo_relative(&file.relative_path)
        .unwrap_or_else(|| file.relative_path.clone())
}

fn dependency_edge(
    from: &CacheNode,
    to: CacheNode,
    kind: DependencyKind,
    required_shape: ShapeKind,
) -> DependencyEdge {
    DependencyEdge {
        from: from.clone(),
        to,
        kind,
        required_shape,
    }
}

fn upstream_layer_key(layer_kind: LayerKind, provider_id: &str, output_digest: Digest) -> LayerKey {
    let output_dependency = dependency_layer_digest(output_digest);

    LayerKey::new(
        layer_kind,
        provider_id,
        "output-digest",
        "output-digest",
        output_dependency.clone(),
        Digest::absent(DigestKind::DependencyLayer, "upstream_lifecycle_unknown"),
        Digest::absent(DigestKind::Config, "upstream_config_unknown"),
        Digest::absent(DigestKind::ToolInvocation, "upstream_toolchain_unknown"),
        vec![output_dependency],
        Vec::new(),
        Vec::new(),
    )
}

fn language_cache_label(language: crate::core::Language) -> &'static str {
    match language {
        crate::core::Language::Go => "go",
        crate::core::Language::TypeScript => "typescript",
        crate::core::Language::Tsx => "tsx",
        crate::core::Language::JavaScript => "javascript",
        crate::core::Language::Jsx => "jsx",
        crate::core::Language::Unknown => "unknown",
    }
}

fn sort_file_metrics(metrics: &mut [FileMetricFact]) {
    metrics.sort_by_key(|metric| (metric.file, metric.language));
}

fn sort_function_metrics(metrics: &mut [FunctionMetricFact]) {
    metrics.sort_by(|left, right| {
        metric_order_key(left.file, left.function, &left.name, &left.span).cmp(&metric_order_key(
            right.file,
            right.function,
            &right.name,
            &right.span,
        ))
    });
}

fn sort_complexity_metrics(metrics: &mut [ComplexityMetricFact]) {
    metrics.sort_by(|left, right| {
        metric_order_key(left.file, left.function, &left.name, &left.span).cmp(&metric_order_key(
            right.file,
            right.function,
            &right.name,
            &right.span,
        ))
    });
}

fn metric_order_key<'a>(
    file: FileId,
    function: crate::core::FunctionId,
    name: &'a str,
    span: &Span,
) -> (FileId, u64, u32, u32, &'a str) {
    (file, function.0, span.start_byte, span.end_byte, name)
}

fn line_count(source: &str) -> u32 {
    saturating_u32(source.lines().count())
}

fn non_empty_line_count(source: &str) -> u32 {
    saturating_u32(
        source
            .lines()
            .filter(|line| !line.trim().is_empty())
            .count(),
    )
}

fn span_line_count(span: &crate::core::Span) -> u32 {
    if span.end_line < span.start_line {
        return 0;
    }
    span.end_line
        .saturating_sub(span.start_line)
        .saturating_add(1)
}

fn saturating_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_kernel::{
        FactConfidence, FactFamily, FactPrecision, FactRef, ValidationStatus,
    };
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use crate::core::{
        FileId, FunctionFact, FunctionId, Language, Span, TS_JS_MODULE_FUNCTION_NAME,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    fn metrics_manifest() -> &'static crate::analysis_kernel::ProviderManifest {
        crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.metrics")
            .expect("metrics provider manifest exists")
    }

    fn requested_metrics_plan() -> AnalysisPlan {
        AnalysisPlan::from_capability_names_for_test(&[
            "file_metrics",
            "function_metrics",
            "complexity_metrics",
        ])
    }

    fn metrics_input_snapshot(
        loaded: &crate::config::LoadedConfig,
        db: &AnalysisDb,
        plan: &AnalysisPlan,
        config_digest: &str,
    ) -> InputSnapshot {
        let identity_sources = InputSnapshot::identity_sources_from_plan(loaded, plan);
        let requested_capabilities = plan.requested_capability_snapshots();
        assert!(!requested_capabilities.is_empty());
        assert_eq!(
            identity_sources.requested_capabilities,
            requested_capabilities
        );
        assert_eq!(
            identity_sources.analysis_requirements_identity,
            plan.analysis_requirements_digest()
        );

        InputSnapshot::from_run_inputs_with_plan(
            loaded,
            db,
            config_digest,
            "rule-digest",
            plan,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        )
    }

    fn derive_metrics_with_cache(
        db: &mut AnalysisDb,
        loaded: &crate::config::LoadedConfig,
        cache: &Cache,
        plan: &AnalysisPlan,
        config_digest: &str,
        upstream_label: &str,
    ) -> MetricsDerivation {
        let snapshot = metrics_input_snapshot(loaded, db, plan, config_digest);
        derive_requested_metrics_with_cache_stats(
            db,
            plan,
            cache,
            &snapshot,
            metrics_manifest(),
            vec![
                Digest::from_parts(DigestKind::ProviderOutput, "go_syntax", &[upstream_label]),
                Digest::from_parts(DigestKind::ProviderOutput, "ts_syntax", &[upstream_label]),
            ],
        )
    }

    fn collect_files(root: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        collect_files_into(root, &mut files);
        files.sort();
        files
    }

    fn collect_files_into(root: &Path, files: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries {
            let path = entry.expect("read cache entry").path();
            if path.is_dir() {
                collect_files_into(&path, files);
            } else {
                files.push(path);
            }
        }
    }

    fn first_layer_file(cache_root: &Path, category: &str) -> PathBuf {
        collect_files(&cache_root.join("layers").join(category))
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("expected layer cache {category} file"))
    }

    fn fixture_db(root: &Path, function_name: &str, source: &str) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let path = root.join("src/app.ts");
        fs::create_dir_all(path.parent().expect("test file has parent")).expect("mkdirs");
        fs::write(&path, source).expect("write fixture file");
        let file = db.add_file(path, "src/app.ts".to_string(), source.to_string());
        push_function(&mut db, file, function_name, source);
        db
    }

    fn push_function(
        db: &mut AnalysisDb,
        file: FileId,
        function_name: &str,
        source: &str,
    ) -> FunctionId {
        let start = source.find(function_name).unwrap_or(0);
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: function_name.to_string(),
            span: Span {
                file,
                start_byte: start as u32,
                end_byte: source.len() as u32,
                start_line: 1,
                start_col: (start + 1) as u32,
                end_line: source.lines().count() as u32,
                end_col: 1,
            },
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 2,
            calls: Vec::new(),
        })
    }

    #[test]
    fn derive_requested_metrics_skips_when_plan_does_not_request_metrics() {
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "x\n".to_string(),
        );

        derive_requested_metrics(&mut db, &AnalysisPlan::empty());

        assert!(db.file_metrics().is_empty());
        assert!(db.function_metrics().is_empty());
        assert!(db.complexity_metrics().is_empty());
    }

    #[test]
    fn derive_requested_metrics_populates_shared_file_function_and_complexity_facts() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function handler() {\n  if (ok) return 1;\n  return 0;\n}\n".to_string(),
        );
        let span = Span {
            file,
            start_byte: 0,
            end_byte: 62,
            start_line: 1,
            start_col: 1,
            end_line: 4,
            end_col: 2,
        };
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: TS_JS_MODULE_FUNCTION_NAME.to_string(),
            span: Span {
                file,
                start_byte: 0,
                end_byte: 63,
                start_line: 1,
                start_col: 1,
                end_line: 5,
                end_col: 1,
            },
            language: Language::TypeScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        let function = db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "handler".to_string(),
            span,
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 2,
            calls: Vec::new(),
        });
        let plan = AnalysisPlan::from_capability_names_for_test(&["file_metrics"]);

        derive_requested_metrics(&mut db, &plan);

        assert_eq!(db.file_metrics().len(), 1);
        assert_eq!(db.file_metrics()[0].line_count, 4);
        assert_eq!(db.file_metrics()[0].non_empty_line_count, 4);
        assert_eq!(db.file_metrics()[0].function_count, 1);
        assert_eq!(db.function_metrics().len(), 1);
        assert_eq!(db.function_metrics()[0].function, function);
        assert_eq!(db.function_metrics()[0].line_count, 4);
        assert_eq!(db.function_metrics()[0].byte_count, 62);
        assert_eq!(db.complexity_metrics().len(), 1);
        assert_eq!(db.complexity_metrics()[0].function, function);
        assert_eq!(db.complexity_metrics()[0].cyclomatic_complexity, 2);
        assert_eq!(metrics_function_fact_digests(&db).len(), 1);
    }

    #[test]
    fn metrics_metadata_is_recorded_only_when_metrics_are_requested() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function handler() { return 1; }\n".to_string(),
        );
        let span = Span {
            file,
            start_byte: 0,
            end_byte: 37,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 38,
        };
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "handler".to_string(),
            span,
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });

        derive_requested_metrics(&mut db, &AnalysisPlan::empty());

        assert!(
            db.metadata_for(FactRef::new(FactFamily::FileMetric, 0))
                .is_none()
        );
        assert!(
            db.metadata_for(FactRef::new(FactFamily::FunctionMetric, 0))
                .is_none()
        );
        assert!(
            db.metadata_for(FactRef::new(FactFamily::ComplexityMetric, 0))
                .is_none()
        );

        derive_requested_metrics(
            &mut db,
            &AnalysisPlan::from_capability_names_for_test(&["complexity_metrics"]),
        );

        assert!(
            db.metadata_for(FactRef::new(FactFamily::FileMetric, 0))
                .is_some()
        );
        assert!(
            db.metadata_for(FactRef::new(FactFamily::FunctionMetric, 0))
                .is_some()
        );
        assert!(
            db.metadata_for(FactRef::new(FactFamily::ComplexityMetric, 0))
                .is_some()
        );
    }

    #[test]
    fn metrics_metadata_uses_provider_defaults_and_source_stable_keys() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function handler() {\n  return 1;\n}\n".to_string(),
        );
        let span = Span {
            file,
            start_byte: 0,
            end_byte: 40,
            start_line: 1,
            start_col: 1,
            end_line: 3,
            end_col: 2,
        };
        let function = db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "handler".to_string(),
            span,
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        let file_key = db
            .metadata_for(FactRef::new(FactFamily::SourceFile, u64::from(file.0)))
            .expect("source metadata should exist")
            .stable_key
            .clone();
        let function_key = db
            .metadata_for(FactRef::new(FactFamily::Function, function.0))
            .expect("function metadata should exist")
            .stable_key
            .clone();

        derive_requested_metrics(
            &mut db,
            &AnalysisPlan::from_capability_names_for_test(&["function_metrics"]),
        );

        let file_metric = db
            .metadata_for(FactRef::new(FactFamily::FileMetric, 0))
            .expect("file metric metadata should be recorded");
        let function_metric = db
            .metadata_for(FactRef::new(FactFamily::FunctionMetric, 0))
            .expect("function metric metadata should be recorded");
        let complexity_metric = db
            .metadata_for(FactRef::new(FactFamily::ComplexityMetric, 0))
            .expect("complexity metric metadata should be recorded");

        assert_eq!(file_metric.producer_id, "polint.metrics");
        assert_eq!(file_metric.layer_id, "polint.metrics");
        assert_eq!(file_metric.precision, FactPrecision::Syntax);
        assert_eq!(file_metric.confidence, FactConfidence::High);
        assert_eq!(file_metric.validation, ValidationStatus::NativeTrusted);
        assert!(file_metric.stable_key.contains(&file_key));
        assert!(function_metric.stable_key.contains(&function_key));
        assert!(function_metric.stable_key.contains("metric_name"));
        assert!(function_metric.stable_key.contains("function_size"));
        assert!(complexity_metric.stable_key.contains(&function_key));
        assert!(complexity_metric.stable_key.contains("metric_name"));
        assert!(
            complexity_metric
                .stable_key
                .contains("cyclomatic_complexity")
        );
    }

    mod metrics_layer_cache {
        use super::*;

        #[test]
        fn metrics_layer_reuses_warm_cache() {
            let temp = tempfile::tempdir().expect("tempdir");
            let loaded = load_config(temp.path()).expect("default config loads");
            let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
            let plan = requested_metrics_plan();
            let source = "export function handler() {\n  if (ok) return 1;\n  return 0;\n}\n";
            let mut first_db = fixture_db(temp.path(), "handler", source);
            let mut second_db = fixture_db(temp.path(), "handler", source);

            let first = derive_metrics_with_cache(
                &mut first_db,
                &loaded,
                &cache,
                &plan,
                "config",
                "stable",
            );
            let second = derive_metrics_with_cache(
                &mut second_db,
                &loaded,
                &cache,
                &plan,
                "config",
                "stable",
            );

            assert_eq!(first.cache_stats.misses, 1);
            assert_eq!(first.cache_stats.recomputes, 1);
            assert_eq!(first.cache_stats.writes, 1);
            assert_eq!(second.cache_stats.hits, 1);
            assert_eq!(second.cache_stats.verified_reuse, 1);
            assert_eq!(second.cache_stats.recomputes, 0);
            assert_eq!(first.output_digest, second.output_digest);
            assert_eq!(metric_rows(&first_db), metric_rows(&second_db));
        }

        #[test]
        fn metrics_layer_invalidates_on_function_input_change() {
            let temp = tempfile::tempdir().expect("tempdir");
            let loaded = load_config(temp.path()).expect("default config loads");
            let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
            let plan = requested_metrics_plan();
            let source = "export function handler() {\n  return 1;\n}\n";
            let changed = "export function renamed() {\n  return 1;\n}\n";
            let mut base_db = fixture_db(temp.path(), "handler", source);
            derive_metrics_with_cache(&mut base_db, &loaded, &cache, &plan, "config", "stable");
            let mut changed_db = fixture_db(temp.path(), "renamed", changed);

            let changed = derive_metrics_with_cache(
                &mut changed_db,
                &loaded,
                &cache,
                &plan,
                "config",
                "stable",
            );

            assert_eq!(changed.cache_stats.misses, 1);
            assert_eq!(changed.cache_stats.recomputes, 1);
        }

        #[test]
        fn metrics_layer_corrupt_cache_recomputes() {
            let temp = tempfile::tempdir().expect("tempdir");
            let loaded = load_config(temp.path()).expect("default config loads");
            let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
            let plan = requested_metrics_plan();
            let source = "export function handler() {\n  return 1;\n}\n";
            let mut first_db = fixture_db(temp.path(), "handler", source);
            derive_metrics_with_cache(&mut first_db, &loaded, &cache, &plan, "config", "stable");
            let manifest = first_layer_file(temp.path().join("cache").as_path(), "manifests");
            fs::write(manifest, "{broken").expect("corrupt metrics manifest");
            let mut second_db = fixture_db(temp.path(), "handler", source);

            let second = derive_metrics_with_cache(
                &mut second_db,
                &loaded,
                &cache,
                &plan,
                "config",
                "stable",
            );

            assert_eq!(second.cache_stats.invalid_evicted_reads, 1);
            assert_eq!(second.cache_stats.recomputes, 1);
        }

        #[test]
        fn metrics_layer_cache_write_failure_is_reported() {
            let temp = tempfile::tempdir().expect("tempdir");
            let loaded = load_config(temp.path()).expect("default config loads");
            let cache_root = temp.path().join("cache");
            fs::create_dir_all(&cache_root).expect("cache root");
            fs::write(cache_root.join("layers"), "not a directory").expect("layer root file");
            let cache = Cache::new(cache_root.join("analysis"), true);
            let plan = requested_metrics_plan();
            let source = "export function handler() {\n  return 1;\n}\n";
            let mut db = fixture_db(temp.path(), "handler", source);

            let derivation =
                derive_metrics_with_cache(&mut db, &loaded, &cache, &plan, "config", "stable");

            assert_eq!(derivation.cache_stats.misses, 0);
            assert_eq!(derivation.cache_stats.invalid_evicted_reads, 1);
            assert_eq!(derivation.cache_stats.recomputes, 1);
            assert_eq!(derivation.cache_stats.writes, 0);
            assert!(derivation.diagnostics.iter().any(|diagnostic| {
                diagnostic.rule_id == "internal/cache"
                    && diagnostic.file == "metrics layer"
                    && diagnostic.message.contains("cache write failed")
            }));
            assert!(!db.file_metrics().is_empty());
        }

        #[test]
        fn metrics_layer_disabled_cache_records_bypass_without_layer_files() {
            let temp = tempfile::tempdir().expect("tempdir");
            let loaded = load_config(temp.path()).expect("default config loads");
            let cache_root = temp.path().join("cache").join("analysis");
            let cache = Cache::new(&cache_root, false);
            let plan = requested_metrics_plan();
            let source = "export function handler() {\n  return 1;\n}\n";
            let mut db = fixture_db(temp.path(), "handler", source);

            let derivation =
                derive_metrics_with_cache(&mut db, &loaded, &cache, &plan, "config", "stable");

            assert_eq!(derivation.cache_stats.bypasses_disabled, 1);
            assert_eq!(derivation.cache_stats.recomputes, 1);
            assert!(!temp.path().join("cache").join("layers").exists());
            assert!(!db.file_metrics().is_empty());
        }
    }

    fn metric_rows(db: &AnalysisDb) -> Vec<(String, u32, u32, u32)> {
        let mut rows = db
            .file_metrics()
            .iter()
            .map(|metric| {
                (
                    format!("file:{}", db.path_for(metric.file)),
                    metric.line_count,
                    metric.byte_count,
                    metric.function_count,
                )
            })
            .chain(db.function_metrics().iter().map(|metric| {
                (
                    format!("function:{}:{}", db.path_for(metric.file), metric.name),
                    metric.line_count,
                    metric.byte_count,
                    0,
                )
            }))
            .chain(db.complexity_metrics().iter().map(|metric| {
                (
                    format!("complexity:{}:{}", db.path_for(metric.file), metric.name),
                    metric.cyclomatic_complexity,
                    0,
                    0,
                )
            }))
            .collect::<Vec<_>>();
        rows.sort();
        rows
    }
}
