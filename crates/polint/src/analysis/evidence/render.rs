use serde_json::{Value, json};

use crate::analysis::evidence::facts::EvidenceExpansion;
use crate::analysis::evidence::store::EvidenceStore;
use crate::analysis::ids::EvidenceBundleId;
use crate::diagnostics::Evidence;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvidenceRenderLimit {
    pub(crate) max_paths: usize,
    pub(crate) max_edges_per_path: usize,
    pub(crate) max_unknowns: usize,
    pub(crate) max_omitted_regions: usize,
}

impl Default for EvidenceRenderLimit {
    fn default() -> Self {
        Self {
            max_paths: 5,
            max_edges_per_path: 96,
            max_unknowns: 32,
            max_omitted_regions: 32,
        }
    }
}

pub(crate) fn render_bundle_json_v1(
    store: &EvidenceStore,
    bundle_id: EvidenceBundleId,
    limit: EvidenceRenderLimit,
) -> Option<Value> {
    let bundle = store
        .bundles()
        .iter()
        .find(|bundle| bundle.id == bundle_id)?;
    let mut paths = bundle
        .selected_paths
        .iter()
        .filter_map(|path_id| store.paths().iter().find(|path| path.id == *path_id))
        .collect::<Vec<_>>();
    paths.sort_by(|left, right| {
        (left.rank, left.stable_key.as_str()).cmp(&(right.rank, right.stable_key.as_str()))
    });

    let mut unknowns = store
        .unknowns()
        .iter()
        .filter(|unknown| unknown.bundle == Some(bundle.id))
        .collect::<Vec<_>>();
    unknowns.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));

    let mut omitted = store
        .omitted_regions()
        .iter()
        .filter(|region| region.bundle == Some(bundle.id))
        .collect::<Vec<_>>();
    omitted.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));

    let replay_key = store
        .replay_keys()
        .iter()
        .find(|key| key.bundle == bundle.id)
        .map(|key| key.stable_key.clone())
        .or_else(|| bundle.replay_key.clone());

    Some(json!({
        "version": 1,
        "bundle": {
            "id": bundle.id.0,
            "stable_key": bundle.stable_key,
            "diagnostic_stable_key": bundle.diagnostic_stable_key,
            "status": label(bundle.status),
            "precision": label(bundle.precision),
            "provenance": label(bundle.provenance),
            "validation": label(bundle.validation),
            "confidence": label(bundle.confidence),
            "replay_key": replay_key,
        },
        "paths": paths.into_iter().take(limit.max_paths).map(|path| {
            let edge_values = path
                .edges
                .iter()
                .take(limit.max_edges_per_path)
                .filter_map(|edge_id| store.edge(*edge_id))
                .map(|edge| json!({
                    "id": edge.id.0,
                    "stable_key": edge.stable_key,
                    "kind": label(edge.kind),
                    "status": label(edge.status),
                    "precision": label(edge.precision),
                    "provenance": label(edge.provenance),
                    "validation": label(edge.validation),
                    "summary_stable_key": edge.summary_stable_key,
                    "expansion": expansion_json(&edge.expansion),
                }))
                .collect::<Vec<_>>();
            json!({
                "id": path.id.0,
                "stable_key": path.stable_key,
                "rank": path.rank,
                "status": label(path.status),
                "hidden_node_count": path.hidden_node_count,
                "nodes": path.nodes.iter().map(|node| node.0).collect::<Vec<_>>(),
                "edges": edge_values,
                "omitted_regions": path.omitted_regions.iter().map(|region| region.0).collect::<Vec<_>>(),
            })
        }).collect::<Vec<_>>(),
        "unknowns": unknowns.into_iter().take(limit.max_unknowns).map(|unknown| json!({
            "stable_key": unknown.stable_key,
            "reason": label(unknown.reason),
            "message": unknown.message,
            "edge": unknown.edge.map(|edge| edge.0),
        })).collect::<Vec<_>>(),
        "omitted_regions": omitted.into_iter().take(limit.max_omitted_regions).map(|region| json!({
            "id": region.id.0,
            "stable_key": region.stable_key,
            "reason": label(region.reason),
            "hidden_node_count": region.hidden_node_count,
            "hidden_edge_count": region.hidden_edge_count,
            "budget_label": region.budget_label,
        })).collect::<Vec<_>>(),
    }))
}

pub(crate) fn scalar_evidence_for_bundle_json(value: &Value) -> Vec<Evidence> {
    let Some(bundle) = value.get("bundle") else {
        return Vec::new();
    };
    ["stable_key", "status", "precision", "provenance"]
        .into_iter()
        .filter_map(|label| {
            bundle
                .get(label)
                .and_then(Value::as_str)
                .map(|value| Evidence {
                    label: format!("evidence_{label}"),
                    value: value.to_string(),
                })
        })
        .collect()
}

pub(crate) fn sarif_thread_flow_messages(value: &Value) -> Vec<String> {
    let mut messages = value
        .get("paths")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .flat_map(|path| {
            path.get("edges")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|edge| {
                    let kind = edge.get("kind").and_then(Value::as_str).unwrap_or("edge");
                    let status = edge.get("status").and_then(Value::as_str).unwrap_or("unknown");
                    let precision = edge
                        .get("precision")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown");
                    let expansion = edge
                        .get("expansion")
                        .and_then(Value::as_object)
                        .and_then(|object| object.get("state"))
                        .and_then(Value::as_str)
                        .unwrap_or("none");
                    format!(
                        "evidence {kind}: status={status}; precision={precision}; expansion={expansion}"
                    )
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    if let Some(unknowns) = value.get("unknowns").and_then(Value::as_array)
        && !unknowns.is_empty()
    {
        messages.push("evidence unknown: path contains unknown evidence".to_string());
    }
    if let Some(omitted) = value.get("omitted_regions").and_then(Value::as_array)
        && !omitted.is_empty()
    {
        messages.push("evidence omitted: budget or compact rendering hid details".to_string());
    }
    messages
}

fn expansion_json(expansion: &EvidenceExpansion) -> Value {
    match expansion {
        EvidenceExpansion::None => json!({"state": "none"}),
        EvidenceExpansion::Expandable { key } => json!({"state": "expandable", "key": key}),
        EvidenceExpansion::Opaque { reason } => json!({"state": "opaque", "reason": reason}),
        EvidenceExpansion::ExternalModel { model } => {
            json!({"state": "external_model", "model": model})
        }
    }
}

fn label<T: std::fmt::Debug>(value: T) -> String {
    format!("{value:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::evidence::facts::{
        EvidenceBundleFact, EvidenceConfidence, EvidenceEdgeFact, EvidenceEdgeKind,
        EvidenceNodeFact, EvidenceNodeKind, EvidenceOmittedReason, EvidenceOmittedRegionFact,
        EvidencePathFact, EvidencePrecision, EvidenceProvenance, EvidenceQueryMode,
        EvidenceRankScore, EvidenceReplayKeyFact, EvidenceStatus, EvidenceUnknownFact,
        EvidenceUnknownReason, EvidenceValidation,
    };
    use crate::analysis::evidence::store::EvidenceOutput;
    use crate::analysis::ids::{
        EvidenceEdgeId, EvidenceNodeId, EvidenceOmittedRegionId, EvidencePathId,
    };

    #[test]
    fn bundle_json_is_deterministic_and_bounded() {
        let store = render_store();

        let first = render_bundle_json_v1(
            &store,
            EvidenceBundleId(0),
            EvidenceRenderLimit {
                max_paths: 1,
                max_edges_per_path: 1,
                max_unknowns: 1,
                max_omitted_regions: 1,
            },
        )
        .expect("bundle");
        let second = render_bundle_json_v1(
            &store,
            EvidenceBundleId(0),
            EvidenceRenderLimit {
                max_paths: 1,
                max_edges_per_path: 1,
                max_unknowns: 1,
                max_omitted_regions: 1,
            },
        )
        .expect("bundle");

        assert_eq!(first, second);
        assert_eq!(first["paths"].as_array().expect("paths").len(), 1);
        assert_eq!(first["paths"][0]["hidden_node_count"], 2);
        assert_eq!(first["bundle"]["replay_key"], "replay:bundle");
        assert_eq!(
            first["paths"][0]["edges"][0]["expansion"]["key"],
            "expand:summary"
        );
        let rendered = serde_json::to_string(&first).expect("json");
        assert!(!rendered.contains("/Users/"));
        assert!(!rendered.contains("raw source body"));
        assert!(!rendered.contains("timestamp"));
    }

    #[test]
    fn scalar_bridge_preserves_existing_evidence_shape() {
        let store = render_store();
        let value =
            render_bundle_json_v1(&store, EvidenceBundleId(0), EvidenceRenderLimit::default())
                .expect("bundle");

        let scalar = scalar_evidence_for_bundle_json(&value);

        assert!(scalar.iter().any(|item| item.label == "evidence_status"));
        assert!(scalar.iter().all(|item| !item.value.is_empty()));
    }

    #[test]
    fn sarif_messages_include_lossy_unknown_and_budget_wording() {
        let store = render_store();
        let value =
            render_bundle_json_v1(&store, EvidenceBundleId(0), EvidenceRenderLimit::default())
                .expect("bundle");

        let messages = sarif_thread_flow_messages(&value);

        assert!(messages.iter().any(|message| message.contains("unknown")));
        assert!(messages.iter().any(|message| message.contains("omitted")));
    }

    fn render_store() -> EvidenceStore {
        EvidenceStore::from_output(EvidenceOutput {
            nodes: vec![node(0), node(1)],
            edges: vec![EvidenceEdgeFact {
                id: EvidenceEdgeId(0),
                from: EvidenceNodeId(0),
                to: EvidenceNodeId(1),
                kind: EvidenceEdgeKind::Summary,
                query_mode: EvidenceQueryMode::Path,
                status: EvidenceStatus::Present,
                precision: EvidencePrecision::SetupAware,
                provenance: EvidenceProvenance::Summary,
                validation: EvidenceValidation::ReferentiallyValidated,
                confidence: EvidenceConfidence::Medium,
                call_site: None,
                summary_stable_key: Some("summary:tito".to_string()),
                expansion: EvidenceExpansion::Expandable {
                    key: "expand:summary".to_string(),
                },
                compact_label: None,
                source_fact_stable_keys: Vec::new(),
                stable_key: "edge:summary".to_string(),
            }],
            bundles: vec![EvidenceBundleFact {
                id: EvidenceBundleId(0),
                diagnostic_stable_key: "diag:1".to_string(),
                query_mode: EvidenceQueryMode::Path,
                status: EvidenceStatus::Partial,
                precision: EvidencePrecision::SetupAware,
                provenance: EvidenceProvenance::Query,
                validation: EvidenceValidation::RendererValidated,
                confidence: EvidenceConfidence::Medium,
                entry_node: None,
                selected_paths: vec![EvidencePathId(0)],
                selected_slices: Vec::new(),
                replay_key: Some("replay:bundle".to_string()),
                stable_key: "bundle:diag".to_string(),
            }],
            paths: vec![EvidencePathFact {
                id: EvidencePathId(0),
                bundle: Some(EvidenceBundleId(0)),
                query_mode: EvidenceQueryMode::Path,
                nodes: vec![EvidenceNodeId(0), EvidenceNodeId(1)],
                edges: vec![EvidenceEdgeId(0)],
                rank: 0,
                score: EvidenceRankScore::default(),
                status: EvidenceStatus::Partial,
                hidden_node_count: 2,
                omitted_regions: vec![EvidenceOmittedRegionId(0)],
                stable_key: "path:summary".to_string(),
            }],
            slices: Vec::new(),
            unknowns: vec![EvidenceUnknownFact {
                bundle: Some(EvidenceBundleId(0)),
                path: Some(EvidencePathId(0)),
                slice: None,
                edge: Some(EvidenceEdgeId(0)),
                reason: EvidenceUnknownReason::OpaqueSummary,
                message: "opaque summary".to_string(),
                source_fact_stable_keys: Vec::new(),
                stable_key: "unknown:summary".to_string(),
            }],
            omitted_regions: vec![EvidenceOmittedRegionFact {
                id: EvidenceOmittedRegionId(0),
                bundle: Some(EvidenceBundleId(0)),
                path: Some(EvidencePathId(0)),
                slice: None,
                reason: EvidenceOmittedReason::CompactRendering,
                hidden_node_count: 2,
                hidden_edge_count: 1,
                budget_label: Some("compact".to_string()),
                stable_key: "omitted:summary".to_string(),
            }],
            replay_keys: vec![EvidenceReplayKeyFact {
                bundle: EvidenceBundleId(0),
                query_mode: EvidenceQueryMode::Path,
                graph_schema: "evidence.graph.v1".to_string(),
                query_budget: Default::default(),
                ranking:
                    crate::analysis::evidence::facts::EvidenceRankingMode::DeterministicDisplay,
                renderer: crate::analysis::evidence::facts::EvidenceRendererMode::Json,
                upstream_digest_keys: Vec::new(),
                stable_key: "replay:bundle".to_string(),
            }],
        })
        .expect("valid evidence")
    }

    fn node(id: u64) -> EvidenceNodeFact {
        EvidenceNodeFact {
            id: EvidenceNodeId(id),
            kind: EvidenceNodeKind::Summary,
            language: crate::core::Language::Go,
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
            stable_key: format!("node:{id}"),
        }
    }
}
