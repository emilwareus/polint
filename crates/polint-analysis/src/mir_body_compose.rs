//! Language-neutral MIR output composition.

use crate::ids::{MirBodyId, MirOpId, MirStatementId, MirTerminatorId, PlaceId, UnsupportedId};
use crate::mir_body::{MirBlockId, MirOutput, MirTerminatorKind};
use crate::mir_op::{MirOperationKind, MirValue};
use crate::places::PlaceRoot;
use polint_core::StableKeyInterner;

pub fn merge_language_outputs(
    outputs: impl IntoIterator<Item = MirOutput>,
    interner: &StableKeyInterner,
) -> MirOutput {
    let mut merged = MirOutput {
        bodies: Vec::new(),
        places: Vec::new(),
        operations: Vec::new(),
        unsupported: Vec::new(),
        ..MirOutput::default()
    };
    for output in outputs {
        append_language_output(&mut merged, output);
    }
    merged.normalized(interner)
}

fn append_language_output(merged: &mut MirOutput, mut output: MirOutput) {
    let body_offset = merged.bodies.len() as u64;
    let block_offset = merged.blocks.len() as u64;
    let statement_offset = merged.statements.len() as u64;
    let terminator_offset = merged.terminators.len() as u64;
    let place_offset = merged.places.len() as u64;
    let operation_offset = merged
        .operations
        .iter()
        .map(|operation| operation.id.0)
        .max()
        .map_or(0, |id| id + 1);
    let unsupported_offset = merged.unsupported.len() as u64;

    for body in &mut output.bodies {
        body.id = offset_body_id(body.id, body_offset);
    }
    for block in &mut output.blocks {
        block.id = offset_block_id(block.id, block_offset);
        block.body = offset_body_id(block.body, body_offset);
        block.terminator = offset_terminator_id(block.terminator, terminator_offset);
        for statement in &mut block.statements {
            *statement = offset_statement_id(*statement, statement_offset);
        }
    }
    for statement in &mut output.statements {
        statement.id = offset_statement_id(statement.id, statement_offset);
        statement.body = offset_body_id(statement.body, body_offset);
        statement.operation = offset_operation_id(statement.operation, operation_offset);
    }
    for terminator in &mut output.terminators {
        terminator.id = offset_terminator_id(terminator.id, terminator_offset);
        terminator.body = offset_body_id(terminator.body, body_offset);
        offset_terminator_kind_refs(
            &mut terminator.kind,
            body_offset,
            block_offset,
            place_offset,
            unsupported_offset,
        );
    }
    for place in &mut output.places {
        place.id = offset_place_id(place.id, place_offset);
        if let PlaceRoot::Temporary { body, .. } = &mut place.root {
            *body = offset_body_id(*body, body_offset);
        }
    }
    for place_type in &mut output.place_types {
        place_type.place = offset_place_id(place_type.place, place_offset);
    }
    for operation in &mut output.operations {
        operation.id = offset_operation_id(operation.id, operation_offset);
        operation.body = offset_body_id(operation.body, body_offset);
        offset_operation_kind_refs(
            &mut operation.kind,
            body_offset,
            place_offset,
            unsupported_offset,
        );
    }
    for row in &mut output.unsupported {
        row.id = offset_unsupported_id(row.id, unsupported_offset);
        row.body = row.body.map(|body| offset_body_id(body, body_offset));
        row.operation = row
            .operation
            .map(|operation| offset_operation_id(operation, operation_offset));
        for place in &mut row.affected_places {
            *place = offset_place_id(*place, place_offset);
        }
    }

    merged.bodies.extend(output.bodies);
    merged.blocks.extend(output.blocks);
    merged.statements.extend(output.statements);
    merged.terminators.extend(output.terminators);
    merged.places.extend(output.places);
    merged.place_types.extend(output.place_types);
    merged.operations.extend(output.operations);
    merged.unsupported.extend(output.unsupported);
}

fn offset_terminator_kind_refs(
    kind: &mut MirTerminatorKind,
    body_offset: u64,
    block_offset: u64,
    place_offset: u64,
    unsupported_offset: u64,
) {
    match kind {
        MirTerminatorKind::Goto { target } => {
            *target = offset_block_id(*target, block_offset);
        }
        MirTerminatorKind::Branch {
            predicate_place,
            then_target,
            else_target,
            ..
        } => {
            *then_target = offset_block_id(*then_target, block_offset);
            *else_target = offset_block_id(*else_target, block_offset);
            if let Some(place) = predicate_place {
                *place = offset_place_id(*place, place_offset);
            }
        }
        MirTerminatorKind::Switch {
            discriminant,
            cases,
            otherwise,
        } => {
            offset_value_ref(discriminant, body_offset, place_offset);
            for (value, target) in cases {
                offset_value_ref(value, body_offset, place_offset);
                *target = offset_block_id(*target, block_offset);
            }
            *otherwise = offset_block_id(*otherwise, block_offset);
        }
        MirTerminatorKind::Return { value } => {
            if let Some(value) = value {
                offset_value_ref(value, body_offset, place_offset);
            }
        }
        MirTerminatorKind::Throw { value, unwind } => {
            if let Some(value) = value {
                offset_value_ref(value, body_offset, place_offset);
            }
            *unwind = offset_block_id(*unwind, block_offset);
        }
        MirTerminatorKind::Call {
            callee,
            arguments,
            return_place,
            normal,
            unwind,
            ..
        } => {
            offset_value_ref(callee, body_offset, place_offset);
            for argument in arguments {
                *argument = offset_place_id(*argument, place_offset);
            }
            *return_place = offset_place_id(*return_place, place_offset);
            *normal = offset_block_id(*normal, block_offset);
            if let Some(unwind) = unwind {
                *unwind = offset_block_id(*unwind, block_offset);
            }
        }
        MirTerminatorKind::Suspend { value, resume, .. } => {
            if let Some(value) = value {
                offset_value_ref(value, body_offset, place_offset);
            }
            *resume = offset_block_id(*resume, block_offset);
        }
        MirTerminatorKind::Unreachable => {}
        MirTerminatorKind::Unsupported { unsupported } => {
            *unsupported = offset_unsupported_id(*unsupported, unsupported_offset);
        }
    }
}

fn offset_operation_kind_refs(
    kind: &mut MirOperationKind,
    body_offset: u64,
    place_offset: u64,
    unsupported_offset: u64,
) {
    match kind {
        MirOperationKind::StorageLive { place } | MirOperationKind::Read { place } => {
            *place = offset_place_id(*place, place_offset);
        }
        MirOperationKind::Bind { place, value }
        | MirOperationKind::Assign { place, value, .. }
        | MirOperationKind::Write { place, value } => {
            *place = offset_place_id(*place, place_offset);
            offset_value_ref(value, body_offset, place_offset);
        }
        MirOperationKind::Branch {
            predicate_place, ..
        } => {
            if let Some(place) = predicate_place {
                *place = offset_place_id(*place, place_offset);
            }
        }
        MirOperationKind::Call {
            callee,
            arguments,
            return_place,
            ..
        } => {
            offset_value_ref(callee, body_offset, place_offset);
            for argument in arguments {
                *argument = offset_place_id(*argument, place_offset);
            }
            *return_place = offset_place_id(*return_place, place_offset);
        }
        MirOperationKind::Return { value } => {
            if let Some(value) = value {
                offset_value_ref(value, body_offset, place_offset);
            }
        }
        MirOperationKind::Unsupported { unsupported } => {
            *unsupported = offset_unsupported_id(*unsupported, unsupported_offset);
        }
    }
}

fn offset_value_ref(value: &mut MirValue, body_offset: u64, place_offset: u64) {
    match value {
        MirValue::Place(place) => *place = offset_place_id(*place, place_offset),
        MirValue::BinOp { lhs, rhs, .. } => {
            offset_value_ref(lhs, body_offset, place_offset);
            offset_value_ref(rhs, body_offset, place_offset);
        }
        MirValue::Aggregate { fields, .. } => {
            for field in fields {
                offset_value_ref(&mut field.value, body_offset, place_offset);
            }
        }
        MirValue::Closure { body, captures } => {
            *body = offset_body_id(*body, body_offset);
            for capture in captures {
                *capture = offset_place_id(*capture, place_offset);
            }
        }
        MirValue::Literal { .. }
        | MirValue::Temporary(_)
        | MirValue::CallReturn(_)
        | MirValue::Unknown { .. } => {}
    }
}

fn offset_body_id(id: MirBodyId, offset: u64) -> MirBodyId {
    MirBodyId(id.0 + offset)
}

fn offset_block_id(id: MirBlockId, offset: u64) -> MirBlockId {
    MirBlockId(id.0 + offset)
}

fn offset_statement_id(id: MirStatementId, offset: u64) -> MirStatementId {
    MirStatementId(id.0 + offset)
}

fn offset_terminator_id(id: MirTerminatorId, offset: u64) -> MirTerminatorId {
    MirTerminatorId(id.0 + offset)
}

fn offset_place_id(id: PlaceId, offset: u64) -> PlaceId {
    PlaceId(id.0 + offset)
}

fn offset_operation_id(id: MirOpId, offset: u64) -> MirOpId {
    MirOpId(id.0 + offset)
}

fn offset_unsupported_id(id: UnsupportedId, offset: u64) -> UnsupportedId {
    UnsupportedId(id.0 + offset)
}
