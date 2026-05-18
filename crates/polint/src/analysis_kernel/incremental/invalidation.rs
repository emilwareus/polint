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
