use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::cfg::facts::{
    BasicBlockKind, CfgEdgeFact, CfgPrecision, CfgStatus, CfgView, ControlDependenceFact,
    DominatorFact, PostDominatorFact, ReachabilityFact,
};
use crate::analysis::cfg::graph::{CfgGraph, CfgGraphIndex};
use crate::analysis::cfg::ids::{
    BasicBlockId, CfgFunctionId, ControlDependenceId, DominatorId, PostDominatorId, ReachabilityId,
};
use crate::analysis::cfg::store::CfgOutput;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;

pub(crate) fn derive_reachability(
    interner: &crate::core::StableKeyInterner,
    output: &CfgOutput,
    view: CfgView,
) -> Vec<ReachabilityFact> {
    let mut facts = Vec::new();
    let mut next_id = 1;
    let index = CfgGraphIndex::new(output);
    for graph in index.graphs(view) {
        let function = graph.function_id();
        let function_key = graph.function_stable_key();
        let reachable = reachable_blocks(&graph);
        for block in graph.block_refs() {
            facts.push(ReachabilityFact {
                id: ReachabilityId(next_id),
                cfg_function: function,
                view,
                block: block.id,
                reachable: reachable.contains(&block.id),
                stable_key: stable_key(
                    interner,
                    FactFamily::CfgReachability,
                    &[
                        ("function", function_key.clone()),
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

pub(crate) fn derive_dominators(
    interner: &crate::core::StableKeyInterner,
    output: &CfgOutput,
    view: CfgView,
) -> Vec<DominatorFact> {
    let mut facts = Vec::new();
    let mut next_id = 1;
    let index = CfgGraphIndex::new(output);
    for graph in index.graphs(view) {
        let function = graph.function_id();
        let function_key = graph.function_stable_key();
        let block_keys = block_key_map(&graph);
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
                        interner,
                        FactFamily::CfgDominator,
                        &[
                            ("function", function_key.clone()),
                            ("view", format!("{view:?}")),
                            ("dominator", stable_block_key(&block_keys, *dominator)),
                            ("dominated", stable_block_key(&block_keys, *dominated)),
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

pub(crate) fn derive_postdominators(
    interner: &crate::core::StableKeyInterner,
    output: &CfgOutput,
    view: CfgView,
) -> Vec<PostDominatorFact> {
    let mut facts = Vec::new();
    let mut next_id = 1;
    let index = CfgGraphIndex::new(output);
    for graph in index.graphs(view) {
        let function = graph.function_id();
        let function_key = graph.function_stable_key();
        let block_keys = block_key_map(&graph);
        let blocks = graph
            .block_refs()
            .iter()
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
                        interner,
                        FactFamily::CfgPostDominator,
                        &[
                            ("function", function_key.clone()),
                            ("view", format!("{view:?}")),
                            (
                                "postdominator",
                                stable_block_key(&block_keys, *postdominator),
                            ),
                            (
                                "postdominated",
                                stable_block_key(&block_keys, *postdominated),
                            ),
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
    interner: &crate::core::StableKeyInterner,
    output: &CfgOutput,
    view: CfgView,
) -> Vec<ControlDependenceFact> {
    let mut facts = Vec::new();
    let mut next_id = 1;
    let index = CfgGraphIndex::new(output);
    for graph in index.graphs(view) {
        let function = graph.function_id();
        let function_key = graph.function_stable_key();
        let postdominators = postdominator_relation_for_graph(&graph);
        let immediate = immediate_relation(&postdominators);
        let block_keys = block_key_map(&graph);
        let mut seen = BTreeSet::new();

        for edge in graph.edge_refs() {
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
                        interner,
                        next_id,
                        function_key.as_str(),
                        function,
                        view,
                        edge,
                        (runner, stable_block_key(&block_keys, runner)),
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
    interner: &crate::core::StableKeyInterner,
    id: u64,
    function_key: &str,
    function: CfgFunctionId,
    view: CfgView,
    edge: &CfgEdgeFact,
    pair: (BasicBlockId, String),
) -> ControlDependenceFact {
    let (controlled_block, controlled_block_key) = pair;
    ControlDependenceFact {
        id: ControlDependenceId(id),
        cfg_function: function,
        view,
        controlling_edge: edge.id,
        controlling_edge_kind: edge.kind,
        controlled_block,
        stable_key: stable_key(
            interner,
            FactFamily::CfgControlDependence,
            &[
                ("function", function_key.to_string()),
                ("view", format!("{view:?}")),
                ("edge", edge.stable_key.clone()),
                ("controlled_block", controlled_block_key),
            ],
        ),
        status: edge.status,
        precision: edge.precision,
    }
}

fn block_key_map(graph: &CfgGraph<'_>) -> BTreeMap<BasicBlockId, String> {
    graph
        .block_refs()
        .iter()
        .map(|block| (block.id, block.stable_key.clone()))
        .collect()
}

fn stable_block_key(block_keys: &BTreeMap<BasicBlockId, String>, block: BasicBlockId) -> String {
    block_keys
        .get(&block)
        .cloned()
        .unwrap_or_else(|| format!("<missing-block:{}>", block.0))
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
        stack.extend(graph.successor_blocks(block).iter().rev().copied());
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
            let mut neighbors = Vec::new();
            match direction {
                Direction::Forward => {
                    neighbors.extend(
                        graph
                            .predecessor_blocks(block)
                            .iter()
                            .copied()
                            .filter(|neighbor| universe.contains(neighbor)),
                    );
                }
                Direction::Reverse => {
                    collect_reversed_predecessors(
                        graph,
                        block,
                        start,
                        selected_exits,
                        universe,
                        &mut neighbors,
                    );
                }
            }

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

fn collect_reversed_predecessors(
    graph: &CfgGraph<'_>,
    block: BasicBlockId,
    virtual_exit: BasicBlockId,
    selected_exits: &BTreeSet<BasicBlockId>,
    universe: &BTreeSet<BasicBlockId>,
    out: &mut Vec<BasicBlockId>,
) {
    if block == virtual_exit {
        return;
    }
    out.extend(
        graph
            .successor_blocks(block)
            .iter()
            .copied()
            .filter(|neighbor| universe.contains(neighbor)),
    );
    if selected_exits.contains(&block) {
        out.push(virtual_exit);
    }
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

fn postdominator_relation_for_graph(
    graph: &CfgGraph<'_>,
) -> BTreeMap<BasicBlockId, BTreeSet<BasicBlockId>> {
    let blocks = graph
        .block_refs()
        .iter()
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    let exits = selected_exit_blocks(graph);
    if blocks.is_empty() || exits.is_empty() {
        return BTreeMap::new();
    }
    let virtual_exit = virtual_exit_for(graph.function_id());
    let mut universe = blocks;
    universe.insert(virtual_exit);
    let mut relation = dominator_relation_with_extra_exit(
        graph,
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
        .block_refs()
        .iter()
        .filter(|block| {
            matches!(
                block.kind,
                BasicBlockKind::ExitNormal | BasicBlockKind::ExitExceptional
            ) || graph.successor_blocks(block.id).is_empty()
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

fn stable_key(
    interner: &crate::core::StableKeyInterner,
    family: FactFamily,
    parts: &[(&str, String)],
) -> String {
    semantic_stable_key(interner, family, parts).into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::builder::CfgBuilder;
    use crate::analysis::cfg::facts::{BasicBlockKind, CfgEdgeKind, CfgNodeKind};
    use crate::analysis::cfg::ids::{CfgEdgeId, CfgNodeId};
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
        builder.start_function(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &body(),
            false,
        );
        let entry = builder.current_block();
        let condition = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::Branch,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(1, 1)),
            CfgNodeKind::Condition,
            Some(span()),
        );
        let then_block = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::StraightLine,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(2, 2)),
            CfgNodeKind::Operation,
            Some(span()),
        );
        let else_block = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::StraightLine,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(3, 3)),
            CfgNodeKind::Operation,
            Some(span()),
        );
        let join = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::Join,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(4, 4)),
            CfgNodeKind::Operation,
            Some(span()),
        );

        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            entry,
            condition,
            CfgEdgeKind::Normal,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            condition,
            then_block,
            CfgEdgeKind::True,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            condition,
            else_block,
            CfgEdgeKind::False,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            then_block,
            join,
            CfgEdgeKind::Normal,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            else_block,
            join,
            CfgEdgeKind::Normal,
        );
        builder.finish_function();
        builder.finish()
    }

    #[test]
    fn reachability_excludes_unreachable_blocks_from_dominators() {
        let mut builder = CfgBuilder::new();
        builder.start_function(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &body(),
            false,
        );
        let entry = builder.current_block();
        let reachable = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::StraightLine,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(1, 1)),
            CfgNodeKind::Operation,
            Some(span()),
        );
        let unreachable = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::Unreachable,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(2, 2)),
            CfgNodeKind::Operation,
            Some(span()),
        );
        builder.mark_unreachable(unreachable);
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            entry,
            reachable,
            CfgEdgeKind::Normal,
        );
        builder.finish_function();
        let output = builder.finish();

        let reachability = derive_reachability(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &output,
            CfgView::NormalControl,
        );
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

        let dominators = derive_dominators(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &output,
            CfgView::NormalControl,
        );
        assert!(
            !dominators
                .iter()
                .any(|fact| fact.dominated == unreachable || fact.dominator == unreachable)
        );
    }

    #[test]
    fn dominators_are_deterministic_for_branch_join_graphs() {
        let output = if_else_graph();
        let first = derive_dominators(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &output,
            CfgView::NormalControl,
        );
        let second = derive_dominators(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &output,
            CfgView::NormalControl,
        );
        assert_eq!(first, second);
        assert!(first.iter().any(|fact| fact.immediate));
    }

    #[test]
    fn postdominators_handle_multiple_returns_and_unified_exit() {
        let mut builder = CfgBuilder::new();
        builder.start_function(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &body(),
            false,
        );
        let entry = builder.current_block();
        let first_return = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::StraightLine,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(1, 1)),
            CfgNodeKind::Return,
            Some(span()),
        );
        let second_return = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::StraightLine,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(2, 2)),
            CfgNodeKind::Return,
            Some(span()),
        );
        let exit = builder.normal_exit_block();
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            entry,
            first_return,
            CfgEdgeKind::True,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            entry,
            second_return,
            CfgEdgeKind::False,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            first_return,
            exit,
            CfgEdgeKind::Return,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            second_return,
            exit,
            CfgEdgeKind::Return,
        );
        builder.finish_function();
        let output = builder.finish();

        let postdominators = derive_postdominators(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &output,
            CfgView::NormalControl,
        );
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
        let dependence = derive_control_dependence(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &output,
            CfgView::NormalControl,
        );
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
        builder.start_function(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &body(),
            false,
        );
        let entry = builder.current_block();
        let header = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::LoopHeader,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(1, 1)),
            CfgNodeKind::Condition,
            Some(span()),
        );
        let body_block = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::LoopBody,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(2, 2)),
            CfgNodeKind::Operation,
            Some(span()),
        );
        let exit_block = builder.start_block(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            BasicBlockKind::Join,
        );
        builder.append_operation_node(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            Some(&op(3, 3)),
            CfgNodeKind::Operation,
            Some(span()),
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            entry,
            header,
            CfgEdgeKind::LoopEnter,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            header,
            body_block,
            CfgEdgeKind::True,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            header,
            exit_block,
            CfgEdgeKind::LoopExit,
        );
        builder.add_edge(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            body_block,
            header,
            CfgEdgeKind::LoopBack,
        );
        builder.finish_function();
        let output = builder.finish();

        let dependence = derive_control_dependence(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &output,
            CfgView::NormalControl,
        );
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

    #[test]
    fn derived_stable_keys_do_not_depend_on_dense_ids() {
        let output = if_else_graph();
        let shifted = shift_dense_ids(output.clone());

        assert_eq!(
            stable_keys(derive_reachability(
                &crate::core::AnalysisDb::new().stable_key_interner(),
                &output,
                CfgView::NormalControl
            )),
            stable_keys(derive_reachability(
                &crate::core::AnalysisDb::new().stable_key_interner(),
                &shifted,
                CfgView::NormalControl
            ))
        );
        assert_eq!(
            stable_keys(derive_dominators(
                &crate::core::AnalysisDb::new().stable_key_interner(),
                &output,
                CfgView::NormalControl
            )),
            stable_keys(derive_dominators(
                &crate::core::AnalysisDb::new().stable_key_interner(),
                &shifted,
                CfgView::NormalControl
            ))
        );
        assert_eq!(
            stable_keys(derive_postdominators(
                &crate::core::AnalysisDb::new().stable_key_interner(),
                &output,
                CfgView::NormalControl
            )),
            stable_keys(derive_postdominators(
                &crate::core::AnalysisDb::new().stable_key_interner(),
                &shifted,
                CfgView::NormalControl
            ))
        );
        assert_eq!(
            stable_keys(derive_control_dependence(
                &crate::core::AnalysisDb::new().stable_key_interner(),
                &output,
                CfgView::NormalControl
            )),
            stable_keys(derive_control_dependence(
                &crate::core::AnalysisDb::new().stable_key_interner(),
                &shifted,
                CfgView::NormalControl
            ))
        );
    }

    trait StableKeyRow {
        fn stable_key(&self) -> &str;
    }

    impl StableKeyRow for ReachabilityFact {
        fn stable_key(&self) -> &str {
            &self.stable_key
        }
    }

    impl StableKeyRow for DominatorFact {
        fn stable_key(&self) -> &str {
            &self.stable_key
        }
    }

    impl StableKeyRow for PostDominatorFact {
        fn stable_key(&self) -> &str {
            &self.stable_key
        }
    }

    impl StableKeyRow for ControlDependenceFact {
        fn stable_key(&self) -> &str {
            &self.stable_key
        }
    }

    fn stable_keys(rows: impl IntoIterator<Item = impl StableKeyRow>) -> Vec<String> {
        rows.into_iter()
            .map(|row| row.stable_key().to_string())
            .collect()
    }

    fn shift_dense_ids(mut output: CfgOutput) -> CfgOutput {
        for function in &mut output.functions {
            function.id = CfgFunctionId(function.id.0 + 100);
            function.entry_node = CfgNodeId(function.entry_node.0 + 1_000);
            function.normal_exit_node = CfgNodeId(function.normal_exit_node.0 + 1_000);
            function.exceptional_exit_node = function
                .exceptional_exit_node
                .map(|node| CfgNodeId(node.0 + 1_000));
        }
        for node in &mut output.nodes {
            node.id = CfgNodeId(node.id.0 + 1_000);
            node.cfg_function = CfgFunctionId(node.cfg_function.0 + 100);
            node.block = BasicBlockId(node.block.0 + 2_000);
        }
        for block in &mut output.blocks {
            block.id = BasicBlockId(block.id.0 + 2_000);
            block.cfg_function = CfgFunctionId(block.cfg_function.0 + 100);
            block.first_node = block.first_node.map(|node| CfgNodeId(node.0 + 1_000));
            block.last_node = block.last_node.map(|node| CfgNodeId(node.0 + 1_000));
        }
        for edge in &mut output.edges {
            edge.id = CfgEdgeId(edge.id.0 + 3_000);
            edge.cfg_function = CfgFunctionId(edge.cfg_function.0 + 100);
            edge.from = CfgNodeId(edge.from.0 + 1_000);
            edge.to = CfgNodeId(edge.to.0 + 1_000);
            edge.from_block = BasicBlockId(edge.from_block.0 + 2_000);
            edge.to_block = BasicBlockId(edge.to_block.0 + 2_000);
        }
        output
    }
}
