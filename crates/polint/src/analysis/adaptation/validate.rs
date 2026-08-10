use std::collections::BTreeSet;

use crate::analysis::adaptation::facts::{LoadedModelFact, RejectionReason};

#[derive(Debug, Clone, Default)]
pub(crate) struct ValidationUniverse {
    source_keys: BTreeSet<String>,
    target_keys: BTreeSet<String>,
    oracle_labels: BTreeSet<String>,
}

impl ValidationUniverse {
    pub(crate) fn new(
        source_keys: impl IntoIterator<Item = impl Into<String>>,
        target_keys: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            source_keys: source_keys.into_iter().map(Into::into).collect(),
            target_keys: target_keys.into_iter().map(Into::into).collect(),
            oracle_labels: BTreeSet::new(),
        }
    }

    pub(crate) fn with_oracle_labels(
        mut self,
        oracle_labels: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.oracle_labels = oracle_labels.into_iter().map(Into::into).collect();
        self
    }

    pub(crate) fn contains_source(&self, key: &str) -> bool {
        self.source_keys.contains(key)
    }

    pub(crate) fn contains_target(&self, key: &str) -> bool {
        self.target_keys.contains(key)
    }

    pub(crate) fn is_oracle_label(&self, key: &str) -> bool {
        self.oracle_labels.contains(key)
    }
}

pub(crate) fn validate_model_fact(
    fact: &LoadedModelFact,
    universe: &ValidationUniverse,
) -> Result<(), RejectionReason> {
    if fact.source_pattern.trim().is_empty() {
        return Err(RejectionReason::EmptySourcePattern);
    }
    if fact.target_pattern.trim().is_empty() {
        return Err(RejectionReason::EmptyTargetPattern);
    }
    if fact.scope.trim().is_empty() {
        return Err(RejectionReason::EmptyScope);
    }
    if fact.evidence.iter().all(|item| item.trim().is_empty()) {
        return Err(RejectionReason::EmptyEvidence);
    }
    if is_broad_pattern(&fact.source_pattern) {
        return Err(RejectionReason::BroadSourcePattern);
    }
    if is_broad_pattern(&fact.target_pattern) {
        return Err(RejectionReason::BroadTargetPattern);
    }
    if is_oracle_shaped_target(&fact.target_pattern)
        || universe.is_oracle_label(&fact.target_pattern)
    {
        return Err(RejectionReason::OracleShapedTarget);
    }
    if !universe.contains_source(&fact.source_pattern) {
        return Err(RejectionReason::NonResolvingSource);
    }
    if !universe.contains_target(&fact.target_pattern) {
        return Err(RejectionReason::NonResolvingTarget);
    }
    Ok(())
}

fn is_broad_pattern(pattern: &str) -> bool {
    let trimmed = pattern.trim();
    trimmed == "*"
        || trimmed == "**"
        || trimmed.ends_with("::*")
        || trimmed.contains("<any>")
        || trimmed.contains("**/")
}

fn is_oracle_shaped_target(pattern: &str) -> bool {
    let normalized = pattern.to_ascii_lowercase();
    normalized.starts_with("expected:")
        || normalized.starts_with("oracle:")
        || normalized.starts_with("answer_key:")
        || normalized.contains("/expected")
        || normalized.contains("want:")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::adaptation::facts::{LoadedModelFact, ModelConfidence, ModelLanguage};

    #[test]
    fn accepts_source_evident_resolving_fact() {
        let fact = fact(
            "call:src/app.ts:10:register",
            "function:src/app.ts:1:onRegister",
        );
        let universe = ValidationUniverse::new(
            ["call:src/app.ts:10:register"],
            ["function:src/app.ts:1:onRegister"],
        );
        assert_eq!(validate_model_fact(&fact, &universe), Ok(()));
    }

    #[test]
    fn rejects_non_resolving_target() {
        let fact = fact("call:src/app.ts:10:register", "function:missing");
        let universe = ValidationUniverse::new(["call:src/app.ts:10:register"], ["function:real"]);
        assert_eq!(
            validate_model_fact(&fact, &universe),
            Err(RejectionReason::NonResolvingTarget)
        );
    }

    #[test]
    fn rejects_broad_and_oracle_shaped_patterns() {
        let universe = ValidationUniverse::new(["call:a"], ["function:b"])
            .with_oracle_labels(["function:oracle"]);
        assert_eq!(
            validate_model_fact(&fact("call:a", "function::*"), &universe),
            Err(RejectionReason::BroadTargetPattern)
        );
        assert_eq!(
            validate_model_fact(&fact("call:a", "function:oracle"), &universe),
            Err(RejectionReason::OracleShapedTarget)
        );
        assert_eq!(
            validate_model_fact(&fact("call:a", "expected:case-label"), &universe),
            Err(RejectionReason::OracleShapedTarget)
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
            stable_key: crate::core::stable_key_for_test(&format!("{source}->{target}")),
        }
    }
}
