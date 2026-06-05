use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::calls::facts::{CallAlgorithm, CallProvenance, CallTargetStatus};
use crate::analysis::error::AnalysisError;
use crate::analysis::ids::CallSiteId;
use crate::analysis::refined_calls::facts::{RefinedCallEdgeFact, RefinedCallTier};
use crate::core::{FunctionId, SymbolId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RefinedCallOutput {
    pub(crate) edges: Vec<RefinedCallEdgeFact>,
}

impl RefinedCallOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn normalized(mut self) -> Self {
        self.edges = self
            .edges
            .into_iter()
            .map(RefinedCallEdgeFact::normalized)
            .collect();
        self.edges.sort_by(|left, right| {
            (
                left.stable_key.as_str(),
                left.site,
                left.tier,
                left.algorithm,
                left.provenance,
                left.status,
                left.id,
            )
                .cmp(&(
                    right.stable_key.as_str(),
                    right.site,
                    right.tier,
                    right.algorithm,
                    right.provenance,
                    right.status,
                    right.id,
                ))
        });
        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RefinedCallStore {
    output: RefinedCallOutput,
    by_site: BTreeMap<CallSiteId, Vec<usize>>,
    by_caller: BTreeMap<FunctionId, Vec<usize>>,
    by_target_function: BTreeMap<FunctionId, Vec<usize>>,
    by_target_symbol: BTreeMap<SymbolId, Vec<usize>>,
    by_status: BTreeMap<CallTargetStatus, Vec<usize>>,
    by_algorithm: BTreeMap<CallAlgorithm, Vec<usize>>,
    by_provenance: BTreeMap<CallProvenance, Vec<usize>>,
    by_tier: BTreeMap<RefinedCallTier, Vec<usize>>,
}

impl RefinedCallStore {
    pub(crate) fn from_output(output: RefinedCallOutput) -> Result<Self, AnalysisError> {
        let output = output.normalized();
        let mut seen_keys = BTreeSet::new();
        let mut seen_ids = BTreeSet::new();
        for edge in &output.edges {
            if !seen_keys.insert(edge.stable_key.clone()) {
                return Err(AnalysisError::InvalidFact {
                    provider: "polint.refined_calls",
                    reason: format!("duplicate refined call stable key `{}`", edge.stable_key),
                });
            }
            if !seen_ids.insert(edge.id) {
                return Err(AnalysisError::InvalidFact {
                    provider: "polint.refined_calls",
                    reason: format!("duplicate refined call id {:?}", edge.id),
                });
            }
        }

        let mut store = Self {
            output,
            ..Self::default()
        };
        for (index, edge) in store.output.edges.iter().enumerate() {
            store.by_site.entry(edge.site).or_default().push(index);
            store.by_caller.entry(edge.caller).or_default().push(index);
            if let Some(target_function) = edge.target_function {
                store
                    .by_target_function
                    .entry(target_function)
                    .or_default()
                    .push(index);
            }
            if let Some(target_symbol) = edge.target_symbol {
                store
                    .by_target_symbol
                    .entry(target_symbol)
                    .or_default()
                    .push(index);
            }
            store.by_status.entry(edge.status).or_default().push(index);
            store
                .by_algorithm
                .entry(edge.algorithm)
                .or_default()
                .push(index);
            store
                .by_provenance
                .entry(edge.provenance)
                .or_default()
                .push(index);
            store.by_tier.entry(edge.tier).or_default().push(index);
        }
        Ok(store)
    }

    pub(crate) fn edges(&self) -> &[RefinedCallEdgeFact] {
        &self.output.edges
    }

    #[allow(
        dead_code,
        reason = "Private query indexes are exposed to later refined-call plan slices."
    )]
    pub(crate) fn by_site(&self, site: CallSiteId) -> Vec<&RefinedCallEdgeFact> {
        self.edge_refs(self.by_site.get(&site))
    }

    pub(crate) fn by_tier(&self, tier: RefinedCallTier) -> Vec<&RefinedCallEdgeFact> {
        self.edge_refs(self.by_tier.get(&tier))
    }

    #[allow(
        dead_code,
        reason = "Private query indexes are exposed to later refined-call plan slices."
    )]
    pub(crate) fn by_status(&self, status: CallTargetStatus) -> Vec<&RefinedCallEdgeFact> {
        self.edge_refs(self.by_status.get(&status))
    }

    #[allow(
        dead_code,
        reason = "Private query indexes are exposed to later refined-call plan slices."
    )]
    pub(crate) fn by_algorithm(&self, algorithm: CallAlgorithm) -> Vec<&RefinedCallEdgeFact> {
        self.edge_refs(self.by_algorithm.get(&algorithm))
    }

    #[allow(
        dead_code,
        reason = "Private query indexes are exposed to later refined-call plan slices."
    )]
    pub(crate) fn by_provenance(&self, provenance: CallProvenance) -> Vec<&RefinedCallEdgeFact> {
        self.edge_refs(self.by_provenance.get(&provenance))
    }

    #[allow(
        dead_code,
        reason = "Private query indexes are exposed to later refined-call plan slices."
    )]
    pub(crate) fn by_caller(&self, caller: FunctionId) -> Vec<&RefinedCallEdgeFact> {
        self.edge_refs(self.by_caller.get(&caller))
    }

    #[allow(
        dead_code,
        reason = "Private query indexes are exposed to later refined-call plan slices."
    )]
    pub(crate) fn by_target_function(&self, target: FunctionId) -> Vec<&RefinedCallEdgeFact> {
        self.edge_refs(self.by_target_function.get(&target))
    }

    #[allow(
        dead_code,
        reason = "Private query indexes are exposed to later refined-call plan slices."
    )]
    pub(crate) fn by_target_symbol(&self, target: SymbolId) -> Vec<&RefinedCallEdgeFact> {
        self.edge_refs(self.by_target_symbol.get(&target))
    }

    fn edge_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&RefinedCallEdgeFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|index| &self.output.edges[*index])
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{CallEdgeKind, CallPrecision};
    use crate::analysis::ids::RefinedCallEdgeId;
    use crate::analysis::refined_calls::facts::{RefinedCallConfidence, RefinedCallValidation};
    use crate::core::Language;

    #[test]
    fn store_normalizes_edges_and_indexes_by_tier() {
        let store = RefinedCallStore::from_output(RefinedCallOutput {
            edges: vec![
                edge(1, "b", RefinedCallTier::PointsToAssisted),
                edge(0, "a", RefinedCallTier::DirectPlusFramework),
            ],
        })
        .expect("valid refined calls");

        assert_eq!(store.edges()[0].stable_key, "a");
        assert_eq!(store.by_tier(RefinedCallTier::DirectPlusFramework).len(), 1);
    }

    #[test]
    fn store_rejects_duplicate_stable_keys() {
        let result = RefinedCallStore::from_output(RefinedCallOutput {
            edges: vec![
                edge(0, "duplicate", RefinedCallTier::DirectOnly),
                edge(1, "duplicate", RefinedCallTier::DirectOnly),
            ],
        });

        assert!(result.is_err());
    }

    fn edge(id: u64, stable_key: &str, tier: RefinedCallTier) -> RefinedCallEdgeFact {
        RefinedCallEdgeFact {
            id: RefinedCallEdgeId(id),
            site: CallSiteId(0),
            base_target: None,
            caller: FunctionId(0),
            target_function: None,
            target_symbol: None,
            synthetic_target: None,
            language: Language::TypeScript,
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            tier,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::SetupAware,
            validation: RefinedCallValidation::Native,
            confidence: RefinedCallConfidence::High,
            evidence: vec![],
            input_stable_keys: vec![],
            stable_key: stable_key.to_string(),
        }
    }
}
