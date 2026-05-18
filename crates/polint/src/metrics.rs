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
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_kernel::{
        FactConfidence, FactFamily, FactPrecision, FactRef, ValidationStatus,
    };
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use crate::core::{FileId, FunctionFact, FunctionId, Language, Span};
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
        InputSnapshot::from_run_inputs(
            loaded,
            db,
            config_digest,
            "rule-digest",
            plan.digest(),
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
