use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid glob `{glob}`: {source}")]
    InvalidGlob {
        glob: String,
        source: globset::Error,
    },
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolintConfig {
    #[serde(default)]
    pub workspace: WorkspaceConfig,
    #[serde(default)]
    pub rules: RuleSection,
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileConfig>,
    #[serde(default)]
    pub languages: LanguageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    #[serde(default = "default_include")]
    pub include: Vec<String>,
    #[serde(default = "default_exclude")]
    pub exclude: Vec<String>,
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
pub struct RuleSection {
    #[serde(default = "default_rule_paths")]
    pub paths: Vec<String>,
    #[serde(default)]
    pub config: Vec<RuleConfig>,
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
pub struct RuleConfig {
    pub id: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub files: Vec<String>,
    #[serde(default)]
    pub allow_files: Vec<String>,
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub max: Option<u32>,
    #[serde(default)]
    pub deny: Vec<String>,
    #[serde(default)]
    pub forbidden_imports: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProfileConfig {
    #[serde(default)]
    pub rules: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LanguageConfig {
    #[serde(default)]
    pub go: BTreeMap<String, toml::Value>,
    #[serde(default)]
    pub ts: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub root: PathBuf,
    pub path: Option<PathBuf>,
    pub config: PolintConfig,
    pub missing: bool,
}

impl LoadedConfig {
    pub fn profile_rules(&self, profile: Option<&str>) -> Result<Option<Vec<String>>> {
        let Some(profile) = profile else {
            return Ok(None);
        };
        let Some(config) = self.config.profiles.get(profile) else {
            anyhow::bail!("profile `{profile}` is not defined in .polint.toml");
        };
        Ok(Some(config.rules.clone()))
    }

    pub fn rule_config(&self, id: &str) -> Option<&RuleConfig> {
        self.config.rules.config.iter().find(|rule| rule.id == id)
    }

    pub fn include_set(&self) -> Result<GlobSet> {
        if self.config.workspace.include.is_empty() {
            build_glob_set(&["**/*".to_string()])
        } else {
            build_glob_set(&self.config.workspace.include)
        }
    }

    pub fn exclude_set(&self) -> Result<GlobSet> {
        build_glob_set(&self.config.workspace.exclude)
    }
}

pub fn load_config(root: impl AsRef<Path>) -> Result<LoadedConfig> {
    let root = root.as_ref().to_path_buf();
    let path = root.join(".polint.toml");
    if !path.exists() {
        return Ok(LoadedConfig {
            root,
            path: None,
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
        path: Some(path),
        config,
        missing: false,
    })
}

pub fn default_config_toml() -> &'static str {
    r#"# polint is for repo-local engineering policy as code.

[workspace]
include = ["**/*"]
exclude = ["**/vendor/**", "**/node_modules/**", "**/.git/**", "**/target/**", "**/*.pb.go"]

[rules]
paths = [".polint/rules"]
"#
}

pub fn build_glob_set(patterns: &[String]) -> Result<GlobSet> {
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
            path: None,
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
            path: Some(PathBuf::from(".polint.toml")),
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
}
