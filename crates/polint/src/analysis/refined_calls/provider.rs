use serde::Serialize;
use std::fmt::Debug;

use super::cache_key::{
    refined_calls_provider_parameter_digest, refined_calls_provider_parameter_digest_for_snapshot,
};
use super::facts::{
    RefinedCallConfidence, RefinedCallEdgeFact, RefinedCallTier, RefinedCallValidation,
};
use super::store::{RefinedCallOutput, next_refined_call_id};
use crate::analysis::calls::facts::{CallTargetFact, CallTargetStatus};
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::analysis_kernel::{FactFamily, FactRef, ProviderManifest};
use crate::core::AnalysisDb;
use crate::diagnostics::Diagnostic;

pub(crate) const REFINED_CALLS_PROVIDER_ID: &str = "polint.refined_calls";

#[derive(Debug, Clone, Default)]
pub(crate) struct RefinedCallsProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_refined_calls_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    calls_output_digest: Digest,
    entrypoints_output_digest: Digest,
    direct_summaries_output_digest: Digest,
    type_value_alias_output_digest: Digest,
    extensions_output_digest: Digest,
) -> RefinedCallsProviderOutput {
    debug_assert_eq!(manifest.id, REFINED_CALLS_PROVIDER_ID);
    let mut output = RefinedCallOutput::empty();
    for target in db.call_targets() {
        let id = next_refined_call_id(&output.edges);
        output.edges.push(refined_edge_from_base_target(
            db,
            target,
            id,
            RefinedCallTier::DirectOnly,
        ));
    }
    output
        .edges
        .extend(super::framework::derive_framework_refinements(db).edges);
    output
        .edges
        .extend(super::go::derive_go_refinements(db).edges);
    output
        .edges
        .extend(super::ts_js::derive_ts_js_refinements(db).edges);
    output
        .edges
        .extend(super::summaries::derive_summary_assisted_refinements(db).edges);
    output
        .edges
        .extend(super::extensions::derive_extension_refinements(db).edges);
    output = output.normalized();

    let output_digest = refined_calls_output_digest(
        manifest,
        input_snapshot,
        &calls_output_digest,
        &entrypoints_output_digest,
        &direct_summaries_output_digest,
        &type_value_alias_output_digest,
        &extensions_output_digest,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_refined_call_facts(output) {
        Ok(()) => RefinedCallsProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => RefinedCallsProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

fn refined_edge_from_base_target(
    db: &AnalysisDb,
    target: &CallTargetFact,
    id: crate::analysis::ids::RefinedCallEdgeId,
    tier: RefinedCallTier,
) -> RefinedCallEdgeFact {
    RefinedCallEdgeFact {
        id,
        site: target.site,
        base_target: Some(target.id),
        caller: target.caller,
        target_function: target.target_function,
        target_symbol: target.target_symbol,
        synthetic_target: None,
        language: db
            .call_sites()
            .iter()
            .find(|site| site.id == target.site)
            .map(|site| site.language)
            .unwrap_or(crate::core::Language::Unknown),
        edge_kind: target.edge_kind,
        algorithm: target.algorithm,
        tier,
        status: target.status,
        reason: target.reason,
        provenance: target.provenance,
        precision: target.precision,
        validation: validation_for_target(target),
        confidence: confidence_for_target(target),
        evidence: vec!["base_call_target".to_string()],
        input_stable_keys: vec![
            db.metadata_for(FactRef::new(FactFamily::CallTarget, target.id.0))
                .map(|metadata| metadata.stable_key.clone())
                .unwrap_or_else(|| target.stable_key.clone()),
        ],
        stable_key: stable_refined_call_key(db, target, tier),
    }
}

fn validation_for_target(target: &CallTargetFact) -> RefinedCallValidation {
    match target.status {
        CallTargetStatus::Rejected => RefinedCallValidation::Rejected,
        _ => RefinedCallValidation::Native,
    }
}

fn confidence_for_target(target: &CallTargetFact) -> RefinedCallConfidence {
    match target.status {
        CallTargetStatus::Resolved => RefinedCallConfidence::High,
        CallTargetStatus::Ambiguous => RefinedCallConfidence::Medium,
        CallTargetStatus::Unresolved
        | CallTargetStatus::Unsupported
        | CallTargetStatus::SetupMissing
        | CallTargetStatus::BudgetExceeded
        | CallTargetStatus::Rejected => RefinedCallConfidence::Low,
    }
}

fn stable_refined_call_key(
    db: &AnalysisDb,
    target: &CallTargetFact,
    tier: RefinedCallTier,
) -> String {
    let base_key = db
        .metadata_for(FactRef::new(FactFamily::CallTarget, target.id.0))
        .map(|metadata| metadata.stable_key.clone())
        .unwrap_or_else(|| target.stable_key.clone());
    crate::analysis_kernel::stable_key_from_parts(
        FactFamily::RefinedCallEdge,
        &[
            ("tier", format!("{tier:?}")),
            ("base_target", base_key),
            ("site", target.site.0.to_string()),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn refined_calls_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    calls_output_digest: &Digest,
    entrypoints_output_digest: &Digest,
    direct_summaries_output_digest: &Digest,
    type_value_alias_output_digest: &Digest,
    extensions_output_digest: &Digest,
    output: &RefinedCallOutput,
) -> Digest {
    let upstream = vec![
        calls_output_digest.clone(),
        entrypoints_output_digest.clone(),
        direct_summaries_output_digest.clone(),
        type_value_alias_output_digest.clone(),
        extensions_output_digest.clone(),
    ];
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", refined_calls_provider_parameter_digest()),
        format!(
            "input_parameters={}",
            refined_calls_provider_parameter_digest_for_snapshot(input_snapshot, &upstream)
        ),
        format!("config={}", input_snapshot.config.digest),
        format!("calls={calls_output_digest}"),
        format!("entrypoints={entrypoints_output_digest}"),
        format!("direct_summaries={direct_summaries_output_digest}"),
        format!("type_value_alias={type_value_alias_output_digest}"),
        format!("extensions={extensions_output_digest}"),
    ];
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
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);
    parts.extend(
        output
            .edges
            .iter()
            .map(|edge| format!("refined_call_edge={}", stable_fact_payload(edge))),
    );
    if output.edges.is_empty() {
        parts.push("refined_calls_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "refined_calls_output", &refs)
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

fn stable_fact_payload<T>(fact: &T) -> String
where
    T: Serialize + Debug,
{
    serde_json::to_string(fact).unwrap_or_else(|_| format!("{fact:?}"))
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        crate::diagnostics::TextRange::point(1, 1),
        format!("Refined call provider failed: {message}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallEdgeKind, CallPrecision, CallProvenance, CallTargetFact,
        CallTargetStatus,
    };
    use crate::analysis::ids::{CallSiteId, CallTargetId};
    use crate::core::{FunctionId, SymbolId};

    #[test]
    fn refined_key_is_stable_for_base_target_and_tier() {
        let db = AnalysisDb::new();
        let target = CallTargetFact {
            id: CallTargetId(7),
            site: CallSiteId(3),
            caller: FunctionId(1),
            target_function: Some(FunctionId(2)),
            target_symbol: Some(SymbolId(4)),
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::SetupAware,
            stable_key: "call-target:stable".to_string(),
        };

        assert_eq!(
            stable_refined_call_key(&db, &target, RefinedCallTier::DirectOnly),
            stable_refined_call_key(&db, &target, RefinedCallTier::DirectOnly)
        );
    }
}
