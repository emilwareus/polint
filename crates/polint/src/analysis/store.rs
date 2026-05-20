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
        let (mir_operations, operation_ids) =
            normalize_operations(output.operations, &body_ids, &place_ids)?;
        let (unsupported_semantics, _unsupported_ids) =
            normalize_unsupported(output.unsupported, &body_ids, &operation_ids, &place_ids)?;

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
) -> Result<(Vec<MirOperation>, BTreeMap<MirOpId, MirOpId>), AnalysisError> {
    operations.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    let mut operation_ids = BTreeMap::new();
    for (index, operation) in operations.iter_mut().enumerate() {
        let new_id = MirOpId(index as u64);
        operation_ids.insert(operation.id, new_id);
        operation.id = new_id;
        operation.body = remap_body_id(operation.body, body_ids, "dangling MIR operation body")?;
        remap_operation_kind(&mut operation.kind, place_ids)?;
    }
    Ok((operations, operation_ids))
}

fn normalize_unsupported(
    mut rows: Vec<UnsupportedSemanticFact>,
    body_ids: &BTreeMap<MirBodyId, MirBodyId>,
    operation_ids: &BTreeMap<MirOpId, MirOpId>,
    place_ids: &BTreeMap<PlaceId, PlaceId>,
) -> Result<
    (
        Vec<UnsupportedSemanticFact>,
        BTreeMap<UnsupportedId, UnsupportedId>,
    ),
    AnalysisError,
> {
    rows.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    let mut unsupported_ids = BTreeMap::new();
    for (index, row) in rows.iter_mut().enumerate() {
        let new_id = UnsupportedId(index as u64);
        unsupported_ids.insert(row.id, new_id);
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
    Ok((rows, unsupported_ids))
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
        MirOperationKind::Unsupported { .. } => {}
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
