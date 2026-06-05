use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::eval::matcher::{MatchItemKind, MatchOutcome};
use crate::eval::metrics::compute_metrics;
use crate::eval::model::{ObservedFact, ObservedItem, ObservedStatus};
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
    #[serde(default)]
    pub(crate) accepted_model_facts: u64,
    #[serde(default)]
    pub(crate) rejected_model_facts: u64,
    pub(crate) changed_graph_edges: u64,
    pub(crate) changed_paths: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) runtime_overhead_ratio: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) cache_invalidation_scope: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) held_out: Option<HeldOutDeltaReport>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) accepted_model_fact_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) rejected_model_fact_keys: Vec<String>,
    #[serde(default)]
    pub(crate) held_out: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct HeldOutDeltaReport {
    pub(crate) selection_cases: u64,
    pub(crate) held_out_cases: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) missing_selection_case_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) missing_held_out_case_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) overlapping_case_ids: Vec<String>,
    pub(crate) held_out_unknown_delta: i64,
    pub(crate) held_out_precision_delta: Option<f64>,
    pub(crate) held_out_recall_delta: Option<f64>,
    pub(crate) held_out_runtime_overhead_ratio: Option<f64>,
    pub(crate) held_out_cache_invalidation_scope: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct HeldOutCasePartition {
    pub(crate) selection_cases: Vec<String>,
    pub(crate) held_out_cases: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) held_out_cache_invalidation_scope: Option<String>,
}

impl HeldOutCasePartition {
    fn held_out_case_ids(&self) -> BTreeSet<&str> {
        self.held_out_cases.iter().map(String::as_str).collect()
    }

    fn is_held_out(&self, case_id: &str) -> bool {
        self.held_out_cases
            .iter()
            .any(|held_out| held_out == case_id)
    }

    fn is_selection(&self, case_id: &str) -> bool {
        self.selection_cases
            .iter()
            .any(|selection| selection == case_id)
            && !self.is_held_out(case_id)
    }

    fn exclusive_selection_case_ids(&self) -> BTreeSet<&str> {
        let held_out = self.held_out_case_ids();
        self.selection_cases
            .iter()
            .map(String::as_str)
            .filter(|case_id| !held_out.contains(case_id))
            .collect()
    }

    fn partition_case_ids(&self) -> BTreeSet<&str> {
        self.selection_cases
            .iter()
            .chain(self.held_out_cases.iter())
            .map(String::as_str)
            .collect()
    }

    fn overlapping_case_ids(&self) -> Vec<String> {
        let held_out = self.held_out_case_ids();
        self.selection_cases
            .iter()
            .filter(|case_id| held_out.contains(case_id.as_str()))
            .cloned()
            .collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AdaptationDeltaOptions {
    pub(crate) held_out_partition: Option<HeldOutCasePartition>,
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
        self.accepted_model_fact_keys.sort();
        self.rejected_model_fact_keys.sort();
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
            && self.accepted_model_fact_keys.is_empty()
            && self.rejected_model_fact_keys.is_empty()
    }
}

pub(crate) fn compute_adaptation_delta(
    baseline: &EvaluationRun,
    adapted: &EvaluationRun,
) -> AdaptationDeltaReport {
    compute_adaptation_delta_with_options(baseline, adapted, AdaptationDeltaOptions::default())
}

pub(crate) fn compute_adaptation_delta_with_options(
    baseline: &EvaluationRun,
    adapted: &EvaluationRun,
    options: AdaptationDeltaOptions,
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
    let common_case_ids = common_case_ids(&baseline.cases, &adapted.cases);
    let partition_case_ids = options
        .held_out_partition
        .as_ref()
        .map(HeldOutCasePartition::partition_case_ids);
    let runtime_overhead_ratio = if let Some(partition) = &options.held_out_partition {
        let measured_selection_ids =
            measured_partition_ids(&partition.exclusive_selection_case_ids(), &common_case_ids);
        let baseline_selection = cases_with_ids(&baseline.cases, &measured_selection_ids);
        let adapted_selection = cases_with_ids(&adapted.cases, &measured_selection_ids);
        runtime_overhead_ratio_for_refs(&baseline_selection, &adapted_selection)
    } else {
        runtime_overhead_ratio(&baseline.cases, &adapted.cases)
    };
    let mut report = AdaptationDeltaReport {
        baseline_suite_id: baseline.suite_id.clone(),
        adapted_suite_id: adapted.suite_id.clone(),
        runtime_overhead_ratio,
        ..AdaptationDeltaReport::default()
    };

    for case_id in case_ids {
        if let Some(partition_case_ids) = &partition_case_ids
            && (!partition_case_ids.contains(case_id) || !common_case_ids.contains(case_id))
        {
            continue;
        }
        let mut case_delta = CaseDelta {
            case_id: case_id.to_string(),
            ..CaseDelta::default()
        };
        let baseline_case = baseline_cases.get(case_id).copied();
        let adapted_case = adapted_cases.get(case_id).copied();
        collect_match_deltas(baseline_case, adapted_case, &mut case_delta);
        collect_extension_fact_deltas(baseline_case, adapted_case, &mut case_delta);
        collect_model_fact_deltas(baseline_case, adapted_case, &mut case_delta);
        let is_selection = options
            .held_out_partition
            .as_ref()
            .is_some_and(|partition| partition.is_selection(case_id));
        if let Some(partition) = &options.held_out_partition {
            case_delta.held_out = partition.is_held_out(case_id);
        }
        if !case_delta.is_empty() || case_delta.held_out {
            if options.held_out_partition.is_none() || is_selection {
                accumulate_case(&mut report, &case_delta);
            }
            case_delta.normalize();
            report.cases.push(case_delta);
        }
    }

    if let Some(partition) = &options.held_out_partition {
        report.held_out = Some(compute_held_out_delta(&baseline, &adapted, partition));
    }

    set_unique_model_fact_counts(&mut report);
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

fn set_unique_model_fact_counts(report: &mut AdaptationDeltaReport) {
    let mut accepted = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    for case in report.cases.iter().filter(|case| !case.held_out) {
        accepted.extend(case.accepted_model_fact_keys.iter().cloned());
        rejected.extend(case.rejected_model_fact_keys.iter().cloned());
    }
    report.accepted_model_facts = accepted.len() as u64;
    report.rejected_model_facts = rejected.len() as u64;
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

fn collect_model_fact_deltas(
    baseline_case: Option<&CaseResult>,
    adapted_case: Option<&CaseResult>,
    case_delta: &mut CaseDelta,
) {
    let baseline_facts = model_fact_keys(baseline_case);
    for (key, status) in model_fact_keys(adapted_case) {
        if baseline_facts.contains_key(&key) {
            continue;
        }
        match status {
            Some(ObservedStatus::Accepted) => case_delta.accepted_model_fact_keys.push(key),
            Some(ObservedStatus::Rejected) => case_delta.rejected_model_fact_keys.push(key),
            _ => {}
        }
    }
}

fn model_fact_keys(case: Option<&CaseResult>) -> BTreeMap<String, Option<ObservedStatus>> {
    case.into_iter()
        .flat_map(|case| case.observed.iter())
        .filter_map(|item| match item {
            ObservedItem::Fact(fact) if is_adaptation_model_fact(fact) => {
                Some((format!("{}:{}", fact.family, fact.stable_key), fact.status))
            }
            _ => None,
        })
        .collect()
}

fn is_adaptation_model_fact(fact: &ObservedFact) -> bool {
    is_adaptation_model_label(&fact.family)
        || fact
            .producer_id
            .as_deref()
            .is_some_and(is_adaptation_model_label)
        || fact
            .provenance
            .as_deref()
            .is_some_and(is_adaptation_model_label)
}

fn is_adaptation_model_label(label: &str) -> bool {
    label
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
        .contains("adaptationmodel")
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

fn compute_held_out_delta(
    baseline: &EvaluationRun,
    adapted: &EvaluationRun,
    partition: &HeldOutCasePartition,
) -> HeldOutDeltaReport {
    let common_case_ids = common_case_ids(&baseline.cases, &adapted.cases);
    let selection_ids = partition.exclusive_selection_case_ids();
    let held_out_ids = partition.held_out_case_ids();
    let measured_selection_ids = measured_partition_ids(&selection_ids, &common_case_ids);
    let measured_held_out_ids = measured_partition_ids(&held_out_ids, &common_case_ids);
    let baseline_cases = cases_with_ids(&baseline.cases, &measured_held_out_ids);
    let adapted_cases = cases_with_ids(&adapted.cases, &measured_held_out_ids);
    let baseline_matches = matches_for_cases(&baseline_cases);
    let adapted_matches = matches_for_cases(&adapted_cases);
    let baseline_metrics = compute_metrics(&baseline_matches);
    let adapted_metrics = compute_metrics(&adapted_matches);

    HeldOutDeltaReport {
        selection_cases: measured_selection_ids.len() as u64,
        held_out_cases: measured_held_out_ids.len() as u64,
        missing_selection_case_ids: missing_partition_ids(
            &partition.selection_cases,
            &common_case_ids,
        ),
        missing_held_out_case_ids: missing_partition_ids(
            &partition.held_out_cases,
            &common_case_ids,
        ),
        overlapping_case_ids: partition.overlapping_case_ids(),
        held_out_unknown_delta: adapted_metrics.unknown_count as i64
            - baseline_metrics.unknown_count as i64,
        held_out_precision_delta: metric_delta(
            baseline_metrics.precision,
            adapted_metrics.precision,
        ),
        held_out_recall_delta: metric_delta(baseline_metrics.recall, adapted_metrics.recall),
        held_out_runtime_overhead_ratio: runtime_overhead_ratio_for_refs(
            &baseline_cases,
            &adapted_cases,
        ),
        held_out_cache_invalidation_scope: partition.held_out_cache_invalidation_scope.clone(),
    }
}

fn common_case_ids<'a>(baseline: &'a [CaseResult], adapted: &'a [CaseResult]) -> BTreeSet<&'a str> {
    let baseline_ids = baseline
        .iter()
        .map(|case| case.case_id.as_str())
        .collect::<BTreeSet<_>>();
    adapted
        .iter()
        .filter_map(|case| {
            let case_id = case.case_id.as_str();
            baseline_ids.contains(case_id).then_some(case_id)
        })
        .collect()
}

fn measured_partition_ids<'a>(
    partition_ids: &BTreeSet<&'a str>,
    common_case_ids: &BTreeSet<&'a str>,
) -> BTreeSet<&'a str> {
    partition_ids
        .iter()
        .filter_map(|case_id| common_case_ids.contains(case_id).then_some(*case_id))
        .collect()
}

fn missing_partition_ids(
    partition_ids: &[String],
    common_case_ids: &BTreeSet<&str>,
) -> Vec<String> {
    partition_ids
        .iter()
        .filter(|case_id| !common_case_ids.contains(case_id.as_str()))
        .cloned()
        .collect()
}

fn cases_with_ids<'a>(cases: &'a [CaseResult], ids: &BTreeSet<&str>) -> Vec<&'a CaseResult> {
    cases
        .iter()
        .filter(|case| ids.contains(case.case_id.as_str()))
        .collect()
}

fn matches_for_cases(cases: &[&CaseResult]) -> Vec<crate::eval::report::MatchSummary> {
    cases
        .iter()
        .flat_map(|case| case.matches.iter().cloned())
        .collect()
}

fn metric_delta(before: Option<f64>, after: Option<f64>) -> Option<f64> {
    Some(after? - before?)
}

fn runtime_overhead_ratio_for_refs(
    baseline: &[&CaseResult],
    adapted: &[&CaseResult],
) -> Option<f64> {
    let baseline_ms = total_runtime_ms_for_refs(baseline)?;
    let adapted_ms = total_runtime_ms_for_refs(adapted)?;
    if baseline_ms == 0 {
        return None;
    }
    Some(adapted_ms as f64 / baseline_ms as f64)
}

fn total_runtime_ms_for_refs(cases: &[&CaseResult]) -> Option<u64> {
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
        assert_eq!(delta.accepted_model_facts, 0);
        assert_eq!(delta.runtime_overhead_ratio, Some(1.5));
        assert_eq!(delta.cases[0].case_id, "case-a");
        assert!(
            delta.cases[0]
                .removed_false_negative_keys
                .contains(&"diagnostic:missing".to_string())
        );
    }

    #[test]
    fn model_fact_deltas_are_reported_separately_from_extension_facts() {
        let baseline = run(vec![case("case-a", Vec::new(), Vec::new(), Some(10))]);
        let adapted = run(vec![case(
            "case-a",
            vec![summary(
                "diagnostic:new-fp",
                MatchOutcome::FalsePositive,
                MatchItemKind::Diagnostic,
            )],
            vec![
                model_fact(
                    "adaptation_model.edge",
                    "edge:/ok",
                    ObservedStatus::Accepted,
                ),
                model_fact(
                    "adaptation_model.edge",
                    "edge:/rejected",
                    ObservedStatus::Rejected,
                ),
            ],
            Some(15),
        )]);

        let delta = compute_adaptation_delta(&baseline, &adapted);

        assert_eq!(delta.new_false_positives, 1);
        assert_eq!(delta.accepted_model_facts, 1);
        assert_eq!(delta.rejected_model_facts, 1);
        assert_eq!(delta.accepted_extension_facts, 0);
        assert_eq!(delta.runtime_overhead_ratio, Some(1.5));
    }

    #[test]
    fn adaptation_model_family_facts_are_reported_without_synthetic_producer_hints() {
        let baseline = run(vec![case("case-a", Vec::new(), Vec::new(), Some(10))]);
        let adapted = run(vec![case(
            "case-a",
            Vec::new(),
            vec![ObservedItem::Fact(ObservedFact {
                family: "AdaptationModel".to_string(),
                stable_key: "edge:/real".to_string(),
                mode: AssertionMode::Exact,
                producer_id: None,
                provenance: None,
                precision: Some("heuristic".to_string()),
                status: Some(ObservedStatus::Accepted),
                payload: None,
            })],
            Some(10),
        )]);

        let delta = compute_adaptation_delta(&baseline, &adapted);

        assert_eq!(delta.accepted_model_facts, 1);
        assert_eq!(delta.rejected_model_facts, 0);
    }

    #[test]
    fn model_fact_counts_are_deduplicated_across_cases() {
        let baseline = run(vec![
            case("case-a", Vec::new(), Vec::new(), Some(10)),
            case("case-b", Vec::new(), Vec::new(), Some(10)),
        ]);
        let adapted = run(vec![
            case(
                "case-a",
                Vec::new(),
                vec![
                    model_fact(
                        "AdaptationModel",
                        "edge:/accepted",
                        ObservedStatus::Accepted,
                    ),
                    model_fact(
                        "AdaptationModel",
                        "edge:/rejected",
                        ObservedStatus::Rejected,
                    ),
                ],
                Some(10),
            ),
            case(
                "case-b",
                Vec::new(),
                vec![
                    model_fact(
                        "AdaptationModel",
                        "edge:/accepted",
                        ObservedStatus::Accepted,
                    ),
                    model_fact(
                        "AdaptationModel",
                        "edge:/rejected",
                        ObservedStatus::Rejected,
                    ),
                ],
                Some(10),
            ),
        ]);

        let delta = compute_adaptation_delta(&baseline, &adapted);

        assert_eq!(delta.accepted_model_facts, 1);
        assert_eq!(delta.rejected_model_facts, 1);
    }

    #[test]
    fn extension_adaptation_facts_do_not_count_as_model_facts() {
        let baseline = run(vec![case("case-a", Vec::new(), Vec::new(), Some(10))]);
        let adapted = run(vec![case(
            "case-a",
            Vec::new(),
            vec![extension_adaptation_fact(
                "extension.adaptation",
                "extension.adaptation.accepted_source",
                ObservedStatus::Accepted,
            )],
            Some(10),
        )]);

        let delta = compute_adaptation_delta(&baseline, &adapted);

        assert_eq!(delta.accepted_extension_facts, 1);
        assert_eq!(delta.accepted_model_facts, 0);
        assert_eq!(delta.rejected_model_facts, 0);
    }

    #[test]
    fn held_out_delta_report_labels_selection_and_held_out_cases() {
        #[derive(Deserialize)]
        struct HeldOutFixture {
            selection_cases: Vec<String>,
            held_out_cases: Vec<String>,
            held_out_unknown_delta: i64,
            held_out_precision_delta: f64,
            held_out_recall_delta: f64,
            held_out_runtime_overhead_ratio: f64,
            held_out_cache_invalidation_scope: String,
        }

        let fixture: HeldOutFixture = toml::from_str(
            &std::fs::read_to_string(
                repo_root()
                    .join("tests/eval-fixtures/adaptation-model/held-out-delta/partition.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        let partition = HeldOutCasePartition {
            selection_cases: fixture.selection_cases.clone(),
            held_out_cases: fixture.held_out_cases.clone(),
            held_out_cache_invalidation_scope: Some(
                fixture.held_out_cache_invalidation_scope.clone(),
            ),
        };
        let baseline = run(vec![
            case("case-selection-a", Vec::new(), Vec::new(), Some(1)),
            case("case-selection-b", Vec::new(), Vec::new(), Some(1)),
            case(
                "case-held-out-a",
                vec![
                    summary(
                        "diagnostic:tp",
                        MatchOutcome::TruePositive,
                        MatchItemKind::Diagnostic,
                    ),
                    summary(
                        "diagnostic:fp",
                        MatchOutcome::FalsePositive,
                        MatchItemKind::Diagnostic,
                    ),
                    summary(
                        "diagnostic:fn",
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
            ),
            case("case-held-out-b", Vec::new(), Vec::new(), Some(0)),
            case("case-held-out-c", Vec::new(), Vec::new(), Some(0)),
        ]);
        let adapted = run(vec![
            case("case-selection-a", Vec::new(), Vec::new(), Some(1)),
            case("case-selection-b", Vec::new(), Vec::new(), Some(1)),
            case(
                "case-held-out-a",
                vec![
                    summary(
                        "diagnostic:tp-1",
                        MatchOutcome::TruePositive,
                        MatchItemKind::Diagnostic,
                    ),
                    summary(
                        "diagnostic:tp-2",
                        MatchOutcome::TruePositive,
                        MatchItemKind::Diagnostic,
                    ),
                    summary(
                        "diagnostic:tp-3",
                        MatchOutcome::TruePositive,
                        MatchItemKind::Diagnostic,
                    ),
                    summary(
                        "diagnostic:fp-1",
                        MatchOutcome::FalsePositive,
                        MatchItemKind::Diagnostic,
                    ),
                    summary(
                        "diagnostic:fp-2",
                        MatchOutcome::FalsePositive,
                        MatchItemKind::Diagnostic,
                    ),
                    summary(
                        "diagnostic:fn-1",
                        MatchOutcome::FalseNegative,
                        MatchItemKind::Diagnostic,
                    ),
                    summary(
                        "diagnostic:fn-2",
                        MatchOutcome::FalseNegative,
                        MatchItemKind::Diagnostic,
                    ),
                ],
                Vec::new(),
                Some(12),
            ),
            case("case-held-out-b", Vec::new(), Vec::new(), Some(0)),
            case("case-held-out-c", Vec::new(), Vec::new(), Some(0)),
        ]);

        let delta = compute_adaptation_delta_with_options(
            &baseline,
            &adapted,
            AdaptationDeltaOptions {
                held_out_partition: Some(partition),
            },
        );
        let report = delta.held_out.unwrap();

        assert_eq!(report.selection_cases, fixture.selection_cases.len() as u64);
        assert_eq!(report.held_out_cases, fixture.held_out_cases.len() as u64);
        assert!(report.missing_selection_case_ids.is_empty());
        assert!(report.missing_held_out_case_ids.is_empty());
        assert_eq!(
            report.held_out_unknown_delta,
            fixture.held_out_unknown_delta
        );
        assert_delta(
            report.held_out_precision_delta,
            fixture.held_out_precision_delta,
        );
        assert_delta(report.held_out_recall_delta, fixture.held_out_recall_delta);
        assert_delta(
            report.held_out_runtime_overhead_ratio,
            fixture.held_out_runtime_overhead_ratio,
        );
        assert_eq!(
            report.held_out_cache_invalidation_scope.as_deref(),
            Some(fixture.held_out_cache_invalidation_scope.as_str())
        );
        assert_eq!(delta.new_true_positives, 0);
        assert_eq!(delta.new_false_positives, 0);
        assert_eq!(delta.resolved_unknowns, 0);
        assert_eq!(delta.runtime_overhead_ratio, Some(1.0));
        assert!(
            delta
                .cases
                .iter()
                .any(|case| { case.case_id == "case-held-out-b" && case.held_out })
        );
    }

    #[test]
    fn held_out_partition_excludes_unlisted_and_missing_cases_from_deltas() {
        let partition = HeldOutCasePartition {
            selection_cases: vec![
                "case-selection".to_string(),
                "case-missing-selection".to_string(),
                "case-overlap".to_string(),
            ],
            held_out_cases: vec![
                "case-held-out".to_string(),
                "case-missing-held-out".to_string(),
                "case-overlap".to_string(),
            ],
            held_out_cache_invalidation_scope: None,
        };
        let baseline = run(vec![
            case(
                "case-selection",
                vec![summary(
                    "diagnostic:selection",
                    MatchOutcome::FalseNegative,
                    MatchItemKind::Diagnostic,
                )],
                Vec::new(),
                Some(10),
            ),
            case(
                "case-held-out",
                vec![summary(
                    "diagnostic:held-out",
                    MatchOutcome::Unknown,
                    MatchItemKind::Diagnostic,
                )],
                Vec::new(),
                Some(10),
            ),
            case(
                "case-overlap",
                vec![summary(
                    "diagnostic:overlap",
                    MatchOutcome::FalseNegative,
                    MatchItemKind::Diagnostic,
                )],
                Vec::new(),
                Some(5),
            ),
            case(
                "case-missing-selection",
                vec![summary(
                    "diagnostic:missing-selection",
                    MatchOutcome::FalseNegative,
                    MatchItemKind::Diagnostic,
                )],
                Vec::new(),
                Some(10),
            ),
            case(
                "case-extra",
                vec![summary(
                    "diagnostic:extra",
                    MatchOutcome::FalseNegative,
                    MatchItemKind::Diagnostic,
                )],
                Vec::new(),
                Some(100),
            ),
        ]);
        let adapted = run(vec![
            case(
                "case-selection",
                vec![summary(
                    "diagnostic:selection",
                    MatchOutcome::TruePositive,
                    MatchItemKind::Diagnostic,
                )],
                Vec::new(),
                Some(20),
            ),
            case(
                "case-held-out",
                vec![summary(
                    "diagnostic:held-out",
                    MatchOutcome::TruePositive,
                    MatchItemKind::Diagnostic,
                )],
                Vec::new(),
                Some(20),
            ),
            case(
                "case-overlap",
                vec![summary(
                    "diagnostic:overlap",
                    MatchOutcome::TruePositive,
                    MatchItemKind::Diagnostic,
                )],
                Vec::new(),
                Some(10),
            ),
            case(
                "case-missing-held-out",
                vec![summary(
                    "diagnostic:missing-held-out",
                    MatchOutcome::TruePositive,
                    MatchItemKind::Diagnostic,
                )],
                Vec::new(),
                Some(20),
            ),
            case(
                "case-extra",
                vec![summary(
                    "diagnostic:extra",
                    MatchOutcome::TruePositive,
                    MatchItemKind::Diagnostic,
                )],
                Vec::new(),
                Some(100),
            ),
        ]);

        let delta = compute_adaptation_delta_with_options(
            &baseline,
            &adapted,
            AdaptationDeltaOptions {
                held_out_partition: Some(partition),
            },
        );
        let report = delta.held_out.as_ref().unwrap();

        assert_eq!(delta.new_true_positives, 1);
        assert_eq!(delta.removed_false_negatives, 1);
        assert_eq!(delta.resolved_unknowns, 0);
        assert_eq!(delta.runtime_overhead_ratio, Some(2.0));
        assert_eq!(report.selection_cases, 1);
        assert_eq!(report.held_out_cases, 2);
        assert_eq!(
            report.missing_selection_case_ids,
            vec!["case-missing-selection"]
        );
        assert_eq!(
            report.missing_held_out_case_ids,
            vec!["case-missing-held-out"]
        );
        assert_eq!(report.overlapping_case_ids, vec!["case-overlap"]);
        assert_eq!(report.held_out_unknown_delta, -1);
        assert!(!delta.cases.iter().any(|case| case.case_id == "case-extra"));
        assert!(
            !delta
                .cases
                .iter()
                .any(|case| case.case_id == "case-missing-selection")
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

    fn extension_adaptation_fact(
        family: &str,
        stable_key: &str,
        status: ObservedStatus,
    ) -> ObservedItem {
        ObservedItem::Fact(ObservedFact {
            family: family.to_string(),
            stable_key: stable_key.to_string(),
            mode: AssertionMode::Exact,
            producer_id: Some("polint.extension.adaptation".to_string()),
            provenance: Some("extension".to_string()),
            precision: Some("heuristic".to_string()),
            status: Some(status),
            payload: None,
        })
    }

    fn model_fact(family: &str, stable_key: &str, status: ObservedStatus) -> ObservedItem {
        ObservedItem::Fact(ObservedFact {
            family: family.to_string(),
            stable_key: stable_key.to_string(),
            mode: AssertionMode::Exact,
            producer_id: Some("polint.adaptation.model".to_string()),
            provenance: Some("adaptation_model".to_string()),
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

    fn assert_delta(actual: Option<f64>, expected: f64) {
        let actual = actual.expect("delta should be present");
        assert!(
            (actual - expected).abs() < 1e-12,
            "expected {expected}, got {actual}"
        );
    }
}
