use crate::analysis_plan::{ANALYSIS_PLAN_SCHEMA, AnalysisPlan, ExplainPlanReport};
use crate::config::{LoadedConfig, default_config_toml, load_config};
use crate::core::{Rule, RuleOptions, run_rules};
use crate::diagnostics::{
    ColorChoice, Diagnostic, JsonReportMeta, OutputFormat, RenderOpts, Severity,
    apply_report_filters, diagnostics_from_json_report, limit_report_diagnostics,
    render_with_sarif_help,
};
use crate::fs::load_analysis_files;
use crate::graph::{FunctionGraph, ImportGraph};
use crate::ignores::{apply_ignores, filter_report, render_ignore_report_human};
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::time::Instant;

mod rules_host_error;
mod skill;

const CHILD_EXPLAIN_PLAN_JSON_ARGS: [&str; 4] = ["explain", "plan", "--format", "json"];

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
    /// Run enabled rules.
    Check(CheckArgs),
    /// Inspect polint comment-ignore directives and suppression statistics.
    Ignores(IgnoresArgs),
    /// Explain rules or harvested facts.
    Explain(ExplainArgs),
    /// Run the fixture-based custom-rule harness.
    TestRules(CheckArgs),
    /// Print per-rule timing.
    ProfileRules(CheckArgs),
    /// Export analysis graphs.
    Graph {
        #[command(subcommand)]
        command: GraphCommand,
    },
}

#[derive(Debug, Args, Clone)]
struct NewRuleArgs {
    /// Rule language focus: go, ts, js, or generic.
    language: String,
    /// Rule directory/name.
    rule_name: String,
}

#[derive(Debug, Args)]
struct ExplainArgs {
    #[command(subcommand)]
    command: ExplainCommand,
}

#[derive(Debug, Subcommand)]
enum ExplainCommand {
    /// Describe a rule id (polint ships no bundled rules; mostly a stub for tooling).
    Rule { rule_id: String },
    /// Print the resolved analysis plan without parsing source files.
    Plan(ExplainPlanArgs),
    /// Print one harvested Go [`crate::core::TestFact`] as JSON (`docs/facts/go-tests.md`).
    #[command(name = "go-test")]
    GoTest(ExplainGoTestArgs),
}

#[derive(Debug, Args)]
struct ExplainPlanArgs {
    /// Output format.
    #[arg(long, value_enum, default_value_t = ExplainPlanFormat::Human)]
    format: ExplainPlanFormat,
    /// Named profile from .polint.toml. When omitted, all discovered rules are planned.
    #[arg(long)]
    profile: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ExplainPlanFormat {
    Human,
    Json,
}

#[derive(Debug, Args)]
struct ExplainGoTestArgs {
    /// Repository-relative path (must be analyzed under current workspace includes).
    #[arg(long)]
    file: String,
    /// Test function name (for example `TestFoo`).
    #[arg(long)]
    test: String,
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

#[derive(Debug, Subcommand)]
enum GraphCommand {
    /// Export import graph.
    Imports {
        #[arg(long, value_enum, default_value_t = GraphFormat::Dot)]
        format: GraphFormat,
    },
    /// Export function call graph for a function name.
    Function {
        function_name: String,
        #[arg(long, value_enum, default_value_t = GraphFormat::Dot)]
        format: GraphFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphFormat {
    Dot,
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
        Command::Check(args) => check(std::env::current_dir()?, &args),
        Command::Ignores(args) => ignores(std::env::current_dir()?, &args),
        Command::Explain(args) => explain_command(std::env::current_dir()?, &args),
        Command::TestRules(args) => {
            if matches!(args.format, FormatArg::Human) {
                println!("Running rule tests against the current repository fixtures.");
            }
            check(std::env::current_dir()?, &args)
        }
        Command::ProfileRules(args) => profile_rules(std::env::current_dir()?, &args),
        Command::Graph { command } => graph(std::env::current_dir()?, command),
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

fn check(root: PathBuf, args: &CheckArgs) -> Result<u8> {
    let local_rule_hosts = discover_local_rule_hosts(&root)?;
    if !local_rule_hosts.is_empty() {
        return check_local_rule_hosts(&root, args, &local_rule_hosts);
    }

    let (mut diagnostics, db, loaded) = analyze_and_run(&root, args, true)?;
    if args.ignore_comments {
        diagnostics = apply_ignores(&db, diagnostics, &loaded.config.ignores).diagnostics;
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
    let rendered_diagnostics = limit_report_diagnostics(diagnostics.clone(), args.max_diagnostics);
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

    Ok(exit_code_for(&diagnostics, args.fail_on))
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

fn explain_command(root: PathBuf, args: &ExplainArgs) -> Result<u8> {
    match &args.command {
        ExplainCommand::Rule { rule_id } => {
            println!(
                "No bundled rule found for `{rule_id}`. polint ships no built-in policy rules; repo-local rules own their metadata."
            );
            Ok(0)
        }
        ExplainCommand::Plan(plan) => explain_plan(root, plan),
        ExplainCommand::GoTest(go) => explain_go_test_fact(root, go),
    }
}

fn explain_plan(root: PathBuf, args: &ExplainPlanArgs) -> Result<u8> {
    let manifests = discover_local_rule_hosts(&root)?;
    let report = if manifests.is_empty() {
        let loaded = load_config_for_check(&root, &[])?;
        selected_rule_patterns(&loaded, args.profile.as_deref())?;
        AnalysisPlan::empty().explain_report()
    } else {
        let mut reports = Vec::new();
        for manifest in &manifests {
            reports.push(run_local_rule_host_explain_plan(&root, manifest, args)?);
        }
        combine_explain_plan_reports(reports)
    };

    match args.format {
        ExplainPlanFormat::Human => print!("{}", report.to_human()),
        ExplainPlanFormat::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }

    Ok(0)
}

fn explain_go_test_fact(root: PathBuf, args: &ExplainGoTestArgs) -> Result<u8> {
    let loaded = load_config_for_check(&root, &[])?;
    let cache = crate::cache::Cache::default_for_repo(&root, false);
    let config_digest = crate::cache::keys::config_hash(&loaded);
    let rules: Vec<Rule> = Vec::new();
    let enabled = selected_rule_patterns(&loaded, None)?;
    let options = BTreeMap::<String, RuleOptions>::new();
    let rule_digest = crate::cache::keys::rule_hash(&rules, enabled.as_ref(), &options);
    let plan = AnalysisPlan::empty();

    let mut db = load_analysis_files(&loaded)?;
    let analyze_with_plan_options = crate::go::analyze_with_plan_options;
    let _ = analyze_with_plan_options(&mut db, &cache, &config_digest, &rule_digest, &plan, true);

    let file_id = db
        .files()
        .iter()
        .find(|f| f.relative_path == args.file)
        .map(|f| f.id)
        .with_context(|| {
            format!(
                "file `{}` not found in analysis (check include globs)",
                args.file
            )
        })?;

    let fact = db
        .tests()
        .iter()
        .find(|t| t.file == file_id && t.name == args.test)
        .with_context(|| {
            format!(
                "no TestFact for function `{}` in `{}` (only `_test.go` top-level tests are harvested)",
                args.test, args.file
            )
        })?;

    let path = db.path_for(fact.file);
    let function_name = fact.function.and_then(|id| {
        db.functions()
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.name.clone())
    });

    let out = serde_json::json!({
        "file": path,
        "function": function_name,
        "name": fact.name,
        "span": {
            "start_line": fact.span.start_line,
            "start_col": fact.span.start_col,
            "end_line": fact.span.end_line,
            "end_col": fact.span.end_col,
            "start_byte": fact.span.start_byte,
            "end_byte": fact.span.end_byte,
        },
        "evidence_terms": fact.evidence_terms,
        "assertion_count": fact.assertion_count,
        "subtest_count": fact.subtest_count,
        "subtest_names": fact.subtest_names,
        "table_rows": fact.table_rows,
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(0)
}

fn profile_rules(root: PathBuf, args: &CheckArgs) -> Result<u8> {
    let config = load_config_for_check(&root, &args.paths)?;
    let cache = crate::cache::Cache::default_for_repo(&root, !args.no_cache);
    let config_digest = crate::cache::keys::config_hash(&config);
    let rules: Vec<Rule> = Vec::new();
    let mut parser_diagnostics = Vec::new();
    let enabled = selected_rule_patterns(&config, args.profile.as_deref())?;
    let all_options = BTreeMap::<String, RuleOptions>::new();
    let rule_digest = crate::cache::keys::rule_hash(&rules, enabled.as_ref(), &all_options);
    let plan = AnalysisPlan::empty();
    let mut db = load_analysis_files(&config)?;
    let analyze_with_plan_options = crate::go::analyze_with_plan_options;
    let go_diagnostics =
        analyze_with_plan_options(&mut db, &cache, &config_digest, &rule_digest, &plan, true);
    parser_diagnostics.extend(go_diagnostics);

    let analyze_with_plan_options = crate::ts::analyze_with_plan_options;
    let ts_diagnostics =
        analyze_with_plan_options(&mut db, &cache, &config_digest, &rule_digest, &plan, true);
    parser_diagnostics.extend(ts_diagnostics);
    let mut all_diagnostics = parser_diagnostics;

    for rule in &rules {
        let meta = rule.meta();
        if let Some(enabled) = &enabled
            && !enabled
                .iter()
                .any(|pattern| crate::core::rule_id_matches(pattern, &meta.id))
        {
            continue;
        }
        let mut options = BTreeMap::new();
        options.insert(meta.id.clone(), RuleOptions::default());
        let start = Instant::now();
        let diagnostics = run_rules(
            &db,
            std::slice::from_ref(rule),
            &options,
            enabled.as_ref(),
            false,
        );
        let elapsed = start.elapsed();
        println!(
            "{}\telapsed_ms={:.3}\tdiagnostics={}",
            meta.id,
            elapsed.as_secs_f64() * 1000.0,
            diagnostics.len()
        );
        all_diagnostics.extend(diagnostics);
    }
    if rules.is_empty() {
        println!("No rules registered. polint ships no built-in policy rules.");
    }

    let all_diagnostics = apply_report_filters(all_diagnostics, args.only_rule.as_deref());
    Ok(exit_code_for(&all_diagnostics, args.fail_on))
}

fn graph(root: PathBuf, command: GraphCommand) -> Result<u8> {
    let args = CheckArgs {
        paths: Vec::new(),
        profile: None,
        format: FormatArg::Human,
        color: ColorArg::Auto,
        no_cache: true,
        fail_on: FailOn::None,
        only_rule: None,
        max_diagnostics: None,
        ignore_comments: true,
    };
    let (_diagnostics, db, _) = analyze_and_run(&root, &args, false)?;
    match command {
        GraphCommand::Imports {
            format: GraphFormat::Dot,
        } => {
            println!("{}", ImportGraph::from_db(&db).to_dot());
        }
        GraphCommand::Function {
            function_name,
            format: GraphFormat::Dot,
        } => {
            println!("{}", FunctionGraph::from_db(&db, &function_name).to_dot());
        }
    }
    Ok(0)
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

    if args.ignore_comments {
        let db = load_analysis_files(&config)?;
        diagnostics = apply_ignores(&db, diagnostics, &config.config.ignores).diagnostics;
    }
    diagnostics = apply_report_filters(diagnostics, args.only_rule.as_deref());
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
    Ok(exit_code_for(&diagnostics, args.fail_on))
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

fn run_local_rule_host_explain_plan(
    root: &Path,
    manifest: &Path,
    args: &ExplainPlanArgs,
) -> Result<ExplainPlanReport> {
    let cargo = std::env::var("POLINT_CARGO")
        .or_else(|_| std::env::var("CARGO"))
        .unwrap_or_else(|_| "cargo".to_string());
    let manifest_str = manifest
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-UTF-8 manifest path: {}", manifest.display()))?;
    let mut command = ProcessCommand::new(&cargo);
    command
        .current_dir(root)
        .args(["run", "--quiet", "--manifest-path", manifest_str, "--"])
        .args(CHILD_EXPLAIN_PLAN_JSON_ARGS);
    if let Some(profile) = &args.profile {
        command.args(["--profile", profile]);
    }
    if let Ok(toolchain) = std::env::var(rules_host_error::POLINT_RULES_TOOLCHAIN)
        && !toolchain.is_empty()
    {
        command.env("RUSTUP_TOOLCHAIN", toolchain);
    }

    let output = command.output().with_context(|| {
        format!(
            "failed to run local rule host explain plan {} with `{cargo}`",
            manifest.display()
        )
    })?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        anyhow::bail!(
            "local rule host explain plan failed for {} with status {}\nstdout:\n{}\nstderr:\n{}",
            manifest.display(),
            output.status,
            stdout,
            stderr
        );
    }

    serde_json::from_str(stdout.as_ref()).with_context(|| {
        format!(
            "local rule host explain plan emitted invalid JSON: {}\nstdout:\n{}\nstderr:\n{}",
            manifest.display(),
            stdout,
            stderr
        )
    })
}

fn combine_explain_plan_reports(mut reports: Vec<ExplainPlanReport>) -> ExplainPlanReport {
    if reports.len() == 1 {
        return reports.remove(0);
    }

    let mut digest_parts = Vec::new();
    let mut rules = Vec::new();
    let mut capabilities = Vec::new();
    let mut setup_checks = Vec::new();
    for report in reports {
        digest_parts.push(report.digest);
        rules.extend(report.rules);
        capabilities.extend(report.capabilities);
        setup_checks.extend(report.setup_checks);
    }
    let digest_refs = digest_parts.iter().map(String::as_str).collect::<Vec<_>>();

    ExplainPlanReport {
        schema: ANALYSIS_PLAN_SCHEMA.to_string(),
        digest: crate::cache::stable_hash(&digest_refs),
        rules,
        capabilities,
        setup_checks,
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
