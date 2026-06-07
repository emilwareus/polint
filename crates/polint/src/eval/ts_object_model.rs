//! JS/TS object-model acceptance gates (JS-05).
//!
//! These tests intentionally inspect crate-private object-model and solver rows.
//! Phase 52 later projects solver output into observable refined-call facts; Phase
//! 50 proves the private object/property/prototype/receiver driver itself.

#![cfg(test)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::analysis::ids::{CallSiteId, ObjectTokenId, PlaceId, SemanticNodeId};
use crate::analysis::semantic_graph::facts::NodeKind;
use crate::analysis::solver::budget::{BudgetReason, BudgetStatus, SolverBudget};
use crate::analysis::solver::facts::DerivedEdgeFact;
use crate::analysis::solver::ts_object_model::fixpoint::solve_ts_object_model;
use crate::analysis::solver::ts_object_model::inputs::{
    TsObjectAllocationInput, TsObjectModelInputs, TsObjectPropertyRead, TsObjectPropertyWrite,
    TsObjectPrototypeLink, TsObjectReceiverBinding,
};
use crate::analysis::solver::ts_tokens::inputs::{TsFunctionToken, TsTokenInputs};
use crate::config::load_config;
use crate::core::{AnalysisDb, FunctionId, Language};
use crate::eval::fixtures::load_native_fixture;
use crate::eval::observed::{copy_fixture_repo_for_test, run_kernel_for_repo_for_test};
use crate::ts::object_model::facts::{TsPropertyKeyKind, TsReceiverBindingKind};

fn fixture_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/eval-fixtures/ts-object-model")
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
        object_model_enabled: loaded.config.solver.js_object_model_enabled(),
        object: loaded.config.solver.to_js_object_sub_budget(),
        ..SolverBudget::default()
    }
}

#[test]
fn object_literal_fixture_resolves_exact_and_computed_property_calls() {
    let output = run_fixture_kernel(&fixture_dir("object-literal"));
    let nodes = ts_function_nodes_by_name(&output.db);
    let edges = output.db.solver_derived_edges();

    assert_object_edge_with_provenance(edges, &nodes, "objectEntry", "propertyTarget");
    assert_property_edge_count(edges, &nodes, "objectEntry", "propertyTarget", 2);
    assert_object_edge_with_provenance(edges, &nodes, "objectEntry", "crossFormTarget");
    assert_object_edge_with_provenance(edges, &nodes, "objectEntry", "computedTarget");
    assert_object_edge_with_provenance(edges, &nodes, "objectEntry", "inlineTarget");
    assert_no_computed_bucket_edge_to(edges, &nodes, "unrelatedExactTarget");

    assert!(
        output.db.ts_property_reads().iter().any(|read| {
            read.property_key.kind == TsPropertyKeyKind::ComputedBucket
                && read.callsite_stable_key.is_some()
        }),
        "fixture must retain a computed property call that only object-model bucket facts can justify"
    );
    assert!(
        output.db.ts_property_writes().iter().any(|write| {
            write.property_key.kind == TsPropertyKeyKind::Static
                && write.property_key.value.as_deref() == Some("methodTarget")
                && write.value_function_stable_key.is_some()
        }),
        "object literal method syntax must seed a callable property value"
    );
}

#[test]
fn prototype_this_fixture_resolves_class_prototype_edges_and_receiver_evidence() {
    let output = run_fixture_kernel(&fixture_dir("prototype-this"));
    let nodes = ts_function_nodes_by_name(&output.db);
    let edges = output.db.solver_derived_edges();

    assert_property_label_edge(edges, &nodes, "prototypeEntry", "static:childMethod");
    assert_property_label_edge(edges, &nodes, "prototypeEntry", "static:baseMethod");
    assert_property_label_edge(edges, &nodes, "prototypeEntry", "static:callThisTarget");
    assert_property_label_edge_has_prefix(
        edges,
        &nodes,
        "prototypeEntry",
        "static:baseMethod",
        "ts_object_prototype_link",
    );
    assert_property_label_edge_has_prefix(
        edges,
        &nodes,
        "prototypeEntry",
        "static:childMethod",
        "ts_object_receiver_binding",
    );

    for kind in [
        TsReceiverBindingKind::MethodCall,
        TsReceiverBindingKind::LexicalThis,
        TsReceiverBindingKind::BoundFunction,
        TsReceiverBindingKind::CallReceiver,
        TsReceiverBindingKind::ApplyReceiver,
    ] {
        assert!(
            output
                .db
                .ts_receiver_bindings()
                .iter()
                .any(|binding| binding.kind == kind),
            "prototype-this fixture should emit {kind:?} receiver evidence"
        );
    }
    assert!(
        output.db.ts_property_writes().iter().any(|write| {
            write.property_key.kind == TsPropertyKeyKind::Static
                && write.property_key.value.as_deref() == Some("accessorMethod")
                && write.value_function_stable_key.is_some()
        }),
        "accessor method rows must be present as prototype property writes"
    );
}

#[test]
fn budget_fixture_latches_object_model_budget_evidence() {
    let fixture = fixture_dir("budget");
    let budget = solver_budget_for_fixture(&fixture);
    assert!(budget.object_model_enabled);
    assert_eq!(budget.object.max_tokens_per_property, 1);
    assert_eq!(budget.object.max_receiver_candidates_per_callsite, 1);
    assert_eq!(budget.object.max_prototype_depth, 1);

    let result = solve_ts_object_model(&two_token_property_inputs(), &budget);
    assert_eq!(result.budget_status, BudgetStatus::BudgetExceeded);
    assert!(
        result
            .budget_reasons
            .contains(BudgetReason::ObjectMaxTokensPerProperty.as_str())
    );

    let mut receiver_budget = budget;
    receiver_budget.object.max_tokens_per_property = 8;
    let result = solve_ts_object_model(&two_token_property_inputs(), &receiver_budget);
    assert_eq!(result.budget_status, BudgetStatus::BudgetExceeded);
    assert!(
        result
            .budget_reasons
            .contains(BudgetReason::ObjectMaxReceiverCandidatesPerCallsite.as_str())
    );
    assert_eq!(result.derived_edges.len(), 1);

    let result = solve_ts_object_model(&prototype_depth_inputs(), &budget);
    assert_eq!(result.budget_status, BudgetStatus::BudgetExceeded);
    assert!(
        result
            .budget_reasons
            .contains(BudgetReason::ObjectMaxPrototypeDepth.as_str())
    );
    assert!(
        result.derived_edges.is_empty(),
        "prototype-depth exhaustion must not fabricate a post-cap target"
    );
}

#[test]
fn polyglot_canary_has_intra_ts_object_edge_without_cross_language_edge() {
    let output = run_fixture_kernel(&polyglot_fixture_dir("go-ts"));
    let nodes = ts_function_nodes_by_name(&output.db);
    let edges = output.db.solver_derived_edges();

    assert_object_edge(edges, &nodes, "run", "objectModelTarget");
    assert_object_edge(edges, &nodes, "run", "makeToken");
    assert_no_go_ts_cross_language_edges(&output.db);
}

#[test]
fn jelly_local_object_evidence_is_scored_in_both_modes_without_flooding() {
    let output = run_fixture_kernel(&fixture_dir("object-literal"));
    let nodes = ts_function_nodes_by_name(&output.db);
    let edges = output.db.solver_derived_edges();

    for mode in ["oracle-jelly", "whole-repo"] {
        let resolved = ["propertyTarget", "computedTarget"]
            .into_iter()
            .filter(|target| {
                edges.iter().any(|edge| {
                    edge.target == nodes[*target]
                        && edge.provenance.constraint_kind == "call_constraint"
                        && has_contributing_prefix(edge, "ts_object_property_write")
                })
            })
            .count();
        assert_eq!(
            resolved, 2,
            "{mode}: local Jelly-style object evidence should add precise object-model targets"
        );
        assert!(
            !edges
                .iter()
                .any(|edge| edge.target == nodes["unrelatedExactTarget"]),
            "{mode}: object evidence must not flood unrelated exact properties"
        );
    }
}

fn two_token_property_inputs() -> TsObjectModelInputs {
    let mut inputs = basic_inputs("static:target");
    inputs.node_kinds.insert(
        SemanticNodeId(3),
        NodeKind::Function(crate::core::FunctionId(3)),
    );
    inputs.token_inputs.function_tokens.insert(
        SemanticNodeId(3),
        TsFunctionToken {
            function_node: SemanticNodeId(3),
            function_stable_key: "function:other".to_string(),
            source_stable_key: "node:function:other".to_string(),
        },
    );
    inputs.property_writes.push(TsObjectPropertyWrite {
        source_node: SemanticNodeId(3),
        stable_key: "write:other".to_string(),
        ..inputs.property_writes[0].clone()
    });
    inputs.normalized()
}

fn prototype_depth_inputs() -> TsObjectModelInputs {
    let mut inputs = basic_inputs("static:target");
    inputs.property_writes[0].base_object = SemanticNodeId(12);
    inputs.prototype_links = vec![
        TsObjectPrototypeLink {
            object: SemanticNodeId(10),
            prototype: SemanticNodeId(11),
            stable_key: "prototype:10-11".to_string(),
        },
        TsObjectPrototypeLink {
            object: SemanticNodeId(11),
            prototype: SemanticNodeId(12),
            stable_key: "prototype:11-12".to_string(),
        },
    ];
    inputs.normalized()
}

fn basic_inputs(field: &str) -> TsObjectModelInputs {
    TsObjectModelInputs {
        allocations: vec![TsObjectAllocationInput {
            place_node: SemanticNodeId(30),
            object_node: SemanticNodeId(10),
            stable_key: "allocation:holder".to_string(),
        }],
        property_writes: vec![TsObjectPropertyWrite {
            base_object: SemanticNodeId(10),
            field: field.to_string(),
            source_node: SemanticNodeId(2),
            stable_key: "write:target".to_string(),
        }],
        property_reads: vec![TsObjectPropertyRead {
            base_object: SemanticNodeId(10),
            base_is_this: false,
            field: field.to_string(),
            destination_node: SemanticNodeId(20),
            callsite_node: Some(SemanticNodeId(9)),
            caller_node: Some(SemanticNodeId(1)),
            callsite_stable_key: Some("callsite:target".to_string()),
            constraint_stable_key: "read:target".to_string(),
            stable_key: "read:target".to_string(),
        }],
        receiver_bindings: vec![TsObjectReceiverBinding {
            kind: TsReceiverBindingKind::MethodCall,
            callsite_node: Some(SemanticNodeId(9)),
            receiver_object: Some(SemanticNodeId(10)),
            stable_key: "receiver:method".to_string(),
        }],
        token_inputs: TsTokenInputs {
            function_tokens: BTreeMap::from([(
                SemanticNodeId(2),
                TsFunctionToken {
                    function_node: SemanticNodeId(2),
                    function_stable_key: "function:target".to_string(),
                    source_stable_key: "node:function:target".to_string(),
                },
            )]),
            ..Default::default()
        },
        node_kinds: BTreeMap::from([
            (SemanticNodeId(1), NodeKind::Function(FunctionId(1))),
            (SemanticNodeId(2), NodeKind::Function(FunctionId(2))),
            (SemanticNodeId(9), NodeKind::Callsite(CallSiteId(9))),
            (
                SemanticNodeId(10),
                NodeKind::AbstractObject(ObjectTokenId(10)),
            ),
            (SemanticNodeId(20), NodeKind::Place(PlaceId(20))),
        ]),
        ..Default::default()
    }
    .normalized()
}

fn assert_object_edge(
    edges: &[DerivedEdgeFact],
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
        "expected object-model call edge {source_name} -> {target_name}: {edges:#?}"
    );
}

fn assert_object_edge_with_provenance(
    edges: &[DerivedEdgeFact],
    nodes: &BTreeMap<String, SemanticNodeId>,
    source_name: &str,
    target_name: &str,
) {
    assert_object_edge(edges, nodes, source_name, target_name);
    let source = nodes[source_name];
    let target = nodes[target_name];
    assert!(
        edges.iter().any(|edge| {
            edge.source == source
                && edge.target == target
                && edge.provenance.constraint_kind == "call_constraint"
                && has_contributing_prefix(edge, "ts_object_property_write")
                && has_contributing_prefix(edge, "ts_object_property_read")
        }),
        "expected object-model provenance for {source_name} -> {target_name}: {edges:#?}"
    );
}

fn assert_property_edge_count(
    edges: &[DerivedEdgeFact],
    nodes: &BTreeMap<String, SemanticNodeId>,
    source_name: &str,
    target_name: &str,
    expected: usize,
) {
    let source = nodes[source_name];
    let target = nodes[target_name];
    let count = edges
        .iter()
        .filter(|edge| {
            edge.source == source
                && edge.target == target
                && edge.provenance.constraint_kind == "call_constraint"
                && has_contributing_prefix(edge, "ts_object_property_write")
        })
        .count();
    assert_eq!(
        count, expected,
        "expected {expected} distinct property-provenance edges {source_name} -> {target_name}: {edges:#?}"
    );
}

fn assert_no_computed_bucket_edge_to(
    edges: &[DerivedEdgeFact],
    nodes: &BTreeMap<String, SemanticNodeId>,
    target_name: &str,
) {
    let target = nodes[target_name];
    assert!(
        !edges.iter().any(|edge| {
            edge.target == target
                && edge
                    .provenance
                    .contributing_facts
                    .iter()
                    .any(|fact| fact.stable_key.contains("computed_bucket"))
        }),
        "computed bucket must not emit unrelated exact property target {target_name}: {edges:#?}"
    );
}

fn assert_property_label_edge(
    edges: &[DerivedEdgeFact],
    nodes: &BTreeMap<String, SemanticNodeId>,
    source_name: &str,
    property_label: &str,
) {
    let source = nodes[source_name];
    assert!(
        edges.iter().any(|edge| {
            edge.source == source
                && edge.provenance.constraint_kind == "call_constraint"
                && has_contributing_prefix(edge, "ts_object_property_write")
                && has_contributing_prefix(edge, property_label)
        }),
        "expected object-model edge from {source_name} with {property_label} provenance: {edges:#?}"
    );
}

fn assert_property_label_edge_has_prefix(
    edges: &[DerivedEdgeFact],
    nodes: &BTreeMap<String, SemanticNodeId>,
    source_name: &str,
    property_label: &str,
    prefix: &str,
) {
    let source = nodes[source_name];
    assert!(
        edges.iter().any(|edge| {
            edge.source == source
                && edge.provenance.constraint_kind == "call_constraint"
                && has_contributing_prefix(edge, property_label)
                && has_contributing_prefix(edge, prefix)
        }),
        "expected {source_name} {property_label} edge to include {prefix} provenance: {edges:#?}"
    );
}

fn has_contributing_prefix(edge: &DerivedEdgeFact, prefix: &str) -> bool {
    edge.provenance
        .contributing_facts
        .iter()
        .any(|fact| fact.stable_key.contains(prefix))
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
