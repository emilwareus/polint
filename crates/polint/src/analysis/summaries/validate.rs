use std::collections::BTreeSet;

use crate::analysis_kernel::{FactFamily, FactPrecision, FactRef};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

use super::facts::{SummaryDomainKind, SummaryEventFact, SummaryFact};

pub(crate) fn validate_summaries(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
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
            .map(|row| row.stable_key.as_str()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "SummaryCall",
        db.summary_facts()
            .iter()
            .filter(|row| row.domain == SummaryDomainKind::CallEffects)
            .map(|row| row.stable_key.as_str()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "SummaryMemory",
        db.summary_facts()
            .iter()
            .filter(|row| row.domain == SummaryDomainKind::MemoryEffects)
            .map(|row| row.stable_key.as_str()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "SummaryTito",
        db.summary_facts()
            .iter()
            .filter(|row| row.domain == SummaryDomainKind::DataFlowTito)
            .map(|row| row.stable_key.as_str()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "SummaryEvent",
        db.summary_events()
            .iter()
            .map(|row| row.stable_key.as_str()),
    );

    for fact in db.summary_facts() {
        validate_summary_fact(db, diagnostics, &functions, fact);
    }

    for event in db.summary_events() {
        validate_summary_event(diagnostics, &functions, event);
    }

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

fn validate_summary_fact(
    db: &AnalysisDb,
    diagnostics: &mut Vec<Diagnostic>,
    functions: &BTreeSet<crate::core::FunctionId>,
    fact: &SummaryFact,
) {
    let family = summary_domain_family_label(fact.domain);

    if !functions.contains(&fact.function) {
        push_summary_diagnostic(
            diagnostics,
            family,
            &fact.stable_key,
            "function",
            "dangling function reference",
        );
    }

    if fact.callable_stable_key.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            family,
            &fact.stable_key,
            "callable_stable_key",
            "callable stable key must not be empty",
        );
    }

    if fact.payload_digest.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            family,
            &fact.stable_key,
            "payload_digest",
            "payload digest must not be empty",
        );
    }

    if fact.stable_key.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            family,
            &fact.stable_key,
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
        &fact.stable_key,
    );
}

fn validate_summary_event(
    diagnostics: &mut Vec<Diagnostic>,
    functions: &BTreeSet<crate::core::FunctionId>,
    event: &SummaryEventFact,
) {
    if !functions.contains(&event.function) {
        push_summary_diagnostic(
            diagnostics,
            "SummaryEvent",
            &event.stable_key,
            "function",
            "dangling function reference",
        );
    }

    if event.callable_stable_key.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            "SummaryEvent",
            &event.stable_key,
            "callable_stable_key",
            "callable stable key must not be empty",
        );
    }

    if event.event_kind.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            "SummaryEvent",
            &event.stable_key,
            "event_kind",
            "event kind must not be empty",
        );
    }

    if event.reason.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            "SummaryEvent",
            &event.stable_key,
            "reason",
            "event reason must not be empty",
        );
    }

    if event.stable_key.trim().is_empty() {
        push_summary_diagnostic(
            diagnostics,
            "SummaryEvent",
            &event.stable_key,
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

fn check_duplicate_stable_keys<'a>(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    keys: impl Iterator<Item = &'a str>,
) {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key) {
            push_summary_diagnostic(
                diagnostics,
                family,
                key,
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
    diagnostics.push(Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Internal analysis validation failed.",
    ));
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
    use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span};
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
            callable_stable_key: callable_key.to_string(),
            function: FunctionId(function),
            domain,
            status: SummaryStatus::Present,
            precision: SummaryPrecision::Local,
            provenance: SummaryProvenance::NativeLocal,
            payload_digest: format!("digest:{stable_key}"),
            stable_key: stable_key.to_string(),
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
            callable_stable_key: callable_key.to_string(),
            function: FunctionId(function),
            domain: SummaryDomainKind::CallEffects,
            event_kind: "unresolved_callee".to_string(),
            reason: "dynamic".to_string(),
            status: SummaryStatus::Unknown,
            precision: SummaryPrecision::UnknownTop,
            stable_key: stable_key.to_string(),
        }
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
}
