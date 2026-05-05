use crate::config::{LoadedConfig, default_config_toml, load_config};
use crate::core::{Rule, RuleOptions, run_rules};
use crate::diagnostics::{
    ColorChoice, Diagnostic, JsonReportMeta, OutputFormat, RenderOpts, Severity,
    dedupe_diagnostics, diagnostics_from_json_report, render,
};
use crate::fs::load_analysis_files;
use crate::graph::{FunctionGraph, ImportGraph};
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::Arc;
use std::time::Instant;

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
    /// Create .polint.toml and .polint/rules.
    Init,
    /// Install a repo-local AI-agent skill for using polint.
    AddSkill(skill::AddSkillArgs),
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
        Command::Explain { rule_id } => {
            explain(&rule_id);
            Ok(0)
        }
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
    fs::create_dir_all(root.join(".polint/rules"))?;
    println!("Initialized polint config at {}", config_path.display());
    Ok(())
}

fn new_rule(root: PathBuf, args: &NewRuleArgs) -> Result<()> {
    let rule_name = validate_rule_name(&args.rule_name)?;
    let rules_dir = root.join(".polint/rules");
    let module = rust_module_name(&rule_name);
    let struct_name = to_pascal_case(&rule_name);
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
        write_initial_pack_main(&rules_dir, &module, &struct_name)?;
    } else if !rules_dir.join("src/main.rs").is_file() {
        write_initial_pack_main(&rules_dir, &module, &struct_name)?;
    } else {
        prepend_pack_mod_use(&rules_dir, &module, &struct_name)?;
        append_rule_to_run_cli_vec(&rules_dir, &struct_name)?;
    }

    fs::write(
        &module_path,
        rule_module_template(&args.language, &rule_name, &struct_name),
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

fn write_initial_pack_main(rules_dir: &Path, module: &str, struct_name: &str) -> Result<()> {
    let initial = format!(
        r#"mod {module};
use {module}::{struct_name};

use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {{
    polint::runner::run_cli(vec![
        Arc::new({struct_name}),
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

fn prepend_pack_mod_use(rules_dir: &Path, module: &str, struct_name: &str) -> Result<()> {
    let main_path = rules_dir.join("src/main.rs");
    let mut src = fs::read_to_string(&main_path)
        .with_context(|| format!("failed to read {}", main_path.display()))?;
    let mod_decl = format!("mod {module};");
    let use_decl = format!("use {module}::{struct_name};");
    if !src.contains(&mod_decl) {
        src.insert_str(0, &format!("{mod_decl}\n{use_decl}\n\n"));
        fs::write(&main_path, src)?;
    }
    Ok(())
}

fn append_rule_to_run_cli_vec(rules_dir: &Path, struct_name: &str) -> Result<()> {
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
        format!("\n        Arc::new({struct_name}),\n    ")
    } else {
        format!("{trimmed}\n        Arc::new({struct_name}),\n    ")
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

fn rule_module_template(language: &str, rule_name: &str, struct_name: &str) -> String {
    let capability = match language {
        "go" => ".go_tests().branch_obligations()",
        "ts" | "tsx" | "js" | "jsx" => ".string_literals().jsx_attributes()",
        _ => ".syntax()",
    };
    let query_example = match language {
        "go" => {
            r#"        for file in ctx.files() {
            let test_count = ctx.go_tests_for_file(file.id).count();
            let branch_count = ctx.branch_obligations_for_file(file.id).count();
            let _ = (test_count, branch_count);
        }"#
        }
        "ts" | "tsx" | "js" | "jsx" => {
            r#"        for file in ctx.files() {
            let literal_count = ctx.string_literals_for_file(file.id).count();
            let attribute_count = ctx.jsx_attributes_for_file(file.id).count();
            let _ = (literal_count, attribute_count);
        }"#
        }
        _ => {
            r#"        for file in ctx.files() {
            let function_count = ctx.functions_for_file(file.id).count();
            let _ = function_count;
        }"#
        }
    };
    format!(
        r#"use polint::sdk::prelude::*;

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
{query_example}
        Ok(())
    }}
}}
"#
    )
}

fn check(root: PathBuf, args: &CheckArgs) -> Result<u8> {
    let local_rule_hosts = discover_local_rule_hosts(&root)?;
    if !local_rule_hosts.is_empty() {
        return check_local_rule_hosts(&root, args, &local_rule_hosts);
    }

    let (mut diagnostics, db) = analyze_and_run(&root, args, true)?;
    let source_map = match args.format {
        FormatArg::Human => db.sources_by_relative_path(),
        _ => BTreeMap::new(),
    };
    let format = match args.format {
        FormatArg::Human => OutputFormat::Human,
        FormatArg::Json => OutputFormat::Json,
        FormatArg::Sarif => OutputFormat::Sarif,
    };
    diagnostics = dedupe_diagnostics(diagnostics);
    let sources = match args.format {
        FormatArg::Human => Some(&source_map),
        _ => None,
    };
    print!(
        "{}",
        render(format, &diagnostics, render_opts(args, sources))
    );

    if load_config(&root)?.missing && matches!(args.format, FormatArg::Human) {
        println!("Config not found. Run `polint init` to create .polint.toml.");
    }

    Ok(exit_code_for(&diagnostics, args.fail_on))
}

fn analyze_and_run(
    root: &Path,
    args: &CheckArgs,
    parallel: bool,
) -> Result<(Vec<crate::diagnostics::Diagnostic>, crate::core::AnalysisDb)> {
    let config = load_config_for_check(root, &args.paths)?;
    let cache = crate::cache::Cache::default_for_repo(root, !args.no_cache);
    let config_digest = crate::cache::keys::config_hash(&config);
    let rules: Vec<Arc<dyn Rule>> = Vec::new();
    let enabled = selected_rule_patterns(&config, args.profile.as_deref())?;
    let options = BTreeMap::<String, RuleOptions>::new();
    let rule_digest = crate::cache::keys::rule_hash(&rules, enabled.as_ref(), &options);

    let mut db = load_analysis_files(&config)?;
    let mut diagnostics = Vec::new();
    diagnostics.extend(crate::go::analyze_with_options(
        &mut db,
        &cache,
        &config_digest,
        &rule_digest,
        parallel,
    ));
    diagnostics.extend(crate::ts::analyze_with_options(
        &mut db,
        &cache,
        &config_digest,
        &rule_digest,
        parallel,
    ));

    diagnostics.extend(run_rules(&db, &rules, &options, enabled.as_ref(), parallel));
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

fn explain(rule_id: &str) {
    println!(
        "No bundled rule found for `{rule_id}`. polint ships no built-in policy rules; repo-local rules own their metadata."
    );
}

fn profile_rules(root: PathBuf, args: &CheckArgs) -> Result<u8> {
    let config = load_config_for_check(&root, &args.paths)?;
    let cache = crate::cache::Cache::default_for_repo(&root, !args.no_cache);
    let config_digest = crate::cache::keys::config_hash(&config);
    let rules: Vec<Arc<dyn Rule>> = Vec::new();
    let mut parser_diagnostics = Vec::new();
    let enabled = selected_rule_patterns(&config, args.profile.as_deref())?;
    let all_options = BTreeMap::<String, RuleOptions>::new();
    let rule_digest = crate::cache::keys::rule_hash(&rules, enabled.as_ref(), &all_options);
    let mut db = load_analysis_files(&config)?;
    parser_diagnostics.extend(crate::go::analyze_with_options(
        &mut db,
        &cache,
        &config_digest,
        &rule_digest,
        true,
    ));
    parser_diagnostics.extend(crate::ts::analyze_with_options(
        &mut db,
        &cache,
        &config_digest,
        &rule_digest,
        true,
    ));
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

    diagnostics = dedupe_diagnostics(diagnostics);
    let source_map = if matches!(args.format, FormatArg::Human) {
        read_sources_for_diagnostics(root, &diagnostics)
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
        render(format, &diagnostics, render_opts(args, sources))
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
    ]);
    if let Some(profile) = &args.profile {
        command.args(["--profile", profile]);
    }
    if args.no_cache {
        command.arg("--no-cache");
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
        anyhow::bail!(
            "local rule host failed: {}\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
            manifest.display(),
            output.status,
            stdout.trim(),
            stderr.trim()
        );
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
