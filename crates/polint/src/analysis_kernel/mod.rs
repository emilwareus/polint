use crate::analysis_plan::AnalysisPlan;
use crate::cache::Cache;
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, CapabilitySupportView};
use crate::diagnostics::Diagnostic;

#[rustfmt::skip]
#[cfg(test)] mod debug;
pub(crate) mod incremental;
mod metadata;
mod provider;
mod validation;

pub(crate) use metadata::{
    FactConfidence, FactFamily, FactMeta, FactMetaStore, FactPrecision, FactRef, MissingFactMeta,
    ValidationStatus, resolution_metadata, resolution_status_metadata, stable_key_from_parts,
    symbol_metadata,
};
#[cfg(test)]
pub(crate) use provider::ProviderKind;
pub(crate) use provider::{
    CachePolicy, LanguageScope, PrecisionCeiling, ProviderManifest, SchemaVersion,
};

pub(crate) struct AnalysisKernel;

pub(crate) struct KernelInput<'a> {
    pub(crate) loaded: &'a LoadedConfig,
    pub(crate) cache: &'a Cache,
    pub(crate) config_digest: &'a str,
    pub(crate) rule_digest: &'a str,
    pub(crate) plan: &'a AnalysisPlan,
    pub(crate) parallel: bool,
}

pub(crate) struct KernelOutput {
    pub(crate) db: AnalysisDb,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) capability_support: CapabilitySupportView,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "The crate-private run report is consumed by internal tests and eval fixtures before a public surface exists."
        )
    )]
    pub(crate) run_report: incremental::KernelRunReport,
}

impl AnalysisKernel {
    pub(crate) fn provider_manifests() -> &'static [ProviderManifest] {
        provider::provider_manifests()
    }

    pub(crate) fn run(input: KernelInput<'_>) -> anyhow::Result<KernelOutput> {
        let mut db = crate::fs::load_analysis_files(input.loaded)?;
        let input_snapshot = incremental::InputSnapshot::from_run_inputs(
            input.loaded,
            &db,
            input.config_digest,
            input.rule_digest,
            input.plan.digest(),
            Self::provider_manifests(),
        );
        let mut diagnostics = Vec::new();
        let mut provider_outputs = Vec::new();

        provider_outputs.push(Self::provider_output_for(
            "polint.source",
            &db,
            incremental::CacheStats::default(),
        ));

        let go_output = crate::go::analyze_with_plan_options_and_cache_stats(
            &mut db,
            input.cache,
            input.config_digest,
            input.rule_digest,
            input.plan,
            input.parallel,
        );
        diagnostics.extend(go_output.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.go.syntax",
            &db,
            go_output.cache_stats,
            go_output.output_digest,
        ));

        let ts_output = crate::ts::analyze_with_plan_options_and_cache_stats(
            &mut db,
            input.cache,
            input.config_digest,
            input.rule_digest,
            input.plan,
            input.parallel,
        );
        diagnostics.extend(ts_output.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.ts.syntax",
            &db,
            ts_output.cache_stats,
            ts_output.output_digest,
        ));

        let module_graph =
            crate::module_graph::derive_requested_module_graph(&mut db, input.loaded, input.plan);
        let module_support = module_graph.support_view(input.plan.support_view());
        diagnostics.extend(module_graph.diagnostics);
        provider_outputs.push(Self::provider_output_for(
            "polint.module_graph",
            &db,
            incremental::CacheStats::default(),
        ));

        let symbol_graph =
            crate::symbol_graph::derive_requested_symbols(&mut db, input.loaded, input.plan);
        let capability_support = symbol_graph.support_view(&module_support);
        diagnostics.extend(symbol_graph.diagnostics);
        provider_outputs.push(Self::provider_output_for(
            "polint.symbol_graph",
            &db,
            incremental::CacheStats::default(),
        ));

        crate::metrics::derive_requested_metrics(&mut db, input.plan);
        provider_outputs.push(Self::provider_output_for(
            "polint.metrics",
            &db,
            incremental::CacheStats::default(),
        ));
        let validation_diagnostics =
            validation::validate_fact_metadata(&db, Self::provider_manifests());
        diagnostics.extend(validation_diagnostics);
        let run_report = incremental::KernelRunReport::new(input_snapshot, provider_outputs);

        Ok(KernelOutput {
            db,
            diagnostics,
            capability_support,
            run_report,
        })
    }

    #[cfg(test)]
    pub(crate) fn missing_fact_metadata_for_test(db: &AnalysisDb) -> Vec<MissingFactMeta> {
        db.missing_fact_metadata()
    }

    #[cfg(test)]
    pub(crate) fn metadata_debug_json_for_test(db: &AnalysisDb) -> serde_json::Value {
        debug::metadata_debug_json_for_test(db)
    }

    #[cfg(test)]
    pub(crate) fn input_snapshot_json_for_test(output: &KernelOutput) -> serde_json::Value {
        serde_json::to_value(&output.run_report.input_snapshot)
            .expect("input snapshot should serialize")
    }

    #[cfg(test)]
    pub(crate) fn provider_output_report_for_test(
        output: &KernelOutput,
    ) -> Vec<incremental::ProviderOutputMeta> {
        output.run_report.provider_outputs.clone()
    }

    fn provider_output_for(
        provider_id: &'static str,
        db: &AnalysisDb,
        cache_stats: incremental::CacheStats,
    ) -> incremental::ProviderOutputMeta {
        Self::provider_output_for_with_optional_digest(provider_id, db, cache_stats, None)
    }

    fn provider_output_for_with_optional_digest(
        provider_id: &'static str,
        db: &AnalysisDb,
        cache_stats: incremental::CacheStats,
        output_digest: Option<incremental::Digest>,
    ) -> incremental::ProviderOutputMeta {
        let manifest = Self::provider_manifest(provider_id);
        let output_digest = output_digest.unwrap_or_else(|| {
            incremental::provider_output_digest_from_manifest(
                manifest,
                &provider_output_summary_parts(db, manifest),
            )
        });
        incremental::provider_output_from_manifest(manifest, output_digest, cache_stats)
    }

    fn provider_manifest(provider_id: &str) -> &'static ProviderManifest {
        Self::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == provider_id)
            .unwrap_or_else(|| panic!("missing provider manifest {provider_id}"))
    }
}

fn provider_output_summary_parts(db: &AnalysisDb, manifest: &ProviderManifest) -> Vec<String> {
    let mut parts = db
        .fact_meta()
        .rows()
        .filter(|(_reference, metadata)| {
            metadata.producer_id == manifest.id || metadata.layer_id == manifest.id
        })
        .flat_map(|(reference, metadata)| {
            [
                format!("fact_family={}", reference.family.label()),
                format!("run_id={}", reference.run_id),
                format!("stable_key={}", metadata.stable_key),
                format!("payload_digest={}", metadata.payload_digest),
                format!("precision={:?}", metadata.precision),
                format!("validation={:?}", metadata.validation),
            ]
        })
        .collect::<Vec<_>>();

    if parts.is_empty() {
        parts.push("fact_summary=empty".to_string());
    }
    parts.sort();
    parts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{CacheStats, INPUT_SNAPSHOT_SCHEMA_VERSION};
    use crate::config::load_config;
    use crate::core::{
        BranchObligation, ComplexityMetricFact, CoverageFact, DefinitionFact, DefinitionId,
        DefinitionKind, FileMetricFact, FunctionFact, FunctionId, FunctionMetricFact, ImportFact,
        ImportId, JsxAttributeFact, Language, ModuleEdge, ModuleEdgeId, ModuleEdgeKind, ModuleNode,
        ModuleNodeId, ModuleNodeKind, PackageFact, PackageId, ReferenceFact, ReferenceId,
        ReferenceKind, ResolutionPrecision, ResolutionStatus, ResolvedImportFact, ResolvedImportId,
        Span, StringLiteralFact, SymbolFact, SymbolId, SymbolKind, SymbolNamespace,
        SymbolPrecision, SymbolResolutionStatus, TestFact, TsClassFact, TsComponentFact,
    };
    use std::path::PathBuf;

    fn span(file: crate::core::FileId, start_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte: start_byte + 10,
            start_line: 1,
            start_col: start_byte + 1,
            end_line: 1,
            end_col: start_byte + 11,
        }
    }

    fn db_with_one_fact_from_every_current_family() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.tsx"),
            "src/app.tsx".to_string(),
            "import React from 'react';\nexport function Button() { return <button aria-label=\"Save\">Save</button>; }\n".to_string(),
        );
        let package = db.push_package(PackageFact {
            id: PackageId(99),
            file,
            name: "app".to_string(),
            span: span(file, 0),
            language: Language::Tsx,
        });
        let function = db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "Button".to_string(),
            span: span(file, 27),
            language: Language::Tsx,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: vec!["React.createElement".to_string()],
        });
        let import = db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "react".to_string(),
            span: span(file, 0),
            language: Language::Tsx,
        });
        let branch = db.push_branch(BranchObligation {
            id: crate::core::BranchId(99),
            function: Some(function),
            file,
            decision_span: span(file, 40),
            condition_text: "enabled".to_string(),
            edge_label: "true".to_string(),
            is_error_path: false,
            stable_fingerprint: "branch:key".to_string(),
        });
        db.push_test(TestFact {
            file,
            function: Some(function),
            name: "TestButton".to_string(),
            span: span(file, 50),
            evidence_terms: vec!["render".to_string()],
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_coverage(CoverageFact {
            branch,
            covered: Some(true),
            source: "fixture".to_string(),
        });
        db.push_ts_component(TsComponentFact {
            file,
            function: Some(function),
            name: "Button".to_string(),
            span: span(file, 27),
        });
        db.push_ts_class(TsClassFact {
            file,
            name: "Dialog".to_string(),
            span: span(file, 61),
            is_exported: true,
            is_component_like: false,
        });
        db.push_string_literal(StringLiteralFact {
            file,
            value: "Save".to_string(),
            span: span(file, 88),
            language: Language::Tsx,
        });
        db.push_jsx_attribute(JsxAttributeFact {
            file,
            name: "aria-label".to_string(),
            value: Some("Save".to_string()),
            span: span(file, 72),
        });
        db.replace_module_graph_facts(
            vec![ResolvedImportFact {
                id: ResolvedImportId(99),
                import,
                from_file: file,
                target_node: Some(ModuleNodeId(1)),
                status: ResolutionStatus::Resolved,
                precision: ResolutionPrecision::ExactFile,
                reason: None,
            }],
            vec![
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::File,
                    label: "src/app.tsx".to_string(),
                    file: Some(file),
                    package: Some(package),
                    language: Some(Language::Tsx),
                },
                ModuleNode {
                    id: ModuleNodeId(100),
                    kind: ModuleNodeKind::External,
                    label: "react".to_string(),
                    file: None,
                    package: None,
                    language: Some(Language::Tsx),
                },
            ],
            vec![ModuleEdge {
                id: ModuleEdgeId(99),
                from: ModuleNodeId(0),
                to: ModuleNodeId(1),
                import: Some(import),
                resolved_import: Some(ResolvedImportId(0)),
                kind: ModuleEdgeKind::Imports,
                status: ResolutionStatus::Resolved,
            }],
        );
        db.replace_symbol_graph_facts(
            vec![SymbolFact {
                id: SymbolId(0),
                language: Language::Tsx,
                name: "Button".to_string(),
                qualified_name: "Button".to_string(),
                kind: SymbolKind::Function,
                namespace: SymbolNamespace::Value,
                file: Some(file),
                package: Some(package),
                module: Some(ModuleNodeId(0)),
                owner: None,
                primary_span: Some(span(file, 27)),
                is_exported: true,
                stable_key: "symbol:Button".to_string(),
                precision: SymbolPrecision::ExactLocal,
            }],
            vec![DefinitionFact {
                id: DefinitionId(0),
                symbol: SymbolId(0),
                language: Language::Tsx,
                name: "Button".to_string(),
                qualified_name: "Button".to_string(),
                kind: DefinitionKind::Declaration,
                namespace: SymbolNamespace::Value,
                file: Some(file),
                package: Some(package),
                module: Some(ModuleNodeId(0)),
                owner: None,
                primary_span: Some(span(file, 27)),
                is_primary: true,
                is_exported: true,
                stable_key: "definition:Button".to_string(),
                precision: SymbolPrecision::ExactLocal,
            }],
            vec![ReferenceFact {
                id: ReferenceId(0),
                language: Language::Tsx,
                name: "Button".to_string(),
                qualified_name: "Button".to_string(),
                kind: ReferenceKind::Read,
                namespace: SymbolNamespace::Value,
                file: Some(file),
                package: Some(package),
                module: Some(ModuleNodeId(0)),
                owner: None,
                primary_span: Some(span(file, 27)),
                target: Some(SymbolId(0)),
                candidates: Vec::new(),
                stable_key: "reference:Button".to_string(),
                status: SymbolResolutionStatus::Resolved,
                precision: SymbolPrecision::ExactLocal,
            }],
        );
        db.replace_metric_facts(
            vec![FileMetricFact {
                file,
                language: Language::Tsx,
                line_count: 2,
                non_empty_line_count: 2,
                byte_count: 100,
                function_count: 1,
            }],
            vec![FunctionMetricFact {
                function,
                file,
                name: "Button".to_string(),
                span: span(file, 27),
                language: Language::Tsx,
                line_count: 1,
                byte_count: 10,
            }],
            vec![ComplexityMetricFact {
                function,
                file,
                name: "Button".to_string(),
                span: span(file, 27),
                language: Language::Tsx,
                cyclomatic_complexity: 1,
            }],
        );
        db
    }

    #[test]
    fn run_with_empty_plan_returns_empty_db_and_plan_support() {
        let temp = tempfile::tempdir().expect("tempdir");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::empty();

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run on an empty repo");

        assert!(output.db.files().is_empty());
        assert!(output.diagnostics.is_empty());
        assert_eq!(&output.capability_support, plan.support_view());
    }

    #[test]
    fn kernel_run_report_records_input_snapshot_and_provider_outputs() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("main.go"), "package main\n").expect("write go");
        std::fs::create_dir_all(temp.path().join("src")).expect("create src");
        std::fs::write(
            temp.path().join("src/app.ts"),
            "export const answer = 42;\n",
        )
        .expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::empty();

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");

        let snapshot = AnalysisKernel::input_snapshot_json_for_test(&output);
        let provider_outputs = AnalysisKernel::provider_output_report_for_test(&output);

        assert_eq!(snapshot["schema_version"], INPUT_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(
            provider_outputs
                .iter()
                .map(|row| row.provider_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "polint.source",
                "polint.go.syntax",
                "polint.ts.syntax",
                "polint.module_graph",
                "polint.symbol_graph",
                "polint.metrics",
            ]
        );
    }

    #[test]
    fn kernel_run_report_syntax_provider_rows_carry_adapter_cache_stats() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("main.go"), "package main\n").expect("write go");
        std::fs::write(temp.path().join("app.ts"), "export const app = 1;\n").expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::empty();

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");
        let go = provider_output(&output, "polint.go.syntax");
        let ts = provider_output(&output, "polint.ts.syntax");

        assert!(go.cache_stats.bypasses_disabled > 0);
        assert!(go.cache_stats.recomputes > 0);
        assert!(ts.cache_stats.bypasses_disabled > 0);
        assert!(ts.cache_stats.recomputes > 0);
    }

    #[test]
    fn kernel_run_report_module_graph_row_carries_layer_cache_stats() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("src")).expect("create src");
        std::fs::write(
            temp.path().join("src/app.ts"),
            "import tokens from './tokens';\n",
        )
        .expect("write app");
        std::fs::write(
            temp.path().join("src/tokens.ts"),
            "export const tokens = {};\n",
        )
        .expect("write tokens");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
        let plan = AnalysisPlan::from_capability_names_for_test(&["resolved_imports"]);

        let first = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("first kernel run should succeed");
        let second = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("second kernel run should succeed");

        let first_module_graph = provider_output(&first, "polint.module_graph");
        let second_module_graph = provider_output(&second, "polint.module_graph");

        assert_eq!(first_module_graph.cache_stats.misses, 1);
        assert_eq!(first_module_graph.cache_stats.recomputes, 1);
        assert_eq!(first_module_graph.cache_stats.writes, 1);
        assert_eq!(second_module_graph.cache_stats.hits, 1);
        assert_eq!(second_module_graph.cache_stats.verified_reuse, 1);
        assert_eq!(second_module_graph.cache_stats.recomputes, 0);
        assert_eq!(
            first_module_graph.output_digest,
            second_module_graph.output_digest
        );
    }

    #[test]
    fn kernel_run_report_source_and_derived_provider_rows_have_zero_stats_and_output_digests() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("app.ts"), "export const app = 1;\n").expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::empty();

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");

        for provider_id in [
            "polint.source",
            "polint.module_graph",
            "polint.symbol_graph",
            "polint.metrics",
        ] {
            let row = provider_output(&output, provider_id);
            assert_eq!(row.cache_stats, CacheStats::default());
            assert!(!row.output_digest.value.is_empty());
        }
    }

    #[test]
    fn kernel_run_report_synthetic_manifest_consumption_helpers_are_removed_from_kernel() {
        let source = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/analysis_kernel/mod.rs"),
        )
        .expect("read analysis kernel source");

        let forbidden_terms = [
            ["provider", "manifest", "metadata", "token"].join("_"),
            ["provider", "manifest", "metadata", "weight"].join("_"),
            ["provider", "kind", "weight"].join("_"),
            ["language", "scope", "weight"].join("_"),
            ["cache", "policy", "weight"].join("_"),
            ["precision", "ceiling", "weight"].join("_"),
            ["schema", "version", "weight"].join("_"),
            ["", "manifest", "metadata", "token"].join("_"),
        ];

        for forbidden in forbidden_terms {
            assert!(
                !source.contains(&forbidden),
                "synthetic manifest helper remains in kernel: {forbidden}"
            );
        }
    }

    #[test]
    fn run_propagates_file_loading_errors() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut loaded = load_config(temp.path()).expect("default config loads");
        loaded.config.workspace.include = vec!["[".to_string()];
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::empty();

        let result = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        });
        let Err(error) = result else {
            panic!("kernel should propagate load_analysis_files errors");
        };

        assert!(
            error.to_string().contains("invalid glob"),
            "unexpected error: {error:?}"
        );
    }

    #[test]
    fn provider_manifests_cover_existing_kernel_providers() {
        let ids = AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| manifest.id)
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            [
                "polint.source",
                "polint.go.syntax",
                "polint.ts.syntax",
                "polint.module_graph",
                "polint.symbol_graph",
                "polint.metrics",
            ]
        );
    }

    #[test]
    fn missing_fact_metadata_reports_no_gaps_when_all_current_families_have_metadata() {
        let db = db_with_one_fact_from_every_current_family();

        let report = AnalysisKernel::missing_fact_metadata_for_test(&db);

        assert!(report.is_empty(), "unexpected missing metadata: {report:?}");
    }

    #[test]
    fn missing_fact_metadata_reports_removed_rows_sorted_by_family_and_run_id() {
        let mut db = db_with_one_fact_from_every_current_family();
        db.remove_fact_metadata_for_test(FactRef::new(FactFamily::Reference, 0));
        db.remove_fact_metadata_for_test(FactRef::new(FactFamily::FileMetric, 0));

        let report = AnalysisKernel::missing_fact_metadata_for_test(&db);

        assert_eq!(
            report,
            vec![
                MissingFactMeta {
                    family: FactFamily::FileMetric,
                    run_id: 0,
                },
                MissingFactMeta {
                    family: FactFamily::Reference,
                    run_id: 0,
                },
            ]
        );
    }

    fn provider_output<'a>(
        output: &'a KernelOutput,
        provider_id: &str,
    ) -> &'a crate::analysis_kernel::incremental::ProviderOutputMeta {
        output
            .run_report
            .provider_outputs
            .iter()
            .find(|row| row.provider_id == provider_id)
            .unwrap_or_else(|| panic!("missing provider output row {provider_id}"))
    }
}
