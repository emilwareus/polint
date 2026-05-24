#![cfg(test)]

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::analysis::aliases::facts::AliasStatus;
use crate::analysis::points_to::facts::PointsToBudgetStatus;
use crate::core::{AnalysisDb, Language};

pub(crate) fn type_value_alias_debug_json_for_test(db: &AnalysisDb) -> Value {
    serde_json::to_value(TypeValueAliasDebugReport {
        counts: debug_counts(db),
        stable_keys: stable_key_rows(db),
    })
    .expect("type/value/alias debug report should serialize")
}

#[derive(Serialize)]
struct TypeValueAliasDebugReport {
    counts: TypeValueAliasDebugCounts,
    stable_keys: Vec<String>,
}

#[derive(Default, Serialize)]
struct TypeValueAliasDebugCounts {
    total_type_facts: usize,
    total_narrowed_type_facts: usize,
    total_value_facts: usize,
    total_allocation_tokens: usize,
    total_access_paths: usize,
    total_points_to_constraints: usize,
    total_points_to_sets: usize,
    total_alias_answers: usize,
    by_language: BTreeMap<String, usize>,
    by_type_status: BTreeMap<String, usize>,
    by_type_precision: BTreeMap<String, usize>,
    by_value_status: BTreeMap<String, usize>,
    by_value_precision: BTreeMap<String, usize>,
    by_alias_status: BTreeMap<String, usize>,
    by_points_to_budget: BTreeMap<String, usize>,
}

fn debug_counts(db: &AnalysisDb) -> TypeValueAliasDebugCounts {
    let mut counts = TypeValueAliasDebugCounts {
        total_type_facts: db.type_facts().len(),
        total_narrowed_type_facts: db.narrowed_type_facts().len(),
        total_value_facts: db.value_facts().len(),
        total_allocation_tokens: db.allocation_tokens().len(),
        total_access_paths: db.access_path_facts().len(),
        total_points_to_constraints: db.points_to_constraints().len(),
        total_points_to_sets: db.points_to_sets().len(),
        total_alias_answers: db.alias_answers().len(),
        ..Default::default()
    };

    for fact in db.type_facts() {
        increment(&mut counts.by_language, language_label(fact.language));
        increment(&mut counts.by_type_status, &format!("{:?}", fact.status));
        increment(
            &mut counts.by_type_precision,
            &format!("{:?}", fact.precision),
        );
    }
    for fact in db.value_facts() {
        increment(&mut counts.by_language, language_label(fact.language));
        increment(&mut counts.by_value_status, &format!("{:?}", fact.status));
        increment(
            &mut counts.by_value_precision,
            &format!("{:?}", fact.precision),
        );
    }
    for fact in db.alias_answers() {
        increment(&mut counts.by_alias_status, alias_status_label(fact.status));
    }
    for fact in db.points_to_sets() {
        increment(&mut counts.by_points_to_budget, budget_label(fact.budget));
    }

    counts
}

fn stable_key_rows(db: &AnalysisDb) -> Vec<String> {
    let mut rows = Vec::new();
    rows.extend(
        db.type_facts()
            .iter()
            .map(|fact| format!("type:{}", fact.stable_key)),
    );
    rows.extend(
        db.narrowed_type_facts()
            .iter()
            .map(|fact| format!("narrowed_type:{}", fact.stable_key)),
    );
    rows.extend(
        db.value_facts()
            .iter()
            .map(|fact| format!("value:{}", fact.stable_key)),
    );
    rows.extend(
        db.allocation_tokens()
            .iter()
            .map(|fact| format!("allocation:{}", fact.stable_key)),
    );
    rows.extend(
        db.access_path_facts()
            .iter()
            .map(|fact| format!("access_path:{}", fact.stable_key)),
    );
    rows.extend(
        db.points_to_constraints()
            .iter()
            .map(|fact| format!("points_to_constraint:{}", fact.stable_key)),
    );
    rows.extend(
        db.points_to_sets()
            .iter()
            .map(|fact| format!("points_to_set:{}", fact.stable_key)),
    );
    rows.extend(
        db.alias_answers()
            .iter()
            .map(|fact| format!("alias_answer:{}", fact.stable_key)),
    );
    rows.sort();
    rows
}

fn increment(counts: &mut BTreeMap<String, usize>, key: &str) {
    *counts.entry(key.to_string()).or_default() += 1;
}

fn language_label(language: Language) -> &'static str {
    match language {
        Language::Go => "go",
        Language::TypeScript => "typescript",
        Language::Tsx => "tsx",
        Language::JavaScript => "javascript",
        Language::Jsx => "jsx",
        Language::Unknown => "unknown",
    }
}

fn alias_status_label(status: AliasStatus) -> &'static str {
    match status {
        AliasStatus::NoAlias => "NoAlias",
        AliasStatus::MayAlias => "MayAlias",
        AliasStatus::MustAlias => "MustAlias",
        AliasStatus::PartialAlias => "PartialAlias",
        AliasStatus::Unknown => "Unknown",
    }
}

fn budget_label(status: PointsToBudgetStatus) -> &'static str {
    match status {
        PointsToBudgetStatus::WithinBudget => "WithinBudget",
        PointsToBudgetStatus::BudgetExceeded => "BudgetExceeded",
        PointsToBudgetStatus::NotRun => "NotRun",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_type_value_alias_debug_is_deterministic_and_path_free() {
        let db = AnalysisDb::new();
        let first = type_value_alias_debug_json_for_test(&db);
        let second = type_value_alias_debug_json_for_test(&db);

        assert_eq!(first, second);
        let text = serde_json::to_string(&first).expect("json");
        assert!(!text.contains("/Users/"));
        assert!(!text.contains("parser_id"));
        assert!(!text.contains("timestamp"));
    }
}
