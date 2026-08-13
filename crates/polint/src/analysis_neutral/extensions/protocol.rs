use serde::{Deserialize, Serialize};

use super::manifest::{ExtensionActivationStatus, FactFamilyLabel};

pub const HANDSHAKE_SCHEMA: &str = "polint-extension-handshake-v1";
pub const PROVIDER_RUN_SCHEMA: &str = "polint-extension-provider-run-v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ExtensionHandshakeRequest {
    pub schema_version: String,
    pub extension_id: String,
    pub manifest_path: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ExtensionHandshakeResponse {
    pub schema_version: String,
    pub extension_id: String,
    pub activation_status: ExtensionActivationStatus,
    pub providers: Vec<ExtensionProviderDeclaration>,
    pub diagnostics: Vec<ExtensionProtocolDiagnostic>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ExtensionProviderRunRequest {
    pub schema_version: String,
    pub extension_id: String,
    pub provider_id: String,
    pub declared_inputs: Vec<FactFamilyLabel>,
    pub declared_outputs: Vec<FactFamilyLabel>,
    pub input_digest_labels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ExtensionProviderRunResponse {
    pub schema_version: String,
    pub extension_id: String,
    pub provider_id: String,
    pub activation_status: ExtensionActivationStatus,
    pub diagnostics: Vec<ExtensionProtocolDiagnostic>,
    pub facts: Vec<ExtensionFactCandidateWire>,
    pub output_digest_inputs: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ExtensionProviderDeclaration {
    pub provider_id: String,
    pub declared_inputs: Vec<FactFamilyLabel>,
    pub declared_outputs: Vec<FactFamilyLabel>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ExtensionProtocolDiagnostic {
    pub rule_id: String,
    pub severity: String,
    pub message: String,
    pub evidence: Vec<ExtensionProtocolEvidence>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ExtensionProtocolEvidence {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ExtensionFactCandidateWire {
    pub fact_family: String,
    #[serde(rename = "stable_key")]
    pub stable_key_text: String,
    pub binding_refs: Vec<String>,
    #[serde(default)]
    pub span: Option<ExtensionSpanWire>,
    pub precision: String,
    pub confidence: String,
    pub evidence: Vec<String>,
    pub payload_labels: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ExtensionSpanWire {
    pub relative_path: String,
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionProtocolError {
    UnsupportedProtocol {
        expected: &'static str,
        actual: String,
    },
}

impl ExtensionHandshakeResponse {
    pub fn validate_schema(&self) -> Result<(), ExtensionProtocolError> {
        validate_schema_label(HANDSHAKE_SCHEMA, &self.schema_version)
    }
}

impl ExtensionProviderRunResponse {
    pub fn validate_schema(&self) -> Result<(), ExtensionProtocolError> {
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
