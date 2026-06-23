use std::collections::{BTreeMap, BTreeSet};

use super::lattice::{Changed, TopReason};
use super::results::DomainResults;
pub(crate) use super::results::SolverStatus;
use super::state::ProductState;
use super::transfer::{BranchSense, EdgeTransfer, OperationTransfer, TransferCx};
use crate::analysis::cfg::facts::{
    BasicBlockFact, BasicBlockKind, CfgEdgeFact, CfgEdgeKind, CfgNodeFact, CfgView,
};
use crate::analysis::cfg::ids::{BasicBlockId, CfgFunctionId};
use crate::analysis::ids::{MirBodyId, MirOpId};
use crate::analysis::mir::body::MirBody;
use crate::analysis::mir::op::{MirOperation, MirOperationKind};
use crate::core::AnalysisDb;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SolverInput<'a> {
    db: &'a AnalysisDb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SolverBudget {
    pub(crate) max_iterations: u32,
    pub(crate) widening_fuel: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SolverPolicy {
    pub(crate) budget: SolverBudget,
    pub(crate) reduction_rounds: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SolverResult {
    results: DomainResults,
}

#[derive(Clone, Debug)]
pub(crate) struct LocalDomainSolver {
    policy: SolverPolicy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SolverOutputMode {
    Full,
    SummaryInputs,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct QueueKey {
    reverse_postorder: u32,
    stable_key: String,
    block: BasicBlockId,
}

impl<'a> From<&'a AnalysisDb> for SolverInput<'a> {
    fn from(db: &'a AnalysisDb) -> Self {
        Self { db }
    }
}

impl SolverPolicy {
    pub(crate) fn deterministic() -> Self {
        Self {
            budget: SolverBudget {
                max_iterations: 10_000,
                widening_fuel: 8,
            },
            reduction_rounds: 4,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test(budget: SolverBudget) -> Self {
        Self {
            budget,
            reduction_rounds: 4,
        }
    }
}

impl LocalDomainSolver {
    pub(crate) fn new(policy: SolverPolicy) -> Self {
        Self { policy }
    }

    pub(crate) fn solve(&self, input: SolverInput<'_>) -> SolverResult {
        self.solve_with_output_mode(input, SolverOutputMode::Full)
    }

    pub(crate) fn solve_summary_inputs(&self, input: SolverInput<'_>) -> SolverResult {
        self.solve_with_output_mode(input, SolverOutputMode::SummaryInputs)
    }

    fn solve_with_output_mode(
        &self,
        input: SolverInput<'_>,
        output_mode: SolverOutputMode,
    ) -> SolverResult {
        let mut results = DomainResults::new();
        let facts = SolverFactIndex::new(input.db);
        let transfer_cx = TransferCx::from_db(input.db);

        for function in &facts.functions {
            let Some(body) = facts.body_by_id.get(&function.body).copied() else {
                continue;
            };
            self.solve_function(
                body,
                function.id,
                &facts,
                &transfer_cx,
                output_mode,
                &mut results,
            );
        }

        SolverResult { results }
    }

    fn solve_function(
        &self,
        body: &MirBody,
        function: CfgFunctionId,
        facts: &SolverFactIndex<'_>,
        transfer_cx: &TransferCx<'_>,
        output_mode: SolverOutputMode,
        results: &mut DomainResults,
    ) {
        let blocks = facts.blocks(function);
        let nodes = facts.nodes(function);
        let edges = facts.edges(function);
        let operations_by_block = operations_by_block(nodes, &facts.operations_by_id);
        let outgoing_edges = edges_by_source_block(edges);
        let block_keys = blocks
            .iter()
            .map(|block| {
                (
                    block.id,
                    QueueKey {
                        reverse_postorder: block.reverse_postorder,
                        stable_key: block.stable_key.clone(),
                        block: block.id,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let mut entry_states = blocks
            .iter()
            .map(|block| (block.id, ProductState::bottom()))
            .collect::<BTreeMap<_, _>>();
        let mut exit_states = entry_states.clone();
        let mut operation_states = if output_mode.records_operation_states() {
            Some(BTreeMap::<
                MirOpId,
                (BasicBlockId, String, ProductState, ProductState),
            >::new())
        } else {
            None
        };
        let mut status = SolverStatus::Solved;
        let mut widening_fuel = self.policy.budget.widening_fuel;

        let Some(entry_block) = entry_block(blocks) else {
            results.insert_function(
                body.id,
                body.stable_key.clone(),
                status,
                ProductState::bottom(),
            );
            return;
        };

        entry_states.insert(entry_block, ProductState::entry());
        let mut queue = BTreeSet::from([block_keys[&entry_block].clone()]);
        let mut iterations = 0_u32;

        while let Some(next) = queue.iter().next().cloned() {
            queue.remove(&next);
            if iterations >= self.policy.budget.max_iterations {
                status = SolverStatus::BudgetExceeded;
                mark_budget_exceeded(
                    results,
                    body.id,
                    next.block,
                    &mut entry_states,
                    &mut exit_states,
                );
                break;
            }
            iterations += 1;

            let mut state = entry_states
                .get(&next.block)
                .cloned()
                .unwrap_or_else(ProductState::bottom);
            for operation in operations_by_block.get(&next.block).into_iter().flatten() {
                if let Some(operation_states) = &mut operation_states {
                    let before = state.clone();
                    OperationTransfer::apply(transfer_cx, operation, &mut state);
                    state.reduce_value_only(self.policy.reduction_rounds);
                    let after = state.clone();
                    operation_states.insert(
                        operation.id,
                        (next.block, operation.stable_key.clone(), before, after),
                    );
                } else {
                    OperationTransfer::apply(transfer_cx, operation, &mut state);
                    state.reduce_value_only(self.policy.reduction_rounds);
                }
            }
            state.reduce_value_only(self.policy.reduction_rounds);
            exit_states.insert(next.block, state.clone());

            for edge in outgoing_edges.get(&next.block).into_iter().flatten() {
                let mut candidate = state.clone();
                if let Some((predicate_place, sense)) =
                    branch_assumption(edge.kind, operations_by_block.get(&next.block))
                {
                    EdgeTransfer::apply_branch_assumption(
                        transfer_cx,
                        predicate_place,
                        sense,
                        &mut candidate,
                    );
                }
                if edge.kind == CfgEdgeKind::LoopBack {
                    if widening_fuel == 0 {
                        status = SolverStatus::Widened;
                        candidate.mark_reachability_top(TopReason::Widened);
                        results.record_top_event(
                            body.id,
                            Some(edge.to_block),
                            None,
                            TopReason::Widened,
                            format!("{};reason=widened", edge.stable_key),
                        );
                    } else {
                        widening_fuel -= 1;
                    }
                }

                let changed = entry_states
                    .entry(edge.to_block)
                    .or_insert_with(ProductState::bottom)
                    .join_into(&candidate);
                if changed == Changed::Yes
                    && let Some(queue_key) = block_keys.get(&edge.to_block)
                {
                    queue.insert(queue_key.clone());
                }
            }
        }

        let entry_state = entry_states
            .get(&entry_block)
            .cloned()
            .unwrap_or_else(ProductState::bottom);
        results.insert_function(body.id, body.stable_key.clone(), status, entry_state);

        for block in blocks {
            results.insert_block_state(
                body.id,
                block.id,
                block.stable_key.clone(),
                entry_states
                    .get(&block.id)
                    .cloned()
                    .unwrap_or_else(ProductState::bottom),
                exit_states
                    .get(&block.id)
                    .cloned()
                    .unwrap_or_else(ProductState::bottom),
            );
        }

        if let Some(operation_states) = operation_states {
            for (operation, (block, stable_key, before, after)) in operation_states {
                results
                    .insert_operation_state(body.id, block, operation, stable_key, before, after);
            }
        }
    }
}

impl SolverOutputMode {
    fn records_operation_states(self) -> bool {
        matches!(self, Self::Full)
    }
}

struct SolverFactIndex<'a> {
    body_by_id: BTreeMap<MirBodyId, &'a MirBody>,
    operations_by_id: BTreeMap<MirOpId, &'a MirOperation>,
    functions: Vec<&'a crate::analysis::cfg::facts::CfgFunctionFact>,
    blocks_by_function: BTreeMap<CfgFunctionId, Vec<&'a BasicBlockFact>>,
    nodes_by_function: BTreeMap<CfgFunctionId, Vec<&'a CfgNodeFact>>,
    edges_by_function: BTreeMap<CfgFunctionId, Vec<&'a CfgEdgeFact>>,
}

impl<'a> SolverFactIndex<'a> {
    fn new(db: &'a AnalysisDb) -> Self {
        let body_by_id = db
            .mir_bodies()
            .iter()
            .map(|body| (body.id, body))
            .collect::<BTreeMap<_, _>>();
        let operations_by_id = db
            .mir_operations()
            .iter()
            .map(|operation| (operation.id, operation))
            .collect::<BTreeMap<_, _>>();

        let mut functions = db.cfg_functions().iter().collect::<Vec<_>>();
        functions.sort_by(|left, right| {
            let left_body = body_by_id
                .get(&left.body)
                .map_or("", |body| body.stable_key.as_str());
            let right_body = body_by_id
                .get(&right.body)
                .map_or("", |body| body.stable_key.as_str());
            (left_body, left.stable_key.as_str()).cmp(&(right_body, right.stable_key.as_str()))
        });

        let mut blocks_by_function = BTreeMap::<CfgFunctionId, Vec<&BasicBlockFact>>::new();
        for block in db.cfg_blocks() {
            blocks_by_function
                .entry(block.cfg_function)
                .or_default()
                .push(block);
        }
        for blocks in blocks_by_function.values_mut() {
            blocks.sort_by(|left, right| {
                (left.reverse_postorder, left.stable_key.as_str(), left.id).cmp(&(
                    right.reverse_postorder,
                    right.stable_key.as_str(),
                    right.id,
                ))
            });
        }

        let mut nodes_by_function = BTreeMap::<CfgFunctionId, Vec<&CfgNodeFact>>::new();
        for node in db.cfg_nodes() {
            nodes_by_function
                .entry(node.cfg_function)
                .or_default()
                .push(node);
        }
        for nodes in nodes_by_function.values_mut() {
            nodes.sort_by(|left, right| {
                (left.block, left.operation_ordinal, left.stable_key.as_str()).cmp(&(
                    right.block,
                    right.operation_ordinal,
                    right.stable_key.as_str(),
                ))
            });
        }

        let mut edges_by_function = BTreeMap::<CfgFunctionId, Vec<&CfgEdgeFact>>::new();
        for edge in db.cfg_edges() {
            if edge.view == CfgView::NormalControl {
                edges_by_function
                    .entry(edge.cfg_function)
                    .or_default()
                    .push(edge);
            }
        }
        for edges in edges_by_function.values_mut() {
            edges.sort_by(|left, right| {
                (
                    left.from_block,
                    left.to_block,
                    left.kind,
                    left.stable_key.as_str(),
                )
                    .cmp(&(
                        right.from_block,
                        right.to_block,
                        right.kind,
                        right.stable_key.as_str(),
                    ))
            });
        }

        Self {
            body_by_id,
            operations_by_id,
            functions,
            blocks_by_function,
            nodes_by_function,
            edges_by_function,
        }
    }

    fn blocks(&self, function: CfgFunctionId) -> &[&'a BasicBlockFact] {
        self.blocks_by_function
            .get(&function)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn nodes(&self, function: CfgFunctionId) -> &[&'a CfgNodeFact] {
        self.nodes_by_function
            .get(&function)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn edges(&self, function: CfgFunctionId) -> &[&'a CfgEdgeFact] {
        self.edges_by_function
            .get(&function)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

impl SolverResult {
    pub(crate) fn results(&self) -> &DomainResults {
        &self.results
    }

    pub(crate) fn statuses(&self) -> impl Iterator<Item = SolverStatus> + '_ {
        self.results.statuses()
    }

    pub(crate) fn has_top_reason(&self, reason: TopReason) -> bool {
        self.results.has_top_reason(reason)
    }

    pub(crate) fn stable_digest_parts(&self) -> Vec<String> {
        self.results.stable_digest_parts()
    }
}

fn operations_by_block<'a>(
    nodes: &[&CfgNodeFact],
    operations_by_id: &BTreeMap<MirOpId, &'a MirOperation>,
) -> BTreeMap<BasicBlockId, Vec<&'a MirOperation>> {
    let mut operations = BTreeMap::<BasicBlockId, Vec<&MirOperation>>::new();
    for node in nodes {
        let Some(operation_id) = node.operation else {
            continue;
        };
        let Some(operation) = operations_by_id.get(&operation_id).copied() else {
            continue;
        };
        operations.entry(node.block).or_default().push(operation);
    }
    for block_operations in operations.values_mut() {
        block_operations.sort_by(|left, right| {
            (left.ordinal, left.stable_key.as_str(), left.id).cmp(&(
                right.ordinal,
                right.stable_key.as_str(),
                right.id,
            ))
        });
    }
    operations
}

fn edges_by_source_block<'a>(
    edges: &[&'a CfgEdgeFact],
) -> BTreeMap<BasicBlockId, Vec<&'a CfgEdgeFact>> {
    let mut by_source = BTreeMap::<BasicBlockId, Vec<&CfgEdgeFact>>::new();
    for edge in edges {
        by_source.entry(edge.from_block).or_default().push(*edge);
    }
    by_source
}

fn entry_block(blocks: &[&BasicBlockFact]) -> Option<BasicBlockId> {
    blocks
        .iter()
        .find(|block| block.kind == BasicBlockKind::Entry)
        .or_else(|| blocks.first())
        .map(|block| block.id)
}

fn branch_assumption(
    edge: CfgEdgeKind,
    operations: Option<&Vec<&MirOperation>>,
) -> Option<(Option<crate::analysis::ids::PlaceId>, BranchSense)> {
    let sense = match edge {
        CfgEdgeKind::True => BranchSense::True,
        CfgEdgeKind::False => BranchSense::False,
        _ => return None,
    };
    operations.and_then(|operations| {
        operations.iter().rev().find_map(|operation| {
            if let MirOperationKind::Branch {
                predicate_place, ..
            } = operation.kind
            {
                Some((predicate_place, sense))
            } else {
                None
            }
        })
    })
}

fn mark_budget_exceeded(
    results: &mut DomainResults,
    body: MirBodyId,
    block: BasicBlockId,
    entry_states: &mut BTreeMap<BasicBlockId, ProductState>,
    exit_states: &mut BTreeMap<BasicBlockId, ProductState>,
) {
    for state in entry_states.values_mut().chain(exit_states.values_mut()) {
        state.mark_reachability_top(TopReason::BudgetExceeded);
    }
    results.record_top_event(
        body,
        Some(block),
        None,
        TopReason::BudgetExceeded,
        format!("body={};block={};reason=budget_exceeded", body.0, block.0),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::facts::{
        BasicBlockFact, BasicBlockKind, CfgEdgeFact, CfgEdgeKind, CfgFunctionFact, CfgNodeFact,
        CfgNodeKind, CfgPrecision, CfgStatus, CfgView,
    };
    use crate::analysis::cfg::ids::{BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId};
    use crate::analysis::cfg::store::CfgOutput;
    use crate::analysis::domains::core::{ConstantDomain, ConstantLiteral, ReachabilityDomain};
    use crate::analysis::domains::lattice::TopReason;
    use crate::analysis::domains::store::{DomainMaterialization, DomainOutput};
    use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{AssignMode, MirOperation, MirOperationKind, MirValue};
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::core::{FileId, FunctionId, Language, Span};

    #[test]
    fn deterministic_shuffled_rows_produce_byte_identical_result_digests() {
        let first = test_fixture::solver_input(false);
        let second = test_fixture::solver_input(true);
        let solver = LocalDomainSolver::new(SolverPolicy::for_test(SolverBudget {
            max_iterations: 16,
            widening_fuel: 2,
        }));

        let first_digest = solver.solve(first).stable_digest_parts();
        let second_digest = solver.solve(second).stable_digest_parts();

        assert_eq!(first_digest, second_digest);
    }

    #[test]
    fn loop_back_edges_consume_widening_fuel_and_record_widening_top() {
        let input = test_fixture::looping_solver_input();
        let solver = LocalDomainSolver::new(SolverPolicy::for_test(SolverBudget {
            max_iterations: 16,
            widening_fuel: 0,
        }));

        let result = solver.solve(input);

        assert!(result.has_top_reason(TopReason::Widened));
    }

    #[test]
    fn budget_exhaustion_marks_function_result_budget_top() {
        let input = test_fixture::looping_solver_input();
        let solver = LocalDomainSolver::new(SolverPolicy::for_test(SolverBudget {
            max_iterations: 1,
            widening_fuel: 4,
        }));

        let result = solver.solve(input);

        assert_eq!(result.statuses().next(), Some(SolverStatus::BudgetExceeded));
        assert!(result.has_top_reason(TopReason::BudgetExceeded));
    }

    #[test]
    fn cursor_after_operation_exposes_transfer_materialized_state() {
        let input = test_fixture::solver_input(false);
        let solver = LocalDomainSolver::new(SolverPolicy::for_test(SolverBudget {
            max_iterations: 16,
            widening_fuel: 2,
        }));

        let result = solver.solve(input);
        let before = result
            .results()
            .before_operation(MirOpId(0))
            .expect("before operation state");
        let after = result
            .results()
            .after_operation(MirOpId(0))
            .expect("after operation state");

        assert!(!before.core.constants.contains_key(&PlaceId(0)));
        assert_eq!(
            after.core.constants[&PlaceId(0)],
            ConstantDomain::from_literal(ConstantLiteral::String("ready".to_string()))
        );
    }

    #[test]
    fn unreachable_blocks_expose_unreachable_without_value_facts() {
        let input = test_fixture::unreachable_solver_input();
        let solver = LocalDomainSolver::new(SolverPolicy::for_test(SolverBudget {
            max_iterations: 16,
            widening_fuel: 2,
        }));

        let result = solver.solve(input);
        let unreachable_entry = result
            .results()
            .block_entry(BasicBlockId(4))
            .expect("unreachable block state");

        assert_eq!(
            unreachable_entry.core.reachability,
            ReachabilityDomain::Unreachable
        );
        assert!(unreachable_entry.observed_places().is_empty());
    }

    #[test]
    fn solving_same_function_twice_returns_identical_stable_result_rows() {
        let input = test_fixture::solver_input(false);
        let solver = LocalDomainSolver::new(SolverPolicy::for_test(SolverBudget {
            max_iterations: 16,
            widening_fuel: 2,
        }));

        let first = solver.solve(input).stable_digest_parts();
        let second = solver.solve(input).stable_digest_parts();

        assert_eq!(first, second);
    }

    #[test]
    fn summary_input_solver_skips_operation_states_without_changing_output_rows() {
        let input = test_fixture::solver_input(false);
        let solver = LocalDomainSolver::new(SolverPolicy::for_test(SolverBudget {
            max_iterations: 16,
            widening_fuel: 2,
        }));

        let full = solver.solve(input);
        let summary = solver.solve_summary_inputs(input);
        let full_output = DomainOutput::from_results_with_materialization(
            full.results(),
            None,
            DomainMaterialization::SummaryInputs,
        );
        let summary_output = DomainOutput::from_results_with_materialization(
            summary.results(),
            None,
            DomainMaterialization::SummaryInputs,
        );

        assert_eq!(full_output, summary_output);
        assert!(summary.results().before_operation(MirOpId(0)).is_none());
    }

    mod test_fixture {
        use super::*;

        pub(super) fn solver_input(shuffled: bool) -> SolverInput<'static> {
            let mut db = AnalysisDb::new();
            db.replace_semantic_mir(mir_output(shuffled))
                .expect("semantic MIR should store");
            db.replace_cfg_facts(cfg_output(shuffled, false))
                .expect("CFG should store");
            let db = Box::leak(Box::new(db));
            (&*db).into()
        }

        pub(super) fn looping_solver_input() -> SolverInput<'static> {
            let mut db = AnalysisDb::new();
            db.replace_semantic_mir(mir_output(false))
                .expect("semantic MIR should store");
            db.replace_cfg_facts(cfg_output(false, true))
                .expect("CFG should store");
            let db = Box::leak(Box::new(db));
            (&*db).into()
        }

        pub(super) fn unreachable_solver_input() -> SolverInput<'static> {
            let mut db = AnalysisDb::new();
            db.replace_semantic_mir(mir_output(false))
                .expect("semantic MIR should store");
            let mut cfg = cfg_output(false, false);
            cfg.blocks.push(BasicBlockFact {
                id: BasicBlockId(4),
                cfg_function: CfgFunctionId(1),
                kind: BasicBlockKind::Unreachable,
                first_node: Some(CfgNodeId(4)),
                last_node: Some(CfgNodeId(4)),
                reachable: false,
                reverse_postorder: 3,
                stable_key: "block:unreachable".to_string(),
                status: CfgStatus::Resolved,
                precision: CfgPrecision::ExactLowered,
            });
            cfg.nodes.push(node(4, 4, None, 3, "node:unreachable"));
            db.replace_cfg_facts(cfg).expect("CFG should store");
            let db = Box::leak(Box::new(db));
            (&*db).into()
        }

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

        fn mir_output(shuffled: bool) -> MirOutput {
            let body = MirBody {
                id: MirBodyId(0),
                language: Language::Go,
                file: FileId(1),
                function: FunctionId(1),
                package: None,
                module: None,
                owner_stable_key: "owner:test".to_string(),
                span: span(),
                stable_key: "body:test".to_string(),
                status: MirStatus::Resolved,
            };
            let place = PlaceFact {
                id: PlaceId(0),
                language: Language::Go,
                file: Some(FileId(1)),
                function: Some(FunctionId(1)),
                root: PlaceRoot::Local {
                    function: FunctionId(1),
                    name: "value".to_string(),
                },
                projections: Vec::new(),
                stable_key: "place:value".to_string(),
                status: PlaceStatus::Resolved,
            };
            let op = MirOperation {
                id: MirOpId(0),
                body: MirBodyId(0),
                ordinal: 1,
                span: span(),
                kind: MirOperationKind::Assign {
                    place: PlaceId(0),
                    value: MirValue::Literal {
                        value: "\"ready\"".to_string(),
                    },
                    mode: AssignMode::Overwrite,
                },
                stable_key: "op:assign".to_string(),
                status: MirStatus::Resolved,
            };
            let mut operations = vec![op];
            if shuffled {
                operations.reverse();
            }
            MirOutput {
                bodies: vec![body],
                places: vec![place],
                operations,
                unsupported: Vec::new(),
            }
        }

        fn cfg_output(shuffled: bool, loop_back: bool) -> CfgOutput {
            let function = CfgFunctionFact {
                id: CfgFunctionId(1),
                body: MirBodyId(0),
                function: FunctionId(1),
                language: Language::Go,
                file: FileId(1),
                span: span(),
                entry_node: CfgNodeId(1),
                normal_exit_node: CfgNodeId(3),
                exceptional_exit_node: None,
                stable_key: "cfg:function:test".to_string(),
                status: CfgStatus::Resolved,
                precision: CfgPrecision::ExactLowered,
            };
            let entry = block(1, BasicBlockKind::Entry, 0, "block:entry");
            let body = block(2, BasicBlockKind::LoopHeader, 1, "block:body");
            let exit = block(3, BasicBlockKind::ExitNormal, 2, "block:exit");
            let nodes = vec![
                node(1, 1, None, 0, "node:entry"),
                node(2, 2, Some(MirOpId(0)), 1, "node:op"),
                node(3, 3, None, u32::MAX - 1, "node:exit"),
            ];
            let mut edges = vec![
                edge(1, 1, 2, CfgEdgeKind::Normal, "edge:entry-body"),
                edge(2, 2, 3, CfgEdgeKind::Return, "edge:body-exit"),
            ];
            if loop_back {
                edges.push(edge(3, 2, 2, CfgEdgeKind::LoopBack, "edge:loop-back"));
            }
            if shuffled {
                edges.reverse();
            }
            CfgOutput {
                functions: vec![function],
                nodes,
                blocks: vec![exit, body, entry],
                edges,
                ..CfgOutput::empty()
            }
        }

        fn block(
            id: u64,
            kind: BasicBlockKind,
            reverse_postorder: u32,
            stable_key: &str,
        ) -> BasicBlockFact {
            BasicBlockFact {
                id: BasicBlockId(id),
                cfg_function: CfgFunctionId(1),
                kind,
                first_node: Some(CfgNodeId(id)),
                last_node: Some(CfgNodeId(id)),
                reachable: true,
                reverse_postorder,
                stable_key: stable_key.to_string(),
                status: CfgStatus::Resolved,
                precision: CfgPrecision::ExactLowered,
            }
        }

        fn node(
            id: u64,
            block: u64,
            operation: Option<MirOpId>,
            ordinal: u32,
            stable_key: &str,
        ) -> CfgNodeFact {
            CfgNodeFact {
                id: CfgNodeId(id),
                cfg_function: CfgFunctionId(1),
                body: MirBodyId(0),
                operation,
                block: BasicBlockId(block),
                kind: if operation.is_some() {
                    CfgNodeKind::Operation
                } else {
                    CfgNodeKind::Synthetic
                },
                span: Some(span()),
                generated: operation.is_none(),
                operation_ordinal: ordinal,
                stable_key: stable_key.to_string(),
                status: CfgStatus::Resolved,
                precision: CfgPrecision::ExactLowered,
            }
        }

        fn edge(
            id: u64,
            from_block: u64,
            to_block: u64,
            kind: CfgEdgeKind,
            stable_key: &str,
        ) -> CfgEdgeFact {
            CfgEdgeFact {
                id: CfgEdgeId(id),
                cfg_function: CfgFunctionId(1),
                view: CfgView::NormalControl,
                from: CfgNodeId(from_block),
                to: CfgNodeId(to_block),
                from_block: BasicBlockId(from_block),
                to_block: BasicBlockId(to_block),
                kind,
                label: None,
                stable_key: stable_key.to_string(),
                status: CfgStatus::Resolved,
                precision: CfgPrecision::ExactLowered,
            }
        }
    }
}
