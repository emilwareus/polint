//! Language-neutral metric fact derivation.
//!
//! Cache orchestration remains in the facade; this module owns deterministic
//! metric projection and normalization over the shared analysis host.

use polint_analysis_api::{
    ComplexityMetricFact, FileMetricFact, FunctionFact, FunctionMetricFact, SourceFile,
    is_synthetic_ts_js_module_function,
};
use polint_core::{FileId, Language, Span};
use std::collections::BTreeMap;

use crate::AnalysisHost;

/// Whether a plan requests any of the metric fact families.
pub const METRIC_CAPABILITIES: &[&str] =
    &["file_metrics", "function_metrics", "complexity_metrics"];

/// Cache payload for normalized metric facts.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsLayerPayload {
    pub schema: String,
    pub file_metrics: Vec<FileMetricFact>,
    pub function_metrics: Vec<FunctionMetricFact>,
    pub complexity_metrics: Vec<ComplexityMetricFact>,
}

/// Newly projected metric rows, before they are installed by a composition root.
#[derive(Debug, Clone, Default)]
pub struct MetricsOutput {
    pub file_metrics: Vec<FileMetricFact>,
    pub function_metrics: Vec<FunctionMetricFact>,
    pub complexity_metrics: Vec<ComplexityMetricFact>,
}

/// Schema label for the normalized metric layer.
pub const METRICS_LAYER_SCHEMA: &str = "metrics-facts-v1";

/// Derive deterministic file, function, and complexity metrics when requested.
pub fn derive_requested_metrics(
    db: &(impl AnalysisHost + ?Sized),
    requested: bool,
) -> Option<MetricsOutput> {
    if !requested {
        return None;
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
        .map(|file| {
            FileMetricFact::new(
                file.id,
                file.language,
                line_count(&file.source),
                non_empty_line_count(&file.source),
                saturating_u32(file.source.len()),
                function_counts.get(&file.id).copied().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();

    let function_metrics = db
        .functions()
        .iter()
        .filter(|function| !is_synthetic_ts_js_module_function(function))
        .map(|function| {
            FunctionMetricFact::new(
                function.id,
                function.file,
                function.name.clone(),
                function.span.clone(),
                function.language,
                span_line_count(&function.span),
                function
                    .span
                    .end_byte
                    .saturating_sub(function.span.start_byte),
            )
        })
        .collect::<Vec<_>>();

    let complexity_metrics = db
        .functions()
        .iter()
        .filter(|function| !is_synthetic_ts_js_module_function(function))
        .map(|function| {
            ComplexityMetricFact::new(
                function.id,
                function.file,
                function.name.clone(),
                function.span.clone(),
                function.language,
                function.cyclomatic_complexity,
            )
        })
        .collect::<Vec<_>>();

    Some(MetricsOutput {
        file_metrics,
        function_metrics,
        complexity_metrics,
    })
}

/// Return source files in stable relative-path order.
pub fn sorted_files(db: &(impl AnalysisHost + ?Sized)) -> Vec<&SourceFile> {
    let mut files = db.files().iter().collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files
}

/// Return non-synthetic functions in stable source order.
pub fn sorted_functions(db: &(impl AnalysisHost + ?Sized)) -> Vec<&FunctionFact> {
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

/// Return a cache-stable digest for the metric projection parameters.
pub fn metrics_parameter_digest() -> polint_analysis_api::Digest {
    polint_analysis_api::Digest::from_parts(
        polint_analysis_api::DigestKind::ProviderParameters,
        "metrics_parameters",
        &[
            "output=file_metrics",
            "output=function_metrics",
            "output=complexity_metrics",
        ],
    )
}

/// Return the normalized metric payload currently held by the host.
pub fn metrics_layer_payload(db: &(impl AnalysisHost + ?Sized)) -> MetricsLayerPayload {
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

/// Restore a normalized metric payload into the host.
pub fn restore_metrics_layer_payload(payload: &MetricsLayerPayload) -> MetricsOutput {
    let mut file_metrics = payload.file_metrics.clone();
    let mut function_metrics = payload.function_metrics.clone();
    let mut complexity_metrics = payload.complexity_metrics.clone();
    sort_file_metrics(&mut file_metrics);
    sort_function_metrics(&mut function_metrics);
    sort_complexity_metrics(&mut complexity_metrics);
    MetricsOutput {
        file_metrics,
        function_metrics,
        complexity_metrics,
    }
}

pub fn sort_file_metrics(metrics: &mut [FileMetricFact]) {
    metrics.sort_by_key(|metric| (metric.file, metric.language));
}

pub fn sort_function_metrics(metrics: &mut [FunctionMetricFact]) {
    metrics.sort_by(|left, right| {
        metric_order_key(left.file, left.function, &left.name, &left.span).cmp(&metric_order_key(
            right.file,
            right.function,
            &right.name,
            &right.span,
        ))
    });
}

pub fn sort_complexity_metrics(metrics: &mut [ComplexityMetricFact]) {
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
    function: polint_core::FunctionId,
    name: &'a str,
    span: &Span,
) -> (FileId, u64, u32, u32, &'a str) {
    (file, function.0, span.start_byte, span.end_byte, name)
}

pub fn language_cache_label(language: Language) -> &'static str {
    match language {
        Language::Go => "go",
        Language::TypeScript => "typescript",
        Language::Tsx => "tsx",
        Language::JavaScript => "javascript",
        Language::Jsx => "jsx",
        Language::Unknown => "unknown",
        _ => unreachable!(),
    }
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

fn span_line_count(span: &Span) -> u32 {
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
