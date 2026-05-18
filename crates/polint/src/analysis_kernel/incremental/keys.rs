#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::incremental::digest::{Digest, DigestKind};
    use crate::cache::{CACHE_VERSION, CacheKey};

    fn digest(label: &str, value: &str) -> Digest {
        Digest::from_parts(DigestKind::SourceText, label, &[value])
    }

    #[test]
    fn layer_key_constructor_sorts_variable_digest_lists() {
        let a = digest("file", "a");
        let b = digest("file", "b");

        let left = LayerKey::new(
            LayerKind::TsSyntax,
            "polint.ts.syntax",
            "1",
            "ts-facts-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::TsJsLifecycle, "none"),
            Digest::absent(DigestKind::Config, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            vec![b.clone(), a.clone()],
            vec![b.clone(), a.clone()],
            vec![b.clone(), a.clone()],
        );
        let right = LayerKey::new(
            LayerKind::TsSyntax,
            "polint.ts.syntax",
            "1",
            "ts-facts-v1",
            Digest::absent(DigestKind::ProviderParameters, "none"),
            Digest::absent(DigestKind::TsJsLifecycle, "none"),
            Digest::absent(DigestKind::Config, "none"),
            Digest::absent(DigestKind::ToolInvocation, "none"),
            vec![a.clone(), b.clone()],
            vec![a.clone(), b.clone()],
            vec![a, b],
        );

        assert_eq!(left, right);
    }

    #[test]
    fn existing_file_cache_bridge_includes_all_cache_key_fields_as_digests() {
        let cache_key = CacheKey {
            file_hash: "file-hash".to_string(),
            config_hash: "config-hash".to_string(),
            rule_hash: "rule-hash".to_string(),
            plan_hash: "plan-hash".to_string(),
            version: CACHE_VERSION.to_string(),
            schema: "ts-facts-v1".to_string(),
        };
        let key = LayerKey::from_existing_file_cache(
            LayerKind::TsSyntax,
            "polint.ts.syntax",
            "1",
            &cache_key,
        );

        assert_eq!(key.config_digest, Digest::from_parts(DigestKind::Config, "config_hash", &["config-hash"]));
        assert!(key.input_digests.contains(&Digest::from_parts(DigestKind::SourceText, "file_hash", &["file-hash"])));
        assert!(key.input_digests.contains(&Digest::from_parts(DigestKind::RuleCode, "rule_hash", &["rule-hash"])));
        assert!(key.input_digests.contains(&Digest::from_parts(DigestKind::ProviderParameters, "plan_hash", &["plan-hash"])));
        assert!(key.input_digests.contains(&Digest::from_parts(DigestKind::ToolInvocation, "version", &[CACHE_VERSION])));
        assert!(key.input_digests.contains(&Digest::from_parts(DigestKind::ProviderOutput, "schema", &["ts-facts-v1"])));
    }

    #[test]
    fn query_summary_and_diagnostic_keys_serialize_snake_case_with_sorted_digest_lists() {
        let a = digest("digest", "a");
        let b = digest("digest", "b");
        let query = QueryKey::new(
            "call_graph",
            "1",
            Digest::absent(DigestKind::QueryParameters, "none"),
            vec![b.clone(), a.clone()],
            Digest::absent(DigestKind::Budget, "none"),
            PrecisionTier::Syntax,
        );
        let summary = SummaryKey::new(
            "function:src/main.rs:main",
            "return_effects",
            "1",
            Digest::absent(DigestKind::SummaryBody, "none"),
            vec![b.clone(), a.clone()],
            Digest::absent(DigestKind::ExtensionCode, "none"),
        );
        let diagnostic = DiagnosticKey::new(
            "local/example",
            "1",
            Digest::absent(DigestKind::RuleCode, "none"),
            Digest::absent(DigestKind::RuleOptions, "none"),
            vec![b, a],
            Digest::absent(DigestKind::Evidence, "none"),
        );

        let query_json = serde_json::to_value(query).expect("query key should serialize");
        let summary_json = serde_json::to_value(summary).expect("summary key should serialize");
        let diagnostic_json =
            serde_json::to_value(diagnostic).expect("diagnostic key should serialize");

        assert!(query_json.get("query_kind").is_some());
        assert!(summary_json.get("callable_stable_key").is_some());
        assert!(diagnostic_json.get("requested_view_digests").is_some());
        assert!(query_json["layer_digests"][0].to_string() < query_json["layer_digests"][1].to_string());
        assert!(summary_json["dependency_summary_digests"][0].to_string() < summary_json["dependency_summary_digests"][1].to_string());
        assert!(diagnostic_json["requested_view_digests"][0].to_string() < diagnostic_json["requested_view_digests"][1].to_string());
    }
}
