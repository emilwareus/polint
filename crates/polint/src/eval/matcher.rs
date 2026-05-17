#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::model::{
        AssertionMode, ExpectedDiagnostic, ExpectedFact, ExpectedGraphEdge, ExpectedItem,
        ExpectedPath, ExpectedRuntimeBudget, ObservedDiagnostic, ObservedFact, ObservedGraphEdge,
        ObservedItem, ObservedPath, ObservedRuntimeBudget, ObservedStatus,
    };

    #[test]
    fn eval_matcher_exact_diagnostics_match_on_stable_fingerprint() {
        let matches = match_case(
            &[ExpectedItem::Diagnostic(expected_diagnostic(
                "local/rule",
                "src/main.go",
                Some(10),
                Some("fingerprint-a"),
                AssertionMode::Exact,
            ))],
            &[ObservedItem::Diagnostic(observed_diagnostic(
                "local/rule",
                "src/main.go",
                Some(10),
                Some("fingerprint-a"),
                ObservedStatus::Present,
            ))],
            MatcherConfig::default(),
        );

        assert_eq!(matches[0].outcome, MatchOutcome::TruePositive);
    }

    #[test]
    fn eval_matcher_exact_diagnostics_reject_different_fingerprint() {
        let matches = match_case(
            &[ExpectedItem::Diagnostic(expected_diagnostic(
                "local/rule",
                "src/main.go",
                Some(10),
                Some("fingerprint-a"),
                AssertionMode::Exact,
            ))],
            &[ObservedItem::Diagnostic(observed_diagnostic(
                "local/rule",
                "src/main.go",
                Some(10),
                Some("fingerprint-b"),
                ObservedStatus::Present,
            ))],
            MatcherConfig::default(),
        );

        assert_eq!(
            outcomes(&matches),
            vec![MatchOutcome::FalseNegative, MatchOutcome::FalsePositive]
        );
    }

    #[test]
    fn eval_matcher_tolerant_diagnostics_match_within_line_tolerance() {
        let matches = match_case(
            &[ExpectedItem::Diagnostic(expected_diagnostic(
                "local/rule",
                "src/main.go",
                Some(10),
                None,
                AssertionMode::Tolerant,
            ))],
            &[ObservedItem::Diagnostic(observed_diagnostic(
                "local/rule",
                "src/main.go",
                Some(12),
                None,
                ObservedStatus::Present,
            ))],
            MatcherConfig::default(),
        );

        assert_eq!(matches[0].outcome, MatchOutcome::TruePositive);
    }

    #[test]
    fn eval_matcher_tolerant_diagnostics_reject_outside_line_tolerance() {
        let matches = match_case(
            &[ExpectedItem::Diagnostic(expected_diagnostic(
                "local/rule",
                "src/main.go",
                Some(10),
                None,
                AssertionMode::Tolerant,
            ))],
            &[ObservedItem::Diagnostic(observed_diagnostic(
                "local/rule",
                "src/main.go",
                Some(13),
                None,
                ObservedStatus::Present,
            ))],
            MatcherConfig::default(),
        );

        assert_eq!(
            outcomes(&matches),
            vec![MatchOutcome::FalseNegative, MatchOutcome::FalsePositive]
        );
    }

    #[test]
    fn eval_matcher_forbidden_items_distinguish_hits_from_true_negatives() {
        let forbidden = ExpectedItem::Fact(ExpectedFact {
            family: "extension".to_string(),
            stable_key: "fact:rejected".to_string(),
            mode: AssertionMode::Forbidden,
            producer_id: None,
            precision: None,
            status: None,
            false_positive_trap: false,
        });
        let observed = ObservedItem::Fact(observed_fact("extension", "fact:rejected"));

        assert_eq!(
            match_case(
                std::slice::from_ref(&forbidden),
                std::slice::from_ref(&observed),
                MatcherConfig::default()
            )[0]
                .outcome,
            MatchOutcome::ForbiddenHit
        );
        assert_eq!(
            match_case(&[forbidden], &[], MatcherConfig::default())[0].outcome,
            MatchOutcome::TrueNegative
        );
    }

    #[test]
    fn eval_matcher_false_positive_traps_report_trap_hits() {
        let expected = ExpectedItem::Diagnostic(ExpectedDiagnostic {
            rule_id: "local/noise".to_string(),
            relative_path: "src/lib.ts".to_string(),
            line: Some(4),
            fingerprint: Some("benign".to_string()),
            mode: AssertionMode::Exact,
            false_positive_trap: true,
        });
        let observed = ObservedItem::Diagnostic(observed_diagnostic(
            "local/noise",
            "src/lib.ts",
            Some(4),
            Some("benign"),
            ObservedStatus::Present,
        ));

        assert_eq!(
            match_case(&[expected], &[observed], MatcherConfig::default())[0].outcome,
            MatchOutcome::TrapHit
        );
    }

    #[test]
    fn eval_matcher_partial_graph_and_path_extras_are_unconfirmed() {
        let expected = [
            ExpectedItem::GraphEdge(ExpectedGraphEdge {
                graph: "module".to_string(),
                from: "a".to_string(),
                to: "b".to_string(),
                mode: AssertionMode::Partial,
                partial_truth: true,
            }),
            ExpectedItem::Path(ExpectedPath {
                path_id: "flow".to_string(),
                nodes: vec!["source".to_string(), "sink".to_string()],
                mode: AssertionMode::Partial,
                partial_truth: true,
            }),
        ];
        let observed = [
            ObservedItem::GraphEdge(observed_graph_edge("module", "a", "b")),
            ObservedItem::GraphEdge(observed_graph_edge("module", "a", "c")),
            ObservedItem::Path(observed_path("flow", &["source", "sink"])),
            ObservedItem::Path(observed_path("flow", &["source", "middle", "sink"])),
        ];

        assert_eq!(
            outcomes(&match_case(&expected, &observed, MatcherConfig::default())),
            vec![
                MatchOutcome::TruePositive,
                MatchOutcome::TruePositive,
                MatchOutcome::Unconfirmed,
                MatchOutcome::Unconfirmed,
            ]
        );
    }

    #[test]
    fn eval_matcher_unknown_setup_missing_and_unsupported_statuses_are_preserved() {
        let expected = [
            ExpectedItem::Fact(expected_fact("symbols", "symbol:unknown")),
            ExpectedItem::Fact(expected_fact("symbols", "symbol:setup-missing")),
            ExpectedItem::Fact(expected_fact("symbols", "symbol:unsupported")),
        ];
        let observed = [
            ObservedItem::Fact(observed_fact_with_status(
                "symbols",
                "symbol:unknown",
                ObservedStatus::Unknown,
            )),
            ObservedItem::Fact(observed_fact_with_status(
                "symbols",
                "symbol:setup-missing",
                ObservedStatus::SetupMissing,
            )),
            ObservedItem::Fact(observed_fact_with_status(
                "symbols",
                "symbol:unsupported",
                ObservedStatus::Unsupported,
            )),
        ];

        assert_eq!(
            outcomes(&match_case(&expected, &observed, MatcherConfig::default())),
            vec![
                MatchOutcome::Unknown,
                MatchOutcome::Unknown,
                MatchOutcome::Unknown,
            ]
        );
    }

    #[test]
    fn eval_matcher_runtime_budgets_use_observed_pass_fail_rows() {
        let expected = [
            ExpectedItem::RuntimeBudget(ExpectedRuntimeBudget {
                name: "fast-ci".to_string(),
                max_runtime_ms: 500,
                mode: AssertionMode::Exact,
            }),
            ExpectedItem::RuntimeBudget(ExpectedRuntimeBudget {
                name: "slow-ci".to_string(),
                max_runtime_ms: 800,
                mode: AssertionMode::Tolerant,
            }),
        ];
        let observed = [
            ObservedItem::RuntimeBudget(ObservedRuntimeBudget {
                name: "fast-ci".to_string(),
                budget_passed: true,
                observed_runtime_ms: Some(900),
            }),
            ObservedItem::RuntimeBudget(ObservedRuntimeBudget {
                name: "slow-ci".to_string(),
                budget_passed: false,
                observed_runtime_ms: Some(400),
            }),
        ];

        let matches = match_case(&expected, &observed, MatcherConfig::default());

        assert_eq!(
            outcomes(&matches),
            vec![
                MatchOutcome::RuntimeBudgetPassed,
                MatchOutcome::RuntimeBudgetFailed,
            ]
        );
        assert_eq!(matches[0].expected_runtime_budget_ms, Some(500));
        assert_eq!(matches[1].expected_mode, Some(AssertionMode::Tolerant));
    }

    fn outcomes(matches: &[crate::eval::report::MatchSummary]) -> Vec<MatchOutcome> {
        let mut outcomes: Vec<_> = matches.iter().map(|summary| summary.outcome).collect();
        outcomes.sort();
        outcomes
    }

    fn expected_diagnostic(
        rule_id: &str,
        relative_path: &str,
        line: Option<u32>,
        fingerprint: Option<&str>,
        mode: AssertionMode,
    ) -> ExpectedDiagnostic {
        ExpectedDiagnostic {
            rule_id: rule_id.to_string(),
            relative_path: relative_path.to_string(),
            line,
            fingerprint: fingerprint.map(str::to_string),
            mode,
            false_positive_trap: false,
        }
    }

    fn observed_diagnostic(
        rule_id: &str,
        relative_path: &str,
        line: Option<u32>,
        fingerprint: Option<&str>,
        status: ObservedStatus,
    ) -> ObservedDiagnostic {
        ObservedDiagnostic {
            rule_id: rule_id.to_string(),
            relative_path: relative_path.to_string(),
            line,
            fingerprint: fingerprint.map(str::to_string),
            mode: AssertionMode::Exact,
            producer_id: Some("polint.eval".to_string()),
            provenance: Some("fixture".to_string()),
            precision: Some("exact".to_string()),
            status: Some(status),
        }
    }

    fn expected_fact(family: &str, stable_key: &str) -> ExpectedFact {
        ExpectedFact {
            family: family.to_string(),
            stable_key: stable_key.to_string(),
            mode: AssertionMode::Exact,
            producer_id: None,
            precision: None,
            status: None,
            false_positive_trap: false,
        }
    }

    fn observed_fact(family: &str, stable_key: &str) -> ObservedFact {
        observed_fact_with_status(family, stable_key, ObservedStatus::Present)
    }

    fn observed_fact_with_status(
        family: &str,
        stable_key: &str,
        status: ObservedStatus,
    ) -> ObservedFact {
        ObservedFact {
            family: family.to_string(),
            stable_key: stable_key.to_string(),
            mode: AssertionMode::Exact,
            producer_id: None,
            provenance: None,
            precision: None,
            status: Some(status),
        }
    }

    fn observed_graph_edge(graph: &str, from: &str, to: &str) -> ObservedGraphEdge {
        ObservedGraphEdge {
            graph: graph.to_string(),
            from: from.to_string(),
            to: to.to_string(),
            mode: AssertionMode::Exact,
            partial_truth: false,
            producer_id: None,
            provenance: None,
            precision: None,
            status: Some(ObservedStatus::Present),
        }
    }

    fn observed_path(path_id: &str, nodes: &[&str]) -> ObservedPath {
        ObservedPath {
            path_id: path_id.to_string(),
            nodes: nodes.iter().map(|node| (*node).to_string()).collect(),
            mode: AssertionMode::Exact,
            partial_truth: false,
            producer_id: None,
            provenance: None,
            precision: None,
            status: Some(ObservedStatus::Present),
        }
    }
}
