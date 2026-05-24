use super::facts::{
    PointsToConstraintFact, PointsToConstraintKind, PointsToPrecision, PointsToStatus,
};
use crate::analysis::access_paths::facts::{AccessPathFact, AccessPathProjection};
use crate::analysis::ids::{ObjectTokenId, PointsToConstraintId, PtVarId};
use crate::analysis::types::store::TypeValueAliasOutput;
use crate::analysis::values::facts::{AllocationTokenFact, ValueFact, ValueKind, ValueSubject};
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};

pub(crate) fn derive_points_to_constraints(
    output: &TypeValueAliasOutput,
) -> Vec<PointsToConstraintFact> {
    let mut builder = ConstraintBuilder::default();
    builder.collect_values(&output.values.values);
    builder.collect_allocations(&output.values.allocations);
    builder.collect_access_paths(&output.access_paths.access_paths);
    builder.finish()
}

#[derive(Default)]
struct ConstraintBuilder {
    constraints: Vec<PointsToConstraintFact>,
}

impl ConstraintBuilder {
    fn collect_values(&mut self, values: &[ValueFact]) {
        for value in values {
            let Some(dst) = subject_var(&value.subject) else {
                continue;
            };
            match &value.kind {
                ValueKind::Object(object)
                | ValueKind::Array(object)
                | ValueKind::CompositeLiteral(object) => {
                    self.push(PointsToConstraintKind::AddressOf {
                        dst,
                        object: ObjectTokenId(object.0),
                    });
                }
                ValueKind::PlaceRef(src) => {
                    self.push(PointsToConstraintKind::Copy {
                        dst,
                        src: PtVarId(src.0),
                    });
                }
                ValueKind::CallReturn(place) => {
                    self.push(PointsToConstraintKind::CallReturn {
                        dst,
                        value: value.id,
                    });
                    self.push(PointsToConstraintKind::Copy {
                        dst,
                        src: PtVarId(place.0),
                    });
                }
                ValueKind::FunctionObject | ValueKind::ClassObject | ValueKind::ModuleObject => {
                    self.push(PointsToConstraintKind::AddressOf {
                        dst,
                        object: ObjectTokenId(value.value.0),
                    });
                }
                ValueKind::Unknown { .. }
                | ValueKind::Null
                | ValueKind::Undefined
                | ValueKind::Nil
                | ValueKind::Bool(_)
                | ValueKind::Number(_)
                | ValueKind::String(_)
                | ValueKind::Literal(_) => {}
            }
        }
    }

    fn collect_allocations(&mut self, allocations: &[AllocationTokenFact]) {
        for allocation in allocations {
            if let Some(place) = allocation.source_place {
                self.push(PointsToConstraintKind::AddressOf {
                    dst: PtVarId(place.0),
                    object: ObjectTokenId(allocation.id.0),
                });
            }
        }
    }

    fn collect_access_paths(&mut self, paths: &[AccessPathFact]) {
        for path in paths {
            let dst = access_path_var(path);
            let base = PtVarId(path.base.0);
            for projection in &path.projections {
                match projection {
                    AccessPathProjection::Field(field) | AccessPathProjection::Property(field) => {
                        self.push(PointsToConstraintKind::FieldLoad {
                            dst,
                            base,
                            field: field.clone(),
                        });
                        self.push(PointsToConstraintKind::FieldStore {
                            base,
                            field: field.clone(),
                            src: dst,
                        });
                    }
                    AccessPathProjection::IndexKnown(index) => {
                        self.push(PointsToConstraintKind::ElementLoad {
                            dst,
                            base,
                            index: index.clone(),
                        });
                        self.push(PointsToConstraintKind::ElementStore {
                            base,
                            index: index.clone(),
                            src: dst,
                        });
                    }
                    AccessPathProjection::IndexUnknown { evidence } => {
                        self.push(PointsToConstraintKind::ElementLoad {
                            dst,
                            base,
                            index: format!("unknown:{evidence}"),
                        });
                        self.push(PointsToConstraintKind::ElementStore {
                            base,
                            index: format!("unknown:{evidence}"),
                            src: dst,
                        });
                    }
                    AccessPathProjection::Deref => {
                        self.push(PointsToConstraintKind::Load { dst, pointer: base });
                        self.push(PointsToConstraintKind::Store {
                            pointer: base,
                            src: dst,
                        });
                    }
                    AccessPathProjection::CallReturn(call) => {
                        self.push(PointsToConstraintKind::SummaryFlow {
                            dst,
                            src: base,
                            summary_key: format!("call:{}", call.0),
                        });
                    }
                    AccessPathProjection::AwaitResult | AccessPathProjection::Unknown { .. } => {}
                }
            }
        }
    }

    fn push(&mut self, kind: PointsToConstraintKind) {
        let stable_key = constraint_stable_key(&kind);
        self.constraints.push(PointsToConstraintFact {
            id: PointsToConstraintId(self.constraints.len() as u64),
            kind,
            status: PointsToStatus::Present,
            precision: PointsToPrecision::FlowInsensitive,
            stable_key,
        });
    }

    fn finish(mut self) -> Vec<PointsToConstraintFact> {
        self.constraints
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.constraints
            .dedup_by(|left, right| left.stable_key == right.stable_key);
        for (index, constraint) in self.constraints.iter_mut().enumerate() {
            constraint.id = PointsToConstraintId(index as u64);
        }
        self.constraints
    }
}

fn subject_var(subject: &ValueSubject) -> Option<PtVarId> {
    match subject {
        ValueSubject::Place(place) => Some(PtVarId(place.0)),
        ValueSubject::Operation(operation) => Some(PtVarId(500_000 + operation.0)),
        ValueSubject::Allocation(allocation) => Some(PtVarId(700_000 + allocation.0)),
        ValueSubject::Synthetic(_) | ValueSubject::Unknown(_) => None,
    }
}

fn access_path_var(path: &AccessPathFact) -> PtVarId {
    PtVarId(200_000 + path.id.0)
}

fn constraint_stable_key(kind: &PointsToConstraintKind) -> String {
    stable_key_from_parts(
        FactFamily::PointsToConstraint,
        &[("kind", format!("{kind:?}"))],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::access_paths::facts::AccessPathStatus;
    use crate::analysis::access_paths::store::AccessPathOutput;
    use crate::analysis::ids::{AccessPathId, AllocationTokenId, MirBodyId, ValueFactId};
    use crate::analysis::types::store::TypeValueAliasOutput;
    use crate::analysis::values::facts::{ValuePrecision, ValueProvenance, ValueStatus};
    use crate::analysis::values::store::ValueOutput;
    use crate::core::Language;

    #[test]
    fn derives_core_constraints_from_values_and_access_paths() {
        let output = TypeValueAliasOutput {
            values: ValueOutput {
                values: vec![
                    value(
                        ValueFactId(0),
                        ValueSubject::Place(crate::analysis::ids::PlaceId(1)),
                        ValueKind::Object(AllocationTokenId(7)),
                    ),
                    value(
                        ValueFactId(1),
                        ValueSubject::Place(crate::analysis::ids::PlaceId(2)),
                        ValueKind::PlaceRef(crate::analysis::ids::PlaceId(1)),
                    ),
                    value(
                        ValueFactId(2),
                        ValueSubject::Place(crate::analysis::ids::PlaceId(3)),
                        ValueKind::CallReturn(crate::analysis::ids::PlaceId(2)),
                    ),
                ],
                allocations: Vec::new(),
            },
            access_paths: AccessPathOutput {
                access_paths: vec![
                    path(
                        AccessPathId(0),
                        AccessPathProjection::Property("name".to_string()),
                    ),
                    path(
                        AccessPathId(1),
                        AccessPathProjection::IndexUnknown {
                            evidence: "key".to_string(),
                        },
                    ),
                    path(AccessPathId(2), AccessPathProjection::Deref),
                    path(
                        AccessPathId(3),
                        AccessPathProjection::CallReturn(crate::analysis::ids::CallSiteId(9)),
                    ),
                ],
            },
            ..TypeValueAliasOutput::default()
        };

        let constraints = derive_points_to_constraints(&output);
        assert!(
            constraints
                .iter()
                .any(|row| matches!(row.kind, PointsToConstraintKind::AddressOf { .. }))
        );
        assert!(
            constraints
                .iter()
                .any(|row| matches!(row.kind, PointsToConstraintKind::Copy { .. }))
        );
        assert!(
            constraints
                .iter()
                .any(|row| matches!(row.kind, PointsToConstraintKind::FieldLoad { .. }))
        );
        assert!(
            constraints
                .iter()
                .any(|row| matches!(row.kind, PointsToConstraintKind::FieldStore { .. }))
        );
        assert!(
            constraints
                .iter()
                .any(|row| matches!(row.kind, PointsToConstraintKind::ElementLoad { .. }))
        );
        assert!(
            constraints
                .iter()
                .any(|row| matches!(row.kind, PointsToConstraintKind::ElementStore { .. }))
        );
        assert!(
            constraints
                .iter()
                .any(|row| matches!(row.kind, PointsToConstraintKind::Load { .. }))
        );
        assert!(
            constraints
                .iter()
                .any(|row| matches!(row.kind, PointsToConstraintKind::Store { .. }))
        );
        assert!(
            constraints
                .iter()
                .any(|row| matches!(row.kind, PointsToConstraintKind::CallReturn { .. }))
        );
        assert!(
            constraints
                .iter()
                .any(|row| matches!(row.kind, PointsToConstraintKind::SummaryFlow { .. }))
        );
    }

    fn value(id: ValueFactId, subject: ValueSubject, kind: ValueKind) -> ValueFact {
        ValueFact {
            id,
            subject,
            value: crate::analysis::ids::AbstractValueId(id.0),
            kind,
            language: Language::TypeScript,
            file: None,
            function: None,
            body: Some(MirBodyId(0)),
            precision: ValuePrecision::Heuristic,
            status: ValueStatus::Present,
            provenance: ValueProvenance::Native,
            stable_key: format!("value:{}", id.0),
        }
    }

    fn path(id: AccessPathId, projection: AccessPathProjection) -> AccessPathFact {
        AccessPathFact {
            id,
            base: crate::analysis::ids::PlaceId(1),
            projections: vec![projection],
            depth: 1,
            language: Language::TypeScript,
            file: None,
            function: None,
            body: Some(MirBodyId(0)),
            status: AccessPathStatus::Partial,
            stable_key: format!("path:{}", id.0),
        }
    }
}
