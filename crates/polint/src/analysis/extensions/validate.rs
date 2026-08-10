use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::sinks::{
    ExtensionFactCandidate, ExtensionFactPrecision, REFINED_CALL_EDGE_FAMILY,
    TYPE_VALUE_ALIAS_ACCESS_PATH_FAMILY, TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY,
    TYPE_VALUE_ALIAS_ALLOCATION_FAMILY, TYPE_VALUE_ALIAS_POINTS_TO_CONSTRAINT_FAMILY,
    TYPE_VALUE_ALIAS_TYPE_FAMILY, TYPE_VALUE_ALIAS_VALUE_FAMILY, has_required_refined_call_payload,
    has_required_type_value_alias_payload, is_type_value_alias_fact_family,
};
use super::store::{AcceptedExtensionFact, ExtensionOutput, RejectedExtensionFact};
use crate::analysis::calls::facts::CallCallee;
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
    FrameworkPrecisionCeiling,
    TypeValueAliasPrecisionCeiling,
    MalformedPayload,
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
    let interner = db.stable_key_interner();
    let rule_input = ExtensionValidationInput {
        declared_outputs,
        native_stable_keys,
        exact_validation_evidence,
        candidates: Vec::new(),
    };
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();
    let mut accepted_keys = BTreeSet::<crate::core::StableKeyId>::new();

    let mut type_value_alias_refs = TypeValueAliasRefContext::from_db(db);
    let mut relation_candidates = Vec::new();

    for candidate in sorted_candidates(db, candidates) {
        if is_type_value_alias_relation_family(&candidate.fact_family) {
            relation_candidates.push(candidate);
            continue;
        }

        let reason = rejection_reason(
            db,
            &rule_input,
            &candidate,
            &accepted_keys,
            &type_value_alias_refs,
        );
        if let Some(reason) = reason {
            rejected.push(RejectedExtensionFact::from_candidate(
                &candidate, reason, &interner,
            ));
        } else {
            accepted_keys.insert(candidate.stable_key);
            type_value_alias_refs.record_extension_base_fact(&candidate);
            accepted.push(AcceptedExtensionFact::from_candidate(candidate, &interner));
        }
    }

    for candidate in relation_candidates {
        let reason = rejection_reason(
            db,
            &rule_input,
            &candidate,
            &accepted_keys,
            &type_value_alias_refs,
        );
        if let Some(reason) = reason {
            rejected.push(RejectedExtensionFact::from_candidate(
                &candidate, reason, &interner,
            ));
        } else {
            accepted_keys.insert(candidate.stable_key);
            accepted.push(AcceptedExtensionFact::from_candidate(candidate, &interner));
        }
    }

    ExtensionOutput {
        activations: Vec::new(),
        accepted,
        rejected,
    }
    .normalized(&interner)
}

fn sorted_candidates(
    db: &AnalysisDb,
    mut candidates: Vec<ExtensionFactCandidate>,
) -> Vec<ExtensionFactCandidate> {
    let interner = db.stable_key_interner();
    candidates = candidates
        .into_iter()
        .map(ExtensionFactCandidate::normalized)
        .collect();
    candidates.sort_by(|left, right| {
        left.output_sort_key(&interner)
            .cmp(&right.output_sort_key(&interner))
    });
    candidates
}

/// Framework fact families that are subject to framework-specific validation rules.
/// Extension-emitted facts with these families cannot claim Exact precision per D-18.
const FRAMEWORK_FACT_FAMILIES: &[&str] = &[
    "entrypoint",
    "trust_boundary",
    "dispatch_edge",
    "unresolved_framework",
];

fn is_framework_fact_family(family: &str) -> bool {
    FRAMEWORK_FACT_FAMILIES.contains(&family)
}

fn rejection_reason(
    db: &AnalysisDb,
    input: &ExtensionValidationInput,
    candidate: &ExtensionFactCandidate,
    accepted_keys: &BTreeSet<crate::core::StableKeyId>,
    type_value_alias_refs: &TypeValueAliasRefContext,
) -> Option<ExtensionRejectionReason> {
    let stable_key_text = db.resolve_stable_key(candidate.stable_key);
    if !input.declared_outputs.contains(&candidate.fact_family) {
        return Some(ExtensionRejectionReason::UndeclaredOutput);
    }
    if candidate.precision.is_none() {
        return Some(ExtensionRejectionReason::MissingPrecision);
    }
    if is_type_value_alias_fact_family(&candidate.fact_family)
        && !has_required_type_value_alias_payload(candidate)
    {
        return Some(ExtensionRejectionReason::MalformedPayload);
    }
    if candidate.fact_family == REFINED_CALL_EDGE_FAMILY
        && !has_required_refined_call_payload(candidate)
    {
        return Some(ExtensionRejectionReason::MalformedPayload);
    }
    if candidate.fact_family == REFINED_CALL_EDGE_FAMILY
        && !refined_call_refs_resolve(db, candidate)
    {
        return Some(ExtensionRejectionReason::MalformedPayload);
    }
    if is_type_value_alias_fact_family(&candidate.fact_family)
        && !type_value_alias_refs.resolve_candidate_refs(candidate)
    {
        return Some(ExtensionRejectionReason::MalformedPayload);
    }
    // Framework-specific precision ceiling: Exact is unconditionally rejected for
    // framework fact families per D-18, regardless of validation evidence.
    if is_framework_fact_family(&candidate.fact_family)
        && candidate.precision == Some(ExtensionFactPrecision::Exact)
    {
        return Some(ExtensionRejectionReason::FrameworkPrecisionCeiling);
    }
    if is_type_value_alias_fact_family(&candidate.fact_family)
        && candidate.precision == Some(ExtensionFactPrecision::Exact)
        && !input
            .exact_validation_evidence
            .contains(stable_key_text.as_ref())
    {
        return Some(ExtensionRejectionReason::TypeValueAliasPrecisionCeiling);
    }
    if stable_key_text.starts_with("synthetic:") && candidate.evidence.is_empty() {
        return Some(ExtensionRejectionReason::SyntheticIdMissingEvidence);
    }
    if candidate.evidence.is_empty() {
        return Some(ExtensionRejectionReason::MissingProvenance);
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
    if input.native_stable_keys.contains(stable_key_text.as_ref()) {
        return Some(ExtensionRejectionReason::NativeConflict);
    }
    if candidate.precision == Some(ExtensionFactPrecision::Exact)
        && !input
            .exact_validation_evidence
            .contains(stable_key_text.as_ref())
    {
        return Some(ExtensionRejectionReason::MissingProvenance);
    }
    None
}

fn is_type_value_alias_relation_family(family: &str) -> bool {
    matches!(
        family,
        TYPE_VALUE_ALIAS_POINTS_TO_CONSTRAINT_FAMILY | TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY
    )
}

fn refined_call_refs_resolve(db: &AnalysisDb, candidate: &ExtensionFactCandidate) -> bool {
    payload_value(candidate, "site=").is_some_and(|site| resolve_call_site_ref(db, site))
        && payload_value(candidate, "target_function=")
            .is_none_or(|function| resolve_function_ref(db, function))
        && payload_value(candidate, "target_symbol=")
            .is_none_or(|symbol| resolve_symbol_ref(db, symbol))
}

fn resolve_call_site_ref(db: &AnalysisDb, value: &str) -> bool {
    if let Some(id) = value.strip_prefix("call_site:") {
        return id.parse::<u64>().map_or_else(
            |_| {
                db.call_sites()
                    .iter()
                    .any(|site| db.resolve_stable_key(site.stable_key).as_ref() == id)
            },
            |id| {
                db.call_sites()
                    .iter()
                    .any(|site| site.id == crate::analysis::ids::CallSiteId(id))
            },
        );
    }
    value.strip_prefix("stable:").is_some_and(|stable| {
        db.call_sites()
            .iter()
            .any(|site| db.resolve_stable_key(site.stable_key).as_ref() == stable)
    }) || value
        .strip_prefix("file_span:")
        .is_some_and(|file_span| resolve_call_site_file_span_ref(db, file_span))
        || value
            .strip_prefix("file_callee:")
            .is_some_and(|file_callee| resolve_call_site_file_callee_ref(db, file_callee))
}

fn resolve_call_site_file_span_ref(db: &AnalysisDb, value: &str) -> bool {
    let Some((relative_path, start_byte)) = value.rsplit_once(':') else {
        return false;
    };
    let Ok(start_byte) = start_byte.parse::<u32>() else {
        return false;
    };
    db.call_sites().iter().any(|site| {
        site.span.start_byte == start_byte
            && db
                .file(site.file)
                .is_some_and(|file| file.relative_path == relative_path)
    })
}

fn resolve_call_site_file_callee_ref(db: &AnalysisDb, value: &str) -> bool {
    let Some((relative_path, callee)) = value.rsplit_once(':') else {
        return false;
    };
    db.call_sites()
        .iter()
        .filter(|site| {
            db.file(site.file)
                .is_some_and(|file| file.relative_path == relative_path)
                && call_site_callee_label(&site.callee) == Some(callee)
        })
        .count()
        == 1
}

fn call_site_callee_label(callee: &CallCallee) -> Option<&str> {
    match callee {
        CallCallee::Identifier { name, .. } => Some(name.as_str()),
        CallCallee::Member { property, .. } => Some(property.as_str()),
        CallCallee::Constructor { name, .. } => name.as_deref(),
        _ => None,
    }
}

fn resolve_function_ref(db: &AnalysisDb, value: &str) -> bool {
    value
        .strip_prefix("function:")
        .and_then(|id| id.parse::<u64>().ok())
        .is_some_and(|id| {
            db.functions()
                .iter()
                .any(|function| function.id == crate::core::FunctionId(id))
        })
}

fn resolve_symbol_ref(db: &AnalysisDb, value: &str) -> bool {
    value
        .strip_prefix("symbol:")
        .and_then(|id| id.parse::<u64>().ok())
        .is_some_and(|id| {
            db.symbols()
                .iter()
                .any(|symbol| symbol.id == crate::core::SymbolId(id))
        })
}

#[derive(Debug, Default)]
struct TypeValueAliasRefContext {
    places: BTreeSet<u64>,
    extension_access_paths: BTreeSet<u64>,
    extension_allocations: BTreeSet<u64>,
    extension_values: BTreeSet<u64>,
}

impl TypeValueAliasRefContext {
    fn from_db(db: &AnalysisDb) -> Self {
        Self {
            places: db.mir_places().iter().map(|place| place.id.0).collect(),
            ..Self::default()
        }
    }

    fn record_extension_base_fact(&mut self, candidate: &ExtensionFactCandidate) {
        match candidate.fact_family.as_str() {
            TYPE_VALUE_ALIAS_VALUE_FAMILY => {
                self.extension_values
                    .insert(self.extension_values.len() as u64);
            }
            TYPE_VALUE_ALIAS_ALLOCATION_FAMILY => {
                self.extension_allocations
                    .insert(self.extension_allocations.len() as u64);
            }
            TYPE_VALUE_ALIAS_ACCESS_PATH_FAMILY => {
                self.extension_access_paths
                    .insert(self.extension_access_paths.len() as u64);
            }
            _ => {}
        }
    }

    fn resolve_candidate_refs(&self, candidate: &ExtensionFactCandidate) -> bool {
        match candidate.fact_family.as_str() {
            TYPE_VALUE_ALIAS_TYPE_FAMILY => {
                payload_value(candidate, "subject=")
                    .is_none_or(|value| self.resolve_place_ref_or_non_numeric(value, "place:"))
                    && payload_value(candidate, "place=")
                        .is_none_or(|value| self.resolve_place_ref(value))
            }
            TYPE_VALUE_ALIAS_VALUE_FAMILY => {
                payload_value(candidate, "subject=")
                    .is_none_or(|value| self.resolve_place_ref_or_non_numeric(value, "place:"))
                    && payload_value(candidate, "kind=").is_none_or(|value| {
                        self.resolve_place_ref_or_non_numeric(value, "place_ref:")
                    })
            }
            TYPE_VALUE_ALIAS_ALLOCATION_FAMILY => payload_value(candidate, "source_place=")
                .is_none_or(|value| self.resolve_place_ref(value)),
            TYPE_VALUE_ALIAS_ACCESS_PATH_FAMILY => {
                payload_value(candidate, "base=").is_some_and(|value| self.resolve_place_ref(value))
            }
            TYPE_VALUE_ALIAS_POINTS_TO_CONSTRAINT_FAMILY => {
                self.resolve_points_to_ref(payload_value(candidate, "dst="))
                    && payload_value(candidate, "src=")
                        .is_none_or(|value| self.resolve_points_to_ref(Some(value)))
                    && payload_value(candidate, "object=")
                        .is_none_or(|value| self.resolve_object_ref(Some(value)))
            }
            TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY => {
                self.resolve_alias_operand_ref(payload_value(candidate, "left="))
                    && self.resolve_alias_operand_ref(payload_value(candidate, "right="))
            }
            _ => true,
        }
    }

    fn resolve_place_ref_or_non_numeric(&self, value: &str, prefix: &str) -> bool {
        value
            .strip_prefix(prefix)
            .is_none_or(|id| self.resolve_id(id, &self.places))
    }

    fn resolve_place_ref(&self, value: &str) -> bool {
        value
            .strip_prefix("place:")
            .is_some_and(|id| self.resolve_id(id, &self.places))
    }

    fn resolve_points_to_ref(&self, value: Option<&str>) -> bool {
        let Some(value) = value else {
            return false;
        };
        if let Some(id) = value.strip_prefix("place:") {
            self.resolve_id(id, &self.places)
        } else if let Some(id) = value.strip_prefix("access_path:") {
            self.resolve_id(id, &self.extension_access_paths)
        } else if let Some(id) = value.strip_prefix("allocation:") {
            self.resolve_id(id, &self.extension_allocations)
        } else {
            false
        }
    }

    fn resolve_object_ref(&self, value: Option<&str>) -> bool {
        let Some(value) = value else {
            return false;
        };
        if let Some(id) = value.strip_prefix("allocation:") {
            self.resolve_id(id, &self.extension_allocations)
        } else if let Some(id) = value.strip_prefix("value:") {
            self.resolve_id(id, &self.extension_values)
        } else if let Some(id) = value.strip_prefix("abstract_value:") {
            self.resolve_id(id, &self.extension_values)
        } else {
            false
        }
    }

    fn resolve_alias_operand_ref(&self, value: Option<&str>) -> bool {
        let Some(value) = value else {
            return false;
        };
        if let Some(id) = value.strip_prefix("place:") {
            self.resolve_id(id, &self.places)
        } else if let Some(id) = value.strip_prefix("access_path:") {
            self.resolve_id(id, &self.extension_access_paths)
        } else {
            false
        }
    }

    fn resolve_id(&self, id: &str, accepted: &BTreeSet<u64>) -> bool {
        id.parse::<u64>()
            .is_ok_and(|parsed| accepted.contains(&parsed))
    }
}

fn payload_value<'a>(candidate: &'a ExtensionFactCandidate, prefix: &str) -> Option<&'a str> {
    candidate
        .payload_labels
        .iter()
        .find_map(|label| label.strip_prefix(prefix))
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
    use crate::analysis::calls::facts::{
        CallCallee, CallPrecision, CallSiteFact, CallSyntaxKind, CallTargetStatus,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::extensions::sinks::{
        ExtensionFactConfidence, ExtensionFactStatus, ExtensionSpanRef, REFINED_CALL_EDGE_FAMILY,
        TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY,
    };
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId};
    use crate::analysis::mir::body::MirOutput;
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span};
    use std::sync::Arc;

    fn db() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_source_file(
            "src/app.ts".into(),
            "src/app.ts".to_string(),
            Language::TypeScript,
            Arc::from("export const app = 1;\n"),
            "hash".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "app".to_string(),
            span: span(),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: vec!["model".to_string()],
        });
        let interner = db.stable_key_interner();
        db.replace_semantic_mir(MirOutput {
            bodies: Vec::new(),
            places: (0..3).map(|id| test_place(&interner, id)).collect(),
            operations: Vec::new(),
            unsupported: Vec::new(),
            ..MirOutput::default()
        })
        .expect("semantic MIR fixture should be valid");
        db.replace_call_facts(CallOutput {
            sites: vec![CallSiteFact {
                in_throw: false,
                id: CallSiteId(0),
                language: Language::TypeScript,
                file,
                caller: FunctionId(0),
                owner_symbol: None,
                body: MirBodyId(0),
                operation: MirOpId(0),
                span: span(),
                kind: CallSyntaxKind::Function,
                callee: CallCallee::Identifier {
                    reference: None,
                    name: "model".to_string(),
                },
                receiver: None,
                arguments: Vec::new(),
                result: None,
                status: CallTargetStatus::Ambiguous,
                precision: CallPrecision::Heuristic,
                stable_key: crate::core::StableKeyId(0),
            }],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call fixture should be valid");
        db
    }

    fn span() -> Span {
        Span::point(FileId(0), 1, 1)
    }

    fn test_place(interner: &crate::core::StableKeyInterner, id: u64) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            function: Some(FunctionId(0)),
            root: PlaceRoot::Local {
                function: FunctionId(0),
                name: format!("p{id}"),
            },
            projections: Vec::new(),
            stable_key: interner.intern(format!("place:{id}")),
            status: PlaceStatus::Resolved,
        }
    }

    fn candidate(stable_key: &str) -> ExtensionFactCandidate {
        ExtensionFactCandidate {
            extension_id: "demo".to_string(),
            provider_id: "routes".to_string(),
            fact_family: "extension.routes".to_string(),
            stable_key: crate::core::stable_key_for_test(stable_key),
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

    fn refined_call_candidate(
        stable_key: &str,
        payload_labels: Vec<&str>,
    ) -> ExtensionFactCandidate {
        let mut c = candidate(stable_key);
        c.fact_family = REFINED_CALL_EDGE_FAMILY.to_string();
        c.payload_labels = payload_labels
            .into_iter()
            .map(str::to_string)
            .collect::<Vec<_>>();
        c
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
        let db = db();
        let output =
            validate_extension_output(&db, input(vec![candidate("route:z"), candidate("route:a")]));

        let interner = db.stable_key_interner();
        assert_eq!(
            output
                .accepted
                .iter()
                .map(|fact| interner.resolve(fact.stable_key).to_string())
                .collect::<Vec<_>>(),
            vec!["route:a".to_string(), "route:z".to_string()]
        );
        assert!(output.rejected.is_empty());
    }

    #[test]
    fn rejects_exact_precision_for_entrypoint_framework_family() {
        let mut c = candidate("ep:exact");
        c.fact_family = "entrypoint".to_string();
        c.precision = Some(ExtensionFactPrecision::Exact);
        let mut validation_input = input(vec![c]);
        validation_input
            .declared_outputs
            .insert("entrypoint".to_string());

        let output = validate_extension_output(&db(), validation_input);

        assert_eq!(output.accepted.len(), 0);
        assert_eq!(output.rejected.len(), 1);
        assert_eq!(
            output.rejected[0].reason,
            ExtensionRejectionReason::FrameworkPrecisionCeiling
        );
    }

    #[test]
    fn rejects_exact_precision_for_all_framework_fact_families() {
        for family in &[
            "entrypoint",
            "trust_boundary",
            "dispatch_edge",
            "unresolved_framework",
        ] {
            let mut c = candidate(&format!("{family}:exact"));
            c.fact_family = family.to_string();
            c.precision = Some(ExtensionFactPrecision::Exact);
            let mut validation_input = input(vec![c]);
            validation_input.declared_outputs.insert(family.to_string());

            let output = validate_extension_output(&db(), validation_input);

            assert_eq!(
                output.rejected[0].reason,
                ExtensionRejectionReason::FrameworkPrecisionCeiling,
                "expected FrameworkPrecisionCeiling for family {family}"
            );
        }
    }

    #[test]
    fn extension_fact_conflicting_with_native_entrypoint_stable_key_is_rejected() {
        let mut c = candidate("ep:native-route");
        c.fact_family = "entrypoint".to_string();
        c.precision = Some(ExtensionFactPrecision::Heuristic);
        let mut validation_input = input(vec![c]);
        validation_input
            .declared_outputs
            .insert("entrypoint".to_string());
        validation_input
            .native_stable_keys
            .insert("ep:native-route".to_string());

        let output = validate_extension_output(&db(), validation_input);

        assert_eq!(output.accepted.len(), 0);
        assert_eq!(output.rejected.len(), 1);
        assert_eq!(
            output.rejected[0].reason,
            ExtensionRejectionReason::NativeConflict
        );
    }

    #[test]
    fn non_conflicting_extension_entrypoint_is_accepted_with_extension_precision() {
        let mut c = candidate("ep:extension-only");
        c.fact_family = "entrypoint".to_string();
        c.precision = Some(ExtensionFactPrecision::Heuristic);
        let mut validation_input = input(vec![c]);
        validation_input
            .declared_outputs
            .insert("entrypoint".to_string());

        let output = validate_extension_output(&db(), validation_input);

        assert_eq!(output.accepted.len(), 1);
        assert_eq!(output.rejected.len(), 0);
        assert_eq!(
            output.accepted[0].precision,
            ExtensionFactPrecision::Heuristic
        );
        assert_eq!(output.accepted[0].fact_family, "entrypoint");
    }

    #[test]
    fn non_exact_framework_fact_precision_is_accepted() {
        for precision in &[
            ExtensionFactPrecision::SetupAware,
            ExtensionFactPrecision::Heuristic,
            ExtensionFactPrecision::GeneratedUnvalidated,
        ] {
            let mut c = candidate(&format!("ep:{precision:?}"));
            c.fact_family = "entrypoint".to_string();
            c.precision = Some(*precision);
            let mut validation_input = input(vec![c]);
            validation_input
                .declared_outputs
                .insert("entrypoint".to_string());

            let output = validate_extension_output(&db(), validation_input);

            assert_eq!(
                output.accepted.len(),
                1,
                "expected acceptance for precision {precision:?}"
            );
            assert_eq!(output.accepted[0].precision, *precision);
        }
    }

    #[test]
    fn type_value_alias_fact_family_accepts_well_formed_candidate_payload() {
        let mut c = candidate("alias:extension");
        c.fact_family = TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY.to_string();
        c.payload_labels = vec![
            "left=place:1".to_string(),
            "right=place:2".to_string(),
            "status=no_alias".to_string(),
        ];
        let mut validation_input = input(vec![c]);
        validation_input
            .declared_outputs
            .insert(TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY.to_string());

        let output = validate_extension_output(&db(), validation_input);

        assert_eq!(output.accepted.len(), 1);
        assert_eq!(output.accepted[0].status, ExtensionFactStatus::Accepted);
        assert_eq!(
            output.accepted[0].fact_family,
            TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY
        );
        assert_eq!(
            output.accepted[0].precision,
            ExtensionFactPrecision::Heuristic
        );
        assert!(!output.accepted[0].evidence.is_empty());
        assert!(!output.accepted[0].payload_digest.is_empty());
    }

    #[test]
    fn type_value_alias_fact_family_rejects_malformed_payload_before_merge() {
        let cases = [
            (
                "alias:missing_status",
                vec!["left=place:1", "right=place:2"],
            ),
            (
                "alias:invalid_left",
                vec!["left=garbage", "right=place:2", "status=no_alias"],
            ),
            (
                "alias:invalid_right",
                vec!["left=place:1", "right=garbage", "status=no_alias"],
            ),
            (
                "alias:unresolved_left",
                vec!["left=place:999", "right=place:2", "status=no_alias"],
            ),
            (
                "alias:unresolved_right",
                vec!["left=place:1", "right=place:999", "status=no_alias"],
            ),
        ];

        for (stable_key, payload_labels) in cases {
            let mut c = candidate(stable_key);
            c.fact_family = TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY.to_string();
            c.payload_labels = payload_labels
                .into_iter()
                .map(str::to_string)
                .collect::<Vec<_>>();
            let mut validation_input = input(vec![c]);
            validation_input
                .declared_outputs
                .insert(TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY.to_string());

            let output = validate_extension_output(&db(), validation_input);

            assert_eq!(output.accepted.len(), 0, "{stable_key}");
            assert_eq!(
                output.rejected[0].reason,
                ExtensionRejectionReason::MalformedPayload,
                "{stable_key}"
            );
        }
    }

    #[test]
    fn type_value_alias_exact_claims_require_validation_evidence() {
        let mut c = candidate("alias:exact");
        c.fact_family = TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY.to_string();
        c.precision = Some(ExtensionFactPrecision::Exact);
        c.payload_labels = vec![
            "left=place:1".to_string(),
            "right=place:2".to_string(),
            "status=must_alias".to_string(),
        ];
        let mut validation_input = input(vec![c]);
        validation_input
            .declared_outputs
            .insert(TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY.to_string());

        let output = validate_extension_output(&db(), validation_input);

        assert_eq!(
            output.rejected[0].reason,
            ExtensionRejectionReason::TypeValueAliasPrecisionCeiling
        );
    }

    #[test]
    fn refined_call_fact_family_accepts_valid_file_callee_site_payload() {
        let mut validation_input = input(vec![refined_call_candidate(
            "refined:valid",
            vec![
                "site=file_callee:src/app.ts:model",
                "synthetic_target=extension:model-target",
                "algorithm=repo_model",
                "status=resolved",
            ],
        )]);
        validation_input
            .declared_outputs
            .insert(REFINED_CALL_EDGE_FAMILY.to_string());

        let output = validate_extension_output(&db(), validation_input);

        assert_eq!(output.accepted.len(), 1);
        assert!(output.rejected.is_empty());
    }

    #[test]
    fn refined_call_fact_family_rejects_dangling_site_and_native_targets() {
        let cases = [
            (
                "refined:missing-site",
                vec![
                    "site=call_site:99",
                    "synthetic_target=extension:model-target",
                    "algorithm=repo_model",
                    "status=resolved",
                ],
            ),
            (
                "refined:missing-function",
                vec![
                    "site=call_site:0",
                    "target_function=function:99",
                    "algorithm=repo_model",
                    "status=resolved",
                ],
            ),
            (
                "refined:missing-symbol",
                vec![
                    "site=call_site:0",
                    "target_symbol=symbol:99",
                    "algorithm=repo_model",
                    "status=resolved",
                ],
            ),
        ];

        for (stable_key, payload_labels) in cases {
            let mut validation_input =
                input(vec![refined_call_candidate(stable_key, payload_labels)]);
            validation_input
                .declared_outputs
                .insert(REFINED_CALL_EDGE_FAMILY.to_string());

            let output = validate_extension_output(&db(), validation_input);

            assert_eq!(output.accepted.len(), 0, "{stable_key}");
            assert_eq!(
                output.rejected[0].reason,
                ExtensionRejectionReason::MalformedPayload,
                "{stable_key}"
            );
        }
    }
}
