use serde::Serialize;
use std::fmt::Debug;

use crate::analysis::ids::ReachabilityRootId;
use crate::analysis::reachability::cache_key::reachability_provider_parameter_digest;
use crate::analysis::reachability::discover::discover_reachability_roots;
use crate::analysis::reachability::store::{REACHABILITY_PROVIDER_ID, ReachabilityProviderOutput};
use crate::analysis::reachability::traverse::mark_call_reachability;
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{CacheStats, Digest, DigestKind, InputSnapshot};
use crate::cache::keys::AnalysisSettingsScope;
use crate::core::AnalysisDb;
use crate::diagnostics::{Diagnostic, TextRange};

#[derive(Debug, Clone, Default)]
pub(crate) struct ReachabilityProviderRunOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

/// `polint.reachability` provider entry point.
///
/// Extracts roots, partitions unresolvable configured roots, normalizes stored
/// facts, computes their stable digest, assigns dense in-memory IDs, and replaces
/// the reachability store.
///
/// Configured-unresolvable roots are folded into the digest via a dedicated
/// `unresolved_configured=<stable-keys>` part (so the cache still invalidates when
/// they change) and surfaced as honest diagnostics rather than being serialized
/// as whole `root=...` facts.
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

    // Root discovery projects existing facts together with configured input.
    let roots = discover_reachability_roots(db, configured_roots);
    // Configured roots without a real target cannot enter the referential store.
    // Their stable keys still participate in identity and diagnostics.
    let (real_roots, unresolved_roots): (Vec<_>, Vec<_>) = roots
        .into_iter()
        .partition(|root| db.functions().iter().any(|f| f.id == root.target_function));
    let unresolved_stable_keys: Vec<String> = unresolved_roots
        .iter()
        .map(|root| root.stable_key.clone())
        .collect();
    // Reachability composes resolved call edges without mutating call facts.
    //
    // Marks are stored even while their current readers (`reachable_graph_lookup`,
    // `filter_scored_edges_by_scoring_mode`, `scored_call_graph_edges_for_db`) are
    // test-only. A solver-derived edge set can replace direct calls behind the same
    // contract without changing how marks are represented.
    let marks = mark_call_reachability(db, &real_roots);
    // Normalize the storable set before hashing so the digest certifies the facts
    // that actually land in the database;
    // `ReachabilityRootFact.id` carries `#[serde(skip)]`, so the digest payload
    // never folds in run-local dense IDs.
    let mut storable = ReachabilityProviderOutput {
        roots: real_roots,
        marks,
    }
    .normalized();
    // A dedicated stable-key part invalidates unresolvable configured roots
    // without serializing their non-stored facts into `root=` parts.
    let output_digest = reachability_output_digest(
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
    // Dense IDs are assigned after hashing and stripped from serialization.
    for (index, root) in storable.roots.iter_mut().enumerate() {
        root.id = ReachabilityRootId(index as u64);
    }

    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    // Report configured roots that did not resolve even if storing other facts fails.
    let mut diagnostics: Vec<Diagnostic> = unresolved_roots
        .iter()
        .map(unresolved_configured_root_diagnostic)
        .collect();

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

/// Output digest over stable payloads, never dense IDs.
///
/// The reachability settings scope carries configured roots. Every upstream
/// provider output consumed by reachability is folded in explicitly.
#[allow(clippy::too_many_arguments)]
fn reachability_output_digest(
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
        format!(
            "analysis_settings={}",
            input_snapshot.analysis_settings_digest(AnalysisSettingsScope::Reachability)
        ),
        format!("calls_output={calls_output_digest}"),
        format!("entrypoints_output={entrypoints_output_digest}"),
        format!("identity_output={identity_output_digest}"),
        format!("symbol_graph={symbol_output_digest}"),
        format!("module_topology={module_topology_output_digest}"),
    ];
    parts.extend(
        output
            .roots
            .iter()
            .map(|root| format!("root={}", stable_fact_payload(root))),
    );
    parts.extend(
        output
            .marks
            .iter()
            .map(|mark| format!("mark={}", stable_fact_payload(mark))),
    );
    // Non-stored configured roots contribute only stable keys, avoiding dense IDs
    // and non-stored payloads while still invalidating changed configuration.
    parts.extend(
        unresolved_configured_stable_keys
            .iter()
            .map(|key| format!("unresolved_configured={key}")),
    );
    if output.roots.is_empty()
        && output.marks.is_empty()
        && unresolved_configured_stable_keys.is_empty()
    {
        parts.push("reachability_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "reachability_output", &refs)
}

fn stable_fact_payload<T>(fact: &T) -> String
where
    T: Serialize + Debug,
{
    serde_json::to_string(fact).unwrap_or_else(|_| format!("{fact:?}"))
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    // Propagate the underlying store-failure message into the evidence so the
    // failure is debuggable, mirroring `validate.rs::push_diagnostic`.
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Reachability analysis failed; reachability facts were not stored.",
    )
    .with_evidence("provider", REACHABILITY_PROVIDER_ID)
    .with_evidence("reason", message)
}

/// Diagnostic for a configured root the provider could not resolve to a function.
/// It identifies the exact `[reachability] roots` entry that failed.
fn unresolved_configured_root_diagnostic(
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
    .with_evidence("stable_key", root.stable_key.clone())
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
    use crate::cache::keys::config_hash;
    use crate::config::{LoadedConfig, PolintConfig, RuleConfig, load_config};
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
        let empty_plan = AnalysisPlan::empty();
        assert!(empty_plan.requested_capability_snapshots().is_empty());
        let identity_sources = InputSnapshot::identity_sources_from_plan(&loaded, &empty_plan);
        assert!(identity_sources.requested_capabilities.is_empty());
        assert_eq!(
            identity_sources.analysis_requirements_identity,
            Digest::absent(DigestKind::AnalysisRequirements, "requested_capabilities")
        );

        InputSnapshot::from_run_inputs_with_plan(
            &loaded,
            db,
            "config-a",
            "rules-a",
            &empty_plan,
            AnalysisKernel::provider_manifests(),
        )
    }

    #[test]
    fn output_identity_tracks_roots_and_graph_inputs_only() {
        let temp = tempdir().expect("tempdir");
        let baseline_loaded = loaded_with_rule(temp.path());
        let baseline_snapshot = identity_snapshot(&baseline_loaded);
        let upstream = absent("upstream");
        let output = ReachabilityProviderOutput::empty();
        let digest = |snapshot: &InputSnapshot, upstream: &Digest| {
            reachability_output_digest(
                manifest(),
                snapshot,
                upstream,
                upstream,
                upstream,
                upstream,
                upstream,
                &output,
                &[],
            )
        };
        let baseline_digest = digest(&baseline_snapshot, &upstream);

        let mut rule_settings = baseline_loaded.clone();
        rule_settings.config.rules.config[0]
            .settings
            .insert("threshold".to_string(), toml::Value::Integer(7));
        let rule_snapshot = identity_snapshot(&rule_settings);
        assert_ne!(
            baseline_snapshot.config_identity,
            rule_snapshot.config_identity
        );
        assert_eq!(baseline_digest, digest(&rule_snapshot, &upstream));

        let mut unrelated = baseline_loaded.clone();
        unrelated
            .config
            .languages
            .go
            .insert("offline".to_string(), toml::Value::Boolean(true));
        let unrelated_snapshot = identity_snapshot(&unrelated);
        assert_ne!(
            baseline_snapshot.config_identity,
            unrelated_snapshot.config_identity
        );
        assert_eq!(baseline_digest, digest(&unrelated_snapshot, &upstream));

        let mut relevant = baseline_loaded;
        relevant.config.reachability.roots = vec!["main.main".to_string()];
        let relevant_snapshot = identity_snapshot(&relevant);
        assert_ne!(baseline_digest, digest(&relevant_snapshot, &upstream));

        let changed_upstream =
            Digest::from_parts(DigestKind::ProviderOutput, "upstream", &["changed"]);
        assert_ne!(
            baseline_digest,
            digest(&baseline_snapshot, &changed_upstream)
        );
    }

    fn loaded_with_rule(root: &std::path::Path) -> LoadedConfig {
        let mut config = PolintConfig::default();
        config.rules.config.push(RuleConfig {
            id: "local/provider-identity".to_string(),
            severity: None,
            files: Vec::new(),
            allow_files: Vec::new(),
            allow: Vec::new(),
            max: None,
            deny: Vec::new(),
            forbidden_imports: Default::default(),
            settings: Default::default(),
        });
        LoadedConfig {
            root: root.to_path_buf(),
            config,
            missing: false,
            respect_gitignore: true,
        }
    }

    fn identity_snapshot(loaded: &LoadedConfig) -> InputSnapshot {
        let plan = AnalysisPlan::empty();
        let config_digest = config_hash(loaded);
        InputSnapshot::from_run_inputs_with_plan(
            loaded,
            &AnalysisDb::new(),
            &config_digest,
            "rule-digest",
            &plan,
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
        // An unresolvable configured root must be visible to the operator.
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
        // stored fact set is identical. The key still captures the configured
        // input without serializing whole non-stored facts.
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
