use std::collections::BTreeMap;

use super::facts::{
    PointsToBudgetStatus, PointsToConstraintFact, PointsToPrecision, PointsToSetFact,
    PointsToStatus,
};
use crate::analysis::ids::{ObjectTokenId, PointsToConstraintId, PointsToSetId, PtVarId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PointsToOutput {
    pub(crate) constraints: Vec<PointsToConstraintFact>,
    pub(crate) sets: Vec<PointsToSetFact>,
}

impl PointsToOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.constraints.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        self.sets.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        for (index, row) in self.constraints.iter_mut().enumerate() {
            row.id = PointsToConstraintId(index as u64);
        }
        for (index, row) in self.sets.iter_mut().enumerate() {
            row.id = PointsToSetId(index as u64);
            row.objects.sort();
            row.objects.dedup();
        }
        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PointsToStore {
    output: PointsToOutput,
    constraints_by_status: BTreeMap<PointsToStatus, Vec<usize>>,
    constraints_by_precision: BTreeMap<PointsToPrecision, Vec<usize>>,
    sets_by_variable: BTreeMap<PtVarId, Vec<usize>>,
    sets_by_object: BTreeMap<ObjectTokenId, Vec<usize>>,
    sets_by_budget: BTreeMap<PointsToBudgetStatus, Vec<usize>>,
}

impl PointsToStore {
    #[allow(
        dead_code,
        reason = "Compatibility callers can still pass unnormalized output; providers use from_normalized_output."
    )]
    pub(crate) fn from_output(output: PointsToOutput) -> Self {
        Self::from_normalized_output(output.normalized())
    }

    pub(crate) fn from_normalized_output(output: PointsToOutput) -> Self {
        let mut store = Self {
            output,
            ..Self::default()
        };
        for (index, row) in store.output.constraints.iter().enumerate() {
            store
                .constraints_by_status
                .entry(row.status)
                .or_default()
                .push(index);
            store
                .constraints_by_precision
                .entry(row.precision)
                .or_default()
                .push(index);
        }
        for (index, row) in store.output.sets.iter().enumerate() {
            store
                .sets_by_variable
                .entry(row.variable)
                .or_default()
                .push(index);
            for object in &row.objects {
                store.sets_by_object.entry(*object).or_default().push(index);
            }
            store
                .sets_by_budget
                .entry(row.budget)
                .or_default()
                .push(index);
        }
        store
    }

    pub(crate) fn constraints(&self) -> &[PointsToConstraintFact] {
        &self.output.constraints
    }

    pub(crate) fn sets(&self) -> &[PointsToSetFact] {
        &self.output.sets
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::points_to::facts::{
        PointsToConstraintKind, PointsToPrecision, PointsToStatus,
    };

    fn constraint(id: u64, stable_key: &str) -> PointsToConstraintFact {
        PointsToConstraintFact {
            id: PointsToConstraintId(id),
            kind: PointsToConstraintKind::Copy {
                dst: PtVarId(1),
                src: PtVarId(2),
            },
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn points_to_output_sorts_by_stable_key_and_reassigns_ids() {
        let output = PointsToOutput {
            constraints: vec![constraint(9, "pt:z"), constraint(2, "pt:a")],
            sets: Vec::new(),
        }
        .normalized();

        assert_eq!(output.constraints[0].stable_key, "pt:a");
        assert_eq!(output.constraints[0].id, PointsToConstraintId(0));
        assert_eq!(output.constraints[1].id, PointsToConstraintId(1));
    }
}
