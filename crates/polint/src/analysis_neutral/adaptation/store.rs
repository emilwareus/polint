use crate::analysis_neutral::adaptation::budget::AdaptationModelBudget;
use crate::analysis_neutral::adaptation::facts::{
    AcceptedModelFact, LoadedModelFact, RejectedModelFact, RejectionReason,
};
use crate::analysis_neutral::adaptation::validate::{ValidationUniverse, validate_model_fact};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdaptationModelStore {
    loaded: Vec<LoadedModelFact>,
    accepted: Vec<AcceptedModelFact>,
    rejected: Vec<RejectedModelFact>,
}

impl AdaptationModelStore {
    pub fn build(
        interner: &crate::internal_core::StableKeyInterner,
        mut loaded: Vec<LoadedModelFact>,
        universe: &ValidationUniverse,
        budget: AdaptationModelBudget,
    ) -> Self {
        loaded.sort_by(|left, right| left.sort_key(interner).cmp(&right.sort_key(interner)));

        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        let mut targets_by_source: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        let budget_exceeded = loaded.len() > budget.max_model_facts;
        for (index, fact) in loaded.iter().cloned().enumerate() {
            let mut reason = if budget_exceeded && index >= budget.max_model_facts {
                Err(RejectionReason::BudgetExceeded)
            } else {
                validate_model_fact(&fact, universe)
            };
            if reason.is_ok() {
                let targets = targets_by_source
                    .entry(fact.source_pattern.clone())
                    .or_default();
                if !targets.contains(&fact.target_pattern)
                    && targets.len() >= budget.max_targets_per_source
                {
                    reason = Err(RejectionReason::BudgetExceeded);
                } else {
                    targets.insert(fact.target_pattern.clone());
                }
            }
            match reason {
                Ok(()) => accepted.push(AcceptedModelFact { fact }),
                Err(reason) => rejected.push(RejectedModelFact { fact, reason }),
            }
        }

        accepted.sort_by(|left, right| {
            left.fact
                .sort_key(interner)
                .cmp(&right.fact.sort_key(interner))
        });
        rejected.sort_by(|left, right| {
            left.fact
                .sort_key(interner)
                .cmp(&right.fact.sort_key(interner))
        });
        Self {
            loaded,
            accepted,
            rejected,
        }
    }

    pub fn loaded(&self) -> &[LoadedModelFact] {
        &self.loaded
    }

    pub fn accepted(&self) -> &[AcceptedModelFact] {
        &self.accepted
    }

    pub fn rejected(&self) -> &[RejectedModelFact] {
        &self.rejected
    }

    pub fn digest_parts(&self, interner: &crate::internal_core::StableKeyInterner) -> Vec<String> {
        let mut parts = Vec::new();
        for fact in &self.accepted {
            parts.push("accepted.reason=accepted".to_string());
            parts.extend(fact.fact.digest_parts(interner));
        }
        for fact in &self.rejected {
            parts.push(format!("rejected.reason={}", fact.reason.as_str()));
            parts.extend(fact.fact.digest_parts(interner));
        }
        parts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_neutral::adaptation::facts::{
        LoadedModelFact, ModelConfidence, ModelLanguage,
    };

    #[test]
    fn store_sorts_and_separates_accepted_and_rejected_facts() {
        let loaded = vec![
            fact("call:b", "function:missing"),
            fact("call:a", "function:a"),
        ];
        let universe = ValidationUniverse::new(["call:a", "call:b"], ["function:a"]);
        let interner = crate::internal_core::test_stable_key_interner();
        let store = AdaptationModelStore::build(
            &interner,
            loaded,
            &universe,
            AdaptationModelBudget {
                max_model_facts: 10,
                ..AdaptationModelBudget::default()
            },
        );
        assert_eq!(store.loaded()[0].source_pattern, "call:a");
        assert_eq!(store.accepted().len(), 1);
        assert_eq!(store.rejected().len(), 1);
        assert_eq!(
            store.rejected()[0].reason,
            RejectionReason::NonResolvingTarget
        );
    }

    #[test]
    fn budget_overflow_is_explicit_rejection_evidence() {
        let loaded = vec![fact("call:a", "function:a"), fact("call:b", "function:b")];
        let universe = ValidationUniverse::new(["call:a", "call:b"], ["function:a", "function:b"]);
        let interner = crate::internal_core::test_stable_key_interner();
        let store = AdaptationModelStore::build(
            &interner,
            loaded,
            &universe,
            AdaptationModelBudget {
                max_model_facts: 1,
                ..AdaptationModelBudget::default()
            },
        );
        assert_eq!(store.accepted().len(), 1);
        assert_eq!(store.rejected()[0].reason, RejectionReason::BudgetExceeded);
    }

    #[test]
    fn per_source_target_fanout_overflow_is_explicit_rejection_evidence() {
        let loaded = vec![fact("call:a", "function:a"), fact("call:a", "function:b")];
        let universe = ValidationUniverse::new(["call:a"], ["function:a", "function:b"]);
        let interner = crate::internal_core::test_stable_key_interner();
        let store = AdaptationModelStore::build(
            &interner,
            loaded,
            &universe,
            AdaptationModelBudget {
                max_targets_per_source: 1,
                ..AdaptationModelBudget::default()
            },
        );
        assert_eq!(store.accepted().len(), 1);
        assert_eq!(store.rejected().len(), 1);
        assert_eq!(store.rejected()[0].reason, RejectionReason::BudgetExceeded);
    }

    fn fact(source: &str, target: &str) -> LoadedModelFact {
        LoadedModelFact {
            model_path: ".polint/models/framework.toml".to_string(),
            source_pattern: source.to_string(),
            target_pattern: target.to_string(),
            confidence: ModelConfidence::Heuristic,
            language: ModelLanguage::TypeScript,
            scope: "src/app.ts".to_string(),
            evidence: vec!["src/app.ts:10".to_string()],
            stable_key: crate::internal_core::stable_key_for_test(&format!("{source}->{target}")),
        }
    }
}
