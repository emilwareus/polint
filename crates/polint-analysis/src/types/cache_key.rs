use polint_analysis_api::{Digest, DigestKind, InputComponent, InputSnapshot};

pub const TYPE_VALUE_ALIAS_SCHEMA_LABEL: &str = "type-value-alias-facts-1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeValueAliasProviderParameters {
    pub precision_tier: &'static str,
    pub alias_budget: u32,
    pub points_to_max_steps: u32,
    pub points_to_max_objects_per_var: u32,
    pub points_to_max_dynamic_vars: u32,
    pub extension_slot: &'static str,
    pub model_slot: &'static str,
    pub tool_slot: &'static str,
}

impl TypeValueAliasProviderParameters {
    pub fn deterministic_default() -> Self {
        let points_to_budget = crate::points_to::solver::PointsToBudget::default();
        Self {
            precision_tier: "setup-aware",
            alias_budget: crate::aliases::provider_stack::MAX_PROVIDER_STACK_PAIRS as u32,
            points_to_max_steps: points_to_budget.max_steps as u32,
            points_to_max_objects_per_var: points_to_budget.max_objects_per_var as u32,
            points_to_max_dynamic_vars: points_to_budget.max_dynamic_vars as u32,
            extension_slot: "absent",
            model_slot: "absent",
            tool_slot: "absent",
        }
    }
}

pub fn type_value_alias_provider_parameter_digest() -> Digest {
    type_value_alias_provider_parameter_digest_for_settings(
        &TypeValueAliasProviderParameters::deterministic_default(),
    )
}

pub fn type_value_alias_provider_parameter_digest_for_snapshot(
    input_snapshot: &InputSnapshot,
    upstream_output_digests: &[Digest],
) -> Digest {
    let mut parts = parameter_parts(&TypeValueAliasProviderParameters::deterministic_default());
    parts.push(format!("config={}", input_snapshot.config.digest));
    extend_component_parts(
        &mut parts,
        "go_lifecycle",
        &input_snapshot.go_lifecycle.components,
    );
    extend_component_parts(
        &mut parts,
        "ts_js_lifecycle",
        &input_snapshot.ts_js_lifecycle.components,
    );
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);
    parts.extend(
        upstream_output_digests
            .iter()
            .map(|digest| format!("upstream={digest}")),
    );
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "type_value_alias_inputs",
        &refs,
    )
}

fn type_value_alias_provider_parameter_digest_for_settings(
    settings: &TypeValueAliasProviderParameters,
) -> Digest {
    let parts = parameter_parts(settings);
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "type_value_alias_parameters",
        &refs,
    )
}

fn parameter_parts(settings: &TypeValueAliasProviderParameters) -> Vec<String> {
    vec![
        format!("schema={TYPE_VALUE_ALIAS_SCHEMA_LABEL}:1"),
        "output=type_facts".to_string(),
        "output=narrowed_type_facts".to_string(),
        "output=value_facts".to_string(),
        "output=allocation_tokens".to_string(),
        "output=access_paths".to_string(),
        "output=points_to_constraints".to_string(),
        "output=points_to_sets".to_string(),
        "output=alias_answers".to_string(),
        format!("precision_tier={}", settings.precision_tier),
        format!("alias_budget={}", settings.alias_budget),
        format!("points_to_max_steps={}", settings.points_to_max_steps),
        format!(
            "points_to_max_objects_per_var={}",
            settings.points_to_max_objects_per_var
        ),
        format!(
            "points_to_max_dynamic_vars={}",
            settings.points_to_max_dynamic_vars
        ),
        format!("extension_slot={}", settings.extension_slot),
        format!("model_slot={}", settings.model_slot),
        format!("tool_slot={}", settings.tool_slot),
    ]
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    if components.is_empty() {
        parts.push(format!("{prefix}=absent"));
        return;
    }
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

#[cfg(test)]
pub fn type_value_alias_provider_parameter_digest_for_test(
    precision_tier: &'static str,
    alias_budget: u32,
    points_to_budget: u32,
    extension_slot: &'static str,
    model_slot: &'static str,
    tool_slot: &'static str,
) -> Digest {
    type_value_alias_provider_parameter_digest_for_settings(&TypeValueAliasProviderParameters {
        precision_tier,
        alias_budget,
        points_to_max_steps: points_to_budget,
        points_to_max_objects_per_var: TypeValueAliasProviderParameters::deterministic_default()
            .points_to_max_objects_per_var,
        points_to_max_dynamic_vars: TypeValueAliasProviderParameters::deterministic_default()
            .points_to_max_dynamic_vars,
        extension_slot,
        model_slot,
        tool_slot,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use polint_analysis_api::{
        GoLifecycleSnapshot, InputComponentStatus, ProviderSchemaSnapshot, TsJsLifecycleSnapshot,
    };

    #[test]
    fn provider_parameter_digest_changes_for_behavior_inputs() {
        let baseline = type_value_alias_provider_parameter_digest();
        for changed in [
            type_value_alias_provider_parameter_digest_for_test(
                "exact", 0, 0, "absent", "absent", "absent",
            ),
            type_value_alias_provider_parameter_digest_for_test(
                "setup-aware",
                1,
                0,
                "absent",
                "absent",
                "absent",
            ),
            type_value_alias_provider_parameter_digest_for_test(
                "setup-aware",
                0,
                1,
                "absent",
                "absent",
                "absent",
            ),
            type_value_alias_provider_parameter_digest_for_test(
                "setup-aware",
                0,
                0,
                "extension-present",
                "absent",
                "absent",
            ),
            type_value_alias_provider_parameter_digest_for_test(
                "setup-aware",
                0,
                0,
                "absent",
                "model-present",
                "absent",
            ),
            type_value_alias_provider_parameter_digest_for_test(
                "setup-aware",
                0,
                0,
                "absent",
                "absent",
                "tool-present",
            ),
        ] {
            assert_ne!(baseline, changed);
        }
    }

    #[test]
    fn snapshot_digest_changes_for_go_lifecycle_tool_and_upstream_inputs() {
        let baseline_snapshot = snapshot("go-base", "tool-base");
        let changed_go = snapshot("go-changed", "tool-base");
        let changed_tool = snapshot("go-base", "tool-changed");
        let upstream_base = [Digest::from_parts(
            DigestKind::ProviderOutput,
            "semantic_mir",
            &["base"],
        )];
        let upstream_changed = [Digest::from_parts(
            DigestKind::ProviderOutput,
            "semantic_mir",
            &["changed"],
        )];

        let baseline = type_value_alias_provider_parameter_digest_for_snapshot(
            &baseline_snapshot,
            &upstream_base,
        );
        assert_ne!(
            baseline,
            type_value_alias_provider_parameter_digest_for_snapshot(&changed_go, &upstream_base)
        );
        assert_ne!(
            baseline,
            type_value_alias_provider_parameter_digest_for_snapshot(&changed_tool, &upstream_base)
        );
        assert_ne!(
            baseline,
            type_value_alias_provider_parameter_digest_for_snapshot(
                &baseline_snapshot,
                &upstream_changed
            )
        );
    }

    #[test]
    fn snapshot_digest_changes_for_ts_js_lifecycle_inputs() {
        let baseline_snapshot = snapshot_with_ts_js("tsconfig-base", "package-base");
        let changed_tsconfig = snapshot_with_ts_js("tsconfig-changed", "package-base");
        let changed_package = snapshot_with_ts_js("tsconfig-base", "package-changed");
        let upstream = [Digest::from_parts(
            DigestKind::ProviderOutput,
            "semantic_mir",
            &["base"],
        )];

        let baseline =
            type_value_alias_provider_parameter_digest_for_snapshot(&baseline_snapshot, &upstream);
        assert_ne!(
            baseline,
            type_value_alias_provider_parameter_digest_for_snapshot(&changed_tsconfig, &upstream)
        );
        assert_ne!(
            baseline,
            type_value_alias_provider_parameter_digest_for_snapshot(&changed_package, &upstream)
        );
    }

    #[test]
    fn snapshot_digest_changes_for_extension_inputs() {
        let mut baseline_snapshot = snapshot("go-base", "tool-base");
        let mut changed_extension = baseline_snapshot.clone();
        baseline_snapshot.extensions = vec![component(
            "extension.type_precision",
            Digest::from_parts(DigestKind::ExtensionCode, "extension", &["base"]),
        )];
        changed_extension.extensions = vec![component(
            "extension.type_precision",
            Digest::from_parts(DigestKind::ExtensionCode, "extension", &["changed"]),
        )];
        let upstream = [Digest::from_parts(
            DigestKind::ProviderOutput,
            "polint.extensions",
            &["accepted=alias:extension:no_alias"],
        )];

        let baseline =
            type_value_alias_provider_parameter_digest_for_snapshot(&baseline_snapshot, &upstream);
        assert_ne!(
            baseline,
            type_value_alias_provider_parameter_digest_for_snapshot(&changed_extension, &upstream)
        );
    }

    fn snapshot(go_suffix: &str, tool_suffix: &str) -> InputSnapshot {
        let go_component = component(
            "go.tool_invocation",
            Digest::from_parts(DigestKind::GoLifecycle, "go", &[go_suffix]),
        );
        let tool_component = component(
            "go.tool_invocation",
            Digest::from_parts(DigestKind::ToolInvocation, "go", &[tool_suffix]),
        );
        InputSnapshot {
            schema_version: "test".to_string(),
            files: Vec::new(),
            config: component(
                "config",
                Digest::from_parts(DigestKind::Config, "config", &["base"]),
            ),
            go_lifecycle: GoLifecycleSnapshot {
                components: vec![go_component],
            },
            ts_js_lifecycle: TsJsLifecycleSnapshot {
                components: Vec::new(),
            },
            rules: Vec::new(),
            models: Vec::new(),
            extensions: Vec::new(),
            tool_invocations: vec![tool_component],
            provider_schemas: Vec::<ProviderSchemaSnapshot>::new(),
        }
    }

    fn snapshot_with_ts_js(tsconfig_suffix: &str, package_suffix: &str) -> InputSnapshot {
        let mut snapshot = snapshot("go-base", "tool-base");
        snapshot.ts_js_lifecycle = TsJsLifecycleSnapshot {
            components: vec![
                component(
                    "ts_js.config_files",
                    Digest::from_parts(DigestKind::TsJsLifecycle, "tsconfig", &[tsconfig_suffix]),
                ),
                component(
                    "ts_js.package_manifests",
                    Digest::from_parts(DigestKind::TsJsLifecycle, "package", &[package_suffix]),
                ),
            ],
        };
        snapshot
    }

    fn component(name: &str, digest: Digest) -> InputComponent {
        InputComponent {
            name: name.to_string(),
            status: InputComponentStatus::Present,
            digest,
            detail: Vec::new(),
        }
    }
}
