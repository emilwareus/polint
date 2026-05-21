#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::UnresolvedCallReason;
    use crate::analysis::domains::core::{
        ConstantDomain, ConstantLiteral, InitializednessDomain, NilnessDomain, StringDomain,
        TruthinessDomain,
    };
    use crate::analysis::domains::lattice::{AbstractDomain, TopReason};
    use crate::analysis::domains::state::ProductState;
    use crate::analysis::ids::{CallSiteId, MirOpId, MirPredicateId, PlaceId, UnsupportedId};
    use crate::analysis::mir::body::MirStatus;
    use crate::analysis::mir::op::{AssignMode, MirOperation, MirOperationKind, MirValue};
    use crate::core::{FileId, Span};

    #[test]
    fn operation_transfer_is_monotone_for_literal_assignment_samples() {
        let place = PlaceId(1);
        let operation = operation(MirOperationKind::Assign {
            place,
            value: MirValue::Literal {
                value: "true".to_string(),
            },
            mode: AssignMode::Overwrite,
        });
        let mut lower = ProductState::entry();
        let mut upper = ProductState::entry();
        upper.core.truthiness.insert(place, TruthinessDomain::Maybe);

        let cx = TransferCx::empty_for_test();
        OperationTransfer::apply(&cx, &operation, &mut lower);
        OperationTransfer::apply(&cx, &operation, &mut upper);

        assert!(lower.leq(&upper));
    }

    #[test]
    fn literal_assignment_updates_p0_value_slots() {
        let place = PlaceId(1);
        let operation = operation(MirOperationKind::Bind {
            place,
            value: MirValue::Literal {
                value: "\"route\"".to_string(),
            },
        });
        let mut state = ProductState::entry();

        OperationTransfer::apply(&TransferCx::empty_for_test(), &operation, &mut state);

        assert_eq!(
            state.core.constants[&place],
            ConstantDomain::from_literal(ConstantLiteral::String("route".to_string()))
        );
        assert_eq!(state.core.nilness[&place], NilnessDomain::NonNil);
        assert_eq!(state.core.truthiness[&place], TruthinessDomain::Truthy);
        assert_eq!(state.core.strings[&place], StringDomain::from_literal("route"));
        assert_eq!(
            state.core.initializedness[&place],
            InitializednessDomain::Initialized
        );
    }

    #[test]
    fn branch_assumption_refines_truthiness_and_constant_bools() {
        let place = PlaceId(7);
        let mut state = ProductState::entry();
        state
            .core
            .constants
            .insert(place, ConstantDomain::from_literal(ConstantLiteral::Bool(true)));

        EdgeTransfer::apply_branch_assumption(
            &TransferCx::empty_for_test(),
            MirPredicateId(place.0),
            BranchSense::True,
            &mut state,
        );

        assert_eq!(state.core.truthiness[&place], TruthinessDomain::Truthy);
    }

    #[test]
    fn unresolved_call_havocs_return_place_with_explicit_reason() {
        let return_place = PlaceId(3);
        let operation = operation(MirOperationKind::Call {
            site: CallSiteId(9),
            callee: MirValue::Unknown {
                evidence: "dynamic".to_string(),
            },
            arguments: vec![PlaceId(1)],
            return_place,
        });
        let cx = TransferCx::with_unresolved_for_test(
            CallSiteId(9),
            UnresolvedCallReason::UnknownCallee,
        );
        let mut state = ProductState::entry();

        OperationTransfer::apply(&cx, &operation, &mut state);

        assert_eq!(
            state.core.constants[&return_place],
            ConstantDomain::top(TopReason::UnresolvedCall)
        );
    }

    #[test]
    fn unsupported_operation_havocs_affected_places() {
        let place = PlaceId(1);
        let mut cx = TransferCx::empty_for_test();
        cx.add_unsupported_for_test(UnsupportedId(2), vec![place]);
        let operation = operation(MirOperationKind::Unsupported {
            unsupported: UnsupportedId(2),
        });
        let mut state = ProductState::entry();

        OperationTransfer::apply(&cx, &operation, &mut state);

        assert_eq!(
            state.core.constants[&place],
            ConstantDomain::top(TopReason::UnsupportedSemantic)
        );
    }

    #[test]
    fn dynamic_write_havocs_written_place() {
        let place = PlaceId(1);
        let operation = operation(MirOperationKind::Write {
            place,
            value: MirValue::Unknown {
                evidence: "computed property".to_string(),
            },
        });
        let mut state = ProductState::entry();

        OperationTransfer::apply(&TransferCx::empty_for_test(), &operation, &mut state);

        assert_eq!(
            state.core.constants[&place],
            ConstantDomain::top(TopReason::DynamicWrite)
        );
    }

    fn operation(kind: MirOperationKind) -> MirOperation {
        MirOperation {
            id: MirOpId(1),
            body: crate::analysis::ids::MirBodyId(1),
            ordinal: 1,
            span: Span::point(FileId(1), 1, 1),
            kind,
            stable_key: "op:test".to_string(),
            status: MirStatus::Resolved,
        }
    }
}
