#![expect(
    dead_code,
    reason = "Phase 31 stores domain rows before provider/debug/eval plans consume every index."
)]

use std::collections::BTreeMap;

use super::facts::{DomainEventFact, DomainObservationFact, DomainSlot, DomainStatus};
use crate::analysis::cfg::ids::BasicBlockId;
use crate::analysis::ids::{DomainEventId, DomainObservationId, MirBodyId, MirOpId, PlaceId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DomainOutput {
    pub(crate) observations: Vec<DomainObservationFact>,
    pub(crate) events: Vec<DomainEventFact>,
}

impl DomainOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
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
}
