use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use polint_config::{default_config_toml, load_config};
use polint_core::{RuleOptions, run_rules};
use polint_diagnostics::{OutputFormat, Severity, dedupe_diagnostics, render};
use polint_fs::load_analysis_files;
use polint_graph::{FunctionGraph, ImportGraph};
use polint_rules::{built_in_rules, rule_options_from_config};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

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
    /// Create .polint.toml and .polint/rules.
    Init,
    /// Scaffold a repo-local Rust rule.
    NewRule(NewRuleArgs),
    /// Run enabled rules.
    Check(CheckArgs),
    /// Explain a rule by id.
    Explain { rule_id: String },
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

fn main() -> ExitCode {
    match run() {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("polint: {error:?}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<u8> {
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
        Command::NewRule(args) => {
            new_rule(std::env::current_dir()?, &args)?;
            Ok(0)
        }
        Command::Check(args) => check(std::env::current_dir()?, &args),
        Command::Explain { rule_id } => {
            explain(&rule_id);
            Ok(0)
        }
        Command::TestRules(args) => {
            println!("Running rule tests against the current repository fixtures.");
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
    fs::create_dir_all(root.join(".polint/rules"))?;
    println!("Initialized polint config at {}", config_path.display());
    Ok(())
}

fn new_rule(root: PathBuf, args: &NewRuleArgs) -> Result<()> {
    let rule_dir = root.join(".polint/rules").join(&args.rule_name);
    let src_dir = rule_dir.join("src");
    fs::create_dir_all(&src_dir)?;
    fs::write(
        rule_dir.join("Cargo.toml"),
        format!(
            r#"[package]
name = "polint-rule-{}"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
polint-sdk = {{ path = "../../../crates/polint-sdk" }}
"#,
            sanitize_name(&args.rule_name)
        ),
    )?;
    fs::write(
        src_dir.join("lib.rs"),
        rule_template(&args.language, &args.rule_name),
    )?;
    println!("Created rule skeleton at {}", rule_dir.display());
    Ok(())
}

fn rule_template(language: &str, rule_name: &str) -> String {
    let struct_name = to_pascal_case(rule_name);
    let capability = match language {
        "go" => ".go_tests().branch_obligations()",
        "ts" | "tsx" | "js" | "jsx" => ".string_literals().jsx_attributes()",
        _ => ".syntax()",
    };
    format!(
        r#"use polint_sdk::prelude::*;

pub struct {struct_name};

impl Rule for {struct_name} {{
    fn meta(&self) -> RuleMeta {{
        RuleMeta {{
            id: "custom/{rule_name}".to_string(),
            description: "Project-specific policy: {rule_name}.".to_string(),
            severity: Severity::Warn,
        }}
    }}

    fn capabilities(&self) -> Capabilities {{
        Capabilities::new(){capability}
    }}

    fn run(&self, ctx: &mut RuleCtx<'_>) -> Result<()> {{
        for file in ctx.files() {{
            let _ = file;
        }}
        Ok(())
    }}
}}
"#
    )
}

fn check(root: PathBuf, args: &CheckArgs) -> Result<u8> {
    let (mut diagnostics, _db) = analyze_and_run(&root, args, true)?;
    let format = match args.format {
        FormatArg::Human => OutputFormat::Human,
        FormatArg::Json => OutputFormat::Json,
        FormatArg::Sarif => OutputFormat::Sarif,
    };
    diagnostics = dedupe_diagnostics(diagnostics);
    print!("{}", render(format, &diagnostics));

    if load_config(&root)?.missing && matches!(args.format, FormatArg::Human) {
        println!("Config not found. Run `polint init` to create .polint.toml.");
    }

    Ok(exit_code_for(&diagnostics, args.fail_on))
}

fn analyze_and_run(
    root: &Path,
    args: &CheckArgs,
    parallel: bool,
) -> Result<(Vec<polint_diagnostics::Diagnostic>, polint_core::AnalysisDb)> {
    let config = load_config(root)?;
    let _cache = polint_cache::Cache::default_for_repo(root, !args.no_cache);
    let mut db = load_analysis_files(&config)?;
    let mut diagnostics = Vec::new();
    diagnostics.extend(polint_go::analyze(&mut db));
    diagnostics.extend(polint_ts::analyze(&mut db));

    let rules = built_in_rules();
    let enabled: BTreeSet<String> = config.profile_rules(&args.profile).into_iter().collect();
    let mut options = BTreeMap::<String, RuleOptions>::new();
    for rule in &rules {
        let meta = rule.meta();
        options.insert(
            meta.id.clone(),
            rule_options_from_config(config.rule_config(&meta.id)),
        );
    }
    diagnostics.extend(run_rules(&db, &rules, &options, &enabled, parallel));
    Ok((diagnostics, db))
}

fn explain(rule_id: &str) {
    for rule in built_in_rules() {
        let meta = rule.meta();
        if meta.id == rule_id {
            println!(
                "{}\n\nSeverity: {}\n\n{}",
                meta.id, meta.severity, meta.description
            );
            return;
        }
    }
    println!("No built-in rule found for `{rule_id}`. Custom rules use their own metadata.");
}

fn profile_rules(root: PathBuf, args: &CheckArgs) -> Result<u8> {
    let config = load_config(&root)?;
    let mut db = load_analysis_files(&config)?;
    let mut parser_diagnostics = Vec::new();
    parser_diagnostics.extend(polint_go::analyze(&mut db));
    parser_diagnostics.extend(polint_ts::analyze(&mut db));
    let enabled: BTreeSet<String> = config.profile_rules(&args.profile).into_iter().collect();
    let rules = built_in_rules();
    let mut all_diagnostics = parser_diagnostics;

    for rule in &rules {
        let meta = rule.meta();
        if !enabled.is_empty()
            && !enabled
                .iter()
                .any(|pattern| polint_core::rule_id_matches(pattern, &meta.id))
        {
            continue;
        }
        let mut options = BTreeMap::new();
        options.insert(
            meta.id.clone(),
            rule_options_from_config(config.rule_config(&meta.id)),
        );
        let start = Instant::now();
        let diagnostics = run_rules(&db, std::slice::from_ref(rule), &options, &enabled, false);
        let elapsed = start.elapsed();
        println!(
            "{:<42} {:>8.2?} {} diagnostics",
            meta.id,
            elapsed,
            diagnostics.len()
        );
        all_diagnostics.extend(diagnostics);
    }

    Ok(exit_code_for(&all_diagnostics, args.fail_on))
}

fn graph(root: PathBuf, command: GraphCommand) -> Result<u8> {
    let args = CheckArgs {
        profile: "full".to_string(),
        format: FormatArg::Human,
        no_cache: true,
        fail_on: FailOn::None,
    };
    let (_diagnostics, db) = analyze_and_run(&root, &args, false)?;
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

fn to_pascal_case(name: &str) -> String {
    let mut out = String::new();
    for part in name.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        if part.is_empty() {
            continue;
        }
        let mut chars = part.chars();
        if let Some(first) = chars.next() {
            out.extend(first.to_uppercase());
            out.push_str(chars.as_str());
        }
    }
    if out.is_empty() {
        "CustomRule".to_string()
    } else {
        out
    }
}
