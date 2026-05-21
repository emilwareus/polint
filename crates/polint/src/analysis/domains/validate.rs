use std::collections::BTreeSet;

use super::facts::{
    DomainEventFact, DomainObservationFact, DomainPrecision, DomainStatus, DomainValue,
};
use crate::analysis::ids::MirOpId;
use crate::analysis_kernel::{FactFamily, FactPrecision, FactRef};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

pub(crate) fn validate_abstract_domains(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
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
            .map(|row| row.stable_key.as_str()),
    );
    check_duplicate_stable_keys(
        diagnostics,
        "DomainEvent",
        db.abstract_domain_events()
            .iter()
            .map(|row| row.stable_key.as_str()),
    );

    for row in db.abstract_domain_observations() {
        validate_observation(
            db,
            diagnostics,
            &bodies,
            &blocks,
            &operations,
            &places,
            &call_operations,
            row,
        );
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
                    &metadata.stable_key,
                    "precision",
                    "precision ceiling exceeded: polint.abstract_domains metadata is SetupAware or weaker, not Exact",
                );
            }
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "Validation checks a normalized domain row against several existing fact indexes."
)]
fn validate_observation(
    db: &AnalysisDb,
    diagnostics: &mut Vec<Diagnostic>,
    bodies: &BTreeSet<crate::analysis::ids::MirBodyId>,
    blocks: &BTreeSet<crate::analysis::cfg::ids::BasicBlockId>,
    operations: &BTreeSet<MirOpId>,
    places: &BTreeSet<crate::analysis::ids::PlaceId>,
    call_operations: &BTreeSet<MirOpId>,
    row: &DomainObservationFact,
) {
    if row.stable_key.trim().is_empty() {
        push_domain_diagnostic(
            diagnostics,
            "DomainObservation",
            &row.stable_key,
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
        &row.stable_key,
    );
    check_ref(
        diagnostics,
        bodies,
        row.body,
        "DomainObservation",
        &row.stable_key,
        "body",
        "dangling MIR body reference",
    );
    check_optional_ref(
        diagnostics,
        blocks,
        row.block,
        "DomainObservation",
        &row.stable_key,
        "block",
        "dangling basic block reference",
    );
    check_optional_ref(
        diagnostics,
        operations,
        row.operation,
        "DomainObservation",
        &row.stable_key,
        "operation",
        "dangling MIR operation reference",
    );
    check_optional_ref(
        diagnostics,
        places,
        row.place,
        "DomainObservation",
        &row.stable_key,
        "place",
        "dangling place reference",
    );

    match &row.value {
        DomainValue::TopReason(reason) => validate_reasoned_status(
            diagnostics,
            "DomainObservation",
            &row.stable_key,
            row.status,
            row.precision,
            reason,
        ),
        DomainValue::Label(value) if value.trim().is_empty() => push_domain_diagnostic(
            diagnostics,
            "DomainObservation",
            &row.stable_key,
            "value",
            "label values must not be empty",
        ),
        DomainValue::DigestParts(parts) if parts.is_empty() => push_domain_diagnostic(
            diagnostics,
            "DomainObservation",
            &row.stable_key,
            "value",
            "digest-part values must not be empty",
        ),
        DomainValue::Label(_) | DomainValue::DigestParts(_) => {
            if reason_required(row.status) {
                push_domain_diagnostic(
                    diagnostics,
                    "DomainObservation",
                    &row.stable_key,
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
                    &row.stable_key,
                    "precision",
                    "present rows require ExactLocal, SetupAware, Conservative, or Heuristic precision",
                );
            }
        }
    }

    validate_unresolved_call_reference(
        diagnostics,
        "DomainObservation",
        &row.stable_key,
        row.operation,
        call_operations,
        reason_from_value(&row.value),
    );
}

fn validate_event(
    db: &AnalysisDb,
    diagnostics: &mut Vec<Diagnostic>,
    bodies: &BTreeSet<crate::analysis::ids::MirBodyId>,
    blocks: &BTreeSet<crate::analysis::cfg::ids::BasicBlockId>,
    operations: &BTreeSet<MirOpId>,
    call_operations: &BTreeSet<MirOpId>,
    row: &DomainEventFact,
) {
    if row.stable_key.trim().is_empty() {
        push_domain_diagnostic(
            diagnostics,
            "DomainEvent",
            &row.stable_key,
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
        &row.stable_key,
    );
    check_ref(
        diagnostics,
        bodies,
        row.body,
        "DomainEvent",
        &row.stable_key,
        "body",
        "dangling MIR body reference",
    );
    check_optional_ref(
        diagnostics,
        blocks,
        row.block,
        "DomainEvent",
        &row.stable_key,
        "block",
        "dangling basic block reference",
    );
    check_optional_ref(
        diagnostics,
        operations,
        row.operation,
        "DomainEvent",
        &row.stable_key,
        "operation",
        "dangling MIR operation reference",
    );
    if row.status == DomainStatus::Present {
        push_domain_diagnostic(
            diagnostics,
            "DomainEvent",
            &row.stable_key,
            "status",
            "domain events must describe top, unknown, setup, unsupported, or budget loss",
        );
    }
    validate_reasoned_status(
        diagnostics,
        "DomainEvent",
        &row.stable_key,
        row.status,
        row.precision,
        row.reason.as_str(),
    );
    validate_unresolved_call_reference(
        diagnostics,
        "DomainEvent",
        &row.stable_key,
        row.operation,
        call_operations,
        Some(row.reason.as_str()),
    );
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
    if metadata.stable_key != stable_key {
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

fn reason_from_value(value: &DomainValue) -> Option<&str> {
    match value {
        DomainValue::TopReason(reason) => Some(reason.as_str()),
        DomainValue::Label(_) | DomainValue::DigestParts(_) => None,
    }
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

fn check_duplicate_stable_keys<'a>(
    diagnostics: &mut Vec<Diagnostic>,
    family: &'static str,
    keys: impl Iterator<Item = &'a str>,
) {
    let mut seen = BTreeSet::new();
    for key in keys {
        if !seen.insert(key) {
            push_domain_diagnostic(
                diagnostics,
                family,
                key,
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
    diagnostics.push(Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Internal analysis validation failed.",
    ));
}
