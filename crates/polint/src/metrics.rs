use crate::analysis_plan::AnalysisPlan;
use crate::core::{AnalysisDb, ComplexityMetricFact, FileId, FileMetricFact, FunctionMetricFact};
use std::collections::BTreeMap;

const METRIC_CAPABILITIES: &[&str] = &["file_metrics", "function_metrics", "complexity_metrics"];

pub(crate) fn derive_requested_metrics(db: &mut AnalysisDb, plan: &AnalysisPlan) {
    if !plan.requests_any_capability(METRIC_CAPABILITIES) {
        return;
    }

    let mut function_counts = BTreeMap::<FileId, u32>::new();
    for function in db.functions() {
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
    use crate::analysis_kernel::{
        FactConfidence, FactFamily, FactPrecision, FactRef, ValidationStatus,
    };
    use crate::analysis_plan::AnalysisPlan;
    use crate::core::{FunctionFact, FunctionId, Language, Span};
    use std::path::PathBuf;

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
        assert!(complexity_metric.stable_key.contains("cyclomatic_complexity"));
    }
}
