use serde::Serialize;

use crate::analysis::evidence::facts::{EvidenceExpansion, EvidenceStatus};
use crate::analysis::evidence::store::EvidenceStore;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EvidenceDebugReport {
    pub(crate) counts: EvidenceDebugCounts,
    pub(crate) statuses: EvidenceDebugStatuses,
    pub(crate) summary_expansion_keys: Vec<String>,
    pub(crate) summary_opaque_reasons: Vec<String>,
    pub(crate) replay_keys: Vec<String>,
    pub(crate) unknown_reasons: Vec<String>,
    pub(crate) omitted_regions: Vec<EvidenceDebugOmittedRegion>,
    pub(crate) hidden_node_count: u32,
    pub(crate) budget_caps: EvidenceDebugBudgetCaps,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EvidenceDebugCounts {
    pub(crate) nodes: usize,
    pub(crate) edges: usize,
    pub(crate) bundles: usize,
    pub(crate) paths: usize,
    pub(crate) slices: usize,
    pub(crate) unknowns: usize,
    pub(crate) omitted_regions: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub(crate) struct EvidenceDebugStatuses {
    pub(crate) exact: usize,
    pub(crate) partial: usize,
    pub(crate) unknown: usize,
    pub(crate) summary_backed: usize,
    pub(crate) extension_backed: usize,
    pub(crate) budget_limited: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct EvidenceDebugOmittedRegion {
    pub(crate) stable_key: String,
    pub(crate) reason: String,
    pub(crate) hidden_node_count: u32,
    pub(crate) hidden_edge_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(crate) struct EvidenceDebugBudgetCaps {
    pub(crate) max_paths: u32,
    pub(crate) max_nodes: u32,
    pub(crate) max_edges: u32,
    pub(crate) max_depth: u32,
}

pub(crate) fn evidence_debug_report(store: &EvidenceStore) -> EvidenceDebugReport {
    let mut summary_expansion_keys = store
        .edges()
        .iter()
        .filter_map(|edge| match &edge.expansion {
            EvidenceExpansion::Expandable { key } => Some(key.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    summary_expansion_keys.sort();
    summary_expansion_keys.dedup();

    let mut summary_opaque_reasons = store
        .edges()
        .iter()
        .filter_map(|edge| match &edge.expansion {
            EvidenceExpansion::Opaque { reason } => Some(reason.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    summary_opaque_reasons.sort();
    summary_opaque_reasons.dedup();

    let mut replay_keys = store
        .replay_keys()
        .iter()
        .map(|key| key.stable_key.clone())
        .chain(
            store
                .bundles()
                .iter()
                .filter_map(|bundle| bundle.replay_key.clone()),
        )
        .collect::<Vec<_>>();
    replay_keys.sort();
    replay_keys.dedup();

    let mut unknown_reasons = store
        .unknowns()
        .iter()
        .map(|unknown| format!("{:?}", unknown.reason))
        .collect::<Vec<_>>();
    unknown_reasons.sort();
    unknown_reasons.dedup();

    let mut omitted_regions = store
        .omitted_regions()
        .iter()
        .map(|region| EvidenceDebugOmittedRegion {
            stable_key: region.stable_key.clone(),
            reason: format!("{:?}", region.reason),
            hidden_node_count: region.hidden_node_count,
            hidden_edge_count: region.hidden_edge_count,
        })
        .collect::<Vec<_>>();
    omitted_regions.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));

    let budget_caps = store
        .replay_keys()
        .first()
        .map(|key| EvidenceDebugBudgetCaps {
            max_paths: key.query_budget.max_paths,
            max_nodes: key.query_budget.max_nodes,
            max_edges: key.query_budget.max_edges,
            max_depth: key.query_budget.max_depth,
        })
        .unwrap_or(EvidenceDebugBudgetCaps {
            max_paths: 0,
            max_nodes: 0,
            max_edges: 0,
            max_depth: 0,
        });

    EvidenceDebugReport {
        counts: EvidenceDebugCounts {
            nodes: store.nodes().len(),
            edges: store.edges().len(),
            bundles: store.bundles().len(),
            paths: store.paths().len(),
            slices: store.slices().len(),
            unknowns: store.unknowns().len(),
            omitted_regions: store.omitted_regions().len(),
        },
        statuses: EvidenceDebugStatuses {
            exact: store
                .edges()
                .iter()
                .filter(|edge| {
                    edge.precision == crate::analysis::evidence::facts::EvidencePrecision::Exact
                })
                .count(),
            partial: store
                .paths()
                .iter()
                .filter(|path| path.status == EvidenceStatus::Partial)
                .count(),
            unknown: store.unknowns().len(),
            summary_backed: store
                .edges()
                .iter()
                .filter(|edge| edge.summary_stable_key.is_some())
                .count(),
            extension_backed: store
                .edges()
                .iter()
                .filter(|edge| {
                    edge.provenance
                        == crate::analysis::evidence::facts::EvidenceProvenance::Extension
                })
                .count(),
            budget_limited: store
                .paths()
                .iter()
                .filter(|path| path.status == EvidenceStatus::BudgetExceeded)
                .count()
                + store.omitted_regions().len(),
        },
        summary_expansion_keys,
        summary_opaque_reasons,
        replay_keys,
        unknown_reasons,
        hidden_node_count: store
            .paths()
            .iter()
            .map(|path| path.hidden_node_count)
            .sum(),
        omitted_regions,
        budget_caps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::evidence::facts::{
        EvidenceBundleFact, EvidenceConfidence, EvidenceEdgeFact, EvidenceEdgeKind,
        EvidenceNodeFact, EvidenceNodeKind, EvidenceOmittedReason, EvidenceOmittedRegionFact,
        EvidencePathFact, EvidencePrecision, EvidenceProvenance, EvidenceQueryBudget,
        EvidenceQueryMode, EvidenceRankScore, EvidenceRankingMode, EvidenceRendererMode,
        EvidenceReplayKeyFact, EvidenceUnknownFact, EvidenceUnknownReason, EvidenceValidation,
    };
    use crate::analysis::evidence::store::EvidenceOutput;
    use crate::analysis::ids::{
        EvidenceBundleId, EvidenceEdgeId, EvidenceNodeId, EvidenceOmittedRegionId, EvidencePathId,
    };
    use crate::core::Language;

    #[test]
    fn debug_report_is_deterministic_compact_and_private() {
        let store = store();

        let first = evidence_debug_report(&store);
        let second = evidence_debug_report(&store);

        assert_eq!(first, second);
        assert_eq!(first.counts.edges, 1);
        assert_eq!(first.statuses.summary_backed, 1);
        assert_eq!(first.summary_expansion_keys, vec!["expand:summary"]);
        assert_eq!(first.hidden_node_count, 3);
        let rendered = serde_json::to_string(&first).expect("debug serializes");
        assert!(!rendered.contains("/Users/"));
        assert!(!rendered.contains("raw source"));
        assert!(!rendered.contains("timestamp"));
        assert!(!rendered.contains("parser"));
    }

    fn store() -> EvidenceStore {
        EvidenceStore::from_output(EvidenceOutput {
            nodes: vec![node(0), node(1)],
            edges: vec![EvidenceEdgeFact {
                id: EvidenceEdgeId(0),
                from: EvidenceNodeId(0),
                to: EvidenceNodeId(1),
                kind: EvidenceEdgeKind::Summary,
                query_mode: EvidenceQueryMode::Path,
                status: EvidenceStatus::Present,
                precision: EvidencePrecision::Exact,
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
                hidden_node_count: 3,
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
                message: "opaque".to_string(),
                source_fact_stable_keys: Vec::new(),
                stable_key: "unknown:summary".to_string(),
            }],
            omitted_regions: vec![EvidenceOmittedRegionFact {
                id: EvidenceOmittedRegionId(0),
                bundle: Some(EvidenceBundleId(0)),
                path: Some(EvidencePathId(0)),
                slice: None,
                reason: EvidenceOmittedReason::CompactRendering,
                hidden_node_count: 3,
                hidden_edge_count: 1,
                budget_label: Some("compact".to_string()),
                stable_key: "omitted:summary".to_string(),
            }],
            replay_keys: vec![EvidenceReplayKeyFact {
                bundle: EvidenceBundleId(0),
                query_mode: EvidenceQueryMode::Path,
                graph_schema: "evidence.graph.v1".to_string(),
                query_budget: EvidenceQueryBudget::default(),
                ranking: EvidenceRankingMode::DeterministicDisplay,
                renderer: EvidenceRendererMode::Debug,
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
            stable_key: format!("node:{id}"),
        }
    }
}
