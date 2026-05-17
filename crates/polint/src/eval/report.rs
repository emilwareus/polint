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
        let baseline = report_with_order(Ordering::Forward);
        let baseline_hash = deterministic_output_hash(&baseline);

        let mut diagnostic_changed = baseline.clone();
        let ObservedItem::Diagnostic(diagnostic) = &mut diagnostic_changed.cases[0].observed[0]
        else {
            panic!("expected diagnostic item");
        };
        diagnostic.fingerprint = Some("changed-fingerprint".to_string());
        assert_ne!(
            baseline_hash,
            deterministic_output_hash(&diagnostic_changed)
        );

        let mut fact_changed = baseline.clone();
        let ObservedItem::Fact(fact) = &mut fact_changed.cases[0].observed[1] else {
            panic!("expected fact item");
        };
        fact.stable_key = "fact:changed".to_string();
        assert_ne!(baseline_hash, deterministic_output_hash(&fact_changed));

        let mut graph_changed = baseline.clone();
        let ObservedItem::GraphEdge(edge) = &mut graph_changed.cases[0].observed[2] else {
            panic!("expected graph edge item");
        };
        edge.to = "module:changed".to_string();
        assert_ne!(baseline_hash, deterministic_output_hash(&graph_changed));

        let mut path_changed = baseline.clone();
        let ObservedItem::Path(path) = &mut path_changed.cases[0].observed[3] else {
            panic!("expected path item");
        };
        path.nodes[1] = "changed-node".to_string();
        assert_ne!(baseline_hash, deterministic_output_hash(&path_changed));

        let mut invariant_changed = baseline.clone();
        let ObservedItem::Invariant(invariant) = &mut invariant_changed.cases[0].observed[4] else {
            panic!("expected invariant item");
        };
        invariant.value = "false".to_string();
        assert_ne!(baseline_hash, deterministic_output_hash(&invariant_changed));

        let mut budget_changed = baseline.clone();
        let ObservedItem::RuntimeBudget(budget) = &mut budget_changed.cases[0].observed[5] else {
            panic!("expected runtime budget item");
        };
        budget.budget_passed = false;
        budget_changed.cases[0].runtime.budget_passed = false;
        assert_ne!(baseline_hash, deterministic_output_hash(&budget_changed));
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
                unknown: 1,
                runtime_budget_passed: 1,
                runtime_budget_failed: 0,
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
                    outcome: "true_positive".to_string(),
                },
                MatchSummary {
                    item_key: "diagnostic:local/rule:src/main.go".to_string(),
                    outcome: "false_positive".to_string(),
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
            }),
            ExpectedItem::Diagnostic(ExpectedDiagnostic {
                rule_id: "local/rule".to_string(),
                relative_path: "src/main.go".to_string(),
                line: Some(7),
                fingerprint: Some("diag-fingerprint".to_string()),
                mode: AssertionMode::Exact,
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
}
