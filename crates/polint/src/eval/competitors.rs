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
            !self.product.name.trim().is_empty(),
            "product name must not be empty"
        );
        self.result_source.validate()
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
            source_name: "OWASP scoreboard".to_string(),
            source_url: "https://owasp.org/benchmark".to_string(),
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
            manifest_path: "research/evaluation-harness/suites/owasp-java.toml".to_string(),
            reason: String::new(),
        });

        assert!(row.validate().is_err());
    }

    fn row(result_source: ResultSource) -> BenchmarkComparisonRow {
        BenchmarkComparisonRow {
            suite_id: SuiteId("owasp-java".to_string()),
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
}
