use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::analysis::identity::categorize::IdentityCategory;
use crate::cache::stable_hash;
use crate::eval::adaptation::AdaptationRecord;
use crate::eval::competitors::BenchmarkComparisonRow;
use crate::eval::delta::AdaptationDeltaReport;
use crate::eval::matcher::{MatchItemKind, MatchOutcome};
use crate::eval::model::{
    AssertionMode, EvaluationMode, ExpectedFact, ExpectedGraphEdge, ExpectedInvariant,
    ExpectedItem, ExpectedPath, ExpectedRuntimeBudget, FixtureArea, ObservedFact,
    ObservedGraphEdge, ObservedInvariant, ObservedItem, ObservedPath, ObservedRuntimeBudget,
    ObservedStatus,
};
use crate::eval::performance::{self, EvalPerformanceReport};
use crate::eval::suite::SuiteManifest;

pub(crate) const EVALUATION_SCHEMA_VERSION: &str = "polint-eval-internal-1";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct EvaluationRun {
    pub(crate) schema_version: String,
    pub(crate) suite_id: String,
    pub(crate) mode: EvaluationMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) suite_manifest: Option<SuiteManifest>,
    pub(crate) cases: Vec<CaseResult>,
    pub(crate) metrics: MetricSummary,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) performance: Option<EvalPerformanceReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) comparison_rows: Vec<BenchmarkComparisonRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) adaptation: Option<AdaptationRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) adaptation_delta: Option<AdaptationDeltaReport>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) limitations: Vec<String>,
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
    #[serde(default)]
    pub(crate) sections: MetricSections,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct MetricSections {
    pub(crate) scanner: ScannerMetricSection,
    pub(crate) graph: GraphMetricSection,
    pub(crate) paths: PathMetricSection,
    pub(crate) unknowns: UnknownMetricSection,
    pub(crate) performance: PerformanceMetricSection,
    #[serde(default)]
    pub(crate) suite_native: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) adaptation: Option<AdaptationMetricSection>,
    #[serde(default)]
    pub(crate) jelly_oracle_coverage: JellyOracleCoverageSection,
    #[serde(default)]
    pub(crate) categorized_failures: CategorizedFailureSection,
    #[serde(default)]
    pub(crate) solver: SolverMetricSection,
}

/// One Jelly oracle span that no identity record renders, surfaced for
/// debuggability (D-21).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct JellyUnmatchedSpan {
    pub(crate) file: String,
    pub(crate) span: String,
    pub(crate) reason: String,
}

/// Deterministic Jelly oracle-span coverage over the Jelly micro fixture set
/// (D-20, D-21). `ratio = matched / total` (or `1.0` when `total == 0`); the
/// `unmatched` list carries one entry per oracle span that no identity record
/// renders.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct JellyOracleCoverageSection {
    pub(crate) matched: u32,
    pub(crate) total: u32,
    pub(crate) ratio: f64,
    pub(crate) unmatched: Vec<JellyUnmatchedSpan>,
}

/// Closed identity-vs-unsupported failure counter map (D-14, D-15).
///
/// Exactly five `u32` counters, one per [`IdentityCategory`] variant, with
/// snake_case discriminators matching the enum's serde representation. Placed as
/// a sibling of [`JellyOracleCoverageSection`] on [`MetricSections`] (Plan 42-03
/// BLOCKER #3). `#[serde(default)]` on the `MetricSections` field keeps older
/// v1.2 JSON deserializable (Pattern M).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct CategorizedFailureSection {
    pub(crate) wrong_identity: u32,
    pub(crate) unsupported_edge: u32,
    pub(crate) unresolved_edge: u32,
    pub(crate) package_load_limitation: u32,
    pub(crate) model_missing: u32,
}

impl CategorizedFailureSection {
    /// Increments the counter for `category`. Uses `saturating_add` so a hostile
    /// repo with tens of millions of failing edges saturates at `u32::MAX`
    /// instead of wrapping (threat T-42-03-05). Exhaustive `match`, no wildcard
    /// arm (Pattern H consistency).
    pub(crate) fn record_category(&mut self, category: IdentityCategory) {
        match category {
            IdentityCategory::WrongIdentity => {
                self.wrong_identity = self.wrong_identity.saturating_add(1);
            }
            IdentityCategory::UnsupportedEdge => {
                self.unsupported_edge = self.unsupported_edge.saturating_add(1);
            }
            IdentityCategory::UnresolvedEdge => {
                self.unresolved_edge = self.unresolved_edge.saturating_add(1);
            }
            IdentityCategory::PackageLoadLimitation => {
                self.package_load_limitation = self.package_load_limitation.saturating_add(1);
            }
            IdentityCategory::ModelMissing => {
                self.model_missing = self.model_missing.saturating_add(1);
            }
        }
    }
}

/// RESERVED solver metrics, defaulted to zero/empty in Phase 43 (D-23).
///
/// `solver_step_count` (default `0`) and `budget_exceeded_reasons` (default
/// empty) reserve the JSON shape the unified call-graph solver introduced in
/// **Phase 47+** will populate. They live here on [`MetricSections`] — a
/// `#[serde(default)]` sibling of [`CategorizedFailureSection`] — and NOT on the
/// frozen [`MetricSummary`] (which is layout-locked by
/// `metric_summary_layout_unchanged`). Reserving the shape now keeps the N=10
/// byte-identical determinism gate (`eval::determinism_gate`) stable across the
/// whole v1.3 milestone: when Phase 47+ starts emitting real values, the
/// observed-JSON shape does not change, only the values, so no fixture or gate
/// breaks merely because the section appeared. `#[serde(default)]` on the
/// `MetricSections` field keeps older v1.2 report JSON (which lacks the `solver`
/// section entirely) deserializable (Pattern M, threat T-43-03-02).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct SolverMetricSection {
    pub(crate) solver_step_count: u64,
    pub(crate) budget_exceeded_reasons: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ScannerMetricSection {
    pub(crate) true_positives: u64,
    pub(crate) false_positives: u64,
    pub(crate) false_negatives: u64,
    pub(crate) true_negatives: u64,
    pub(crate) precision: Option<f64>,
    pub(crate) recall: Option<f64>,
    pub(crate) f1: Option<f64>,
    pub(crate) f2: Option<f64>,
    pub(crate) f3: Option<f64>,
    pub(crate) false_positive_rate: Option<f64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct GraphMetricSection {
    pub(crate) edges_expected: u64,
    pub(crate) edges_observed: u64,
    pub(crate) edges_unconfirmed: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct PathMetricSection {
    pub(crate) paths_expected: u64,
    pub(crate) paths_observed: u64,
    pub(crate) paths_unconfirmed: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct UnknownMetricSection {
    pub(crate) total: u64,
    #[serde(default)]
    pub(crate) by_status: BTreeMap<String, u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct PerformanceMetricSection {
    pub(crate) runtime_budget_passed: u64,
    pub(crate) runtime_budget_failed: u64,
    pub(crate) cache_hits: u64,
    pub(crate) cache_misses: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct AdaptationMetricSection {
    pub(crate) resolved_unknowns: u64,
    pub(crate) new_false_positives: u64,
    pub(crate) removed_false_negatives: u64,
    pub(crate) accepted_extension_facts: u64,
    pub(crate) rejected_extension_facts: u64,
    #[serde(default)]
    pub(crate) accepted_model_facts: u64,
    #[serde(default)]
    pub(crate) rejected_model_facts: u64,
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
        case.expected.sort_by_cached_key(expected_item_sort_key);
        case.observed.sort_by_cached_key(observed_item_sort_key);
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
        .comparison_rows
        .sort_by_cached_key(canonical_json_key);
    normalized.limitations.sort();
    if let Some(performance) = &mut normalized.performance {
        performance.sync_peak_rss_from_runtime();
        performance.providers.sort();
        performance.demand_queries.sort();
    }
    if let Some(delta) = &mut normalized.adaptation_delta {
        delta.normalize();
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
    if let Some(performance) = &mut normalized.performance {
        performance::strip_volatile_runtime(performance);
    }
    normalized = normalize_run(&normalized);
    let canonical_json =
        serde_json::to_string_pretty(&normalized).unwrap_or_else(|_| "{}".to_string());
    stable_hash(&[canonical_json.as_str()])
}

fn expected_item_sort_key(item: &ExpectedItem) -> (String, String) {
    (expected_item_key(item), canonical_json_key(item))
}

fn observed_item_sort_key(item: &ObservedItem) -> (String, String) {
    (observed_item_key(item), canonical_json_key(item))
}

fn canonical_json_key<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_default()
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
        ObservedStatus::Resolved => "resolved",
        ObservedStatus::Partial => "partial",
        ObservedStatus::Top => "top",
        ObservedStatus::Unknown => "unknown",
        ObservedStatus::Unresolved => "unresolved",
        ObservedStatus::Ambiguous => "ambiguous",
        ObservedStatus::Dynamic => "dynamic",
        ObservedStatus::SetupMissing => "setup_missing",
        ObservedStatus::MissingLockfile => "missing_lockfile",
        ObservedStatus::Unsupported => "unsupported",
        ObservedStatus::External => "external",
        ObservedStatus::Cycle => "cycle",
        ObservedStatus::Generated => "generated",
        ObservedStatus::Undeclared => "undeclared",
        ObservedStatus::OutsideWorkspace => "outside_workspace",
        ObservedStatus::BudgetExceeded => "budget_exceeded",
        ObservedStatus::Rejected => "rejected",
        ObservedStatus::Accepted => "accepted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::adaptation::{
        ADAPTATION_SCHEMA_VERSION, AdaptationAgent, AdaptationBudget, AdaptationOutputs,
        AdaptationRecord,
    };
    use crate::eval::competitors::{BenchmarkComparisonRow, ProductIdentity, ResultSource};
    use crate::eval::model::{
        AssertionMode, EvaluationMode, ExpectedDiagnostic, ExpectedFact, ExpectedGraphEdge,
        ExpectedInvariant, ExpectedItem, ExpectedPath, ExpectedRuntimeBudget, FixtureArea,
        ObservedDiagnostic, ObservedFact, ObservedGraphEdge, ObservedInvariant, ObservedItem,
        ObservedPath, ObservedRuntimeBudget, ObservedStatus,
    };
    use crate::eval::performance::{
        CacheStatsSummary, DemandQueryStatsRow, EvalPerformanceReport, ProviderStatsRow,
        RuntimeStatsSummary,
    };
    use crate::eval::suite::SuiteId;

    #[test]
    fn jelly_oracle_coverage_serde_round_trip() {
        let section = JellyOracleCoverageSection {
            matched: 98,
            total: 99,
            ratio: 98.0 / 99.0,
            unmatched: vec![JellyUnmatchedSpan {
                file: "tests/micro/app.js".to_string(),
                span: "tests/micro/app.js:7:1:9:2".to_string(),
                reason: "no identity record renders this span".to_string(),
            }],
        };
        let json = serde_json::to_string(&section).unwrap();
        let restored: JellyOracleCoverageSection = serde_json::from_str(&json).unwrap();
        assert_eq!(section, restored);

        let default = JellyOracleCoverageSection::default();
        let default_json = serde_json::to_string(&default).unwrap();
        let default_restored: JellyOracleCoverageSection =
            serde_json::from_str(&default_json).unwrap();
        assert_eq!(default, default_restored);
    }

    #[test]
    fn v1_2_metric_sections_json_reverse_compat_jelly() {
        // A v1.2 MetricSections JSON has no `jelly_oracle_coverage` field; it must
        // still deserialize, defaulting the new section (Pattern M, #[serde(default)]).
        let v1_2_json = serde_json::json!({
            "scanner": {
                "true_positives": 0,
                "false_positives": 0,
                "false_negatives": 0,
                "true_negatives": 0,
                "precision": null,
                "recall": null,
                "f1": null,
                "f2": null,
                "f3": null,
                "false_positive_rate": null
            },
            "graph": { "edges_expected": 0, "edges_observed": 0, "edges_unconfirmed": 0 },
            "paths": { "paths_expected": 0, "paths_observed": 0, "paths_unconfirmed": 0 },
            "unknowns": { "total": 0, "by_status": {} },
            "performance": {
                "runtime_budget_passed": 0,
                "runtime_budget_failed": 0,
                "cache_hits": 0,
                "cache_misses": 0
            },
            "suite_native": {}
        });
        let sections: MetricSections = serde_json::from_value(v1_2_json).unwrap();
        assert_eq!(
            sections.jelly_oracle_coverage,
            JellyOracleCoverageSection::default()
        );
    }

    #[test]
    fn categorized_failures_serde_round_trip() {
        let section = CategorizedFailureSection {
            wrong_identity: 1,
            unsupported_edge: 2,
            unresolved_edge: 3,
            package_load_limitation: 4,
            model_missing: 5,
        };
        let json = serde_json::to_string(&section).unwrap();
        let restored: CategorizedFailureSection = serde_json::from_str(&json).unwrap();
        assert_eq!(section, restored);

        // Default-serialization byte string is the downstream lock contract.
        let default_json = serde_json::to_string(&CategorizedFailureSection::default()).unwrap();
        assert_eq!(
            default_json,
            r#"{"wrong_identity":0,"unsupported_edge":0,"unresolved_edge":0,"package_load_limitation":0,"model_missing":0}"#
        );
    }

    #[test]
    fn v1_2_metric_sections_json_reverse_compat() {
        // A v1.2 MetricSections JSON carries neither `jelly_oracle_coverage` nor
        // `categorized_failures`; both must default in (Pattern M).
        let v1_2_json = serde_json::json!({
            "scanner": {
                "true_positives": 0,
                "false_positives": 0,
                "false_negatives": 0,
                "true_negatives": 0,
                "precision": null,
                "recall": null,
                "f1": null,
                "f2": null,
                "f3": null,
                "false_positive_rate": null
            },
            "graph": { "edges_expected": 0, "edges_observed": 0, "edges_unconfirmed": 0 },
            "paths": { "paths_expected": 0, "paths_observed": 0, "paths_unconfirmed": 0 },
            "unknowns": { "total": 0, "by_status": {} },
            "performance": {
                "runtime_budget_passed": 0,
                "runtime_budget_failed": 0,
                "cache_hits": 0,
                "cache_misses": 0
            },
            "suite_native": {}
        });
        let sections: MetricSections = serde_json::from_value(v1_2_json).unwrap();
        assert_eq!(
            sections.categorized_failures,
            CategorizedFailureSection::default()
        );
    }

    #[test]
    fn record_category_increments_each_counter() {
        let mut section = CategorizedFailureSection::default();
        section.record_category(IdentityCategory::WrongIdentity);
        section.record_category(IdentityCategory::UnsupportedEdge);
        section.record_category(IdentityCategory::UnresolvedEdge);
        section.record_category(IdentityCategory::PackageLoadLimitation);
        section.record_category(IdentityCategory::ModelMissing);
        assert_eq!(
            section,
            CategorizedFailureSection {
                wrong_identity: 1,
                unsupported_edge: 1,
                unresolved_edge: 1,
                package_load_limitation: 1,
                model_missing: 1,
            }
        );

        // A second WrongIdentity event bumps only that counter.
        section.record_category(IdentityCategory::WrongIdentity);
        assert_eq!(section.wrong_identity, 2);
    }

    #[test]
    fn drive_record_category_model_missing() {
        // BLOCKER #4: prove the fifth (ModelMissing) counter's wiring directly so
        // it is non-zero in the test suite even if v1.2 source never naturally
        // emits CallTargetStatus::Rejected on the fixture repos.
        let mut section = CategorizedFailureSection::default();
        section.record_category(IdentityCategory::ModelMissing);
        assert_eq!(section.model_missing, 1);
        assert_eq!(section.wrong_identity, 0);
        assert_eq!(section.unsupported_edge, 0);
        assert_eq!(section.unresolved_edge, 0);
        assert_eq!(section.package_load_limitation, 0);
    }

    #[test]
    fn solver_metric_section_defaults_to_zero_and_empty() {
        // D-23: a fresh SolverMetricSection serializes with solver_step_count = 0
        // and budget_exceeded_reasons = []. This default-serialization byte string
        // is the downstream lock contract the determinism gate inherits.
        let section = SolverMetricSection::default();
        assert_eq!(section.solver_step_count, 0);
        assert!(section.budget_exceeded_reasons.is_empty());

        let default_json = serde_json::to_string(&SolverMetricSection::default()).unwrap();
        assert_eq!(
            default_json,
            r#"{"solver_step_count":0,"budget_exceeded_reasons":[]}"#
        );
    }

    #[test]
    fn solver_metric_section_serde_round_trip() {
        let section = SolverMetricSection {
            solver_step_count: 42,
            budget_exceeded_reasons: vec!["fanout_budget".to_string(), "token_budget".to_string()],
        };
        let json = serde_json::to_string(&section).unwrap();
        let restored: SolverMetricSection = serde_json::from_str(&json).unwrap();
        assert_eq!(section, restored);
    }

    #[test]
    fn v1_2_metric_sections_json_without_solver_section_reverse_compat() {
        // A v1.2 (and Phase 42) MetricSections JSON carries no `solver` section at
        // all; #[serde(default)] on the MetricSections field must default it in so
        // older report JSON still deserializes (Pattern M, threat T-43-03-02).
        let older_json = serde_json::json!({
            "scanner": {
                "true_positives": 0,
                "false_positives": 0,
                "false_negatives": 0,
                "true_negatives": 0,
                "precision": null,
                "recall": null,
                "f1": null,
                "f2": null,
                "f3": null,
                "false_positive_rate": null
            },
            "graph": { "edges_expected": 0, "edges_observed": 0, "edges_unconfirmed": 0 },
            "paths": { "paths_expected": 0, "paths_observed": 0, "paths_unconfirmed": 0 },
            "unknowns": { "total": 0, "by_status": {} },
            "performance": {
                "runtime_budget_passed": 0,
                "runtime_budget_failed": 0,
                "cache_hits": 0,
                "cache_misses": 0
            },
            "suite_native": {}
        });
        let sections: MetricSections = serde_json::from_value(older_json).unwrap();
        assert_eq!(sections.solver, SolverMetricSection::default());
    }

    #[test]
    fn solver_metric_section_layout_unchanged() {
        // Destructure layout-lock mirroring metric_summary_layout_unchanged: the
        // SolverMetricSection field set is reserved as exactly solver_step_count +
        // budget_exceeded_reasons. Adding a field forces this test to update,
        // signaling a deliberate shape change of the reserved solver JSON (D-23).
        let section = SolverMetricSection::default();
        let SolverMetricSection {
            solver_step_count: _,
            budget_exceeded_reasons: _,
        } = section;
    }

    #[test]
    fn metric_summary_layout_unchanged() {
        // Compile-time + structural lock on the MetricSummary field set
        // (WARNING #1): downstream gates lock this shape. Extensions live on
        // MetricSections only. Destructuring with every current field name fails
        // to compile if a field is added or removed.
        let summary = report_with_order(Ordering::Forward).metrics;
        let MetricSummary {
            true_positives: _,
            false_positives: _,
            false_negatives: _,
            true_negatives: _,
            unconfirmed: _,
            false_positive_trap_hits: _,
            forbidden_hits: _,
            unknown_count: _,
            facts_present: _,
            facts_accepted: _,
            facts_rejected: _,
            graph_edges_expected: _,
            graph_edges_observed: _,
            graph_edges_unconfirmed: _,
            paths_expected: _,
            paths_observed: _,
            paths_unconfirmed: _,
            runtime_budget_passed: _,
            runtime_budget_failed: _,
            precision: _,
            recall: _,
            f1: _,
            f2: _,
            f3: _,
            false_positive_rate: _,
            sections: _,
        } = summary;
    }

    #[test]
    fn eval_report_normalization_makes_json_order_independent() {
        let left = to_deterministic_json_pretty(&report_with_order(Ordering::Forward));
        let right = to_deterministic_json_pretty(&report_with_order(Ordering::Reverse));

        assert_eq!(left, right);
    }

    #[test]
    fn eval_report_normalization_orders_equal_identity_items_by_serialized_fields() {
        let left = report_with_order(Ordering::Forward);
        let right = report_with_order(Ordering::Reverse);

        assert_eq!(
            to_deterministic_json_pretty(&left),
            to_deterministic_json_pretty(&right)
        );
        assert_eq!(
            deterministic_output_hash(&left),
            deterministic_output_hash(&right)
        );
    }

    #[test]
    fn eval_report_hash_excludes_runtime_and_machine_local_fields_by_shape() {
        let mut left = report_with_order(Ordering::Forward);
        let mut right = left.clone();
        left.cases[0].runtime.observed_runtime_ms = Some(17);
        right.cases[0].runtime.observed_runtime_ms = Some(999);
        left.performance = Some(performance_report(17));
        right.performance = Some(performance_report(999));

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

    #[test]
    fn eval_report_contains_separate_comparison_rows_for_three_benchmark_modes() {
        let mut run = report_with_order(Ordering::Forward);
        run.comparison_rows = vec![
            comparison_row("Semgrep", EvaluationMode::ImportedScanner),
            comparison_row("polint", EvaluationMode::PolintAgentAdapted),
            comparison_row("polint", EvaluationMode::PolintBaseline),
        ];

        let json = to_deterministic_json_pretty(&run);

        assert!(json.contains("\"mode\": \"imported_scanner\""));
        assert!(json.contains("\"mode\": \"polint_baseline\""));
        assert!(json.contains("\"mode\": \"polint_agent_adapted\""));
        assert!(json.contains("\"product\""));
    }

    #[test]
    fn eval_report_hash_includes_comparison_and_adaptation_metadata() {
        let reference = report_with_order(Ordering::Forward);
        let mut with_comparison = reference.clone();
        with_comparison.comparison_rows = vec![comparison_row(
            "CodeQL",
            EvaluationMode::LocallyReproducedScanner,
        )];
        assert_ne!(
            deterministic_output_hash(&reference),
            deterministic_output_hash(&with_comparison)
        );

        let mut with_adaptation = reference.clone();
        with_adaptation.mode = EvaluationMode::PolintAgentAdapted;
        with_adaptation.adaptation = Some(adaptation_record("prompt-hash-a"));
        assert_ne!(
            deterministic_output_hash(&reference),
            deterministic_output_hash(&with_adaptation)
        );
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
            mode: EvaluationMode::PolintBaseline,
            suite_manifest: None,
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

    fn comparison_row(product: &str, mode: EvaluationMode) -> BenchmarkComparisonRow {
        BenchmarkComparisonRow {
            suite_id: SuiteId("deterministic-suite".to_string()),
            suite_commit: Some("suite-commit".to_string()),
            mode,
            product: ProductIdentity {
                name: product.to_string(),
                version: Some("1.0.0".to_string()),
                vendor: None,
            },
            result_source: ResultSource::PolintRun {
                report_path: format!("target/polint-eval/{product}.json"),
                config_digest: Some("config-digest".to_string()),
            },
            metrics: [("precision".to_string(), 0.8)].into_iter().collect(),
            limitations: Vec::new(),
        }
    }

    fn adaptation_record(prompt_hash: &str) -> AdaptationRecord {
        AdaptationRecord {
            schema_version: ADAPTATION_SCHEMA_VERSION.to_string(),
            suite_id: SuiteId("deterministic-suite".to_string()),
            case_selection: "fast".to_string(),
            agent: AdaptationAgent {
                kind: "subagent".to_string(),
                model: "inherit".to_string(),
                prompt_path: "research/evaluation-harness/prompts/default-adaptation-agent.md"
                    .to_string(),
                prompt_hash: prompt_hash.to_string(),
                budget: AdaptationBudget {
                    wall_time_minutes: 60,
                    max_iterations: 5,
                },
            },
            inputs_allowed: vec!["target repository source".to_string()],
            inputs_forbidden: vec!["expected labels before adaptation".to_string()],
            sandbox_root: Some("target/polint-eval/adaptation-sandbox".to_string()),
            outputs: AdaptationOutputs {
                rules_or_extensions_changed: Vec::new(),
                rule_digests: Vec::new(),
                extension_digests: Vec::new(),
                model_digests: Vec::new(),
                notes_path: "target/polint-eval/adaptation-notes.md".to_string(),
                final_adapted_report_path: "target/polint-eval/adapted.json".to_string(),
                no_change_reason: Some("report hash fixture uses metadata only".to_string()),
                commands_run: Vec::new(),
            },
        }
    }

    fn performance_report(runtime: u64) -> EvalPerformanceReport {
        EvalPerformanceReport {
            providers: vec![ProviderStatsRow {
                provider_id: "polint.source".to_string(),
                provider_version: "1".to_string(),
                schema_version: "source".to_string(),
                output_digest: "digest".to_string(),
                precision: "exact".to_string(),
                validation: "native_trusted".to_string(),
                dependency_input_count: 0,
                facts_emitted: None,
                diagnostics_emitted: None,
                validation_rejections: None,
                cache: CacheStatsSummary::default(),
                observed_runtime_ms: Some(runtime),
            }],
            cache: CacheStatsSummary::default(),
            demand_queries: vec![DemandQueryStatsRow {
                query_kind: "calls".to_string(),
                query_version: "1".to_string(),
                parameter_digest: "params".to_string(),
                cache_status: "computed".to_string(),
                result_digest: "result".to_string(),
                precision_tier: "setup_aware".to_string(),
                compute_duration_micros: Some(runtime * 1000),
            }],
            runtime: RuntimeStatsSummary {
                observed_runtime_ms: Some(runtime),
                peak_rss_bytes: Some(runtime * 1024),
            },
            rss: Default::default(),
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
            ExpectedItem::Fact(ExpectedFact {
                family: "symbols".to_string(),
                stable_key: "fact:module:handler".to_string(),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.symbol_graph".to_string()),
                precision: Some("syntactic".to_string()),
                status: Some(ObservedStatus::Present),
                false_positive_trap: true,
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
                payload: None,
            }),
            ObservedItem::Fact(ObservedFact {
                family: "symbols".to_string(),
                stable_key: "fact:module:handler".to_string(),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.symbol_graph".to_string()),
                provenance: Some("native-provider".to_string()),
                precision: Some("syntactic".to_string()),
                status: Some(ObservedStatus::Present),
                payload: None,
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
