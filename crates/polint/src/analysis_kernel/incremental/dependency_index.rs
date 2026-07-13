use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::dependency_input::InputDependencyKey;
use super::keys::{DiagnosticKey, LayerKey, QueryKey, SummaryKey};

pub(crate) const DEPENDENCY_INDEX_SCHEMA: &str = "polint-dependency-index-next-typed";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CacheNode {
    DependencyInput(InputDependencyKey),
    Layer(LayerKey),
    Query(QueryKey),
    Summary(SummaryKey),
    Diagnostic(DiagnosticKey),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CacheNodeKind {
    DependencyInput,
    Layer,
    Query,
    Summary,
    Diagnostic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DependencyKind {
    Input,
    SourceText,
    ImportShape,
    Layer,
    Lifecycle,
    Config,
    Toolchain,
    Rule,
    Extension,
    Model,
    Provider,
    ProviderSchema,
    UpstreamLayer,
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

macro_rules! unknown_label_error {
    ($name:ident, $description:literal) => {
        #[cfg_attr(
            not(test),
            allow(
                dead_code,
                reason = "canonical label errors are returned by private persistence readers"
            )
        )]
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub(crate) struct $name {
            label: String,
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    formatter,
                    concat!("unknown ", $description, " label `{}`"),
                    self.label
                )
            }
        }

        impl std::error::Error for $name {}
    };
}

unknown_label_error!(UnknownDependencyKindLabel, "dependency kind");
unknown_label_error!(UnknownShapeKindLabel, "shape kind");
unknown_label_error!(UnknownCacheNodeKindLabel, "cache node kind");

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "canonical node-kind codecs are consumed by private persistence readers"
    )
)]
impl CacheNodeKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::DependencyInput => "dependency_input",
            Self::Layer => "layer",
            Self::Query => "query",
            Self::Summary => "summary",
            Self::Diagnostic => "diagnostic",
        }
    }

    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownCacheNodeKindLabel> {
        match label {
            "dependency_input" => Ok(Self::DependencyInput),
            "layer" => Ok(Self::Layer),
            "query" => Ok(Self::Query),
            "summary" => Ok(Self::Summary),
            "diagnostic" => Ok(Self::Diagnostic),
            _ => Err(UnknownCacheNodeKindLabel {
                label: label.to_string(),
            }),
        }
    }
}

impl CacheNode {
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "canonical node-kind derivation is reserved for private persistence readers"
        )
    )]
    pub(crate) fn kind(&self) -> CacheNodeKind {
        match self {
            Self::DependencyInput(_) => CacheNodeKind::DependencyInput,
            Self::Layer(_) => CacheNodeKind::Layer,
            Self::Query(_) => CacheNodeKind::Query,
            Self::Summary(_) => CacheNodeKind::Summary,
            Self::Diagnostic(_) => CacheNodeKind::Diagnostic,
        }
    }
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "canonical dependency-kind codecs are consumed by private persistence readers"
    )
)]
impl DependencyKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Input => "input",
            Self::SourceText => "source_text",
            Self::ImportShape => "import_shape",
            Self::Layer => "layer",
            Self::Lifecycle => "lifecycle",
            Self::Config => "config",
            Self::Toolchain => "toolchain",
            Self::Rule => "rule",
            Self::Extension => "extension",
            Self::Model => "model",
            Self::Provider => "provider",
            Self::ProviderSchema => "provider_schema",
            Self::UpstreamLayer => "upstream_layer",
            Self::ToolInvocation => "tool_invocation",
        }
    }

    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownDependencyKindLabel> {
        match label {
            "input" => Ok(Self::Input),
            "source_text" => Ok(Self::SourceText),
            "import_shape" => Ok(Self::ImportShape),
            "layer" => Ok(Self::Layer),
            "lifecycle" => Ok(Self::Lifecycle),
            "config" => Ok(Self::Config),
            "toolchain" => Ok(Self::Toolchain),
            "rule" => Ok(Self::Rule),
            "extension" => Ok(Self::Extension),
            "model" => Ok(Self::Model),
            "provider" => Ok(Self::Provider),
            "provider_schema" => Ok(Self::ProviderSchema),
            "upstream_layer" => Ok(Self::UpstreamLayer),
            "tool_invocation" => Ok(Self::ToolInvocation),
            _ => Err(UnknownDependencyKindLabel {
                label: label.to_string(),
            }),
        }
    }
}

#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "canonical dependency-shape codecs are consumed by private persistence readers"
    )
)]
impl ShapeKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Content => "content",
            Self::Syntax => "syntax",
            Self::Import => "import",
            Self::PublicApi => "public_api",
            Self::ModuleTopology => "module_topology",
            Self::Lifecycle => "lifecycle",
            Self::Toolchain => "toolchain",
            Self::RuleCode => "rule_code",
            Self::RuleOptions => "rule_options",
            Self::ExtensionCode => "extension_code",
            Self::ExtensionDeclaredInput => "extension_declared_input",
            Self::Model => "model",
            Self::ProviderVersion => "provider_version",
            Self::Output => "output",
            Self::Unknown => "unknown",
        }
    }

    pub(crate) fn parse_label(label: &str) -> Result<Self, UnknownShapeKindLabel> {
        match label {
            "content" => Ok(Self::Content),
            "syntax" => Ok(Self::Syntax),
            "import" => Ok(Self::Import),
            "public_api" => Ok(Self::PublicApi),
            "module_topology" => Ok(Self::ModuleTopology),
            "lifecycle" => Ok(Self::Lifecycle),
            "toolchain" => Ok(Self::Toolchain),
            "rule_code" => Ok(Self::RuleCode),
            "rule_options" => Ok(Self::RuleOptions),
            "extension_code" => Ok(Self::ExtensionCode),
            "extension_declared_input" => Ok(Self::ExtensionDeclaredInput),
            "model" => Ok(Self::Model),
            "provider_version" => Ok(Self::ProviderVersion),
            "output" => Ok(Self::Output),
            "unknown" => Ok(Self::Unknown),
            _ => Err(UnknownShapeKindLabel {
                label: label.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct DependencyEdge {
    pub(crate) from: CacheNode,
    pub(crate) to: CacheNode,
    pub(crate) kind: DependencyKind,
    pub(crate) required_shape: ShapeKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DependencyIndex {
    pub(crate) schema_version: String,
    canonical_edges: Vec<DependencyEdge>,
    forward: BTreeMap<CacheNode, Vec<DependencyEdge>>,
    reverse: BTreeMap<CacheNode, Vec<DependencyEdge>>,
}

impl Default for DependencyIndex {
    fn default() -> Self {
        Self::from_edges(Vec::new())
    }
}

impl DependencyIndex {
    pub(crate) fn from_edges(mut edges: Vec<DependencyEdge>) -> Self {
        edges.sort();
        edges.dedup();

        let mut forward: BTreeMap<CacheNode, Vec<DependencyEdge>> = BTreeMap::new();
        let mut reverse: BTreeMap<CacheNode, Vec<DependencyEdge>> = BTreeMap::new();
        for edge in &edges {
            forward
                .entry(edge.from.clone())
                .or_default()
                .push(edge.clone());
            reverse
                .entry(edge.to.clone())
                .or_default()
                .push(edge.clone());
        }
        sort_and_dedup_edges(&mut forward);
        sort_and_dedup_edges(&mut reverse);

        Self {
            schema_version: DEPENDENCY_INDEX_SCHEMA.to_string(),
            canonical_edges: edges,
            forward,
            reverse,
        }
    }

    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "canonical edge iteration is consumed by private persistence planning"
        )
    )]
    pub(crate) fn canonical_edges(&self) -> &[DependencyEdge] {
        &self.canonical_edges
    }

    #[cfg(test)]
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
        for edge in &self.canonical_edges {
            nodes.insert(edge.from.clone());
            nodes.insert(edge.to.clone());
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

#[derive(Deserialize, Serialize)]
struct DependencyIndexWire {
    schema_version: String,
    edges: Vec<DependencyEdge>,
}

impl Serialize for DependencyIndex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        DependencyIndexWire {
            schema_version: self.schema_version.clone(),
            edges: self.canonical_edges.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for DependencyIndex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DependencyIndexWire::deserialize(deserializer)?;
        if wire.schema_version != DEPENDENCY_INDEX_SCHEMA {
            return Err(serde::de::Error::custom(format!(
                "unsupported dependency index schema `{}`",
                wire.schema_version
            )));
        }
        Ok(Self::from_edges(wire.edges))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{
        Digest, DigestKind, InputComponentStatus, InputDependencyKey, LayerKey, PrecisionTier,
    };

    fn digest(kind: DigestKind, label: &str) -> Digest {
        Digest::from_parts(kind, label, &[label])
    }

    fn source(label: &str) -> CacheNode {
        CacheNode::DependencyInput(
            InputDependencyKey::source_file(
                label,
                digest(DigestKind::SourceText, label),
                InputComponentStatus::Present,
            )
            .expect("source dependency uses a source-text digest"),
        )
    }

    fn tool(label: &str) -> CacheNode {
        CacheNode::DependencyInput(
            InputDependencyKey::tool_invocation(
                label,
                Digest::absent(DigestKind::ToolInvocation, label),
                InputComponentStatus::Absent,
            )
            .expect("tool dependency uses a tool-invocation digest"),
        )
    }

    fn extension(label: &str) -> CacheNode {
        CacheNode::DependencyInput(
            InputDependencyKey::extension_code(
                label,
                digest(DigestKind::ExtensionCode, label),
                InputComponentStatus::Present,
            )
            .expect("extension dependency uses an extension-code digest"),
        )
    }

    fn provider(label: &str) -> CacheNode {
        CacheNode::DependencyInput(
            InputDependencyKey::provider_manifest(
                label,
                digest(DigestKind::ProviderManifest, label),
                InputComponentStatus::Present,
            )
            .expect("provider dependency uses a provider-manifest digest"),
        )
    }

    fn layer(provider_id: &str, input_label: &str) -> LayerKey {
        LayerKey::new(
            crate::analysis_kernel::incremental::keys::LayerKind::TsSyntax,
            provider_id,
            "1",
            "ts-facts-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::TsJsLifecycle, "none"),
            Digest::absent(DigestKind::AnalysisSettings, "none"),
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
        let source_a = source("src/a.ts");
        let source_b = source("src/b.ts");
        let layer_a = CacheNode::Layer(layer("polint.ts.syntax", "a"));
        let layer_b = CacheNode::Layer(layer("polint.ts.syntax", "b"));

        let index = DependencyIndex::from_edges(vec![
            edge(layer_b.clone(), source_b.clone(), ShapeKind::Syntax),
            edge(layer_a.clone(), source_a.clone(), ShapeKind::Content),
        ]);

        assert_eq!(index.schema_version, DEPENDENCY_INDEX_SCHEMA);
        let forward_a = index
            .forward_edges(&layer_a)
            .expect("layer a should have forward edges");
        let expected_forward_a = [edge(layer_a.clone(), source_a, ShapeKind::Content)];
        assert_eq!(forward_a, expected_forward_a.as_slice());

        let reverse_b = index
            .reverse_edges(&source_b)
            .expect("source b should have reverse edges");
        let expected_reverse_b = [edge(layer_b, source_b, ShapeKind::Syntax)];
        assert_eq!(reverse_b, expected_reverse_b.as_slice());
    }

    #[test]
    fn from_edges_sorts_and_deduplicates_duplicate_rows() {
        let source = source("src/a.ts");
        let layer = CacheNode::Layer(layer("polint.ts.syntax", "a"));
        let duplicate = edge(layer.clone(), source.clone(), ShapeKind::Content);

        let index = DependencyIndex::from_edges(vec![duplicate.clone(), duplicate.clone()]);

        assert_eq!(
            index.forward_edges(&layer).expect("forward edges"),
            std::slice::from_ref(&duplicate)
        );
        assert_eq!(
            index.reverse_edges(&source).expect("reverse edges"),
            std::slice::from_ref(&duplicate)
        );
    }

    #[test]
    fn dependency_index_serializes_schema_version() {
        let index = DependencyIndex::from_edges(Vec::new());
        let json = serde_json::to_value(index).expect("index should serialize");

        assert_eq!(json["schema_version"], DEPENDENCY_INDEX_SCHEMA);
        assert!(json.get("edges").is_some());
        assert!(json.get("forward").is_none());
        assert!(json.get("reverse").is_none());
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

        let mut nodes = [
            tool("go"),
            extension("extension"),
            CacheNode::Diagnostic(diagnostic),
            CacheNode::Summary(summary),
            CacheNode::Query(query),
            CacheNode::Layer(layer("polint.ts.syntax", "a")),
            source("src/a.ts"),
        ];
        nodes.sort();

        assert_eq!(nodes.len(), 7);
        assert_eq!(
            nodes.iter().map(CacheNode::kind).collect::<BTreeSet<_>>(),
            BTreeSet::from([
                CacheNodeKind::DependencyInput,
                CacheNodeKind::Layer,
                CacheNodeKind::Query,
                CacheNodeKind::Summary,
                CacheNodeKind::Diagnostic,
            ])
        );
    }

    #[test]
    fn cache_node_dependency_and_shape_codecs_are_exact_and_closed() {
        for (kind, label) in [
            (CacheNodeKind::DependencyInput, "dependency_input"),
            (CacheNodeKind::Layer, "layer"),
            (CacheNodeKind::Query, "query"),
            (CacheNodeKind::Summary, "summary"),
            (CacheNodeKind::Diagnostic, "diagnostic"),
        ] {
            assert_eq!(kind.label(), label);
            assert_eq!(CacheNodeKind::parse_label(label), Ok(kind));
        }

        for kind in [
            DependencyKind::Input,
            DependencyKind::SourceText,
            DependencyKind::ImportShape,
            DependencyKind::Layer,
            DependencyKind::Lifecycle,
            DependencyKind::Config,
            DependencyKind::Toolchain,
            DependencyKind::Rule,
            DependencyKind::Extension,
            DependencyKind::Model,
            DependencyKind::Provider,
            DependencyKind::ProviderSchema,
            DependencyKind::UpstreamLayer,
            DependencyKind::ToolInvocation,
        ] {
            assert_eq!(DependencyKind::parse_label(kind.label()), Ok(kind));
        }

        for shape in [
            ShapeKind::Content,
            ShapeKind::Syntax,
            ShapeKind::Import,
            ShapeKind::PublicApi,
            ShapeKind::ModuleTopology,
            ShapeKind::Lifecycle,
            ShapeKind::Toolchain,
            ShapeKind::RuleCode,
            ShapeKind::RuleOptions,
            ShapeKind::ExtensionCode,
            ShapeKind::ExtensionDeclaredInput,
            ShapeKind::Model,
            ShapeKind::ProviderVersion,
            ShapeKind::Output,
            ShapeKind::Unknown,
        ] {
            assert_eq!(ShapeKind::parse_label(shape.label()), Ok(shape));
        }

        assert!(CacheNodeKind::parse_label("input").is_err());
        assert!(DependencyKind::parse_label("source").is_err());
        assert!(ShapeKind::parse_label("public").is_err());
    }

    #[test]
    fn typed_index_round_trips_and_v1_labels_fail_closed() {
        let source = source("src/a.ts");
        let layer = CacheNode::Layer(layer("polint.ts.syntax", "a"));
        let index = DependencyIndex::from_edges(vec![edge(layer, source, ShapeKind::Content)]);
        let typed_wire = serde_json::to_value(&index).expect("typed index serializes");

        assert_eq!(typed_wire["schema_version"], DEPENDENCY_INDEX_SCHEMA);
        assert_eq!(
            serde_json::from_value::<DependencyIndex>(typed_wire.clone())
                .expect("current typed index deserializes"),
            index
        );

        let legacy_wire = serde_json::json!({
            "schema_version": "polint-dependency-index-1",
            "edges": [],
        });
        assert!(serde_json::from_value::<DependencyIndex>(legacy_wire).is_err());

        let mut forged_v1 = typed_wire;
        forged_v1["schema_version"] = "polint-dependency-index-1".into();
        assert!(serde_json::from_value::<DependencyIndex>(forged_v1).is_err());
    }

    #[test]
    fn canonical_edges_and_indexes_ignore_all_twenty_four_provider_source_orders() {
        let layer_a = CacheNode::Layer(layer("polint.ts.syntax", "a"));
        let layer_b = CacheNode::Layer(layer("polint.ts.syntax", "b"));
        let mut rows = vec![
            edge(layer_a.clone(), source("src/a.ts"), ShapeKind::Content),
            edge(
                layer_a,
                provider("polint.ts.syntax"),
                ShapeKind::ProviderVersion,
            ),
            edge(layer_b.clone(), source("src/b.ts"), ShapeKind::Syntax),
            edge(
                layer_b,
                provider("polint.module_graph"),
                ShapeKind::ProviderVersion,
            ),
        ];
        let baseline = DependencyIndex::from_edges(rows.clone());
        let baseline_wire = serde_json::to_value(&baseline).expect("baseline serializes");
        let mut permutations = Vec::new();
        collect_permutations(&mut rows, 0, &mut permutations);

        assert_eq!(permutations.len(), 24);
        for permutation in permutations {
            let candidate = DependencyIndex::from_edges(permutation);
            assert_eq!(candidate.canonical_edges(), baseline.canonical_edges());
            assert_eq!(candidate.forward, baseline.forward);
            assert_eq!(candidate.reverse, baseline.reverse);
            assert_eq!(
                serde_json::to_value(candidate).expect("candidate serializes"),
                baseline_wire
            );
        }
    }

    fn collect_permutations(
        rows: &mut [DependencyEdge],
        start: usize,
        output: &mut Vec<Vec<DependencyEdge>>,
    ) {
        if start == rows.len() {
            output.push(rows.to_vec());
            return;
        }
        for index in start..rows.len() {
            rows.swap(start, index);
            collect_permutations(rows, start + 1, output);
            rows.swap(start, index);
        }
    }
}
