use std::collections::{BTreeMap, BTreeSet};

use crate::analysis_neutral::AnalysisHost;
use crate::analysis_neutral::cfg::facts::{
    BasicBlockFact, BasicBlockKind, CfgEdgeFact, CfgFunctionFact, CfgNodeFact, CfgPrecision,
    CfgStatus, CfgView,
};
use crate::analysis_neutral::cfg::ids::{BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId};
use crate::analysis_neutral::ids::MirBodyId;
use crate::internal_core::{Diagnostic, DiagnosticRange};

pub fn validate_cfg(db: &impl AnalysisHost, diagnostics: &mut Vec<Diagnostic>) {
    let index = CfgValidationIndex::from_db(db);
    let interner = db.stable_key_interner();

    check_duplicate_stable_keys(
        diagnostics,
        "CfgFunction",
        &interner,
        db.cfg_functions().iter().map(|row| row.stable_key),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "CfgNode",
        &interner,
        db.cfg_nodes().iter().map(|row| row.stable_key),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "BasicBlock",
        &interner,
        db.cfg_blocks().iter().map(|row| row.stable_key),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "CfgEdge",
        &interner,
        db.cfg_edges().iter().map(|row| row.stable_key),
    );

    validate_function_graph_shapes(&interner, &index, diagnostics);

    for function in db.cfg_functions() {
        if !index.nodes.contains_key(&function.entry_node) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgFunction",
                interner.resolve(function.stable_key).as_ref(),
                "entry_node",
                "dangling entry node",
            );
        }
        if !index.nodes.contains_key(&function.normal_exit_node) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgFunction",
                interner.resolve(function.stable_key).as_ref(),
                "normal_exit_node",
                "dangling normal exit node",
            );
        }
        if let Some(exceptional_exit) = function.exceptional_exit_node
            && !index.nodes.contains_key(&exceptional_exit)
        {
            push_cfg_diagnostic(
                diagnostics,
                "CfgFunction",
                interner.resolve(function.stable_key).as_ref(),
                "exceptional_exit_node",
                "dangling exceptional exit node",
            );
        }
        if !index.mir_bodies.contains(&function.body) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgFunction",
                interner.resolve(function.stable_key).as_ref(),
                "body",
                "dangling MIR body reference",
            );
        }
    }

    for node in db.cfg_nodes() {
        if !index.functions.contains_key(&node.cfg_function) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgNode",
                interner.resolve(node.stable_key).as_ref(),
                "cfg_function",
                "dangling CFG function reference",
            );
        }
        if !index.blocks.contains_key(&node.block) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgNode",
                interner.resolve(node.stable_key).as_ref(),
                "block",
                "dangling basic block reference",
            );
        }
        if let Some(span) = &node.span
            && span.start_byte > span.end_byte
        {
            push_cfg_diagnostic(
                diagnostics,
                "CfgNode",
                interner.resolve(node.stable_key).as_ref(),
                "span",
                "invalid span byte range",
            );
        }
    }

    for block in db.cfg_blocks() {
        if !index.functions.contains_key(&block.cfg_function) {
            push_cfg_diagnostic(
                diagnostics,
                "BasicBlock",
                interner.resolve(block.stable_key).as_ref(),
                "cfg_function",
                "dangling CFG function reference",
            );
        }
        if !synthetic_block_kind(block.kind)
            && (block.first_node.is_none() || block.last_node.is_none())
        {
            push_cfg_diagnostic(
                diagnostics,
                "BasicBlock",
                interner.resolve(block.stable_key).as_ref(),
                "node_range",
                "block node ranges are non-empty except synthetic entry/exit blocks",
            );
        }
        check_optional_node(
            diagnostics,
            &index.nodes,
            block.first_node,
            "BasicBlock",
            interner.resolve(block.stable_key).as_ref(),
            "first_node",
        );
        check_optional_node(
            diagnostics,
            &index.nodes,
            block.last_node,
            "BasicBlock",
            interner.resolve(block.stable_key).as_ref(),
            "last_node",
        );
    }

    let mut edge_shapes = BTreeSet::new();
    for edge in db.cfg_edges() {
        if !index.functions.contains_key(&edge.cfg_function) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgEdge",
                interner.resolve(edge.stable_key).as_ref(),
                "cfg_function",
                "dangling CFG function reference",
            );
        }
        check_edge_node(
            diagnostics,
            &index.nodes,
            edge.from,
            edge.cfg_function,
            "CfgEdge",
            interner.resolve(edge.stable_key).as_ref(),
            "from",
        );
        check_edge_node(
            diagnostics,
            &index.nodes,
            edge.to,
            edge.cfg_function,
            "CfgEdge",
            interner.resolve(edge.stable_key).as_ref(),
            "to",
        );
        check_edge_block(
            diagnostics,
            &index.blocks,
            edge.from_block,
            edge.cfg_function,
            "CfgEdge",
            interner.resolve(edge.stable_key).as_ref(),
            "from_block",
        );
        check_edge_block(
            diagnostics,
            &index.blocks,
            edge.to_block,
            edge.cfg_function,
            "CfgEdge",
            interner.resolve(edge.stable_key).as_ref(),
            "to_block",
        );
        if !edge_shapes.insert((
            edge.cfg_function,
            edge.view,
            edge.from,
            edge.to,
            edge.from_block,
            edge.to_block,
            edge.kind,
        )) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgEdge",
                interner.resolve(edge.stable_key).as_ref(),
                "edge",
                "duplicate identical edges after normalization",
            );
        }
    }

    for row in db.cfg_reachability() {
        check_block_ref(
            diagnostics,
            &index.blocks,
            row.block,
            row.cfg_function,
            "CfgReachability",
            interner.resolve(row.stable_key).as_ref(),
            "block",
        );
    }
    for row in db.cfg_dominators() {
        check_block_ref(
            diagnostics,
            &index.blocks,
            row.dominator,
            row.cfg_function,
            "CfgDominator",
            interner.resolve(row.stable_key).as_ref(),
            "dominator",
        );
        check_block_ref(
            diagnostics,
            &index.blocks,
            row.dominated,
            row.cfg_function,
            "CfgDominator",
            interner.resolve(row.stable_key).as_ref(),
            "dominated",
        );
    }
    for row in db.cfg_postdominators() {
        check_block_ref(
            diagnostics,
            &index.blocks,
            row.postdominator,
            row.cfg_function,
            "CfgPostDominator",
            interner.resolve(row.stable_key).as_ref(),
            "postdominator",
        );
        check_block_ref(
            diagnostics,
            &index.blocks,
            row.postdominated,
            row.cfg_function,
            "CfgPostDominator",
            interner.resolve(row.stable_key).as_ref(),
            "postdominated",
        );
    }
    for row in db.cfg_control_dependence() {
        check_edge_ref(
            diagnostics,
            &index.edges,
            row.controlling_edge,
            row.cfg_function,
            "CfgControlDependence",
            interner.resolve(row.stable_key).as_ref(),
            "controlling_edge",
        );
        check_block_ref(
            diagnostics,
            &index.blocks,
            row.controlled_block,
            row.cfg_function,
            "CfgControlDependence",
            interner.resolve(row.stable_key).as_ref(),
            "controlled_block",
        );
    }
    for row in db.unsupported_control_flow() {
        if row.construct.is_empty() || row.source_evidence.is_empty() {
            push_cfg_diagnostic(
                diagnostics,
                "UnsupportedControlFlow",
                interner.resolve(row.stable_key).as_ref(),
                "source_evidence",
                "unsupported control-flow rows require construct and source evidence",
            );
        }
        if matches!(row.status, CfgStatus::Resolved)
            || matches!(
                row.precision,
                CfgPrecision::ExactSyntax | CfgPrecision::ExactLowered
            )
        {
            push_cfg_diagnostic(
                diagnostics,
                "UnsupportedControlFlow",
                interner.resolve(row.stable_key).as_ref(),
                "precision",
                "unsupported control-flow rows cannot claim exact resolved precision",
            );
        }
    }
}

struct CfgValidationIndex<'a> {
    functions: BTreeMap<CfgFunctionId, &'a CfgFunctionFact>,
    nodes: BTreeMap<CfgNodeId, &'a CfgNodeFact>,
    blocks: BTreeMap<BasicBlockId, &'a BasicBlockFact>,
    edges: BTreeMap<CfgEdgeId, &'a CfgEdgeFact>,
    mir_bodies: BTreeSet<MirBodyId>,
    blocks_by_function: BTreeMap<CfgFunctionId, Vec<&'a BasicBlockFact>>,
    normal_successors_by_block: BTreeMap<(CfgFunctionId, BasicBlockId), Vec<BasicBlockId>>,
    unsupported_by_function: BTreeSet<CfgFunctionId>,
}

impl<'a> CfgValidationIndex<'a> {
    fn from_db(db: &'a impl AnalysisHost) -> Self {
        let functions = db
            .cfg_functions()
            .iter()
            .map(|row| (row.id, row))
            .collect::<BTreeMap<_, _>>();
        let nodes = db
            .cfg_nodes()
            .iter()
            .map(|row| (row.id, row))
            .collect::<BTreeMap<_, _>>();
        let blocks = db
            .cfg_blocks()
            .iter()
            .map(|row| (row.id, row))
            .collect::<BTreeMap<_, _>>();
        let edges = db
            .cfg_edges()
            .iter()
            .map(|row| (row.id, row))
            .collect::<BTreeMap<_, _>>();
        let mir_bodies = db.mir_bodies().iter().map(|body| body.id).collect();

        let mut blocks_by_function = BTreeMap::<CfgFunctionId, Vec<&BasicBlockFact>>::new();
        for block in db.cfg_blocks() {
            blocks_by_function
                .entry(block.cfg_function)
                .or_default()
                .push(block);
        }

        let mut normal_successors_by_block =
            BTreeMap::<(CfgFunctionId, BasicBlockId), Vec<BasicBlockId>>::new();
        for edge in db.cfg_edges() {
            if edge.view == CfgView::NormalControl && edge.to_block != edge.from_block {
                normal_successors_by_block
                    .entry((edge.cfg_function, edge.from_block))
                    .or_default()
                    .push(edge.to_block);
            }
        }
        for successors in normal_successors_by_block.values_mut() {
            successors.sort();
            successors.dedup();
        }

        let unsupported_by_function = db
            .unsupported_control_flow()
            .iter()
            .filter_map(|row| row.cfg_function)
            .collect();

        Self {
            functions,
            nodes,
            blocks,
            edges,
            mir_bodies,
            blocks_by_function,
            normal_successors_by_block,
            unsupported_by_function,
        }
    }

    fn blocks_for_function(
        &self,
        function: CfgFunctionId,
    ) -> impl Iterator<Item = &'a BasicBlockFact> + '_ {
        self.blocks_by_function
            .get(&function)
            .into_iter()
            .flat_map(|blocks| blocks.iter().copied())
    }

    fn normal_successors(&self, function: CfgFunctionId, block: BasicBlockId) -> &[BasicBlockId] {
        self.normal_successors_by_block
            .get(&(function, block))
            .map_or(&[], Vec::as_slice)
    }
}

fn validate_function_graph_shapes(
    interner: &crate::internal_core::StableKeyInterner,
    index: &CfgValidationIndex<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for function in index.functions.values().copied() {
        let function_blocks = index.blocks_for_function(function.id).collect::<Vec<_>>();
        let entry_blocks = function_blocks
            .iter()
            .filter(|block| block.kind == BasicBlockKind::Entry)
            .count();
        if entry_blocks != 1 {
            push_cfg_diagnostic(
                diagnostics,
                "CfgFunction",
                interner.resolve(function.stable_key).as_ref(),
                "entry_block",
                "expected exactly one entry block",
            );
        }

        let selected_exits = function_blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.kind,
                    BasicBlockKind::ExitNormal | BasicBlockKind::ExitExceptional
                ) || index.normal_successors(function.id, block.id).is_empty()
            })
            .count();
        let has_unsupported_boundary = index.unsupported_by_function.contains(&function.id);
        if selected_exits == 0 && !has_unsupported_boundary {
            push_cfg_diagnostic(
                diagnostics,
                "CfgFunction",
                interner.resolve(function.stable_key).as_ref(),
                "exit_block",
                "expected at least one selected exit block or unsupported boundary",
            );
        }

        let reachable = reachable_blocks(index, function.id);
        for block in function_blocks {
            if block.reachable != reachable.contains(&block.id) {
                push_cfg_diagnostic(
                    diagnostics,
                    "BasicBlock",
                    interner.resolve(block.stable_key).as_ref(),
                    "reachable",
                    "stored reachable flag disagrees with graph reachability",
                );
            }
        }
    }
}

fn reachable_blocks(
    index: &CfgValidationIndex<'_>,
    function: CfgFunctionId,
) -> BTreeSet<BasicBlockId> {
    let Some(entry) = index
        .blocks_for_function(function)
        .find(|block| block.kind == BasicBlockKind::Entry)
        .map(|block| block.id)
    else {
        return BTreeSet::new();
    };
    let mut seen = BTreeSet::new();
    let mut stack = vec![entry];
    while let Some(block) = stack.pop() {
        if !seen.insert(block) {
            continue;
        }
        stack.extend(
            index
                .normal_successors(function, block)
                .iter()
                .rev()
                .copied(),
        );
    }
    seen
}

fn check_duplicate_stable_keys(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    interner: &crate::internal_core::StableKeyInterner,
    stable_keys: impl Iterator<Item = crate::internal_core::StableKeyId>,
) {
    let mut seen = BTreeSet::new();
    for stable_key in stable_keys {
        if !seen.insert(stable_key) {
            push_cfg_diagnostic(
                diagnostics,
                family,
                interner.resolve(stable_key).as_ref(),
                "stable_key",
                "duplicate stable_key",
            );
        }
    }
}

fn synthetic_block_kind(kind: BasicBlockKind) -> bool {
    matches!(
        kind,
        BasicBlockKind::Entry | BasicBlockKind::ExitNormal | BasicBlockKind::ExitExceptional
    )
}

fn check_optional_node(
    diagnostics: &mut Vec<Diagnostic>,
    nodes: &BTreeMap<CfgNodeId, &crate::analysis_neutral::cfg::facts::CfgNodeFact>,
    node: Option<CfgNodeId>,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
) {
    if let Some(node) = node
        && !nodes.contains_key(&node)
    {
        push_cfg_diagnostic(
            diagnostics,
            family,
            stable_key,
            field,
            "dangling node reference",
        );
    }
}

fn check_edge_node(
    diagnostics: &mut Vec<Diagnostic>,
    nodes: &BTreeMap<CfgNodeId, &crate::analysis_neutral::cfg::facts::CfgNodeFact>,
    node: CfgNodeId,
    function: CfgFunctionId,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
) {
    match nodes.get(&node) {
        Some(row) if row.cfg_function == function => {}
        Some(_) => push_cfg_diagnostic(
            diagnostics,
            family,
            stable_key,
            field,
            "cross-function edge endpoint",
        ),
        None => push_cfg_diagnostic(
            diagnostics,
            family,
            stable_key,
            field,
            "dangling node reference",
        ),
    }
}

fn check_edge_block(
    diagnostics: &mut Vec<Diagnostic>,
    blocks: &BTreeMap<BasicBlockId, &crate::analysis_neutral::cfg::facts::BasicBlockFact>,
    block: BasicBlockId,
    function: CfgFunctionId,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
) {
    match blocks.get(&block) {
        Some(row) if row.cfg_function == function => {}
        Some(_) => push_cfg_diagnostic(
            diagnostics,
            family,
            stable_key,
            field,
            "cross-function edge endpoint",
        ),
        None => push_cfg_diagnostic(
            diagnostics,
            family,
            stable_key,
            field,
            "dangling block reference",
        ),
    }
}

fn check_block_ref(
    diagnostics: &mut Vec<Diagnostic>,
    blocks: &BTreeMap<BasicBlockId, &crate::analysis_neutral::cfg::facts::BasicBlockFact>,
    block: BasicBlockId,
    function: CfgFunctionId,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
) {
    check_edge_block(
        diagnostics,
        blocks,
        block,
        function,
        family,
        stable_key,
        field,
    );
}

fn check_edge_ref(
    diagnostics: &mut Vec<Diagnostic>,
    edges: &BTreeMap<CfgEdgeId, &crate::analysis_neutral::cfg::facts::CfgEdgeFact>,
    edge: CfgEdgeId,
    function: CfgFunctionId,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
) {
    match edges.get(&edge) {
        Some(row) if row.cfg_function == function => {}
        Some(_) => push_cfg_diagnostic(
            diagnostics,
            family,
            stable_key,
            field,
            "cross-function edge reference",
        ),
        None => push_cfg_diagnostic(
            diagnostics,
            family,
            stable_key,
            field,
            "dangling edge reference",
        ),
    }
}

fn push_cfg_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    diagnostics.push(
        Diagnostic::error(
            "polint/internal",
            "<workspace>",
            DiagnosticRange::point(1, 1),
            format!("CFG validation failed for {family} stable key."),
        )
        .with_evidence("family", family)
        .with_evidence("stable_key", stable_key.to_string())
        .with_evidence("field", field)
        .with_evidence("reason", reason),
    );
}
