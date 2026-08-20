use serde::{Deserialize, Serialize};

use crate::cache::stable_hash;
use crate::eval::suite::{SuiteId, normalize_repo_relative_path};

pub(crate) const ADAPTATION_SCHEMA_VERSION: &str = "polint-eval-adaptation-0";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct AdaptationRecord {
    pub(crate) schema_version: String,
    pub(crate) suite_id: SuiteId,
    pub(crate) case_selection: String,
    pub(crate) agent: AdaptationAgent,
    pub(crate) inputs_allowed: Vec<String>,
    pub(crate) inputs_forbidden: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sandbox_root: Option<String>,
    pub(crate) outputs: AdaptationOutputs,
}

impl AdaptationRecord {
    pub(crate) fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.schema_version == ADAPTATION_SCHEMA_VERSION,
            "unsupported adaptation schema_version `{}`; expected `{ADAPTATION_SCHEMA_VERSION}`",
            self.schema_version
        );
        anyhow::ensure!(
            !self.case_selection.trim().is_empty(),
            "adaptation case_selection must not be empty"
        );
        self.agent.validate()?;
        anyhow::ensure!(
            !self.inputs_allowed.is_empty(),
            "adapted runs must record allowed inputs"
        );
        anyhow::ensure!(
            !self.inputs_forbidden.is_empty(),
            "adapted runs must record forbidden inputs"
        );
        if let Some(sandbox_root) = &self.sandbox_root {
            normalize_repo_relative_path("adaptation.sandbox_root", sandbox_root)?;
        }
        for forbidden in &self.inputs_forbidden {
            anyhow::ensure!(
                !forbidden.trim().is_empty(),
                "forbidden adaptation inputs must not be empty"
            );
        }
        self.outputs.validate()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct AdaptationAgent {
    pub(crate) kind: String,
    pub(crate) model: String,
    pub(crate) prompt_path: String,
    pub(crate) prompt_hash: String,
    pub(crate) budget: AdaptationBudget,
}

impl AdaptationAgent {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.kind.trim().is_empty(),
            "adapted runs must record agent kind"
        );
        anyhow::ensure!(
            !self.model.trim().is_empty(),
            "adapted runs must record agent model"
        );
        anyhow::ensure!(
            !self.prompt_path.trim().is_empty(),
            "adapted runs must record prompt_path"
        );
        normalize_repo_relative_path("adaptation.agent.prompt_path", &self.prompt_path)?;
        anyhow::ensure!(
            !self.prompt_hash.trim().is_empty(),
            "adapted runs must record prompt_hash"
        );
        self.budget.validate()?;
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct AdaptationBudget {
    pub(crate) wall_time_minutes: u32,
    pub(crate) max_iterations: u32,
}

impl AdaptationBudget {
    fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.wall_time_minutes > 0,
            "adapted runs must declare a positive wall-time budget"
        );
        anyhow::ensure!(
            self.max_iterations > 0,
            "adapted runs must declare a positive iteration budget"
        );
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct AdaptationOutputs {
    #[serde(default)]
    pub(crate) rules_or_extensions_changed: Vec<ChangedArtifact>,
    #[serde(default)]
    pub(crate) rule_digests: Vec<String>,
    #[serde(default)]
    pub(crate) extension_digests: Vec<String>,
    #[serde(default)]
    pub(crate) model_digests: Vec<String>,
    pub(crate) notes_path: String,
    pub(crate) final_adapted_report_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) no_change_reason: Option<String>,
    #[serde(default)]
    pub(crate) commands_run: Vec<String>,
}

impl AdaptationOutputs {
    fn validate(&self) -> anyhow::Result<()> {
        normalize_repo_relative_path("adaptation.outputs.notes_path", &self.notes_path)?;
        normalize_repo_relative_path(
            "adaptation.outputs.final_adapted_report_path",
            &self.final_adapted_report_path,
        )?;
        for artifact in &self.rules_or_extensions_changed {
            artifact.validate()?;
        }
        if self.rules_or_extensions_changed.is_empty() {
            let reason = self.no_change_reason.as_deref().unwrap_or_default().trim();
            anyhow::ensure!(
                !reason.is_empty(),
                "adapted runs with no changed files must record no_change_reason"
            );
        }
        let changed_rules = self
            .rules_or_extensions_changed
            .iter()
            .any(|artifact| artifact.kind == ChangedArtifactKind::Rule);
        let changed_extensions = self
            .rules_or_extensions_changed
            .iter()
            .any(|artifact| artifact.kind == ChangedArtifactKind::Extension);
        let changed_models = self
            .rules_or_extensions_changed
            .iter()
            .any(|artifact| artifact.kind == ChangedArtifactKind::Model);
        if changed_rules {
            anyhow::ensure!(
                !self.rule_digests.is_empty(),
                "changed rule artifacts require rule_digests"
            );
        }
        if changed_extensions {
            anyhow::ensure!(
                !self.extension_digests.is_empty(),
                "changed extension artifacts require extension_digests"
            );
        }
        if changed_models {
            anyhow::ensure!(
                !self.model_digests.is_empty(),
                "changed model artifacts require model_digests"
            );
        }
        for digest in self
            .rule_digests
            .iter()
            .chain(self.extension_digests.iter())
            .chain(self.model_digests.iter())
        {
            anyhow::ensure!(
                !digest.trim().is_empty(),
                "adaptation digests must not be empty"
            );
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ChangedArtifact {
    pub(crate) path: String,
    pub(crate) digest: String,
    pub(crate) kind: ChangedArtifactKind,
}

impl ChangedArtifact {
    fn validate(&self) -> anyhow::Result<()> {
        normalize_repo_relative_path("adaptation.outputs.changed_artifact.path", &self.path)?;
        anyhow::ensure!(
            !self.digest.trim().is_empty(),
            "changed artifact digest must not be empty"
        );
        Ok(())
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ChangedArtifactKind {
    Rule,
    Extension,
    Model,
    Fixture,
    Config,
    Notes,
}

pub(crate) fn prompt_hash(prompt_text: &str) -> String {
    stable_hash(&[prompt_text])
}

pub(crate) fn is_forbidden_oracle_path(path: &str) -> bool {
    let normalized = path.replace('\\', "/").to_ascii_lowercase();
    normalized.contains("research/evaluation-harness/repos/") && normalized.contains("/expected")
        || normalized.contains("research/evaluation-harness/suites/")
        || normalized.contains("jelly")
            && (normalized.contains("oracle") || normalized.contains("expected"))
        || normalized.contains("want:")
        || normalized.contains("expected-label")
        || normalized.contains("answer-key")
}

pub(crate) fn sanitize_agent_input_paths(paths: &[String]) -> (Vec<String>, Vec<String>) {
    let mut allowed = Vec::new();
    let mut forbidden = Vec::new();
    for path in paths {
        if is_forbidden_oracle_path(path) {
            forbidden.push(path.clone());
        } else {
            allowed.push(path.clone());
        }
    }
    allowed.sort();
    allowed.dedup();
    forbidden.sort();
    forbidden.dedup();
    (allowed, forbidden)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptation_record_requires_prompt_reference_and_digest() {
        let mut record = valid_record();
        record.agent.prompt_path.clear();
        assert!(record.validate().is_err());

        let mut record = valid_record();
        record.agent.prompt_hash.clear();
        assert!(record.validate().is_err());
    }

    #[test]
    fn adaptation_record_requires_allowed_and_forbidden_inputs() {
        let mut record = valid_record();
        record.inputs_allowed.clear();
        assert!(record.validate().is_err());

        let mut record = valid_record();
        record.inputs_forbidden.clear();
        assert!(record.validate().is_err());
    }

    #[test]
    fn adaptation_record_requires_no_change_reason_when_nothing_changed() {
        let mut record = valid_record();
        record.outputs.rules_or_extensions_changed.clear();
        record.outputs.rule_digests.clear();
        record.outputs.extension_digests.clear();
        record.outputs.no_change_reason = None;
        assert!(record.validate().is_err());

        record.outputs.no_change_reason =
            Some("baseline already modeled the target repository semantics".to_string());
        record.validate().unwrap();
    }

    #[test]
    fn adaptation_record_requires_digests_for_changed_rules_and_extensions() {
        let mut record = valid_record();
        record.outputs.rule_digests.clear();
        assert!(record.validate().is_err());

        let mut record = valid_record();
        record
            .outputs
            .rules_or_extensions_changed
            .push(ChangedArtifact {
                path: ".polint/extensions/src/main.rs".to_string(),
                digest: "def456".to_string(),
                kind: ChangedArtifactKind::Extension,
            });
        record.outputs.extension_digests.clear();
        assert!(record.validate().is_err());
    }

    #[test]
    fn adaptation_record_supports_changed_model_artifacts() {
        let mut record = valid_record();
        record
            .outputs
            .rules_or_extensions_changed
            .push(ChangedArtifact {
                path: ".polint/models/framework.toml".to_string(),
                digest: "model123".to_string(),
                kind: ChangedArtifactKind::Model,
            });
        record.outputs.model_digests.clear();
        assert!(record.validate().is_err());

        record.outputs.model_digests = vec!["model:model123".to_string()];
        record.validate().unwrap();
    }

    #[test]
    fn sandbox_forbidden_paths_are_filtered_from_agent_inputs() {
        let inputs = vec![
            "src/app.ts".to_string(),
            "package.json".to_string(),
            "research/evaluation-harness/repos/case-a/expected.polint-eval.toml".to_string(),
            "research/evaluation-harness/suites/jelly.toml".to_string(),
            "target/generated-expected-labels.json".to_string(),
        ];
        let (allowed, forbidden) = sanitize_agent_input_paths(&inputs);

        assert!(allowed.contains(&"src/app.ts".to_string()));
        assert!(allowed.contains(&"package.json".to_string()));
        assert!(!allowed.iter().any(|path| path.contains("expected")));
        assert_eq!(forbidden.len(), 3);
    }

    #[test]
    fn adaptation_record_requires_positive_budget() {
        let mut record = valid_record();
        record.agent.budget.max_iterations = 0;
        assert!(record.validate().is_err());
    }

    #[test]
    fn adaptation_record_rejects_non_repo_relative_changed_paths() {
        let mut record = valid_record();
        record.outputs.rules_or_extensions_changed[0].path = "../answer-key.rs".to_string();
        assert!(record.validate().is_err());

        let mut record = valid_record();
        record.outputs.notes_path = "/tmp/notes.md".to_string();
        assert!(record.validate().is_err());
    }

    #[test]
    fn prompt_hash_is_stable_for_exact_prompt_text() {
        assert_eq!(prompt_hash("inspect repo"), prompt_hash("inspect repo"));
        assert_ne!(
            prompt_hash("inspect repo"),
            prompt_hash("inspect answer key")
        );
    }

    fn valid_record() -> AdaptationRecord {
        let prompt_text = "adapt polint honestly";
        AdaptationRecord {
            schema_version: ADAPTATION_SCHEMA_VERSION.to_string(),
            suite_id: SuiteId("secbench-js-smoke".to_string()),
            case_selection: "fast".to_string(),
            agent: AdaptationAgent {
                kind: "subagent".to_string(),
                model: "inherit".to_string(),
                prompt_path: "research/evaluation-harness/prompts/default-adaptation-agent.md"
                    .to_string(),
                prompt_hash: prompt_hash(prompt_text),
                budget: AdaptationBudget {
                    wall_time_minutes: 60,
                    max_iterations: 5,
                },
            },
            inputs_allowed: vec![
                "target repository source".to_string(),
                "baseline polint output".to_string(),
            ],
            inputs_forbidden: vec![
                "expected labels before adaptation".to_string(),
                "hardcoded benchmark case ids".to_string(),
            ],
            sandbox_root: Some("target/polint-eval/adaptation-sandbox".to_string()),
            outputs: AdaptationOutputs {
                rules_or_extensions_changed: vec![ChangedArtifact {
                    path: ".polint/rules/src/framework_sources.rs".to_string(),
                    digest: "abc123".to_string(),
                    kind: ChangedArtifactKind::Rule,
                }],
                rule_digests: vec!["rule:abc123".to_string()],
                extension_digests: vec!["extension:def456".to_string()],
                model_digests: Vec::new(),
                notes_path: "target/polint-eval/adaptation-notes.md".to_string(),
                final_adapted_report_path: "target/polint-eval/adapted.json".to_string(),
                no_change_reason: None,
                commands_run: vec!["polint check --format json".to_string()],
            },
        }
    }
}
