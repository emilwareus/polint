//! Cache invalidation digest helpers (config + enabled rules/options).
//!
//! Encoding is deterministic and infallible so cache hashing never relies on panicking serializers.

use crate::config::{
    LoadedConfig, PathContextPair, PathContextsConfig, PolintConfig, ProfileConfig, RuleConfig,
    RuleSection, WorkspaceConfig,
};
use crate::core::{Rule, RuleOptions};
use std::collections::BTreeSet;
use std::sync::Arc;

use super::stable_hash;

/// Stable digest of loaded config fields that can affect analysis or rule output.
///
/// Compared to JSON serialization, cache keys intentionally **invalidate** entries when this
/// format changes (fine for correctness; avoids hidden serde-json key ordering quirks).
pub(crate) fn config_hash(config: &LoadedConfig) -> String {
    let missing = if config.missing { "missing" } else { "loaded" };
    let serialized = deterministic_polint_config(&config.config);
    stable_hash(&[missing, &serialized])
}

pub(crate) fn rule_hash(
    rules: &[Arc<dyn Rule>],
    enabled: Option<&BTreeSet<String>>,
    options: &std::collections::BTreeMap<String, RuleOptions>,
) -> String {
    let mut parts = Vec::new();
    for rule in rules {
        let meta = rule.meta();
        if let Some(enabled) = enabled
            && !enabled
                .iter()
                .any(|pattern| crate::core::rule_id_matches(pattern, &meta.id))
        {
            continue;
        }

        parts.push(format!("rule:{}", meta.id));
        parts.push(format!("description:{}", meta.description));
        parts.push(format!("severity:{}", meta.severity));
        if let Some(opts) = options.get(&meta.id) {
            parts.push(format!("options:{}", deterministic_rule_options(opts)));
        }
    }
    let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    stable_hash(&part_refs)
}

pub(crate) fn deterministic_rule_options(options: &RuleOptions) -> String {
    let forbidden_imports = options
        .forbidden_imports
        .iter()
        .map(|(source, targets)| {
            format!(
                "{}->{}",
                deterministic_string(source),
                deterministic_string_list(targets)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let settings = deterministic_toml_map(&options.settings);
    format!(
        "severity={};files={};allow_files={};allow={};max={};deny={};forbidden_imports={};settings={}",
        options
            .severity
            .map(|severity| severity.to_string())
            .unwrap_or_default(),
        deterministic_string_list(&options.files),
        deterministic_string_list(&options.allow_files),
        deterministic_string_list(&options.allow),
        options.max.map(|max| max.to_string()).unwrap_or_default(),
        deterministic_string_list(&options.deny),
        forbidden_imports,
        settings
    )
}

fn deterministic_polint_config(config: &PolintConfig) -> String {
    [
        deterministic_workspace(&config.workspace),
        deterministic_rules_section(&config.rules),
        deterministic_profiles(&config.profiles),
        format!(
            "languages.go={}",
            deterministic_toml_map(&config.languages.go)
        ),
        format!(
            "languages.ts={}",
            deterministic_toml_map(&config.languages.ts)
        ),
        format!(
            "sarif={}",
            deterministic_string_map(&config.sarif.rule_help_uri)
        ),
        format!(
            "path_contexts={}",
            deterministic_path_contexts(&config.path_contexts)
        ),
    ]
    .join("\x1e")
}

fn deterministic_workspace(workspace: &WorkspaceConfig) -> String {
    format!(
        "workspace:include={}|exclude={}",
        deterministic_string_list(&workspace.include),
        deterministic_string_list(&workspace.exclude)
    )
}

fn deterministic_rules_section(rules: &RuleSection) -> String {
    let mut configs = Vec::new();
    for rc in &rules.config {
        configs.push(deterministic_rule_config(rc));
    }
    format!(
        "rules.paths={}|configs={}",
        deterministic_string_list(&rules.paths),
        configs.join(";")
    )
}

fn deterministic_rule_config(rule: &RuleConfig) -> String {
    format!(
        "id={}|severity={}|files={}|allow_files={}|allow={}|max={}|deny={}|forbidden_imports={}|settings={}",
        deterministic_string(&rule.id),
        rule.severity
            .as_deref()
            .map(deterministic_string)
            .unwrap_or_default(),
        deterministic_string_list(&rule.files),
        deterministic_string_list(&rule.allow_files),
        deterministic_string_list(&rule.allow),
        rule.max.map(|m| m.to_string()).unwrap_or_default(),
        deterministic_string_list(&rule.deny),
        rule.forbidden_imports
            .iter()
            .map(|(src, tgt)| {
                format!(
                    "{}->{}",
                    deterministic_string(src),
                    deterministic_string_list(tgt)
                )
            })
            .collect::<Vec<_>>()
            .join(","),
        deterministic_toml_map(&rule.settings)
    )
}

fn deterministic_profiles(profiles: &std::collections::BTreeMap<String, ProfileConfig>) -> String {
    profiles
        .iter()
        .map(|(name, profile)| {
            format!(
                "profile[{}]={}",
                deterministic_string(name),
                deterministic_string_list(&profile.rules)
            )
        })
        .collect::<Vec<_>>()
        .join(";")
}

fn deterministic_path_contexts(path_contexts: &PathContextsConfig) -> String {
    path_contexts
        .pairs
        .iter()
        .map(deterministic_path_context_pair)
        .collect::<Vec<_>>()
        .join(";")
}

fn deterministic_path_context_pair(pair: &PathContextPair) -> String {
    format!(
        "name={}|left_before_ctx={}|left_after_ctx={}|right_before_ctx={}|right_after_ctx={}",
        deterministic_string(&pair.name),
        deterministic_string(&pair.left_before_ctx),
        deterministic_string(&pair.left_after_ctx),
        deterministic_string(&pair.right_before_ctx),
        deterministic_string(&pair.right_after_ctx)
    )
}

fn deterministic_string_map(map: &std::collections::BTreeMap<String, String>) -> String {
    map.iter()
        .map(|(key, value)| {
            format!(
                "{}={}",
                deterministic_string(key),
                deterministic_string(value)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn deterministic_toml_map(map: &std::collections::BTreeMap<String, toml::Value>) -> String {
    map.iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                deterministic_toml_key(k),
                deterministic_toml_value(v)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn deterministic_toml_key(key: &str) -> String {
    format!("K:{}:{key}", key.len())
}

fn deterministic_string(value: &str) -> String {
    format!("S:{}:{value}", value.len())
}

fn deterministic_toml_value(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => deterministic_string(s),
        toml::Value::Integer(i) => format!("I:{i}"),
        toml::Value::Boolean(b) => format!("B:{b}"),
        toml::Value::Float(f) => format!("F:{}", f.to_bits()),
        toml::Value::Datetime(dt) => format!("D:{dt}"),
        toml::Value::Array(items) => {
            let encoded = items
                .iter()
                .map(deterministic_toml_value)
                .collect::<Vec<_>>()
                .join(",");
            format!("A[{encoded}]")
        }
        toml::Value::Table(t) => {
            let mut keys: Vec<_> = t.keys().cloned().collect();
            keys.sort();
            let inner = keys
                .into_iter()
                .map(|k| {
                    format!(
                        "{}={}",
                        deterministic_toml_key(&k),
                        deterministic_toml_value(&t[&k])
                    )
                })
                .collect::<Vec<_>>()
                .join(";");
            format!("T{{{inner}}}")
        }
    }
}

fn deterministic_string_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| deterministic_string(value))
        .collect::<Vec<_>>()
        .join("|")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_loaded(missing: bool) -> LoadedConfig {
        LoadedConfig {
            root: Default::default(),
            config: PolintConfig::default(),
            missing,
        }
    }

    #[test]
    fn config_hash_stable_for_clone() {
        let loaded = sample_loaded(false);
        assert_eq!(config_hash(&loaded), config_hash(&loaded.clone()));
    }

    #[test]
    fn config_hash_differs_when_include_changes() {
        let baseline = sample_loaded(false);
        let mut modified = baseline.clone();
        modified
            .config
            .workspace
            .exclude
            .push("**/only-in-exclude/**".to_string());
        assert_ne!(config_hash(&baseline), config_hash(&modified));
    }

    #[test]
    fn config_hash_differs_when_missing_flag_changes() {
        let on_disk = sample_loaded(false);
        let missing = sample_loaded(true);
        assert_ne!(config_hash(&on_disk), config_hash(&missing));
    }

    #[test]
    fn config_hash_distinguishes_string_list_boundaries() {
        let mut baseline = sample_loaded(false);
        baseline.config.workspace.include = vec!["src|generated".to_string()];

        let mut modified = sample_loaded(false);
        modified.config.workspace.include = vec!["src".to_string(), "generated".to_string()];

        assert_ne!(config_hash(&baseline), config_hash(&modified));
    }

    #[test]
    fn config_hash_differs_when_sarif_help_uri_changes() {
        let baseline = sample_loaded(false);
        let mut modified = baseline.clone();
        modified.config.sarif.rule_help_uri.insert(
            "local/no-todo".to_string(),
            "https://example.com/no-todo".to_string(),
        );

        assert_ne!(config_hash(&baseline), config_hash(&modified));
    }

    #[test]
    fn config_hash_differs_when_path_contexts_change() {
        let baseline = sample_loaded(false);
        let mut modified = baseline.clone();
        modified.config.path_contexts.pairs.push(PathContextPair {
            name: "service_ports".to_string(),
            left_before_ctx: "services/".to_string(),
            left_after_ctx: "/api".to_string(),
            right_before_ctx: "clients/".to_string(),
            right_after_ctx: "/client".to_string(),
        });

        assert_ne!(config_hash(&baseline), config_hash(&modified));
    }

    #[test]
    fn config_hash_differs_when_rule_custom_settings_change() {
        let mut baseline = sample_loaded(false);
        baseline.config.rules.config.push(RuleConfig {
            id: "local/no-todo".to_string(),
            severity: None,
            files: Vec::new(),
            allow_files: Vec::new(),
            allow: Vec::new(),
            max: None,
            deny: Vec::new(),
            forbidden_imports: Default::default(),
            settings: std::collections::BTreeMap::from([(
                "token".to_string(),
                toml::Value::String("TODO".to_string()),
            )]),
        });

        let mut modified = baseline.clone();
        modified.config.rules.config[0].settings.insert(
            "token".to_string(),
            toml::Value::String("FIXME".to_string()),
        );

        assert_ne!(config_hash(&baseline), config_hash(&modified));
    }

    #[test]
    fn deterministic_rule_options_includes_custom_settings() {
        let baseline = RuleOptions::default();
        let mut modified = RuleOptions::default();
        modified.settings.insert(
            "message".to_string(),
            toml::Value::String("use design tokens".to_string()),
        );

        assert_ne!(
            deterministic_rule_options(&baseline),
            deterministic_rule_options(&modified)
        );
    }

    #[test]
    fn deterministic_rule_options_distinguishes_string_list_boundaries() {
        let baseline = RuleOptions {
            files: vec!["src|generated".to_string()],
            ..RuleOptions::default()
        };
        let modified = RuleOptions {
            files: vec!["src".to_string(), "generated".to_string()],
            ..RuleOptions::default()
        };

        assert_ne!(
            deterministic_rule_options(&baseline),
            deterministic_rule_options(&modified)
        );
    }

    #[test]
    fn deterministic_toml_map_distinguishes_keys_with_delimiters() {
        let with_delimited_key = std::collections::BTreeMap::from([(
            "a=b,c".to_string(),
            toml::Value::String("value".to_string()),
        )]);
        let with_plain_key = std::collections::BTreeMap::from([(
            "a".to_string(),
            toml::Value::String("b,c=value".to_string()),
        )]);

        assert_ne!(
            deterministic_toml_map(&with_delimited_key),
            deterministic_toml_map(&with_plain_key)
        );
    }
}
