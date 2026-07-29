use crate::analysis_kernel::{AnalysisKernel, KernelInput, KernelOutput};
use crate::analysis_plan::{AnalysisPlan, RulePlanInputs};
use crate::config::{LoadedConfig, load_config};
use crate::core::{Rule, RuleKind, run_rules_with_runtime_provider_blockers};
use crate::diagnostics::{
    ColorChoice, JsonReportMeta, OutputFormat, RenderOpts, Severity, apply_report_filters,
    build_ai_friendly_report, limit_report_diagnostics, render_ai_friendly_stdout,
    render_with_sarif_help,
};
use crate::ignores::apply_ignores;
use crate::rule_manifest::{InspectRuleReport, RuleManifestWire};
use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

const AI_FRIENDLY_OUTPUT_DIR: &str = ".polint/output";
const AI_FRIENDLY_LATEST_OUTPUT: &str = ".polint/output/latest.json";

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
    /// Inspect registered local rules without running analysis.
    Inspect(InspectArgs),
}

#[derive(Debug, Args, Clone)]
struct InspectArgs {
    #[command(subcommand)]
    command: InspectCommand,
}

#[derive(Debug, Subcommand, Clone)]
enum InspectCommand {
    /// Show registered rule manifests.
    Rule(InspectRuleArgs),
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
#[command(
    after_help = "AI agents: use `--format ai-friendly` to print a compact summary and save queryable JSON under `.polint/output/`."
)]
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
    /// Disable analysis/fact cache reads and writes.
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
    /// Which rule kind to run (internal; set by `polint review`).
    #[arg(long, value_enum, default_value_t = KindArg::Check, hide = true)]
    kind: KindArg,
    /// Internal: path to a JSON changeset injected by `polint review`.
    #[arg(long, value_name = "FILE", hide = true)]
    changed_files: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum KindArg {
    Check,
    Review,
}

impl From<KindArg> for RuleKind {
    fn from(value: KindArg) -> Self {
        match value {
            KindArg::Check => RuleKind::Check,
            KindArg::Review => RuleKind::Review,
        }
    }
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
enum InspectFormatArg {
    Human,
    Json,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum FailOn {
    Warn,
    Error,
    None,
}

pub fn run_cli(rules: Vec<Rule>) -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .try_init()
        .ok();
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
        Command::Inspect(args) => inspect(std::env::current_dir()?, &args, &rules),
    }
}

fn inspect(root: PathBuf, args: &InspectArgs, rules: &[Rule]) -> Result<u8> {
    match &args.command {
        InspectCommand::Rule(rule_args) => inspect_rule(root, rule_args, rules),
    }
}

fn inspect_rule(root: PathBuf, args: &InspectRuleArgs, rules: &[Rule]) -> Result<u8> {
    let loaded = load_config_for_check(&root, &[])?;
    let mut plan_inputs = RulePlanInputs::collect(rules, None);
    plan_inputs.retain_rule_pattern(args.rule.as_deref());
    let selected_rule_ids = plan_inputs.exact_rule_ids();
    if args.rule.is_some() && selected_rule_ids.is_empty() {
        anyhow::bail!(
            "no registered rule matched `{}`",
            args.rule.as_deref().unwrap_or_default()
        );
    }

    let options = plan_inputs.rule_options_from_config(&loaded);
    let plan = AnalysisPlan::from_inputs(&plan_inputs, &options);
    let mut manifests = Vec::new();
    for rule in rules {
        let meta = rule.meta();
        if !selected_rule_ids.contains(&meta.id) {
            continue;
        }
        let manifest = rule.manifest(options.get(&meta.id));
        manifests.push(RuleManifestWire::from_manifest(
            manifest,
            None,
            plan.support_view(),
        ));
    }
    let report = InspectRuleReport::new("polint-local-rules", env!("CARGO_PKG_VERSION"), manifests);
    match args.format {
        InspectFormatArg::Human => print!("{}", render_inspect_rule_human(&report)),
        InspectFormatArg::Json => println!("{}", serde_json::to_string_pretty(&report)?),
    }
    Ok(0)
}

fn render_inspect_rule_human(report: &InspectRuleReport) -> String {
    if report.rules.is_empty() {
        return "No registered local rules.\n".to_string();
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
        out.push_str(&format!(
            "{} [{}]: {} (capabilities: {})\n",
            rule.rule_id, rule.severity, rule.description, capabilities
        ));
    }
    out
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
        FormatArg::Github => OutputFormat::Github,
        FormatArg::Json => OutputFormat::Json,
        FormatArg::Sarif => OutputFormat::Sarif,
        FormatArg::AiFriendly => OutputFormat::AiFriendly,
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
    if matches!(args.format, FormatArg::AiFriendly) {
        let report = write_ai_friendly_report(&root, &diagnostics, &rendered_diagnostics)?;
        print!(
            "{}",
            render_ai_friendly_stdout(&report, AI_FRIENDLY_LATEST_OUTPUT)
        );
    } else {
        print!(
            "{}",
            render_with_sarif_help(format, &rendered_diagnostics, opts, sarif_map)
        );
    }

    Ok(exit_code_for(&diagnostics, args.fail_on))
}

fn write_ai_friendly_report(
    root: &Path,
    diagnostics: &[crate::diagnostics::Diagnostic],
    persisted_diagnostics: &[crate::diagnostics::Diagnostic],
) -> Result<crate::diagnostics::AiFriendlyReport> {
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
        JsonReportMeta {
            tool_name: "polint-local-rules",
            tool_version: env!("CARGO_PKG_VERSION"),
        },
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
    Ok(report)
}

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

fn gitignore_line_covers(line: &str, entry: &str) -> bool {
    let t = line.trim();
    if t.is_empty() || t.starts_with('#') {
        return false;
    }
    t.trim_end_matches('/') == entry.trim_end_matches('/')
}

fn generated_at_label() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    secs.to_string()
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
    // Run only the requested rule kind. `polint check` runs `Check` rules;
    // `polint review` runs `Review` rules. Filtering here (rather than in the
    // frozen `run_rules_with_capability_support` signature) keeps capability
    // planning honest: only the selected kind's facts are planned and built.
    let wanted_kind: RuleKind = args.kind.into();
    let rules: Vec<Rule> = rules
        .iter()
        .filter(|rule| rule.meta().kind == wanted_kind)
        .cloned()
        .collect();
    let rules: &[Rule] = &rules;
    let mut plan_inputs = RulePlanInputs::collect(rules, enabled.as_ref());
    plan_inputs.retain_rule_pattern(args.only_rule.as_deref());
    let exact_enabled = plan_inputs.exact_rule_ids();
    let options = plan_inputs.rule_options_from_config(&loaded);
    let rule_digest = plan_inputs.rule_digest(&options);
    let plan = AnalysisPlan::from_inputs(&plan_inputs, &options);

    let mut diagnostics = plan_inputs.diagnostics();
    diagnostics.extend(plan.diagnostics());
    let mut output = AnalysisKernel::run(KernelInput {
        loaded: &loaded,
        cache: &cache,
        config_digest: &config_digest,
        rule_digest: &rule_digest,
        plan: &plan,
        parallel: true,
    })?;
    diagnostics.extend(std::mem::take(&mut output.diagnostics));
    // Inject the `polint review` changeset (if any) AFTER the kernel runs and
    // BEFORE rules execute, so the `ChangedFiles` fact view sees the diff. The
    // changeset is set post-kernel, so it never participates in cache identity.
    if let Some(path) = &args.changed_files {
        let changeset = read_changeset(path)?;
        output.db.set_changeset(changeset);
    }
    diagnostics.extend(dispatch_kernel_output_rules(
        &output,
        rules,
        &options,
        Some(&exact_enabled),
        true,
    ));
    Ok((diagnostics, output.db, loaded))
}

fn dispatch_kernel_output_rules(
    output: &KernelOutput,
    rules: &[Rule],
    options: &BTreeMap<String, crate::core::RuleOptions>,
    enabled: Option<&BTreeSet<String>>,
    parallel: bool,
) -> Vec<crate::diagnostics::Diagnostic> {
    run_rules_with_runtime_provider_blockers(
        &output.db,
        rules,
        options,
        enabled,
        parallel,
        &output.capability_support,
        &output.runtime_blocked_rules,
    )
}

/// Read a `polint review` changeset JSON file injected via `--changed-files`.
///
/// The file is polint-internal (written by the outer `review` command), so a
/// missing or malformed file is a loud error rather than a silent empty diff.
fn read_changeset(path: &Path) -> Result<crate::core::ReviewChangeset> {
    let raw = std::fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read injected changeset file {} (polint review internal)",
            path.display()
        )
    })?;
    serde_json::from_str(&raw).with_context(|| {
        format!(
            "failed to parse injected changeset file {} (polint review internal)",
            path.display()
        )
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Capabilities, CapabilitySupportView};
    use crate::diagnostics::{Diagnostic, TextRange, sort_diagnostics};
    use crate::sdk::facts::{FactView, FileMetrics as Metrics};
    use crate::{analysis_kernel::ProviderOutcome, cache::Cache};
    use std::sync::atomic::{AtomicUsize, Ordering};

    type Counter = std::sync::Arc<AtomicUsize>;

    #[derive(Debug, PartialEq, Eq)]
    struct Projection {
        support: CapabilitySupportView,
        provider_outcomes: Vec<ProviderOutcome>,
        runtime_blockers: BTreeSet<String>,
        kernel_diagnostics: Vec<Diagnostic>,
        policy_diagnostics: Vec<Diagnostic>,
        policy_answers: Vec<String>,
        combined_diagnostics: Vec<Diagnostic>,
        decisions: [usize; 2],
        exit: u8,
    }

    fn run_kernel(loaded: &LoadedConfig, cache: &Cache, plan: &AnalysisPlan) -> KernelOutput {
        AnalysisKernel::run(KernelInput {
            loaded,
            cache,
            config_digest: "config",
            rule_digest: "rules",
            plan,
            parallel: false,
        })
        .expect("kernel")
    }

    fn counted_rule(id: &'static str, caps: Capabilities, counter: &Counter) -> Rule {
        let counter = Counter::clone(counter);
        Rule::from_parts(
            move || crate::core::RuleMeta {
                id: id.to_string(),
                description: id.to_string(),
                severity: Severity::Warn,
                kind: RuleKind::Check,
            },
            move || caps,
            move |db, ctx| {
                counter.fetch_add(1, Ordering::SeqCst);
                let answer = if caps.file_metrics {
                    Metrics::build(db).iter().count()
                } else {
                    0
                };
                let diagnostic =
                    Diagnostic::warning(id, "", TextRange::point(1, 1), "policy answer");
                ctx.report(diagnostic.with_evidence("answer", answer.to_string()));
                Ok(())
            },
        )
    }

    fn counts(counters: &[Counter; 2]) -> [usize; 2] {
        [
            counters[0].load(Ordering::SeqCst),
            counters[1].load(Ordering::SeqCst),
        ]
    }

    fn project(output: &KernelOutput, rules: &[Rule], counters: &[Counter; 2]) -> Projection {
        let before = counts(counters);
        let mut policy_diagnostics =
            dispatch_kernel_output_rules(output, rules, &BTreeMap::new(), None, false);
        let after = counts(counters);
        let decisions = std::array::from_fn(|index| after[index] - before[index]);
        sort_diagnostics(&mut policy_diagnostics);
        let policy_answers = policy_diagnostics
            .iter()
            .map(|diagnostic| format!("{}={}", diagnostic.rule_id, diagnostic.evidence[0].value))
            .collect();
        let mut kernel_diagnostics = output.diagnostics.clone();
        sort_diagnostics(&mut kernel_diagnostics);
        let mut combined_diagnostics =
            [kernel_diagnostics.as_slice(), policy_diagnostics.as_slice()].concat();
        sort_diagnostics(&mut combined_diagnostics);
        Projection {
            support: output.capability_support.clone(),
            provider_outcomes: output.run_report.provider_outcomes.clone(),
            runtime_blockers: output.runtime_blocked_rules.clone(),
            kernel_diagnostics,
            policy_diagnostics,
            policy_answers,
            exit: exit_code_for(&combined_diagnostics, FailOn::Warn),
            combined_diagnostics,
            decisions,
        }
    }

    #[test]
    fn production_dispatch_blocks_events_from_rejected_scheduled_refinement() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("main.ts"), "export function f(){f()}").expect("source");
        let loaded = load_config(temp.path()).expect("config");
        let counters = [Counter::default(), Counter::default()];
        let rules = vec![
            counted_rule("test/events", Capabilities::new().events(), &counters[0]),
            counted_rule("test/calls", Capabilities::new().calls(), &counters[0]),
            counted_rule("allowed", Capabilities::new(), &counters[1]),
        ];
        let plan = AnalysisPlan::from_rules(&rules, None, &BTreeMap::new());
        let mut output = run_kernel(&loaded, &Cache::new("", false), &plan);
        let refined_calls = output
            .run_report
            .provider_outcomes
            .iter_mut()
            .find(|row| row.provider_id == "polint.refined_calls")
            .expect("refined calls outcome");
        refined_calls.reject_validation_for_test();
        let (blocked_rules, diagnostics) = AnalysisKernel::runtime_capability_blockers(
            &plan,
            &output.db,
            &output.run_report.provider_outcomes,
        );
        output.runtime_blocked_rules = blocked_rules;

        dispatch_kernel_output_rules(&output, &rules, &BTreeMap::new(), None, true);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(counts(&counters), [0, 1]);
    }

    #[test]
    fn cold_warm_production_semantic_projection_matches() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("main.ts"), "export const n = 1;\n").expect("source");
        let loaded = load_config(temp.path()).expect("config");
        let cache = Cache::default_for_repo(temp.path(), true);
        let counters = [Counter::default(), Counter::default()];
        let rules = vec![
            counted_rule("metrics", Capabilities::new().file_metrics(), &counters[0]),
            counted_rule("unaffected", Capabilities::new(), &counters[1]),
        ];
        let plan = AnalysisPlan::from_rules(&rules, None, &BTreeMap::new());
        let cold = run_kernel(&loaded, &cache, &plan);
        let cold_projection = project(&cold, &rules, &counters);
        let warm = run_kernel(&loaded, &cache, &plan);
        let warm_projection = project(&warm, &rules, &counters);

        assert_eq!(cold_projection, warm_projection);
        assert_eq!(cold_projection.decisions, [1, 1]);
        let answers = &cold_projection.policy_answers;
        assert_eq!(answers.as_slice(), ["metrics=1", "unaffected=0"]);
        let cold_telemetry = &cold.run_report.provider_telemetry;
        assert_ne!(cold_telemetry, &warm.run_report.provider_telemetry);
        let expected_outcomes = &warm.run_report.provider_outcomes;
        let blob = std::fs::read_dir(cache.layer_cache_dir().join("blobs"))
            .expect("cache blobs")
            .next()
            .expect("cached payload")
            .expect("blob")
            .path();
        std::fs::write(blob, b"corrupt").expect("corrupt");
        let repaired = run_kernel(&loaded, &cache, &plan);
        assert_eq!(expected_outcomes, &repaired.run_report.provider_outcomes);
        assert!(repaired.run_report.provider_telemetry.iter().any(|row| {
            row.cache_stats.invalid_evicted_reads > 0 && row.cache_stats.recomputes > 0
        }));

        let warning_cache = Cache::new(temp.path().join("warning-cache"), true);
        std::fs::create_dir_all(warning_cache.root()).expect("warning cache root");
        std::fs::write(warning_cache.layer_cache_dir(), b"not a directory").expect("block writes");
        let warned = run_kernel(&loaded, &warning_cache, &plan);
        assert_eq!(expected_outcomes, &warned.run_report.provider_outcomes);
        let warnings = &warned.diagnostics;
        assert!(
            warnings
                .iter()
                .any(|d| d.message.contains("cache write failed"))
        );
    }
}
