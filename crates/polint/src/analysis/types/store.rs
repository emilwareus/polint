use std::collections::BTreeMap;

use super::facts::{NarrowedTypeFact, TypeFact, TypePrecision, TypeShape, TypeStatus};
use crate::analysis::access_paths::store::AccessPathOutput;
use crate::analysis::aliases::store::AliasOutput;
use crate::analysis::ids::{NarrowedTypeId, PlaceId, TypeFactId, TypeSetId};
use crate::analysis::points_to::store::PointsToOutput;
use crate::analysis::values::store::ValueOutput;
use crate::core::{FunctionId, Language};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TypeValueAliasOutput {
    pub(crate) types: TypeOutput,
    pub(crate) values: ValueOutput,
    pub(crate) access_paths: AccessPathOutput,
    pub(crate) points_to: PointsToOutput,
    pub(crate) aliases: AliasOutput,
}

impl TypeValueAliasOutput {
    pub(crate) fn normalized(self) -> Self {
        Self {
            types: self.types.normalized(),
            values: self.values.normalized(),
            access_paths: self.access_paths.normalized(),
            points_to: self.points_to.normalized(),
            aliases: self.aliases.normalized(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TypeOutput {
    pub(crate) types: Vec<TypeFact>,
    pub(crate) narrowed: Vec<NarrowedTypeFact>,
}

impl TypeOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.types.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        self.narrowed.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        let mut type_set_remap = BTreeMap::new();
        for (index, row) in self.types.iter().enumerate() {
            type_set_remap
                .entry(row.type_set)
                .or_insert(TypeSetId(index as u64));
        }
        for (index, row) in self.types.iter_mut().enumerate() {
            row.id = TypeFactId(index as u64);
            if let Some(remapped_set) = type_set_remap.get(&row.type_set) {
                row.type_set = *remapped_set;
            }
            remap_type_shape_sets(&mut row.shape, &type_set_remap);
        }
        for (index, row) in self.narrowed.iter_mut().enumerate() {
            row.id = NarrowedTypeId(index as u64);
            if let Some(remapped_set) = type_set_remap.get(&row.type_set) {
                row.type_set = *remapped_set;
            }
        }
        self
    }
}

fn remap_type_shape_sets(shape: &mut TypeShape, type_set_remap: &BTreeMap<TypeSetId, TypeSetId>) {
    match shape {
        TypeShape::Union(sets) | TypeShape::Intersection(sets) => {
            for set in sets {
                if let Some(remapped) = type_set_remap.get(set) {
                    *set = *remapped;
                }
            }
        }
        _ => {}
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TypeStore {
    output: TypeOutput,
    by_language: BTreeMap<Language, Vec<usize>>,
    by_function: BTreeMap<FunctionId, Vec<usize>>,
    by_place: BTreeMap<PlaceId, Vec<usize>>,
    by_status: BTreeMap<TypeStatus, Vec<usize>>,
    by_precision: BTreeMap<TypePrecision, Vec<usize>>,
}

impl TypeStore {
    pub(crate) fn from_output(output: TypeOutput) -> Self {
        let output = output.normalized();
        let mut store = Self {
            output,
            ..Self::default()
        };
        for (index, row) in store.output.types.iter().enumerate() {
            store
                .by_language
                .entry(row.language)
                .or_default()
                .push(index);
            if let Some(function) = row.function {
                store.by_function.entry(function).or_default().push(index);
            }
            if let Some(place) = row.place {
                store.by_place.entry(place).or_default().push(index);
            }
            store.by_status.entry(row.status).or_default().push(index);
            store
                .by_precision
                .entry(row.precision)
                .or_default()
                .push(index);
        }
        store
    }

    pub(crate) fn types(&self) -> &[TypeFact] {
        &self.output.types
    }

    pub(crate) fn narrowed(&self) -> &[NarrowedTypeFact] {
        &self.output.narrowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::types::facts::{
        TypeConfidence, TypePhase, TypeProvenance, TypeShape, TypeSubject,
    };
    use crate::core::Language;

    fn type_fact(id: u64, stable_key: &str) -> TypeFact {
        TypeFact {
            id: TypeFactId(id),
            subject: TypeSubject::Synthetic(stable_key.to_string()),
            type_set: TypeSetId(id),
            shape: TypeShape::Any,
            phase: TypePhase::Declared,
            language: Language::Go,
            file: None,
            function: None,
            body: None,
            place: None,
            cfg_block: None,
            operation: None,
            precision: TypePrecision::Conservative,
            confidence: TypeConfidence::Medium,
            status: TypeStatus::Present,
            provenance: TypeProvenance::Native,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn type_output_sorts_by_stable_key_and_reassigns_ids() {
        let output = TypeOutput {
            types: vec![type_fact(7, "type:z"), type_fact(3, "type:a")],
            narrowed: Vec::new(),
        }
        .normalized();

        assert_eq!(output.types[0].stable_key, "type:a");
        assert_eq!(output.types[0].id, TypeFactId(0));
        assert_eq!(output.types[0].type_set, TypeSetId(0));
        assert_eq!(output.types[1].id, TypeFactId(1));
        assert_eq!(output.types[1].type_set, TypeSetId(1));
    }

    #[test]
    fn type_output_remaps_type_set_references_after_sorting() {
        let output = TypeOutput {
            types: vec![
                TypeFact {
                    shape: TypeShape::Union(vec![TypeSetId(7), TypeSetId(3)]),
                    ..type_fact(7, "type:z")
                },
                type_fact(3, "type:a"),
            ],
            narrowed: vec![NarrowedTypeFact {
                id: NarrowedTypeId(9),
                place: PlaceId(1),
                type_set: TypeSetId(7),
                cfg_block: None,
                operation: None,
                predicate: None,
                evidence: "narrowed".to_string(),
                language: Language::Go,
                file: None,
                function: None,
                body: None,
                precision: TypePrecision::Conservative,
                status: TypeStatus::Present,
                stable_key: "narrowed:z".to_string(),
            }],
        }
        .normalized();

        assert_eq!(output.narrowed[0].type_set, TypeSetId(1));
        assert_eq!(
            output.types[1].shape,
            TypeShape::Union(vec![TypeSetId(1), TypeSetId(0)])
        );
    }
}
