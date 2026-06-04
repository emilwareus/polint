use crate::analysis::adaptation::budget::AdaptationModelBudget;
use crate::analysis::adaptation::facts::{
    AcceptedModelFact, LoadedModelFact, RejectedModelFact, RejectionReason,
};
use crate::analysis::adaptation::validate::{ValidationUniverse, validate_model_fact};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AdaptationModelStore {
    loaded: Vec<LoadedModelFact>,
    accepted: Vec<AcceptedModelFact>,
    rejected: Vec<RejectedModelFact>,
}

impl AdaptationModelStore {
    pub(crate) fn build(
        mut loaded: Vec<LoadedModelFact>,
        universe: &ValidationUniverse,
        budget: AdaptationModelBudget,
    ) -> Self {
        loaded.sort();

        let mut accepted = Vec::new();
        let mut rejected = Vec::new();

        let budget_exceeded = loaded.len() > budget.max_model_facts;
        for (index, fact) in loaded.iter().cloned().enumerate() {
            let reason = if budget_exceeded && index >= budget.max_model_facts {
                Err(RejectionReason::BudgetExceeded)
            } else {
                validate_model_fact(&fact, universe)
            };
            match reason {
                Ok(()) => accepted.push(AcceptedModelFact { fact }),
                Err(reason) => rejected.push(RejectedModelFact { fact, reason }),
            }
        }

        accepted.sort();
        rejected.sort();
        Self {
            loaded,
            accepted,
            rejected,
        }
    }

    pub(crate) fn loaded(&self) -> &[LoadedModelFact] {
        &self.loaded
    }

    pub(crate) fn accepted(&self) -> &[AcceptedModelFact] {
        &self.accepted
    }

    pub(crate) fn rejected(&self) -> &[RejectedModelFact] {
        &self.rejected
    }

    pub(crate) fn digest_parts(&self) -> Vec<String> {
        let mut parts = Vec::new();
        for fact in &self.accepted {
            parts.push("accepted.reason=accepted".to_string());
            parts.extend(fact.fact.digest_parts());
        }
        for fact in &self.rejected {
            parts.push(format!("rejected.reason={}", fact.reason.as_str()));
            parts.extend(fact.fact.digest_parts());
        }
        parts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::adaptation::facts::{LoadedModelFact, ModelConfidence, ModelLanguage};

    #[test]
    fn store_sorts_and_separates_accepted_and_rejected_facts() {
        let loaded = vec![
            fact("call:b", "function:missing"),
            fact("call:a", "function:a"),
        ];
        let universe = ValidationUniverse::new(["call:a", "call:b"], ["function:a"]);
        let store = AdaptationModelStore::build(
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
        let store = AdaptationModelStore::build(
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

    fn fact(source: &str, target: &str) -> LoadedModelFact {
        LoadedModelFact {
            model_path: ".polint/models/framework.toml".to_string(),
            source_pattern: source.to_string(),
            target_pattern: target.to_string(),
            confidence: ModelConfidence::Heuristic,
            language: ModelLanguage::TypeScript,
            scope: "src/app.ts".to_string(),
            evidence: vec!["src/app.ts:10".to_string()],
            stable_key: format!("{source}->{target}"),
        }
    }
}
