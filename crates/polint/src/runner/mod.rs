use crate::analysis_plan::{AnalysisPlan, RulePlanInputs};
use crate::config::{LoadedConfig, load_config};
use crate::core::{Rule, run_rules_with_capability_support};
use crate::diagnostics::{
    ColorChoice, JsonReportMeta, OutputFormat, RenderOpts, Severity, apply_report_filters,
    limit_report_diagnostics, render_with_sarif_help,
};
use crate::fs::load_analysis_files;
use crate::ignores::apply_ignores;
use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

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
    /// Only include diagnostics whose `rule_id` matches this pattern (same rules as profiles: exact id, `prefix/*`, or `*`).
    #[arg(long, value_name = "PATTERN")]
    only_rule: Option<String>,
    /// Emit at most this many diagnostics after stable sort (applies after `--only-rule`).
    #[arg(long, value_name = "N")]
    max_diagnostics: Option<usize>,
    /// Apply polint comment-ignore directives.
    #[arg(long, default_value_t = true, hide = true, action = clap::ArgAction::Set)]
    ignore_comments: bool,
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

pub fn run_cli(rules: Vec<Rule>) -> ExitCode {
    match run(rules) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("polint-local-rules: {error:?}");
            ExitCode::from(2)
        }
    }
}

fn run(rules: Vec<Rule>) -> Result<u8> {
    let cli = Cli::parse();
    match cli.command {
        Command::Check(args) => check(std::env::current_dir()?, &args, &rules),
    }
}

fn check(root: PathBuf, args: &CheckArgs, rules: &[Rule]) -> Result<u8> {
    let (mut diagnostics, db, loaded) = analyze_and_run(&root, args, rules)?;
    if args.ignore_comments {
        diagnostics = apply_ignores(&db, diagnostics, &loaded.config.ignores).diagnostics;
    }
    diagnostics = apply_report_filters(diagnostics, args.only_rule.as_deref());
    let rendered_diagnostics = limit_report_diagnostics(diagnostics.clone(), args.max_diagnostics);
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
    let sarif_map = if loaded.config.sarif.rule_help_uri.is_empty() {
        None
    } else {
        Some(&loaded.config.sarif.rule_help_uri)
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
    print!(
        "{}",
        render_with_sarif_help(format, &rendered_diagnostics, opts, sarif_map)
    );

    Ok(exit_code_for(&diagnostics, args.fail_on))
}

fn analyze_and_run(
    root: &Path,
    args: &CheckArgs,
    rules: &[Rule],
) -> Result<(
    Vec<crate::diagnostics::Diagnostic>,
    crate::core::AnalysisDb,
    LoadedConfig,
)> {
    let loaded = load_config_for_check(root, &args.paths)?;
    let cache = crate::cache::Cache::default_for_repo(root, !args.no_cache);
    let config_digest = crate::cache::keys::config_hash(&loaded);
    let enabled = selected_rule_patterns(&loaded, args.profile.as_deref())?;
    let plan_inputs = RulePlanInputs::collect(rules, enabled.as_ref());
    let options = plan_inputs.rule_options_from_config(&loaded);
    let rule_digest = plan_inputs.rule_digest(&options);
    let plan = AnalysisPlan::from_inputs(&plan_inputs, &options);

    let mut db = load_analysis_files(&loaded)?;
    let mut diagnostics = plan_inputs.diagnostics();
    diagnostics.extend(plan.diagnostics());
    let analyze_with_plan_options = crate::go::analyze_with_plan_options;
    let go_diagnostics =
        analyze_with_plan_options(&mut db, &cache, &config_digest, &rule_digest, &plan, true);
    diagnostics.extend(go_diagnostics);

    let analyze_with_plan_options = crate::ts::analyze_with_plan_options;
    let ts_diagnostics =
        analyze_with_plan_options(&mut db, &cache, &config_digest, &rule_digest, &plan, true);
    diagnostics.extend(ts_diagnostics);
    let module_graph = crate::module_graph::derive_requested_module_graph(&mut db, &loaded, &plan);
    let capability_support = module_graph.support_view(plan.support_view());
    diagnostics.extend(module_graph.diagnostics);
    crate::metrics::derive_requested_metrics(&mut db, &plan);
    diagnostics.extend(run_rules_with_capability_support(
        &db,
        rules,
        &options,
        enabled.as_ref(),
        true,
        &capability_support,
    ));
    Ok((diagnostics, db, loaded))
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
