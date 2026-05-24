use serde::{Deserialize, Serialize};

pub(crate) const TYPE_VALUE_ALIAS_TYPE_FAMILY: &str = "type_value_alias.type";
pub(crate) const TYPE_VALUE_ALIAS_VALUE_FAMILY: &str = "type_value_alias.value";
pub(crate) const TYPE_VALUE_ALIAS_ALLOCATION_FAMILY: &str = "type_value_alias.allocation_token";
pub(crate) const TYPE_VALUE_ALIAS_ACCESS_PATH_FAMILY: &str = "type_value_alias.access_path";
pub(crate) const TYPE_VALUE_ALIAS_POINTS_TO_CONSTRAINT_FAMILY: &str =
    "type_value_alias.points_to_constraint";
pub(crate) const TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY: &str = "type_value_alias.alias_answer";

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
            has_payload_label(candidate, "subject=") && has_payload_label(candidate, "shape=")
        }
        TYPE_VALUE_ALIAS_VALUE_FAMILY => {
            has_payload_label(candidate, "subject=") && has_payload_label(candidate, "kind=")
        }
        TYPE_VALUE_ALIAS_ALLOCATION_FAMILY => has_payload_label(candidate, "kind="),
        TYPE_VALUE_ALIAS_ACCESS_PATH_FAMILY => {
            has_payload_label(candidate, "base=place:")
                && candidate
                    .payload_labels
                    .iter()
                    .any(|label| label == "projection=base" || label.starts_with("projection="))
        }
        TYPE_VALUE_ALIAS_POINTS_TO_CONSTRAINT_FAMILY => {
            has_payload_label(candidate, "kind=") && has_payload_label(candidate, "dst=pt:")
        }
        TYPE_VALUE_ALIAS_ALIAS_ANSWER_FAMILY => {
            has_payload_label(candidate, "left=")
                && has_payload_label(candidate, "right=")
                && has_payload_label(candidate, "status=")
        }
        _ => true,
    }
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
}
