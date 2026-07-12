//! Cache invalidation digest helpers (config + enabled rules/options).
//!
//! Encoding is deterministic and infallible so cache hashing never relies on panicking serializers.

use std::collections::BTreeSet;

use crate::analysis_kernel::incremental::{Digest, DigestKind};
use crate::config::{
    IgnoreConfig, LoadedConfig, PathContextPair, PathContextsConfig, PolintConfig, ProfileConfig,
    ReachabilityConfig, RuleConfig, RuleSection, SolverConfig, WorkspaceConfig,
};
use crate::core::{Rule, RuleOptions};

use super::stable_hash;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum AnalysisSettingsScope {
    Source,
    GoSyntax,
    TsSyntax,
    ModuleGraph,
    SymbolGraph,
    ModuleTopology,
    SemanticMir,
    Cfg,
    Calls,
    GoSemantic,
    Identity,
    AbstractDomains,
    DirectSummaries,
    Entrypoints,
    Reachability,
    Extensions,
    TypeValueAlias,
    SemanticGraph,
    Solver,
    RefinedCalls,
    DataFlow,
    Evidence,
    Metrics,
}

impl AnalysisSettingsScope {
    pub(crate) const ALL: [Self; 23] = [
        Self::Source,
        Self::GoSyntax,
        Self::TsSyntax,
        Self::ModuleGraph,
        Self::SymbolGraph,
        Self::ModuleTopology,
        Self::SemanticMir,
        Self::Cfg,
        Self::Calls,
        Self::GoSemantic,
        Self::Identity,
        Self::AbstractDomains,
        Self::DirectSummaries,
        Self::Entrypoints,
        Self::Reachability,
        Self::Extensions,
        Self::TypeValueAlias,
        Self::SemanticGraph,
        Self::Solver,
        Self::RefinedCalls,
        Self::DataFlow,
        Self::Evidence,
        Self::Metrics,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Source => "polint.source",
            Self::GoSyntax => "polint.go.syntax",
            Self::TsSyntax => "polint.ts.syntax",
            Self::ModuleGraph => "polint.module_graph",
            Self::SymbolGraph => "polint.symbol_graph",
            Self::ModuleTopology => "polint.module_topology",
            Self::SemanticMir => "polint.semantic_mir",
            Self::Cfg => "polint.cfg",
            Self::Calls => "polint.calls",
            Self::GoSemantic => "polint.go.semantic",
            Self::Identity => "polint.identity",
            Self::AbstractDomains => "polint.abstract_domains",
            Self::DirectSummaries => "polint.direct_summaries",
            Self::Entrypoints => "polint.entrypoints",
            Self::Reachability => "polint.reachability",
            Self::Extensions => "polint.extensions",
            Self::TypeValueAlias => "polint.type_value_alias",
            Self::SemanticGraph => "polint.semantic_graph",
            Self::Solver => "polint.solver",
            Self::RefinedCalls => "polint.refined_calls",
            Self::DataFlow => "polint.data_flow",
            Self::Evidence => "polint.evidence",
            Self::Metrics => "polint.metrics",
        }
    }
}

/// Stable digest of loaded config fields that can affect analysis or rule output.
///
/// Compared to JSON serialization, cache keys intentionally **invalidate** entries when this
/// format changes (fine for correctness; avoids hidden serde-json key ordering quirks).
pub(crate) fn config_hash(config: &LoadedConfig) -> String {
    let missing = if config.missing { "missing" } else { "loaded" };
    let respect_gitignore = if config.respect_gitignore {
        "respect_gitignore=true"
    } else {
        "respect_gitignore=false"
    };
    let serialized = deterministic_polint_config(&config.config);
    stable_hash(&[missing, respect_gitignore, &serialized])
}

pub(crate) fn analysis_settings_hash(
    loaded: &LoadedConfig,
    scope: AnalysisSettingsScope,
) -> Digest {
    let serialized = match scope {
        AnalysisSettingsScope::Source => Some(format!(
            "respect_gitignore={}|{}",
            loaded.respect_gitignore,
            deterministic_workspace(&loaded.config.workspace)
        )),
        AnalysisSettingsScope::GoSyntax | AnalysisSettingsScope::GoSemantic => Some(format!(
            "languages.go={}",
            deterministic_toml_map(&loaded.config.languages.go)
        )),
        AnalysisSettingsScope::TsSyntax => Some(format!(
            "languages.ts={}",
            deterministic_toml_map(&loaded.config.languages.ts)
        )),
        AnalysisSettingsScope::ModuleGraph
        | AnalysisSettingsScope::SymbolGraph
        | AnalysisSettingsScope::ModuleTopology
        | AnalysisSettingsScope::SemanticMir
        | AnalysisSettingsScope::Cfg
        | AnalysisSettingsScope::Calls
        | AnalysisSettingsScope::AbstractDomains
        | AnalysisSettingsScope::DirectSummaries
        | AnalysisSettingsScope::TypeValueAlias => Some(format!(
            "languages.go={}|languages.ts={}",
            deterministic_toml_map(&loaded.config.languages.go),
            deterministic_toml_map(&loaded.config.languages.ts)
        )),
        AnalysisSettingsScope::Reachability => {
            Some(deterministic_reachability(&loaded.config.reachability))
        }
        AnalysisSettingsScope::SemanticGraph => {
            Some(deterministic_js_object_settings(&loaded.config.solver))
        }
        AnalysisSettingsScope::Solver => {
            Some(deterministic_effective_solver(&loaded.config.solver))
        }
        AnalysisSettingsScope::Identity
        | AnalysisSettingsScope::Entrypoints
        | AnalysisSettingsScope::Extensions
        | AnalysisSettingsScope::RefinedCalls
        | AnalysisSettingsScope::DataFlow
        | AnalysisSettingsScope::Evidence
        | AnalysisSettingsScope::Metrics => None,
    };

    serialized.map_or_else(
        || Digest::absent(DigestKind::AnalysisSettings, scope.label()),
        |serialized| {
            Digest::from_parts(
                DigestKind::AnalysisSettings,
                "provider_analysis_settings",
                &[scope.label(), &serialized],
            )
        },
    )
}

pub(crate) fn rule_hash(
    rules: &[Rule],
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
        format!("ignores={}", deterministic_ignores(&config.ignores)),
        format!(
            "reachability={}",
            deterministic_reachability(&config.reachability)
        ),
        format!("solver={}", deterministic_solver(&config.solver)),
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

fn deterministic_ignores(ignores: &IgnoreConfig) -> String {
    format!("require_reason={}", ignores.require_reason)
}

fn deterministic_reachability(reachability: &ReachabilityConfig) -> String {
    format!("roots={}", deterministic_string_list(&reachability.roots))
}

fn deterministic_solver(solver: &SolverConfig) -> String {
    format!(
        "go.address_taken_threshold={}|go.max_candidates_per_callsite={}|go.max_rta_rounds={}|go.max_worklist_steps={}|js.object_model={}|js.max_tokens_per_var={}|js.max_candidates_per_callsite={}|js.max_token_worklist_steps={}|js.max_object_objects_per_place={}|js.max_object_properties_per_object={}|js.max_object_tokens_per_property={}|js.max_object_computed_buckets_per_object={}|js.max_object_prototype_depth={}|js.max_object_receiver_candidates_per_callsite={}|js.max_object_worklist_steps={}",
        deterministic_usize_option(solver.go.address_taken_threshold),
        deterministic_usize_option(solver.go.max_candidates_per_callsite),
        deterministic_usize_option(solver.go.max_rta_rounds),
        deterministic_usize_option(solver.go.max_worklist_steps),
        deterministic_bool_option(solver.js.object_model),
        deterministic_usize_option(solver.js.max_tokens_per_var),
        deterministic_usize_option(solver.js.max_candidates_per_callsite),
        deterministic_usize_option(solver.js.max_token_worklist_steps),
        deterministic_usize_option(solver.js.max_object_objects_per_place),
        deterministic_usize_option(solver.js.max_object_properties_per_object),
        deterministic_usize_option(solver.js.max_object_tokens_per_property),
        deterministic_usize_option(solver.js.max_object_computed_buckets_per_object),
        deterministic_usize_option(solver.js.max_object_prototype_depth),
        deterministic_usize_option(solver.js.max_object_receiver_candidates_per_callsite),
        deterministic_usize_option(solver.js.max_object_worklist_steps),
    )
}

fn deterministic_effective_solver(solver: &SolverConfig) -> String {
    let go = solver.to_go_sub_budget();
    let js = solver.to_js_sub_budget();
    format!(
        "go.address_taken_threshold={}|go.max_candidates_per_callsite={}|go.max_rta_rounds={}|go.max_worklist_steps={}|js.max_tokens_per_var={}|js.max_candidates_per_callsite={}|js.max_token_worklist_steps={}|{}",
        go.address_taken_threshold,
        go.max_candidates_per_callsite,
        go.max_rta_rounds,
        go.max_worklist_steps,
        js.max_tokens_per_var,
        js.max_candidates_per_callsite,
        js.max_token_worklist_steps,
        deterministic_js_object_settings(solver),
    )
}

fn deterministic_js_object_settings(solver: &SolverConfig) -> String {
    let object = solver.to_js_object_sub_budget();
    format!(
        "js.object_model={}|js.max_object_objects_per_place={}|js.max_object_properties_per_object={}|js.max_object_tokens_per_property={}|js.max_object_computed_buckets_per_object={}|js.max_object_prototype_depth={}|js.max_object_receiver_candidates_per_callsite={}|js.max_object_worklist_steps={}",
        solver.js_object_model_enabled(),
        object.max_objects_per_place,
        object.max_properties_per_object,
        object.max_tokens_per_property,
        object.max_computed_buckets_per_object,
        object.max_prototype_depth,
        object.max_receiver_candidates_per_callsite,
        object.max_object_worklist_steps,
    )
}

fn deterministic_usize_option(value: Option<usize>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn deterministic_bool_option(value: Option<bool>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
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

    fn analysis_settings(loaded: &LoadedConfig) -> Vec<(AnalysisSettingsScope, Digest)> {
        AnalysisSettingsScope::ALL
            .into_iter()
            .map(|scope| (scope, analysis_settings_hash(loaded, scope)))
            .collect()
    }

    fn loaded_with_rule_config() -> LoadedConfig {
        let mut loaded = sample_loaded(false);
        loaded.config.rules.config.push(RuleConfig {
            id: "local/no-todo".to_string(),
            severity: None,
            files: Vec::new(),
            allow_files: Vec::new(),
            allow: Vec::new(),
            max: None,
            deny: Vec::new(),
            forbidden_imports: Default::default(),
            settings: Default::default(),
        });
        loaded
    }

    fn sample_loaded(missing: bool) -> LoadedConfig {
        LoadedConfig {
            root: Default::default(),
            config: PolintConfig::default(),
            missing,
            respect_gitignore: true,
        }
    }

    #[test]
    fn config_hash_stable_for_clone() {
        let loaded = sample_loaded(false);
        assert_eq!(config_hash(&loaded), config_hash(&loaded.clone()));
    }

    #[test]
    fn analysis_setting_scopes_cover_every_current_provider() {
        let scope_labels = AnalysisSettingsScope::ALL
            .into_iter()
            .map(AnalysisSettingsScope::label)
            .collect::<BTreeSet<_>>();
        let provider_ids = crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .map(|manifest| manifest.id)
            .collect::<BTreeSet<_>>();

        assert_eq!(scope_labels, provider_ids);
    }

    #[test]
    fn rule_only_config_mutations_preserve_every_analysis_setting_scope() {
        let baseline = loaded_with_rule_config();
        let baseline_settings = analysis_settings(&baseline);
        let mut mutations = Vec::new();

        let mut severity = baseline.clone();
        severity.config.rules.config[0].severity = Some("error".to_string());
        mutations.push(severity);

        let mut files = baseline.clone();
        files.config.rules.config[0].files = vec!["src/**".to_string()];
        mutations.push(files);

        let mut allow_files = baseline.clone();
        allow_files.config.rules.config[0].allow_files = vec!["src/generated/**".to_string()];
        mutations.push(allow_files);

        let mut allow = baseline.clone();
        allow.config.rules.config[0].allow = vec!["legacy".to_string()];
        mutations.push(allow);

        let mut deny = baseline.clone();
        deny.config.rules.config[0].deny = vec!["unsafe".to_string()];
        mutations.push(deny);

        let mut max = baseline.clone();
        max.config.rules.config[0].max = Some(7);
        mutations.push(max);

        let mut forbidden_imports = baseline.clone();
        forbidden_imports.config.rules.config[0]
            .forbidden_imports
            .insert("src/**".to_string(), vec!["legacy/**".to_string()]);
        mutations.push(forbidden_imports);

        let mut settings = baseline.clone();
        settings.config.rules.config[0]
            .settings
            .insert("token".to_string(), toml::Value::String("TODO".to_string()));
        mutations.push(settings);

        for mutation in mutations {
            assert_ne!(config_hash(&baseline), config_hash(&mutation));
            assert_eq!(baseline_settings, analysis_settings(&mutation));
        }
    }

    #[test]
    fn provider_setting_mutations_change_only_declared_scopes() {
        let baseline = sample_loaded(false);

        let mut source = baseline.clone();
        source.respect_gitignore = false;
        assert_eq!(
            changed_analysis_setting_scopes(&baseline, &source),
            BTreeSet::from([AnalysisSettingsScope::Source])
        );

        let mut reachability = baseline.clone();
        reachability
            .config
            .reachability
            .roots
            .push("cmd/server.main".to_string());
        assert_eq!(
            changed_analysis_setting_scopes(&baseline, &reachability),
            BTreeSet::from([AnalysisSettingsScope::Reachability])
        );

        let mut solver = baseline.clone();
        solver.config.solver.go.max_rta_rounds = Some(8);
        assert_eq!(
            changed_analysis_setting_scopes(&baseline, &solver),
            BTreeSet::from([AnalysisSettingsScope::Solver])
        );

        let mut object_model = baseline.clone();
        object_model.config.solver.js.object_model = Some(true);
        assert_eq!(
            changed_analysis_setting_scopes(&baseline, &object_model),
            BTreeSet::from([
                AnalysisSettingsScope::SemanticGraph,
                AnalysisSettingsScope::Solver,
            ])
        );
    }

    fn changed_analysis_setting_scopes(
        baseline: &LoadedConfig,
        modified: &LoadedConfig,
    ) -> BTreeSet<AnalysisSettingsScope> {
        AnalysisSettingsScope::ALL
            .into_iter()
            .filter(|scope| {
                analysis_settings_hash(baseline, *scope) != analysis_settings_hash(modified, *scope)
            })
            .collect()
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
    fn config_hash_differs_when_gitignore_policy_changes() {
        let baseline = sample_loaded(false);
        let mut modified = baseline.clone();
        modified.respect_gitignore = false;

        assert_ne!(config_hash(&baseline), config_hash(&modified));
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
    fn config_hash_differs_when_ignore_reason_policy_changes() {
        let baseline = sample_loaded(false);
        let mut modified = baseline.clone();
        modified.config.ignores.require_reason = true;

        assert_ne!(config_hash(&baseline), config_hash(&modified));
    }

    #[test]
    fn config_hash_differs_when_reachability_roots_change() {
        let baseline = sample_loaded(false);
        let mut modified = baseline.clone();
        modified
            .config
            .reachability
            .roots
            .push("cmd/server.main".to_string());

        assert_ne!(config_hash(&baseline), config_hash(&modified));
    }

    #[test]
    fn config_hash_differs_when_solver_knobs_change() {
        let baseline = sample_loaded(false);

        let mut go_modified = baseline.clone();
        go_modified.config.solver.go.address_taken_threshold = Some(17);

        let mut js_modified = baseline.clone();
        js_modified.config.solver.js.object_model = Some(true);

        let mut object_modified = baseline.clone();
        object_modified.config.solver.js.max_object_prototype_depth = Some(4);

        assert_ne!(config_hash(&baseline), config_hash(&go_modified));
        assert_ne!(config_hash(&baseline), config_hash(&js_modified));
        assert_ne!(config_hash(&baseline), config_hash(&object_modified));
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
    fn config_hash_differs_when_go_symbol_settings_change() {
        let mut baseline = sample_loaded(false);
        baseline.config.languages.go.insert(
            "package_patterns".to_string(),
            toml::Value::String("./...".to_string()),
        );
        baseline.config.languages.go.insert(
            "module_roots".to_string(),
            toml::Value::Array(vec![toml::Value::String("services/app".to_string())]),
        );
        baseline.config.languages.go.insert(
            "build_tags".to_string(),
            toml::Value::String("enterprise".to_string()),
        );
        baseline
            .config
            .languages
            .go
            .insert("include_tests".to_string(), toml::Value::Boolean(true));

        let mut changed_patterns = baseline.clone();
        changed_patterns.config.languages.go.insert(
            "package_patterns".to_string(),
            toml::Value::Array(vec![toml::Value::String("./cmd/...".to_string())]),
        );

        let mut changed_roots = baseline.clone();
        changed_roots.config.languages.go.insert(
            "module_roots".to_string(),
            toml::Value::Array(vec![toml::Value::String("services/worker".to_string())]),
        );

        let mut changed_tags = baseline.clone();
        changed_tags.config.languages.go.insert(
            "build_tags".to_string(),
            toml::Value::String("enterprise,polint".to_string()),
        );

        let mut changed_tests = baseline.clone();
        changed_tests
            .config
            .languages
            .go
            .insert("include_tests".to_string(), toml::Value::Boolean(false));

        assert_ne!(config_hash(&baseline), config_hash(&changed_patterns));
        assert_ne!(config_hash(&baseline), config_hash(&changed_roots));
        assert_ne!(config_hash(&baseline), config_hash(&changed_tags));
        assert_ne!(config_hash(&baseline), config_hash(&changed_tests));
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
