use std::collections::BTreeSet;

use crate::analysis_kernel::FactFamily;
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, Severity, TextRange, fingerprint};

pub(crate) fn validate_refined_calls(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    let call_sites = db
        .call_sites()
        .iter()
        .map(|site| site.id)
        .collect::<BTreeSet<_>>();
    let call_targets = db
        .call_targets()
        .iter()
        .map(|target| target.id)
        .collect::<BTreeSet<_>>();
    let functions = db
        .functions()
        .iter()
        .map(|function| function.id)
        .collect::<BTreeSet<_>>();
    let symbols = db
        .symbols()
        .iter()
        .map(|symbol| symbol.id)
        .collect::<BTreeSet<_>>();

    for edge in db.refined_call_edges() {
        if !call_sites.contains(&edge.site) {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                format!("dangling call site {:?}", edge.site),
            ));
        }
        if let Some(base_target) = edge.base_target
            && !call_targets.contains(&base_target)
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                format!("dangling base call target {base_target:?}"),
            ));
        }
        if !functions.contains(&edge.caller) {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                format!("dangling caller {:?}", edge.caller),
            ));
        }
        if let Some(target_function) = edge.target_function
            && !functions.contains(&target_function)
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                format!("dangling target function {target_function:?}"),
            ));
        }
        if let Some(target_symbol) = edge.target_symbol
            && !symbols.contains(&target_symbol)
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                format!("dangling target symbol {target_symbol:?}"),
            ));
        }
        if !edge
            .stable_key
            .contains(FactFamily::RefinedCallEdge.label())
        {
            diagnostics.push(invalid_refined_call_diagnostic(
                &edge.stable_key,
                "stable key does not include refined call fact family".to_string(),
            ));
        }
    }
}

fn invalid_refined_call_diagnostic(stable_key: &str, reason: String) -> Diagnostic {
    Diagnostic {
        rule_id: "polint/internal".to_string(),
        severity: Severity::Error,
        message: format!("invalid refined call fact `{stable_key}`: {reason}"),
        file: "<workspace>".to_string(),
        range: TextRange::point(1, 1),
        labels: Vec::new(),
        help: None,
        evidence: Vec::new(),
        suggestions: Vec::new(),
        fix: None,
        stable_fingerprint: fingerprint(&["polint.refined_calls.validate", stable_key, &reason]),
    }
}
