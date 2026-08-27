//! Run `cargo run -p polint-bench --release` from the workspace root for stable numbers.
//!
//! Without a subcommand, sweeps `examples/*` with `--no-cache` semantics (disabled
//! polint cache) and prints per-example breakdown plus an aggregate table.
//! `build-cost` measures what a repo-local rule host costs to build instead.

use anyhow::{Context, Result};
use polint_bench::build_cost;
use polint_bench::{PipelineBreakdown, cold_analyze_breakdown, example_repo_roots};
use std::ffi::OsString;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

fn main() -> Result<ExitCode> {
    // The build-cost harness points `POLINT_CARGO` and `RUSTC_WRAPPER` at this
    // binary, so shim mode has to be recognized before any argument parsing:
    // the argument vector then belongs to cargo or rustc, not to polint-bench.
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if let Some((role, log_dir)) = build_cost::shim::role_from_env() {
        let code = build_cost::shim::run(role, &log_dir, &args)?;
        return Ok(ExitCode::from(u8::try_from(code).unwrap_or(1)));
    }

    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let text: Vec<String> = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    if text.first().is_some_and(|first| first == "build-cost") {
        return run_build_cost(&repo_root, &text[1..]);
    }
    sweep_examples(&repo_root)?;
    Ok(ExitCode::SUCCESS)
}

/// Measure the rule-host build cost matrix and report it as JSON.
fn run_build_cost(repo_root: &Path, args: &[String]) -> Result<ExitCode> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print!("{}", build_cost::USAGE);
        return Ok(ExitCode::SUCCESS);
    }
    let options = build_cost::parse_args(args)?;
    // Read the baseline first so a bad path fails before a long measurement, and
    // so `--out` may point at the same file the comparison came from.
    let baseline = options
        .baseline
        .as_deref()
        .map(|path| -> Result<build_cost::BuildCostReport> {
            let raw = fs::read_to_string(path)?;
            Ok(serde_json::from_str(&raw)?)
        })
        .transpose()?;

    let report = build_cost::run(repo_root, options.clone())?;
    let json = format!("{}\n", serde_json::to_string_pretty(&report)?);
    match &options.out {
        Some(path) => {
            if let Err(error) = write_report(path, &json) {
                // The measurement took minutes; hand it to stdout rather than
                // lose it because the destination could not be written.
                print!("{json}");
                return Err(error);
            }
            eprintln!("wrote {}", path.display());
        }
        None => print!("{json}"),
    }
    eprint!("{}", build_cost::render_table(&report, baseline.as_ref()));
    Ok(if report.cells.iter().any(|cell| cell.status != "ok") {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}

/// Publish the report atomically.
///
/// A half-written report is worse than none: it replaces a baseline that other
/// numbers are compared against with something that no longer parses.
fn write_report(path: &Path, json: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let staged = path.with_extension("part");
    fs::write(&staged, json).with_context(|| format!("failed to write {}", staged.display()))?;
    fs::rename(&staged, path).with_context(|| format!("failed to publish {}", path.display()))
}

fn sweep_examples(repo_root: &Path) -> Result<()> {
    let parallel = std::env::args().any(|a| a == "--sequential");
    eprintln!(
        "polint-bench: repo root = {}\nmode = {}\n",
        repo_root.display(),
        if parallel {
            "sequential parsers"
        } else {
            "parallel parsers (Rayon)"
        }
    );

    let mut totals = PipelineBreakdown::default();
    let examples = example_repo_roots(repo_root)?;
    let n = examples.len();
    let mut rows: Vec<(String, PipelineBreakdown, usize)> = Vec::new();

    for ex in examples {
        let name = ex
            .strip_prefix(repo_root)
            .unwrap_or(&ex)
            .display()
            .to_string();
        let (b, db) = cold_analyze_breakdown(&ex, !parallel)?;
        eprintln!("{}\nfiles = {}", b.format_ms(&name), db.files().len());
        totals.config_and_hashes += b.config_and_hashes;
        totals.load_sources.discover += b.load_sources.discover;
        totals.load_sources.read_parallel += b.load_sources.read_parallel;
        totals.load_sources.fingerprint_and_push += b.load_sources.fingerprint_and_push;
        totals.go_analysis += b.go_analysis;
        totals.ts_analysis += b.ts_analysis;
        totals.run_rules += b.run_rules;
        rows.push((name, b, db.files().len()));
    }

    let denom = n.max(1) as f64;
    eprintln!(
        "\n=== Aggregate over {n} examples (sum of per-repo times — use for A/B, not abs latency) ==="
    );
    eprintln!(
        "avg config+hashes {:.3} ms",
        totals.config_and_hashes.as_secs_f64() * 1000.0 / denom
    );
    eprintln!(
        "avg load total    {:.3} ms  (disc {:.3} | read {:.3} | hash+db {:.3})",
        totals.load_sources.total().as_secs_f64() * 1000.0 / denom,
        totals.load_sources.discover.as_secs_f64() * 1000.0 / denom,
        totals.load_sources.read_parallel.as_secs_f64() * 1000.0 / denom,
        totals.load_sources.fingerprint_and_push.as_secs_f64() * 1000.0 / denom,
    );
    eprintln!(
        "avg go analysis   {:.3} ms",
        totals.go_analysis.as_secs_f64() * 1000.0 / denom
    );
    eprintln!(
        "avg ts analysis   {:.3} ms",
        totals.ts_analysis.as_secs_f64() * 1000.0 / denom
    );
    eprintln!(
        "avg run_rules     {:.3} ms",
        totals.run_rules.as_secs_f64() * 1000.0 / denom
    );
    eprintln!(
        "avg TOTAL (sum of phases) {:.3} ms\n",
        totals.accounted().as_secs_f64() * 1000.0 / denom
    );

    println!(
        "example\tfiles\tms_config\tms_discover\tms_read\tms_hash_db\tms_go\tms_ts\tms_rules\tms_sum"
    );
    for (name, b, files) in rows {
        println!(
            "{name}\t{files}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}",
            b.config_and_hashes.as_secs_f64() * 1000.0,
            b.load_sources.discover.as_secs_f64() * 1000.0,
            b.load_sources.read_parallel.as_secs_f64() * 1000.0,
            b.load_sources.fingerprint_and_push.as_secs_f64() * 1000.0,
            b.go_analysis.as_secs_f64() * 1000.0,
            b.ts_analysis.as_secs_f64() * 1000.0,
            b.run_rules.as_secs_f64() * 1000.0,
            b.accounted().as_secs_f64() * 1000.0,
        );
    }

    Ok(())
}
