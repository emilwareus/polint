use serde::{Deserialize, Serialize};

pub(crate) const TYPE_VALUE_ALIAS_TYPE_FAMILY: &str = "type_value_alias.type";
pub(crate) const TYPE_VALUE_ALIAS_VALUE_FAMILY: &str = "type_value_alias.value";
pub(crate) const TYPE_VALUE_ALIAS_ALLOCATION_FAMILY: &str = "type_value_alias.allocation_token";
pub(crate) const TYPE_VALUE_ALIAS_ACCESS_PATH_FAMILY: &str = "type_value_alias.access_path";
pub(crate) const TYPE_VALUE_ALIAS_POINTS_TO_CONSTRAINT_FAMILY: &str =
    "type_value_alias.points_to_constraint";
pub(crate) const TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY: &str = "type_value_alias.alias_answer";
pub(crate) const REFINED_CALL_EDGE_FAMILY: &str = "refined_calls.edge";

pub(crate) const TYPE_VALUE_ALIAS_FACT_FAMILIES: &[&str] = &[
    TYPE_VALUE_ALIAS_TYPE_FAMILY,
    TYPE_VALUE_ALIAS_VALUE_FAMILY,
    TYPE_VALUE_ALIAS_ALLOCATION_FAMILY,
    TYPE_VALUE_ALIAS_ACCESS_PATH_FAMILY,
    TYPE_VALUE_ALIAS_POINTS_TO_CONSTRAINT_FAMILY,
    TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY,
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExtensionFactCandidate {
    pub(crate) extension_id: String,
    pub(crate) provider_id: String,
    pub(crate) fact_family: String,
    pub(crate) stable_key: String,
    pub(crate) binding_refs: Vec<String>,
    pub(crate) span: Option<ExtensionSpanRef>,
    pub(crate) precision: Option<ExtensionFactPrecision>,
    pub(crate) confidence: ExtensionFactConfidence,
    pub(crate) status: ExtensionFactStatus,
    pub(crate) evidence: Vec<String>,
    pub(crate) payload_labels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExtensionSpanRef {
    pub(crate) relative_path: String,
    pub(crate) start_byte: u32,
    pub(crate) end_byte: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExtensionFactPrecision {
    Exact,
    SetupAware,
    Heuristic,
    GeneratedUnvalidated,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExtensionFactConfidence {
    High,
    Medium,
    Low,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExtensionFactStatus {
    Accepted,
    Candidate,
    Rejected,
}

impl ExtensionFactCandidate {
    pub(crate) fn normalized(mut self) -> Self {
        self.binding_refs.sort();
        self.binding_refs.dedup();
        self.evidence.sort();
        self.evidence.dedup();
        self.payload_labels.sort();
        self.payload_labels.dedup();
        self
    }

    pub(crate) fn output_sort_key(&self) -> (&str, &str, &str, &str) {
        (
            &self.extension_id,
            &self.provider_id,
            &self.fact_family,
            &self.stable_key,
        )
    }
}

pub(crate) fn is_type_value_alias_fact_family(family: &str) -> bool {
    TYPE_VALUE_ALIAS_FACT_FAMILIES.contains(&family)
}

pub(crate) fn has_required_type_value_alias_payload(candidate: &ExtensionFactCandidate) -> bool {
    if !is_type_value_alias_fact_family(&candidate.fact_family) {
        return true;
    }
    match candidate.fact_family.as_str() {
        TYPE_VALUE_ALIAS_TYPE_FAMILY => {
            payload_label(candidate, "subject=").is_some_and(valid_type_subject)
                && payload_label(candidate, "shape=").is_some_and(valid_type_shape)
                && valid_optional_language(candidate)
        }
        TYPE_VALUE_ALIAS_VALUE_FAMILY => {
            payload_label(candidate, "subject=").is_some_and(valid_value_subject)
                && payload_label(candidate, "kind=").is_some_and(valid_value_kind)
                && valid_optional_language(candidate)
        }
        TYPE_VALUE_ALIAS_ALLOCATION_FAMILY => {
            payload_label(candidate, "kind=").is_some_and(valid_allocation_kind)
                && valid_optional_language(candidate)
        }
        TYPE_VALUE_ALIAS_ACCESS_PATH_FAMILY => {
            payload_label(candidate, "base=").is_some_and(valid_place_ref)
                && candidate
                    .payload_labels
                    .iter()
                    .filter_map(|label| label.strip_prefix("projection="))
                    .all(valid_access_path_projection)
                && has_payload_label(candidate, "projection=")
                && valid_optional_language(candidate)
        }
        TYPE_VALUE_ALIAS_POINTS_TO_CONSTRAINT_FAMILY => {
            has_type_value_alias_points_to_payload(candidate)
        }
        TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY => {
            payload_label(candidate, "left=").is_some_and(valid_alias_operand_ref)
                && payload_label(candidate, "right=").is_some_and(valid_alias_operand_ref)
                && payload_label(candidate, "status=").is_some_and(|status| {
                    matches!(
                        status,
                        "no_alias" | "may_alias" | "must_alias" | "partial_alias" | "unknown"
                    )
                })
        }
        _ => true,
    }
}

pub(crate) fn has_required_refined_call_payload(candidate: &ExtensionFactCandidate) -> bool {
    if candidate.fact_family != REFINED_CALL_EDGE_FAMILY {
        return true;
    }
    payload_label(candidate, "site=").is_some_and(valid_call_site_ref)
        && has_refined_call_target(candidate)
        && payload_label(candidate, "algorithm=").is_some_and(valid_refined_call_algorithm)
        && payload_label(candidate, "status=").is_some_and(valid_refined_call_status)
}

fn has_refined_call_target(candidate: &ExtensionFactCandidate) -> bool {
    payload_label(candidate, "target_function=").is_some_and(valid_function_ref)
        || payload_label(candidate, "target_symbol=").is_some_and(valid_symbol_ref)
        || payload_label(candidate, "synthetic_target=").is_some_and(|value| !value.is_empty())
}

fn valid_call_site_ref(value: &str) -> bool {
    has_nonempty_prefixed_value(value, "call_site:")
        || has_nonempty_prefixed_value(value, "stable:")
}

fn valid_function_ref(value: &str) -> bool {
    valid_numeric_ref(value, "function:")
}

fn valid_symbol_ref(value: &str) -> bool {
    valid_numeric_ref(value, "symbol:")
}

fn valid_refined_call_algorithm(value: &str) -> bool {
    matches!(
        value,
        "repo_model" | "framework_model" | "function_token" | "points_to"
    )
}

fn valid_refined_call_status(value: &str) -> bool {
    matches!(
        value,
        "resolved" | "ambiguous" | "unresolved" | "budget_exceeded" | "rejected"
    )
}

fn has_type_value_alias_points_to_payload(candidate: &ExtensionFactCandidate) -> bool {
    let Some(kind) = payload_label(candidate, "kind=") else {
        return false;
    };
    if !payload_label(candidate, "dst=").is_some_and(valid_points_to_var_ref) {
        return false;
    }
    match kind {
        "address_of" => payload_label(candidate, "object=").is_some_and(valid_object_token_ref),
        "copy" => payload_label(candidate, "src=").is_some_and(valid_points_to_var_ref),
        _ => false,
    }
}

fn valid_type_subject(value: &str) -> bool {
    valid_numeric_ref(value, "place:")
        || valid_numeric_ref(value, "symbol:")
        || has_nonempty_prefixed_value(value, "synthetic:")
        || has_nonempty_prefixed_value(value, "unknown:")
}

fn valid_value_subject(value: &str) -> bool {
    valid_numeric_ref(value, "place:")
        || has_nonempty_prefixed_value(value, "synthetic:")
        || has_nonempty_prefixed_value(value, "unknown:")
}

fn valid_type_shape(value: &str) -> bool {
    matches!(value, "any" | "object" | "class")
        || has_nonempty_prefixed_value(value, "primitive:")
        || has_nonempty_prefixed_value(value, "literal:")
        || has_nonempty_prefixed_value(value, "callable:")
        || has_nonempty_prefixed_value(value, "module:")
        || has_nonempty_prefixed_value(value, "nominal:")
        || has_nonempty_prefixed_value(value, "structural:")
        || has_nonempty_prefixed_value(value, "unsupported:")
        || has_nonempty_prefixed_value(value, "unknown:")
}

fn valid_value_kind(value: &str) -> bool {
    matches!(
        value,
        "null" | "undefined" | "nil" | "function" | "class" | "module"
    ) || has_nonempty_prefixed_value(value, "bool:")
        || has_nonempty_prefixed_value(value, "number:")
        || has_nonempty_prefixed_value(value, "string:")
        || has_nonempty_prefixed_value(value, "literal:")
        || valid_numeric_ref(value, "place_ref:")
        || has_nonempty_prefixed_value(value, "unknown:")
}

fn valid_allocation_kind(value: &str) -> bool {
    matches!(
        value,
        "object"
            | "array"
            | "composite"
            | "function"
            | "class"
            | "module"
            | "closure"
            | "synthetic_framework"
            | "extension"
            | "unknown"
    )
}

fn valid_access_path_projection(value: &str) -> bool {
    value == "base"
        || value == "deref"
        || value == "await"
        || has_nonempty_prefixed_value(value, "field:")
        || has_nonempty_prefixed_value(value, "property:")
        || has_nonempty_prefixed_value(value, "index:")
        || has_nonempty_prefixed_value(value, "unknown:")
}

fn valid_optional_language(candidate: &ExtensionFactCandidate) -> bool {
    payload_label(candidate, "language=").is_none_or(|language| {
        matches!(
            language,
            "go" | "javascript" | "jsx" | "typescript" | "tsx" | "unknown"
        )
    })
}

fn valid_place_ref(value: &str) -> bool {
    valid_numeric_ref(value, "place:")
}

fn valid_points_to_var_ref(value: &str) -> bool {
    valid_numeric_ref(value, "place:")
        || valid_numeric_ref(value, "access_path:")
        || valid_numeric_ref(value, "allocation:")
}

fn valid_object_token_ref(value: &str) -> bool {
    valid_numeric_ref(value, "allocation:")
        || valid_numeric_ref(value, "value:")
        || valid_numeric_ref(value, "abstract_value:")
}

fn valid_alias_operand_ref(value: &str) -> bool {
    valid_numeric_ref(value, "place:") || valid_numeric_ref(value, "access_path:")
}

fn valid_numeric_ref(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|id| !id.is_empty() && id.parse::<u64>().is_ok())
}

fn has_nonempty_prefixed_value(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|suffix| !suffix.is_empty())
}

fn payload_label<'a>(candidate: &'a ExtensionFactCandidate, prefix: &str) -> Option<&'a str> {
    candidate
        .payload_labels
        .iter()
        .find_map(|label| label.strip_prefix(prefix))
}

fn has_payload_label(candidate: &ExtensionFactCandidate, prefix: &str) -> bool {
    candidate
        .payload_labels
        .iter()
        .any(|label| label.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sink_output_normalization_is_independent_of_emission_order() {
        let first = ExtensionFactCandidate {
            extension_id: "demo".to_string(),
            provider_id: "routes".to_string(),
            fact_family: "extension.routes".to_string(),
            stable_key: "route:/a".to_string(),
            binding_refs: vec!["file:src/a.ts".to_string(), "file:src/a.ts".to_string()],
            span: None,
            precision: Some(ExtensionFactPrecision::Heuristic),
            confidence: ExtensionFactConfidence::Medium,
            status: ExtensionFactStatus::Candidate,
            evidence: vec!["b".to_string(), "a".to_string()],
            payload_labels: vec!["kind=route".to_string(), "method=GET".to_string()],
        }
        .normalized();
        let second = ExtensionFactCandidate {
            evidence: vec!["a".to_string(), "b".to_string()],
            payload_labels: vec!["method=GET".to_string(), "kind=route".to_string()],
            binding_refs: vec!["file:src/a.ts".to_string()],
            ..first.clone()
        }
        .normalized();

        assert_eq!(first, second);
        assert_eq!(
            first.output_sort_key(),
            ("demo", "routes", "extension.routes", "route:/a")
        );
    }

    #[test]
    fn type_value_alias_families_require_typed_payload_labels() {
        let mut candidate = ExtensionFactCandidate {
            extension_id: "demo".to_string(),
            provider_id: "types".to_string(),
            fact_family: TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY.to_string(),
            stable_key: "alias:extension".to_string(),
            binding_refs: vec!["file:src/app.ts".to_string()],
            span: None,
            precision: Some(ExtensionFactPrecision::Heuristic),
            confidence: ExtensionFactConfidence::Medium,
            status: ExtensionFactStatus::Candidate,
            evidence: vec!["extension-evidence".to_string()],
            payload_labels: vec![
                "left=place:1".to_string(),
                "right=place:2".to_string(),
                "status=no_alias".to_string(),
            ],
        };

        assert!(is_type_value_alias_fact_family(&candidate.fact_family));
        assert!(has_required_type_value_alias_payload(&candidate));

        candidate.payload_labels.pop();

        assert!(!has_required_type_value_alias_payload(&candidate));
    }

    #[test]
    fn type_value_alias_payload_rejects_unknown_enums_and_raw_points_to_refs() {
        let mut candidate = ExtensionFactCandidate {
            extension_id: "demo".to_string(),
            provider_id: "types".to_string(),
            fact_family: TYPE_VALUE_ALIAS_POINTS_TO_CONSTRAINT_FAMILY.to_string(),
            stable_key: "pt:extension".to_string(),
            binding_refs: vec!["file:src/app.ts".to_string()],
            span: None,
            precision: Some(ExtensionFactPrecision::Heuristic),
            confidence: ExtensionFactConfidence::Medium,
            status: ExtensionFactStatus::Candidate,
            evidence: vec!["extension-evidence".to_string()],
            payload_labels: vec![
                "kind=address_of".to_string(),
                "dst=place:1".to_string(),
                "object=allocation:2".to_string(),
            ],
        };

        assert!(has_required_type_value_alias_payload(&candidate));

        candidate.payload_labels = vec![
            "kind=address_of".to_string(),
            "dst=pt:1".to_string(),
            "object=obj:2".to_string(),
        ];
        assert!(!has_required_type_value_alias_payload(&candidate));

        candidate.fact_family = TYPE_VALUE_ALIAS_TYPE_FAMILY.to_string();
        candidate.payload_labels = vec!["subject=place:1".to_string(), "shape=typo".to_string()];
        assert!(!has_required_type_value_alias_payload(&candidate));

        candidate.fact_family = TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY.to_string();
        candidate.payload_labels = vec![
            "left=garbage".to_string(),
            "right=place:1".to_string(),
            "status=no_alias".to_string(),
        ];
        assert!(!has_required_type_value_alias_payload(&candidate));
    }

    #[test]
    fn refined_call_payload_requires_site_target_algorithm_and_status() {
        let mut candidate = ExtensionFactCandidate {
            extension_id: "demo".to_string(),
            provider_id: "model".to_string(),
            fact_family: REFINED_CALL_EDGE_FAMILY.to_string(),
            stable_key: "refined:extension".to_string(),
            binding_refs: vec!["file:src/app.ts".to_string()],
            span: None,
            precision: Some(ExtensionFactPrecision::Heuristic),
            confidence: ExtensionFactConfidence::Medium,
            status: ExtensionFactStatus::Candidate,
            evidence: vec!["extension-evidence".to_string()],
            payload_labels: vec![
                "site=call_site:0".to_string(),
                "target_function=function:1".to_string(),
                "algorithm=repo_model".to_string(),
                "status=resolved".to_string(),
            ],
        };

        assert!(has_required_refined_call_payload(&candidate));

        candidate
            .payload_labels
            .retain(|label| !label.starts_with("site="));
        assert!(!has_required_refined_call_payload(&candidate));
    }
}
