use crate::config::{LoadedConfig, load_config};
use crate::core::{Rule, RuleOptions, run_rules};
use crate::diagnostics::{
    ColorChoice, JsonReportMeta, OutputFormat, RenderOpts, Severity, dedupe_diagnostics, render,
};
use crate::fs::load_analysis_files;
use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

#[derive(Debug, Parser)]
#[command(name = "polint-local-rules")]
#[command(about = "Run a repo-local native polint rule host.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the registered local rules against the current directory.
    Check(CheckArgs),
}

#[derive(Debug, Args, Clone)]
struct CheckArgs {
    /// Files or directories to check. Defaults to the workspace include config.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Named profile from .polint.toml. When omitted, all registered rules run.
    #[arg(long)]
    profile: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = FormatArg::Human)]
    format: FormatArg,
    /// ANSI colors for human output (no effect on JSON/SARIF).
    #[arg(long, value_enum, default_value_t = ColorArg::Auto)]
    color: ColorArg,
    /// Disable .polint/cache reads and writes.
    #[arg(long)]
    no_cache: bool,
    /// Diagnostic level that fails the process.
    #[arg(long, value_enum, default_value_t = FailOn::Error)]
    fail_on: FailOn,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ColorArg {
    Auto,
    Always,
    Never,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FormatArg {
    Human,
    Json,
    Sarif,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FailOn {
    Warn,
    Error,
    None,
}

pub fn run_cli(rules: Vec<Arc<dyn Rule>>) -> ExitCode {
    match run(rules) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("polint-local-rules: {error:?}");
            ExitCode::from(2)
        }
    }
}

fn run(rules: Vec<Arc<dyn Rule>>) -> Result<u8> {
    let cli = Cli::parse();
    match cli.command {
        Command::Check(args) => check(std::env::current_dir()?, &args, &rules),
    }
}

fn check(root: PathBuf, args: &CheckArgs, rules: &[Arc<dyn Rule>]) -> Result<u8> {
    let (mut diagnostics, db) = analyze_and_run(&root, args, rules)?;
    diagnostics = dedupe_diagnostics(diagnostics);
    let source_map = match args.format {
        FormatArg::Human => db.sources_by_relative_path(),
        _ => BTreeMap::new(),
    };

    let format = match args.format {
        FormatArg::Human => OutputFormat::Human,
        FormatArg::Json => OutputFormat::Json,
        FormatArg::Sarif => OutputFormat::Sarif,
    };
    let sources = match args.format {
        FormatArg::Human => Some(&source_map),
        _ => None,
    };
    let opts = RenderOpts {
        json: JsonReportMeta {
            tool_name: "polint-local-rules",
            tool_version: env!("CARGO_PKG_VERSION"),
        },
        color: match args.color {
            ColorArg::Auto => ColorChoice::Auto,
            ColorArg::Always => ColorChoice::Always,
            ColorArg::Never => ColorChoice::Never,
        },
        sources,
    };
    print!("{}", render(format, &diagnostics, opts));

    Ok(exit_code_for(&diagnostics, args.fail_on))
}

fn analyze_and_run(
    root: &Path,
    args: &CheckArgs,
    rules: &[Arc<dyn Rule>],
) -> Result<(Vec<crate::diagnostics::Diagnostic>, crate::core::AnalysisDb)> {
    let config = load_config_for_check(root, &args.paths)?;
    let cache = crate::cache::Cache::default_for_repo(root, !args.no_cache);
    let config_digest = config_hash(&config);
    let enabled = selected_rule_patterns(&config, args.profile.as_deref())?;
    let mut options = BTreeMap::<String, RuleOptions>::new();
    for rule in rules {
        let meta = rule.meta();
        options.insert(
            meta.id.clone(),
            rule_options_from_config(config.rule_config(&meta.id)),
        );
    }
    let rule_digest = rule_hash(rules, enabled.as_ref(), &options);

    let mut db = load_analysis_files(&config)?;
    let mut diagnostics = Vec::new();
    diagnostics.extend(crate::go::analyze_with_options(
        &mut db,
        &cache,
        &config_digest,
        &rule_digest,
        true,
    ));
    diagnostics.extend(crate::ts::analyze_with_options(
        &mut db,
        &cache,
        &config_digest,
        &rule_digest,
        true,
    ));
    diagnostics.extend(run_rules(&db, rules, &options, enabled.as_ref(), true));
    Ok((diagnostics, db))
}

fn selected_rule_patterns(
    config: &LoadedConfig,
    profile: Option<&str>,
) -> Result<Option<BTreeSet<String>>> {
    Ok(config
        .profile_rules(profile)?
        .map(|rules| rules.into_iter().collect()))
}

fn load_config_for_check(root: &Path, paths: &[PathBuf]) -> Result<LoadedConfig> {
    let mut config = load_config(root)?;
    if !paths.is_empty() {
        config.config.workspace.include = paths
            .iter()
            .map(|path| check_path_pattern(root, path))
            .collect();
    }
    Ok(config)
}

fn check_path_pattern(root: &Path, path: &Path) -> String {
    let display_path = if path.is_absolute() {
        path.strip_prefix(root).unwrap_or(path)
    } else {
        path
    };
    let normalized = display_path
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_string();
    if normalized.contains(['*', '?', '[', ']', '{', '}']) {
        return normalized;
    }
    if root.join(&normalized).is_dir() || normalized.ends_with('/') {
        format!("{}/**", normalized.trim_end_matches('/'))
    } else {
        normalized
    }
}

fn config_hash(config: &LoadedConfig) -> String {
    let missing = if config.missing { "missing" } else { "loaded" };
    let serialized =
        serde_json::to_string(&config.config).expect("polint config should serialize to JSON");
    crate::cache::stable_hash(&[missing, &serialized])
}

fn rule_hash(
    rules: &[Arc<dyn Rule>],
    enabled: Option<&BTreeSet<String>>,
    options: &BTreeMap<String, RuleOptions>,
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
        if let Some(options) = options.get(&meta.id) {
            parts.push(format!("options:{}", deterministic_rule_options(options)));
        }
    }
    let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    crate::cache::stable_hash(&part_refs)
}

fn deterministic_rule_options(options: &RuleOptions) -> String {
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

pub fn rule_options_from_config(config: Option<&crate::config::RuleConfig>) -> RuleOptions {
    let Some(config) = config else {
        return RuleOptions::default();
    };
    RuleOptions {
        severity: config.severity.as_deref().and_then(parse_severity),
        files: config.files.clone(),
        allow_files: config.allow_files.clone(),
        allow: config.allow.clone(),
        max: config.max,
        deny: config.deny.clone(),
        forbidden_imports: config.forbidden_imports.clone(),
    }
}

fn parse_severity(value: &str) -> Option<Severity> {
    match value {
        "info" => Some(Severity::Info),
        "warn" | "warning" => Some(Severity::Warn),
        "error" => Some(Severity::Error),
        _ => None,
    }
}

fn exit_code_for(diagnostics: &[crate::diagnostics::Diagnostic], fail_on: FailOn) -> u8 {
    let threshold = match fail_on {
        FailOn::Warn => Some(Severity::Warn),
        FailOn::Error => Some(Severity::Error),
        FailOn::None => None,
    };
    if let Some(threshold) = threshold
        && diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity.is_at_least(threshold))
    {
        return 1;
    }
    0
}
