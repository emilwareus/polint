#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::matcher::{match_case, MatchItemKind, MatchOutcome, MatcherConfig};
    use crate::eval::model::{
        AssertionMode, ExpectedRuntimeBudget, ExpectedItem, ObservedItem, ObservedRuntimeBudget,
    };
    use crate::eval::report::{MatchSummary, MetricSummary};

    #[test]
    fn eval_metrics_count_mixed_match_outcomes() {
        let metrics = compute_metrics(&[
            summary(MatchOutcome::TruePositive, MatchItemKind::Diagnostic, true, true),
            summary(MatchOutcome::FalsePositive, MatchItemKind::Diagnostic, false, true),
            summary(MatchOutcome::FalseNegative, MatchItemKind::Fact, true, false),
            summary(MatchOutcome::TrueNegative, MatchItemKind::Fact, true, false),
            summary(MatchOutcome::Unconfirmed, MatchItemKind::GraphEdge, false, true),
            summary(MatchOutcome::TrapHit, MatchItemKind::Diagnostic, true, true),
            summary(MatchOutcome::ForbiddenHit, MatchItemKind::Fact, true, true),
            summary(MatchOutcome::Unknown, MatchItemKind::Path, true, true),
            summary(
                MatchOutcome::RuntimeBudgetPassed,
                MatchItemKind::RuntimeBudget,
                true,
                true,
            ),
            summary(
                MatchOutcome::RuntimeBudgetFailed,
                MatchItemKind::RuntimeBudget,
                true,
                true,
            ),
            summary(MatchOutcome::TruePositive, MatchItemKind::GraphEdge, true, true),
            summary(MatchOutcome::FalseNegative, MatchItemKind::Path, true, false),
            summary(MatchOutcome::Unconfirmed, MatchItemKind::Path, false, true),
        ]);

        assert_eq!(
            metrics,
            ComputedMetrics {
                true_positives: 2,
                false_positives: 1,
                false_negatives: 2,
                true_negatives: 1,
                unconfirmed: 2,
                false_positive_trap_hits: 1,
                forbidden_hits: 1,
                unknown_count: 1,
                graph_edges_expected: 1,
                graph_edges_observed: 2,
                graph_edges_unconfirmed: 1,
                paths_expected: 2,
                paths_observed: 2,
                paths_unconfirmed: 1,
                runtime_budget_passed: 1,
                runtime_budget_failed: 1,
                precision: Some(2.0 / 3.0),
                recall: Some(0.5),
                f1: Some(4.0 / 7.0),
                f2: Some(10.0 / 19.0),
                f3: Some(20.0 / 39.0),
                false_positive_rate: Some(0.5),
            }
        );
    }

    #[test]
    fn eval_metrics_zero_denominators_are_none() {
        let metrics = compute_metrics(&[]);

        assert_eq!(metrics.precision, None);
        assert_eq!(metrics.recall, None);
        assert_eq!(metrics.f1, None);
        assert_eq!(metrics.f2, None);
        assert_eq!(metrics.f3, None);
        assert_eq!(metrics.false_positive_rate, None);
    }

    #[test]
    fn eval_metrics_serialization_is_byte_stable_with_deterministic_field_order() {
        let metrics = compute_metrics(&[
            summary(MatchOutcome::TruePositive, MatchItemKind::Diagnostic, true, true),
            summary(MatchOutcome::FalsePositive, MatchItemKind::Diagnostic, false, true),
        ]);

        let left = serde_json::to_string(&metrics).unwrap();
        let right = serde_json::to_string(&metrics).unwrap();

        assert_eq!(left, right);
        assert!(left.starts_with("{\"true_positives\":"));
        assert!(
            left.find("\"false_positives\"").unwrap()
                < left.find("\"false_negatives\"").unwrap()
        );
        assert!(
            left.find("\"runtime_budget_failed\"").unwrap() < left.find("\"precision\"").unwrap()
        );
    }

    #[test]
    fn eval_metrics_runtime_budget_counts_follow_observed_budget_passed() {
        let matches = match_case(
            &[
                ExpectedItem::RuntimeBudget(ExpectedRuntimeBudget {
                    name: "fast-ci".to_string(),
                    max_runtime_ms: 500,
                    mode: AssertionMode::Exact,
                }),
                ExpectedItem::RuntimeBudget(ExpectedRuntimeBudget {
                    name: "slow-ci".to_string(),
                    max_runtime_ms: 800,
                    mode: AssertionMode::Exact,
                }),
            ],
            &[
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
            ],
            MatcherConfig::default(),
        );

        let metrics = compute_metrics(&matches);

        assert_eq!(metrics.runtime_budget_passed, 1);
        assert_eq!(metrics.runtime_budget_failed, 1);
    }

    #[test]
    fn eval_metrics_convert_into_report_metric_summary() {
        let metrics = compute_metrics(&[
            summary(MatchOutcome::TruePositive, MatchItemKind::Diagnostic, true, true),
            summary(MatchOutcome::TrapHit, MatchItemKind::Diagnostic, true, true),
            summary(
                MatchOutcome::RuntimeBudgetFailed,
                MatchItemKind::RuntimeBudget,
                true,
                true,
            ),
        ]);

        let summary: MetricSummary = metrics.into();

        assert_eq!(summary.true_positives, 1);
        assert_eq!(summary.false_positive_trap_hits, 1);
        assert_eq!(summary.runtime_budget_failed, 1);
    }

    fn summary(
        outcome: MatchOutcome,
        item_kind: MatchItemKind,
        has_expected: bool,
        has_observed: bool,
    ) -> MatchSummary {
        MatchSummary {
            item_key: format!("{item_kind:?}:{outcome:?}"),
            outcome,
            item_kind,
            expected_key: has_expected.then(|| "expected".to_string()),
            observed_key: has_observed.then(|| "observed".to_string()),
            expected_runtime_budget_ms: None,
            expected_mode: has_expected.then_some(AssertionMode::Exact),
            observed_runtime_ms: None,
        }
    }
}
