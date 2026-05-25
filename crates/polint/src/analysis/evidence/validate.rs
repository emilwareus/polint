use crate::analysis::evidence::facts::{
    EvidenceExpansion, EvidencePrecision, EvidenceProvenance, EvidenceStatus, EvidenceValidation,
    ExtensionEvidenceCandidateFact, ExtensionEvidenceMergeFact, ExtensionEvidenceMergeReason,
    ExtensionEvidenceMergeVerdict,
};
use crate::analysis::evidence::store::EvidenceStore;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ExtensionEvidenceMergeReport {
    pub(crate) rows: Vec<ExtensionEvidenceMergeFact>,
}

pub(crate) fn validate_extension_evidence(
    store: &EvidenceStore,
    candidates: Vec<ExtensionEvidenceCandidateFact>,
) -> ExtensionEvidenceMergeReport {
    let mut rows = candidates
        .into_iter()
        .map(ExtensionEvidenceCandidateFact::normalized)
        .map(|candidate| validate_candidate(store, candidate))
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        (
            left.extension_id.as_str(),
            left.provider_id.as_str(),
            left.stable_key.as_str(),
            left.verdict,
            left.reason,
        )
            .cmp(&(
                right.extension_id.as_str(),
                right.provider_id.as_str(),
                right.stable_key.as_str(),
                right.verdict,
                right.reason,
            ))
    });
    ExtensionEvidenceMergeReport { rows }
}

fn validate_candidate(
    store: &EvidenceStore,
    candidate: ExtensionEvidenceCandidateFact,
) -> ExtensionEvidenceMergeFact {
    let reason = rejection_or_downgrade_reason(store, &candidate);
    let (verdict, effective_status, effective_precision) = match reason {
        Some(ExtensionEvidenceMergeReason::InvalidEndpoint)
        | Some(ExtensionEvidenceMergeReason::InvalidSpan)
        | Some(ExtensionEvidenceMergeReason::UnboundedExpansion) => (
            ExtensionEvidenceMergeVerdict::Rejected,
            EvidenceStatus::Rejected,
            EvidencePrecision::Unknown,
        ),
        Some(ExtensionEvidenceMergeReason::CandidateCannotStrengthenDiagnostic) => (
            ExtensionEvidenceMergeVerdict::CandidateOnly,
            candidate.claimed_status,
            downgrade_precision(candidate.claimed_precision),
        ),
        Some(ExtensionEvidenceMergeReason::ExactClaimRequiresNativeAnchor) => (
            ExtensionEvidenceMergeVerdict::AcceptedWithPrecisionDowngrade,
            candidate.claimed_status,
            downgrade_precision(candidate.claimed_precision),
        ),
        None => (
            ExtensionEvidenceMergeVerdict::Accepted,
            candidate.claimed_status,
            candidate.claimed_precision,
        ),
    };
    ExtensionEvidenceMergeFact {
        extension_id: candidate.extension_id,
        provider_id: candidate.provider_id,
        stable_key: candidate.stable_key,
        verdict,
        reason,
        effective_status,
        effective_precision,
    }
}

fn rejection_or_downgrade_reason(
    store: &EvidenceStore,
    candidate: &ExtensionEvidenceCandidateFact,
) -> Option<ExtensionEvidenceMergeReason> {
    if store.node(candidate.from).is_none() || store.node(candidate.to).is_none() {
        return Some(ExtensionEvidenceMergeReason::InvalidEndpoint);
    }
    if candidate
        .source_path
        .as_deref()
        .is_some_and(|path| path.starts_with('/') || path.contains(".."))
    {
        return Some(ExtensionEvidenceMergeReason::InvalidSpan);
    }
    if candidate
        .source_span
        .as_ref()
        .is_some_and(|span| span.start_byte > span.end_byte)
    {
        return Some(ExtensionEvidenceMergeReason::InvalidSpan);
    }
    if matches!(candidate.expansion, EvidenceExpansion::Expandable { ref key } if key.is_empty())
        || candidate.replay_key.as_deref() == Some("")
    {
        return Some(ExtensionEvidenceMergeReason::UnboundedExpansion);
    }
    if candidate.claimed_status != EvidenceStatus::Present {
        return Some(ExtensionEvidenceMergeReason::CandidateCannotStrengthenDiagnostic);
    }
    if candidate.claimed_precision == EvidencePrecision::Exact
        && !has_native_anchor(store, candidate)
    {
        return Some(ExtensionEvidenceMergeReason::ExactClaimRequiresNativeAnchor);
    }
    None
}

fn has_native_anchor(store: &EvidenceStore, candidate: &ExtensionEvidenceCandidateFact) -> bool {
    !candidate.native_anchor_stable_keys.is_empty()
        && store.edges().iter().any(|edge| {
            edge.provenance == EvidenceProvenance::Native
                && matches!(
                    edge.validation,
                    EvidenceValidation::Native | EvidenceValidation::ReferentiallyValidated
                )
                && candidate.native_anchor_stable_keys.iter().any(|key| {
                    edge.stable_key == *key || edge.source_fact_stable_keys.contains(key)
                })
        })
}

fn downgrade_precision(precision: EvidencePrecision) -> EvidencePrecision {
    match precision {
        EvidencePrecision::Exact => EvidencePrecision::Conservative,
        other => other,
    }
}

pub(crate) fn merge_validated_extension_edges(
    native_edges: usize,
    rows: &[ExtensionEvidenceMergeFact],
) -> ExtensionEvidenceDelta {
    ExtensionEvidenceDelta {
        native_edge_count: native_edges,
        accepted: count(rows, ExtensionEvidenceMergeVerdict::Accepted),
        downgraded: count(
            rows,
            ExtensionEvidenceMergeVerdict::AcceptedWithPrecisionDowngrade,
        ),
        candidate_only: count(rows, ExtensionEvidenceMergeVerdict::CandidateOnly),
        rejected: count(rows, ExtensionEvidenceMergeVerdict::Rejected),
        representative_reasons: rows
            .iter()
            .filter_map(|row| row.reason.map(|reason| format!("{reason:?}")))
            .take(8)
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtensionEvidenceDelta {
    pub(crate) native_edge_count: usize,
    pub(crate) accepted: usize,
    pub(crate) downgraded: usize,
    pub(crate) candidate_only: usize,
    pub(crate) rejected: usize,
    pub(crate) representative_reasons: Vec<String>,
}

fn count(rows: &[ExtensionEvidenceMergeFact], verdict: ExtensionEvidenceMergeVerdict) -> usize {
    rows.iter().filter(|row| row.verdict == verdict).count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::evidence::facts::{
        EvidenceConfidence, EvidenceEdgeFact, EvidenceEdgeKind, EvidenceNodeFact, EvidenceNodeKind,
        EvidenceQueryMode,
    };
    use crate::analysis::evidence::store::EvidenceOutput;
    use crate::analysis::ids::{EvidenceEdgeId, EvidenceNodeId};
    use crate::core::{FileId, Language, Span};

    #[test]
    fn merge_verdicts_are_sorted_and_serializable() {
        let store = store();
        let rows = validate_extension_evidence(
            &store,
            vec![
                candidate("z", EvidenceNodeId(99)),
                candidate("a", EvidenceNodeId(0)),
            ],
        )
        .rows;

        assert_eq!(rows[0].stable_key, "a");
        assert_eq!(rows[1].verdict, ExtensionEvidenceMergeVerdict::Rejected);
        serde_json::to_string(&rows).expect("verdict rows serialize");
    }

    #[test]
    fn exact_claims_are_downgraded_without_native_anchor() {
        let store = store();
        let mut candidate = candidate("edge:extension", EvidenceNodeId(0));
        candidate.claimed_precision = EvidencePrecision::Exact;

        let row = validate_extension_evidence(&store, vec![candidate])
            .rows
            .remove(0);

        assert_eq!(
            row.verdict,
            ExtensionEvidenceMergeVerdict::AcceptedWithPrecisionDowngrade
        );
        assert_eq!(row.effective_precision, EvidencePrecision::Conservative);
    }

    #[test]
    fn native_anchor_allows_exact_but_does_not_remove_native_may_edges() {
        let store = store();
        let mut candidate = candidate("edge:extension", EvidenceNodeId(0));
        candidate.claimed_precision = EvidencePrecision::Exact;
        candidate.native_anchor_stable_keys = vec!["edge:native".to_string()];

        let report = validate_extension_evidence(&store, vec![candidate]);
        let delta = merge_validated_extension_edges(store.edges().len(), &report.rows);

        assert_eq!(
            report.rows[0].verdict,
            ExtensionEvidenceMergeVerdict::Accepted
        );
        assert_eq!(delta.native_edge_count, 1);
        assert_eq!(store.edges()[0].stable_key, "edge:native");
    }

    #[test]
    fn invalid_spans_and_unbounded_expansion_are_rejected() {
        let store = store();
        let mut bad_span = candidate("edge:bad-span", EvidenceNodeId(0));
        bad_span.source_path = Some("/Users/me/repo/src.rs".to_string());
        let mut bad_expansion = candidate("edge:bad-expansion", EvidenceNodeId(0));
        bad_expansion.expansion = EvidenceExpansion::Expandable { key: String::new() };

        let rows = validate_extension_evidence(&store, vec![bad_span, bad_expansion]).rows;

        assert!(
            rows.iter()
                .all(|row| row.verdict == ExtensionEvidenceMergeVerdict::Rejected)
        );
        assert!(
            rows.iter()
                .any(|row| row.reason == Some(ExtensionEvidenceMergeReason::InvalidSpan))
        );
        assert!(
            rows.iter()
                .any(|row| row.reason == Some(ExtensionEvidenceMergeReason::UnboundedExpansion))
        );
    }

    #[test]
    fn non_present_extension_evidence_is_candidate_only() {
        let store = store();
        let mut candidate = candidate("edge:uncertain", EvidenceNodeId(0));
        candidate.claimed_status = EvidenceStatus::Unknown;

        let row = validate_extension_evidence(&store, vec![candidate])
            .rows
            .remove(0);

        assert_eq!(row.verdict, ExtensionEvidenceMergeVerdict::CandidateOnly);
    }

    fn store() -> EvidenceStore {
        EvidenceStore::from_output(EvidenceOutput {
            nodes: vec![node(0), node(1)],
            edges: vec![EvidenceEdgeFact {
                id: EvidenceEdgeId(0),
                from: EvidenceNodeId(0),
                to: EvidenceNodeId(1),
                kind: EvidenceEdgeKind::DataValue,
                query_mode: EvidenceQueryMode::Path,
                status: EvidenceStatus::Present,
                precision: EvidencePrecision::Conservative,
                provenance: EvidenceProvenance::Native,
                validation: EvidenceValidation::ReferentiallyValidated,
                confidence: EvidenceConfidence::Medium,
                call_site: None,
                summary_stable_key: None,
                expansion: EvidenceExpansion::None,
                compact_label: None,
                source_fact_stable_keys: vec!["df:native".to_string()],
                stable_key: "edge:native".to_string(),
            }],
            bundles: Vec::new(),
            paths: Vec::new(),
            slices: Vec::new(),
            unknowns: Vec::new(),
            omitted_regions: Vec::new(),
            replay_keys: Vec::new(),
        })
        .expect("valid evidence")
    }

    fn candidate(stable_key: &str, from: EvidenceNodeId) -> ExtensionEvidenceCandidateFact {
        ExtensionEvidenceCandidateFact {
            extension_id: "demo".to_string(),
            provider_id: "model".to_string(),
            stable_key: stable_key.to_string(),
            from,
            to: EvidenceNodeId(1),
            kind: EvidenceEdgeKind::Model,
            claimed_status: EvidenceStatus::Present,
            claimed_precision: EvidencePrecision::Heuristic,
            confidence: EvidenceConfidence::Medium,
            source_path: Some("src/lib.rs".to_string()),
            source_span: Some(Span {
                file: FileId(1),
                start_line: 1,
                start_col: 1,
                end_line: 1,
                end_col: 2,
                start_byte: 0,
                end_byte: 1,
            }),
            summary_stable_key: None,
            expansion: EvidenceExpansion::None,
            replay_key: Some("replay:extension".to_string()),
            native_anchor_stable_keys: Vec::new(),
            evidence: vec!["extension-model".to_string()],
        }
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
            stable_key: format!("node:{id}"),
        }
    }
}
