use std::collections::{BTreeMap, BTreeSet};

use super::facts::{AliasAnswerFact, AliasOperand, AliasPrecision, AliasReason, AliasStatus};
use crate::analysis::access_paths::facts::AccessPathFact;
use crate::analysis::ids::{AliasAnswerId, ObjectTokenId, PtVarId};
use crate::analysis::points_to::facts::{PointsToBudgetStatus, PointsToSetFact, PointsToStatus};
use crate::analysis::points_to::vars;
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};

#[derive(Debug, Default)]
pub(crate) struct AliasQueryIndex<'a> {
    access_paths: BTreeMap<crate::analysis::ids::AccessPathId, &'a AccessPathFact>,
    points_to: BTreeMap<PtVarId, &'a PointsToSetFact>,
    budget_exceeded: bool,
}

impl<'a> AliasQueryIndex<'a> {
    pub(crate) fn new(
        access_paths: &'a [AccessPathFact],
        points_to: &'a [PointsToSetFact],
    ) -> Self {
        Self {
            access_paths: access_paths.iter().map(|path| (path.id, path)).collect(),
            points_to: points_to.iter().map(|set| (set.variable, set)).collect(),
            budget_exceeded: points_to
                .iter()
                .any(|set| set.budget == PointsToBudgetStatus::BudgetExceeded),
        }
    }

    pub(crate) fn answer(&self, left: AliasOperand, right: AliasOperand) -> AliasAnswerFact {
        let (status, reason, precision, evidence) = self.classify(left, right);
        AliasAnswerFact {
            id: AliasAnswerId(0),
            left,
            right,
            status,
            reason,
            evidence,
            precision,
            stable_key: stable_key_from_parts(
                FactFamily::AliasAnswer,
                &[
                    ("left", format!("{left:?}")),
                    ("right", format!("{right:?}")),
                    ("status", format!("{status:?}")),
                ],
            ),
        }
    }

    fn classify(
        &self,
        left: AliasOperand,
        right: AliasOperand,
    ) -> (AliasStatus, AliasReason, AliasPrecision, Vec<String>) {
        if left == right || self.same_access_path(left, right) {
            return (
                AliasStatus::MustAlias,
                AliasReason::SameStablePlace,
                AliasPrecision::ExactLocal,
                vec!["same stable place/access path".to_string()],
            );
        }
        if self.common_base_different_projection(left, right) {
            return (
                AliasStatus::PartialAlias,
                AliasReason::CommonBaseDifferentProjection,
                AliasPrecision::Conservative,
                vec!["common base with different known projections".to_string()],
            );
        }
        if self.budget_exceeded {
            return (
                AliasStatus::Unknown,
                AliasReason::BudgetExceeded,
                AliasPrecision::Unknown,
                vec!["points-to budget exceeded".to_string()],
            );
        }
        let Some(left_objects) = self.objects_for(left) else {
            return (
                AliasStatus::Unknown,
                AliasReason::MissingPointsTo,
                AliasPrecision::Unknown,
                vec!["missing left points-to set".to_string()],
            );
        };
        let Some(right_objects) = self.objects_for(right) else {
            return (
                AliasStatus::Unknown,
                AliasReason::MissingPointsTo,
                AliasPrecision::Unknown,
                vec!["missing right points-to set".to_string()],
            );
        };
        if left_objects.is_empty() || right_objects.is_empty() {
            return (
                AliasStatus::Unknown,
                AliasReason::MissingPointsTo,
                AliasPrecision::Unknown,
                vec!["empty points-to set".to_string()],
            );
        }
        if left_objects.is_disjoint(&right_objects) {
            return (
                AliasStatus::NoAlias,
                AliasReason::DisjointPointsToSets,
                AliasPrecision::FlowInsensitive,
                vec!["disjoint points-to sets".to_string()],
            );
        }
        if left_objects.len() == 1 && left_objects == right_objects {
            return (
                AliasStatus::MustAlias,
                AliasReason::SingletonEqualObject,
                AliasPrecision::FlowInsensitive,
                vec!["singleton-equal object token".to_string()],
            );
        }
        (
            AliasStatus::MayAlias,
            AliasReason::OverlappingPointsToSets,
            AliasPrecision::Conservative,
            vec!["overlapping points-to sets".to_string()],
        )
    }

    fn same_access_path(&self, left: AliasOperand, right: AliasOperand) -> bool {
        match (left, right) {
            (AliasOperand::AccessPath(left), AliasOperand::AccessPath(right)) => self
                .access_paths
                .get(&left)
                .zip(self.access_paths.get(&right))
                .is_some_and(|(left, right)| {
                    left.base == right.base && left.projections == right.projections
                }),
            _ => false,
        }
    }

    fn common_base_different_projection(&self, left: AliasOperand, right: AliasOperand) -> bool {
        match (left, right) {
            (AliasOperand::AccessPath(left), AliasOperand::AccessPath(right)) => self
                .access_paths
                .get(&left)
                .zip(self.access_paths.get(&right))
                .is_some_and(|(left, right)| {
                    left.base == right.base && left.projections != right.projections
                }),
            _ => false,
        }
    }

    fn objects_for(&self, operand: AliasOperand) -> Option<BTreeSet<ObjectTokenId>> {
        let variable = match operand {
            AliasOperand::Place(place) => vars::place_var(place),
            AliasOperand::AccessPath(path) => vars::access_path_var(path),
        };
        let set = self.points_to.get(&variable)?;
        if set.status == PointsToStatus::BudgetExceeded {
            return None;
        }
        Some(set.objects.iter().copied().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::access_paths::facts::{AccessPathProjection, AccessPathStatus};
    use crate::analysis::ids::{AccessPathId, PlaceId, PointsToSetId};
    use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
    use crate::analysis::points_to::vars;
    use crate::core::Language;

    #[test]
    fn alias_query_returns_all_statuses_with_evidence() {
        let paths = vec![
            path(AccessPathId(0), PlaceId(1), "a"),
            path(AccessPathId(1), PlaceId(1), "b"),
        ];
        let sets = vec![
            set(vars::place_var(PlaceId(1)), &[ObjectTokenId(1)]),
            set(vars::place_var(PlaceId(2)), &[ObjectTokenId(2)]),
            set(
                vars::place_var(PlaceId(3)),
                &[ObjectTokenId(1), ObjectTokenId(2)],
            ),
            set(
                vars::place_var(PlaceId(4)),
                &[ObjectTokenId(1), ObjectTokenId(3)],
            ),
        ];
        let index = AliasQueryIndex::new(&paths, &sets);

        let answers = [
            index.answer(
                AliasOperand::Place(PlaceId(1)),
                AliasOperand::Place(PlaceId(1)),
            ),
            index.answer(
                AliasOperand::Place(PlaceId(1)),
                AliasOperand::Place(PlaceId(2)),
            ),
            index.answer(
                AliasOperand::Place(PlaceId(3)),
                AliasOperand::Place(PlaceId(4)),
            ),
            index.answer(
                AliasOperand::AccessPath(AccessPathId(0)),
                AliasOperand::AccessPath(AccessPathId(1)),
            ),
            index.answer(
                AliasOperand::Place(PlaceId(1)),
                AliasOperand::Place(PlaceId(9)),
            ),
        ];
        for status in [
            AliasStatus::MustAlias,
            AliasStatus::NoAlias,
            AliasStatus::MayAlias,
            AliasStatus::PartialAlias,
            AliasStatus::Unknown,
        ] {
            assert!(answers.iter().any(|answer| answer.status == status));
        }
        assert!(answers.iter().all(|answer| !answer.evidence.is_empty()));
    }

    fn path(id: AccessPathId, base: PlaceId, field: &str) -> AccessPathFact {
        AccessPathFact {
            id,
            base,
            projections: vec![AccessPathProjection::Property(field.to_string())],
            depth: 1,
            language: Language::TypeScript,
            file: None,
            function: None,
            body: None,
            status: AccessPathStatus::Partial,
            stable_key: format!("path:{}", id.0),
        }
    }

    fn set(variable: PtVarId, objects: &[ObjectTokenId]) -> PointsToSetFact {
        PointsToSetFact {
            id: PointsToSetId(0),
            variable,
            objects: objects.to_vec(),
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            budget: PointsToBudgetStatus::WithinBudget,
            stable_key: format!("set:{}", variable.0),
        }
    }
}
