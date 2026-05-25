use serde::Serialize;
use std::fmt::Debug;

use super::cache_key::{
    data_flow_provider_parameter_digest, data_flow_provider_parameter_digest_for_snapshot,
};
use super::facts::{
    DataFlowAlgorithm, DataFlowConfidence, DataFlowEdgeFact, DataFlowEdgeKind, DataFlowModelFact,
    DataFlowModelKind, DataFlowNodeFact, DataFlowNodeKind, DataFlowPrecision, DataFlowProvenance,
    DataFlowStatus, DataFlowValidation,
};
use super::store::{
    DataFlowOutput, next_data_flow_edge_id, next_data_flow_model_id, next_data_flow_node_id,
};
use crate::analysis::ids::DataFlowNodeId;
use crate::analysis::places::PlaceFact;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::analysis_kernel::{FactFamily, FactRef, ProviderManifest, stable_key_from_parts};
use crate::core::AnalysisDb;
use crate::diagnostics::Diagnostic;

pub(crate) const DATA_FLOW_PROVIDER_ID: &str = "polint.data_flow";

#[derive(Debug, Clone, Default)]
pub(crate) struct DataFlowProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_data_flow_with_cache_stats(
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
) -> DataFlowProviderOutput {
    debug_assert_eq!(manifest.id, DATA_FLOW_PROVIDER_ID);
    let mut output = DataFlowOutput::empty();
    derive_local_place_nodes(db, &mut output);
    derive_refined_call_edges(db, &mut output);
    derive_source_models(db, &mut output);
    derive_extension_models(db, &mut output);
    output = output.normalized();

    let output_digest = data_flow_output_digest(
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
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_data_flow_facts(output) {
        Ok(()) => DataFlowProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => DataFlowProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

fn derive_local_place_nodes(db: &AnalysisDb, output: &mut DataFlowOutput) {
    for place in db.mir_places() {
        output.nodes.push(node_from_place(output, place, db));
    }
}

fn derive_refined_call_edges(db: &AnalysisDb, output: &mut DataFlowOutput) {
    for edge in db.refined_call_edges() {
        if edge.status != crate::analysis::calls::facts::CallTargetStatus::Resolved {
            continue;
        }
        let from = push_call_node(
            output,
            DataFlowNodeKind::CallArgument,
            edge.site.0,
            format!("site:{}:arg", edge.site.0),
        );
        let to = push_call_node(
            output,
            DataFlowNodeKind::CallReturn,
            edge.site.0,
            format!("site:{}:return", edge.site.0),
        );
        let id = next_data_flow_edge_id(&output.edges);
        output.edges.push(DataFlowEdgeFact {
            id,
            from,
            to,
            kind: DataFlowEdgeKind::CallArgumentToParameter,
            algorithm: DataFlowAlgorithm::DirectCall,
            status: DataFlowStatus::Present,
            precision: DataFlowPrecision::SetupAware,
            validation: DataFlowValidation::ReferentiallyValidated,
            confidence: DataFlowConfidence::High,
            provenance: DataFlowProvenance::Native,
            call_site: Some(edge.site),
            call_target: edge.base_target,
            refined_call: Some(edge.id),
            model: None,
            budget: None,
            evidence: vec!["refined_call_edge".to_string()],
            input_stable_keys: vec![edge.stable_key.clone()],
            stable_key: stable_key_from_parts(
                FactFamily::DataFlowEdge,
                &[
                    ("kind", "call_argument_to_parameter".to_string()),
                    ("refined_call", edge.stable_key.clone()),
                ],
            ),
        });
    }
}

fn derive_source_models(db: &AnalysisDb, output: &mut DataFlowOutput) {
    for boundary in db.trust_boundary_facts() {
        let model_id = next_data_flow_model_id(&output.models);
        let stable_key = stable_key_from_parts(
            FactFamily::DataFlowModel,
            &[
                ("kind", "source".to_string()),
                ("trust_boundary", boundary.stable_key.clone()),
            ],
        );
        output.models.push(DataFlowModelFact {
            id: model_id,
            kind: DataFlowModelKind::Source,
            language: boundary.language,
            provider_id: boundary.provider_id.clone(),
            model_id: Some(format!("{:?}", boundary.source_kind)),
            source_stable_key: Some(boundary.stable_key.clone()),
            status: DataFlowStatus::Present,
            precision: DataFlowPrecision::SetupAware,
            validation: DataFlowValidation::ReferentiallyValidated,
            confidence: DataFlowConfidence::High,
            provenance: DataFlowProvenance::Native,
            evidence: vec!["trust_boundary".to_string()],
            payload_labels: vec![
                format!("source_kind={:?}", boundary.source_kind),
                boundary.access_path.clone().unwrap_or_default(),
            ],
            stable_key: stable_key.clone(),
        });
        output.nodes.push(DataFlowNodeFact {
            id: next_data_flow_node_id(&output.nodes),
            kind: DataFlowNodeKind::Source,
            language: boundary.language,
            file: Some(boundary.file),
            function: boundary.target_parameter,
            body: None,
            operation: None,
            cfg_node: None,
            place: None,
            symbol: None,
            reference: None,
            call_site: None,
            model: Some(model_id),
            span: Some(boundary.span.clone()),
            stable_key: stable_key_from_parts(
                FactFamily::DataFlowNode,
                &[("source_model", stable_key)],
            ),
        });
    }
}

fn derive_extension_models(db: &AnalysisDb, output: &mut DataFlowOutput) {
    for fact in db.extension_facts() {
        let kind = match fact.fact_family.as_str() {
            "data_flow.source" | "source" => DataFlowModelKind::Source,
            "data_flow.sink" | "sink" => DataFlowModelKind::Sink,
            "data_flow.sanitizer" | "sanitizer" => DataFlowModelKind::Sanitizer,
            "data_flow.barrier" | "barrier" => DataFlowModelKind::Barrier,
            "data_flow.tito" | "tito" => DataFlowModelKind::Tito,
            _ => continue,
        };
        output.models.push(DataFlowModelFact {
            id: next_data_flow_model_id(&output.models),
            kind,
            language: crate::core::Language::Unknown,
            provider_id: fact.provider_id.clone(),
            model_id: Some(fact.extension_id.clone()),
            source_stable_key: Some(fact.stable_key.clone()),
            status: DataFlowStatus::Present,
            precision: extension_precision(fact.precision),
            validation: DataFlowValidation::ExtensionValidated,
            confidence: extension_confidence(fact.confidence),
            provenance: DataFlowProvenance::Extension,
            evidence: fact.evidence.clone(),
            payload_labels: fact.payload_labels.clone(),
            stable_key: stable_key_from_parts(
                FactFamily::DataFlowModel,
                &[
                    ("kind", format!("{kind:?}")),
                    ("extension_fact", fact.stable_key.clone()),
                ],
            ),
        });
    }
}

fn node_from_place(
    output: &DataFlowOutput,
    place: &PlaceFact,
    db: &AnalysisDb,
) -> DataFlowNodeFact {
    DataFlowNodeFact {
        id: next_data_flow_node_id(&output.nodes),
        kind: DataFlowNodeKind::Place,
        language: place.language,
        file: place.file,
        function: place.function,
        body: None,
        operation: None,
        cfg_node: None,
        place: Some(place.id),
        symbol: None,
        reference: None,
        call_site: None,
        model: None,
        span: None,
        stable_key: db
            .metadata_for(FactRef::new(FactFamily::Place, place.id.0))
            .map(|metadata| {
                stable_key_from_parts(
                    FactFamily::DataFlowNode,
                    &[("place", metadata.stable_key.clone())],
                )
            })
            .unwrap_or_else(|| {
                stable_key_from_parts(
                    FactFamily::DataFlowNode,
                    &[("place_id", place.id.0.to_string())],
                )
            }),
    }
}

fn extension_precision(
    precision: crate::analysis::extensions::sinks::ExtensionFactPrecision,
) -> DataFlowPrecision {
    match precision {
        crate::analysis::extensions::sinks::ExtensionFactPrecision::Exact => {
            DataFlowPrecision::Exact
        }
        crate::analysis::extensions::sinks::ExtensionFactPrecision::SetupAware => {
            DataFlowPrecision::SetupAware
        }
        crate::analysis::extensions::sinks::ExtensionFactPrecision::Heuristic => {
            DataFlowPrecision::Heuristic
        }
        crate::analysis::extensions::sinks::ExtensionFactPrecision::GeneratedUnvalidated => {
            DataFlowPrecision::Heuristic
        }
    }
}

fn extension_confidence(
    confidence: crate::analysis::extensions::sinks::ExtensionFactConfidence,
) -> DataFlowConfidence {
    match confidence {
        crate::analysis::extensions::sinks::ExtensionFactConfidence::High => {
            DataFlowConfidence::High
        }
        crate::analysis::extensions::sinks::ExtensionFactConfidence::Medium => {
            DataFlowConfidence::Medium
        }
        crate::analysis::extensions::sinks::ExtensionFactConfidence::Low => DataFlowConfidence::Low,
    }
}

fn push_call_node(
    output: &mut DataFlowOutput,
    kind: DataFlowNodeKind,
    site_id: u64,
    suffix: String,
) -> DataFlowNodeId {
    let stable_key = stable_key_from_parts(
        FactFamily::DataFlowNode,
        &[("call_node", suffix), ("kind", format!("{kind:?}"))],
    );
    if let Some(existing) = output
        .nodes
        .iter()
        .find(|node| node.stable_key == stable_key)
        .map(|node| node.id)
    {
        return existing;
    }
    let id = next_data_flow_node_id(&output.nodes);
    output.nodes.push(DataFlowNodeFact {
        id,
        kind,
        language: crate::core::Language::Unknown,
        file: None,
        function: None,
        body: None,
        operation: None,
        cfg_node: None,
        place: None,
        symbol: None,
        reference: None,
        call_site: Some(crate::analysis::ids::CallSiteId(site_id)),
        model: None,
        span: None,
        stable_key,
    });
    id
}

#[allow(clippy::too_many_arguments)]
fn data_flow_output_digest(
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
    output: &DataFlowOutput,
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
    ];
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", data_flow_provider_parameter_digest()),
        format!(
            "input_parameters={}",
            data_flow_provider_parameter_digest_for_snapshot(input_snapshot, &upstream)
        ),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
        format!("calls={calls_output_digest}"),
        format!("refined_calls={refined_calls_output_digest}"),
        format!("direct_summaries={direct_summaries_output_digest}"),
        format!("type_value_alias={type_value_alias_output_digest}"),
        format!("entrypoints={entrypoints_output_digest}"),
        format!("extensions={extensions_output_digest}"),
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
            .map(|node| format!("data_flow_node={}", stable_fact_payload(node))),
    );
    parts.extend(
        output
            .edges
            .iter()
            .map(|edge| format!("data_flow_edge={}", stable_fact_payload(edge))),
    );
    parts.extend(
        output
            .models
            .iter()
            .map(|model| format!("data_flow_model={}", stable_fact_payload(model))),
    );
    parts.extend(
        output
            .budgets
            .iter()
            .map(|budget| format!("data_flow_budget={}", stable_fact_payload(budget))),
    );
    if output.nodes.is_empty() && output.edges.is_empty() && output.models.is_empty() {
        parts.push("data_flow_output=empty".to_string());
    }
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "data_flow_output", &refs)
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
        format!("Data-flow provider failed: {message}"),
    )
}
