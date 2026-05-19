use serde::{Deserialize, Serialize};

use crate::eval::matcher::{MatchItemKind, MatchOutcome};
use crate::eval::report::{MatchSummary, MetricSummary};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct ComputedMetrics {
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

impl From<ComputedMetrics> for MetricSummary {
    fn from(metrics: ComputedMetrics) -> Self {
        Self {
            true_positives: metrics.true_positives,
            false_positives: metrics.false_positives,
            false_negatives: metrics.false_negatives,
            true_negatives: metrics.true_negatives,
            unconfirmed: metrics.unconfirmed,
            false_positive_trap_hits: metrics.false_positive_trap_hits,
            forbidden_hits: metrics.forbidden_hits,
            unknown_count: metrics.unknown_count,
            facts_present: metrics.facts_present,
            facts_accepted: metrics.facts_accepted,
            facts_rejected: metrics.facts_rejected,
            graph_edges_expected: metrics.graph_edges_expected,
            graph_edges_observed: metrics.graph_edges_observed,
            graph_edges_unconfirmed: metrics.graph_edges_unconfirmed,
            paths_expected: metrics.paths_expected,
            paths_observed: metrics.paths_observed,
            paths_unconfirmed: metrics.paths_unconfirmed,
            runtime_budget_passed: metrics.runtime_budget_passed,
            runtime_budget_failed: metrics.runtime_budget_failed,
            precision: metrics.precision,
            recall: metrics.recall,
            f1: metrics.f1,
            f2: metrics.f2,
            f3: metrics.f3,
            false_positive_rate: metrics.false_positive_rate,
        }
    }
}

pub(crate) fn compute_metrics(matches: &[MatchSummary]) -> ComputedMetrics {
    let mut metrics = ComputedMetrics {
        true_positives: 0,
        false_positives: 0,
        false_negatives: 0,
        true_negatives: 0,
        unconfirmed: 0,
        false_positive_trap_hits: 0,
        forbidden_hits: 0,
        unknown_count: 0,
        facts_present: 0,
        facts_accepted: 0,
        facts_rejected: 0,
        graph_edges_expected: 0,
        graph_edges_observed: 0,
        graph_edges_unconfirmed: 0,
        paths_expected: 0,
        paths_observed: 0,
        paths_unconfirmed: 0,
        runtime_budget_passed: 0,
        runtime_budget_failed: 0,
        precision: None,
        recall: None,
        f1: None,
        f2: None,
        f3: None,
        false_positive_rate: None,
    };

    for summary in matches {
        match summary.outcome {
            MatchOutcome::TruePositive => metrics.true_positives += 1,
            MatchOutcome::FalsePositive => metrics.false_positives += 1,
            MatchOutcome::FalseNegative => metrics.false_negatives += 1,
            MatchOutcome::TrueNegative => metrics.true_negatives += 1,
            MatchOutcome::Unconfirmed => metrics.unconfirmed += 1,
            MatchOutcome::ForbiddenHit => metrics.forbidden_hits += 1,
            MatchOutcome::TrapHit => metrics.false_positive_trap_hits += 1,
            MatchOutcome::Unknown => metrics.unknown_count += 1,
            MatchOutcome::RuntimeBudgetPassed => metrics.runtime_budget_passed += 1,
            MatchOutcome::RuntimeBudgetFailed => metrics.runtime_budget_failed += 1,
        }

        match summary.item_kind {
            MatchItemKind::GraphEdge => {
                if summary.expected_key.is_some() {
                    metrics.graph_edges_expected += 1;
                }
                if summary.observed_key.is_some() {
                    metrics.graph_edges_observed += 1;
                }
                if summary.outcome == MatchOutcome::Unconfirmed {
                    metrics.graph_edges_unconfirmed += 1;
                }
            }
            MatchItemKind::Fact => match summary.observed_status {
                Some(
                    crate::eval::model::ObservedStatus::Present
                    | crate::eval::model::ObservedStatus::Resolved,
                ) => metrics.facts_present += 1,
                Some(crate::eval::model::ObservedStatus::Accepted) => metrics.facts_accepted += 1,
                Some(crate::eval::model::ObservedStatus::Rejected) => metrics.facts_rejected += 1,
                Some(
                    crate::eval::model::ObservedStatus::Unknown
                        | crate::eval::model::ObservedStatus::Unresolved
                        | crate::eval::model::ObservedStatus::Ambiguous
                        | crate::eval::model::ObservedStatus::Dynamic
                        | crate::eval::model::ObservedStatus::SetupMissing
                        | crate::eval::model::ObservedStatus::Unsupported
                        | crate::eval::model::ObservedStatus::External
                        | crate::eval::model::ObservedStatus::Cycle
                        | crate::eval::model::ObservedStatus::Generated,
                )
                | None => {}
            },
            MatchItemKind::Path => {
                if summary.expected_key.is_some() {
                    metrics.paths_expected += 1;
                }
                if summary.observed_key.is_some() {
                    metrics.paths_observed += 1;
                }
                if summary.outcome == MatchOutcome::Unconfirmed {
                    metrics.paths_unconfirmed += 1;
                }
            }
            MatchItemKind::Diagnostic | MatchItemKind::Invariant | MatchItemKind::RuntimeBudget => {
            }
        }
    }

    metrics.precision = ratio(
        metrics.true_positives,
        metrics.true_positives + metrics.false_positives,
    );
    metrics.recall = ratio(
        metrics.true_positives,
        metrics.true_positives + metrics.false_negatives,
    );
    metrics.f1 = f_score(
        1.0,
        metrics.true_positives,
        metrics.false_positives,
        metrics.false_negatives,
    );
    metrics.f2 = f_score(
        2.0,
        metrics.true_positives,
        metrics.false_positives,
        metrics.false_negatives,
    );
    metrics.f3 = f_score(
        3.0,
        metrics.true_positives,
        metrics.false_positives,
        metrics.false_negatives,
    );
    metrics.false_positive_rate = ratio(
        metrics.false_positives,
        metrics.false_positives + metrics.true_negatives,
    );

    metrics
}

fn ratio(numerator: u64, denominator: u64) -> Option<f64> {
    if denominator == 0 {
        None
    } else {
        Some(numerator as f64 / denominator as f64)
    }
}

fn f_score(
    beta: f64,
    true_positives: u64,
    false_positives: u64,
    false_negatives: u64,
) -> Option<f64> {
    let beta_squared = beta * beta;
    let true_positives = true_positives as f64;
    let denominator = (1.0 + beta_squared) * true_positives
        + beta_squared * false_negatives as f64
        + false_positives as f64;
    if denominator == 0.0 {
        None
    } else {
        Some((1.0 + beta_squared) * true_positives / denominator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::matcher::{MatchItemKind, MatchOutcome, MatcherConfig, match_case};
    use crate::eval::model::{
        AssertionMode, ExpectedItem, ExpectedRuntimeBudget, ObservedItem, ObservedRuntimeBudget,
    };
    use crate::eval::report::{MatchSummary, MetricSummary};

    #[test]
    fn eval_metrics_count_mixed_match_outcomes() {
        let metrics = compute_metrics(&[
            summary(
                MatchOutcome::TruePositive,
                MatchItemKind::Diagnostic,
                true,
                true,
            ),
            summary(
                MatchOutcome::FalsePositive,
                MatchItemKind::Diagnostic,
                false,
                true,
            ),
            summary(
                MatchOutcome::FalseNegative,
                MatchItemKind::Fact,
                true,
                false,
            ),
            summary(MatchOutcome::TrueNegative, MatchItemKind::Fact, true, false),
            summary(
                MatchOutcome::Unconfirmed,
                MatchItemKind::GraphEdge,
                false,
                true,
            ),
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
            summary(
                MatchOutcome::TruePositive,
                MatchItemKind::GraphEdge,
                true,
                true,
            ),
            summary(
                MatchOutcome::FalseNegative,
                MatchItemKind::Path,
                true,
                false,
            ),
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
                facts_present: 1,
                facts_accepted: 0,
                facts_rejected: 0,
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
            summary(
                MatchOutcome::TruePositive,
                MatchItemKind::Diagnostic,
                true,
                true,
            ),
            summary(
                MatchOutcome::FalsePositive,
                MatchItemKind::Diagnostic,
                false,
                true,
            ),
        ]);

        let left = serde_json::to_string(&metrics).unwrap();
        let right = serde_json::to_string(&metrics).unwrap();

        assert_eq!(left, right);
        assert!(left.starts_with("{\"true_positives\":"));
        assert!(
            left.find("\"false_positives\"").unwrap() < left.find("\"false_negatives\"").unwrap()
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
    fn eval_metrics_count_fact_statuses_separately() {
        let metrics = compute_metrics(&[
            summary_with_status(
                MatchOutcome::TruePositive,
                MatchItemKind::Fact,
                crate::eval::model::ObservedStatus::Present,
            ),
            summary_with_status(
                MatchOutcome::TruePositive,
                MatchItemKind::Fact,
                crate::eval::model::ObservedStatus::Accepted,
            ),
            summary_with_status(
                MatchOutcome::TruePositive,
                MatchItemKind::Fact,
                crate::eval::model::ObservedStatus::Rejected,
            ),
        ]);

        assert_eq!(metrics.facts_present, 1);
        assert_eq!(metrics.facts_accepted, 1);
        assert_eq!(metrics.facts_rejected, 1);
    }

    #[test]
    fn eval_metrics_convert_into_report_metric_summary() {
        let metrics = compute_metrics(&[
            summary(
                MatchOutcome::TruePositive,
                MatchItemKind::Diagnostic,
                true,
                true,
            ),
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
            observed_status: has_observed.then_some(crate::eval::model::ObservedStatus::Present),
            expected_runtime_budget_ms: None,
            expected_mode: has_expected.then_some(AssertionMode::Exact),
            observed_runtime_ms: None,
        }
    }

    fn summary_with_status(
        outcome: MatchOutcome,
        item_kind: MatchItemKind,
        observed_status: crate::eval::model::ObservedStatus,
    ) -> MatchSummary {
        MatchSummary {
            item_key: format!("{item_kind:?}:{outcome:?}:{observed_status:?}"),
            outcome,
            item_kind,
            expected_key: Some("expected".to_string()),
            observed_key: Some("observed".to_string()),
            observed_status: Some(observed_status),
            expected_runtime_budget_ms: None,
            expected_mode: Some(AssertionMode::Exact),
            observed_runtime_ms: None,
        }
    }
}
