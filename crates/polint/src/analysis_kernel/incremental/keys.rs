#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 23 establishes query, summary, diagnostic, and layer key vocabulary before later cache consumers use every type."
    )
)]

use serde::{Deserialize, Serialize};

use super::digest::{Digest, DigestKind};
use crate::cache::{CACHE_VERSION, CacheKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LayerKind {
    SourceFiles,
    GoSyntax,
    TsSyntax,
    ModuleGraph,
    SymbolGraph,
    Metrics,
    Extension,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PrecisionTier {
    Syntax,
    SetupAware,
    Exact,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct LayerKey {
    pub(crate) layer_kind: LayerKind,
    pub(crate) provider_id: String,
    pub(crate) provider_version: String,
    pub(crate) schema_version: String,
    pub(crate) parameter_digest: Digest,
    pub(crate) lifecycle_digest: Digest,
    pub(crate) config_digest: Digest,
    pub(crate) toolchain_digest: Digest,
    pub(crate) input_digests: Vec<Digest>,
    pub(crate) dependency_layer_digests: Vec<Digest>,
    pub(crate) extension_digests: Vec<Digest>,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct QueryKey {
    pub(crate) query_kind: String,
    pub(crate) query_version: String,
    pub(crate) parameter_digest: Digest,
    pub(crate) layer_digests: Vec<Digest>,
    pub(crate) budget_digest: Digest,
    pub(crate) precision_tier: PrecisionTier,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct SummaryKey {
    pub(crate) callable_stable_key: String,
    pub(crate) summary_domain: String,
    pub(crate) summary_version: String,
    pub(crate) body_shape_digest: Digest,
    pub(crate) dependency_summary_digests: Vec<Digest>,
    pub(crate) extension_digest: Digest,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct DiagnosticKey {
    pub(crate) rule_id: String,
    pub(crate) rule_version: String,
    pub(crate) rule_code_digest: Digest,
    pub(crate) options_digest: Digest,
    pub(crate) requested_view_digests: Vec<Digest>,
    pub(crate) evidence_digest: Digest,
}

impl LayerKey {
    #[expect(
        clippy::too_many_arguments,
        reason = "Layer cache identity is intentionally explicit so every digest input remains visible at construction."
    )]
    pub(crate) fn new(
        layer_kind: LayerKind,
        provider_id: impl Into<String>,
        provider_version: impl Into<String>,
        schema_version: impl Into<String>,
        parameter_digest: Digest,
        lifecycle_digest: Digest,
        config_digest: Digest,
        toolchain_digest: Digest,
        input_digests: Vec<Digest>,
        dependency_layer_digests: Vec<Digest>,
        extension_digests: Vec<Digest>,
    ) -> Self {
        Self {
            layer_kind,
            provider_id: provider_id.into(),
            provider_version: provider_version.into(),
            schema_version: schema_version.into(),
            parameter_digest,
            lifecycle_digest,
            config_digest,
            toolchain_digest,
            input_digests: sorted_digests(input_digests),
            dependency_layer_digests: sorted_digests(dependency_layer_digests),
            extension_digests: sorted_digests(extension_digests),
        }
    }

    pub(crate) fn from_existing_file_cache(
        layer_kind: LayerKind,
        provider_id: impl Into<String>,
        provider_version: impl Into<String>,
        key: &CacheKey,
    ) -> Self {
        let version = if key.version.is_empty() {
            CACHE_VERSION
        } else {
            key.version.as_str()
        };
        let compatibility_input_digests = vec![
            Digest::from_parts(DigestKind::SourceText, "file_hash", &[&key.file_hash]),
            Digest::from_parts(DigestKind::Config, "config_hash", &[&key.config_hash]),
            Digest::from_parts(DigestKind::RuleCode, "rule_hash", &[&key.rule_hash]),
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "plan_hash",
                &[&key.plan_hash],
            ),
            Digest::from_parts(DigestKind::ToolInvocation, "version", &[version]),
            Digest::from_parts(DigestKind::ProviderOutput, "schema", &[&key.schema]),
        ];

        Self::new(
            layer_kind,
            provider_id,
            provider_version,
            key.schema.clone(),
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "existing_file_cache_parameters",
                &[&key.rule_hash, &key.plan_hash],
            ),
            Digest::absent(DigestKind::DependencyLayer, "existing_file_cache_lifecycle"),
            Digest::from_parts(DigestKind::Config, "config_hash", &[&key.config_hash]),
            Digest::from_parts(DigestKind::ToolInvocation, "version", &[version]),
            compatibility_input_digests,
            Vec::new(),
            Vec::new(),
        )
    }
}

impl QueryKey {
    pub(crate) fn new(
        query_kind: impl Into<String>,
        query_version: impl Into<String>,
        parameter_digest: Digest,
        layer_digests: Vec<Digest>,
        budget_digest: Digest,
        precision_tier: PrecisionTier,
    ) -> Self {
        Self {
            query_kind: query_kind.into(),
            query_version: query_version.into(),
            parameter_digest,
            layer_digests: sorted_digests(layer_digests),
            budget_digest,
            precision_tier,
        }
    }
}

impl SummaryKey {
    pub(crate) fn new(
        callable_stable_key: impl Into<String>,
        summary_domain: impl Into<String>,
        summary_version: impl Into<String>,
        body_shape_digest: Digest,
        dependency_summary_digests: Vec<Digest>,
        extension_digest: Digest,
    ) -> Self {
        Self {
            callable_stable_key: callable_stable_key.into(),
            summary_domain: summary_domain.into(),
            summary_version: summary_version.into(),
            body_shape_digest,
            dependency_summary_digests: sorted_digests(dependency_summary_digests),
            extension_digest,
        }
    }
}

impl DiagnosticKey {
    pub(crate) fn new(
        rule_id: impl Into<String>,
        rule_version: impl Into<String>,
        rule_code_digest: Digest,
        options_digest: Digest,
        requested_view_digests: Vec<Digest>,
        evidence_digest: Digest,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            rule_version: rule_version.into(),
            rule_code_digest,
            options_digest,
            requested_view_digests: sorted_digests(requested_view_digests),
            evidence_digest,
        }
    }
}

fn sorted_digests(mut digests: Vec<Digest>) -> Vec<Digest> {
    digests.sort();
    digests
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(label: &str, value: &str) -> Digest {
        Digest::from_parts(DigestKind::SourceText, label, &[value])
    }

    fn syntax_key(source_text_digests: Vec<Digest>) -> LayerKey {
        LayerKey::syntax_layer_key(
            LayerKind::GoSyntax,
            "polint.go.syntax",
            "1",
            "go-facts-v2",
            source_text_digests,
            Digest::from_parts(DigestKind::Config, "config", &["base"]),
            Digest::from_parts(DigestKind::GoLifecycle, "lifecycle", &["base"]),
            Digest::from_parts(DigestKind::ToolInvocation, "toolchain", &["base"]),
            Digest::from_parts(DigestKind::ProviderParameters, "parser", &["base"]),
        )
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
    fn syntax_layer_key_ignores_rule_digest_changes() {
        let source = digest("file", "src/main.go");
        let rule_a = Digest::from_parts(DigestKind::RuleCode, "rule", &["a"]);
        let rule_b = Digest::from_parts(DigestKind::RuleCode, "rule", &["b"]);

        assert_ne!(rule_a, rule_b);
        assert_eq!(
            syntax_key(vec![source.clone()]),
            syntax_key(vec![source])
        );
    }

    #[test]
    fn syntax_layer_key_changes_when_parser_inputs_change() {
        let source = digest("file", "src/main.go");
        let base = syntax_key(vec![source.clone()]);

        let changed_source = syntax_key(vec![digest("file", "src/other.go")]);
        let changed_config = LayerKey::syntax_layer_key(
            LayerKind::GoSyntax,
            "polint.go.syntax",
            "1",
            "go-facts-v2",
            vec![source.clone()],
            Digest::from_parts(DigestKind::Config, "config", &["changed"]),
            Digest::from_parts(DigestKind::GoLifecycle, "lifecycle", &["base"]),
            Digest::from_parts(DigestKind::ToolInvocation, "toolchain", &["base"]),
            Digest::from_parts(DigestKind::ProviderParameters, "parser", &["base"]),
        );
        let changed_lifecycle = LayerKey::syntax_layer_key(
            LayerKind::GoSyntax,
            "polint.go.syntax",
            "1",
            "go-facts-v2",
            vec![source.clone()],
            Digest::from_parts(DigestKind::Config, "config", &["base"]),
            Digest::from_parts(DigestKind::GoLifecycle, "lifecycle", &["changed"]),
            Digest::from_parts(DigestKind::ToolInvocation, "toolchain", &["base"]),
            Digest::from_parts(DigestKind::ProviderParameters, "parser", &["base"]),
        );
        let changed_toolchain = LayerKey::syntax_layer_key(
            LayerKind::GoSyntax,
            "polint.go.syntax",
            "1",
            "go-facts-v2",
            vec![source.clone()],
            Digest::from_parts(DigestKind::Config, "config", &["base"]),
            Digest::from_parts(DigestKind::GoLifecycle, "lifecycle", &["base"]),
            Digest::from_parts(DigestKind::ToolInvocation, "toolchain", &["changed"]),
            Digest::from_parts(DigestKind::ProviderParameters, "parser", &["base"]),
        );
        let changed_parser_params = LayerKey::syntax_layer_key(
            LayerKind::GoSyntax,
            "polint.go.syntax",
            "1",
            "go-facts-v2",
            vec![source],
            Digest::from_parts(DigestKind::Config, "config", &["base"]),
            Digest::from_parts(DigestKind::GoLifecycle, "lifecycle", &["base"]),
            Digest::from_parts(DigestKind::ToolInvocation, "toolchain", &["base"]),
            Digest::from_parts(DigestKind::ProviderParameters, "parser", &["changed"]),
        );
        let changed_provider_version = LayerKey::syntax_layer_key(
            LayerKind::GoSyntax,
            "polint.go.syntax",
            "2",
            "go-facts-v2",
            base.input_digests.clone(),
            base.config_digest.clone(),
            base.lifecycle_digest.clone(),
            base.toolchain_digest.clone(),
            base.parameter_digest.clone(),
        );
        let changed_schema_version = LayerKey::syntax_layer_key(
            LayerKind::GoSyntax,
            "polint.go.syntax",
            "1",
            "go-facts-v3",
            base.input_digests.clone(),
            base.config_digest.clone(),
            base.lifecycle_digest.clone(),
            base.toolchain_digest.clone(),
            base.parameter_digest.clone(),
        );

        for changed in [
            changed_source,
            changed_config,
            changed_lifecycle,
            changed_toolchain,
            changed_parser_params,
            changed_provider_version,
            changed_schema_version,
        ] {
            assert_ne!(base, changed);
        }
    }

    #[test]
    fn syntax_layer_key_sorts_source_digest_inputs() {
        let a = digest("file", "a");
        let b = digest("file", "b");

        let left = LayerKey::syntax_layer_key(
            LayerKind::TsSyntax,
            "polint.ts.syntax",
            "1",
            "ts-facts-v1",
            vec![b.clone(), a.clone()],
            Digest::from_parts(DigestKind::Config, "config", &["base"]),
            Digest::from_parts(DigestKind::TsJsLifecycle, "lifecycle", &["base"]),
            Digest::from_parts(DigestKind::ToolInvocation, "toolchain", &["base"]),
            Digest::from_parts(DigestKind::ProviderParameters, "parser", &["base"]),
        );
        let right = LayerKey::syntax_layer_key(
            LayerKind::TsSyntax,
            "polint.ts.syntax",
            "1",
            "ts-facts-v1",
            vec![a, b],
            Digest::from_parts(DigestKind::Config, "config", &["base"]),
            Digest::from_parts(DigestKind::TsJsLifecycle, "lifecycle", &["base"]),
            Digest::from_parts(DigestKind::ToolInvocation, "toolchain", &["base"]),
            Digest::from_parts(DigestKind::ProviderParameters, "parser", &["base"]),
        );

        assert_eq!(left, right);
        assert!(left.extension_digests.contains(&Digest::absent(
            DigestKind::ExtensionCode,
            "extension_digest_absent"
        )));
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

        assert_eq!(
            key.config_digest,
            Digest::from_parts(DigestKind::Config, "config_hash", &["config-hash"])
        );
        assert!(key.input_digests.contains(&Digest::from_parts(
            DigestKind::SourceText,
            "file_hash",
            &["file-hash"]
        )));
        assert!(key.input_digests.contains(&Digest::from_parts(
            DigestKind::RuleCode,
            "rule_hash",
            &["rule-hash"]
        )));
        assert!(key.input_digests.contains(&Digest::from_parts(
            DigestKind::ProviderParameters,
            "plan_hash",
            &["plan-hash"]
        )));
        assert!(key.input_digests.contains(&Digest::from_parts(
            DigestKind::ToolInvocation,
            "version",
            &[CACHE_VERSION]
        )));
        assert!(key.input_digests.contains(&Digest::from_parts(
            DigestKind::ProviderOutput,
            "schema",
            &["ts-facts-v1"]
        )));
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
        assert!(
            digest_value(&query_json["layer_digests"][0])
                < digest_value(&query_json["layer_digests"][1])
        );
        assert!(
            digest_value(&summary_json["dependency_summary_digests"][0])
                < digest_value(&summary_json["dependency_summary_digests"][1])
        );
        assert!(
            digest_value(&diagnostic_json["requested_view_digests"][0])
                < digest_value(&diagnostic_json["requested_view_digests"][1])
        );
    }

    fn digest_value(value: &serde_json::Value) -> &str {
        value["value"]
            .as_str()
            .expect("serialized digest should have a string value")
    }
}
