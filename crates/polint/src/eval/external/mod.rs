pub(crate) mod go_x_tools_callgraph;
pub(crate) mod gosec;
pub(crate) mod jelly_callgraph;
pub(crate) mod secbench_js;

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::eval::adapter::BenchmarkAdapter;
    use crate::eval::external::go_x_tools_callgraph::GoXToolsCallgraphAdapter;
    use crate::eval::external::jelly_callgraph::JellyCallgraphAdapter;
    use crate::eval::model::EvaluationMode;
    use crate::eval::report::EvaluationRun;
    use crate::eval::runner::run_external_suite_for_test;
    use crate::eval::suite::{LocalClonePolicy, SuiteTier};

    #[test]
    fn external_graph_baseline_reports_can_be_generated() {
        let root = repo_root();
        let go_manifest =
            root.join("research/evaluation-harness/suites/go-x-tools-rta-callgraph.toml");
        let jelly_manifest =
            root.join("research/evaluation-harness/suites/jelly-callgraph-micro.toml");
        let go_repo = root.join("research/evaluation-harness/repos/golang-tools");
        let jelly_repo = root.join("research/evaluation-harness/repos/jelly");
        if !go_repo.exists() || !jelly_repo.exists() {
            return;
        }
        let output_dir = if std::env::var_os("POLINT_WRITE_GRAPH_BENCH").is_some() {
            root.join(".context/graph-benchmarks")
        } else {
            tempfile::tempdir().unwrap().keep()
        };

        let mut go_suite = GoXToolsCallgraphAdapter
            .load_manifest(&std::fs::read_to_string(&go_manifest).unwrap())
            .unwrap();
        go_suite.checkout.path = go_repo.to_string_lossy().to_string();
        go_suite.checkout.local_clone_policy = LocalClonePolicy::AllowAbsolute;
        let mut jelly_suite = JellyCallgraphAdapter
            .load_manifest(&std::fs::read_to_string(&jelly_manifest).unwrap())
            .unwrap();
        jelly_suite.checkout.path = jelly_repo.to_string_lossy().to_string();
        jelly_suite.checkout.local_clone_policy = LocalClonePolicy::AllowAbsolute;

        let tier = graph_bench_tier();
        let go_artifacts = run_external_suite_for_test(
            &GoXToolsCallgraphAdapter,
            &go_suite,
            tier,
            EvaluationMode::PolintBaseline,
            &output_dir,
        )
        .unwrap();
        let jelly_artifacts = run_external_suite_for_test(
            &JellyCallgraphAdapter,
            &jelly_suite,
            tier,
            EvaluationMode::PolintBaseline,
            &output_dir,
        )
        .unwrap();

        let go = read_run(&go_artifacts.json_path);
        let jelly = read_run(&jelly_artifacts.json_path);
        assert!(!go.cases.is_empty());
        assert!(!jelly.cases.is_empty());
        assert!(go.metrics.graph_edges_expected > 0);
        assert!(jelly.metrics.graph_edges_expected > 0);
        assert!(!go.output_hash.is_empty());
        assert!(!jelly.output_hash.is_empty());
        assert!(go.comparison_rows.len() >= 2);
        assert!(jelly.comparison_rows.len() >= 2);

        if std::env::var_os("POLINT_WRITE_GRAPH_BENCH").is_some() {
            write_summary(&output_dir, &[go, jelly]).unwrap();
        }
    }

    fn graph_bench_tier() -> SuiteTier {
        match std::env::var("POLINT_GRAPH_BENCH_TIER")
            .unwrap_or_else(|_| "fast".to_string())
            .as_str()
        {
            "fast" => SuiteTier::Fast,
            "nightly" => SuiteTier::Nightly,
            "release" => SuiteTier::Release,
            other => panic!("unsupported POLINT_GRAPH_BENCH_TIER: {other}"),
        }
    }

    fn read_run(path: &Path) -> EvaluationRun {
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
    }

    fn write_summary(output_dir: &Path, runs: &[EvaluationRun]) -> anyhow::Result<()> {
        let mut markdown = String::from(
            "# Graph Benchmark Summary\n\n| Suite | Mode | TP | FP | FN | Precision | Recall | Unknowns | Runtime ms | Output hash |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---|\n",
        );
        for run in runs {
            let runtime_ms = run
                .cases
                .iter()
                .filter_map(|case| case.runtime.observed_runtime_ms)
                .sum::<u64>();
            markdown.push_str(&format!(
                "| {} | {:?} | {} | {} | {} | {} | {} | {} | {} | `{}` |\n",
                run.suite_id,
                run.mode,
                run.metrics.true_positives,
                run.metrics.false_positives,
                run.metrics.false_negatives,
                pct(run.metrics.precision),
                pct(run.metrics.recall),
                run.metrics.unknown_count,
                runtime_ms,
                run.output_hash
            ));
        }
        std::fs::create_dir_all(output_dir)?;
        std::fs::write(output_dir.join("summary.md"), markdown)?;
        Ok(())
    }

    fn pct(value: Option<f64>) -> String {
        value
            .map(|value| format!("{:.4}", value))
            .unwrap_or_else(|| "n/a".to_string())
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }
}
