use serde::Serialize;
use std::fmt::Debug;

use super::cache_key::{
    evidence_provider_parameter_digest, evidence_provider_parameter_digest_for_snapshot,
};
use super::facts::{
    EvidenceConfidence, EvidenceEdgeFact, EvidenceEdgeKind, EvidenceExpansion, EvidenceNodeFact,
    EvidenceNodeKind, EvidencePrecision, EvidenceProvenance, EvidenceQueryMode, EvidenceStatus,
    EvidenceValidation,
};
use super::store::EvidenceOutput;
use crate::analysis::cfg::facts::{CfgPrecision, CfgStatus};
use crate::analysis::data_flow::facts::{
    DataFlowConfidence, DataFlowEdgeFact, DataFlowEdgeKind, DataFlowNodeFact, DataFlowNodeKind,
    DataFlowPrecision, DataFlowProvenance, DataFlowStatus, DataFlowValidation,
};
use crate::analysis::ids::{EvidenceEdgeId, EvidenceNodeId};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};
use crate::core::AnalysisDb;
use crate::diagnostics::Diagnostic;

pub(crate) const EVIDENCE_PROVIDER_ID: &str = "polint.evidence";

#[derive(Debug, Clone, Default)]
pub(crate) struct EvidenceProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_evidence_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    calls_output_digest: Digest,
    refined_calls_output_digest: Digest,
    direct_summaries_output_digest: Digest,
    type_value_alias_output_digest: Digest,
    entrypoints_output_digest: Digest,
    extensions_output_digest: Digest,
    data_flow_output_digest: Digest,
) -> EvidenceProviderOutput {
    debug_assert_eq!(manifest.id, EVIDENCE_PROVIDER_ID);
    let mut output = EvidenceOutput::empty();
    derive_data_flow_evidence(db, &mut output);
    derive_control_dependence_evidence(db, &mut output);
    let output = output.normalized();
    let output_digest = evidence_output_digest(
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &cfg_output_digest,
        &calls_output_digest,
        &refined_calls_output_digest,
        &direct_summaries_output_digest,
        &type_value_alias_output_digest,
        &entrypoints_output_digest,
        &extensions_output_digest,
        &data_flow_output_digest,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_evidence_facts(output) {
        Ok(()) => EvidenceProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => EvidenceProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: None,
        },
    }
}

fn derive_data_flow_evidence(db: &AnalysisDb, output: &mut EvidenceOutput) {
    let mut node_map = std::collections::BTreeMap::new();
    for node in db.data_flow_nodes() {
        let evidence_id = EvidenceNodeId(output.nodes.len() as u64);
        node_map.insert(node.id, evidence_id);
        output
            .nodes
            .push(evidence_node_from_data_flow(node, evidence_id));
    }

    for edge in db.data_flow_edges() {
        let (Some(from), Some(to)) = (node_map.get(&edge.from), node_map.get(&edge.to)) else {
            continue;
        };
        output.edges.push(evidence_edge_from_data_flow(
            edge,
            EvidenceEdgeId(output.edges.len() as u64),
            *from,
            *to,
        ));
    }
}

fn evidence_node_from_data_flow(node: &DataFlowNodeFact, id: EvidenceNodeId) -> EvidenceNodeFact {
    EvidenceNodeFact {
        id,
        kind: data_flow_node_kind(node),
        language: node.language,
        file: node.file,
        function: node.function,
        body: node.body,
        operation: node.operation,
        cfg_node: node.cfg_node,
        place: node.place,
        symbol: node.symbol,
        reference: node.reference,
        call_site: node.call_site,
        span: node.span.clone(),
        status: EvidenceStatus::Present,
        precision: EvidencePrecision::Syntax,
        provenance: EvidenceProvenance::Native,
        validation: EvidenceValidation::ReferentiallyValidated,
        confidence: EvidenceConfidence::High,
        compact_label: Some(format!("{:?}", node.kind)),
        source_fact_stable_keys: vec![node.stable_key.clone()],
        stable_key: stable_key_from_parts(
            FactFamily::EvidenceNode,
            &[("data_flow_node", node.stable_key.clone())],
        ),
    }
}

fn evidence_edge_from_data_flow(
    edge: &DataFlowEdgeFact,
    id: EvidenceEdgeId,
    from: EvidenceNodeId,
    to: EvidenceNodeId,
) -> EvidenceEdgeFact {
    let summary_stable_key = summary_source_key(edge);
    EvidenceEdgeFact {
        id,
        from,
        to,
        kind: data_flow_edge_kind(edge.kind),
        query_mode: EvidenceQueryMode::ThinBackward,
        status: data_flow_status(edge.status),
        precision: data_flow_precision(edge.precision),
        provenance: data_flow_provenance(edge.provenance),
        validation: data_flow_validation(edge.validation),
        confidence: data_flow_confidence(edge.confidence),
        call_site: edge.call_site,
        summary_stable_key: summary_stable_key.clone(),
        expansion: evidence_expansion(edge, summary_stable_key.as_deref()),
        compact_label: Some(format!("{:?}", edge.kind)),
        source_fact_stable_keys: std::iter::once(edge.stable_key.clone())
            .chain(edge.input_stable_keys.iter().cloned())
            .collect(),
        stable_key: stable_key_from_parts(
            FactFamily::EvidenceEdge,
            &[("data_flow_edge", edge.stable_key.clone())],
        ),
    }
}

fn derive_control_dependence_evidence(db: &AnalysisDb, output: &mut EvidenceOutput) {
    for dependence in db.cfg_control_dependence() {
        let Some(controlling_edge) = db
            .cfg_edges()
            .iter()
            .find(|edge| edge.id == dependence.controlling_edge)
        else {
            continue;
        };
        let from = EvidenceNodeId(output.nodes.len() as u64);
        let controlled_node = db
            .cfg_blocks()
            .iter()
            .find(|block| block.id == dependence.controlled_block)
            .and_then(|block| block.first_node.or(block.last_node));
        output.nodes.push(EvidenceNodeFact {
            id: from,
            kind: EvidenceNodeKind::Statement,
            language: db
                .cfg_functions()
                .iter()
                .find(|function| function.id == dependence.cfg_function)
                .map(|function| function.language)
                .unwrap_or(crate::core::Language::Unknown),
            file: db
                .cfg_functions()
                .iter()
                .find(|function| function.id == dependence.cfg_function)
                .map(|function| function.file),
            function: db
                .cfg_functions()
                .iter()
                .find(|function| function.id == dependence.cfg_function)
                .map(|function| function.function),
            body: None,
            operation: None,
            cfg_node: Some(controlling_edge.from),
            place: None,
            symbol: None,
            reference: None,
            call_site: None,
            span: None,
            status: cfg_status(dependence.status),
            precision: cfg_precision(dependence.precision),
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::ReferentiallyValidated,
            confidence: EvidenceConfidence::High,
            compact_label: Some(format!("control:{:?}", dependence.controlling_edge_kind)),
            source_fact_stable_keys: vec![
                dependence.stable_key.clone(),
                controlling_edge.stable_key.clone(),
            ],
            stable_key: stable_key_from_parts(
                FactFamily::EvidenceNode,
                &[
                    ("control_dependence", dependence.stable_key.clone()),
                    ("role", "controller".to_string()),
                ],
            ),
        });
        let to = EvidenceNodeId(output.nodes.len() as u64);
        output.nodes.push(EvidenceNodeFact {
            id: to,
            kind: EvidenceNodeKind::Synthetic,
            language: crate::core::Language::Unknown,
            file: None,
            function: None,
            body: None,
            operation: None,
            cfg_node: controlled_node,
            place: None,
            symbol: None,
            reference: None,
            call_site: None,
            span: None,
            status: cfg_status(dependence.status),
            precision: cfg_precision(dependence.precision),
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::ReferentiallyValidated,
            confidence: EvidenceConfidence::High,
            compact_label: Some("controlled_block".to_string()),
            source_fact_stable_keys: vec![dependence.stable_key.clone()],
            stable_key: stable_key_from_parts(
                FactFamily::EvidenceNode,
                &[
                    ("control_dependence", dependence.stable_key.clone()),
                    ("role", "controlled".to_string()),
                ],
            ),
        });
        output.edges.push(EvidenceEdgeFact {
            id: EvidenceEdgeId(output.edges.len() as u64),
            from,
            to,
            kind: EvidenceEdgeKind::Control,
            query_mode: EvidenceQueryMode::FullBackward,
            status: cfg_status(dependence.status),
            precision: cfg_precision(dependence.precision),
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::ReferentiallyValidated,
            confidence: EvidenceConfidence::High,
            call_site: None,
            summary_stable_key: None,
            expansion: EvidenceExpansion::None,
            compact_label: Some(format!("{:?}", dependence.controlling_edge_kind)),
            source_fact_stable_keys: vec![
                dependence.stable_key.clone(),
                controlling_edge.stable_key.clone(),
            ],
            stable_key: stable_key_from_parts(
                FactFamily::EvidenceEdge,
                &[("control_dependence", dependence.stable_key.clone())],
            ),
        });
    }
}

fn data_flow_node_kind(node: &DataFlowNodeFact) -> EvidenceNodeKind {
    if node.operation.is_some() {
        EvidenceNodeKind::Operation
    } else if node.place.is_some() {
        EvidenceNodeKind::Place
    } else if node.call_site.is_some() {
        EvidenceNodeKind::CallSite
    } else if node.model.is_some()
        || matches!(
            node.kind,
            DataFlowNodeKind::Source
                | DataFlowNodeKind::Sink
                | DataFlowNodeKind::Sanitizer
                | DataFlowNodeKind::Barrier
        )
    {
        EvidenceNodeKind::Model
    } else {
        EvidenceNodeKind::Synthetic
    }
}

fn data_flow_edge_kind(kind: DataFlowEdgeKind) -> EvidenceEdgeKind {
    match kind {
        DataFlowEdgeKind::LocalRead
        | DataFlowEdgeKind::LocalBinding
        | DataFlowEdgeKind::LocalAssignment
        | DataFlowEdgeKind::LocalUse
        | DataFlowEdgeKind::LocalWrite
        | DataFlowEdgeKind::ReturnValue
        | DataFlowEdgeKind::CallArgumentToReturn
        | DataFlowEdgeKind::SourceIntroduction => EvidenceEdgeKind::DataValue,
        DataFlowEdgeKind::CallArgumentToParameter => EvidenceEdgeKind::ParameterIn,
        DataFlowEdgeKind::CallReturnToUse => EvidenceEdgeKind::ParameterOut,
        DataFlowEdgeKind::ReceiverToMethod => EvidenceEdgeKind::Call,
        DataFlowEdgeKind::FieldProjection
        | DataFlowEdgeKind::IndexProjection
        | DataFlowEdgeKind::Dereference
        | DataFlowEdgeKind::AddressOf => EvidenceEdgeKind::DataAddress,
        DataFlowEdgeKind::SummaryTito | DataFlowEdgeKind::SummaryProjected => {
            EvidenceEdgeKind::Summary
        }
        DataFlowEdgeKind::Model => EvidenceEdgeKind::Model,
        DataFlowEdgeKind::Sanitizer | DataFlowEdgeKind::Barrier => {
            EvidenceEdgeKind::ExplanationOnly
        }
        DataFlowEdgeKind::UnknownFlow
        | DataFlowEdgeKind::HavocFlow
        | DataFlowEdgeKind::BudgetTruncated => EvidenceEdgeKind::Unknown,
    }
}

fn summary_source_key(edge: &DataFlowEdgeFact) -> Option<String> {
    edge.input_stable_keys
        .iter()
        .find(|key| is_summary_key(key))
        .cloned()
}

fn is_summary_key(key: &str) -> bool {
    key.starts_with("summary:")
        || key.contains("SummaryTito")
        || key.contains("SummaryControl")
        || key.contains("SummaryCall")
        || key.contains("SummaryMemory")
        || key.contains("SummaryEvent")
}

fn evidence_expansion(
    edge: &DataFlowEdgeFact,
    summary_stable_key: Option<&str>,
) -> EvidenceExpansion {
    let Some(summary_stable_key) = summary_stable_key else {
        return EvidenceExpansion::None;
    };
    if edge.status == DataFlowStatus::Present
        && matches!(
            edge.kind,
            DataFlowEdgeKind::SummaryTito | DataFlowEdgeKind::SummaryProjected
        )
    {
        EvidenceExpansion::Expandable {
            key: format!("evidence:expand:{summary_stable_key}"),
        }
    } else if edge.provenance == DataFlowProvenance::Model {
        EvidenceExpansion::ExternalModel {
            model: edge
                .model
                .map(|model| format!("data_flow_model:{}", model.0))
                .unwrap_or_else(|| "external_model".to_string()),
        }
    } else {
        EvidenceExpansion::Opaque {
            reason: summary_opaque_reason(edge),
        }
    }
}

fn summary_opaque_reason(edge: &DataFlowEdgeFact) -> String {
    edge.evidence
        .iter()
        .find_map(|entry| entry.strip_prefix("reason=").map(str::to_string))
        .unwrap_or_else(|| format!("summary_status={:?}", edge.status))
}

fn data_flow_status(status: DataFlowStatus) -> EvidenceStatus {
    match status {
        DataFlowStatus::Present => EvidenceStatus::Present,
        DataFlowStatus::Unknown => EvidenceStatus::Unknown,
        DataFlowStatus::Unsupported => EvidenceStatus::Unsupported,
        DataFlowStatus::SetupMissing => EvidenceStatus::SetupMissing,
        DataFlowStatus::BudgetExceeded => EvidenceStatus::BudgetExceeded,
        DataFlowStatus::Rejected => EvidenceStatus::Rejected,
    }
}

fn data_flow_precision(precision: DataFlowPrecision) -> EvidencePrecision {
    match precision {
        DataFlowPrecision::Exact => EvidencePrecision::Exact,
        DataFlowPrecision::SetupAware => EvidencePrecision::SetupAware,
        DataFlowPrecision::Syntax => EvidencePrecision::Syntax,
        DataFlowPrecision::Conservative => EvidencePrecision::Conservative,
        DataFlowPrecision::Heuristic => EvidencePrecision::Heuristic,
        DataFlowPrecision::Unknown => EvidencePrecision::Unknown,
    }
}

fn data_flow_provenance(provenance: DataFlowProvenance) -> EvidenceProvenance {
    match provenance {
        DataFlowProvenance::Native => EvidenceProvenance::Native,
        DataFlowProvenance::Summary => EvidenceProvenance::Summary,
        DataFlowProvenance::Extension => EvidenceProvenance::Extension,
        DataFlowProvenance::Model => EvidenceProvenance::Model,
        DataFlowProvenance::Query => EvidenceProvenance::Query,
    }
}

fn data_flow_validation(validation: DataFlowValidation) -> EvidenceValidation {
    match validation {
        DataFlowValidation::Native => EvidenceValidation::Native,
        DataFlowValidation::ReferentiallyValidated => EvidenceValidation::ReferentiallyValidated,
        DataFlowValidation::ExtensionValidated => EvidenceValidation::ExtensionValidated,
        DataFlowValidation::BudgetValidated => EvidenceValidation::BudgetValidated,
        DataFlowValidation::Rejected => EvidenceValidation::Rejected,
    }
}

fn data_flow_confidence(confidence: DataFlowConfidence) -> EvidenceConfidence {
    match confidence {
        DataFlowConfidence::High => EvidenceConfidence::High,
        DataFlowConfidence::Medium => EvidenceConfidence::Medium,
        DataFlowConfidence::Low => EvidenceConfidence::Low,
    }
}

fn cfg_status(status: CfgStatus) -> EvidenceStatus {
    match status {
        CfgStatus::Resolved => EvidenceStatus::Present,
        CfgStatus::Partial => EvidenceStatus::Partial,
        CfgStatus::Unknown => EvidenceStatus::Unknown,
        CfgStatus::Unsupported => EvidenceStatus::Unsupported,
    }
}

fn cfg_precision(precision: CfgPrecision) -> EvidencePrecision {
    match precision {
        CfgPrecision::ExactSyntax | CfgPrecision::ExactLowered => EvidencePrecision::Exact,
        CfgPrecision::SetupAware => EvidencePrecision::SetupAware,
        CfgPrecision::Conservative => EvidencePrecision::Conservative,
        CfgPrecision::Heuristic => EvidencePrecision::Heuristic,
        CfgPrecision::Unknown => EvidencePrecision::Unknown,
        CfgPrecision::Unsupported => EvidencePrecision::Unknown,
    }
}

#[allow(clippy::too_many_arguments)]
fn evidence_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    cfg_output_digest: &Digest,
    calls_output_digest: &Digest,
    refined_calls_output_digest: &Digest,
    direct_summaries_output_digest: &Digest,
    type_value_alias_output_digest: &Digest,
    entrypoints_output_digest: &Digest,
    extensions_output_digest: &Digest,
    data_flow_output_digest: &Digest,
    output: &EvidenceOutput,
) -> Digest {
    let upstream = vec![
        semantic_mir_output_digest.clone(),
        cfg_output_digest.clone(),
        calls_output_digest.clone(),
        refined_calls_output_digest.clone(),
        direct_summaries_output_digest.clone(),
        type_value_alias_output_digest.clone(),
        entrypoints_output_digest.clone(),
        extensions_output_digest.clone(),
        data_flow_output_digest.clone(),
    ];
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", evidence_provider_parameter_digest()),
        format!(
            "input_parameters={}",
            evidence_provider_parameter_digest_for_snapshot(input_snapshot, &upstream)
        ),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
        format!("calls={calls_output_digest}"),
        format!("refined_calls={refined_calls_output_digest}"),
        format!("direct_summaries={direct_summaries_output_digest}"),
        format!("type_value_alias={type_value_alias_output_digest}"),
        format!("entrypoints={entrypoints_output_digest}"),
        format!("extensions={extensions_output_digest}"),
        format!("data_flow={data_flow_output_digest}"),
    ];
    extend_component_parts(
        &mut parts,
        "go_lifecycle",
        &input_snapshot.go_lifecycle.components,
    );
    extend_component_parts(
        &mut parts,
        "ts_js_lifecycle",
        &input_snapshot.ts_js_lifecycle.components,
    );
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);
    parts.extend(
        output
            .nodes
            .iter()
            .map(|node| format!("evidence_node={}", stable_fact_payload(node))),
    );
    parts.extend(
        output
            .edges
            .iter()
            .map(|edge| format!("evidence_edge={}", stable_fact_payload(edge))),
    );
    parts.extend(
        output
            .bundles
            .iter()
            .map(|bundle| format!("evidence_bundle={}", stable_fact_payload(bundle))),
    );
    parts.extend(
        output
            .paths
            .iter()
            .map(|path| format!("evidence_path={}", stable_fact_payload(path))),
    );
    parts.extend(
        output
            .slices
            .iter()
            .map(|slice| format!("evidence_slice={}", stable_fact_payload(slice))),
    );
    parts.extend(
        output
            .unknowns
            .iter()
            .map(|unknown| format!("evidence_unknown={}", stable_fact_payload(unknown))),
    );
    parts.extend(
        output
            .omitted_regions
            .iter()
            .map(|omitted| format!("evidence_omitted_region={}", stable_fact_payload(omitted))),
    );
    parts.extend(
        output
            .replay_keys
            .iter()
            .map(|replay| format!("evidence_replay_key={}", stable_fact_payload(replay))),
    );
    if output.nodes.is_empty()
        && output.edges.is_empty()
        && output.bundles.is_empty()
        && output.paths.is_empty()
        && output.slices.is_empty()
        && output.unknowns.is_empty()
        && output.omitted_regions.is_empty()
        && output.replay_keys.is_empty()
    {
        parts.push("evidence_output=empty".to_string());
    }
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "evidence_output", &refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    if components.is_empty() {
        parts.push(format!("{prefix}=absent"));
        return;
    }
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

fn stable_fact_payload<T>(fact: &T) -> String
where
    T: Serialize + Debug,
{
    serde_json::to_string(fact).unwrap_or_else(|_| format!("{fact:?}"))
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        crate::diagnostics::TextRange::point(1, 1),
        format!("Evidence provider failed: {message}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::facts::{
        BasicBlockFact, BasicBlockKind, CfgEdgeFact, CfgEdgeKind, CfgFunctionFact, CfgNodeFact,
        CfgNodeKind, CfgPrecision, CfgStatus, CfgView, ControlDependenceFact,
    };
    use crate::analysis::cfg::ids::{
        BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId, ControlDependenceId,
    };
    use crate::analysis::cfg::store::CfgOutput;
    use crate::analysis::data_flow::facts::{
        DataFlowAlgorithm, DataFlowConfidence, DataFlowEdgeFact, DataFlowEdgeKind,
        DataFlowNodeFact, DataFlowNodeKind, DataFlowPrecision, DataFlowProvenance, DataFlowStatus,
        DataFlowValidation,
    };
    use crate::analysis::data_flow::store::DataFlowOutput;
    use crate::analysis::evidence::facts::{
        EvidenceBundleFact, EvidenceOmittedReason, EvidenceOmittedRegionFact, EvidencePathFact,
        EvidenceQueryBudget, EvidenceRankScore, EvidenceRankingMode, EvidenceRendererMode,
        EvidenceReplayKeyFact, EvidenceSliceFact, EvidenceUnknownFact, EvidenceUnknownReason,
    };
    use crate::analysis::ids::{DataFlowEdgeId, DataFlowNodeId, MirBodyId, PlaceId};
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{
        GoLifecycleSnapshot, InputComponent, InputComponentStatus, InputSnapshot,
        TsJsLifecycleSnapshot,
    };
    use crate::core::{FileId, FunctionId, Language, Span};

    #[test]
    fn evidence_runs_after_data_flow_and_before_metrics() {
        let order = AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| manifest.id)
            .collect::<Vec<_>>();
        let data_flow = order
            .iter()
            .position(|provider| *provider == "polint.data_flow")
            .expect("data-flow provider");
        let evidence = order
            .iter()
            .position(|provider| *provider == EVIDENCE_PROVIDER_ID)
            .expect("evidence provider");
        let metrics = order
            .iter()
            .position(|provider| *provider == "polint.metrics")
            .expect("metrics provider");

        assert!(data_flow < evidence);
        assert!(evidence < metrics);
    }

    #[test]
    fn data_flow_rows_become_evidence_nodes_and_value_edges() {
        let mut db = AnalysisDb::new();
        db.replace_data_flow_facts(DataFlowOutput {
            nodes: vec![data_flow_node(0, "df:source"), data_flow_node(1, "df:sink")],
            edges: vec![data_flow_edge(0, "df:edge:value")],
            models: Vec::new(),
            budgets: Vec::new(),
        })
        .expect("valid data-flow output");
        let mut output = EvidenceOutput::empty();

        derive_data_flow_evidence(&db, &mut output);

        assert_eq!(output.nodes.len(), 2);
        assert_eq!(output.edges[0].kind, EvidenceEdgeKind::DataValue);
        assert!(
            output.edges[0]
                .source_fact_stable_keys
                .contains(&"df:edge:value".to_string())
        );
        assert!(output.nodes.iter().any(|node| {
            node.source_fact_stable_keys
                .contains(&"df:source".to_string())
        }));
    }

    #[test]
    fn sanitizer_and_barrier_edges_are_not_source_to_sink_propagation_models() {
        let mut sanitizer = data_flow_edge(0, "df:edge:sanitizer");
        sanitizer.kind = DataFlowEdgeKind::Sanitizer;
        let mut barrier = data_flow_edge(1, "df:edge:barrier");
        barrier.kind = DataFlowEdgeKind::Barrier;
        let mut db = AnalysisDb::new();
        db.replace_data_flow_facts(DataFlowOutput {
            nodes: vec![data_flow_node(0, "df:source"), data_flow_node(1, "df:sink")],
            edges: vec![sanitizer, barrier],
            models: Vec::new(),
            budgets: Vec::new(),
        })
        .expect("valid data-flow output");
        let mut output = EvidenceOutput::empty();

        derive_data_flow_evidence(&db, &mut output);

        assert_eq!(output.edges.len(), 2);
        assert!(
            output
                .edges
                .iter()
                .all(|edge| edge.kind == EvidenceEdgeKind::ExplanationOnly)
        );
    }

    #[test]
    fn control_dependence_rows_become_control_evidence_edges() {
        let mut db = AnalysisDb::new();
        db.replace_cfg_facts(CfgOutput {
            functions: vec![cfg_function()],
            nodes: vec![cfg_node(1), cfg_node(2)],
            blocks: vec![basic_block(1), basic_block(2)],
            edges: vec![cfg_edge()],
            control_dependence: vec![control_dependence()],
            ..CfgOutput::empty()
        })
        .expect("valid cfg output");
        let mut output = EvidenceOutput::empty();

        derive_control_dependence_evidence(&db, &mut output);

        assert_eq!(output.edges[0].kind, EvidenceEdgeKind::Control);
        assert_eq!(output.edges[0].query_mode, EvidenceQueryMode::FullBackward);
        assert!(
            output.edges[0]
                .source_fact_stable_keys
                .contains(&"cfg:control".to_string())
        );
    }

    #[test]
    fn control_dependence_controlled_node_anchors_to_controlled_block() {
        let mut db = AnalysisDb::new();
        let mut dependence = control_dependence();
        dependence.controlled_block = BasicBlockId(3);
        db.replace_cfg_facts(CfgOutput {
            functions: vec![cfg_function()],
            nodes: vec![cfg_node(1), cfg_node(2), cfg_node(3)],
            blocks: vec![basic_block(1), basic_block(2), basic_block(3)],
            edges: vec![cfg_edge()],
            control_dependence: vec![dependence],
            ..CfgOutput::empty()
        })
        .expect("valid cfg output");
        let mut output = EvidenceOutput::empty();

        derive_control_dependence_evidence(&db, &mut output);

        assert!(output.nodes.iter().any(|node| {
            node.compact_label.as_deref() == Some("controlled_block")
                && node.cfg_node == Some(CfgNodeId(3))
        }));
    }

    #[test]
    fn output_digest_changes_for_all_advertised_output_families() {
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == EVIDENCE_PROVIDER_ID)
            .expect("evidence provider manifest");
        let snapshot = minimal_snapshot();
        let upstream = digest("upstream");
        let base = EvidenceOutput::empty();
        let base_digest = digest_for_output(manifest, &snapshot, &upstream, &base);

        for output in [
            EvidenceOutput {
                nodes: vec![evidence_node(0, "node:output")],
                ..EvidenceOutput::empty()
            },
            EvidenceOutput {
                edges: vec![evidence_edge(0, 0, 1, "edge:output")],
                ..EvidenceOutput::empty()
            },
            EvidenceOutput {
                bundles: vec![evidence_bundle(0, "bundle:output")],
                ..EvidenceOutput::empty()
            },
            EvidenceOutput {
                paths: vec![evidence_path(0, "path:output")],
                ..EvidenceOutput::empty()
            },
            EvidenceOutput {
                slices: vec![evidence_slice(0, "slice:output")],
                ..EvidenceOutput::empty()
            },
            EvidenceOutput {
                unknowns: vec![evidence_unknown("unknown:output")],
                ..EvidenceOutput::empty()
            },
            EvidenceOutput {
                omitted_regions: vec![evidence_omitted(0, "omitted:output")],
                ..EvidenceOutput::empty()
            },
            EvidenceOutput {
                replay_keys: vec![evidence_replay_key("replay:output")],
                ..EvidenceOutput::empty()
            },
        ] {
            assert_ne!(
                base_digest,
                digest_for_output(manifest, &snapshot, &upstream, &output)
            );
        }
    }

    fn data_flow_node(id: u64, stable_key: &str) -> DataFlowNodeFact {
        DataFlowNodeFact {
            id: DataFlowNodeId(id),
            kind: DataFlowNodeKind::Place,
            language: Language::Go,
            file: Some(FileId(1)),
            function: Some(FunctionId(1)),
            body: Some(MirBodyId(1)),
            operation: None,
            cfg_node: None,
            place: Some(PlaceId(id)),
            symbol: None,
            reference: None,
            call_site: None,
            model: None,
            span: Some(span()),
            stable_key: stable_key.to_string(),
        }
    }

    fn data_flow_edge(id: u64, stable_key: &str) -> DataFlowEdgeFact {
        DataFlowEdgeFact {
            id: DataFlowEdgeId(id),
            from: DataFlowNodeId(0),
            to: DataFlowNodeId(1),
            kind: DataFlowEdgeKind::LocalAssignment,
            algorithm: DataFlowAlgorithm::LocalMir,
            status: DataFlowStatus::Present,
            precision: DataFlowPrecision::Syntax,
            validation: DataFlowValidation::Native,
            confidence: DataFlowConfidence::High,
            provenance: DataFlowProvenance::Native,
            call_site: None,
            call_target: None,
            refined_call: None,
            model: None,
            budget: None,
            evidence: Vec::new(),
            input_stable_keys: vec!["df:source".to_string(), "df:sink".to_string()],
            stable_key: stable_key.to_string(),
        }
    }

    fn cfg_function() -> CfgFunctionFact {
        CfgFunctionFact {
            id: CfgFunctionId(1),
            body: MirBodyId(1),
            function: FunctionId(1),
            language: Language::Go,
            file: FileId(1),
            span: span(),
            entry_node: CfgNodeId(1),
            normal_exit_node: CfgNodeId(2),
            exceptional_exit_node: None,
            stable_key: "cfg:function".to_string(),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactSyntax,
        }
    }

    fn cfg_node(id: u64) -> CfgNodeFact {
        CfgNodeFact {
            id: CfgNodeId(id),
            cfg_function: CfgFunctionId(1),
            body: MirBodyId(1),
            operation: None,
            block: BasicBlockId(id),
            kind: CfgNodeKind::Operation,
            span: Some(span()),
            generated: false,
            operation_ordinal: id as u32,
            stable_key: format!("cfg:node:{id}"),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactSyntax,
        }
    }

    fn basic_block(id: u64) -> BasicBlockFact {
        BasicBlockFact {
            id: BasicBlockId(id),
            cfg_function: CfgFunctionId(1),
            kind: BasicBlockKind::StraightLine,
            first_node: Some(CfgNodeId(id)),
            last_node: Some(CfgNodeId(id)),
            reachable: true,
            reverse_postorder: id as u32,
            stable_key: format!("cfg:block:{id}"),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactSyntax,
        }
    }

    fn cfg_edge() -> CfgEdgeFact {
        CfgEdgeFact {
            id: CfgEdgeId(1),
            cfg_function: CfgFunctionId(1),
            view: CfgView::NormalControl,
            from: CfgNodeId(1),
            to: CfgNodeId(2),
            from_block: BasicBlockId(1),
            to_block: BasicBlockId(2),
            kind: CfgEdgeKind::True,
            label: Some("if".to_string()),
            stable_key: "cfg:edge".to_string(),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactSyntax,
        }
    }

    fn control_dependence() -> ControlDependenceFact {
        ControlDependenceFact {
            id: ControlDependenceId(1),
            cfg_function: CfgFunctionId(1),
            view: CfgView::NormalControl,
            controlling_edge: CfgEdgeId(1),
            controlling_edge_kind: CfgEdgeKind::True,
            controlled_block: BasicBlockId(2),
            stable_key: "cfg:control".to_string(),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactSyntax,
        }
    }

    fn span() -> Span {
        Span {
            file: FileId(1),
            start_byte: 0,
            end_byte: 1,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 2,
        }
    }

    fn minimal_snapshot() -> InputSnapshot {
        InputSnapshot {
            schema_version: "test".to_string(),
            files: Vec::new(),
            config: InputComponent {
                name: "config".to_string(),
                status: InputComponentStatus::Present,
                digest: digest("config"),
                detail: Vec::new(),
            },
            go_lifecycle: GoLifecycleSnapshot {
                components: Vec::new(),
            },
            ts_js_lifecycle: TsJsLifecycleSnapshot {
                components: Vec::new(),
            },
            rules: Vec::new(),
            models: Vec::new(),
            extensions: Vec::new(),
            tool_invocations: Vec::new(),
            provider_schemas: Vec::new(),
        }
    }

    fn digest(label: &str) -> Digest {
        Digest::from_parts(DigestKind::ProviderOutput, label, &[label])
    }

    fn digest_for_output(
        manifest: &ProviderManifest,
        snapshot: &InputSnapshot,
        upstream: &Digest,
        output: &EvidenceOutput,
    ) -> Digest {
        evidence_output_digest(
            manifest, snapshot, upstream, upstream, upstream, upstream, upstream, upstream,
            upstream, upstream, upstream, output,
        )
    }

    fn evidence_node(id: u64, stable_key: &str) -> EvidenceNodeFact {
        EvidenceNodeFact {
            id: EvidenceNodeId(id),
            kind: EvidenceNodeKind::Synthetic,
            language: Language::Go,
            file: None,
            function: None,
            body: None,
            operation: None,
            cfg_node: None,
            place: None,
            symbol: None,
            reference: None,
            call_site: None,
            span: None,
            status: EvidenceStatus::Present,
            precision: EvidencePrecision::Syntax,
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::Native,
            confidence: EvidenceConfidence::High,
            compact_label: None,
            source_fact_stable_keys: Vec::new(),
            stable_key: stable_key.to_string(),
        }
    }

    fn evidence_edge(id: u64, from: u64, to: u64, stable_key: &str) -> EvidenceEdgeFact {
        EvidenceEdgeFact {
            id: EvidenceEdgeId(id),
            from: EvidenceNodeId(from),
            to: EvidenceNodeId(to),
            kind: EvidenceEdgeKind::DataValue,
            query_mode: EvidenceQueryMode::ThinBackward,
            status: EvidenceStatus::Present,
            precision: EvidencePrecision::Syntax,
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::Native,
            confidence: EvidenceConfidence::High,
            call_site: None,
            summary_stable_key: None,
            expansion: EvidenceExpansion::None,
            compact_label: None,
            source_fact_stable_keys: Vec::new(),
            stable_key: stable_key.to_string(),
        }
    }

    fn evidence_bundle(id: u64, stable_key: &str) -> EvidenceBundleFact {
        EvidenceBundleFact {
            id: crate::analysis::ids::EvidenceBundleId(id),
            diagnostic_stable_key: "diag:output".to_string(),
            query_mode: EvidenceQueryMode::ThinBackward,
            status: EvidenceStatus::Present,
            precision: EvidencePrecision::Syntax,
            provenance: EvidenceProvenance::Native,
            validation: EvidenceValidation::Native,
            confidence: EvidenceConfidence::High,
            entry_node: None,
            selected_paths: Vec::new(),
            selected_slices: Vec::new(),
            replay_key: None,
            stable_key: stable_key.to_string(),
        }
    }

    fn evidence_path(id: u64, stable_key: &str) -> EvidencePathFact {
        EvidencePathFact {
            id: crate::analysis::ids::EvidencePathId(id),
            bundle: None,
            query_mode: EvidenceQueryMode::Path,
            nodes: Vec::new(),
            edges: Vec::new(),
            rank: 0,
            score: EvidenceRankScore::default(),
            status: EvidenceStatus::Present,
            hidden_node_count: 0,
            omitted_regions: Vec::new(),
            stable_key: stable_key.to_string(),
        }
    }

    fn evidence_slice(id: u64, stable_key: &str) -> EvidenceSliceFact {
        EvidenceSliceFact {
            id: crate::analysis::ids::EvidenceSliceId(id),
            bundle: None,
            query_mode: EvidenceQueryMode::ThinBackward,
            root_nodes: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            status: EvidenceStatus::Present,
            omitted_regions: Vec::new(),
            stable_key: stable_key.to_string(),
        }
    }

    fn evidence_unknown(stable_key: &str) -> EvidenceUnknownFact {
        EvidenceUnknownFact {
            bundle: None,
            path: None,
            slice: None,
            edge: None,
            reason: EvidenceUnknownReason::OpaqueSummary,
            message: "unknown".to_string(),
            source_fact_stable_keys: Vec::new(),
            stable_key: stable_key.to_string(),
        }
    }

    fn evidence_omitted(id: u64, stable_key: &str) -> EvidenceOmittedRegionFact {
        EvidenceOmittedRegionFact {
            id: crate::analysis::ids::EvidenceOmittedRegionId(id),
            bundle: None,
            path: None,
            slice: None,
            reason: EvidenceOmittedReason::CompactRendering,
            hidden_node_count: 1,
            hidden_edge_count: 0,
            budget_label: Some("test".to_string()),
            stable_key: stable_key.to_string(),
        }
    }

    fn evidence_replay_key(stable_key: &str) -> EvidenceReplayKeyFact {
        EvidenceReplayKeyFact {
            bundle: crate::analysis::ids::EvidenceBundleId(0),
            query_mode: EvidenceQueryMode::Path,
            graph_schema: "evidence.graph.v1".to_string(),
            query_budget: EvidenceQueryBudget::default(),
            ranking: EvidenceRankingMode::DeterministicDisplay,
            renderer: EvidenceRendererMode::Debug,
            upstream_digest_keys: Vec::new(),
            stable_key: stable_key.to_string(),
        }
    }
}
