use std::collections::BTreeSet;

use super::discovery::discover_local_extensions;
use super::host::{ExtensionHost, ExtensionHostError};
use super::manifest::ExtensionActivationStatus;
use super::protocol::ExtensionProtocolDiagnostic;
use super::sinks::{
    ExtensionFactCandidate, ExtensionFactConfidence, ExtensionFactPrecision, ExtensionFactStatus,
};
use super::store::{ExtensionActivationRow, ExtensionOutput};
use super::validate::{ExtensionValidationInput, validate_extension_output};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{CacheStats, Digest, DigestKind, InputSnapshot};
use crate::core::AnalysisDb;
use crate::diagnostics::Diagnostic;

#[derive(Debug, Clone, Default)]
pub(crate) struct ExtensionProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

pub(crate) fn derive_extension_provider_outputs_with_cache_stats(
    db: &mut AnalysisDb,
    repo_root: &std::path::Path,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
) -> ExtensionProviderOutput {
    let discovered = discover_local_extensions(repo_root);
    if discovered.is_empty() {
        db.replace_extension_facts(ExtensionOutput::default());
        let mut cache_stats = CacheStats::default();
        cache_stats.record_recompute();
        return ExtensionProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(extension_output_digest(manifest, input_snapshot, db)),
        };
    }

    let host = ExtensionHost::new(repo_root);
    let mut diagnostics = Vec::new();
    let mut activations = Vec::new();
    let mut accepted = Vec::new();
    let mut rejected = Vec::new();

    for extension in discovered {
        let handshake = match host.handshake(&extension) {
            Ok(response) => {
                activations.push(ExtensionActivationRow {
                    extension_id: extension.extension_id.clone(),
                    provider_id: None,
                    status: response.activation_status,
                    diagnostic_count: response.diagnostics.len(),
                    output_digest_inputs: Vec::new(),
                    diagnostic_digest: protocol_diagnostic_digest(&response.diagnostics),
                });
                response
            }
            Err(error) => {
                diagnostics.push(error.diagnostic());
                activations.push(ExtensionActivationRow {
                    extension_id: extension.extension_id.clone(),
                    provider_id: None,
                    status: error.activation_status(),
                    diagnostic_count: 1,
                    output_digest_inputs: Vec::new(),
                    diagnostic_digest: host_error_digest(&error),
                });
                continue;
            }
        };

        for provider in handshake.providers {
            let provider_id = provider.provider_id.clone();
            let run = host.run_provider(
                &extension,
                &provider_id,
                provider.declared_inputs.clone(),
                provider.declared_outputs.clone(),
                extension
                    .digest_input_paths
                    .iter()
                    .map(|path| format!("extension_input={path}"))
                    .collect(),
            );
            match run {
                Ok(response) => {
                    let diagnostic_digest = protocol_diagnostic_digest(&response.diagnostics);
                    let output_digest_inputs = response.output_digest_inputs.clone();
                    let candidates = response
                        .facts
                        .into_iter()
                        .map(|fact| ExtensionFactCandidate {
                            extension_id: response.extension_id.clone(),
                            provider_id: response.provider_id.clone(),
                            fact_family: fact.fact_family,
                            stable_key: fact.stable_key,
                            binding_refs: fact.binding_refs,
                            span: None,
                            precision: parse_precision(&fact.precision),
                            confidence: parse_confidence(&fact.confidence),
                            status: ExtensionFactStatus::Candidate,
                            evidence: fact.evidence,
                            payload_labels: fact.payload_labels,
                        })
                        .collect::<Vec<_>>();
                    let validation = validate_extension_output(
                        db,
                        ExtensionValidationInput {
                            declared_outputs: provider
                                .declared_outputs
                                .iter()
                                .map(|label| label.as_str().to_string())
                                .collect::<BTreeSet<_>>(),
                            native_stable_keys: native_stable_keys(db),
                            exact_validation_evidence: BTreeSet::new(),
                            candidates,
                        },
                    );
                    activations.push(ExtensionActivationRow {
                        extension_id: response.extension_id,
                        provider_id: Some(response.provider_id),
                        status: if validation.rejected.is_empty() {
                            ExtensionActivationStatus::Active
                        } else {
                            ExtensionActivationStatus::ValidationFailed
                        },
                        diagnostic_count: response.diagnostics.len(),
                        output_digest_inputs,
                        diagnostic_digest,
                    });
                    accepted.extend(validation.accepted);
                    rejected.extend(validation.rejected);
                }
                Err(error) => {
                    diagnostics.push(error.diagnostic());
                    activations.push(ExtensionActivationRow {
                        extension_id: extension.extension_id.clone(),
                        provider_id: Some(provider_id),
                        status: error.activation_status(),
                        diagnostic_count: 1,
                        output_digest_inputs: Vec::new(),
                        diagnostic_digest: host_error_digest(&error),
                    });
                }
            }
        }
    }

    db.replace_extension_facts(ExtensionOutput {
        activations,
        accepted,
        rejected,
    });
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();
    ExtensionProviderOutput {
        diagnostics,
        cache_stats,
        output_digest: Some(extension_output_digest(manifest, input_snapshot, db)),
    }
}

fn extension_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    db: &AnalysisDb,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("schema={}", manifest.primary_schema_label()),
    ];
    parts.extend(input_snapshot.extensions.iter().map(|component| {
        format!(
            "extension_input={}:{}:{:?}",
            component.name, component.digest, component.status
        )
    }));
    for row in db.extension_activations() {
        let provider_id = row.provider_id.as_deref().unwrap_or("<handshake>");
        parts.push(format!(
            "activation={}:{}:{:?}:{}:{}",
            row.extension_id, provider_id, row.status, row.diagnostic_count, row.diagnostic_digest
        ));
        parts.extend(row.output_digest_inputs.iter().map(|input| {
            format!(
                "activation_output_input={}:{}:{}",
                row.extension_id, provider_id, input
            )
        }));
    }
    parts.extend(db.extension_facts().iter().map(|fact| {
        format!(
            "accepted={}:{}:{}:{}:{}",
            fact.extension_id,
            fact.provider_id,
            fact.fact_family,
            fact.stable_key,
            fact.payload_digest
        )
    }));
    parts.extend(db.rejected_extension_facts().iter().map(|fact| {
        format!(
            "rejected={}:{}:{}:{:?}",
            fact.extension_id, fact.provider_id, fact.stable_key, fact.reason
        )
    }));
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "extension_output", &refs)
}

fn protocol_diagnostic_digest(diagnostics: &[ExtensionProtocolDiagnostic]) -> String {
    let mut parts = vec![format!("diagnostic_count={}", diagnostics.len())];
    for diagnostic in diagnostics {
        let mut evidence = diagnostic
            .evidence
            .iter()
            .map(|evidence| format!("evidence={}:{}", evidence.label, evidence.value))
            .collect::<Vec<_>>();
        evidence.sort();
        let evidence_refs = evidence.iter().map(String::as_str).collect::<Vec<_>>();
        let evidence_digest = crate::cache::stable_hash(&evidence_refs);
        parts.push(format!(
            "diagnostic={}:{}:{}:{}",
            diagnostic.rule_id, diagnostic.severity, diagnostic.message, evidence_digest
        ));
    }
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    crate::cache::stable_hash(&refs)
}

fn host_error_digest(error: &ExtensionHostError) -> String {
    let parts = [
        format!("kind={}", error.kind.as_str()),
        format!("summary={}", error.summary),
    ];
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    crate::cache::stable_hash(&refs)
}

fn native_stable_keys(db: &AnalysisDb) -> BTreeSet<String> {
    db.fact_meta()
        .rows()
        .filter(|(_reference, metadata)| !metadata.producer_id.starts_with("polint.extension."))
        .map(|(_reference, metadata)| metadata.stable_key.clone())
        .collect()
}

fn parse_precision(value: &str) -> Option<ExtensionFactPrecision> {
    match value {
        "exact" => Some(ExtensionFactPrecision::Exact),
        "setup_aware" => Some(ExtensionFactPrecision::SetupAware),
        "heuristic" => Some(ExtensionFactPrecision::Heuristic),
        "generated_unvalidated" => Some(ExtensionFactPrecision::GeneratedUnvalidated),
        _ => None,
    }
}

fn parse_confidence(value: &str) -> ExtensionFactConfidence {
    match value {
        "high" => ExtensionFactConfidence::High,
        "low" => ExtensionFactConfidence::Low,
        _ => ExtensionFactConfidence::Medium,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::extensions::protocol::{
        ExtensionProtocolDiagnostic, ExtensionProtocolEvidence,
    };
    use crate::analysis::extensions::sinks::ExtensionFactStatus;
    use crate::analysis::extensions::store::AcceptedExtensionFact;
    use crate::analysis_kernel::incremental::InputSnapshot;

    #[test]
    fn extension_output_digest_covers_accepted_and_rejected_rows() {
        let mut db = AnalysisDb::new();
        db.replace_extension_facts(ExtensionOutput {
            activations: vec![ExtensionActivationRow {
                extension_id: "demo".to_string(),
                provider_id: Some("routes".to_string()),
                status: ExtensionActivationStatus::Active,
                diagnostic_count: 0,
                output_digest_inputs: Vec::new(),
                diagnostic_digest: "empty".to_string(),
            }],
            accepted: vec![AcceptedExtensionFact {
                extension_id: "demo".to_string(),
                provider_id: "routes".to_string(),
                fact_family: "extension.routes".to_string(),
                stable_key: "route:/a".to_string(),
                binding_refs: Vec::new(),
                precision: ExtensionFactPrecision::Heuristic,
                confidence: ExtensionFactConfidence::Medium,
                status: ExtensionFactStatus::Accepted,
                evidence: vec!["fixture".to_string()],
                payload_labels: Vec::new(),
                payload_digest: "payload".to_string(),
            }],
            rejected: Vec::new(),
        });
        let snapshot = InputSnapshot {
            schema_version: "test".to_string(),
            files: Vec::new(),
            config: crate::analysis_kernel::incremental::InputComponent {
                name: "config".to_string(),
                status: crate::analysis_kernel::incremental::InputComponentStatus::Present,
                digest: Digest::absent(DigestKind::Config, "test"),
                detail: Vec::new(),
            },
            go_lifecycle: crate::analysis_kernel::incremental::GoLifecycleSnapshot {
                components: Vec::new(),
            },
            ts_js_lifecycle: crate::analysis_kernel::incremental::TsJsLifecycleSnapshot {
                components: Vec::new(),
            },
            rules: Vec::new(),
            models: Vec::new(),
            extensions: Vec::new(),
            tool_invocations: Vec::new(),
            provider_schemas: Vec::new(),
        };
        let digest = extension_output_digest(
            crate::analysis_kernel::AnalysisKernel::provider_manifests()
                .iter()
                .find(|manifest| manifest.id == "polint.extensions")
                .expect("extension provider manifest exists"),
            &snapshot,
            &db,
        );

        assert_eq!(digest.kind, DigestKind::ProviderOutput);
    }

    #[test]
    fn extension_output_digest_changes_for_provider_digest_inputs_and_diagnostics() {
        let snapshot = empty_snapshot();
        let manifest = crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.extensions")
            .expect("extension provider manifest exists");

        let mut first = AnalysisDb::new();
        first.replace_extension_facts(ExtensionOutput {
            activations: vec![activation_row(vec!["output=one".to_string()], "diag:a")],
            accepted: Vec::new(),
            rejected: Vec::new(),
        });
        let mut second = AnalysisDb::new();
        second.replace_extension_facts(ExtensionOutput {
            activations: vec![activation_row(vec!["output=two".to_string()], "diag:a")],
            accepted: Vec::new(),
            rejected: Vec::new(),
        });
        let mut third = AnalysisDb::new();
        third.replace_extension_facts(ExtensionOutput {
            activations: vec![activation_row(vec!["output=one".to_string()], "diag:b")],
            accepted: Vec::new(),
            rejected: Vec::new(),
        });

        let first_digest = extension_output_digest(manifest, &snapshot, &first);

        assert_ne!(
            first_digest,
            extension_output_digest(manifest, &snapshot, &second)
        );
        assert_ne!(
            first_digest,
            extension_output_digest(manifest, &snapshot, &third)
        );
    }

    #[test]
    fn protocol_diagnostic_digest_preserves_field_association() {
        let first = vec![
            protocol_diagnostic("extension/a", "message one"),
            protocol_diagnostic("extension/b", "message two"),
        ];
        let second = vec![
            protocol_diagnostic("extension/a", "message two"),
            protocol_diagnostic("extension/b", "message one"),
        ];

        assert_ne!(
            protocol_diagnostic_digest(&first),
            protocol_diagnostic_digest(&second)
        );
    }

    fn activation_row(
        output_digest_inputs: Vec<String>,
        diagnostic_digest: &str,
    ) -> ExtensionActivationRow {
        ExtensionActivationRow {
            extension_id: "demo".to_string(),
            provider_id: Some("routes".to_string()),
            status: ExtensionActivationStatus::Active,
            diagnostic_count: 0,
            output_digest_inputs,
            diagnostic_digest: diagnostic_digest.to_string(),
        }
    }

    fn protocol_diagnostic(rule_id: &str, message: &str) -> ExtensionProtocolDiagnostic {
        ExtensionProtocolDiagnostic {
            rule_id: rule_id.to_string(),
            severity: "warning".to_string(),
            message: message.to_string(),
            evidence: vec![ExtensionProtocolEvidence {
                label: "fixture".to_string(),
                value: "value".to_string(),
            }],
        }
    }

    fn empty_snapshot() -> InputSnapshot {
        InputSnapshot {
            schema_version: "test".to_string(),
            files: Vec::new(),
            config: crate::analysis_kernel::incremental::InputComponent {
                name: "config".to_string(),
                status: crate::analysis_kernel::incremental::InputComponentStatus::Present,
                digest: Digest::absent(DigestKind::Config, "test"),
                detail: Vec::new(),
            },
            go_lifecycle: crate::analysis_kernel::incremental::GoLifecycleSnapshot {
                components: Vec::new(),
            },
            ts_js_lifecycle: crate::analysis_kernel::incremental::TsJsLifecycleSnapshot {
                components: Vec::new(),
            },
            rules: Vec::new(),
            models: Vec::new(),
            extensions: Vec::new(),
            tool_invocations: Vec::new(),
            provider_schemas: Vec::new(),
        }
    }
}
