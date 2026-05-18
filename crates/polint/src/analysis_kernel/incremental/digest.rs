use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct Digest {
    pub(crate) kind: DigestKind,
    pub(crate) value: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DigestKind {
    SourceText,
    Config,
    GoLifecycle,
    TsJsLifecycle,
    RuleCode,
    RuleOptions,
    ModelFile,
    ExtensionCode,
    ToolInvocation,
    ProviderParameters,
    ProviderOutput,
    LayerOutput,
    DependencyLayer,
    QueryParameters,
    Budget,
    Evidence,
    SummaryBody,
    SummaryDependency,
}

impl Digest {
    pub(crate) fn from_parts(kind: DigestKind, label: &str, parts: &[&str]) -> Self {
        let kind_label = kind.as_str();
        let mut encoded_parts = Vec::with_capacity(parts.len() + 1);
        encoded_parts.push(length_prefixed_part("kind", kind_label));
        encoded_parts.extend(parts.iter().map(|part| length_prefixed_part(label, part)));
        let hash_parts = encoded_parts.iter().map(String::as_str).collect::<Vec<_>>();

        Self {
            kind,
            value: crate::cache::stable_hash(&hash_parts),
        }
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "Future cache-key consumers will need unordered digest construction; Phase 23 verifies it in unit tests."
        )
    )]
    pub(crate) fn from_unordered(kind: DigestKind, label: &str, mut digests: Vec<Digest>) -> Self {
        digests.sort();
        let digest_parts = digests.iter().map(ToString::to_string).collect::<Vec<_>>();
        let hash_parts = digest_parts.iter().map(String::as_str).collect::<Vec<_>>();

        Self::from_parts(kind, label, &hash_parts)
    }

    pub(crate) fn absent(kind: DigestKind, label: &str) -> Self {
        Self::from_parts(kind, "absent", &[label])
    }

    pub(crate) fn unsupported(kind: DigestKind, label: &str, reason: &str) -> Self {
        Self::from_parts(kind, "unsupported", &[label, reason])
    }
}

impl DigestKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::SourceText => "source_text",
            Self::Config => "config",
            Self::GoLifecycle => "go_lifecycle",
            Self::TsJsLifecycle => "ts_js_lifecycle",
            Self::RuleCode => "rule_code",
            Self::RuleOptions => "rule_options",
            Self::ModelFile => "model_file",
            Self::ExtensionCode => "extension_code",
            Self::ToolInvocation => "tool_invocation",
            Self::ProviderParameters => "provider_parameters",
            Self::ProviderOutput => "provider_output",
            Self::LayerOutput => "layer_output",
            Self::DependencyLayer => "dependency_layer",
            Self::QueryParameters => "query_parameters",
            Self::Budget => "budget",
            Self::Evidence => "evidence",
            Self::SummaryBody => "summary_body",
            Self::SummaryDependency => "summary_dependency",
        }
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.kind.as_str(), self.value)
    }
}

fn length_prefixed_part(label: &str, value: &str) -> String {
    format!("{}:{}={}:{}", label.len(), label, value.len(), value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_parts_is_deterministic_and_kind_separated() {
        let first = Digest::from_parts(DigestKind::SourceText, "source_text", &["path", "hash"]);
        let second = Digest::from_parts(DigestKind::SourceText, "source_text", &["path", "hash"]);
        let config = Digest::from_parts(DigestKind::Config, "source_text", &["path", "hash"]);

        assert_eq!(first, second);
        assert_ne!(first, config);
    }

    #[test]
    fn from_unordered_sorts_input_digests_canonically() {
        let a = Digest::from_parts(DigestKind::SourceText, "file", &["a"]);
        let b = Digest::from_parts(DigestKind::SourceText, "file", &["b"]);

        assert_eq!(
            Digest::from_unordered(DigestKind::LayerOutput, "layer", vec![b.clone(), a.clone()]),
            Digest::from_unordered(DigestKind::LayerOutput, "layer", vec![a, b])
        );
    }

    #[test]
    fn serde_and_display_include_kind_and_value() {
        let digest = Digest::from_parts(DigestKind::SourceText, "source_text", &["path"]);
        let json = serde_json::to_string(&digest).expect("digest should serialize");

        assert!(json.contains("\"kind\""));
        assert!(json.contains("\"value\""));
        assert_eq!(digest.to_string(), format!("source_text:{}", digest.value));
    }

    #[test]
    fn absent_and_unsupported_helpers_are_explicit() {
        assert_ne!(
            Digest::absent(DigestKind::ToolInvocation, "go"),
            Digest::unsupported(DigestKind::ToolInvocation, "go", "not on path")
        );
    }
}
