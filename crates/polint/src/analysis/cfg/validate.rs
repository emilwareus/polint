use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::cfg::facts::{BasicBlockKind, CfgPrecision, CfgStatus};
use crate::analysis::cfg::ids::{BasicBlockId, CfgEdgeId, CfgFunctionId, CfgNodeId};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

pub(crate) fn validate_cfg(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
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

    check_duplicate_stable_keys(
        diagnostics,
        "CfgFunction",
        db.cfg_functions().iter().map(|row| row.stable_key.as_str()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "CfgNode",
        db.cfg_nodes().iter().map(|row| row.stable_key.as_str()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "BasicBlock",
        db.cfg_blocks().iter().map(|row| row.stable_key.as_str()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "CfgEdge",
        db.cfg_edges().iter().map(|row| row.stable_key.as_str()),
    );

    for function in db.cfg_functions() {
        if !nodes.contains_key(&function.entry_node) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgFunction",
                &function.stable_key,
                "entry_node",
                "dangling entry node",
            );
        }
        if !nodes.contains_key(&function.normal_exit_node) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgFunction",
                &function.stable_key,
                "normal_exit_node",
                "dangling normal exit node",
            );
        }
        if let Some(exceptional_exit) = function.exceptional_exit_node
            && !nodes.contains_key(&exceptional_exit)
        {
            push_cfg_diagnostic(
                diagnostics,
                "CfgFunction",
                &function.stable_key,
                "exceptional_exit_node",
                "dangling exceptional exit node",
            );
        }
        if !db.mir_bodies().iter().any(|body| body.id == function.body) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgFunction",
                &function.stable_key,
                "body",
                "dangling MIR body reference",
            );
        }
    }

    for node in db.cfg_nodes() {
        if !functions.contains_key(&node.cfg_function) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgNode",
                &node.stable_key,
                "cfg_function",
                "dangling CFG function reference",
            );
        }
        if !blocks.contains_key(&node.block) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgNode",
                &node.stable_key,
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
                &node.stable_key,
                "span",
                "invalid span byte range",
            );
        }
    }

    for block in db.cfg_blocks() {
        if !functions.contains_key(&block.cfg_function) {
            push_cfg_diagnostic(
                diagnostics,
                "BasicBlock",
                &block.stable_key,
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
                &block.stable_key,
                "node_range",
                "block node ranges are non-empty except synthetic entry/exit blocks",
            );
        }
        check_optional_node(
            diagnostics,
            &nodes,
            block.first_node,
            "BasicBlock",
            &block.stable_key,
            "first_node",
        );
        check_optional_node(
            diagnostics,
            &nodes,
            block.last_node,
            "BasicBlock",
            &block.stable_key,
            "last_node",
        );
    }

    let mut edge_shapes = BTreeSet::new();
    for edge in db.cfg_edges() {
        if !functions.contains_key(&edge.cfg_function) {
            push_cfg_diagnostic(
                diagnostics,
                "CfgEdge",
                &edge.stable_key,
                "cfg_function",
                "dangling CFG function reference",
            );
        }
        check_edge_node(
            diagnostics,
            &nodes,
            edge.from,
            edge.cfg_function,
            "CfgEdge",
            &edge.stable_key,
            "from",
        );
        check_edge_node(
            diagnostics,
            &nodes,
            edge.to,
            edge.cfg_function,
            "CfgEdge",
            &edge.stable_key,
            "to",
        );
        check_edge_block(
            diagnostics,
            &blocks,
            edge.from_block,
            edge.cfg_function,
            "CfgEdge",
            &edge.stable_key,
            "from_block",
        );
        check_edge_block(
            diagnostics,
            &blocks,
            edge.to_block,
            edge.cfg_function,
            "CfgEdge",
            &edge.stable_key,
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
                &edge.stable_key,
                "edge",
                "duplicate identical edges after normalization",
            );
        }
    }

    for row in db.cfg_reachability() {
        check_block_ref(
            diagnostics,
            &blocks,
            row.block,
            row.cfg_function,
            "CfgReachability",
            &row.stable_key,
            "block",
        );
    }
    for row in db.cfg_dominators() {
        check_block_ref(
            diagnostics,
            &blocks,
            row.dominator,
            row.cfg_function,
            "CfgDominator",
            &row.stable_key,
            "dominator",
        );
        check_block_ref(
            diagnostics,
            &blocks,
            row.dominated,
            row.cfg_function,
            "CfgDominator",
            &row.stable_key,
            "dominated",
        );
    }
    for row in db.cfg_postdominators() {
        check_block_ref(
            diagnostics,
            &blocks,
            row.postdominator,
            row.cfg_function,
            "CfgPostDominator",
            &row.stable_key,
            "postdominator",
        );
        check_block_ref(
            diagnostics,
            &blocks,
            row.postdominated,
            row.cfg_function,
            "CfgPostDominator",
            &row.stable_key,
            "postdominated",
        );
    }
    for row in db.cfg_control_dependence() {
        check_edge_ref(
            diagnostics,
            &edges,
            row.controlling_edge,
            row.cfg_function,
            "CfgControlDependence",
            &row.stable_key,
            "controlling_edge",
        );
        check_block_ref(
            diagnostics,
            &blocks,
            row.controlled_block,
            row.cfg_function,
            "CfgControlDependence",
            &row.stable_key,
            "controlled_block",
        );
    }
    for row in db.unsupported_control_flow() {
        if row.construct.is_empty() || row.source_evidence.is_empty() {
            push_cfg_diagnostic(
                diagnostics,
                "UnsupportedControlFlow",
                &row.stable_key,
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
                &row.stable_key,
                "precision",
                "unsupported control-flow rows cannot claim exact resolved precision",
            );
        }
    }
}

fn check_duplicate_stable_keys<'a>(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    stable_keys: impl Iterator<Item = &'a str>,
) {
    let mut seen = BTreeSet::new();
    for stable_key in stable_keys {
        if !seen.insert(stable_key) {
            push_cfg_diagnostic(
                diagnostics,
                family,
                stable_key,
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
    nodes: &BTreeMap<CfgNodeId, &crate::analysis::cfg::facts::CfgNodeFact>,
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
    nodes: &BTreeMap<CfgNodeId, &crate::analysis::cfg::facts::CfgNodeFact>,
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
    blocks: &BTreeMap<BasicBlockId, &crate::analysis::cfg::facts::BasicBlockFact>,
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
    blocks: &BTreeMap<BasicBlockId, &crate::analysis::cfg::facts::BasicBlockFact>,
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
    edges: &BTreeMap<CfgEdgeId, &crate::analysis::cfg::facts::CfgEdgeFact>,
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
            TextRange::point(1, 1),
            format!("CFG validation failed for {family} stable key."),
        )
        .with_evidence("family", family)
        .with_evidence("stable_key", stable_key.to_string())
        .with_evidence("field", field)
        .with_evidence("reason", reason),
    );
}
