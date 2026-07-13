use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::builder::DirectSummaryBuilder;
use super::cache_key::direct_summaries_provider_parameter_digest;
#[cfg(test)]
use super::closure::SccClosureResult;
use super::closure::{SccClosureConfig, close_summaries_by_scc};
#[cfg(test)]
use super::scc::SccSchedule;
use super::scc::compute_scc_schedule;
use super::store::SummaryOutput;
use crate::analysis::ids::MirBodyId;
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, DemandQueryEngine, DemandQueryTrace, Digest, DigestKind, InputComponentStatus,
    InputDependencyKey, InputSnapshot, QueryDependencyInputs,
};
use crate::cache::keys::AnalysisSettingsScope;
use crate::cache::{Cache, CacheKey};
use crate::core::{AnalysisDb, CapabilitySupportStatus};
use crate::diagnostics::Diagnostic;

const REQUESTED_CAPABILITIES: &[&str] = &["calls", "control_flow", "dataflow"];
const SCC_CLOSURE_QUERY_VERSION: &str = "1";
const SCC_CLOSURE_CACHE_SCHEMA: &str = "summary-scc-closure-digests-v2";

#[derive(Debug, Clone, Default)]
pub(crate) struct DirectSummariesProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_direct_summaries_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    abstract_domains_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> DirectSummariesProviderOutput {
    let output = DirectSummaryBuilder::build(db);
    let callable_keys = callable_stable_key_map(db);
    let output_digest = direct_summaries_output_digest(
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &cfg_output_digest,
        &calls_output_digest,
        &abstract_domains_output_digest,
        &symbol_graph_output_digest,
        &module_topology_output_digest,
        &upstream_syntax_output_digests,
        &callable_keys,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();
    db.replace_summary_facts(output);

    DirectSummariesProviderOutput {
        diagnostics: Vec::new(),
        cache_stats,
        output_digest: Some(output_digest),
    }
}

// ---------------------------------------------------------------------------
// SCC closure orchestration
// ---------------------------------------------------------------------------

/// Output of the SCC closure step.
#[derive(Debug, Clone, Default)]
pub(crate) struct SccClosureProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) demand_query_trace: DemandQueryTrace,
    pub(crate) scc_output_digests: BTreeMap<Vec<String>, String>,
    #[cfg(test)]
    pub(crate) debug_snapshot: Option<SccClosureDebugSnapshot>,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SccClosureDebugSnapshot {
    pub(crate) schedule: SccSchedule,
    pub(crate) result: SccClosureResult,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
struct SccClosureDigestCache {
    schema: String,
    scc_digests: Vec<SccClosureDigestCacheEntry>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct SccClosureDigestCacheEntry {
    members: Vec<String>,
    digest: String,
}

/// Runs interprocedural summary closure over SCCs discovered from the current
/// call graph.
///
/// Steps:
/// 1. Compute SCC schedule from current call targets.
/// 2. If no SCCs exist, return empty output with default trace.
/// 3. Create SccClosureConfig, empty previous_scc_digests (cross-run caching
///    is future work), and a DemandQueryEngine.
/// 4. Call close_summaries_by_scc.
/// 5. Return closure result, demand query trace, and any diagnostics.
#[cfg(test)]
pub(crate) fn run_scc_closure(db: &mut AnalysisDb) -> SccClosureProviderOutput {
    run_scc_closure_with_previous_digests(
        db,
        BTreeMap::new(),
        &SccClosureConfig::default(),
        &QueryDependencyInputs::new(Vec::new()),
        &[],
    )
}

pub(crate) fn run_scc_closure_with_cache(
    db: &mut AnalysisDb,
    cache: &Cache,
    input_snapshot: &InputSnapshot,
    direct_summaries_output_digest: &Digest,
    calls_output_digest: &Digest,
) -> SccClosureProviderOutput {
    let config = SccClosureConfig::default();
    run_scc_closure_with_cache_config(
        db,
        cache,
        input_snapshot,
        direct_summaries_output_digest,
        calls_output_digest,
        &config,
    )
}

fn run_scc_closure_with_cache_config(
    db: &mut AnalysisDb,
    cache: &Cache,
    input_snapshot: &InputSnapshot,
    direct_summaries_output_digest: &Digest,
    calls_output_digest: &Digest,
    config: &SccClosureConfig,
) -> SccClosureProviderOutput {
    let analysis_identity = scc_closure_analysis_identity(
        input_snapshot,
        config,
        direct_summaries_output_digest,
        calls_output_digest,
    );
    let cache_key = scc_closure_cache_key(&analysis_identity);
    let previous_scc_digests = cache
        .read_json_with_status::<SccClosureDigestCache>(&cache_key)
        .value
        .filter(|entry| entry.schema == SCC_CLOSURE_CACHE_SCHEMA)
        .map(|entry| {
            entry
                .scc_digests
                .into_iter()
                .map(|entry| (entry.members, entry.digest))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    let query_dependency_inputs = scc_closure_query_dependency_inputs(input_snapshot);
    let query_layer_digests = [
        direct_summaries_output_digest.clone(),
        calls_output_digest.clone(),
    ];
    let output = run_scc_closure_with_previous_digests(
        db,
        previous_scc_digests,
        config,
        &query_dependency_inputs,
        &query_layer_digests,
    );

    let scc_digests = output.scc_output_digests.clone();

    if !scc_digests.is_empty() {
        let _ = cache.write_json_with_status(
            &cache_key,
            &SccClosureDigestCache {
                schema: SCC_CLOSURE_CACHE_SCHEMA.to_string(),
                scc_digests: scc_digests
                    .into_iter()
                    .map(|(members, digest)| SccClosureDigestCacheEntry { members, digest })
                    .collect(),
            },
        );
    }

    output
}

fn run_scc_closure_with_previous_digests(
    db: &mut AnalysisDb,
    previous_scc_digests: BTreeMap<Vec<String>, String>,
    config: &SccClosureConfig,
    query_dependency_inputs: &QueryDependencyInputs,
    query_layer_digests: &[Digest],
) -> SccClosureProviderOutput {
    let schedule = compute_scc_schedule(db);

    if schedule.sccs.is_empty() {
        return SccClosureProviderOutput::default();
    }

    let mut demand_engine = DemandQueryEngine::default();

    let closure_result = close_summaries_by_scc(
        db,
        &schedule,
        config,
        &mut demand_engine,
        &previous_scc_digests,
        query_dependency_inputs,
        query_layer_digests,
    );
    #[cfg(test)]
    let debug_snapshot = Some(SccClosureDebugSnapshot {
        schedule: schedule.clone(),
        result: closure_result.clone(),
    });

    // Generate diagnostics for budget-exceeded SCCs
    let mut diagnostics = Vec::new();
    if closure_result.budget_exceeded_sccs > 0 {
        for (members, iterations) in &closure_result.scc_iteration_counts {
            if *iterations >= config.max_iterations {
                diagnostics.push(Diagnostic::warning(
                    "internal/scc-closure",
                    members.join(", "),
                    crate::diagnostics::TextRange::point(0, 0),
                    format!(
                        "SCC fixpoint did not converge within {} iterations for SCC with members: {}",
                        config.max_iterations,
                        members.join(", ")
                    ),
                ));
            }
        }
    }

    let trace = demand_engine.into_trace();
    let scc_output_digests = closure_result.scc_output_digests;

    SccClosureProviderOutput {
        diagnostics,
        demand_query_trace: trace,
        scc_output_digests,
        #[cfg(test)]
        debug_snapshot,
    }
}

fn scc_closure_query_dependency_inputs(input_snapshot: &InputSnapshot) -> QueryDependencyInputs {
    let mut inputs = input_snapshot
        .requested_capabilities
        .iter()
        .filter(|capability| REQUESTED_CAPABILITIES.contains(&capability.capability.as_str()))
        .map(|capability| {
            let language = capability
                .language
                .map(crate::core::Language::label)
                .unwrap_or("none");
            let status = match &capability.support_status {
                CapabilitySupportStatus::Supported => InputComponentStatus::Present,
                CapabilitySupportStatus::Unsupported => InputComponentStatus::Unsupported,
                CapabilitySupportStatus::SetupMissing => InputComponentStatus::SetupMissing,
            };
            InputDependencyKey::requested_capability(
                format!("requested_capability:{}:{language}", capability.capability),
                capability.analysis_dependency_digest.clone(),
                status,
            )
            .expect("requested capabilities retain analysis-requirements digests")
        })
        .collect::<Vec<_>>();
    inputs.push(
        InputDependencyKey::analysis_setting(
            AnalysisSettingsScope::DirectSummaries.label(),
            input_snapshot
                .analysis_settings_digest(AnalysisSettingsScope::DirectSummaries)
                .clone(),
            InputComponentStatus::Present,
        )
        .expect("SCC analysis settings retain an analysis-settings digest"),
    );
    QueryDependencyInputs::new(inputs)
}

fn scc_closure_analysis_identity(
    input_snapshot: &InputSnapshot,
    config: &SccClosureConfig,
    direct_summaries_output_digest: &Digest,
    calls_output_digest: &Digest,
) -> Digest {
    let max_iterations = config.max_iterations.to_string();
    let enable_backdating = config.enable_backdating.to_string();
    Digest::from_parts(
        DigestKind::AnalysisSettings,
        "summary_scc_closure_analysis",
        &[
            &input_snapshot
                .analysis_settings_digest(AnalysisSettingsScope::DirectSummaries)
                .to_string(),
            &input_snapshot
                .analysis_requirements_digest_for(REQUESTED_CAPABILITIES)
                .to_string(),
            &format!("max_iterations={max_iterations}"),
            &format!("enable_backdating={enable_backdating}"),
            &format!("query_version={SCC_CLOSURE_QUERY_VERSION}"),
            &format!("direct_summaries={direct_summaries_output_digest}"),
            &format!("calls={calls_output_digest}"),
        ],
    )
}

fn scc_closure_cache_key(analysis_identity: &Digest) -> CacheKey {
    assert_eq!(
        analysis_identity.kind,
        DigestKind::AnalysisSettings,
        "SCC closure cache keys require a typed analysis identity"
    );
    CacheKey::for_file(
        "summary-scc-closure",
        "scc-output-digests",
        &analysis_identity.value,
        "",
        "",
        SCC_CLOSURE_CACHE_SCHEMA,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn direct_summaries_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    cfg_output_digest: &Digest,
    calls_output_digest: &Digest,
    abstract_domains_output_digest: &Digest,
    symbol_graph_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    callable_keys: &std::collections::BTreeMap<MirBodyId, String>,
    output: &SummaryOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!(
            "parameters={}",
            direct_summaries_provider_parameter_digest()
        ),
        format!(
            "analysis_settings={}",
            input_snapshot.analysis_settings_digest(AnalysisSettingsScope::DirectSummaries)
        ),
        format!(
            "requested_capabilities={}",
            input_snapshot.analysis_requirements_digest_for(REQUESTED_CAPABILITIES)
        ),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
        format!("calls={calls_output_digest}"),
        format!("abstract_domains={abstract_domains_output_digest}"),
        format!("symbol_graph={symbol_graph_output_digest}"),
        format!("module_topology={module_topology_output_digest}"),
    ];
    parts.extend(
        upstream_syntax_output_digests
            .iter()
            .map(|digest| format!("upstream_syntax={digest}")),
    );
    parts.extend(output.summaries.iter().map(|row| {
        format!(
            "summary={} callable={} domain={:?} status={:?} precision={:?} provenance={:?} payload={} tito_flows={:?}",
            row.stable_key,
            callable_keys
                .get(&MirBodyId(row.function.0))
                .cloned()
                .unwrap_or_else(|| row.callable_stable_key.clone()),
            row.domain,
            row.status,
            row.precision,
            row.provenance,
            row.payload_digest,
            row.tito_flows,
        )
    }));
    parts.extend(output.events.iter().map(|row| {
        format!(
            "event={} callable={} domain={:?} kind={} status={:?} precision={:?} reason={}",
            row.stable_key,
            callable_keys
                .get(&MirBodyId(row.function.0))
                .cloned()
                .unwrap_or_else(|| row.callable_stable_key.clone()),
            row.domain,
            row.event_kind,
            row.status,
            row.precision,
            row.reason,
        )
    }));
    if output.summaries.is_empty() && output.events.is_empty() {
        parts.push("summaries_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "direct_summaries_output", &refs)
}

pub(crate) fn callable_stable_key_map(
    db: &AnalysisDb,
) -> std::collections::BTreeMap<MirBodyId, String> {
    db.mir_bodies()
        .iter()
        .map(|body| (body.id, body.stable_key.clone()))
        .collect()
}

#[cfg(test)]
mod direct_summaries_provider {
    use super::*;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::AnalysisDb;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn output_identity_uses_only_declared_summary_inputs() {
        let temp = tempdir().expect("tempdir");
        let db = AnalysisDb::new();
        let callable_keys = callable_stable_key_map(&db);
        let output = SummaryOutput::empty();
        let upstream = Digest::absent(DigestKind::ProviderOutput, "upstream");
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.direct_summaries")
            .expect("direct summaries manifest");

        crate::analysis::provider::scoped_identity_test_support::assert_provider_identity(
            temp.path(),
            AnalysisSettingsScope::DirectSummaries,
            true,
            false,
            false,
            |snapshot| {
                direct_summaries_output_digest(
                    manifest,
                    snapshot,
                    &upstream,
                    &upstream,
                    &upstream,
                    &upstream,
                    &upstream,
                    &upstream,
                    &[],
                    &callable_keys,
                    &output,
                )
            },
        );
    }

    #[test]
    fn provider_accepts_empty_output_with_deterministic_digest() {
        let mut db = AnalysisDb::new();
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let empty_plan = AnalysisPlan::empty();
        assert!(empty_plan.requested_capability_snapshots().is_empty());
        let identity_sources = InputSnapshot::identity_sources_from_plan(&loaded, &empty_plan);
        assert!(identity_sources.requested_capabilities.is_empty());
        assert_eq!(
            identity_sources.analysis_requirements_identity,
            Digest::absent(DigestKind::AnalysisRequirements, "requested_capabilities")
        );
        let input_snapshot = InputSnapshot::from_run_inputs_with_plan(
            &loaded,
            &db,
            "config",
            "rules",
            &empty_plan,
            AnalysisKernel::provider_manifests(),
        );
        let output = derive_direct_summaries_with_cache_stats(
            &mut db,
            &input_snapshot,
            AnalysisKernel::provider_manifests()
                .iter()
                .find(|manifest| manifest.id == "polint.direct_summaries")
                .expect("direct summaries manifest should exist"),
            Digest::absent(DigestKind::ProviderOutput, "semantic_mir"),
            Digest::absent(DigestKind::ProviderOutput, "cfg"),
            Digest::absent(DigestKind::ProviderOutput, "calls"),
            Digest::absent(DigestKind::ProviderOutput, "abstract_domains"),
            Digest::absent(DigestKind::ProviderOutput, "symbol_graph"),
            Digest::absent(DigestKind::ProviderOutput, "module_topology"),
            Vec::new(),
        );

        assert!(output.diagnostics.is_empty());
        assert!(output.output_digest.is_some());
        assert_eq!(output.cache_stats.recomputes, 1);

        // Verify determinism
        let mut db2 = AnalysisDb::new();
        let output2 = derive_direct_summaries_with_cache_stats(
            &mut db2,
            &input_snapshot,
            AnalysisKernel::provider_manifests()
                .iter()
                .find(|manifest| manifest.id == "polint.direct_summaries")
                .expect("direct summaries manifest should exist"),
            Digest::absent(DigestKind::ProviderOutput, "semantic_mir"),
            Digest::absent(DigestKind::ProviderOutput, "cfg"),
            Digest::absent(DigestKind::ProviderOutput, "calls"),
            Digest::absent(DigestKind::ProviderOutput, "abstract_domains"),
            Digest::absent(DigestKind::ProviderOutput, "symbol_graph"),
            Digest::absent(DigestKind::ProviderOutput, "module_topology"),
            Vec::new(),
        );
        assert_eq!(output.output_digest, output2.output_digest);
    }
}

#[cfg(test)]
mod direct_summaries_provider_order {
    use crate::analysis_kernel::AnalysisKernel;

    #[test]
    fn direct_summaries_runs_after_abstract_domains_and_before_metrics() {
        let order = AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| manifest.id)
            .collect::<Vec<_>>();
        let calls = order
            .iter()
            .position(|provider| *provider == "polint.calls")
            .expect("calls provider");
        let abstract_domains = order
            .iter()
            .position(|provider| *provider == "polint.abstract_domains")
            .expect("abstract domains provider");
        let direct_summaries = order
            .iter()
            .position(|provider| *provider == "polint.direct_summaries")
            .expect("direct summaries provider");
        let metrics = order
            .iter()
            .position(|provider| *provider == "polint.metrics")
            .expect("metrics provider");

        assert!(calls < abstract_domains);
        assert!(abstract_domains < direct_summaries);
        assert!(direct_summaries < metrics);
    }
}

#[cfg(test)]
mod scc_closure_provider {
    use super::*;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
        CallSyntaxKind, CallTargetFact, CallTargetStatus,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId, SummaryId};
    use crate::analysis::summaries::facts::{
        SummaryDomainKind, SummaryFact, SummaryPrecision, SummaryProvenance, SummaryStatus,
    };
    use crate::analysis::summaries::store::SummaryOutput;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{
        DemandCacheStatus, InputDependencyKind, InputSnapshot,
    };
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::cache::keys::config_hash;
    use crate::config::{LoadedConfig, RuleConfig, load_config};
    use crate::core::{FileId, FunctionId, Language, Span};

    fn span() -> Span {
        Span::point(FileId(1), 1, 1)
    }

    fn summary_fact(function_id: u64, callable_key: &str) -> SummaryFact {
        SummaryFact {
            id: SummaryId(0),
            callable_stable_key: callable_key.to_string(),
            function: FunctionId(function_id),
            domain: SummaryDomainKind::ControlEffects,
            status: SummaryStatus::Present,
            precision: SummaryPrecision::Local,
            provenance: SummaryProvenance::NativeLocal,
            payload_digest: format!("digest:{callable_key}"),
            tito_flows: Vec::new(),
            stable_key: format!("summary:control_effects:{callable_key}"),
        }
    }

    fn call_site(id: u64, caller: u64) -> CallSiteFact {
        CallSiteFact {
            in_throw: false,
            id: CallSiteId(id),
            language: Language::TypeScript,
            file: FileId(1),
            caller: FunctionId(caller),
            owner_symbol: None,
            body: MirBodyId(caller),
            operation: MirOpId(id),
            span: span(),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: None,
                name: format!("call_{id}"),
            },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::Exact,
            stable_key: format!("site:{id}"),
        }
    }

    fn call_target(id: u64, site_id: u64, caller: u64, target_func: u64) -> CallTargetFact {
        CallTargetFact {
            id: CallTargetId(id),
            site: CallSiteId(site_id),
            caller: FunctionId(caller),
            target_function: Some(FunctionId(target_func)),
            target_symbol: None,
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::Exact,
            stable_key: format!("target:{id}"),
        }
    }

    fn closure_db(
        summaries: Vec<SummaryFact>,
        sites: Vec<CallSiteFact>,
        targets: Vec<CallTargetFact>,
    ) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        db.replace_summary_facts(SummaryOutput {
            summaries,
            events: Vec::new(),
        });
        db.replace_call_facts(CallOutput {
            sites,
            targets,
            unresolved: Vec::new(),
        })
        .expect("call output should be valid");
        db
    }

    fn snapshot(loaded: &LoadedConfig, db: &AnalysisDb, plan: &AnalysisPlan) -> InputSnapshot {
        let config_digest = config_hash(loaded);
        InputSnapshot::from_run_inputs_with_plan(
            loaded,
            db,
            &config_digest,
            "rule-digest",
            plan,
            AnalysisKernel::provider_manifests(),
        )
    }

    #[test]
    fn scc_query_dependencies_select_only_declared_snapshot_rows() {
        let temp = tempfile::tempdir().expect("tempdir");
        let loaded = load_config(temp.path()).expect("config loads");
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls"]);
        let baseline_snapshot = snapshot(&loaded, &AnalysisDb::new(), &plan);
        let baseline = scc_closure_query_dependency_inputs(&baseline_snapshot);

        assert_eq!(baseline.as_slice().len(), 2);
        assert!(baseline.as_slice().iter().any(|input| {
            input.kind == InputDependencyKind::RequestedCapability
                && input.stable_key == "requested_capability:calls:none"
        }));
        assert!(baseline.as_slice().iter().any(|input| {
            input.kind == InputDependencyKind::AnalysisSetting
                && input.stable_key == "polint.direct_summaries"
        }));
        assert!(baseline.as_slice().iter().all(|input| {
            !matches!(
                input.kind,
                InputDependencyKind::Model
                    | InputDependencyKind::ExtensionCode
                    | InputDependencyKind::ExtensionDeclaredInput
            )
        }));

        let mut unreferenced_changed = baseline_snapshot.clone();
        unreferenced_changed.models[0].digest =
            Digest::from_parts(DigestKind::ModelFile, "unreferenced_model", &["changed"]);
        unreferenced_changed.extensions[0].digest = Digest::from_parts(
            DigestKind::ExtensionCode,
            "unreferenced_extension",
            &["changed"],
        );
        assert_eq!(
            scc_closure_query_dependency_inputs(&unreferenced_changed),
            baseline
        );

        let mut capability_changed = baseline_snapshot.clone();
        capability_changed.requested_capabilities[0].analysis_dependency_digest =
            Digest::from_parts(
                DigestKind::AnalysisRequirements,
                "requested_capability",
                &["changed"],
            );
        assert_ne!(
            scc_closure_query_dependency_inputs(&capability_changed),
            baseline
        );

        let mut setting_changed = baseline_snapshot;
        setting_changed
            .analysis_settings
            .iter_mut()
            .find(|setting| setting.scope == AnalysisSettingsScope::DirectSummaries)
            .expect("direct summaries settings row")
            .digest = Digest::from_parts(
            DigestKind::AnalysisSettings,
            "summary_settings",
            &["changed"],
        );
        assert_ne!(
            scc_closure_query_dependency_inputs(&setting_changed),
            baseline
        );
    }

    #[test]
    fn scc_closure_processes_chain_in_correct_order_and_records_demand_trace() {
        // A calls B. Both have direct summaries.
        // SCC closure should process B first (leaf callee), then A.
        let summaries = vec![summary_fact(1, "func::a"), summary_fact(2, "func::b")];
        let sites = vec![call_site(1, 1)]; // A calls something
        let targets = vec![call_target(1, 1, 1, 2)]; // A -> B

        let mut db = AnalysisDb::new();
        db.replace_summary_facts(SummaryOutput {
            summaries,
            events: Vec::new(),
        });
        db.replace_call_facts(CallOutput {
            sites,
            targets,
            unresolved: Vec::new(),
        })
        .expect("call output should be valid");

        let output = run_scc_closure(&mut db);

        // Should have processed 2 SCCs (B and A, both non-recursive)
        let result = output
            .debug_snapshot
            .expect("closure debug snapshot should be present")
            .result;
        assert_eq!(result.total_sccs_processed, 2);
        assert_eq!(result.non_recursive_sccs, 2);
        assert_eq!(result.recursive_sccs, 0);

        // Demand query trace should have entries for each SCC
        assert_eq!(
            output.demand_query_trace.len(),
            2,
            "demand trace should have one entry per SCC"
        );

        // All entries should be scc_closure queries
        for entry in output.demand_query_trace.entries() {
            assert_eq!(entry.query_key.query_kind, "scc_closure");
        }

        // No diagnostics for non-recursive SCCs
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn scc_closure_skips_empty_schedule() {
        let mut db = AnalysisDb::new();

        let output = run_scc_closure(&mut db);

        assert!(output.debug_snapshot.is_none());
        assert!(output.demand_query_trace.is_empty());
        assert!(output.diagnostics.is_empty());
    }

    #[test]
    fn scc_closure_with_cache_backdates_warm_run_and_records_hit_trace() {
        let temp = tempfile::tempdir().expect("tempdir");
        let cache = Cache::default_for_repo(temp.path(), true);
        let loaded = load_config(temp.path()).expect("config loads");
        let plan = AnalysisPlan::from_capability_names_for_test(&["calls"]);
        let summaries = vec![summary_fact(1, "func::a"), summary_fact(2, "func::b")];
        let sites = vec![call_site(1, 1)];
        let targets = vec![call_target(1, 1, 1, 2)];
        let direct_summaries_digest =
            Digest::from_parts(DigestKind::ProviderOutput, "direct_summaries", &["base"]);
        let calls_digest = Digest::from_parts(DigestKind::ProviderOutput, "calls", &["base"]);

        let mut cold_db = closure_db(summaries.clone(), sites.clone(), targets.clone());
        let input_snapshot = snapshot(&loaded, &cold_db, &plan);
        let cold = run_scc_closure_with_cache(
            &mut cold_db,
            &cache,
            &input_snapshot,
            &direct_summaries_digest,
            &calls_digest,
        );
        assert_eq!(
            cold.debug_snapshot
                .as_ref()
                .expect("cold snapshot")
                .result
                .backdated_sccs,
            0
        );
        let expected_query_inputs = scc_closure_query_dependency_inputs(&input_snapshot);
        for entry in cold.demand_query_trace.entries() {
            assert_eq!(entry.query_key.dependency_inputs, expected_query_inputs);
            assert_eq!(entry.query_key.layer_digests.len(), 2);
            assert!(
                entry
                    .query_key
                    .layer_digests
                    .contains(&direct_summaries_digest)
            );
            assert!(entry.query_key.layer_digests.contains(&calls_digest));
        }

        let mut warm_db = closure_db(summaries.clone(), sites.clone(), targets.clone());
        let warm = run_scc_closure_with_cache(
            &mut warm_db,
            &cache,
            &input_snapshot,
            &direct_summaries_digest,
            &calls_digest,
        );

        assert!(
            warm.debug_snapshot
                .as_ref()
                .expect("warm snapshot")
                .result
                .backdated_sccs
                > 0,
            "warm SCC closure should backdate from persisted SCC digests"
        );
        assert!(
            warm.demand_query_trace
                .entries()
                .iter()
                .any(|entry| entry.cache_status == DemandCacheStatus::Hit),
            "warm SCC closure should record hit trace rows: {:#?}",
            warm.demand_query_trace
        );

        let mut rule_loaded = loaded;
        rule_loaded.config.rules.config.push(RuleConfig {
            id: "local/scc-identity".to_string(),
            severity: None,
            files: Vec::new(),
            allow_files: Vec::new(),
            allow: Vec::new(),
            max: None,
            deny: Vec::new(),
            forbidden_imports: Default::default(),
            settings: std::collections::BTreeMap::from([(
                "threshold".to_string(),
                toml::Value::Integer(7),
            )]),
        });
        let rule_snapshot = snapshot(&rule_loaded, &warm_db, &plan);
        assert_ne!(
            input_snapshot.config_identity,
            rule_snapshot.config_identity
        );
        let mut rule_only_db = closure_db(summaries.clone(), sites.clone(), targets.clone());
        let rule_only = run_scc_closure_with_cache(
            &mut rule_only_db,
            &cache,
            &rule_snapshot,
            &direct_summaries_digest,
            &calls_digest,
        );
        assert!(
            rule_only
                .debug_snapshot
                .as_ref()
                .expect("rule-only snapshot")
                .result
                .backdated_sccs
                > 0
        );

        let mut budget_db = closure_db(summaries.clone(), sites.clone(), targets.clone());
        let changed_budget = SccClosureConfig {
            max_iterations: SccClosureConfig::default().max_iterations + 1,
            ..SccClosureConfig::default()
        };
        let budget_changed = run_scc_closure_with_cache_config(
            &mut budget_db,
            &cache,
            &input_snapshot,
            &direct_summaries_digest,
            &calls_digest,
            &changed_budget,
        );
        assert_eq!(
            budget_changed
                .debug_snapshot
                .as_ref()
                .expect("budget snapshot")
                .result
                .backdated_sccs,
            0
        );

        let mut upstream_db = closure_db(summaries, sites, targets);
        let changed_direct_summaries =
            Digest::from_parts(DigestKind::ProviderOutput, "direct_summaries", &["changed"]);
        let upstream_changed = run_scc_closure_with_cache(
            &mut upstream_db,
            &cache,
            &input_snapshot,
            &changed_direct_summaries,
            &calls_digest,
        );
        assert_eq!(
            upstream_changed
                .debug_snapshot
                .as_ref()
                .expect("upstream snapshot")
                .result
                .backdated_sccs,
            0
        );
    }
}
