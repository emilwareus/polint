use crate::analysis_api::{Digest, DigestKind};
use crate::analysis_neutral::adaptation::ADAPTATION_MODEL_ALGORITHM;
use crate::analysis_neutral::adaptation::budget::AdaptationModelBudget;
use crate::analysis_neutral::adaptation::store::AdaptationModelStore;

pub const ADAPTATION_MODEL_SCHEMA_LABEL: &str = "adaptation-model-facts-1";
pub const ADAPTATION_MODEL_VALIDATOR_VERSION: &str = "adaptation_model_validator_v1";

pub fn adaptation_model_digest(
    store: &AdaptationModelStore,
    budget: AdaptationModelBudget,
    interner: &crate::internal_core::StableKeyInterner,
) -> Digest {
    let budget_parts = budget.digest_parts();
    let store_parts = store.digest_parts(interner);
    let mut parts: Vec<&str> = vec![
        ADAPTATION_MODEL_SCHEMA_LABEL,
        ADAPTATION_MODEL_ALGORITHM,
        ADAPTATION_MODEL_VALIDATOR_VERSION,
    ];
    parts.extend(budget_parts.iter().map(String::as_str));
    parts.extend(store_parts.iter().map(String::as_str));
    Digest::from_parts(DigestKind::ProviderParameters, "adaptation_model", &parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_neutral::LocalAnalysisDb;
    use crate::analysis_neutral::adaptation::facts::{
        LoadedModelFact, ModelConfidence, ModelLanguage,
    };
    use crate::analysis_neutral::adaptation::validate::ValidationUniverse;

    #[test]
    fn digest_changes_when_model_behavior_changes() {
        let interner = LocalAnalysisDb::new().stable_key_interner();
        let universe = ValidationUniverse::new(["call:a"], ["function:a", "function:b"]);
        let budget = AdaptationModelBudget::default();
        let first = AdaptationModelStore::build(
            &interner,
            vec![fact("call:a", "function:a")],
            &universe,
            budget,
        );
        let second = AdaptationModelStore::build(
            &interner,
            vec![fact("call:a", "function:b")],
            &universe,
            budget,
        );
        assert_ne!(
            adaptation_model_digest(&first, budget, &interner),
            adaptation_model_digest(&second, budget, &interner)
        );
    }

    #[test]
    fn digest_is_stable_for_file_reorder() {
        let interner = LocalAnalysisDb::new().stable_key_interner();
        let universe = ValidationUniverse::new(["call:a", "call:b"], ["function:a", "function:b"]);
        let budget = AdaptationModelBudget::default();
        let first = AdaptationModelStore::build(
            &interner,
            vec![fact("call:b", "function:b"), fact("call:a", "function:a")],
            &universe,
            budget,
        );
        let second = AdaptationModelStore::build(
            &interner,
            vec![fact("call:a", "function:a"), fact("call:b", "function:b")],
            &universe,
            budget,
        );
        assert_eq!(
            adaptation_model_digest(&first, budget, &interner),
            adaptation_model_digest(&second, budget, &interner)
        );
    }

    #[test]
    fn digest_changes_when_budget_changes() {
        let interner = LocalAnalysisDb::new().stable_key_interner();
        let universe = ValidationUniverse::new(["call:a"], ["function:a"]);
        let store = AdaptationModelStore::build(
            &interner,
            vec![fact("call:a", "function:a")],
            &universe,
            AdaptationModelBudget::default(),
        );
        let mut changed = AdaptationModelBudget::default();
        changed.max_model_facts += 1;
        assert_ne!(
            adaptation_model_digest(&store, AdaptationModelBudget::default(), &interner),
            adaptation_model_digest(&store, changed, &interner)
        );
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
