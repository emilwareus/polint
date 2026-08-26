//! Build-cost baseline harness for the repo-local rule-host path.
//!
//! `polint check` in a repository that has `.polint/rules` delegates the scan to
//! a subprocess Cargo builds first, so what a user waits for is a compile, not an
//! analysis. This harness measures that compile as it stands today: how often
//! Cargo starts, how many units are really compiled, how long a run takes, how
//! much the rule-host target directory and the polint cache grow, and how much
//! resident memory the rule host reaches.
//!
//! Nothing here changes product behaviour. Cargo invocations are observed
//! through the `POLINT_CARGO` indirection the CLI already supports, compiled
//! units through `RUSTC_WRAPPER`, and rule-host wall-clock and peak RSS through
//! the `POLINT_GOLDEN_COST_PATH` sidecar the rule host already writes. A metric
//! the harness cannot observe is recorded as `null` and explained in the
//! report's `limits`; none is ever estimated.

pub mod scratch;
pub mod shim;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime};

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use scratch::{DirUsage, ScratchRepo};
use shim::InvocationTotals;

/// Schema of the committed baseline artifact.
pub const SCHEMA_VERSION: &str = "polint-build-cost-1";
/// Sidecar the rule host writes when this is set: the only peak-RSS source
/// available without adding instrumentation to the engine.
const GOLDEN_COST_PATH_ENV: &str = "POLINT_GOLDEN_COST_PATH";
const GOLDEN_COST_SCHEMA_VERSION: &str = "polint-golden-cost-1";
/// Generated fixture cases per rule id when a repository ships none.
const GENERATED_CASES_PER_RULE: u32 = 2;

/// Every metric key a run record carries, in report order. A `null` value means
/// the run could not observe that metric; the key is still present so a reader
/// can tell "not measured" from "absent in this schema version".
pub const METRIC_KEYS: &[&str] = &[
    "cargo_failed_invocations",
    "cargo_invocations",
    "cargo_registry_bytes_after",
    "cargo_registry_bytes_before",
    "cargo_wall_clock_ms",
    "compiled_units",
    "compiler_peak_rss_bytes",
    "polint_cache_bytes_after",
    "polint_cache_bytes_before",
    "polint_cache_bytes_written",
    "rule_host_peak_rss_bytes",
    "rule_host_peak_rss_delta_bytes",
    "rule_host_wall_clock_ms",
    "rule_tests_failed",
    "rule_tests_passed",
    "rule_tests_total",
    "rules_target_bytes_after",
    "rules_target_bytes_before",
    "rules_target_bytes_written",
    "rules_target_files_after",
    "rustc_invocations",
    "wall_clock_ms",
];

/// Metrics the summary and diff tables lead with.
const HEADLINE_KEYS: [&str; 4] = [
    "cargo_invocations",
    "compiled_units",
    "wall_clock_ms",
    "rules_target_bytes_after",
];

/// A measured cell of the scenario matrix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Scenario {
    /// No compiler output and no analysis cache: the full first-run cost.
    Cold,
    /// Nothing changed since the previous check.
    WarmNoop,
    /// One rule source changed, so the pack must be recompiled.
    WarmRuleEdit,
    /// One analyzed source changed; no rule source did.
    WarmSourceEdit,
    /// `polint test` over the fixture suite, from a fresh cache.
    TestSuite,
}

impl Scenario {
    /// Every scenario, in the order the report lists them.
    pub const ALL: [Self; 5] = [
        Self::Cold,
        Self::WarmNoop,
        Self::WarmRuleEdit,
        Self::WarmSourceEdit,
        Self::TestSuite,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cold => "cold",
            Self::WarmNoop => "warm-noop",
            Self::WarmRuleEdit => "warm-rule-edit",
            Self::WarmSourceEdit => "warm-source-edit",
            Self::TestSuite => "test-suite",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|it| it.as_str() == value)
    }

    /// True when each run has to start from no compiler output and no cache.
    fn needs_fresh_state(self) -> bool {
        matches!(self, Self::Cold | Self::TestSuite)
    }
}

/// One run's observations, keyed by [`METRIC_KEYS`]. A map keeps the artifact
/// self-describing and lets medians and diffs stay metric-agnostic.
pub type Metrics = BTreeMap<String, Option<u64>>;

/// Committed build-cost artifact.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BuildCostReport {
    pub schema_version: String,
    pub command: String,
    pub environment: Environment,
    /// What this report does not measure, and why.
    pub limits: Vec<String>,
    pub cells: Vec<Cell>,
}

/// What the numbers were measured on. No machine-local paths or host identity
/// are recorded: committed baselines have to stay portable.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    /// Caller-supplied name of the reference machine these numbers belong to.
    pub label: String,
    pub os: String,
    pub arch: String,
    pub logical_cpus: u32,
    pub cargo_version: String,
    pub rustc_version: String,
    pub polint_version: String,
    /// Rule-host Cargo profile; `release` is the product default.
    pub rules_profile: String,
    pub cargo_offline: bool,
}

/// One repository crossed with one scenario.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Cell {
    /// Repository under test, relative to this repository's root.
    pub repo: String,
    pub scenario: String,
    /// `ok` when every run completed, `failed` otherwise.
    pub status: String,
    pub failure_detail: Option<String>,
    pub rule_packs: u32,
    pub test_cases: u32,
    /// True when the fixture cases were generated because the repository ships
    /// none. A generated case asserts nothing, so its pass/fail tally carries no
    /// signal; it exists to be built and run.
    pub test_cases_generated: bool,
    pub runs: Vec<RunRecord>,
}

/// One measured execution of a cell.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RunRecord {
    pub index: u32,
    pub status: String,
    pub failure_detail: Option<String>,
    pub metrics: Metrics,
}

/// Per-metric median over the given runs. A metric no run observed stays `null`;
/// one only some runs observed is the median of those.
pub fn median(runs: &[&Metrics]) -> Option<Metrics> {
    if runs.is_empty() {
        return None;
    }
    Some(
        METRIC_KEYS
            .iter()
            .map(|key| {
                let mut observed: Vec<u64> = runs
                    .iter()
                    .filter_map(|metrics| metrics.get(*key).copied().flatten())
                    .collect();
                observed.sort_unstable();
                (
                    (*key).to_string(),
                    observed.get(observed.len() / 2).copied(),
                )
            })
            .collect(),
    )
}

/// Median across a cell's successful runs, which is what baselines compare.
pub fn cell_median(cell: &Cell) -> Option<Metrics> {
    median(
        &cell
            .runs
            .iter()
            .filter(|run| run.status == "ok")
            .map(|run| &run.metrics)
            .collect::<Vec<_>>(),
    )
}

/// Sidecar written by the rule host; mirrors polint's own cost record.
#[derive(Clone, Debug, Deserialize)]
struct GoldenCostRecord {
    schema_version: String,
    wall_clock_ms: u64,
    peak_rss_bytes: u64,
    peak_rss_delta_bytes: u64,
}

/// Parsed command line for `polint-bench build-cost`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub repos: Vec<String>,
    pub scenarios: Vec<Scenario>,
    pub runs: u32,
    pub rules_profile: String,
    pub label: String,
    pub polint_bin: Option<PathBuf>,
    pub scratch_root: Option<PathBuf>,
    pub out: Option<PathBuf>,
    pub baseline: Option<PathBuf>,
    pub offline: bool,
    pub skip_registry: bool,
    pub keep_scratch: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            repos: vec!["examples/basic".to_string()],
            scenarios: Scenario::ALL.to_vec(),
            runs: 1,
            rules_profile: "release".to_string(),
            label: "local".to_string(),
            polint_bin: None,
            scratch_root: None,
            out: None,
            baseline: None,
            offline: false,
            skip_registry: false,
            keep_scratch: false,
        }
    }
}

pub const USAGE: &str = "\
usage: polint-bench build-cost [options]

  --repo <PATH>        repository under test, repo-relative (repeatable)
  --scenario <NAME>    cold | warm-noop | warm-rule-edit | warm-source-edit | test-suite
  --runs <N>           measured runs per cell (default 1)
  --profile <NAME>     rule-host Cargo profile (default release)
  --label <NAME>       reference-machine label recorded in the report
  --polint-bin <PATH>  polint CLI to exercise (default: alongside this binary)
  --scratch <PATH>     scratch root (default: target/polint-build-cost)
  --out <PATH>         write the JSON report here
  --baseline <PATH>    compare measured medians against this baseline
  --offline            run Cargo with CARGO_NET_OFFLINE=true
  --skip-registry      do not walk CARGO_HOME/registry
  --keep-scratch       keep each cell's scratch tree instead of deleting it
";

/// Parse `build-cost` arguments, rejecting anything unrecognized rather than
/// silently measuring something other than what was asked for.
pub fn parse_args(args: &[String]) -> Result<Options> {
    let mut options = Options::default();
    let (mut repos, mut scenarios) = (Vec::new(), Vec::new());
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| anyhow!("{name} needs a value"))
        };
        match arg.as_str() {
            "--repo" => repos.push(value("--repo")?),
            "--scenario" => {
                let raw = value("--scenario")?;
                scenarios.push(
                    Scenario::parse(&raw).ok_or_else(|| anyhow!("unknown scenario `{raw}`"))?,
                );
            }
            "--runs" => options.runs = value("--runs")?.parse().context("--runs")?,
            "--profile" => options.rules_profile = value("--profile")?,
            "--label" => options.label = value("--label")?,
            "--polint-bin" => options.polint_bin = Some(PathBuf::from(value("--polint-bin")?)),
            "--scratch" => options.scratch_root = Some(PathBuf::from(value("--scratch")?)),
            "--out" => options.out = Some(PathBuf::from(value("--out")?)),
            "--baseline" => options.baseline = Some(PathBuf::from(value("--baseline")?)),
            "--offline" => options.offline = true,
            "--skip-registry" => options.skip_registry = true,
            "--keep-scratch" => options.keep_scratch = true,
            other => bail!("unknown build-cost argument `{other}`\n\n{USAGE}"),
        }
    }
    if options.runs == 0 {
        bail!("--runs must be at least 1");
    }
    if !repos.is_empty() {
        options.repos = repos;
    }
    if !scenarios.is_empty() {
        scenarios.sort();
        scenarios.dedup();
        options.scenarios = scenarios;
    }
    Ok(options)
}

/// Resolved paths and settings shared by every cell.
struct Harness {
    repo_root: PathBuf,
    polint_bin: PathBuf,
    cargo: PathBuf,
    self_bin: PathBuf,
    scratch_root: PathBuf,
    registry_dir: Option<PathBuf>,
    options: Options,
}

/// Per-cell scratch locations the harness owns end to end.
struct CellPaths {
    cache: PathBuf,
    rules_target: PathBuf,
    shim: PathBuf,
    cost: PathBuf,
}

/// Directory accounting captured either side of a measured command.
struct CellUsage {
    rules_target: DirUsage,
    cache: DirUsage,
    registry: Option<DirUsage>,
}

/// Run the scenario matrix and return the report.
pub fn run(repo_root: &Path, options: Options) -> Result<BuildCostReport> {
    let harness = Harness::resolve(repo_root, options)?;
    let mut cells = Vec::new();
    for repo in &harness.options.repos {
        for scenario in &harness.options.scenarios {
            eprintln!("[build-cost] {repo} :: {}", scenario.as_str());
            cells.push(harness.run_cell(repo, *scenario));
        }
    }
    Ok(BuildCostReport {
        schema_version: SCHEMA_VERSION.to_string(),
        command: command_line(&harness.options),
        environment: harness.environment()?,
        limits: harness.limits(),
        cells,
    })
}

/// The invocation that produced a report, as a reader could retype it.
///
/// Options that only say where to read or write - `--out`, `--baseline`,
/// `--polint-bin`, `--scratch` - are omitted: they carry machine-local paths and
/// change nothing about what was measured. Everything that does change a
/// measurement is included.
fn command_line(options: &Options) -> String {
    let mut parts = vec![
        "polint-bench build-cost".to_string(),
        format!("--label {}", options.label),
        format!("--runs {}", options.runs),
        format!("--profile {}", options.rules_profile),
    ];
    for repo in &options.repos {
        parts.push(format!("--repo {repo}"));
    }
    for scenario in &options.scenarios {
        parts.push(format!("--scenario {}", scenario.as_str()));
    }
    if options.offline {
        parts.push("--offline".to_string());
    }
    if options.skip_registry {
        parts.push("--skip-registry".to_string());
    }
    if options.keep_scratch {
        parts.push("--keep-scratch".to_string());
    }
    parts.join(" ")
}

impl Harness {
    fn resolve(repo_root: &Path, options: Options) -> Result<Self> {
        // Canonicalize before anything derives a path from the root. Child
        // processes report the paths they were given, so a root carrying `../`
        // segments produces two spellings of the same directory and only one of
        // them can be redacted out of a committed report.
        let repo_root = repo_root
            .canonicalize()
            .unwrap_or_else(|_| repo_root.to_path_buf());
        let self_bin = std::env::current_exe().context("resolve polint-bench path")?;
        let polint_bin = match &options.polint_bin {
            Some(path) => path.clone(),
            None => self_bin
                .parent()
                .map(|dir| dir.join(format!("polint{}", std::env::consts::EXE_SUFFIX)))
                .ok_or_else(|| anyhow!("cannot locate a polint binary next to polint-bench"))?,
        };
        if !polint_bin.is_file() {
            bail!(
                "polint binary not found at {}; build it first (cargo build --release -p polint)",
                polint_bin.display()
            );
        }
        Ok(Self {
            polint_bin,
            cargo: std::env::var_os("CARGO")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("cargo")),
            self_bin,
            scratch_root: options
                .scratch_root
                .clone()
                .unwrap_or_else(|| repo_root.join("target/polint-build-cost")),
            registry_dir: (!options.skip_registry).then(cargo_registry).flatten(),
            repo_root,
            options,
        })
    }

    fn environment(&self) -> Result<Environment> {
        Ok(Environment {
            label: self.options.label.clone(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            logical_cpus: std::thread::available_parallelism()
                .map(|count| count.get() as u32)
                .unwrap_or_default(),
            cargo_version: tool_version(&self.cargo, "-V")?,
            rustc_version: tool_version(Path::new("rustc"), "-V")?,
            polint_version: tool_version(&self.polint_bin, "--version")?,
            rules_profile: self.options.rules_profile.clone(),
            cargo_offline: self.options.offline,
        })
    }

    fn limits(&self) -> Vec<String> {
        let mut notes = vec![
            "compiler_peak_rss_bytes is null: peak RSS comes from the rule host's own \
             POLINT_GOLDEN_COST_PATH sidecar and the harness adds no process instrumentation, so \
             cargo and rustc memory is not observed."
                .to_string(),
            "rule_host_* describes the last rule-host process a run started; a run that starts one \
             per fixture case reports only the last."
                .to_string(),
            "Rule packs resolve polint through a path dependency on this checkout, so \
             cargo_registry_* excludes polint itself and a cold cell downloads only the \
             third-party closure."
                .to_string(),
            "RUSTC_WRAPPER is installed to count compiled units and participates in Cargo \
             fingerprints, so every cell primes its own state; numbers are not comparable to runs \
             taken without the harness."
                .to_string(),
        ];
        if self.registry_dir.is_none() {
            notes.push("cargo_registry_* is null: registry accounting was skipped.".to_string());
        }
        if self.scratch_root.starts_with(&self.repo_root) {
            notes.push(
                "The scratch tree is inside this checkout, so Cargo discovers this repository's \
                 .cargo/config.toml when it builds the rule pack: [build] incremental = false, an \
                 aarch64-unknown-linux-gnu linker, and two SCCACHE_* variables. A consumer's \
                 repository carries none of them. incremental = false is already the release \
                 default, so a release cell is unaffected; pass --scratch outside the checkout for \
                 a build that inherits no Cargo configuration."
                    .to_string(),
            );
        }
        if !cfg!(unix) {
            notes.push(
                "Byte totals count hard-linked content once only where the platform reports \
                 stable file identity. This report was taken where it does not, so a file Cargo \
                 linked into more than one directory is counted per link and byte totals read \
                 high."
                    .to_string(),
            );
        }
        if self.options.runs == 1 {
            notes.push(
                "One run per cell was recorded. Invocation and compiled-unit counts are stable \
                 across runs; wall-clock and byte totals move with machine load and free disk. \
                 Re-run with --runs N for a median."
                    .to_string(),
            );
        }
        notes
    }

    fn run_cell(&self, repo_rel: &str, scenario: Scenario) -> Cell {
        let mut cell = Cell {
            repo: repo_rel.to_string(),
            scenario: scenario.as_str().to_string(),
            status: "ok".to_string(),
            failure_detail: None,
            rule_packs: 0,
            test_cases: 0,
            test_cases_generated: false,
            runs: Vec::new(),
        };
        let cell_dir = self
            .scratch_root
            .join(repo_rel.replace(['/', '\\'], "-"))
            .join(scenario.as_str());
        if let Err(error) = self.measure_cell(&cell_dir, repo_rel, scenario, &mut cell) {
            cell.status = "failed".to_string();
            cell.failure_detail = Some(self.sanitize(&format!("{error:#}")));
        }
        if cell.runs.iter().any(|run| run.status != "ok") {
            cell.status = "failed".to_string();
        }
        if !self.options.keep_scratch {
            // Compiler output dominates the scratch tree. Every reading a cell
            // reports is already taken by now, and keeping each cell's copy of
            // the dependency closure would make a matrix cost disk in proportion
            // to its size rather than to one cell.
            let _ = fs::remove_dir_all(&cell_dir);
        }
        cell
    }

    fn measure_cell(
        &self,
        cell_dir: &Path,
        repo_rel: &str,
        scenario: Scenario,
        cell: &mut Cell,
    ) -> Result<()> {
        let paths = CellPaths {
            cache: cell_dir.join("cache"),
            rules_target: cell_dir.join("rules-target"),
            shim: cell_dir.join("shim"),
            cost: cell_dir.join("rule-host-cost.json"),
        };

        let mut repo = self.materialize(repo_rel, cell_dir, scenario, cell)?;
        if !scenario.needs_fresh_state() {
            // A warm cell measures a repeat run, so the first build is setup and
            // deliberately sits outside every measurement.
            self.invoke(scenario, &repo, &paths)
                .context("warm-up run")?;
        }

        for index in 0..self.options.runs {
            if scenario.needs_fresh_state() {
                remove_dir(&paths.cache)?;
                remove_dir(&paths.rules_target)?;
                repo = self.materialize(repo_rel, cell_dir, scenario, cell)?;
            }
            match scenario {
                Scenario::WarmRuleEdit => scratch::edit_rule_source(&repo, index)?,
                Scenario::WarmSourceEdit => scratch::edit_scanned_source(&repo)?,
                _ => {}
            }
            remove_dir(&paths.shim)?;
            let _ = fs::remove_file(&paths.cost);

            let before = self.usage(&paths, None)?;
            let mark = mark_after_prior_writes();
            let started = Instant::now();
            let outcome = self.invoke(scenario, &repo, &paths);
            let wall_clock_ms = started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
            let after = self.usage(&paths, Some(mark))?;

            let totals = InvocationTotals::from_records(&shim::read_records(&paths.shim)?);
            let mut metrics = build_metrics(wall_clock_ms, &totals, &before, &after);
            if let Some((wall, peak, delta)) = read_golden_cost(&paths.cost)? {
                metrics.insert("rule_host_wall_clock_ms".to_string(), Some(wall));
                metrics.insert("rule_host_peak_rss_bytes".to_string(), Some(peak));
                metrics.insert("rule_host_peak_rss_delta_bytes".to_string(), Some(delta));
            }
            if let Ok(Some((passed, failed, total))) = outcome {
                metrics.insert("rule_tests_passed".to_string(), Some(passed));
                metrics.insert("rule_tests_failed".to_string(), Some(failed));
                metrics.insert("rule_tests_total".to_string(), Some(total));
            }
            let (status, failure_detail) = match &outcome {
                Ok(_) => ("ok".to_string(), None),
                Err(error) => (
                    "failed".to_string(),
                    Some(self.sanitize(&format!("{error:#}"))),
                ),
            };
            cell.runs.push(RunRecord {
                index,
                status,
                failure_detail,
                metrics,
            });
        }
        Ok(())
    }

    fn materialize(
        &self,
        repo_rel: &str,
        cell_dir: &Path,
        scenario: Scenario,
        cell: &mut Cell,
    ) -> Result<ScratchRepo> {
        let lockfile = self.repo_root.join("Cargo.lock");
        let repo = scratch::materialize(
            &self.repo_root.join(repo_rel),
            &cell_dir.join("repo"),
            &self.repo_root.join("crates/polint"),
            lockfile.is_file().then_some(lockfile.as_path()),
        )?;
        cell.rule_packs = repo.rule_packs.len() as u32;
        if scenario == Scenario::TestSuite {
            let (cases, generated) = scratch::test_cases(&repo, GENERATED_CASES_PER_RULE)?;
            cell.test_cases = cases;
            cell.test_cases_generated = generated;
        }
        Ok(repo)
    }

    /// Run the measured `polint` command, returning the fixture tally when the
    /// scenario produced one.
    fn invoke(
        &self,
        scenario: Scenario,
        repo: &ScratchRepo,
        paths: &CellPaths,
    ) -> Result<Option<(u64, u64, u64)>> {
        let mut command = Command::new(&self.polint_bin);
        command.current_dir(&repo.root);
        match scenario {
            Scenario::TestSuite => command.args(["test", "--format", "json"]),
            _ => command.args(["check", "--format", "json", "--fail-on", "none"]),
        };
        command
            .env("POLINT_CACHE_DIR", &paths.cache)
            .env("POLINT_RULES_TARGET_DIR", &paths.rules_target)
            .env("POLINT_RULES_PROFILE", &self.options.rules_profile)
            .env("POLINT_CARGO", &self.self_bin)
            .env(shim::REAL_CARGO_ENV, &self.cargo)
            .env(shim::SHIM_ROLE_ENV, "cargo")
            .env(shim::SHIM_LOG_DIR_ENV, &paths.shim)
            .env(GOLDEN_COST_PATH_ENV, &paths.cost)
            // The shim owns the compiler wrapper; an ambient one (sccache) would
            // change the measurement and hide rustc invocations.
            .env_remove("RUSTC_WRAPPER")
            .env_remove("RUSTC_WORKSPACE_WRAPPER")
            .env_remove("CARGO_TARGET_DIR");
        if self.options.offline {
            command.env("CARGO_NET_OFFLINE", "true");
        }

        let output = command
            .output()
            .with_context(|| format!("failed to run {}", self.polint_bin.display()))?;
        let code = output.status.code().unwrap_or(-1);
        // `polint test` exits 1 when fixture cases fail. Generated cases assert
        // nothing, so only a hard error (2 or more) is a failed measurement.
        let tolerated = scenario == Scenario::TestSuite && code == 1;
        if !(output.status.success() || tolerated) {
            bail!("polint exited with {code}: {}", tail(&output.stderr));
        }
        Ok((scenario == Scenario::TestSuite)
            .then(|| parse_rule_test_summary(&String::from_utf8_lossy(&output.stdout)))
            .flatten())
    }

    fn usage(&self, paths: &CellPaths, since: Option<SystemTime>) -> Result<CellUsage> {
        Ok(CellUsage {
            rules_target: scratch::dir_usage(&paths.rules_target, since)?,
            cache: scratch::dir_usage(&paths.cache, since)?,
            registry: self
                .registry_dir
                .as_deref()
                .map(|dir| scratch::dir_usage(dir, since))
                .transpose()?,
        })
    }

    /// Replace absolute paths that identify this machine: committed baselines
    /// must not carry machine-local or temporary paths. Scratch is inside the
    /// checkout, so it is redacted first.
    fn sanitize(&self, text: &str) -> String {
        text.replace(&self.scratch_root.display().to_string(), "<scratch>")
            .replace(&self.repo_root.display().to_string(), "<repo>")
            .replace(&std::env::temp_dir().display().to_string(), "<tmp>")
    }
}

fn build_metrics(
    wall_clock_ms: u64,
    totals: &InvocationTotals,
    before: &CellUsage,
    after: &CellUsage,
) -> Metrics {
    let mut metrics: Metrics = METRIC_KEYS
        .iter()
        .map(|key| ((*key).to_string(), None))
        .collect();
    let mut set = |key: &str, value: u64| {
        metrics.insert(key.to_string(), Some(value));
    };
    set("wall_clock_ms", wall_clock_ms);
    set("cargo_invocations", totals.cargo_invocations);
    set("cargo_failed_invocations", totals.cargo_failed_invocations);
    set("cargo_wall_clock_ms", totals.cargo_wall_clock_ms);
    set("rustc_invocations", totals.rustc_invocations);
    set("compiled_units", totals.compiled_units);
    set("rules_target_bytes_before", before.rules_target.bytes);
    set("rules_target_bytes_after", after.rules_target.bytes);
    set(
        "rules_target_bytes_written",
        after.rules_target.bytes_written,
    );
    set("rules_target_files_after", after.rules_target.files);
    set("polint_cache_bytes_before", before.cache.bytes);
    set("polint_cache_bytes_after", after.cache.bytes);
    set("polint_cache_bytes_written", after.cache.bytes_written);
    if let (Some(before), Some(after)) = (before.registry, after.registry) {
        set("cargo_registry_bytes_before", before.bytes);
        set("cargo_registry_bytes_after", after.bytes);
    }
    metrics
}

fn parse_rule_test_summary(stdout: &str) -> Option<(u64, u64, u64)> {
    let report: serde_json::Value = serde_json::from_str(stdout).ok()?;
    let summary = report.get("summary")?;
    let field = |name: &str| summary.get(name).and_then(serde_json::Value::as_u64);
    Some((field("passed")?, field("failed")?, field("total")?))
}

/// Read the rule host's cost sidecar as `(wall_clock_ms, peak, peak_delta)`.
fn read_golden_cost(path: &Path) -> Result<Option<(u64, u64, u64)>> {
    let Ok(raw) = fs::read_to_string(path) else {
        return Ok(None);
    };
    let record: GoldenCostRecord = serde_json::from_str(raw.trim_end())
        .with_context(|| format!("failed to parse {}", path.display()))?;
    if record.schema_version != GOLDEN_COST_SCHEMA_VERSION {
        bail!(
            "rule host wrote cost schema `{}`, expected `{GOLDEN_COST_SCHEMA_VERSION}`",
            record.schema_version
        );
    }
    Ok(Some((
        record.wall_clock_ms,
        record.peak_rss_bytes,
        record.peak_rss_delta_bytes,
    )))
}

fn tail(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let start = text.char_indices().nth_back(600).map_or(0, |(at, _)| at);
    text[start..].to_string()
}

/// Wait for the wall clock to cross the next whole second and return that
/// boundary as the "written by this run" mark.
///
/// Filesystems differ in modification-time granularity, and some store whole
/// seconds. A mark taken mid-second cannot separate what this run wrote from
/// what the warm-up or the previous run wrote in the same second, which would
/// credit a no-op run with the previous build's output. Waiting costs under a
/// second per run and makes every prior write strictly older than the mark.
fn mark_after_prior_writes() -> SystemTime {
    let now = SystemTime::now();
    let Ok(since) = now.duration_since(SystemTime::UNIX_EPOCH) else {
        return now;
    };
    let boundary =
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(since.as_secs().saturating_add(1));
    if let Ok(remaining) = boundary.duration_since(now) {
        std::thread::sleep(remaining);
    }
    boundary
}

fn remove_dir(path: &Path) -> Result<()> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("failed to remove {}", path.display())),
    }
}

fn cargo_registry() -> Option<PathBuf> {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cargo")))?;
    let registry = home.join("registry");
    registry.is_dir().then_some(registry)
}

fn tool_version(program: &Path, flag: &str) -> Result<String> {
    let output = Command::new(program)
        .arg(flag)
        .output()
        .with_context(|| format!("failed to run {} {flag}", program.display()))?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn show(metrics: Option<&Metrics>, key: &str) -> String {
    metrics
        .and_then(|metrics| metrics.get(key).copied().flatten())
        .map_or_else(|| "-".to_string(), |value| value.to_string())
}

/// Tab-separated table of the headline metrics, one row per cell. With a
/// baseline it adds that cell's committed value and the measured ratio, which is
/// what `make build-cost` reports.
pub fn render_table(measured: &BuildCostReport, baseline: Option<&BuildCostReport>) -> String {
    let key = |cell: &Cell| (cell.repo.clone(), cell.scenario.clone());
    let references: BTreeMap<_, _> = baseline
        .into_iter()
        .flat_map(|report| report.cells.iter().map(|cell| (key(cell), cell)))
        .collect();
    let environment = &measured.environment;
    let mut out = format!(
        "build cost [{}] {}/{} {} cpus, profile {}\n",
        environment.label,
        environment.os,
        environment.arch,
        environment.logical_cpus,
        environment.rules_profile,
    );
    out.push_str("repo\tscenario\tstatus\tmetric\tmeasured");
    out.push_str(if baseline.is_some() {
        "\tbaseline\tratio\n"
    } else {
        "\n"
    });
    for cell in &measured.cells {
        let metrics = cell_median(cell);
        let reference = references
            .get(&key(cell))
            .and_then(|cell| cell_median(cell));
        for metric in HEADLINE_KEYS {
            let now = metrics
                .as_ref()
                .and_then(|m| m.get(metric).copied().flatten());
            out.push_str(&format!(
                "{}\t{}\t{}\t{metric}\t{}",
                cell.repo,
                cell.scenario,
                cell.status,
                show(metrics.as_ref(), metric),
            ));
            if baseline.is_some() {
                let then = reference
                    .as_ref()
                    .and_then(|m| m.get(metric).copied().flatten());
                let ratio = match (then, now) {
                    (Some(then), Some(now)) if then != 0 => {
                        format!("{:.2}x", now as f64 / then as f64)
                    }
                    _ => "n/a".to_string(),
                };
                out.push_str(&format!("\t{}\t{ratio}", show(reference.as_ref(), metric)));
            }
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASELINE_REL: &str = "../../research/evaluation-harness/baselines/build-cost.json";

    fn metrics_of(pairs: &[(&str, u64)]) -> Metrics {
        let mut metrics: Metrics = METRIC_KEYS
            .iter()
            .map(|key| ((*key).to_string(), None))
            .collect();
        for (key, value) in pairs {
            metrics.insert((*key).to_string(), Some(*value));
        }
        metrics
    }

    fn report_of(wall: u64) -> BuildCostReport {
        BuildCostReport {
            schema_version: SCHEMA_VERSION.to_string(),
            command: "polint-bench build-cost --label test --runs 1 --profile release".to_string(),
            environment: Environment {
                label: "test".to_string(),
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                logical_cpus: 8,
                cargo_version: "cargo".to_string(),
                rustc_version: "rustc".to_string(),
                polint_version: "polint".to_string(),
                rules_profile: "release".to_string(),
                cargo_offline: false,
            },
            limits: Vec::new(),
            cells: vec![Cell {
                repo: "examples/basic".to_string(),
                scenario: "cold".to_string(),
                status: "ok".to_string(),
                failure_detail: None,
                rule_packs: 1,
                test_cases: 0,
                test_cases_generated: false,
                runs: vec![RunRecord {
                    index: 0,
                    status: "ok".to_string(),
                    failure_detail: None,
                    metrics: metrics_of(&[("wall_clock_ms", wall)]),
                }],
            }],
        }
    }

    #[test]
    fn every_scenario_round_trips_through_its_name() {
        for scenario in Scenario::ALL {
            assert_eq!(Scenario::parse(scenario.as_str()), Some(scenario));
        }
        assert_eq!(Scenario::parse("warm"), None);
    }

    #[test]
    fn only_cold_and_test_suite_reset_state_between_runs() {
        assert!(Scenario::Cold.needs_fresh_state());
        assert!(Scenario::TestSuite.needs_fresh_state());
        assert!(!Scenario::WarmNoop.needs_fresh_state());
    }

    #[test]
    fn metric_keys_are_sorted_and_unique() {
        // The key list is the schema: a duplicate or stray order would let two
        // reports claim the same version with different contents.
        let mut sorted = METRIC_KEYS.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted, METRIC_KEYS.to_vec());
    }

    #[test]
    fn a_run_carries_the_whole_key_set_with_unobservable_metrics_null() {
        let usage = CellUsage {
            rules_target: DirUsage::default(),
            cache: DirUsage::default(),
            registry: None,
        };
        let metrics = build_metrics(1, &InvocationTotals::default(), &usage, &usage);
        assert_eq!(
            metrics.keys().map(String::as_str).collect::<Vec<_>>(),
            METRIC_KEYS
        );
        assert_eq!(metrics["compiler_peak_rss_bytes"], None);
        assert_eq!(metrics["cargo_registry_bytes_after"], None);
        assert_eq!(metrics["wall_clock_ms"], Some(1));
    }

    #[test]
    fn the_report_records_the_invocation_that_produced_it() {
        // A fixed string would claim `make build-cost` for a baseline actually
        // taken over three runs of one scenario, which is the kind of label a
        // reader has no way to check.
        let args: Vec<String> = [
            "--label",
            "rig-a",
            "--runs",
            "3",
            "--repo",
            "examples/basic",
            "--scenario",
            "cold",
            "--skip-registry",
        ]
        .iter()
        .map(|value| value.to_string())
        .collect();
        assert_eq!(
            command_line(&parse_args(&args).unwrap()),
            "polint-bench build-cost --label rig-a --runs 3 --profile release \
             --repo examples/basic --scenario cold --skip-registry"
        );
    }

    #[test]
    fn the_recorded_invocation_omits_machine_local_paths() {
        let args: Vec<String> = [
            "--out",
            "/home/someone/out.json",
            "--scratch",
            "/tmp/scratch",
        ]
        .iter()
        .map(|value| value.to_string())
        .collect();
        let recorded = command_line(&parse_args(&args).unwrap());
        assert!(!recorded.contains("/home/someone"), "{recorded}");
        assert!(!recorded.contains("/tmp/scratch"), "{recorded}");
    }

    #[test]
    fn parse_args_defaults_to_the_whole_matrix() {
        let options = parse_args(&[]).unwrap();
        assert_eq!(options.scenarios, Scenario::ALL.to_vec());
        assert_eq!(options.repos, vec!["examples/basic".to_string()]);
    }

    #[test]
    fn parse_args_collects_repeated_repos_and_scenarios() {
        let args: Vec<String> = ["--repo", "a", "--repo", "b", "--scenario", "warm-noop"]
            .iter()
            .map(|value| value.to_string())
            .collect();
        let options = parse_args(&args).unwrap();
        assert_eq!(options.repos, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(options.scenarios, vec![Scenario::WarmNoop]);
    }

    #[test]
    fn parse_args_rejects_bad_input_instead_of_measuring_something_else() {
        assert!(parse_args(&["--nope".to_string()]).is_err());
        assert!(parse_args(&["--runs".to_string(), "0".to_string()]).is_err());
        assert!(parse_args(&["--repo".to_string()]).is_err());
        assert!(parse_args(&["--scenario".to_string(), "warm".to_string()]).is_err());
    }

    #[test]
    fn median_picks_the_middle_of_an_odd_sample() {
        let runs = [
            metrics_of(&[("wall_clock_ms", 30)]),
            metrics_of(&[("wall_clock_ms", 10)]),
            metrics_of(&[("wall_clock_ms", 20)]),
        ];
        let merged = median(&runs.iter().collect::<Vec<_>>()).unwrap();
        assert_eq!(merged["wall_clock_ms"], Some(20));
        assert!(median(&[]).is_none());
    }

    #[test]
    fn median_keeps_a_metric_only_some_runs_observed() {
        let runs = [
            metrics_of(&[]),
            metrics_of(&[("rule_host_peak_rss_bytes", 64)]),
        ];
        let merged = median(&runs.iter().collect::<Vec<_>>()).unwrap();
        assert_eq!(merged["rule_host_peak_rss_bytes"], Some(64));
    }

    #[test]
    fn cell_median_ignores_runs_that_failed() {
        let mut report = report_of(40);
        report.cells[0].runs.insert(
            0,
            RunRecord {
                index: 0,
                status: "failed".to_string(),
                failure_detail: Some("boom".to_string()),
                metrics: metrics_of(&[("wall_clock_ms", 5)]),
            },
        );
        assert_eq!(
            cell_median(&report.cells[0]).unwrap()["wall_clock_ms"],
            Some(40)
        );
    }

    #[test]
    fn rule_test_summary_is_read_from_the_json_report() {
        assert_eq!(
            parse_rule_test_summary(r#"{"summary":{"passed":1,"failed":2,"total":3}}"#),
            Some((1, 2, 3))
        );
        assert_eq!(parse_rule_test_summary("not json"), None);
    }

    #[test]
    fn golden_cost_sidecar_supplies_rule_host_peak_rss() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cost.json");
        fs::write(
            &path,
            r#"{"schema_version":"polint-golden-cost-1","wall_clock_ms":12,"peak_rss_bytes":34,"peak_rss_delta_bytes":5}"#,
        )
        .unwrap();
        assert_eq!(read_golden_cost(&path).unwrap(), Some((12, 34, 5)));
        assert_eq!(read_golden_cost(&dir.path().join("absent")).unwrap(), None);
    }

    #[test]
    fn an_unexpected_sidecar_schema_fails_loudly() {
        // Reading a renamed schema silently would publish numbers that mean
        // something else.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cost.json");
        fs::write(
            &path,
            r#"{"schema_version":"polint-golden-cost-2","wall_clock_ms":1,"peak_rss_bytes":2,"peak_rss_delta_bytes":3}"#,
        )
        .unwrap();
        assert!(read_golden_cost(&path).is_err());
    }

    #[test]
    fn the_table_reports_the_ratio_against_a_baseline() {
        let table = render_table(&report_of(50), Some(&report_of(100)));
        assert!(table.contains("wall_clock_ms\t50\t100\t0.50x"), "{table}");
    }

    /// The committed baseline is what every later ratio budget is expressed
    /// against, so it has to keep parsing as this schema and stay portable.
    #[test]
    fn the_committed_baseline_matches_the_schema_and_has_no_machine_paths() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(BASELINE_REL);
        let raw = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        let report: BuildCostReport = serde_json::from_str(&raw)
            .unwrap_or_else(|error| panic!("parse {}: {error}", path.display()));
        assert_eq!(report.schema_version, SCHEMA_VERSION);
        assert!(!report.cells.is_empty(), "baseline records no cell");
        for cell in &report.cells {
            for run in &cell.runs {
                assert_eq!(
                    run.metrics.keys().map(String::as_str).collect::<Vec<_>>(),
                    METRIC_KEYS,
                    "{}/{} run {} does not carry the metric key set",
                    cell.repo,
                    cell.scenario,
                    run.index
                );
            }
        }
        for machine_path in ["/home/", "/Users/", "/workspace/", "/tmp/"] {
            assert!(
                !raw.contains(machine_path),
                "baseline leaks the machine-local path {machine_path}"
            );
        }
    }
}
