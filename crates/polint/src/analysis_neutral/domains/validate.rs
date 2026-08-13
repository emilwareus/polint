use crate::analysis_neutral::AnalysisHost;
use std::collections::BTreeSet;

use super::facts::{
    DomainEventFact, DomainObservationFact, DomainPrecision, DomainStatus, DomainValue,
};
use crate::analysis_api::{FactFamily, FactPrecision, FactRef};
use crate::analysis_neutral::ids::MirOpId;
use crate::internal_core::{Diagnostic, DiagnosticRange as TextRange};

pub fn validate_abstract_domains(db: &impl AnalysisHost, diagnostics: &mut Vec<Diagnostic>) {
    let bodies = db.mir_bodies().iter().map(|row| row.id).collect();
    let blocks = db.cfg_blocks().iter().map(|row| row.id).collect();
    let operations = db.mir_operations().iter().map(|row| row.id).collect();
    let places = db.mir_places().iter().map(|row| row.id).collect();
    let call_operations = db
        .call_sites()
        .iter()
        .map(|site| site.operation)
        .collect::<BTreeSet<_>>();

    check_duplicate_stable_keys(
        diagnostics,
        "DomainObservation",
        db.abstract_domain_observations()
            .iter()
            .map(|row| db.resolve_stable_key(row.stable_key)),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "DomainEvent",
        db.abstract_domain_events()
            .iter()
            .map(|row| db.resolve_stable_key(row.stable_key)),
    );

    for row in db.abstract_domain_observations() {
        validate_observation(db, diagnostics, &bodies, &blocks, &operations, &places, row);
    }

    for row in db.abstract_domain_events() {
        validate_event(
            db,
            diagnostics,
            &bodies,
            &blocks,
            &operations,
            &call_operations,
            row,
        );
    }

    for family in [FactFamily::DomainObservation, FactFamily::DomainEvent] {
        for (reference, metadata) in db.fact_meta().rows() {
            if reference.family != family || metadata.producer_id != "polint.abstract_domains" {
                continue;
            }
            if metadata.precision == FactPrecision::Exact {
                push_domain_diagnostic(
                    diagnostics,
                    family.label(),
                    db.resolve_stable_key(metadata.stable_key).as_ref(),
                    "precision",
                    "precision ceiling exceeded: polint.abstract_domains metadata is SetupAware or weaker, not Exact",
                );
            }
        }
    }
}

fn validate_observation(
    db: &impl AnalysisHost,
    diagnostics: &mut Vec<Diagnostic>,
    bodies: &BTreeSet<crate::analysis_neutral::ids::MirBodyId>,
    blocks: &BTreeSet<crate::analysis_neutral::cfg::ids::BasicBlockId>,
    operations: &BTreeSet<MirOpId>,
    places: &BTreeSet<crate::analysis_neutral::ids::PlaceId>,
    row: &DomainObservationFact,
) {
    let stable_key = db.resolve_stable_key(row.stable_key);
    if stable_key.trim().is_empty() {
        push_domain_diagnostic(
            diagnostics,
            "DomainObservation",
            stable_key.as_ref(),
            "stable_key",
            "stable key is required",
        );
    }
    check_metadata(
        db,
        diagnostics,
        FactFamily::DomainObservation,
        row.id.0,
        "DomainObservation",
        stable_key.as_ref(),
    );
    check_ref(
        diagnostics,
        bodies,
        row.body,
        "DomainObservation",
        stable_key.as_ref(),
        "body",
        "dangling MIR body reference",
    );
    check_optional_ref(
        diagnostics,
        blocks,
        row.block,
        "DomainObservation",
        stable_key.as_ref(),
        "block",
        "dangling basic block reference",
    );
    check_optional_ref(
        diagnostics,
        operations,
        row.operation,
        "DomainObservation",
        stable_key.as_ref(),
        "operation",
        "dangling MIR operation reference",
    );
    check_optional_ref(
        diagnostics,
        places,
        row.place,
        "DomainObservation",
        stable_key.as_ref(),
        "place",
        "dangling place reference",
    );

    match &row.value {
        DomainValue::TopReason(reason) => validate_reasoned_status(
            diagnostics,
            "DomainObservation",
            stable_key.as_ref(),
            row.status,
            row.precision,
            reason,
        ),
        DomainValue::Label(value) if value.trim().is_empty() => push_domain_diagnostic(
            diagnostics,
            "DomainObservation",
            stable_key.as_ref(),
            "value",
            "label values must not be empty",
        ),
        DomainValue::DigestParts(parts) if parts.is_empty() => push_domain_diagnostic(
            diagnostics,
            "DomainObservation",
            stable_key.as_ref(),
            "value",
            "digest-part values must not be empty",
        ),
        DomainValue::Label(_) | DomainValue::DigestParts(_) => {
            if reason_required(row.status) {
                push_domain_diagnostic(
                    diagnostics,
                    "DomainObservation",
                    stable_key.as_ref(),
                    "value",
                    "top, unknown, unsupported, setup-missing, and budget rows require a TopReason",
                );
            }
            if row.status == DomainStatus::Present
                && matches!(
                    row.precision,
                    DomainPrecision::Unknown | DomainPrecision::Unsupported
                )
            {
                push_domain_diagnostic(
                    diagnostics,
                    "DomainObservation",
                    stable_key.as_ref(),
                    "precision",
                    "present rows require ExactLocal, SetupAware, Conservative, or Heuristic precision",
                );
            }
        }
    }
}

fn validate_event(
    db: &impl AnalysisHost,
    diagnostics: &mut Vec<Diagnostic>,
    bodies: &BTreeSet<crate::analysis_neutral::ids::MirBodyId>,
    blocks: &BTreeSet<crate::analysis_neutral::cfg::ids::BasicBlockId>,
    operations: &BTreeSet<MirOpId>,
    call_operations: &BTreeSet<MirOpId>,
    row: &DomainEventFact,
) {
    let stable_key = db.resolve_stable_key(row.stable_key);
    if stable_key.trim().is_empty() {
        push_domain_diagnostic(
            diagnostics,
            "DomainEvent",
            stable_key.as_ref(),
            "stable_key",
            "stable key is required",
        );
    }
    check_metadata(
        db,
        diagnostics,
        FactFamily::DomainEvent,
        row.id.0,
        "DomainEvent",
        stable_key.as_ref(),
    );
    check_ref(
        diagnostics,
        bodies,
        row.body,
        "DomainEvent",
        stable_key.as_ref(),
        "body",
        "dangling MIR body reference",
    );
    check_optional_ref(
        diagnostics,
        blocks,
        row.block,
        "DomainEvent",
        stable_key.as_ref(),
        "block",
        "dangling basic block reference",
    );
    check_optional_ref(
        diagnostics,
        operations,
        row.operation,
        "DomainEvent",
        stable_key.as_ref(),
        "operation",
        "dangling MIR operation reference",
    );
    if row.status == DomainStatus::Present {
        push_domain_diagnostic(
            diagnostics,
            "DomainEvent",
            stable_key.as_ref(),
            "status",
            "domain events must describe top, unknown, setup, unsupported, or budget loss",
        );
    }
    validate_reasoned_status(
        diagnostics,
        "DomainEvent",
        stable_key.as_ref(),
        row.status,
        row.precision,
        row.reason.as_str(),
    );
    validate_unresolved_call_reference(
        diagnostics,
        "DomainEvent",
        stable_key.as_ref(),
        row.operation,
        call_operations,
        Some(row.reason.as_str()),
    );
}

fn check_metadata(
    db: &impl AnalysisHost,
    diagnostics: &mut Vec<Diagnostic>,
    family: FactFamily,
    run_id: u64,
    family_label: &'static str,
    stable_key: &str,
) {
    let Some(metadata) = db.metadata_for(FactRef::new(family, run_id)) else {
        push_domain_diagnostic(
            diagnostics,
            family_label,
            stable_key,
            "metadata",
            "required metadata is missing",
        );
        return;
    };
    if metadata.producer_id != "polint.abstract_domains"
        || metadata.layer_id != "polint.abstract_domains"
    {
        push_domain_diagnostic(
            diagnostics,
            family_label,
            stable_key,
            "metadata",
            "domain rows require polint.abstract_domains producer and layer metadata",
        );
    }
    if db.resolve_stable_key(metadata.stable_key).as_ref() != stable_key {
        push_domain_diagnostic(
            diagnostics,
            family_label,
            stable_key,
            "metadata",
            "metadata stable key does not match row stable key",
        );
    }
}

fn validate_reasoned_status(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    stable_key: &str,
    status: DomainStatus,
    precision: DomainPrecision,
    reason: &str,
) {
    let Some((expected_status, expected_precisions)) = reason_contract(reason) else {
        push_domain_diagnostic(
            diagnostics,
            family,
            stable_key,
            "reason",
            "unknown top reason",
        );
        return;
    };
    if status != expected_status {
        push_domain_diagnostic(
            diagnostics,
            family,
            stable_key,
            "status",
            "status is inconsistent with top reason",
        );
    }
    if !expected_precisions.contains(&precision) {
        push_domain_diagnostic(
            diagnostics,
            family,
            stable_key,
            "precision",
            "precision is inconsistent with top reason",
        );
    }
}

fn validate_unresolved_call_reference(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    stable_key: &str,
    operation: Option<MirOpId>,
    call_operations: &BTreeSet<MirOpId>,
    reason: Option<&str>,
) {
    if reason != Some("unresolved_call") {
        return;
    }
    let Some(operation) = operation else {
        push_domain_diagnostic(
            diagnostics,
            family,
            stable_key,
            "operation",
            "unresolved-call rows require a MIR operation reference",
        );
        return;
    };
    if !call_operations.contains(&operation) {
        push_domain_diagnostic(
            diagnostics,
            family,
            stable_key,
            "operation",
            "unresolved-call rows require a matching call-site operation",
        );
    }
}

fn reason_required(status: DomainStatus) -> bool {
    matches!(
        status,
        DomainStatus::Top
            | DomainStatus::Unknown
            | DomainStatus::Unsupported
            | DomainStatus::SetupMissing
            | DomainStatus::BudgetExceeded
    )
}

fn reason_contract(reason: &str) -> Option<(DomainStatus, &'static [DomainPrecision])> {
    match reason {
        "unknown_value" | "dynamic_write" | "unresolved_call" => {
            Some((DomainStatus::Unknown, &[DomainPrecision::Unknown]))
        }
        "unsupported_semantic" => {
            Some((DomainStatus::Unsupported, &[DomainPrecision::Unsupported]))
        }
        "setup_missing" => Some((DomainStatus::SetupMissing, &[DomainPrecision::SetupAware])),
        "budget_exceeded" => Some((
            DomainStatus::BudgetExceeded,
            &[DomainPrecision::Unknown, DomainPrecision::Conservative],
        )),
        "widened" | "conflicting_facts" => {
            Some((DomainStatus::Top, &[DomainPrecision::Conservative]))
        }
        _ => None,
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
            push_domain_diagnostic(
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
        push_domain_diagnostic(diagnostics, family, stable_key, field, reason);
    }
}

fn check_optional_ref<T: Ord + Copy>(
    diagnostics: &mut Vec<Diagnostic>,
    valid: &BTreeSet<T>,
    value: Option<T>,
    family: &'static str,
    stable_key: &str,
    field: &'static str,
    reason: &'static str,
) {
    if let Some(value) = value {
        check_ref(diagnostics, valid, value, family, stable_key, field, reason);
    }
}

fn push_domain_diagnostic(
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
        "abstract domain validation failed"
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
