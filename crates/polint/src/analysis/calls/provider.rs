use std::collections::BTreeMap;
use std::collections::BTreeSet;

use crate::analysis::calls::cache_key::calls_provider_parameter_digest;
use crate::analysis::calls::direct::resolve_direct_call_targets;
use crate::analysis::calls::extract::extract_call_sites;
use crate::analysis::calls::store::CallOutput;
use crate::analysis::calls::unresolved::derive_unresolved_calls;
use crate::analysis::ids::CallSiteId;
use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, InputComponent, InputSnapshot,
};
use crate::analysis_kernel::{FactFamily, FactRef, ProviderManifest};
use crate::core::{AnalysisDb, FunctionFact, FunctionId, SymbolFact, SymbolId};
use crate::diagnostics::{Diagnostic, TextRange};

#[derive(Debug, Clone, Default)]
pub(crate) struct CallsProviderOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

pub(crate) fn derive_calls_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    semantic_mir_output_digest: Digest,
    cfg_output_digest: Digest,
    symbol_graph_output_digest: Digest,
    module_topology_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> CallsProviderOutput {
    let mut sites = extract_call_sites(db);
    let targets = resolve_direct_call_targets(db, &sites);
    let resolved_sites = targets
        .iter()
        .filter(|target| target.status == crate::analysis::calls::facts::CallTargetStatus::Resolved)
        .map(|target| target.site)
        .collect::<BTreeSet<_>>();
    for site in &mut sites {
        if resolved_sites.contains(&site.id) {
            site.status = crate::analysis::calls::facts::CallTargetStatus::Resolved;
            site.precision = crate::analysis::calls::facts::CallPrecision::SetupAware;
        }
    }
    let unresolved = derive_unresolved_calls(db, &sites)
        .into_iter()
        .filter(|row| !resolved_sites.contains(&row.site))
        .collect();
    let output = CallOutput {
        sites,
        targets,
        unresolved,
    }
    .normalized();
    let output_digest = calls_output_digest(
        db,
        manifest,
        input_snapshot,
        &semantic_mir_output_digest,
        &cfg_output_digest,
        &symbol_graph_output_digest,
        &module_topology_output_digest,
        &upstream_syntax_output_digests,
        &output,
    );
    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_call_facts(output) {
        Ok(()) => CallsProviderOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => CallsProviderOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

fn calls_output_digest(
    db: &AnalysisDb,
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    semantic_mir_output_digest: &Digest,
    cfg_output_digest: &Digest,
    symbol_graph_output_digest: &Digest,
    module_topology_output_digest: &Digest,
    upstream_syntax_output_digests: &[Digest],
    output: &CallOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", calls_provider_parameter_digest()),
        format!("config={}", input_snapshot.config.digest),
        format!("semantic_mir={semantic_mir_output_digest}"),
        format!("cfg={cfg_output_digest}"),
        format!("symbol_graph={symbol_graph_output_digest}"),
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
    extend_component_parts(&mut parts, "model", &input_snapshot.models);
    extend_component_parts(&mut parts, "extension", &input_snapshot.extensions);
    extend_component_parts(&mut parts, "tool", &input_snapshot.tool_invocations);

    parts.extend(
        upstream_syntax_output_digests
            .iter()
            .map(|digest| format!("upstream_syntax={digest}")),
    );

    let site_keys = call_site_key_map(output);
    let function_keys = function_key_map(db);
    let symbol_keys = symbol_key_map(db);
    parts.extend(output.sites.iter().map(|site| {
        format!(
            "call_site={} language={:?} span={} kind={:?} status={:?} precision={:?} callee={}",
            site.stable_key,
            site.language,
            span_part(&site.span),
            site.kind,
            site.status,
            site.precision,
            callee_part(&site.callee),
        )
    }));
    parts.extend(output.targets.iter().map(|target| {
        format!(
            "call_target={} site={} edge={:?} algorithm={:?} status={:?} reason={} provenance={:?} precision={:?} target_function={} target_symbol={}",
            target.stable_key,
            stable_site_key(&site_keys, target.site),
            target.edge_kind,
            target.algorithm,
            target.status,
            target.reason.map(|reason| format!("{reason:?}")).unwrap_or_else(|| "none".to_string()),
            target.provenance,
            target.precision,
            stable_function_key(&function_keys, target.target_function),
            stable_symbol_key(&symbol_keys, target.target_symbol),
        )
    }));
    parts.extend(output.unresolved.iter().map(|unresolved| {
        format!(
            "unresolved_call={} site={} algorithm={:?} status={:?} reason={:?} provenance={:?} precision={:?}",
            unresolved.stable_key,
            stable_site_key(&site_keys, unresolved.site),
            unresolved.algorithm,
            unresolved.status,
            unresolved.reason,
            unresolved.provenance,
            unresolved.precision,
        )
    }));
    if output.sites.is_empty() && output.targets.is_empty() && output.unresolved.is_empty() {
        parts.push("call_output=empty".to_string());
    }

    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "calls_output", &refs)
}

fn extend_component_parts(parts: &mut Vec<String>, prefix: &str, components: &[InputComponent]) {
    parts.extend(components.iter().map(|component| {
        format!(
            "{prefix}:{}:{:?}:{}",
            component.name, component.status, component.digest
        )
    }));
}

fn call_site_key_map(output: &CallOutput) -> BTreeMap<CallSiteId, String> {
    output
        .sites
        .iter()
        .map(|site| (site.id, site.stable_key.clone()))
        .collect()
}

fn function_key_map(db: &AnalysisDb) -> BTreeMap<FunctionId, String> {
    db.functions()
        .iter()
        .map(|function| (function.id, function_key(db, function)))
        .collect()
}

fn function_key(db: &AnalysisDb, function: &FunctionFact) -> String {
    db.metadata_for(FactRef::new(FactFamily::Function, function.id.0))
        .map(|metadata| metadata.stable_key.clone())
        .unwrap_or_else(|| {
            format!(
                "{}:{}:{}",
                db.path_for(function.file),
                function.name,
                span_part(&function.span)
            )
        })
}

fn symbol_key_map(db: &AnalysisDb) -> BTreeMap<SymbolId, String> {
    db.symbols()
        .iter()
        .map(|symbol| (symbol.id, symbol_key(db, symbol)))
        .collect()
}

fn symbol_key(db: &AnalysisDb, symbol: &SymbolFact) -> String {
    db.metadata_for(FactRef::new(FactFamily::Symbol, symbol.id.0))
        .map(|metadata| metadata.stable_key.clone())
        .unwrap_or_else(|| symbol.stable_key.clone())
}

fn stable_site_key(keys: &BTreeMap<CallSiteId, String>, site: CallSiteId) -> String {
    keys.get(&site)
        .cloned()
        .unwrap_or_else(|| "<missing-site>".to_string())
}

fn stable_function_key(
    keys: &BTreeMap<FunctionId, String>,
    function: Option<FunctionId>,
) -> String {
    function
        .map(|function| {
            keys.get(&function)
                .cloned()
                .unwrap_or_else(|| format!("<missing-function:{}>", function.0))
        })
        .unwrap_or_else(|| "none".to_string())
}

fn stable_symbol_key(keys: &BTreeMap<SymbolId, String>, symbol: Option<SymbolId>) -> String {
    symbol
        .map(|symbol| {
            keys.get(&symbol)
                .cloned()
                .unwrap_or_else(|| format!("<missing-symbol:{}>", symbol.0))
        })
        .unwrap_or_else(|| "none".to_string())
}

fn span_part(span: &crate::core::Span) -> String {
    format!(
        "{}:{}..{}:{}@{}..{}",
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col,
        span.start_byte,
        span.end_byte
    )
}

fn callee_part(callee: &crate::analysis::calls::facts::CallCallee) -> String {
    match callee {
        crate::analysis::calls::facts::CallCallee::Identifier { name, .. } => {
            format!("identifier:{name}")
        }
        crate::analysis::calls::facts::CallCallee::Member { property, .. } => {
            format!("member:{property}")
        }
        crate::analysis::calls::facts::CallCallee::Index { .. } => "index".to_string(),
        crate::analysis::calls::facts::CallCallee::Super => "super".to_string(),
        crate::analysis::calls::facts::CallCallee::Import => "import".to_string(),
        crate::analysis::calls::facts::CallCallee::FunctionValue { .. } => {
            "function_value".to_string()
        }
        crate::analysis::calls::facts::CallCallee::Constructor { name, .. } => {
            format!("constructor:{}", name.as_deref().unwrap_or("unknown"))
        }
        crate::analysis::calls::facts::CallCallee::Unknown { reason } => {
            format!("unknown:{reason:?}")
        }
    }
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        format!("Calls provider failed: {message}"),
    )
}

#[cfg(test)]
fn calls_output_digest_for_test(parts: &[&str]) -> Digest {
    Digest::from_parts(DigestKind::ProviderOutput, "calls_output", parts)
}

#[cfg(test)]
mod calls_provider {
    use std::fs;
    use std::path::PathBuf;

    use tempfile::tempdir;

    use crate::analysis::calls::facts::CallAlgorithm;
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{MirOperation, MirOperationKind, MirValue};
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, FileId, FunctionFact, FunctionId, Language, ReferenceId, Span, SymbolFact,
        SymbolId, SymbolKind, SymbolNamespace, SymbolPrecision,
    };

    #[test]
    fn calls_provider_accepts_empty_output_with_deterministic_digest() {
        let first = super::calls_output_digest_for_test(&[]);
        let second = super::calls_output_digest_for_test(&[]);

        assert_eq!(first, second);
        assert!(!first.value.is_empty());
    }

    #[test]
    fn calls_provider_manifest_declares_private_outputs() {
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.calls")
            .expect("calls manifest should exist");

        assert_eq!(manifest.primary_schema_label(), "calls-facts-1:1");
        assert!(manifest.outputs.contains(&"call_sites"));
        assert!(manifest.outputs.contains(&"call_targets"));
        assert!(manifest.outputs.contains(&"unresolved_calls"));
    }

    #[test]
    fn calls_output_digest_uses_stable_payloads_not_dense_ids() {
        let output = call_output(1, "resolved");
        let shifted_dense_ids = call_output(100, "resolved");
        let changed_status = call_output(1, "unresolved");
        let changed_algorithm =
            call_output_with_algorithm(1, "resolved", CallAlgorithm::StaticMember);

        assert_eq!(
            digest_for_output(&output),
            digest_for_output(&shifted_dense_ids)
        );
        assert_ne!(
            digest_for_output(&output),
            digest_for_output(&changed_status)
        );
        assert_ne!(
            digest_for_output(&output),
            digest_for_output(&changed_algorithm)
        );
        assert_ne!(
            digest_for_output(&output),
            digest_for_output_with_target_keys(&output, "alternate_target", "symbol:target")
        );
        assert_ne!(
            digest_for_output(&output),
            digest_for_output_with_target_keys(&output, "target", "symbol:alternate")
        );
    }

    #[test]
    fn calls_provider_populates_site_unresolved_indexes_and_repeats_digest() {
        let mut first_db = mir_call_db();
        let mut second_db = mir_call_db();
        let first = derive_for_test(&mut first_db);
        let second = derive_for_test(&mut second_db);

        assert_eq!(first.output_digest, second.output_digest);
        assert_eq!(first_db.call_sites().len(), 1);
        assert_eq!(first_db.unresolved_calls().len(), 1);
        assert_eq!(first_db.call_sites_by_caller(FunctionId(0)).len(), 1);
        assert!(first_db.call_targets_by_site(CallSiteId(10)).is_empty());
        assert_eq!(
            first_db
                .unresolved_calls_by_reason(
                    crate::analysis::calls::facts::UnresolvedCallReason::FunctionValue
                )
                .len(),
            1
        );
        assert_eq!(
            first_db
                .unresolved_calls_by_status(
                    crate::analysis::calls::facts::CallTargetStatus::Unresolved
                )
                .len(),
            1
        );
    }

    fn digest_for_output(output: &crate::analysis::calls::store::CallOutput) -> Digest {
        digest_for_output_with_target_keys(output, "target", "symbol:target")
    }

    fn digest_for_output_with_target_keys(
        output: &crate::analysis::calls::store::CallOutput,
        target_function_name: &str,
        target_symbol_key: &str,
    ) -> Digest {
        let db = db_for_output(output, target_function_name, target_symbol_key);
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.calls")
            .expect("calls manifest");

        super::calls_output_digest(
            &db,
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "semantic_mir", &["a"]),
            &Digest::from_parts(DigestKind::ProviderOutput, "cfg", &["a"]),
            &Digest::from_parts(DigestKind::ProviderOutput, "symbol_graph", &["a"]),
            &Digest::from_parts(DigestKind::ProviderOutput, "module_topology", &["a"]),
            &[],
            output,
        )
    }

    fn db_for_output(
        output: &crate::analysis::calls::store::CallOutput,
        target_function_name: &str,
        target_symbol_key: &str,
    ) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from("ignored.ts"),
            "ignored.ts".to_string(),
            "export const ignored = true;\n".to_string(),
        );
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function target() {}\n".to_string(),
        );

        let max_function_id = output
            .targets
            .iter()
            .filter_map(|target| target.target_function)
            .map(|function| function.0)
            .max();
        if let Some(max_function_id) = max_function_id {
            for id in 0..=max_function_id {
                let name = output
                    .targets
                    .iter()
                    .any(|target| target.target_function == Some(FunctionId(id)))
                    .then_some(target_function_name)
                    .unwrap_or("placeholder");
                db.push_function(FunctionFact {
                    id: FunctionId(999),
                    file,
                    name: name.to_string(),
                    span: span(file, 20, 30),
                    language: Language::TypeScript,
                    is_test: false,
                    is_exported: true,
                    cyclomatic_complexity: 1,
                    calls: Vec::new(),
                });
            }
        }

        let symbols = output
            .targets
            .iter()
            .filter_map(|target| target.target_symbol)
            .map(|symbol| SymbolFact {
                id: symbol,
                language: Language::TypeScript,
                name: "target".to_string(),
                qualified_name: "target".to_string(),
                kind: SymbolKind::Function,
                namespace: SymbolNamespace::Value,
                file: Some(file),
                package: None,
                module: None,
                owner: None,
                primary_span: Some(span(file, 20, 30)),
                is_exported: true,
                stable_key: target_symbol_key.to_string(),
                precision: SymbolPrecision::ExactSemantic,
            })
            .collect();
        db.replace_symbol_graph_facts(symbols, Vec::new(), Vec::new());
        db
    }

    fn derive_for_test(db: &mut AnalysisDb) -> super::CallsProviderOutput {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.calls")
            .expect("calls manifest");

        super::derive_calls_with_cache_stats(
            db,
            &snapshot,
            manifest,
            Digest::from_parts(DigestKind::ProviderOutput, "semantic_mir", &["a"]),
            Digest::from_parts(DigestKind::ProviderOutput, "cfg", &["a"]),
            Digest::from_parts(DigestKind::ProviderOutput, "symbol_graph", &["a"]),
            Digest::from_parts(DigestKind::ProviderOutput, "module_topology", &["a"]),
            Vec::new(),
        )
    }

    fn mir_call_db() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function app(callback) { callback(); }\n".to_string(),
        );
        let function = db.push_function(FunctionFact {
            id: FunctionId(999),
            file,
            name: "app".to_string(),
            span: span(file, 1, 0),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.replace_semantic_mir(MirOutput {
            bodies: vec![MirBody {
                id: MirBodyId(0),
                language: Language::TypeScript,
                file,
                function,
                package: None,
                module: None,
                owner_stable_key: "function:app".to_string(),
                span: span(file, 1, 0),
                stable_key: "mir-body:app".to_string(),
                status: MirStatus::Partial,
            }],
            places: vec![
                PlaceFact {
                    id: PlaceId(1),
                    language: Language::TypeScript,
                    file: Some(file),
                    function: Some(function),
                    root: PlaceRoot::Local {
                        function,
                        name: "callback".to_string(),
                    },
                    projections: Vec::new(),
                    stable_key: "place:callback".to_string(),
                    status: PlaceStatus::Partial,
                },
                PlaceFact {
                    id: PlaceId(2),
                    language: Language::TypeScript,
                    file: Some(file),
                    function: Some(function),
                    root: PlaceRoot::CallReturn {
                        call: CallSiteId(10),
                    },
                    projections: Vec::new(),
                    stable_key: "place:return".to_string(),
                    status: PlaceStatus::Partial,
                },
            ],
            operations: vec![MirOperation {
                id: MirOpId(0),
                body: MirBodyId(0),
                ordinal: 0,
                span: span(file, 1, 31),
                kind: MirOperationKind::Call {
                    site: CallSiteId(10),
                    callee: MirValue::Place(PlaceId(1)),
                    arguments: Vec::new(),
                    return_place: PlaceId(2),
                },
                stable_key: "mir-op:callback-call".to_string(),
                status: MirStatus::Partial,
            }],
            unsupported: Vec::new(),
        })
        .expect("semantic MIR should store");
        db
    }

    fn call_output(id_offset: u64, status: &str) -> crate::analysis::calls::store::CallOutput {
        call_output_with_algorithm(id_offset, status, CallAlgorithm::DirectReference)
    }

    fn call_output_with_algorithm(
        id_offset: u64,
        status: &str,
        algorithm: CallAlgorithm,
    ) -> crate::analysis::calls::store::CallOutput {
        use crate::analysis::calls::facts::{
            CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact, CallSyntaxKind,
            CallTargetFact, CallTargetStatus, UnresolvedCallFact, UnresolvedCallReason,
        };
        use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId, PlaceId};

        let site = CallSiteFact {
            id: CallSiteId(id_offset),
            language: Language::TypeScript,
            file: FileId(1),
            caller: FunctionId(id_offset),
            owner_symbol: Some(SymbolId(id_offset)),
            body: MirBodyId(id_offset),
            operation: MirOpId(id_offset),
            span: Span::point(FileId(1), 10, 20),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: Some(ReferenceId(id_offset)),
                name: "run".to_string(),
            },
            receiver: None,
            arguments: vec![PlaceId(id_offset)],
            result: Some(PlaceId(id_offset + 1)),
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::Exact,
            stable_key: "call-site:stable".to_string(),
        };
        let target_status = if status == "resolved" {
            CallTargetStatus::Resolved
        } else {
            CallTargetStatus::Unresolved
        };
        let target = CallTargetFact {
            id: CallTargetId(id_offset),
            site: site.id,
            caller: site.caller,
            target_function: Some(FunctionId(id_offset + 10)),
            target_symbol: Some(SymbolId(id_offset + 20)),
            edge_kind: CallEdgeKind::Direct,
            algorithm,
            status: target_status,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::Exact,
            stable_key: "call-target:stable".to_string(),
        };
        let unresolved = UnresolvedCallFact {
            site: site.id,
            caller: site.caller,
            status: CallTargetStatus::Unresolved,
            reason: UnresolvedCallReason::FunctionValue,
            algorithm: CallAlgorithm::SyntaxOnly,
            provenance: CallProvenance::MirShape,
            precision: CallPrecision::Unknown,
            stable_key: "unresolved-call:stable".to_string(),
        };

        crate::analysis::calls::store::CallOutput {
            sites: vec![site],
            targets: vec![target],
            unresolved: vec![unresolved],
        }
        .normalized()
    }

    fn span(file: FileId, line: u32, start_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte: start_byte + 8,
            start_line: line,
            start_col: start_byte + 1,
            end_line: line,
            end_col: start_byte + 9,
        }
    }
}
