use serde::{Deserialize, Serialize};
use std::fmt;

use super::dependency_index::CacheNode;
use super::digest::Digest;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChangeKind {
    ContentOnly,
    SyntaxShape,
    ImportShape,
    PublicApiShape,
    ModuleTopology,
    Lifecycle,
    Toolchain,
    RuleCode,
    RuleOptions,
    ExtensionCode,
    ExtensionDeclaredInput,
    ModelFile,
    ProviderVersion,
    Unknown,
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "canonical change-kind errors are returned by private persistence readers"
    )
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UnknownChangeKindLabel {
    label: String,
}

impl fmt::Display for UnknownChangeKindLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "unknown change kind label `{}`", self.label)
    }
}

impl std::error::Error for UnknownChangeKindLabel {}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "canonical change-kind codecs are consumed by private persistence readers"
    )
)]
impl ChangeKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::ContentOnly => "content_only",
            Self::SyntaxShape => "syntax_shape",
            Self::ImportShape => "import_shape",
            Self::PublicApiShape => "public_api_shape",
            Self::ModuleTopology => "module_topology",
            Self::Lifecycle => "lifecycle",
            Self::Toolchain => "toolchain",
            Self::RuleCode => "rule_code",
            Self::RuleOptions => "rule_options",
            Self::ExtensionCode => "extension_code",
            Self::ExtensionDeclaredInput => "extension_declared_input",
            Self::ModelFile => "model_file",
            Self::ProviderVersion => "provider_version",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownChangeKindLabel> {
        match label {
            "content_only" => Ok(Self::ContentOnly),
            "syntax_shape" => Ok(Self::SyntaxShape),
            "import_shape" => Ok(Self::ImportShape),
            "public_api_shape" => Ok(Self::PublicApiShape),
            "module_topology" => Ok(Self::ModuleTopology),
            "lifecycle" => Ok(Self::Lifecycle),
            "toolchain" => Ok(Self::Toolchain),
            "rule_code" => Ok(Self::RuleCode),
            "rule_options" => Ok(Self::RuleOptions),
            "extension_code" => Ok(Self::ExtensionCode),
            "extension_declared_input" => Ok(Self::ExtensionDeclaredInput),
            "model_file" => Ok(Self::ModelFile),
            "provider_version" => Ok(Self::ProviderVersion),
            "unknown" => Ok(Self::Unknown),
            _ => Err(UnknownChangeKindLabel {
                label: label.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct ChangeSetRow {
    pub(crate) node: CacheNode,
    pub(crate) kind: ChangeKind,
    pub(crate) digest: Digest,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ChangeSetWire")]
pub(crate) struct ChangeSet {
    pub(crate) rows: Vec<ChangeSetRow>,
}

#[derive(Deserialize)]
struct ChangeSetWire {
    rows: Vec<ChangeSetRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ChangeSetDigestMismatch {
    stable_key: String,
}

impl fmt::Display for ChangeSetDigestMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "change-set digest does not match typed input `{}`",
            self.stable_key
        )
    }
}

impl std::error::Error for ChangeSetDigestMismatch {}

impl TryFrom<ChangeSetWire> for ChangeSet {
    type Error = ChangeSetDigestMismatch;

    fn try_from(wire: ChangeSetWire) -> Result<Self, Self::Error> {
        Self::try_from_rows(wire.rows)
    }
}

impl ChangeSet {
    pub(crate) fn from_rows(mut rows: Vec<ChangeSetRow>) -> Self {
        assert!(
            rows.iter().all(change_row_digest_matches_node),
            "change-set typed input digests must match their rows"
        );
        rows.sort();
        rows.dedup();
        Self { rows }
    }

    fn try_from_rows(mut rows: Vec<ChangeSetRow>) -> Result<Self, ChangeSetDigestMismatch> {
        if let Some(stable_key) = rows.iter().find_map(|row| match &row.node {
            CacheNode::DependencyInput(input) if input.digest != row.digest => {
                Some(input.stable_key.clone())
            }
            _ => None,
        }) {
            return Err(ChangeSetDigestMismatch { stable_key });
        }
        rows.sort();
        rows.dedup();
        Ok(Self { rows })
    }

    pub(crate) fn typed_input_digests_match(&self) -> bool {
        self.rows.iter().all(change_row_digest_matches_node)
    }

    pub(crate) fn rows(&self) -> &[ChangeSetRow] {
        &self.rows
    }
}

fn change_row_digest_matches_node(row: &ChangeSetRow) -> bool {
    match &row.node {
        CacheNode::DependencyInput(input) => input.digest == row.digest,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{
        DigestKind, InputComponentStatus, InputDependencyKey, LayerKey,
    };

    fn digest(label: &str) -> Digest {
        Digest::from_parts(DigestKind::SourceText, label, &[label])
    }

    fn layer() -> LayerKey {
        LayerKey::new(
            crate::analysis_kernel::incremental::keys::LayerKind::TsSyntax,
            "polint.ts.syntax",
            "1",
            "ts-facts-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::TsJsLifecycle, "none"),
            Digest::absent(DigestKind::AnalysisSettings, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            vec![digest("a")],
            Vec::new(),
            Vec::new(),
        )
    }

    fn source(stable_key: &str, digest: Digest) -> CacheNode {
        CacheNode::DependencyInput(
            InputDependencyKey::source_file(stable_key, digest, InputComponentStatus::Present)
                .expect("source dependency uses a source-text digest"),
        )
    }

    #[test]
    fn from_rows_sorts_and_deduplicates_changes() {
        let source_digest = digest("source");
        let source = source("src/a.ts", source_digest.clone());
        let layer = CacheNode::Layer(layer());
        let first = ChangeSetRow {
            node: layer,
            kind: ChangeKind::SyntaxShape,
            digest: digest("layer"),
        };
        let second = ChangeSetRow {
            node: source,
            kind: ChangeKind::ContentOnly,
            digest: source_digest,
        };

        let change_set = ChangeSet::from_rows(vec![first.clone(), second.clone(), first.clone()]);

        assert_eq!(change_set.rows(), &[second, first]);
    }

    #[test]
    fn change_kind_codecs_round_trip_every_exact_label() {
        for kind in [
            ChangeKind::ContentOnly,
            ChangeKind::SyntaxShape,
            ChangeKind::ImportShape,
            ChangeKind::PublicApiShape,
            ChangeKind::ModuleTopology,
            ChangeKind::Lifecycle,
            ChangeKind::Toolchain,
            ChangeKind::RuleCode,
            ChangeKind::RuleOptions,
            ChangeKind::ExtensionCode,
            ChangeKind::ExtensionDeclaredInput,
            ChangeKind::ModelFile,
            ChangeKind::ProviderVersion,
            ChangeKind::Unknown,
        ] {
            assert_eq!(ChangeKind::parse_label(kind.label()), Ok(kind));
        }
        assert!(ChangeKind::parse_label("content").is_err());
    }

    #[test]
    fn serde_rejects_a_change_row_with_a_mismatched_typed_input_digest() {
        let input_digest = digest("source");
        let wire = serde_json::json!({
            "rows": [{
                "node": source("src/a.ts", input_digest),
                "kind": "content_only",
                "digest": digest("different"),
            }]
        });

        let error = serde_json::from_value::<ChangeSet>(wire)
            .expect_err("mismatched change set must fail closed");

        assert!(error.to_string().contains("src/a.ts"));
    }
}
