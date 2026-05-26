use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::eval::model::EvaluationMode;
use crate::eval::suite::{SuiteId, normalize_repo_relative_path};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct BenchmarkComparisonRow {
    pub(crate) suite_id: SuiteId,
    pub(crate) suite_commit: Option<String>,
    pub(crate) mode: EvaluationMode,
    pub(crate) product: ProductIdentity,
    pub(crate) result_source: ResultSource,
    #[serde(default)]
    pub(crate) metrics: BTreeMap<String, f64>,
    #[serde(default)]
    pub(crate) limitations: Vec<String>,
}

impl BenchmarkComparisonRow {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.suite_id.0.trim().is_empty(),
            "suite id must not be empty"
        );
        anyhow::ensure!(
            !self.product.name.trim().is_empty(),
            "product name must not be empty"
        );
        anyhow::ensure!(
            !self.metrics.is_empty(),
            "comparison rows must include at least one metric"
        );
        anyhow::ensure!(
            !self.limitations.is_empty(),
            "comparison rows must record limitations or provenance notes"
        );
        match &self.result_source {
            ResultSource::ImportedPublished { .. } => {}
            ResultSource::LocallyReproduced { .. } | ResultSource::PolintRun { .. } => {
                anyhow::ensure!(
                    self.product
                        .version
                        .as_deref()
                        .is_some_and(|version| !version.trim().is_empty()),
                    "locally reproduced rows must record product version"
                );
                anyhow::ensure!(
                    self.suite_commit
                        .as_deref()
                        .is_some_and(|commit| !commit.trim().is_empty()),
                    "locally reproduced rows must record suite commit or version"
                );
            }
            ResultSource::AdapterOnly { .. } => {}
        }
        self.result_source.validate()
    }
}

pub(crate) fn sort_comparison_rows(rows: &mut [BenchmarkComparisonRow]) {
    rows.sort_by_key(comparison_sort_key);
}

pub(crate) fn semgrep_row(
    suite_id: SuiteId,
    suite_commit: Option<String>,
    version: String,
    result_source: ResultSource,
    metrics: BTreeMap<String, f64>,
    limitations: Vec<String>,
) -> BenchmarkComparisonRow {
    product_row(
        suite_id,
        suite_commit,
        EvaluationMode::ImportedScanner,
        ProductIdentity {
            name: "Semgrep".to_string(),
            version: Some(version),
            vendor: Some("Semgrep".to_string()),
        },
        result_source,
        metrics,
        limitations,
    )
}

pub(crate) fn codeql_row(
    suite_id: SuiteId,
    suite_commit: Option<String>,
    version: String,
    result_source: ResultSource,
    metrics: BTreeMap<String, f64>,
    limitations: Vec<String>,
) -> BenchmarkComparisonRow {
    product_row(
        suite_id,
        suite_commit,
        EvaluationMode::ImportedScanner,
        ProductIdentity {
            name: "CodeQL".to_string(),
            version: Some(version),
            vendor: Some("GitHub".to_string()),
        },
        result_source,
        metrics,
        limitations,
    )
}

pub(crate) fn gosec_row(
    suite_id: SuiteId,
    suite_commit: Option<String>,
    version: String,
    result_source: ResultSource,
    metrics: BTreeMap<String, f64>,
    limitations: Vec<String>,
) -> BenchmarkComparisonRow {
    product_row(
        suite_id,
        suite_commit,
        EvaluationMode::LocallyReproducedScanner,
        ProductIdentity {
            name: "gosec".to_string(),
            version: Some(version),
            vendor: Some("securego".to_string()),
        },
        result_source,
        metrics,
        limitations,
    )
}

pub(crate) fn suite_native_reference_row(
    suite_id: SuiteId,
    suite_commit: Option<String>,
    product_name: String,
    result_source: ResultSource,
    metrics: BTreeMap<String, f64>,
    limitations: Vec<String>,
) -> BenchmarkComparisonRow {
    product_row(
        suite_id,
        suite_commit,
        EvaluationMode::ImportedScanner,
        ProductIdentity {
            name: product_name,
            version: None,
            vendor: None,
        },
        result_source,
        metrics,
        limitations,
    )
}

fn product_row(
    suite_id: SuiteId,
    suite_commit: Option<String>,
    mode: EvaluationMode,
    product: ProductIdentity,
    result_source: ResultSource,
    metrics: BTreeMap<String, f64>,
    limitations: Vec<String>,
) -> BenchmarkComparisonRow {
    BenchmarkComparisonRow {
        suite_id,
        suite_commit,
        mode,
        product,
        result_source,
        metrics,
        limitations,
    }
}

fn comparison_sort_key(row: &BenchmarkComparisonRow) -> (String, String, String, String) {
    (
        row.suite_id.0.clone(),
        row.product.name.clone(),
        row.product.version.clone().unwrap_or_default(),
        result_source_sort_key(&row.result_source),
    )
}

fn result_source_sort_key(source: &ResultSource) -> String {
    match source {
        ResultSource::ImportedPublished {
            source_name,
            source_url,
            retrieved_at,
        } => format!("imported:{source_name}:{source_url}:{retrieved_at}"),
        ResultSource::LocallyReproduced {
            command,
            config_path,
            artifact_path,
        } => format!(
            "local:{command}:{}:{artifact_path}",
            config_path.as_deref().unwrap_or_default()
        ),
        ResultSource::PolintRun {
            report_path,
            config_digest,
        } => format!(
            "polint:{report_path}:{}",
            config_digest.as_deref().unwrap_or_default()
        ),
        ResultSource::AdapterOnly {
            manifest_path,
            reason,
        } => format!("adapter:{manifest_path}:{reason}"),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ProductIdentity {
    pub(crate) name: String,
    pub(crate) version: Option<String>,
    pub(crate) vendor: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub(crate) enum ResultSource {
    ImportedPublished {
        source_name: String,
        source_url: String,
        retrieved_at: String,
    },
    LocallyReproduced {
        command: String,
        config_path: Option<String>,
        artifact_path: String,
    },
    PolintRun {
        report_path: String,
        config_digest: Option<String>,
    },
    AdapterOnly {
        manifest_path: String,
        reason: String,
    },
}

impl ResultSource {
    fn validate(&self) -> anyhow::Result<()> {
        match self {
            ResultSource::ImportedPublished {
                source_name,
                source_url,
                retrieved_at,
            } => {
                anyhow::ensure!(
                    !source_name.trim().is_empty(),
                    "source_name must not be empty"
                );
                anyhow::ensure!(
                    !source_url.trim().is_empty(),
                    "source_url must not be empty"
                );
                anyhow::ensure!(
                    !retrieved_at.trim().is_empty(),
                    "retrieved_at must not be empty"
                );
            }
            ResultSource::LocallyReproduced {
                command,
                config_path,
                artifact_path,
            } => {
                anyhow::ensure!(!command.trim().is_empty(), "command must not be empty");
                if let Some(config_path) = config_path {
                    normalize_repo_relative_path("comparison.config_path", config_path)?;
                }
                normalize_repo_relative_path("comparison.artifact_path", artifact_path)?;
            }
            ResultSource::PolintRun {
                report_path,
                config_digest: _,
            } => {
                normalize_repo_relative_path("comparison.report_path", report_path)?;
            }
            ResultSource::AdapterOnly {
                manifest_path,
                reason,
            } => {
                normalize_repo_relative_path("comparison.manifest_path", manifest_path)?;
                anyhow::ensure!(
                    !reason.trim().is_empty(),
                    "adapter-only rows must explain the limitation"
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn competitor_rows_distinguish_imported_and_locally_reproduced_results() {
        let imported = row(ResultSource::ImportedPublished {
            source_name: "SecBench.js paper table".to_string(),
            source_url: "https://publications.cispa.saarland/3909/".to_string(),
            retrieved_at: "2026-05-26".to_string(),
        });
        let local = row(ResultSource::LocallyReproduced {
            command: "semgrep scan --config p/security-audit".to_string(),
            config_path: Some("research/evaluation-harness/configs/semgrep.yml".to_string()),
            artifact_path: "target/polint-eval/semgrep.json".to_string(),
        });

        imported.validate().unwrap();
        local.validate().unwrap();
        assert_ne!(imported.result_source, local.result_source);
    }

    #[test]
    fn competitor_rows_reject_absolute_local_artifacts() {
        let row = row(ResultSource::LocallyReproduced {
            command: "codeql database analyze".to_string(),
            config_path: None,
            artifact_path: "/tmp/codeql.sarif".to_string(),
        });

        assert!(row.validate().is_err());
    }

    #[test]
    fn adapter_only_rows_require_limitations() {
        let row = row(ResultSource::AdapterOnly {
            manifest_path: "research/evaluation-harness/suites/secbench-js-smoke.toml".to_string(),
            reason: String::new(),
        });

        assert!(row.validate().is_err());
    }

    #[test]
    fn locally_reproduced_rows_require_version_and_suite_commit() {
        let mut row = row(ResultSource::LocallyReproduced {
            command: "semgrep scan --config p/security-audit".to_string(),
            config_path: None,
            artifact_path: "target/polint-eval/semgrep.json".to_string(),
        });
        row.product.version = None;
        assert!(row.validate().is_err());

        row.product.version = Some("1.2.3".to_string());
        row.suite_commit = None;
        assert!(row.validate().is_err());
    }

    #[test]
    fn competitor_rows_sort_deterministically() {
        let mut rows = vec![
            codeql_row(
                SuiteId("suite-b".to_string()),
                Some("commit".to_string()),
                "2.0".to_string(),
                imported_source("CodeQL docs"),
                metrics(),
                vec!["published".to_string()],
            ),
            semgrep_row(
                SuiteId("suite-a".to_string()),
                Some("commit".to_string()),
                "1.0".to_string(),
                imported_source("Semgrep docs"),
                metrics(),
                vec!["published".to_string()],
            ),
        ];

        sort_comparison_rows(&mut rows);

        assert_eq!(rows[0].suite_id.0, "suite-a");
        assert_eq!(rows[0].product.name, "Semgrep");
    }

    #[test]
    fn known_product_constructors_do_not_hardcode_scores() {
        let row = gosec_row(
            SuiteId("gosec-samples".to_string()),
            Some("de65614d10a6".to_string()),
            "2.21.0".to_string(),
            ResultSource::LocallyReproduced {
                command: "gosec ./...".to_string(),
                config_path: None,
                artifact_path: "target/polint-eval/gosec.json".to_string(),
            },
            metrics(),
            vec!["Go-specific practical baseline".to_string()],
        );

        row.validate().unwrap();
        assert_eq!(row.metrics["precision"], 0.75);

        let suite_native = suite_native_reference_row(
            SuiteId("secbench-js-smoke".to_string()),
            Some("bc3156219138".to_string()),
            "SecBench.js reference runner".to_string(),
            imported_source("SecBench.js paper table"),
            metrics(),
            vec!["suite-native reference score".to_string()],
        );
        suite_native.validate().unwrap();
        assert_eq!(suite_native.product.name, "SecBench.js reference runner");
    }

    fn row(result_source: ResultSource) -> BenchmarkComparisonRow {
        BenchmarkComparisonRow {
            suite_id: SuiteId("secbench-js-smoke".to_string()),
            suite_commit: Some("abc123".to_string()),
            mode: EvaluationMode::ImportedScanner,
            product: ProductIdentity {
                name: "Semgrep".to_string(),
                version: Some("1.2.3".to_string()),
                vendor: Some("Semgrep".to_string()),
            },
            result_source,
            metrics: [("precision".to_string(), 0.75)].into_iter().collect(),
            limitations: vec!["published result".to_string()],
        }
    }

    fn imported_source(source_name: &str) -> ResultSource {
        ResultSource::ImportedPublished {
            source_name: source_name.to_string(),
            source_url: "https://example.test/result".to_string(),
            retrieved_at: "2026-05-26".to_string(),
        }
    }

    fn metrics() -> BTreeMap<String, f64> {
        [("precision".to_string(), 0.75)].into_iter().collect()
    }
}
