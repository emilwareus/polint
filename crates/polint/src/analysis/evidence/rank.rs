use crate::analysis::evidence::facts::{
    EvidenceEdgeKind, EvidenceExpansion, EvidencePrecision, EvidenceProvenance, EvidenceStatus,
    EvidenceValidation,
};
use crate::analysis::evidence::store::EvidenceStore;
use crate::analysis::ids::EvidenceEdgeId;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct PathRankScore {
    pub(crate) unknown_penalty: u32,
    pub(crate) unvalidated_penalty: u32,
    pub(crate) model_or_heuristic_penalty: u32,
    pub(crate) length_penalty: u32,
    pub(crate) summary_opaque_penalty: u32,
    pub(crate) native_exact_bonus: u32,
    pub(crate) direct_source_sink_bonus: u32,
}

pub(crate) fn rank_score_for_edges(
    store: &EvidenceStore,
    edges: &[EvidenceEdgeId],
) -> PathRankScore {
    let mut score = PathRankScore {
        length_penalty: edges.len() as u32,
        ..PathRankScore::default()
    };
    for edge_id in edges {
        let Some(edge) = store.edge(*edge_id) else {
            score.unknown_penalty += 1;
            continue;
        };
        if edge.status != EvidenceStatus::Present || edge.kind == EvidenceEdgeKind::Unknown {
            score.unknown_penalty += 1;
        }
        if !matches!(
            edge.validation,
            EvidenceValidation::Native | EvidenceValidation::ReferentiallyValidated
        ) {
            score.unvalidated_penalty += 1;
        }
        if matches!(
            edge.provenance,
            EvidenceProvenance::Model | EvidenceProvenance::Extension
        ) || matches!(
            edge.precision,
            EvidencePrecision::Heuristic | EvidencePrecision::Conservative
        ) || edge.kind == EvidenceEdgeKind::Model
        {
            score.model_or_heuristic_penalty += 1;
        }
        if edge.provenance == EvidenceProvenance::Native
            && edge.precision == EvidencePrecision::Exact
        {
            score.native_exact_bonus += 1;
        }
        if matches!(edge.expansion, EvidenceExpansion::Opaque { .. }) {
            score.summary_opaque_penalty += 1;
        }
    }
    if edges.len() == 1 {
        score.direct_source_sink_bonus = 1;
    }
    score
}

pub(crate) fn compare_scores(left: PathRankScore, right: PathRankScore) -> std::cmp::Ordering {
    (
        left.unknown_penalty,
        left.unvalidated_penalty,
        left.model_or_heuristic_penalty,
        left.summary_opaque_penalty,
        left.length_penalty,
        std::cmp::Reverse(left.native_exact_bonus),
        std::cmp::Reverse(left.direct_source_sink_bonus),
    )
        .cmp(&(
            right.unknown_penalty,
            right.unvalidated_penalty,
            right.model_or_heuristic_penalty,
            right.summary_opaque_penalty,
            right.length_penalty,
            std::cmp::Reverse(right.native_exact_bonus),
            std::cmp::Reverse(right.direct_source_sink_bonus),
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::evidence::facts::{
        EvidenceConfidence, EvidenceEdgeFact, EvidenceNodeFact, EvidenceNodeKind, EvidenceQueryMode,
    };
    use crate::analysis::evidence::store::EvidenceOutput;
    use crate::analysis::ids::EvidenceNodeId;
    use crate::core::Language;

    #[test]
    fn exact_native_short_paths_sort_before_unknown_heuristic_paths() {
        let store = store_with_rank_edges();

        let exact = rank_score_for_edges(&store, &[EvidenceEdgeId(0)]);
        let unknown = rank_score_for_edges(&store, &[EvidenceEdgeId(1), EvidenceEdgeId(2)]);

        assert_eq!(compare_scores(exact, unknown), std::cmp::Ordering::Less);
    }

    #[test]
    fn identical_scores_can_use_stable_edge_key_tie_breaks_without_mutation() {
        let store = store_with_rank_edges();
        let first = rank_score_for_edges(&store, &[EvidenceEdgeId(0)]);
        let second = rank_score_for_edges(&store, &[EvidenceEdgeId(0)]);

        assert_eq!(compare_scores(first, second), std::cmp::Ordering::Equal);
        assert_eq!(
            store.edge(EvidenceEdgeId(0)).expect("edge").status,
            EvidenceStatus::Present
        );
    }

    fn store_with_rank_edges() -> EvidenceStore {
        EvidenceStore::from_output(
            EvidenceOutput {
                nodes: vec![node(0), node(1), node(2)],
                edges: vec![
                    edge(
                        0,
                        0,
                        1,
                        EvidenceStatus::Present,
                        EvidencePrecision::Exact,
                        EvidenceProvenance::Native,
                    ),
                    edge(
                        1,
                        0,
                        2,
                        EvidenceStatus::Unknown,
                        EvidencePrecision::Heuristic,
                        EvidenceProvenance::Model,
                    ),
                    edge(
                        2,
                        2,
                        1,
                        EvidenceStatus::Present,
                        EvidencePrecision::Heuristic,
                        EvidenceProvenance::Extension,
                    ),
                ],
                bundles: Vec::new(),
                paths: Vec::new(),
                slices: Vec::new(),
                unknowns: Vec::new(),
                omitted_regions: Vec::new(),
                replay_keys: Vec::new(),
            },
            &crate::core::test_stable_key_interner(),
        )
        .expect("valid evidence")
    }

    fn node(id: u64) -> EvidenceNodeFact {
        EvidenceNodeFact {
            id: EvidenceNodeId(id),
            kind: EvidenceNodeKind::Operation,
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
            stable_key: crate::core::stable_key_for_test(&format!("node:{id}")),
        }
    }

    fn edge(
        id: u64,
        from: u64,
        to: u64,
        status: EvidenceStatus,
        precision: EvidencePrecision,
        provenance: EvidenceProvenance,
    ) -> EvidenceEdgeFact {
        EvidenceEdgeFact {
            id: EvidenceEdgeId(id),
            from: EvidenceNodeId(from),
            to: EvidenceNodeId(to),
            kind: if status == EvidenceStatus::Unknown {
                EvidenceEdgeKind::Unknown
            } else {
                EvidenceEdgeKind::DataValue
            },
            query_mode: EvidenceQueryMode::Path,
            status,
            precision,
            provenance,
            validation: EvidenceValidation::Native,
            confidence: EvidenceConfidence::High,
            call_site: None,
            summary_stable_key: None,
            expansion: EvidenceExpansion::None,
            compact_label: None,
            source_fact_stable_keys: Vec::new(),
            stable_key: crate::core::stable_key_for_test(&format!("edge:{id}")),
        }
    }
}
