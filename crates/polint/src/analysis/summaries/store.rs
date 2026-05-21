use std::collections::BTreeMap;

use super::facts::{SummaryDomainKind, SummaryEventFact, SummaryFact};
use crate::analysis::error::AnalysisError;
use crate::analysis::ids::{SummaryEventId, SummaryId};
use crate::core::FunctionId;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SummaryOutput {
    pub(crate) summaries: Vec<SummaryFact>,
    pub(crate) events: Vec<SummaryEventFact>,
}

impl SummaryOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn normalized(mut self) -> Self {
        self.summaries.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        self.events.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        for (index, fact) in self.summaries.iter_mut().enumerate() {
            fact.id = SummaryId(index as u64);
        }
        for (index, fact) in self.events.iter_mut().enumerate() {
            fact.id = SummaryEventId(index as u64);
        }
        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SummaryStore {
    output: SummaryOutput,
    summaries_by_callable: BTreeMap<String, Vec<usize>>,
    summaries_by_domain: BTreeMap<SummaryDomainKind, Vec<usize>>,
    summaries_by_function: BTreeMap<FunctionId, Vec<usize>>,
}

impl SummaryStore {
    pub(crate) fn from_output(output: SummaryOutput) -> Result<Self, AnalysisError> {
        let output = output.normalized();

        let mut store = Self {
            output,
            ..Self::default()
        };

        for (index, fact) in store.output.summaries.iter().enumerate() {
            store
                .summaries_by_callable
                .entry(fact.callable_stable_key.clone())
                .or_default()
                .push(index);
            store
                .summaries_by_domain
                .entry(fact.domain)
                .or_default()
                .push(index);
            store
                .summaries_by_function
                .entry(fact.function)
                .or_default()
                .push(index);
        }

        Ok(store)
    }

    pub(crate) fn summaries_by_callable(&self, callable_key: &str) -> Vec<&SummaryFact> {
        self.summary_refs(self.summaries_by_callable.get(callable_key))
    }

    pub(crate) fn summaries_by_domain(&self, domain: SummaryDomainKind) -> Vec<&SummaryFact> {
        self.summary_refs(self.summaries_by_domain.get(&domain))
    }

    pub(crate) fn summaries_by_function(&self, function: FunctionId) -> Vec<&SummaryFact> {
        self.summary_refs(self.summaries_by_function.get(&function))
    }

    pub(crate) fn all_summaries(&self) -> &[SummaryFact] {
        &self.output.summaries
    }

    pub(crate) fn all_events(&self) -> &[SummaryEventFact] {
        &self.output.events
    }

    pub(crate) fn summary_count(&self) -> usize {
        self.output.summaries.len()
    }

    pub(crate) fn event_count(&self) -> usize {
        self.output.events.len()
    }

    fn summary_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&SummaryFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.summaries[index])
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::summaries::facts::{
        SummaryDomainKind, SummaryPrecision, SummaryProvenance, SummaryStatus,
    };

    fn summary_fact(
        id: u64,
        callable_key: &str,
        function: u64,
        domain: SummaryDomainKind,
        stable_key: &str,
    ) -> SummaryFact {
        SummaryFact {
            id: SummaryId(id),
            callable_stable_key: callable_key.to_string(),
            function: FunctionId(function),
            domain,
            status: SummaryStatus::Present,
            precision: SummaryPrecision::Local,
            provenance: SummaryProvenance::NativeLocal,
            payload_digest: format!("digest:{stable_key}"),
            stable_key: stable_key.to_string(),
        }
    }

    fn event_fact(id: u64, callable_key: &str, function: u64, stable_key: &str) -> SummaryEventFact {
        SummaryEventFact {
            id: SummaryEventId(id),
            callable_stable_key: callable_key.to_string(),
            function: FunctionId(function),
            domain: SummaryDomainKind::CallEffects,
            event_kind: "unresolved_callee".to_string(),
            reason: "dynamic".to_string(),
            status: SummaryStatus::Unknown,
            precision: SummaryPrecision::UnknownTop,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn normalized_sorts_summaries_by_stable_key_and_reassigns_ids() {
        let output = SummaryOutput {
            summaries: vec![
                summary_fact(
                    99,
                    "func::b",
                    2,
                    SummaryDomainKind::ControlEffects,
                    "summary:z",
                ),
                summary_fact(
                    50,
                    "func::a",
                    1,
                    SummaryDomainKind::MemoryEffects,
                    "summary:a",
                ),
                summary_fact(
                    10,
                    "func::a",
                    1,
                    SummaryDomainKind::CallEffects,
                    "summary:m",
                ),
            ],
            events: vec![
                event_fact(5, "func::b", 2, "event:z"),
                event_fact(1, "func::a", 1, "event:a"),
            ],
        }
        .normalized();

        assert_eq!(
            output
                .summaries
                .iter()
                .map(|f| (f.id.0, f.stable_key.as_str()))
                .collect::<Vec<_>>(),
            vec![(0, "summary:a"), (1, "summary:m"), (2, "summary:z")]
        );
        assert_eq!(
            output
                .events
                .iter()
                .map(|f| (f.id.0, f.stable_key.as_str()))
                .collect::<Vec<_>>(),
            vec![(0, "event:a"), (1, "event:z")]
        );
    }

    #[test]
    fn from_output_builds_deterministic_indexes() {
        let store = SummaryStore::from_output(SummaryOutput {
            summaries: vec![
                summary_fact(
                    2,
                    "func::b",
                    2,
                    SummaryDomainKind::ControlEffects,
                    "summary:b-control",
                ),
                summary_fact(
                    1,
                    "func::a",
                    1,
                    SummaryDomainKind::ControlEffects,
                    "summary:a-control",
                ),
                summary_fact(
                    3,
                    "func::a",
                    1,
                    SummaryDomainKind::MemoryEffects,
                    "summary:a-memory",
                ),
            ],
            events: vec![event_fact(1, "func::a", 1, "event:a")],
        })
        .expect("valid output");

        assert_eq!(store.summary_count(), 3);
        assert_eq!(store.event_count(), 1);

        // IDs are reassigned sequentially
        assert_eq!(store.all_summaries()[0].id.0, 0);
        assert_eq!(store.all_summaries()[1].id.0, 1);
        assert_eq!(store.all_summaries()[2].id.0, 2);

        // Sorted by stable key
        assert_eq!(store.all_summaries()[0].stable_key, "summary:a-control");
        assert_eq!(store.all_summaries()[1].stable_key, "summary:a-memory");
        assert_eq!(store.all_summaries()[2].stable_key, "summary:b-control");
    }

    #[test]
    fn empty_output_builds_empty_store() {
        let store = SummaryStore::from_output(SummaryOutput::empty()).expect("empty is valid");

        assert!(store.all_summaries().is_empty());
        assert!(store.all_events().is_empty());
        assert_eq!(store.summary_count(), 0);
        assert_eq!(store.event_count(), 0);
    }

    #[test]
    fn summaries_indexed_by_callable_and_domain() {
        let store = SummaryStore::from_output(SummaryOutput {
            summaries: vec![
                summary_fact(
                    1,
                    "func::a",
                    1,
                    SummaryDomainKind::ControlEffects,
                    "summary:a-control",
                ),
                summary_fact(
                    2,
                    "func::a",
                    1,
                    SummaryDomainKind::MemoryEffects,
                    "summary:a-memory",
                ),
                summary_fact(
                    3,
                    "func::b",
                    2,
                    SummaryDomainKind::ControlEffects,
                    "summary:b-control",
                ),
            ],
            events: Vec::new(),
        })
        .expect("valid output");

        // by callable
        let a_summaries = store.summaries_by_callable("func::a");
        assert_eq!(a_summaries.len(), 2);
        assert!(a_summaries.iter().all(|f| f.callable_stable_key == "func::a"));

        let b_summaries = store.summaries_by_callable("func::b");
        assert_eq!(b_summaries.len(), 1);
        assert_eq!(b_summaries[0].callable_stable_key, "func::b");

        let none = store.summaries_by_callable("func::nonexistent");
        assert!(none.is_empty());

        // by domain
        let control = store.summaries_by_domain(SummaryDomainKind::ControlEffects);
        assert_eq!(control.len(), 2);

        let memory = store.summaries_by_domain(SummaryDomainKind::MemoryEffects);
        assert_eq!(memory.len(), 1);

        let tito = store.summaries_by_domain(SummaryDomainKind::DataFlowTito);
        assert!(tito.is_empty());

        // by function
        let func1 = store.summaries_by_function(FunctionId(1));
        assert_eq!(func1.len(), 2);

        let func2 = store.summaries_by_function(FunctionId(2));
        assert_eq!(func2.len(), 1);

        let func99 = store.summaries_by_function(FunctionId(99));
        assert!(func99.is_empty());
    }
}
