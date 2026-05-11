use crate::analysis_plan::AnalysisPlan;
use crate::baseline::{
    BaselineConfig, BaselineSummary, DEFAULT_BASELINE_PATH, classify_diagnostics, load_baseline,
    render_baseline_summary, write_baseline,
};
use crate::config::{LoadedConfig, default_config_toml, load_config};
use crate::core::{AnalysisDb, Language, Rule, RuleOptions, run_rules};
use crate::diagnostics::{
    ColorChoice, Diagnostic, JsonReportMeta, OutputFormat, RenderOpts, Severity,
    apply_report_filters, diagnostics_from_json_report, limit_report_diagnostics,
    render_with_sarif_help,
};
use crate::fs::load_analysis_files;
use crate::ignores::{apply_ignores, filter_report, render_ignore_report_human};
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Arc;

mod rules_host_error;
mod skill;

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
        let path = root.join(rel);
        if path.metadata().map(|m| m.is_file()).unwrap_or(false)
            && let Ok(text) = fs::read_to_string(&path)
        {
            map.insert(diagnostic.file.clone(), Arc::from(text.into_boxed_str()));
        }
    }
    map
}

#[derive(Debug, Parser)]
#[command(name = "polint")]
#[command(about = "Repo-local static analysis policy as code.")]
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
    /// Run enabled rules.
    Check(CheckArgs),
    /// Inspect polint comment-ignore directives and suppression statistics.
    Ignores(IgnoresArgs),
}

#[derive(Debug, Args, Clone)]
struct NewRuleArgs {
    /// Rule language focus: go, ts, js, or generic.
    language: String,
    /// Rule directory/name.
    rule_name: String,
}

#[derive(Debug, Args, Clone)]
struct BaselineArgs {
    #[command(subcommand)]
    command: BaselineCommand,
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
    /// Disable .polint/cache reads and writes while creating the baseline.
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
    /// Disable .polint/cache reads and writes while updating the baseline.
    #[arg(long)]
    no_cache: bool,
    /// Files or directories to check. Defaults to the workspace include config.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

#[derive(Debug, Args, Clone)]
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
struct IgnoresArgs {
    /// Files or directories to inspect. Defaults to the workspace include config.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
    /// Named profile from .polint.toml. When omitted, all discovered rules run.
    #[arg(long)]
    profile: Option<String>,
    /// Disable .polint/cache reads and writes while collecting diagnostics.
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
    Json,
    Sarif,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum IgnoresFormatArg {
    Human,
    Json,
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
        Command::Check(args) => check(std::env::current_dir()?, &args),
        Command::Ignores(args) => ignores(std::env::current_dir()?, &args),
    }
}

fn init_project(root: PathBuf) -> Result<()> {
    let config_path = root.join(".polint.toml");
    if !config_path.exists() {
        fs::write(&config_path, default_config_toml())
            .with_context(|| format!("failed to write {}", config_path.display()))?;
    }

    let polint_dir = root.join(".polint");
    fs::create_dir_all(polint_dir.join("rules/src"))
        .with_context(|| format!("failed to create {}", polint_dir.display()))?;
    fs::create_dir_all(polint_dir.join("cache"))
        .with_context(|| format!("failed to create {}", polint_dir.join("cache").display()))?;
    ensure_polint_nested_gitignore(&polint_dir)?;
    ensure_repo_rust_toolchain_shim(&root)?;

    println!("Initialized polint config at {}", config_path.display());
    Ok(())
}

/// Ensures `.polint/.gitignore` lists `cache/` so analysis cache stays local to each machine.
fn ensure_polint_nested_gitignore(polint_dir: &Path) -> Result<()> {
    let path = polint_dir.join(".gitignore");
    const ENTRY: &str = "cache/";

    if path.is_file() {
        let existing = fs::read_to_string(&path).with_context(|| path.display().to_string())?;
        if existing.lines().any(gitignore_line_covers_polint_cache) {
            return Ok(());
        }
        let mut out = existing;
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(ENTRY);
        out.push('\n');
        fs::write(&path, out).with_context(|| format!("failed to update {}", path.display()))?;
    } else {
        let content = format!("# polint: local cache (not shared between checkouts)\n{ENTRY}\n");
        fs::write(&path, content).with_context(|| format!("failed to write {}", path.display()))?;
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

fn gitignore_line_covers_polint_cache(line: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return false;
    }
    t.trim_end_matches('/') == "cache"
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
        rule_module_template(&args.language, &rule_name),
    )?;
    println!("Created rule module {}", module_path.display());
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

fn rule_module_template(language: &str, rule_name: &str) -> String {
    let module = rust_module_name(rule_name);
    let fact_params = match language {
        "go" => "tests: GoTests<'_>, branches: BranchObligations<'_>",
        "ts" | "tsx" | "js" | "jsx" => "literals: StringLiterals<'_>, jsx: JsxAttributes<'_>",
        _ => "functions: Functions<'_>",
    };
    let query_example = match language {
        "go" => {
            r#"    for branch in branches.iter() {
        let related_test_count = tests.related_for_file(branch.file).len();
        let _ = related_test_count;
    }"#
        }
        "ts" | "tsx" | "js" | "jsx" => {
            r#"    let literal_count = literals.iter().count();
    let attribute_count = jsx.iter().count();
    let _ = (literal_count, attribute_count);"#
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
pub(crate) fn {module}(_ctx: &mut RuleCtx<'_>, {fact_params}) -> RuleResult {{
{query_example}
    // To report a diagnostic, call _ctx.warn(&some_fact.span, "message")
    // or _ctx.report(Diagnostic::...). Custom TOML fields are in
    // _ctx.options().settings.
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
    write_baseline(&output, &config)?;
    println!(
        "Created baseline {} with {} entries",
        output.display(),
        config.baseline.len()
    );
    Ok(0)
}

fn update_baseline(root: PathBuf, args: &BaselineUpdateArgs) -> Result<u8> {
    let baseline_path = baseline_path(&root);
    let config = load_baseline(&baseline_path)?;
    let diagnostics = collect_diagnostics_for_baseline(
        &root,
        &args.paths,
        args.profile.as_deref(),
        args.no_cache,
    )?;
    let classification = classify_diagnostics(&diagnostics, &config);
    let updated = config.updated_for_current_diagnostics(&diagnostics);
    write_baseline(&baseline_path, &updated)?;
    println!(
        "Updated baseline {} with {} entries",
        baseline_path.display(),
        updated.baseline.len()
    );
    print!("{}", render_baseline_summary(&classification.summary));
    Ok(0)
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
        FormatArg::Json => OutputFormat::Json,
        FormatArg::Sarif => OutputFormat::Sarif,
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
    print!(
        "{}",
        render_with_sarif_help(
            format,
            &rendered_diagnostics,
            render_opts(args, sources),
            sarif_help_map(&loaded),
        )
    );

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
    selected_rule_patterns(&config, args.profile.as_deref())?;
    let db = load_analysis_files(&config)?;
    let mut diagnostics = Vec::new();
    for manifest in &local_rule_hosts {
        diagnostics.extend(run_local_rule_host(root, manifest, &check_args)?);
    }
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

    let baseline_path = baseline_path(root);
    let config = load_baseline(&baseline_path)?;
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
        selected_rule_patterns(&config, profile)?;
        let loaded_db = load_analysis_files(&config)?;
        let mut diagnostics = Vec::new();
        for manifest in &local_rule_hosts {
            diagnostics.extend(run_local_rule_host(root, manifest, &check_args)?);
        }
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

    let mut db = load_analysis_files(&loaded)?;
    let mut diagnostics = Vec::new();
    let analyze_with_plan_options = crate::go::analyze_with_plan_options;
    let go_diagnostics = analyze_with_plan_options(
        &mut db,
        &cache,
        &config_digest,
        &rule_digest,
        &plan,
        parallel,
    );
    diagnostics.extend(go_diagnostics);

    let analyze_with_plan_options = crate::ts::analyze_with_plan_options;
    let ts_diagnostics = analyze_with_plan_options(
        &mut db,
        &cache,
        &config_digest,
        &rule_digest,
        &plan,
        parallel,
    );
    diagnostics.extend(ts_diagnostics);

    diagnostics.extend(run_rules(&db, &rules, &options, enabled.as_ref(), parallel));
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

fn check_local_rule_hosts(root: &Path, args: &CheckArgs, manifests: &[PathBuf]) -> Result<u8> {
    let config = load_config_for_check(root, &args.paths)?;
    selected_rule_patterns(&config, args.profile.as_deref())?;

    let mut diagnostics = Vec::new();
    for manifest in manifests {
        diagnostics.extend(run_local_rule_host(root, manifest, args)?);
    }

    let mut db = None;
    let mut ignore_report = None;
    if args.ignore_comments {
        let loaded_db = load_analysis_files(&config)?;
        let ignore_application = apply_ignores(&loaded_db, diagnostics, &config.config.ignores);
        diagnostics = ignore_application.diagnostics;
        ignore_report = Some(ignore_application.report);
        db = Some(loaded_db);
    } else if should_render_check_stats(args) {
        db = Some(load_analysis_files(&config)?);
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
        FormatArg::Json => OutputFormat::Json,
        FormatArg::Sarif => OutputFormat::Sarif,
    };
    let sources = match args.format {
        FormatArg::Human => Some(&source_map),
        _ => None,
    };
    print!(
        "{}",
        render_with_sarif_help(
            format,
            &rendered_diagnostics,
            render_opts(args, sources),
            sarif_help_map(&config),
        )
    );
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

fn run_local_rule_host(root: &Path, manifest: &Path, args: &CheckArgs) -> Result<Vec<Diagnostic>> {
    let cargo = std::env::var("POLINT_CARGO")
        .or_else(|_| std::env::var("CARGO"))
        .unwrap_or_else(|_| "cargo".to_string());
    let mut command = ProcessCommand::new(&cargo);
    command.current_dir(root).args([
        "run",
        "--quiet",
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
        "false",
    ]);
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
    diagnostics_from_json_report(&stdout).with_context(|| {
        format!(
            "local rule host did not emit polint JSON report: {}",
            manifest.display()
        )
    })
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
