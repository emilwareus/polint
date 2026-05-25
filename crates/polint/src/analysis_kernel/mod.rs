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
pub(crate) mod validation;

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
        let go_output_digest = go_output.output_digest.clone();
        diagnostics.extend(go_output.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.go.syntax",
            &db,
            go_output.cache_stats,
            go_output_digest.clone(),
        ));

        let ts_output = crate::ts::analyze_with_plan_options_and_cache_stats(
            &mut db,
            input.cache,
            input.config_digest,
            input.rule_digest,
            input.plan,
            input.parallel,
        );
        let ts_output_digest = ts_output.output_digest.clone();
        diagnostics.extend(ts_output.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.ts.syntax",
            &db,
            ts_output.cache_stats,
            ts_output_digest.clone(),
        ));

        let go_dependency_output_digest = go_output_digest.unwrap_or_else(|| {
            incremental::Digest::absent(incremental::DigestKind::ProviderOutput, "polint.go.syntax")
        });
        let ts_dependency_output_digest = ts_output_digest.unwrap_or_else(|| {
            incremental::Digest::absent(incremental::DigestKind::ProviderOutput, "polint.ts.syntax")
        });
        let module_graph = crate::module_graph::derive_requested_module_graph_with_cache_stats(
            &mut db,
            input.loaded,
            input.plan,
            input.cache,
            &input_snapshot,
            Self::provider_manifest("polint.module_graph"),
            vec![
                go_dependency_output_digest.clone(),
                ts_dependency_output_digest.clone(),
            ],
        );
        let module_support = module_graph.support_view(input.plan.support_view());
        // Keep polint.module_graph cache_stats internal to KernelRunReport.
        let polint_module_graph_cache_stats = module_graph.cache_stats.clone();
        let module_output_digest = module_graph.output_digest.clone();
        diagnostics.extend(module_graph.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.module_graph",
            &db,
            polint_module_graph_cache_stats,
            module_output_digest.clone(),
        ));

        let module_dependency_output_digest = module_output_digest.unwrap_or_else(|| {
            incremental::Digest::absent(
                incremental::DigestKind::ProviderOutput,
                "polint.module_graph",
            )
        });
        let symbol_graph = crate::symbol_graph::derive_requested_symbols_with_cache_stats(
            &mut db,
            input.loaded,
            input.plan,
            input.cache,
            &input_snapshot,
            Self::provider_manifest("polint.symbol_graph"),
            module_dependency_output_digest.clone(),
            vec![
                go_dependency_output_digest.clone(),
                ts_dependency_output_digest.clone(),
            ],
        );
        let capability_support = symbol_graph.support_view(&module_support);
        let polint_symbol_graph_cache_stats = symbol_graph.cache_stats.clone();
        let symbol_output_digest = symbol_graph.output_digest.clone();
        diagnostics.extend(symbol_graph.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.symbol_graph",
            &db,
            polint_symbol_graph_cache_stats,
            symbol_output_digest.clone(),
        ));

        let symbol_dependency_output_digest = symbol_output_digest.unwrap_or_else(|| {
            incremental::Digest::absent(
                incremental::DigestKind::ProviderOutput,
                "polint.symbol_graph",
            )
        });
        let module_topology = crate::module_graph::derive_module_topology_with_cache_stats(
            &mut db,
            input.cache,
            &input_snapshot,
            Self::provider_manifest("polint.module_topology"),
            module_dependency_output_digest,
            symbol_dependency_output_digest.clone(),
        );
        let polint_module_topology_cache_stats = module_topology.cache_stats.clone();
        let module_topology_output_digest = module_topology.output_digest.clone();
        diagnostics.extend(module_topology.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.module_topology",
            &db,
            polint_module_topology_cache_stats,
            module_topology_output_digest.clone(),
        ));

        let module_topology_dependency_output_digest = module_topology_output_digest
            .unwrap_or_else(|| {
                incremental::Digest::absent(
                    incremental::DigestKind::ProviderOutput,
                    "polint.module_topology",
                )
            });
        let semantic_mir = crate::analysis::provider::derive_semantic_mir_with_cache_stats(
            &mut db,
            &input_snapshot,
            Self::provider_manifest("polint.semantic_mir"),
            module_topology_dependency_output_digest.clone(),
            symbol_dependency_output_digest.clone(),
            vec![
                go_dependency_output_digest.clone(),
                ts_dependency_output_digest.clone(),
            ],
        );
        let polint_semantic_mir_cache_stats = semantic_mir.cache_stats.clone();
        let semantic_mir_output_digest = semantic_mir.output_digest.clone();
        diagnostics.extend(semantic_mir.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.semantic_mir",
            &db,
            polint_semantic_mir_cache_stats,
            semantic_mir_output_digest.clone(),
        ));

        let semantic_mir_dependency_output_digest =
            semantic_mir_output_digest.unwrap_or_else(|| {
                incremental::Digest::absent(
                    incremental::DigestKind::ProviderOutput,
                    "polint.semantic_mir",
                )
            });
        let cfg = crate::analysis::cfg::provider::derive_cfg_with_cache_stats(
            &mut db,
            &input_snapshot,
            Self::provider_manifest("polint.cfg"),
            semantic_mir_dependency_output_digest.clone(),
            vec![
                go_dependency_output_digest.clone(),
                ts_dependency_output_digest.clone(),
            ],
        );
        let polint_cfg_cache_stats = cfg.cache_stats.clone();
        let cfg_output_digest = cfg.output_digest.clone();
        diagnostics.extend(cfg.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.cfg",
            &db,
            polint_cfg_cache_stats,
            cfg_output_digest.clone(),
        ));

        let cfg_dependency_output_digest = cfg_output_digest.unwrap_or_else(|| {
            incremental::Digest::absent(incremental::DigestKind::ProviderOutput, "polint.cfg")
        });
        let calls = crate::analysis::calls::provider::derive_calls_with_cache_stats(
            &mut db,
            &input_snapshot,
            Self::provider_manifest("polint.calls"),
            semantic_mir_dependency_output_digest.clone(),
            cfg_dependency_output_digest.clone(),
            symbol_dependency_output_digest.clone(),
            module_topology_dependency_output_digest.clone(),
            vec![
                go_dependency_output_digest.clone(),
                ts_dependency_output_digest.clone(),
            ],
        );
        let polint_calls_cache_stats = calls.cache_stats.clone();
        let calls_output_digest = calls.output_digest.clone();
        diagnostics.extend(calls.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.calls",
            &db,
            polint_calls_cache_stats,
            calls_output_digest.clone(),
        ));

        let calls_dependency_output_digest = calls_output_digest.unwrap_or_else(|| {
            incremental::Digest::absent(incremental::DigestKind::ProviderOutput, "polint.calls")
        });
        let abstract_domains =
            crate::analysis::domains::provider::derive_abstract_domains_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.abstract_domains"),
                semantic_mir_dependency_output_digest.clone(),
                cfg_dependency_output_digest.clone(),
                calls_dependency_output_digest.clone(),
                symbol_dependency_output_digest.clone(),
                module_topology_dependency_output_digest.clone(),
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            );
        let polint_abstract_domains_cache_stats = abstract_domains.cache_stats.clone();
        let abstract_domains_output_digest = abstract_domains.output_digest;
        diagnostics.extend(abstract_domains.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.abstract_domains",
            &db,
            polint_abstract_domains_cache_stats,
            abstract_domains_output_digest.clone(),
        ));

        let abstract_domains_dependency_output_digest = abstract_domains_output_digest
            .unwrap_or_else(|| {
                incremental::Digest::absent(
                    incremental::DigestKind::ProviderOutput,
                    "polint.abstract_domains",
                )
            });
        let entrypoints_semantic_mir_digest = semantic_mir_dependency_output_digest.clone();
        let entrypoints_cfg_digest = cfg_dependency_output_digest.clone();
        let entrypoints_calls_digest = calls_dependency_output_digest.clone();
        let entrypoints_symbol_digest = symbol_dependency_output_digest.clone();
        let entrypoints_topology_digest = module_topology_dependency_output_digest.clone();
        let direct_summaries =
            crate::analysis::summaries::provider::derive_direct_summaries_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.direct_summaries"),
                semantic_mir_dependency_output_digest,
                cfg_dependency_output_digest,
                calls_dependency_output_digest,
                abstract_domains_dependency_output_digest.clone(),
                symbol_dependency_output_digest,
                module_topology_dependency_output_digest,
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            );
        let polint_direct_summaries_cache_stats = direct_summaries.cache_stats.clone();
        let direct_summaries_output_digest = direct_summaries.output_digest.clone();
        diagnostics.extend(direct_summaries.diagnostics);

        // SCC closure: interprocedural summary improvement over SCCs.
        // Runs after direct summaries so callee summaries are available.
        let scc_closure = crate::analysis::summaries::provider::run_scc_closure_with_cache(
            &mut db,
            input.cache,
            input.config_digest,
            input.rule_digest,
            input.plan.digest(),
        );
        #[cfg(test)]
        let scc_closure_debug = scc_closure.debug_snapshot;
        diagnostics.extend(scc_closure.diagnostics);
        provider_outputs.push(Self::provider_output_for(
            "polint.direct_summaries",
            &db,
            polint_direct_summaries_cache_stats,
        ));

        let entrypoints =
            crate::analysis::entrypoints::provider::derive_entrypoints_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.entrypoints"),
                entrypoints_semantic_mir_digest.clone(),
                entrypoints_cfg_digest.clone(),
                entrypoints_calls_digest.clone(),
                entrypoints_symbol_digest.clone(),
                entrypoints_topology_digest.clone(),
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            );
        let polint_entrypoints_cache_stats = entrypoints.cache_stats.clone();
        let entrypoints_output_digest = entrypoints.output_digest.clone();
        diagnostics.extend(entrypoints.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.entrypoints",
            &db,
            polint_entrypoints_cache_stats,
            entrypoints_output_digest.clone(),
        ));

        let direct_summaries_dependency_output_digest = direct_summaries_output_digest
            .unwrap_or_else(|| {
                incremental::Digest::absent(
                    incremental::DigestKind::ProviderOutput,
                    "polint.direct_summaries",
                )
            });
        let entrypoints_dependency_output_digest = entrypoints_output_digest.unwrap_or_else(|| {
            incremental::Digest::absent(
                incremental::DigestKind::ProviderOutput,
                "polint.entrypoints",
            )
        });

        let extensions =
            crate::analysis::extensions::provider::derive_extension_provider_outputs_with_cache_stats(
                &mut db,
                &input.loaded.root,
                &input_snapshot,
                Self::provider_manifest("polint.extensions"),
            );
        let polint_extensions_cache_stats = extensions.cache_stats.clone();
        let extensions_output_digest = extensions.output_digest.clone();
        diagnostics.extend(extensions.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.extensions",
            &db,
            polint_extensions_cache_stats,
            extensions_output_digest.clone(),
        ));
        let extensions_dependency_output_digest = extensions_output_digest.unwrap_or_else(|| {
            incremental::Digest::absent(
                incremental::DigestKind::ProviderOutput,
                "polint.extensions",
            )
        });

        let type_value_alias =
            crate::analysis::types::provider::derive_type_value_alias_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.type_value_alias"),
                entrypoints_semantic_mir_digest.clone(),
                entrypoints_cfg_digest.clone(),
                entrypoints_calls_digest.clone(),
                abstract_domains_dependency_output_digest.clone(),
                direct_summaries_dependency_output_digest.clone(),
                entrypoints_dependency_output_digest.clone(),
                extensions_dependency_output_digest.clone(),
                entrypoints_symbol_digest.clone(),
                entrypoints_topology_digest.clone(),
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            );
        let polint_type_value_alias_cache_stats = type_value_alias.cache_stats.clone();
        let type_value_alias_output_digest = type_value_alias.output_digest.clone();
        diagnostics.extend(type_value_alias.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.type_value_alias",
            &db,
            polint_type_value_alias_cache_stats,
            type_value_alias_output_digest.clone(),
        ));

        let type_value_alias_dependency_output_digest = type_value_alias_output_digest
            .unwrap_or_else(|| {
                incremental::Digest::absent(
                    incremental::DigestKind::ProviderOutput,
                    "polint.type_value_alias",
                )
            });
        let refined_calls =
            crate::analysis::refined_calls::provider::derive_refined_calls_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.refined_calls"),
                entrypoints_calls_digest.clone(),
                entrypoints_dependency_output_digest.clone(),
                direct_summaries_dependency_output_digest.clone(),
                type_value_alias_dependency_output_digest.clone(),
                extensions_dependency_output_digest.clone(),
            );
        let polint_refined_calls_cache_stats = refined_calls.cache_stats.clone();
        let refined_calls_output_digest = refined_calls.output_digest.clone();
        diagnostics.extend(refined_calls.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.refined_calls",
            &db,
            polint_refined_calls_cache_stats,
            refined_calls_output_digest.clone(),
        ));

        let refined_calls_dependency_output_digest =
            refined_calls_output_digest.unwrap_or_else(|| {
                incremental::Digest::absent(
                    incremental::DigestKind::ProviderOutput,
                    "polint.refined_calls",
                )
            });
        let data_flow = crate::analysis::data_flow::provider::derive_data_flow_with_cache_stats(
            &mut db,
            &input_snapshot,
            Self::provider_manifest("polint.data_flow"),
            entrypoints_semantic_mir_digest,
            entrypoints_cfg_digest,
            entrypoints_calls_digest,
            refined_calls_dependency_output_digest,
            direct_summaries_dependency_output_digest,
            type_value_alias_dependency_output_digest,
            entrypoints_dependency_output_digest,
            extensions_dependency_output_digest,
        );
        let polint_data_flow_cache_stats = data_flow.cache_stats.clone();
        let data_flow_output_digest = data_flow.output_digest.clone();
        diagnostics.extend(data_flow.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.data_flow",
            &db,
            polint_data_flow_cache_stats,
            data_flow_output_digest,
        ));

        let metrics = crate::metrics::derive_requested_metrics_with_cache_stats(
            &mut db,
            input.plan,
            input.cache,
            &input_snapshot,
            Self::provider_manifest("polint.metrics"),
            vec![go_dependency_output_digest, ts_dependency_output_digest],
        );
        let polint_metrics_cache_stats = metrics.cache_stats.clone();
        let metrics_output_digest = metrics.output_digest;
        diagnostics.extend(metrics.diagnostics);
        provider_outputs.push(Self::provider_output_for_with_optional_digest(
            "polint.metrics",
            &db,
            polint_metrics_cache_stats,
            metrics_output_digest,
        ));
        let validation_diagnostics =
            validation::validate_fact_metadata(&db, Self::provider_manifests());
        diagnostics.extend(validation_diagnostics);
        let run_report = incremental::KernelRunReport::new(
            input_snapshot,
            provider_outputs,
            scc_closure.demand_query_trace,
        );
        #[cfg(test)]
        let run_report = run_report.with_scc_closure_debug(scc_closure_debug);

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
    pub(crate) fn metadata_debug_json_for_output_for_test(
        output: &KernelOutput,
    ) -> serde_json::Value {
        debug::metadata_debug_json_with_demand_trace_for_test(
            &output.db,
            output.run_report.demand_query_trace(),
            output.run_report.scc_closure_debug(),
        )
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
    use std::path::{Path, PathBuf};

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
                "polint.module_topology",
                "polint.semantic_mir",
                "polint.cfg",
                "polint.calls",
                "polint.abstract_domains",
                "polint.direct_summaries",
                "polint.entrypoints",
                "polint.extensions",
                "polint.type_value_alias",
                "polint.refined_calls",
                "polint.metrics",
            ]
        );
    }

    #[test]
    fn direct_summaries_provider_output_reflects_final_summary_metadata() {
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
        .expect("kernel run should succeed");
        let manifest = AnalysisKernel::provider_manifest("polint.direct_summaries");
        let expected = incremental::provider_output_digest_from_manifest(
            manifest,
            &provider_output_summary_parts(&output.db, manifest),
        );
        let direct_summaries = provider_output(&output, "polint.direct_summaries");

        assert_eq!(direct_summaries.output_digest, expected);
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
    fn kernel_run_report_symbol_graph_row_carries_layer_cache_stats() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("src")).expect("create src");
        std::fs::write(
            temp.path().join("src/app.ts"),
            "export function answer() { return 42; }\nexport const value = answer();\n",
        )
        .expect("write app");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
        let plan = AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]);

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

        let first_symbol_graph = provider_output(&first, "polint.symbol_graph");
        let second_symbol_graph = provider_output(&second, "polint.symbol_graph");

        assert_eq!(first_symbol_graph.cache_stats.misses, 1);
        assert_eq!(first_symbol_graph.cache_stats.recomputes, 1);
        assert_eq!(first_symbol_graph.cache_stats.writes, 1);
        assert_eq!(second_symbol_graph.cache_stats.hits, 1);
        assert_eq!(second_symbol_graph.cache_stats.verified_reuse, 1);
        assert_eq!(second_symbol_graph.cache_stats.recomputes, 0);
        assert_eq!(
            first_symbol_graph.output_digest,
            second_symbol_graph.output_digest
        );
    }

    #[test]
    fn kernel_run_report_module_topology_row_carries_layer_cache_stats() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name":"root","dependencies":{"react":"^18.0.0"}}"#,
        )
        .expect("write package");
        std::fs::create_dir_all(temp.path().join("src")).expect("create src");
        std::fs::write(
            temp.path().join("src/app.ts"),
            "import React from 'react';\nexport function App() { return React.createElement('main'); }\n",
        )
        .expect("write app");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
        let plan = AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]);

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
        let disabled_cache = Cache::new("", false);
        let disabled = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &disabled_cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("disabled-cache kernel run should succeed");

        let first_module_topology = provider_output(&first, "polint.module_topology");
        let second_module_topology = provider_output(&second, "polint.module_topology");
        let disabled_module_topology = provider_output(&disabled, "polint.module_topology");

        assert_eq!(first_module_topology.cache_stats.misses, 1);
        assert_eq!(first_module_topology.cache_stats.recomputes, 1);
        assert_eq!(first_module_topology.cache_stats.writes, 1);
        assert!(!first_module_topology.output_digest.value.is_empty());
        assert_eq!(second_module_topology.cache_stats.hits, 1);
        assert_eq!(second_module_topology.cache_stats.verified_reuse, 1);
        assert_eq!(second_module_topology.cache_stats.recomputes, 0);
        assert_eq!(
            first_module_topology.output_digest,
            second_module_topology.output_digest
        );
        assert_eq!(disabled_module_topology.cache_stats.bypasses_disabled, 1);
        assert_eq!(disabled_module_topology.cache_stats.recomputes, 1);
        assert!(!disabled_module_topology.output_digest.value.is_empty());
    }

    #[test]
    fn kernel_run_report_semantic_mir_row_carries_output_digest() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("main.go"),
            "package main\nfunc answer() int { return 42 }\n",
        )
        .expect("write go");
        std::fs::write(
            temp.path().join("app.ts"),
            "export function app() { return 42; }\n",
        )
        .expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]);

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");
        let semantic_mir = provider_output(&output, "polint.semantic_mir");

        assert_eq!(semantic_mir.schema_version, "semantic-mir-facts-1:1");
        assert!(!semantic_mir.output_digest.value.is_empty());
        assert_eq!(semantic_mir.cache_stats.recomputes, 1);
    }

    #[test]
    fn kernel_run_report_cfg_row_carries_output_digest() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("app.ts"),
            "export function app() { return 42; }\n",
        )
        .expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]);

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");
        let cfg = provider_output(&output, "polint.cfg");

        assert_eq!(cfg.schema_version, "cfg-facts-1:1");
        assert!(!cfg.output_digest.value.is_empty());
        assert_eq!(cfg.cache_stats.recomputes, 1);
    }

    #[test]
    fn kernel_run_report_calls_row_carries_output_digest() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("app.ts"),
            "export function app() { return 42; }\n",
        )
        .expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]);

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");
        let calls = provider_output(&output, "polint.calls");

        assert_eq!(calls.schema_version, "calls-facts-1:1");
        assert!(!calls.output_digest.value.is_empty());
        assert_eq!(calls.cache_stats.recomputes, 1);
    }

    #[test]
    fn kernel_run_report_metrics_row_carries_layer_cache_stats() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("src")).expect("create src");
        std::fs::write(
            temp.path().join("src/app.ts"),
            "export function answer() { return 42; }\n",
        )
        .expect("write app");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
        let plan = AnalysisPlan::from_capability_names_for_test(&[
            "file_metrics",
            "function_metrics",
            "complexity_metrics",
        ]);

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

        let first_metrics = provider_output(&first, "polint.metrics");
        let second_metrics = provider_output(&second, "polint.metrics");

        assert_eq!(first_metrics.cache_stats.misses, 1);
        assert_eq!(first_metrics.cache_stats.recomputes, 1);
        assert_eq!(first_metrics.cache_stats.writes, 1);
        assert_eq!(second_metrics.cache_stats.hits, 1);
        assert_eq!(second_metrics.cache_stats.verified_reuse, 1);
        assert_eq!(second_metrics.cache_stats.recomputes, 0);
        assert_eq!(first_metrics.output_digest, second_metrics.output_digest);
    }

    #[test]
    fn kernel_surfaces_metrics_layer_cache_write_diagnostics() {
        let temp = tempfile::tempdir().expect("tempdir");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache_root = temp.path().join("cache");
        std::fs::create_dir_all(&cache_root).expect("cache root");
        std::fs::write(cache_root.join("layers"), "not a directory").expect("layer root file");
        let cache = Cache::new(cache_root.join("analysis"), true);
        let plan = AnalysisPlan::from_capability_names_for_test(&["file_metrics"]);

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");
        let metrics = provider_output(&output, "polint.metrics");

        assert_eq!(metrics.cache_stats.misses, 0);
        assert_eq!(metrics.cache_stats.invalid_evicted_reads, 1);
        assert_eq!(metrics.cache_stats.recomputes, 1);
        assert_eq!(metrics.cache_stats.writes, 0);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.rule_id == "internal/cache"
                && diagnostic.file == "metrics layer"
                && diagnostic.message.contains("cache write failed")
        }));
    }

    #[test]
    fn kernel_run_report_source_and_derived_provider_rows_have_expected_stats_and_output_digests() {
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
            "polint.module_topology",
            "polint.metrics",
        ] {
            let row = provider_output(&output, provider_id);
            assert_eq!(row.cache_stats, CacheStats::default());
            assert!(!row.output_digest.value.is_empty());
        }

        let semantic_mir = provider_output(&output, "polint.semantic_mir");
        assert_eq!(semantic_mir.cache_stats.recomputes, 1);
        assert!(!semantic_mir.output_digest.value.is_empty());
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
    fn framework_entrypoint_internals_do_not_leak_into_public_surfaces_no_leak() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let markers = framework_internal_markers();

        let rendered = crate::diagnostics::render(
            crate::diagnostics::OutputFormat::Json,
            &[],
            crate::diagnostics::RenderOpts {
                json: crate::diagnostics::JsonReportMeta {
                    tool_name: "polint",
                    tool_version: "test",
                },
                color: crate::diagnostics::ColorChoice::Never,
                sources: None,
            },
        );
        assert_no_framework_markers("polint check --format json", &rendered, &markers);

        let mut public_surfaces = Vec::new();
        collect_files_with_extensions(&crate_root.join("src/sdk"), &["rs"], &mut public_surfaces);
        public_surfaces.extend([
            crate_root.join("src/runner/mod.rs"),
            crate_root.join("src/cli/mod.rs"),
            crate_root.join("src/lib.rs"),
            repo_root.join("README.md"),
            repo_root.join("docs/API-VISIBILITY-PLAN.md"),
        ]);
        collect_files_with_extensions(&repo_root.join("docs/facts"), &["md"], &mut public_surfaces);
        public_surfaces.sort();
        public_surfaces.dedup();

        for source_path in public_surfaces {
            if !source_path.exists() {
                continue;
            }
            let source = std::fs::read_to_string(&source_path)
                .unwrap_or_else(|error| panic!("read {}: {error}", source_path.display()));
            assert_no_framework_markers(&source_path.display().to_string(), &source, &markers);
        }
    }

    #[test]
    fn refined_call_internals_do_not_leak_into_public_surfaces_no_leak() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let markers = refined_call_internal_markers();

        let rendered = crate::diagnostics::render(
            crate::diagnostics::OutputFormat::Json,
            &[],
            crate::diagnostics::RenderOpts {
                json: crate::diagnostics::JsonReportMeta {
                    tool_name: "polint",
                    tool_version: "test",
                },
                color: crate::diagnostics::ColorChoice::Never,
                sources: None,
            },
        );
        assert_no_refined_call_markers("polint check --format json", &rendered, &markers);

        let mut public_surfaces = Vec::new();
        collect_files_with_extensions(&crate_root.join("src/sdk"), &["rs"], &mut public_surfaces);
        public_surfaces.extend([
            crate_root.join("src/runner/mod.rs"),
            crate_root.join("src/cli/mod.rs"),
            crate_root.join("src/lib.rs"),
            repo_root.join("README.md"),
        ]);
        collect_files_with_extensions(&repo_root.join("docs/facts"), &["md"], &mut public_surfaces);
        public_surfaces.sort();
        public_surfaces.dedup();

        for source_path in public_surfaces {
            if !source_path.exists() {
                continue;
            }
            let source = std::fs::read_to_string(&source_path)
                .unwrap_or_else(|error| panic!("read {}: {error}", source_path.display()));
            assert_no_refined_call_markers(&source_path.display().to_string(), &source, &markers);
        }
    }

    #[test]
    fn typescript_framework_entrypoints_from_real_source_include_handler_and_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join(".polint.toml"),
            r#"
[workspace]
include = ["*.ts"]
"#,
        )
        .expect("write config");
        std::fs::write(
            temp.path().join("app.ts"),
            r#"
import express from "express";

const app = express();

function getUsers(req, res) {
  res.json([]);
}

function setup() {
  app.get("/api/users/:id", getUsers);
}
"#,
        )
        .expect("write ts");
        let loaded = load_config(temp.path()).expect("config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&[
            "symbols",
            "references",
            "resolved_imports",
            "module_graph",
        ]);

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");

        let entrypoint = output
            .db
            .entrypoint_facts()
            .iter()
            .find(|entrypoint| entrypoint.framework_id == "ts.express")
            .expect("express entrypoint");
        let function_name = output
            .db
            .functions()
            .iter()
            .find(|function| function.id == entrypoint.target_function)
            .map(|function| function.name.as_str());

        assert_eq!(function_name, Some("getUsers"));
        assert_eq!(
            entrypoint.trigger_metadata.path.as_deref(),
            Some("/api/users/:id")
        );
    }

    #[test]
    fn framework_entrypoint_eval_fixture_sources_include_go_and_ts_entrypoints() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let fixture_repo =
            repo_root.join("tests/eval-fixtures/framework-entrypoints/mixed-go-ts/repo");
        let loaded = load_config(&fixture_repo).expect("fixture config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&[
            "symbols",
            "references",
            "resolved_imports",
            "module_graph",
        ]);

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");

        let frameworks = output
            .db
            .entrypoint_facts()
            .iter()
            .map(|entrypoint| entrypoint.framework_id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let files = output
            .db
            .files()
            .iter()
            .map(|file| (file.relative_path.as_str(), file.language))
            .collect::<Vec<_>>();
        let imports = output
            .db
            .imports()
            .iter()
            .map(|import| (import.path.as_str(), import.language))
            .collect::<Vec<_>>();
        let call_sites = output
            .db
            .call_sites()
            .iter()
            .map(|site| (&site.callee, site.language))
            .collect::<Vec<_>>();

        assert!(
            frameworks.contains("go.net_http"),
            "expected Go net/http entrypoints, got {frameworks:#?}"
        );
        assert!(
            frameworks.contains("ts.express"),
            "expected TS Express entrypoints, got frameworks={frameworks:#?} files={files:#?} imports={imports:#?} calls={call_sites:#?}"
        );
        assert!(
            frameworks.contains("ts.mcp_sdk"),
            "expected TS MCP SDK entrypoints, got {frameworks:#?}"
        );
    }

    fn framework_internal_markers() -> [&'static str; 26] {
        [
            "polint.entrypoints",
            "EntrypointFact",
            "TrustBoundaryFact",
            "FrameworkDispatchEdgeFact",
            "UnresolvedFrameworkFact",
            "EntrypointKind",
            "TrustBoundarySourceKind",
            "DispatchEdgeKind",
            "UnresolvedFrameworkReason",
            "EntrypointPrecision",
            "EntrypointProvenance",
            "EntrypointConfidence",
            "EntrypointStatus",
            "recognizers_go",
            "recognizers_ts",
            "trust_boundaries",
            "dispatch",
            "derive_entrypoints_with_cache_stats",
            "extract_entrypoints",
            "recognize_go_entrypoints",
            "recognize_ts_entrypoints",
            "entrypoints_debug",
            "metadata_debug_json_for_test",
            "EntrypointStore",
            "EntrypointOutput",
            "Entrypoints<'_>",
        ]
    }

    fn refined_call_internal_markers() -> [&'static str; 15] {
        [
            "polint.refined_calls",
            "RefinedCallEdgeFact",
            "RefinedCallTier",
            "RefinedCallReason",
            "RefinedCallGraph",
            "refined_call_edges",
            "direct_plus_framework",
            "points_to_assisted",
            "extension_model",
            "derive_refined_calls_with_cache_stats",
            "RefinedCallStore",
            "RefinedCallOutput",
            "refined_calls.edge",
            "TypeValueFunctionToken",
            "DirectPlusFramework",
        ]
    }

    fn assert_no_refined_call_markers(label: &str, source: &str, markers: &[&str]) {
        for marker in markers {
            assert!(
                !source.contains(marker),
                "{label} leaked Phase 37 refined-call internal marker `{marker}`"
            );
        }
    }

    fn assert_no_framework_markers(label: &str, source: &str, markers: &[&str]) {
        for marker in markers {
            assert!(
                !source.contains(marker),
                "{label} leaked Phase 35 framework internal marker `{marker}`"
            );
        }
    }

    fn collect_files_with_extensions(root: &Path, extensions: &[&str], files: &mut Vec<PathBuf>) {
        if !root.exists() {
            return;
        }
        for entry in std::fs::read_dir(root)
            .unwrap_or_else(|error| panic!("read {}: {error}", root.display()))
        {
            let entry = entry.expect("read public surface entry");
            let path = entry.path();
            if entry
                .file_type()
                .expect("public surface file type")
                .is_dir()
            {
                collect_files_with_extensions(&path, extensions, files);
            } else if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extensions.contains(&extension))
            {
                files.push(path);
            }
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
                "polint.module_topology",
                "polint.semantic_mir",
                "polint.cfg",
                "polint.calls",
                "polint.abstract_domains",
                "polint.direct_summaries",
                "polint.entrypoints",
                "polint.extensions",
                "polint.type_value_alias",
                "polint.refined_calls",
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
