use anyhow::{Context, Result};
use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

use crate::analysis::solver::budget::{GoRtaSubBudget, JsObjectModelSubBudget, JsTokensSubBudget};

const CONFIG_MAX_BYTES: u64 = 1_048_576;

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
    #[serde(default)]
    pub(crate) ignores: IgnoreConfig,
    #[serde(default)]
    pub(crate) reachability: ReachabilityConfig,
    #[serde(default)]
    pub(crate) solver: SolverConfig,
}

/// Configured whole-program reachability roots (D-13).
///
/// Each entry is a repo-controlled string such as `"pkg/path.Func"` or
/// `"src/x.ts#handler"` that discovery resolves against existing in-DB symbol
/// facts. An unresolvable entry becomes a `RootStatus::Unresolved` root fact —
/// never a silent drop and never a path-traversal read outside the repo.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ReachabilityConfig {
    #[serde(default)]
    pub(crate) roots: Vec<String>,
}

/// `[solver]` config table (D-10). `.polint.toml` config surface for the unified
/// solver — NOT an SDK promotion. Sits beside [`ReachabilityConfig`].
///
/// Today it threads the per-language Go RTA caps (`solver.go.*`) and JS/TS token
/// caps (`solver.js.*`) into their solver sub-budgets; later phases extend it with
/// cross-language knobs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SolverConfig {
    #[serde(default)]
    pub(crate) go: SolverGoConfig,
    #[serde(default)]
    pub(crate) js: SolverJsConfig,
}

/// `[solver.go]` config sub-table (D-10). Each knob is an `Option<usize>` so an
/// absent key falls back to [`GoRtaSubBudget::default()`] (D-11). The roadmap-named
/// `address_taken_threshold` is the headline knob.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SolverGoConfig {
    #[serde(default)]
    pub(crate) address_taken_threshold: Option<usize>,
    #[serde(default)]
    pub(crate) max_candidates_per_callsite: Option<usize>,
    #[serde(default)]
    pub(crate) max_rta_rounds: Option<usize>,
    #[serde(default)]
    pub(crate) max_worklist_steps: Option<usize>,
}

/// `[solver.js]` config sub-table (JS-04/JS-05). Each numeric knob is an
/// `Option<usize>` so an absent key falls back to its sub-budget default. A zero
/// value is treated as a typo and also falls back to the default.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct SolverJsConfig {
    #[serde(default)]
    pub(crate) object_model: Option<bool>,
    #[serde(default)]
    pub(crate) max_tokens_per_var: Option<usize>,
    #[serde(default)]
    pub(crate) max_candidates_per_callsite: Option<usize>,
    #[serde(default)]
    pub(crate) max_token_worklist_steps: Option<usize>,
    #[serde(default)]
    pub(crate) max_object_objects_per_place: Option<usize>,
    #[serde(default)]
    pub(crate) max_object_properties_per_object: Option<usize>,
    #[serde(default)]
    pub(crate) max_object_tokens_per_property: Option<usize>,
    #[serde(default)]
    pub(crate) max_object_computed_buckets_per_object: Option<usize>,
    #[serde(default)]
    pub(crate) max_object_prototype_depth: Option<usize>,
    #[serde(default)]
    pub(crate) max_object_receiver_candidates_per_callsite: Option<usize>,
    #[serde(default)]
    pub(crate) max_object_worklist_steps: Option<usize>,
}

impl SolverConfig {
    /// Overlay present `[solver.go]` config values onto [`GoRtaSubBudget::default()`]
    /// (D-11). Absent knobs keep their default; this is the single mapper the kernel
    /// uses to build `SolverBudget.go` from `.polint.toml`.
    ///
    /// FINDING D: a non-positive cap (`0`) is NOT overlaid verbatim. Every Go RTA cap is a
    /// strictly-positive ceiling — `max_worklist_steps = 0` / `max_rta_rounds = 0` /
    /// `address_taken_threshold = 0` would make the fixpoint latch
    /// [`crate::analysis::solver::budget::BudgetStatus::BudgetExceeded`] immediately (zero
    /// edges, EVERY run), so a config typo would silently disable all Go RTA. A `Some(0)` is
    /// therefore treated as the documented default rather than "unbounded/disabled".
    pub(crate) fn to_go_sub_budget(&self) -> GoRtaSubBudget {
        let mut budget = GoRtaSubBudget::default();
        overlay_positive_cap(
            &mut budget.address_taken_threshold,
            self.go.address_taken_threshold,
        );
        overlay_positive_cap(
            &mut budget.max_candidates_per_callsite,
            self.go.max_candidates_per_callsite,
        );
        overlay_positive_cap(&mut budget.max_rta_rounds, self.go.max_rta_rounds);
        overlay_positive_cap(&mut budget.max_worklist_steps, self.go.max_worklist_steps);
        budget
    }

    /// Overlay present `[solver.js]` config values onto
    /// [`JsTokensSubBudget::default()`]. Like Go RTA caps, every JS token cap is a
    /// strictly-positive ceiling: `0` falls back to the documented default rather
    /// than disabling the token driver.
    pub(crate) fn to_js_sub_budget(&self) -> JsTokensSubBudget {
        let mut budget = JsTokensSubBudget::default();
        overlay_positive_cap(&mut budget.max_tokens_per_var, self.js.max_tokens_per_var);
        overlay_positive_cap(
            &mut budget.max_candidates_per_callsite,
            self.js.max_candidates_per_callsite,
        );
        overlay_positive_cap(
            &mut budget.max_token_worklist_steps,
            self.js.max_token_worklist_steps,
        );
        budget
    }

    /// Return whether the JS/TS object-model solver driver should run. It is
    /// disabled by default until benchmark promotion gates approve default enablement.
    pub(crate) fn js_object_model_enabled(&self) -> bool {
        self.js.object_model.unwrap_or(false)
    }

    /// Overlay present `[solver.js]` object-model config values onto
    /// [`JsObjectModelSubBudget::default()`]. Every object cap is a strictly-positive
    /// ceiling: `0` falls back to the documented default rather than disabling proof
    /// or making the future driver immediately exhausted.
    pub(crate) fn to_js_object_sub_budget(&self) -> JsObjectModelSubBudget {
        let mut budget = JsObjectModelSubBudget::default();
        overlay_positive_cap(
            &mut budget.max_objects_per_place,
            self.js.max_object_objects_per_place,
        );
        overlay_positive_cap(
            &mut budget.max_properties_per_object,
            self.js.max_object_properties_per_object,
        );
        overlay_positive_cap(
            &mut budget.max_tokens_per_property,
            self.js.max_object_tokens_per_property,
        );
        overlay_positive_cap(
            &mut budget.max_computed_buckets_per_object,
            self.js.max_object_computed_buckets_per_object,
        );
        overlay_positive_cap(
            &mut budget.max_prototype_depth,
            self.js.max_object_prototype_depth,
        );
        overlay_positive_cap(
            &mut budget.max_receiver_candidates_per_callsite,
            self.js.max_object_receiver_candidates_per_callsite,
        );
        overlay_positive_cap(
            &mut budget.max_object_worklist_steps,
            self.js.max_object_worklist_steps,
        );
        budget
    }
}

/// Overlay a configured cap onto its default, treating a non-positive (`Some(0)`) value as
/// "keep the default" (FINDING D). An absent knob (`None`) also keeps the default. Every
/// solver cap is a strictly-positive ceiling, so `0` is never a meaningful "disable" — it
/// is a typo that must not silently zero an analysis driver.
fn overlay_positive_cap(slot: &mut usize, configured: Option<usize>) {
    if let Some(value) = configured
        && value > 0
    {
        *slot = value;
    }
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
    #[serde(flatten)]
    pub(crate) settings: BTreeMap<String, toml::Value>,
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

/// Comment-ignore behavior.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct IgnoreConfig {
    /// Require `-- reason` on suppressing ignore directives.
    #[serde(default)]
    pub(crate) require_reason: bool,
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
/// `load_config` and `polint::_bench::keys` can use it in public `bench` API surfaces.
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
    let path = ".polint.toml";
    let raw =
        match crate::repo_fs::read_repo_file_to_string_with_limit(&root, path, CONFIG_MAX_BYTES) {
            Ok(raw) => raw,
            Err(error) if error.is_not_found() => {
                return Ok(LoadedConfig {
                    root,
                    config: PolintConfig::default(),
                    missing: true,
                });
            }
            Err(error) => anyhow::bail!(
                "failed to read config {}: {error}",
                root.join(path).display()
            ),
        };
    let config: PolintConfig = toml::from_str(&raw)
        .with_context(|| format!("failed to parse config {}", root.join(path).display()))?;
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

[ignores]
require_reason = false
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
        assert!(!config.ignores.require_reason);
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

    #[cfg(unix)]
    #[test]
    fn load_config_rejects_symlink_escape() {
        let repo = tempfile::tempdir().expect("repo");
        let outside = tempfile::tempdir().expect("outside");
        std::fs::write(outside.path().join(".polint.toml"), "[rules]\npaths = []\n")
            .expect("write outside config");
        std::os::unix::fs::symlink(
            outside.path().join(".polint.toml"),
            repo.path().join(".polint.toml"),
        )
        .expect("symlink config");

        let error = load_config(repo.path()).expect_err("symlink escape should fail");

        assert!(error.to_string().contains("path escapes repository root"));
    }

    #[test]
    fn load_config_rejects_oversized_config() {
        let repo = tempfile::tempdir().expect("repo");
        std::fs::write(
            repo.path().join(".polint.toml"),
            " ".repeat(CONFIG_MAX_BYTES as usize + 1),
        )
        .expect("write config");

        let error = load_config(repo.path()).expect_err("oversized config should fail");

        assert!(
            error
                .to_string()
                .contains("file exceeds topology input size limit")
        );
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
    fn rule_config_preserves_custom_settings() {
        let config: PolintConfig = toml::from_str(
            r#"
[[rules.config]]
id = "local/require-wrapper"
files = ["src/**"]
required_prefix = "safe_"
owners = ["payments", "platform"]
[rules.config.thresholds]
warn = 2
error = 5
"#,
        )
        .unwrap();

        let rule = &config.rules.config[0];
        assert_eq!(rule.files, vec!["src/**"]);
        assert_eq!(rule.settings["required_prefix"].as_str(), Some("safe_"));
        assert_eq!(rule.settings["owners"].as_array().map(Vec::len), Some(2));
        assert_eq!(
            rule.settings["thresholds"]
                .get("error")
                .and_then(toml::Value::as_integer),
            Some(5)
        );
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

    #[test]
    fn reachability_config_defaults_to_empty_roots() {
        let config: PolintConfig = toml::from_str("").unwrap();
        assert!(config.reachability.roots.is_empty());
    }

    #[test]
    fn reachability_config_parses_configured_roots() {
        let config: PolintConfig = toml::from_str(
            r#"
[reachability]
roots = ["pkg/path.Func", "src/x.ts#handler"]
"#,
        )
        .unwrap();
        assert_eq!(
            config.reachability.roots,
            vec!["pkg/path.Func".to_string(), "src/x.ts#handler".to_string()]
        );
    }

    #[test]
    fn solver_config_defaults_to_go_sub_budget_defaults() {
        // Absent [solver] table falls back to GoRtaSubBudget::default() (D-11).
        let config: PolintConfig = toml::from_str("").unwrap();
        assert_eq!(config.solver.to_go_sub_budget(), GoRtaSubBudget::default());
    }

    #[test]
    fn solver_go_override_maps_into_go_sub_budget() {
        // A [solver.go] override changes ONLY the present knobs; absent knobs keep
        // their default (D-10/D-11).
        let config: PolintConfig = toml::from_str(
            r#"
[solver.go]
address_taken_threshold = 999
max_rta_rounds = 7
max_worklist_steps = 50000
"#,
        )
        .unwrap();
        let budget = config.solver.to_go_sub_budget();
        assert_eq!(budget.address_taken_threshold, 999);
        assert_eq!(budget.max_rta_rounds, 7);
        assert_eq!(budget.max_worklist_steps, 50_000);
        // The unspecified knob stays at its default.
        assert_eq!(
            budget.max_candidates_per_callsite,
            GoRtaSubBudget::default().max_candidates_per_callsite
        );
    }

    #[test]
    fn solver_go_zero_knob_falls_back_to_default_not_self_disable() {
        // FINDING D: a `[solver.go]` cap of 0 must NOT be overlaid verbatim. A
        // `max_worklist_steps = 0` / `max_rta_rounds = 0` / `address_taken_threshold = 0`
        // would make the RTA fixpoint latch BudgetExceeded immediately (zero edges, every
        // run) — a config typo silently disabling all Go RTA. A non-positive cap is treated
        // as the documented default instead.
        let config: PolintConfig = toml::from_str(
            r#"
[solver.go]
address_taken_threshold = 0
max_candidates_per_callsite = 0
max_rta_rounds = 0
max_worklist_steps = 0
"#,
        )
        .unwrap();
        let budget = config.solver.to_go_sub_budget();
        // Every zeroed knob falls back to its honest default rather than self-disabling.
        assert_eq!(budget, GoRtaSubBudget::default());
        assert_eq!(
            budget.max_worklist_steps,
            GoRtaSubBudget::default().max_worklist_steps
        );
        assert_eq!(
            budget.max_rta_rounds,
            GoRtaSubBudget::default().max_rta_rounds
        );
        assert_eq!(
            budget.address_taken_threshold,
            GoRtaSubBudget::default().address_taken_threshold
        );
        assert_eq!(
            budget.max_candidates_per_callsite,
            GoRtaSubBudget::default().max_candidates_per_callsite
        );
    }

    #[test]
    fn solver_go_positive_knob_still_overrides_after_zero_clamp() {
        // The zero-clamp must not block a legitimate positive override: a present positive
        // value still maps through, while a zeroed sibling falls back to its default.
        let config: PolintConfig = toml::from_str(
            r#"
[solver.go]
max_rta_rounds = 0
max_worklist_steps = 5
"#,
        )
        .unwrap();
        let budget = config.solver.to_go_sub_budget();
        assert_eq!(budget.max_worklist_steps, 5, "positive override applies");
        assert_eq!(
            budget.max_rta_rounds,
            GoRtaSubBudget::default().max_rta_rounds,
            "a zeroed sibling falls back to default, not 0"
        );
    }

    #[test]
    fn solver_config_defaults_to_js_sub_budget_defaults() {
        // Absent [solver] table falls back to JsTokensSubBudget::default() (JS-04).
        let config: PolintConfig = toml::from_str("").unwrap();
        assert_eq!(
            config.solver.to_js_sub_budget(),
            JsTokensSubBudget::default()
        );
    }

    #[test]
    fn solver_js_override_maps_into_js_sub_budget() {
        // A [solver.js] override changes ONLY the present knobs; absent knobs keep
        // their default.
        let config: PolintConfig = toml::from_str(
            r#"
[solver.js]
max_tokens_per_var = 12
max_token_worklist_steps = 50000
"#,
        )
        .unwrap();
        let budget = config.solver.to_js_sub_budget();
        assert_eq!(budget.max_tokens_per_var, 12);
        assert_eq!(budget.max_token_worklist_steps, 50_000);
        assert_eq!(
            budget.max_candidates_per_callsite,
            JsTokensSubBudget::default().max_candidates_per_callsite
        );
    }

    #[test]
    fn solver_js_zero_knob_falls_back_to_default_not_self_disable() {
        // A `[solver.js]` cap of 0 must not be overlaid verbatim: every token cap is
        // strictly positive, and a zero typo would otherwise force immediate budget
        // exhaustion or suppress useful propagation.
        let config: PolintConfig = toml::from_str(
            r#"
[solver.js]
max_tokens_per_var = 0
max_candidates_per_callsite = 0
max_token_worklist_steps = 0
"#,
        )
        .unwrap();
        let budget = config.solver.to_js_sub_budget();
        assert_eq!(budget, JsTokensSubBudget::default());
    }

    #[test]
    fn solver_js_positive_knob_still_overrides_after_zero_clamp() {
        // The zero-clamp must not block a legitimate positive override: a present
        // positive value still maps through, while a zeroed sibling falls back.
        let config: PolintConfig = toml::from_str(
            r#"
[solver.js]
max_tokens_per_var = 0
max_candidates_per_callsite = 17
"#,
        )
        .unwrap();
        let budget = config.solver.to_js_sub_budget();
        assert_eq!(
            budget.max_tokens_per_var,
            JsTokensSubBudget::default().max_tokens_per_var,
            "a zeroed sibling falls back to default, not 0"
        );
        assert_eq!(
            budget.max_candidates_per_callsite, 17,
            "positive override applies"
        );
    }

    #[test]
    fn solver_config_defaults_to_object_model_disabled() {
        let config: PolintConfig = toml::from_str("").unwrap();
        assert!(!config.solver.js_object_model_enabled());
        assert_eq!(
            config.solver.to_js_object_sub_budget(),
            JsObjectModelSubBudget::default()
        );
    }

    #[test]
    fn solver_js_object_model_flag_enables_object_model() {
        let config: PolintConfig = toml::from_str(
            r#"
[solver.js]
object_model = true
"#,
        )
        .unwrap();
        assert!(config.solver.js_object_model_enabled());
    }

    #[test]
    fn solver_js_object_override_maps_into_object_sub_budget() {
        let config: PolintConfig = toml::from_str(
            r#"
[solver.js]
max_object_objects_per_place = 12
max_object_tokens_per_property = 17
max_object_receiver_candidates_per_callsite = 5
max_object_worklist_steps = 50000
"#,
        )
        .unwrap();
        let budget = config.solver.to_js_object_sub_budget();
        assert_eq!(budget.max_objects_per_place, 12);
        assert_eq!(budget.max_tokens_per_property, 17);
        assert_eq!(budget.max_receiver_candidates_per_callsite, 5);
        assert_eq!(budget.max_object_worklist_steps, 50_000);
        assert_eq!(
            budget.max_properties_per_object,
            JsObjectModelSubBudget::default().max_properties_per_object
        );
        assert_eq!(
            budget.max_computed_buckets_per_object,
            JsObjectModelSubBudget::default().max_computed_buckets_per_object
        );
        assert_eq!(
            budget.max_prototype_depth,
            JsObjectModelSubBudget::default().max_prototype_depth
        );
    }

    #[test]
    fn solver_js_object_zero_knobs_fall_back_to_default() {
        let config: PolintConfig = toml::from_str(
            r#"
[solver.js]
max_object_objects_per_place = 0
max_object_properties_per_object = 0
max_object_tokens_per_property = 0
max_object_computed_buckets_per_object = 0
max_object_prototype_depth = 0
max_object_receiver_candidates_per_callsite = 0
max_object_worklist_steps = 0
"#,
        )
        .unwrap();
        assert_eq!(
            config.solver.to_js_object_sub_budget(),
            JsObjectModelSubBudget::default()
        );
    }

    #[test]
    fn solver_js_object_positive_knob_still_overrides_after_zero_clamp() {
        let config: PolintConfig = toml::from_str(
            r#"
[solver.js]
max_object_objects_per_place = 0
max_object_properties_per_object = 33
"#,
        )
        .unwrap();
        let budget = config.solver.to_js_object_sub_budget();
        assert_eq!(
            budget.max_objects_per_place,
            JsObjectModelSubBudget::default().max_objects_per_place
        );
        assert_eq!(budget.max_properties_per_object, 33);
    }

    #[test]
    fn ignore_config_parses_reason_policy() {
        let config: PolintConfig = toml::from_str(
            r#"
[ignores]
require_reason = true
"#,
        )
        .unwrap();

        assert!(config.ignores.require_reason);
    }
}
