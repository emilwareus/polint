use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct ChangeSetRow {
    pub(crate) node: CacheNode,
    pub(crate) kind: ChangeKind,
    pub(crate) digest: Digest,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChangeSet {
    pub(crate) rows: Vec<ChangeSetRow>,
}

impl ChangeSet {
    pub(crate) fn from_rows(mut rows: Vec<ChangeSetRow>) -> Self {
        rows.sort();
        rows.dedup();
        Self { rows }
    }

    pub(crate) fn rows(&self) -> &[ChangeSetRow] {
        &self.rows
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{DigestKind, LayerKey};

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
            Digest::absent(DigestKind::Config, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            vec![digest("a")],
            Vec::new(),
            Vec::new(),
        )
    }

    #[test]
    fn from_rows_sorts_and_deduplicates_changes() {
        let source = CacheNode::Input("src/a.ts".to_string());
        let layer = CacheNode::Layer(layer());
        let first = ChangeSetRow {
            node: layer,
            kind: ChangeKind::SyntaxShape,
            digest: digest("layer"),
        };
        let second = ChangeSetRow {
            node: source,
            kind: ChangeKind::ContentOnly,
            digest: digest("source"),
        };

        let change_set = ChangeSet::from_rows(vec![first.clone(), second.clone(), first.clone()]);

        assert_eq!(change_set.rows(), &[second, first]);
    }
}
