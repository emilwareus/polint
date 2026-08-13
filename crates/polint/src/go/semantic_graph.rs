//! Go semantic-graph projection.
//!
//! This adapter resolves sidecar Go identities back to source-backed host rows;
//! the graph builder and constraint vocabulary remain owned by polint-analysis.

use std::collections::BTreeMap;

use crate::analysis_api::FunctionFact;
use crate::analysis_neutral::AnalysisHost;
use crate::analysis_neutral::calls::facts::CallSiteFact;
use crate::analysis_neutral::semantic_graph::build::{
    SemanticGraphBuilder, node_key_from_identity,
};
use crate::analysis_neutral::semantic_graph::constraints::ConstraintKind;
use crate::internal_core::Language;

use crate::go::semantic::facts::{
    GoSemanticCallStatus, GoSemanticCallsiteFact, GoSemanticFunctionFact,
};

pub fn project_go_semantic(
    db: &impl AnalysisHost,
    builder: &mut SemanticGraphBuilder,
    semantic_functions: &[GoSemanticFunctionFact],
    semantic_callsites: &[GoSemanticCallsiteFact],
) {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let function_node_by_qualified = semantic_functions
        .iter()
        .filter_map(|semantic_function| {
            let function = matching_core_function(db, semantic_function)?;
            let node = builder.function_node(interner, db, function)?;
            Some((semantic_function.qualified.as_str(), node))
        })
        .collect::<BTreeMap<_, _>>();

    for callsite in semantic_callsites {
        let Some(core_callsite) = matching_core_callsite(db, callsite) else {
            continue;
        };
        let site_key = node_key_from_identity(
            interner,
            "callsite",
            &interner.resolve(core_callsite.stable_key),
        );
        let Some(callsite_node) = builder.node_for_key(interner, &site_key) else {
            continue;
        };
        builder.push_constraint(
            interner,
            ConstraintKind::CallConstraint {
                callsite: callsite_node,
            },
            &interner.resolve(callsite.stable_key),
        );
        if callsite.status != GoSemanticCallStatus::ResolvedStatic {
            continue;
        }
        let Some(static_callee) = callsite.static_callee.as_deref() else {
            continue;
        };
        let Some(&target_node) = function_node_by_qualified.get(static_callee) else {
            continue;
        };
        builder.push_constraint(
            interner,
            ConstraintKind::CopyEdge {
                dst: callsite_node,
                src: target_node,
            },
            &interner.resolve(callsite.stable_key),
        );
    }
}

fn matching_core_function<'a>(
    db: &'a impl AnalysisHost,
    semantic_function: &GoSemanticFunctionFact,
) -> Option<&'a FunctionFact> {
    let file = semantic_function.file?;
    let span = semantic_function.span.as_ref()?;
    db.functions().iter().find(|function| {
        function.file == file
            && function.language == Language::Go
            && function.name == semantic_function.name
            && function.span.start_byte == span.start_byte
            && function.span.end_byte == span.end_byte
    })
}

fn matching_core_callsite<'a>(
    db: &'a impl AnalysisHost,
    semantic_callsite: &GoSemanticCallsiteFact,
) -> Option<&'a CallSiteFact> {
    let file = semantic_callsite.file?;
    let span = semantic_callsite.span.as_ref()?;
    db.call_sites().iter().find(|callsite| {
        callsite.file == file
            && callsite.language == Language::Go
            && callsite.span.start_byte == span.start_byte
            && callsite.span.end_byte == span.end_byte
    })
}
