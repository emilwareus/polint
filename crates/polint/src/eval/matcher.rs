use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::eval::model::{
    AssertionMode, ExpectedDiagnostic, ExpectedFact, ExpectedItem, ObservedDiagnostic,
    ObservedFact, ObservedItem, ObservedStatus,
};
use crate::eval::report::MatchSummary;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MatchOutcome {
    TruePositive,
    FalseNegative,
    FalsePositive,
    TrueNegative,
    Unconfirmed,
    ForbiddenHit,
    TrapHit,
    Unknown,
    RuntimeBudgetPassed,
    RuntimeBudgetFailed,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MatchItemKind {
    Diagnostic,
    Fact,
    GraphEdge,
    Path,
    Invariant,
    RuntimeBudget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MatcherConfig {
    pub(crate) line_tolerance: u32,
}

impl Default for MatcherConfig {
    fn default() -> Self {
        Self { line_tolerance: 2 }
    }
}

pub(crate) fn match_case(
    expected: &[ExpectedItem],
    observed: &[ObservedItem],
    config: MatcherConfig,
) -> Vec<MatchSummary> {
    let mut expected_rows: Vec<_> = expected
        .iter()
        .enumerate()
        .map(|(index, item)| (index, item, expected_item_key(item)))
        .collect();
    expected_rows.sort_by(|left, right| left.2.cmp(&right.2).then_with(|| left.0.cmp(&right.0)));

    let mut observed_rows: Vec<_> = observed
        .iter()
        .enumerate()
        .map(|(index, item)| (index, item, observed_item_key(item)))
        .collect();
    observed_rows.sort_by(|left, right| left.2.cmp(&right.2).then_with(|| left.0.cmp(&right.0)));

    let mut matched_observed = BTreeSet::new();
    let mut summaries = Vec::new();

    for (_, expected_item, expected_key) in &expected_rows {
        if let Some((observed_index, observed_item, observed_key)) =
            matching_observed(expected_item, &observed_rows, &matched_observed, config)
        {
            matched_observed.insert(observed_index);
            summaries.push(summary_for_match(
                expected_item,
                observed_item,
                expected_key.clone(),
                observed_key.clone(),
            ));
        } else {
            summaries.push(summary_for_missing_expected(
                expected_item,
                expected_key.clone(),
            ));
        }
    }

    for (observed_index, observed_item, observed_key) in &observed_rows {
        if matched_observed.contains(observed_index) {
            continue;
        }
        summaries.push(summary_for_extra_observed(
            observed_item,
            observed_key.clone(),
            expected,
        ));
    }

    summaries.sort_by(|left, right| {
        (
            left.item_key.as_str(),
            left.outcome,
            left.item_kind,
            left.expected_key.as_deref(),
            left.observed_key.as_deref(),
        )
            .cmp(&(
                right.item_key.as_str(),
                right.outcome,
                right.item_kind,
                right.expected_key.as_deref(),
                right.observed_key.as_deref(),
            ))
    });
    summaries
}

fn matching_observed<'a>(
    expected: &ExpectedItem,
    observed_rows: &'a [(usize, &'a ObservedItem, String)],
    matched_observed: &BTreeSet<usize>,
    config: MatcherConfig,
) -> Option<(usize, &'a ObservedItem, &'a String)> {
    observed_rows
        .iter()
        .find(|(index, observed, _)| {
            !matched_observed.contains(index) && items_match(expected, observed, config)
        })
        .map(|(index, observed, key)| (*index, *observed, key))
}

fn items_match(expected: &ExpectedItem, observed: &ObservedItem, config: MatcherConfig) -> bool {
    match (expected, observed) {
        (ExpectedItem::Diagnostic(expected), ObservedItem::Diagnostic(observed)) => {
            diagnostic_matches(expected, observed, config)
        }
        (ExpectedItem::Fact(expected), ObservedItem::Fact(observed)) => {
            fact_matches(expected, observed)
        }
        (ExpectedItem::GraphEdge(expected), ObservedItem::GraphEdge(observed)) => {
            expected.graph == observed.graph
                && expected.from == observed.from
                && expected.to == observed.to
        }
        (ExpectedItem::Path(expected), ObservedItem::Path(observed)) => {
            expected.path_id == observed.path_id && expected.nodes == observed.nodes
        }
        (ExpectedItem::Invariant(expected), ObservedItem::Invariant(observed)) => {
            expected.name == observed.name && expected.value == observed.value
        }
        (ExpectedItem::RuntimeBudget(expected), ObservedItem::RuntimeBudget(observed)) => {
            expected.name == observed.name
        }
        _ => false,
    }
}

fn fact_matches(expected: &ExpectedFact, observed: &ObservedFact) -> bool {
    expected.family == observed.family
        && fact_stable_key_matches(expected.mode, &expected.stable_key, &observed.stable_key)
        && optional_str_matches(expected.producer_id.as_deref(), observed.producer_id.as_deref())
        && optional_str_matches(expected.precision.as_deref(), observed.precision.as_deref())
        && optional_value_matches(expected.status, observed.status)
}

fn fact_stable_key_matches(mode: AssertionMode, expected: &str, observed: &str) -> bool {
    if mode == AssertionMode::Partial {
        observed.contains(expected)
    } else {
        expected == observed
    }
}

fn optional_str_matches(expected: Option<&str>, observed: Option<&str>) -> bool {
    expected.is_none_or(|expected| observed == Some(expected))
}

fn optional_value_matches<T>(expected: Option<T>, observed: Option<T>) -> bool
where
    T: Copy + Eq,
{
    expected.is_none_or(|expected| observed == Some(expected))
}

fn diagnostic_matches(
    expected: &ExpectedDiagnostic,
    observed: &ObservedDiagnostic,
    config: MatcherConfig,
) -> bool {
    if expected.rule_id != observed.rule_id || expected.relative_path != observed.relative_path {
        return false;
    }

    if expected.mode == AssertionMode::Tolerant {
        return match (expected.line, observed.line) {
            (Some(expected_line), Some(observed_line)) => {
                expected_line.abs_diff(observed_line) <= config.line_tolerance
                    && diagnostic_fingerprint_compatible(expected, observed)
            }
            _ => false,
        };
    }

    diagnostic_identity_part(expected.fingerprint.as_deref(), expected.line)
        == diagnostic_identity_part(observed.fingerprint.as_deref(), observed.line)
}

fn diagnostic_fingerprint_compatible(
    expected: &ExpectedDiagnostic,
    observed: &ObservedDiagnostic,
) -> bool {
    match (
        expected.fingerprint.as_deref(),
        observed.fingerprint.as_deref(),
    ) {
        (Some(expected), Some(observed)) => expected == observed,
        _ => true,
    }
}

fn summary_for_match(
    expected: &ExpectedItem,
    observed: &ObservedItem,
    expected_key: String,
    observed_key: String,
) -> MatchSummary {
    MatchSummary {
        item_key: expected_key.clone(),
        outcome: matched_outcome(expected, observed),
        item_kind: expected_item_kind(expected),
        expected_key: Some(expected_key),
        observed_key: Some(observed_key),
        expected_runtime_budget_ms: expected_runtime_budget_ms(expected),
        expected_mode: Some(expected_mode(expected)),
        observed_runtime_ms: observed_runtime_ms(observed),
    }
}

fn summary_for_missing_expected(expected: &ExpectedItem, expected_key: String) -> MatchSummary {
    MatchSummary {
        item_key: expected_key.clone(),
        outcome: missing_expected_outcome(expected),
        item_kind: expected_item_kind(expected),
        expected_key: Some(expected_key),
        observed_key: None,
        expected_runtime_budget_ms: expected_runtime_budget_ms(expected),
        expected_mode: Some(expected_mode(expected)),
        observed_runtime_ms: None,
    }
}

fn summary_for_extra_observed(
    observed: &ObservedItem,
    observed_key: String,
    expected: &[ExpectedItem],
) -> MatchSummary {
    MatchSummary {
        item_key: observed_key.clone(),
        outcome: extra_observed_outcome(observed, expected),
        item_kind: observed_item_kind(observed),
        expected_key: None,
        observed_key: Some(observed_key),
        expected_runtime_budget_ms: None,
        expected_mode: None,
        observed_runtime_ms: observed_runtime_ms(observed),
    }
}

fn matched_outcome(expected: &ExpectedItem, observed: &ObservedItem) -> MatchOutcome {
    if observed_unknown_outcome(observed) {
        return MatchOutcome::Unknown;
    }
    if expected_false_positive_trap(expected) {
        return MatchOutcome::TrapHit;
    }
    if expected_mode(expected) == AssertionMode::Forbidden {
        return MatchOutcome::ForbiddenHit;
    }
    if let ObservedItem::RuntimeBudget(budget) = observed {
        return if budget.budget_passed {
            MatchOutcome::RuntimeBudgetPassed
        } else {
            MatchOutcome::RuntimeBudgetFailed
        };
    }
    MatchOutcome::TruePositive
}

fn missing_expected_outcome(expected: &ExpectedItem) -> MatchOutcome {
    if expected_false_positive_trap(expected) || expected_mode(expected) == AssertionMode::Forbidden
    {
        MatchOutcome::TrueNegative
    } else {
        MatchOutcome::FalseNegative
    }
}

fn extra_observed_outcome(observed: &ObservedItem, expected: &[ExpectedItem]) -> MatchOutcome {
    if observed_unknown_outcome(observed) {
        return MatchOutcome::Unknown;
    }
    if graph_or_path_extra_is_unconfirmed(observed, expected) {
        return MatchOutcome::Unconfirmed;
    }
    MatchOutcome::FalsePositive
}

fn observed_unknown_outcome(observed: &ObservedItem) -> bool {
    matches!(
        observed_status(observed),
        Some(ObservedStatus::Unknown | ObservedStatus::SetupMissing | ObservedStatus::Unsupported)
    )
}

fn graph_or_path_extra_is_unconfirmed(observed: &ObservedItem, expected: &[ExpectedItem]) -> bool {
    match observed {
        ObservedItem::GraphEdge(observed) => expected.iter().any(|item| match item {
            ExpectedItem::GraphEdge(expected) => {
                expected.graph == observed.graph
                    && (expected.partial_truth || expected.mode == AssertionMode::Partial)
            }
            _ => false,
        }),
        ObservedItem::Path(observed) => expected.iter().any(|item| match item {
            ExpectedItem::Path(expected) => {
                expected.path_id == observed.path_id
                    && (expected.partial_truth || expected.mode == AssertionMode::Partial)
            }
            _ => false,
        }),
        _ => false,
    }
}

fn expected_false_positive_trap(expected: &ExpectedItem) -> bool {
    match expected {
        ExpectedItem::Diagnostic(diagnostic) => diagnostic.false_positive_trap,
        ExpectedItem::Fact(fact) => fact.false_positive_trap,
        ExpectedItem::GraphEdge(_)
        | ExpectedItem::Path(_)
        | ExpectedItem::Invariant(_)
        | ExpectedItem::RuntimeBudget(_) => false,
    }
}

fn expected_mode(expected: &ExpectedItem) -> AssertionMode {
    match expected {
        ExpectedItem::Diagnostic(item) => item.mode,
        ExpectedItem::Fact(item) => item.mode,
        ExpectedItem::GraphEdge(item) => item.mode,
        ExpectedItem::Path(item) => item.mode,
        ExpectedItem::Invariant(item) => item.mode,
        ExpectedItem::RuntimeBudget(item) => item.mode,
    }
}

fn observed_status(observed: &ObservedItem) -> Option<ObservedStatus> {
    match observed {
        ObservedItem::Diagnostic(item) => item.status,
        ObservedItem::Fact(item) => item.status,
        ObservedItem::GraphEdge(item) => item.status,
        ObservedItem::Path(item) => item.status,
        ObservedItem::Invariant(item) => item.status,
        ObservedItem::RuntimeBudget(_) => None,
    }
}

fn expected_runtime_budget_ms(expected: &ExpectedItem) -> Option<u64> {
    match expected {
        ExpectedItem::RuntimeBudget(budget) => Some(budget.max_runtime_ms),
        _ => None,
    }
}

fn observed_runtime_ms(observed: &ObservedItem) -> Option<u64> {
    match observed {
        ObservedItem::RuntimeBudget(budget) => budget.observed_runtime_ms,
        _ => None,
    }
}

fn expected_item_kind(expected: &ExpectedItem) -> MatchItemKind {
    match expected {
        ExpectedItem::Diagnostic(_) => MatchItemKind::Diagnostic,
        ExpectedItem::Fact(_) => MatchItemKind::Fact,
        ExpectedItem::GraphEdge(_) => MatchItemKind::GraphEdge,
        ExpectedItem::Path(_) => MatchItemKind::Path,
        ExpectedItem::Invariant(_) => MatchItemKind::Invariant,
        ExpectedItem::RuntimeBudget(_) => MatchItemKind::RuntimeBudget,
    }
}

fn observed_item_kind(observed: &ObservedItem) -> MatchItemKind {
    match observed {
        ObservedItem::Diagnostic(_) => MatchItemKind::Diagnostic,
        ObservedItem::Fact(_) => MatchItemKind::Fact,
        ObservedItem::GraphEdge(_) => MatchItemKind::GraphEdge,
        ObservedItem::Path(_) => MatchItemKind::Path,
        ObservedItem::Invariant(_) => MatchItemKind::Invariant,
        ObservedItem::RuntimeBudget(_) => MatchItemKind::RuntimeBudget,
    }
}

fn expected_item_key(item: &ExpectedItem) -> String {
    match item {
        ExpectedItem::Diagnostic(diagnostic) => diagnostic_key(
            &diagnostic.rule_id,
            &diagnostic.relative_path,
            diagnostic.fingerprint.as_deref(),
            diagnostic.line,
        ),
        ExpectedItem::Fact(fact) => fact_key(&fact.family, &fact.stable_key),
        ExpectedItem::GraphEdge(edge) => graph_edge_key(&edge.graph, &edge.from, &edge.to),
        ExpectedItem::Path(path) => path_key(&path.path_id, &path.nodes),
        ExpectedItem::Invariant(invariant) => invariant_key(&invariant.name, &invariant.value),
        ExpectedItem::RuntimeBudget(budget) => runtime_budget_key(&budget.name),
    }
}

fn observed_item_key(item: &ObservedItem) -> String {
    match item {
        ObservedItem::Diagnostic(diagnostic) => diagnostic_key(
            &diagnostic.rule_id,
            &diagnostic.relative_path,
            diagnostic.fingerprint.as_deref(),
            diagnostic.line,
        ),
        ObservedItem::Fact(fact) => fact_key(&fact.family, &fact.stable_key),
        ObservedItem::GraphEdge(edge) => graph_edge_key(&edge.graph, &edge.from, &edge.to),
        ObservedItem::Path(path) => path_key(&path.path_id, &path.nodes),
        ObservedItem::Invariant(invariant) => invariant_key(&invariant.name, &invariant.value),
        ObservedItem::RuntimeBudget(budget) => runtime_budget_key(&budget.name),
    }
}

fn diagnostic_key(
    rule_id: &str,
    relative_path: &str,
    fingerprint: Option<&str>,
    line: Option<u32>,
) -> String {
    format!(
        "diagnostic:{rule_id}:{relative_path}:{}",
        diagnostic_identity_part(fingerprint, line)
    )
}

fn diagnostic_identity_part(fingerprint: Option<&str>, line: Option<u32>) -> String {
    fingerprint.map_or_else(
        || line.map_or_else(String::new, |line| line.to_string()),
        str::to_string,
    )
}

fn fact_key(family: &str, stable_key: &str) -> String {
    format!("fact:{family}:{stable_key}")
}

fn graph_edge_key(graph: &str, from: &str, to: &str) -> String {
    format!("graph_edge:{graph}:{from}:{to}")
}

fn path_key(path_id: &str, nodes: &[String]) -> String {
    format!("path:{path_id}:{}", nodes.join(">"))
}

fn invariant_key(name: &str, value: &str) -> String {
    format!("invariant:{name}:{value}")
}

fn runtime_budget_key(name: &str) -> String {
    format!("runtime_budget:{name}")
}

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
