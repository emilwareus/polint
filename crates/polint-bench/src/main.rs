//! Run `cargo run -p polint-bench --release` from the workspace root for stable numbers.
//!
//! Sweeps `examples/*` with `--no-cache` semantics (disabled polint cache) and prints
//! per-example breakdown plus an aggregate table.

use anyhow::Result;
use polint_bench::{PipelineBreakdown, cold_analyze_breakdown, example_repo_roots};
use std::path::Path;

fn main() -> Result<()> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
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
    let examples = example_repo_roots(&repo_root)?;
    let n = examples.len();
    let mut rows: Vec<(String, PipelineBreakdown, usize)> = Vec::new();

    for ex in examples {
        let name = ex
            .strip_prefix(&repo_root)
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
