use crate::analysis_api::ProviderManifest;
use crate::analysis_api::{
    CacheStats, Digest, DigestKind, FactFamily, FactRef, InputComponent, InputSnapshot,
    ProviderExecution, ProviderFailureReason, ProviderFailureStage, stable_key_text_from_parts,
};
use crate::analysis_neutral::AnalysisHost;
use crate::analysis_neutral::ids::ReachabilityRootId;
use crate::analysis_neutral::reachability::cache_key::reachability_provider_parameter_digest;
use crate::analysis_neutral::reachability::discover::discover_reachability_roots;
use crate::analysis_neutral::reachability::facts::{
    ReachabilityRootFact, RootKind, RootProvenance, RootStatus,
};
use crate::analysis_neutral::reachability::store::{
    REACHABILITY_PROVIDER_ID, ReachabilityProviderOutput,
};
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

    // Step: extract roots from existing facts + configured input. Only the sentinel
    // rows created for configured names that did not resolve are excluded from the
    // storable set. A dangling relation on any other root is a provider validation
    // failure, not another spelling of an unresolved configured input.
    let roots = discover_reachability_roots(db, configured_roots);
    let (real_roots, unresolved_roots) = partition_configured_unresolved_roots(roots);
    let unresolved_stable_keys: Vec<String> = unresolved_roots
        .iter()
        .map(|root| interner.resolve(root.stable_key).to_string())
        .collect();
    let mut storable = ReachabilityProviderOutput { roots: real_roots }.normalized(interner);

    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();
    let diagnostics: Vec<Diagnostic> = unresolved_roots
        .iter()
        .map(|root| unresolved_configured_root_diagnostic(interner, root))
        .collect();

    // Resolve every dense relation before certifying the output. Missing relations
    // must not be collapsed to placeholder strings because that would produce a
    // cacheable digest for facts the provider cannot validate or store honestly.
    let output_digest = match reachability_output_digest(
        db,
        manifest,
        input_snapshot,
        &calls_output_digest,
        &entrypoints_output_digest,
        &identity_output_digest,
        &symbol_output_digest,
        &module_topology_output_digest,
        &storable,
        &unresolved_stable_keys,
    ) {
        Ok(digest) => digest,
        Err(error) => {
            return provider_validation_failure(diagnostics, cache_stats, error.to_string());
        }
    };

    // Dense IDs are assigned only after the stable digest has been computed.
    for (index, root) in storable.roots.iter_mut().enumerate() {
        root.id = ReachabilityRootId(index as u64);
    }

    match db.replace_reachability_facts(storable) {
        Ok(()) => ReachabilityProviderRunOutput {
            diagnostics,
            cache_stats,
            output_digest: Some(output_digest),
            execution: Default::default(),
        },
        Err(error) => provider_validation_failure(diagnostics, cache_stats, error.to_string()),
    }
}

fn partition_configured_unresolved_roots(
    roots: Vec<ReachabilityRootFact>,
) -> (Vec<ReachabilityRootFact>, Vec<ReachabilityRootFact>) {
    roots.into_iter().partition(|root| {
        !(root.kind == RootKind::ConfiguredEntrypoint
            && root.provenance == RootProvenance::Configured
            && root.status == RootStatus::Unresolved)
    })
}

/// Output digest over stable payloads, never dense IDs (D-19).
///
/// The configured-roots input rides on `input_snapshot.config.digest`, so any
/// change to `[reachability] roots` invalidates the cache. Every upstream
/// provider output digest the reachability provider consumes is also folded in.
#[allow(clippy::too_many_arguments)]
fn reachability_output_digest(
    db: &impl AnalysisHost,
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    calls_output_digest: &Digest,
    entrypoints_output_digest: &Digest,
    identity_output_digest: &Digest,
    symbol_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    output: &ReachabilityProviderOutput,
    unresolved_configured_stable_keys: &[String],
) -> Result<Digest, ReachabilityDigestError> {
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

    for root in &output.roots {
        parts.push(format!("root={}", reachability_root_payload(db, root)?));
    }
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
    Ok(Digest::from_parts(
        DigestKind::ProviderOutput,
        "reachability_output",
        &refs,
    ))
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

#[derive(Debug, thiserror::Error)]
enum ReachabilityDigestError {
    #[error(
        "missing `{field}` relation `{run_id}` while digesting reachability root `{stable_key}`"
    )]
    MissingRelation {
        stable_key: String,
        field: &'static str,
        run_id: u64,
    },
    #[error(
        "reachability root `{stable_key}` has file `{file}` but its span belongs to file `{span_file}`"
    )]
    InconsistentFileRelation {
        stable_key: String,
        file: u32,
        span_file: u32,
    },
    #[error("failed to serialize reachability root `{stable_key}`: {source}")]
    Serialization {
        stable_key: String,
        #[source]
        source: serde_json::Error,
    },
}

/// Stable digest payload for a reachability root. Every referenced dense ID is
/// resolved through its owning fact family before serialization.
fn reachability_root_payload(
    db: &impl AnalysisHost,
    root: &ReachabilityRootFact,
) -> Result<String, ReachabilityDigestError> {
    let interner = db.stable_key_interner();
    let stable_key = interner.resolve(root.stable_key).to_string();
    let target_function = stable_function_text(db, root.target_function)
        .ok_or_else(|| missing_relation(&stable_key, "target_function", root.target_function.0))?;
    let target_symbol = root
        .target_symbol
        .map(|symbol| {
            stable_symbol_text(db, symbol)
                .ok_or_else(|| missing_relation(&stable_key, "target_symbol", symbol.0))
        })
        .transpose()?;
    let originating_entrypoint = root
        .originating_entrypoint
        .map(|entrypoint| {
            stable_entrypoint_text(db, entrypoint).ok_or_else(|| {
                missing_relation(&stable_key, "originating_entrypoint", entrypoint.0)
            })
        })
        .transpose()?;
    let file = stable_file_text(db, root.file)
        .ok_or_else(|| missing_relation(&stable_key, "file", u64::from(root.file.0)))?;
    let span = StableSpanDigest::try_new(db, &root.span, &stable_key)?;
    if root.file != root.span.file {
        return Err(ReachabilityDigestError::InconsistentFileRelation {
            stable_key,
            file: root.file.0,
            span_file: root.span.file.0,
        });
    }

    serde_json::to_string(&ReachabilityRootDigest {
        kind: root.kind,
        language: root.language,
        target_function,
        target_symbol,
        originating_entrypoint,
        file,
        span,
        precision: root.precision,
        provenance: root.provenance,
        status: root.status,
        provider_id: root.provider_id.as_str(),
        stable_key: &stable_key,
    })
    .map_err(|source| ReachabilityDigestError::Serialization { stable_key, source })
}

fn missing_relation(stable_key: &str, field: &'static str, run_id: u64) -> ReachabilityDigestError {
    ReachabilityDigestError::MissingRelation {
        stable_key: stable_key.to_string(),
        field,
        run_id,
    }
}

fn stable_function_text(
    db: &impl AnalysisHost,
    id: crate::internal_core::FunctionId,
) -> Option<String> {
    db.functions()
        .iter()
        .find(|function| function.id == id)
        .map(|function| {
            db.metadata_for(FactRef::new(FactFamily::Function, function.id.0))
                .map(|metadata| db.resolve_stable_key(metadata.stable_key).to_string())
                .unwrap_or_else(|| {
                    stable_key_text_from_parts(
                        &db.stable_key_interner(),
                        FactFamily::Function,
                        &[
                            ("path", db.path_for(function.file)),
                            ("name", function.name.clone()),
                            ("span", stable_span_text(&function.span)),
                        ],
                    )
                })
        })
}

fn stable_span_text(span: &crate::internal_core::Span) -> String {
    format!(
        "{}-{}:{}:{}-{}:{}",
        span.start_byte,
        span.end_byte,
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col
    )
}

fn stable_symbol_text(
    db: &impl AnalysisHost,
    id: crate::internal_core::SymbolId,
) -> Option<String> {
    db.symbols()
        .iter()
        .find(|symbol| symbol.id == id)
        .map(|symbol| db.resolve_stable_key(symbol.stable_key).to_string())
}

fn stable_entrypoint_text(
    db: &impl AnalysisHost,
    id: crate::analysis_neutral::ids::EntrypointId,
) -> Option<String> {
    db.entrypoint_facts()
        .iter()
        .find(|entrypoint| entrypoint.id == id)
        .map(|entrypoint| db.resolve_stable_key(entrypoint.stable_key).to_string())
}

fn stable_file_text(db: &impl AnalysisHost, id: crate::internal_core::FileId) -> Option<String> {
    db.file(id).map(|_| db.path_for(id))
}

#[derive(Serialize)]
struct ReachabilityRootDigest<'a> {
    kind: crate::analysis_neutral::reachability::facts::RootKind,
    language: crate::internal_core::Language,
    target_function: String,
    target_symbol: Option<String>,
    originating_entrypoint: Option<String>,
    file: String,
    span: StableSpanDigest,
    precision: crate::analysis_neutral::reachability::facts::RootPrecision,
    provenance: crate::analysis_neutral::reachability::facts::RootProvenance,
    status: crate::analysis_neutral::reachability::facts::RootStatus,
    provider_id: &'a str,
    stable_key: &'a str,
}

#[derive(Serialize)]
struct StableSpanDigest {
    file: String,
    start_byte: u32,
    end_byte: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
}

impl StableSpanDigest {
    fn try_new(
        db: &impl AnalysisHost,
        span: &crate::internal_core::Span,
        root_stable_key: &str,
    ) -> Result<Self, ReachabilityDigestError> {
        let file = stable_file_text(db, span.file).ok_or_else(|| {
            missing_relation(root_stable_key, "span.file", u64::from(span.file.0))
        })?;
        Ok(Self {
            file,
            start_byte: span.start_byte,
            end_byte: span.end_byte,
            start_line: span.start_line,
            start_col: span.start_col,
            end_line: span.end_line,
            end_col: span.end_col,
        })
    }
}

fn provider_validation_failure(
    mut diagnostics: Vec<Diagnostic>,
    cache_stats: CacheStats,
    message: String,
) -> ReachabilityProviderRunOutput {
    diagnostics.push(provider_error_diagnostic(message));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_api::{
        FunctionFact, GoLifecycleSnapshot, InputComponentStatus, SymbolFact, SymbolKind,
        SymbolNamespace, SymbolPrecision, TsJsLifecycleSnapshot,
    };
    use crate::analysis_neutral::LocalAnalysisDb;
    use crate::analysis_neutral::entrypoints::facts::{
        EntrypointConfidence, EntrypointFact, EntrypointKind, EntrypointPrecision,
        EntrypointProvenance, EntrypointStatus, TriggerMetadata,
    };
    use crate::analysis_neutral::entrypoints::store::EntrypointOutput;
    use crate::analysis_neutral::ids::EntrypointId;
    use crate::analysis_neutral::reachability::facts::{
        RootKind, RootPrecision, RootProvenance, RootStatus,
    };
    use crate::internal_core::{FileId, FunctionId, Language, Span, StableKeyInterner, SymbolId};
    use std::path::PathBuf;

    #[test]
    fn output_digest_is_invariant_under_complete_dense_id_remapping() {
        let (first_db, first_root) = fixture(false);
        let (remapped_db, remapped_root) = fixture(true);

        assert_ne!(first_root.file, remapped_root.file);
        assert_ne!(first_root.target_function, remapped_root.target_function);
        assert_ne!(first_root.target_symbol, remapped_root.target_symbol);
        assert_ne!(
            first_root.originating_entrypoint,
            remapped_root.originating_entrypoint
        );

        let manifest = crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == REACHABILITY_PROVIDER_ID)
            .expect("reachability manifest");
        let snapshot = input_snapshot();
        let upstream = Digest::from_parts(DigestKind::ProviderOutput, "upstream", &["same"]);
        let first_output = ReachabilityProviderOutput {
            roots: vec![first_root],
        };
        let remapped_output = ReachabilityProviderOutput {
            roots: vec![remapped_root],
        };

        let first = reachability_output_digest(
            &first_db,
            manifest,
            &snapshot,
            &upstream,
            &upstream,
            &upstream,
            &upstream,
            &upstream,
            &first_output,
            &[],
        )
        .expect("valid first digest");
        let remapped = reachability_output_digest(
            &remapped_db,
            manifest,
            &snapshot,
            &upstream,
            &upstream,
            &upstream,
            &upstream,
            &upstream,
            &remapped_output,
            &[],
        )
        .expect("valid remapped digest");

        assert_eq!(first, remapped);
    }

    #[test]
    fn dangling_relations_reject_the_digest_and_map_to_provider_validation_failure() {
        let (db, root) = fixture(false);

        let mut dangling_function = root.clone();
        dangling_function.target_function = FunctionId::from_raw(999);
        let error = assert_missing_relation(&db, dangling_function, "target_function");
        let failure =
            provider_validation_failure(Vec::new(), CacheStats::default(), error.to_string());
        assert!(failure.output_digest.is_none());
        assert_eq!(
            failure.execution,
            ProviderExecution::Failed {
                stage: ProviderFailureStage::Validation,
                reason: ProviderFailureReason::ValidationRejected,
            }
        );

        let mut dangling_symbol = root.clone();
        dangling_symbol.target_symbol = Some(SymbolId::from_raw(999));
        assert_missing_relation(&db, dangling_symbol, "target_symbol");

        let mut dangling_entrypoint = root.clone();
        dangling_entrypoint.originating_entrypoint = Some(EntrypointId(999));
        assert_missing_relation(&db, dangling_entrypoint, "originating_entrypoint");

        let mut dangling_file = root.clone();
        dangling_file.file = FileId::from_raw(999);
        assert_missing_relation(&db, dangling_file, "file");

        let mut dangling_span_file = root;
        dangling_span_file.span.file = FileId::from_raw(999);
        assert_missing_relation(&db, dangling_span_file, "span.file");
    }

    #[test]
    fn only_configured_unresolved_roots_are_partitioned_out() {
        let (db, root) = fixture(false);
        let mut configured_unresolved = root.clone();
        configured_unresolved.kind = RootKind::ConfiguredEntrypoint;
        configured_unresolved.provenance = RootProvenance::Configured;
        configured_unresolved.status = RootStatus::Unresolved;
        configured_unresolved.target_function = FunctionId::from_raw(u64::MAX);

        let mut dangling_bridge = root;
        dangling_bridge.target_function = FunctionId::from_raw(u64::MAX);

        let (storable, unresolved) =
            partition_configured_unresolved_roots(vec![configured_unresolved, dangling_bridge]);
        assert_eq!(storable.len(), 1);
        assert_eq!(unresolved.len(), 1);
        assert!(matches!(
            digest_for_root(&db, storable.into_iter().next().expect("storable root")),
            Err(ReachabilityDigestError::MissingRelation {
                field: "target_function",
                ..
            })
        ));
    }

    #[test]
    fn function_fallback_is_length_prefixed_and_delimiter_unambiguous() {
        let left = function_fallback_text("a", "b|typescript|c");
        let right = function_fallback_text("a|typescript|b", "c");

        let legacy_left = legacy_function_text("a", "b|typescript|c");
        let legacy_right = legacy_function_text("a|typescript|b", "c");
        assert_eq!(legacy_left, legacy_right);
        assert_ne!(left, right);
        assert!(left.contains("8:Function"));
        assert!(left.contains("4:path=1:a"));
    }

    fn assert_missing_relation(
        db: &LocalAnalysisDb,
        root: ReachabilityRootFact,
        expected_field: &'static str,
    ) -> ReachabilityDigestError {
        let error = digest_for_root(db, root).expect_err("dangling relation must reject digest");
        assert!(matches!(
            &error,
            ReachabilityDigestError::MissingRelation { field, .. } if *field == expected_field
        ));
        error
    }

    fn digest_for_root(
        db: &LocalAnalysisDb,
        root: ReachabilityRootFact,
    ) -> Result<Digest, ReachabilityDigestError> {
        let manifest = crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == REACHABILITY_PROVIDER_ID)
            .expect("reachability manifest");
        let snapshot = input_snapshot();
        let upstream = Digest::from_parts(DigestKind::ProviderOutput, "upstream", &["same"]);
        reachability_output_digest(
            db,
            manifest,
            &snapshot,
            &upstream,
            &upstream,
            &upstream,
            &upstream,
            &upstream,
            &ReachabilityProviderOutput { roots: vec![root] },
            &[],
        )
    }

    fn legacy_function_text(path: &str, name: &str) -> String {
        format!("function|{path}|typescript|{name}|7..25")
    }

    fn function_fallback_text(path: &str, name: &str) -> String {
        let mut db = LocalAnalysisDb::new();
        let file = db.add_file(
            PathBuf::from(path),
            path.to_string(),
            "export function fixture() {}\n".to_string(),
        );
        let function = db.push_function(function_fact(file, name));
        assert!(
            db.metadata_for(FactRef::new(FactFamily::Function, function.0))
                .is_none(),
            "fixture must exercise the metadata-free fallback"
        );
        stable_function_text(&db, function).expect("function exists")
    }

    fn fixture(remap: bool) -> (LocalAnalysisDb, ReachabilityRootFact) {
        let mut db = LocalAnalysisDb::new();
        let interner = db.stable_key_interner();

        let dummy_file = remap.then(|| {
            db.add_file(
                PathBuf::from("src/dummy.ts"),
                "src/dummy.ts".to_string(),
                "function dummy() {}\n".to_string(),
            )
        });
        let file = db.add_file(
            PathBuf::from("src/handler.ts"),
            "src/handler.ts".to_string(),
            "export function handler() {}\n".to_string(),
        );

        if let Some(dummy_file) = dummy_file {
            db.push_function(function_fact(dummy_file, "dummy"));
        }
        let function = db.push_function(function_fact(file, "handler"));

        let mut symbols = Vec::new();
        if let Some(dummy_file) = dummy_file {
            symbols.push(symbol_fact(
                &interner,
                SymbolId::from_raw(0),
                dummy_file,
                "dummy",
                "symbol:dummy",
            ));
        }
        let symbol = SymbolId::from_raw(symbols.len() as u64);
        symbols.push(symbol_fact(
            &interner,
            symbol,
            file,
            "handler",
            "symbol:handler",
        ));
        db.replace_symbol_graph_facts(symbols, Vec::new(), Vec::new());

        let mut entrypoints = Vec::new();
        if let Some(dummy_file) = dummy_file {
            entrypoints.push(entrypoint_fact(
                &interner,
                EntrypointId(0),
                dummy_file,
                FunctionId::from_raw(0),
                Some(SymbolId::from_raw(0)),
                "entrypoint:000-dummy",
            ));
        }
        entrypoints.push(entrypoint_fact(
            &interner,
            EntrypointId(entrypoints.len() as u64),
            file,
            function,
            Some(symbol),
            "entrypoint:handler",
        ));
        db.replace_entrypoint_facts(EntrypointOutput {
            entrypoints,
            ..EntrypointOutput::default()
        })
        .expect("valid entrypoints");
        let entrypoint = db
            .entrypoint_facts()
            .iter()
            .find(|entrypoint| {
                db.resolve_stable_key(entrypoint.stable_key).as_ref() == "entrypoint:handler"
            })
            .expect("handler entrypoint")
            .id;

        let span = Span::new(file, 7, 25, 1, 8, 1, 26);
        let root = ReachabilityRootFact {
            id: ReachabilityRootId(u64::from(remap)),
            kind: RootKind::FrameworkEntrypoint,
            language: Language::TypeScript,
            target_function: function,
            target_symbol: Some(symbol),
            originating_entrypoint: Some(entrypoint),
            file,
            span,
            precision: RootPrecision::ResolvedStatic,
            provenance: RootProvenance::EntrypointBridge,
            status: RootStatus::Resolved,
            provider_id: REACHABILITY_PROVIDER_ID.to_string(),
            stable_key: interner.intern("reachability-root:handler"),
        };
        (db, root)
    }

    fn function_fact(file: FileId, name: &str) -> FunctionFact {
        FunctionFact::new(
            FunctionId::from_raw(0),
            file,
            name.to_string(),
            Span::new(file, 7, 25, 1, 8, 1, 26),
            Language::TypeScript,
            false,
            true,
            1,
            Vec::new(),
        )
    }

    fn symbol_fact(
        interner: &StableKeyInterner,
        id: SymbolId,
        file: FileId,
        name: &str,
        stable_key: &str,
    ) -> SymbolFact {
        SymbolFact::new(
            id,
            Language::TypeScript,
            name.to_string(),
            name.to_string(),
            SymbolKind::Function,
            SymbolNamespace::Value,
            Some(file),
            None,
            None,
            None,
            Some(Span::new(file, 7, 25, 1, 8, 1, 26)),
            true,
            interner.intern(stable_key),
            SymbolPrecision::ExactLocal,
        )
    }

    fn entrypoint_fact(
        interner: &StableKeyInterner,
        id: EntrypointId,
        file: FileId,
        target_function: FunctionId,
        target_symbol: Option<SymbolId>,
        stable_key: &str,
    ) -> EntrypointFact {
        EntrypointFact {
            id,
            language: Language::TypeScript,
            framework_id: "test-framework".to_string(),
            kind: EntrypointKind::HttpRoute,
            target_function,
            target_symbol,
            registration_span: Span::new(file, 7, 25, 1, 8, 1, 26),
            registration_file: file,
            trigger_metadata: TriggerMetadata::empty(),
            trust_boundary_link: None,
            precision: EntrypointPrecision::ResolvedStatic,
            provenance: EntrypointProvenance::NativeRecognizer,
            confidence: EntrypointConfidence::High,
            status: EntrypointStatus::Resolved,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: interner.intern(stable_key),
        }
    }

    fn input_snapshot() -> InputSnapshot {
        InputSnapshot {
            schema_version: "test".to_string(),
            files: Vec::new(),
            config: InputComponent {
                name: "config".to_string(),
                status: InputComponentStatus::Present,
                digest: Digest::from_parts(DigestKind::Config, "config", &["same"]),
                detail: Vec::new(),
            },
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
}
