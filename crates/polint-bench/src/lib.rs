//! Cold-cache pipeline timing for Polint (no analysis cache reads/writes).
//!
//! Used by the `polint-bench` binary and integration tests to see where time
//! goes: config, disk+`AnalysisDb`, Go parse/facts, TS parse/facts, `run_rules`.

use anyhow::Result;
use polint::_bench::cache;
use polint::_bench::config::load_config;
use polint::_bench::core::{AnalysisDb, Rule, RuleOptions, run_rules};
use polint::_bench::fs::{LoadSourcesTimings, load_analysis_files_with_timings};
use polint::_bench::keys::config_hash;
use polint::_bench::{go, ts};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Wall-time breakdown of a single cold `polint check`-like pass (no polint cache).
#[derive(Debug, Clone, Default)]
pub struct PipelineBreakdown {
    /// Config load + digest + empty rule/options setup.
    pub config_and_hashes: Duration,
    /// File discovery, parallel read, and DB/hash (`polint::_bench::fs::load_analysis_files` sub-stages).
    pub load_sources: LoadSourcesTimings,
    /// `polint::_bench::go::analyze_with_options` (parse + facts; cache disabled).
    pub go_analysis: Duration,
    /// `polint::_bench::ts::analyze_with_options` (parse + facts; cache disabled).
    pub ts_analysis: Duration,
    /// `polint::_bench::core::run_rules` (empty rule list in bench harness).
    pub run_rules: Duration,
}

impl PipelineBreakdown {
    pub fn accounted(&self) -> Duration {
        self.config_and_hashes
            + self.load_sources.total()
            + self.go_analysis
            + self.ts_analysis
            + self.run_rules
    }

    /// Human-readable multi-line summary (ms, 3 decimal places).
    pub fn format_ms(&self, label: &str) -> String {
        let total = self.accounted();
        let lt = self.load_sources.total();
        let pct = |d: Duration| {
            if total.is_zero() {
                0.0
            } else {
                d.as_secs_f64() / total.as_secs_f64() * 100.0
            }
        };
        let sub_pct = |d: Duration| {
            if lt.is_zero() {
                0.0
            } else {
                d.as_secs_f64() / lt.as_secs_f64() * 100.0
            }
        };
        format!(
            "\
[{label}] pipeline breakdown (cold cache, accounted {:.3} ms)
  config+hashes {:>8.3} ms ({:5.1}%)
  load total    {:>8.3} ms ({:5.1}%)
    discover    {:>8.3} ms ({:5.1}% of load)  ignore walk + globs
    read (rayon){:>8.3} ms ({:5.1}% of load)  read_to_string
    hash+db     {:>8.3} ms ({:5.1}% of load)  content fingerprint + file ids
  go analysis   {:>8.3} ms ({:5.1}%)  tree-sitter + facts
  ts analysis   {:>8.3} ms ({:5.1}%)  oxc parse + facts
  run_rules     {:>8.3} ms ({:5.1}%)  (empty in this harness)
",
            total.as_secs_f64() * 1000.0,
            self.config_and_hashes.as_secs_f64() * 1000.0,
            pct(self.config_and_hashes),
            lt.as_secs_f64() * 1000.0,
            pct(lt),
            self.load_sources.discover.as_secs_f64() * 1000.0,
            sub_pct(self.load_sources.discover),
            self.load_sources.read_parallel.as_secs_f64() * 1000.0,
            sub_pct(self.load_sources.read_parallel),
            self.load_sources.fingerprint_and_push.as_secs_f64() * 1000.0,
            sub_pct(self.load_sources.fingerprint_and_push),
            self.go_analysis.as_secs_f64() * 1000.0,
            pct(self.go_analysis),
            self.ts_analysis.as_secs_f64() * 1000.0,
            pct(self.ts_analysis),
            self.run_rules.as_secs_f64() * 1000.0,
            pct(self.run_rules),
        )
    }
}

/// Same as `polint check` cold path with **no disk cache**, **no rules**, parallel parsers.
pub fn cold_analyze_breakdown(
    root: &Path,
    parallel: bool,
) -> Result<(PipelineBreakdown, AnalysisDb)> {
    let mut breakdown = PipelineBreakdown::default();

    let t0 = Instant::now();
    let config = load_config(root)?;
    let config_digest = config_hash(&config);
    let rules: Vec<Rule> = Vec::new();
    let options = BTreeMap::<String, RuleOptions>::new();
    let rule_digest = cache::stable_hash(&[] as &[&str]);
    let cache = cache::Cache::default_for_repo(root, false);
    breakdown.config_and_hashes = t0.elapsed();

    let (mut db, load_timings) = load_analysis_files_with_timings(&config)?;
    breakdown.load_sources = load_timings;

    let t2 = Instant::now();
    let _ = go::analyze_with_options(&mut db, &cache, &config_digest, &rule_digest, parallel);
    breakdown.go_analysis = t2.elapsed();

    let t3 = Instant::now();
    let _ = ts::analyze_with_options(&mut db, &cache, &config_digest, &rule_digest, parallel);
    breakdown.ts_analysis = t3.elapsed();

    let t4 = Instant::now();
    let _ = run_rules(&db, &rules, &options, None, parallel);
    breakdown.run_rules = t4.elapsed();

    Ok((breakdown, db))
}

/// Discover `examples/*/\.polint.toml` from the polint repo root (parent of `crates/`).
pub fn example_repo_roots(repo_root: &Path) -> Result<Vec<PathBuf>> {
    let examples = repo_root.join("examples");
    let mut roots = Vec::new();
    for entry in std::fs::read_dir(&examples)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        if path.join(".polint.toml").is_file() {
            roots.push(path);
        }
    }
    roots.sort();
    Ok(roots)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// Every checked-in example completes a cold pass; stages are sequential slices of work.
    #[test]
    fn all_examples_cold_pipeline_succeeds_and_phases_sum() {
        let root = repo_root();
        for ex in example_repo_roots(&root).unwrap() {
            let (b, db) = cold_analyze_breakdown(&ex, true).unwrap_or_else(|e| {
                panic!("cold_analyze_breakdown({}): {e:?}", ex.display());
            });
            let n = db.files().len();
            assert!(
                n > 0,
                "example {} should load at least one file",
                ex.display()
            );
            let sum = b.accounted();
            assert!(
                !sum.is_zero(),
                "example {} reported zero work with {} files",
                ex.display(),
                n
            );
            if std::env::var_os("POLINT_BENCH_LOG").is_some() {
                eprintln!("{}", b.format_ms(&ex.display().to_string()));
            }
        }
    }

    /// Accounted wall time should not exceed actual wall by more than a small margin (timer noise).
    #[test]
    fn breakdown_track_wall_clock_on_fixture() {
        let root = repo_root().join("examples/basic");
        let wall = Instant::now();
        let (b, _) = cold_analyze_breakdown(&root, true).unwrap();
        let actual = wall.elapsed();
        assert!(
            b.accounted() <= actual + Duration::from_millis(2),
            "accounted {:?} > wall {:?}",
            b.accounted(),
            actual
        );
    }
}
