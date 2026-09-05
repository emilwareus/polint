use std::collections::BTreeMap;

use super::core::{
    ConstantDomain, ConstantLiteral, InitializednessDomain, NilnessDomain, ReachabilityDomain,
    TruthinessDomain,
};
use super::lattice::{AbstractDomain, TopReason};
use super::state::ProductState;
use crate::analysis_neutral::calls::facts::{UnresolvedCallFact, UnresolvedCallReason};
use crate::analysis_neutral::host::AnalysisHost;
use crate::analysis_neutral::ids::{CallSiteId, PlaceId, UnsupportedId};
use crate::analysis_neutral::mir_op::{
    AssignMode, ConservativeAction, MirOperation, MirOperationKind, MirValue,
    UnsupportedSemanticFact,
};

#[derive(Clone, Debug)]
pub struct TransferCx<'a> {
    unresolved_calls: BTreeMap<CallSiteId, &'a UnresolvedCallFact>,
    unsupported: BTreeMap<UnsupportedId, &'a UnsupportedSemanticFact>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationTransfer;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeTransfer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BranchSense {
    True,
    False,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferReason {
    LiteralAssignment,
    PlaceCopy,
    UnknownValue,
    UnsupportedSemantic,
    UnresolvedCall(UnresolvedCallReason),
    DynamicWrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HavocTarget {
    Place(PlaceId),
    ReturnPlace(PlaceId),
    Argument(PlaceId),
}

impl<'a> TransferCx<'a> {
    pub fn from_db(db: &'a impl AnalysisHost) -> Self {
        Self {
            unresolved_calls: db
                .unresolved_calls()
                .iter()
                .map(|row| (row.site, row))
                .collect(),
            unsupported: db
                .unsupported_semantics()
                .iter()
                .map(|row| (row.id, row))
                .collect(),
        }
    }

    #[cfg(test)]
    pub fn empty_for_test() -> Self {
        Self {
            unresolved_calls: BTreeMap::new(),
            unsupported: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    pub fn with_unresolved_for_test(
        site: CallSiteId,
        reason: UnresolvedCallReason,
    ) -> TransferCx<'static> {
        use crate::analysis_neutral::calls::facts::{
            CallAlgorithm, CallPrecision, CallProvenance, CallTargetStatus,
        };
        use crate::internal_core::FunctionId;

        let row = Box::leak(Box::new(UnresolvedCallFact {
            site,
            caller: FunctionId::from_raw(1),
            status: CallTargetStatus::Unresolved,
            reason,
            algorithm: CallAlgorithm::Unsupported,
            provenance: CallProvenance::MirShape,
            precision: CallPrecision::Unknown,
            stable_key: crate::internal_core::StableKeyId(0),
        }));
        TransferCx {
            unresolved_calls: BTreeMap::from([(site, &*row)]),
            unsupported: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    pub fn add_unsupported_for_test(
        &mut self,
        interner: &crate::internal_core::StableKeyInterner,
        unsupported: UnsupportedId,
        affected_places: Vec<PlaceId>,
    ) {
        use crate::analysis_neutral::mir_body::MirStatus;
        use crate::analysis_neutral::mir_op::{UnsupportedDomain, UnsupportedPrecision};
        use crate::internal_core::{FileId, Language, Span};

        let row = Box::leak(Box::new(UnsupportedSemanticFact {
            id: unsupported,
            body: None,
            operation: None,
            language: Language::Go,
            file: FileId::from_raw(1),
            span: Span::point(FileId::from_raw(1), 1, 1),
            construct: "unsupported".to_string(),
            source_evidence: "unsupported".to_string(),
            affected_places,
            affected_domains: vec![UnsupportedDomain::Domains],
            conservative_action: ConservativeAction::HavocAffectedPlaces,
            precision: UnsupportedPrecision::Unsupported,
            status: MirStatus::Unsupported,
            stable_key: interner.intern("unsupported:test".to_string()),
        }));
        self.unsupported.insert(unsupported, &*row);
    }
}

impl OperationTransfer {
    pub fn apply(cx: &TransferCx<'_>, operation: &MirOperation, state: &mut ProductState) {
        match &operation.kind {
            MirOperationKind::StorageLive { place } => {
                state
                    .core
                    .initializedness
                    .insert(*place, InitializednessDomain::Uninitialized);
            }
            MirOperationKind::Bind { place, value } => assign_value(state, *place, value),
            MirOperationKind::Assign { place, value, mode } => {
                if dynamic_assign_mode(*mode) {
                    havoc(state, HavocTarget::Place(*place), TopReason::DynamicWrite);
                } else {
                    assign_value(state, *place, value);
                }
            }
            MirOperationKind::Read { place } => {
                state
                    .core
                    .initializedness
                    .entry(*place)
                    .or_insert(InitializednessDomain::MaybeUninitialized);
            }
            MirOperationKind::Write { place, .. } => {
                havoc(state, HavocTarget::Place(*place), TopReason::DynamicWrite);
            }
            MirOperationKind::Branch { .. } => {}
            MirOperationKind::Call {
                site,
                arguments,
                return_place,
                ..
            } => apply_call(cx, *site, arguments, *return_place, state),
            MirOperationKind::Return { value } => {
                if let Some(MirValue::Unknown { .. }) = value {
                    state.mark_reachability_top(TopReason::UnknownValue);
                }
            }
            MirOperationKind::Unsupported { unsupported } => {
                apply_unsupported(cx, *unsupported, state);
            }
        }
        state.reduce_value_only(4);
    }

    pub fn apply_return_value(state: &mut ProductState, return_place: PlaceId, value: &MirValue) {
        assign_value(state, return_place, value);
        state.reduce_value_only(4);
    }
}

impl EdgeTransfer {
    pub fn apply_branch_assumption(
        _cx: &TransferCx<'_>,
        predicate_place: Option<PlaceId>,
        nil_on_true: Option<bool>,
        sense: BranchSense,
        state: &mut ProductState,
    ) {
        let Some(place) = predicate_place else {
            return;
        };
        if let Some(nil_on_true) = nil_on_true {
            let desired_nil = nil_on_true == matches!(sense, BranchSense::True);
            if !refine_nilness(
                state,
                place,
                if desired_nil {
                    NilnessDomain::Nil
                } else {
                    NilnessDomain::NonNil
                },
            ) {
                state.core.reachability = ReachabilityDomain::Unreachable;
            }
            state.reduce_value_only(4);
            return;
        }
        match sense {
            BranchSense::True => {
                if !refine_truthiness(state, place, TruthinessDomain::Truthy)
                    || !refine_nilness_for_truthy(state, place)
                    || !refine_bool_constant(state, place, true)
                {
                    state.core.reachability = ReachabilityDomain::Unreachable;
                    return;
                }
            }
            BranchSense::False => {
                if !refine_truthiness(state, place, TruthinessDomain::Falsy)
                    || !refine_bool_constant(state, place, false)
                {
                    state.core.reachability = ReachabilityDomain::Unreachable;
                    return;
                }
            }
        }
        state.reduce_value_only(4);
    }
}

fn refine_nilness(state: &mut ProductState, place: PlaceId, desired: NilnessDomain) -> bool {
    let current = state
        .core
        .nilness
        .get(&place)
        .copied()
        .unwrap_or_else(NilnessDomain::bottom);
    if matches!(
        (current, desired),
        (NilnessDomain::Nil, NilnessDomain::NonNil) | (NilnessDomain::NonNil, NilnessDomain::Nil)
    ) {
        return false;
    }
    state.core.nilness.insert(place, desired);
    true
}

fn refine_truthiness(state: &mut ProductState, place: PlaceId, desired: TruthinessDomain) -> bool {
    let current = state
        .core
        .truthiness
        .get(&place)
        .copied()
        .unwrap_or_else(TruthinessDomain::bottom);
    match (current, desired) {
        (TruthinessDomain::Truthy, TruthinessDomain::Falsy)
        | (TruthinessDomain::Falsy, TruthinessDomain::Truthy) => false,
        (TruthinessDomain::Top(_), desired) => {
            state.core.truthiness.insert(place, desired);
            true
        }
        (_, desired) => {
            state.core.truthiness.insert(place, desired);
            true
        }
    }
}

fn refine_nilness_for_truthy(state: &mut ProductState, place: PlaceId) -> bool {
    let current = state
        .core
        .nilness
        .get(&place)
        .copied()
        .unwrap_or_else(NilnessDomain::bottom);
    match current {
        NilnessDomain::Nil => false,
        NilnessDomain::NonNil => true,
        NilnessDomain::Bottom | NilnessDomain::MaybeNil | NilnessDomain::Top(_) => {
            state.core.nilness.insert(place, NilnessDomain::NonNil);
            true
        }
    }
}

fn refine_bool_constant(state: &mut ProductState, place: PlaceId, desired: bool) -> bool {
    let Some(current) = state.core.constants.get(&place).cloned() else {
        return true;
    };
    let ConstantDomain::Values(values) = current else {
        return true;
    };
    if values
        .iter()
        .all(|literal| matches!(literal, ConstantLiteral::Bool(_)))
    {
        if values.contains(&ConstantLiteral::Bool(desired)) {
            state.core.constants.insert(
                place,
                ConstantDomain::from_literal(ConstantLiteral::Bool(desired)),
            );
            true
        } else {
            false
        }
    } else {
        true
    }
}

fn assign_value(state: &mut ProductState, place: PlaceId, value: &MirValue) {
    match value {
        MirValue::Literal { value } => {
            clear_place_facts(state, place);
            mark_initialized(state, place);
            apply_literal(state, place, value);
        }
        MirValue::Place(source) => {
            if *source != place {
                clear_place_facts(state, place);
                copy_place(state, place, *source);
            }
            mark_initialized(state, place);
        }
        MirValue::Aggregate { .. } | MirValue::Closure { .. } => {
            clear_place_facts(state, place);
            mark_initialized(state, place);
        }
        MirValue::Temporary(_)
        | MirValue::CallReturn(_)
        | MirValue::BinOp { .. }
        | MirValue::Unknown { .. } => {
            clear_place_facts(state, place);
            state.mark_place_top(place, TopReason::UnknownValue);
        }
    }
}

fn mark_initialized(state: &mut ProductState, place: PlaceId) {
    state
        .core
        .initializedness
        .insert(place, InitializednessDomain::Initialized);
}

fn clear_place_facts(state: &mut ProductState, place: PlaceId) {
    state.core.nilness.remove(&place);
    state.core.truthiness.remove(&place);
    state.core.constants.remove(&place);
    state.core.strings.remove(&place);
    state.core.initializedness.remove(&place);
}

fn apply_literal(state: &mut ProductState, place: PlaceId, label: &str) {
    let literal = literal_from_label(label);
    state
        .core
        .constants
        .insert(place, ConstantDomain::from_literal(literal));
}

fn literal_from_label(label: &str) -> ConstantLiteral {
    match label {
        "true" => ConstantLiteral::Bool(true),
        "false" => ConstantLiteral::Bool(false),
        "null" => ConstantLiteral::Null,
        "undefined" => ConstantLiteral::Undefined,
        "nil" => ConstantLiteral::Nil,
        value if quoted(value) => ConstantLiteral::String(value[1..value.len() - 1].to_string()),
        value if numeric_label(value) => ConstantLiteral::Number(value.to_string()),
        value => ConstantLiteral::Unknown(value.to_string()),
    }
}

fn quoted(value: &str) -> bool {
    value.len() >= 2 && value.starts_with('"') && value.ends_with('"')
}

fn numeric_label(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'.' | b'-'))
}

fn copy_place(state: &mut ProductState, target: PlaceId, source: PlaceId) {
    if let Some(value) = state.core.nilness.get(&source).copied() {
        state.core.nilness.insert(target, value);
    }
    if let Some(value) = state.core.truthiness.get(&source).copied() {
        state.core.truthiness.insert(target, value);
    }
    if let Some(value) = state.core.constants.get(&source).cloned() {
        state.core.constants.insert(target, value);
    }
    if let Some(value) = state.core.strings.get(&source).cloned() {
        state.core.strings.insert(target, value);
    }
}

fn dynamic_assign_mode(mode: AssignMode) -> bool {
    matches!(
        mode,
        AssignMode::PartialWrite | AssignMode::ProjectionMutation | AssignMode::UnknownWrite
    )
}

fn apply_call(
    cx: &TransferCx<'_>,
    site: CallSiteId,
    arguments: &[PlaceId],
    return_place: PlaceId,
    state: &mut ProductState,
) {
    if let Some(unresolved) = cx.unresolved_calls.get(&site) {
        let reason = top_reason_for_unresolved(unresolved);
        havoc(state, HavocTarget::ReturnPlace(return_place), reason);
        for argument in arguments {
            havoc(state, HavocTarget::Argument(*argument), reason);
        }
    } else {
        state.mark_place_top(return_place, TopReason::UnknownValue);
        for argument in arguments {
            state
                .core
                .initializedness
                .entry(*argument)
                .or_insert(InitializednessDomain::Initialized);
        }
    }
}

fn top_reason_for_unresolved(unresolved: &UnresolvedCallFact) -> TopReason {
    match unresolved.reason {
        UnresolvedCallReason::SetupMissing => TopReason::SetupMissing,
        UnresolvedCallReason::BudgetExceeded => TopReason::BudgetExceeded,
        _ => TopReason::UnresolvedCall,
    }
}

fn apply_unsupported(cx: &TransferCx<'_>, unsupported: UnsupportedId, state: &mut ProductState) {
    let Some(row) = cx.unsupported.get(&unsupported) else {
        state.mark_reachability_top(TopReason::UnsupportedSemantic);
        return;
    };
    match row.conservative_action {
        ConservativeAction::SkipOperation => {}
        ConservativeAction::HavocAffectedPlaces => {
            for place in &row.affected_places {
                havoc(
                    state,
                    HavocTarget::Place(*place),
                    TopReason::UnsupportedSemantic,
                );
            }
        }
        ConservativeAction::PreserveWithUnknownValue | ConservativeAction::StopLowering => {
            state.mark_reachability_top(TopReason::UnsupportedSemantic);
        }
    }
}

fn havoc(state: &mut ProductState, target: HavocTarget, reason: TopReason) {
    let place = match target {
        HavocTarget::Place(place)
        | HavocTarget::ReturnPlace(place)
        | HavocTarget::Argument(place) => place,
    };
    state.mark_place_top(place, reason);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_neutral::calls::facts::UnresolvedCallReason;
    use crate::analysis_neutral::domains::core::{
        ConstantDomain, ConstantLiteral, InitializednessDomain, NilnessDomain, StringDomain,
        TruthinessDomain,
    };
    use crate::analysis_neutral::domains::lattice::{AbstractDomain, TopReason};
    use crate::analysis_neutral::domains::state::ProductState;
    use crate::analysis_neutral::ids::{CallSiteId, MirOpId, PlaceId, UnsupportedId};
    use crate::analysis_neutral::mir_body::MirStatus;
    use crate::analysis_neutral::mir_op::{AssignMode, MirOperation, MirOperationKind, MirValue};
    use crate::internal_core::{FileId, Span};

    #[test]
    fn operation_transfer_is_monotone_for_literal_assignment_samples() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let place = PlaceId(1);
        let operation = operation(
            &interner,
            MirOperationKind::Assign {
                place,
                value: MirValue::Literal {
                    value: "true".to_string(),
                },
                mode: AssignMode::Overwrite,
            },
        );
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
        let interner = crate::internal_core::StableKeyInterner::default();
        let place = PlaceId(1);
        let operation = operation(
            &interner,
            MirOperationKind::Bind {
                place,
                value: MirValue::Literal {
                    value: "\"route\"".to_string(),
                },
            },
        );
        let mut state = ProductState::entry();

        OperationTransfer::apply(&TransferCx::empty_for_test(), &operation, &mut state);

        assert_eq!(
            state.core.constants[&place],
            ConstantDomain::from_literal(ConstantLiteral::String("route".to_string()))
        );
        assert_eq!(state.core.nilness[&place], NilnessDomain::NonNil);
        assert_eq!(state.core.truthiness[&place], TruthinessDomain::Truthy);
        assert_eq!(
            state.core.strings[&place],
            StringDomain::from_literal("route")
        );
        assert_eq!(
            state.core.initializedness[&place],
            InitializednessDomain::Initialized
        );
    }

    #[test]
    fn literal_overwrite_clears_stale_derived_string_slot() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let place = PlaceId(1);
        let operation = operation(
            &interner,
            MirOperationKind::Assign {
                place,
                value: MirValue::Literal {
                    value: "42".to_string(),
                },
                mode: AssignMode::Overwrite,
            },
        );
        let mut state = ProductState::entry();
        state
            .core
            .strings
            .insert(place, StringDomain::from_literal("stale"));

        OperationTransfer::apply(&TransferCx::empty_for_test(), &operation, &mut state);

        assert!(!state.core.strings.contains_key(&place));
    }

    #[test]
    fn copy_overwrite_clears_target_slots_missing_from_source() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let target = PlaceId(1);
        let source = PlaceId(2);
        let operation = operation(
            &interner,
            MirOperationKind::Assign {
                place: target,
                value: MirValue::Place(source),
                mode: AssignMode::Overwrite,
            },
        );
        let mut state = ProductState::entry();
        state.core.constants.insert(
            source,
            ConstantDomain::from_literal(ConstantLiteral::Bool(false)),
        );
        state
            .core
            .strings
            .insert(target, StringDomain::from_literal("stale"));

        OperationTransfer::apply(&TransferCx::empty_for_test(), &operation, &mut state);

        assert!(!state.core.strings.contains_key(&target));
    }

    #[test]
    fn branch_assumption_refines_truthiness_and_constant_bools() {
        let place = PlaceId(7);
        let mut state = ProductState::entry();
        state
            .core
            .truthiness
            .insert(place, TruthinessDomain::Top(TopReason::UnknownValue));
        state
            .core
            .nilness
            .insert(place, NilnessDomain::Top(TopReason::UnknownValue));
        state.core.constants.insert(
            place,
            ConstantDomain::from_literal(ConstantLiteral::Bool(true)),
        );

        EdgeTransfer::apply_branch_assumption(
            &TransferCx::empty_for_test(),
            Some(place),
            None,
            BranchSense::True,
            &mut state,
        );

        assert_eq!(state.core.truthiness[&place], TruthinessDomain::Truthy);
        assert_eq!(state.core.nilness[&place], NilnessDomain::NonNil);
    }

    #[test]
    fn branch_assumption_without_place_is_conservative() {
        let unrelated = PlaceId(7);
        let mut state = ProductState::entry();

        EdgeTransfer::apply_branch_assumption(
            &TransferCx::empty_for_test(),
            None,
            None,
            BranchSense::True,
            &mut state,
        );

        assert!(!state.core.truthiness.contains_key(&unrelated));
    }

    #[test]
    fn relational_nil_constraint_refines_opposite_edges() {
        let place = PlaceId(7);
        let mut true_edge = ProductState::entry();
        let mut false_edge = ProductState::entry();

        EdgeTransfer::apply_branch_assumption(
            &TransferCx::empty_for_test(),
            Some(place),
            Some(false),
            BranchSense::True,
            &mut true_edge,
        );
        EdgeTransfer::apply_branch_assumption(
            &TransferCx::empty_for_test(),
            Some(place),
            Some(false),
            BranchSense::False,
            &mut false_edge,
        );

        assert_eq!(true_edge.core.nilness[&place], NilnessDomain::NonNil);
        assert_eq!(false_edge.core.nilness[&place], NilnessDomain::Nil);
    }

    #[test]
    fn unresolved_call_havocs_return_place_with_explicit_reason() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let return_place = PlaceId(3);
        let operation = operation(
            &interner,
            MirOperationKind::Call {
                site: CallSiteId(9),
                callee: MirValue::Unknown {
                    evidence: "dynamic".to_string(),
                },
                arguments: vec![PlaceId(1)],
                return_place,
            },
        );
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
        let interner = crate::internal_core::StableKeyInterner::default();
        let place = PlaceId(1);
        let mut cx = TransferCx::empty_for_test();
        cx.add_unsupported_for_test(&interner, UnsupportedId(2), vec![place]);
        let operation = operation(
            &interner,
            MirOperationKind::Unsupported {
                unsupported: UnsupportedId(2),
            },
        );
        let mut state = ProductState::entry();

        OperationTransfer::apply(&cx, &operation, &mut state);

        assert_eq!(
            state.core.constants[&place],
            ConstantDomain::top(TopReason::UnsupportedSemantic)
        );
    }

    #[test]
    fn dynamic_write_havocs_written_place() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let place = PlaceId(1);
        let operation = operation(
            &interner,
            MirOperationKind::Write {
                place,
                value: MirValue::Unknown {
                    evidence: "computed property".to_string(),
                },
            },
        );
        let mut state = ProductState::entry();

        OperationTransfer::apply(&TransferCx::empty_for_test(), &operation, &mut state);

        assert_eq!(
            state.core.constants[&place],
            ConstantDomain::top(TopReason::DynamicWrite)
        );
    }

    #[test]
    fn read_without_prior_write_is_maybe_uninitialized_not_initialized() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let place = PlaceId(1);
        let operation = operation(&interner, MirOperationKind::Read { place });
        let mut state = ProductState::entry();

        OperationTransfer::apply(&TransferCx::empty_for_test(), &operation, &mut state);

        assert_eq!(
            state.core.initializedness[&place],
            InitializednessDomain::MaybeUninitialized
        );
    }

    #[test]
    fn branch_true_contradicting_falsy_state_marks_path_unreachable() {
        let place = PlaceId(1);
        let mut state = ProductState::entry();
        state.core.truthiness.insert(place, TruthinessDomain::Falsy);

        EdgeTransfer::apply_branch_assumption(
            &TransferCx::empty_for_test(),
            Some(place),
            None,
            BranchSense::True,
            &mut state,
        );

        assert_eq!(state.core.reachability, ReachabilityDomain::Unreachable);
        assert_eq!(state.core.truthiness[&place], TruthinessDomain::Falsy);
    }

    #[test]
    fn branch_refines_existing_boolean_constant_without_inventing_contradictions() {
        let place = PlaceId(1);
        let mut state = ProductState::entry();
        state.core.constants.insert(
            place,
            ConstantDomain::from_literal(ConstantLiteral::Bool(false)),
        );

        EdgeTransfer::apply_branch_assumption(
            &TransferCx::empty_for_test(),
            Some(place),
            None,
            BranchSense::True,
            &mut state,
        );

        assert_eq!(state.core.reachability, ReachabilityDomain::Unreachable);
        assert_eq!(
            state.core.constants[&place],
            ConstantDomain::from_literal(ConstantLiteral::Bool(false))
        );
    }

    fn operation(
        interner: &crate::internal_core::StableKeyInterner,
        kind: MirOperationKind,
    ) -> MirOperation {
        MirOperation {
            id: MirOpId(1),
            body: crate::analysis_neutral::ids::MirBodyId(1),
            ordinal: 1,
            span: Span::point(FileId::from_raw(1), 1, 1),
            kind,
            stable_key: interner.intern("op:test".to_string()),
            status: MirStatus::Resolved,
        }
    }
}
