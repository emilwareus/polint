#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Layer manifest reuse now calls the planner; some future invalidation actions remain reserved."
    )
)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use super::change_set::{ChangeKind, ChangeSet, ChangeSetRow};
use super::dependency_index::{CacheNode, DEPENDENCY_INDEX_SCHEMA, DependencyIndex};
use super::digest::Digest;
use super::keys::{DiagnosticKey, LayerKey, QueryKey, SummaryKey};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum InvalidationAction {
    Reuse(CacheNode),
    Verify(CacheNode, VerifyReason),
    Recompute(CacheNode, RecomputeReason),
    Drop(CacheNode, DropReason),
    Quarantine(CacheNode, QuarantineReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum VerifyReason {
    SourceContentChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum RecomputeReason {
    UnknownChange,
    MissingDependencyTarget,
    SourceShapeChanged,
    ImportShapeChanged,
    PublicApiShapeChanged,
    ModuleTopologyChanged,
    LifecycleChanged,
    ToolchainChanged,
    RuleInputChanged,
    ModelChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum DropReason {
    DependencyIndexSchemaMismatch,
    ProviderVersionChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum QuarantineReason {
    ExtensionChanged,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InvalidationStats {
    pub(crate) reuse: u64,
    pub(crate) verify: u64,
    pub(crate) recompute: u64,
    pub(crate) drop: u64,
    pub(crate) quarantine: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InvalidationPlan {
    pub(crate) actions: Vec<InvalidationAction>,
    pub(crate) affected_nodes: Vec<CacheNode>,
    pub(crate) stats: InvalidationStats,
}

impl InvalidationPlan {
    pub(crate) fn from_change_set(index: &DependencyIndex, change_set: &ChangeSet) -> Self {
        let mut actions = default_reuse_actions(index, change_set);

        if index.schema_version != DEPENDENCY_INDEX_SCHEMA {
            let nodes = actions.keys().cloned().collect::<Vec<_>>();
            for node in nodes {
                actions.insert(
                    node.clone(),
                    InvalidationAction::Drop(node, DropReason::DependencyIndexSchemaMismatch),
                );
            }
            return Self::from_actions(actions);
        }

        for row in change_set.rows() {
            if !index.contains_node(&row.node) {
                actions.insert(
                    row.node.clone(),
                    InvalidationAction::Recompute(
                        row.node.clone(),
                        RecomputeReason::MissingDependencyTarget,
                    ),
                );
                continue;
            }

            if let Some(action) = action_for_changed_node(row) {
                actions.insert(row.node.clone(), action);
            }

            apply_dependent_actions(index, row, &mut actions);
        }

        Self::from_actions(actions)
    }

    #[cfg(test)]
    pub(crate) fn action_for(&self, node: &CacheNode) -> Option<&InvalidationAction> {
        self.actions.iter().find(|action| action.node() == node)
    }

    fn from_actions(actions: BTreeMap<CacheNode, InvalidationAction>) -> Self {
        let mut stats = InvalidationStats::default();
        let mut affected_nodes = Vec::new();
        let actions = actions
            .into_values()
            .inspect(|action| match action {
                InvalidationAction::Reuse(_) => stats.reuse += 1,
                InvalidationAction::Verify(node, _) => {
                    stats.verify += 1;
                    affected_nodes.push(node.clone());
                }
                InvalidationAction::Recompute(node, _) => {
                    stats.recompute += 1;
                    affected_nodes.push(node.clone());
                }
                InvalidationAction::Drop(node, _) => {
                    stats.drop += 1;
                    affected_nodes.push(node.clone());
                }
                InvalidationAction::Quarantine(node, _) => {
                    stats.quarantine += 1;
                    affected_nodes.push(node.clone());
                }
            })
            .collect::<Vec<_>>();
        affected_nodes.sort();
        affected_nodes.dedup();

        Self {
            actions,
            affected_nodes,
            stats,
        }
    }
}

impl InvalidationAction {
    fn node(&self) -> &CacheNode {
        match self {
            Self::Reuse(node)
            | Self::Verify(node, _)
            | Self::Recompute(node, _)
            | Self::Drop(node, _)
            | Self::Quarantine(node, _) => node,
        }
    }
}

fn default_reuse_actions(
    index: &DependencyIndex,
    change_set: &ChangeSet,
) -> BTreeMap<CacheNode, InvalidationAction> {
    let mut nodes = index.all_nodes();
    nodes.extend(change_set.rows().iter().map(|row| row.node.clone()));
    nodes
        .into_iter()
        .map(|node| (node.clone(), InvalidationAction::Reuse(node)))
        .collect()
}

fn action_for_changed_node(row: &ChangeSetRow) -> Option<InvalidationAction> {
    if matches!(row.node, CacheNode::Input(_) | CacheNode::ToolInvocation(_)) {
        return None;
    }
    action_for_change(row, &row.node)
}

fn apply_dependent_actions(
    index: &DependencyIndex,
    row: &ChangeSetRow,
    actions: &mut BTreeMap<CacheNode, InvalidationAction>,
) {
    let mut queue = VecDeque::from([row.node.clone()]);
    let mut visited = BTreeSet::new();

    while let Some(node) = queue.pop_front() {
        let Some(edges) = index.reverse_edges(&node) else {
            continue;
        };
        for edge in edges {
            let dependent = edge.from.clone();
            if !visited.insert(dependent.clone()) {
                continue;
            }
            let Some(action) = action_for_change(row, &dependent) else {
                continue;
            };
            actions.insert(dependent.clone(), action);
            queue.push_back(dependent);
        }
    }
}

fn action_for_change(row: &ChangeSetRow, node: &CacheNode) -> Option<InvalidationAction> {
    match row.kind {
        ChangeKind::ContentOnly => Some(InvalidationAction::Verify(
            node.clone(),
            VerifyReason::SourceContentChanged,
        )),
        ChangeKind::SyntaxShape => Some(InvalidationAction::Recompute(
            node.clone(),
            RecomputeReason::SourceShapeChanged,
        )),
        ChangeKind::ImportShape => Some(InvalidationAction::Recompute(
            node.clone(),
            RecomputeReason::ImportShapeChanged,
        )),
        ChangeKind::PublicApiShape => Some(InvalidationAction::Recompute(
            node.clone(),
            RecomputeReason::PublicApiShapeChanged,
        )),
        ChangeKind::ModuleTopology => Some(InvalidationAction::Recompute(
            node.clone(),
            RecomputeReason::ModuleTopologyChanged,
        )),
        ChangeKind::Lifecycle => Some(InvalidationAction::Recompute(
            node.clone(),
            RecomputeReason::LifecycleChanged,
        )),
        ChangeKind::Toolchain => Some(InvalidationAction::Recompute(
            node.clone(),
            RecomputeReason::ToolchainChanged,
        )),
        ChangeKind::RuleCode | ChangeKind::RuleOptions => {
            if node_contains_digest(node, &row.digest) {
                Some(InvalidationAction::Recompute(
                    node.clone(),
                    RecomputeReason::RuleInputChanged,
                ))
            } else {
                None
            }
        }
        ChangeKind::ExtensionCode | ChangeKind::ExtensionDeclaredInput => Some(
            InvalidationAction::Quarantine(node.clone(), QuarantineReason::ExtensionChanged),
        ),
        ChangeKind::ModelFile => Some(InvalidationAction::Recompute(
            node.clone(),
            RecomputeReason::ModelChanged,
        )),
        ChangeKind::ProviderVersion => Some(InvalidationAction::Drop(
            node.clone(),
            DropReason::ProviderVersionChanged,
        )),
        ChangeKind::Unknown => Some(InvalidationAction::Recompute(
            node.clone(),
            RecomputeReason::UnknownChange,
        )),
    }
}

fn node_contains_digest(node: &CacheNode, digest: &Digest) -> bool {
    match node {
        CacheNode::Layer(key) => layer_key_contains_digest(key, digest),
        CacheNode::Query(key) => query_key_contains_digest(key, digest),
        CacheNode::Summary(key) => summary_key_contains_digest(key, digest),
        CacheNode::Diagnostic(key) => diagnostic_key_contains_digest(key, digest),
        CacheNode::Input(_) | CacheNode::Extension(_) | CacheNode::ToolInvocation(_) => false,
    }
}

fn layer_key_contains_digest(key: &LayerKey, digest: &Digest) -> bool {
    key.parameter_digest == *digest
        || key.lifecycle_digest == *digest
        || key.config_digest == *digest
        || key.toolchain_digest == *digest
        || key.input_digests.contains(digest)
        || key.dependency_layer_digests.contains(digest)
        || key.extension_digests.contains(digest)
}

fn query_key_contains_digest(key: &QueryKey, digest: &Digest) -> bool {
    key.parameter_digest == *digest
        || key.budget_digest == *digest
        || key.layer_digests.contains(digest)
}

fn summary_key_contains_digest(key: &SummaryKey, digest: &Digest) -> bool {
    key.body_shape_digest == *digest
        || key.extension_digest == *digest
        || key.dependency_summary_digests.contains(digest)
}

fn diagnostic_key_contains_digest(key: &DiagnosticKey, digest: &Digest) -> bool {
    key.rule_code_digest == *digest
        || key.options_digest == *digest
        || key.evidence_digest == *digest
        || key.requested_view_digests.contains(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::{
        CacheNode, ChangeKind, ChangeSet, ChangeSetRow, DEPENDENCY_INDEX_SCHEMA, DependencyEdge,
        DependencyIndex, DependencyKind, Digest, DigestKind, LayerKey, ShapeKind,
    };

    fn digest(kind: DigestKind, label: &str) -> Digest {
        Digest::from_parts(kind, label, &[label])
    }

    fn layer(provider_id: &str, input_label: &str, rule_digest: Option<Digest>) -> LayerKey {
        let mut input_digests = vec![digest(DigestKind::SourceText, input_label)];
        if let Some(rule_digest) = rule_digest {
            input_digests.push(rule_digest);
        }
        LayerKey::new(
            crate::analysis_kernel::incremental::keys::LayerKind::TsSyntax,
            provider_id,
            "1",
            "ts-facts-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::TsJsLifecycle, "none"),
            Digest::absent(DigestKind::Config, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            input_digests,
            Vec::new(),
            Vec::new(),
        )
    }

    fn edge(
        from: CacheNode,
        to: CacheNode,
        kind: DependencyKind,
        shape: ShapeKind,
    ) -> DependencyEdge {
        DependencyEdge {
            from,
            to,
            kind,
            required_shape: shape,
        }
    }

    fn change(node: CacheNode, kind: ChangeKind, digest: Digest) -> ChangeSet {
        ChangeSet::from_rows(vec![ChangeSetRow { node, kind, digest }])
    }

    #[test]
    fn unknown_change_never_reuses_affected_dependents() {
        let source = CacheNode::Input("src/a.ts".to_string());
        let layer = CacheNode::Layer(layer("polint.ts.syntax", "a", None));
        let index = DependencyIndex::from_edges(vec![edge(
            layer.clone(),
            source.clone(),
            DependencyKind::Input,
            ShapeKind::Content,
        )]);

        let plan = InvalidationPlan::from_change_set(
            &index,
            &change(
                source,
                ChangeKind::Unknown,
                digest(DigestKind::SourceText, "a"),
            ),
        );

        assert!(matches!(
            plan.action_for(&layer),
            Some(InvalidationAction::Recompute(
                _,
                RecomputeReason::UnknownChange
            ))
        ));
    }

    #[test]
    fn corrupt_index_schema_drops_instead_of_reusing() {
        let source = CacheNode::Input("src/a.ts".to_string());
        let layer = CacheNode::Layer(layer("polint.ts.syntax", "a", None));
        let mut index = DependencyIndex::from_edges(vec![edge(
            layer.clone(),
            source.clone(),
            DependencyKind::Input,
            ShapeKind::Content,
        )]);
        index.schema_version = "old-schema".to_string();

        let plan = InvalidationPlan::from_change_set(
            &index,
            &change(
                source,
                ChangeKind::ContentOnly,
                digest(DigestKind::SourceText, "a"),
            ),
        );

        assert!(matches!(
            plan.action_for(&layer),
            Some(InvalidationAction::Drop(
                _,
                DropReason::DependencyIndexSchemaMismatch
            ))
        ));
    }

    #[test]
    fn missing_dependency_target_recomputes_instead_of_reusing() {
        let missing = CacheNode::Input("src/missing.ts".to_string());
        let index = DependencyIndex::from_edges(Vec::new());

        let plan = InvalidationPlan::from_change_set(
            &index,
            &change(
                missing.clone(),
                ChangeKind::ContentOnly,
                digest(DigestKind::SourceText, "missing"),
            ),
        );

        assert!(matches!(
            plan.action_for(&missing),
            Some(InvalidationAction::Recompute(
                _,
                RecomputeReason::MissingDependencyTarget
            ))
        ));
    }

    #[test]
    fn source_content_changes_affect_only_matching_source_dependents() {
        let source_a = CacheNode::Input("src/a.ts".to_string());
        let source_b = CacheNode::Input("src/b.ts".to_string());
        let layer_a = CacheNode::Layer(layer("polint.ts.syntax", "a", None));
        let layer_b = CacheNode::Layer(layer("polint.ts.syntax", "b", None));
        let index = DependencyIndex::from_edges(vec![
            edge(
                layer_a.clone(),
                source_a.clone(),
                DependencyKind::Input,
                ShapeKind::Content,
            ),
            edge(
                layer_b.clone(),
                source_b,
                DependencyKind::Input,
                ShapeKind::Content,
            ),
        ]);

        let plan = InvalidationPlan::from_change_set(
            &index,
            &change(
                source_a,
                ChangeKind::ContentOnly,
                digest(DigestKind::SourceText, "a"),
            ),
        );

        assert!(matches!(
            plan.action_for(&layer_a),
            Some(InvalidationAction::Verify(
                _,
                VerifyReason::SourceContentChanged
            ))
        ));
        assert!(matches!(
            plan.action_for(&layer_b),
            Some(InvalidationAction::Reuse(_))
        ));
    }

    #[test]
    fn lifecycle_and_provider_changes_fail_closed_for_dependents() {
        let lifecycle = CacheNode::ToolInvocation("go-lifecycle".to_string());
        let layer = CacheNode::Layer(layer("polint.go.syntax", "a", None));
        let index = DependencyIndex::from_edges(vec![edge(
            layer.clone(),
            lifecycle.clone(),
            DependencyKind::Lifecycle,
            ShapeKind::Lifecycle,
        )]);

        let lifecycle_plan = InvalidationPlan::from_change_set(
            &index,
            &change(
                lifecycle.clone(),
                ChangeKind::Lifecycle,
                digest(DigestKind::GoLifecycle, "go"),
            ),
        );
        let provider_plan = InvalidationPlan::from_change_set(
            &index,
            &change(
                lifecycle,
                ChangeKind::ProviderVersion,
                digest(DigestKind::ProviderOutput, "provider"),
            ),
        );

        assert!(matches!(
            lifecycle_plan.action_for(&layer),
            Some(InvalidationAction::Recompute(
                _,
                RecomputeReason::LifecycleChanged
            ))
        ));
        assert!(matches!(
            provider_plan.action_for(&layer),
            Some(InvalidationAction::Drop(
                _,
                DropReason::ProviderVersionChanged
            ))
        ));
    }

    #[test]
    fn rule_changes_do_not_affect_syntax_layers_unless_digest_is_in_key() {
        let rule = CacheNode::Input("rule:local/example".to_string());
        let rule_digest = digest(DigestKind::RuleCode, "rule");
        let unaffected = CacheNode::Layer(layer("polint.ts.syntax", "a", None));
        let affected = CacheNode::Layer(layer("polint.ts.syntax", "b", Some(rule_digest.clone())));
        let index = DependencyIndex::from_edges(vec![
            edge(
                unaffected.clone(),
                rule.clone(),
                DependencyKind::Rule,
                ShapeKind::RuleCode,
            ),
            edge(
                affected.clone(),
                rule.clone(),
                DependencyKind::Rule,
                ShapeKind::RuleCode,
            ),
        ]);

        let plan = InvalidationPlan::from_change_set(
            &index,
            &change(rule, ChangeKind::RuleCode, rule_digest),
        );

        assert!(matches!(
            plan.action_for(&unaffected),
            Some(InvalidationAction::Reuse(_))
        ));
        assert!(matches!(
            plan.action_for(&affected),
            Some(InvalidationAction::Recompute(
                _,
                RecomputeReason::RuleInputChanged
            ))
        ));
    }

    #[test]
    fn dependency_index_schema_constant_matches_expected_value() {
        assert_eq!(DEPENDENCY_INDEX_SCHEMA, "polint-dependency-index-1");
    }
}
