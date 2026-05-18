use crate::core::{Capabilities, RuleMeta, RuleOptions};
use crate::diagnostics::Severity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const RULE_MANIFEST_INTERNAL_SCHEMA: &str = "polint-rule-manifest-internal-1";

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
}
