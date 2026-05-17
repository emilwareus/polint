use crate::analysis_plan::AnalysisPlan;
use crate::cache::Cache;
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, CapabilitySupportView};
use crate::diagnostics::Diagnostic;

mod metadata;
mod provider;
mod validation;

pub(crate) use metadata::{
    FactConfidence, FactFamily, FactMeta, FactMetaStore, FactPrecision, FactRef, MissingFactMeta,
    ValidationStatus, resolution_metadata, resolution_status_metadata, stable_key_from_parts,
    symbol_metadata,
};
pub(crate) use provider::{
    CachePolicy, LanguageScope, PrecisionCeiling, ProviderKind, ProviderManifest, SchemaVersion,
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
}

impl AnalysisKernel {
    pub(crate) fn provider_manifests() -> &'static [ProviderManifest] {
        provider::provider_manifests()
    }

    pub(crate) fn run(input: KernelInput<'_>) -> anyhow::Result<KernelOutput> {
        let _manifest_metadata_token = Self::provider_manifest_metadata_token();
        let mut db = crate::fs::load_analysis_files(input.loaded)?;
        let mut diagnostics = Vec::new();

        diagnostics.extend(crate::go::analyze_with_plan_options(
            &mut db,
            input.cache,
            input.config_digest,
            input.rule_digest,
            input.plan,
            input.parallel,
        ));
        diagnostics.extend(crate::ts::analyze_with_plan_options(
            &mut db,
            input.cache,
            input.config_digest,
            input.rule_digest,
            input.plan,
            input.parallel,
        ));

        let module_graph =
            crate::module_graph::derive_requested_module_graph(&mut db, input.loaded, input.plan);
        let module_support = module_graph.support_view(input.plan.support_view());
        diagnostics.extend(module_graph.diagnostics);

        let symbol_graph =
            crate::symbol_graph::derive_requested_symbols(&mut db, input.loaded, input.plan);
        let capability_support = symbol_graph.support_view(&module_support);
        diagnostics.extend(symbol_graph.diagnostics);

        crate::metrics::derive_requested_metrics(&mut db, input.plan);
        debug_assert!(db.missing_fact_metadata().is_empty());
        let validation_diagnostics =
            validation::validate_fact_metadata(&db, Self::provider_manifests());
        diagnostics.extend(validation_diagnostics);

        Ok(KernelOutput {
            db,
            diagnostics,
            capability_support,
        })
    }

    #[cfg(test)]
    pub(crate) fn missing_fact_metadata_for_test(db: &AnalysisDb) -> Vec<MissingFactMeta> {
        db.missing_fact_metadata()
    }

    fn provider_manifest_metadata_token() -> usize {
        metadata::metadata_vocabulary_weight()
            + Self::provider_manifests()
                .iter()
                .map(provider_manifest_metadata_weight)
                .sum::<usize>()
    }
}

fn provider_manifest_metadata_weight(manifest: &ProviderManifest) -> usize {
    manifest.id.len()
        + provider_kind_weight(manifest.kind)
        + language_scope_weight(manifest.language_scope)
        + cache_policy_weight(manifest.cache_policy)
        + precision_ceiling_weight(manifest.precision_ceiling)
        + manifest
            .inputs
            .iter()
            .map(|input| input.len())
            .sum::<usize>()
        + manifest
            .outputs
            .iter()
            .map(|output| output.len())
            .sum::<usize>()
        + manifest
            .schema_versions
            .iter()
            .map(schema_version_weight)
            .sum::<usize>()
}

fn provider_kind_weight(kind: ProviderKind) -> usize {
    match kind {
        ProviderKind::SourceDiscovery => 1,
        ProviderKind::LanguageSyntax => 2,
        ProviderKind::WholeRepoDerived => 3,
        ProviderKind::MetricsDerived => 4,
    }
}

fn language_scope_weight(scope: LanguageScope) -> usize {
    match scope {
        LanguageScope::Workspace => 1,
        LanguageScope::Go => 2,
        LanguageScope::TypeScriptJavaScript => 3,
        LanguageScope::MultiLanguage => 4,
    }
}

fn cache_policy_weight(policy: CachePolicy) -> usize {
    match policy {
        CachePolicy::NoCache => 1,
        CachePolicy::ExistingFileFactCache { schema } => schema.len(),
        CachePolicy::InMemoryDerived => 2,
    }
}

fn precision_ceiling_weight(precision: PrecisionCeiling) -> usize {
    match precision {
        PrecisionCeiling::Exact => 1,
        PrecisionCeiling::Syntax => 2,
        PrecisionCeiling::SetupAware => 3,
    }
}

fn schema_version_weight(schema: &SchemaVersion) -> usize {
    schema.name.len() + schema.version as usize
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
