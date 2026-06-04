//! JS/TS function-token acceptance gates (JS-04).
//!
//! These tests intentionally inspect crate-private solver rows. Phase 52 later
//! projects solver output into observable refined-call facts; Phase 49 proves the
//! private solver driver itself.

#![cfg(test)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::semantic_graph::build::collect_ts_direct_bindings;
use crate::analysis::semantic_graph::facts::NodeKind;
use crate::analysis::solver::budget::{BudgetStatus, SolverBudget};
use crate::analysis::solver::ts_tokens::fixpoint::{TOO_MANY_TOKENS, solve_ts_tokens};
use crate::analysis::solver::ts_tokens::inputs::{TsTokenCallsite, TsTokenCopyEdge, TsTokenInputs};
use crate::config::load_config;
use crate::core::{AnalysisDb, Language};
use crate::eval::fixtures::load_native_fixture;
use crate::eval::observed::{copy_fixture_repo_for_test, run_kernel_for_repo_for_test};
use crate::ts::binding::facts::{TsDirectBindingReason, TsDirectBindingStatus};

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/eval-fixtures/ts-tokens")
        .join(name)
}

fn polyglot_fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/eval-fixtures/polyglot-canary")
        .join(name)
}

fn run_fixture_kernel(fixture_dir: &Path) -> crate::analysis_kernel::KernelOutput {
    let fixture = load_native_fixture(fixture_dir).expect("fixture loads");
    let temp = copy_fixture_repo_for_test(&fixture).expect("copy fixture repo");
    run_kernel_for_repo_for_test(temp.path()).expect("kernel runs over fixture repo")
}

fn solver_budget_for_fixture(fixture_dir: &Path) -> SolverBudget {
    let fixture = load_native_fixture(fixture_dir).expect("fixture loads");
    let loaded = load_config(&fixture.repo_dir).expect("config loads");
    SolverBudget {
        js: loaded.config.solver.to_js_sub_budget(),
        go: loaded.config.solver.to_go_sub_budget(),
        ..SolverBudget::default()
    }
}

#[test]
fn alias_parameter_return_fixture_proves_source_flow_token_edges() {
    let output = run_fixture_kernel(&fixture_dir("alias-parameter-return"));
    let nodes = ts_function_nodes_by_name(&output.db);
    let edges = output.db.solver_derived_edges();

    assert_token_edge(edges, &nodes, "entry", "aliasTarget");
    assert_token_edge(edges, &nodes, "entry", "assignedTarget");
    assert_token_edge(edges, &nodes, "invokeCallback", "parameterTarget");
    assert_token_edge(edges, &nodes, "entry", "returnTarget");
    assert_token_edge(edges, &nodes, "entry", "closureTarget");

    let direct_bindings = collect_ts_direct_bindings(&output.db);
    assert!(
        direct_bindings.bindings.iter().any(|binding| {
            binding.status == TsDirectBindingStatus::Unresolved
                && binding.reason == Some(TsDirectBindingReason::TokenFlowRequired)
        }),
        "fixture must retain a TokenFlowRequired parameter/return handoff row for future frontend copy-edge producers"
    );
    assert!(
        direct_bindings.bindings.iter().any(|binding| {
            binding.status == TsDirectBindingStatus::Unresolved
                && matches!(
                    binding.reason,
                    Some(
                        TsDirectBindingReason::ComputedProperty
                            | TsDirectBindingReason::PropertyFlowRequired
                            | TsDirectBindingReason::PrototypeModelRequired
                            | TsDirectBindingReason::ThisModelRequired
                    )
                )
        }),
        "property/prototype/this behavior must remain outside the token driver boundary"
    );
    assert!(
        !edges
            .iter()
            .any(|edge| edge.target == nodes["propertyTarget"]),
        "propertyTarget must not be guessed by the function-token driver"
    );
}

#[test]
fn token_explosion_latches_budget_exceeded_and_bounds_output() {
    let fixture = fixture_dir("token-explosion");
    let output = run_fixture_kernel(&fixture);
    let budget = solver_budget_for_fixture(&fixture);
    assert_eq!(budget.js.max_tokens_per_var, 1);

    let inputs = synthetic_explosion_inputs(&output.db);
    let result = solve_ts_tokens(&inputs, &budget);

    assert_eq!(result.budget_status, BudgetStatus::BudgetExceeded);
    assert_eq!(
        result.tokens_by_var[&SemanticNodeId(900_000)].render_stable(),
        TOO_MANY_TOKENS
    );
    assert!(
        result.derived_edges.is_empty(),
        "the too-many-tokens sentinel must never become a callable target"
    );
    assert!(
        result.tokens_by_var.len() <= inputs.function_tokens.len() + 1,
        "token-explosion output should stay bounded to source functions plus one sentinel"
    );
}

#[test]
fn polyglot_canary_has_intra_ts_token_edge_without_cross_language_edge() {
    let output = run_fixture_kernel(&polyglot_fixture_dir("go-ts"));
    let nodes = ts_function_nodes_by_name(&output.db);
    let edges = output.db.solver_derived_edges();

    assert_token_edge(edges, &nodes, "run", "makeToken");
    assert_no_go_ts_cross_language_edges(&output.db);
}

#[test]
fn jelly_local_token_evidence_improves_recall_without_flooding() {
    let output = run_fixture_kernel(&fixture_dir("alias-parameter-return"));
    let nodes = ts_function_nodes_by_name(&output.db);
    let edges = output.db.solver_derived_edges();

    let resolved_targets = ["aliasTarget", "assignedTarget"]
        .into_iter()
        .filter(|target| {
            edges.iter().any(|edge| {
                edge.target == nodes[*target]
                    && edge.provenance.constraint_kind == "call_constraint"
            })
        })
        .count();
    assert_eq!(
        resolved_targets, 2,
        "local Jelly-style token evidence should add two precise token targets from current CopyEdge producers"
    );
    assert!(
        !edges
            .iter()
            .any(|edge| edge.target == nodes["propertyTarget"]),
        "token evidence must not flood into property-model targets"
    );
}

fn synthetic_explosion_inputs(db: &AnalysisDb) -> TsTokenInputs {
    let base = TsTokenInputs::from_db(db);
    let wanted = ts_function_nodes_by_name(db)
        .into_iter()
        .filter(|(name, _)| name.starts_with("token"))
        .map(|(_, node)| node)
        .collect::<BTreeSet<_>>();
    let function_tokens = base
        .function_tokens
        .into_iter()
        .filter(|(node, _)| wanted.contains(node))
        .collect::<BTreeMap<_, _>>();
    assert!(
        function_tokens.len() >= 3,
        "token-explosion fixture must provide at least three token functions"
    );
    let copy_edges = function_tokens
        .keys()
        .map(|node| TsTokenCopyEdge {
            dst: SemanticNodeId(900_000),
            src: *node,
            stable_key: format!("synthetic:token-explosion:{}->sink", node.0),
        })
        .collect();

    TsTokenInputs {
        function_tokens,
        copy_edges,
        callsites: vec![TsTokenCallsite {
            caller_node: SemanticNodeId(900_001),
            callsite_node: SemanticNodeId(900_000),
            callsite_stable_key: "synthetic:token-explosion:callsite".to_string(),
            constraint_stable_key: "synthetic:token-explosion:constraint".to_string(),
        }],
        handoffs: Vec::new(),
    }
    .normalized()
}

fn assert_token_edge(
    edges: &[crate::analysis::solver::facts::DerivedEdgeFact],
    nodes: &BTreeMap<String, SemanticNodeId>,
    source_name: &str,
    target_name: &str,
) {
    let source = nodes
        .get(source_name)
        .unwrap_or_else(|| panic!("source function {source_name} should exist"));
    let target = nodes
        .get(target_name)
        .unwrap_or_else(|| panic!("target function {target_name} should exist"));
    assert!(
        edges.iter().any(|edge| {
            edge.source == *source
                && edge.target == *target
                && edge.provenance.constraint_kind == "call_constraint"
                && edge.honors_precision_ceiling()
        }),
        "expected TS token call edge {source_name} -> {target_name}: {edges:#?}"
    );
}

fn ts_function_nodes_by_name(db: &AnalysisDb) -> BTreeMap<String, SemanticNodeId> {
    let function_by_id = db
        .functions()
        .iter()
        .filter(|function| function.language.is_ts_family())
        .map(|function| (function.id, function.name.clone()))
        .collect::<BTreeMap<_, _>>();
    db.semantic_nodes()
        .iter()
        .filter_map(|node| {
            let NodeKind::Function(function_id) = node.kind else {
                return None;
            };
            let name = function_by_id.get(&function_id)?;
            Some((name.clone(), node.id))
        })
        .collect()
}

fn assert_no_go_ts_cross_language_edges(db: &AnalysisDb) {
    let node_language = |node_id: SemanticNodeId| -> Option<Language> {
        db.semantic_nodes().iter().find_map(|node| {
            if node.id != node_id {
                return None;
            }
            match node.kind {
                NodeKind::Function(function_id) => db
                    .functions()
                    .iter()
                    .find(|function| function.id == function_id)
                    .map(|function| function.language),
                _ => None,
            }
        })
    };

    for edge in db.solver_derived_edges() {
        let source = node_language(edge.source);
        let target = node_language(edge.target);
        assert!(
            !matches!(
                (source, target),
                (Some(Language::Go), Some(language)) if language.is_ts_family()
            ) && !matches!(
                (source, target),
                (Some(language), Some(Language::Go)) if language.is_ts_family()
            ),
            "no solver edge may cross Go<->TS language boundary: {edge:#?}"
        );
    }
}
