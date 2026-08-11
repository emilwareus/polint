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
use crate::cfg::facts::{CfgPrecision, CfgStatus};
use crate::data_flow::facts::{
    DataFlowConfidence, DataFlowEdgeFact, DataFlowEdgeKind, DataFlowNodeFact, DataFlowNodeKind,
    DataFlowPrecision, DataFlowProvenance, DataFlowStatus, DataFlowValidation,
};
use crate::ids::{EvidenceEdgeId, EvidenceNodeId};
use polint_analysis_api::ProviderManifest;
use polint_analysis_api::{CacheStats, Digest, DigestKind, InputComponent, InputSnapshot};
use polint_analysis_api::{FactFamily, stable_key_from_parts};
use crate::AnalysisHost;
use polint_core::Diagnostic;

pub const EVIDENCE_PROVIDER_ID: &str = "polint.evidence";

#[derive(Debug, Clone, Default)]
pub struct EvidenceProviderOutput {
    pub diagnostics: Vec<Diagnostic>,
    pub cache_stats: CacheStats,
    pub output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub fn derive_evidence_with_cache_stats(
    db: &mut impl AnalysisHost,
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
    let interner = db.stable_key_interner();
    let output = output.normalized(&interner);
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
        &interner,
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

fn derive_data_flow_evidence(db: &impl AnalysisHost, output: &mut EvidenceOutput) {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let mut node_map = std::collections::BTreeMap::new();
    for node in db.data_flow_nodes() {
        let evidence_id = EvidenceNodeId(output.nodes.len() as u64);
        node_map.insert(node.id, evidence_id);
        output
            .nodes
            .push(evidence_node_from_data_flow(interner, node, evidence_id));
    }

    for edge in db.data_flow_edges() {
        let (Some(from), Some(to)) = (node_map.get(&edge.from), node_map.get(&edge.to)) else {
            continue;
        };
        output.edges.push(evidence_edge_from_data_flow(
            interner,
            edge,
            EvidenceEdgeId(output.edges.len() as u64),
            *from,
            *to,
        ));
    }
}

fn evidence_node_from_data_flow(
    interner: &polint_core::StableKeyInterner,
    node: &DataFlowNodeFact,
    id: EvidenceNodeId,
) -> EvidenceNodeFact {
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
        source_fact_stable_keys: vec![interner.resolve(node.stable_key).to_string()],
        stable_key: stable_key_from_parts(
            interner,
            FactFamily::EvidenceNode,
            &[(
                "data_flow_node",
                interner.resolve(node.stable_key).to_string(),
            )],
        ),
    }
}

fn evidence_edge_from_data_flow(
    interner: &polint_core::StableKeyInterner,
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
        source_fact_stable_keys: std::iter::once(interner.resolve(edge.stable_key).to_string())
            .chain(edge.input_stable_keys.iter().cloned())
            .collect(),
        stable_key: stable_key_from_parts(
            interner,
            FactFamily::EvidenceEdge,
            &[(
                "data_flow_edge",
                interner.resolve(edge.stable_key).to_string(),
            )],
        ),
    }
}

fn derive_control_dependence_evidence(db: &impl AnalysisHost, output: &mut EvidenceOutput) {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
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
                .unwrap_or(polint_core::Language::Unknown),
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
                interner.resolve(dependence.stable_key).to_string(),
                interner.resolve(controlling_edge.stable_key).to_string(),
            ],
            stable_key: stable_key_from_parts(
                interner,
                FactFamily::EvidenceNode,
                &[
                    (
                        "control_dependence",
                        interner.resolve(dependence.stable_key).to_string(),
                    ),
                    ("role", "controller".to_string()),
                ],
            ),
        });
        let to = EvidenceNodeId(output.nodes.len() as u64);
        output.nodes.push(EvidenceNodeFact {
            id: to,
            kind: EvidenceNodeKind::Synthetic,
            language: polint_core::Language::Unknown,
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
            source_fact_stable_keys: vec![interner.resolve(dependence.stable_key).to_string()],
            stable_key: stable_key_from_parts(
                interner,
                FactFamily::EvidenceNode,
                &[
                    (
                        "control_dependence",
                        interner.resolve(dependence.stable_key).to_string(),
                    ),
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
                interner.resolve(dependence.stable_key).to_string(),
                interner.resolve(controlling_edge.stable_key).to_string(),
            ],
            stable_key: stable_key_from_parts(
                interner,
                FactFamily::EvidenceEdge,
                &[(
                    "control_dependence",
                    interner.resolve(dependence.stable_key).to_string(),
                )],
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
    interner: &polint_core::StableKeyInterner,
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
            .map(|node| format!("evidence_node={}", stable_fact_payload(interner, node))),
    );
    parts.extend(
        output
            .edges
            .iter()
            .map(|edge| format!("evidence_edge={}", stable_fact_payload(interner, edge))),
    );
    parts.extend(
        output
            .bundles
            .iter()
            .map(|bundle| format!("evidence_bundle={}", stable_fact_payload(interner, bundle))),
    );
    parts.extend(
        output
            .paths
            .iter()
            .map(|path| format!("evidence_path={}", stable_fact_payload(interner, path))),
    );
    parts.extend(
        output
            .slices
            .iter()
            .map(|slice| format!("evidence_slice={}", stable_fact_payload(interner, slice))),
    );
    parts.extend(output.unknowns.iter().map(|unknown| {
        format!(
            "evidence_unknown={}",
            stable_fact_payload(interner, unknown)
        )
    }));
    parts.extend(output.omitted_regions.iter().map(|omitted| {
        format!(
            "evidence_omitted_region={}",
            stable_fact_payload(interner, omitted)
        )
    }));
    parts.extend(output.replay_keys.iter().map(|replay| {
        format!(
            "evidence_replay_key={}",
            stable_fact_payload(interner, replay)
        )
    }));
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

fn stable_fact_payload<T>(interner: &polint_core::StableKeyInterner, fact: &T) -> String
where
    T: Debug,
{
    resolve_stable_key_ids(interner, &format!("{fact:?}"))
}

fn resolve_stable_key_ids(interner: &polint_core::StableKeyInterner, payload: &str) -> String {
    let mut resolved = String::with_capacity(payload.len());
    let mut remaining = payload;
    while let Some(start) = remaining.find("StableKeyId(") {
        resolved.push_str(&remaining[..start]);
        let id_start = start + "StableKeyId(".len();
        let Some(relative_end) = remaining[id_start..].find(')') else {
            resolved.push_str(&remaining[start..]);
            return resolved;
        };
        let id_end = id_start + relative_end;
        let Ok(id) = remaining[id_start..id_end].parse::<u32>() else {
            resolved.push_str(&remaining[start..=id_end]);
            remaining = &remaining[id_end + 1..];
            continue;
        };
        resolved.push_str(&format!(
            "{:?}",
            interner.resolve(polint_core::StableKeyId(id))
        ));
        remaining = &remaining[id_end + 1..];
    }
    resolved.push_str(remaining);
    resolved
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        polint_core::DiagnosticRange::point(1, 1),
        format!("Evidence provider failed: {message}"),
    )
}
