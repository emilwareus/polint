#![expect(
    dead_code,
    reason = "Phase 31 stores domain rows before provider/debug/eval plans consume every index."
)]

use std::collections::BTreeMap;

use super::core::{
    ConstantDomain, InitializednessDomain, NilnessDomain, ReachabilityDomain, StringDomain,
    TruthinessDomain,
};
use super::facts::{
    DomainEventFact, DomainLocation, DomainObservationFact, DomainPrecision, DomainSlot,
    DomainStatus, DomainValue,
};
use super::lattice::{AbstractDomain, TopReason};
use super::results::{DomainResults, SolverStatus};
use super::state::ProductState;
use crate::analysis::cfg::ids::BasicBlockId;
use crate::analysis::ids::{DomainEventId, DomainObservationId, MirBodyId, MirOpId, PlaceId};
use crate::analysis_kernel::{FactFamily, stable_key_from_parts};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DomainOutput {
    pub(crate) observations: Vec<DomainObservationFact>,
    pub(crate) events: Vec<DomainEventFact>,
}

impl DomainOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn from_results(results: &DomainResults) -> Self {
        Self::from_results_with_place_filter(results, None)
    }

    pub(crate) fn from_results_with_place_keys(
        results: &DomainResults,
        place_stable_keys: &BTreeMap<PlaceId, String>,
    ) -> Self {
        Self::from_results_with_place_filter(results, Some(place_stable_keys))
    }

    fn from_results_with_place_filter(
        results: &DomainResults,
        place_stable_keys: Option<&BTreeMap<PlaceId, String>>,
    ) -> Self {
        let mut output = Self::empty();
        for function in results.functions() {
            push_state_observations(
                &mut output.observations,
                function.body,
                None,
                None,
                DomainLocation::FunctionEntry,
                &function.body_stable_key,
                &function.entry_state,
                place_stable_keys,
            );
            if function.status == SolverStatus::BudgetExceeded {
                output.events.push(DomainEventFact {
                    id: DomainEventId(0),
                    body: function.body,
                    block: None,
                    operation: None,
                    slot: None,
                    status: DomainStatus::BudgetExceeded,
                    precision: DomainPrecision::Unknown,
                    reason: "solver_budget_exceeded".to_string(),
                    stable_key: stable_key_from_parts(
                        FactFamily::DomainEvent,
                        &[
                            ("body", function.body_stable_key.clone()),
                            ("reason", "solver_budget_exceeded".to_string()),
                        ],
                    ),
                });
            }
        }
        for block in results.blocks() {
            push_state_observations(
                &mut output.observations,
                block.body,
                Some(block.block),
                None,
                DomainLocation::BlockEntry,
                &block.stable_key,
                &block.entry,
                place_stable_keys,
            );
            push_state_observations(
                &mut output.observations,
                block.body,
                Some(block.block),
                None,
                DomainLocation::BlockExit,
                &block.stable_key,
                &block.exit,
                place_stable_keys,
            );
        }
        for operation in results.operations() {
            push_state_observations(
                &mut output.observations,
                operation.body,
                Some(operation.block),
                Some(operation.operation),
                DomainLocation::BeforeOperation,
                &operation.stable_key,
                &operation.before,
                place_stable_keys,
            );
            push_state_observations(
                &mut output.observations,
                operation.body,
                Some(operation.block),
                Some(operation.operation),
                DomainLocation::AfterOperation,
                &operation.stable_key,
                &operation.after,
                place_stable_keys,
            );
        }
        for event in results.unknown_top_events() {
            output.events.push(DomainEventFact {
                id: DomainEventId(0),
                body: event.body,
                block: event.block,
                operation: event.operation,
                slot: None,
                status: status_for_top_reason(event.reason),
                precision: precision_for_top_reason(event.reason),
                reason: event.reason.as_str().to_string(),
                stable_key: stable_key_from_parts(
                    FactFamily::DomainEvent,
                    &[
                        ("source", event.stable_key.clone()),
                        ("reason", event.reason.as_str().to_string()),
                    ],
                ),
            });
        }
        output.normalized()
    }

    pub(crate) fn normalized(mut self) -> Self {
        self.observations.sort_by(|left, right| {
            (
                left.stable_key.as_str(),
                left.body,
                left.block,
                left.operation,
                left.place,
                left.slot,
                left.location,
                left.status,
                left.id,
            )
                .cmp(&(
                    right.stable_key.as_str(),
                    right.body,
                    right.block,
                    right.operation,
                    right.place,
                    right.slot,
                    right.location,
                    right.status,
                    right.id,
                ))
        });
        self.events.sort_by(|left, right| {
            (
                left.stable_key.as_str(),
                left.body,
                left.block,
                left.operation,
                left.slot,
                left.status,
                left.reason.as_str(),
                left.id,
            )
                .cmp(&(
                    right.stable_key.as_str(),
                    right.body,
                    right.block,
                    right.operation,
                    right.slot,
                    right.status,
                    right.reason.as_str(),
                    right.id,
                ))
        });
        for (index, fact) in self.observations.iter_mut().enumerate() {
            fact.id = DomainObservationId(index as u64);
        }
        for (index, fact) in self.events.iter_mut().enumerate() {
            fact.id = DomainEventId(index as u64);
        }
        self
    }
}

#[allow(clippy::too_many_arguments)]
fn push_state_observations(
    rows: &mut Vec<DomainObservationFact>,
    body: MirBodyId,
    block: Option<BasicBlockId>,
    operation: Option<MirOpId>,
    location: DomainLocation,
    source_stable_key: &str,
    state: &ProductState,
    place_stable_keys: Option<&BTreeMap<PlaceId, String>>,
) {
    let (status, precision, value) = reachability_fact_value(&state.core.reachability);
    rows.push(observation(
        body,
        block,
        operation,
        None,
        None,
        DomainSlot::Reachability,
        location,
        source_stable_key,
        status,
        precision,
        value,
    ));
    for (place, value) in &state.core.nilness {
        let (status, precision, value) = nilness_fact_value(value);
        let (place_ref, stable_place) = stable_place_ref(*place, place_stable_keys);
        rows.push(observation(
            body,
            block,
            operation,
            place_ref,
            stable_place,
            DomainSlot::Nilness,
            location,
            source_stable_key,
            status,
            precision,
            value,
        ));
    }
    for (place, value) in &state.core.truthiness {
        let (status, precision, value) = truthiness_fact_value(value);
        let (place_ref, stable_place) = stable_place_ref(*place, place_stable_keys);
        rows.push(observation(
            body,
            block,
            operation,
            place_ref,
            stable_place,
            DomainSlot::Truthiness,
            location,
            source_stable_key,
            status,
            precision,
            value,
        ));
    }
    for (place, value) in &state.core.constants {
        let (status, precision, value) = constant_fact_value(value);
        let (place_ref, stable_place) = stable_place_ref(*place, place_stable_keys);
        rows.push(observation(
            body,
            block,
            operation,
            place_ref,
            stable_place,
            DomainSlot::Constants,
            location,
            source_stable_key,
            status,
            precision,
            value,
        ));
    }
    for (place, value) in &state.core.strings {
        let (status, precision, value) = string_fact_value(value);
        let (place_ref, stable_place) = stable_place_ref(*place, place_stable_keys);
        rows.push(observation(
            body,
            block,
            operation,
            place_ref,
            stable_place,
            DomainSlot::Strings,
            location,
            source_stable_key,
            status,
            precision,
            value,
        ));
    }
    for (place, value) in &state.core.initializedness {
        let (status, precision, value) = initializedness_fact_value(value);
        let (place_ref, stable_place) = stable_place_ref(*place, place_stable_keys);
        rows.push(observation(
            body,
            block,
            operation,
            place_ref,
            stable_place,
            DomainSlot::Initializedness,
            location,
            source_stable_key,
            status,
            precision,
            value,
        ));
    }
}

fn stable_place_ref(
    place: PlaceId,
    place_stable_keys: Option<&BTreeMap<PlaceId, String>>,
) -> (Option<PlaceId>, Option<String>) {
    match place_stable_keys {
        Some(place_stable_keys) => place_stable_keys
            .get(&place)
            .map(|stable_key| (Some(place), Some(stable_key.clone())))
            .unwrap_or((None, None)),
        None => (Some(place), Some(format!("place:{}", place.0))),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Domain fact identity is a normalized tuple over location, slot, and optional place."
)]
fn observation(
    body: MirBodyId,
    block: Option<BasicBlockId>,
    operation: Option<MirOpId>,
    place: Option<PlaceId>,
    stable_place: Option<String>,
    slot: DomainSlot,
    location: DomainLocation,
    source_stable_key: &str,
    status: DomainStatus,
    precision: DomainPrecision,
    value: DomainValue,
) -> DomainObservationFact {
    let (status, precision, value) =
        normalize_observation_value_for_location(operation, status, precision, value);
    DomainObservationFact {
        id: DomainObservationId(0),
        body,
        block,
        operation,
        place,
        slot,
        location,
        value,
        status,
        precision,
        stable_key: stable_key_from_parts(
            FactFamily::DomainObservation,
            &[
                ("source", source_stable_key.to_string()),
                ("slot", slot.as_str().to_string()),
                ("location", location.as_str().to_string()),
                ("place", stable_place.unwrap_or_else(|| "none".to_string())),
            ],
        ),
    }
}

fn normalize_observation_value_for_location(
    operation: Option<MirOpId>,
    status: DomainStatus,
    precision: DomainPrecision,
    value: DomainValue,
) -> (DomainStatus, DomainPrecision, DomainValue) {
    if operation.is_none()
        && matches!(
            value,
            DomainValue::TopReason(ref reason) if reason == TopReason::UnresolvedCall.as_str()
        )
    {
        return top_value(TopReason::UnknownValue);
    }
    (status, precision, value)
}

fn reachability_fact_value(
    domain: &ReachabilityDomain,
) -> (DomainStatus, DomainPrecision, DomainValue) {
    match domain {
        ReachabilityDomain::Unreachable => label("unreachable", DomainPrecision::ExactLocal),
        ReachabilityDomain::Reachable => label("reachable", DomainPrecision::ExactLocal),
        ReachabilityDomain::Ambiguous => label("ambiguous", DomainPrecision::Conservative),
        ReachabilityDomain::Top(reason) => top_value(*reason),
    }
}

fn nilness_fact_value(domain: &NilnessDomain) -> (DomainStatus, DomainPrecision, DomainValue) {
    match domain {
        NilnessDomain::Bottom => top_value(TopReason::UnknownValue),
        NilnessDomain::Nil => label("nil", DomainPrecision::ExactLocal),
        NilnessDomain::NonNil => label("non_nil", DomainPrecision::ExactLocal),
        NilnessDomain::MaybeNil => label("maybe_nil", DomainPrecision::Conservative),
        NilnessDomain::Top(reason) => top_value(*reason),
    }
}

fn truthiness_fact_value(
    domain: &TruthinessDomain,
) -> (DomainStatus, DomainPrecision, DomainValue) {
    match domain {
        TruthinessDomain::Bottom => top_value(TopReason::UnknownValue),
        TruthinessDomain::Truthy => label("truthy", DomainPrecision::ExactLocal),
        TruthinessDomain::Falsy => label("falsy", DomainPrecision::ExactLocal),
        TruthinessDomain::Maybe => label("maybe", DomainPrecision::Conservative),
        TruthinessDomain::Top(reason) => top_value(*reason),
    }
}

fn constant_fact_value(domain: &ConstantDomain) -> (DomainStatus, DomainPrecision, DomainValue) {
    match domain {
        ConstantDomain::Bottom => top_value(TopReason::UnknownValue),
        ConstantDomain::Values(_) => digest_value(domain.stable_digest_parts()),
        ConstantDomain::Top(reason) => top_value(*reason),
    }
}

fn string_fact_value(domain: &StringDomain) -> (DomainStatus, DomainPrecision, DomainValue) {
    match domain {
        StringDomain::Bottom => top_value(TopReason::UnknownValue),
        StringDomain::Values(_) => digest_value(domain.stable_digest_parts()),
        StringDomain::Top(reason) => top_value(*reason),
    }
}

fn initializedness_fact_value(
    domain: &InitializednessDomain,
) -> (DomainStatus, DomainPrecision, DomainValue) {
    match domain {
        InitializednessDomain::Bottom => top_value(TopReason::UnknownValue),
        InitializednessDomain::Initialized => label("initialized", DomainPrecision::ExactLocal),
        InitializednessDomain::Uninitialized => label("uninitialized", DomainPrecision::ExactLocal),
        InitializednessDomain::MaybeUninitialized => {
            label("maybe_uninitialized", DomainPrecision::Conservative)
        }
        InitializednessDomain::Top(reason) => top_value(*reason),
    }
}

fn label(value: &str, precision: DomainPrecision) -> (DomainStatus, DomainPrecision, DomainValue) {
    let status = if precision == DomainPrecision::Unknown {
        DomainStatus::Unknown
    } else {
        DomainStatus::Present
    };
    (status, precision, DomainValue::Label(value.to_string()))
}

fn digest_value(parts: Vec<String>) -> (DomainStatus, DomainPrecision, DomainValue) {
    (
        DomainStatus::Present,
        DomainPrecision::ExactLocal,
        DomainValue::DigestParts(parts),
    )
}

fn top_value(reason: TopReason) -> (DomainStatus, DomainPrecision, DomainValue) {
    (
        status_for_top_reason(reason),
        precision_for_top_reason(reason),
        DomainValue::TopReason(reason.as_str().to_string()),
    )
}

fn status_for_top_reason(reason: TopReason) -> DomainStatus {
    match reason {
        TopReason::UnknownValue | TopReason::DynamicWrite | TopReason::UnresolvedCall => {
            DomainStatus::Unknown
        }
        TopReason::UnsupportedSemantic => DomainStatus::Unsupported,
        TopReason::SetupMissing => DomainStatus::SetupMissing,
        TopReason::BudgetExceeded => DomainStatus::BudgetExceeded,
        TopReason::Widened | TopReason::ConflictingFacts => DomainStatus::Top,
    }
}

fn precision_for_top_reason(reason: TopReason) -> DomainPrecision {
    match reason {
        TopReason::UnsupportedSemantic => DomainPrecision::Unsupported,
        TopReason::SetupMissing => DomainPrecision::SetupAware,
        TopReason::UnknownValue | TopReason::DynamicWrite | TopReason::UnresolvedCall => {
            DomainPrecision::Unknown
        }
        TopReason::BudgetExceeded | TopReason::Widened | TopReason::ConflictingFacts => {
            DomainPrecision::Conservative
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct DomainStore {
    output: DomainOutput,
    observations_by_body: BTreeMap<MirBodyId, Vec<usize>>,
    observations_by_block: BTreeMap<BasicBlockId, Vec<usize>>,
    observations_by_operation: BTreeMap<MirOpId, Vec<usize>>,
    observations_by_place: BTreeMap<PlaceId, Vec<usize>>,
    observations_by_slot: BTreeMap<DomainSlot, Vec<usize>>,
    observations_by_status: BTreeMap<DomainStatus, Vec<usize>>,
    observations_by_stable_key: BTreeMap<String, usize>,
    events_by_body: BTreeMap<MirBodyId, Vec<usize>>,
    events_by_status: BTreeMap<DomainStatus, Vec<usize>>,
    events_by_stable_key: BTreeMap<String, usize>,
}

impl DomainStore {
    pub(crate) fn from_output(output: DomainOutput) -> Self {
        let output = output.normalized();
        let mut store = Self {
            output,
            ..Self::default()
        };

        for (index, fact) in store.output.observations.iter().enumerate() {
            store
                .observations_by_body
                .entry(fact.body)
                .or_default()
                .push(index);
            if let Some(block) = fact.block {
                store
                    .observations_by_block
                    .entry(block)
                    .or_default()
                    .push(index);
            }
            if let Some(operation) = fact.operation {
                store
                    .observations_by_operation
                    .entry(operation)
                    .or_default()
                    .push(index);
            }
            if let Some(place) = fact.place {
                store
                    .observations_by_place
                    .entry(place)
                    .or_default()
                    .push(index);
            }
            store
                .observations_by_slot
                .entry(fact.slot)
                .or_default()
                .push(index);
            store
                .observations_by_status
                .entry(fact.status)
                .or_default()
                .push(index);
            store
                .observations_by_stable_key
                .insert(fact.stable_key.clone(), index);
        }

        for (index, fact) in store.output.events.iter().enumerate() {
            store
                .events_by_body
                .entry(fact.body)
                .or_default()
                .push(index);
            store
                .events_by_status
                .entry(fact.status)
                .or_default()
                .push(index);
            store
                .events_by_stable_key
                .insert(fact.stable_key.clone(), index);
        }

        store
    }

    pub(crate) fn observations(&self) -> &[DomainObservationFact] {
        &self.output.observations
    }

    pub(crate) fn events(&self) -> &[DomainEventFact] {
        &self.output.events
    }

    pub(crate) fn observations_by_body(&self, body: MirBodyId) -> Vec<&DomainObservationFact> {
        self.observation_refs(self.observations_by_body.get(&body))
    }

    pub(crate) fn observations_by_block(&self, block: BasicBlockId) -> Vec<&DomainObservationFact> {
        self.observation_refs(self.observations_by_block.get(&block))
    }

    pub(crate) fn observations_by_operation(
        &self,
        operation: MirOpId,
    ) -> Vec<&DomainObservationFact> {
        self.observation_refs(self.observations_by_operation.get(&operation))
    }

    pub(crate) fn observations_by_place(&self, place: PlaceId) -> Vec<&DomainObservationFact> {
        self.observation_refs(self.observations_by_place.get(&place))
    }

    pub(crate) fn observations_by_slot(&self, slot: DomainSlot) -> Vec<&DomainObservationFact> {
        self.observation_refs(self.observations_by_slot.get(&slot))
    }

    pub(crate) fn observations_by_status(
        &self,
        status: DomainStatus,
    ) -> Vec<&DomainObservationFact> {
        self.observation_refs(self.observations_by_status.get(&status))
    }

    pub(crate) fn observation_by_stable_key(
        &self,
        stable_key: &str,
    ) -> Option<&DomainObservationFact> {
        self.observations_by_stable_key
            .get(stable_key)
            .map(|&index| &self.output.observations[index])
    }

    pub(crate) fn events_by_status(&self, status: DomainStatus) -> Vec<&DomainEventFact> {
        self.event_refs(self.events_by_status.get(&status))
    }

    pub(crate) fn event_by_stable_key(&self, stable_key: &str) -> Option<&DomainEventFact> {
        self.events_by_stable_key
            .get(stable_key)
            .map(|&index| &self.output.events[index])
    }

    fn observation_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&DomainObservationFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.observations[index])
                .collect()
        })
    }

    fn event_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&DomainEventFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.events[index])
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::ids::BasicBlockId;
    use crate::analysis::domains::core::ConstantLiteral;
    use crate::analysis::domains::facts::{
        DomainLocation, DomainObservationFact, DomainPrecision, DomainSlot, DomainStatus,
        DomainValue,
    };
    use crate::analysis::ids::{DomainObservationId, MirBodyId, MirOpId, PlaceId};
    use crate::analysis_kernel::{FactFamily, FactRef};
    use crate::core::AnalysisDb;

    fn observation(id: u64, stable_key: &str, status: DomainStatus) -> DomainObservationFact {
        DomainObservationFact {
            id: DomainObservationId(id),
            body: MirBodyId(1),
            block: Some(BasicBlockId(2)),
            operation: Some(MirOpId(id)),
            place: Some(PlaceId(4)),
            slot: DomainSlot::Nilness,
            location: DomainLocation::AfterOperation,
            value: DomainValue::Label(format!("{status:?}")),
            status,
            precision: DomainPrecision::Conservative,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn abstract_domain_fact_storage_normalized_sorts_rows_without_dropping_unknown_statuses() {
        let output = DomainOutput {
            observations: vec![
                observation(2, "domain:z", DomainStatus::Unsupported),
                observation(1, "domain:a", DomainStatus::Unknown),
                observation(3, "domain:m", DomainStatus::BudgetExceeded),
            ],
            events: Vec::new(),
        }
        .normalized();

        assert_eq!(
            output
                .observations
                .iter()
                .map(|fact| (fact.stable_key.as_str(), fact.status))
                .collect::<Vec<_>>(),
            vec![
                ("domain:a", DomainStatus::Unknown),
                ("domain:m", DomainStatus::BudgetExceeded),
                ("domain:z", DomainStatus::Unsupported),
            ]
        );
    }

    #[test]
    fn abstract_domain_fact_metadata_replace_removes_stale_rows_and_refreshes_metadata() {
        let mut db = AnalysisDb::new();
        db.replace_abstract_domain_facts(DomainOutput {
            observations: vec![observation(1, "domain:first", DomainStatus::Present)],
            events: Vec::new(),
        });
        db.replace_abstract_domain_facts(DomainOutput {
            observations: vec![observation(2, "domain:second", DomainStatus::Top)],
            events: Vec::new(),
        });

        assert_eq!(db.abstract_domain_observations().len(), 1);
        assert_eq!(
            db.abstract_domain_observations()[0].stable_key,
            "domain:second"
        );
        assert!(
            db.fact_meta()
                .get(FactRef::new(FactFamily::DomainObservation, 0))
                .is_some()
        );
        assert!(
            db.fact_meta()
                .get(FactRef::new(FactFamily::DomainObservation, 1))
                .is_none()
        );
    }

    #[test]
    fn domain_observation_stable_key_uses_place_stable_key_not_dense_place_id() {
        let first = domain_output_for_place(PlaceId(7), "place:stable");
        let second = domain_output_for_place(PlaceId(99), "place:stable");
        let first_key = first
            .observations
            .iter()
            .find(|row| row.slot == DomainSlot::Constants)
            .expect("constant row")
            .stable_key
            .clone();
        let second_key = second
            .observations
            .iter()
            .find(|row| row.slot == DomainSlot::Constants)
            .expect("constant row")
            .stable_key
            .clone();

        assert_eq!(first_key, second_key);
    }

    fn domain_output_for_place(place: PlaceId, place_key: &str) -> DomainOutput {
        let mut results = DomainResults::new();
        let mut state = ProductState::entry();
        state.core.constants.insert(
            place,
            ConstantDomain::from_literal(ConstantLiteral::Bool(true)),
        );
        results.insert_function(
            MirBodyId(1),
            "body:stable".to_string(),
            SolverStatus::Solved,
            state,
        );
        DomainOutput::from_results_with_place_keys(
            &results,
            &BTreeMap::from([(place, place_key.to_string())]),
        )
    }
}
