use super::facts::AliasOperand;
use super::query::AliasQueryIndex;
use super::store::AliasOutput;
use crate::analysis::access_paths::facts::AccessPathFact;
use crate::analysis::points_to::facts::PointsToSetFact;

const MAX_PROVIDER_STACK_PAIRS: usize = 64;

pub(crate) fn derive_alias_answers(
    access_paths: &[AccessPathFact],
    points_to_sets: &[PointsToSetFact],
) -> AliasOutput {
    let index = AliasQueryIndex::new(access_paths, points_to_sets);
    let mut operands = access_paths
        .iter()
        .flat_map(|path| {
            [
                AliasOperand::Place(path.base),
                AliasOperand::AccessPath(path.id),
            ]
        })
        .collect::<Vec<_>>();
    operands.sort();
    operands.dedup();

    let mut answers = Vec::new();
    for (left_index, left) in operands.iter().enumerate() {
        for right in operands.iter().skip(left_index) {
            if answers.len() >= MAX_PROVIDER_STACK_PAIRS {
                break;
            }
            answers.push(index.answer(*left, *right));
        }
    }
    AliasOutput { answers }.normalized()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::access_paths::facts::{AccessPathProjection, AccessPathStatus};
    use crate::analysis::ids::{AccessPathId, ObjectTokenId, PlaceId, PointsToSetId, PtVarId};
    use crate::analysis::points_to::facts::{
        PointsToBudgetStatus, PointsToPrecision, PointsToStatus,
    };
    use crate::core::Language;

    #[test]
    fn provider_stack_derives_evidence_backed_answers() {
        let paths = vec![
            path(AccessPathId(0), PlaceId(1), "a"),
            path(AccessPathId(1), PlaceId(1), "b"),
            path(AccessPathId(2), PlaceId(2), "a"),
        ];
        let sets = vec![
            set(PtVarId(1), &[ObjectTokenId(1)]),
            set(PtVarId(2), &[ObjectTokenId(2)]),
            set(PtVarId(200_000), &[ObjectTokenId(1), ObjectTokenId(2)]),
            set(PtVarId(200_001), &[ObjectTokenId(1), ObjectTokenId(3)]),
        ];
        let output = derive_alias_answers(&paths, &sets);

        assert!(!output.answers.is_empty());
        assert!(
            output
                .answers
                .iter()
                .all(|answer| !answer.evidence.is_empty())
        );
        assert!(output.answers.iter().any(|answer| {
            answer.status == crate::analysis::aliases::facts::AliasStatus::PartialAlias
        }));
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
