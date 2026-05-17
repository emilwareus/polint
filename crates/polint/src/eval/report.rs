use serde::{Deserialize, Serialize};

use crate::cache::stable_hash;
use crate::eval::matcher::{MatchItemKind, MatchOutcome};
use crate::eval::model::{
    AssertionMode, ExpectedFact, ExpectedGraphEdge, ExpectedInvariant, ExpectedItem, ExpectedPath,
    ExpectedRuntimeBudget, FixtureArea, ObservedFact, ObservedGraphEdge, ObservedInvariant,
    ObservedItem, ObservedPath, ObservedRuntimeBudget, ObservedStatus,
};

pub(crate) const EVALUATION_SCHEMA_VERSION: &str = "polint-eval-internal-1";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct EvaluationRun {
    pub(crate) schema_version: String,
    pub(crate) suite_id: String,
    pub(crate) cases: Vec<CaseResult>,
    pub(crate) metrics: MetricSummary,
    pub(crate) output_hash: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct CaseResult {
    pub(crate) case_id: String,
    pub(crate) area: FixtureArea,
    pub(crate) expected: Vec<ExpectedItem>,
    pub(crate) observed: Vec<ObservedItem>,
    pub(crate) matches: Vec<MatchSummary>,
    pub(crate) runtime: RuntimeObservation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct RuntimeObservation {
    pub(crate) budget_name: String,
    pub(crate) budget_passed: bool,
    pub(crate) observed_runtime_ms: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct MetricSummary {
    pub(crate) true_positives: u64,
    pub(crate) false_positives: u64,
    pub(crate) false_negatives: u64,
    pub(crate) true_negatives: u64,
    pub(crate) unconfirmed: u64,
    pub(crate) false_positive_trap_hits: u64,
    pub(crate) forbidden_hits: u64,
    pub(crate) unknown_count: u64,
    pub(crate) facts_present: u64,
    pub(crate) facts_accepted: u64,
    pub(crate) facts_rejected: u64,
    pub(crate) graph_edges_expected: u64,
    pub(crate) graph_edges_observed: u64,
    pub(crate) graph_edges_unconfirmed: u64,
    pub(crate) paths_expected: u64,
    pub(crate) paths_observed: u64,
    pub(crate) paths_unconfirmed: u64,
    pub(crate) runtime_budget_passed: u64,
    pub(crate) runtime_budget_failed: u64,
    pub(crate) precision: Option<f64>,
    pub(crate) recall: Option<f64>,
    pub(crate) f1: Option<f64>,
    pub(crate) f2: Option<f64>,
    pub(crate) f3: Option<f64>,
    pub(crate) false_positive_rate: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct MatchSummary {
    pub(crate) item_key: String,
    pub(crate) outcome: MatchOutcome,
    pub(crate) item_kind: MatchItemKind,
    pub(crate) expected_key: Option<String>,
    pub(crate) observed_key: Option<String>,
    pub(crate) observed_status: Option<ObservedStatus>,
    pub(crate) expected_runtime_budget_ms: Option<u64>,
    pub(crate) expected_mode: Option<AssertionMode>,
    pub(crate) observed_runtime_ms: Option<u64>,
}

pub(crate) fn normalize_run(run: &EvaluationRun) -> EvaluationRun {
    let mut normalized = run.clone();
    normalized.schema_version = EVALUATION_SCHEMA_VERSION.to_string();
    normalized.cases.sort_by(|left, right| {
        (left.area, left.case_id.as_str()).cmp(&(right.area, right.case_id.as_str()))
    });
    for case in &mut normalized.cases {
        case.expected.sort_by_key(expected_item_key);
        case.observed.sort_by_key(observed_item_key);
        case.matches.sort_by(|left, right| {
            (
                left.item_key.as_str(),
                left.outcome,
                left.item_kind,
                left.expected_key.as_deref(),
                left.observed_key.as_deref(),
                left.observed_status,
                left.expected_runtime_budget_ms,
                left.expected_mode,
                left.observed_runtime_ms,
            )
                .cmp(&(
                    right.item_key.as_str(),
                    right.outcome,
                    right.item_kind,
                    right.expected_key.as_deref(),
                    right.observed_key.as_deref(),
                    right.observed_status,
                    right.expected_runtime_budget_ms,
                    right.expected_mode,
                    right.observed_runtime_ms,
                ))
        });
    }
    normalized
}

pub(crate) fn to_deterministic_json_pretty(run: &EvaluationRun) -> String {
    let mut normalized = normalize_run(run);
    normalized.output_hash = deterministic_output_hash(&normalized);
    serde_json::to_string_pretty(&normalized).unwrap_or_else(|_| "{}".to_string())
}

pub(crate) fn deterministic_output_hash(run: &EvaluationRun) -> String {
    let mut normalized = normalize_run(run);
    normalized.output_hash.clear();
    for case in &mut normalized.cases {
        case.runtime.observed_runtime_ms = None;
        for item in &mut case.observed {
            if let ObservedItem::RuntimeBudget(budget) = item {
                budget.observed_runtime_ms = None;
            }
        }
        for summary in &mut case.matches {
            summary.observed_runtime_ms = None;
        }
    }
    let canonical_json =
        serde_json::to_string_pretty(&normalized).unwrap_or_else(|_| "{}".to_string());
    stable_hash(&[canonical_json.as_str()])
}

fn expected_item_key(item: &ExpectedItem) -> String {
    match item {
        ExpectedItem::Diagnostic(diagnostic) => stable_key_parts(&[
            "diagnostic",
            &diagnostic.rule_id,
            &diagnostic.relative_path,
            &option_u32_key(diagnostic.line),
            &option_str_key(diagnostic.fingerprint.as_deref()),
            mode_key(diagnostic.mode),
        ]),
        ExpectedItem::Fact(fact) => expected_fact_key(fact),
        ExpectedItem::GraphEdge(edge) => expected_graph_edge_key(edge),
        ExpectedItem::Path(path) => expected_path_key(path),
        ExpectedItem::Invariant(invariant) => expected_invariant_key(invariant),
        ExpectedItem::RuntimeBudget(budget) => expected_runtime_budget_key(budget),
    }
}

fn observed_item_key(item: &ObservedItem) -> String {
    match item {
        ObservedItem::Diagnostic(diagnostic) => stable_key_parts(&[
            "diagnostic",
            &diagnostic.rule_id,
            &diagnostic.relative_path,
            &option_u32_key(diagnostic.line),
            &option_str_key(diagnostic.fingerprint.as_deref()),
            mode_key(diagnostic.mode),
            &option_str_key(diagnostic.producer_id.as_deref()),
            &option_status_key(diagnostic.status),
        ]),
        ObservedItem::Fact(fact) => observed_fact_key(fact),
        ObservedItem::GraphEdge(edge) => observed_graph_edge_key(edge),
        ObservedItem::Path(path) => observed_path_key(path),
        ObservedItem::Invariant(invariant) => observed_invariant_key(invariant),
        ObservedItem::RuntimeBudget(budget) => observed_runtime_budget_key(budget),
    }
}

fn expected_fact_key(fact: &ExpectedFact) -> String {
    stable_key_parts(&[
        "fact",
        &fact.family,
        &fact.stable_key,
        mode_key(fact.mode),
        &option_str_key(fact.producer_id.as_deref()),
        &option_str_key(fact.precision.as_deref()),
        &option_status_key(fact.status),
    ])
}

fn observed_fact_key(fact: &ObservedFact) -> String {
    stable_key_parts(&[
        "fact",
        &fact.family,
        &fact.stable_key,
        mode_key(fact.mode),
        &option_str_key(fact.producer_id.as_deref()),
        &option_str_key(fact.precision.as_deref()),
        &option_status_key(fact.status),
    ])
}

fn expected_graph_edge_key(edge: &ExpectedGraphEdge) -> String {
    stable_key_parts(&[
        "graph_edge",
        &edge.graph,
        &edge.from,
        &edge.to,
        mode_key(edge.mode),
        bool_key(edge.partial_truth),
    ])
}

fn observed_graph_edge_key(edge: &ObservedGraphEdge) -> String {
    stable_key_parts(&[
        "graph_edge",
        &edge.graph,
        &edge.from,
        &edge.to,
        mode_key(edge.mode),
        bool_key(edge.partial_truth),
        &option_str_key(edge.producer_id.as_deref()),
        &option_status_key(edge.status),
    ])
}

fn expected_path_key(path: &ExpectedPath) -> String {
    stable_key_parts(&[
        "path",
        &path.path_id,
        &list_key(&path.nodes),
        mode_key(path.mode),
        bool_key(path.partial_truth),
    ])
}

fn observed_path_key(path: &ObservedPath) -> String {
    stable_key_parts(&[
        "path",
        &path.path_id,
        &list_key(&path.nodes),
        mode_key(path.mode),
        bool_key(path.partial_truth),
        &option_str_key(path.producer_id.as_deref()),
        &option_status_key(path.status),
    ])
}

fn expected_invariant_key(invariant: &ExpectedInvariant) -> String {
    stable_key_parts(&[
        "invariant",
        &invariant.name,
        &invariant.value,
        mode_key(invariant.mode),
    ])
}

fn observed_invariant_key(invariant: &ObservedInvariant) -> String {
    stable_key_parts(&[
        "invariant",
        &invariant.name,
        &invariant.value,
        mode_key(invariant.mode),
        &option_str_key(invariant.producer_id.as_deref()),
        &option_status_key(invariant.status),
    ])
}

fn expected_runtime_budget_key(budget: &ExpectedRuntimeBudget) -> String {
    stable_key_parts(&[
        "runtime_budget",
        &budget.name,
        &budget.max_runtime_ms.to_string(),
        mode_key(budget.mode),
    ])
}

fn observed_runtime_budget_key(budget: &ObservedRuntimeBudget) -> String {
    stable_key_parts(&[
        "runtime_budget",
        &budget.name,
        bool_key(budget.budget_passed),
    ])
}

fn stable_key_parts(parts: &[&str]) -> String {
    let mut key = String::new();
    for part in parts {
        key.push_str(&part.len().to_string());
        key.push(':');
        key.push_str(part);
        key.push('|');
    }
    key
}

fn list_key(parts: &[String]) -> String {
    let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
    stable_key_parts(&refs)
}

fn option_str_key(value: Option<&str>) -> String {
    value.unwrap_or("").to_string()
}

fn option_u32_key(value: Option<u32>) -> String {
    value.map_or_else(String::new, |value| value.to_string())
}

fn option_status_key(value: Option<ObservedStatus>) -> String {
    value.map_or("", status_key).to_string()
}

fn bool_key(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn mode_key(mode: AssertionMode) -> &'static str {
    match mode {
        AssertionMode::Exact => "exact",
        AssertionMode::Tolerant => "tolerant",
        AssertionMode::Partial => "partial",
        AssertionMode::Forbidden => "forbidden",
    }
}

fn status_key(status: ObservedStatus) -> &'static str {
    match status {
        ObservedStatus::Present => "present",
        ObservedStatus::Unknown => "unknown",
        ObservedStatus::SetupMissing => "setup_missing",
        ObservedStatus::Unsupported => "unsupported",
        ObservedStatus::Rejected => "rejected",
        ObservedStatus::Accepted => "accepted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::model::{
        AssertionMode, ExpectedDiagnostic, ExpectedFact, ExpectedGraphEdge, ExpectedInvariant,
        ExpectedItem, ExpectedPath, ExpectedRuntimeBudget, FixtureArea, ObservedDiagnostic,
        ObservedFact, ObservedGraphEdge, ObservedInvariant, ObservedItem, ObservedPath,
        ObservedRuntimeBudget, ObservedStatus,
    };

    #[test]
    fn eval_report_normalization_makes_json_order_independent() {
        let left = to_deterministic_json_pretty(&report_with_order(Ordering::Forward));
        let right = to_deterministic_json_pretty(&report_with_order(Ordering::Reverse));

        assert_eq!(left, right);
    }

    #[test]
    fn eval_report_hash_excludes_runtime_and_machine_local_fields_by_shape() {
        let mut left = report_with_order(Ordering::Forward);
        let mut right = left.clone();
        left.cases[0].runtime.observed_runtime_ms = Some(17);
        right.cases[0].runtime.observed_runtime_ms = Some(999);

        assert_eq!(
            deterministic_output_hash(&left),
            deterministic_output_hash(&right)
        );

        let json = to_deterministic_json_pretty(&left);
        for forbidden in [
            "started_at",
            "finished_at",
            "absolute_root",
            "temp_root",
            "SystemTime",
            "Instant",
            "0x",
            "/var/folders",
            "/tmp/polint",
            "elapsed",
        ] {
            assert!(
                !json.contains(forbidden),
                "{forbidden} leaked into eval JSON"
            );
        }
    }

    #[test]
    fn eval_report_hash_changes_when_semantic_output_changes() {
        let reference = report_with_order(Ordering::Forward);
        let reference_hash = deterministic_output_hash(&reference);

        let mut diagnostic_changed = reference.clone();
        let diagnostic = observed_diagnostic_mut(&mut diagnostic_changed);
        diagnostic.fingerprint = Some("changed-fingerprint".to_string());
        assert_ne!(
            reference_hash,
            deterministic_output_hash(&diagnostic_changed)
        );

        let mut fact_changed = reference.clone();
        let fact = observed_fact_mut(&mut fact_changed);
        fact.stable_key = "fact:changed".to_string();
        assert_ne!(reference_hash, deterministic_output_hash(&fact_changed));

        let mut graph_changed = reference.clone();
        let edge = observed_graph_edge_mut(&mut graph_changed);
        edge.to = "module:changed".to_string();
        assert_ne!(reference_hash, deterministic_output_hash(&graph_changed));

        let mut path_changed = reference.clone();
        let path = observed_path_mut(&mut path_changed);
        path.nodes[1] = "changed-node".to_string();
        assert_ne!(reference_hash, deterministic_output_hash(&path_changed));

        let mut invariant_changed = reference.clone();
        let invariant = observed_invariant_mut(&mut invariant_changed);
        invariant.value = "false".to_string();
        assert_ne!(
            reference_hash,
            deterministic_output_hash(&invariant_changed)
        );

        let mut budget_changed = reference;
        let budget = observed_runtime_budget_mut(&mut budget_changed);
        budget.budget_passed = false;
        budget_changed.cases[0].runtime.budget_passed = false;
        assert_ne!(reference_hash, deterministic_output_hash(&budget_changed));
    }

    #[test]
    fn eval_report_json_contains_internal_schema_and_no_transient_fields() {
        let json = to_deterministic_json_pretty(&report_with_order(Ordering::Reverse));

        assert!(json.contains("\"schema_version\": \"polint-eval-internal-1\""));
        for forbidden in [
            "started_at",
            "finished_at",
            "absolute_root",
            "temp_root",
            "SystemTime",
            "Instant",
            "0x",
            "as_ptr",
            "{:p}",
        ] {
            assert!(
                !json.contains(forbidden),
                "{forbidden} leaked into eval JSON"
            );
        }
    }

    #[derive(Clone, Copy)]
    enum Ordering {
        Forward,
        Reverse,
    }

    fn report_with_order(ordering: Ordering) -> EvaluationRun {
        let mut cases = vec![
            case_result("case-b", FixtureArea::Graphs),
            case_result("case-a", FixtureArea::Kernel),
        ];
        if matches!(ordering, Ordering::Reverse) {
            cases.reverse();
            cases.iter_mut().for_each(reverse_case_vectors);
        }
        EvaluationRun {
            schema_version: "polint-eval-internal-1".to_string(),
            suite_id: "deterministic-suite".to_string(),
            cases,
            metrics: MetricSummary {
                true_positives: 3,
                false_positives: 1,
                false_negatives: 2,
                true_negatives: 5,
                unconfirmed: 1,
                false_positive_trap_hits: 1,
                forbidden_hits: 1,
                unknown_count: 1,
                facts_present: 1,
                facts_accepted: 0,
                facts_rejected: 0,
                graph_edges_expected: 1,
                graph_edges_observed: 2,
                graph_edges_unconfirmed: 1,
                paths_expected: 1,
                paths_observed: 1,
                paths_unconfirmed: 1,
                runtime_budget_passed: 1,
                runtime_budget_failed: 0,
                precision: Some(0.75),
                recall: Some(0.6),
                f1: Some(2.0 * 0.75 * 0.6 / (0.75 + 0.6)),
                f2: Some(5.0 * 0.75 * 0.6 / (4.0 * 0.75 + 0.6)),
                f3: Some(10.0 * 0.75 * 0.6 / (9.0 * 0.75 + 0.6)),
                false_positive_rate: Some(1.0 / 6.0),
            },
            output_hash: String::new(),
        }
    }

    fn reverse_case_vectors(case: &mut CaseResult) {
        case.expected.reverse();
        case.observed.reverse();
        case.matches.reverse();
    }

    fn case_result(case_id: &str, area: FixtureArea) -> CaseResult {
        CaseResult {
            case_id: case_id.to_string(),
            area,
            expected: expected_items(),
            observed: observed_items(),
            matches: vec![
                MatchSummary {
                    item_key: "fact:module:handler".to_string(),
                    outcome: MatchOutcome::TruePositive,
                    item_kind: MatchItemKind::Fact,
                    expected_key: Some("fact:symbols:fact:module:handler".to_string()),
                    observed_key: Some("fact:symbols:fact:module:handler".to_string()),
                    observed_status: Some(ObservedStatus::Present),
                    expected_runtime_budget_ms: None,
                    expected_mode: Some(AssertionMode::Exact),
                    observed_runtime_ms: None,
                },
                MatchSummary {
                    item_key: "diagnostic:local/rule:src/main.go".to_string(),
                    outcome: MatchOutcome::FalsePositive,
                    item_kind: MatchItemKind::Diagnostic,
                    expected_key: None,
                    observed_key: Some(
                        "diagnostic:local/rule:src/main.go:diag-fingerprint".to_string(),
                    ),
                    observed_status: Some(ObservedStatus::Present),
                    expected_runtime_budget_ms: None,
                    expected_mode: None,
                    observed_runtime_ms: Some(42),
                },
            ],
            runtime: RuntimeObservation {
                budget_name: "fast-ci".to_string(),
                budget_passed: true,
                observed_runtime_ms: Some(42),
            },
        }
    }

    fn expected_items() -> Vec<ExpectedItem> {
        vec![
            ExpectedItem::RuntimeBudget(ExpectedRuntimeBudget {
                name: "fast-ci".to_string(),
                max_runtime_ms: 500,
                mode: AssertionMode::Exact,
            }),
            ExpectedItem::Invariant(ExpectedInvariant {
                name: "provider_order_stable".to_string(),
                value: "true".to_string(),
                mode: AssertionMode::Exact,
            }),
            ExpectedItem::Path(ExpectedPath {
                path_id: "handler-to-sink".to_string(),
                nodes: vec!["handler".to_string(), "sink".to_string()],
                mode: AssertionMode::Partial,
                partial_truth: true,
            }),
            ExpectedItem::GraphEdge(ExpectedGraphEdge {
                graph: "module".to_string(),
                from: "module:handler".to_string(),
                to: "module:sink".to_string(),
                mode: AssertionMode::Tolerant,
                partial_truth: true,
            }),
            ExpectedItem::Fact(ExpectedFact {
                family: "symbols".to_string(),
                stable_key: "fact:module:handler".to_string(),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.symbol_graph".to_string()),
                precision: Some("syntactic".to_string()),
                status: Some(ObservedStatus::Present),
                false_positive_trap: false,
            }),
            ExpectedItem::Diagnostic(ExpectedDiagnostic {
                rule_id: "local/rule".to_string(),
                relative_path: "src/main.go".to_string(),
                line: Some(7),
                fingerprint: Some("diag-fingerprint".to_string()),
                mode: AssertionMode::Exact,
                false_positive_trap: false,
            }),
        ]
    }

    fn observed_items() -> Vec<ObservedItem> {
        vec![
            ObservedItem::RuntimeBudget(ObservedRuntimeBudget {
                name: "fast-ci".to_string(),
                budget_passed: true,
                observed_runtime_ms: Some(42),
            }),
            ObservedItem::Invariant(ObservedInvariant {
                name: "provider_order_stable".to_string(),
                value: "true".to_string(),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.eval".to_string()),
                provenance: Some("fixture".to_string()),
                precision: Some("exact".to_string()),
                status: Some(ObservedStatus::Accepted),
            }),
            ObservedItem::Path(ObservedPath {
                path_id: "handler-to-sink".to_string(),
                nodes: vec!["handler".to_string(), "sink".to_string()],
                mode: AssertionMode::Partial,
                partial_truth: true,
                producer_id: Some("polint.paths".to_string()),
                provenance: Some("derived".to_string()),
                precision: Some("partial".to_string()),
                status: Some(ObservedStatus::Unknown),
            }),
            ObservedItem::GraphEdge(ObservedGraphEdge {
                graph: "module".to_string(),
                from: "module:handler".to_string(),
                to: "module:sink".to_string(),
                mode: AssertionMode::Tolerant,
                partial_truth: true,
                producer_id: Some("polint.module_graph".to_string()),
                provenance: Some("derived".to_string()),
                precision: Some("syntactic".to_string()),
                status: Some(ObservedStatus::Present),
            }),
            ObservedItem::Fact(ObservedFact {
                family: "symbols".to_string(),
                stable_key: "fact:module:handler".to_string(),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.symbol_graph".to_string()),
                provenance: Some("metadata-sidecar".to_string()),
                precision: Some("syntactic".to_string()),
                status: Some(ObservedStatus::Present),
            }),
            ObservedItem::Diagnostic(ObservedDiagnostic {
                rule_id: "local/rule".to_string(),
                relative_path: "src/main.go".to_string(),
                line: Some(7),
                fingerprint: Some("diag-fingerprint".to_string()),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.eval".to_string()),
                provenance: Some("fixture".to_string()),
                precision: Some("exact".to_string()),
                status: Some(ObservedStatus::Present),
            }),
        ]
    }

    fn observed_diagnostic_mut(run: &mut EvaluationRun) -> &mut ObservedDiagnostic {
        run.cases[0]
            .observed
            .iter_mut()
            .find_map(|item| match item {
                ObservedItem::Diagnostic(diagnostic) => Some(diagnostic),
                _ => None,
            })
            .expect("expected diagnostic item")
    }

    fn observed_fact_mut(run: &mut EvaluationRun) -> &mut ObservedFact {
        run.cases[0]
            .observed
            .iter_mut()
            .find_map(|item| match item {
                ObservedItem::Fact(fact) => Some(fact),
                _ => None,
            })
            .expect("expected fact item")
    }

    fn observed_graph_edge_mut(run: &mut EvaluationRun) -> &mut ObservedGraphEdge {
        run.cases[0]
            .observed
            .iter_mut()
            .find_map(|item| match item {
                ObservedItem::GraphEdge(edge) => Some(edge),
                _ => None,
            })
            .expect("expected graph edge item")
    }

    fn observed_path_mut(run: &mut EvaluationRun) -> &mut ObservedPath {
        run.cases[0]
            .observed
            .iter_mut()
            .find_map(|item| match item {
                ObservedItem::Path(path) => Some(path),
                _ => None,
            })
            .expect("expected path item")
    }

    fn observed_invariant_mut(run: &mut EvaluationRun) -> &mut ObservedInvariant {
        run.cases[0]
            .observed
            .iter_mut()
            .find_map(|item| match item {
                ObservedItem::Invariant(invariant) => Some(invariant),
                _ => None,
            })
            .expect("expected invariant item")
    }

    fn observed_runtime_budget_mut(run: &mut EvaluationRun) -> &mut ObservedRuntimeBudget {
        run.cases[0]
            .observed
            .iter_mut()
            .find_map(|item| match item {
                ObservedItem::RuntimeBudget(budget) => Some(budget),
                _ => None,
            })
            .expect("expected runtime budget item")
    }
}
