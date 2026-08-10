use crate::analysis::ids::ReachabilityRootId;
use crate::analysis::reachability::cache_key::reachability_provider_parameter_digest;
use crate::analysis::reachability::discover::discover_reachability_roots;
use crate::analysis::reachability::facts::ReachabilityRootFact;
use crate::analysis::reachability::store::{REACHABILITY_PROVIDER_ID, ReachabilityProviderOutput};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::core::{AnalysisDb, StableKeyInterner};
use crate::diagnostics::{Diagnostic, TextRange};
use serde::Serialize;

#[derive(Debug, Clone, Default)]
pub(crate) struct ReachabilityProviderRunOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
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
pub(crate) fn derive_reachability_with_cache_stats(
    db: &mut AnalysisDb,
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
    interner: &crate::core::StableKeyInterner,
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
    kind: crate::analysis::reachability::facts::RootKind,
    language: crate::core::Language,
    target_function: crate::core::FunctionId,
    target_symbol: Option<crate::core::SymbolId>,
    originating_entrypoint: Option<crate::analysis::ids::EntrypointId>,
    file: crate::core::FileId,
    span: &'a crate::core::Span,
    precision: crate::analysis::reachability::facts::RootPrecision,
    provenance: crate::analysis::reachability::facts::RootProvenance,
    status: crate::analysis::reachability::facts::RootStatus,
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
        TextRange::point(1, 1),
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
    interner: &crate::core::StableKeyInterner,
    root: &crate::analysis::reachability::facts::ReachabilityRootFact,
) -> Diagnostic {
    Diagnostic::warning(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
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
pub(crate) fn reachability_output_digest_for_test(parts: &[&str]) -> Digest {
    Digest::from_parts(DigestKind::ProviderOutput, "reachability_output", parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, FunctionFact, FunctionId, Language, PackageFact, PackageId, Span,
    };
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn manifest() -> &'static ProviderManifest {
        AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == REACHABILITY_PROVIDER_ID)
            .expect("reachability manifest")
    }

    fn snapshot(db: &AnalysisDb) -> InputSnapshot {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        InputSnapshot::from_run_inputs(
            &loaded,
            db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        )
    }

    fn absent(kind: &str) -> Digest {
        Digest::absent(DigestKind::ProviderOutput, kind)
    }

    fn span(file: crate::core::FileId, start: u32, end: u32) -> Span {
        Span {
            file,
            start_byte: start,
            end_byte: end,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 1,
        }
    }

    fn empty_db() -> AnalysisDb {
        AnalysisDb::new()
    }

    fn db_with_go_main() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("cmd/app/main.go"),
            "cmd/app/main.go".to_string(),
            "package main\nfunc main() {}\n".to_string(),
        );
        db.push_package(PackageFact {
            id: PackageId(0),
            file,
            name: "main".to_string(),
            span: span(file, 0, 1),
            language: Language::Go,
        });
        db.push_function(FunctionFact {
            id: FunctionId(1),
            file,
            name: "main".to_string(),
            span: span(file, 1, 2),
            language: Language::Go,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db
    }

    fn run(db: &mut AnalysisDb) -> ReachabilityProviderRunOutput {
        let snapshot = snapshot(db);
        derive_reachability_with_cache_stats(
            db,
            &snapshot,
            manifest(),
            &[],
            absent("polint.calls"),
            absent("polint.entrypoints"),
            absent("polint.identity"),
            absent("polint.symbol_graph"),
            absent("polint.module_topology"),
        )
    }

    #[test]
    fn empty_roots_db_returns_empty_output_sentinel_digest() {
        let mut db = empty_db();
        let snapshot = snapshot(&db);
        let output = ReachabilityProviderOutput::empty();
        let digest = reachability_output_digest(
            &db.stable_key_interner(),
            manifest(),
            &snapshot,
            &absent("polint.calls"),
            &absent("polint.entrypoints"),
            &absent("polint.identity"),
            &absent("polint.symbol_graph"),
            &absent("polint.module_topology"),
            &output,
            &[],
        );
        // The empty-output sentinel part must participate, distinguishing the
        // empty digest from any populated one.
        let run_output = run(&mut db);
        assert_eq!(run_output.output_digest, Some(digest));
    }

    #[test]
    fn empty_output_digest_for_test_is_deterministic() {
        let first = super::reachability_output_digest_for_test(&[]);
        let second = super::reachability_output_digest_for_test(&[]);
        assert_eq!(first, second);
    }

    #[test]
    fn permuted_fact_insertion_order_produces_identical_output_digest() {
        // Two dbs with the same Go main facts inserted in different orders must
        // produce a byte-identical output digest (sort-then-assign-dense-IDs).
        let mut first = db_with_go_main();
        let mut second = {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("cmd/app/main.go"),
                "cmd/app/main.go".to_string(),
                "package main\nfunc main() {}\n".to_string(),
            );
            // Push function before package (permuted relative to db_with_go_main).
            db.push_function(FunctionFact {
                id: FunctionId(7),
                file,
                name: "main".to_string(),
                span: span(file, 1, 2),
                language: Language::Go,
                is_test: false,
                is_exported: false,
                cyclomatic_complexity: 1,
                calls: Vec::new(),
            });
            db.push_package(PackageFact {
                id: PackageId(0),
                file,
                name: "main".to_string(),
                span: span(file, 0, 1),
                language: Language::Go,
            });
            db
        };
        let first_digest = run(&mut first).output_digest;
        let second_digest = run(&mut second).output_digest;
        assert_eq!(first_digest, second_digest);
    }

    #[test]
    fn provider_manifests_list_reachability_immediately_after_entrypoints() {
        let manifests = AnalysisKernel::provider_manifests();
        let entrypoints = manifests
            .iter()
            .position(|manifest| manifest.id == "polint.entrypoints")
            .expect("entrypoints present");
        assert_eq!(manifests[entrypoints + 1].id, "polint.reachability");
    }

    #[test]
    fn reachability_manifest_precision_ceiling_is_setup_aware() {
        use crate::analysis_kernel::PrecisionCeiling;
        assert_eq!(manifest().precision_ceiling, PrecisionCeiling::SetupAware);
    }

    fn run_with_configured(
        db: &mut AnalysisDb,
        configured_roots: &[String],
    ) -> ReachabilityProviderRunOutput {
        let snapshot = snapshot(db);
        derive_reachability_with_cache_stats(
            db,
            &snapshot,
            manifest(),
            configured_roots,
            absent("polint.calls"),
            absent("polint.entrypoints"),
            absent("polint.identity"),
            absent("polint.symbol_graph"),
            absent("polint.module_topology"),
        )
    }

    #[test]
    fn unresolvable_configured_root_is_observable_via_diagnostic() {
        // WR-01: an unresolvable configured root must never be a silent drop — it is
        // surfaced as a diagnostic so an operator can see their configured root
        // failed to resolve (D-13).
        let mut db = empty_db();
        let output = run_with_configured(&mut db, &["does/not.Resolve".to_string()]);
        assert_eq!(
            output.diagnostics.len(),
            1,
            "exactly one unresolvable-configured-root diagnostic expected"
        );
        let rendered = format!("{:?}", output.diagnostics[0]);
        assert!(
            rendered.contains("did not resolve"),
            "diagnostic message missing: {rendered}"
        );
        assert!(
            rendered.contains("configured:does/not.Resolve"),
            "diagnostic should carry the unresolved configured root stable key: {rendered}"
        );
        // The store still succeeds (the unresolvable root is kept out of the
        // validated store), so the digest is present.
        assert!(output.output_digest.is_some());
    }

    #[test]
    fn unresolvable_configured_root_changes_the_output_digest() {
        // The configured-unresolvable root is folded into the digest via the
        // dedicated `unresolved_configured=` part, so two runs that differ only in
        // their unresolvable configured roots produce DIFFERENT digests, while the
        // stored fact set (empty) is identical (CR-01: the digest still keys the
        // cache on the configured input without serializing whole facts).
        let mut db_a = empty_db();
        let with_root =
            run_with_configured(&mut db_a, &["does/not.Resolve".to_string()]).output_digest;
        let mut db_b = empty_db();
        let without_root = run_with_configured(&mut db_b, &[]).output_digest;
        assert_ne!(with_root, without_root);
    }

    #[test]
    fn resolvable_configured_root_emits_no_unresolved_diagnostic() {
        let mut db = db_with_go_main();
        let output = run_with_configured(&mut db, &["main.main".to_string()]);
        assert!(
            output.diagnostics.is_empty(),
            "resolvable configured root should not emit an unresolved diagnostic: {:?}",
            output.diagnostics
        );
        assert!(output.output_digest.is_some());
    }
}
