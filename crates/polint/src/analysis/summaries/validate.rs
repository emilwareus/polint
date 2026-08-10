use std::collections::{BTreeMap, BTreeSet};

use crate::analysis_kernel::{FactFamily, FactPrecision, FactRef};
use crate::core::{AnalysisDb, StableKeyInterner};
use crate::diagnostics::{Diagnostic, TextRange};

use super::facts::{
    SummaryDomainKind, SummaryEventFact, SummaryFact, SummaryProvenance, SummaryStatus,
};

pub(crate) fn validate_summaries(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    let interner = db.stable_key_interner();
    let functions = db
        .functions()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();

    check_duplicate_stable_keys(
        diagnostics,
        "SummaryControl",
        db.summary_facts()
            .iter()
            .filter(|row| row.domain == SummaryDomainKind::ControlEffects)
            .map(|row| interner.resolve(row.stable_key).to_string()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "SummaryCall",
        db.summary_facts()
            .iter()
            .filter(|row| row.domain == SummaryDomainKind::CallEffects)
            .map(|row| interner.resolve(row.stable_key).to_string()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "SummaryMemory",
        db.summary_facts()
            .iter()
            .filter(|row| row.domain == SummaryDomainKind::MemoryEffects)
            .map(|row| interner.resolve(row.stable_key).to_string()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "SummaryTito",
        db.summary_facts()
            .iter()
            .filter(|row| row.domain == SummaryDomainKind::DataFlowTito)
            .map(|row| interner.resolve(row.stable_key).to_string()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "SummaryEvent",
        db.summary_events()
            .iter()
            .map(|row| interner.resolve(row.stable_key).to_string()),
    );

    for fact in db.summary_facts() {
        validate_summary_fact(&interner, db, diagnostics, &functions, fact);
    }

    for event in db.summary_events() {
        validate_summary_event(&interner, diagnostics, &functions, event);
    }

    validate_scc_closure_results(&interner, db, diagnostics);

    for family in [
        FactFamily::SummaryControl,
        FactFamily::SummaryCall,
        FactFamily::SummaryMemory,
        FactFamily::SummaryTito,
        FactFamily::SummaryEvent,
    ] {
        for (reference, metadata) in db.fact_meta().rows() {
            if reference.family != family || metadata.producer_id != "polint.direct_summaries" {
                continue;
            }
            if metadata.precision == FactPrecision::Exact {
                push_summary_diagnostic(
                    diagnostics,
                    family.label(),
                    &metadata.stable_key,
                    "precision",
                    "precision ceiling exceeded: polint.direct_summaries metadata is SetupAware or weaker, not Exact",
                );
            }
        }
    }
}

fn validate_scc_closure_results(
    interner: &StableKeyInterner,
    db: &AnalysisDb,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let budget_events = db
        .summary_events()
        .iter()
        .filter(|event| {
            event.status == SummaryStatus::BudgetExceeded
                && (event.event_kind == "budget_exceeded"
                    || event.reason.contains("budget_exceeded")
                    || event.reason.contains("budget"))
        })
        .map(|event| event.function)
        .collect::<BTreeSet<_>>();

    let outgoing_counts = db
        .call_store()
        .map(|store| {
            db.functions()
                .iter()
                .map(|function| {
                    (
                        function.id,
                        store
                            .outgoing_by_function(function.id)
                            .into_iter()
                            .filter(|target| {
                                target.status
                                    == crate::analysis::calls::facts::CallTargetStatus::Resolved
                            })
                            .count(),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    for fact in db.summary_facts().iter().filter(|fact| {
        fact.provenance == SummaryProvenance::InterproceduralClosure
            || fact.status == SummaryStatus::BudgetExceeded
    }) {
        let family = summary_domain_family_label(fact.domain);
        let stable_key = interner.resolve(fact.stable_key);
        if fact.status == SummaryStatus::BudgetExceeded && !budget_events.contains(&fact.function) {
            push_summary_diagnostic(
                diagnostics,
                family,
                stable_key.as_ref(),
                "budget_exceeded",
                "BudgetExceeded summaries require matching budget-exceeded summary event evidence",
            );
        }

        if fact.provenance == SummaryProvenance::InterproceduralClosure {
            let metadata = db.metadata_for(FactRef::new(
                summary_domain_to_fact_family(fact.domain),
                fact.id.0,
            ));
            if metadata.is_some_and(|metadata| metadata.precision == FactPrecision::Exact) {
                push_summary_diagnostic(
                    diagnostics,
                    family,
                    stable_key.as_ref(),
                    "precision",
                    "interprocedural SCC summaries must not claim Exact metadata precision",
                );
            }

            if outgoing_counts.get(&fact.function).copied().unwrap_or(0) == 0 {
                push_summary_diagnostic(
                    diagnostics,
                    family,
                    stable_key.as_ref(),
                    "provenance",
                    "interprocedural SCC summary has no resolved outgoing call evidence",
                );
            }
        }
    }
}

fn validate_summary_fact(
    interner: &StableKeyInterner,
    db: &AnalysisDb,
    diagnostics: &mut Vec<Diagnostic>,
    functions: &BTreeSet<crate::core::FunctionId>,
    fact: &SummaryFact,
) {
    let family = summary_domain_family_label(fact.domain);
    let stable_key = interner.resolve(fact.stable_key);

    if !functions.contains(&fact.function) {
        push_summary_diagnostic(
            diagnostics,
            family,
            stable_key.as_ref(),
            "function",
            "dangling function reference",
        );
    }

    if interner.resolve(fact.callable_stable_key).trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            family,
            stable_key.as_ref(),
            "callable_stable_key",
            "callable stable key must not be empty",
        );
    }

    if fact.payload_digest.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            family,
            stable_key.as_ref(),
            "payload_digest",
            "payload digest must not be empty",
        );
    }

    if stable_key.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            family,
            stable_key.as_ref(),
            "stable_key",
            "stable key must not be empty",
        );
    }

    let expected_family = summary_domain_to_fact_family(fact.domain);
    check_metadata(
        db,
        diagnostics,
        expected_family,
        fact.id.0,
        family,
        stable_key.as_ref(),
    );
}

fn validate_summary_event(
    interner: &StableKeyInterner,
    diagnostics: &mut Vec<Diagnostic>,
    functions: &BTreeSet<crate::core::FunctionId>,
    event: &SummaryEventFact,
) {
    let stable_key = interner.resolve(event.stable_key);

    if !functions.contains(&event.function) {
        push_summary_diagnostic(
            diagnostics,
            "SummaryEvent",
            stable_key.as_ref(),
            "function",
            "dangling function reference",
        );
    }

    if interner
        .resolve(event.callable_stable_key)
        .trim()
        .is_empty()
    {
        push_summary_diagnostic(
            diagnostics,
            "SummaryEvent",
            stable_key.as_ref(),
            "callable_stable_key",
            "callable stable key must not be empty",
        );
    }

    if event.event_kind.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            "SummaryEvent",
            stable_key.as_ref(),
            "event_kind",
            "event kind must not be empty",
        );
    }

    if event.reason.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            "SummaryEvent",
            stable_key.as_ref(),
            "reason",
            "event reason must not be empty",
        );
    }

    if stable_key.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            "SummaryEvent",
            stable_key.as_ref(),
            "stable_key",
            "stable key must not be empty",
        );
    }
}

fn check_metadata(
    db: &AnalysisDb,
    diagnostics: &mut Vec<Diagnostic>,
    family: FactFamily,
    run_id: u64,
    family_label: &'static str,
    stable_key: &str,
) {
    let Some(metadata) = db.metadata_for(FactRef::new(family, run_id)) else {
        push_summary_diagnostic(
            diagnostics,
            family_label,
            stable_key,
            "metadata",
            "required metadata is missing",
        );
        return;
    };
    if metadata.producer_id != "polint.direct_summaries"
        || metadata.layer_id != "polint.direct_summaries"
    {
        push_summary_diagnostic(
            diagnostics,
            family_label,
            stable_key,
            "metadata",
            "summary rows require polint.direct_summaries producer and layer metadata",
        );
    }
    if metadata.stable_key != stable_key {
        push_summary_diagnostic(
            diagnostics,
            family_label,
            stable_key,
            "metadata",
            "metadata stable key does not match row stable key",
        );
    }
}

fn check_duplicate_stable_keys(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    keys: impl Iterator<Item = String>,
) {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key.clone()) {
            push_summary_diagnostic(
                diagnostics,
                family,
                &key,
                "stable_key",
                "duplicate stable key",
            );
        }
    }
}

fn summary_domain_family_label(domain: SummaryDomainKind) -> &'static str {
    match domain {
        SummaryDomainKind::ControlEffects => "SummaryControl",
        SummaryDomainKind::CallEffects => "SummaryCall",
        SummaryDomainKind::MemoryEffects => "SummaryMemory",
        SummaryDomainKind::DataFlowTito => "SummaryTito",
    }
}

fn summary_domain_to_fact_family(domain: SummaryDomainKind) -> FactFamily {
    match domain {
        SummaryDomainKind::ControlEffects => FactFamily::SummaryControl,
        SummaryDomainKind::CallEffects => FactFamily::SummaryCall,
        SummaryDomainKind::MemoryEffects => FactFamily::SummaryMemory,
        SummaryDomainKind::DataFlowTito => FactFamily::SummaryTito,
    }
}

fn push_summary_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    tracing::debug!(
        family,
        stable_key,
        field,
        reason,
        "summary validation failed"
    );
    diagnostics.push(
        Diagnostic::error(
            "polint/internal",
            "<workspace>",
            TextRange::point(1, 1),
            "Internal analysis validation failed.",
        )
        .with_evidence("family", family)
        .with_evidence("stable_key", stable_key)
        .with_evidence("field", field)
        .with_evidence("reason", reason),
    );
}

#[cfg(test)]
mod tests {
    use super::validate_summaries;
    use crate::analysis::ids::{SummaryEventId, SummaryId};
    use crate::analysis::summaries::facts::{
        SummaryDomainKind, SummaryEventFact, SummaryFact, SummaryPrecision, SummaryProvenance,
        SummaryStatus,
    };
    use crate::analysis::summaries::store::SummaryOutput;
    use crate::analysis_kernel::{
        FactConfidence, FactFamily, FactMeta, FactPrecision, FactRef, ValidationStatus,
    };
    use crate::core::{
        AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span, stable_key_for_test,
    };
    use std::path::PathBuf;

    fn base_db() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.ts"),
            "src/main.ts".to_string(),
            "export function main() {}\n".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "main".to_string(),
            span: span(file),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db
    }

    fn span(file: FileId) -> Span {
        Span {
            file,
            start_byte: 0,
            end_byte: 10,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 11,
        }
    }

    fn summary_fact(
        id: u64,
        callable_key: &str,
        function: u64,
        domain: SummaryDomainKind,
        stable_key: &str,
    ) -> SummaryFact {
        SummaryFact {
            id: SummaryId(id),
            callable_stable_key: stable_key_for_test(callable_key),
            function: FunctionId(function),
            domain,
            status: SummaryStatus::Present,
            precision: SummaryPrecision::Local,
            provenance: SummaryProvenance::NativeLocal,
            payload_digest: format!("digest:{stable_key}"),
            tito_flows: Vec::new(),
            stable_key: stable_key_for_test(stable_key),
        }
    }

    fn interprocedural_summary_fact(
        id: u64,
        callable_key: &str,
        function: u64,
        domain: SummaryDomainKind,
        stable_key: &str,
        status: SummaryStatus,
    ) -> SummaryFact {
        SummaryFact {
            provenance: SummaryProvenance::InterproceduralClosure,
            precision: if status == SummaryStatus::BudgetExceeded {
                SummaryPrecision::UnknownTop
            } else {
                SummaryPrecision::SetupAware
            },
            status,
            ..summary_fact(id, callable_key, function, domain, stable_key)
        }
    }

    fn event_fact(
        id: u64,
        callable_key: &str,
        function: u64,
        stable_key: &str,
    ) -> SummaryEventFact {
        SummaryEventFact {
            id: SummaryEventId(id),
            callable_stable_key: stable_key_for_test(callable_key),
            function: FunctionId(function),
            domain: SummaryDomainKind::CallEffects,
            event_kind: "unresolved_callee".to_string(),
            reason: "dynamic".to_string(),
            status: SummaryStatus::Unknown,
            precision: SummaryPrecision::UnknownTop,
            stable_key: stable_key_for_test(stable_key),
        }
    }

    fn diagnostic_reasons(diagnostics: &[crate::diagnostics::Diagnostic]) -> Vec<&str> {
        diagnostics
            .iter()
            .flat_map(|diagnostic| diagnostic.evidence.iter())
            .filter(|evidence| evidence.label == "reason")
            .map(|evidence| evidence.value.as_str())
            .collect()
    }

    #[test]
    fn rejects_dangling_function_reference() {
        let mut db = base_db();
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![summary_fact(
                0,
                "func::missing",
                99,
                SummaryDomainKind::ControlEffects,
                "summary:dangling",
            )],
            events: vec![event_fact(0, "func::missing", 99, "event:dangling")],
        });

        let mut diagnostics = Vec::new();
        validate_summaries(&db, &mut diagnostics);

        assert!(
            diagnostics.len() >= 2,
            "expected dangling function diagnostics for fact and event: {diagnostics:#?}"
        );
        assert!(diagnostics.iter().all(|d| d.rule_id == "polint/internal"));
    }

    #[test]
    fn rejects_duplicate_stable_keys() {
        let mut db = base_db();
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![
                summary_fact(
                    0,
                    "func::main",
                    0,
                    SummaryDomainKind::ControlEffects,
                    "summary:dup",
                ),
                summary_fact(
                    1,
                    "func::main",
                    0,
                    SummaryDomainKind::ControlEffects,
                    "summary:dup",
                ),
            ],
            events: Vec::new(),
        });

        let mut diagnostics = Vec::new();
        validate_summaries(&db, &mut diagnostics);

        assert!(
            diagnostics.iter().any(|d| d.rule_id == "polint/internal"),
            "expected duplicate stable key diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn rejects_exact_precision_on_summary_metadata() {
        let mut db = base_db();
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![summary_fact(
                0,
                "func::main",
                0,
                SummaryDomainKind::ControlEffects,
                "summary:exact",
            )],
            events: Vec::new(),
        });
        // Override metadata precision to Exact
        db.fact_meta_mut_for_test()
            .remove_for_test(FactRef::new(FactFamily::SummaryControl, 0));
        db.fact_meta_mut_for_test().insert(
            FactRef::new(FactFamily::SummaryControl, 0),
            FactMeta {
                stable_key: "summary:exact".to_string(),
                producer_id: "polint.direct_summaries",
                layer_id: "polint.direct_summaries",
                precision: FactPrecision::Exact,
                confidence: FactConfidence::High,
                validation: ValidationStatus::NativeTrusted,
                payload_digest: "digest:summary:exact".to_string(),
            },
        );

        let mut diagnostics = Vec::new();
        validate_summaries(&db, &mut diagnostics);

        assert!(
            diagnostics.iter().any(|d| d.rule_id == "polint/internal"),
            "expected precision ceiling diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn accepts_valid_summary_facts() {
        let mut db = base_db();
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![
                summary_fact(
                    0,
                    "func::main",
                    0,
                    SummaryDomainKind::ControlEffects,
                    "summary:control:main",
                ),
                summary_fact(
                    1,
                    "func::main",
                    0,
                    SummaryDomainKind::CallEffects,
                    "summary:call:main",
                ),
                summary_fact(
                    2,
                    "func::main",
                    0,
                    SummaryDomainKind::MemoryEffects,
                    "summary:memory:main",
                ),
                summary_fact(
                    3,
                    "func::main",
                    0,
                    SummaryDomainKind::DataFlowTito,
                    "summary:tito:main",
                ),
            ],
            events: vec![event_fact(0, "func::main", 0, "event:main:0")],
        });

        let mut diagnostics = Vec::new();
        validate_summaries(&db, &mut diagnostics);

        assert!(
            diagnostics.is_empty(),
            "expected no diagnostics for valid summary facts: {diagnostics:#?}"
        );
    }

    #[test]
    fn validate_summaries_rejects_budget_exceeded_summary_without_matching_event() {
        let mut db = base_db();
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![interprocedural_summary_fact(
                0,
                "func::main",
                0,
                SummaryDomainKind::ControlEffects,
                "summary:budget-missing-event",
                SummaryStatus::BudgetExceeded,
            )],
            events: Vec::new(),
        });

        let mut diagnostics = Vec::new();
        validate_summaries(&db, &mut diagnostics);

        assert!(
            diagnostic_reasons(&diagnostics).contains(
                &"BudgetExceeded summaries require matching budget-exceeded summary event evidence",
            ),
            "expected BudgetExceeded evidence diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn validate_summaries_rejects_exact_precision_on_scc_produced_summary_metadata() {
        let mut db = base_db();
        db.replace_summary_facts(SummaryOutput {
            summaries: vec![interprocedural_summary_fact(
                0,
                "func::main",
                0,
                SummaryDomainKind::ControlEffects,
                "summary:scc-exact",
                SummaryStatus::Present,
            )],
            events: Vec::new(),
        });
        db.fact_meta_mut_for_test()
            .remove_for_test(FactRef::new(FactFamily::SummaryControl, 0));
        db.fact_meta_mut_for_test().insert(
            FactRef::new(FactFamily::SummaryControl, 0),
            FactMeta {
                stable_key: "summary:scc-exact".to_string(),
                producer_id: "polint.direct_summaries",
                layer_id: "polint.direct_summaries",
                precision: FactPrecision::Exact,
                confidence: FactConfidence::High,
                validation: ValidationStatus::NativeTrusted,
                payload_digest: "digest:summary:scc-exact".to_string(),
            },
        );

        let mut diagnostics = Vec::new();
        validate_summaries(&db, &mut diagnostics);

        assert!(
            diagnostic_reasons(&diagnostics)
                .contains(&"interprocedural SCC summaries must not claim Exact metadata precision"),
            "expected SCC precision diagnostic: {diagnostics:#?}"
        );
    }
}
