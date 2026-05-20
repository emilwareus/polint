use std::collections::BTreeMap;

use crate::analysis::error::AnalysisError;
use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId, UnsupportedId};
use crate::analysis::mir::body::{MirBody, MirOutput};
use crate::analysis::mir::op::{MirOperation, MirOperationKind, MirValue, UnsupportedSemanticFact};
use crate::analysis::places::{PlaceFact, PlaceRoot};
use crate::core::SEMANTIC_MIR_PROVIDER_ID;

#[derive(Debug, Default, Clone)]
pub(crate) struct SemanticStore {
    mir_bodies: Vec<MirBody>,
    mir_operations: Vec<MirOperation>,
    places: Vec<PlaceFact>,
    unsupported_semantics: Vec<UnsupportedSemanticFact>,
    mir_bodies_by_id: BTreeMap<MirBodyId, usize>,
    mir_operations_by_id: BTreeMap<MirOpId, usize>,
    places_by_id: BTreeMap<PlaceId, usize>,
    unsupported_semantics_by_id: BTreeMap<UnsupportedId, usize>,
}

impl SemanticStore {
    pub(crate) fn from_output(output: MirOutput) -> Result<Self, AnalysisError> {
        let output = output.normalized();
        let (mir_bodies, body_ids) = normalize_bodies(output.bodies);
        let (places, place_ids) = normalize_places(output.places, &body_ids)?;
        let unsupported_ids = unsupported_id_map(&output.unsupported);
        let (mir_operations, operation_ids) =
            normalize_operations(output.operations, &body_ids, &place_ids, &unsupported_ids)?;
        let unsupported_semantics = normalize_unsupported_with_ids(
            output.unsupported,
            &body_ids,
            &operation_ids,
            &place_ids,
            &unsupported_ids,
        )?;

        Ok(Self {
            mir_bodies_by_id: index_by_id(&mir_bodies, |body| body.id),
            mir_operations_by_id: index_by_id(&mir_operations, |operation| operation.id),
            places_by_id: index_by_id(&places, |place| place.id),
            unsupported_semantics_by_id: index_by_id(&unsupported_semantics, |row| row.id),
            mir_bodies,
            mir_operations,
            places,
            unsupported_semantics,
        })
    }

    pub(crate) fn mir_bodies(&self) -> &[MirBody] {
        &self.mir_bodies
    }

    pub(crate) fn mir_operations(&self) -> &[MirOperation] {
        &self.mir_operations
    }

    pub(crate) fn places(&self) -> &[PlaceFact] {
        &self.places
    }

    pub(crate) fn unsupported_semantics(&self) -> &[UnsupportedSemanticFact] {
        &self.unsupported_semantics
    }

    pub(crate) fn mir_body(&self, id: MirBodyId) -> Option<&MirBody> {
        self.mir_bodies_by_id
            .get(&id)
            .and_then(|index| self.mir_bodies.get(*index))
    }

    pub(crate) fn mir_operation(&self, id: MirOpId) -> Option<&MirOperation> {
        self.mir_operations_by_id
            .get(&id)
            .and_then(|index| self.mir_operations.get(*index))
    }

    pub(crate) fn place(&self, id: PlaceId) -> Option<&PlaceFact> {
        self.places_by_id
            .get(&id)
            .and_then(|index| self.places.get(*index))
    }

    pub(crate) fn unsupported_semantic(
        &self,
        id: UnsupportedId,
    ) -> Option<&UnsupportedSemanticFact> {
        self.unsupported_semantics_by_id
            .get(&id)
            .and_then(|index| self.unsupported_semantics.get(*index))
    }
}

fn normalize_bodies(mut bodies: Vec<MirBody>) -> (Vec<MirBody>, BTreeMap<MirBodyId, MirBodyId>) {
    bodies.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    let mut body_ids = BTreeMap::new();
    for (index, body) in bodies.iter_mut().enumerate() {
        let new_id = MirBodyId(index as u64);
        body_ids.insert(body.id, new_id);
        body.id = new_id;
    }
    (bodies, body_ids)
}

fn normalize_places(
    mut places: Vec<PlaceFact>,
    body_ids: &BTreeMap<MirBodyId, MirBodyId>,
) -> Result<(Vec<PlaceFact>, BTreeMap<PlaceId, PlaceId>), AnalysisError> {
    places.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    let mut place_ids = BTreeMap::new();
    for (index, place) in places.iter_mut().enumerate() {
        let new_id = PlaceId(index as u64);
        place_ids.insert(place.id, new_id);
        place.id = new_id;
        remap_place_root(&mut place.root, body_ids)?;
    }
    Ok((places, place_ids))
}

fn normalize_operations(
    mut operations: Vec<MirOperation>,
    body_ids: &BTreeMap<MirBodyId, MirBodyId>,
    place_ids: &BTreeMap<PlaceId, PlaceId>,
    unsupported_ids: &BTreeMap<UnsupportedId, UnsupportedId>,
) -> Result<(Vec<MirOperation>, BTreeMap<MirOpId, MirOpId>), AnalysisError> {
    operations.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    let mut operation_ids = BTreeMap::new();
    for (index, operation) in operations.iter_mut().enumerate() {
        let new_id = MirOpId(index as u64);
        operation_ids.insert(operation.id, new_id);
        operation.id = new_id;
        operation.body = remap_body_id(operation.body, body_ids, "dangling MIR operation body")?;
        remap_operation_kind(&mut operation.kind, place_ids, unsupported_ids)?;
    }
    Ok((operations, operation_ids))
}

fn normalize_unsupported_with_ids(
    mut rows: Vec<UnsupportedSemanticFact>,
    body_ids: &BTreeMap<MirBodyId, MirBodyId>,
    operation_ids: &BTreeMap<MirOpId, MirOpId>,
    place_ids: &BTreeMap<PlaceId, PlaceId>,
    unsupported_ids: &BTreeMap<UnsupportedId, UnsupportedId>,
) -> Result<Vec<UnsupportedSemanticFact>, AnalysisError> {
    rows.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    for (index, row) in rows.iter_mut().enumerate() {
        let new_id = UnsupportedId(index as u64);
        let expected_id = unsupported_ids
            .get(&row.id)
            .copied()
            .ok_or_else(|| invalid_fact("dangling unsupported semantic id"))?;
        if expected_id != new_id {
            return Err(invalid_fact("inconsistent unsupported semantic id remap"));
        }
        row.id = new_id;
        row.body = row
            .body
            .map(|body| remap_body_id(body, body_ids, "dangling unsupported semantic body"))
            .transpose()?;
        row.operation = row
            .operation
            .map(|operation| {
                operation_ids
                    .get(&operation)
                    .copied()
                    .ok_or_else(|| invalid_fact("dangling unsupported semantic operation"))
            })
            .transpose()?;
        for place in &mut row.affected_places {
            *place = remap_place_id(*place, place_ids, "dangling unsupported semantic place")?;
        }
    }
    Ok(rows)
}

fn unsupported_id_map(rows: &[UnsupportedSemanticFact]) -> BTreeMap<UnsupportedId, UnsupportedId> {
    let mut sorted = rows
        .iter()
        .map(|row| (row.stable_key.as_str(), row.id))
        .collect::<Vec<_>>();
    sorted.sort_by(|left, right| left.0.cmp(right.0));
    sorted
        .into_iter()
        .enumerate()
        .map(|(index, (_, old_id))| (old_id, UnsupportedId(index as u64)))
        .collect()
}

fn remap_place_root(
    root: &mut PlaceRoot,
    body_ids: &BTreeMap<MirBodyId, MirBodyId>,
) -> Result<(), AnalysisError> {
    if let PlaceRoot::Temporary { body, .. } = root {
        *body = remap_body_id(*body, body_ids, "dangling temporary place body")?;
    }
    Ok(())
}

fn remap_operation_kind(
    kind: &mut MirOperationKind,
    place_ids: &BTreeMap<PlaceId, PlaceId>,
    unsupported_ids: &BTreeMap<UnsupportedId, UnsupportedId>,
) -> Result<(), AnalysisError> {
    match kind {
        MirOperationKind::StorageLive { place } | MirOperationKind::Read { place } => {
            *place = remap_place_id(*place, place_ids, "dangling MIR operation place")?;
        }
        MirOperationKind::Bind { place, value }
        | MirOperationKind::Assign { place, value, .. }
        | MirOperationKind::Write { place, value } => {
            *place = remap_place_id(*place, place_ids, "dangling MIR operation place")?;
            remap_value(value, place_ids)?;
        }
        MirOperationKind::Branch { .. } => {}
        MirOperationKind::Call {
            callee,
            arguments,
            return_place,
            ..
        } => {
            remap_value(callee, place_ids)?;
            for argument in arguments {
                *argument =
                    remap_place_id(*argument, place_ids, "dangling MIR call argument place")?;
            }
            *return_place =
                remap_place_id(*return_place, place_ids, "dangling MIR call return place")?;
        }
        MirOperationKind::Return { value } => {
            if let Some(value) = value {
                remap_value(value, place_ids)?;
            }
        }
        MirOperationKind::Unsupported { unsupported } => {
            *unsupported = remap_unsupported_id(
                *unsupported,
                unsupported_ids,
                "dangling MIR unsupported operation",
            )?;
        }
    }
    Ok(())
}

fn remap_value(
    value: &mut MirValue,
    place_ids: &BTreeMap<PlaceId, PlaceId>,
) -> Result<(), AnalysisError> {
    if let MirValue::Place(place) = value {
        *place = remap_place_id(*place, place_ids, "dangling MIR value place")?;
    }
    Ok(())
}

fn remap_body_id(
    id: MirBodyId,
    body_ids: &BTreeMap<MirBodyId, MirBodyId>,
    reason: &'static str,
) -> Result<MirBodyId, AnalysisError> {
    body_ids
        .get(&id)
        .copied()
        .ok_or_else(|| invalid_fact(reason))
}

fn remap_place_id(
    id: PlaceId,
    place_ids: &BTreeMap<PlaceId, PlaceId>,
    reason: &'static str,
) -> Result<PlaceId, AnalysisError> {
    place_ids
        .get(&id)
        .copied()
        .ok_or_else(|| invalid_fact(reason))
}

fn remap_unsupported_id(
    id: UnsupportedId,
    unsupported_ids: &BTreeMap<UnsupportedId, UnsupportedId>,
    reason: &'static str,
) -> Result<UnsupportedId, AnalysisError> {
    unsupported_ids
        .get(&id)
        .copied()
        .ok_or_else(|| invalid_fact(reason))
}

fn index_by_id<T, Id>(rows: &[T], id: impl Fn(&T) -> Id) -> BTreeMap<Id, usize>
where
    Id: Ord,
{
    rows.iter()
        .enumerate()
        .map(|(index, row)| (id(row), index))
        .collect()
}

fn invalid_fact(reason: impl Into<String>) -> AnalysisError {
    AnalysisError::InvalidFact {
        provider: SEMANTIC_MIR_PROVIDER_ID,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId, UnsupportedId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{
        ConservativeAction, MirOperation, MirOperationKind, UnsupportedDomain,
        UnsupportedPrecision, UnsupportedSemanticFact,
    };
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::core::{FileId, FunctionId, Language, Span};

    fn span() -> Span {
        Span {
            file: FileId(1),
            start_byte: 1,
            end_byte: 2,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 2,
        }
    }

    fn body() -> MirBody {
        MirBody {
            id: MirBodyId(0),
            language: Language::Go,
            file: FileId(1),
            function: FunctionId(1),
            package: None,
            module: None,
            owner_stable_key: "owner".to_string(),
            span: span(),
            stable_key: "body".to_string(),
            status: MirStatus::Partial,
        }
    }

    fn place() -> PlaceFact {
        PlaceFact {
            id: PlaceId(0),
            language: Language::Go,
            file: Some(FileId(1)),
            function: Some(FunctionId(1)),
            root: PlaceRoot::Local {
                function: FunctionId(1),
                name: "value".to_string(),
            },
            projections: Vec::new(),
            stable_key: "place".to_string(),
            status: PlaceStatus::Resolved,
        }
    }

    fn unsupported(id: u64, stable_key: &str, operation: MirOpId) -> UnsupportedSemanticFact {
        UnsupportedSemanticFact {
            id: UnsupportedId(id),
            body: Some(MirBodyId(0)),
            operation: Some(operation),
            language: Language::Go,
            file: FileId(1),
            span: span(),
            construct: stable_key.to_string(),
            source_evidence: stable_key.to_string(),
            affected_places: vec![PlaceId(0)],
            affected_domains: vec![UnsupportedDomain::Mir],
            conservative_action: ConservativeAction::HavocAffectedPlaces,
            precision: UnsupportedPrecision::Unsupported,
            status: MirStatus::Unsupported,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn semantic_store_remaps_unsupported_operation_refs_after_unsupported_sorting() {
        let output = MirOutput {
            bodies: vec![body()],
            places: vec![place()],
            operations: vec![MirOperation {
                id: MirOpId(0),
                body: MirBodyId(0),
                ordinal: 0,
                span: span(),
                kind: MirOperationKind::Unsupported {
                    unsupported: UnsupportedId(0),
                },
                stable_key: "operation".to_string(),
                status: MirStatus::Unsupported,
            }],
            unsupported: vec![
                unsupported(0, "unsupported:z", MirOpId(0)),
                unsupported(1, "unsupported:a", MirOpId(0)),
            ],
        };

        let store = SemanticStore::from_output(output).expect("store normalizes");
        let operation = store
            .mir_operations()
            .first()
            .expect("unsupported operation exists");
        let MirOperationKind::Unsupported { unsupported } = operation.kind else {
            panic!("expected unsupported MIR operation");
        };

        assert_eq!(unsupported, UnsupportedId(1));
        assert_eq!(
            store
                .unsupported_semantic(unsupported)
                .expect("remapped unsupported row exists")
                .stable_key,
            "unsupported:z"
        );
    }
}
