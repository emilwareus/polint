use crate::analysis_api::ProviderManifest;
use crate::analysis_api::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot, ProviderExecution,
    ProviderFailureReason, ProviderFailureStage,
};
use crate::analysis_neutral::AnalysisHost;
use crate::analysis_neutral::ids::ReachabilityRootId;
use crate::analysis_neutral::reachability::cache_key::reachability_provider_parameter_digest;
use crate::analysis_neutral::reachability::discover::discover_reachability_roots;
use crate::analysis_neutral::reachability::facts::ReachabilityRootFact;
use crate::analysis_neutral::reachability::store::{
    REACHABILITY_PROVIDER_ID, ReachabilityProviderOutput,
};
use crate::internal_core::StableKeyInterner;
use crate::internal_core::{Diagnostic, DiagnosticRange};
use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub struct ReachabilityProviderRunOutput {
    pub diagnostics: Vec<Diagnostic>,
    pub cache_stats: CacheStats,
    pub output_digest: Option<Digest>,
    pub execution: ProviderExecution,
}

/// `polint.reachability` provider entry point.
///
/// Pipeline mirroring `polint.identity` and `polint.entrypoints`: extract roots by
/// projecting existing facts (no mutation) -> partition discovered roots into the
/// storable (real-target) set and the configured-unresolvable set -> normalize the
/// STORABLE set -> compute the output digest over EXACTLY the stored stable
/// payloads (D-06/D-19: never dense IDs, never a non-stored superset) -> assign
/// dense IDs as a post-digest read concern -> replace reachability facts.
///
/// Configured-unresolvable roots are folded into the digest via a dedicated
/// `unresolved_configured=<stable-keys>` part (so the cache still invalidates when
/// they change) and surfaced as honest diagnostics (D-13: never a silent drop),
/// rather than being serialized as whole `root=...` facts.
#[allow(clippy::too_many_arguments)]
pub fn derive_reachability_with_cache_stats(
    db: &mut impl AnalysisHost,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    configured_roots: &[String],
    calls_output_digest: Digest,
    entrypoints_output_digest: Digest,
    identity_output_digest: Digest,
    symbol_output_digest: Digest,
    module_topology_output_digest: Digest,
) -> ReachabilityProviderRunOutput {
    debug_assert_eq!(manifest.id, REACHABILITY_PROVIDER_ID);
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;

    // Step: extract roots from existing facts + configured input.
    let roots = discover_reachability_roots(db, configured_roots);
    // Step: partition into the storable (real-target) set and the
    // configured-unresolvable set. Configured-unresolvable roots carry a sentinel
    // target the referential store rejects, so they never reach the validated
    // store; we keep their stable keys to (a) fold into the digest and (b) report
    // as diagnostics so an operator can see their configured root failed to resolve
    // (D-13: never a silent drop).
    let (real_roots, unresolved_roots): (Vec<_>, Vec<_>) = roots
        .into_iter()
        .partition(|root| db.functions().iter().any(|f| f.id == root.target_function));
    let unresolved_stable_keys: Vec<String> = unresolved_roots
        .iter()
        .map(|root| interner.resolve(root.stable_key).to_string())
        .collect();
    // Step: normalize the storable roots. The digest is computed over exactly this
    // set so it certifies what actually lands in the db;
    // `reachability_root_payload` omits dense `id` (matching `#[serde(skip)]`) so
    // digests never fold in run-local dense IDs (D-06/D-19).
    let mut storable = ReachabilityProviderOutput { roots: real_roots }.normalized(interner);
    // Step: digest over the stored stable payloads, plus a dedicated stable-key
    // part for configured-unresolvable roots so the cache invalidates when they
    // change without serializing whole facts (with dense IDs) into the `root=`
    // parts.
    let output_digest = reachability_output_digest(
        interner,
        manifest,
        input_snapshot,
        &calls_output_digest,
        &entrypoints_output_digest,
        &identity_output_digest,
        &symbol_output_digest,
        &module_topology_output_digest,
        &storable,
        &unresolved_stable_keys,
    );
    // Step: assign dense IDs as a post-digest read concern only (never before /
    // independent of the digest). normalized() above fixed the order; the dense IDs
    // simply enumerate that order for any in-memory reader and are stripped from
    // serialization (D-06/D-19).
    for (index, root) in storable.roots.iter_mut().enumerate() {
        root.id = ReachabilityRootId(index as u64);
    }

    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    // Configured-unresolvable roots are reported as honest diagnostics regardless
    // of whether the store succeeds (D-13).
    let mut diagnostics: Vec<Diagnostic> = unresolved_roots
        .iter()
        .map(|root| unresolved_configured_root_diagnostic(interner, root))
        .collect();

    // Step: store the storable set.
    match db.replace_reachability_facts(storable) {
        Ok(()) => ReachabilityProviderRunOutput {
            diagnostics,
            cache_stats,
            output_digest: Some(output_digest),
            execution: Default::default(),
        },
        Err(error) => {
            // A store failure means the db retains its prior state — the facts the
            // digest certifies were NOT persisted. Return `output_digest: None` so a
            // caching layer cannot record a hit for a state that was never stored,
            // and propagate the underlying error message into the diagnostic
            // evidence (mirroring `validate.rs::push_diagnostic`).
            diagnostics.push(provider_error_diagnostic(error.to_string()));
            ReachabilityProviderRunOutput {
                diagnostics,
                cache_stats,
                output_digest: None,
                execution: ProviderExecution::Failed {
                    stage: ProviderFailureStage::Validation,
                    reason: ProviderFailureReason::ValidationRejected,
                },
            }
        }
    }
}

/// Output digest over stable payloads, never dense IDs (D-19).
///
/// The configured-roots input rides on `input_snapshot.config.digest`, so any
/// change to `[reachability] roots` invalidates the cache. Every upstream
/// provider output digest the reachability provider consumes is also folded in.
#[allow(clippy::too_many_arguments)]
fn reachability_output_digest(
    interner: &crate::internal_core::StableKeyInterner,
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    calls_output_digest: &Digest,
    entrypoints_output_digest: &Digest,
    identity_output_digest: &Digest,
    symbol_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    output: &ReachabilityProviderOutput,
    unresolved_configured_stable_keys: &[String],
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", reachability_provider_parameter_digest()),
        format!("config={}", input_snapshot.config.digest),
        format!("calls_output={calls_output_digest}"),
        format!("entrypoints_output={entrypoints_output_digest}"),
        format!("identity_output={identity_output_digest}"),
        format!("symbol_graph={symbol_output_digest}"),
        format!("module_topology={module_topology_output_digest}"),
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

    parts.extend(
        output
            .roots
            .iter()
            .map(|root| format!("root={}", reachability_root_payload(interner, root))),
    );
    // Configured-unresolvable roots are NOT stored and NOT serialized as `root=`
    // facts (that would fold dense IDs / a non-stored superset into the digest,
    // CR-01). Instead each contributes its stable key under a dedicated part so the
    // cache still invalidates when an unresolvable configured root changes (D-13).
    parts.extend(
        unresolved_configured_stable_keys
            .iter()
            .map(|key| format!("unresolved_configured={key}")),
    );
    if output.roots.is_empty() && unresolved_configured_stable_keys.is_empty() {
        parts.push("reachability_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "reachability_output", &refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

/// Stable digest payload for a reachability root: resolved key text, no dense `id`.
fn reachability_root_payload(interner: &StableKeyInterner, root: &ReachabilityRootFact) -> String {
    serde_json::to_string(&ReachabilityRootDigest {
        kind: root.kind,
        language: root.language,
        target_function: root.target_function,
        target_symbol: root.target_symbol,
        originating_entrypoint: root.originating_entrypoint,
        file: root.file,
        span: &root.span,
        precision: root.precision,
        provenance: root.provenance,
        status: root.status,
        provider_id: root.provider_id.as_str(),
        stable_key: interner.resolve(root.stable_key).as_ref(),
    })
    .unwrap_or_else(|_| "{}".to_string())
}

#[derive(Serialize)]
struct ReachabilityRootDigest<'a> {
    kind: crate::analysis_neutral::reachability::facts::RootKind,
    language: crate::internal_core::Language,
    target_function: crate::internal_core::FunctionId,
    target_symbol: Option<crate::internal_core::SymbolId>,
    originating_entrypoint: Option<crate::analysis_neutral::ids::EntrypointId>,
    file: crate::internal_core::FileId,
    span: &'a crate::internal_core::Span,
    precision: crate::analysis_neutral::reachability::facts::RootPrecision,
    provenance: crate::analysis_neutral::reachability::facts::RootProvenance,
    status: crate::analysis_neutral::reachability::facts::RootStatus,
    provider_id: &'a str,
    stable_key: &'a str,
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    // Propagate the underlying store-failure message into the evidence so the
    // failure is debuggable, mirroring `validate.rs::push_diagnostic`'s
    // `.with_evidence(...)` discipline (WR-06).
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        DiagnosticRange::point(1, 1),
        "Reachability analysis failed; reachability facts were not stored.",
    )
    .with_evidence("provider", REACHABILITY_PROVIDER_ID)
    .with_evidence("reason", message)
}

/// Honest `RootStatus::Unresolved` diagnostic for a configured root the provider
/// could not resolve to a real function (D-13: never a silent drop). Surfaced so an
/// operator can see exactly which configured `[reachability] roots` entry failed to
/// resolve, mirroring `validate.rs`'s evidence-bearing diagnostic discipline.
fn unresolved_configured_root_diagnostic(
    interner: &crate::internal_core::StableKeyInterner,
    root: &crate::analysis_neutral::reachability::facts::ReachabilityRootFact,
) -> Diagnostic {
    Diagnostic::warning(
        "polint/internal",
        "<workspace>",
        DiagnosticRange::point(1, 1),
        "Configured reachability root did not resolve to any function.",
    )
    .with_evidence("provider", REACHABILITY_PROVIDER_ID)
    .with_evidence("family", "ReachabilityRoot")
    .with_evidence("stable_key", interner.resolve(root.stable_key).to_string())
    .with_evidence("status", root.status.as_str())
    .with_evidence(
        "reason",
        "configured reachability root resolves to no function",
    )
}

#[cfg(test)]
pub fn reachability_output_digest_for_test(parts: &[&str]) -> Digest {
    Digest::from_parts(DigestKind::ProviderOutput, "reachability_output", parts)
}
