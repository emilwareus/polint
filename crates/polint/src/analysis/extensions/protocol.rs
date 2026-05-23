use serde::{Deserialize, Serialize};

use super::manifest::{ExtensionActivationStatus, FactFamilyLabel};

pub(crate) const HANDSHAKE_SCHEMA: &str = "polint-extension-handshake-v1";
pub(crate) const PROVIDER_RUN_SCHEMA: &str = "polint-extension-provider-run-v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExtensionHandshakeRequest {
    pub(crate) schema_version: String,
    pub(crate) extension_id: String,
    pub(crate) manifest_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExtensionHandshakeResponse {
    pub(crate) schema_version: String,
    pub(crate) extension_id: String,
    pub(crate) activation_status: ExtensionActivationStatus,
    pub(crate) providers: Vec<ExtensionProviderDeclaration>,
    pub(crate) diagnostics: Vec<ExtensionProtocolDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExtensionProviderRunRequest {
    pub(crate) schema_version: String,
    pub(crate) extension_id: String,
    pub(crate) provider_id: String,
    pub(crate) declared_inputs: Vec<FactFamilyLabel>,
    pub(crate) declared_outputs: Vec<FactFamilyLabel>,
    pub(crate) input_digest_labels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExtensionProviderRunResponse {
    pub(crate) schema_version: String,
    pub(crate) extension_id: String,
    pub(crate) provider_id: String,
    pub(crate) activation_status: ExtensionActivationStatus,
    pub(crate) diagnostics: Vec<ExtensionProtocolDiagnostic>,
    pub(crate) facts: Vec<ExtensionFactCandidateWire>,
    pub(crate) output_digest_inputs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExtensionProviderDeclaration {
    pub(crate) provider_id: String,
    pub(crate) declared_inputs: Vec<FactFamilyLabel>,
    pub(crate) declared_outputs: Vec<FactFamilyLabel>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExtensionProtocolDiagnostic {
    pub(crate) rule_id: String,
    pub(crate) severity: String,
    pub(crate) message: String,
    pub(crate) evidence: Vec<ExtensionProtocolEvidence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExtensionProtocolEvidence {
    pub(crate) label: String,
    pub(crate) value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExtensionFactCandidateWire {
    pub(crate) fact_family: String,
    pub(crate) stable_key: String,
    pub(crate) binding_refs: Vec<String>,
    pub(crate) precision: String,
    pub(crate) confidence: String,
    pub(crate) evidence: Vec<String>,
    pub(crate) payload_labels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ExtensionProtocolError {
    UnsupportedProtocol {
        expected: &'static str,
        actual: String,
    },
}

impl ExtensionHandshakeResponse {
    pub(crate) fn validate_schema(&self) -> Result<(), ExtensionProtocolError> {
        validate_schema_label(HANDSHAKE_SCHEMA, &self.schema_version)
    }
}

impl ExtensionProviderRunResponse {
    pub(crate) fn validate_schema(&self) -> Result<(), ExtensionProtocolError> {
        validate_schema_label(PROVIDER_RUN_SCHEMA, &self.schema_version)
    }
}

fn validate_schema_label(
    expected: &'static str,
    actual: &str,
) -> Result<(), ExtensionProtocolError> {
    if actual == expected {
        Ok(())
    } else {
        Err(ExtensionProtocolError::UnsupportedProtocol {
            expected,
            actual: actual.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_response_rejects_unknown_json_fields() {
        let json = r#"{
            "schema_version": "polint-extension-handshake-v1",
            "extension_id": "demo",
            "activation_status": "handshake_ok",
            "providers": [],
            "diagnostics": [],
            "extra": true
        }"#;

        assert!(serde_json::from_str::<ExtensionHandshakeResponse>(json).is_err());
    }

    #[test]
    fn provider_response_rejects_unknown_json_fields() {
        let json = r#"{
            "schema_version": "polint-extension-provider-run-v1",
            "extension_id": "demo",
            "provider_id": "routes",
            "activation_status": "active",
            "diagnostics": [],
            "facts": [],
            "output_digest_inputs": [],
            "extra": true
        }"#;

        assert!(serde_json::from_str::<ExtensionProviderRunResponse>(json).is_err());
    }

    #[test]
    fn unsupported_protocol_schema_is_deterministic_error() {
        let response = ExtensionHandshakeResponse {
            schema_version: "future".to_string(),
            extension_id: "demo".to_string(),
            activation_status: ExtensionActivationStatus::HandshakeOk,
            providers: Vec::new(),
            diagnostics: Vec::new(),
        };

        assert_eq!(
            response.validate_schema().unwrap_err(),
            ExtensionProtocolError::UnsupportedProtocol {
                expected: HANDSHAKE_SCHEMA,
                actual: "future".to_string()
            }
        );
    }
}
