use crate::analysis_kernel::{AnalysisKernel, KernelInput};
use crate::analysis_plan::AnalysisPlan;
use crate::baseline::{
    BaselineConfig, BaselineSummary, DEFAULT_BASELINE_PATH, classify_diagnostics, load_baseline,
    render_baseline_summary, write_baseline,
};
use crate::cache::{
    CacheCleanReport, CacheCleanSelection, CacheLayout, CacheManagedCategory, CachePruneOptions,
    CachePruneReport, CacheStatus, POLINT_CACHE_DIR_ENV,
};
use crate::config::{LoadedConfig, default_config_toml, load_config};
use crate::core::{
    AnalysisDb, Language, ResolutionStatus, Rule, RuleOptions, SymbolPrecision,
    SymbolResolutionStatus, rule_id_matches, run_rules,
};
use crate::diagnostics::{
    AiFriendlyReport, ColorChoice, Diagnostic, JsonReportMeta, OutputFormat, RenderOpts, Severity,
    apply_report_filters, build_ai_friendly_report, diagnostics_from_public_json_report,
    limit_report_diagnostics, render_ai_friendly_stdout, render_with_sarif_help,
};
use crate::fs::load_analysis_files_scoped;
use crate::ignores::{apply_ignores, filter_report, render_ignore_report_human};
use crate::rule_manifest::InspectRuleReport;
use crate::rule_test::{RuleTestOptions, render_rule_test_human, run_rule_tests};
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod rules_host_error;
mod skill;

const POLINT_CACHE_STATUS_JSON_SCHEMA_V1_URL: &str = "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-cache-status-v1.json";
const POLINT_RULES_PROFILE_ENV: &str = "POLINT_RULES_PROFILE";
const AI_FRIENDLY_OUTPUT_DIR: &str = ".polint/output";
const AI_FRIENDLY_LATEST_OUTPUT: &str = ".polint/output/latest.json";
const REPORT_SOURCE_SNIPPET_MAX_BYTES: u64 = 1_048_576;

struct AiFriendlyOutput {
    report: AiFriendlyReport,
}

fn json_report_meta() -> JsonReportMeta<'static> {
    JsonReportMeta {
        tool_name: "polint",
        tool_version: env!("CARGO_PKG_VERSION"),
    }
}

fn render_opts<'a>(
    args: &CheckArgs,
    sources: Option<&'a BTreeMap<String, Arc<str>>>,
) -> RenderOpts<'a> {
    RenderOpts {
        json: json_report_meta(),
        color: match args.color {
            ColorArg::Auto => ColorChoice::Auto,
            ColorArg::Always => ColorChoice::Always,
            ColorArg::Never => ColorChoice::Never,
        },
        sources,
    }
}

fn sarif_help_map(config: &LoadedConfig) -> Option<&BTreeMap<String, String>> {
    if config.config.sarif.rule_help_uri.is_empty() {
        None
    } else {
        Some(&config.config.sarif.rule_help_uri)
    }
}

fn write_ai_friendly_report(
    root: &Path,
    diagnostics: &[Diagnostic],
    persisted_diagnostics: &[Diagnostic],
    json_meta: JsonReportMeta<'_>,
) -> Result<AiFriendlyOutput> {
    crate::repo_fs::ensure_repo_dir(root, AI_FRIENDLY_OUTPUT_DIR).with_context(|| {
        format!(
            "failed to create {}",
            root.join(AI_FRIENDLY_OUTPUT_DIR).display()
        )
    })?;
    ensure_polint_nested_gitignore(root)?;

    let generated_at = generated_at_label();
    let report = build_ai_friendly_report(
        diagnostics,
        persisted_diagnostics,
        json_meta,
        generated_at.clone(),
    );
    let json = serde_json::to_string_pretty(&report)?;
    let hash = crate::cache::stable_hash(&[&json]);
    let run_name = format!("check-{generated_at}-{}.json", &hash[..12]);
    let run_path = format!("{AI_FRIENDLY_OUTPUT_DIR}/{run_name}");
    crate::repo_fs::write_repo_file_atomic(root, &run_path, &json)
        .with_context(|| format!("failed to write {}", root.join(&run_path).display()))?;
    crate::repo_fs::write_repo_file_atomic(root, AI_FRIENDLY_LATEST_OUTPUT, json).with_context(
        || {
            format!(
                "failed to write {}",
                root.join(AI_FRIENDLY_LATEST_OUTPUT).display()
            )
        },
    )?;
    Ok(AiFriendlyOutput { report })
}

fn generated_at_label() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    secs.to_string()
}

fn read_sources_for_diagnostics(
    root: &Path,
    diagnostics: &[Diagnostic],
) -> BTreeMap<String, Arc<str>> {
    let mut map = BTreeMap::new();
    for diagnostic in diagnostics {
        if diagnostic.file.is_empty() || diagnostic.file == "<unknown>" {
            continue;
        }
        if map.contains_key(&diagnostic.file) {
            continue;
        }
        let rel = diagnostic.file.trim_start_matches("./");
        if let Ok(text) = crate::repo_fs::read_repo_file_to_string_with_limit(
            root,
            rel,
            REPORT_SOURCE_SNIPPET_MAX_BYTES,
        ) {
            map.insert(diagnostic.file.clone(), Arc::from(text.into_boxed_str()));
        }
    }
    map
}

#[derive(Debug, Parser)]
#[command(name = "polint")]
#[command(about = "Repo-local static analysis policy as code.")]
#[command(
    after_help = "AI agents: prefer `polint check --format ai-friendly --fail-on none` to avoid context overload."
)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Create `.polint.toml`, `.polint/rules/src`, `.polint/cache`, `.polint/.gitignore`, and root
    /// `rust-toolchain.toml` when missing (matches polint's MSRV for building rule packs).
    Init,
    /// Install a repo-local AI-agent skill for using polint.
    AddSkill(skill::AddSkillArgs),
    /// Scaffold a repo-local Rust rule.
    NewRule(NewRuleArgs),
    /// Create or update a compact YAML diagnostic baseline.
    Baseline(BaselineArgs),
    /// Inspect, prune, or clean polint caches.
    Cache(CacheArgs),
    /// Inspect polint configuration and repo-local rule manifests.
    Inspect(InspectArgs),
    /// List or sample supported public fact views.
    Facts(FactsArgs),
    /// Report public setup and resolution unknowns.
    Unknowns(UnknownsArgs),
    /// Explain rule capability planning.
    Explain(ExplainArgs),
    /// Run repo-local rule fixture tests.
    Test(TestArgs),
    /// Run enabled rules.
    Check(CheckArgs),
    /// Run review-kind rules against a diff to a target branch/commit.
    Review(ReviewArgs),
    /// Inspect polint comment-ignore directives and suppression statistics.
    Ignores(IgnoresArgs),
}

#[derive(Debug, Args, Clone)]
struct NewRuleArgs {
    /// Rule language focus: go, ts, js, or generic.
    language: String,
    /// Rule directory/name.
    rule_name: String,
    /// Scaffold a review-kind rule (`kind = "review"`) with a `ChangedFiles<'_>`
    /// parameter, run via `polint review <ref>` instead of `polint check`.
    #[arg(long)]
    review: bool,
}

#[derive(Debug, Args, Clone)]
struct BaselineArgs {
    #[command(subcommand)]
    command: BaselineCommand,
}

#[derive(Debug, Args, Clone)]
struct CacheArgs {
    #[command(subcommand)]
    command: CacheCommand,
}

#[derive(Debug, Args, Clone)]
struct InspectArgs {
    #[command(subcommand)]
    command: InspectCommand,
}

#[derive(Debug, Args, Clone)]
struct FactsArgs {
    #[command(subcommand)]
    command: FactsCommand,
}

#[derive(Debug, Subcommand, Clone)]
enum FactsCommand {
    /// List supported and reserved public fact-view capabilities.
    List(FactsListArgs),
    /// Sample a bounded set of public facts for one capability.
    Sample(FactsSampleArgs),
}

#[derive(Debug, Args, Clone)]
struct FactsListArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = AgentJsonFormatArg::Json)]
    format: AgentJsonFormatArg,
}

#[derive(Debug, Args, Clone)]
struct FactsSampleArgs {
    /// Capability to sample, such as resolved_imports, symbols, references, or file_metrics.
    #[arg(long = "cap", value_name = "CAPABILITY")]
    capability: String,
    /// Maximum number of rows to return.
    #[arg(long, value_name = "N", default_value_t = 10)]
    limit: usize,
    /// Output format.
    #[arg(long, value_enum, default_value_t = AgentJsonFormatArg::Json)]
    format: AgentJsonFormatArg,
    /// Files or directories to analyze. Defaults to workspace config.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Disable analysis/fact cache reads and writes.
    #[arg(long)]
    no_cache: bool,
}

#[derive(Debug, Args, Clone)]
struct UnknownsArgs {
    /// Capability to inspect, such as resolved_imports, symbols, references, or dataflow.
    #[arg(long = "cap", value_name = "CAPABILITY")]
    capability: String,
    /// Output format.
    #[arg(long, value_enum, default_value_t = AgentJsonFormatArg::Json)]
    format: AgentJsonFormatArg,
    /// Files or directories to analyze. Defaults to workspace config.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Disable analysis/fact cache reads and writes.
    #[arg(long)]
    no_cache: bool,
}

#[derive(Debug, Args, Clone)]
struct ExplainArgs {
    /// Rule id to explain. When omitted, all discovered rules are included.
    #[arg(long, value_name = "RULE_ID")]
    rule: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = AgentJsonFormatArg::Json)]
    format: AgentJsonFormatArg,
}

#[derive(Debug, Subcommand, Clone)]
enum InspectCommand {
    /// Show registered repo-local rule manifests.
    Rule(InspectRuleArgs),
    /// Show consolidated setup, unsupported, budget, model, and resolution unknowns.
    Unknowns(InspectUnknownsArgs),
}

#[derive(Debug, Args, Clone)]
struct InspectRuleArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = InspectFormatArg::Human)]
    format: InspectFormatArg,
    /// Only include rules whose id matches this pattern.
    #[arg(long, value_name = "RULE_ID")]
    rule: Option<String>,
}

#[derive(Debug, Args, Clone)]
struct InspectUnknownsArgs {
    /// Optional capability filter, such as resolved_imports, symbols, or references.
    #[arg(long = "cap", value_name = "CAPABILITY")]
    capability: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = AgentJsonFormatArg::Json)]
    format: AgentJsonFormatArg,
    /// Files or directories to analyze. Defaults to workspace config.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Disable analysis/fact cache reads and writes.
    #[arg(long)]
    no_cache: bool,
}

#[derive(Debug, Args, Clone)]
struct TestArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = TestFormatArg::Human)]
    format: TestFormatArg,
    /// Only run fixture cases whose manifest rule id matches this pattern.
    #[arg(long, value_name = "RULE_ID")]
    rule: Option<String>,
    /// Only run fixture cases with this case directory name.
    #[arg(long, value_name = "CASE_NAME")]
    case: Option<String>,
    /// Disable analysis/fact cache reads and writes while running fixture checks.
    #[arg(long)]
    no_cache: bool,
    /// Keep temporary fixture repositories on disk.
    #[arg(long)]
    keep_temp: bool,
}

#[derive(Debug, Subcommand, Clone)]
enum CacheCommand {
    /// Show cache paths, size, and file counts.
    Status(CacheStatusArgs),
    /// Remove cache files by size or age while preserving newer entries.
    Prune(CachePruneArgs),
    /// Remove a whole cache category.
    Clean(CacheCleanArgs),
}

#[derive(Debug, Args, Clone)]
struct CacheStatusArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = CacheStatusFormatArg::Human)]
    format: CacheStatusFormatArg,
}

#[derive(Debug, Args, Clone)]
struct CachePruneArgs {
    /// Cache category to prune. Defaults to all managed categories.
    #[arg(long, value_enum)]
    category: Option<CacheCategoryArg>,
    /// Remove files older than this many days.
    #[arg(long, value_name = "DAYS")]
    max_age_days: Option<u64>,
    /// Keep selected categories at or below this many MiB.
    #[arg(long, value_name = "MIB")]
    max_size_mb: Option<u64>,
    /// Print what would be removed without deleting files.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args, Clone)]
struct CacheCleanArgs {
    /// Cache category to clean. Defaults to all.
    #[arg(long, value_enum, default_value_t = CacheCategoryArg::All)]
    category: CacheCategoryArg,
}

#[derive(Debug, Subcommand, Clone)]
enum BaselineCommand {
    /// Write current diagnostics to .polint/baseline.yaml.
    Create(BaselineCreateArgs),
    /// Remove fixed entries from .polint/baseline.yaml and refresh moved paths.
    Update(BaselineUpdateArgs),
}

#[derive(Debug, Args, Clone)]
struct BaselineCreateArgs {
    /// Overwrite an existing baseline file.
    #[arg(long)]
    force: bool,
    /// Named profile from .polint.toml. When omitted, all discovered rules run.
    #[arg(long)]
    profile: Option<String>,
    /// Disable analysis/fact cache reads and writes while creating the baseline.
    #[arg(long)]
    no_cache: bool,
    /// Files or directories to baseline. Defaults to the workspace include config.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Args, Clone)]
struct BaselineUpdateArgs {
    /// Named profile from .polint.toml. When omitted, all discovered rules run.
    #[arg(long)]
    profile: Option<String>,
    /// Disable analysis/fact cache reads and writes while updating the baseline.
    #[arg(long)]
    no_cache: bool,
    /// Files or directories to check. Defaults to the workspace include config.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Args, Clone)]
#[command(
    after_help = "AI agents: use `--format ai-friendly` to print a compact summary and save queryable JSON under `.polint/output/`."
)]
struct CheckArgs {
    /// Files or directories to check. Defaults to the workspace include config.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Named profile from .polint.toml. When omitted, all discovered rules run.
    #[arg(long)]
    profile: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = FormatArg::Human)]
    format: FormatArg,
    /// ANSI colors for human output (no effect on JSON/SARIF). `auto` uses color only for a tty and when `NO_COLOR` is unset.
    #[arg(long, value_enum, default_value_t = ColorArg::Auto)]
    color: ColorArg,
    /// Disable analysis/fact cache reads and writes. Does not disable the repo-local rule-host Cargo target cache.
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
    /// Print grouped scan statistics for human output.
    #[arg(long)]
    stat: bool,
    /// Print one scan summary line for human output.
    #[arg(long)]
    shortstat: bool,
    /// Suppress known diagnostics from .polint/baseline.yaml.
    #[arg(long)]
    baseline: bool,
    /// Emit and fail only on diagnostics not covered by `--baseline`.
    #[arg(long)]
    new_only: bool,
    /// Apply polint comment-ignore directives.
    #[arg(long, default_value_t = true, hide = true, action = clap::ArgAction::Set)]
    ignore_comments: bool,
}

#[derive(Debug, Args, Clone)]
struct ReviewArgs {
    /// Target branch or commit to diff against (e.g. origin/main, a SHA, or a...b).
    #[arg(value_name = "REF")]
    reff: String,
    /// Files or directories to review. Defaults to the workspace include config.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Named profile from .polint.toml. When omitted, all discovered rules run.
    #[arg(long)]
    profile: Option<String>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = FormatArg::Human)]
    format: FormatArg,
    /// ANSI colors for human output (no effect on JSON/SARIF). `auto` uses color only for a tty and when `NO_COLOR` is unset.
    #[arg(long, value_enum, default_value_t = ColorArg::Auto)]
    color: ColorArg,
    /// Disable analysis/fact cache reads and writes. Does not disable the repo-local rule-host Cargo target cache.
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
    /// Surface ALL review-rule findings, not just those intersecting the diff.
    #[arg(long)]
    no_diff_gate: bool,
    /// Gate on changed FILES only (ignore line ranges) when diff-gating.
    #[arg(long)]
    whole_file: bool,
}

#[derive(Debug, Args, Clone)]
struct IgnoresArgs {
    /// Files or directories to inspect. Defaults to the workspace include config.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Named profile from .polint.toml. When omitted, all discovered rules run.
    #[arg(long)]
    profile: Option<String>,
    /// Disable analysis/fact cache reads and writes while collecting diagnostics.
    #[arg(long)]
    no_cache: bool,
    /// Comma-separated rule selectors to show, e.g. `local/no-todo,local/*`.
    #[arg(long)]
    filter: Option<String>,
    /// Print grouped ignore statistics.
    #[arg(long)]
    stat: bool,
    /// Print one summary line.
    #[arg(long)]
    shortstat: bool,
    /// Output format.
    #[arg(long, value_enum, default_value_t = IgnoresFormatArg::Human)]
    format: IgnoresFormatArg,
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
    Github,
    Json,
    Sarif,
    AiFriendly,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum IgnoresFormatArg {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InspectFormatArg {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum TestFormatArg {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AgentJsonFormatArg {
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CacheStatusFormatArg {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CacheCategoryArg {
    All,
    Analysis,
    RulesTarget,
    Derived,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FailOn {
    Warn,
    Error,
    None,
}

pub(crate) fn run() -> Result<u8> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init()
        .ok();

    let cli = Cli::parse();
    match cli.command {
        Command::Init => {
            init_project(std::env::current_dir()?)?;
            Ok(0)
        }
        Command::AddSkill(args) => {
            skill::add_skill(std::env::current_dir()?, &args)?;
            Ok(0)
        }
        Command::NewRule(args) => {
            new_rule(std::env::current_dir()?, &args)?;
            Ok(0)
        }
        Command::Baseline(args) => baseline(std::env::current_dir()?, &args),
        Command::Cache(args) => cache_command(std::env::current_dir()?, &args),
        Command::Inspect(args) => inspect(std::env::current_dir()?, &args),
        Command::Facts(args) => facts(std::env::current_dir()?, &args),
        Command::Unknowns(args) => unknowns(std::env::current_dir()?, &args),
        Command::Explain(args) => explain(std::env::current_dir()?, &args),
        Command::Test(args) => test(std::env::current_dir()?, &args),
        Command::Check(args) => check(std::env::current_dir()?, &args),
        Command::Review(args) => review(std::env::current_dir()?, &args),
        Command::Ignores(args) => ignores(std::env::current_dir()?, &args),
    }
}

fn init_project(root: PathBuf) -> Result<()> {
    let config_path = root.join(".polint.toml");
    if !config_path.exists() {
        crate::repo_fs::write_repo_file_atomic(&root, ".polint.toml", default_config_toml())
            .with_context(|| format!("failed to write {}", config_path.display()))?;
    }

    let polint_dir = root.join(".polint");
    crate::repo_fs::ensure_repo_dir(&root, ".polint/rules/src")
        .with_context(|| format!("failed to create {}", polint_dir.display()))?;
    crate::repo_fs::ensure_repo_dir(&root, ".polint/cache")
        .with_context(|| format!("failed to create {}", polint_dir.join("cache").display()))?;
    crate::repo_fs::ensure_repo_dir(&root, ".polint/output")
        .with_context(|| format!("failed to create {}", polint_dir.join("output").display()))?;
    ensure_polint_nested_gitignore(&root)?;
    ensure_repo_rust_toolchain_shim(&root)?;

    println!("Initialized polint config at {}", config_path.display());
    Ok(())
}

/// Ensures `.polint/.gitignore` lists `cache/` so analysis cache stays local to each machine.
fn ensure_polint_nested_gitignore(root: &Path) -> Result<()> {
    let path = root.join(".polint/.gitignore");
    const ENTRIES: &[&str] = &["cache/", "output/"];

    if let Ok(existing) =
        crate::repo_fs::read_repo_file_to_string_with_limit(root, ".polint/.gitignore", 1_048_576)
    {
        if ENTRIES.iter().all(|entry| {
            existing
                .lines()
                .any(|line| gitignore_line_covers(line, entry))
        }) {
            return Ok(());
        }
        let mut out = existing;
        if !out.ends_with('\n') {
            out.push('\n');
        }
        for entry in ENTRIES {
            if !out.lines().any(|line| gitignore_line_covers(line, entry)) {
                out.push_str(entry);
                out.push('\n');
            }
        }
        crate::repo_fs::write_repo_file_atomic(root, ".polint/.gitignore", out)
            .with_context(|| format!("failed to update {}", path.display()))?;
    } else {
        let content = "# polint: local cache and agent output (not shared between checkouts)\ncache/\noutput/\n";
        crate::repo_fs::write_repo_file_atomic(root, ".polint/.gitignore", content)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }
    Ok(())
}

/// Writes `rust-toolchain.toml` at the repo root when absent so `cargo` (invoked from `polint
/// check` with `--manifest-path .polint/rules/Cargo.toml`) picks a toolchain that can build `polint`.
fn ensure_repo_rust_toolchain_shim(root: &Path) -> Result<()> {
    let path = root.join("rust-toolchain.toml");
    if path.exists() {
        return Ok(());
    }
    let msrv = env!("CARGO_PKG_RUST_VERSION");
    fs::write(
        &path,
        format!(
            "# Polint rule packs compile the `polint` crate (MSRV Rust {msrv}).\n\
             # https://github.com/emilwareus/polint#minimum-rust-version\n\
             [toolchain]\n\
             channel = \"{msrv}\"\n"
        ),
    )
    .with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn gitignore_line_covers(line: &str, entry: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return false;
    }
    t.trim_end_matches('/') == entry.trim_end_matches('/')
}

fn new_rule(root: PathBuf, args: &NewRuleArgs) -> Result<()> {
    let rule_name = validate_rule_name(&args.rule_name)?;
    let rules_dir = root.join(".polint/rules");
    let module = rust_module_name(&rule_name);
    let module_path = rules_dir.join("src").join(format!("{module}.rs"));

    match fs::symlink_metadata(&module_path) {
        Ok(_) => anyhow::bail!("rule already exists: {}", module_path.display()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect {}", module_path.display()));
        }
    }

    fs::create_dir_all(rules_dir.join("src"))
        .with_context(|| format!("failed to create {}", rules_dir.display()))?;

    let pack_toml = rules_dir.join("Cargo.toml");
    if !pack_toml.is_file() {
        write_pack_cargo_toml(&rules_dir)?;
        write_initial_pack_main(&rules_dir, &module)?;
    } else if !rules_dir.join("src/main.rs").is_file() {
        write_initial_pack_main(&rules_dir, &module)?;
    } else {
        prepend_pack_mod_use(&rules_dir, &module)?;
        append_rule_to_run_cli_vec(&rules_dir, &module)?;
    }

    fs::write(
        &module_path,
        rule_module_template(&args.language, &rule_name, args.review),
    )?;
    if args.review {
        // A review rule's diagnostics depend on a diff, so the static
        // positive/negative `polint check` fixtures do not apply. Review rules
        // are exercised with `polint review <ref>`.
        println!(
            "Created review rule module {} (run it with `polint review <ref>`; \
             no diff-based fixtures were generated)",
            module_path.display()
        );
    } else {
        write_rule_fixture_skeleton(&root, &args.language, &rule_name, &module)?;
        println!("Created rule module {}", module_path.display());
    }
    Ok(())
}

fn rust_module_name(rule_name: &str) -> String {
    rule_name.replace('-', "_")
}

fn polint_deps_path_prefix(rules_dir: &Path) -> Option<String> {
    let mut dir = rules_dir.to_path_buf();
    let mut up = 0usize;
    loop {
        if dir.join("crates/polint").is_dir() {
            return Some(format!("{}crates/", "../".repeat(up)));
        }
        if !dir.pop() {
            return None;
        }
        up += 1;
    }
}

fn write_pack_cargo_toml(rules_dir: &Path) -> Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let polint_dep_line = match polint_deps_path_prefix(rules_dir) {
        Some(prefix) => format!(r#"polint = {{ path = "{prefix}polint" }}"#),
        None => format!(r#"polint = "{version}""#),
    };
    fs::write(
        rules_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "polint-local-rules"
version = "{version}"
edition = "2024"
publish = false

[dependencies]
{polint_dep_line}

[workspace]
"#,
            version = version,
            polint_dep_line = polint_dep_line,
        ),
    )
    .with_context(|| format!("failed to write {}", rules_dir.join("Cargo.toml").display()))?;
    Ok(())
}

fn write_initial_pack_main(rules_dir: &Path, module: &str) -> Result<()> {
    let initial = format!(
        r#"mod {module};

use std::process::ExitCode;

fn main() -> ExitCode {{
    polint::runner::run_cli(vec![
        {module}::{module}(),
    ])
}}
"#
    );
    fs::write(rules_dir.join("src/main.rs"), initial).with_context(|| {
        format!(
            "failed to write {}",
            rules_dir.join("src/main.rs").display()
        )
    })?;
    Ok(())
}

fn prepend_pack_mod_use(rules_dir: &Path, module: &str) -> Result<()> {
    let main_path = rules_dir.join("src/main.rs");
    let mut src = fs::read_to_string(&main_path)
        .with_context(|| format!("failed to read {}", main_path.display()))?;
    let mod_decl = format!("mod {module};");
    if !src.contains(&mod_decl) {
        src.insert_str(0, &format!("{mod_decl}\n"));
        fs::write(&main_path, src)?;
    }
    Ok(())
}

fn append_rule_to_run_cli_vec(rules_dir: &Path, module: &str) -> Result<()> {
    let main_path = rules_dir.join("src/main.rs");
    let src = fs::read_to_string(&main_path)
        .with_context(|| format!("failed to read {}", main_path.display()))?;
    let needle = "polint::runner::run_cli(vec![";
    let start = src.find(needle).with_context(|| {
        format!(
            "{} must call polint::runner::run_cli(vec![...])",
            main_path.display()
        )
    })?;
    let inner_start = start + needle.len();
    let mut depth = 1u32;
    let mut end = None;
    for (i, ch) in src[inner_start..].char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(inner_start + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end.with_context(|| format!("unclosed vec![ in {}", main_path.display()))?;
    let inner = &src[inner_start..end];
    let trimmed = inner.trim_end();
    let new_inner = if trimmed.is_empty() {
        format!("\n        {module}::{module}(),\n    ")
    } else {
        format!("{trimmed}\n        {module}::{module}(),\n    ")
    };
    let mut new_src = String::new();
    new_src.push_str(&src[..inner_start]);
    new_src.push_str(&new_inner);
    new_src.push_str(&src[end..]);
    fs::write(&main_path, new_src)?;
    Ok(())
}

fn validate_rule_name(name: &str) -> Result<String> {
    let sanitized = sanitize_name(name);
    let path = Path::new(name);
    if name.is_empty()
        || sanitized != name
        || path
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
    {
        anyhow::bail!(
            "rule name must be a non-empty safe path component using only ASCII letters, digits, and '-'"
        );
    }
    Ok(sanitized)
}

fn write_rule_fixture_skeleton(
    root: &Path,
    language: &str,
    rule_name: &str,
    module: &str,
) -> Result<()> {
    let rule_tests_dir = root.join(".polint/tests/rules").join(module);
    if rule_tests_dir.exists() {
        return Ok(());
    }
    let fixture_file = match language {
        "go" => "src/example.go",
        _ => "src/example.ts",
    };
    write_rule_fixture_case(
        &rule_tests_dir.join("positive"),
        language,
        &rule_fixture_positive_source_template(language),
        &rule_fixture_manifest_template(rule_name, fixture_file, false),
    )?;
    write_rule_fixture_case(
        &rule_tests_dir.join("negative"),
        language,
        &rule_fixture_negative_source_template(language),
        &rule_fixture_manifest_template(rule_name, fixture_file, true),
    )?;
    Ok(())
}

fn write_rule_fixture_case(
    case_dir: &Path,
    language: &str,
    source: &str,
    manifest: &str,
) -> Result<()> {
    let source_path = match language {
        "go" => case_dir.join("src/example.go"),
        _ => case_dir.join("src/example.ts"),
    };
    fs::create_dir_all(source_path.parent().unwrap_or(case_dir))
        .with_context(|| format!("failed to create {}", case_dir.display()))?;
    fs::write(case_dir.join("polint-test.toml"), manifest).with_context(|| {
        format!(
            "failed to write {}",
            case_dir.join("polint-test.toml").display()
        )
    })?;
    fs::write(&source_path, source)
        .with_context(|| format!("failed to write {}", source_path.display()))?;
    Ok(())
}

fn rule_fixture_manifest_template(
    rule_name: &str,
    fixture_file: &str,
    expect_diagnostic: bool,
) -> String {
    let expected = if expect_diagnostic {
        format!(
            r#"
[[expect.diagnostic]]
rule_id = "custom/{rule_name}"
file = "{fixture_file}"
severity = "warn"
message_contains = "Project-specific policy"
"#
        )
    } else {
        String::new()
    };
    format!(
        r#"rule = "custom/{rule_name}"
paths = ["src/**"]

[expect]
{expected}
"#
    )
}

fn rule_fixture_positive_source_template(language: &str) -> String {
    match language {
        "go" => r#"package main

func main() {}
"#
        .to_string(),
        _ => r#"export const example = "allowed";
"#
        .to_string(),
    }
}

fn rule_fixture_negative_source_template(language: &str) -> String {
    match language {
        "go" => r#"package main

func replaceMe(err error) error {
	if err != nil {
		return err
	}
	return nil
}
"#
        .to_string(),
        _ => r#"export const example = "replace me";
"#
        .to_string(),
    }
}

fn rule_module_template(language: &str, rule_name: &str, review: bool) -> String {
    if review {
        return review_rule_module_template(rule_name);
    }
    let module = rust_module_name(rule_name);
    let fact_params = match language {
        "go" => "tests: GoTests<'_>, branches: BranchObligations<'_>",
        "ts" | "tsx" | "js" | "jsx" => "literals: StringLiterals<'_>, jsx: JsxAttributes<'_>",
        _ => "functions: Functions<'_>",
    };
    let query_example = match language {
        "go" => {
            r#"    for branch in branches.iter() {
        if branch.is_error_path && tests.related_for_file(branch.file).is_empty() {
            ctx.warn(
                &branch.decision_span,
                "Project-specific policy (heuristic): add nearby test evidence for this Go error branch.",
            );
        }
    }"#
        }
        "ts" | "tsx" | "js" | "jsx" => {
            r#"    for literal in literals.iter() {
        if literal.value == "replace me" {
            ctx.warn(&literal.span, "Project-specific policy: replace this placeholder literal.");
        }
    }
    let attribute_count = jsx.iter().count();
    let _ = attribute_count;"#
        }
        _ => {
            r#"    let function_count = functions.iter().count();
    let _ = function_count;"#
        }
    };
    format!(
        r#"use polint::sdk::prelude::*;

#[polint::rule(
    id = "custom/{rule_name}",
    description = "Project-specific policy: {rule_name}.",
    severity = "warn"
)]
pub(crate) fn {module}(ctx: &mut RuleCtx<'_>, {fact_params}) -> RuleResult {{
{query_example}
    let _ = ctx.options().settings.len();
    Ok(())
}}
"#
    )
}

/// Scaffold a review-kind rule: `kind = "review"` plus a `ChangedFiles<'_>`
/// parameter and a glob-match loop. Run with `polint review <ref>`.
fn review_rule_module_template(rule_name: &str) -> String {
    let module = rust_module_name(rule_name);
    format!(
        r#"use polint::sdk::prelude::*;

#[polint::rule(
    id = "review/{rule_name}",
    description = "Review-only policy (heuristic): {rule_name}.",
    severity = "warn",
    kind = "review"
)]
pub(crate) fn {module}(ctx: &mut RuleCtx<'_>, changes: ChangedFiles<'_>) -> RuleResult {{
    // `changes` is the diff against the target ref passed to `polint review`.
    // It is empty under `polint check`, so this rule only fires on a review.
    let rule_id = ctx.rule_id().to_string();
    for changed in changes.iter() {{
        // Replace the glob with the paths your policy cares about.
        if changed.matches_glob("db/migrations/**") {{
            ctx.report(Diagnostic::warning(
                rule_id.clone(),
                changed.path().to_string(),
                DiagnosticRange::point(1, 1),
                "Reviewer attention required: a watched path changed.",
            ));
        }}
    }}
    Ok(())
}}
"#
    )
}

fn baseline(root: PathBuf, args: &BaselineArgs) -> Result<u8> {
    match &args.command {
        BaselineCommand::Create(create_args) => create_baseline(root, create_args),
        BaselineCommand::Update(update_args) => update_baseline(root, update_args),
    }
}

fn create_baseline(root: PathBuf, args: &BaselineCreateArgs) -> Result<u8> {
    let output = baseline_path(&root);
    if output.exists() && !args.force {
        anyhow::bail!(
            "baseline file already exists: {}; use --force to overwrite or `polint baseline update`",
            output.display()
        );
    }

    let diagnostics = collect_diagnostics_for_baseline(
        &root,
        &args.paths,
        args.profile.as_deref(),
        args.no_cache,
    )?;
    let config = BaselineConfig::from_diagnostics(&diagnostics);
    write_baseline(&root, &config)?;
    println!(
        "Created baseline {} with {} entries",
        output.display(),
        config.baseline.len()
    );
    Ok(0)
}

fn update_baseline(root: PathBuf, args: &BaselineUpdateArgs) -> Result<u8> {
    let baseline_path = baseline_path(&root);
    let config = load_baseline(&root)?;
    let diagnostics = collect_diagnostics_for_baseline(
        &root,
        &args.paths,
        args.profile.as_deref(),
        args.no_cache,
    )?;
    let classification = classify_diagnostics(&diagnostics, &config);
    let updated = config.updated_for_current_diagnostics(&diagnostics);
    write_baseline(&root, &updated)?;
    println!(
        "Updated baseline {} with {} entries",
        baseline_path.display(),
        updated.baseline.len()
    );
    print!("{}", render_baseline_summary(&classification.summary));
    Ok(0)
}

fn cache_command(root: PathBuf, args: &CacheArgs) -> Result<u8> {
    let layout = CacheLayout::for_repo(&root);
    match &args.command {
        CacheCommand::Status(status_args) => cache_status(&layout, status_args),
        CacheCommand::Prune(prune_args) => cache_prune(&layout, prune_args),
        CacheCommand::Clean(clean_args) => cache_clean(&layout, clean_args),
    }
}

fn inspect(root: PathBuf, args: &InspectArgs) -> Result<u8> {
    match &args.command {
        InspectCommand::Rule(rule_args) => inspect_rule(&root, rule_args),
        InspectCommand::Unknowns(unknowns_args) => inspect_unknowns(root, unknowns_args),
    }
}

fn facts(root: PathBuf, args: &FactsArgs) -> Result<u8> {
    match &args.command {
        FactsCommand::List(list_args) => facts_list(list_args),
        FactsCommand::Sample(sample_args) => facts_sample(&root, sample_args),
    }
}

fn facts_list(_args: &FactsListArgs) -> Result<u8> {
    println!("{}", serde_json::to_string_pretty(&FactsListReport::new())?);
    Ok(0)
}

fn facts_sample(root: &Path, args: &FactsSampleArgs) -> Result<u8> {
    let limit = args.limit.min(100);
    let support = public_fact_view(args.capability.as_str())
        .ok_or_else(|| anyhow::anyhow!("unknown public fact capability `{}`", args.capability))?;
    if !support.sampling {
        anyhow::bail!(
            "public fact capability `{}` is reserved and does not support sampling yet; see {}",
            args.capability,
            support.docs_path
        );
    }
    let analysis = analyze_for_agent_json(
        root,
        &args.paths,
        args.no_cache,
        &[args.capability.as_str()],
    )?;
    let db = &analysis.db;
    let mut rows = match args.capability.as_str() {
        "resolved_imports" => db
            .resolved_imports()
            .iter()
            .map(|fact| {
                PublicFactSampleRow::new(
                    db.path_for(fact.from_file),
                    None,
                    status_label(fact.status),
                    Some(resolution_precision_label(fact.precision).to_string()),
                    Some(format!("resolved_import:{}", fact.id.0)),
                )
            })
            .collect(),
        "module_graph" => db
            .module_edges()
            .iter()
            .map(|edge| {
                PublicFactSampleRow::new(
                    "<module-graph>".to_string(),
                    None,
                    status_label(edge.status),
                    None,
                    Some(format!("module_edge:{}", edge.id.0)),
                )
            })
            .collect(),
        "symbols" => db
            .symbols()
            .iter()
            .map(|symbol| {
                PublicFactSampleRow::new(
                    symbol
                        .file
                        .map(|file| db.path_for(file))
                        .unwrap_or_else(|| "<workspace>".to_string()),
                    symbol.primary_span.as_ref().map(span_start),
                    "present",
                    Some(symbol_precision_label(symbol.precision).to_string()),
                    Some(symbol.stable_key.clone()),
                )
            })
            .collect(),
        "references" => db
            .references()
            .iter()
            .map(|reference| {
                PublicFactSampleRow::new(
                    reference
                        .file
                        .map(|file| db.path_for(file))
                        .unwrap_or_else(|| "<workspace>".to_string()),
                    reference.primary_span.as_ref().map(span_start),
                    symbol_status_label(reference.status),
                    Some(symbol_precision_label(reference.precision).to_string()),
                    Some(reference.stable_key.clone()),
                )
            })
            .collect(),
        "file_metrics" => db
            .file_metrics()
            .iter()
            .map(|metric| {
                PublicFactSampleRow::new(
                    db.path_for(metric.file),
                    None,
                    "present",
                    None,
                    Some(format!(
                        "lines:{};bytes:{};functions:{}",
                        metric.line_count, metric.byte_count, metric.function_count
                    )),
                )
            })
            .collect(),
        "function_metrics" => db
            .function_metrics()
            .iter()
            .map(|metric| {
                PublicFactSampleRow::new(
                    db.path_for(metric.file),
                    Some(span_start(&metric.span)),
                    "present",
                    None,
                    Some(format!(
                        "function:{};lines:{};bytes:{}",
                        metric.name, metric.line_count, metric.byte_count
                    )),
                )
            })
            .collect(),
        "complexity_metrics" => db
            .complexity_metrics()
            .iter()
            .map(|metric| {
                PublicFactSampleRow::new(
                    db.path_for(metric.file),
                    Some(span_start(&metric.span)),
                    "present",
                    None,
                    Some(format!(
                        "function:{};cyclomatic_complexity:{}",
                        metric.name, metric.cyclomatic_complexity
                    )),
                )
            })
            .collect(),
        _ => Vec::new(),
    };
    rows.sort_by(|left, right| {
        (
            left.file.as_str(),
            left.span.as_ref().map(|span| (span.line, span.column)),
            left.stable_id.as_deref().unwrap_or_default(),
        )
            .cmp(&(
                right.file.as_str(),
                right.span.as_ref().map(|span| (span.line, span.column)),
                right.stable_id.as_deref().unwrap_or_default(),
            ))
    });
    rows.truncate(limit);
    let report = FactsSampleReport {
        version: 1,
        schema: POLINT_FACTS_JSON_SCHEMA_V1_URL.to_string(),
        tool: polint_tool_info(),
        capability: args.capability.clone(),
        limit,
        sampled: rows.len(),
        rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(0)
}

fn unknowns(root: PathBuf, args: &UnknownsArgs) -> Result<u8> {
    let support = public_fact_view(&args.capability);
    if support
        .as_ref()
        .is_none_or(|view| view.stability != "stable" || !view.unknowns)
    {
        let row = crate::analysis::unknown_taxonomy::collect::unsupported_capability_row(
            &args.capability,
            support.map(|view| view.docs_path),
        );
        let report = UnknownsReport {
            version: 1,
            schema: POLINT_UNKNOWNS_JSON_SCHEMA_V1_URL.to_string(),
            tool: polint_tool_info(),
            capability: args.capability.clone(),
            rows: vec![UnknownsRow::from_taxonomy_compat(row)],
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(2);
    }

    let analysis = analyze_for_agent_json(
        &root,
        &args.paths,
        args.no_cache,
        &[args.capability.as_str()],
    )?;
    let rows = crate::analysis::unknown_taxonomy::collect::public_capability_unknowns(
        &analysis.db,
        &args.capability,
    )
    .into_iter()
    .map(UnknownsRow::from_taxonomy_compat)
    .collect();
    let report = UnknownsReport {
        version: 1,
        schema: POLINT_UNKNOWNS_JSON_SCHEMA_V1_URL.to_string(),
        tool: polint_tool_info(),
        capability: args.capability.clone(),
        rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(0)
}

fn inspect_unknowns(root: PathBuf, args: &InspectUnknownsArgs) -> Result<u8> {
    match args.format {
        AgentJsonFormatArg::Json => {}
    }
    if let Some(capability) = &args.capability {
        let support = public_fact_view(capability);
        if support
            .as_ref()
            .is_none_or(|view| view.stability != "stable" || !view.unknowns)
        {
            let row = crate::analysis::unknown_taxonomy::collect::unsupported_capability_row(
                capability,
                support.map(|view| view.docs_path),
            );
            let report = UnknownsReport {
                version: 1,
                schema: POLINT_UNKNOWNS_JSON_SCHEMA_V1_URL.to_string(),
                tool: polint_tool_info(),
                capability: capability.clone(),
                rows: vec![UnknownsRow::from_taxonomy_full(row)],
            };
            println!("{}", serde_json::to_string_pretty(&report)?);
            return Ok(2);
        }
    }

    let requested_caps = args
        .capability
        .as_deref()
        .map(|capability| vec![capability])
        .unwrap_or_else(|| vec!["resolved_imports", "symbols", "references"]);
    let analysis = analyze_for_agent_json(&root, &args.paths, args.no_cache, &requested_caps)?;
    let rows = if let Some(capability) = &args.capability {
        crate::analysis::unknown_taxonomy::collect::public_capability_unknowns(
            &analysis.db,
            capability,
        )
    } else {
        crate::analysis::unknown_taxonomy::collect::all_unknowns_with_diagnostics(
            &analysis.db,
            &analysis.diagnostics,
        )
    }
    .into_iter()
    .map(UnknownsRow::from_taxonomy_full)
    .collect();
    let report = UnknownsReport {
        version: 1,
        schema: POLINT_UNKNOWNS_JSON_SCHEMA_V1_URL.to_string(),
        tool: polint_tool_info(),
        capability: args.capability.clone().unwrap_or_else(|| "all".to_string()),
        rows,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(0)
}

fn explain(root: PathBuf, args: &ExplainArgs) -> Result<u8> {
    let manifests = discover_local_rule_hosts(&root)?;
    let mut rules = Vec::new();
    for manifest in &manifests {
        let host_path = local_manifest_path(&root, manifest);
        let report = run_local_rule_host_inspect(&root, manifest)?;
        rules.extend(
            report
                .rules
                .into_iter()
                .map(|rule| rule.with_host_path(host_path.clone())),
        );
    }
    if let Some(rule) = &args.rule {
        rules.retain(|candidate| candidate.rule_id == *rule);
        if rules.is_empty() {
            anyhow::bail!("no registered rule matched `{rule}`");
        }
    }
    let report = ExplainReport {
        version: 1,
        schema: POLINT_EXPLAIN_JSON_SCHEMA_V1_URL.to_string(),
        tool: polint_tool_info(),
        scope: args.rule.clone().unwrap_or_else(|| "all_rules".to_string()),
        rules: rules
            .into_iter()
            .map(|rule| ExplainRuleRow {
                rule_id: rule.rule_id,
                host_path: rule.host_path,
                fact_views: rule
                    .fact_views
                    .into_iter()
                    .map(|view| view.view_type)
                    .collect(),
                capabilities: rule.capability_support,
            })
            .collect(),
        public_capabilities: FactsListReport::new().views,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(0)
}

/// Internal (NON-public) view of a derived edge's provenance, surfaced through the
/// existing private plumbing reached from the [`explain`] command (D-10). This is
/// deliberately NOT added to the public `ExplainReport`/`ExplainRuleRow` JSON schema
/// — the only new public CLI surface in v1.3 is `polint inspect unknowns` (Phase 52).
/// All fields mirror the `pub(crate)` `DerivedEdgeProvenance`; nothing here reaches
/// `polint::sdk::prelude` (the Phase 42 leak gate stays green).
///
/// Today this is a `cfg(test)`-facing internal accessor: no PRODUCTION explain path
/// consumes derived edges yet (the unified-solver provider lands in Plan 03; the
/// unknown-taxonomy public surface is Phase 52). D-10 explicitly sanctions a
/// test-exercised internal seam — the plumbing exists and is locked by a unit test.
// `allow` (not `expect`): the struct is only constructed by the test-exercised
// `explain_derived_edge_provenance` seam (Phase 47 D-10); whether dead_code fires
// varies by build config, so an `expect` would be reported unfulfilled.
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DerivedEdgeProvenanceView {
    /// The derived edge's stable key.
    pub(crate) edge_stable_key: String,
    /// The contributing fact stable keys, totally ordered by stable ID (D-08).
    pub(crate) contributing_fact_keys: Vec<String>,
    /// The producing constraint kind (`ConstraintKind::as_str()` label).
    pub(crate) constraint_kind: String,
    /// The monotonic solver step at which the edge was derived.
    pub(crate) solver_step: u64,
}

/// Private plumbing (D-10): for a derived edge identified by its `edge_stable_key`,
/// surface its contributing facts + constraint kind + solver step from the unified
/// [`crate::analysis::solver::store::SolverStore`]. Returns `None` if no derived edge
/// with that stable key exists.
///
/// This extends the EXISTING private/explain plumbing reached from [`explain`]; it
/// adds NO public JSON field. The view is `pub(crate)` and is exercised by a unit
/// test, keeping the provenance types internal (leak gate green).
//
// `allow` (not `expect`): whether dead_code fires for this `pub(crate)` fn varies by
// build config (it is live under `cfg(test)`), so an `expect` would be reported
// unfulfilled in test builds. Phase 47 D-10 sanctions a test-exercised internal seam
// until the Plan 03 provider / Phase 52 surface consumes it in production.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn explain_derived_edge_provenance(
    store: &crate::analysis::solver::store::SolverStore,
    edge_stable_key: &str,
) -> Option<DerivedEdgeProvenanceView> {
    store
        .derived_edges()
        .iter()
        .find(|edge| edge.stable_key == edge_stable_key)
        .map(|edge| DerivedEdgeProvenanceView {
            edge_stable_key: edge.stable_key.clone(),
            contributing_fact_keys: edge
                .provenance
                .contributing_facts
                .iter()
                .map(|fact| fact.stable_key.clone())
                .collect(),
            constraint_kind: edge.provenance.constraint_kind.clone(),
            solver_step: edge.provenance.solver_step,
        })
}

fn inspect_rule(root: &Path, args: &InspectRuleArgs) -> Result<u8> {
    let manifests = discover_local_rule_hosts(root)?;
    let mut rules = Vec::new();
    for manifest in &manifests {
        let host_path = local_manifest_path(root, manifest);
        let report = run_local_rule_host_inspect(root, manifest)?;
        rules.extend(
            report
                .rules
                .into_iter()
                .map(|rule| rule.with_host_path(host_path.clone())),
        );
    }
    if let Some(pattern) = &args.rule {
        rules.retain(|rule| rule_id_matches(pattern, &rule.rule_id));
        if rules.is_empty() {
            anyhow::bail!("no registered rule matched `{pattern}`");
        }
    }

    let report = InspectRuleReport::new("polint", env!("CARGO_PKG_VERSION"), rules);
    match args.format {
        InspectFormatArg::Human => print!("{}", render_inspect_rule_human(&report)),
        InspectFormatArg::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }
    Ok(0)
}

fn test(root: PathBuf, args: &TestArgs) -> Result<u8> {
    let report = run_rule_tests(
        &root,
        RuleTestOptions {
            rule: args.rule.clone(),
            case: args.case.clone(),
            no_cache: args.no_cache,
            keep_temp: args.keep_temp,
            rule_host_manifests: discover_local_rule_hosts(&root)?,
        },
    )?;
    match args.format {
        TestFormatArg::Human => print!("{}", render_rule_test_human(&report)),
        TestFormatArg::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }
    if report.summary.failed > 0 {
        Ok(1)
    } else {
        Ok(0)
    }
}

fn render_inspect_rule_human(report: &InspectRuleReport) -> String {
    if report.rules.is_empty() {
        return "No repo-local rules found.\n".to_string();
    }
    let mut out = String::new();
    for rule in &report.rules {
        let capabilities = if rule.capabilities.is_empty() {
            "none".to_string()
        } else {
            rule.capabilities
                .iter()
                .map(|capability| capability.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let host = rule.host_path.as_deref().unwrap_or("<built-in>");
        out.push_str(&format!(
            "{} [{}] {}: {} (capabilities: {})\n",
            rule.rule_id, rule.severity, host, rule.description, capabilities
        ));
    }
    out
}

fn cache_status(layout: &CacheLayout, args: &CacheStatusArgs) -> Result<u8> {
    let status = layout.status()?;
    match args.format {
        CacheStatusFormatArg::Human => {
            print!("{}", render_cache_status_human(&status));
        }
        CacheStatusFormatArg::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&CacheStatusWire {
                    schema: POLINT_CACHE_STATUS_JSON_SCHEMA_V1_URL,
                    status: &status,
                })?
            );
        }
    }
    Ok(0)
}

fn cache_prune(layout: &CacheLayout, args: &CachePruneArgs) -> Result<u8> {
    let options = CachePruneOptions {
        categories: category_arg_to_prune_categories(args.category),
        max_age: args
            .max_age_days
            .map(|days| Duration::from_secs(days.saturating_mul(24 * 60 * 60))),
        max_bytes: args.max_size_mb.map(mib_to_bytes),
        dry_run: args.dry_run,
    };
    if options.max_age.is_none() && options.max_bytes.is_none() {
        anyhow::bail!("cache prune requires --max-age-days or --max-size-mb");
    }
    let report = layout.prune(&options)?;
    print!("{}", render_cache_prune_human(&report));
    Ok(0)
}

fn cache_clean(layout: &CacheLayout, args: &CacheCleanArgs) -> Result<u8> {
    let selection = category_arg_to_clean_selection(args.category);
    let report = layout.clean(selection)?;
    print!("{}", render_cache_clean_human(&report));
    Ok(0)
}

#[derive(Serialize)]
struct CacheStatusWire<'a> {
    #[serde(rename = "schema")]
    schema: &'static str,
    #[serde(flatten)]
    status: &'a CacheStatus,
}

const POLINT_FACTS_JSON_SCHEMA_V1_URL: &str =
    "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-facts-v1.json";
const POLINT_UNKNOWNS_JSON_SCHEMA_V1_URL: &str =
    "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-unknowns-v1.json";
const POLINT_EXPLAIN_JSON_SCHEMA_V1_URL: &str =
    "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-explain-v1.json";

#[derive(Debug, Clone, Serialize)]
struct PublicFactView {
    capability: &'static str,
    view_type: &'static str,
    canonical_path: &'static str,
    stability: &'static str,
    docs_path: &'static str,
    sampling: bool,
    unknowns: bool,
}

#[derive(Debug, Clone, Serialize)]
struct FactsListReport {
    version: u32,
    schema: String,
    tool: crate::diagnostics::PolintToolInfo,
    views: Vec<PublicFactView>,
}

#[derive(Debug, Clone, Serialize)]
struct FactsSampleReport {
    version: u32,
    schema: String,
    tool: crate::diagnostics::PolintToolInfo,
    capability: String,
    limit: usize,
    sampled: usize,
    rows: Vec<PublicFactSampleRow>,
}

#[derive(Debug, Clone, Serialize)]
struct PublicFactSampleRow {
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    span: Option<PublicSpan>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    precision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stable_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct UnknownsReport {
    version: u32,
    schema: String,
    tool: crate::diagnostics::PolintToolInfo,
    capability: String,
    rows: Vec<UnknownsRow>,
}

#[derive(Debug, Clone, Serialize)]
struct UnknownsRow {
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    span: Option<PublicSpan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    family: Option<String>,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    precision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    docs_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_artifact: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_stable_key: Option<String>,
}

impl UnknownsRow {
    fn from_taxonomy_compat(row: crate::analysis::unknown_taxonomy::facts::UnknownRow) -> Self {
        let mut rendered = Self::from_taxonomy_full(row);
        rendered.category = None;
        rendered.provider = None;
        rendered.family = None;
        rendered.source_stable_key = None;
        rendered
    }

    fn from_taxonomy_full(row: crate::analysis::unknown_taxonomy::facts::UnknownRow) -> Self {
        Self {
            category: Some(row.category.as_str().to_string()),
            provider: None,
            family: None,
            file: row.file,
            span: row.span.map(|span| PublicSpan {
                line: span.line,
                column: span.column,
            }),
            status: row.status,
            reason: row.reason,
            precision: row.precision,
            docs_path: row.docs_path,
            suggested_artifact: row.suggested_artifact,
            source_stable_key: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ExplainReport {
    version: u32,
    schema: String,
    tool: crate::diagnostics::PolintToolInfo,
    scope: String,
    rules: Vec<ExplainRuleRow>,
    public_capabilities: Vec<PublicFactView>,
}

#[derive(Debug, Clone, Serialize)]
struct ExplainRuleRow {
    rule_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    host_path: Option<String>,
    fact_views: Vec<String>,
    capabilities: Vec<crate::rule_manifest::CapabilitySupportWire>,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct PublicSpan {
    line: u32,
    column: u32,
}

impl PublicFactSampleRow {
    fn new(
        file: String,
        span: Option<PublicSpan>,
        status: impl Into<String>,
        precision: Option<String>,
        stable_id: Option<String>,
    ) -> Self {
        Self {
            file,
            span,
            status: status.into(),
            precision,
            stable_id,
        }
    }
}

impl FactsListReport {
    fn new() -> Self {
        let mut views = vec![
            public_fact_view("resolved_imports").unwrap(),
            public_fact_view("module_graph").unwrap(),
            public_fact_view("symbols").unwrap(),
            public_fact_view("references").unwrap(),
            public_fact_view("file_metrics").unwrap(),
            public_fact_view("function_metrics").unwrap(),
            public_fact_view("complexity_metrics").unwrap(),
            public_fact_view("cfg").unwrap(),
            public_fact_view("call_graph").unwrap(),
            public_fact_view("dataflow").unwrap(),
            public_fact_view("coverage_facts").unwrap(),
            public_fact_view("test_suite_metrics").unwrap(),
        ];
        views.sort_by(|left, right| left.capability.cmp(right.capability));
        Self {
            version: 1,
            schema: POLINT_FACTS_JSON_SCHEMA_V1_URL.to_string(),
            tool: polint_tool_info(),
            views,
        }
    }
}

fn public_fact_view(capability: &str) -> Option<PublicFactView> {
    let view = match capability {
        "resolved_imports" => PublicFactView {
            capability: "resolved_imports",
            view_type: "ResolvedImports",
            canonical_path: "polint::sdk::facts::ResolvedImports<'_>",
            stability: "stable",
            docs_path: "docs/facts/resolved-imports.md",
            sampling: true,
            unknowns: true,
        },
        "module_graph" => PublicFactView {
            capability: "module_graph",
            view_type: "ModuleGraphFacts",
            canonical_path: "polint::sdk::facts::ModuleGraphFacts<'_>",
            stability: "stable",
            docs_path: "docs/facts/resolved-imports.md",
            sampling: true,
            unknowns: false,
        },
        "symbols" => PublicFactView {
            capability: "symbols",
            view_type: "Symbols",
            canonical_path: "polint::sdk::facts::Symbols<'_>",
            stability: "stable",
            docs_path: "docs/facts/symbols-and-references.md",
            sampling: true,
            unknowns: true,
        },
        "references" => PublicFactView {
            capability: "references",
            view_type: "References",
            canonical_path: "polint::sdk::facts::References<'_>",
            stability: "stable",
            docs_path: "docs/facts/symbols-and-references.md",
            sampling: true,
            unknowns: true,
        },
        "file_metrics" => PublicFactView {
            capability: "file_metrics",
            view_type: "FileMetrics",
            canonical_path: "polint::sdk::facts::FileMetrics<'_>",
            stability: "stable",
            docs_path: "docs/facts/metrics.md",
            sampling: true,
            unknowns: false,
        },
        "function_metrics" => PublicFactView {
            capability: "function_metrics",
            view_type: "FunctionMetrics",
            canonical_path: "polint::sdk::facts::FunctionMetrics<'_>",
            stability: "stable",
            docs_path: "docs/facts/metrics.md",
            sampling: true,
            unknowns: false,
        },
        "complexity_metrics" => PublicFactView {
            capability: "complexity_metrics",
            view_type: "ComplexityMetrics",
            canonical_path: "polint::sdk::facts::ComplexityMetrics<'_>",
            stability: "stable",
            docs_path: "docs/facts/metrics.md",
            sampling: true,
            unknowns: false,
        },
        "cfg" => PublicFactView {
            capability: "cfg",
            view_type: "Cfg",
            canonical_path: "polint::sdk::facts::Cfg<'_>",
            stability: "reserved",
            docs_path: "docs/facts/capability-plans.md",
            sampling: false,
            unknowns: false,
        },
        "call_graph" => PublicFactView {
            capability: "call_graph",
            view_type: "CallGraph",
            canonical_path: "polint::sdk::facts::CallGraph<'_>",
            stability: "reserved",
            docs_path: "docs/facts/capability-plans.md",
            sampling: false,
            unknowns: false,
        },
        "dataflow" => PublicFactView {
            capability: "dataflow",
            view_type: "DataFlow",
            canonical_path: "polint::sdk::facts::DataFlow<'_>",
            stability: "reserved",
            docs_path: "docs/facts/data-flow.md",
            sampling: false,
            unknowns: false,
        },
        "coverage_facts" => PublicFactView {
            capability: "coverage_facts",
            view_type: "CoverageFacts",
            canonical_path: "polint::sdk::facts::CoverageFacts<'_>",
            stability: "reserved",
            docs_path: "docs/facts/capability-plans.md",
            sampling: false,
            unknowns: false,
        },
        "test_suite_metrics" => PublicFactView {
            capability: "test_suite_metrics",
            view_type: "TestSuiteMetrics",
            canonical_path: "polint::sdk::facts::TestSuiteMetrics<'_>",
            stability: "reserved",
            docs_path: "docs/facts/capability-plans.md",
            sampling: false,
            unknowns: false,
        },
        _ => return None,
    };
    Some(view)
}

fn polint_tool_info() -> crate::diagnostics::PolintToolInfo {
    crate::diagnostics::PolintToolInfo {
        name: "polint".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

struct AgentJsonAnalysis {
    db: AnalysisDb,
    diagnostics: Vec<Diagnostic>,
}

fn analyze_for_agent_json(
    root: &Path,
    paths: &[PathBuf],
    no_cache: bool,
    capabilities: &[&str],
) -> Result<AgentJsonAnalysis> {
    let args = CheckArgs {
        paths: paths.to_vec(),
        profile: None,
        format: FormatArg::Json,
        color: ColorArg::Never,
        no_cache,
        fail_on: FailOn::None,
        only_rule: None,
        max_diagnostics: None,
        stat: false,
        shortstat: false,
        baseline: false,
        new_only: false,
        ignore_comments: false,
    };
    let loaded = load_config_for_check(root, &args.paths)?;
    let cache = crate::cache::Cache::default_for_repo(root, !args.no_cache);
    let config_digest = crate::cache::keys::config_hash(&loaded);
    let rules: Vec<Rule> = Vec::new();
    let enabled = selected_rule_patterns(&loaded, args.profile.as_deref())?;
    let options = BTreeMap::<String, RuleOptions>::new();
    let rule_digest = crate::cache::keys::rule_hash(&rules, enabled.as_ref(), &options);
    let plan = AnalysisPlan::from_capability_names(capabilities);
    let output = AnalysisKernel::run(KernelInput {
        loaded: &loaded,
        cache: &cache,
        config_digest: &config_digest,
        rule_digest: &rule_digest,
        plan: &plan,
        parallel: true,
    })?;
    Ok(AgentJsonAnalysis {
        db: output.db,
        diagnostics: output.diagnostics,
    })
}

fn span_start(span: &crate::core::Span) -> PublicSpan {
    let range = span.diagnostic_range();
    PublicSpan {
        line: range.start_line,
        column: range.start_col,
    }
}

fn status_label(status: ResolutionStatus) -> &'static str {
    match status {
        ResolutionStatus::Resolved => "resolved",
        ResolutionStatus::External => "external",
        ResolutionStatus::Unresolved => "unresolved",
        ResolutionStatus::SetupMissing => "setup_missing",
        ResolutionStatus::Dynamic => "dynamic",
        ResolutionStatus::Unsupported => "unsupported",
    }
}

fn resolution_precision_label(precision: crate::core::ResolutionPrecision) -> &'static str {
    match precision {
        crate::core::ResolutionPrecision::ExactFile => "exact_file",
        crate::core::ResolutionPrecision::Package => "package",
        crate::core::ResolutionPrecision::ExternalPackage => "external_package",
        crate::core::ResolutionPrecision::Heuristic => "heuristic",
        crate::core::ResolutionPrecision::None => "none",
    }
}

fn symbol_status_label(status: SymbolResolutionStatus) -> &'static str {
    match status {
        SymbolResolutionStatus::Resolved => "resolved",
        SymbolResolutionStatus::Unresolved => "unresolved",
        SymbolResolutionStatus::Ambiguous => "ambiguous",
        SymbolResolutionStatus::SetupMissing => "setup_missing",
        SymbolResolutionStatus::Unsupported => "unsupported",
    }
}

fn symbol_precision_label(precision: SymbolPrecision) -> &'static str {
    match precision {
        SymbolPrecision::ExactSemantic => "exact_semantic",
        SymbolPrecision::ExactLocal => "exact_local",
        SymbolPrecision::ModuleLinked => "module_linked",
        SymbolPrecision::Heuristic => "heuristic",
        SymbolPrecision::Unresolved => "unresolved",
        SymbolPrecision::Ambiguous => "ambiguous",
        SymbolPrecision::SetupMissing => "setup_missing",
        SymbolPrecision::Unsupported => "unsupported",
    }
}

fn category_arg_to_prune_categories(
    category: Option<CacheCategoryArg>,
) -> Vec<CacheManagedCategory> {
    match category {
        Some(CacheCategoryArg::All) | None => Vec::new(),
        Some(CacheCategoryArg::Analysis) => vec![CacheManagedCategory::Analysis],
        Some(CacheCategoryArg::RulesTarget) => vec![CacheManagedCategory::RulesTarget],
        Some(CacheCategoryArg::Derived) => vec![CacheManagedCategory::Derived],
    }
}

fn category_arg_to_clean_selection(category: CacheCategoryArg) -> CacheCleanSelection {
    match category {
        CacheCategoryArg::All => CacheCleanSelection::All,
        CacheCategoryArg::Analysis => CacheCleanSelection::Category(CacheManagedCategory::Analysis),
        CacheCategoryArg::RulesTarget => {
            CacheCleanSelection::Category(CacheManagedCategory::RulesTarget)
        }
        CacheCategoryArg::Derived => CacheCleanSelection::Category(CacheManagedCategory::Derived),
    }
}

fn mib_to_bytes(value: u64) -> u64 {
    value.saturating_mul(1024 * 1024)
}

fn render_cache_status_human(status: &CacheStatus) -> String {
    let mut out = String::new();
    out.push_str(&format!("Cache root: {}\n", status.root));
    out.push_str(&format!(
        "Total: {} across {} files\n",
        human_bytes(status.total_bytes),
        status.total_files
    ));
    for category in &status.categories {
        out.push_str(&format!(
            "- {}: {} files, {} ({})\n  {}\n",
            category.name,
            category.files,
            human_bytes(category.bytes),
            if category.exists {
                "present"
            } else {
                "missing"
            },
            category.path
        ));
    }
    out
}

fn render_cache_clean_human(report: &CacheCleanReport) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Removed {} across {} files\n",
        human_bytes(report.removed_bytes),
        report.removed_files
    ));
    for category in &report.categories {
        out.push_str(&format!(
            "- {}: removed {} files, {} from {}\n",
            category.name,
            category.removed_files,
            human_bytes(category.removed_bytes),
            category.path
        ));
    }
    out
}

fn render_cache_prune_human(report: &CachePruneReport) -> String {
    let action = if report.dry_run {
        "Would remove"
    } else {
        "Removed"
    };
    let mut out = String::new();
    out.push_str(&format!(
        "{action} {} across {} files\n",
        human_bytes(report.removed_bytes),
        report.removed_files
    ));
    for category in &report.categories {
        out.push_str(&format!(
            "- {}: {} files, {} before; {} files, {} selected from {}\n",
            category.name,
            category.before_files,
            human_bytes(category.before_bytes),
            category.removed_files,
            human_bytes(category.removed_bytes),
            category.path
        ));
    }
    out
}

fn human_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = bytes as f64;
    if value >= GIB {
        format!("{:.1} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn check(root: PathBuf, args: &CheckArgs) -> Result<u8> {
    if args.new_only && !args.baseline {
        anyhow::bail!("--new-only requires --baseline");
    }
    let local_rule_hosts = discover_local_rule_hosts(&root)?;
    if !local_rule_hosts.is_empty() {
        return check_local_rule_hosts(&root, args, &local_rule_hosts);
    }

    let (mut diagnostics, db, loaded) = analyze_and_run(&root, args, true)?;
    let mut ignore_report = None;
    if args.ignore_comments {
        let ignore_application = apply_ignores(&db, diagnostics, &loaded.config.ignores);
        diagnostics = ignore_application.diagnostics;
        ignore_report = Some(ignore_application.report);
    }
    let source_map = match args.format {
        FormatArg::Human => db.sources_by_relative_path(),
        _ => BTreeMap::new(),
    };
    let format = match args.format {
        FormatArg::Human => OutputFormat::Human,
        FormatArg::Github => OutputFormat::Github,
        FormatArg::Json => OutputFormat::Json,
        FormatArg::Sarif => OutputFormat::Sarif,
        FormatArg::AiFriendly => OutputFormat::AiFriendly,
    };
    diagnostics = apply_report_filters(diagnostics, args.only_rule.as_deref());
    let baseline = apply_baseline(&root, args, diagnostics)?;
    diagnostics = baseline.diagnostics;
    let rendered_diagnostics = limit_report_diagnostics(diagnostics.clone(), args.max_diagnostics);
    let stats = check_stats(
        &db,
        &diagnostics,
        rendered_diagnostics.len(),
        ignore_report.as_ref(),
    );
    let sources = match args.format {
        FormatArg::Human => Some(&source_map),
        _ => None,
    };
    if matches!(args.format, FormatArg::AiFriendly) {
        let output = write_ai_friendly_report(
            &root,
            &diagnostics,
            &rendered_diagnostics,
            json_report_meta(),
        )?;
        print!(
            "{}",
            render_ai_friendly_stdout(&output.report, AI_FRIENDLY_LATEST_OUTPUT)
        );
    } else {
        print!(
            "{}",
            render_with_sarif_help(
                format,
                &rendered_diagnostics,
                render_opts(args, sources),
                sarif_help_map(&loaded),
            )
        );
    }

    if loaded.missing && matches!(args.format, FormatArg::Human) {
        println!("Config not found. Run `polint init` to create .polint.toml.");
    }
    if should_render_check_stats(args) {
        print!("{}", render_check_stats(&stats, args.stat, args.shortstat));
    }
    if matches!(args.format, FormatArg::Human)
        && let Some(summary) = &baseline.summary
    {
        print!("{}", render_baseline_summary(summary));
    }

    Ok(exit_code_for(&baseline.failure_diagnostics, args.fail_on))
}

fn ignores(root: PathBuf, args: &IgnoresArgs) -> Result<u8> {
    let report = collect_ignore_report(&root, args)?;
    let filters = parse_ignore_filters(args.filter.as_deref());
    let report = filter_report(&report, &filters);
    match args.format {
        IgnoresFormatArg::Human => {
            print!(
                "{}",
                render_ignore_report_human(&report, args.stat, args.shortstat)
            );
        }
        IgnoresFormatArg::Json => {
            println!("{}", serde_json::to_string_pretty(&report)?);
        }
    }
    Ok(0)
}

fn collect_ignore_report(root: &Path, args: &IgnoresArgs) -> Result<crate::ignores::IgnoreReport> {
    let check_args = CheckArgs {
        paths: args.paths.clone(),
        profile: args.profile.clone(),
        format: FormatArg::Json,
        color: ColorArg::Never,
        no_cache: args.no_cache,
        fail_on: FailOn::None,
        only_rule: None,
        max_diagnostics: None,
        stat: false,
        shortstat: false,
        baseline: false,
        new_only: false,
        ignore_comments: false,
    };
    let local_rule_hosts = discover_local_rule_hosts(root)?;
    if local_rule_hosts.is_empty() {
        let (diagnostics, db, loaded) = analyze_and_run(root, &check_args, true)?;
        return Ok(apply_ignores(&db, diagnostics, &loaded.config.ignores).report);
    }

    let config = load_config_for_check(root, &args.paths)?;
    let enabled = selected_rule_patterns(&config, args.profile.as_deref())?;
    let mut diagnostics = Vec::new();
    for manifest in &local_rule_hosts {
        diagnostics.extend(run_local_rule_host(root, manifest, &check_args, false)?);
    }
    // Same scope narrowing as `check_local_rule_hosts`: the db only feeds
    // ignore-directive scanning, which is limited to the configured rule scopes
    // plus any files diagnostics actually landed in.
    let rule_scope = config_rule_scope_globset(&config, enabled.as_ref());
    let mut db = load_analysis_files_scoped(&config, rule_scope.as_ref())?;
    backfill_diagnostic_files(&mut db, &diagnostics, root);
    Ok(apply_ignores(&db, diagnostics, &config.config.ignores).report)
}

fn parse_ignore_filters(filter: Option<&str>) -> Vec<String> {
    filter
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|filter| !filter.is_empty())
        .map(ToString::to_string)
        .collect()
}

#[derive(Debug, Clone, Default)]
struct CheckStats {
    scanned_files: usize,
    by_language: BTreeMap<&'static str, usize>,
    diagnostics: usize,
    emitted_diagnostics: usize,
    by_severity: BTreeMap<&'static str, usize>,
    by_rule: BTreeMap<String, usize>,
    ignore_directives: usize,
    active_ignores: usize,
    unused_ignores: usize,
    malformed_ignores: usize,
    missing_reason_ignores: usize,
    suppressed_diagnostics: usize,
}

fn check_stats(
    db: &AnalysisDb,
    diagnostics: &[Diagnostic],
    emitted_diagnostics: usize,
    ignore_report: Option<&crate::ignores::IgnoreReport>,
) -> CheckStats {
    let mut stats = CheckStats {
        scanned_files: db.files().len(),
        diagnostics: diagnostics.len(),
        emitted_diagnostics,
        ..CheckStats::default()
    };
    for file in db.files() {
        *stats
            .by_language
            .entry(language_label(file.language))
            .or_default() += 1;
    }
    for diagnostic in diagnostics {
        *stats
            .by_severity
            .entry(severity_label(diagnostic.severity))
            .or_default() += 1;
        *stats.by_rule.entry(diagnostic.rule_id.clone()).or_default() += 1;
    }
    if let Some(report) = ignore_report {
        stats.ignore_directives = report.summary.directives;
        stats.active_ignores = report.summary.active;
        stats.unused_ignores = report.summary.unused;
        stats.malformed_ignores = report.summary.malformed;
        stats.missing_reason_ignores = report.summary.missing_reasons;
        stats.suppressed_diagnostics = report.summary.suppressed_diagnostics;
    }
    stats
}

fn should_render_check_stats(args: &CheckArgs) -> bool {
    matches!(args.format, FormatArg::Human) && (args.stat || args.shortstat)
}

fn render_check_stats(stats: &CheckStats, stat: bool, shortstat: bool) -> String {
    let mut out = String::new();
    if shortstat {
        out.push_str(&format_check_shortstat(stats));
        out.push('\n');
    }
    if stat {
        if !shortstat {
            out.push_str(&format_check_shortstat(stats));
            out.push('\n');
        }
        out.push_str(&format_check_stat_tables(stats));
    }
    out
}

fn format_check_shortstat(stats: &CheckStats) -> String {
    let emitted = if stats.emitted_diagnostics == stats.diagnostics {
        String::new()
    } else {
        format!(", {} emitted", stats.emitted_diagnostics)
    };
    format!(
        "Scanned {} {}; {} diagnostics{}; {} suppressed by {} ignore directives",
        stats.scanned_files,
        plural(stats.scanned_files, "file", "files"),
        stats.diagnostics,
        emitted,
        stats.suppressed_diagnostics,
        stats.ignore_directives
    )
}

fn format_check_stat_tables(stats: &CheckStats) -> String {
    let mut out = String::new();
    if !stats.by_language.is_empty() {
        out.push_str("\nBy language\n");
        for (language, count) in &stats.by_language {
            out.push_str(&format!("  {language}: {count}\n"));
        }
    }
    if !stats.by_severity.is_empty() {
        out.push_str("\nBy severity\n");
        for (severity, count) in &stats.by_severity {
            out.push_str(&format!("  {severity}: {count}\n"));
        }
    }
    if !stats.by_rule.is_empty() {
        out.push_str("\nBy rule\n");
        for (rule_id, count) in &stats.by_rule {
            out.push_str(&format!("  {rule_id}: {count}\n"));
        }
    }
    if stats.ignore_directives > 0 || stats.suppressed_diagnostics > 0 {
        out.push_str("\nIgnores\n");
        out.push_str(&format!(
            "  directives={}, active={}, unused={}, malformed={}, missing_reasons={}, suppressed={}\n",
            stats.ignore_directives,
            stats.active_ignores,
            stats.unused_ignores,
            stats.malformed_ignores,
            stats.missing_reason_ignores,
            stats.suppressed_diagnostics
        ));
    }
    out
}

fn language_label(language: Language) -> &'static str {
    match language {
        Language::Go => "go",
        Language::TypeScript => "ts",
        Language::Tsx => "tsx",
        Language::JavaScript => "js",
        Language::Jsx => "jsx",
        Language::Unknown => "unknown",
    }
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warn => "warn",
        Severity::Error => "error",
    }
}

fn plural(count: usize, singular: &'static str, plural: &'static str) -> &'static str {
    if count == 1 { singular } else { plural }
}

struct BaselineApplied {
    diagnostics: Vec<Diagnostic>,
    failure_diagnostics: Vec<Diagnostic>,
    summary: Option<BaselineSummary>,
}

fn apply_baseline(
    root: &Path,
    args: &CheckArgs,
    diagnostics: Vec<Diagnostic>,
) -> Result<BaselineApplied> {
    if !args.baseline {
        return Ok(BaselineApplied {
            failure_diagnostics: diagnostics.clone(),
            diagnostics,
            summary: None,
        });
    }

    let config = load_baseline(root)?;
    let classification = classify_diagnostics(&diagnostics, &config);
    let failure_diagnostics = classification.new_diagnostics.clone();
    let diagnostics = if args.new_only {
        classification.new_diagnostics
    } else {
        classification.visible_diagnostics
    };
    Ok(BaselineApplied {
        diagnostics,
        failure_diagnostics,
        summary: Some(classification.summary),
    })
}

fn baseline_path(root: &Path) -> PathBuf {
    root.join(DEFAULT_BASELINE_PATH)
}

fn collect_diagnostics_for_baseline(
    root: &Path,
    paths: &[PathBuf],
    profile: Option<&str>,
    no_cache: bool,
) -> Result<Vec<Diagnostic>> {
    let check_args = CheckArgs {
        paths: paths.to_vec(),
        profile: profile.map(ToString::to_string),
        format: FormatArg::Json,
        color: ColorArg::Never,
        no_cache,
        fail_on: FailOn::None,
        only_rule: None,
        max_diagnostics: None,
        stat: false,
        shortstat: false,
        baseline: false,
        new_only: false,
        ignore_comments: true,
    };
    let local_rule_hosts = discover_local_rule_hosts(root)?;
    let mut diagnostics = if local_rule_hosts.is_empty() {
        let (diagnostics, db, loaded) = analyze_and_run(root, &check_args, true)?;
        apply_ignores(&db, diagnostics, &loaded.config.ignores).diagnostics
    } else {
        let config = load_config_for_check(root, paths)?;
        let enabled = selected_rule_patterns(&config, profile)?;
        let mut diagnostics = Vec::new();
        for manifest in &local_rule_hosts {
            diagnostics.extend(run_local_rule_host(root, manifest, &check_args, false)?);
        }
        // Same scope narrowing as `check_local_rule_hosts`.
        let rule_scope = config_rule_scope_globset(&config, enabled.as_ref());
        let mut loaded_db = load_analysis_files_scoped(&config, rule_scope.as_ref())?;
        backfill_diagnostic_files(&mut loaded_db, &diagnostics, root);
        apply_ignores(&loaded_db, diagnostics, &config.config.ignores).diagnostics
    };
    diagnostics = apply_report_filters(diagnostics, None);
    Ok(diagnostics)
}

fn analyze_and_run(
    root: &Path,
    args: &CheckArgs,
    parallel: bool,
) -> Result<(
    Vec<crate::diagnostics::Diagnostic>,
    crate::core::AnalysisDb,
    LoadedConfig,
)> {
    let loaded = load_config_for_check(root, &args.paths)?;
    let cache = crate::cache::Cache::default_for_repo(root, !args.no_cache);
    let config_digest = crate::cache::keys::config_hash(&loaded);
    let rules: Vec<Rule> = Vec::new();
    let enabled = selected_rule_patterns(&loaded, args.profile.as_deref())?;
    let options = BTreeMap::<String, RuleOptions>::new();
    let rule_digest = crate::cache::keys::rule_hash(&rules, enabled.as_ref(), &options);
    let plan = AnalysisPlan::empty();

    let output = AnalysisKernel::run(KernelInput {
        loaded: &loaded,
        cache: &cache,
        config_digest: &config_digest,
        rule_digest: &rule_digest,
        plan: &plan,
        parallel,
    })?;
    let mut diagnostics = output.diagnostics;
    diagnostics.extend(run_rules(
        &output.db,
        &rules,
        &options,
        enabled.as_ref(),
        parallel,
    ));
    Ok((diagnostics, output.db, loaded))
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

fn discover_local_rule_hosts(root: &Path) -> Result<Vec<PathBuf>> {
    let config = load_config(root)?;
    let mut manifests = BTreeSet::new();
    for rule_path in &config.config.rules.paths {
        let manifest = root.join(rule_path).join("Cargo.toml");
        if manifest.is_file() {
            manifests.insert(manifest);
        }
    }
    Ok(manifests.into_iter().collect())
}

/// Union of the configured rules' `files` scopes (optionally filtered to the
/// enabled profile patterns) as a glob set, or `None` when narrowing is unsafe:
/// no configured rules, or any enabled rule entry without a `files` scope (which
/// matches every file). `None` falls back to full workspace loading.
///
/// This mirrors `AnalysisKernel::rule_scope_globset`, but reads scopes from
/// `.polint.toml` instead of the in-process plan, because the outer CLI never
/// registers the repo-local rules itself — they live in the rule-host subprocess.
fn config_rule_scope_globset(
    config: &LoadedConfig,
    enabled: Option<&BTreeSet<String>>,
) -> Option<globset::GlobSet> {
    let entries = config
        .config
        .rules
        .config
        .iter()
        .filter(|entry| {
            enabled.is_none_or(|patterns| {
                patterns
                    .iter()
                    .any(|pattern| rule_id_matches(pattern, &entry.id))
            })
        })
        .collect::<Vec<_>>();
    if entries.is_empty() {
        return None;
    }
    let mut patterns = Vec::new();
    for entry in &entries {
        if entry.files.is_empty() {
            return None;
        }
        patterns.extend(entry.files.iter().cloned());
    }
    crate::config::build_glob_set(&patterns).ok()
}

/// Ensures every diagnostic's file is present in `db` so ignore-comment
/// directives in those files still apply when the db was loaded with a narrowed
/// scope (e.g. a rule registered in the host without a `[[rules.config]]` entry
/// can emit outside the configured scopes).
fn backfill_diagnostic_files(
    db: &mut AnalysisDb,
    diagnostics: &[crate::diagnostics::Diagnostic],
    root: &Path,
) {
    let known = db
        .files()
        .iter()
        .map(|file| file.relative_path.clone())
        .collect::<BTreeSet<_>>();
    let missing = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.file.clone())
        .filter(|file| !file.is_empty() && !known.contains(file))
        .collect::<BTreeSet<_>>();
    for relative_path in missing {
        let path = root.join(&relative_path);
        if let Ok(source) = fs::read_to_string(&path) {
            db.add_file(path, relative_path, source);
        }
    }
}

fn check_local_rule_hosts(root: &Path, args: &CheckArgs, manifests: &[PathBuf]) -> Result<u8> {
    let config = load_config_for_check(root, &args.paths)?;
    let enabled = selected_rule_patterns(&config, args.profile.as_deref())?;

    let child_applies_ignores =
        args.ignore_comments && manifests.len() == 1 && !should_render_check_stats(args);
    let mut diagnostics = Vec::new();
    for manifest in manifests {
        diagnostics.extend(run_local_rule_host(
            root,
            manifest,
            args,
            child_applies_ignores,
        )?);
    }

    let mut db = None;
    let mut ignore_report = None;
    // The outer db only feeds ignore-comment application and `--stat`
    // rendering, so load just the configured rule scopes instead of the whole
    // workspace — on large monorepos this is the difference between reading
    // a few thousand in-scope files and every file in the repo. Any diagnostic
    // outside those scopes gets its file backfilled so its ignore directives
    // still apply.
    let rule_scope = config_rule_scope_globset(&config, enabled.as_ref());
    if args.ignore_comments && !child_applies_ignores {
        let mut loaded_db = load_analysis_files_scoped(&config, rule_scope.as_ref())?;
        backfill_diagnostic_files(&mut loaded_db, &diagnostics, root);
        let ignore_application = apply_ignores(&loaded_db, diagnostics, &config.config.ignores);
        diagnostics = ignore_application.diagnostics;
        ignore_report = Some(ignore_application.report);
        db = Some(loaded_db);
    } else if should_render_check_stats(args) {
        let mut loaded_db = load_analysis_files_scoped(&config, rule_scope.as_ref())?;
        backfill_diagnostic_files(&mut loaded_db, &diagnostics, root);
        db = Some(loaded_db);
    }
    diagnostics = apply_report_filters(diagnostics, args.only_rule.as_deref());
    let baseline = apply_baseline(root, args, diagnostics)?;
    diagnostics = baseline.diagnostics;
    let rendered_diagnostics = limit_report_diagnostics(diagnostics.clone(), args.max_diagnostics);
    let source_map = if matches!(args.format, FormatArg::Human) {
        read_sources_for_diagnostics(root, &rendered_diagnostics)
    } else {
        BTreeMap::new()
    };
    let format = match args.format {
        FormatArg::Human => OutputFormat::Human,
        FormatArg::Github => OutputFormat::Github,
        FormatArg::Json => OutputFormat::Json,
        FormatArg::Sarif => OutputFormat::Sarif,
        FormatArg::AiFriendly => OutputFormat::AiFriendly,
    };
    let sources = match args.format {
        FormatArg::Human => Some(&source_map),
        _ => None,
    };
    if matches!(args.format, FormatArg::AiFriendly) {
        let output = write_ai_friendly_report(
            root,
            &diagnostics,
            &rendered_diagnostics,
            json_report_meta(),
        )?;
        print!(
            "{}",
            render_ai_friendly_stdout(&output.report, AI_FRIENDLY_LATEST_OUTPUT)
        );
    } else {
        print!(
            "{}",
            render_with_sarif_help(
                format,
                &rendered_diagnostics,
                render_opts(args, sources),
                sarif_help_map(&config),
            )
        );
    }
    if should_render_check_stats(args)
        && let Some(db) = &db
    {
        let stats = check_stats(
            db,
            &diagnostics,
            rendered_diagnostics.len(),
            ignore_report.as_ref(),
        );
        print!("{}", render_check_stats(&stats, args.stat, args.shortstat));
    }
    if matches!(args.format, FormatArg::Human)
        && let Some(summary) = &baseline.summary
    {
        print!("{}", render_baseline_summary(summary));
    }
    Ok(exit_code_for(&baseline.failure_diagnostics, args.fail_on))
}

/// `polint review <ref>`: run review-kind rules against the diff to `<ref>`.
///
/// A near-clone of [`check_local_rule_hosts`] with three additions: it requires
/// repo-local rule hosts, builds + serializes a changeset before running them,
/// runs each host with `--kind review --changed-files <file>`, and (by default)
/// gates the findings to the diff. `polint check` is unaffected.
fn review(root: PathBuf, args: &ReviewArgs) -> Result<u8> {
    let manifests = discover_local_rule_hosts(&root)?;
    if manifests.is_empty() {
        anyhow::bail!(
            "`polint review` requires repo-local rules under `[rules] paths` in .polint.toml; \
             none were found. Scaffold one with `polint new-rule --review <name>`."
        );
    }

    // Build the diff and serialize it to a cache-dir file (a file, not an env
    // var, so large diffs are not bound by env-size limits). The host reads it
    // via `--changed-files` and injects it before rules run.
    let changeset = crate::git::changeset_for_ref(&root, &args.reff)?;
    let changeset_file = write_review_changeset(&root, &changeset)?;

    // Map the review args onto the shared `CheckArgs` shape so the rendering,
    // baseline, and report-filter helpers are reused verbatim. Review has no
    // baseline/ignore-comment/stat surface; those default off.
    let check_args = CheckArgs {
        paths: args.paths.clone(),
        profile: args.profile.clone(),
        format: args.format,
        color: args.color,
        no_cache: args.no_cache,
        fail_on: args.fail_on,
        only_rule: args.only_rule.clone(),
        max_diagnostics: args.max_diagnostics,
        stat: false,
        shortstat: false,
        baseline: false,
        new_only: false,
        ignore_comments: false,
    };

    let config = load_config_for_check(&root, &check_args.paths)?;
    let mut diagnostics = Vec::new();
    for manifest in &manifests {
        diagnostics.extend(run_local_rule_host_kind(
            &root,
            manifest,
            &check_args,
            false,
            "review",
            Some(changeset_file.as_path()),
        )?);
    }

    // Default finding-level diff gate: keep only diagnostics that intersect the
    // changeset. `--no-diff-gate` surfaces every review finding; `--whole-file`
    // gates by changed file only, ignoring line ranges.
    if !args.no_diff_gate {
        diagnostics = gate_to_changeset(diagnostics, &changeset, args.whole_file);
    }

    diagnostics = apply_report_filters(diagnostics, check_args.only_rule.as_deref());
    let baseline = apply_baseline(&root, &check_args, diagnostics)?;
    diagnostics = baseline.diagnostics;
    let rendered_diagnostics =
        limit_report_diagnostics(diagnostics.clone(), check_args.max_diagnostics);
    let source_map = if matches!(check_args.format, FormatArg::Human) {
        read_sources_for_diagnostics(&root, &rendered_diagnostics)
    } else {
        BTreeMap::new()
    };
    let format = match check_args.format {
        FormatArg::Human => OutputFormat::Human,
        FormatArg::Github => OutputFormat::Github,
        FormatArg::Json => OutputFormat::Json,
        FormatArg::Sarif => OutputFormat::Sarif,
        FormatArg::AiFriendly => OutputFormat::AiFriendly,
    };
    let sources = match check_args.format {
        FormatArg::Human => Some(&source_map),
        _ => None,
    };
    if matches!(check_args.format, FormatArg::AiFriendly) {
        let output = write_ai_friendly_report(
            &root,
            &diagnostics,
            &rendered_diagnostics,
            json_report_meta(),
        )?;
        print!(
            "{}",
            render_ai_friendly_stdout(&output.report, AI_FRIENDLY_LATEST_OUTPUT)
        );
    } else {
        print!(
            "{}",
            render_with_sarif_help(
                format,
                &rendered_diagnostics,
                render_opts(&check_args, sources),
                sarif_help_map(&config),
            )
        );
    }
    if matches!(check_args.format, FormatArg::Human)
        && let Some(summary) = &baseline.summary
    {
        print!("{}", render_baseline_summary(summary));
    }
    Ok(exit_code_for(
        &baseline.failure_diagnostics,
        check_args.fail_on,
    ))
}

/// Serialize a review changeset to a stable-named JSON file under the cache dir.
///
/// The file name hashes the JSON so re-runs with the same diff reuse the name.
/// Returns the absolute path passed to the host as `--changed-files`.
fn write_review_changeset(root: &Path, changeset: &crate::core::ChangeSetFacts) -> Result<PathBuf> {
    let json =
        serde_json::to_string(changeset).context("failed to serialize review changeset to JSON")?;
    let cache_layout = CacheLayout::for_repo(root);
    let review_dir = cache_layout.root().join("review");
    std::fs::create_dir_all(&review_dir)
        .with_context(|| format!("failed to create review cache dir {}", review_dir.display()))?;
    let name = format!(
        "changeset-{}.json",
        &crate::cache::stable_hash(&[&json])[..16]
    );
    let path = review_dir.join(name);
    std::fs::write(&path, json)
        .with_context(|| format!("failed to write review changeset {}", path.display()))?;
    Ok(path)
}

/// Retain only diagnostics whose file (and, unless `whole_file`, line span)
/// intersects the review changeset.
///
/// File match is exact against the normalized changeset paths (which share the
/// `Diagnostic.file` form). Line match overlaps `[start_line, end_line]` with
/// any new-side range for that file. Deleted files carry no new-side ranges, so
/// line-gating drops their diagnostics (a review rule normally would not fire on
/// a deleted file anyway).
fn gate_to_changeset(
    diagnostics: Vec<Diagnostic>,
    changeset: &crate::core::ChangeSetFacts,
    whole_file: bool,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .filter(|diagnostic| {
            let Some(file) = changeset
                .files
                .iter()
                .find(|file| file.path == diagnostic.file)
            else {
                return false;
            };
            if whole_file {
                return true;
            }
            let start = diagnostic.range.start_line;
            let end = diagnostic.range.end_line;
            file.new_line_ranges
                .iter()
                .any(|&(lo, hi)| start <= hi && lo <= end)
        })
        .collect()
}

fn run_local_rule_host(
    root: &Path,
    manifest: &Path,
    args: &CheckArgs,
    apply_ignore_comments: bool,
) -> Result<Vec<Diagnostic>> {
    // The outer `check` path always runs Check-kind rules and injects no diff.
    run_local_rule_host_kind(root, manifest, args, apply_ignore_comments, "check", None)
}

/// Run a repo-local rule host, selecting the rule `kind` and optionally
/// injecting a `polint review` changeset file via `--changed-files`.
///
/// `check` calls this with `kind = "check"` and `changed_files = None`;
/// `polint review` calls it with `kind = "review"` and the serialized changeset
/// path. The host runs the same inner `check` subcommand either way.
fn run_local_rule_host_kind(
    root: &Path,
    manifest: &Path,
    args: &CheckArgs,
    apply_ignore_comments: bool,
    kind: &str,
    changed_files: Option<&Path>,
) -> Result<Vec<Diagnostic>> {
    let cargo = std::env::var("POLINT_CARGO")
        .or_else(|_| std::env::var("CARGO"))
        .unwrap_or_else(|_| "cargo".to_string());
    let cache_layout = CacheLayout::for_repo(root);
    let mut command = ProcessCommand::new(&cargo);
    command.current_dir(root).args(["run", "--quiet"]);
    apply_local_rule_host_profile(&mut command);
    command.args([
        "--manifest-path",
        manifest
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("non-UTF-8 manifest path: {}", manifest.display()))?,
        "--",
        "check",
        "--format",
        "json",
        "--fail-on",
        "none",
        "--ignore-comments",
        if apply_ignore_comments {
            "true"
        } else {
            "false"
        },
        "--kind",
        kind,
    ]);
    if let Some(changed_files) = changed_files {
        command.args([
            "--changed-files",
            changed_files.to_str().ok_or_else(|| {
                anyhow::anyhow!("non-UTF-8 changeset path: {}", changed_files.display())
            })?,
        ]);
    }
    command
        .env(POLINT_CACHE_DIR_ENV, cache_layout.root())
        .env("CARGO_TARGET_DIR", cache_layout.rules_target_dir());
    if let Some(profile) = &args.profile {
        command.args(["--profile", profile]);
    }
    if args.no_cache {
        command.arg("--no-cache");
    }
    if let Some(pattern) = &args.only_rule {
        command.args(["--only-rule", pattern]);
    }
    if let Ok(toolchain) = std::env::var(rules_host_error::POLINT_RULES_TOOLCHAIN)
        && !toolchain.is_empty()
    {
        command.env("RUSTUP_TOOLCHAIN", toolchain);
    }
    command.args(args.paths.iter().map(|path| path.as_os_str()));

    let output = command.output().with_context(|| {
        format!(
            "failed to run local rule host {} with `{cargo}`",
            manifest.display()
        )
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(rules_host_error::rules_host_error_message(
            &manifest.display().to_string(),
            output.status,
            stdout.as_ref(),
            stderr.as_ref(),
        ));
    }

    let stdout = String::from_utf8(output.stdout).with_context(|| {
        format!(
            "local rule host emitted non-UTF-8 output: {}",
            manifest.display()
        )
    })?;
    diagnostics_from_public_json_report(&stdout).with_context(|| {
        format!(
            "local rule host did not emit polint JSON report: {}",
            manifest.display()
        )
    })
}

fn run_local_rule_host_inspect(root: &Path, manifest: &Path) -> Result<InspectRuleReport> {
    let cargo = std::env::var("POLINT_CARGO")
        .or_else(|_| std::env::var("CARGO"))
        .unwrap_or_else(|_| "cargo".to_string());
    let cache_layout = CacheLayout::for_repo(root);
    let mut command = ProcessCommand::new(&cargo);
    command.current_dir(root).args(["run", "--quiet"]);
    apply_local_rule_host_profile(&mut command);
    command.args([
        "--manifest-path",
        manifest
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("non-UTF-8 manifest path: {}", manifest.display()))?,
        "--",
        "inspect",
        "rule",
        "--format",
        "json",
    ]);
    command
        .env(POLINT_CACHE_DIR_ENV, cache_layout.root())
        .env("CARGO_TARGET_DIR", cache_layout.rules_target_dir());
    if let Ok(toolchain) = std::env::var(rules_host_error::POLINT_RULES_TOOLCHAIN)
        && !toolchain.is_empty()
    {
        command.env("RUSTUP_TOOLCHAIN", toolchain);
    }

    let output = command.output().with_context(|| {
        format!(
            "failed to inspect local rule host {} with `{cargo}`",
            manifest.display()
        )
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        anyhow::bail!(rules_host_error::rules_host_error_message(
            &manifest.display().to_string(),
            output.status,
            stdout.as_ref(),
            stderr.as_ref(),
        ));
    }

    let stdout = String::from_utf8(output.stdout).with_context(|| {
        format!(
            "local rule host emitted non-UTF-8 inspect output: {}",
            manifest.display()
        )
    })?;
    serde_json::from_str(&stdout).with_context(|| {
        format!(
            "local rule host did not emit polint inspect JSON: {}",
            manifest.display()
        )
    })
}

fn local_manifest_path(root: &Path, manifest: &Path) -> String {
    manifest
        .strip_prefix(root)
        .unwrap_or(manifest)
        .to_string_lossy()
        .replace('\\', "/")
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LocalRuleHostProfile {
    Dev,
    Release,
    Custom(String),
}

impl LocalRuleHostProfile {
    fn from_env() -> Self {
        Self::from_env_value(std::env::var(POLINT_RULES_PROFILE_ENV).ok())
    }

    fn from_env_value(value: Option<String>) -> Self {
        let Some(value) = value else {
            return Self::Release;
        };
        let trimmed = value.trim();
        match trimmed.to_ascii_lowercase().as_str() {
            "" | "dev" | "debug" => Self::Dev,
            "release" => Self::Release,
            _ => Self::Custom(trimmed.to_string()),
        }
    }
}

fn apply_local_rule_host_profile(command: &mut ProcessCommand) {
    match LocalRuleHostProfile::from_env() {
        LocalRuleHostProfile::Dev => {}
        LocalRuleHostProfile::Release => {
            command.arg("--release");
        }
        LocalRuleHostProfile::Custom(profile) => {
            command.args(["--profile", &profile]);
        }
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

fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::{Cli, LocalRuleHostProfile, explain_derived_edge_provenance};

    #[test]
    fn explain_private_plumbing_surfaces_derived_edge_provenance() {
        // D-10: the private plumbing reached from `explain` surfaces, for a derived
        // edge, its contributing facts + constraint kind + solver step — WITHOUT a
        // new public JSON field.
        use crate::analysis::ids::SemanticConstraintId;
        use crate::analysis::points_to::facts::{PointsToPrecision, PointsToStatus};
        use crate::analysis::semantic_graph::constraints::{ConstraintFact, ConstraintKind};
        use crate::analysis::solver::engine::derive_edges;
        use crate::analysis::solver::store::SolverStore;

        fn copy(stable_key: &str, src: u64, dst: u64) -> ConstraintFact {
            ConstraintFact {
                id: SemanticConstraintId(0),
                kind: ConstraintKind::CopyEdge {
                    dst: crate::analysis::ids::SemanticNodeId(dst),
                    src: crate::analysis::ids::SemanticNodeId(src),
                },
                status: PointsToStatus::Present,
                precision: PointsToPrecision::FlowInsensitive,
                stable_key: stable_key.to_string(),
            }
        }

        let constraints = vec![copy("copy|a-b", 1, 2), copy("copy|b-c", 2, 3)];
        let budget = crate::analysis::solver::budget::SolverBudget::default();
        let output = derive_edges(&constraints, &budget);
        let store = SolverStore::from_output(output).expect("store");

        // Pick the transitive edge (the one with 2 contributing facts).
        let transitive = store
            .derived_edges()
            .iter()
            .find(|e| e.provenance.contributing_facts.len() == 2)
            .expect("transitive derived edge");

        let view = explain_derived_edge_provenance(&store, &transitive.stable_key)
            .expect("provenance surfaced via private plumbing");
        assert_eq!(view.constraint_kind, "copy_edge");
        assert_eq!(view.contributing_fact_keys.len(), 2);
        assert!(view.solver_step > 0);
        // A missing edge key yields None (no panic, no public surface).
        assert!(explain_derived_edge_provenance(&store, "edge|does|not|exist").is_none());
    }

    #[test]
    fn local_rule_host_profile_defaults_to_release() {
        assert_eq!(
            LocalRuleHostProfile::from_env_value(None),
            LocalRuleHostProfile::Release
        );
    }

    #[test]
    fn local_rule_host_profile_accepts_dev_aliases_for_fast_local_builds() {
        assert_eq!(
            LocalRuleHostProfile::from_env_value(Some("dev".to_string())),
            LocalRuleHostProfile::Dev
        );
        assert_eq!(
            LocalRuleHostProfile::from_env_value(Some("debug".to_string())),
            LocalRuleHostProfile::Dev
        );
        assert_eq!(
            LocalRuleHostProfile::from_env_value(Some(String::new())),
            LocalRuleHostProfile::Dev
        );
    }

    #[test]
    fn local_rule_host_profile_accepts_custom_cargo_profiles() {
        assert_eq!(
            LocalRuleHostProfile::from_env_value(Some("profiling".to_string())),
            LocalRuleHostProfile::Custom("profiling".to_string())
        );
    }

    #[test]
    fn public_help_does_not_expose_phase33_internal_markers() {
        let mut command = Cli::command();
        let help = command
            .find_subcommand_mut("check")
            .expect("check subcommand exists")
            .render_long_help()
            .to_string();

        for marker in [
            ["de", "mand"].concat(),
            ["s", "cc"].concat(),
            ["quaran", "tine"].concat(),
            ["back", "dating"].concat(),
        ] {
            assert!(
                !help.contains(marker.as_str()),
                "public help leaked Phase 33 internal marker `{marker}`"
            );
        }
    }
}
