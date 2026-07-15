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
mod store;
pub(crate) mod validation;

pub(crate) use metadata::{
    FactConfidence, FactFamily, FactMeta, FactMetaStore, FactPrecision, FactRef, MissingFactMeta,
    StableFactMetaConflict, StableFactMetaRow, ValidationStatus, resolution_metadata,
    resolution_status_metadata, stable_key_from_parts, symbol_metadata,
};
#[cfg_attr(
    test,
    allow(
        unused_imports,
        reason = "unit tests exercise the borrowed cache-policy codec in its defining module"
    )
)]
#[cfg_attr(
    not(test),
    allow(
        unused_imports,
        reason = "canonical provider codecs are shared with private metadata projection and readers"
    )
)]
pub(crate) use provider::{
    CachePolicy, CachePolicyView, LanguageScope, PrecisionCeiling, ProviderKind, ProviderManifest,
    SchemaVersion,
};
#[cfg(test)]
pub(crate) use store::StoreStatus;

/// Capabilities that require whole-repo discovery even when the deep semantic
/// pipeline is skipped. Resolved imports, module graph, symbols, and references
/// are cross-file facts, so file-scoped source loading would change their
/// meaning.
const CROSS_FILE_ANALYSIS_TRIGGER_CAPABILITIES: &[&str] = &[
    "resolved_imports",
    "module_graph",
    "symbols",
    "references",
    "calls",
    "control_flow",
    "dataflow",
];

/// Capabilities whose facts need local semantic lowering: `semantic_mir` and
/// direct call-site facts.
const SEMANTIC_PIPELINE_TRIGGER_CAPABILITIES: &[&str] = &["calls", "control_flow", "dataflow"];

/// Capabilities whose facts need CFG, call targets, and the refined-call
/// projection. `ControlFlow` consumes these same-function facts directly.
const CFG_CALL_PIPELINE_TRIGGER_CAPABILITIES: &[&str] = &["calls", "control_flow", "dataflow"];

/// Capabilities whose facts need the expensive interprocedural refinement stack
/// downstream of calls: Go semantic facts, identity, abstract domains,
/// summaries, entrypoints/reachability, extensions, type/value aliases,
/// semantic graph, and solver output. Control-flow policies consume the
/// resulting refined call graph, so they must preserve the same target
/// precision as call-graph policies.
const FULL_REFINEMENT_PIPELINE_TRIGGER_CAPABILITIES: &[&str] =
    &["calls", "control_flow", "dataflow"];

/// Capabilities that need the data-flow and evidence projections. Reachability
/// and same-function control-flow policies do not consume these facts.
const DATA_FLOW_PIPELINE_TRIGGER_CAPABILITIES: &[&str] = &["dataflow"];

pub(crate) struct AnalysisKernel;

macro_rules! dependency_blocked_output {
    ($output_type:ty) => {{
        let mut output = <$output_type>::default();
        output.execution = incremental::ProviderExecutionOutcome::Failed;
        output
    }};
}

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
            reason = "the crate-private run report is consumed only by internal tests and evaluation fixtures"
        )
    )]
    pub(crate) run_report: incremental::KernelRunReport,
}

impl AnalysisKernel {
    pub(crate) fn provider_manifests() -> &'static [ProviderManifest] {
        provider::provider_manifests()
    }

    fn provider_dependencies_ready(
        required: &[&incremental::ProviderOutputDependency],
        absence_allowed: &[&incremental::ProviderOutputDependency],
    ) -> bool {
        required.iter().all(|dependency| dependency.is_present())
            && absence_allowed
                .iter()
                .all(|dependency| dependency.is_available_or_absent())
    }

    pub(crate) fn run(input: KernelInput<'_>) -> anyhow::Result<KernelOutput> {
        // Gate provider work in slices. Cross-file facts keep whole-repo
        // discovery, but deeper semantic providers only run when a requested
        // view consumes their outputs. Syntactic rule sets skip all of this,
        // which is the dominant memory and CPU cost on large repos.
        let run_cross_file_analysis = input
            .plan
            .requests_any_capability(CROSS_FILE_ANALYSIS_TRIGGER_CAPABILITIES);
        let run_semantic_pipeline = input
            .plan
            .requests_any_capability(SEMANTIC_PIPELINE_TRIGGER_CAPABILITIES);
        let run_cfg_call_pipeline = input
            .plan
            .requests_any_capability(CFG_CALL_PIPELINE_TRIGGER_CAPABILITIES);
        let run_full_refinement_pipeline = input
            .plan
            .requests_any_capability(FULL_REFINEMENT_PIPELINE_TRIGGER_CAPABILITIES);
        let run_data_flow_pipeline = input
            .plan
            .requests_any_capability(DATA_FLOW_PIPELINE_TRIGGER_CAPABILITIES);
        let compact_domain_materialization =
            Self::directly_requests_any_capability(input.plan, &["control_flow"])
                && !Self::directly_requests_any_capability(input.plan, &["calls", "dataflow"]);

        // When cross-file analysis is skipped, the only consumers of a file are
        // the rules scoped to it and the syntactic providers feeding those
        // rules. A file matched by no enabled rule's `files` scope cannot
        // produce any diagnostic, so we never read or parse it. Cross-file facts
        // need the full workspace, so they keep unscoped discovery without
        // forcing the deep semantic provider slices.
        let rule_scope = if run_cross_file_analysis {
            None
        } else {
            Self::rule_scope_globset(input.plan)
        };
        tracing::info!(
            target: "polint::kernel",
            run_cross_file_analysis,
            run_semantic_pipeline,
            run_cfg_call_pipeline,
            run_full_refinement_pipeline,
            run_data_flow_pipeline,
            compact_domain_materialization,
            rule_scoped = rule_scope.is_some(),
            "analysis kernel pipeline gate"
        );

        let mut db = crate::fs::load_analysis_files_scoped(input.loaded, rule_scope.as_ref())?;
        // Source-load summary: the corpus actually read into memory is the dominant
        // memory cost on large repos, so log its size/shape when info tracing is on.
        // Guarded by `enabled!` so it costs nothing in normal (un-instrumented) runs.
        if tracing::enabled!(target: "polint::kernel", tracing::Level::INFO) {
            let files = db.files();
            let total_bytes: usize = files.iter().map(|f| f.source.len()).sum();
            let (mut go, mut ts, mut go_bytes, mut ts_bytes) = (0usize, 0usize, 0usize, 0usize);
            for f in files.iter() {
                match f.language {
                    crate::core::Language::Go => {
                        go += 1;
                        go_bytes += f.source.len();
                    }
                    crate::core::Language::TypeScript
                    | crate::core::Language::Tsx
                    | crate::core::Language::JavaScript
                    | crate::core::Language::Jsx => {
                        ts += 1;
                        ts_bytes += f.source.len();
                    }
                    _ => {}
                }
            }
            tracing::info!(
                target: "polint::kernel",
                files = files.len(),
                total_mb = total_bytes / 1_048_576,
                go,
                go_mb = go_bytes / 1_048_576,
                ts,
                ts_mb = ts_bytes / 1_048_576,
                "stage: source files loaded"
            );
        }
        // Prepare external runtime inputs before sealing the run identity. The same
        // bounded model inventory and exact Go frontend selection are then consumed
        // by their providers, so snapshot identity cannot drift from execution.
        let solver_budget = crate::analysis::solver::budget::SolverBudget {
            go: input.loaded.config.solver.to_go_sub_budget(),
            js: input.loaded.config.solver.to_js_sub_budget(),
            object_model_enabled: input.loaded.config.solver.js_object_model_enabled(),
            object: input.loaded.config.solver.to_js_object_sub_budget(),
            ..crate::analysis::solver::budget::SolverBudget::default()
        };
        let runtime_sources = incremental::InputSnapshotRuntimeSources::prepare(
            input.loaded,
            &db,
            run_full_refinement_pipeline,
            solver_budget.adaptation,
        );
        let input_snapshot =
            incremental::InputSnapshot::from_run_inputs_with_plan_and_runtime_sources(
                input.loaded,
                &db,
                input.config_digest,
                input.rule_digest,
                input.plan,
                Self::provider_manifests(),
                &runtime_sources,
            );
        let mut diagnostics = Vec::new();
        let mut provider_outputs = Vec::new();

        provider_outputs.push(Self::provider_output_for(
            "polint.source",
            &db,
            incremental::CacheStats::default(),
        ));

        let go_analysis_settings_digest = input_snapshot
            .analysis_settings_digest(crate::cache::keys::AnalysisSettingsScope::GoSyntax);
        let go_output = crate::go::analyze_with_plan_options_and_cache_stats(
            &mut db,
            input.cache,
            go_analysis_settings_digest,
            input_snapshot.provider_manifest_digest("polint.go.syntax"),
            input.plan,
            input.parallel,
        );
        let go_execution = go_output.execution;
        let go_output_digest = go_output.output_digest.clone();
        let go_layers = go_output.layers;
        tracing::info!(target: "polint::kernel", "stage: go.syntax done");
        diagnostics.extend(go_output.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome_and_layers(
            "polint.go.syntax",
            go_output.cache_stats,
            go_execution,
            go_output_digest.clone(),
            go_layers,
        ));

        let ts_analysis_settings_digest = input_snapshot
            .analysis_settings_digest(crate::cache::keys::AnalysisSettingsScope::TsSyntax);
        let ts_output = crate::ts::analyze_with_plan_options_and_cache_stats(
            &mut db,
            input.cache,
            ts_analysis_settings_digest,
            input_snapshot.provider_manifest_digest("polint.ts.syntax"),
            input.plan,
            input.parallel,
        );
        let ts_execution = ts_output.execution;
        let ts_output_digest = ts_output.output_digest.clone();
        let ts_layers = ts_output.layers;
        tracing::info!(target: "polint::kernel", "stage: ts.syntax done");
        diagnostics.extend(ts_output.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome_and_layers(
            "polint.ts.syntax",
            ts_output.cache_stats,
            ts_execution,
            ts_output_digest.clone(),
            ts_layers,
        ));

        let go_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.go.syntax",
            go_execution,
            go_output_digest,
        );
        let ts_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.ts.syntax",
            ts_execution,
            ts_output_digest,
        );
        let go_dependency_output_digest = go_dependency_output.output_digest.clone();
        let ts_dependency_output_digest = ts_dependency_output.output_digest.clone();
        let module_graph = crate::module_graph::derive_requested_module_graph_with_cache_stats(
            &mut db,
            input.loaded,
            input.plan,
            input.cache,
            &input_snapshot,
            Self::provider_manifest("polint.module_graph"),
            vec![go_dependency_output.clone(), ts_dependency_output.clone()],
        );
        let module_support = module_graph.support_view(input.plan.support_view());
        // Keep polint.module_graph cache_stats internal to KernelRunReport.
        let polint_module_graph_cache_stats = module_graph.cache_stats.clone();
        let module_execution = module_graph.execution;
        let module_output_digest = module_graph.output_digest.clone();
        let module_graph_layers = module_graph.layers;
        diagnostics.extend(module_graph.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome_and_layers(
            "polint.module_graph",
            polint_module_graph_cache_stats,
            module_execution,
            module_output_digest.clone(),
            module_graph_layers,
        ));

        let module_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.module_graph",
            module_execution,
            module_output_digest,
        );
        let symbol_graph = if module_dependency_output.is_available_or_absent() {
            crate::symbol_graph::derive_requested_symbols_with_cache_stats(
                &mut db,
                input.loaded,
                input.plan,
                input.cache,
                &input_snapshot,
                Self::provider_manifest("polint.symbol_graph"),
                module_dependency_output.clone(),
                vec![go_dependency_output.clone(), ts_dependency_output.clone()],
            )
        } else {
            dependency_blocked_output!(crate::symbol_graph::SymbolGraphDerivation)
        };
        let capability_support = symbol_graph.support_view(&module_support);
        let polint_symbol_graph_cache_stats = symbol_graph.cache_stats.clone();
        let symbol_execution = symbol_graph.execution;
        let symbol_output_digest = symbol_graph.output_digest.clone();
        let symbol_graph_layers = symbol_graph.layers;
        diagnostics.extend(symbol_graph.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome_and_layers(
            "polint.symbol_graph",
            polint_symbol_graph_cache_stats,
            symbol_execution,
            symbol_output_digest.clone(),
            symbol_graph_layers,
        ));

        let symbol_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.symbol_graph",
            symbol_execution,
            symbol_output_digest,
        );
        let module_topology = if !run_cfg_call_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(
            &[&module_dependency_output, &symbol_dependency_output],
            &[],
        ) {
            crate::module_graph::derive_module_topology_with_cache_stats(
                &mut db,
                input.cache,
                &input_snapshot,
                Self::provider_manifest("polint.module_topology"),
                module_dependency_output,
                symbol_dependency_output.clone(),
            )
        } else {
            dependency_blocked_output!(crate::module_graph::ModuleTopologyDerivation)
        };
        let polint_module_topology_cache_stats = module_topology.cache_stats.clone();
        let module_topology_execution = module_topology.execution;
        let module_topology_output_digest = module_topology.output_digest.clone();
        let module_topology_layers = module_topology.layers;
        diagnostics.extend(module_topology.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome_and_layers(
            "polint.module_topology",
            polint_module_topology_cache_stats,
            module_topology_execution,
            module_topology_output_digest.clone(),
            module_topology_layers,
        ));

        let module_topology_dependency_output =
            incremental::ProviderOutputDependency::from_execution(
                "polint.module_topology",
                module_topology_execution,
                module_topology_output_digest,
            );
        let semantic_mir = if !run_semantic_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(
            &[
                &module_topology_dependency_output,
                &symbol_dependency_output,
            ],
            &[],
        ) {
            crate::analysis::provider::derive_semantic_mir_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.semantic_mir"),
                module_topology_dependency_output.output_digest.clone(),
                symbol_dependency_output.output_digest.clone(),
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            )
        } else {
            dependency_blocked_output!(crate::analysis::provider::SemanticMirProviderOutput)
        };
        let polint_semantic_mir_cache_stats = semantic_mir.cache_stats.clone();
        let semantic_mir_execution = semantic_mir.execution;
        let semantic_mir_output_digest = semantic_mir.output_digest.clone();
        diagnostics.extend(semantic_mir.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.semantic_mir",
            polint_semantic_mir_cache_stats,
            semantic_mir_execution,
            semantic_mir_output_digest.clone(),
        ));

        let semantic_mir_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.semantic_mir",
            semantic_mir_execution,
            semantic_mir_output_digest,
        );
        let cfg = if !run_cfg_call_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(&[&semantic_mir_dependency_output], &[]) {
            crate::analysis::cfg::provider::derive_cfg_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.cfg"),
                semantic_mir_dependency_output.output_digest.clone(),
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            )
        } else {
            dependency_blocked_output!(crate::analysis::cfg::provider::CfgProviderOutput)
        };
        let polint_cfg_cache_stats = cfg.cache_stats.clone();
        let cfg_execution = cfg.execution;
        let cfg_output_digest = cfg.output_digest.clone();
        diagnostics.extend(cfg.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.cfg",
            polint_cfg_cache_stats,
            cfg_execution,
            cfg_output_digest.clone(),
        ));

        let cfg_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.cfg",
            cfg_execution,
            cfg_output_digest,
        );
        let calls = if !run_semantic_pipeline {
            Default::default()
        } else if run_cfg_call_pipeline {
            if Self::provider_dependencies_ready(
                &[
                    &semantic_mir_dependency_output,
                    &cfg_dependency_output,
                    &symbol_dependency_output,
                    &module_topology_dependency_output,
                ],
                &[],
            ) {
                crate::analysis::calls::provider::derive_calls_with_cache_stats(
                    &mut db,
                    &input_snapshot,
                    Self::provider_manifest("polint.calls"),
                    semantic_mir_dependency_output.output_digest.clone(),
                    cfg_dependency_output.output_digest.clone(),
                    symbol_dependency_output.output_digest.clone(),
                    module_topology_dependency_output.output_digest.clone(),
                    vec![
                        go_dependency_output_digest.clone(),
                        ts_dependency_output_digest.clone(),
                    ],
                )
            } else {
                dependency_blocked_output!(crate::analysis::calls::provider::CallsProviderOutput)
            }
        } else if Self::provider_dependencies_ready(&[&semantic_mir_dependency_output], &[]) {
            crate::analysis::calls::provider::derive_call_sites_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.calls"),
                semantic_mir_dependency_output.output_digest.clone(),
                cfg_dependency_output.output_digest.clone(),
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            )
        } else {
            dependency_blocked_output!(crate::analysis::calls::provider::CallsProviderOutput)
        };
        let polint_calls_cache_stats = calls.cache_stats.clone();
        let calls_execution = calls.execution;
        let calls_output_digest = calls.output_digest.clone();
        diagnostics.extend(calls.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.calls",
            polint_calls_cache_stats,
            calls_execution,
            calls_output_digest.clone(),
        ));

        let calls_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.calls",
            calls_execution,
            calls_output_digest,
        );

        let go_semantic = if run_full_refinement_pipeline {
            crate::go::semantic::provider::derive_go_semantic_with_cache_stats(
                &mut db,
                input.loaded,
                &input_snapshot,
                Self::provider_manifest("polint.go.semantic"),
                go_dependency_output_digest.clone(),
                &runtime_sources.go_semantic_tool,
            )
        } else {
            Default::default()
        };
        let polint_go_semantic_cache_stats = go_semantic.cache_stats.clone();
        let go_semantic_execution = go_semantic.execution;
        let go_semantic_output_digest = go_semantic.output_digest.clone();
        diagnostics.extend(go_semantic.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.go.semantic",
            polint_go_semantic_cache_stats,
            go_semantic_execution,
            go_semantic_output_digest.clone(),
        ));
        let go_semantic_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.go.semantic",
            go_semantic_execution,
            go_semantic_output_digest,
        );

        let identity = if !run_full_refinement_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(
            &[&calls_dependency_output],
            &[&go_semantic_dependency_output],
        ) {
            crate::analysis::identity::provider::derive_identity_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.identity"),
                calls_dependency_output.output_digest.clone(),
                go_semantic_dependency_output.output_digest.clone(),
            )
        } else {
            dependency_blocked_output!(
                crate::analysis::identity::provider::IdentityProviderRunOutput
            )
        };
        let polint_identity_cache_stats = identity.cache_stats.clone();
        let identity_execution = identity.execution;
        let identity_output_digest = identity.output_digest;
        diagnostics.extend(identity.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.identity",
            polint_identity_cache_stats,
            identity_execution,
            identity_output_digest.clone(),
        ));
        let identity_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.identity",
            identity_execution,
            identity_output_digest,
        );

        let refinement_inputs_ready = Self::provider_dependencies_ready(
            &[
                &semantic_mir_dependency_output,
                &cfg_dependency_output,
                &calls_dependency_output,
                &symbol_dependency_output,
                &module_topology_dependency_output,
            ],
            &[],
        );
        let abstract_domains = if !run_full_refinement_pipeline {
            Default::default()
        } else if !refinement_inputs_ready {
            dependency_blocked_output!(
                crate::analysis::domains::provider::AbstractDomainsProviderOutput
            )
        } else if compact_domain_materialization {
            crate::analysis::domains::provider::derive_summary_input_abstract_domains_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.abstract_domains"),
                semantic_mir_dependency_output.output_digest.clone(),
                cfg_dependency_output.output_digest.clone(),
                calls_dependency_output.output_digest.clone(),
                symbol_dependency_output.output_digest.clone(),
                module_topology_dependency_output.output_digest.clone(),
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            )
        } else {
            crate::analysis::domains::provider::derive_abstract_domains_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.abstract_domains"),
                semantic_mir_dependency_output.output_digest.clone(),
                cfg_dependency_output.output_digest.clone(),
                calls_dependency_output.output_digest.clone(),
                symbol_dependency_output.output_digest.clone(),
                module_topology_dependency_output.output_digest.clone(),
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            )
        };
        let polint_abstract_domains_cache_stats = abstract_domains.cache_stats.clone();
        let abstract_domains_execution = abstract_domains.execution;
        let abstract_domains_output_digest = abstract_domains.output_digest;
        diagnostics.extend(abstract_domains.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.abstract_domains",
            polint_abstract_domains_cache_stats,
            abstract_domains_execution,
            abstract_domains_output_digest.clone(),
        ));

        let abstract_domains_dependency_output =
            incremental::ProviderOutputDependency::from_execution(
                "polint.abstract_domains",
                abstract_domains_execution,
                abstract_domains_output_digest,
            );
        let entrypoints_semantic_mir_digest = semantic_mir_dependency_output.output_digest.clone();
        let entrypoints_cfg_digest = cfg_dependency_output.output_digest.clone();
        let entrypoints_calls_digest = calls_dependency_output.output_digest.clone();
        let entrypoints_symbol_digest = symbol_dependency_output.output_digest.clone();
        let entrypoints_topology_digest = module_topology_dependency_output.output_digest.clone();
        let direct_summaries = if !run_full_refinement_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(
            &[
                &semantic_mir_dependency_output,
                &cfg_dependency_output,
                &calls_dependency_output,
                &abstract_domains_dependency_output,
                &symbol_dependency_output,
                &module_topology_dependency_output,
            ],
            &[],
        ) {
            crate::analysis::summaries::provider::derive_direct_summaries_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.direct_summaries"),
                semantic_mir_dependency_output.output_digest.clone(),
                cfg_dependency_output.output_digest.clone(),
                calls_dependency_output.output_digest.clone(),
                abstract_domains_dependency_output.output_digest.clone(),
                symbol_dependency_output.output_digest.clone(),
                module_topology_dependency_output.output_digest.clone(),
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            )
        } else {
            dependency_blocked_output!(
                crate::analysis::summaries::provider::DirectSummariesProviderOutput
            )
        };
        let polint_direct_summaries_cache_stats = direct_summaries.cache_stats.clone();
        let direct_summaries_execution = direct_summaries.execution;
        let direct_summaries_initial_dependency =
            incremental::ProviderOutputDependency::from_execution(
                "polint.direct_summaries",
                direct_summaries_execution,
                direct_summaries.output_digest.clone(),
            );
        let direct_summaries_initial_output_digest =
            direct_summaries_initial_dependency.output_digest;
        diagnostics.extend(direct_summaries.diagnostics);

        // SCC closure: interprocedural summary improvement over SCCs.
        // Runs after direct summaries so callee summaries are available.
        let scc_closure =
            if direct_summaries_execution == incremental::ProviderExecutionOutcome::Succeeded {
                crate::analysis::summaries::provider::run_scc_closure_with_cache(
                    &mut db,
                    input.cache,
                    &input_snapshot,
                    &direct_summaries_initial_output_digest,
                    &entrypoints_calls_digest,
                )
            } else {
                Default::default()
            };
        #[cfg(test)]
        let scc_closure_debug = scc_closure.debug_snapshot;
        diagnostics.extend(scc_closure.diagnostics);
        let final_direct_summaries_output = crate::analysis::summaries::store::SummaryOutput {
            summaries: db.summary_facts().to_vec(),
            events: db.summary_events().to_vec(),
        };
        let direct_summaries_output_digest = (direct_summaries_execution
            == incremental::ProviderExecutionOutcome::Succeeded)
            .then(|| {
                crate::analysis::summaries::provider::direct_summaries_output_digest(
                    Self::provider_manifest("polint.direct_summaries"),
                    &input_snapshot,
                    &entrypoints_semantic_mir_digest,
                    &entrypoints_cfg_digest,
                    &entrypoints_calls_digest,
                    &abstract_domains_dependency_output.output_digest,
                    &entrypoints_symbol_digest,
                    &entrypoints_topology_digest,
                    &[
                        go_dependency_output_digest.clone(),
                        ts_dependency_output_digest.clone(),
                    ],
                    &crate::analysis::summaries::provider::callable_stable_key_map(&db),
                    &final_direct_summaries_output,
                )
            });
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.direct_summaries",
            polint_direct_summaries_cache_stats,
            direct_summaries_execution,
            direct_summaries_output_digest.clone(),
        ));
        let direct_summaries_dependency_output =
            incremental::ProviderOutputDependency::from_execution(
                "polint.direct_summaries",
                direct_summaries_execution,
                direct_summaries_output_digest,
            );

        let entrypoints = if !run_full_refinement_pipeline {
            Default::default()
        } else if refinement_inputs_ready {
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
            )
        } else {
            dependency_blocked_output!(
                crate::analysis::entrypoints::provider::EntrypointsProviderOutput
            )
        };
        let polint_entrypoints_cache_stats = entrypoints.cache_stats.clone();
        let entrypoints_execution = entrypoints.execution;
        let entrypoints_output_digest = entrypoints.output_digest.clone();
        diagnostics.extend(entrypoints.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.entrypoints",
            polint_entrypoints_cache_stats,
            entrypoints_execution,
            entrypoints_output_digest.clone(),
        ));
        let entrypoints_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.entrypoints",
            entrypoints_execution,
            entrypoints_output_digest,
        );

        // Reachability runs immediately after entrypoint discovery and consumes
        // the calls, entrypoints, identity, symbol, and topology outputs.
        let reachability = if !run_full_refinement_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(
            &[
                &calls_dependency_output,
                &entrypoints_dependency_output,
                &identity_dependency_output,
                &symbol_dependency_output,
                &module_topology_dependency_output,
            ],
            &[],
        ) {
            crate::analysis::reachability::provider::derive_reachability_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.reachability"),
                &input.loaded.config.reachability.roots,
                entrypoints_calls_digest.clone(),
                entrypoints_dependency_output.output_digest.clone(),
                identity_dependency_output.output_digest,
                entrypoints_symbol_digest.clone(),
                entrypoints_topology_digest.clone(),
            )
        } else {
            dependency_blocked_output!(
                crate::analysis::reachability::provider::ReachabilityProviderRunOutput
            )
        };
        let polint_reachability_cache_stats = reachability.cache_stats.clone();
        let reachability_execution = reachability.execution;
        let reachability_output_digest = reachability.output_digest;
        diagnostics.extend(reachability.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.reachability",
            polint_reachability_cache_stats,
            reachability_execution,
            reachability_output_digest.clone(),
        ));
        let reachability_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.reachability",
            reachability_execution,
            reachability_output_digest,
        );

        let extensions = if run_full_refinement_pipeline {
            crate::analysis::extensions::provider::derive_extension_provider_outputs_with_cache_stats(
                &mut db,
                &input.loaded.root,
                &input_snapshot,
                Self::provider_manifest("polint.extensions"),
            )
        } else {
            Default::default()
        };
        let polint_extensions_cache_stats = extensions.cache_stats.clone();
        let extensions_execution = extensions.execution;
        let extensions_output_digest = extensions.output_digest.clone();
        diagnostics.extend(extensions.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.extensions",
            polint_extensions_cache_stats,
            extensions_execution,
            extensions_output_digest.clone(),
        ));
        let extensions_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.extensions",
            extensions_execution,
            extensions_output_digest,
        );

        let type_value_alias = if !run_full_refinement_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(
            &[
                &semantic_mir_dependency_output,
                &cfg_dependency_output,
                &calls_dependency_output,
                &abstract_domains_dependency_output,
                &direct_summaries_dependency_output,
                &entrypoints_dependency_output,
                &extensions_dependency_output,
                &symbol_dependency_output,
                &module_topology_dependency_output,
            ],
            &[],
        ) {
            crate::analysis::types::provider::derive_type_value_alias_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.type_value_alias"),
                entrypoints_semantic_mir_digest.clone(),
                entrypoints_cfg_digest.clone(),
                entrypoints_calls_digest.clone(),
                abstract_domains_dependency_output.output_digest,
                direct_summaries_dependency_output.output_digest.clone(),
                entrypoints_dependency_output.output_digest.clone(),
                extensions_dependency_output.output_digest.clone(),
                entrypoints_symbol_digest.clone(),
                entrypoints_topology_digest,
                vec![
                    go_dependency_output_digest.clone(),
                    ts_dependency_output_digest.clone(),
                ],
            )
        } else {
            dependency_blocked_output!(
                crate::analysis::types::provider::TypeValueAliasProviderOutput
            )
        };
        let polint_type_value_alias_cache_stats = type_value_alias.cache_stats.clone();
        let type_value_alias_execution = type_value_alias.execution;
        let type_value_alias_output_digest = type_value_alias.output_digest.clone();
        diagnostics.extend(type_value_alias.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.type_value_alias",
            polint_type_value_alias_cache_stats,
            type_value_alias_execution,
            type_value_alias_output_digest.clone(),
        ));

        let type_value_alias_dependency_output =
            incremental::ProviderOutputDependency::from_execution(
                "polint.type_value_alias",
                type_value_alias_execution,
                type_value_alias_output_digest,
            );

        // Semantic graph projection runs after type/value aliases. It projects the
        // unified graph from stored facts and repo-local adaptation models, folding
        // every consumed upstream provider output into its own identity.
        let semantic_graph = if !run_full_refinement_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(
            &[
                &calls_dependency_output,
                &type_value_alias_dependency_output,
                &symbol_dependency_output,
                &semantic_mir_dependency_output,
            ],
            &[&go_semantic_dependency_output],
        ) {
            crate::analysis::semantic_graph::provider::derive_semantic_graph_with_cache_stats_and_models(
                &mut db,
                solver_budget.adaptation,
                &runtime_sources.adaptation_models,
                &input_snapshot,
                Self::provider_manifest("polint.semantic_graph"),
                entrypoints_calls_digest.clone(),
                type_value_alias_dependency_output.output_digest.clone(),
                entrypoints_symbol_digest,
                go_dependency_output_digest,
                ts_dependency_output_digest,
                entrypoints_semantic_mir_digest.clone(),
                go_semantic_dependency_output.output_digest.clone(),
            )
        } else {
            dependency_blocked_output!(
                crate::analysis::semantic_graph::provider::SemanticGraphProviderRunOutput
            )
        };
        let polint_semantic_graph_cache_stats = semantic_graph.cache_stats.clone();
        let semantic_graph_execution = semantic_graph.execution;
        let semantic_graph_output_digest = semantic_graph.output_digest.clone();
        diagnostics.extend(semantic_graph.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.semantic_graph",
            polint_semantic_graph_cache_stats,
            semantic_graph_execution,
            semantic_graph_output_digest.clone(),
        ));

        let semantic_graph_dependency_output =
            incremental::ProviderOutputDependency::from_execution(
                "polint.semantic_graph",
                semantic_graph_execution,
                semantic_graph_output_digest,
            );

        // The solver consumes the closed semantic graph and emits derived edges with
        // provenance. Its identity includes semantic graph, type/value aliases,
        // Go-semantic output, and SolverBudget because Go RTA reads instantiated-type,
        // address-taken, dynamic-dispatch, and method-set signals from that output.
        let solver = if !run_full_refinement_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(
            &[
                &semantic_graph_dependency_output,
                &type_value_alias_dependency_output,
                &reachability_dependency_output,
            ],
            &[&go_semantic_dependency_output],
        ) {
            crate::analysis::solver::provider::derive_solver_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.solver"),
                solver_budget,
                semantic_graph_dependency_output.output_digest,
                type_value_alias_dependency_output.output_digest.clone(),
                go_semantic_dependency_output.output_digest,
            )
        } else {
            dependency_blocked_output!(crate::analysis::solver::provider::SolverProviderRunOutput)
        };
        let polint_solver_cache_stats = solver.cache_stats.clone();
        let solver_execution = solver.execution;
        let solver_output_digest = solver.output_digest.clone();
        diagnostics.extend(solver.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.solver",
            polint_solver_cache_stats,
            solver_execution,
            solver_output_digest.clone(),
        ));
        let solver_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.solver",
            solver_execution,
            solver_output_digest,
        );

        let refined_calls = if !run_cfg_call_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(
            &[
                &calls_dependency_output,
                &entrypoints_dependency_output,
                &direct_summaries_dependency_output,
                &type_value_alias_dependency_output,
                &extensions_dependency_output,
                &solver_dependency_output,
            ],
            &[],
        ) {
            crate::analysis::refined_calls::provider::derive_refined_calls_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.refined_calls"),
                entrypoints_calls_digest.clone(),
                entrypoints_dependency_output.output_digest.clone(),
                direct_summaries_dependency_output.output_digest.clone(),
                type_value_alias_dependency_output.output_digest.clone(),
                extensions_dependency_output.output_digest.clone(),
                solver_dependency_output.output_digest,
            )
        } else {
            dependency_blocked_output!(
                crate::analysis::refined_calls::provider::RefinedCallsProviderOutput
            )
        };
        let polint_refined_calls_cache_stats = refined_calls.cache_stats.clone();
        let refined_calls_execution = refined_calls.execution;
        let refined_calls_output_digest = refined_calls.output_digest.clone();
        diagnostics.extend(refined_calls.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.refined_calls",
            polint_refined_calls_cache_stats,
            refined_calls_execution,
            refined_calls_output_digest.clone(),
        ));

        let refined_calls_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.refined_calls",
            refined_calls_execution,
            refined_calls_output_digest,
        );
        let data_flow = if !run_data_flow_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(
            &[
                &semantic_mir_dependency_output,
                &cfg_dependency_output,
                &calls_dependency_output,
                &refined_calls_dependency_output,
                &direct_summaries_dependency_output,
                &type_value_alias_dependency_output,
                &entrypoints_dependency_output,
                &extensions_dependency_output,
            ],
            &[],
        ) {
            crate::analysis::data_flow::provider::derive_data_flow_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.data_flow"),
                entrypoints_semantic_mir_digest.clone(),
                entrypoints_cfg_digest.clone(),
                entrypoints_calls_digest.clone(),
                refined_calls_dependency_output.output_digest.clone(),
                direct_summaries_dependency_output.output_digest.clone(),
                type_value_alias_dependency_output.output_digest.clone(),
                entrypoints_dependency_output.output_digest.clone(),
                extensions_dependency_output.output_digest.clone(),
            )
        } else {
            dependency_blocked_output!(crate::analysis::data_flow::provider::DataFlowProviderOutput)
        };
        let polint_data_flow_cache_stats = data_flow.cache_stats.clone();
        let data_flow_execution = data_flow.execution;
        let data_flow_output_digest = data_flow.output_digest.clone();
        diagnostics.extend(data_flow.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.data_flow",
            polint_data_flow_cache_stats,
            data_flow_execution,
            data_flow_output_digest.clone(),
        ));

        let data_flow_dependency_output = incremental::ProviderOutputDependency::from_execution(
            "polint.data_flow",
            data_flow_execution,
            data_flow_output_digest,
        );
        let evidence = if !run_data_flow_pipeline {
            Default::default()
        } else if Self::provider_dependencies_ready(
            &[
                &semantic_mir_dependency_output,
                &cfg_dependency_output,
                &calls_dependency_output,
                &refined_calls_dependency_output,
                &direct_summaries_dependency_output,
                &type_value_alias_dependency_output,
                &entrypoints_dependency_output,
                &extensions_dependency_output,
                &data_flow_dependency_output,
            ],
            &[],
        ) {
            crate::analysis::evidence::provider::derive_evidence_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.evidence"),
                entrypoints_semantic_mir_digest,
                entrypoints_cfg_digest,
                entrypoints_calls_digest,
                refined_calls_dependency_output.output_digest,
                direct_summaries_dependency_output.output_digest,
                type_value_alias_dependency_output.output_digest,
                entrypoints_dependency_output.output_digest,
                extensions_dependency_output.output_digest,
                data_flow_dependency_output.output_digest,
            )
        } else {
            dependency_blocked_output!(crate::analysis::evidence::provider::EvidenceProviderOutput)
        };
        let polint_evidence_cache_stats = evidence.cache_stats.clone();
        let evidence_execution = evidence.execution;
        let evidence_output_digest = evidence.output_digest;
        diagnostics.extend(evidence.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome(
            "polint.evidence",
            polint_evidence_cache_stats,
            evidence_execution,
            evidence_output_digest,
        ));

        let metrics = crate::metrics::derive_requested_metrics_with_cache_stats(
            &mut db,
            input.plan,
            input.cache,
            &input_snapshot,
            Self::provider_manifest("polint.metrics"),
            vec![go_dependency_output, ts_dependency_output],
        );
        let polint_metrics_cache_stats = metrics.cache_stats.clone();
        let metrics_execution = metrics.execution;
        let metrics_output_digest = metrics.output_digest;
        let metrics_layers = metrics.layers;
        diagnostics.extend(metrics.diagnostics);
        provider_outputs.push(Self::provider_output_for_outcome_and_layers(
            "polint.metrics",
            polint_metrics_cache_stats,
            metrics_execution,
            metrics_output_digest,
            metrics_layers,
        ));
        tracing::info!(target: "polint::kernel", "stage: metrics + derived done");
        let validation_report = validation::validate_fact_metadata(&db, Self::provider_manifests());
        diagnostics.extend(validation_report.diagnostics);
        let validation_events = validation_report.events;
        db.finish_all_fact_meta_insertions();
        let store_outcome = if input.cache.semantic_store_enabled() {
            let fact_meta = db.take_fact_meta_for_store();
            store::record_handoff_materialization();
            match fact_meta.prepare_compact_stable_rows() {
                Ok(prepared_facts) => {
                    // Both operations use internal data parallelism. Finish the
                    // large fact-key compaction first so its plain source keys
                    // do not coexist with the canonical dependency projection.
                    match prepared_facts.finish_validated() {
                        Ok(finalized_facts) => {
                            let prepared =
                                incremental::ValidatedRunMetadata::prepare_finalized_canonical_run(
                                    &input_snapshot,
                                    &provider_outputs,
                                    &scc_closure.demand_query_trace,
                                    &validation_events,
                                    Self::provider_manifests(),
                                );
                            match prepared {
                                Ok(prepared) => {
                                    match incremental::ValidatedRunMetadata::finish_prepared_canonical_run(
                                        prepared,
                                        finalized_facts,
                                    ) {
                                        Ok(validated) => store::SemanticStore::commit_validated_run(
                                            &store::StoreConfig::new(
                                                input.cache.semantic_store_path(),
                                                true,
                                            ),
                                            validated,
                                        ),
                                        Err(_) => store::StoreOutcome::invalid_metadata(),
                                    }
                                }
                                Err(_) => store::StoreOutcome::invalid_metadata(),
                            }
                        }
                        Err(_) => store::StoreOutcome::invalid_metadata(),
                    }
                }
                Err(_) => store::StoreOutcome::invalid_metadata(),
            }
        } else {
            store::StoreOutcome::disabled()
        };
        let run_report = incremental::KernelRunReport::new(
            input_snapshot,
            provider_outputs,
            scc_closure.demand_query_trace,
            validation_events,
            store_outcome,
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

    #[cfg(test)]
    pub(crate) fn semantic_store_schema_is_current_for_test(path: &std::path::Path) -> bool {
        store::current_schema_is_valid_for_test(path)
    }

    fn provider_output_for(
        provider_id: &'static str,
        db: &AnalysisDb,
        cache_stats: incremental::CacheStats,
    ) -> incremental::ProviderOutputMeta {
        let manifest = Self::provider_manifest(provider_id);
        let (output_digest, validation) = match provider_output_summary_parts(db, manifest) {
            Ok(rows) => (
                incremental::provider_output_digest_from_manifest(manifest, &rows),
                incremental::ProviderValidationStatus::NativeTrusted,
            ),
            Err(_) => (
                incremental::Digest::unsupported(
                    incremental::DigestKind::ProviderOutput,
                    manifest.id,
                    "conflicting stable fact metadata",
                ),
                incremental::ProviderValidationStatus::ProviderFailed,
            ),
        };
        let mut meta = incremental::provider_output_from_manifest_with_layers(
            manifest,
            output_digest,
            Vec::new(),
            cache_stats,
        );
        meta.validation = validation;
        meta
    }

    fn provider_output_for_outcome(
        provider_id: &'static str,
        cache_stats: incremental::CacheStats,
        execution: incremental::ProviderExecutionOutcome,
        output_digest: Option<incremental::Digest>,
    ) -> incremental::ProviderOutputMeta {
        Self::provider_output_for_outcome_and_layers(
            provider_id,
            cache_stats,
            execution,
            output_digest,
            Vec::new(),
        )
    }

    fn provider_output_for_outcome_and_layers(
        provider_id: &'static str,
        cache_stats: incremental::CacheStats,
        execution: incremental::ProviderExecutionOutcome,
        output_digest: Option<incremental::Digest>,
        layers: Vec<incremental::LayerRunMetadata>,
    ) -> incremental::ProviderOutputMeta {
        let manifest = Self::provider_manifest(provider_id);
        let layers = if execution == incremental::ProviderExecutionOutcome::Succeeded {
            layers
        } else {
            Vec::new()
        };
        let (output_digest, validation) = match (execution, output_digest) {
            (incremental::ProviderExecutionOutcome::Succeeded, Some(output_digest)) => (
                output_digest,
                incremental::ProviderValidationStatus::NativeTrusted,
            ),
            (incremental::ProviderExecutionOutcome::Skipped, _) => (
                incremental::Digest::absent(incremental::DigestKind::ProviderOutput, provider_id),
                incremental::ProviderValidationStatus::Skipped,
            ),
            (incremental::ProviderExecutionOutcome::Failed, _)
            | (incremental::ProviderExecutionOutcome::Succeeded, None) => (
                incremental::Digest::unsupported(
                    incremental::DigestKind::ProviderOutput,
                    manifest.id,
                    "provider execution failed",
                ),
                incremental::ProviderValidationStatus::ProviderFailed,
            ),
        };
        let mut meta = incremental::provider_output_from_manifest_with_layers(
            manifest,
            output_digest,
            layers,
            cache_stats,
        );
        meta.validation = validation;
        meta
    }

    /// Union of every enabled rule's `files` scope, as a glob set, or `None` when the
    /// set cannot safely narrow discovery — i.e. there are no rules, or some rule has
    /// an empty `files` scope (which matches every file). Returning `None` falls back
    /// to full workspace discovery.
    fn rule_scope_globset(plan: &AnalysisPlan) -> Option<globset::GlobSet> {
        let rules = plan.rules();
        if rules.is_empty() {
            return None;
        }
        let mut patterns = Vec::new();
        for rule in rules {
            if rule.files.is_empty() {
                return None;
            }
            patterns.extend(rule.files.iter().cloned());
        }
        crate::config::build_glob_set(&patterns).ok()
    }

    fn provider_manifest(provider_id: &str) -> &'static ProviderManifest {
        Self::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == provider_id)
            .unwrap_or_else(|| panic!("missing provider manifest {provider_id}"))
    }

    fn directly_requests_any_capability(plan: &AnalysisPlan, capabilities: &[&str]) -> bool {
        plan.rules().iter().any(|rule| {
            rule.requested_capabilities
                .iter()
                .any(|requested| capabilities.contains(&requested.as_str()))
        })
    }
}

fn provider_output_summary_parts(
    db: &AnalysisDb,
    manifest: &ProviderManifest,
) -> Result<Vec<StableFactMetaRow>, StableFactMetaConflict> {
    Ok(db
        .fact_meta()
        .stable_rows()?
        .into_iter()
        .filter(|row| row.producer_id == manifest.id || row.layer_id == manifest.id)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{CacheStats, INPUT_SNAPSHOT_SCHEMA_VERSION};
    use crate::analysis_plan::RulePlanInputs;
    use crate::cache::keys::config_hash;
    use crate::config::{LoadedConfig, RuleConfig, load_config};
    use crate::core::{
        BranchObligation, Capabilities, CapabilitySupportStatus, ComplexityMetricFact,
        CoverageFact, DefinitionFact, DefinitionId, DefinitionKind, FileMetricFact, FunctionFact,
        FunctionId, FunctionMetricFact, ImportFact, ImportId, JsxAttributeFact, Language,
        ModuleEdge, ModuleEdgeId, ModuleEdgeKind, ModuleNode, ModuleNodeId, ModuleNodeKind,
        PackageFact, PackageId, ReferenceFact, ReferenceId, ReferenceKind, ResolutionPrecision,
        ResolutionStatus, ResolvedImportFact, ResolvedImportId, Rule, RuleKind, RuleMeta, Span,
        StringLiteralFact, SymbolFact, SymbolId, SymbolKind, SymbolNamespace, SymbolPrecision,
        SymbolResolutionStatus, TestFact, TsClassFact, TsComponentFact,
    };
    use crate::diagnostics::Severity;
    use std::path::{Path, PathBuf};
    use std::time::Instant;

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

    mod semantic_store {
        use super::*;

        #[test]
        fn kernel_uses_only_the_enabled_post_finalization_store_facade() {
            let source = include_str!("mod.rs");
            let production = source
                .split_once("#[cfg(test)]\nmod tests")
                .expect("kernel test module boundary")
                .0;
            let validation = production
                .find("let validation_report = validation::validate_fact_metadata")
                .expect("authoritative validation report");
            let diagnostics = production
                .find("diagnostics.extend(validation_report.diagnostics)")
                .expect("validation diagnostics are preserved");
            let finalization = production
                .find("db.finish_all_fact_meta_insertions()")
                .expect("fact metadata finalization");
            let enabled_guard = production
                .find("if input.cache.semantic_store_enabled()")
                .expect("enabled-only store branch");
            let handoff = production
                .find("incremental::ValidatedRunMetadata::prepare_finalized_canonical_run")
                .expect("validated-run handoff");
            let handoff_finish = production
                .find("incremental::ValidatedRunMetadata::finish_prepared_canonical_run")
                .expect("validated-run handoff completion");
            let facade = production
                .find("store::SemanticStore::commit_validated_run")
                .expect("sole parent-facing store facade");
            let path = production
                .find("input.cache.semantic_store_path()")
                .expect("private store path construction");
            assert!(
                validation < diagnostics
                    && diagnostics < finalization
                    && finalization < enabled_guard
                    && enabled_guard < handoff
                    && handoff < handoff_finish
                    && handoff_finish < facade
                    && handoff < facade
                    && enabled_guard < path
            );
            for forbidden in ["commit_plan", "StoreCommitPlan", "from_validated_run"] {
                assert!(
                    !production.contains(forbidden),
                    "kernel must not name store planning detail `{forbidden}`"
                );
            }
        }

        #[test]
        fn disabled_full_kernel_run_is_filesystem_free_and_records_status() {
            store::reset_materialization_counters_for_test();
            let temp = tempfile::tempdir().expect("temp directory");
            let loaded = load_config(temp.path()).expect("default config loads");
            let cache = Cache::default_for_repo(temp.path(), true);
            let store_path = cache.semantic_store_path();
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

            assert_eq!(output.run_report.store_status(), &StoreStatus::Disabled);
            assert_eq!(output.run_report.validation_events().len(), 20);
            assert!(!store_path.exists());
            assert!(!store_path.parent().expect("store directory").exists());
            assert_eq!(store::materialization_counters_for_test(), (0, 0, 0));
        }

        #[test]
        fn no_cache_full_kernel_run_is_filesystem_free_and_materialization_free() {
            store::reset_materialization_counters_for_test();
            let temp = tempfile::tempdir().expect("temp directory");
            let loaded = load_config(temp.path()).expect("default config loads");
            let cache = Cache::default_for_repo(temp.path(), false);
            let store_path = cache.semantic_store_path();

            let output = AnalysisKernel::run(KernelInput {
                loaded: &loaded,
                cache: &cache,
                config_digest: "config",
                rule_digest: "rules",
                plan: &AnalysisPlan::empty(),
                parallel: false,
            })
            .expect("kernel should run");

            assert_eq!(output.run_report.store_status(), &StoreStatus::Disabled);
            assert!(!store_path.exists());
            assert!(!store_path.parent().expect("store directory").exists());
            assert_eq!(store::materialization_counters_for_test(), (0, 0, 0));
        }

        #[test]
        fn enabled_maintenance_runs_after_validated_fact_finalization() {
            store::reset_materialization_counters_for_test();
            let temp = tempfile::tempdir().expect("temp directory");
            std::fs::write(temp.path().join("main.go"), "package main\n").expect("write source");
            let loaded = load_config(temp.path()).expect("default config loads");
            let cache =
                Cache::default_for_repo(temp.path(), true).with_semantic_store_enabled_for_test();
            let store_path = cache.semantic_store_path();
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

            assert_eq!(output.run_report.store_status(), &StoreStatus::Ready);
            assert_eq!(output.run_report.validation_events().len(), 20);
            assert!(store_path.is_file());
            assert_eq!(store::materialization_counters_for_test(), (1, 1, 1));
            assert!(!output.db.files().is_empty());
            assert_eq!(output.db.fact_meta().rows().count(), 0);
        }

        fn assert_syntax_dependency_status(
            output: &KernelOutput,
            syntax_provider_id: &str,
            expected: incremental::InputComponentStatus,
        ) {
            for provider_id in [
                "polint.module_graph",
                "polint.symbol_graph",
                "polint.metrics",
            ] {
                let provider = super::provider_output(output, provider_id);
                let layer = provider
                    .layers
                    .first()
                    .unwrap_or_else(|| panic!("{provider_id} should publish a retained layer"));
                let input = layer
                    .dependencies
                    .iter()
                    .find_map(|edge| match &edge.to {
                        incremental::CacheNode::DependencyInput(input)
                            if input.kind == incremental::InputDependencyKind::UpstreamLayer
                                && input.stable_key.starts_with(syntax_provider_id) =>
                        {
                            Some(input)
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| {
                        panic!("{provider_id} should depend on {syntax_provider_id}")
                    });
                assert_eq!(input.status, expected, "{provider_id} dependency status");
            }
        }

        #[test]
        fn language_syntax_availability_round_trips_through_retained_stores() {
            for (initial_path, initial_source, added_path, added_source, present, absent) in [
                (
                    "main.go",
                    "package main\nfunc main() {}\n",
                    "app.ts",
                    "export const app = 1;\n",
                    "polint.go.syntax",
                    "polint.ts.syntax",
                ),
                (
                    "app.ts",
                    "export const app = 1;\n",
                    "main.go",
                    "package main\nfunc main() {}\n",
                    "polint.ts.syntax",
                    "polint.go.syntax",
                ),
            ] {
                let temp = tempfile::tempdir().expect("temporary language fixture");
                std::fs::write(temp.path().join(initial_path), initial_source)
                    .expect("write initial language source");
                let loaded = load_config(temp.path()).expect("default config loads");
                let cache = Cache::default_for_repo(temp.path(), true)
                    .with_semantic_store_enabled_for_test();
                let plan = AnalysisPlan::from_capability_names_for_test(&[
                    "module_graph",
                    "symbols",
                    "references",
                    "file_metrics",
                ]);
                let run = || {
                    AnalysisKernel::run(KernelInput {
                        loaded: &loaded,
                        cache: &cache,
                        config_digest: "config",
                        rule_digest: "rules",
                        plan: &plan,
                        parallel: false,
                    })
                    .expect("language availability kernel run")
                };

                let cold = run();
                assert_eq!(cold.run_report.store_status(), &StoreStatus::Ready);
                assert_syntax_dependency_status(
                    &cold,
                    present,
                    incremental::InputComponentStatus::Present,
                );
                assert_syntax_dependency_status(
                    &cold,
                    absent,
                    incremental::InputComponentStatus::Absent,
                );

                let warm = run();
                assert_eq!(warm.run_report.store_status(), &StoreStatus::Ready);
                assert_syntax_dependency_status(
                    &warm,
                    absent,
                    incremental::InputComponentStatus::Absent,
                );
                for provider_id in [
                    "polint.module_graph",
                    "polint.symbol_graph",
                    "polint.metrics",
                ] {
                    assert_eq!(
                        super::provider_output(&warm, provider_id).cache_stats.hits,
                        1
                    );
                }

                std::fs::write(temp.path().join(added_path), added_source)
                    .expect("write sibling language source");
                let with_sibling = run();
                assert_eq!(with_sibling.run_report.store_status(), &StoreStatus::Ready);
                assert_syntax_dependency_status(
                    &with_sibling,
                    "polint.go.syntax",
                    incremental::InputComponentStatus::Present,
                );
                assert_syntax_dependency_status(
                    &with_sibling,
                    "polint.ts.syntax",
                    incremental::InputComponentStatus::Present,
                );
            }
        }
    }

    mod semantic_store_check_parity {
        use super::*;
        use crate::diagnostics::{
            ColorChoice, JsonReportMeta, OutputFormat, RenderOpts, Severity, render,
            sort_diagnostics,
        };

        #[derive(Clone, Copy)]
        enum StoreMode {
            Disabled,
            Enabled,
            Complete,
            WorkspaceMismatch,
            FirstFailureRecovery,
            AuditedFailed,
            Pending,
            Corrupt,
            Future,
            Invalid,
            Busy,
            #[cfg(unix)]
            Unsafe,
        }

        fn validated_setup_run(
            loaded: &LoadedConfig,
            cache: &Cache,
            plan: &AnalysisPlan,
        ) -> incremental::ValidatedRunMetadata {
            let output = AnalysisKernel::run(KernelInput {
                loaded,
                cache,
                config_digest: "config",
                rule_digest: "rules",
                plan,
                parallel: false,
            })
            .expect("setup kernel run");
            let fact_rows = output
                .db
                .fact_meta()
                .stable_rows()
                .expect("setup fact metadata is canonical");
            incremental::ValidatedRunMetadata::from_finalized_run(
                &output.run_report.input_snapshot,
                &output.run_report.provider_outputs,
                output.run_report.demand_query_trace(),
                output.run_report.validation_events(),
                AnalysisKernel::provider_manifests(),
                fact_rows,
            )
            .expect("setup run is a complete handoff")
        }

        fn run_mode(mode: StoreMode) -> (String, u8, StoreStatus) {
            let temp = tempfile::tempdir().expect("temp directory");
            std::fs::write(
                temp.path().join("main.go"),
                "package main\n\nfunc main() { println(\"hello\") }\n",
            )
            .expect("write source");
            let loaded = load_config(temp.path()).expect("default config loads");
            let plan = AnalysisPlan::empty();
            let base_cache = Cache::default_for_repo(temp.path(), true);
            let path = base_cache.semantic_store_path();
            let config = store::StoreConfig::new(&path, true);
            let setup = matches!(
                mode,
                StoreMode::Complete
                    | StoreMode::FirstFailureRecovery
                    | StoreMode::AuditedFailed
                    | StoreMode::Pending
            )
            .then(|| validated_setup_run(&loaded, &base_cache, &plan));
            let cache = if matches!(mode, StoreMode::Disabled) {
                base_cache
            } else {
                base_cache.with_semantic_store_enabled_for_test()
            };

            let mut corrupt_before = None;
            let mut fixture_before = None;
            let mut held_writer = None;
            #[cfg(unix)]
            let mut unsafe_outside = None;
            match mode {
                StoreMode::Disabled | StoreMode::Enabled => {}
                StoreMode::Complete => {
                    let outcome = store::SemanticStore::commit_validated_run(
                        &config,
                        setup.as_ref().expect("complete setup").clone(),
                    );
                    assert_eq!(outcome.status, StoreStatus::Ready);
                }
                StoreMode::WorkspaceMismatch => {
                    let other = tempfile::tempdir().expect("other workspace");
                    std::fs::write(other.path().join("main.go"), "package main\n")
                        .expect("write other source");
                    let other_loaded = load_config(other.path()).expect("other config loads");
                    let other_cache = Cache::default_for_repo(other.path(), true);
                    let other_validated = validated_setup_run(&other_loaded, &other_cache, &plan);
                    let outcome =
                        store::SemanticStore::commit_validated_run(&config, other_validated);
                    assert_eq!(outcome.status, StoreStatus::Ready);
                }
                StoreMode::FirstFailureRecovery => {
                    let outcome = store::SemanticStore::commit_validated_run(
                        &config,
                        setup.as_ref().expect("failure-recovery setup").clone(),
                    );
                    assert_eq!(outcome.status, StoreStatus::Ready);
                    store::inject_next_commit_failure_for_test();
                }
                StoreMode::AuditedFailed | StoreMode::Pending => {
                    let fixture_state = if matches!(mode, StoreMode::Pending) {
                        store::StoredGenerationFixtureState::Pending
                    } else {
                        store::StoredGenerationFixtureState::AuditedFailed
                    };
                    store::install_generation_fixture_for_test(
                        &config,
                        setup.as_ref().expect("generation-state setup"),
                        fixture_state,
                    )
                    .expect("install generation state");
                }
                StoreMode::Corrupt => {
                    std::fs::create_dir_all(path.parent().expect("store directory"))
                        .expect("create store directory");
                    let bytes = b"parity corrupt sqlite bytes".to_vec();
                    std::fs::write(&path, &bytes).expect("write corrupt fixture");
                    corrupt_before = Some(bytes);
                }
                StoreMode::Future => {
                    std::fs::create_dir_all(path.parent().expect("store directory"))
                        .expect("create store directory");
                    store::install_future_fixture_for_test(&path).expect("install future fixture");
                    fixture_before = Some(
                        store::fixture_snapshot_for_test(&path).expect("snapshot future fixture"),
                    );
                }
                StoreMode::Invalid => {
                    std::fs::create_dir_all(path.parent().expect("store directory"))
                        .expect("create store directory");
                    store::install_invalid_fixture_for_test(&path)
                        .expect("install invalid fixture");
                    fixture_before = Some(
                        store::fixture_snapshot_for_test(&path).expect("snapshot invalid fixture"),
                    );
                }
                StoreMode::Busy => {
                    assert_eq!(
                        store::initialize_empty_fixture_for_test(&config),
                        StoreStatus::Ready
                    );
                    fixture_before = Some(
                        store::fixture_snapshot_for_test(&path).expect("snapshot current fixture"),
                    );
                    held_writer = Some(
                        store::hold_writer_connection_for_test(&path).expect("hold writer lease"),
                    );
                }
                #[cfg(unix)]
                StoreMode::Unsafe => {
                    std::fs::create_dir_all(path.parent().expect("store directory"))
                        .expect("create store directory");
                    let outside = temp.path().join("outside.sqlite3");
                    let bytes = b"unsafe parity target".to_vec();
                    std::fs::write(&outside, &bytes).expect("write outside target");
                    std::os::unix::fs::symlink(&outside, &path).expect("symlink store path");
                    unsafe_outside = Some((outside, bytes));
                }
            }

            let mut output = AnalysisKernel::run(KernelInput {
                loaded: &loaded,
                cache: &cache,
                config_digest: "config",
                rule_digest: "rules",
                plan: &plan,
                parallel: false,
            })
            .expect("kernel run");
            let status = output.run_report.store_status().clone();
            sort_diagnostics(&mut output.diagnostics);
            let exit_code = u8::from(
                output
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity.is_at_least(Severity::Warn)),
            );
            let json = render(
                OutputFormat::Json,
                &output.diagnostics,
                RenderOpts {
                    json: JsonReportMeta {
                        tool_name: "polint",
                        tool_version: env!("CARGO_PKG_VERSION"),
                    },
                    color: ColorChoice::Never,
                    sources: None,
                },
            );

            if let Some(bytes) = corrupt_before {
                assert_eq!(std::fs::read(&path).expect("read corrupt fixture"), bytes);
            }
            if let Some(before) = fixture_before {
                assert_eq!(
                    store::fixture_snapshot_for_test(&path).expect("snapshot fixture after run"),
                    before
                );
            }
            #[cfg(unix)]
            if let Some((outside, bytes)) = unsafe_outside {
                assert_eq!(std::fs::read(outside).expect("read outside target"), bytes);
            }
            drop(held_writer);
            (json, exit_code, status)
        }

        #[test]
        fn all_store_modes_preserve_byte_identical_json_and_exit_semantics() {
            let (disabled_json, disabled_exit, disabled_status) = run_mode(StoreMode::Disabled);
            assert_eq!(disabled_status, StoreStatus::Disabled);

            let mut cases = vec![
                (StoreMode::Enabled, StoreStatus::Ready),
                (StoreMode::Complete, StoreStatus::Ready),
                (
                    StoreMode::WorkspaceMismatch,
                    StoreStatus::Skipped(store::StoreSkipReason::WorkspaceMismatch),
                ),
                (
                    StoreMode::FirstFailureRecovery,
                    StoreStatus::Skipped(store::StoreSkipReason::CommitFailed),
                ),
                (StoreMode::AuditedFailed, StoreStatus::Ready),
                (StoreMode::Pending, StoreStatus::Ready),
                (
                    StoreMode::Corrupt,
                    StoreStatus::RebuildNeeded(store::StoreRebuildReason::Corrupt),
                ),
                (
                    StoreMode::Future,
                    StoreStatus::Skipped(store::StoreSkipReason::FutureSchema {
                        found: store::CURRENT_SCHEMA_VERSION_FOR_TEST + 1,
                        supported: store::CURRENT_SCHEMA_VERSION_FOR_TEST,
                    }),
                ),
                (
                    StoreMode::Invalid,
                    StoreStatus::RebuildNeeded(store::StoreRebuildReason::InvalidSchema),
                ),
                (StoreMode::Busy, StoreStatus::BusySkipped),
            ];
            #[cfg(unix)]
            cases.push((
                StoreMode::Unsafe,
                StoreStatus::Skipped(store::StoreSkipReason::UnsafePath),
            ));

            for (mode, expected_status) in cases {
                let (json, exit_code, status) = run_mode(mode);
                assert_eq!(status, expected_status);
                assert_eq!(json.as_bytes(), disabled_json.as_bytes());
                assert_eq!(exit_code, disabled_exit);
            }
        }
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
                "polint.go.semantic",
                "polint.identity",
                "polint.abstract_domains",
                "polint.direct_summaries",
                "polint.entrypoints",
                "polint.reachability",
                "polint.extensions",
                "polint.type_value_alias",
                "polint.semantic_graph",
                "polint.solver",
                "polint.refined_calls",
                "polint.data_flow",
                "polint.evidence",
                "polint.metrics",
            ]
        );
        for provider_id in ["polint.go.syntax", "polint.ts.syntax"] {
            let provider = provider_outputs
                .iter()
                .find(|row| row.provider_id == provider_id)
                .expect("syntax provider output");
            assert_eq!(provider.layers.len(), 1);
            assert_eq!(provider.output_digest, provider.layers[0].output_digest);
        }
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
        let generic = incremental::provider_output_digest_from_manifest(
            manifest,
            &provider_output_summary_parts(&output.db, manifest)
                .expect("final metadata has canonical stable rows"),
        );
        let direct_summaries = provider_output(&output, "polint.direct_summaries");

        assert_ne!(
            direct_summaries.output_digest, generic,
            "post-SCC direct summaries must use the direct-summaries digest, including provider parameters and dependency digests"
        );
        assert_eq!(
            direct_summaries.output_digest.kind,
            incremental::DigestKind::ProviderOutput
        );
    }

    #[test]
    fn failed_derived_provider_output_is_not_reported_as_trusted_fallback_digest() {
        let candidate = incremental::Digest::from_parts(
            incremental::DigestKind::ProviderOutput,
            "candidate",
            &["must-not-be-trusted"],
        );
        let row = AnalysisKernel::provider_output_for_outcome(
            "polint.data_flow",
            incremental::CacheStats::default(),
            incremental::ProviderExecutionOutcome::Failed,
            Some(candidate.clone()),
        );

        assert_eq!(
            row.validation,
            incremental::ProviderValidationStatus::ProviderFailed
        );
        assert_ne!(row.output_digest, candidate);
        assert_eq!(
            row.output_digest.kind,
            incremental::DigestKind::ProviderOutput
        );
        assert!(row.layers.is_empty());
    }

    #[test]
    fn skipped_provider_output_is_distinct_from_failure_and_discards_candidate_state() {
        let candidate = incremental::Digest::from_parts(
            incremental::DigestKind::ProviderOutput,
            "candidate",
            &["must-not-be-reported"],
        );
        let row = AnalysisKernel::provider_output_for_outcome(
            "polint.ts.syntax",
            incremental::CacheStats::default(),
            incremental::ProviderExecutionOutcome::Skipped,
            Some(candidate),
        );

        assert_eq!(
            row.validation,
            incremental::ProviderValidationStatus::Skipped
        );
        assert_eq!(
            row.output_digest,
            incremental::Digest::absent(
                incremental::DigestKind::ProviderOutput,
                "polint.ts.syntax"
            )
        );
        assert!(row.layers.is_empty());
    }

    #[test]
    fn successful_provider_without_an_output_digest_fails_closed() {
        let row = AnalysisKernel::provider_output_for_outcome(
            "polint.data_flow",
            incremental::CacheStats::default(),
            incremental::ProviderExecutionOutcome::Succeeded,
            None,
        );

        assert_eq!(
            row.validation,
            incremental::ProviderValidationStatus::ProviderFailed
        );
        assert_eq!(
            row.output_digest,
            incremental::Digest::unsupported(
                incremental::DigestKind::ProviderOutput,
                "polint.data_flow",
                "provider execution failed"
            )
        );
    }

    #[test]
    fn failed_dependencies_remain_unsupported_through_provider_chains() {
        fn present(provider_id: &str) -> incremental::ProviderOutputDependency {
            incremental::ProviderOutputDependency::present(incremental::Digest::from_parts(
                incremental::DigestKind::ProviderOutput,
                provider_id,
                &["present"],
            ))
        }

        let failed_go_semantic = incremental::ProviderOutputDependency::from_execution(
            "polint.go.semantic",
            incremental::ProviderExecutionOutcome::Failed,
            None,
        );
        let calls = present("polint.calls");
        assert!(!AnalysisKernel::provider_dependencies_ready(
            &[&calls],
            &[&failed_go_semantic],
        ));
        let identity = dependency_blocked_output!(
            crate::analysis::identity::provider::IdentityProviderRunOutput
        );
        let identity_dependency = incremental::ProviderOutputDependency::from_execution(
            "polint.identity",
            identity.execution,
            identity.output_digest,
        );
        assert_eq!(
            identity_dependency.status,
            incremental::InputComponentStatus::Unsupported
        );

        let semantic_graph = present("polint.semantic_graph");
        let type_value_alias = present("polint.type_value_alias");
        let reachability = present("polint.reachability");
        assert!(!AnalysisKernel::provider_dependencies_ready(
            &[&semantic_graph, &type_value_alias, &reachability],
            &[&failed_go_semantic],
        ));
        let solver =
            dependency_blocked_output!(crate::analysis::solver::provider::SolverProviderRunOutput);
        let solver_dependency = incremental::ProviderOutputDependency::from_execution(
            "polint.solver",
            solver.execution,
            solver.output_digest,
        );
        assert_eq!(
            solver_dependency.status,
            incremental::InputComponentStatus::Unsupported
        );
        let skipped_go_semantic = incremental::ProviderOutputDependency::from_execution(
            "polint.go.semantic",
            incremental::ProviderExecutionOutcome::Skipped,
            None,
        );
        assert!(AnalysisKernel::provider_dependencies_ready(
            &[&calls],
            &[&skipped_go_semantic],
        ));
        assert!(AnalysisKernel::provider_dependencies_ready(
            &[&semantic_graph, &type_value_alias, &reachability],
            &[&skipped_go_semantic],
        ));

        let failed_mir = incremental::ProviderOutputDependency::from_execution(
            "polint.semantic_mir",
            incremental::ProviderExecutionOutcome::Failed,
            None,
        );
        assert!(!AnalysisKernel::provider_dependencies_ready(
            &[&failed_mir],
            &[],
        ));
        let cfg = dependency_blocked_output!(crate::analysis::cfg::provider::CfgProviderOutput);
        let cfg_dependency = incremental::ProviderOutputDependency::from_execution(
            "polint.cfg",
            cfg.execution,
            cfg.output_digest,
        );
        assert_eq!(
            cfg_dependency.status,
            incremental::InputComponentStatus::Unsupported
        );
        let skipped_mir = incremental::ProviderOutputDependency::from_execution(
            "polint.semantic_mir",
            incremental::ProviderExecutionOutcome::Skipped,
            None,
        );
        assert_eq!(
            skipped_mir.status,
            incremental::InputComponentStatus::Absent
        );
        assert!(!AnalysisKernel::provider_dependencies_ready(
            &[&skipped_mir],
            &[],
        ));

        let failed_module_graph = incremental::ProviderOutputDependency::from_execution(
            "polint.module_graph",
            incremental::ProviderExecutionOutcome::Failed,
            None,
        );
        assert!(!failed_module_graph.is_available_or_absent());
        let symbol_graph = dependency_blocked_output!(crate::symbol_graph::SymbolGraphDerivation);
        let symbol_dependency = incremental::ProviderOutputDependency::from_execution(
            "polint.symbol_graph",
            symbol_graph.execution,
            symbol_graph.output_digest,
        );
        assert_eq!(
            symbol_dependency.status,
            incremental::InputComponentStatus::Unsupported
        );
        assert!(!AnalysisKernel::provider_dependencies_ready(
            &[&failed_module_graph, &symbol_dependency],
            &[],
        ));
        let topology = dependency_blocked_output!(crate::module_graph::ModuleTopologyDerivation);
        let topology_dependency = incremental::ProviderOutputDependency::from_execution(
            "polint.module_topology",
            topology.execution,
            topology.output_digest,
        );
        assert_eq!(
            topology_dependency.status,
            incremental::InputComponentStatus::Unsupported
        );

        let skipped_module_graph = incremental::ProviderOutputDependency::from_execution(
            "polint.module_graph",
            incremental::ProviderExecutionOutcome::Skipped,
            None,
        );
        assert!(skipped_module_graph.is_available_or_absent());
        let skipped_symbol_graph = incremental::ProviderOutputDependency::from_execution(
            "polint.symbol_graph",
            incremental::ProviderExecutionOutcome::Skipped,
            None,
        );
        assert!(!AnalysisKernel::provider_dependencies_ready(
            &[&skipped_module_graph, &skipped_symbol_graph],
            &[],
        ));
        let plan_skipped_topology = incremental::ProviderOutputDependency::from_execution(
            "polint.module_topology",
            incremental::ProviderExecutionOutcome::Skipped,
            None,
        );
        assert_eq!(
            plan_skipped_topology.status,
            incremental::InputComponentStatus::Absent
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
        assert_eq!(first_module_graph.layers.len(), 1);
        assert_eq!(second_module_graph.layers, first_module_graph.layers);
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
        assert_eq!(first_symbol_graph.layers.len(), 1);
        assert_eq!(second_symbol_graph.layers, first_symbol_graph.layers);
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
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls"]);

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
        assert_eq!(first_module_topology.layers.len(), 1);
        assert_eq!(second_module_topology.layers, first_module_topology.layers);
        assert_eq!(
            disabled_module_topology.layers,
            first_module_topology.layers
        );
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
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls"]);

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
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls"]);

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
        let plan = AnalysisPlan::from_capability_names_for_test(&["control_flow"]);

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
        assert_eq!(first_metrics.layers.len(), 1);
        assert_eq!(second_metrics.layers, first_metrics.layers);
    }

    #[test]
    fn kernel_preserves_provider_and_run_identities_when_layer_cache_write_fails() {
        fn validated_metadata(output: &KernelOutput) -> incremental::ValidatedRunMetadata {
            let fact_rows = output
                .db
                .fact_meta()
                .stable_rows()
                .expect("fact metadata is canonical");
            incremental::ValidatedRunMetadata::from_finalized_run(
                &output.run_report.input_snapshot,
                &output.run_report.provider_outputs,
                output.run_report.demand_query_trace(),
                output.run_report.validation_events(),
                AnalysisKernel::provider_manifests(),
                fact_rows,
            )
            .expect("kernel output is a complete metadata handoff")
        }

        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("app.ts"),
            "export function handler() { return 1; }\n",
        )
        .expect("write source");
        let loaded = load_config(temp.path()).expect("default config loads");
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls", "file_metrics"]);
        let disabled_cache = Cache::new(temp.path().join("disabled-cache"), false);
        let baseline = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &disabled_cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("disabled-cache kernel should run");

        let cache_root = temp.path().join("cache");
        std::fs::create_dir_all(&cache_root).expect("cache root");
        std::fs::write(cache_root.join("layers"), "not a directory").expect("layer root file");
        let cache = Cache::new(cache_root.join("analysis"), true);

        let failed_write = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");
        let metrics = provider_output(&failed_write, "polint.metrics");

        assert_eq!(metrics.cache_stats.misses, 0);
        assert_eq!(metrics.cache_stats.invalid_evicted_reads, 1);
        assert_eq!(metrics.cache_stats.recomputes, 1);
        assert_eq!(metrics.cache_stats.writes, 0);
        assert_eq!(metrics.layers.len(), 1);
        assert_eq!(
            metrics.validation,
            incremental::ProviderValidationStatus::NativeTrusted
        );
        assert!(failed_write.diagnostics.iter().any(|diagnostic| {
            diagnostic.rule_id == "internal/cache"
                && diagnostic.file == "metrics layer"
                && diagnostic.message.contains("cache write failed")
        }));

        for provider_id in [
            "polint.go.syntax",
            "polint.ts.syntax",
            "polint.module_graph",
            "polint.symbol_graph",
            "polint.module_topology",
            "polint.metrics",
        ] {
            let expected = provider_output(&baseline, provider_id);
            let actual = provider_output(&failed_write, provider_id);
            assert_eq!(actual.validation, expected.validation, "{provider_id}");
            assert_eq!(
                actual.output_digest, expected.output_digest,
                "{provider_id}"
            );
            assert_eq!(actual.layers, expected.layers, "{provider_id}");
        }

        let baseline_metadata = validated_metadata(&baseline);
        let failed_write_metadata = validated_metadata(&failed_write);
        assert_eq!(
            failed_write_metadata.identities().provider_output(),
            baseline_metadata.identities().provider_output()
        );
        assert_eq!(
            failed_write_metadata.identities().run(),
            baseline_metadata.identities().run()
        );
        assert_eq!(
            failed_write_metadata.identities().generation(),
            baseline_metadata.identities().generation()
        );
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

        // With an empty plan no rule requests a graph capability, so the
        // interprocedural/semantic pipeline (semantic_mir and everything
        // downstream) is gated off and never recomputes. Its provider row is still
        // emitted with a deterministic empty-summary output digest. See
        // `SEMANTIC_PIPELINE_TRIGGER_CAPABILITIES`.
        let semantic_mir = provider_output(&output, "polint.semantic_mir");
        assert_eq!(semantic_mir.cache_stats.recomputes, 0);
        assert!(!semantic_mir.output_digest.value.is_empty());
    }

    #[test]
    fn events_only_plan_stays_off_semantic_pipeline() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("app.ts"),
            "export function app() { fetch('/health'); }\n",
        )
        .expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&["events"]);

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");

        assert_eq!(
            provider_output(&output, "polint.semantic_mir")
                .cache_stats
                .recomputes,
            0
        );
        assert_eq!(
            provider_output(&output, "polint.calls")
                .cache_stats
                .recomputes,
            0
        );
        assert_eq!(
            provider_output(&output, "polint.refined_calls")
                .cache_stats
                .recomputes,
            0
        );
        assert_eq!(
            provider_output(&output, "polint.data_flow")
                .cache_stats
                .recomputes,
            0
        );
        assert_eq!(
            provider_output(&output, "polint.evidence")
                .cache_stats
                .recomputes,
            0
        );
    }

    #[test]
    fn symbols_and_references_stay_off_semantic_pipeline() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("app.ts"),
            "export function app() { return 42; }\nexport const value = app();\n",
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

        assert_eq!(
            provider_output(&output, "polint.symbol_graph")
                .cache_stats
                .recomputes,
            1
        );
        for provider_id in [
            "polint.module_topology",
            "polint.semantic_mir",
            "polint.cfg",
            "polint.calls",
            "polint.refined_calls",
            "polint.data_flow",
            "polint.evidence",
        ] {
            assert_eq!(
                provider_output(&output, provider_id).cache_stats.recomputes,
                0,
                "{provider_id} should stay off for symbols/references-only plans"
            );
        }
    }

    #[test]
    fn control_flow_plan_keeps_cfg_and_refined_call_facts() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("app.ts"),
            "function authorize() {}\nfunction dangerous() {}\nexport function app(input: boolean) { if (input) { authorize(); } dangerous(); }\n",
        )
        .expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&["control_flow"]);

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
            "polint.semantic_mir",
            "polint.cfg",
            "polint.calls",
            "polint.go.semantic",
            "polint.identity",
            "polint.abstract_domains",
            "polint.direct_summaries",
            "polint.entrypoints",
            "polint.reachability",
            "polint.extensions",
            "polint.type_value_alias",
            "polint.semantic_graph",
            "polint.solver",
            "polint.refined_calls",
        ] {
            assert_eq!(
                provider_output(&output, provider_id).cache_stats.recomputes,
                1,
                "{provider_id} should run for control-flow plans"
            );
        }
        assert!(
            !output.db.call_sites().is_empty(),
            "control-flow plans should derive call-site facts"
        );
        assert!(
            !output.db.refined_call_edges().is_empty(),
            "control-flow plans should keep refined call edges for semantic target matching"
        );
        assert!(
            !output.db.cfg_nodes().is_empty(),
            "control-flow plans should derive CFG nodes for control ordering"
        );
        assert!(
            !output.db.cfg_reachability().is_empty(),
            "control-flow plans should derive CFG reachability rows"
        );
        assert!(
            !output.db.cfg_dominators().is_empty(),
            "control-flow plans should derive CFG dominator rows"
        );
        assert!(
            !output.db.cfg_postdominators().is_empty(),
            "control-flow plans should derive CFG postdominator rows"
        );
        for provider_id in ["polint.data_flow", "polint.evidence"] {
            assert_eq!(
                provider_output(&output, provider_id).cache_stats.recomputes,
                0,
                "{provider_id} should stay off unless a dataflow rule asks for it"
            );
        }
    }

    #[test]
    fn calls_plan_keeps_full_cfg_relation_rows() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("app.ts"),
            "export function app(input: string) { if (input) { dangerous(); } cleanup(); }\n",
        )
        .expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls"]);

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");

        assert_eq!(
            provider_output(&output, "polint.cfg")
                .cache_stats
                .recomputes,
            1
        );
        assert!(
            !output.db.cfg_reachability().is_empty(),
            "calls plans should still derive full CFG relation rows"
        );
        assert!(
            !output.db.cfg_postdominators().is_empty(),
            "calls plans should keep postdominator rows for downstream refinements"
        );
    }

    #[test]
    #[ignore = "synthetic performance smoke; run manually with POLINT_SYNTHETIC_FILES=N"]
    fn control_flow_synthetic_refined_pipeline_benchmark() {
        let file_count = std::env::var("POLINT_SYNTHETIC_FILES")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(100);
        if file_count > 500 && std::env::var_os("POLINT_SYNTHETIC_ALLOW_LARGE").is_none() {
            panic!(
                "refusing to generate {file_count} files without POLINT_SYNTHETIC_ALLOW_LARGE=1"
            );
        }

        let temp = tempfile::tempdir().expect("tempdir");
        let src = temp.path().join("src");
        std::fs::create_dir_all(&src).expect("create src");
        for index in 0..file_count {
            std::fs::write(
                src.join(format!("route_{index:05}.ts")),
                format!(
                    "\
export function route_{index}(input: string) {{
  authorize(input);
  fetch(`/api/${{input}}`);
  cleanup(input);
}}

function authorize(value: string) {{ return value.length > 0; }}
function cleanup(value: string) {{ return value.trim(); }}
"
                ),
            )
            .expect("write synthetic file");
        }

        let loaded = load_config(temp.path()).expect("default config loads");
        let cache_enabled = std::env::var_os("POLINT_SYNTHETIC_CACHE").is_some();
        let cache_root = std::env::var_os("POLINT_SYNTHETIC_CACHE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| temp.path().join(".polint-cache"));
        let cache = Cache::new(cache_root, cache_enabled);
        let plan = AnalysisPlan::from_capability_names_for_test(&["control_flow"]);

        let started = Instant::now();
        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: true,
        })
        .expect("kernel should run");
        let elapsed = started.elapsed();

        eprintln!(
            "control_flow refined synthetic: files={} functions={} mir_ops={} cfg_nodes={} call_sites={} refined_edges={} domain_obs={} summary_facts={} semantic_nodes={} semantic_edges={} semantic_constraints={} type_facts={} points_to_constraints={} points_to_sets={} solver_edges={} metadata_rows={} elapsed_ms={:.3}",
            output.db.files().len(),
            output.db.functions().len(),
            output.db.mir_operations().len(),
            output.db.cfg_nodes().len(),
            output.db.call_sites().len(),
            output.db.refined_call_edges().len(),
            output.db.abstract_domain_observations().len(),
            output.db.summary_facts().len(),
            output.db.semantic_nodes().len(),
            output.db.semantic_edges().len(),
            output.db.semantic_constraints().len(),
            output.db.type_facts().len(),
            output.db.points_to_constraints().len(),
            output.db.points_to_sets().len(),
            output.db.solver_derived_edges().len(),
            output.db.fact_meta().rows().count(),
            elapsed.as_secs_f64() * 1000.0
        );
        for provider_id in [
            "polint.module_graph",
            "polint.symbol_graph",
            "polint.semantic_mir",
            "polint.cfg",
            "polint.calls",
            "polint.go.semantic",
            "polint.identity",
            "polint.abstract_domains",
            "polint.direct_summaries",
            "polint.entrypoints",
            "polint.reachability",
            "polint.extensions",
            "polint.type_value_alias",
            "polint.semantic_graph",
            "polint.solver",
            "polint.refined_calls",
        ] {
            let stats = &provider_output(&output, provider_id).cache_stats;
            if cache_enabled {
                assert!(
                    stats.recomputes == 1 || stats.hits + stats.verified_reuse > 0,
                    "{provider_id} should recompute or reuse cache for synthetic control-flow benchmark"
                );
            } else {
                assert_eq!(
                    stats.recomputes, 1,
                    "{provider_id} should run for synthetic control-flow benchmark"
                );
            }
        }
        for provider_id in ["polint.data_flow", "polint.evidence"] {
            assert_eq!(
                provider_output(&output, provider_id).cache_stats.recomputes,
                0,
                "{provider_id} should stay off for synthetic control-flow benchmark"
            );
        }
    }

    #[test]
    fn calls_plan_skips_data_flow_and_evidence() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("app.ts"),
            "export function root() { target(); }\nexport function target() {}\n",
        )
        .expect("write ts");
        let loaded = load_config(temp.path()).expect("default config loads");
        let cache = Cache::new("", false);
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls"]);

        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "config",
            rule_digest: "rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel should run");

        assert_eq!(
            provider_output(&output, "polint.refined_calls")
                .cache_stats
                .recomputes,
            1
        );
        assert_eq!(
            provider_output(&output, "polint.data_flow")
                .cache_stats
                .recomputes,
            0
        );
        assert_eq!(
            provider_output(&output, "polint.evidence")
                .cache_stats
                .recomputes,
            0
        );
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
    fn data_flow_public_no_leak() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("workspace root");
        let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let markers = data_flow_internal_markers();

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
        assert_no_data_flow_markers("polint check --format json", &rendered, &markers);

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
            assert_no_data_flow_markers(&source_path.display().to_string(), &source, &markers);
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
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls"]);

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
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls"]);

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

    fn data_flow_internal_markers() -> [&'static str; 7] {
        [
            "polint.data_flow",
            "DataFlowNodeFact",
            "DataFlowEdgeFact",
            "DataFlowModelFact",
            "DataFlowBudgetFact",
            "summary_projected",
            "query path search",
        ]
    }

    fn assert_no_refined_call_markers(label: &str, source: &str, markers: &[&str]) {
        for marker in markers {
            assert!(
                !source.contains(marker),
                "{label} leaked refined-call internal marker `{marker}`"
            );
        }
    }

    fn assert_no_framework_markers(label: &str, source: &str, markers: &[&str]) {
        for marker in markers {
            assert!(
                !source.contains(marker),
                "{label} leaked framework internal marker `{marker}`"
            );
        }
    }

    fn assert_no_data_flow_markers(label: &str, source: &str, markers: &[&str]) {
        for marker in markers {
            assert!(
                !source.contains(marker),
                "{label} leaked data-flow internal marker `{marker}`"
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
                "polint.go.semantic",
                "polint.identity",
                "polint.abstract_domains",
                "polint.direct_summaries",
                "polint.entrypoints",
                "polint.reachability",
                "polint.extensions",
                "polint.type_value_alias",
                "polint.semantic_graph",
                "polint.solver",
                "polint.refined_calls",
                "polint.data_flow",
                "polint.evidence",
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

    #[derive(Clone, Copy, Debug)]
    enum RuleOnlyMutation {
        Severity,
        Files,
        AllowFiles,
        Allow,
        Deny,
        Max,
        ForbiddenImports,
        Description,
        Settings,
    }

    impl RuleOnlyMutation {
        const ALL: [Self; 9] = [
            Self::Severity,
            Self::Files,
            Self::AllowFiles,
            Self::Allow,
            Self::Deny,
            Self::Max,
            Self::ForbiddenImports,
            Self::Description,
            Self::Settings,
        ];

        fn apply(self, config: &mut RuleConfig) {
            match self {
                Self::Severity => config.severity = Some("error".to_string()),
                Self::Files => config.files = vec!["src/**/*.ts".to_string()],
                Self::AllowFiles => config.allow_files = vec!["generated/**".to_string()],
                Self::Allow => config.allow = vec!["permitted".to_string()],
                Self::Deny => config.deny = vec!["forbidden".to_string()],
                Self::Max => config.max = Some(7),
                Self::ForbiddenImports => {
                    config
                        .forbidden_imports
                        .insert("src/**".to_string(), vec!["internal/**".to_string()]);
                }
                Self::Settings => {
                    config
                        .settings
                        .insert("threshold".to_string(), toml::Value::Integer(7));
                }
                Self::Description => {}
            }
        }

        fn description(self) -> &'static str {
            if matches!(self, Self::Description) {
                "Changed rule description"
            } else {
                "Baseline rule description"
            }
        }
    }

    struct IdentityRun {
        output: KernelOutput,
        config_digest: String,
        rule_digest: String,
        plan_digest: String,
    }

    fn identity_rule(description: &str, capabilities: Capabilities) -> Rule {
        let description = description.to_string();
        Rule::from_parts(
            move || RuleMeta {
                id: "local/identity-probe".to_string(),
                description: description.clone(),
                severity: Severity::Warn,
                kind: RuleKind::Check,
            },
            move || capabilities,
            |_db, _ctx| Ok(()),
        )
    }

    fn identity_rule_config() -> RuleConfig {
        RuleConfig {
            id: "local/identity-probe".to_string(),
            severity: None,
            files: Vec::new(),
            allow_files: Vec::new(),
            allow: Vec::new(),
            max: None,
            deny: Vec::new(),
            forbidden_imports: Default::default(),
            settings: Default::default(),
        }
    }

    fn identity_loaded(root: &Path) -> LoadedConfig {
        let mut loaded = load_config(root).expect("default config loads");
        loaded.config.rules.config.push(identity_rule_config());
        loaded
    }

    fn run_identity_case<F>(
        loaded: &LoadedConfig,
        cache: &Cache,
        description: &str,
        capabilities: Capabilities,
        transform_plan: F,
    ) -> IdentityRun
    where
        F: FnOnce(AnalysisPlan) -> AnalysisPlan,
    {
        let rules = [identity_rule(description, capabilities)];
        let inputs = RulePlanInputs::collect(&rules, None);
        let options = inputs.rule_options_from_config(loaded);
        let rule_digest = inputs.rule_digest(&options);
        let plan = transform_plan(AnalysisPlan::from_inputs(&inputs, &options));
        let plan_digest = plan.digest().to_string();
        let config_digest = config_hash(loaded);
        let output = AnalysisKernel::run(KernelInput {
            loaded,
            cache,
            config_digest: &config_digest,
            rule_digest: &rule_digest,
            plan: &plan,
            parallel: false,
        })
        .expect("identity fixture kernel run should succeed");
        IdentityRun {
            output,
            config_digest,
            rule_digest,
            plan_digest,
        }
    }

    fn provider_digest(
        output: &KernelOutput,
        provider_id: &str,
    ) -> crate::analysis_kernel::incremental::Digest {
        provider_output(output, provider_id).output_digest.clone()
    }

    fn scc_backdated_count(output: &KernelOutput) -> usize {
        output
            .run_report
            .scc_closure_debug()
            .unwrap_or_else(|| panic!("missing SCC closure debug snapshot"))
            .result
            .backdated_sccs
    }

    fn assert_cached_analysis_reused(output: &KernelOutput, case: &str) {
        for provider_id in [
            "polint.ts.syntax",
            "polint.module_graph",
            "polint.symbol_graph",
        ] {
            let stats = &provider_output(output, provider_id).cache_stats;
            assert!(
                stats.hits > 0 && stats.verified_reuse > 0,
                "{case}: expected verified reuse for {provider_id}, got {stats:?}"
            );
            assert_eq!(
                stats.recomputes, 0,
                "{case}: cached provider {provider_id} recomputed"
            );
        }
    }

    fn assert_same_provider_digests(
        baseline: &KernelOutput,
        candidate: &KernelOutput,
        provider_ids: &[&str],
        case: &str,
    ) {
        for provider_id in provider_ids {
            assert_eq!(
                provider_digest(baseline, provider_id),
                provider_digest(candidate, provider_id),
                "{case}: {provider_id} output identity changed"
            );
        }
    }

    fn write_identity_source(root: &Path) {
        std::fs::create_dir_all(root.join("src")).expect("create source directory");
        std::fs::write(root.join("package.json"), r#"{"name":"identity-fixture"}"#)
            .expect("write package manifest");
        std::fs::write(
            root.join("src/app.ts"),
            "export function target(value: number) { return value + 1; }\n\
             export function caller(value: number) { return target(value); }\n",
        )
        .expect("write TypeScript source");
    }

    #[test]
    fn rule_only_changes_preserve_analysis_hits() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_identity_source(temp.path());
        let baseline_loaded = identity_loaded(temp.path());
        let cache = Cache::new(temp.path().join(".cache/analysis"), true);
        let baseline = run_identity_case(
            &baseline_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls(),
            |plan| plan,
        );
        assert_eq!(scc_backdated_count(&baseline.output), 0);

        for mutation in RuleOnlyMutation::ALL {
            let mut loaded = baseline_loaded.clone();
            mutation.apply(
                loaded
                    .config
                    .rules
                    .config
                    .first_mut()
                    .expect("identity rule config"),
            );
            let candidate = run_identity_case(
                &loaded,
                &cache,
                mutation.description(),
                Capabilities::new().calls(),
                |plan| plan,
            );
            let case = format!("{mutation:?}");

            assert_ne!(baseline.rule_digest, candidate.rule_digest, "{case}");
            assert_ne!(baseline.plan_digest, candidate.plan_digest, "{case}");
            assert_ne!(
                baseline.output.run_report.input_snapshot.semantic_digest(),
                candidate.output.run_report.input_snapshot.semantic_digest(),
                "{case}: complete input identity did not change"
            );
            assert_ne!(
                baseline.output.run_report.input_snapshot.rules,
                candidate.output.run_report.input_snapshot.rules,
                "{case}: rule snapshot rows did not change"
            );
            if !matches!(mutation, RuleOnlyMutation::Description) {
                assert_ne!(baseline.config_digest, candidate.config_digest, "{case}");
                assert_ne!(
                    baseline.output.run_report.input_snapshot.config_identity,
                    candidate.output.run_report.input_snapshot.config_identity,
                    "{case}: complete config identity did not change"
                );
            }

            assert_cached_analysis_reused(&candidate.output, &case);
            assert!(
                scc_backdated_count(&candidate.output) > 0,
                "{case}: SCC closure did not reuse its prior output"
            );
            assert_same_provider_digests(
                &baseline.output,
                &candidate.output,
                &[
                    "polint.calls",
                    "polint.direct_summaries",
                    "polint.entrypoints",
                    "polint.reachability",
                    "polint.extensions",
                    "polint.type_value_alias",
                    "polint.semantic_graph",
                    "polint.solver",
                    "polint.refined_calls",
                ],
                &case,
            );
        }
    }

    fn write_adaptation_model(root: &Path) {
        let model_dir = root.join(".polint/models");
        std::fs::create_dir_all(&model_dir).expect("create model directory");
        std::fs::write(
            model_dir.join("identity.toml"),
            r#"
[[facts]]
source_pattern = "call:src/app.ts:2:target"
target_pattern = "function:src/app.ts:1:target"
confidence = "heuristic"
language = "typescript"
scope = "src/app.ts"
evidence = ["src/app.ts:2"]
"#,
        )
        .expect("write adaptation model");
    }

    fn write_extension_fixture(root: &Path) -> PathBuf {
        let extension = root.join(".polint/extensions/identity-probe");
        std::fs::create_dir_all(extension.join("src")).expect("create extension source directory");
        std::fs::write(
            extension.join("Cargo.toml"),
            "[package]\nname = \"identity-probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("write extension manifest");
        std::fs::write(
            extension.join("Cargo.lock"),
            "version = 4\n\n[[package]]\nname = \"identity-probe\"\nversion = \"0.1.0\"\n",
        )
        .expect("write extension lockfile");
        std::fs::write(extension.join("declared.txt"), "input-v1\n")
            .expect("write extension-declared input");
        std::fs::write(
            extension.join("src/main.rs"),
            r###"
use std::{env, fs};

fn main() {
    match env::args().nth(1).as_deref() {
        Some("handshake") => println!(
            "{}",
            r#"{"schema_version":"polint-extension-handshake-v1","extension_id":"identity-probe","activation_status":"handshake_ok","providers":[{"provider_id":"probe","declared_inputs":[],"declared_outputs":[]}],"diagnostics":[]}"#
        ),
        Some("run-provider") => {
            let input = fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/declared.txt"))
                .expect("declared input");
            println!(
                r#"{{"schema_version":"polint-extension-provider-run-v1","extension_id":"identity-probe","provider_id":"probe","activation_status":"active","diagnostics":[],"facts":[],"output_digest_inputs":["declared={}"]}}"#,
                input.trim()
            );
        }
        _ => {}
    }
}
"###,
        )
        .expect("write extension source");
        extension
    }

    fn assert_production_analysis_keys_are_scoped() {
        let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let searched = [
            source_root.join("analysis"),
            source_root.join("module_graph"),
            source_root.join("symbol_graph"),
            source_root.join("metrics.rs"),
            source_root.join("analysis_kernel/incremental/keys.rs"),
        ];
        let mut files = Vec::new();
        for path in searched {
            collect_rust_source_paths(&path, &mut files);
        }
        files.sort();
        files.dedup();

        for path in files {
            let source = std::fs::read_to_string(&path).expect("read production-key source");
            assert!(
                !source.contains("input_snapshot.config.digest"),
                "{} consumes the complete config identity",
                path.display()
            );
            assert!(
                !source.contains("plan.digest()"),
                "{} consumes the complete plan identity",
                path.display()
            );
            assert!(
                !source.contains("input_snapshot.models"),
                "{} consumes every model snapshot row",
                path.display()
            );
            assert!(
                !source.contains("input_snapshot.tool_invocations"),
                "{} consumes every tool snapshot row",
                path.display()
            );
            if !path.ends_with("analysis/extensions/provider.rs") {
                assert!(
                    !source.contains("input_snapshot.extensions"),
                    "{} consumes every extension snapshot row",
                    path.display()
                );
            }
            for line in source.lines().filter(|line| line.contains("rule_digest")) {
                assert!(
                    line.contains("_ignores_rule_digest_changes") || line.contains("rule_digest:"),
                    "{} consumes the complete rule identity: {line}",
                    path.display()
                );
            }
        }

        let keys = std::fs::read_to_string(source_root.join("analysis_kernel/incremental/keys.rs"))
            .expect("read layer keys");
        let direct_summaries = source_section(
            &keys,
            "fn direct_summaries_layer_key(",
            "fn combine_digests_into",
        );
        assert!(direct_summaries.contains("analysis_settings_digest: Digest"));
        assert!(direct_summaries.contains("analysis_requirements_digest: Digest"));
        assert!(direct_summaries.contains("Self::new_with_analysis_settings("));
        assert!(!direct_summaries.contains("config_digest"));

        let cache_keys = std::fs::read_to_string(source_root.join("cache/keys.rs"))
            .expect("read cache key builders");
        let scoped_settings =
            source_section(&cache_keys, "fn analysis_settings_hash(", "fn rule_hash(");
        assert!(!scoped_settings.contains("config_hash("));
        assert!(!scoped_settings.contains("deterministic_polint_config"));
        assert!(!scoped_settings.contains("loaded.config.rules"));

        let summaries = std::fs::read_to_string(source_root.join("analysis/summaries/provider.rs"))
            .expect("read summary provider");
        let scc_key = source_section(
            &summaries,
            "fn scc_closure_analysis_identity(",
            "fn scc_closure_cache_key(",
        );
        assert!(scc_key.contains("DigestKind::AnalysisSettings"));
        assert!(scc_key.contains("analysis_requirements_digest_for"));
        assert!(scc_key.contains("direct_summaries_output_digest"));
        assert!(scc_key.contains("calls_output_digest"));
        assert!(!scc_key.contains("config_digest"));
        assert!(!scc_key.contains("rule_digest"));
        assert!(!scc_key.contains("plan_digest"));

        let plan = std::fs::read_to_string(source_root.join("analysis_plan.rs"))
            .expect("read analysis plan");
        let capability_identity = source_section(
            &plan,
            "fn capability_analysis_dependency_digest(",
            "fn capability_rule_behavior_digest(",
        );
        assert!(!capability_identity.contains("requesting_rule_ids"));
        assert!(!capability_identity.contains("rule_behavior_digest"));
        assert!(!capability_identity.contains("options_digest"));

        let snapshot = std::fs::read_to_string(
            source_root.join("analysis_kernel/incremental/input_snapshot.rs"),
        )
        .expect("read input snapshot");
        let projected_requirements = source_section(
            &snapshot,
            "fn analysis_requirements_digest_for(",
            "fn from_run_inputs_with_plan(",
        );
        assert!(projected_requirements.contains("analysis_dependency_digest"));
        assert!(!projected_requirements.contains("rule_behavior_digest"));
    }

    fn collect_rust_source_paths(path: &Path, output: &mut Vec<PathBuf>) {
        if path.is_file() {
            if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
                output.push(path.to_path_buf());
            }
            return;
        }
        let entries = std::fs::read_dir(path)
            .unwrap_or_else(|error| panic!("read source directory {}: {error}", path.display()));
        for entry in entries {
            let entry = entry.expect("read source entry");
            collect_rust_source_paths(&entry.path(), output);
        }
    }

    fn source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        let start = source
            .find(start)
            .unwrap_or_else(|| panic!("missing source section start {start}"));
        let remaining = &source[start..];
        let end = remaining
            .find(end)
            .unwrap_or_else(|| panic!("missing source section end {end}"));
        &remaining[..end]
    }

    #[test]
    fn declared_analysis_inputs_invalidate_linked_providers() {
        assert_production_analysis_keys_are_scoped();

        let temp = tempfile::tempdir().expect("tempdir");
        write_identity_source(temp.path());
        let baseline_loaded = identity_loaded(temp.path());
        let cache = Cache::new(temp.path().join(".cache/analysis"), true);
        let baseline = run_identity_case(
            &baseline_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls(),
            |plan| plan,
        );
        let warm = run_identity_case(
            &baseline_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls(),
            |plan| plan,
        );
        assert_cached_analysis_reused(&warm.output, "warm baseline");
        assert!(scc_backdated_count(&warm.output) > 0);

        let unrelated = run_identity_case(
            &baseline_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls().events(),
            |plan| plan,
        );
        assert_ne!(
            baseline.output.run_report.input_snapshot.semantic_digest(),
            unrelated.output.run_report.input_snapshot.semantic_digest()
        );
        assert_same_provider_digests(
            &baseline.output,
            &unrelated.output,
            &[
                "polint.calls",
                "polint.direct_summaries",
                "polint.refined_calls",
            ],
            "unreferenced events capability",
        );
        assert!(scc_backdated_count(&unrelated.output) > 0);

        let relevant_capability = run_identity_case(
            &baseline_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls().dataflow(),
            |plan| plan,
        );
        assert_ne!(
            provider_digest(&baseline.output, "polint.calls"),
            provider_digest(&relevant_capability.output, "polint.calls")
        );
        assert_ne!(
            provider_digest(&baseline.output, "polint.direct_summaries"),
            provider_digest(&relevant_capability.output, "polint.direct_summaries")
        );
        assert_eq!(scc_backdated_count(&relevant_capability.output), 0);
        assert!(
            provider_output(&relevant_capability.output, "polint.data_flow")
                .cache_stats
                .recomputes
                > 0
        );

        let support_changed = run_identity_case(
            &baseline_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls(),
            |plan| {
                plan.with_capability_support_for_test("calls", CapabilitySupportStatus::Unsupported)
            },
        );
        assert_ne!(
            provider_digest(&baseline.output, "polint.calls"),
            provider_digest(&support_changed.output, "polint.calls")
        );
        assert_eq!(scc_backdated_count(&support_changed.output), 0);

        let setup_changed = run_identity_case(
            &baseline_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls(),
            |plan| plan.with_setup_status_for_test("calls", "missing"),
        );
        assert_ne!(
            provider_digest(&baseline.output, "polint.calls"),
            provider_digest(&setup_changed.output, "polint.calls")
        );
        assert_eq!(scc_backdated_count(&setup_changed.output), 0);

        let mut reachability_loaded = baseline_loaded.clone();
        reachability_loaded.config.reachability.roots = vec!["src/app.ts#caller".to_string()];
        let reachability_changed = run_identity_case(
            &reachability_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls(),
            |plan| plan,
        );
        assert_ne!(
            provider_digest(&baseline.output, "polint.reachability"),
            provider_digest(&reachability_changed.output, "polint.reachability")
        );
        assert_same_provider_digests(
            &baseline.output,
            &reachability_changed.output,
            &[
                "polint.calls",
                "polint.direct_summaries",
                "polint.semantic_graph",
                "polint.solver",
                "polint.refined_calls",
            ],
            "reachability roots",
        );
        assert!(scc_backdated_count(&reachability_changed.output) > 0);

        let mut budget_loaded = baseline_loaded.clone();
        budget_loaded.config.solver.go.max_rta_rounds = Some(1);
        let budget_changed = run_identity_case(
            &budget_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls(),
            |plan| plan,
        );
        assert_ne!(
            provider_digest(&baseline.output, "polint.solver"),
            provider_digest(&budget_changed.output, "polint.solver")
        );
        assert_ne!(
            provider_digest(&baseline.output, "polint.refined_calls"),
            provider_digest(&budget_changed.output, "polint.refined_calls")
        );
        assert_same_provider_digests(
            &baseline.output,
            &budget_changed.output,
            &["polint.calls", "polint.direct_summaries"],
            "solver budget",
        );
        assert!(scc_backdated_count(&budget_changed.output) > 0);

        write_adaptation_model(temp.path());
        let model_changed = run_identity_case(
            &baseline_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls(),
            |plan| plan,
        );
        assert_ne!(
            provider_digest(&baseline.output, "polint.semantic_graph"),
            provider_digest(&model_changed.output, "polint.semantic_graph")
        );
        assert_ne!(
            provider_digest(&baseline.output, "polint.refined_calls"),
            provider_digest(&model_changed.output, "polint.refined_calls")
        );
        assert_same_provider_digests(
            &baseline.output,
            &model_changed.output,
            &["polint.calls", "polint.direct_summaries"],
            "adaptation model",
        );
        assert!(scc_backdated_count(&model_changed.output) > 0);

        let extension_dir = write_extension_fixture(temp.path());
        let extension_baseline = run_identity_case(
            &baseline_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls(),
            |plan| plan,
        );
        let extension_source = extension_dir.join("src/main.rs");
        let source = std::fs::read_to_string(&extension_source).expect("read extension source");
        std::fs::write(
            &extension_source,
            format!("{source}\n// source identity mutation\n"),
        )
        .expect("mutate extension source");
        let extension_code_changed = run_identity_case(
            &baseline_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls(),
            |plan| plan,
        );
        assert_ne!(
            extension_baseline
                .output
                .run_report
                .input_snapshot
                .extensions,
            extension_code_changed
                .output
                .run_report
                .input_snapshot
                .extensions
        );
        assert_ne!(
            provider_digest(&extension_baseline.output, "polint.extensions"),
            provider_digest(&extension_code_changed.output, "polint.extensions")
        );
        assert_ne!(
            provider_digest(&extension_baseline.output, "polint.refined_calls"),
            provider_digest(&extension_code_changed.output, "polint.refined_calls")
        );
        assert_same_provider_digests(
            &extension_baseline.output,
            &extension_code_changed.output,
            &["polint.calls", "polint.direct_summaries"],
            "extension code",
        );
        assert!(scc_backdated_count(&extension_code_changed.output) > 0);

        std::fs::write(extension_dir.join("declared.txt"), "input-v2\n")
            .expect("mutate extension-declared input");
        let extension_input_changed = run_identity_case(
            &baseline_loaded,
            &cache,
            "Baseline rule description",
            Capabilities::new().calls(),
            |plan| plan,
        );
        assert_eq!(
            extension_code_changed
                .output
                .run_report
                .input_snapshot
                .extensions,
            extension_input_changed
                .output
                .run_report
                .input_snapshot
                .extensions
        );
        assert_ne!(
            provider_digest(&extension_code_changed.output, "polint.extensions"),
            provider_digest(&extension_input_changed.output, "polint.extensions")
        );
        assert_ne!(
            provider_digest(&extension_code_changed.output, "polint.refined_calls"),
            provider_digest(&extension_input_changed.output, "polint.refined_calls")
        );
        assert_same_provider_digests(
            &extension_code_changed.output,
            &extension_input_changed.output,
            &["polint.calls", "polint.direct_summaries"],
            "extension-declared input",
        );
        assert!(scc_backdated_count(&extension_input_changed.output) > 0);
        assert_cached_analysis_reused(&extension_input_changed.output, "extension-declared input");
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
