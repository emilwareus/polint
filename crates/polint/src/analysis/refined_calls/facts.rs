use serde::{Deserialize, Serialize};

use crate::analysis::calls::facts::{
    CallAlgorithm, CallEdgeKind, CallPrecision, CallProvenance, CallTargetStatus,
    UnresolvedCallReason,
};
use crate::analysis::ids::{CallSiteId, CallTargetId, RefinedCallEdgeId};
use crate::core::{FunctionId, Language, StableKeyId, SymbolId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RefinedCallEdgeFact {
    pub(crate) id: RefinedCallEdgeId,
    pub(crate) site: CallSiteId,
    pub(crate) base_target: Option<CallTargetId>,
    pub(crate) caller: FunctionId,
    pub(crate) target_function: Option<FunctionId>,
    pub(crate) target_symbol: Option<SymbolId>,
    pub(crate) synthetic_target: Option<String>,
    pub(crate) language: Language,
    pub(crate) edge_kind: CallEdgeKind,
    pub(crate) algorithm: CallAlgorithm,
    pub(crate) tier: RefinedCallTier,
    pub(crate) status: CallTargetStatus,
    pub(crate) reason: Option<UnresolvedCallReason>,
    pub(crate) provenance: CallProvenance,
    pub(crate) precision: CallPrecision,
    pub(crate) validation: RefinedCallValidation,
    pub(crate) confidence: RefinedCallConfidence,
    pub(crate) evidence: Vec<String>,
    pub(crate) input_stable_keys: Vec<String>,
    pub(crate) stable_key: StableKeyId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum RefinedCallTier {
    DirectOnly,
    DirectPlusFramework,
    TypeValueFunctionToken,
    SummaryAssisted,
    PointsToAssisted,
    ExtensionModel,
    AllAccepted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum RefinedCallValidation {
    Native,
    ReferentiallyValidated,
    ExtensionValidated,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum RefinedCallConfidence {
    High,
    Medium,
    Low,
}

impl RefinedCallEdgeFact {
    pub(crate) fn normalized(mut self) -> Self {
        self.evidence.sort();
        self.evidence.dedup();
        self.input_stable_keys.sort();
        self.input_stable_keys.dedup();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Language;

    #[test]
    fn refined_edge_preserves_base_call_site_and_stable_key() {
        let edge = RefinedCallEdgeFact {
            id: RefinedCallEdgeId(1),
            site: CallSiteId(2),
            base_target: Some(CallTargetId(3)),
            caller: FunctionId(4),
            target_function: Some(FunctionId(5)),
            target_symbol: None,
            synthetic_target: None,
            language: Language::TypeScript,
            edge_kind: CallEdgeKind::FunctionValue,
            algorithm: CallAlgorithm::PointsTo,
            tier: RefinedCallTier::TypeValueFunctionToken,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::SetupAware,
            validation: RefinedCallValidation::Native,
            confidence: RefinedCallConfidence::High,
            evidence: vec!["value".to_string()],
            input_stable_keys: vec!["call-site".to_string()],
            stable_key: crate::core::stable_key_for_test("refined:edge"),
        };

        assert_eq!(edge.site, CallSiteId(2));
        assert_eq!(edge.base_target, Some(CallTargetId(3)));
        assert_eq!(
            edge.stable_key,
            crate::core::stable_key_for_test("refined:edge")
        );
    }

    #[test]
    fn normalized_edge_sorts_evidence_and_inputs() {
        let edge = RefinedCallEdgeFact {
            id: RefinedCallEdgeId(0),
            site: CallSiteId(0),
            base_target: None,
            caller: FunctionId(0),
            target_function: None,
            target_symbol: None,
            synthetic_target: Some("synthetic".to_string()),
            language: Language::Unknown,
            edge_kind: CallEdgeKind::Synthetic,
            algorithm: CallAlgorithm::RepoModel,
            tier: RefinedCallTier::ExtensionModel,
            status: CallTargetStatus::Ambiguous,
            reason: Some(UnresolvedCallReason::Unknown),
            provenance: CallProvenance::Extension,
            precision: CallPrecision::Heuristic,
            validation: RefinedCallValidation::ExtensionValidated,
            confidence: RefinedCallConfidence::Medium,
            evidence: vec!["b".to_string(), "a".to_string(), "a".to_string()],
            input_stable_keys: vec!["z".to_string(), "a".to_string()],
            stable_key: crate::core::stable_key_for_test("refined:synthetic"),
        }
        .normalized();

        assert_eq!(edge.evidence, vec!["a", "b"]);
        assert_eq!(edge.input_stable_keys, vec!["a", "z"]);
    }
}
