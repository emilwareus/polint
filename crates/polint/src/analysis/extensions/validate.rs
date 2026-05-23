use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::sinks::{ExtensionFactCandidate, ExtensionFactPrecision};
use super::store::{AcceptedExtensionFact, ExtensionOutput, RejectedExtensionFact};
use crate::core::AnalysisDb;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExtensionRejectionReason {
    UndeclaredOutput,
    MissingBinding,
    InvalidSpan,
    MissingPrecision,
    MissingProvenance,
    SyntheticIdMissingEvidence,
    DuplicateStableKey,
    NativeConflict,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ExtensionValidationInput {
    pub(crate) declared_outputs: BTreeSet<String>,
    pub(crate) native_stable_keys: BTreeSet<String>,
    pub(crate) exact_validation_evidence: BTreeSet<String>,
    pub(crate) candidates: Vec<ExtensionFactCandidate>,
}

pub(crate) fn validate_extension_output(
    db: &AnalysisDb,
    input: ExtensionValidationInput,
) -> ExtensionOutput {
    let ExtensionValidationInput {
        declared_outputs,
        native_stable_keys,
        exact_validation_evidence,
        candidates,
    } = input;
    let rule_input = ExtensionValidationInput {
        declared_outputs,
        native_stable_keys,
        exact_validation_evidence,
        candidates: Vec::new(),
    };
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut accepted_keys = BTreeSet::new();

    for candidate in sorted_candidates(candidates) {
        let reason = rejection_reason(db, &rule_input, &candidate, &accepted_keys);
        if let Some(reason) = reason {
            rejected.push(RejectedExtensionFact::from_candidate(&candidate, reason));
        } else {
            accepted_keys.insert(candidate.stable_key.clone());
            accepted.push(AcceptedExtensionFact::from_candidate(candidate));
        }
    }

    ExtensionOutput {
        activations: Vec::new(),
        accepted,
        rejected,
    }
    .normalized()
}

fn sorted_candidates(mut candidates: Vec<ExtensionFactCandidate>) -> Vec<ExtensionFactCandidate> {
    candidates = candidates
        .into_iter()
        .map(ExtensionFactCandidate::normalized)
        .collect();
    candidates.sort_by(|left, right| left.output_sort_key().cmp(&right.output_sort_key()));
    candidates
}

fn rejection_reason(
    db: &AnalysisDb,
    input: &ExtensionValidationInput,
    candidate: &ExtensionFactCandidate,
    accepted_keys: &BTreeSet<String>,
) -> Option<ExtensionRejectionReason> {
    if !input.declared_outputs.contains(&candidate.fact_family) {
        return Some(ExtensionRejectionReason::UndeclaredOutput);
    }
    if candidate.precision.is_none() {
        return Some(ExtensionRejectionReason::MissingPrecision);
    }
    if candidate.evidence.is_empty() {
        return Some(ExtensionRejectionReason::MissingProvenance);
    }
    if candidate.stable_key.starts_with("synthetic:") && candidate.evidence.is_empty() {
        return Some(ExtensionRejectionReason::SyntheticIdMissingEvidence);
    }
    if !bindings_exist(db, &candidate.binding_refs) {
        return Some(ExtensionRejectionReason::MissingBinding);
    }
    if !span_is_valid(db, candidate) {
        return Some(ExtensionRejectionReason::InvalidSpan);
    }
    if accepted_keys.contains(&candidate.stable_key) {
        return Some(ExtensionRejectionReason::DuplicateStableKey);
    }
    if input.native_stable_keys.contains(&candidate.stable_key) {
        return Some(ExtensionRejectionReason::NativeConflict);
    }
    if candidate.precision == Some(ExtensionFactPrecision::Exact)
        && !input
            .exact_validation_evidence
            .contains(&candidate.stable_key)
    {
        return Some(ExtensionRejectionReason::MissingProvenance);
    }
    None
}

fn bindings_exist(db: &AnalysisDb, bindings: &[String]) -> bool {
    if bindings.is_empty() {
        return false;
    }
    bindings.iter().all(|binding| {
        if let Some(path) = binding.strip_prefix("file:") {
            db.files().iter().any(|file| file.relative_path == path)
        } else if let Some(key) = binding.strip_prefix("function:") {
            db.functions().iter().any(|function| function.name == key)
        } else {
            false
        }
    })
}

fn span_is_valid(db: &AnalysisDb, candidate: &ExtensionFactCandidate) -> bool {
    let Some(span) = &candidate.span else {
        return true;
    };
    db.files()
        .iter()
        .find(|file| file.relative_path == span.relative_path)
        .is_some_and(|file| {
            span.start_byte <= span.end_byte && (span.end_byte as usize) <= file.source.len()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::extensions::sinks::{
        ExtensionFactConfidence, ExtensionFactStatus, ExtensionSpanRef,
    };
    use crate::core::{AnalysisDb, Language};
    use std::sync::Arc;

    fn db() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        db.add_source_file(
            "src/app.ts".into(),
            "src/app.ts".to_string(),
            Language::TypeScript,
            Arc::from("export const app = 1;\n"),
            "hash".to_string(),
        );
        db
    }

    fn candidate(stable_key: &str) -> ExtensionFactCandidate {
        ExtensionFactCandidate {
            extension_id: "demo".to_string(),
            provider_id: "routes".to_string(),
            fact_family: "extension.routes".to_string(),
            stable_key: stable_key.to_string(),
            binding_refs: vec!["file:src/app.ts".to_string()],
            span: Some(ExtensionSpanRef {
                relative_path: "src/app.ts".to_string(),
                start_byte: 0,
                end_byte: 5,
            }),
            precision: Some(ExtensionFactPrecision::Heuristic),
            confidence: ExtensionFactConfidence::Medium,
            status: ExtensionFactStatus::Candidate,
            evidence: vec!["fixture".to_string()],
            payload_labels: vec!["kind=route".to_string()],
        }
    }

    fn input(candidates: Vec<ExtensionFactCandidate>) -> ExtensionValidationInput {
        ExtensionValidationInput {
            declared_outputs: BTreeSet::from(["extension.routes".to_string()]),
            native_stable_keys: BTreeSet::new(),
            exact_validation_evidence: BTreeSet::new(),
            candidates,
        }
    }

    #[test]
    fn rejects_undeclared_output_family() {
        let mut candidate = candidate("route:a");
        candidate.fact_family = "extension.other".to_string();

        let output = validate_extension_output(&db(), input(vec![candidate]));

        assert_eq!(output.accepted.len(), 0);
        assert_eq!(
            output.rejected[0].reason,
            ExtensionRejectionReason::UndeclaredOutput
        );
    }

    #[test]
    fn rejects_empty_binding_refs() {
        let mut candidate = candidate("route:no-bindings");
        candidate.binding_refs.clear();

        let output = validate_extension_output(&db(), input(vec![candidate]));

        assert_eq!(output.accepted.len(), 0);
        assert_eq!(
            output.rejected[0].reason,
            ExtensionRejectionReason::MissingBinding
        );
    }

    #[test]
    fn rejects_missing_binding_invalid_span_missing_precision_duplicate_and_native_conflict() {
        let mut missing_binding = candidate("route:missing-binding");
        missing_binding.binding_refs = vec!["file:missing.ts".to_string()];
        let mut invalid_span = candidate("route:invalid-span");
        invalid_span.span.as_mut().unwrap().end_byte = 10_000;
        let mut missing_precision = candidate("route:missing-precision");
        missing_precision.precision = None;
        let duplicate_a = candidate("route:duplicate");
        let duplicate_b = candidate("route:duplicate");
        let native_conflict = candidate("route:native");
        let mut validation_input = input(vec![
            missing_binding,
            invalid_span,
            missing_precision,
            duplicate_a,
            duplicate_b,
            native_conflict,
        ]);
        validation_input
            .native_stable_keys
            .insert("route:native".to_string());

        let output = validate_extension_output(&db(), validation_input);
        let reasons = output
            .rejected
            .iter()
            .map(|fact| fact.reason)
            .collect::<BTreeSet<_>>();

        assert!(reasons.contains(&ExtensionRejectionReason::MissingBinding));
        assert!(reasons.contains(&ExtensionRejectionReason::InvalidSpan));
        assert!(reasons.contains(&ExtensionRejectionReason::MissingPrecision));
        assert!(reasons.contains(&ExtensionRejectionReason::DuplicateStableKey));
        assert!(reasons.contains(&ExtensionRejectionReason::NativeConflict));
    }

    #[test]
    fn accepted_and_rejected_output_is_sorted_deterministically() {
        let output = validate_extension_output(
            &db(),
            input(vec![candidate("route:z"), candidate("route:a")]),
        );

        assert_eq!(
            output
                .accepted
                .iter()
                .map(|fact| fact.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["route:a", "route:z"]
        );
        assert!(output.rejected.is_empty());
    }
}
