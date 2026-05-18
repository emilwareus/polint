#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 24 establishes dependency-index vocabulary before provider dependency recording is wired in."
    )
)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::keys::{DiagnosticKey, LayerKey, QueryKey, SummaryKey};

pub(crate) const DEPENDENCY_INDEX_SCHEMA: &str = "polint-dependency-index-1";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CacheNode {
    Input(String),
    Layer(LayerKey),
    Query(QueryKey),
    Summary(SummaryKey),
    Diagnostic(DiagnosticKey),
    Extension(String),
    ToolInvocation(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DependencyKind {
    Input,
    Layer,
    Lifecycle,
    Config,
    Toolchain,
    Rule,
    Extension,
    Model,
    Provider,
    ToolInvocation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ShapeKind {
    Content,
    Syntax,
    Import,
    PublicApi,
    ModuleTopology,
    Lifecycle,
    Toolchain,
    RuleCode,
    RuleOptions,
    ExtensionCode,
    ExtensionDeclaredInput,
    Model,
    ProviderVersion,
    Output,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct DependencyEdge {
    pub(crate) from: CacheNode,
    pub(crate) to: CacheNode,
    pub(crate) kind: DependencyKind,
    pub(crate) required_shape: ShapeKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DependencyIndex {
    pub(crate) schema_version: String,
    pub(crate) forward: BTreeMap<CacheNode, Vec<DependencyEdge>>,
    pub(crate) reverse: BTreeMap<CacheNode, Vec<DependencyEdge>>,
}

impl DependencyIndex {
    pub(crate) fn from_edges(mut edges: Vec<DependencyEdge>) -> Self {
        edges.sort();
        edges.dedup();

        let mut forward: BTreeMap<CacheNode, Vec<DependencyEdge>> = BTreeMap::new();
        let mut reverse: BTreeMap<CacheNode, Vec<DependencyEdge>> = BTreeMap::new();
        for edge in edges {
            forward
                .entry(edge.from.clone())
                .or_default()
                .push(edge.clone());
            reverse.entry(edge.to.clone()).or_default().push(edge);
        }
        sort_and_dedup_edges(&mut forward);
        sort_and_dedup_edges(&mut reverse);

        Self {
            schema_version: DEPENDENCY_INDEX_SCHEMA.to_string(),
            forward,
            reverse,
        }
    }

    pub(crate) fn forward_edges(&self, node: &CacheNode) -> Option<&[DependencyEdge]> {
        self.forward.get(node).map(Vec::as_slice)
    }

    pub(crate) fn reverse_edges(&self, node: &CacheNode) -> Option<&[DependencyEdge]> {
        self.reverse.get(node).map(Vec::as_slice)
    }

    pub(crate) fn contains_node(&self, node: &CacheNode) -> bool {
        self.forward.contains_key(node) || self.reverse.contains_key(node)
    }

    pub(crate) fn all_nodes(&self) -> BTreeSet<CacheNode> {
        let mut nodes = BTreeSet::new();
        for (node, edges) in &self.forward {
            nodes.insert(node.clone());
            for edge in edges {
                nodes.insert(edge.from.clone());
                nodes.insert(edge.to.clone());
            }
        }
        for (node, edges) in &self.reverse {
            nodes.insert(node.clone());
            for edge in edges {
                nodes.insert(edge.from.clone());
                nodes.insert(edge.to.clone());
            }
        }
        nodes
    }
}

fn sort_and_dedup_edges(index: &mut BTreeMap<CacheNode, Vec<DependencyEdge>>) {
    for edges in index.values_mut() {
        edges.sort();
        edges.dedup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, LayerKey, PrecisionTier};

    fn digest(kind: DigestKind, label: &str) -> Digest {
        Digest::from_parts(kind, label, &[label])
    }

    fn layer(provider_id: &str, input_label: &str) -> LayerKey {
        LayerKey::new(
            crate::analysis_kernel::incremental::keys::LayerKind::TsSyntax,
            provider_id,
            "1",
            "ts-facts-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::TsJsLifecycle, "none"),
            Digest::absent(DigestKind::Config, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            vec![digest(DigestKind::SourceText, input_label)],
            Vec::new(),
            Vec::new(),
        )
    }

    fn edge(from: CacheNode, to: CacheNode, required_shape: ShapeKind) -> DependencyEdge {
        DependencyEdge {
            from,
            to,
            kind: DependencyKind::Input,
            required_shape,
        }
    }

    #[test]
    fn from_edges_stores_sorted_forward_reverse_edges_and_schema() {
        let source_a = CacheNode::Input("src/a.ts".to_string());
        let source_b = CacheNode::Input("src/b.ts".to_string());
        let layer_a = CacheNode::Layer(layer("polint.ts.syntax", "a"));
        let layer_b = CacheNode::Layer(layer("polint.ts.syntax", "b"));

        let index = DependencyIndex::from_edges(vec![
            edge(layer_b.clone(), source_b.clone(), ShapeKind::Syntax),
            edge(layer_a.clone(), source_a.clone(), ShapeKind::Content),
        ]);

        assert_eq!(index.schema_version, DEPENDENCY_INDEX_SCHEMA);
        assert_eq!(
            index
                .forward_edges(&layer_a)
                .expect("layer a should have forward edges"),
            &[edge(layer_a.clone(), source_a.clone(), ShapeKind::Content)]
        );
        assert_eq!(
            index
                .reverse_edges(&source_b)
                .expect("source b should have reverse edges"),
            &[edge(layer_b, source_b, ShapeKind::Syntax)]
        );
    }

    #[test]
    fn from_edges_sorts_and_deduplicates_duplicate_rows() {
        let source = CacheNode::Input("src/a.ts".to_string());
        let layer = CacheNode::Layer(layer("polint.ts.syntax", "a"));
        let duplicate = edge(layer.clone(), source.clone(), ShapeKind::Content);

        let index = DependencyIndex::from_edges(vec![duplicate.clone(), duplicate.clone()]);

        assert_eq!(
            index.forward_edges(&layer).expect("forward edges"),
            &[duplicate.clone()]
        );
        assert_eq!(
            index.reverse_edges(&source).expect("reverse edges"),
            &[duplicate]
        );
    }

    #[test]
    fn dependency_index_serializes_schema_version() {
        let index = DependencyIndex::from_edges(Vec::new());
        let json = serde_json::to_value(index).expect("index should serialize");

        assert_eq!(json["schema_version"], "polint-dependency-index-1");
    }

    #[test]
    fn cache_node_keeps_future_key_shapes_available() {
        let query = crate::analysis_kernel::incremental::QueryKey::new(
            "call_graph",
            "1",
            Digest::absent(DigestKind::QueryParameters, "none"),
            Vec::new(),
            Digest::absent(DigestKind::Budget, "none"),
            PrecisionTier::Syntax,
        );
        let summary = crate::analysis_kernel::incremental::SummaryKey::new(
            "function:main",
            "effects",
            "1",
            Digest::absent(DigestKind::SummaryBody, "none"),
            Vec::new(),
            Digest::absent(DigestKind::ExtensionCode, "none"),
        );
        let diagnostic = crate::analysis_kernel::incremental::DiagnosticKey::new(
            "local/example",
            "1",
            Digest::absent(DigestKind::RuleCode, "none"),
            Digest::absent(DigestKind::RuleOptions, "none"),
            Vec::new(),
            Digest::absent(DigestKind::Evidence, "none"),
        );

        let mut nodes = vec![
            CacheNode::ToolInvocation("go".to_string()),
            CacheNode::Extension("extension".to_string()),
            CacheNode::Diagnostic(diagnostic),
            CacheNode::Summary(summary),
            CacheNode::Query(query),
            CacheNode::Layer(layer("polint.ts.syntax", "a")),
            CacheNode::Input("src/a.ts".to_string()),
        ];
        nodes.sort();

        assert_eq!(nodes.len(), 7);
    }
}
