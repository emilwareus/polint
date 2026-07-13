use crate::analysis::evidence::facts::{
    EvidenceQueryBudget, EvidenceRankingMode, EvidenceRendererMode,
};
use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
use crate::cache::keys::AnalysisSettingsScope;

const REQUESTED_CAPABILITIES: &[&str] = &["dataflow"];

pub(crate) const EVIDENCE_SCHEMA_LABEL: &str = "evidence-facts-1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvidenceProviderParameters {
    pub(crate) query_budget: EvidenceQueryBudget,
    pub(crate) ranking: EvidenceRankingMode,
    pub(crate) renderer: EvidenceRendererMode,
}

impl EvidenceProviderParameters {
    pub(crate) fn deterministic_default() -> Self {
        Self {
            query_budget: EvidenceQueryBudget::default(),
            ranking: EvidenceRankingMode::DeterministicDisplay,
            renderer: EvidenceRendererMode::Json,
        }
    }
}

pub(crate) fn evidence_provider_parameter_digest() -> Digest {
    evidence_provider_parameter_digest_for_settings(
        &EvidenceProviderParameters::deterministic_default(),
    )
}

pub(crate) fn evidence_provider_parameter_digest_for_snapshot(
    input_snapshot: &InputSnapshot,
    upstream_output_digests: &[Digest],
) -> Digest {
    let mut parts = parameter_parts(&EvidenceProviderParameters::deterministic_default());
    parts.push(format!(
        "analysis_settings={}",
        input_snapshot.analysis_settings_digest(AnalysisSettingsScope::Evidence)
    ));
    parts.push(format!(
        "requested_capabilities={}",
        input_snapshot.analysis_requirements_digest_for(REQUESTED_CAPABILITIES)
    ));
    parts.extend(
        upstream_output_digests
            .iter()
            .map(|digest| format!("upstream={digest}")),
    );
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderParameters, "evidence_inputs", &refs)
}

fn evidence_provider_parameter_digest_for_settings(
    settings: &EvidenceProviderParameters,
) -> Digest {
    let parts = parameter_parts(settings);
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderParameters, "evidence_parameters", &refs)
}

fn parameter_parts(settings: &EvidenceProviderParameters) -> Vec<String> {
    let budget = settings.query_budget;
    vec![
        EVIDENCE_SCHEMA_LABEL.to_string(),
        format!("max_paths={}", budget.max_paths),
        format!("max_nodes={}", budget.max_nodes),
        format!("max_edges={}", budget.max_edges),
        format!("max_depth={}", budget.max_depth),
        format!("ranking={:?}", settings.ranking),
        format!("renderer={:?}", settings.renderer),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::keys::config_hash;
    use crate::config::{LoadedConfig, PolintConfig};
    use crate::core::AnalysisDb;

    #[test]
    fn parameters_include_schema_budget_ranking_and_renderer() {
        let digest = evidence_provider_parameter_digest();

        assert_eq!(
            digest,
            evidence_provider_parameter_digest_for_settings(
                &EvidenceProviderParameters::deterministic_default()
            )
        );
    }

    #[test]
    fn digest_changes_when_query_budget_ranking_or_renderer_changes() {
        let base = EvidenceProviderParameters::deterministic_default();
        let mut changed_budget = base.clone();
        changed_budget.query_budget.max_paths += 1;
        let mut changed_ranking = base.clone();
        changed_ranking.ranking = EvidenceRankingMode::StableKey;
        let mut changed_renderer = base.clone();
        changed_renderer.renderer = EvidenceRendererMode::Sarif;

        let base_digest = evidence_provider_parameter_digest_for_settings(&base);
        assert_ne!(
            base_digest,
            evidence_provider_parameter_digest_for_settings(&changed_budget)
        );
        assert_ne!(
            base_digest,
            evidence_provider_parameter_digest_for_settings(&changed_ranking)
        );
        assert_ne!(
            base_digest,
            evidence_provider_parameter_digest_for_settings(&changed_renderer)
        );
    }

    #[test]
    fn input_digest_changes_when_data_flow_output_digest_changes() {
        let snapshot = minimal_snapshot();
        let first_data_flow =
            Digest::from_parts(DigestKind::ProviderOutput, "data_flow", &["first"]);
        let second_data_flow =
            Digest::from_parts(DigestKind::ProviderOutput, "data_flow", &["second"]);

        assert_ne!(
            evidence_provider_parameter_digest_for_snapshot(&snapshot, &[first_data_flow]),
            evidence_provider_parameter_digest_for_snapshot(&snapshot, &[second_data_flow])
        );
    }

    fn minimal_snapshot() -> InputSnapshot {
        let loaded = LoadedConfig {
            root: "test-workspace".into(),
            config: PolintConfig::default(),
            missing: false,
            respect_gitignore: true,
        };
        let plan = AnalysisPlan::from_capability_names_for_test(&["dataflow"]);
        let config_digest = config_hash(&loaded);
        InputSnapshot::from_run_inputs_with_plan(
            &loaded,
            &AnalysisDb::new(),
            &config_digest,
            "rule-digest",
            &plan,
            AnalysisKernel::provider_manifests(),
        )
    }
}
