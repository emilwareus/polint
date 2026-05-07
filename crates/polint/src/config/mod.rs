use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum ConfigError {
    #[error("invalid glob `{glob}`: {source}")]
    InvalidGlob {
        glob: String,
        source: globset::Error,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct PolintConfig {
    #[serde(default)]
    pub(crate) workspace: WorkspaceConfig,
    #[serde(default)]
    pub(crate) rules: RuleSection,
    #[serde(default)]
    pub(crate) profiles: BTreeMap<String, ProfileConfig>,
    #[serde(default)]
    pub(crate) languages: LanguageConfig,
    #[serde(default)]
    pub(crate) sarif: SarifConfig,
    #[serde(default)]
    pub(crate) path_contexts: PathContextsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorkspaceConfig {
    #[serde(default = "default_include")]
    pub(crate) include: Vec<String>,
    #[serde(default = "default_exclude")]
    pub(crate) exclude: Vec<String>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            include: default_include(),
            exclude: default_exclude(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuleSection {
    #[serde(default = "default_rule_paths")]
    pub(crate) paths: Vec<String>,
    #[serde(default)]
    pub(crate) config: Vec<RuleConfig>,
}

impl Default for RuleSection {
    fn default() -> Self {
        Self {
            paths: default_rule_paths(),
            config: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct RuleConfig {
    pub(crate) id: String,
    #[serde(default)]
    pub(crate) severity: Option<String>,
    #[serde(default)]
    pub(crate) files: Vec<String>,
    #[serde(default)]
    pub(crate) allow_files: Vec<String>,
    #[serde(default)]
    pub(crate) allow: Vec<String>,
    #[serde(default)]
    pub(crate) max: Option<u32>,
    #[serde(default)]
    pub(crate) deny: Vec<String>,
    #[serde(default)]
    pub(crate) forbidden_imports: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ProfileConfig {
    #[serde(default)]
    pub(crate) rules: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct LanguageConfig {
    #[serde(default)]
    pub(crate) go: BTreeMap<String, toml::Value>,
    #[serde(default)]
    pub(crate) ts: BTreeMap<String, toml::Value>,
}

/// Optional SARIF enrichment for GitHub Code Scanning and similar tools.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SarifConfig {
    /// Map `rule_id` → help URI (`reportingDescriptor.helpUri`).
    #[serde(default)]
    pub(crate) rule_help_uri: BTreeMap<String, String>,
}

/// Optional path pairing: same context segment between `left_*` and `right_*` path shapes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct PathContextsConfig {
    #[serde(default)]
    pub(crate) pairs: Vec<PathContextPair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PathContextPair {
    pub(crate) name: String,
    pub(crate) left_before_ctx: String,
    pub(crate) left_after_ctx: String,
    pub(crate) right_before_ctx: String,
    pub(crate) right_after_ctx: String,
}

/// Loaded `.polint.toml` (path + parsed config). Fields are crate-private; this type exists so
/// `load_config` and `polint::_bench::analysis_keys` can use it in public `bench` API surfaces.
#[allow(unreachable_pub)]
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub(crate) root: PathBuf,
    pub(crate) config: PolintConfig,
    pub(crate) missing: bool,
}

impl LoadedConfig {
    pub(crate) fn profile_rules(&self, profile: Option<&str>) -> Result<Option<Vec<String>>> {
        let Some(profile) = profile else {
            return Ok(None);
        };
        let Some(config) = self.config.profiles.get(profile) else {
            anyhow::bail!("profile `{profile}` is not defined in .polint.toml");
        };
        Ok(Some(config.rules.clone()))
    }

    pub(crate) fn rule_config(&self, id: &str) -> Option<&RuleConfig> {
        self.config.rules.config.iter().find(|rule| rule.id == id)
    }

    pub(crate) fn include_set(&self) -> Result<GlobSet> {
        if self.config.workspace.include.is_empty() {
            build_glob_set(&["**/*".to_string()])
        } else {
            build_glob_set(&self.config.workspace.include)
        }
    }

    pub(crate) fn exclude_set(&self) -> Result<GlobSet> {
        build_glob_set(&self.config.workspace.exclude)
    }
}

#[allow(unreachable_pub)]
pub fn load_config(root: impl AsRef<Path>) -> Result<LoadedConfig> {
    let root = root.as_ref().to_path_buf();
    let path = root.join(".polint.toml");
    if !path.exists() {
        return Ok(LoadedConfig {
            root,
            config: PolintConfig::default(),
            missing: true,
        });
    }

    let raw = fs::read_to_string(&path)
        .with_context(|| format!("failed to read config {}", path.display()))?;
    let config: PolintConfig = toml::from_str(&raw)
        .with_context(|| format!("failed to parse config {}", path.display()))?;
    Ok(LoadedConfig {
        root,
        config,
        missing: false,
    })
}

pub(crate) fn default_config_toml() -> &'static str {
    r#"# polint is for repo-local engineering policy as code.

[workspace]
include = ["**/*"]
exclude = ["**/vendor/**", "**/node_modules/**", "**/.git/**", "**/target/**", "**/*.pb.go"]

[rules]
paths = [".polint/rules"]
"#
}

pub(crate) fn build_glob_set(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        add_glob(&mut builder, pattern)?;
        if let Some(prefix) = pattern.strip_suffix("/**") {
            add_glob(&mut builder, &format!("{prefix}/*"))?;
            add_glob(&mut builder, &format!("{prefix}/**/*"))?;
        }
    }
    Ok(builder.build()?)
}

fn add_glob(builder: &mut GlobSetBuilder, pattern: &str) -> Result<()> {
    builder.add(
        Glob::new(pattern).map_err(|source| ConfigError::InvalidGlob {
            glob: pattern.to_string(),
            source,
        })?,
    );
    Ok(())
}

fn default_include() -> Vec<String> {
    vec!["**/*".to_string()]
}

fn default_exclude() -> Vec<String> {
    vec![
        "**/vendor/**".to_string(),
        "**/node_modules/**".to_string(),
        "**/.git/**".to_string(),
        "**/target/**".to_string(),
        "**/*.pb.go".to_string(),
    ]
}

fn default_rule_paths() -> Vec<String> {
    vec![".polint/rules".to_string()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_parses() {
        let config: PolintConfig = toml::from_str(default_config_toml()).unwrap();
        assert_eq!(config.rules.paths, vec![".polint/rules"]);
        assert!(config.profiles.is_empty());
    }

    #[test]
    fn default_config_has_no_profiles() {
        let loaded = LoadedConfig {
            root: PathBuf::from("."),
            config: PolintConfig::default(),
            missing: true,
        };

        assert!(loaded.config.profiles.is_empty());
        assert_eq!(loaded.profile_rules(None).unwrap(), None);
        assert!(loaded.profile_rules(Some("missing")).is_err());
    }

    #[test]
    fn missing_rules_section_uses_default_rule_path() {
        let config: PolintConfig = toml::from_str(
            r#"
[profiles.local]
rules = ["local/example"]
"#,
        )
        .unwrap();
        assert_eq!(config.rules.paths, vec![".polint/rules"]);
    }

    #[test]
    fn profile_selection_requires_exact_name() {
        let config: PolintConfig = toml::from_str(
            r#"
[profiles.local]
rules = ["local/example"]
"#,
        )
        .unwrap();
        let config = LoadedConfig {
            root: PathBuf::from("."),
            config,
            missing: false,
        };

        assert_eq!(config.profile_rules(None).unwrap(), None);
        assert_eq!(
            config.profile_rules(Some("local")).unwrap(),
            Some(vec!["local/example".to_string()])
        );
        assert!(config.profile_rules(Some("missing")).is_err());
    }

    #[test]
    fn rule_config_parses_literal_allow_list() {
        let config: PolintConfig = toml::from_str(
            r##"
[[rules.config]]
id = "examples/ts-no-raw-colors"
allow_files = ["**/theme/**"]
allow = ["#fff", "currentColor"]
"##,
        )
        .unwrap();

        let rule = &config.rules.config[0];
        assert_eq!(rule.allow_files, vec!["**/theme/**"]);
        assert_eq!(rule.allow, vec!["#fff", "currentColor"]);
    }

    #[test]
    fn rule_config_defaults_literal_allow_list_to_empty() {
        let config: PolintConfig = toml::from_str(
            r#"
[[rules.config]]
id = "examples/ts-no-raw-colors"
"#,
        )
        .unwrap();

        assert!(config.rules.config[0].allow.is_empty());
    }

    #[test]
    fn sarif_rule_help_uri_and_path_contexts_parse() {
        let config: PolintConfig = toml::from_str(
            r#"
[sarif.rule_help_uri]
"local/a" = "https://docs.example/a"

[[path_contexts.pairs]]
name = "svc_ports"
left_before_ctx = "internal/"
left_after_ctx = "/service/"
right_before_ctx = "internal/"
right_after_ctx = "/ports/"
"#,
        )
        .unwrap();
        assert_eq!(
            config.sarif.rule_help_uri["local/a"],
            "https://docs.example/a"
        );
        assert_eq!(config.path_contexts.pairs.len(), 1);
        assert_eq!(config.path_contexts.pairs[0].name, "svc_ports");
    }
}
