use serde::Serialize;
use std::fmt::Debug;

use crate::analysis::ids::ReachabilityRootId;
use crate::analysis::reachability::cache_key::reachability_provider_parameter_digest;
use crate::analysis::reachability::discover::discover_reachability_roots;
use crate::analysis::reachability::store::{REACHABILITY_PROVIDER_ID, ReachabilityProviderOutput};
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
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
/// Five-phase pipeline mirroring `polint.identity` and `polint.entrypoints`:
/// extract roots by projecting existing facts (no mutation) -> assign dense IDs
/// only AFTER sort+normalize (D-06) -> normalize -> compute the output digest
/// over stable payloads (D-19) -> replace reachability facts. `marks` stays empty
/// in this plan; Plan 02's marking traversal populates it.
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

    // Phase 1: extract roots from existing facts + configured input.
    let mut roots = discover_reachability_roots(db, configured_roots);
    // Phase 2: no dedup needed (discovery does not emit duplicate roots).
    // Phase 3: normalize, then assign dense IDs from the normalized order (D-06).
    let mut output = ReachabilityProviderOutput {
        roots: std::mem::take(&mut roots),
        marks: Vec::new(),
    }
    .normalized();
    for (index, root) in output.roots.iter_mut().enumerate() {
        root.id = ReachabilityRootId(index as u64);
    }
    // Phase 4: digest over stable payloads (never dense IDs).
    let output_digest = reachability_output_digest(
        manifest,
        input_snapshot,
        &calls_output_digest,
        &entrypoints_output_digest,
        &identity_output_digest,
        &symbol_output_digest,
        &module_topology_output_digest,
        &output,
    );

    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    // Phase 5: store. Configured-unresolvable roots carry a sentinel target the
    // referential store rejects, so keep them out of the validated store while
    // still reporting them via discovery/eval (D-13). Resolved roots (every
    // root whose target is a real function) are the ones stored.
    let storable = ReachabilityProviderOutput {
        roots: output
            .roots
            .iter()
            .filter(|root| db.functions().iter().any(|f| f.id == root.target_function))
            .cloned()
            .collect(),
        marks: Vec::new(),
    };

    match db.replace_reachability_facts(storable) {
        Ok(()) => ReachabilityProviderRunOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => ReachabilityProviderRunOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

/// Output digest over stable payloads, never dense IDs (D-19).
///
/// The configured-roots input rides on `input_snapshot.config.digest`, so any
/// change to `[reachability] roots` invalidates the cache. Every upstream
/// provider output digest the reachability provider consumes is also folded in.
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
            .map(|root| format!("root={}", stable_fact_payload(root))),
    );
    parts.extend(
        output
            .marks
            .iter()
            .map(|mark| format!("mark={}", stable_fact_payload(mark))),
    );
    if output.roots.is_empty() && output.marks.is_empty() {
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

fn stable_fact_payload<T>(fact: &T) -> String
where
    T: Serialize + Debug,
{
    serde_json::to_string(fact).unwrap_or_else(|_| format!("{fact:?}"))
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    let _message = message;
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Reachability analysis failed; run internal debug output for details.",
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
            manifest(),
            &snapshot,
            &absent("polint.calls"),
            &absent("polint.entrypoints"),
            &absent("polint.identity"),
            &absent("polint.symbol_graph"),
            &absent("polint.module_topology"),
            &output,
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
}
