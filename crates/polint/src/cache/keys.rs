//! Cache invalidation digest helpers (config + enabled rules/options).
//!
//! Encoding is deterministic and infallible so cache hashing never relies on panicking serializers.

use crate::config::{
    LoadedConfig, PolintConfig, ProfileConfig, RuleConfig, RuleSection, WorkspaceConfig,
};
use crate::core::{Rule, RuleOptions};
use std::collections::BTreeSet;
use std::sync::Arc;

use super::stable_hash;

/// Stable digest of workspace config affecting discovery/parsing caches.
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
        .map(|(source, targets)| format!("{source}->{}", targets.join("|")))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "severity={};files={};allow_files={};allow={};max={};deny={};forbidden_imports={}",
        options
            .severity
            .map(|severity| severity.to_string())
            .unwrap_or_default(),
        options.files.join("|"),
        options.allow_files.join("|"),
        options.allow.join("|"),
        options.max.map(|max| max.to_string()).unwrap_or_default(),
        options.deny.join("|"),
        forbidden_imports
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
    ]
    .join("\x1e")
}

fn deterministic_workspace(workspace: &WorkspaceConfig) -> String {
    format!(
        "workspace:include={}|exclude={}",
        join_pipe(&workspace.include),
        join_pipe(&workspace.exclude)
    )
}

fn deterministic_rules_section(rules: &RuleSection) -> String {
    let mut configs = Vec::new();
    for rc in &rules.config {
        configs.push(deterministic_rule_config(rc));
    }
    format!(
        "rules.paths={}|configs={}",
        join_pipe(&rules.paths),
        configs.join(";")
    )
}

fn deterministic_rule_config(rule: &RuleConfig) -> String {
    format!(
        "id={}|severity={}|files={}|allow_files={}|allow={}|max={}|deny={}|forbidden_imports={}",
        rule.id,
        rule.severity.clone().unwrap_or_default(),
        join_pipe(&rule.files),
        join_pipe(&rule.allow_files),
        join_pipe(&rule.allow),
        rule.max.map(|m| m.to_string()).unwrap_or_default(),
        join_pipe(&rule.deny),
        rule.forbidden_imports
            .iter()
            .map(|(src, tgt)| format!("{src}->{join}", join = tgt.join("|")))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn deterministic_profiles(profiles: &std::collections::BTreeMap<String, ProfileConfig>) -> String {
    profiles
        .iter()
        .map(|(name, profile)| format!("profile[{name}]={}", join_pipe(&profile.rules)))
        .collect::<Vec<_>>()
        .join(";")
}

fn deterministic_toml_map(map: &std::collections::BTreeMap<String, toml::Value>) -> String {
    map.iter()
        .map(|(k, v)| format!("{k}={}", deterministic_toml_value(v)))
        .collect::<Vec<_>>()
        .join(",")
}

fn deterministic_toml_value(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => format!("S:{}:{}", s.len(), s),
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
                .map(|k| format!("{}={}", k, deterministic_toml_value(&t[&k])))
                .collect::<Vec<_>>()
                .join(";");
            format!("T{{{inner}}}")
        }
    }
}

fn join_pipe(values: &[String]) -> String {
    values.join("|")
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
}
