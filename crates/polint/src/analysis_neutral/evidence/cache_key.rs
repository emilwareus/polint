use crate::analysis_api::{Digest, DigestKind, InputComponent, InputSnapshot};
use crate::analysis_neutral::evidence::facts::{
    EvidenceQueryBudget, EvidenceRankingMode, EvidenceRendererMode,
};

pub const EVIDENCE_SCHEMA_LABEL: &str = "evidence-facts-1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceProviderParameters {
    pub query_budget: EvidenceQueryBudget,
    pub ranking: EvidenceRankingMode,
    pub renderer: EvidenceRendererMode,
}

impl EvidenceProviderParameters {
    pub fn deterministic_default() -> Self {
        Self {
            query_budget: EvidenceQueryBudget::default(),
            ranking: EvidenceRankingMode::DeterministicDisplay,
            renderer: EvidenceRendererMode::Json,
        }
    }
}

pub fn evidence_provider_parameter_digest() -> Digest {
    evidence_provider_parameter_digest_for_settings(
        &EvidenceProviderParameters::deterministic_default(),
    )
}

pub fn evidence_provider_parameter_digest_for_snapshot(
    input_snapshot: &InputSnapshot,
    upstream_output_digests: &[Digest],
) -> Digest {
    let mut parts = parameter_parts(&EvidenceProviderParameters::deterministic_default());
    parts.push(format!("config={}", input_snapshot.config.digest));
    extend_component_parts(
        &mut parts,
        "go_lifecycle",
        &input_snapshot.go_lifecycle.components,
    );
    extend_component_parts(
        &mut parts,
        "ts_js_lifecycle",
        &input_snapshot.ts_js_lifecycle.components,
    );
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);
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

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    if components.is_empty() {
        parts.push(format!("{prefix}=absent"));
        return;
    }
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_api::{
        GoLifecycleSnapshot, InputComponent, InputComponentStatus, InputSnapshot,
        TsJsLifecycleSnapshot,
    };

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
        InputSnapshot {
            schema_version: "test".to_string(),
            files: Vec::new(),
            config: component("config"),
            go_lifecycle: GoLifecycleSnapshot {
                components: Vec::new(),
            },
            ts_js_lifecycle: TsJsLifecycleSnapshot {
                components: Vec::new(),
            },
            rules: Vec::new(),
            models: Vec::new(),
            extensions: Vec::new(),
            tool_invocations: Vec::new(),
            provider_schemas: Vec::new(),
        }
    }

    fn component(name: &str) -> InputComponent {
        InputComponent {
            name: name.to_string(),
            status: InputComponentStatus::Present,
            digest: Digest::from_parts(DigestKind::Config, name, &[name]),
            detail: Vec::new(),
        }
    }
}
