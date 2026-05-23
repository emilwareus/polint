use serde::{Deserialize, Serialize};

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
}
