use crate::analysis_plan::AnalysisPlan;
use crate::cache::Cache;
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, CapabilitySupportView};
use crate::diagnostics::{Diagnostic, TextRange};
use std::collections::{BTreeMap, BTreeSet};

#[rustfmt::skip]
#[cfg(test)] mod debug;
#[cfg(test)]
mod dispatch_tests;
pub(crate) mod go_syntax_projection;
pub(crate) mod incremental;
mod metadata;
pub(crate) mod metrics_projection;
mod outcome;
mod provider;
mod store;
pub(crate) mod validation;
use incremental::{Digest, ProviderTelemetry};

pub(crate) use metadata::{
    FactConfidence, FactFamily, FactMeta, FactMetaStore, FactPrecision, FactRef, MissingFactMeta,
    ValidationStatus, resolution_metadata, resolution_status_metadata, stable_key_from_parts,
    symbol_metadata,
};
pub(crate) use outcome::{
    ProviderFailureReason, ProviderFailureSignal, ProviderFailureStage, ProviderOutcome,
    ProviderOutcomeStatus, ProviderOutcomeTracker, ProviderOutputIdentity, ValidationDowngrades,
    hard_dependencies,
};
#[cfg(test)]
pub(crate) use provider::ProviderKind;
pub(crate) use provider::{
    CachePolicy, LanguageScope, PrecisionCeiling, ProviderManifest, SchemaVersion,
};
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

macro_rules! ready_digest {
    ($digest:expr) => {
        $digest
            .clone()
            .expect("provider readiness requires identity")
    };
}

#[cfg(test)]
std::thread_local! {
    static FAIL_SEMANTIC_MIR_EXECUTION_ONCE: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
}

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
    pub(crate) runtime_blocked_rules: BTreeSet<String>,
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "The crate-private run report is consumed by internal tests and eval fixtures before a public surface exists."
        )
    )]
    pub(crate) run_report: incremental::KernelRunReport,
}

struct ProviderRunState {
    identities: BTreeMap<String, ProviderOutputIdentity>,
    telemetry: Vec<ProviderTelemetry>,
    tracker: ProviderOutcomeTracker,
}

type ProviderResult<T> = Result<T, outcome::ProviderOutcomeError>;

impl AnalysisKernel {
    pub(crate) fn provider_manifests() -> &'static [ProviderManifest] {
        provider::provider_manifests()
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
        let selected_providers = Self::selected_provider_ids(
            run_semantic_pipeline,
            run_cfg_call_pipeline,
            run_full_refinement_pipeline,
            run_data_flow_pipeline,
        );
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
                "phase: source files loaded"
            );
        }
        let input_snapshot = incremental::InputSnapshot::from_run_inputs(
            input.loaded,
            &db,
            input.config_digest,
            input.rule_digest,
            input.plan.digest(),
            Self::provider_manifests(),
        );
        let mut diagnostics = Vec::new();
        let mut provider_run = ProviderRunState {
            identities: BTreeMap::new(),
            telemetry: Vec::new(),
            tracker: ProviderOutcomeTracker::from_manifests(
                Self::provider_manifests(),
                &selected_providers,
            )?,
        };

        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.source",
            &mut db,
            incremental::CacheStats::default(),
            None,
        )?;

        assert!(Self::begin_provider(&mut provider_run, "polint.go.syntax")?);
        let go_output = crate::go::analyze_with_plan_options_and_cache_stats(
            &mut db,
            input.cache,
            input.config_digest,
            input.rule_digest,
            input.plan,
            input.parallel,
        );
        let go_output_digest = go_output.output_digest.clone();
        tracing::info!(target: "polint::kernel", "phase: go.syntax done");
        diagnostics.extend(go_output.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.go.syntax",
            &mut db,
            go_output.cache_stats,
            go_output_digest,
        )?;

        assert!(Self::begin_provider(&mut provider_run, "polint.ts.syntax")?);
        let ts_output = crate::ts::analyze_with_plan_options_and_cache_stats(
            &mut db,
            input.cache,
            input.config_digest,
            input.rule_digest,
            input.plan,
            input.parallel,
        );
        let ts_output_digest = ts_output.output_digest.clone();
        tracing::info!(target: "polint::kernel", "phase: ts.syntax done");
        diagnostics.extend(ts_output.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.ts.syntax",
            &mut db,
            ts_output.cache_stats,
            ts_output_digest,
        )?;

        let go_dependency_output_digest = Self::provider_digest(&provider_run, "polint.go.syntax");
        let ts_dependency_output_digest = Self::provider_digest(&provider_run, "polint.ts.syntax");
        let module_graph = if Self::begin_provider(&mut provider_run, "polint.module_graph")? {
            crate::module_graph::derive_requested_module_graph_with_cache_stats(
                &mut db,
                input.loaded,
                input.plan,
                input.cache,
                &input_snapshot,
                Self::provider_manifest("polint.module_graph"),
                vec![
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
            )
        } else {
            Default::default()
        };
        let module_support = module_graph.support_view(input.plan.support_view());
        // Keep polint.module_graph cache_stats internal to KernelRunReport.
        let polint_module_graph_cache_stats = module_graph.cache_stats.clone();
        let module_output_digest = module_graph.output_digest.clone();
        diagnostics.extend(module_graph.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.module_graph",
            &mut db,
            polint_module_graph_cache_stats,
            module_output_digest,
        )?;

        let module_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.module_graph");
        let symbol_graph = if Self::begin_provider(&mut provider_run, "polint.symbol_graph")? {
            crate::symbol_graph::derive_requested_symbols_with_cache_stats(
                &mut db,
                input.loaded,
                input.plan,
                input.cache,
                &input_snapshot,
                Self::provider_manifest("polint.symbol_graph"),
                ready_digest!(module_dependency_output_digest),
                vec![
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
            )
        } else {
            Default::default()
        };
        let capability_support = symbol_graph.support_view(&module_support);
        let polint_symbol_graph_cache_stats = symbol_graph.cache_stats.clone();
        let symbol_output_digest = symbol_graph.output_digest.clone();
        diagnostics.extend(symbol_graph.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.symbol_graph",
            &mut db,
            polint_symbol_graph_cache_stats,
            symbol_output_digest,
        )?;

        let symbol_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.symbol_graph");
        let module_topology = if run_cfg_call_pipeline
            && Self::begin_provider(&mut provider_run, "polint.module_topology")?
        {
            crate::module_graph::derive_module_topology_with_cache_stats(
                &mut db,
                input.cache,
                &input_snapshot,
                Self::provider_manifest("polint.module_topology"),
                ready_digest!(module_dependency_output_digest),
                ready_digest!(symbol_dependency_output_digest),
            )
        } else {
            Default::default()
        };
        let polint_module_topology_cache_stats = module_topology.cache_stats.clone();
        let module_topology_output_digest = module_topology.output_digest.clone();
        diagnostics.extend(module_topology.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.module_topology",
            &mut db,
            polint_module_topology_cache_stats,
            module_topology_output_digest,
        )?;
        let module_topology_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.module_topology");
        let semantic_mir = if run_semantic_pipeline
            && Self::begin_provider(&mut provider_run, "polint.semantic_mir")?
        {
            crate::analysis::provider::derive_semantic_mir_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.semantic_mir"),
                ready_digest!(module_topology_dependency_output_digest),
                ready_digest!(symbol_dependency_output_digest),
                vec![
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
            )
        } else {
            Default::default()
        };
        let polint_semantic_mir_cache_stats = semantic_mir.cache_stats.clone();
        let semantic_mir_output_digest = semantic_mir.output_digest.clone();
        diagnostics.extend(semantic_mir.diagnostics);
        #[cfg(test)]
        if FAIL_SEMANTIC_MIR_EXECUTION_ONCE.with(|failure| failure.replace(false)) {
            db.record_provider_failure(
                "polint.semantic_mir",
                ProviderOutcomeStatus::Failed,
                ProviderFailureStage::Execution,
                ProviderFailureReason::ExecutionFailed,
            );
        }
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.semantic_mir",
            &mut db,
            polint_semantic_mir_cache_stats,
            semantic_mir_output_digest,
        )?;

        let semantic_mir_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.semantic_mir");
        let cfg = if run_cfg_call_pipeline && Self::begin_provider(&mut provider_run, "polint.cfg")?
        {
            crate::analysis::cfg::provider::derive_cfg_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.cfg"),
                ready_digest!(semantic_mir_dependency_output_digest),
                vec![
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
            )
        } else {
            Default::default()
        };
        let polint_cfg_cache_stats = cfg.cache_stats.clone();
        let cfg_output_digest = cfg.output_digest.clone();
        diagnostics.extend(cfg.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.cfg",
            &mut db,
            polint_cfg_cache_stats,
            cfg_output_digest,
        )?;

        let cfg_dependency_output_digest = Self::provider_digest(&provider_run, "polint.cfg");
        let calls_ready =
            run_cfg_call_pipeline && Self::begin_provider(&mut provider_run, "polint.calls")?;
        let calls = if calls_ready {
            crate::analysis::calls::provider::derive_calls_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.calls"),
                ready_digest!(semantic_mir_dependency_output_digest),
                ready_digest!(cfg_dependency_output_digest),
                ready_digest!(symbol_dependency_output_digest),
                ready_digest!(module_topology_dependency_output_digest),
                vec![
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
            )
        } else if !run_cfg_call_pipeline
            && semantic_mir_dependency_output_digest.is_some()
            && cfg_dependency_output_digest.is_some()
        {
            crate::analysis::calls::provider::derive_call_sites_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.calls"),
                ready_digest!(semantic_mir_dependency_output_digest),
                ready_digest!(cfg_dependency_output_digest),
                vec![
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
            )
        } else {
            Default::default()
        };
        let polint_calls_cache_stats = calls.cache_stats.clone();
        let calls_output_digest = calls.output_digest.clone();
        diagnostics.extend(calls.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.calls",
            &mut db,
            polint_calls_cache_stats,
            calls_output_digest,
        )?;

        let calls_dependency_output_digest = Self::provider_digest(&provider_run, "polint.calls");

        let go_semantic = if run_full_refinement_pipeline
            && Self::begin_provider(&mut provider_run, "polint.go.semantic")?
        {
            crate::go::semantic::provider::derive_go_semantic_with_cache_stats(
                &mut db,
                input.loaded,
                &input_snapshot,
                Self::provider_manifest("polint.go.semantic"),
                ready_digest!(go_dependency_output_digest),
            )
        } else {
            Default::default()
        };
        let polint_go_semantic_cache_stats = go_semantic.cache_stats.clone();
        let go_semantic_output_digest = go_semantic.output_digest.clone();
        diagnostics.extend(go_semantic.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.go.semantic",
            &mut db,
            polint_go_semantic_cache_stats,
            go_semantic_output_digest,
        )?;
        let go_semantic_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.go.semantic");

        let identity = if run_full_refinement_pipeline
            && Self::begin_provider(&mut provider_run, "polint.identity")?
        {
            crate::analysis::identity::provider::derive_identity_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.identity"),
                ready_digest!(calls_dependency_output_digest),
                ready_digest!(go_semantic_dependency_output_digest),
            )
        } else {
            Default::default()
        };
        let polint_identity_cache_stats = identity.cache_stats.clone();
        let identity_output_digest = identity.output_digest;
        diagnostics.extend(identity.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.identity",
            &mut db,
            polint_identity_cache_stats,
            identity_output_digest,
        )?;
        let identity_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.identity");
        let abstract_domains_ready = run_full_refinement_pipeline
            && Self::begin_provider(&mut provider_run, "polint.abstract_domains")?;
        let abstract_domains = if abstract_domains_ready && compact_domain_materialization {
            crate::analysis::domains::provider::derive_summary_input_abstract_domains_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.abstract_domains"),
                ready_digest!(semantic_mir_dependency_output_digest),
                ready_digest!(cfg_dependency_output_digest),
                ready_digest!(calls_dependency_output_digest),
                ready_digest!(symbol_dependency_output_digest),
                ready_digest!(module_topology_dependency_output_digest),
                vec![
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
            )
        } else if abstract_domains_ready {
            crate::analysis::domains::provider::derive_abstract_domains_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.abstract_domains"),
                ready_digest!(semantic_mir_dependency_output_digest),
                ready_digest!(cfg_dependency_output_digest),
                ready_digest!(calls_dependency_output_digest),
                ready_digest!(symbol_dependency_output_digest),
                ready_digest!(module_topology_dependency_output_digest),
                vec![
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
            )
        } else {
            Default::default()
        };
        let polint_abstract_domains_cache_stats = abstract_domains.cache_stats.clone();
        let abstract_domains_output_digest = abstract_domains.output_digest;
        diagnostics.extend(abstract_domains.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.abstract_domains",
            &mut db,
            polint_abstract_domains_cache_stats,
            abstract_domains_output_digest,
        )?;
        let abstract_domains_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.abstract_domains");
        let entrypoints_semantic_mir_digest = semantic_mir_dependency_output_digest.clone();
        let entrypoints_cfg_digest = cfg_dependency_output_digest.clone();
        let entrypoints_calls_digest = calls_dependency_output_digest.clone();
        let entrypoints_symbol_digest = symbol_dependency_output_digest.clone();
        let entrypoints_topology_digest = module_topology_dependency_output_digest.clone();
        let direct_summaries_ready = run_full_refinement_pipeline
            && Self::begin_provider(&mut provider_run, "polint.direct_summaries")?;
        let direct_summaries = if direct_summaries_ready {
            crate::analysis::summaries::provider::derive_direct_summaries_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.direct_summaries"),
                ready_digest!(semantic_mir_dependency_output_digest),
                ready_digest!(cfg_dependency_output_digest),
                ready_digest!(calls_dependency_output_digest),
                ready_digest!(abstract_domains_dependency_output_digest),
                ready_digest!(symbol_dependency_output_digest),
                ready_digest!(module_topology_dependency_output_digest),
                vec![
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
            )
        } else {
            Default::default()
        };
        let polint_direct_summaries_cache_stats = direct_summaries.cache_stats.clone();
        diagnostics.extend(direct_summaries.diagnostics);

        // SCC closure: interprocedural summary improvement over SCCs.
        // Runs after direct summaries so callee summaries are available.
        let scc_closure = if direct_summaries_ready {
            crate::analysis::summaries::provider::run_scc_closure_with_cache(
                &mut db,
                input.cache,
                input.config_digest,
                input.rule_digest,
                input.plan.digest(),
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
        let direct_summaries_output_digest = direct_summaries_ready.then(|| {
            crate::analysis::summaries::provider::direct_summaries_output_digest(
                Self::provider_manifest("polint.direct_summaries"),
                &input_snapshot,
                &ready_digest!(entrypoints_semantic_mir_digest),
                &ready_digest!(entrypoints_cfg_digest),
                &ready_digest!(entrypoints_calls_digest),
                &ready_digest!(abstract_domains_dependency_output_digest),
                &ready_digest!(entrypoints_symbol_digest),
                &ready_digest!(entrypoints_topology_digest),
                &[
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
                &crate::analysis::summaries::provider::callable_stable_key_map(&db),
                &final_direct_summaries_output,
            )
        });
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.direct_summaries",
            &mut db,
            polint_direct_summaries_cache_stats,
            direct_summaries_output_digest,
        )?;

        let entrypoints = if run_full_refinement_pipeline
            && Self::begin_provider(&mut provider_run, "polint.entrypoints")?
        {
            crate::analysis::entrypoints::provider::derive_entrypoints_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.entrypoints"),
                ready_digest!(entrypoints_semantic_mir_digest),
                ready_digest!(entrypoints_cfg_digest),
                ready_digest!(entrypoints_calls_digest),
                ready_digest!(entrypoints_symbol_digest),
                ready_digest!(entrypoints_topology_digest),
                vec![
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
            )
        } else {
            Default::default()
        };
        let polint_entrypoints_cache_stats = entrypoints.cache_stats.clone();
        let entrypoints_output_digest = entrypoints.output_digest.clone();
        diagnostics.extend(entrypoints.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.entrypoints",
            &mut db,
            polint_entrypoints_cache_stats,
            entrypoints_output_digest,
        )?;

        let direct_summaries_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.direct_summaries");
        let entrypoints_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.entrypoints");

        // polint.reachability runs immediately after polint.entrypoints (D-19),
        // consuming the calls/entrypoints/identity/symbol/topology output digests.
        let reachability = if run_full_refinement_pipeline
            && Self::begin_provider(&mut provider_run, "polint.reachability")?
        {
            crate::analysis::reachability::provider::derive_reachability_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.reachability"),
                &input.loaded.config.reachability.roots,
                ready_digest!(entrypoints_calls_digest),
                ready_digest!(entrypoints_dependency_output_digest),
                ready_digest!(identity_dependency_output_digest),
                ready_digest!(entrypoints_symbol_digest),
                ready_digest!(entrypoints_topology_digest),
            )
        } else {
            Default::default()
        };
        let polint_reachability_cache_stats = reachability.cache_stats.clone();
        let reachability_output_digest = reachability.output_digest;
        diagnostics.extend(reachability.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.reachability",
            &mut db,
            polint_reachability_cache_stats,
            reachability_output_digest,
        )?;
        let reachability_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.reachability");

        let extensions = if run_full_refinement_pipeline
            && Self::begin_provider(&mut provider_run, "polint.extensions")?
        {
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
        let extensions_output_digest = extensions.output_digest.clone();
        diagnostics.extend(extensions.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.extensions",
            &mut db,
            polint_extensions_cache_stats,
            extensions_output_digest,
        )?;
        let extensions_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.extensions");

        let type_value_alias = if run_full_refinement_pipeline
            && Self::begin_provider(&mut provider_run, "polint.type_value_alias")?
        {
            crate::analysis::types::provider::derive_type_value_alias_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.type_value_alias"),
                ready_digest!(entrypoints_semantic_mir_digest),
                ready_digest!(entrypoints_cfg_digest),
                ready_digest!(entrypoints_calls_digest),
                ready_digest!(abstract_domains_dependency_output_digest),
                ready_digest!(direct_summaries_dependency_output_digest),
                ready_digest!(entrypoints_dependency_output_digest),
                ready_digest!(extensions_dependency_output_digest),
                ready_digest!(entrypoints_symbol_digest),
                ready_digest!(entrypoints_topology_digest),
                vec![
                    ready_digest!(go_dependency_output_digest),
                    ready_digest!(ts_dependency_output_digest),
                ],
            )
        } else {
            Default::default()
        };
        let polint_type_value_alias_cache_stats = type_value_alias.cache_stats.clone();
        let type_value_alias_output_digest = type_value_alias.output_digest.clone();
        diagnostics.extend(type_value_alias.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.type_value_alias",
            &mut db,
            polint_type_value_alias_cache_stats,
            type_value_alias_output_digest,
        )?;
        let type_value_alias_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.type_value_alias");

        // Thread the per-language solver config into SolverBudget (D-10/D-11),
        // mirroring how the reachability provider reaches `reachability.roots`.
        // Cross-domain fields stay at their defaults; absent config falls back to each
        // sub-budget default. The adaptation sub-budget is passed to semantic_graph as
        // well because model files lower into semantic constraints before the solver
        // expands them.
        let solver_budget = crate::analysis::solver::budget::SolverBudget {
            go: input.loaded.config.solver.to_go_sub_budget(),
            js: input.loaded.config.solver.to_js_sub_budget(),
            object_model_enabled: input.loaded.config.solver.js_object_model_enabled(),
            object: input.loaded.config.solver.to_js_object_sub_budget(),
            ..crate::analysis::solver::budget::SolverBudget::default()
        };

        // polint.semantic_graph runs between polint.type_value_alias and
        // polint.refined_calls (D-16). It projects the unified node/edge/constraint
        // graph from already-stored facts plus repo-local adaptation models, and folds
        // every consumed upstream provider output digest into its own output digest
        // (D-17).
        let semantic_graph = if run_full_refinement_pipeline
            && Self::begin_provider(&mut provider_run, "polint.semantic_graph")?
        {
            crate::analysis::semantic_graph::provider::derive_semantic_graph_with_cache_stats(
                &mut db,
                input.loaded,
                solver_budget.adaptation,
                &input_snapshot,
                Self::provider_manifest("polint.semantic_graph"),
                ready_digest!(entrypoints_calls_digest),
                ready_digest!(identity_dependency_output_digest),
                ready_digest!(abstract_domains_dependency_output_digest),
                ready_digest!(entrypoints_dependency_output_digest),
                ready_digest!(reachability_dependency_output_digest),
                ready_digest!(type_value_alias_dependency_output_digest),
                ready_digest!(entrypoints_symbol_digest),
                ready_digest!(entrypoints_topology_digest),
                ready_digest!(go_dependency_output_digest),
                ready_digest!(ts_dependency_output_digest),
                ready_digest!(entrypoints_semantic_mir_digest),
                ready_digest!(go_semantic_dependency_output_digest),
            )
        } else {
            Default::default()
        };
        let polint_semantic_graph_cache_stats = semantic_graph.cache_stats.clone();
        let semantic_graph_output_digest = semantic_graph.output_digest.clone();
        diagnostics.extend(semantic_graph.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.semantic_graph",
            &mut db,
            polint_semantic_graph_cache_stats,
            semantic_graph_output_digest,
        )?;

        let semantic_graph_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.semantic_graph");

        // polint.solver runs between polint.semantic_graph and polint.refined_calls
        // (D-13). It drives the unified solver engine over
        // the closed input snapshot (the stored semantic-graph constraints), emits
        // derived edges + provenance, and folds the semantic_graph + points-to source
        // (type_value_alias) + go.semantic output digests plus the SolverBudget into its
        // own output digest (D-15). The go.semantic digest is folded because the Go RTA
        // policy reads the stored go.semantic RTA-signal families (instantiated_types /
        // address_taken / dynamic_dispatch / method_sets) via GoRtaInputs::from_db, so a Go
        // edit touching ONLY those families changes the RTA-resolved edges and must
        // invalidate the solver cache (FIX 4 — without this the provider docstring's "any
        // upstream change invalidates the solver cache" was false). Auto-enrolls in the
        // the determinism gate (D-14). Thread the per-language solver config into
        // SolverBudget (D-10/D-11).
        let solver = if run_full_refinement_pipeline
            && Self::begin_provider(&mut provider_run, "polint.solver")?
        {
            crate::analysis::solver::provider::derive_solver_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.solver"),
                solver_budget,
                ready_digest!(semantic_graph_dependency_output_digest),
                ready_digest!(type_value_alias_dependency_output_digest),
                ready_digest!(go_semantic_dependency_output_digest),
            )
        } else {
            Default::default()
        };
        let polint_solver_cache_stats = solver.cache_stats.clone();
        let solver_output_digest = solver.output_digest.clone();
        diagnostics.extend(solver.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.solver",
            &mut db,
            polint_solver_cache_stats,
            solver_output_digest,
        )?;
        let solver_dependency_output_digest = Self::provider_digest(&provider_run, "polint.solver");

        let refined_calls = if run_cfg_call_pipeline
            && Self::begin_provider(&mut provider_run, "polint.refined_calls")?
        {
            crate::analysis::refined_calls::provider::derive_refined_calls_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.refined_calls"),
                ready_digest!(entrypoints_calls_digest),
                ready_digest!(entrypoints_dependency_output_digest),
                ready_digest!(direct_summaries_dependency_output_digest),
                ready_digest!(type_value_alias_dependency_output_digest),
                ready_digest!(extensions_dependency_output_digest),
                ready_digest!(solver_dependency_output_digest),
            )
        } else {
            Default::default()
        };
        let polint_refined_calls_cache_stats = refined_calls.cache_stats.clone();
        let refined_calls_output_digest = refined_calls.output_digest.clone();
        diagnostics.extend(refined_calls.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.refined_calls",
            &mut db,
            polint_refined_calls_cache_stats,
            refined_calls_output_digest,
        )?;

        let refined_calls_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.refined_calls");
        let data_flow = if run_data_flow_pipeline
            && Self::begin_provider(&mut provider_run, "polint.data_flow")?
        {
            crate::analysis::data_flow::provider::derive_data_flow_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.data_flow"),
                ready_digest!(entrypoints_semantic_mir_digest),
                ready_digest!(entrypoints_cfg_digest),
                ready_digest!(entrypoints_calls_digest),
                ready_digest!(refined_calls_dependency_output_digest),
                ready_digest!(direct_summaries_dependency_output_digest),
                ready_digest!(type_value_alias_dependency_output_digest),
                ready_digest!(entrypoints_dependency_output_digest),
                ready_digest!(extensions_dependency_output_digest),
            )
        } else {
            Default::default()
        };
        let polint_data_flow_cache_stats = data_flow.cache_stats.clone();
        let data_flow_output_digest = data_flow.output_digest.clone();
        diagnostics.extend(data_flow.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.data_flow",
            &mut db,
            polint_data_flow_cache_stats,
            data_flow_output_digest,
        )?;

        let data_flow_dependency_output_digest =
            Self::provider_digest(&provider_run, "polint.data_flow");
        let evidence = if run_data_flow_pipeline
            && Self::begin_provider(&mut provider_run, "polint.evidence")?
        {
            crate::analysis::evidence::provider::derive_evidence_with_cache_stats(
                &mut db,
                &input_snapshot,
                Self::provider_manifest("polint.evidence"),
                ready_digest!(entrypoints_semantic_mir_digest),
                ready_digest!(entrypoints_cfg_digest),
                ready_digest!(entrypoints_calls_digest),
                ready_digest!(refined_calls_dependency_output_digest),
                ready_digest!(direct_summaries_dependency_output_digest),
                ready_digest!(type_value_alias_dependency_output_digest),
                ready_digest!(entrypoints_dependency_output_digest),
                ready_digest!(extensions_dependency_output_digest),
                ready_digest!(data_flow_dependency_output_digest),
            )
        } else {
            Default::default()
        };
        let polint_evidence_cache_stats = evidence.cache_stats.clone();
        let evidence_output_digest = evidence.output_digest;
        diagnostics.extend(evidence.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.evidence",
            &mut db,
            polint_evidence_cache_stats,
            evidence_output_digest,
        )?;

        let metrics = if Self::begin_provider(&mut provider_run, "polint.metrics")? {
            crate::metrics::derive_requested_metrics_with_cache_stats(
                &mut db,
                input.plan,
                input.cache,
                Self::provider_manifest("polint.metrics"),
            )?
        } else {
            Default::default()
        };
        let polint_metrics_cache_stats = metrics.cache_stats.clone();
        let metrics_output_digest = metrics.output_digest;
        diagnostics.extend(metrics.diagnostics);
        Self::record_provider_projection(
            &mut provider_run,
            &selected_providers,
            "polint.metrics",
            &mut db,
            polint_metrics_cache_stats,
            metrics_output_digest,
        )?;
        tracing::info!(target: "polint::kernel", "phase: metrics + derived done");
        let validation_report = validation::validate_fact_metadata(&db, Self::provider_manifests());
        diagnostics.extend(validation_report.iter().cloned());
        let provider_outcomes = provider_run.tracker.seal(&validation_report.downgrades())?;
        let (runtime_blocked_rules, capability_diagnostics) =
            Self::runtime_capability_blockers(input.plan, &db, &provider_outcomes);
        diagnostics.extend(capability_diagnostics);
        db.finish_all_fact_meta_insertions();
        // Persistence is deliberately last: store availability must not change
        // provider execution, validated facts, diagnostics, or capability
        // support. Only this private maintenance status is stored.
        let store_config = store::StoreConfig::new(
            input.cache.semantic_store_path(),
            input.cache.semantic_store_enabled(),
        );
        let store_status = store::SemanticStore::maintain(&store_config);
        let run_report = incremental::KernelRunReport::new(
            input_snapshot,
            provider_outcomes,
            provider_run.telemetry,
            scc_closure.demand_query_trace,
            store_status,
        );
        #[cfg(test)]
        let run_report = run_report.with_scc_closure_debug(scc_closure_debug);

        Ok(KernelOutput {
            db,
            diagnostics,
            capability_support,
            runtime_blocked_rules,
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
    ) -> Vec<ProviderOutputIdentity> {
        output
            .run_report
            .provider_outcomes
            .iter()
            .filter_map(|outcome| outcome.output_identity.clone())
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn semantic_store_schema_is_current_for_test(path: &std::path::Path) -> bool {
        store::current_schema_is_valid_for_test(path)
    }

    fn record_provider_projection(
        state: &mut ProviderRunState,
        selected_providers: &BTreeSet<&'static str>,
        id: &'static str,
        db: &mut AnalysisDb,
        cache_stats: incremental::CacheStats,
        output_digest: Option<Digest>,
    ) -> ProviderResult<()> {
        state
            .telemetry
            .push(ProviderTelemetry::new(id, cache_stats));
        if !selected_providers.contains(id) || !state.tracker.is_pending(id)? {
            return Ok(());
        }
        if let Some(failure) = db.take_provider_failure(id) {
            return state.tracker.record_non_success(
                id,
                failure.status,
                failure.stage,
                failure.reason,
            );
        }
        let manifest = Self::provider_manifest(id);
        let output_digest = output_digest.unwrap_or_else(|| {
            incremental::provider_output_digest_from_manifest(
                manifest,
                &provider_output_summary_parts(db, manifest),
            )
        });
        let identity = incremental::provider_output_identity_from_manifest(manifest, output_digest);
        state.tracker.record_success(id, identity.clone())?;
        let unique = state.identities.insert(id.to_string(), identity).is_none();
        assert!(unique, "provider {id} recorded twice");
        Ok(())
    }

    fn begin_provider(state: &mut ProviderRunState, id: &str) -> ProviderResult<bool> {
        let blockers = state.tracker.can_run(id)?;
        let ready = blockers.is_empty();
        if !ready {
            state.tracker.record_dependency_blocked(id, blockers)?;
        }
        Ok(ready)
    }
    fn provider_digest(state: &ProviderRunState, id: &str) -> Option<Digest> {
        state
            .identities
            .get(id)
            .map(|identity| identity.output_digest.clone())
    }
    fn selected_provider_ids(
        run_semantic_pipeline: bool,
        run_cfg_call_pipeline: bool,
        run_full_refinement_pipeline: bool,
        run_data_flow_pipeline: bool,
    ) -> BTreeSet<&'static str> {
        let mut selected = [
            "polint.source",
            "polint.go.syntax",
            "polint.ts.syntax",
            "polint.module_graph",
            "polint.symbol_graph",
            "polint.metrics",
        ]
        .into_iter()
        .collect::<BTreeSet<_>>();
        if run_semantic_pipeline {
            selected.insert("polint.semantic_mir");
        }
        if run_cfg_call_pipeline {
            selected.extend([
                "polint.module_topology",
                "polint.cfg",
                "polint.calls",
                "polint.refined_calls",
            ]);
        }
        if run_full_refinement_pipeline {
            selected.extend([
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
            ]);
        }
        if run_data_flow_pipeline {
            selected.extend(["polint.data_flow", "polint.evidence"]);
        }
        selected
    }
    pub(crate) fn runtime_capability_blockers(
        plan: &AnalysisPlan,
        db: &AnalysisDb,
        outcomes: &[ProviderOutcome],
    ) -> (BTreeSet<String>, Vec<Diagnostic>) {
        let by_id = outcomes
            .iter()
            .map(|outcome| (outcome.provider_id.as_str(), outcome))
            .collect::<BTreeMap<_, _>>();
        let mut blocked_rules = BTreeSet::new();
        let mut diagnostics = Vec::new();
        for rule in plan.rules() {
            for capability in &rule.requested_capabilities {
                if plan.support_view().status_for(capability)
                    != Some(crate::core::CapabilitySupportStatus::Supported)
                {
                    continue;
                }
                let mut providers = Self::capability_providers(capability, db);
                if capability == "events" {
                    for (provider_id, has_rows) in [
                        ("polint.calls", !db.call_sites().is_empty()),
                        ("polint.refined_calls", !db.refined_call_edges().is_empty()),
                    ] {
                        if has_rows
                            && by_id.get(provider_id).is_some_and(|outcome| {
                                outcome.status != ProviderOutcomeStatus::PlannedAbsent
                            })
                        {
                            providers.push(provider_id);
                        }
                    }
                }
                // `capability_providers` is a hand-maintained table; a provider
                // id that is not in the sealed inventory is a table bug, caught
                // by `capability_provider_table_references_only_manifest_ids`.
                // Skipping it here keeps that bug from panicking a real run.
                let failed = providers
                    .into_iter()
                    .filter_map(|provider_id| by_id.get(provider_id).copied())
                    .filter(|outcome| outcome.status != ProviderOutcomeStatus::Succeeded)
                    .collect::<Vec<_>>();
                if failed.is_empty() {
                    continue;
                }
                blocked_rules.insert(rule.id.clone());
                let blockers = failed
                    .iter()
                    .flat_map(|outcome| match outcome.blockers.as_slice() {
                        [] => std::slice::from_ref(&outcome.provider_id),
                        blockers => blockers,
                    })
                    .cloned()
                    .collect::<BTreeSet<_>>();
                diagnostics.push(
                    Diagnostic::error(
                        "polint/capability",
                        "<workspace>",
                        TextRange::point(1, 1),
                        format!(
                            "Rule `{}` requested capability `{capability}`, but provider closure did not succeed.",
                            rule.id
                        ),
                    )
                    .with_evidence("rule", rule.id.clone())
                    .with_evidence("capability", capability.clone())
                    .with_evidence("status", failed[0].status.label())
                    .with_evidence("blockers", blockers.into_iter().collect::<Vec<_>>().join(",")),
                );
            }
        }
        (blocked_rules, diagnostics)
    }
    fn capability_providers(capability: &str, db: &AnalysisDb) -> Vec<&'static str> {
        let providers: &[&str] = match capability {
            "source_files" => &["polint.source"],
            "syntax" | "packages" | "functions" | "imports" => {
                &["polint.go.syntax", "polint.ts.syntax"]
            }
            "go_tests" | "branch_obligations" | "coverage_facts" => &["polint.go.syntax"],
            "string_literals" => &["polint.go.syntax", "polint.ts.syntax"],
            "ts_components" | "ts_classes" | "jsx_attributes" => &["polint.ts.syntax"],
            "events" => &["polint.go.syntax", "polint.ts.syntax"],
            "resolved_imports" | "module_graph" => &["polint.module_graph"],
            "symbols" | "references" => &["polint.symbol_graph"],
            "calls" | "control_flow" | "cfg" | "call_graph" => &["polint.refined_calls"],
            "dataflow" => &["polint.evidence"],
            "file_metrics" | "function_metrics" | "complexity_metrics" => &["polint.metrics"],
            _ => &[],
        };
        providers
            .iter()
            .copied()
            .filter(|provider| {
                !matches!(capability, "events" | "string_literals")
                    || db.files().iter().any(|file| match *provider {
                        "polint.go.syntax" => file.language == crate::core::Language::Go,
                        "polint.ts.syntax" => file.language.is_ts_family(),
                        _ => false,
                    })
            })
            .collect()
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

    #[test]
    fn string_literals_capability_ownership_and_failure_gating_tracks_present_languages() {
        let plan = AnalysisPlan::from_capability_names_for_test(&["string_literals"]);
        for (paths, expected) in [
            (vec!["a.go"], vec!["polint.go.syntax"]),
            (vec!["a.ts"], vec!["polint.ts.syntax"]),
            (
                vec!["a.go", "a.ts"],
                vec!["polint.go.syntax", "polint.ts.syntax"],
            ),
        ] {
            let mut db = AnalysisDb::new();
            for path in paths {
                db.add_file(path.into(), path.into(), "x\n".into());
            }
            assert_eq!(
                AnalysisKernel::capability_providers("string_literals", &db),
                expected
            );
            for failed_id in ["polint.go.syntax", "polint.ts.syntax"] {
                let outcomes = ["polint.go.syntax", "polint.ts.syntax"]
                    .map(|id| syntax_outcome(id, id == failed_id));
                let (blocked, _) =
                    AnalysisKernel::runtime_capability_blockers(&plan, &db, &outcomes);
                assert_eq!(
                    blocked.contains("test/requested-capabilities"),
                    expected.contains(&failed_id)
                );
            }
        }
    }

    fn syntax_outcome(id: &str, failed: bool) -> ProviderOutcome {
        if failed {
            return ProviderOutcome::from_closed_parts(
                id.into(),
                ProviderOutcomeStatus::Failed,
                None,
                Some(ProviderFailureStage::Execution),
                Some(ProviderFailureReason::ExecutionFailed),
                Vec::new(),
            )
            .unwrap();
        }
        let manifest = AnalysisKernel::provider_manifest(id);
        let identity = incremental::provider_output_identity_from_manifest(
            manifest,
            Digest::from_parts(incremental::DigestKind::ProviderOutput, "test", &[id]),
        );
        ProviderOutcome::from_closed_parts(
            id.into(),
            ProviderOutcomeStatus::Succeeded,
            Some(identity),
            None,
            None,
            Vec::new(),
        )
        .unwrap()
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
        fn disabled_full_kernel_run_is_filesystem_free_and_records_status() {
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
            assert!(!store_path.exists());
            assert!(!store_path.parent().expect("store directory").exists());
        }

        #[test]
        fn enabled_maintenance_runs_after_validated_fact_finalization() {
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

            assert!(AnalysisKernel::missing_fact_metadata_for_test(&output.db).is_empty());
            assert_eq!(output.run_report.store_status(), &StoreStatus::Ready);
            assert!(store_path.is_file());
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
            Corrupt,
            Future,
            Invalid,
            Busy,
        }

        fn run_mode(mode: StoreMode) -> (String, u8, StoreStatus) {
            let temp = tempfile::tempdir().expect("temp directory");
            std::fs::write(
                temp.path().join("main.go"),
                "package main\n\nfunc main() { println(\"hello\") }\n",
            )
            .expect("write source");
            let loaded = load_config(temp.path()).expect("default config loads");
            let cache = Cache::default_for_repo(temp.path(), true);
            let cache = if matches!(mode, StoreMode::Disabled) {
                cache
            } else {
                cache.with_semantic_store_enabled_for_test()
            };
            let path = cache.semantic_store_path();
            let config = store::StoreConfig::new(&path, true);

            let mut corrupt_before = None;
            let mut fixture_before = None;
            let mut held_writer = None;
            match mode {
                StoreMode::Disabled | StoreMode::Enabled => {}
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
                    assert_eq!(store::SemanticStore::maintain(&config), StoreStatus::Ready);
                    fixture_before = Some(
                        store::fixture_snapshot_for_test(&path).expect("snapshot current fixture"),
                    );
                    held_writer = Some(
                        store::hold_writer_connection_for_test(&path).expect("hold writer lease"),
                    );
                }
            }

            let plan = AnalysisPlan::empty();
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
            drop(held_writer);
            (json, exit_code, status)
        }

        #[test]
        fn all_store_modes_preserve_byte_identical_json_and_exit_semantics() {
            let (disabled_json, disabled_exit, disabled_status) = run_mode(StoreMode::Disabled);
            assert_eq!(disabled_status, StoreStatus::Disabled);

            let cases = [
                (StoreMode::Enabled, StoreStatus::Ready),
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
        let provider_outcomes = &output.run_report.provider_outcomes;

        assert_eq!(snapshot["schema_version"], INPUT_SNAPSHOT_SCHEMA_VERSION);
        assert_eq!(
            provider_outcomes
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
    }

    #[test]
    fn direct_summaries_provider_output_reflects_final_summary_metadata() {
        let temp = tempfile::tempdir().expect("tempdir");
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
        .expect("kernel run should succeed");
        let manifest = AnalysisKernel::provider_manifest("polint.direct_summaries");
        let generic = incremental::provider_output_digest_from_manifest(
            manifest,
            &provider_output_summary_parts(&output.db, manifest),
        );
        let direct_summaries = provider_identity(&output, "polint.direct_summaries");

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
        let go = provider_telemetry(&output, "polint.go.syntax");
        let ts = provider_telemetry(&output, "polint.ts.syntax");

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

        let first_module_graph = provider_telemetry(&first, "polint.module_graph");
        let second_module_graph = provider_telemetry(&second, "polint.module_graph");

        assert_eq!(first_module_graph.cache_stats.misses, 1);
        assert_eq!(first_module_graph.cache_stats.recomputes, 1);
        assert_eq!(first_module_graph.cache_stats.writes, 1);
        assert_eq!(second_module_graph.cache_stats.hits, 1);
        assert_eq!(second_module_graph.cache_stats.verified_reuse, 1);
        assert_eq!(second_module_graph.cache_stats.recomputes, 0);
        assert_eq!(
            provider_identity(&first, "polint.module_graph").output_digest,
            provider_identity(&second, "polint.module_graph").output_digest
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

        let first_symbol_graph = provider_telemetry(&first, "polint.symbol_graph");
        let second_symbol_graph = provider_telemetry(&second, "polint.symbol_graph");

        assert_eq!(first_symbol_graph.cache_stats.misses, 1);
        assert_eq!(first_symbol_graph.cache_stats.recomputes, 1);
        assert_eq!(first_symbol_graph.cache_stats.writes, 1);
        assert_eq!(second_symbol_graph.cache_stats.hits, 1);
        assert_eq!(second_symbol_graph.cache_stats.verified_reuse, 1);
        assert_eq!(second_symbol_graph.cache_stats.recomputes, 0);
        assert_eq!(
            provider_identity(&first, "polint.symbol_graph").output_digest,
            provider_identity(&second, "polint.symbol_graph").output_digest
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

        let first_module_topology = provider_telemetry(&first, "polint.module_topology");
        let second_module_topology = provider_telemetry(&second, "polint.module_topology");
        let disabled_module_topology = provider_telemetry(&disabled, "polint.module_topology");
        let first_identity = provider_identity(&first, "polint.module_topology");
        let second_identity = provider_identity(&second, "polint.module_topology");

        assert_eq!(first_module_topology.cache_stats.misses, 1);
        assert_eq!(first_module_topology.cache_stats.recomputes, 1);
        assert_eq!(first_module_topology.cache_stats.writes, 1);
        assert_eq!(second_module_topology.cache_stats.hits, 1);
        assert_eq!(second_module_topology.cache_stats.verified_reuse, 1);
        assert_eq!(second_module_topology.cache_stats.recomputes, 0);
        assert_eq!(first_identity.output_digest, second_identity.output_digest);
        assert_eq!(disabled_module_topology.cache_stats.bypasses_disabled, 1);
        assert_eq!(disabled_module_topology.cache_stats.recomputes, 1);
        let _ = provider_identity(&disabled, "polint.module_topology");
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
        let semantic_mir = provider_identity(&output, "polint.semantic_mir");

        assert_eq!(semantic_mir.schema_version, "semantic-mir-facts-1:1");
        assert!(!semantic_mir.output_digest.value.is_empty());
        assert_eq!(provider_recomputes(&output, "polint.semantic_mir"), 1);
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
        let cfg = provider_identity(&output, "polint.cfg");

        assert_eq!(cfg.schema_version, "cfg-facts-1:1");
        assert!(!cfg.output_digest.value.is_empty());
        assert_eq!(provider_recomputes(&output, "polint.cfg"), 1);
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
        let calls = provider_identity(&output, "polint.calls");

        assert_eq!(calls.schema_version, "calls-facts-1:1");
        assert!(!calls.output_digest.value.is_empty());
        assert_eq!(provider_recomputes(&output, "polint.calls"), 1);
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

        let first_metrics = provider_telemetry(&first, "polint.metrics");
        let second_metrics = provider_telemetry(&second, "polint.metrics");

        assert_eq!(first_metrics.cache_stats.misses, 1);
        assert_eq!(first_metrics.cache_stats.recomputes, 1);
        assert_eq!(first_metrics.cache_stats.writes, 1);
        assert_eq!(second_metrics.cache_stats.hits, 1);
        assert_eq!(second_metrics.cache_stats.verified_reuse, 1);
        assert_eq!(second_metrics.cache_stats.recomputes, 0);
        let first_identity = provider_identity(&first, "polint.metrics");
        let second_identity = provider_identity(&second, "polint.metrics");
        assert_eq!(first_identity.output_digest, second_identity.output_digest);
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
        let metrics = provider_telemetry(&output, "polint.metrics");

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
            "polint.metrics",
        ] {
            assert_eq!(
                provider_telemetry(&output, provider_id).cache_stats,
                CacheStats::default()
            );
            let _ = provider_identity(&output, provider_id);
        }

        let semantic_mir = provider_outcome(&output, "polint.semantic_mir");
        assert_eq!(semantic_mir.status, ProviderOutcomeStatus::PlannedAbsent);
        assert!(semantic_mir.output_identity.is_none());
        assert_eq!(provider_recomputes(&output, "polint.semantic_mir"), 0);
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

        assert_eq!(provider_recomputes(&output, "polint.semantic_mir"), 0);
        assert_eq!(provider_recomputes(&output, "polint.calls"), 0);
        assert_eq!(provider_recomputes(&output, "polint.refined_calls"), 0);
        assert_eq!(provider_recomputes(&output, "polint.data_flow"), 0);
        assert_eq!(provider_recomputes(&output, "polint.evidence"), 0);
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

        assert_eq!(provider_recomputes(&output, "polint.symbol_graph"), 1);
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
                provider_recomputes(&output, provider_id),
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
                provider_recomputes(&output, provider_id),
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
                provider_recomputes(&output, provider_id),
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

        assert_eq!(provider_recomputes(&output, "polint.cfg"), 1);
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
            let stats = &provider_telemetry(&output, provider_id).cache_stats;
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
                provider_recomputes(&output, provider_id),
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

        assert_eq!(provider_recomputes(&output, "polint.refined_calls"), 1);
        assert_eq!(provider_recomputes(&output, "polint.data_flow"), 0);
        assert_eq!(provider_recomputes(&output, "polint.evidence"), 0);
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

    /// `capability_providers` and the `events` special case name providers by
    /// string. An id outside the sealed inventory silently contributes no
    /// blocker, so a rule would run against a failed provider closure.
    #[test]
    fn capability_provider_table_references_only_manifest_ids() {
        let source = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/analysis_kernel/mod.rs"),
        )
        .expect("this source file is readable");
        let body = source
            .split("fn capability_providers(")
            .nth(1)
            .expect("capability_providers is defined here")
            .split("\n    }\n")
            .next()
            .expect("capability_providers has a body");
        let inventory = AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| manifest.id)
            .collect::<BTreeSet<_>>();
        let mut referenced = body
            .split("\"polint.")
            .skip(1)
            .filter_map(|rest| rest.split('"').next())
            .map(|suffix| format!("polint.{suffix}"))
            .collect::<Vec<_>>();
        // The `events` fan-out in `runtime_capability_blockers` names two more.
        referenced.extend([
            "polint.calls".to_string(),
            "polint.refined_calls".to_string(),
        ]);
        assert!(!referenced.is_empty(), "no provider ids were extracted");
        for provider_id in referenced {
            assert!(
                inventory.contains(provider_id.as_str()),
                "capability table names `{provider_id}`, which is not a manifest provider"
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

    #[test]
    fn provider_outcomes_block_calls_after_upstream_execution_failure() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("main.ts"), "export function f(){f()}").expect("source");
        FAIL_SEMANTIC_MIR_EXECUTION_ONCE.with(|failure| failure.set(true));
        let output = AnalysisKernel::run(KernelInput {
            loaded: &load_config(temp.path()).expect("config"),
            cache: &Cache::new("", false),
            config_digest: "config",
            rule_digest: "rules",
            plan: &AnalysisPlan::from_capability_names_for_test(&["calls"]),
            parallel: false,
        })
        .expect("kernel run");
        let calls = provider_outcome(&output, "polint.calls");
        assert_eq!(calls.status, ProviderOutcomeStatus::DependencyBlocked);
        assert_eq!(calls.blockers, ["polint.cfg", "polint.semantic_mir"]);
        assert!(calls.output_identity.is_none() && output.db.call_sites().is_empty());
        let metrics = provider_outcome(&output, "polint.metrics");
        assert_eq!(metrics.status, ProviderOutcomeStatus::Succeeded);
    }

    fn provider_outcome<'a>(output: &'a KernelOutput, id: &str) -> &'a ProviderOutcome {
        let rows = &output.run_report.provider_outcomes;
        let outcome = rows.iter().find(|row| row.provider_id == id);
        outcome.expect("provider outcome")
    }
    fn provider_identity<'a>(output: &'a KernelOutput, id: &str) -> &'a ProviderOutputIdentity {
        let identity = &provider_outcome(output, id).output_identity;
        let identity = identity.as_ref().expect("provider success");
        assert!(!identity.output_digest.value.is_empty());
        identity
    }
    fn provider_telemetry<'a>(output: &'a KernelOutput, id: &str) -> &'a ProviderTelemetry {
        let rows = &output.run_report.provider_telemetry;
        let telemetry = rows.iter().find(|row| row.provider_id == id);
        telemetry.expect("provider telemetry")
    }
    fn provider_recomputes(output: &KernelOutput, id: &str) -> u64 {
        provider_telemetry(output, id).cache_stats.recomputes
    }
}
