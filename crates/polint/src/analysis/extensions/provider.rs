use std::collections::BTreeSet;

use super::discovery::discover_local_extensions;
use super::host::{ExtensionHost, ExtensionHostError};
use super::manifest::ExtensionActivationStatus;
use super::protocol::ExtensionProtocolDiagnostic;
use super::sinks::{
    ExtensionFactCandidate, ExtensionFactConfidence, ExtensionFactPrecision, ExtensionFactStatus,
    ExtensionSpanRef,
};
use super::store::{AcceptedExtensionFact, ExtensionActivationRow, ExtensionOutput};
use super::validate::{ExtensionValidationInput, validate_extension_output};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{CacheStats, Digest, DigestKind, InputSnapshot};
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, Severity, TextRange};

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
    let mut accepted_stable_keys = BTreeSet::<crate::core::StableKeyId>::new();

    for extension in discovered {
        let handshake = match host.handshake(&extension) {
            Ok(response) => {
                diagnostics.extend(response.diagnostics.iter().map(|diagnostic| {
                    protocol_diagnostic(&extension.extension_id, None, diagnostic)
                }));
                activations.push(ExtensionActivationRow {
                    extension_id: extension.extension_id.clone(),
                    provider_id: None,
                    status: response.activation_status,
                    diagnostic_count: response.diagnostics.len(),
                    output_digest_inputs: Vec::new(),
                    diagnostic_digest: protocol_diagnostic_digest(&response.diagnostics),
                });
                if response.activation_status != ExtensionActivationStatus::HandshakeOk {
                    continue;
                }
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
                    diagnostics.extend(response.diagnostics.iter().map(|diagnostic| {
                        protocol_diagnostic(
                            &response.extension_id,
                            Some(&response.provider_id),
                            diagnostic,
                        )
                    }));
                    let diagnostic_digest = protocol_diagnostic_digest(&response.diagnostics);
                    let output_digest_inputs = response.output_digest_inputs.clone();
                    let validation =
                        if response.activation_status == ExtensionActivationStatus::Active {
                            let candidates = response
                                .facts
                                .into_iter()
                                .map(|fact| {
                                    extension_candidate_from_wire(
                                        db,
                                        &response.extension_id,
                                        &response.provider_id,
                                        fact,
                                    )
                                })
                                .collect::<Vec<_>>();
                            validate_extension_output(
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
                            )
                        } else {
                            ExtensionOutput::default()
                        };
                    let mut provider_rejected = validation.rejected;
                    let provider_accepted = deduplicate_provider_accepted(
                        validation.accepted,
                        &mut accepted_stable_keys,
                        &mut provider_rejected,
                    );
                    diagnostics.extend(
                        provider_rejected
                            .iter()
                            .map(|fact| extension_rejection_diagnostic(db, fact)),
                    );
                    activations.push(ExtensionActivationRow {
                        extension_id: response.extension_id,
                        provider_id: Some(response.provider_id),
                        status: if response.activation_status != ExtensionActivationStatus::Active {
                            response.activation_status
                        } else if provider_rejected.is_empty() {
                            ExtensionActivationStatus::Active
                        } else {
                            ExtensionActivationStatus::ValidationFailed
                        },
                        diagnostic_count: response.diagnostics.len(),
                        output_digest_inputs,
                        diagnostic_digest,
                    });
                    accepted.extend(provider_accepted);
                    rejected.extend(provider_rejected);
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
            db.resolve_stable_key(fact.stable_key),
            fact.payload_digest
        )
    }));
    parts.extend(db.rejected_extension_facts().iter().map(|fact| {
        let mut evidence = fact
            .evidence
            .iter()
            .map(|evidence| format!("evidence={evidence}"))
            .collect::<Vec<_>>();
        evidence.sort();
        let evidence_refs = evidence.iter().map(String::as_str).collect::<Vec<_>>();
        let evidence_digest = crate::cache::stable_hash(&evidence_refs);
        format!(
            "rejected={}:{}:{}:{}:{:?}:{}",
            fact.extension_id,
            fact.provider_id,
            fact.fact_family,
            db.resolve_stable_key(fact.stable_key),
            fact.reason,
            evidence_digest
        )
    }));
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "extension_output", &refs)
}

fn deduplicate_provider_accepted(
    accepted: Vec<AcceptedExtensionFact>,
    accepted_stable_keys: &mut BTreeSet<crate::core::StableKeyId>,
    rejected: &mut Vec<super::store::RejectedExtensionFact>,
) -> Vec<AcceptedExtensionFact> {
    let mut deduped = Vec::new();
    for fact in accepted {
        if accepted_stable_keys.insert(fact.stable_key) {
            deduped.push(fact);
        } else {
            rejected.push(super::store::RejectedExtensionFact {
                extension_id: fact.extension_id,
                provider_id: fact.provider_id,
                fact_family: fact.fact_family,
                stable_key: fact.stable_key,
                reason: super::validate::ExtensionRejectionReason::DuplicateStableKey,
                evidence: fact.evidence,
            });
        }
    }
    deduped
}

fn protocol_diagnostic(
    extension_id: &str,
    provider_id: Option<&str>,
    diagnostic: &ExtensionProtocolDiagnostic,
) -> Diagnostic {
    let severity = match diagnostic.severity.as_str() {
        "error" => Severity::Error,
        "info" => Severity::Info,
        _ => Severity::Warn,
    };
    let mut output = Diagnostic::new(
        diagnostic.rule_id.clone(),
        severity,
        "<workspace>",
        TextRange::point(1, 1),
        diagnostic.message.clone(),
    )
    .with_evidence("extension_id", extension_id.to_string());
    if let Some(provider_id) = provider_id {
        output = output.with_evidence("provider_id", provider_id.to_string());
    }
    for evidence in &diagnostic.evidence {
        output = output.with_evidence(evidence.label.clone(), evidence.value.clone());
    }
    output
}

fn extension_rejection_diagnostic(
    db: &AnalysisDb,
    fact: &super::store::RejectedExtensionFact,
) -> Diagnostic {
    Diagnostic::error(
        "polint/extension",
        "<workspace>",
        TextRange::point(1, 1),
        "Extension fact failed validation.",
    )
    .with_evidence("extension_id", fact.extension_id.clone())
    .with_evidence("provider_id", fact.provider_id.clone())
    .with_evidence("fact_family", fact.fact_family.clone())
    .with_evidence(
        "stable_key",
        db.resolve_stable_key(fact.stable_key).to_string(),
    )
    .with_evidence("reason", format!("{:?}", fact.reason))
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
        .map(|(_reference, metadata)| db.resolve_stable_key(metadata.stable_key).to_string())
        .collect()
}

fn extension_candidate_from_wire(
    db: &AnalysisDb,
    extension_id: &str,
    provider_id: &str,
    fact: super::protocol::ExtensionFactCandidateWire,
) -> ExtensionFactCandidate {
    let span = fact.span.map(|span| ExtensionSpanRef {
        relative_path: span.relative_path,
        start_byte: span.start_byte,
        end_byte: span.end_byte,
    });

    ExtensionFactCandidate {
        extension_id: extension_id.to_string(),
        provider_id: provider_id.to_string(),
        fact_family: fact.fact_family,
        stable_key: db.stable_key_interner().intern(fact.stable_key_text),
        binding_refs: fact.binding_refs,
        span,
        precision: parse_precision(&fact.precision),
        confidence: parse_confidence(&fact.confidence),
        status: ExtensionFactStatus::Candidate,
        evidence: fact.evidence,
        payload_labels: fact.payload_labels,
    }
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
        ExtensionFactCandidateWire, ExtensionProtocolDiagnostic, ExtensionProtocolEvidence,
        ExtensionSpanWire,
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
                stable_key: crate::core::stable_key_for_test("route:/a"),
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

    #[test]
    fn wire_fact_span_is_preserved_for_sink_validation() {
        let candidate = extension_candidate_from_wire(
            &crate::core::AnalysisDb::new(),
            "demo",
            "routes",
            ExtensionFactCandidateWire {
                fact_family: "extension.routes".to_string(),
                stable_key_text: "route:/a".to_string(),
                binding_refs: Vec::new(),
                span: Some(ExtensionSpanWire {
                    relative_path: "src/app.ts".to_string(),
                    start_byte: 10,
                    end_byte: 20,
                }),
                precision: "heuristic".to_string(),
                confidence: "high".to_string(),
                evidence: vec!["route literal".to_string()],
                payload_labels: vec!["method=GET".to_string()],
            },
        );

        assert_eq!(
            candidate.span,
            Some(ExtensionSpanRef {
                relative_path: "src/app.ts".to_string(),
                start_byte: 10,
                end_byte: 20,
            })
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
