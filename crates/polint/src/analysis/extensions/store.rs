use serde::{Deserialize, Serialize};

use super::manifest::ExtensionActivationStatus;
use super::sinks::{
    ExtensionFactCandidate, ExtensionFactConfidence, ExtensionFactPrecision, ExtensionFactStatus,
};
use super::validate::ExtensionRejectionReason;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExtensionOutput {
    pub(crate) activations: Vec<ExtensionActivationRow>,
    pub(crate) accepted: Vec<AcceptedExtensionFact>,
    pub(crate) rejected: Vec<RejectedExtensionFact>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExtensionActivationRow {
    pub(crate) extension_id: String,
    pub(crate) provider_id: Option<String>,
    pub(crate) status: ExtensionActivationStatus,
    pub(crate) diagnostic_count: usize,
    pub(crate) output_digest_inputs: Vec<String>,
    pub(crate) diagnostic_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AcceptedExtensionFact {
    pub(crate) extension_id: String,
    pub(crate) provider_id: String,
    pub(crate) fact_family: String,
    pub(crate) stable_key: String,
    pub(crate) binding_refs: Vec<String>,
    pub(crate) precision: ExtensionFactPrecision,
    pub(crate) confidence: ExtensionFactConfidence,
    pub(crate) status: ExtensionFactStatus,
    pub(crate) evidence: Vec<String>,
    pub(crate) payload_labels: Vec<String>,
    pub(crate) payload_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RejectedExtensionFact {
    pub(crate) extension_id: String,
    pub(crate) provider_id: String,
    pub(crate) fact_family: String,
    pub(crate) stable_key: String,
    pub(crate) reason: ExtensionRejectionReason,
    pub(crate) evidence: Vec<String>,
}

impl ExtensionOutput {
    pub(crate) fn normalized(mut self) -> Self {
        for activation in &mut self.activations {
            activation.output_digest_inputs.sort();
            activation.output_digest_inputs.dedup();
        }
        self.activations.sort_by(|left, right| {
            (
                left.extension_id.as_str(),
                left.provider_id.as_deref().unwrap_or(""),
                left.status,
            )
                .cmp(&(
                    right.extension_id.as_str(),
                    right.provider_id.as_deref().unwrap_or(""),
                    right.status,
                ))
        });
        self.accepted.sort_by(|left, right| {
            (
                left.extension_id.as_str(),
                left.provider_id.as_str(),
                left.fact_family.as_str(),
                left.stable_key.as_str(),
            )
                .cmp(&(
                    right.extension_id.as_str(),
                    right.provider_id.as_str(),
                    right.fact_family.as_str(),
                    right.stable_key.as_str(),
                ))
        });
        self.rejected.sort_by(|left, right| {
            (
                left.extension_id.as_str(),
                left.provider_id.as_str(),
                left.fact_family.as_str(),
                left.stable_key.as_str(),
                left.reason,
            )
                .cmp(&(
                    right.extension_id.as_str(),
                    right.provider_id.as_str(),
                    right.fact_family.as_str(),
                    right.stable_key.as_str(),
                    right.reason,
                ))
        });
        self
    }
}

impl AcceptedExtensionFact {
    pub(crate) fn from_candidate(candidate: ExtensionFactCandidate) -> Self {
        let candidate = candidate.normalized();
        let payload_digest = extension_payload_digest(&candidate);
        Self {
            extension_id: candidate.extension_id,
            provider_id: candidate.provider_id,
            fact_family: candidate.fact_family,
            stable_key: candidate.stable_key,
            binding_refs: candidate.binding_refs,
            precision: candidate.precision.expect("accepted fact has precision"),
            confidence: candidate.confidence,
            status: ExtensionFactStatus::Accepted,
            evidence: candidate.evidence,
            payload_labels: candidate.payload_labels,
            payload_digest,
        }
    }
}

impl RejectedExtensionFact {
    pub(crate) fn from_candidate(
        candidate: &ExtensionFactCandidate,
        reason: ExtensionRejectionReason,
    ) -> Self {
        let candidate = candidate.clone().normalized();
        Self {
            extension_id: candidate.extension_id,
            provider_id: candidate.provider_id,
            fact_family: candidate.fact_family,
            stable_key: candidate.stable_key,
            reason,
            evidence: candidate.evidence,
        }
    }
}

fn extension_payload_digest(candidate: &ExtensionFactCandidate) -> String {
    let mut parts = vec![
        format!("extension_id={}", candidate.extension_id),
        format!("provider_id={}", candidate.provider_id),
        format!("family={}", candidate.fact_family),
        format!("stable_key={}", candidate.stable_key),
        format!("precision={:?}", candidate.precision),
        format!("confidence={:?}", candidate.confidence),
    ];
    parts.extend(
        candidate
            .binding_refs
            .iter()
            .map(|binding| format!("binding={binding}")),
    );
    parts.extend(
        candidate
            .evidence
            .iter()
            .map(|evidence| format!("evidence={evidence}")),
    );
    parts.extend(
        candidate
            .payload_labels
            .iter()
            .map(|payload| format!("payload={payload}")),
    );
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    crate::cache::stable_hash(&refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_sorts_accepted_and_rejected_rows_deterministically() {
        let output = ExtensionOutput {
            activations: Vec::new(),
            accepted: vec![
                accepted("demo", "z", "family", "b"),
                accepted("demo", "a", "family", "a"),
            ],
            rejected: vec![
                rejected(
                    "demo",
                    "z",
                    "family",
                    "b",
                    ExtensionRejectionReason::MissingPrecision,
                ),
                rejected(
                    "demo",
                    "a",
                    "family",
                    "a",
                    ExtensionRejectionReason::UndeclaredOutput,
                ),
            ],
        }
        .normalized();

        assert_eq!(output.accepted[0].provider_id, "a");
        assert_eq!(
            output.rejected[0].reason,
            ExtensionRejectionReason::UndeclaredOutput
        );
    }

    fn accepted(
        extension_id: &str,
        provider_id: &str,
        fact_family: &str,
        stable_key: &str,
    ) -> AcceptedExtensionFact {
        AcceptedExtensionFact {
            extension_id: extension_id.to_string(),
            provider_id: provider_id.to_string(),
            fact_family: fact_family.to_string(),
            stable_key: stable_key.to_string(),
            binding_refs: Vec::new(),
            precision: ExtensionFactPrecision::Heuristic,
            confidence: ExtensionFactConfidence::Medium,
            status: ExtensionFactStatus::Accepted,
            evidence: Vec::new(),
            payload_labels: Vec::new(),
            payload_digest: stable_key.to_string(),
        }
    }

    fn rejected(
        extension_id: &str,
        provider_id: &str,
        fact_family: &str,
        stable_key: &str,
        reason: ExtensionRejectionReason,
    ) -> RejectedExtensionFact {
        RejectedExtensionFact {
            extension_id: extension_id.to_string(),
            provider_id: provider_id.to_string(),
            fact_family: fact_family.to_string(),
            stable_key: stable_key.to_string(),
            reason,
            evidence: Vec::new(),
        }
    }
}
