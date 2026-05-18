use crate::core::{
    Capabilities, CapabilitySupportStatus, CapabilitySupportView, Language, RuleMeta, RuleOptions,
};
use crate::diagnostics::{PolintToolInfo, Severity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const RULE_MANIFEST_INTERNAL_SCHEMA: &str = "polint-rule-manifest-internal-1";
pub(crate) const POLINT_RULE_INSPECT_JSON_SCHEMA_V1_URL: &str = "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-rule-inspect-v1.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RuleManifest {
    pub(crate) schema_version: String,
    pub(crate) id: String,
    pub(crate) description: String,
    pub(crate) severity: Severity,
    pub(crate) fact_views: Vec<FactViewRequirement>,
    pub(crate) capabilities: Vec<CapabilityRequirement>,
    pub(crate) options: RuleOptionsManifest,
    pub(crate) sdk_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct FactViewRequirement {
    pub(crate) view_type: String,
    pub(crate) canonical_path: String,
    pub(crate) capability: String,
    pub(crate) parameter_name: String,
}

impl FactViewRequirement {
    pub(crate) fn generated(
        view_type: &'static str,
        canonical_path: &'static str,
        capability: &'static str,
        parameter_name: &'static str,
    ) -> Self {
        Self {
            view_type: view_type.to_string(),
            canonical_path: canonical_path.to_string(),
            capability: capability.to_string(),
            parameter_name: parameter_name.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CapabilityRequirement {
    pub(crate) name: String,
    pub(crate) derived_from_fact_views: Vec<String>,
    pub(crate) status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RuleOptionsManifest {
    pub(crate) common_fields: Vec<OptionFieldManifest>,
    pub(crate) custom_settings: Vec<String>,
    pub(crate) typed_schema: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OptionFieldManifest {
    pub(crate) name: String,
    pub(crate) configured: bool,
}

impl RuleManifest {
    pub(crate) fn from_parts(
        meta: RuleMeta,
        capabilities: Capabilities,
        mut fact_views: Vec<FactViewRequirement>,
        options: Option<&RuleOptions>,
    ) -> Self {
        fact_views.sort();

        let mut views_by_capability = BTreeMap::<String, Vec<String>>::new();
        for fact_view in &fact_views {
            views_by_capability
                .entry(fact_view.capability.clone())
                .or_default()
                .push(fact_view.view_type.clone());
        }
        for view_types in views_by_capability.values_mut() {
            view_types.sort();
            view_types.dedup();
        }

        let mut capability_rows = capabilities
            .requested_names()
            .map(|name| CapabilityRequirement {
                name: name.to_string(),
                derived_from_fact_views: views_by_capability.remove(name).unwrap_or_default(),
                status: "requested".to_string(),
            })
            .collect::<Vec<_>>();
        capability_rows.sort_by(|left, right| left.name.cmp(&right.name));

        Self {
            schema_version: RULE_MANIFEST_INTERNAL_SCHEMA.to_string(),
            id: meta.id,
            description: meta.description,
            severity: meta.severity,
            fact_views,
            capabilities: capability_rows,
            options: RuleOptionsManifest::from_options(options),
            sdk_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

impl RuleOptionsManifest {
    fn from_options(options: Option<&RuleOptions>) -> Self {
        let common_fields = vec![
            option_field(
                "severity",
                options.is_some_and(|options| options.severity.is_some()),
            ),
            option_field(
                "files",
                options.is_some_and(|options| !options.files.is_empty()),
            ),
            option_field(
                "allow_files",
                options.is_some_and(|options| !options.allow_files.is_empty()),
            ),
            option_field(
                "allow",
                options.is_some_and(|options| !options.allow.is_empty()),
            ),
            option_field("max", options.is_some_and(|options| options.max.is_some())),
            option_field(
                "deny",
                options.is_some_and(|options| !options.deny.is_empty()),
            ),
            option_field(
                "forbidden_imports",
                options.is_some_and(|options| !options.forbidden_imports.is_empty()),
            ),
            option_field(
                "settings",
                options.is_some_and(|options| !options.settings.is_empty()),
            ),
        ];
        let custom_settings = options
            .map(|options| options.settings.keys().cloned().collect())
            .unwrap_or_default();
        Self {
            common_fields,
            custom_settings,
            typed_schema: None,
        }
    }
}

fn option_field(name: &str, configured: bool) -> OptionFieldManifest {
    OptionFieldManifest {
        name: name.to_string(),
        configured,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct InspectRuleReport {
    pub(crate) version: u32,
    pub(crate) schema: String,
    pub(crate) tool: PolintToolInfo,
    pub(crate) rules: Vec<RuleManifestWire>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RuleManifestWire {
    pub(crate) rule_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) host_path: Option<String>,
    pub(crate) description: String,
    pub(crate) severity: Severity,
    pub(crate) sdk_version: String,
    pub(crate) fact_views: Vec<FactViewRequirementWire>,
    pub(crate) capabilities: Vec<CapabilityRequirementWire>,
    pub(crate) options: RuleOptionsManifestWire,
    pub(crate) capability_support: Vec<CapabilitySupportWire>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FactViewRequirementWire {
    pub(crate) view_type: String,
    pub(crate) canonical_path: String,
    pub(crate) capability: String,
    pub(crate) parameter_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CapabilityRequirementWire {
    pub(crate) name: String,
    pub(crate) derived_from_fact_views: Vec<String>,
    pub(crate) status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RuleOptionsManifestWire {
    pub(crate) common_fields: Vec<OptionFieldManifest>,
    pub(crate) custom_settings: Vec<String>,
    pub(crate) typed_schema: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CapabilitySupportWire {
    pub(crate) capability: String,
    pub(crate) language: Option<Language>,
    pub(crate) status: String,
    pub(crate) rules: Vec<String>,
    pub(crate) reason: Option<String>,
    pub(crate) hint: Option<String>,
    pub(crate) docs_path: Option<String>,
}

impl InspectRuleReport {
    pub(crate) fn new(
        tool_name: impl Into<String>,
        tool_version: impl Into<String>,
        mut rules: Vec<RuleManifestWire>,
    ) -> Self {
        rules.sort_by(|left, right| {
            (
                left.rule_id.as_str(),
                left.host_path.as_deref().unwrap_or_default(),
            )
                .cmp(&(
                    right.rule_id.as_str(),
                    right.host_path.as_deref().unwrap_or_default(),
                ))
        });
        Self {
            version: 1,
            schema: POLINT_RULE_INSPECT_JSON_SCHEMA_V1_URL.to_string(),
            tool: PolintToolInfo {
                name: tool_name.into(),
                version: tool_version.into(),
            },
            rules,
        }
    }
}

impl RuleManifestWire {
    pub(crate) fn from_manifest(
        manifest: RuleManifest,
        host_path: Option<String>,
        support_view: &CapabilitySupportView,
    ) -> Self {
        let mut capability_support = support_view
            .entries()
            .iter()
            .filter(|entry| entry.rules.iter().any(|rule| rule == &manifest.id))
            .map(|entry| CapabilitySupportWire {
                capability: entry.capability.clone(),
                language: entry.language,
                status: support_status_json(&entry.status).to_string(),
                rules: entry.rules.clone(),
                reason: entry.reason.clone(),
                hint: entry.hint.clone(),
                docs_path: entry.docs_path.clone(),
            })
            .collect::<Vec<_>>();
        capability_support.sort_by(|left, right| {
            (left.capability.as_str(), left.status.as_str())
                .cmp(&(right.capability.as_str(), right.status.as_str()))
        });

        Self {
            rule_id: manifest.id,
            host_path,
            description: manifest.description,
            severity: manifest.severity,
            sdk_version: manifest.sdk_version,
            fact_views: manifest
                .fact_views
                .into_iter()
                .map(|view| FactViewRequirementWire {
                    view_type: view.view_type,
                    canonical_path: view.canonical_path,
                    capability: view.capability,
                    parameter_name: view.parameter_name,
                })
                .collect(),
            capabilities: manifest
                .capabilities
                .into_iter()
                .map(|capability| CapabilityRequirementWire {
                    name: capability.name,
                    derived_from_fact_views: capability.derived_from_fact_views,
                    status: capability.status,
                })
                .collect(),
            options: RuleOptionsManifestWire {
                common_fields: manifest.options.common_fields,
                custom_settings: manifest.options.custom_settings,
                typed_schema: manifest.options.typed_schema,
            },
            capability_support,
        }
    }

    pub(crate) fn with_host_path(mut self, host_path: String) -> Self {
        self.host_path = Some(host_path);
        self
    }
}

fn support_status_json(status: &CapabilitySupportStatus) -> &'static str {
    match status {
        CapabilitySupportStatus::Supported => "supported",
        CapabilitySupportStatus::Unsupported => "unsupported",
        CapabilitySupportStatus::SetupMissing => "setup_missing",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Severity;
    use toml::Value;

    fn meta() -> RuleMeta {
        RuleMeta {
            id: "local/rule".to_string(),
            description: "Rule description".to_string(),
            severity: Severity::Warn,
        }
    }

    #[test]
    fn manifest_sorts_fact_views_and_capabilities_deterministically() {
        let manifest = RuleManifest::from_parts(
            meta(),
            Capabilities::new().imports().syntax(),
            vec![
                FactViewRequirement::generated(
                    "Imports",
                    "polint::sdk::facts::Imports<'_>",
                    "imports",
                    "imports",
                ),
                FactViewRequirement::generated(
                    "SourceFiles",
                    "polint::sdk::facts::SourceFiles<'_>",
                    "syntax",
                    "files",
                ),
            ],
            None,
        );

        assert_eq!(manifest.schema_version, RULE_MANIFEST_INTERNAL_SCHEMA);
        assert_eq!(
            manifest
                .fact_views
                .iter()
                .map(|view| view.view_type.as_str())
                .collect::<Vec<_>>(),
            ["Imports", "SourceFiles"]
        );
        assert_eq!(
            manifest
                .capabilities
                .iter()
                .map(|capability| capability.name.as_str())
                .collect::<Vec<_>>(),
            ["imports", "syntax"]
        );
        assert_eq!(
            manifest.capabilities[0].derived_from_fact_views,
            ["Imports"]
        );
    }

    #[test]
    fn manifest_reports_resolved_common_options_and_settings_keys() {
        let mut options = RuleOptions {
            severity: Some(Severity::Error),
            files: vec!["src/**".to_string()],
            max: Some(10),
            ..RuleOptions::default()
        };
        options
            .settings
            .insert("owner".to_string(), Value::String("platform".to_string()));

        let manifest =
            RuleManifest::from_parts(meta(), Capabilities::default(), Vec::new(), Some(&options));

        let configured = manifest
            .options
            .common_fields
            .iter()
            .filter(|field| field.configured)
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(configured, ["severity", "files", "max", "settings"]);
        assert_eq!(manifest.options.custom_settings, ["owner"]);
        assert_eq!(manifest.options.typed_schema, None);
    }

    #[test]
    fn inspect_rule_report_sorts_rules_and_uses_stable_top_level_fields() {
        let first = RuleManifestWire::from_manifest(
            RuleManifest::from_parts(
                RuleMeta {
                    id: "local/zeta".to_string(),
                    description: "Zeta".to_string(),
                    severity: Severity::Warn,
                },
                Capabilities::new().imports(),
                vec![FactViewRequirement::generated(
                    "Imports",
                    "polint::sdk::facts::Imports<'_>",
                    "imports",
                    "imports",
                )],
                None,
            ),
            None,
            &CapabilitySupportView::empty(),
        );
        let second = RuleManifestWire::from_manifest(
            RuleManifest::from_parts(
                RuleMeta {
                    id: "local/alpha".to_string(),
                    description: "Alpha".to_string(),
                    severity: Severity::Info,
                },
                Capabilities::default(),
                Vec::new(),
                None,
            ),
            None,
            &CapabilitySupportView::empty(),
        );

        let report = InspectRuleReport::new("polint", "0.0.0", vec![first, second]);
        let json = serde_json::to_string(&report).unwrap();

        assert_eq!(report.rules[0].rule_id, "local/alpha");
        assert_eq!(
            json,
            r#"{"version":1,"schema":"https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-rule-inspect-v1.json","tool":{"name":"polint","version":"0.0.0"},"rules":[{"rule_id":"local/alpha","description":"Alpha","severity":"info","sdk_version":"0.1.13","fact_views":[],"capabilities":[],"options":{"common_fields":[{"name":"severity","configured":false},{"name":"files","configured":false},{"name":"allow_files","configured":false},{"name":"allow","configured":false},{"name":"max","configured":false},{"name":"deny","configured":false},{"name":"forbidden_imports","configured":false},{"name":"settings","configured":false}],"custom_settings":[],"typed_schema":null},"capability_support":[]},{"rule_id":"local/zeta","description":"Zeta","severity":"warn","sdk_version":"0.1.13","fact_views":[{"view_type":"Imports","canonical_path":"polint::sdk::facts::Imports<'_>","capability":"imports","parameter_name":"imports"}],"capabilities":[{"name":"imports","derived_from_fact_views":["Imports"],"status":"requested"}],"options":{"common_fields":[{"name":"severity","configured":false},{"name":"files","configured":false},{"name":"allow_files","configured":false},{"name":"allow","configured":false},{"name":"max","configured":false},{"name":"deny","configured":false},{"name":"forbidden_imports","configured":false},{"name":"settings","configured":false}],"custom_settings":[],"typed_schema":null},"capability_support":[]}]}"#
        );
    }
}
