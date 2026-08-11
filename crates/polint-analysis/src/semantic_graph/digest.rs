//! Stable semantic-graph output digesting.
//!
//! The composition root supplies the upstream/frontend component labels; this
//! module owns the language-neutral graph-row digest and the lifecycle component
//! folding shared by all hosts.

use polint_analysis_api::{Digest, DigestKind, InputComponent, InputSnapshot, ProviderManifest};
use polint_core::StableKeyInterner;

use super::cache_key::semantic_graph_provider_parameter_digest;
use super::store::SemanticGraphOutput;

pub fn semantic_graph_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    mut parts: Vec<String>,
    interner: &StableKeyInterner,
    output: &SemanticGraphOutput,
) -> Digest {
    parts.extend_component_parts("go_lifecycle", &input_snapshot.go_lifecycle.components);
    parts.extend_component_parts(
        "ts_js_lifecycle",
        &input_snapshot.ts_js_lifecycle.components,
    );

    let node_key_by_id: Vec<_> = output
        .nodes
        .iter()
        .map(|node| interner.resolve(node.stable_key))
        .collect();
    parts.extend(output.nodes.iter().map(|node| {
        format!(
            "node={}|prec={:?}",
            interner.resolve(node.stable_key),
            node.precision
        )
    }));
    parts.extend(output.edges.iter().map(|edge| {
        format!(
            "edge={}|prec={:?}",
            interner.resolve(edge.stable_key),
            edge.precision
        )
    }));
    parts.extend(output.constraints.iter().map(|constraint| {
        let mut refs: Vec<_> = constraint
            .kind
            .referenced_nodes()
            .into_iter()
            .map(|node| {
                node_key_by_id
                    .get(node.0 as usize)
                    .cloned()
                    .unwrap_or_else(|| "<unresolved>".into())
            })
            .collect();
        refs.sort_unstable();
        format!(
            "constraint={}|status={:?}|prec={:?}|refs=[{}]",
            interner.resolve(constraint.stable_key),
            constraint.status,
            constraint.precision,
            refs.join(","),
        )
    }));
    if output.nodes.is_empty() && output.edges.is_empty() && output.constraints.is_empty() {
        parts.push("semantic_graph_output=empty".to_string());
    }

    parts.push(format!("provider_id={}", manifest.id));
    parts.push(format!("provider_version={}", manifest.provider_version()));
    parts.push(format!("schema={}", manifest.primary_schema_label()));
    parts.push(format!(
        "parameters={}",
        semantic_graph_provider_parameter_digest()
    ));
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "semantic_graph_output", &refs)
}

trait ComponentParts {
    fn extend_component_parts(&mut self, prefix: &str, components: &[InputComponent]);
}

impl ComponentParts for Vec<String> {
    fn extend_component_parts(&mut self, prefix: &str, components: &[InputComponent]) {
        self.extend(components.iter().map(|component| {
            format!(
                "{prefix}:{}:{:?}:{}",
                component.name, component.status, component.digest
            )
        }));
    }
}
