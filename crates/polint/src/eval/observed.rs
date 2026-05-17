#[cfg(test)]
mod eval_observed_kernel_tests {
    use std::fs;
    use std::path::Path;

    use crate::eval::fixtures::{NativeFixture, load_native_fixture};
    use crate::eval::model::{ObservedItem, ObservedStatus};
    use crate::eval::report::{
        CaseResult, EvaluationRun, MetricSummary, RuntimeObservation, deterministic_output_hash,
    };

    use super::*;

    fn write_fixture(root: &Path, source: &str, budget: Option<u64>) {
        fs::create_dir_all(root.join("repo/src")).unwrap();
        fs::write(
            root.join("repo/.polint.toml"),
            r#"
[workspace]
include = ["src/**"]
exclude = []
"#,
        )
        .unwrap();
        fs::write(root.join("repo/src/app.ts"), source).unwrap();

        let budget = budget.map_or_else(String::new, |max_runtime_ms| {
            format!(
                r#"
[budget]
max_runtime_ms = {max_runtime_ms}
"#
            )
        });
        fs::write(
            root.join("expected.polint-eval.toml"),
            format!(
                r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "repo"
{budget}
"#
            ),
        )
        .unwrap();
    }

    fn native_fixture(source: &str, budget: Option<u64>) -> (tempfile::TempDir, NativeFixture) {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(&fixture_dir, source, budget);
        let fixture = load_native_fixture(&fixture_dir).unwrap();
        (temp, fixture)
    }

    fn observed_for(source: &str, budget: Option<u64>) -> (tempfile::TempDir, Vec<ObservedItem>) {
        let (temp, fixture) = native_fixture(source, budget);
        let observed = observe_kernel_fixture(&fixture).unwrap();
        (temp, observed)
    }

    #[test]
    fn eval_observed_kernel_collects_provider_order_invariants_from_real_kernel() {
        let (_temp, observed) =
            observed_for("export function answer() { return 42; }\n", None);

        let provider_order = observed
            .iter()
            .filter_map(|item| match item {
                ObservedItem::Invariant(invariant)
                    if invariant.name.starts_with("provider_order.") =>
                {
                    Some((invariant.name.as_str(), invariant.value.as_str()))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            provider_order,
            vec![
                ("provider_order.0", "polint.source"),
                ("provider_order.1", "polint.go.syntax"),
                ("provider_order.2", "polint.ts.syntax"),
                ("provider_order.3", "polint.module_graph"),
                ("provider_order.4", "polint.symbol_graph"),
                ("provider_order.5", "polint.metrics"),
            ]
        );
    }

    #[test]
    fn eval_observed_kernel_normalizes_diagnostics_to_relative_paths_and_fingerprints() {
        let (temp, observed) = observed_for("export function broken(\n", None);
        let rendered = serde_json::to_string_pretty(&observed).unwrap();
        let temp_root = temp.path().to_string_lossy();

        let diagnostics = observed
            .iter()
            .filter_map(|item| match item {
                ObservedItem::Diagnostic(diagnostic) => Some(diagnostic),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.rule_id == "parser/ts"
                && diagnostic.relative_path == "src/app.ts"
                && diagnostic.fingerprint.as_deref().is_some_and(|fingerprint| !fingerprint.is_empty())
                && diagnostic.status == Some(ObservedStatus::Present)
        }));
        assert!(!rendered.contains(temp_root.as_ref()));
    }

    #[test]
    fn eval_observed_kernel_collects_metadata_debug_facts_with_stable_identity() {
        let (_temp, observed) =
            observed_for("export function answer() { return 42; }\n", None);

        assert!(observed.iter().any(|item| match item {
            ObservedItem::Fact(fact) => {
                fact.family == "SourceFile"
                    && fact.stable_key.contains("src/app.ts")
                    && fact.producer_id.as_deref() == Some("polint.source")
                    && fact.precision.as_deref() == Some("exact")
                    && fact.status == Some(ObservedStatus::Present)
            }
            _ => false,
        }));
    }

    #[test]
    fn eval_observed_kernel_json_excludes_absolute_temp_paths() {
        let (temp, observed) =
            observed_for("export function answer() { return 42; }\n", Some(60_000));
        let rendered = serde_json::to_string_pretty(&observed).unwrap();
        let temp_root = temp.path().to_string_lossy();

        assert!(!rendered.contains(temp_root.as_ref()));
        assert!(!rendered.contains("SystemTime"));
        assert!(!rendered.contains("Instant"));
        assert!(!rendered.contains("0x"));
    }

    #[test]
    fn eval_observed_kernel_runtime_budget_duration_does_not_change_output_hash() {
        let (_temp, observed) =
            observed_for("export function answer() { return 42; }\n", Some(60_000));
        let first = run_with_observed_runtime(observed.clone(), 1);
        let second = run_with_observed_runtime(observed, 999);

        assert_eq!(
            deterministic_output_hash(&first),
            deterministic_output_hash(&second)
        );
    }

    fn run_with_observed_runtime(mut observed: Vec<ObservedItem>, runtime_ms: u64) -> EvaluationRun {
        for item in &mut observed {
            if let ObservedItem::RuntimeBudget(budget) = item {
                budget.observed_runtime_ms = Some(runtime_ms);
            }
        }

        EvaluationRun {
            schema_version: "polint-eval-internal-1".to_string(),
            suite_id: "native-fixture".to_string(),
            cases: vec![CaseResult {
                case_id: "provider-order".to_string(),
                area: crate::eval::model::FixtureArea::Kernel,
                expected: Vec::new(),
                observed,
                matches: Vec::new(),
                runtime: RuntimeObservation {
                    budget_name: "provider-order".to_string(),
                    budget_passed: true,
                    observed_runtime_ms: Some(runtime_ms),
                },
            }],
            metrics: MetricSummary {
                true_positives: 0,
                false_positives: 0,
                false_negatives: 0,
                true_negatives: 0,
                unconfirmed: 0,
                false_positive_trap_hits: 0,
                forbidden_hits: 0,
                unknown_count: 0,
                graph_edges_expected: 0,
                graph_edges_observed: 0,
                graph_edges_unconfirmed: 0,
                paths_expected: 0,
                paths_observed: 0,
                paths_unconfirmed: 0,
                runtime_budget_passed: 1,
                runtime_budget_failed: 0,
                precision: None,
                recall: None,
                f1: None,
                f2: None,
                f3: None,
                false_positive_rate: None,
            },
            output_hash: String::new(),
        }
    }
}
