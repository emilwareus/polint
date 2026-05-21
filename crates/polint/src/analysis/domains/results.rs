#![expect(
    dead_code,
    reason = "Phase 31 adds private result cursors before provider/debug plans consume every iterator."
)]

use std::collections::BTreeMap;

use super::lattice::TopReason;
use super::state::ProductState;
use crate::analysis::cfg::ids::BasicBlockId;
use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SolverStatus {
    Solved,
    Widened,
    BudgetExceeded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DomainResults {
    functions: BTreeMap<MirBodyId, FunctionResult>,
    block_states: BTreeMap<BasicBlockId, BlockState>,
    operation_states: BTreeMap<MirOpId, OperationState>,
    top_events: BTreeMap<String, TopEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FunctionResult {
    pub(crate) body: MirBodyId,
    pub(crate) body_stable_key: String,
    pub(crate) status: SolverStatus,
    pub(crate) entry_state: ProductState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BlockState {
    pub(crate) body: MirBodyId,
    pub(crate) block: BasicBlockId,
    pub(crate) stable_key: String,
    pub(crate) entry: ProductState,
    pub(crate) exit: ProductState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OperationState {
    pub(crate) body: MirBodyId,
    pub(crate) block: BasicBlockId,
    pub(crate) operation: MirOpId,
    pub(crate) stable_key: String,
    pub(crate) before: ProductState,
    pub(crate) after: ProductState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlaceObservation {
    pub(crate) body: MirBodyId,
    pub(crate) block: Option<BasicBlockId>,
    pub(crate) operation: Option<MirOpId>,
    pub(crate) place: PlaceId,
    pub(crate) stable_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TopEvent {
    pub(crate) body: MirBodyId,
    pub(crate) block: Option<BasicBlockId>,
    pub(crate) operation: Option<MirOpId>,
    pub(crate) reason: TopReason,
    pub(crate) stable_key: String,
}

impl DomainResults {
    pub(crate) fn new() -> Self {
        Self {
            functions: BTreeMap::new(),
            block_states: BTreeMap::new(),
            operation_states: BTreeMap::new(),
            top_events: BTreeMap::new(),
        }
    }

    pub(crate) fn insert_function(
        &mut self,
        body: MirBodyId,
        body_stable_key: String,
        status: SolverStatus,
        entry_state: ProductState,
    ) {
        self.functions.insert(
            body,
            FunctionResult {
                body,
                body_stable_key,
                status,
                entry_state,
            },
        );
    }

    pub(crate) fn insert_block_state(
        &mut self,
        body: MirBodyId,
        block: BasicBlockId,
        stable_key: String,
        entry: ProductState,
        exit: ProductState,
    ) {
        self.block_states.insert(
            block,
            BlockState {
                body,
                block,
                stable_key,
                entry,
                exit,
            },
        );
    }

    pub(crate) fn insert_operation_state(
        &mut self,
        body: MirBodyId,
        block: BasicBlockId,
        operation: MirOpId,
        stable_key: String,
        before: ProductState,
        after: ProductState,
    ) {
        self.operation_states.insert(
            operation,
            OperationState {
                body,
                block,
                operation,
                stable_key,
                before,
                after,
            },
        );
    }

    pub(crate) fn record_top_event(
        &mut self,
        body: MirBodyId,
        block: Option<BasicBlockId>,
        operation: Option<MirOpId>,
        reason: TopReason,
        stable_key: String,
    ) {
        self.top_events.insert(
            stable_key.clone(),
            TopEvent {
                body,
                block,
                operation,
                reason,
                stable_key,
            },
        );
    }

    pub(crate) fn entry_state(&self, body: MirBodyId) -> Option<&ProductState> {
        self.functions
            .get(&body)
            .map(|function| &function.entry_state)
    }

    pub(crate) fn block_entry(&self, block: BasicBlockId) -> Option<&ProductState> {
        self.block_states.get(&block).map(|state| &state.entry)
    }

    pub(crate) fn before_operation(&self, operation: MirOpId) -> Option<&ProductState> {
        self.operation_states
            .get(&operation)
            .map(|state| &state.before)
    }

    pub(crate) fn after_operation(&self, operation: MirOpId) -> Option<&ProductState> {
        self.operation_states
            .get(&operation)
            .map(|state| &state.after)
    }

    pub(crate) fn block_exit(&self, block: BasicBlockId) -> Option<&ProductState> {
        self.block_states.get(&block).map(|state| &state.exit)
    }

    pub(crate) fn statuses(&self) -> impl Iterator<Item = SolverStatus> + '_ {
        self.functions.values().map(|function| function.status)
    }

    pub(crate) fn top_events(&self) -> impl Iterator<Item = &TopEvent> {
        self.top_events.values()
    }

    pub(crate) fn blocks(&self) -> impl Iterator<Item = &BlockState> {
        let mut blocks = self.block_states.values().collect::<Vec<_>>();
        blocks.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        blocks.into_iter()
    }

    pub(crate) fn operations(&self) -> impl Iterator<Item = &OperationState> {
        let mut operations = self.operation_states.values().collect::<Vec<_>>();
        operations.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        operations.into_iter()
    }

    pub(crate) fn place_observations(&self) -> impl Iterator<Item = PlaceObservation> + '_ {
        let mut rows = Vec::new();
        for block in self.blocks() {
            push_place_observations(
                &mut rows,
                block.body,
                Some(block.block),
                None,
                &block.stable_key,
                &block.entry,
            );
            push_place_observations(
                &mut rows,
                block.body,
                Some(block.block),
                None,
                &block.stable_key,
                &block.exit,
            );
        }
        for operation in self.operations() {
            push_place_observations(
                &mut rows,
                operation.body,
                Some(operation.block),
                Some(operation.operation),
                &operation.stable_key,
                &operation.before,
            );
            push_place_observations(
                &mut rows,
                operation.body,
                Some(operation.block),
                Some(operation.operation),
                &operation.stable_key,
                &operation.after,
            );
        }
        rows.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        rows.into_iter()
    }

    pub(crate) fn has_top_reason(&self, reason: TopReason) -> bool {
        self.top_events.values().any(|event| event.reason == reason)
    }

    pub(crate) fn stable_digest_parts(&self) -> Vec<String> {
        let mut parts = Vec::new();
        for function in self.functions.values() {
            parts.push(format!(
                "function;stable_key={};status={:?}",
                function.body_stable_key, function.status
            ));
            parts.extend(
                function
                    .entry_state
                    .stable_digest_parts()
                    .into_iter()
                    .map(|part| {
                        format!(
                            "function;stable_key={};entry;{part}",
                            function.body_stable_key
                        )
                    }),
            );
        }
        for block in self.blocks() {
            parts.extend(
                block
                    .entry
                    .stable_digest_parts()
                    .into_iter()
                    .map(|part| format!("block;stable_key={};entry;{part}", block.stable_key)),
            );
            parts.extend(
                block
                    .exit
                    .stable_digest_parts()
                    .into_iter()
                    .map(|part| format!("block;stable_key={};exit;{part}", block.stable_key)),
            );
        }
        for operation in self.operations() {
            parts.extend(
                operation
                    .before
                    .stable_digest_parts()
                    .into_iter()
                    .map(|part| {
                        format!(
                            "operation;stable_key={};before;{part}",
                            operation.stable_key
                        )
                    }),
            );
            parts.extend(
                operation
                    .after
                    .stable_digest_parts()
                    .into_iter()
                    .map(|part| {
                        format!("operation;stable_key={};after;{part}", operation.stable_key)
                    }),
            );
        }
        for event in self.top_events.values() {
            parts.push(format!(
                "top;stable_key={};reason={}",
                event.stable_key,
                event.reason.as_str()
            ));
        }
        parts.sort();
        parts
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        body: MirBodyId,
        block: BasicBlockId,
        operation: MirOpId,
        place: PlaceId,
    ) -> Self {
        let mut state = ProductState::entry();
        state.mark_place_top(place, TopReason::UnknownValue);

        let mut results = Self::new();
        results.insert_function(
            body,
            "stable_key:test-body".to_string(),
            SolverStatus::Solved,
            state.clone(),
        );
        results.insert_block_state(
            body,
            block,
            "stable_key:test-block".to_string(),
            state.clone(),
            state.clone(),
        );
        results.insert_operation_state(
            body,
            block,
            operation,
            "stable_key:test-operation".to_string(),
            state.clone(),
            state,
        );
        results
    }
}

fn push_place_observations(
    rows: &mut Vec<PlaceObservation>,
    body: MirBodyId,
    block: Option<BasicBlockId>,
    operation: Option<MirOpId>,
    prefix: &str,
    state: &ProductState,
) {
    for place in state.observed_places() {
        rows.push(PlaceObservation {
            body,
            block,
            operation,
            place,
            stable_key: format!(
                "{prefix};block={:?};operation={:?};place={}",
                block, operation, place.0
            ),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::ids::BasicBlockId;
    use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId};

    #[test]
    fn result_cursor_exposes_entry_block_operation_and_exit_states() {
        let body = MirBodyId(1);
        let block = BasicBlockId(2);
        let operation = MirOpId(3);
        let place = PlaceId(4);
        let results = DomainResults::for_test(body, block, operation, place);

        assert!(results.entry_state(body).is_some());
        assert!(results.block_entry(block).is_some());
        assert!(results.before_operation(operation).is_some());
        assert!(results.after_operation(operation).is_some());
        assert!(results.block_exit(block).is_some());
        assert!(results.place_observations().any(|row| row.place == place));
    }

    #[test]
    fn stable_key_result_iteration_is_deterministic() {
        let results =
            DomainResults::for_test(MirBodyId(1), BasicBlockId(2), MirOpId(3), PlaceId(4));

        assert_eq!(results.stable_digest_parts(), results.stable_digest_parts());
    }
}
