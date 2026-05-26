use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::eval::matcher::{MatchItemKind, MatchOutcome};
use crate::eval::model::{ObservedItem, ObservedStatus};
use crate::eval::report::{CaseResult, EvaluationRun, normalize_run};

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct AdaptationDeltaReport {
    pub(crate) baseline_suite_id: String,
    pub(crate) adapted_suite_id: String,
    pub(crate) new_true_positives: u64,
    pub(crate) removed_false_negatives: u64,
    pub(crate) removed_false_positives: u64,
    pub(crate) new_false_positives: u64,
    pub(crate) new_unknowns: u64,
    pub(crate) resolved_unknowns: u64,
    pub(crate) accepted_extension_facts: u64,
    pub(crate) rejected_extension_facts: u64,
    pub(crate) changed_graph_edges: u64,
    pub(crate) changed_paths: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) runtime_overhead_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cache_invalidation_scope: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) cases: Vec<CaseDelta>,
}

impl AdaptationDeltaReport {
    pub(crate) fn normalize(&mut self) {
        self.cases
            .sort_by(|left, right| left.case_id.cmp(&right.case_id));
        for case in &mut self.cases {
            case.normalize();
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct CaseDelta {
    pub(crate) case_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) new_true_positive_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) removed_false_negative_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) removed_false_positive_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) new_false_positive_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) new_unknown_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) resolved_unknown_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) changed_graph_edge_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) changed_path_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) accepted_extension_fact_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) rejected_extension_fact_keys: Vec<String>,
}

impl CaseDelta {
    fn normalize(&mut self) {
        self.new_true_positive_keys.sort();
        self.removed_false_negative_keys.sort();
        self.removed_false_positive_keys.sort();
        self.new_false_positive_keys.sort();
        self.new_unknown_keys.sort();
        self.resolved_unknown_keys.sort();
        self.changed_graph_edge_keys.sort();
        self.changed_path_keys.sort();
        self.accepted_extension_fact_keys.sort();
        self.rejected_extension_fact_keys.sort();
    }

    fn is_empty(&self) -> bool {
        self.new_true_positive_keys.is_empty()
            && self.removed_false_negative_keys.is_empty()
            && self.removed_false_positive_keys.is_empty()
            && self.new_false_positive_keys.is_empty()
            && self.new_unknown_keys.is_empty()
            && self.resolved_unknown_keys.is_empty()
            && self.changed_graph_edge_keys.is_empty()
            && self.changed_path_keys.is_empty()
            && self.accepted_extension_fact_keys.is_empty()
            && self.rejected_extension_fact_keys.is_empty()
    }
}

pub(crate) fn compute_adaptation_delta(
    baseline: &EvaluationRun,
    adapted: &EvaluationRun,
) -> AdaptationDeltaReport {
    let baseline = normalize_run(baseline);
    let adapted = normalize_run(adapted);
    let baseline_cases = case_map(&baseline.cases);
    let adapted_cases = case_map(&adapted.cases);
    let case_ids = baseline_cases
        .keys()
        .chain(adapted_cases.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    let mut report = AdaptationDeltaReport {
        baseline_suite_id: baseline.suite_id.clone(),
        adapted_suite_id: adapted.suite_id.clone(),
        runtime_overhead_ratio: runtime_overhead_ratio(&baseline.cases, &adapted.cases),
        ..AdaptationDeltaReport::default()
    };

    for case_id in case_ids {
        let mut case_delta = CaseDelta {
            case_id: case_id.to_string(),
            ..CaseDelta::default()
        };
        let baseline_case = baseline_cases.get(case_id).copied();
        let adapted_case = adapted_cases.get(case_id).copied();
        collect_match_deltas(baseline_case, adapted_case, &mut case_delta);
        collect_extension_fact_deltas(baseline_case, adapted_case, &mut case_delta);
        if !case_delta.is_empty() {
            accumulate_case(&mut report, &case_delta);
            case_delta.normalize();
            report.cases.push(case_delta);
        }
    }

    report.normalize();
    report
}

fn case_map(cases: &[CaseResult]) -> BTreeMap<&str, &CaseResult> {
    cases
        .iter()
        .map(|case| (case.case_id.as_str(), case))
        .collect()
}

fn accumulate_case(report: &mut AdaptationDeltaReport, case_delta: &CaseDelta) {
    report.new_true_positives += case_delta.new_true_positive_keys.len() as u64;
    report.removed_false_negatives += case_delta.removed_false_negative_keys.len() as u64;
    report.removed_false_positives += case_delta.removed_false_positive_keys.len() as u64;
    report.new_false_positives += case_delta.new_false_positive_keys.len() as u64;
    report.new_unknowns += case_delta.new_unknown_keys.len() as u64;
    report.resolved_unknowns += case_delta.resolved_unknown_keys.len() as u64;
    report.changed_graph_edges += case_delta.changed_graph_edge_keys.len() as u64;
    report.changed_paths += case_delta.changed_path_keys.len() as u64;
    report.accepted_extension_facts += case_delta.accepted_extension_fact_keys.len() as u64;
    report.rejected_extension_facts += case_delta.rejected_extension_fact_keys.len() as u64;
}

fn collect_match_deltas(
    baseline_case: Option<&CaseResult>,
    adapted_case: Option<&CaseResult>,
    case_delta: &mut CaseDelta,
) {
    let baseline = match_map(baseline_case);
    let adapted = match_map(adapted_case);
    let keys = baseline
        .keys()
        .chain(adapted.keys())
        .cloned()
        .collect::<BTreeSet<_>>();

    for key in keys {
        let before = baseline.get(&key).copied();
        let after = adapted.get(&key).copied();
        if before == after {
            continue;
        }
        if after == Some(MatchOutcome::TruePositive) && before != Some(MatchOutcome::TruePositive) {
            case_delta.new_true_positive_keys.push(key.clone());
        }
        if before == Some(MatchOutcome::FalseNegative) && after != Some(MatchOutcome::FalseNegative)
        {
            case_delta.removed_false_negative_keys.push(key.clone());
        }
        if before.is_some_and(is_false_positive_outcome)
            && !after.is_some_and(is_false_positive_outcome)
        {
            case_delta.removed_false_positive_keys.push(key.clone());
        }
        if after.is_some_and(is_false_positive_outcome)
            && !before.is_some_and(is_false_positive_outcome)
        {
            case_delta.new_false_positive_keys.push(key.clone());
        }
        if after == Some(MatchOutcome::Unknown) && before != Some(MatchOutcome::Unknown) {
            case_delta.new_unknown_keys.push(key.clone());
        }
        if before == Some(MatchOutcome::Unknown) && after != Some(MatchOutcome::Unknown) {
            case_delta.resolved_unknown_keys.push(key);
        }
    }

    collect_kind_changes(
        baseline_case,
        adapted_case,
        MatchItemKind::GraphEdge,
        &mut case_delta.changed_graph_edge_keys,
    );
    collect_kind_changes(
        baseline_case,
        adapted_case,
        MatchItemKind::Path,
        &mut case_delta.changed_path_keys,
    );
}

fn match_map(case: Option<&CaseResult>) -> BTreeMap<String, MatchOutcome> {
    case.into_iter()
        .flat_map(|case| case.matches.iter())
        .map(|summary| (summary.item_key.clone(), summary.outcome))
        .collect()
}

fn collect_kind_changes(
    baseline_case: Option<&CaseResult>,
    adapted_case: Option<&CaseResult>,
    kind: MatchItemKind,
    output: &mut Vec<String>,
) {
    let before = kind_match_map(baseline_case, kind);
    let after = kind_match_map(adapted_case, kind);
    output.extend(
        before
            .keys()
            .chain(after.keys())
            .filter(|key| before.get(*key) != after.get(*key))
            .cloned(),
    );
}

fn kind_match_map(
    case: Option<&CaseResult>,
    kind: MatchItemKind,
) -> BTreeMap<String, MatchOutcome> {
    case.into_iter()
        .flat_map(|case| case.matches.iter())
        .filter(|summary| summary.item_kind == kind)
        .map(|summary| (summary.item_key.clone(), summary.outcome))
        .collect()
}

fn collect_extension_fact_deltas(
    baseline_case: Option<&CaseResult>,
    adapted_case: Option<&CaseResult>,
    case_delta: &mut CaseDelta,
) {
    let baseline_facts = extension_fact_keys(baseline_case);
    for (key, status) in extension_fact_keys(adapted_case) {
        if baseline_facts.contains_key(&key) {
            continue;
        }
        match status {
            Some(ObservedStatus::Accepted) => case_delta.accepted_extension_fact_keys.push(key),
            Some(ObservedStatus::Rejected) => case_delta.rejected_extension_fact_keys.push(key),
            _ => {}
        }
    }
}

fn extension_fact_keys(case: Option<&CaseResult>) -> BTreeMap<String, Option<ObservedStatus>> {
    case.into_iter()
        .flat_map(|case| case.observed.iter())
        .filter_map(|item| match item {
            ObservedItem::Fact(fact)
                if fact
                    .producer_id
                    .as_deref()
                    .is_some_and(|producer| producer.contains("extension")) =>
            {
                Some((format!("{}:{}", fact.family, fact.stable_key), fact.status))
            }
            _ => None,
        })
        .collect()
}

fn is_false_positive_outcome(outcome: MatchOutcome) -> bool {
    matches!(
        outcome,
        MatchOutcome::FalsePositive | MatchOutcome::ForbiddenHit | MatchOutcome::TrapHit
    )
}

fn runtime_overhead_ratio(baseline: &[CaseResult], adapted: &[CaseResult]) -> Option<f64> {
    let baseline_ms = total_runtime_ms(baseline)?;
    let adapted_ms = total_runtime_ms(adapted)?;
    if baseline_ms == 0 {
        return None;
    }
    Some(adapted_ms as f64 / baseline_ms as f64)
}

fn total_runtime_ms(cases: &[CaseResult]) -> Option<u64> {
    let mut total = 0_u64;
    for case in cases {
        total = total.checked_add(case.runtime.observed_runtime_ms?)?;
    }
    Some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::matcher::{MatchItemKind, MatchOutcome};
    use crate::eval::model::{
        AssertionMode, EvaluationMode, ExpectedDiagnostic, ExpectedItem, FixtureArea, ObservedFact,
        ObservedItem,
    };
    use crate::eval::report::{
        EVALUATION_SCHEMA_VERSION, MatchSummary, MetricSections, MetricSummary, RuntimeObservation,
    };

    #[test]
    fn delta_names_changed_cases_and_item_keys() {
        let baseline = run(vec![case(
            "case-a",
            vec![
                summary(
                    "diagnostic:missing",
                    MatchOutcome::FalseNegative,
                    MatchItemKind::Diagnostic,
                ),
                summary(
                    "diagnostic:unknown",
                    MatchOutcome::Unknown,
                    MatchItemKind::Diagnostic,
                ),
            ],
            Vec::new(),
            Some(10),
        )]);
        let adapted = run(vec![case(
            "case-a",
            vec![
                summary(
                    "diagnostic:missing",
                    MatchOutcome::TruePositive,
                    MatchItemKind::Diagnostic,
                ),
                summary(
                    "diagnostic:unknown",
                    MatchOutcome::TruePositive,
                    MatchItemKind::Diagnostic,
                ),
                summary(
                    "graph:edge-a",
                    MatchOutcome::TruePositive,
                    MatchItemKind::GraphEdge,
                ),
                summary(
                    "path:path-a",
                    MatchOutcome::TruePositive,
                    MatchItemKind::Path,
                ),
            ],
            vec![extension_fact(
                "extension.routes",
                "route:/ok",
                ObservedStatus::Accepted,
            )],
            Some(15),
        )]);

        let delta = compute_adaptation_delta(&baseline, &adapted);

        assert_eq!(delta.new_true_positives, 4);
        assert_eq!(delta.removed_false_negatives, 1);
        assert_eq!(delta.resolved_unknowns, 1);
        assert_eq!(delta.changed_graph_edges, 1);
        assert_eq!(delta.changed_paths, 1);
        assert_eq!(delta.accepted_extension_facts, 1);
        assert_eq!(delta.runtime_overhead_ratio, Some(1.5));
        assert_eq!(delta.cases[0].case_id, "case-a");
        assert!(
            delta.cases[0]
                .removed_false_negative_keys
                .contains(&"diagnostic:missing".to_string())
        );
    }

    #[test]
    fn rejected_extension_facts_remain_visible_when_score_improves() {
        let baseline = run(vec![case(
            "case-a",
            vec![summary(
                "diagnostic:missing",
                MatchOutcome::FalseNegative,
                MatchItemKind::Diagnostic,
            )],
            Vec::new(),
            None,
        )]);
        let adapted = run(vec![case(
            "case-a",
            vec![summary(
                "diagnostic:missing",
                MatchOutcome::TruePositive,
                MatchItemKind::Diagnostic,
            )],
            vec![
                extension_fact("extension.routes", "route:/ok", ObservedStatus::Accepted),
                extension_fact(
                    "extension.routes",
                    "route:/rejected",
                    ObservedStatus::Rejected,
                ),
            ],
            None,
        )]);

        let delta = compute_adaptation_delta(&baseline, &adapted);

        assert_eq!(delta.new_true_positives, 1);
        assert_eq!(delta.rejected_extension_facts, 1);
        assert!(
            delta.cases[0]
                .rejected_extension_fact_keys
                .contains(&"extension.routes:route:/rejected".to_string())
        );
        assert_eq!(delta.runtime_overhead_ratio, None);
    }

    #[test]
    fn native_adaptation_delta_fixture_exposes_improvements_and_rejections() {
        let adapted = crate::eval::fixtures::run_native_fixture_for_test(
            &repo_root().join("tests/eval-fixtures/extension/adaptation-delta"),
        )
        .unwrap();
        let mut baseline = adapted.clone();
        baseline.cases[0].observed.clear();
        baseline.cases[0].matches = vec![
            summary(
                "diagnostic:extension.adaptation.source_model:src/app.ts:none:none:tolerant",
                MatchOutcome::FalseNegative,
                MatchItemKind::Diagnostic,
            ),
            summary(
                "fact:extension.adaptation:extension.adaptation.accepted_source:exact:polint.extension.adaptation:heuristic:accepted",
                MatchOutcome::Unknown,
                MatchItemKind::Fact,
            ),
        ];

        let delta = compute_adaptation_delta(&baseline, &adapted);

        assert_eq!(delta.removed_false_negatives, 1);
        assert_eq!(delta.resolved_unknowns, 1);
        assert_eq!(delta.accepted_extension_facts, 1);
        assert_eq!(delta.rejected_extension_facts, 1);
    }

    fn run(cases: Vec<CaseResult>) -> EvaluationRun {
        EvaluationRun {
            schema_version: EVALUATION_SCHEMA_VERSION.to_string(),
            suite_id: "adaptation-suite".to_string(),
            mode: EvaluationMode::PolintAgentAdapted,
            suite_manifest: None,
            cases,
            metrics: MetricSummary {
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
                sections: MetricSections::default(),
            },
            performance: None,
            comparison_rows: Vec::new(),
            adaptation: None,
            adaptation_delta: None,
            limitations: Vec::new(),
            output_hash: String::new(),
        }
    }

    fn case(
        case_id: &str,
        matches: Vec<MatchSummary>,
        observed: Vec<ObservedItem>,
        runtime_ms: Option<u64>,
    ) -> CaseResult {
        CaseResult {
            case_id: case_id.to_string(),
            area: FixtureArea::Diagnostics,
            expected: vec![ExpectedItem::Diagnostic(ExpectedDiagnostic {
                rule_id: "local/example".to_string(),
                relative_path: "src/app.ts".to_string(),
                line: None,
                fingerprint: None,
                mode: AssertionMode::Exact,
                false_positive_trap: false,
            })],
            observed,
            matches,
            runtime: RuntimeObservation {
                budget_name: "delta".to_string(),
                budget_passed: true,
                observed_runtime_ms: runtime_ms,
            },
        }
    }

    fn summary(item_key: &str, outcome: MatchOutcome, item_kind: MatchItemKind) -> MatchSummary {
        MatchSummary {
            item_key: item_key.to_string(),
            outcome,
            item_kind,
            expected_key: Some(item_key.to_string()),
            observed_key: None,
            observed_status: None,
            expected_runtime_budget_ms: None,
            expected_mode: Some(AssertionMode::Exact),
            observed_runtime_ms: None,
        }
    }

    fn extension_fact(family: &str, stable_key: &str, status: ObservedStatus) -> ObservedItem {
        ObservedItem::Fact(ObservedFact {
            family: family.to_string(),
            stable_key: stable_key.to_string(),
            mode: AssertionMode::Exact,
            producer_id: Some("polint.extension.test".to_string()),
            provenance: Some("extension".to_string()),
            precision: Some("heuristic".to_string()),
            status: Some(status),
            payload: None,
        })
    }

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }
}
