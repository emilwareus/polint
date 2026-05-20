use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::cfg::facts::{
    BasicBlockKind, CfgEdgeFact, CfgPrecision, CfgStatus, CfgView, ControlDependenceFact,
    DominatorFact, PostDominatorFact, ReachabilityFact,
};
use crate::analysis::cfg::graph::CfgGraph;
use crate::analysis::cfg::ids::{
    BasicBlockId, CfgFunctionId, ControlDependenceId, DominatorId, PostDominatorId, ReachabilityId,
};
use crate::analysis::cfg::store::CfgOutput;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;

pub(crate) fn derive_reachability(output: &CfgOutput, view: CfgView) -> Vec<ReachabilityFact> {
    let mut facts = Vec::new();
    let mut next_id = 1;
    for function in sorted_functions(output) {
        let graph = CfgGraph::new(output, function, view);
        let reachable = reachable_blocks(&graph);
        for block in graph.blocks() {
            facts.push(ReachabilityFact {
                id: ReachabilityId(next_id),
                cfg_function: function,
                view,
                block: block.id,
                reachable: reachable.contains(&block.id),
                stable_key: stable_key(
                    FactFamily::CfgReachability,
                    &[
                        ("function", function.0.to_string()),
                        ("view", format!("{view:?}")),
                        ("block", block.stable_key.clone()),
                    ],
                ),
                status: CfgStatus::Resolved,
                precision: CfgPrecision::ExactLowered,
            });
            next_id += 1;
        }
    }
    facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    facts
}

pub(crate) fn derive_dominators(output: &CfgOutput, view: CfgView) -> Vec<DominatorFact> {
    let mut facts = Vec::new();
    let mut next_id = 1;
    for function in sorted_functions(output) {
        let graph = CfgGraph::new(output, function, view);
        let Some(entry) = graph.entry_block() else {
            continue;
        };
        let reachable = reachable_blocks(&graph);
        let relation = dominator_relation(&graph, entry, &reachable, Direction::Forward);
        let immediate = immediate_relation(&relation);
        for (dominated, dominators) in &relation {
            for dominator in dominators {
                facts.push(DominatorFact {
                    id: DominatorId(next_id),
                    cfg_function: function,
                    view,
                    dominator: *dominator,
                    dominated: *dominated,
                    immediate: immediate.get(dominated) == Some(dominator),
                    stable_key: stable_key(
                        FactFamily::CfgDominator,
                        &[
                            ("function", function.0.to_string()),
                            ("view", format!("{view:?}")),
                            ("dominator", dominator.0.to_string()),
                            ("dominated", dominated.0.to_string()),
                        ],
                    ),
                    status: CfgStatus::Resolved,
                    precision: CfgPrecision::ExactLowered,
                });
                next_id += 1;
            }
        }
    }
    facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    facts
}

pub(crate) fn derive_postdominators(output: &CfgOutput, view: CfgView) -> Vec<PostDominatorFact> {
    let mut facts = Vec::new();
    let mut next_id = 1;
    for function in sorted_functions(output) {
        let graph = CfgGraph::new(output, function, view);
        let blocks = graph
            .blocks()
            .into_iter()
            .map(|block| block.id)
            .collect::<BTreeSet<_>>();
        if blocks.is_empty() {
            continue;
        }
        let exits = selected_exit_blocks(&graph);
        if exits.is_empty() {
            continue;
        }
        let virtual_exit = virtual_exit_for(function);
        let mut universe = blocks.clone();
        universe.insert(virtual_exit);
        let relation = dominator_relation_with_extra_exit(
            &graph,
            virtual_exit,
            &universe,
            &exits,
            Direction::Reverse,
        );
        let immediate = immediate_relation(&relation);
        for (postdominated, postdominators) in &relation {
            if *postdominated == virtual_exit {
                continue;
            }
            for postdominator in postdominators {
                if *postdominator == virtual_exit {
                    continue;
                }
                facts.push(PostDominatorFact {
                    id: PostDominatorId(next_id),
                    cfg_function: function,
                    view,
                    postdominator: *postdominator,
                    postdominated: *postdominated,
                    immediate: immediate.get(postdominated) == Some(postdominator),
                    stable_key: stable_key(
                        FactFamily::CfgPostDominator,
                        &[
                            ("function", function.0.to_string()),
                            ("view", format!("{view:?}")),
                            ("postdominator", postdominator.0.to_string()),
                            ("postdominated", postdominated.0.to_string()),
                        ],
                    ),
                    status: CfgStatus::Resolved,
                    precision: CfgPrecision::ExactLowered,
                });
                next_id += 1;
            }
        }
    }
    facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    facts
}

pub(crate) fn derive_control_dependence(
    output: &CfgOutput,
    view: CfgView,
) -> Vec<ControlDependenceFact> {
    let mut facts = Vec::new();
    let mut next_id = 1;
    for function in sorted_functions(output) {
        let graph = CfgGraph::new(output, function, view);
        let postdominators = postdominator_relation(output, function, view);
        let immediate = immediate_relation(&postdominators);
        let block_keys = graph
            .blocks()
            .into_iter()
            .map(|block| (block.id, block.stable_key.clone()))
            .collect::<BTreeMap<_, _>>();
        let mut seen = BTreeSet::new();

        for edge in graph.edges() {
            if edge.from_block == edge.to_block {
                continue;
            }
            if postdominators
                .get(&edge.from_block)
                .is_some_and(|set| set.contains(&edge.to_block))
            {
                continue;
            }
            let stop = immediate.get(&edge.from_block).copied();
            let mut runner = edge.to_block;
            while Some(runner) != stop {
                let key = (edge.id, runner);
                if seen.insert(key) {
                    facts.push(control_dependence_fact(
                        next_id,
                        function,
                        view,
                        edge,
                        runner,
                        block_keys.get(&runner).cloned().unwrap_or_default(),
                    ));
                    next_id += 1;
                }
                let Some(next) = immediate.get(&runner).copied() else {
                    break;
                };
                if next == runner {
                    break;
                }
                runner = next;
            }
        }
    }
    facts.sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
    facts
}

fn control_dependence_fact(
    id: u64,
    function: CfgFunctionId,
    view: CfgView,
    edge: &CfgEdgeFact,
    controlled_block: BasicBlockId,
    controlled_block_key: String,
) -> ControlDependenceFact {
    ControlDependenceFact {
        id: ControlDependenceId(id),
        cfg_function: function,
        view,
        controlling_edge: edge.id,
        controlling_edge_kind: edge.kind,
        controlled_block,
        stable_key: stable_key(
            FactFamily::CfgControlDependence,
            &[
                ("function", function.0.to_string()),
                ("view", format!("{view:?}")),
                ("edge", edge.stable_key.clone()),
                ("controlled_block", controlled_block_key),
            ],
        ),
        status: edge.status,
        precision: edge.precision,
    }
}

fn sorted_functions(output: &CfgOutput) -> Vec<CfgFunctionId> {
    let mut functions = output
        .functions
        .iter()
        .map(|function| (function.stable_key.as_str(), function.id))
        .collect::<Vec<_>>();
    functions.sort();
    functions
        .into_iter()
        .map(|(_, function)| function)
        .collect()
}

fn reachable_blocks(graph: &CfgGraph<'_>) -> BTreeSet<BasicBlockId> {
    let Some(entry) = graph.entry_block() else {
        return BTreeSet::new();
    };
    let mut seen = BTreeSet::new();
    let mut stack = vec![entry];
    while let Some(block) = stack.pop() {
        if !seen.insert(block) {
            continue;
        }
        let mut successors = graph.successors(block);
        successors.sort_by(|left, right| right.cmp(left));
        stack.extend(successors);
    }
    seen
}

#[derive(Debug, Clone, Copy)]
enum Direction {
    Forward,
    Reverse,
}

fn dominator_relation(
    graph: &CfgGraph<'_>,
    start: BasicBlockId,
    universe: &BTreeSet<BasicBlockId>,
    direction: Direction,
) -> BTreeMap<BasicBlockId, BTreeSet<BasicBlockId>> {
    dominator_relation_with_extra_exit(graph, start, universe, &BTreeSet::new(), direction)
}

fn dominator_relation_with_extra_exit(
    graph: &CfgGraph<'_>,
    start: BasicBlockId,
    universe: &BTreeSet<BasicBlockId>,
    selected_exits: &BTreeSet<BasicBlockId>,
    direction: Direction,
) -> BTreeMap<BasicBlockId, BTreeSet<BasicBlockId>> {
    let mut relation = universe
        .iter()
        .map(|block| {
            let initial = if *block == start {
                BTreeSet::from([start])
            } else {
                universe.clone()
            };
            (*block, initial)
        })
        .collect::<BTreeMap<_, _>>();

    let mut changed = true;
    while changed {
        changed = false;
        for block in universe.iter().copied().filter(|block| *block != start) {
            let neighbors = match direction {
                Direction::Forward => graph.predecessors(block),
                Direction::Reverse => reversed_predecessors(graph, block, start, selected_exits),
            }
            .into_iter()
            .filter(|neighbor| universe.contains(neighbor))
            .collect::<Vec<_>>();

            let mut new_set = if neighbors.is_empty() {
                BTreeSet::new()
            } else {
                intersect_sets(
                    neighbors
                        .iter()
                        .filter_map(|neighbor| relation.get(neighbor)),
                )
            };
            new_set.insert(block);
            if relation.get(&block) != Some(&new_set) {
                relation.insert(block, new_set);
                changed = true;
            }
        }
    }
    relation
}

fn reversed_predecessors(
    graph: &CfgGraph<'_>,
    block: BasicBlockId,
    virtual_exit: BasicBlockId,
    selected_exits: &BTreeSet<BasicBlockId>,
) -> Vec<BasicBlockId> {
    if block == virtual_exit {
        return Vec::new();
    }
    let mut predecessors = graph.successors(block);
    if selected_exits.contains(&block) {
        predecessors.push(virtual_exit);
    }
    predecessors
}

fn intersect_sets<'a>(
    mut sets: impl Iterator<Item = &'a BTreeSet<BasicBlockId>>,
) -> BTreeSet<BasicBlockId> {
    let Some(first) = sets.next() else {
        return BTreeSet::new();
    };
    sets.fold(first.clone(), |acc, set| {
        acc.intersection(set).copied().collect()
    })
}

fn immediate_relation(
    relation: &BTreeMap<BasicBlockId, BTreeSet<BasicBlockId>>,
) -> BTreeMap<BasicBlockId, BasicBlockId> {
    let mut immediate = BTreeMap::new();
    for (node, dominators) in relation {
        let strict = dominators
            .iter()
            .copied()
            .filter(|candidate| candidate != node)
            .collect::<BTreeSet<_>>();
        for candidate in strict.iter().copied() {
            let candidate_dominators = relation.get(&candidate).cloned().unwrap_or_default();
            if strict
                .iter()
                .copied()
                .filter(|other| *other != candidate)
                .all(|other| candidate_dominators.contains(&other))
            {
                immediate.insert(*node, candidate);
                break;
            }
        }
    }
    immediate
}

fn postdominator_relation(
    output: &CfgOutput,
    function: CfgFunctionId,
    view: CfgView,
) -> BTreeMap<BasicBlockId, BTreeSet<BasicBlockId>> {
    let graph = CfgGraph::new(output, function, view);
    let blocks = graph
        .blocks()
        .into_iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let exits = selected_exit_blocks(&graph);
    if blocks.is_empty() || exits.is_empty() {
        return BTreeMap::new();
    }
    let virtual_exit = virtual_exit_for(function);
    let mut universe = blocks;
    universe.insert(virtual_exit);
    let mut relation = dominator_relation_with_extra_exit(
        &graph,
        virtual_exit,
        &universe,
        &exits,
        Direction::Reverse,
    );
    relation.remove(&virtual_exit);
    for set in relation.values_mut() {
        set.remove(&virtual_exit);
    }
    relation
}

fn selected_exit_blocks(graph: &CfgGraph<'_>) -> BTreeSet<BasicBlockId> {
    let mut exits = graph
        .blocks()
        .into_iter()
        .filter(|block| {
            matches!(
                block.kind,
                BasicBlockKind::ExitNormal | BasicBlockKind::ExitExceptional
            ) || graph.successors(block.id).is_empty()
        })
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    if let Some(exit) = graph.synthetic_exit_block(CfgView::NormalControl) {
        exits.insert(exit);
    }
    exits
}

fn virtual_exit_for(function: CfgFunctionId) -> BasicBlockId {
    BasicBlockId(u64::MAX - function.0)
}

fn stable_key(family: FactFamily, parts: &[(&str, String)]) -> String {
    semantic_stable_key(family, parts).into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::builder::CfgBuilder;
    use crate::analysis::cfg::facts::{BasicBlockKind, CfgEdgeKind, CfgNodeKind};
    use crate::analysis::ids::{MirBodyId, MirOpId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirStatus};
    use crate::analysis::mir::op::{AssignMode, MirOperation, MirOperationKind, MirValue};
    use crate::core::{FileId, FunctionId, Language, Span};

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

    fn body() -> MirBody {
        MirBody {
            id: MirBodyId(1),
            language: Language::Go,
            file: FileId(1),
            function: FunctionId(1),
            package: None,
            module: None,
            owner_stable_key: "owner".to_string(),
            span: span(),
            stable_key: "body:one".to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn op(id: u64, ordinal: u32) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body: MirBodyId(1),
            ordinal,
            span: span(),
            kind: MirOperationKind::Assign {
                place: PlaceId(1),
                value: MirValue::Place(PlaceId(2)),
                mode: AssignMode::Overwrite,
            },
            stable_key: format!("op:{ordinal}"),
            status: MirStatus::Resolved,
        }
    }

    fn if_else_graph() -> CfgOutput {
        let mut builder = CfgBuilder::new();
        builder.start_function(&body(), false);
        let entry = builder.current_block();
        let condition = builder.start_block(BasicBlockKind::Branch);
        builder.append_operation_node(Some(&op(1, 1)), CfgNodeKind::Condition, Some(span()));
        let then_block = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(2, 2)), CfgNodeKind::Operation, Some(span()));
        let else_block = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(3, 3)), CfgNodeKind::Operation, Some(span()));
        let join = builder.start_block(BasicBlockKind::Join);
        builder.append_operation_node(Some(&op(4, 4)), CfgNodeKind::Operation, Some(span()));

        builder.add_edge(entry, condition, CfgEdgeKind::Normal);
        builder.add_edge(condition, then_block, CfgEdgeKind::True);
        builder.add_edge(condition, else_block, CfgEdgeKind::False);
        builder.add_edge(then_block, join, CfgEdgeKind::Normal);
        builder.add_edge(else_block, join, CfgEdgeKind::Normal);
        builder.finish_function();
        builder.finish()
    }

    #[test]
    fn reachability_excludes_unreachable_blocks_from_dominators() {
        let mut builder = CfgBuilder::new();
        builder.start_function(&body(), false);
        let entry = builder.current_block();
        let reachable = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(1, 1)), CfgNodeKind::Operation, Some(span()));
        let unreachable = builder.start_block(BasicBlockKind::Unreachable);
        builder.append_operation_node(Some(&op(2, 2)), CfgNodeKind::Operation, Some(span()));
        builder.mark_unreachable(unreachable);
        builder.add_edge(entry, reachable, CfgEdgeKind::Normal);
        builder.finish_function();
        let output = builder.finish();

        let reachability = derive_reachability(&output, CfgView::NormalControl);
        assert!(
            reachability
                .iter()
                .any(|fact| fact.block == reachable && fact.reachable)
        );
        assert!(
            reachability
                .iter()
                .any(|fact| fact.block == unreachable && !fact.reachable)
        );

        let dominators = derive_dominators(&output, CfgView::NormalControl);
        assert!(
            !dominators
                .iter()
                .any(|fact| fact.dominated == unreachable || fact.dominator == unreachable)
        );
    }

    #[test]
    fn dominators_are_deterministic_for_branch_join_graphs() {
        let output = if_else_graph();
        let first = derive_dominators(&output, CfgView::NormalControl);
        let second = derive_dominators(&output, CfgView::NormalControl);
        assert_eq!(first, second);
        assert!(first.iter().any(|fact| fact.immediate));
    }

    #[test]
    fn postdominators_handle_multiple_returns_and_unified_exit() {
        let mut builder = CfgBuilder::new();
        builder.start_function(&body(), false);
        let entry = builder.current_block();
        let first_return = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(1, 1)), CfgNodeKind::Return, Some(span()));
        let second_return = builder.start_block(BasicBlockKind::StraightLine);
        builder.append_operation_node(Some(&op(2, 2)), CfgNodeKind::Return, Some(span()));
        let exit = builder.normal_exit_block();
        builder.add_edge(entry, first_return, CfgEdgeKind::True);
        builder.add_edge(entry, second_return, CfgEdgeKind::False);
        builder.add_edge(first_return, exit, CfgEdgeKind::Return);
        builder.add_edge(second_return, exit, CfgEdgeKind::Return);
        builder.finish_function();
        let output = builder.finish();

        let postdominators = derive_postdominators(&output, CfgView::NormalControl);
        assert!(postdominators.iter().any(|fact| fact.immediate));
        assert_eq!(
            postdominators
                .iter()
                .filter(|fact| fact.postdominated == first_return && fact.immediate)
                .count(),
            1
        );
    }

    #[test]
    fn control_dependence_records_branch_edges_without_unreachable_tails() {
        let output = if_else_graph();
        let dependence = derive_control_dependence(&output, CfgView::NormalControl);
        assert!(
            dependence
                .iter()
                .any(|fact| fact.controlling_edge_kind == CfgEdgeKind::True)
        );
        assert!(
            dependence
                .iter()
                .any(|fact| fact.controlling_edge_kind == CfgEdgeKind::False)
        );
        assert!(
            dependence
                .iter()
                .all(|fact| fact.view == CfgView::NormalControl)
        );
    }

    #[test]
    fn loop_control_dependence_deduplicates_structurally_identical_rows() {
        let mut builder = CfgBuilder::new();
        builder.start_function(&body(), false);
        let entry = builder.current_block();
        let header = builder.start_block(BasicBlockKind::LoopHeader);
        builder.append_operation_node(Some(&op(1, 1)), CfgNodeKind::Condition, Some(span()));
        let body_block = builder.start_block(BasicBlockKind::LoopBody);
        builder.append_operation_node(Some(&op(2, 2)), CfgNodeKind::Operation, Some(span()));
        let exit_block = builder.start_block(BasicBlockKind::Join);
        builder.append_operation_node(Some(&op(3, 3)), CfgNodeKind::Operation, Some(span()));
        builder.add_edge(entry, header, CfgEdgeKind::LoopEnter);
        builder.add_edge(header, body_block, CfgEdgeKind::True);
        builder.add_edge(header, exit_block, CfgEdgeKind::LoopExit);
        builder.add_edge(body_block, header, CfgEdgeKind::LoopBack);
        builder.finish_function();
        let output = builder.finish();

        let dependence = derive_control_dependence(&output, CfgView::NormalControl);
        let keys = dependence
            .iter()
            .map(|fact| fact.stable_key.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(keys.len(), dependence.len());
        assert!(
            dependence
                .iter()
                .any(|fact| fact.controlling_edge_kind == CfgEdgeKind::True)
        );
    }
}
