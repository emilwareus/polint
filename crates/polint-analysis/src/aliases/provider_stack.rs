use super::facts::{AliasAnswerFact, AliasOperand, AliasPrecision, AliasReason, AliasStatus};
use super::query::AliasQueryIndex;
use super::store::AliasOutput;
use crate::access_paths::facts::AccessPathFact;
use crate::ids::AliasAnswerId;
use crate::points_to::facts::PointsToSetFact;
use polint_analysis_api::{FactFamily, stable_key_from_parts};

pub const MAX_PROVIDER_STACK_PAIRS: usize = 64;

pub fn derive_alias_answers(
    interner: &polint_core::StableKeyInterner,
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
    let mut budget_reported = false;
    for (left_index, left) in operands.iter().enumerate() {
        for right in operands.iter().skip(left_index) {
            if answers.len() >= MAX_PROVIDER_STACK_PAIRS {
                if !budget_reported {
                    answers.push(budget_exceeded_answer(interner, *left, *right));
                    budget_reported = true;
                }
                break;
            }
            answers.push(index.answer(interner, *left, *right));
        }
    }
    AliasOutput { answers }.normalized(interner)
}

fn budget_exceeded_answer(
    interner: &polint_core::StableKeyInterner,
    left: AliasOperand,
    right: AliasOperand,
) -> AliasAnswerFact {
    AliasAnswerFact {
        id: AliasAnswerId(0),
        left,
        right,
        status: AliasStatus::Unknown,
        reason: AliasReason::BudgetExceeded,
        evidence: vec!["provider-stack alias pair budget exceeded".to_string()],
        precision: AliasPrecision::Unknown,
        stable_key: stable_key_from_parts(
            interner,
            FactFamily::AliasAnswer,
            &[
                ("left", format!("{left:?}")),
                ("right", format!("{right:?}")),
                ("status", format!("{:?}", AliasStatus::Unknown)),
                ("reason", format!("{:?}", AliasReason::BudgetExceeded)),
            ],
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::access_paths::facts::{AccessPathProjection, AccessPathStatus};
    use crate::ids::{AccessPathId, ObjectTokenId, PlaceId, PointsToSetId, PtVarId};
    use crate::points_to::facts::{PointsToBudgetStatus, PointsToPrecision, PointsToStatus};
    use crate::points_to::vars;
    use polint_core::Language;

    #[test]
    fn provider_stack_derives_evidence_backed_answers() {
        let paths = vec![
            path(AccessPathId(0), PlaceId(1), "a"),
            path(AccessPathId(1), PlaceId(1), "b"),
            path(AccessPathId(2), PlaceId(2), "a"),
        ];
        let sets = vec![
            set(vars::place_var(PlaceId(1)), &[ObjectTokenId(1)]),
            set(vars::place_var(PlaceId(2)), &[ObjectTokenId(2)]),
            set(
                vars::access_path_var(AccessPathId(0)),
                &[ObjectTokenId(1), ObjectTokenId(2)],
            ),
            set(
                vars::access_path_var(AccessPathId(1)),
                &[ObjectTokenId(1), ObjectTokenId(3)],
            ),
        ];
        let output = derive_alias_answers(&polint_core::test_stable_key_interner(), &paths, &sets);

        assert!(!output.answers.is_empty());
        assert!(
            output
                .answers
                .iter()
                .all(|answer| !answer.evidence.is_empty())
        );
        assert!(
            output.answers.iter().any(|answer| {
                answer.status == crate::aliases::facts::AliasStatus::PartialAlias
            })
        );
    }

    #[test]
    fn provider_stack_reports_budget_exhaustion_instead_of_silent_truncation() {
        let paths = (0..20)
            .map(|id| path(AccessPathId(id), PlaceId(id + 1), "field"))
            .collect::<Vec<_>>();

        let output = derive_alias_answers(&polint_core::test_stable_key_interner(), &paths, &[]);

        assert!(output.answers.iter().any(|answer| {
            answer.status == AliasStatus::Unknown && answer.reason == AliasReason::BudgetExceeded
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
            stable_key: polint_core::stable_key_for_test(&format!("path:{}", id.0)),
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
            stable_key: polint_core::stable_key_for_test(&format!("set:{}", variable.0)),
        }
    }
}
