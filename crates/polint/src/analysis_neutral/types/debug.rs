use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::analysis_neutral::AnalysisHost;
use crate::analysis_neutral::aliases::facts::AliasStatus;
use crate::analysis_neutral::points_to::facts::PointsToBudgetStatus;
use crate::analysis_neutral::types::facts::TypeShape;
use crate::analysis_neutral::values::facts::ValueKind;
use crate::internal_core::Language;

pub fn type_value_alias_debug_json_for_test(db: &impl AnalysisHost) -> Value {
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
    by_provenance: BTreeMap<String, usize>,
    by_fact_family: BTreeMap<String, usize>,
    by_unknown_or_unsupported_reason: BTreeMap<String, usize>,
}

fn debug_counts(db: &impl AnalysisHost) -> TypeValueAliasDebugCounts {
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

    increment_by(&mut counts.by_fact_family, "Type", counts.total_type_facts);
    increment_by(
        &mut counts.by_fact_family,
        "NarrowedType",
        counts.total_narrowed_type_facts,
    );
    increment_by(
        &mut counts.by_fact_family,
        "Value",
        counts.total_value_facts,
    );
    increment_by(
        &mut counts.by_fact_family,
        "AllocationToken",
        counts.total_allocation_tokens,
    );
    increment_by(
        &mut counts.by_fact_family,
        "AccessPath",
        counts.total_access_paths,
    );
    increment_by(
        &mut counts.by_fact_family,
        "PointsToConstraint",
        counts.total_points_to_constraints,
    );
    increment_by(
        &mut counts.by_fact_family,
        "PointsToSet",
        counts.total_points_to_sets,
    );
    increment_by(
        &mut counts.by_fact_family,
        "AliasAnswer",
        counts.total_alias_answers,
    );

    for fact in db.type_facts() {
        increment(&mut counts.by_language, language_label(fact.language));
        increment(&mut counts.by_type_status, &format!("{:?}", fact.status));
        increment(
            &mut counts.by_type_precision,
            &format!("{:?}", fact.precision),
        );
        increment(&mut counts.by_provenance, &format!("{:?}", fact.provenance));
        if let TypeShape::Unknown { reason } | TypeShape::Unsupported { reason } = &fact.shape {
            increment(&mut counts.by_unknown_or_unsupported_reason, reason);
        }
    }
    for fact in db.value_facts() {
        increment(&mut counts.by_language, language_label(fact.language));
        increment(&mut counts.by_value_status, &format!("{:?}", fact.status));
        increment(
            &mut counts.by_value_precision,
            &format!("{:?}", fact.precision),
        );
        increment(&mut counts.by_provenance, &format!("{:?}", fact.provenance));
        if let ValueKind::Unknown { evidence } = &fact.kind {
            increment(&mut counts.by_unknown_or_unsupported_reason, evidence);
        }
    }
    for fact in db.allocation_tokens() {
        increment(&mut counts.by_provenance, &format!("{:?}", fact.provenance));
    }
    for fact in db.alias_answers() {
        increment(&mut counts.by_alias_status, alias_status_label(fact.status));
    }
    for fact in db.points_to_sets() {
        increment(&mut counts.by_points_to_budget, budget_label(fact.budget));
    }

    counts
}

fn stable_key_rows(db: &impl AnalysisHost) -> Vec<String> {
    let interner = db.stable_key_interner();
    let mut rows = Vec::new();
    rows.extend(
        db.type_facts()
            .iter()
            .map(|fact| format!("type:{}", interner.resolve(fact.stable_key))),
    );
    rows.extend(
        db.narrowed_type_facts()
            .iter()
            .map(|fact| format!("narrowed_type:{}", interner.resolve(fact.stable_key))),
    );
    rows.extend(
        db.value_facts()
            .iter()
            .map(|fact| format!("value:{}", interner.resolve(fact.stable_key))),
    );
    rows.extend(
        db.allocation_tokens()
            .iter()
            .map(|fact| format!("allocation:{}", interner.resolve(fact.stable_key))),
    );
    rows.extend(
        db.access_path_facts()
            .iter()
            .map(|fact| format!("access_path:{}", interner.resolve(fact.stable_key))),
    );
    rows.extend(
        db.points_to_constraints()
            .iter()
            .map(|fact| format!("points_to_constraint:{}", interner.resolve(fact.stable_key))),
    );
    rows.extend(
        db.points_to_sets()
            .iter()
            .map(|fact| format!("points_to_set:{}", interner.resolve(fact.stable_key))),
    );
    rows.extend(
        db.alias_answers()
            .iter()
            .map(|fact| format!("alias_answer:{}", interner.resolve(fact.stable_key))),
    );
    rows.sort();
    rows
}

fn increment(counts: &mut BTreeMap<String, usize>, key: &str) {
    *counts.entry(key.to_string()).or_default() += 1;
}

fn increment_by(counts: &mut BTreeMap<String, usize>, key: &str, amount: usize) {
    if amount == 0 {
        return;
    }
    *counts.entry(key.to_string()).or_default() += amount;
}

fn language_label(language: Language) -> &'static str {
    match language {
        Language::Go => "go",
        Language::TypeScript => "typescript",
        Language::Tsx => "tsx",
        Language::JavaScript => "javascript",
        Language::Jsx => "jsx",
        Language::Unknown => "unknown",
        _ => "unknown",
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
    use crate::analysis_neutral::LocalAnalysisDb;
    use crate::analysis_neutral::access_paths::facts::{AccessPathFact, AccessPathStatus};
    use crate::analysis_neutral::access_paths::store::AccessPathOutput;
    use crate::analysis_neutral::aliases::facts::{
        AliasAnswerFact, AliasOperand, AliasPrecision, AliasReason,
    };
    use crate::analysis_neutral::aliases::store::AliasOutput;
    use crate::analysis_neutral::ids::{
        AccessPathId, AliasAnswerId, AllocationTokenId, ObjectTokenId, PlaceId, PointsToSetId,
        PtVarId, TypeFactId, TypeSetId, ValueFactId,
    };
    use crate::analysis_neutral::points_to::facts::{
        PointsToPrecision, PointsToSetFact, PointsToStatus,
    };
    use crate::analysis_neutral::points_to::store::PointsToOutput;
    use crate::analysis_neutral::types::facts::{
        TypeConfidence, TypeFact, TypePhase, TypePrecision, TypeProvenance, TypeStatus, TypeSubject,
    };
    use crate::analysis_neutral::types::store::{TypeOutput, TypeValueAliasOutput};
    use crate::analysis_neutral::values::facts::{
        AllocationKind, AllocationTokenFact, ValueFact, ValuePrecision, ValueProvenance,
        ValueStatus, ValueSubject,
    };
    use crate::analysis_neutral::values::store::ValueOutput;

    #[test]
    fn empty_type_value_alias_debug_is_deterministic_and_path_free() {
        let db = LocalAnalysisDb::new();
        let first = type_value_alias_debug_json_for_test(&db);
        let second = type_value_alias_debug_json_for_test(&db);

        assert_eq!(first, second);
        let text = serde_json::to_string(&first).expect("json");
        assert!(!text.contains("/Users/"));
        assert!(!text.contains("parser_id"));
        assert!(!text.contains("timestamp"));
    }

    #[test]
    fn type_value_alias_debug_counts_alias_statuses_deterministically() {
        let mut db = LocalAnalysisDb::new();
        db.replace_type_value_alias_facts(TypeValueAliasOutput {
            aliases: AliasOutput {
                answers: vec![
                    alias(AliasStatus::NoAlias, "alias:no"),
                    alias(AliasStatus::MayAlias, "alias:may"),
                    alias(AliasStatus::MustAlias, "alias:must"),
                    alias(AliasStatus::PartialAlias, "alias:partial"),
                    alias(AliasStatus::Unknown, "alias:unknown"),
                ],
            },
            ..TypeValueAliasOutput::default()
        });

        let debug = type_value_alias_debug_json_for_test(&db);
        let counts = &debug["counts"]["by_alias_status"];

        for status in [
            "NoAlias",
            "MayAlias",
            "MustAlias",
            "PartialAlias",
            "Unknown",
        ] {
            assert_eq!(counts[status], 1);
        }
    }

    #[test]
    fn populated_type_value_alias_debug_counts_all_families_and_reasons() {
        let mut db = LocalAnalysisDb::new();
        db.replace_type_value_alias_facts(TypeValueAliasOutput {
            types: TypeOutput {
                types: vec![TypeFact {
                    id: TypeFactId(0),
                    subject: TypeSubject::Synthetic("fixture".to_string()),
                    type_set: TypeSetId(0),
                    shape: TypeShape::Unknown {
                        reason: "dynamic".to_string(),
                    },
                    phase: TypePhase::Unknown,
                    language: Language::TypeScript,
                    file: None,
                    function: None,
                    body: None,
                    place: None,
                    cfg_block: None,
                    operation: None,
                    precision: TypePrecision::Unknown,
                    confidence: TypeConfidence::Low,
                    status: TypeStatus::Unknown,
                    provenance: TypeProvenance::Native,
                    stable_key: crate::internal_core::stable_key_for_test("type:fixture"),
                }],
                narrowed: Vec::new(),
            },
            values: ValueOutput {
                values: vec![ValueFact {
                    id: ValueFactId(0),
                    subject: ValueSubject::Synthetic("fixture".to_string()),
                    value: crate::analysis_neutral::ids::AbstractValueId(0),
                    kind: ValueKind::Unknown {
                        evidence: "unknown-value".to_string(),
                    },
                    language: Language::TypeScript,
                    file: None,
                    function: None,
                    body: None,
                    precision: ValuePrecision::Unknown,
                    status: ValueStatus::Unknown,
                    provenance: ValueProvenance::Generated,
                    stable_key: crate::internal_core::stable_key_for_test("value:fixture"),
                }],
                allocations: vec![AllocationTokenFact {
                    id: AllocationTokenId(0),
                    kind: AllocationKind::ObjectLiteral,
                    language: Language::TypeScript,
                    file: None,
                    function: None,
                    body: None,
                    source_place: None,
                    source_operation: None,
                    span: None,
                    provenance: ValueProvenance::Native,
                    stable_key: crate::internal_core::stable_key_for_test("allocation:fixture"),
                }],
            },
            access_paths: AccessPathOutput {
                access_paths: vec![AccessPathFact {
                    id: AccessPathId(0),
                    base: PlaceId(0),
                    projections: Vec::new(),
                    depth: 0,
                    language: Language::TypeScript,
                    file: None,
                    function: None,
                    body: None,
                    status: AccessPathStatus::Unknown,
                    stable_key: crate::internal_core::stable_key_for_test("path:fixture"),
                }],
            },
            points_to: PointsToOutput {
                constraints: Vec::new(),
                sets: vec![PointsToSetFact {
                    id: PointsToSetId(0),
                    variable: PtVarId(0),
                    objects: vec![ObjectTokenId(0)],
                    status: PointsToStatus::Present,
                    precision: PointsToPrecision::FlowInsensitive,
                    budget: PointsToBudgetStatus::WithinBudget,
                    stable_key: crate::internal_core::stable_key_for_test("points-to:fixture"),
                }],
            },
            aliases: AliasOutput {
                answers: vec![alias(AliasStatus::Unknown, "alias:fixture")],
            },
        });

        let debug = type_value_alias_debug_json_for_test(&db);
        let text = serde_json::to_string(&debug).expect("json");

        for family in [
            "Type",
            "Value",
            "AllocationToken",
            "AccessPath",
            "PointsToSet",
            "AliasAnswer",
        ] {
            assert_eq!(debug["counts"]["by_fact_family"][family], 1);
        }
        assert_eq!(
            debug["counts"]["by_unknown_or_unsupported_reason"]["dynamic"],
            1
        );
        assert_eq!(
            debug["counts"]["by_unknown_or_unsupported_reason"]["unknown-value"],
            1
        );
        assert!(!text.contains("/Users/"));
        assert!(!text.contains("timestamp"));
    }

    fn alias(status: AliasStatus, stable_key: &str) -> AliasAnswerFact {
        AliasAnswerFact {
            id: AliasAnswerId(0),
            left: AliasOperand::Place(PlaceId(1)),
            right: AliasOperand::Place(PlaceId(2)),
            status,
            reason: AliasReason::ExtensionProvided,
            evidence: vec![stable_key.to_string()],
            precision: AliasPrecision::Heuristic,
            stable_key: crate::internal_core::stable_key_for_test(stable_key),
        }
    }
}
