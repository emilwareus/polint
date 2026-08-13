use std::collections::{BTreeMap, BTreeSet};

use crate::analysis_api::{FactFamily, FactPrecision};
use crate::analysis_neutral::AnalysisHost;
use crate::analysis_neutral::entrypoints::provider::ENTRYPOINTS_PROVIDER_ID;
use crate::internal_core::{Diagnostic, DiagnosticRange};

pub fn validate_entrypoints(db: &impl AnalysisHost, diagnostics: &mut Vec<Diagnostic>) {
    let files = db.files().iter().map(|row| row.id).collect::<BTreeSet<_>>();
    let functions = db
        .functions()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();
    let symbols = db
        .symbols()
        .iter()
        .map(|row| row.id)
        .collect::<BTreeSet<_>>();

    // 1. Check duplicate stable keys for all four fact families
    check_duplicate_stable_keys(
        diagnostics,
        "Entrypoint",
        db.entrypoint_facts()
            .iter()
            .map(|row| db.resolve_stable_key(row.stable_key)),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "TrustBoundary",
        db.trust_boundary_facts()
            .iter()
            .map(|row| db.resolve_stable_key(row.stable_key)),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "DispatchEdge",
        db.dispatch_edge_facts()
            .iter()
            .map(|row| db.resolve_stable_key(row.stable_key)),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "UnresolvedFramework",
        db.unresolved_framework_facts()
            .iter()
            .map(|row| db.resolve_stable_key(row.stable_key)),
    );

    // Collect entrypoint stable keys for referential integrity checks
    let entrypoint_keys: BTreeSet<crate::internal_core::StableKeyId> = db
        .entrypoint_facts()
        .iter()
        .map(|ep| ep.stable_key)
        .collect();

    // Collect entrypoints indexed by target_function for conflicting registration check
    let mut entrypoints_by_target: BTreeMap<
        crate::internal_core::FunctionId,
        Vec<std::sync::Arc<str>>,
    > = BTreeMap::new();

    // 2. Validate each EntrypointFact
    for entrypoint in db.entrypoint_facts() {
        check_ref(
            diagnostics,
            &functions,
            entrypoint.target_function,
            "Entrypoint",
            db.resolve_stable_key(entrypoint.stable_key).as_ref(),
            "target_function",
            "dangling entrypoint target function reference",
        );
        if let Some(target_symbol) = entrypoint.target_symbol {
            check_ref(
                diagnostics,
                &symbols,
                target_symbol,
                "Entrypoint",
                db.resolve_stable_key(entrypoint.stable_key).as_ref(),
                "target_symbol",
                "dangling entrypoint target symbol reference",
            );
        }
        check_ref(
            diagnostics,
            &files,
            entrypoint.registration_file,
            "Entrypoint",
            db.resolve_stable_key(entrypoint.stable_key).as_ref(),
            "registration_file",
            "dangling entrypoint registration file reference",
        );
        if entrypoint.registration_span.start_byte > entrypoint.registration_span.end_byte {
            push_entrypoint_diagnostic(
                diagnostics,
                "Entrypoint",
                db.resolve_stable_key(entrypoint.stable_key).as_ref(),
                "registration_span",
                "invalid span byte range",
            );
        }

        // Track entrypoints by target_function for conflicting registration detection
        entrypoints_by_target
            .entry(entrypoint.target_function)
            .or_default()
            .push(db.resolve_stable_key(entrypoint.stable_key));
    }

    // 3. Validate each TrustBoundaryFact
    for boundary in db.trust_boundary_facts() {
        check_ref(
            diagnostics,
            &files,
            boundary.file,
            "TrustBoundary",
            db.resolve_stable_key(boundary.stable_key).as_ref(),
            "file",
            "dangling trust boundary file reference",
        );
        if boundary.span.start_byte > boundary.span.end_byte {
            push_entrypoint_diagnostic(
                diagnostics,
                "TrustBoundary",
                db.resolve_stable_key(boundary.stable_key).as_ref(),
                "span",
                "invalid span byte range",
            );
        }
        if !entrypoint_keys.contains(&boundary.entrypoint_stable_key) {
            push_entrypoint_diagnostic(
                diagnostics,
                "TrustBoundary",
                db.resolve_stable_key(boundary.stable_key).as_ref(),
                "entrypoint_stable_key",
                "dangling trust boundary entrypoint reference",
            );
        }
    }

    // 4. Validate each FrameworkDispatchEdgeFact
    for edge in db.dispatch_edge_facts() {
        check_ref(
            diagnostics,
            &functions,
            edge.to_target,
            "DispatchEdge",
            db.resolve_stable_key(edge.stable_key).as_ref(),
            "to_target",
            "dangling dispatch edge target function reference",
        );
        if let Some(to_symbol) = edge.to_symbol {
            check_ref(
                diagnostics,
                &symbols,
                to_symbol,
                "DispatchEdge",
                db.resolve_stable_key(edge.stable_key).as_ref(),
                "to_symbol",
                "dangling dispatch edge target symbol reference",
            );
        }
        check_ref(
            diagnostics,
            &files,
            edge.file,
            "DispatchEdge",
            db.resolve_stable_key(edge.stable_key).as_ref(),
            "file",
            "dangling dispatch edge file reference",
        );
        if edge.span.start_byte > edge.span.end_byte {
            push_entrypoint_diagnostic(
                diagnostics,
                "DispatchEdge",
                db.resolve_stable_key(edge.stable_key).as_ref(),
                "span",
                "invalid span byte range",
            );
        }
        if edge.from_source.is_empty() {
            push_entrypoint_diagnostic(
                diagnostics,
                "DispatchEdge",
                db.resolve_stable_key(edge.stable_key).as_ref(),
                "from_source",
                "empty dispatch edge source",
            );
        }
    }

    // 5. Validate each UnresolvedFrameworkFact
    for fact in db.unresolved_framework_facts() {
        check_ref(
            diagnostics,
            &files,
            fact.file,
            "UnresolvedFramework",
            db.resolve_stable_key(fact.stable_key).as_ref(),
            "file",
            "dangling unresolved framework file reference",
        );
        if fact.span.start_byte > fact.span.end_byte {
            push_entrypoint_diagnostic(
                diagnostics,
                "UnresolvedFramework",
                db.resolve_stable_key(fact.stable_key).as_ref(),
                "span",
                "invalid span byte range",
            );
        }
    }

    // 6. Precision ceiling check: framework facts must not claim Exact
    for family in [
        FactFamily::Entrypoint,
        FactFamily::TrustBoundary,
        FactFamily::DispatchEdge,
        FactFamily::UnresolvedFramework,
    ] {
        for (reference, metadata) in db.fact_meta().rows() {
            if reference.family != family || metadata.producer_id != ENTRYPOINTS_PROVIDER_ID {
                continue;
            }
            if metadata.precision == FactPrecision::Exact {
                push_entrypoint_diagnostic(
                    diagnostics,
                    family.label(),
                    db.resolve_stable_key(metadata.stable_key).as_ref(),
                    "precision",
                    "precision ceiling exceeded: framework facts must not claim Exact",
                );
            }
        }
    }

    // 7. Conflicting entrypoint registrations: multiple entrypoints for the same target_function
    // with different framework_ids
    let entrypoints = db.entrypoint_facts();
    for keys in entrypoints_by_target.values() {
        if keys.len() > 1 {
            // Check if they have different framework_ids
            let framework_ids: BTreeSet<&str> = keys
                .iter()
                .filter_map(|key| {
                    entrypoints
                        .iter()
                        .find(|ep| db.resolve_stable_key(ep.stable_key).as_ref() == key.as_ref())
                        .map(|ep| ep.framework_id.as_str())
                })
                .collect();
            if framework_ids.len() > 1 {
                for key in keys {
                    push_entrypoint_diagnostic(
                        diagnostics,
                        "Entrypoint",
                        key.as_ref(),
                        "target_function",
                        "conflicting entrypoint registrations for same target function with different frameworks",
                    );
                }
            }
        }
    }
}

fn check_duplicate_stable_keys(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    keys: impl Iterator<Item = std::sync::Arc<str>>,
) {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key.clone()) {
            push_entrypoint_diagnostic(
                diagnostics,
                family,
                key.as_ref(),
                "stable_key",
                "duplicate stable key",
            );
        }
    }
}

fn check_ref<T: Ord + Copy>(
    diagnostics: &mut Vec<Diagnostic>,
    valid: &BTreeSet<T>,
    value: T,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    if !valid.contains(&value) {
        push_entrypoint_diagnostic(diagnostics, family, stable_key, field, reason);
    }
}

fn push_entrypoint_diagnostic(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    diagnostics.push(
        Diagnostic::error(
            "polint/internal",
            "<workspace>",
            DiagnosticRange::point(1, 1),
            format!("Entrypoints validation failed for {family} stable key."),
        )
        .with_evidence("family", family)
        .with_evidence("stable_key", stable_key.to_string())
        .with_evidence("field", field)
        .with_evidence("reason", reason),
    );
}
