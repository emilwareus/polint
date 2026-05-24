use std::collections::BTreeMap;

use super::facts::{
    AllocationKind, AllocationTokenFact, ValueFact, ValueKind, ValueStatus, ValueSubject,
};
use crate::analysis::ids::{AbstractValueId, AllocationTokenId, PlaceId, ValueFactId};
use crate::core::{FunctionId, Language};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ValueOutput {
    pub(crate) values: Vec<ValueFact>,
    pub(crate) allocations: Vec<AllocationTokenFact>,
}

impl ValueOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.allocations.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        let allocation_remap = self
            .allocations
            .iter()
            .enumerate()
            .map(|(index, row)| (row.id, AllocationTokenId(index as u64)))
            .collect::<BTreeMap<_, _>>();
        for (index, row) in self.allocations.iter_mut().enumerate() {
            row.id = AllocationTokenId(index as u64);
        }
        self.values.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        for (index, row) in self.values.iter_mut().enumerate() {
            row.id = ValueFactId(index as u64);
            row.value = AbstractValueId(index as u64);
            remap_value_allocation_refs(row, &allocation_remap);
        }
        self
    }
}

fn remap_value_allocation_refs(
    row: &mut ValueFact,
    allocation_remap: &BTreeMap<AllocationTokenId, AllocationTokenId>,
) {
    if let ValueSubject::Allocation(token) = &mut row.subject
        && let Some(remapped) = allocation_remap.get(token)
    {
        *token = *remapped;
    }

    match &mut row.kind {
        ValueKind::Object(token) | ValueKind::Array(token) | ValueKind::CompositeLiteral(token) => {
            if let Some(remapped) = allocation_remap.get(token) {
                *token = *remapped;
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ValueStore {
    output: ValueOutput,
    by_language: BTreeMap<Language, Vec<usize>>,
    by_function: BTreeMap<FunctionId, Vec<usize>>,
    by_status: BTreeMap<ValueStatus, Vec<usize>>,
    allocations_by_kind: BTreeMap<AllocationKind, Vec<usize>>,
    by_place: BTreeMap<PlaceId, Vec<usize>>,
}

impl ValueStore {
    pub(crate) fn from_output(output: ValueOutput) -> Self {
        let output = output.normalized();
        let mut store = Self {
            output,
            ..Self::default()
        };
        for (index, row) in store.output.values.iter().enumerate() {
            store
                .by_language
                .entry(row.language)
                .or_default()
                .push(index);
            if let Some(function) = row.function {
                store.by_function.entry(function).or_default().push(index);
            }
            if let super::facts::ValueSubject::Place(place) = row.subject {
                store.by_place.entry(place).or_default().push(index);
            }
            store.by_status.entry(row.status).or_default().push(index);
        }
        for (index, row) in store.output.allocations.iter().enumerate() {
            store
                .allocations_by_kind
                .entry(row.kind)
                .or_default()
                .push(index);
        }
        store
    }

    pub(crate) fn values(&self) -> &[ValueFact] {
        &self.output.values
    }

    pub(crate) fn allocations(&self) -> &[AllocationTokenFact] {
        &self.output.allocations
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::values::facts::{
        ValueKind, ValuePrecision, ValueProvenance, ValueSubject,
    };

    fn value(id: u64, stable_key: &str) -> ValueFact {
        ValueFact {
            id: ValueFactId(id),
            subject: ValueSubject::Synthetic(stable_key.to_string()),
            value: crate::analysis::ids::AbstractValueId(id),
            kind: ValueKind::Null,
            language: Language::TypeScript,
            file: None,
            function: None,
            body: None,
            precision: ValuePrecision::ExactLocal,
            status: ValueStatus::Present,
            provenance: ValueProvenance::Native,
            stable_key: stable_key.to_string(),
        }
    }

    fn allocation(id: u64, stable_key: &str) -> AllocationTokenFact {
        AllocationTokenFact {
            id: AllocationTokenId(id),
            kind: AllocationKind::CompositeLiteral,
            language: Language::Go,
            file: None,
            function: None,
            body: None,
            source_place: None,
            source_operation: None,
            span: None,
            provenance: ValueProvenance::Native,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn value_output_sorts_by_stable_key_and_reassigns_ids() {
        let output = ValueOutput {
            values: vec![value(9, "value:z"), value(3, "value:a")],
            allocations: Vec::new(),
        }
        .normalized();

        assert_eq!(output.values[0].stable_key, "value:a");
        assert_eq!(output.values[0].id, ValueFactId(0));
        assert_eq!(output.values[0].value, AbstractValueId(0));
        assert_eq!(output.values[1].id, ValueFactId(1));
        assert_eq!(output.values[1].value, AbstractValueId(1));
    }

    #[test]
    fn value_output_remaps_allocation_references_after_sorting_allocations() {
        let output = ValueOutput {
            values: vec![
                ValueFact {
                    kind: ValueKind::CompositeLiteral(AllocationTokenId(0)),
                    ..value(0, "value:z")
                },
                ValueFact {
                    subject: ValueSubject::Allocation(AllocationTokenId(1)),
                    kind: ValueKind::Object(AllocationTokenId(1)),
                    ..value(1, "value:a")
                },
            ],
            allocations: vec![allocation(0, "alloc:z"), allocation(1, "alloc:a")],
        }
        .normalized();

        assert_eq!(output.allocations[0].stable_key, "alloc:a");
        assert_eq!(output.allocations[0].id, AllocationTokenId(0));
        assert_eq!(output.allocations[1].stable_key, "alloc:z");
        assert_eq!(output.allocations[1].id, AllocationTokenId(1));
        assert_eq!(
            output.values[0].subject,
            ValueSubject::Allocation(AllocationTokenId(0))
        );
        assert_eq!(
            output.values[0].kind,
            ValueKind::Object(AllocationTokenId(0))
        );
        assert_eq!(
            output.values[1].kind,
            ValueKind::CompositeLiteral(AllocationTokenId(1))
        );
    }
}
