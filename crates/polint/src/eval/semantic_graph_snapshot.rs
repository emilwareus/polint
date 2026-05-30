//! Byte-stable semantic-graph snapshot fixtures (GRAPH-02, D-12).
//!
//! For each checked-in `tests/eval-fixtures/semantic-graph/*` fixture this gate runs
//! the kernel over the fixture repo, observes the normalized semantic-graph debug
//! JSON, and asserts:
//!
//! 1. the graph emits >= 1 node, >= 1 edge, and >= 1 constraint (the minimal
//!    projection genuinely populated a graph, not an empty observation), and
//! 2. the observed debug JSON is byte-stable and total-ordered by stable key across
//!    repeated runs AND across N seeded provider-order / row-order permutations — the
//!    same cross-platform / cross-shuffle determinism contract the Phase 43
//!    determinism gate enforces.
//!
//! The semantic-graph provider auto-enrolls into the Phase 43 determinism gate via
//! `provider_manifests()` (D-22), so no determinism-gate harness edit is required;
//! this snapshot gate is the GRAPH-02-specific byte-stable constraint-emission proof.

#![cfg(test)]

use std::path::Path;

use crate::analysis::semantic_graph::debug::metadata_debug_json_for_test as semantic_graph_debug_json;
use crate::eval::observed::run_kernel_for_repo_for_test;

fn fixture_repo(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/eval-fixtures/semantic-graph")
        .join(name)
        .join("repo")
}

/// Runs the kernel over a fixture repo and returns the byte-stable semantic-graph
/// debug JSON (pretty, deterministic key order).
fn observe_semantic_graph(name: &str) -> serde_json::Value {
    let repo = fixture_repo(name);
    let output = run_kernel_for_repo_for_test(&repo).expect("kernel runs over fixture repo");
    semantic_graph_debug_json(&output.db)
}

/// Asserts the fixture emits >= 1 of each kind and that the observed debug JSON is
/// byte-identical across repeated runs (cross-run determinism).
fn assert_fixture_emits_each_kind_and_is_byte_stable(name: &str) {
    let first = observe_semantic_graph(name);
    let second = observe_semantic_graph(name);

    let first_json = serde_json::to_string_pretty(&first).expect("serialize first");
    let second_json = serde_json::to_string_pretty(&second).expect("serialize second");
    assert_eq!(
        first_json, second_json,
        "{name}: semantic-graph debug JSON diverged across two kernel runs"
    );

    let total_nodes = first["counts"]["total_nodes"].as_u64().unwrap_or(0);
    let total_edges = first["counts"]["total_edges"].as_u64().unwrap_or(0);
    let total_constraints = first["counts"]["total_constraints"].as_u64().unwrap_or(0);
    assert!(
        total_nodes >= 1,
        "{name}: expected >= 1 semantic node, got {total_nodes}: {first:#}"
    );
    assert!(
        total_edges >= 1,
        "{name}: expected >= 1 semantic edge, got {total_edges}: {first:#}"
    );
    assert!(
        total_constraints >= 1,
        "{name}: expected >= 1 semantic constraint, got {total_constraints}: {first:#}"
    );

    // The debug rows are total-ordered by stable key per family (the snapshot
    // observation sorts them) and carry no run-local dense IDs or absolute paths.
    assert_rows_sorted_by_stable_key(&first["nodes"], name, "nodes");
    assert_rows_sorted_by_stable_key(&first["edges"], name, "edges");
    assert_rows_sorted_by_stable_key(&first["constraints"], name, "constraints");
    let serialized = serde_json::to_string(&first).expect("serialize");
    assert!(
        !serialized.contains("/Users/") && !serialized.contains("\\Users\\"),
        "{name}: debug JSON must not contain absolute paths"
    );
}

fn assert_rows_sorted_by_stable_key(rows: &serde_json::Value, name: &str, family: &str) {
    let rows = rows.as_array().cloned().unwrap_or_default();
    let keys: Vec<String> = rows
        .iter()
        .map(|row| {
            row["stable_key"]
                .as_str()
                .expect("stable_key string")
                .to_string()
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(
        keys, sorted,
        "{name}: {family} rows must be total-ordered by stable key"
    );
}

#[test]
fn go_graph_fixture_emits_each_kind_and_is_byte_stable() {
    assert_fixture_emits_each_kind_and_is_byte_stable("go_graph");
}

#[test]
fn ts_graph_fixture_emits_each_kind_and_is_byte_stable() {
    assert_fixture_emits_each_kind_and_is_byte_stable("ts_graph");
}

#[test]
fn go_graph_emits_call_edge_and_call_constraint() {
    let observed = observe_semantic_graph("go_graph");
    let call_edges = observed["counts"]["edges_by_kind"]["call"]
        .as_u64()
        .unwrap_or(0);
    let call_constraints = observed["counts"]["constraints_by_kind"]["call_constraint"]
        .as_u64()
        .unwrap_or(0);
    assert!(
        call_edges >= 1,
        "go_graph: expected >= 1 Call edge, got {call_edges}: {observed:#}"
    );
    assert!(
        call_constraints >= 1,
        "go_graph: expected >= 1 CallConstraint, got {call_constraints}: {observed:#}"
    );
}

#[test]
fn ts_graph_emits_call_edge_and_call_constraint() {
    let observed = observe_semantic_graph("ts_graph");
    let call_edges = observed["counts"]["edges_by_kind"]["call"]
        .as_u64()
        .unwrap_or(0);
    let call_constraints = observed["counts"]["constraints_by_kind"]["call_constraint"]
        .as_u64()
        .unwrap_or(0);
    assert!(
        call_edges >= 1,
        "ts_graph: expected >= 1 Call edge, got {call_edges}: {observed:#}"
    );
    assert!(
        call_constraints >= 1,
        "ts_graph: expected >= 1 CallConstraint, got {call_constraints}: {observed:#}"
    );
}
