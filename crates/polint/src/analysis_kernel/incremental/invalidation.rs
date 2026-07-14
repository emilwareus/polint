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

        if !change_set.typed_input_digests_match() {
            let nodes = actions.keys().cloned().collect::<Vec<_>>();
            for node in nodes {
                actions.insert(
                    node.clone(),
                    InvalidationAction::Recompute(node, RecomputeReason::UnknownChange),
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
    if matches!(row.node, CacheNode::DependencyInput(_)) {
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
        CacheNode::DependencyInput(input) => input.digest == *digest,
        CacheNode::RunManifest(key) => {
            key.run.digest() == digest || key.full_config.digest() == digest
        }
        CacheNode::Layer(key) => layer_key_contains_digest(key, digest),
        CacheNode::Query(key) => query_key_contains_digest(key, digest),
        CacheNode::Summary(key) => summary_key_contains_digest(key, digest),
        CacheNode::Diagnostic(key) => diagnostic_key_contains_digest(key, digest),
    }
}

fn layer_key_contains_digest(key: &LayerKey, digest: &Digest) -> bool {
    key.parameter_digest == *digest
        || key.lifecycle_digest == *digest
        || key.analysis_settings_digest == *digest
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
        DependencyIndex, DependencyKind, DiagnosticKey, Digest, DigestKind, InputComponentStatus,
        InputDependencyKey, LayerKey, ShapeKind,
    };

    fn digest(kind: DigestKind, label: &str) -> Digest {
        Digest::from_parts(kind, label, &[label])
    }

    fn source(stable_key: &str, digest: Digest) -> CacheNode {
        CacheNode::DependencyInput(
            InputDependencyKey::source_file(stable_key, digest, InputComponentStatus::Present)
                .expect("source dependency uses a source-text digest"),
        )
    }

    fn lifecycle(stable_key: &str, digest: Digest) -> CacheNode {
        CacheNode::DependencyInput(
            InputDependencyKey::language_lifecycle(
                stable_key,
                digest,
                InputComponentStatus::Present,
            )
            .expect("lifecycle dependency uses a language-lifecycle digest"),
        )
    }

    fn provider(stable_key: &str, digest: Digest) -> CacheNode {
        CacheNode::DependencyInput(
            InputDependencyKey::provider_manifest(
                stable_key,
                digest,
                InputComponentStatus::Present,
            )
            .expect("provider dependency uses a provider-manifest digest"),
        )
    }

    fn rule_dependency(stable_key: &str, digest: Digest) -> CacheNode {
        CacheNode::DependencyInput(
            InputDependencyKey::rule_code(stable_key, digest, InputComponentStatus::Present)
                .expect("rule dependency uses a rule-code digest"),
        )
    }

    fn diagnostic(rule_id: &str, rule_digest: Digest) -> CacheNode {
        CacheNode::Diagnostic(DiagnosticKey::new(
            rule_id,
            "1",
            rule_digest,
            Digest::absent(DigestKind::RuleOptions, "none"),
            Vec::new(),
            Digest::absent(DigestKind::Evidence, "none"),
        ))
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
            Digest::absent(DigestKind::AnalysisSettings, "none"),
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
        let source_digest = digest(DigestKind::SourceText, "a");
        let source = source("src/a.ts", source_digest.clone());
        let layer = CacheNode::Layer(layer("polint.ts.syntax", "a", None));
        let index = DependencyIndex::from_edges(vec![edge(
            layer.clone(),
            source.clone(),
            DependencyKind::Input,
            ShapeKind::Content,
        )]);

        let plan = InvalidationPlan::from_change_set(
            &index,
            &change(source, ChangeKind::Unknown, source_digest),
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
    fn stale_index_schema_labels_drop_instead_of_reusing() {
        let source_digest = digest(DigestKind::SourceText, "a");
        let source = source("src/a.ts", source_digest.clone());
        let layer = CacheNode::Layer(layer("polint.ts.syntax", "a", None));
        for stale_schema in [
            "polint-dependency-index-1",
            "polint-dependency-index-next-typed",
            "polint-dependency-index-next-query-inputs",
            "polint-dependency-index-unknown",
            "polint-dependency-index-3",
        ] {
            let mut index = DependencyIndex::from_edges(vec![edge(
                layer.clone(),
                source.clone(),
                DependencyKind::Input,
                ShapeKind::Content,
            )]);
            index.schema_version = stale_schema.to_string();

            let plan = InvalidationPlan::from_change_set(
                &index,
                &change(
                    source.clone(),
                    ChangeKind::ContentOnly,
                    source_digest.clone(),
                ),
            );

            assert!(
                matches!(
                    plan.action_for(&layer),
                    Some(InvalidationAction::Drop(
                        _,
                        DropReason::DependencyIndexSchemaMismatch
                    ))
                ),
                "stale schema `{stale_schema}` must conservatively drop reuse"
            );
        }
    }

    #[test]
    fn missing_dependency_target_recomputes_instead_of_reusing() {
        let missing_digest = digest(DigestKind::SourceText, "missing");
        let missing = source("src/missing.ts", missing_digest.clone());
        let index = DependencyIndex::from_edges(Vec::new());

        let plan = InvalidationPlan::from_change_set(
            &index,
            &change(missing.clone(), ChangeKind::ContentOnly, missing_digest),
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
        let source_a_digest = digest(DigestKind::SourceText, "a");
        let source_a = source("src/a.ts", source_a_digest.clone());
        let source_b = source("src/b.ts", digest(DigestKind::SourceText, "b"));
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
            &change(source_a, ChangeKind::ContentOnly, source_a_digest),
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
        let lifecycle_digest = digest(DigestKind::GoLifecycle, "go");
        let lifecycle = lifecycle("go-lifecycle", lifecycle_digest.clone());
        let provider_digest = digest(DigestKind::ProviderManifest, "provider");
        let provider = provider("polint.go.syntax", provider_digest.clone());
        let layer = CacheNode::Layer(layer("polint.go.syntax", "a", None));
        let index = DependencyIndex::from_edges(vec![
            edge(
                layer.clone(),
                lifecycle.clone(),
                DependencyKind::Lifecycle,
                ShapeKind::Lifecycle,
            ),
            edge(
                layer.clone(),
                provider.clone(),
                DependencyKind::Provider,
                ShapeKind::ProviderVersion,
            ),
        ]);

        let lifecycle_plan = InvalidationPlan::from_change_set(
            &index,
            &change(lifecycle, ChangeKind::Lifecycle, lifecycle_digest),
        );
        let provider_plan = InvalidationPlan::from_change_set(
            &index,
            &change(provider, ChangeKind::ProviderVersion, provider_digest),
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
    fn rule_changes_recompute_only_linked_diagnostics() {
        let rule_digest = digest(DigestKind::RuleCode, "rule");
        let other_rule_digest = digest(DigestKind::RuleCode, "other-rule");
        let rule = rule_dependency("local/example", rule_digest.clone());
        let other_rule = rule_dependency("local/other", other_rule_digest.clone());
        let affected = diagnostic("local/example", rule_digest.clone());
        let unaffected = diagnostic("local/other", other_rule_digest);
        let index = DependencyIndex::from_edges(vec![
            edge(
                affected.clone(),
                rule.clone(),
                DependencyKind::Rule,
                ShapeKind::RuleCode,
            ),
            edge(
                unaffected.clone(),
                other_rule,
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
    fn dependency_index_schema_constant_matches_default_index() {
        assert_eq!(
            DependencyIndex::default().schema_version,
            DEPENDENCY_INDEX_SCHEMA
        );
    }

    #[test]
    fn mismatched_typed_change_digest_fails_closed_for_the_whole_index() {
        let source = source("src/a.ts", digest(DigestKind::SourceText, "a"));
        let layer = CacheNode::Layer(layer("polint.ts.syntax", "a", None));
        let index = DependencyIndex::from_edges(vec![edge(
            layer.clone(),
            source.clone(),
            DependencyKind::Input,
            ShapeKind::Content,
        )]);
        let invalid = ChangeSet {
            rows: vec![ChangeSetRow {
                node: source,
                kind: ChangeKind::ContentOnly,
                digest: digest(DigestKind::SourceText, "different"),
            }],
        };

        let plan = InvalidationPlan::from_change_set(&index, &invalid);

        assert!(matches!(
            plan.action_for(&layer),
            Some(InvalidationAction::Recompute(
                _,
                RecomputeReason::UnknownChange
            ))
        ));
    }
}
