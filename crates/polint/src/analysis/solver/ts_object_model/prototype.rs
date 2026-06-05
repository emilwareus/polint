//! Bounded prototype/class lookup for the JS/TS object-model solver.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::solver::budget::{BudgetReason, SolverBudget};

use super::fixpoint::{TsObjectPropertyBucketKey, TsObjectPropertyBucketState, TsObjectValueToken};
use super::inputs::TsObjectPrototypeLink;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PrototypeLookupResult {
    pub(crate) tokens: BTreeMap<TsObjectValueToken, BTreeSet<String>>,
    pub(crate) budget_reasons: BTreeSet<String>,
}

pub(crate) fn lookup_property_with_prototypes(
    buckets: &BTreeMap<TsObjectPropertyBucketKey, TsObjectPropertyBucketState>,
    links: &[TsObjectPrototypeLink],
    object: SemanticNodeId,
    field: &str,
    budget: &SolverBudget,
) -> PrototypeLookupResult {
    let prototype_by_object = prototype_by_object(links);
    let link_key_by_pair = link_key_by_pair(links);
    let mut result = PrototypeLookupResult::default();
    let mut visited = BTreeSet::new();
    let mut current = object;
    let mut path_evidence = BTreeSet::new();

    for depth in 0..=budget.object.max_prototype_depth {
        if !visited.insert(current) {
            return result;
        }

        let key = TsObjectPropertyBucketKey {
            object: current,
            field: field.to_string(),
        };
        if let Some(bucket) = buckets.get(&key) {
            for (token, evidence) in &bucket.tokens {
                let mut combined = evidence.clone();
                combined.extend(path_evidence.iter().cloned());
                result
                    .tokens
                    .entry(token.clone())
                    .or_default()
                    .extend(combined);
            }
            return result;
        }

        let Some(&prototype) = prototype_by_object.get(&current) else {
            return result;
        };
        if depth == budget.object.max_prototype_depth {
            result
                .budget_reasons
                .insert(BudgetReason::ObjectMaxPrototypeDepth.as_str().to_string());
            return result;
        }
        if let Some(link_key) = link_key_by_pair.get(&(current, prototype)) {
            path_evidence.insert(link_key.clone());
        }
        current = prototype;
    }

    result
}

fn prototype_by_object(
    links: &[TsObjectPrototypeLink],
) -> BTreeMap<SemanticNodeId, SemanticNodeId> {
    links
        .iter()
        .map(|link| (link.object, link.prototype))
        .collect()
}

fn link_key_by_pair(
    links: &[TsObjectPrototypeLink],
) -> BTreeMap<(SemanticNodeId, SemanticNodeId), String> {
    links
        .iter()
        .map(|link| ((link.object, link.prototype), link.stable_key.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::solver::budget::BudgetReason;

    #[test]
    fn instance_lookup_reaches_prototype_bucket() {
        let buckets = buckets_with_prototype_method();
        let links = vec![link(1, 2, "prototype:C")];

        let result = lookup_property_with_prototypes(
            &buckets,
            &links,
            SemanticNodeId(1),
            "static:m",
            &SolverBudget::default(),
        );

        assert_eq!(result.tokens.len(), 1);
        let evidence = result
            .tokens
            .get(&TsObjectValueToken::Function(SemanticNodeId(10)))
            .expect("method token");
        assert!(evidence.contains("prototype:C"));
        assert!(evidence.contains("write:m"));
    }

    #[test]
    fn extends_lookup_reaches_superclass_method_under_depth_cap() {
        let buckets = buckets_with_prototype_method();
        let links = vec![link(1, 2, "prototype:D"), link(2, 3, "extends:C")];
        let mut buckets = buckets;
        buckets.insert(
            TsObjectPropertyBucketKey {
                object: SemanticNodeId(3),
                field: "static:base".to_string(),
            },
            bucket(11, "write:base"),
        );

        let result = lookup_property_with_prototypes(
            &buckets,
            &links,
            SemanticNodeId(1),
            "static:base",
            &SolverBudget::default(),
        );

        assert_eq!(result.tokens.len(), 1);
        let evidence = result
            .tokens
            .get(&TsObjectValueToken::Function(SemanticNodeId(11)))
            .expect("super method token");
        assert!(evidence.contains("prototype:D"));
        assert!(evidence.contains("extends:C"));
    }

    #[test]
    fn prototype_cycle_terminates_without_budget_evidence() {
        let links = vec![link(1, 2, "p:1-2"), link(2, 1, "p:2-1")];

        let result = lookup_property_with_prototypes(
            &BTreeMap::new(),
            &links,
            SemanticNodeId(1),
            "static:missing",
            &SolverBudget::default(),
        );

        assert!(result.budget_reasons.is_empty());
        assert!(result.tokens.is_empty());
    }

    #[test]
    fn max_prototype_depth_terminates_without_fake_tokens() {
        let links = vec![link(1, 2, "p:1-2"), link(2, 3, "p:2-3")];
        let mut budget = SolverBudget::default();
        budget.object.max_prototype_depth = 1;

        let result = lookup_property_with_prototypes(
            &BTreeMap::new(),
            &links,
            SemanticNodeId(1),
            "static:missing",
            &budget,
        );

        assert!(
            result
                .budget_reasons
                .contains(BudgetReason::ObjectMaxPrototypeDepth.as_str())
        );
        assert!(result.tokens.is_empty());
    }

    fn buckets_with_prototype_method()
    -> BTreeMap<TsObjectPropertyBucketKey, TsObjectPropertyBucketState> {
        BTreeMap::from([(
            TsObjectPropertyBucketKey {
                object: SemanticNodeId(2),
                field: "static:m".to_string(),
            },
            bucket(10, "write:m"),
        )])
    }

    fn bucket(function: u64, evidence: &str) -> TsObjectPropertyBucketState {
        TsObjectPropertyBucketState {
            tokens: BTreeMap::from([(
                TsObjectValueToken::Function(SemanticNodeId(function)),
                BTreeSet::from([evidence.to_string()]),
            )]),
        }
    }

    fn link(object: u64, prototype: u64, stable_key: &str) -> TsObjectPrototypeLink {
        TsObjectPrototypeLink {
            object: SemanticNodeId(object),
            prototype: SemanticNodeId(prototype),
            stable_key: stable_key.to_string(),
        }
    }
}
