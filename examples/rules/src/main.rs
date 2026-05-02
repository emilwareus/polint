use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use polint_config::{LoadedConfig, load_config};
use polint_core::{Rule, RuleOptions, run_rules};
use polint_diagnostics::{OutputFormat, Severity, dedupe_diagnostics, render};
use polint_example_rules::{example_rules, rule_options_from_config};
use polint_fs::load_analysis_files;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

#[derive(Debug, Parser)]
#[command(name = "polint-example-rules")]
#[command(about = "Runner for the example rules in this repository.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the example rules against the current directory.
    Check(CheckArgs),
}

#[derive(Debug, Args, Clone)]
struct CheckArgs {
    /// Profile from .polint.toml.
    #[arg(long, default_value = "fast")]
    profile: String,
    /// Output format.
    #[arg(long, value_enum, default_value_t = FormatArg::Human)]
    format: FormatArg,
    /// Disable .polint/cache reads and writes.
    #[arg(long)]
    no_cache: bool,
    /// Diagnostic level that fails the process.
    #[arg(long, value_enum, default_value_t = FailOn::Error)]
    fail_on: FailOn,
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

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("polint-example-rules: {error:?}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<u8> {
    let cli = Cli::parse();
    match cli.command {
        Command::Check(args) => check(std::env::current_dir()?, &args),
    }
}

fn check(root: PathBuf, args: &CheckArgs) -> Result<u8> {
    let mut diagnostics = analyze_and_run(&root, args)?;
    diagnostics = dedupe_diagnostics(diagnostics);

    let format = match args.format {
        FormatArg::Human => OutputFormat::Human,
        FormatArg::Json => OutputFormat::Json,
        FormatArg::Sarif => OutputFormat::Sarif,
    };
    print!("{}", render(format, &diagnostics));

    Ok(exit_code_for(&diagnostics, args.fail_on))
}

fn analyze_and_run(root: &Path, args: &CheckArgs) -> Result<Vec<polint_diagnostics::Diagnostic>> {
    let config = load_config(root)?;
    let cache = polint_cache::Cache::default_for_repo(root, !args.no_cache);
    let config_digest = config_hash(&config);
    let rules = example_rules();
    let enabled: BTreeSet<String> = config.profile_rules(&args.profile).into_iter().collect();
    let mut options = BTreeMap::<String, RuleOptions>::new();
    for rule in &rules {
        let meta = rule.meta();
        options.insert(
            meta.id.clone(),
            rule_options_from_config(config.rule_config(&meta.id)),
        );
    }
    let rule_digest = rule_hash(&rules, &enabled, &options);

    let mut db = load_analysis_files(&config)?;
    let mut diagnostics = Vec::new();
    diagnostics.extend(polint_go::analyze_with_options(
        &mut db,
        &cache,
        &config_digest,
        &rule_digest,
        true,
    ));
    diagnostics.extend(polint_ts::analyze_with_options(
        &mut db,
        &cache,
        &config_digest,
        &rule_digest,
        true,
    ));
    diagnostics.extend(run_rules(&db, &rules, &options, &enabled, true));
    Ok(diagnostics)
}

fn config_hash(config: &LoadedConfig) -> String {
    let missing = if config.missing { "missing" } else { "loaded" };
    let serialized =
        serde_json::to_string(&config.config).expect("polint config should serialize to JSON");
    polint_cache::stable_hash(&[missing, &serialized])
}

fn rule_hash(
    rules: &[Arc<dyn Rule>],
    enabled: &BTreeSet<String>,
    options: &BTreeMap<String, RuleOptions>,
) -> String {
    let mut parts = Vec::new();
    for rule in rules {
        let meta = rule.meta();
        if !enabled.is_empty()
            && !enabled
                .iter()
                .any(|pattern| polint_core::rule_id_matches(pattern, &meta.id))
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
    polint_cache::stable_hash(&part_refs)
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

fn exit_code_for(diagnostics: &[polint_diagnostics::Diagnostic], fail_on: FailOn) -> u8 {
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
