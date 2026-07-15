pub(crate) mod go;
pub(crate) mod model;
pub(crate) mod query;
pub(crate) mod semantic;
pub(crate) mod stable_id;
pub(crate) mod ts;

use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{
    CacheNode, CacheStats, DependencyEdge, DependencyKind, Digest, DigestKind, InputComponent,
    InputComponentStatus, InputDependencyKey, InputSnapshot, LayerCacheManifest,
    LayerCacheReadStatus, LayerCacheStore, LayerCacheWriteStatus, LayerKey, LayerKind,
    LayerRunMetadata, PrecisionTier, ProviderOutputDependency, ProviderValidationStatus, ShapeKind,
    relative_manifest_dependency_source, semantic_provider_parameter_digest,
};
use crate::analysis_plan::AnalysisPlan;
use crate::cache::Cache;
use crate::cache::keys::AnalysisSettingsScope;
use crate::config::LoadedConfig;
use crate::core::{
    AnalysisDb, CapabilitySupport, CapabilitySupportStatus, CapabilitySupportView, DefinitionFact,
    FunctionFact, ImportFact, Language, PackageFact, ReferenceFact, SourceFile, Span, SymbolFact,
};
use crate::diagnostics::{Diagnostic, TextRange};
use model::{SYMBOL_GRAPH_LAYER_SCHEMA, SymbolGraphBuilder, SymbolGraphLayerPayload};
use semantic::{
    AliasFact, ExportFact, GeneratedSymbolFact, ResolutionFact, ScopeFact, SemanticImportFact,
    SemanticIndexOutput, StableExportIdentity, alias_reexport_closure,
    emit_native_generated_symbol_hooks,
};

const SYMBOL_GRAPH_CAPABILITIES: &[&str] = &["symbols", "references"];
const SYMBOL_FACTS_DOCS_PATH: &str = "docs/facts/symbols-and-references.md";

#[derive(Debug, Clone, Default)]
pub(crate) struct SymbolGraphDerivation {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) capability_support: Vec<CapabilitySupport>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome,
    pub(crate) output_digest: Option<Digest>,
    pub(crate) layers: Vec<LayerRunMetadata>,
}

impl SymbolGraphDerivation {
    pub(crate) fn support_view(&self, base: &CapabilitySupportView) -> CapabilitySupportView {
        let mut entries = base.entries().to_vec();
        for override_entry in &self.capability_support {
            if let Some(existing) = entries.iter_mut().find(|entry| {
                entry.capability == override_entry.capability
                    && entry.language == override_entry.language
            }) {
                *existing = override_entry.clone();
            } else {
                entries.push(override_entry.clone());
            }
        }
        CapabilitySupportView::new(entries)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LanguageSymbolOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) capability_support: Vec<CapabilitySupport>,
    pub(crate) semantic: SemanticIndexOutput,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Compatibility wrapper remains for direct in-crate symbol derivation callers while the kernel uses the stats-returning cache path."
    )
)]
pub(crate) fn derive_requested_symbols(
    db: &mut AnalysisDb,
    loaded: &LoadedConfig,
    plan: &AnalysisPlan,
) -> SymbolGraphDerivation {
    derive_requested_symbols_uncached(db, loaded, plan)
}

pub(crate) fn symbol_graph_layer_key(
    db: &AnalysisDb,
    manifest: &ProviderManifest,
    analysis_settings_digest: Digest,
    go_lifecycle_digest: Digest,
    ts_js_lifecycle_digest: Digest,
    module_graph_output_digest: Digest,
    upstream_syntax_output_digests: Vec<Digest>,
) -> LayerKey {
    LayerKey::symbol_graph_layer_key(
        manifest,
        symbol_graph_source_function_digests(db),
        symbol_graph_package_context_digests(db),
        symbol_graph_import_shape_digests(db),
        analysis_settings_digest,
        go_lifecycle_digest,
        ts_js_lifecycle_digest,
        module_graph_output_digest,
        upstream_syntax_output_digests,
        symbol_graph_parameter_digest(),
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "Symbol graph cache identity consumes cache, snapshot, provider manifest, module graph output, and upstream syntax outputs explicitly."
)]
pub(crate) fn derive_requested_symbols_with_cache_stats(
    db: &mut AnalysisDb,
    loaded: &LoadedConfig,
    plan: &AnalysisPlan,
    cache: &Cache,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    module_graph_output_digest: Digest,
    upstream_syntax_outputs: Vec<ProviderOutputDependency>,
) -> SymbolGraphDerivation {
    if !plan.requests_any_capability(SYMBOL_GRAPH_CAPABILITIES) {
        return SymbolGraphDerivation::default();
    }

    let analysis_settings_digest = input_snapshot
        .analysis_settings_digest(AnalysisSettingsScope::SymbolGraph)
        .clone();
    let go_lifecycle_digest = lifecycle_component_digest(
        DigestKind::GoLifecycle,
        "symbol_graph_go_lifecycle",
        &input_snapshot.go_lifecycle.components,
    );
    let ts_js_lifecycle_digest = lifecycle_component_digest(
        DigestKind::TsJsLifecycle,
        "symbol_graph_ts_js_lifecycle",
        &input_snapshot.ts_js_lifecycle.components,
    );
    let upstream_syntax_output_digests = upstream_syntax_outputs
        .iter()
        .map(|output| output.output_digest.clone())
        .collect::<Vec<_>>();
    let layer_key = symbol_graph_layer_key(
        db,
        manifest,
        analysis_settings_digest.clone(),
        go_lifecycle_digest,
        ts_js_lifecycle_digest,
        module_graph_output_digest.clone(),
        upstream_syntax_output_digests,
    );
    let store = cache.layer_cache_store();
    let mut cache_stats = CacheStats::default();
    let read = store
        .read_json_validated::<SymbolGraphLayerPayload, _>(&layer_key, |payload, manifest| {
            validate_symbol_graph_layer_payload(payload, manifest)
        });

    match read.status {
        LayerCacheReadStatus::Hit => {
            cache_stats.record_hit();
            cache_stats.record_verified_reuse();
            let payload = read
                .value
                .expect("layer cache hit should include symbol graph payload");
            let layer = LayerRunMetadata::from_manifest(
                read.manifest
                    .expect("layer cache hit should include symbol graph manifest"),
            );
            restore_symbol_graph_layer_payload(db, &payload);
            SymbolGraphDerivation {
                diagnostics: payload.diagnostics,
                capability_support: payload.capability_support,
                cache_stats,
                execution: crate::analysis_kernel::incremental::ProviderExecutionOutcome::Succeeded,
                output_digest: Some(layer.output_digest.clone()),
                layers: vec![layer],
            }
        }
        status @ (LayerCacheReadStatus::BypassedDisabled
        | LayerCacheReadStatus::Miss
        | LayerCacheReadStatus::InvalidEvicted) => {
            match status {
                LayerCacheReadStatus::BypassedDisabled => cache_stats.record_disabled_bypass(),
                LayerCacheReadStatus::Miss => cache_stats.record_miss(),
                LayerCacheReadStatus::InvalidEvicted => cache_stats.record_invalid_evicted_read(),
                LayerCacheReadStatus::Hit => {}
            }
            cache_stats.record_recompute();
            let (mut derivation, payload) =
                derive_requested_symbols_uncached_with_payload(db, loaded, plan);
            let payload = payload.unwrap_or_else(|| symbol_graph_layer_payload(db, &derivation));
            let dependencies = symbol_graph_layer_dependency_edges(
                db,
                &layer_key,
                manifest,
                &module_graph_output_digest,
                &upstream_syntax_outputs,
                analysis_settings_digest,
                &input_snapshot.go_lifecycle.components,
                &input_snapshot.ts_js_lifecycle.components,
                provider_manifest_dependency_digest(input_snapshot, manifest),
            );
            let (manifest, payload_bytes) =
                match symbol_graph_layer_manifest(layer_key, &payload, dependencies) {
                    Ok(result) => result,
                    Err(error) => {
                        derivation
                            .diagnostics
                            .push(cache_write_diagnostic("symbol graph layer", error));
                        derivation.cache_stats = cache_stats;
                        derivation.execution =
                            crate::analysis_kernel::incremental::ProviderExecutionOutcome::Failed;
                        derivation.output_digest = None;
                        return derivation;
                    }
                };
            if status != LayerCacheReadStatus::BypassedDisabled {
                write_symbol_graph_layer_payload(
                    &store,
                    &manifest,
                    payload_bytes,
                    &mut cache_stats,
                    &mut derivation.diagnostics,
                );
            }
            let layer = LayerRunMetadata::from_manifest(manifest);
            derivation.execution =
                crate::analysis_kernel::incremental::ProviderExecutionOutcome::Succeeded;
            derivation.output_digest = Some(layer.output_digest.clone());
            derivation.layers = vec![layer];
            derivation.cache_stats = cache_stats;
            derivation
        }
    }
}

fn derive_requested_symbols_uncached(
    db: &mut AnalysisDb,
    loaded: &LoadedConfig,
    plan: &AnalysisPlan,
) -> SymbolGraphDerivation {
    derive_requested_symbols_uncached_with_payload(db, loaded, plan).0
}

fn derive_requested_symbols_uncached_with_payload(
    db: &mut AnalysisDb,
    loaded: &LoadedConfig,
    plan: &AnalysisPlan,
) -> (SymbolGraphDerivation, Option<SymbolGraphLayerPayload>) {
    if !plan.requests_any_capability(SYMBOL_GRAPH_CAPABILITIES) {
        return (SymbolGraphDerivation::default(), None);
    }

    let mut builder = SymbolGraphBuilder::new();
    let mut derivation = SymbolGraphDerivation::default();
    let mut semantic_output = SemanticIndexOutput::default();

    merge_language_output(
        &mut derivation,
        &mut semantic_output,
        ts::derive_ts_symbols(&mut builder, db, loaded, plan),
    );
    merge_language_output(
        &mut derivation,
        &mut semantic_output,
        go::derive_go_symbols(&mut builder, db, loaded, plan),
    );

    let output = builder.finish();
    let closure = alias_reexport_closure(
        &semantic_output.aliases,
        &semantic_output.exports,
        &semantic_output.stable_exports,
    );
    semantic_output.aliases = closure.aliases;
    semantic_output.resolutions.extend(closure.resolutions);
    let generated_hooks = emit_native_generated_symbol_hooks(&semantic_output);
    semantic_output
        .generated_symbols
        .extend(generated_hooks.generated_symbols);
    semantic_output
        .resolutions
        .extend(generated_hooks.resolutions);
    derivation.diagnostics.extend(generated_hooks.diagnostics);
    db.replace_semantic_index_facts(
        semantic_output.scopes,
        semantic_output.semantic_imports,
        semantic_output.exports,
        semantic_output.aliases,
        semantic_output.resolutions,
        semantic_output.generated_symbols,
        semantic_output.stable_exports,
    );
    db.replace_symbol_graph_facts(output.symbols, output.definitions, output.references);
    derivation.diagnostics.extend(output.diagnostics);
    derivation
        .diagnostics
        .extend(capability_diagnostics(&derivation.capability_support));
    sort_symbol_derivation(&mut derivation);
    let payload = symbol_graph_layer_payload(db, &derivation);
    derivation.execution = crate::analysis_kernel::incremental::ProviderExecutionOutcome::Succeeded;
    derivation.output_digest = Some(symbol_graph_output_digest_for_payload(&payload, None));
    (derivation, Some(payload))
}

#[expect(
    clippy::too_many_arguments,
    reason = "Symbol graph cache identity consumes provider, module graph, syntax, config, and language lifecycle inputs explicitly."
)]
fn symbol_graph_layer_dependency_edges(
    db: &AnalysisDb,
    key: &LayerKey,
    manifest: &ProviderManifest,
    module_graph_output_digest: &Digest,
    upstream_syntax_outputs: &[ProviderOutputDependency],
    analysis_settings_digest: Digest,
    go_lifecycle_components: &[InputComponent],
    ts_js_lifecycle_components: &[InputComponent],
    provider_manifest_digest: Digest,
) -> Vec<DependencyEdge> {
    let from = CacheNode::layer(key.clone());
    let mut edges = Vec::new();

    for file in sorted_files(db) {
        edges.push(dependency_edge(
            &from,
            source_file_dependency(file),
            DependencyKind::SourceText,
            ShapeKind::Content,
        ));
    }

    for function in sorted_functions(db) {
        let file = db
            .file(function.file)
            .expect("function dependency should reference a discovered file");
        edges.push(dependency_edge(
            &from,
            source_file_dependency(file),
            DependencyKind::Input,
            ShapeKind::Syntax,
        ));
    }

    for package in sorted_packages(db) {
        edges.push(dependency_edge(
            &from,
            dependency_input(
                InputDependencyKey::package_project(
                    db.path_for(package.file),
                    Digest::from_parts(
                        DigestKind::Workspace,
                        "package_project",
                        &[
                            &db.path_for(package.file),
                            language_cache_label(package.language),
                            &package.name,
                        ],
                    ),
                    InputComponentStatus::Present,
                ),
                "package/project dependency",
            ),
            DependencyKind::Input,
            ShapeKind::ModuleTopology,
        ));
    }

    for import in sorted_imports(db) {
        let file = db
            .file(import.file)
            .expect("import dependency should reference a discovered file");
        edges.push(dependency_edge(
            &from,
            source_file_dependency(file),
            DependencyKind::ImportShape,
            ShapeKind::Import,
        ));
    }

    edges.push(dependency_edge(
        &from,
        dependency_input(
            InputDependencyKey::analysis_setting(
                format!("{}/analysis-settings", manifest.id),
                analysis_settings_digest,
                InputComponentStatus::Present,
            ),
            "analysis-settings dependency",
        ),
        DependencyKind::Config,
        ShapeKind::Unknown,
    ));
    append_lifecycle_dependencies(&mut edges, &from, go_lifecycle_components);
    append_lifecycle_dependencies(&mut edges, &from, ts_js_lifecycle_components);
    edges.push(dependency_edge(
        &from,
        dependency_input(
            InputDependencyKey::provider_manifest(
                manifest.id,
                provider_manifest_digest.clone(),
                InputComponentStatus::Present,
            ),
            "provider-manifest dependency",
        ),
        DependencyKind::Provider,
        ShapeKind::ProviderVersion,
    ));
    edges.push(dependency_edge(
        &from,
        dependency_input(
            InputDependencyKey::provider_schema(
                format!("{}/{}", manifest.id, manifest.primary_schema_label()),
                provider_manifest_digest,
                InputComponentStatus::Present,
            ),
            "provider-schema dependency",
        ),
        DependencyKind::ProviderSchema,
        ShapeKind::ProviderVersion,
    ));
    edges.push(dependency_edge(
        &from,
        dependency_input(
            InputDependencyKey::tool_invocation(
                format!("{}/toolchain", manifest.id),
                key.toolchain_digest.clone(),
                InputComponentStatus::Absent,
            ),
            "tool invocation dependency",
        ),
        DependencyKind::Toolchain,
        ShapeKind::Toolchain,
    ));
    edges.push(dependency_edge(
        &from,
        upstream_layer_dependency(
            LayerKind::ModuleGraph,
            "polint.module_graph",
            module_graph_output_digest.clone(),
            InputComponentStatus::Present,
        ),
        DependencyKind::UpstreamLayer,
        ShapeKind::Output,
    ));

    for (index, output) in upstream_syntax_outputs.iter().enumerate() {
        let (layer_kind, provider_id) = match index {
            0 => (LayerKind::GoSyntax, "polint.go.syntax"),
            1 => (LayerKind::TsSyntax, "polint.ts.syntax"),
            _ => (LayerKind::Extension, "polint.unknown_upstream"),
        };
        edges.push(dependency_edge(
            &from,
            upstream_layer_dependency(
                layer_kind,
                provider_id,
                output.output_digest.clone(),
                output.status,
            ),
            DependencyKind::UpstreamLayer,
            ShapeKind::Output,
        ));
    }

    edges.sort();
    edges.dedup();
    edges
}

fn symbol_graph_source_function_digests(db: &AnalysisDb) -> Vec<Digest> {
    let mut digests = sorted_files(db)
        .into_iter()
        .map(|file| {
            let parts = [
                normalized_file_path(file),
                file.content_hash.clone(),
                language_cache_label(file.language).to_string(),
            ];
            let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
            Digest::from_parts(DigestKind::SourceText, "source_text", &refs)
        })
        .collect::<Vec<_>>();
    digests.extend(sorted_functions(db).into_iter().map(|function| {
        let mut calls = function.calls.clone();
        calls.sort();
        calls.dedup();
        let parts = [
            db.path_for(function.file),
            function.name.clone(),
            function.span.start_byte.to_string(),
            function.span.end_byte.to_string(),
            function.is_test.to_string(),
            function.is_exported.to_string(),
            function.cyclomatic_complexity.to_string(),
            language_cache_label(function.language).to_string(),
            calls.join("\n"),
        ];
        let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
        Digest::from_parts(DigestKind::ProviderParameters, "function_fact", &refs)
    }));
    digests.sort();
    digests
}

fn symbol_graph_package_context_digests(db: &AnalysisDb) -> Vec<Digest> {
    sorted_packages(db)
        .into_iter()
        .map(|package| {
            let parts = [
                db.path_for(package.file),
                package.name.clone(),
                language_cache_label(package.language).to_string(),
            ];
            let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
            Digest::from_parts(DigestKind::ProviderParameters, "package_context", &refs)
        })
        .collect()
}

fn symbol_graph_import_shape_digests(db: &AnalysisDb) -> Vec<Digest> {
    sorted_imports(db)
        .into_iter()
        .map(|import| {
            let parts = [
                db.path_for(import.file),
                import.path.clone(),
                import.package.clone().unwrap_or_default(),
                import.span.start_byte.to_string(),
                import.span.end_byte.to_string(),
                language_cache_label(import.language).to_string(),
            ];
            let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
            Digest::from_parts(DigestKind::ProviderParameters, "import_shape", &refs)
        })
        .collect()
}

fn symbol_graph_parameter_digest() -> Digest {
    Digest::from_unordered(
        DigestKind::ProviderParameters,
        "symbol_graph_parameters",
        vec![
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "symbol_graph_outputs",
                &["output=symbols", "output=definitions", "output=references"],
            ),
            semantic_provider_parameter_digest(),
        ],
    )
}

fn symbol_graph_layer_payload(
    db: &AnalysisDb,
    derivation: &SymbolGraphDerivation,
) -> SymbolGraphLayerPayload {
    let mut symbols = db.symbols().to_vec();
    let mut definitions = db.definitions().to_vec();
    let mut references = db.references().to_vec();
    sort_symbol_facts(&mut symbols);
    sort_definition_facts(&mut definitions);
    sort_reference_facts(&mut references);

    SymbolGraphLayerPayload {
        schema: SYMBOL_GRAPH_LAYER_SCHEMA.to_string(),
        diagnostics: derivation.diagnostics.clone(),
        capability_support: derivation.capability_support.clone(),
        symbols,
        definitions,
        references,
        semantic_index: semantic_index_payload(db),
    }
}

fn restore_symbol_graph_layer_payload(db: &mut AnalysisDb, payload: &SymbolGraphLayerPayload) {
    let mut symbols = payload.symbols.clone();
    let mut definitions = payload.definitions.clone();
    let mut references = payload.references.clone();
    sort_symbol_facts(&mut symbols);
    sort_definition_facts(&mut definitions);
    sort_reference_facts(&mut references);
    db.replace_symbol_graph_facts(symbols, definitions, references);
    db.replace_semantic_index_facts(
        payload.semantic_index.scopes.clone(),
        payload.semantic_index.semantic_imports.clone(),
        payload.semantic_index.exports.clone(),
        payload.semantic_index.aliases.clone(),
        payload.semantic_index.resolutions.clone(),
        payload.semantic_index.generated_symbols.clone(),
        payload.semantic_index.stable_exports.clone(),
    );
}

fn validate_symbol_graph_layer_payload(
    payload: &SymbolGraphLayerPayload,
    manifest: &LayerCacheManifest,
) -> bool {
    payload.schema == SYMBOL_GRAPH_LAYER_SCHEMA
        && semantic_payload_rows_are_valid(&payload.semantic_index)
        && manifest.output_digest
            == symbol_graph_output_digest_for_payload(payload, Some(&manifest.key))
}

fn semantic_index_payload(db: &AnalysisDb) -> SemanticIndexOutput {
    SemanticIndexOutput {
        scopes: sorted_semantic_rows(db.scopes().to_vec(), scope_payload_sort_key),
        semantic_imports: sorted_semantic_rows(
            db.semantic_imports().to_vec(),
            semantic_import_payload_sort_key,
        ),
        exports: sorted_semantic_rows(db.exports().to_vec(), export_payload_sort_key),
        aliases: sorted_semantic_rows(db.aliases().to_vec(), alias_payload_sort_key),
        resolutions: sorted_semantic_rows(
            db.resolution_facts().to_vec(),
            resolution_payload_sort_key,
        ),
        generated_symbols: sorted_semantic_rows(
            db.generated_symbols().to_vec(),
            generated_symbol_payload_sort_key,
        ),
        stable_exports: sorted_semantic_rows(
            db.stable_exports().to_vec(),
            stable_export_payload_sort_key,
        ),
    }
}

fn semantic_payload_rows_are_valid(semantic: &SemanticIndexOutput) -> bool {
    semantic.scopes.iter().all(scope_payload_row_is_valid)
        && semantic
            .semantic_imports
            .iter()
            .all(semantic_import_payload_row_is_valid)
        && semantic.exports.iter().all(export_payload_row_is_valid)
        && semantic.aliases.iter().all(alias_payload_row_is_valid)
        && semantic
            .resolutions
            .iter()
            .all(resolution_payload_row_is_valid)
        && semantic
            .generated_symbols
            .iter()
            .all(generated_symbol_payload_row_is_valid)
        && semantic
            .stable_exports
            .iter()
            .all(stable_export_payload_row_is_valid)
        && stable_export_identities_are_consistent(&semantic.stable_exports)
}

fn scope_payload_row_is_valid(row: &ScopeFact) -> bool {
    !row.stable_key.is_empty()
        && row
            .scope_path
            .iter()
            .all(|part| !is_absolute_path_like(part))
}

fn semantic_import_payload_row_is_valid(row: &SemanticImportFact) -> bool {
    !row.stable_key.is_empty() && !is_absolute_path_like(&row.import_path)
}

fn export_payload_row_is_valid(row: &ExportFact) -> bool {
    !row.stable_key.is_empty()
}

fn alias_payload_row_is_valid(row: &AliasFact) -> bool {
    !row.stable_key.is_empty()
}

fn resolution_payload_row_is_valid(row: &ResolutionFact) -> bool {
    !row.stable_key.is_empty()
}

fn generated_symbol_payload_row_is_valid(row: &GeneratedSymbolFact) -> bool {
    !row.stable_key.is_empty()
}

fn stable_export_payload_row_is_valid(row: &StableExportIdentity) -> bool {
    !row.stable_key.is_empty()
        && row
            .package_key
            .as_deref()
            .is_none_or(|package_key| !is_absolute_path_like(package_key))
        && row
            .module_key
            .as_deref()
            .is_none_or(|module_key| !is_absolute_path_like(module_key))
}

fn stable_export_identities_are_consistent(rows: &[StableExportIdentity]) -> bool {
    let mut targets_by_stable_key = std::collections::BTreeMap::<&str, &str>::new();
    for row in rows {
        if let Some(existing) = targets_by_stable_key.get(row.stable_key.as_str()) {
            if *existing != row.symbol_stable_key {
                return false;
            }
        } else {
            targets_by_stable_key.insert(row.stable_key.as_str(), row.symbol_stable_key.as_str());
        }
    }
    true
}

fn is_absolute_path_like(value: &str) -> bool {
    std::path::Path::new(value).is_absolute()
        || value.starts_with('/')
        || value.starts_with("\\\\")
        || is_windows_drive_absolute(value)
}

fn is_windows_drive_absolute(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

fn write_symbol_graph_layer_payload(
    store: &LayerCacheStore,
    manifest: &LayerCacheManifest,
    payload_bytes: Vec<u8>,
    stats: &mut CacheStats,
    diagnostics: &mut Vec<Diagnostic>,
) -> bool {
    match store.write_json_bytes(manifest, payload_bytes) {
        Ok(LayerCacheWriteStatus::Written) => {
            stats.record_write();
            true
        }
        Ok(LayerCacheWriteStatus::BypassedDisabled) => {
            stats.record_disabled_bypass();
            true
        }
        Err(error) => {
            diagnostics.push(cache_write_diagnostic("symbol graph layer", error));
            false
        }
    }
}

fn symbol_graph_layer_manifest(
    layer_key: LayerKey,
    payload: &SymbolGraphLayerPayload,
    dependencies: Vec<DependencyEdge>,
) -> anyhow::Result<(LayerCacheManifest, Vec<u8>)> {
    let payload_bytes = serde_json::to_vec(payload)?;
    let payload_digest = LayerCacheStore::payload_digest_for_json_bytes(&payload_bytes)?;
    let output_digest = symbol_graph_output_digest(&layer_key, &payload_digest);
    let manifest = LayerCacheManifest::new(
        layer_key,
        output_digest,
        payload_digest,
        dependencies,
        PrecisionTier::SetupAware,
        ProviderValidationStatus::NativeTrusted,
        Vec::new(),
    );
    Ok((manifest, payload_bytes))
}

fn symbol_graph_output_digest_for_payload(
    payload: &SymbolGraphLayerPayload,
    layer_key: Option<&LayerKey>,
) -> Digest {
    let payload_digest = LayerCacheStore::payload_digest_for_json(payload)
        .unwrap_or_else(|_| Digest::unsupported(DigestKind::LayerOutput, "symbol_graph", "json"));
    if let Some(layer_key) = layer_key {
        symbol_graph_output_digest(layer_key, &payload_digest)
    } else {
        Digest::from_parts(
            DigestKind::ProviderOutput,
            "symbol_graph_layer_output",
            &[&payload_digest.to_string()],
        )
    }
}

fn symbol_graph_output_digest(layer_key: &LayerKey, payload_digest: &Digest) -> Digest {
    let layer_key_json =
        serde_json::to_string(layer_key).unwrap_or_else(|_| "unserializable_layer_key".to_string());
    Digest::from_parts(
        DigestKind::ProviderOutput,
        "symbol_graph_layer_output",
        &[&payload_digest.to_string(), &layer_key_json],
    )
}

fn lifecycle_component_digest(
    kind: DigestKind,
    label: &str,
    components: &[InputComponent],
) -> Digest {
    Digest::from_unordered(
        kind,
        label,
        components
            .iter()
            .map(|component| component.digest.clone())
            .collect(),
    )
}

fn cache_write_diagnostic(path: &str, error: anyhow::Error) -> Diagnostic {
    Diagnostic::warning(
        "internal/cache",
        path,
        TextRange::point(1, 1),
        format!("cache write failed: {error}"),
    )
}

fn sorted_files(db: &AnalysisDb) -> Vec<&SourceFile> {
    let mut files = db.files().iter().collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files
}

fn sorted_functions(db: &AnalysisDb) -> Vec<&FunctionFact> {
    let mut functions = db.functions().iter().collect::<Vec<_>>();
    functions.sort_by(|left, right| {
        (
            db.path_for(left.file),
            left.span.start_byte,
            left.span.end_byte,
            left.name.as_str(),
            left.id,
        )
            .cmp(&(
                db.path_for(right.file),
                right.span.start_byte,
                right.span.end_byte,
                right.name.as_str(),
                right.id,
            ))
    });
    functions
}

fn sorted_packages(db: &AnalysisDb) -> Vec<&PackageFact> {
    let mut packages = db.packages().iter().collect::<Vec<_>>();
    packages.sort_by(|left, right| {
        (
            db.path_for(left.file),
            left.language,
            left.name.as_str(),
            left.id,
        )
            .cmp(&(
                db.path_for(right.file),
                right.language,
                right.name.as_str(),
                right.id,
            ))
    });
    packages
}

fn sorted_imports(db: &AnalysisDb) -> Vec<&ImportFact> {
    let mut imports = db.imports().iter().collect::<Vec<_>>();
    imports.sort_by(|left, right| {
        (
            db.path_for(left.file),
            left.span.start_byte,
            left.path.as_str(),
            left.id,
        )
            .cmp(&(
                db.path_for(right.file),
                right.span.start_byte,
                right.path.as_str(),
                right.id,
            ))
    });
    imports
}

fn normalized_file_path(file: &SourceFile) -> String {
    crate::module_graph::paths::normalize_repo_relative(&file.relative_path)
        .unwrap_or_else(|| file.relative_path.clone())
}

fn source_file_dependency(file: &SourceFile) -> CacheNode {
    dependency_input(
        InputDependencyKey::source_file(
            normalized_file_path(file),
            Digest {
                kind: DigestKind::SourceText,
                value: file.content_hash.clone(),
            },
            InputComponentStatus::Present,
        ),
        "source-file dependency",
    )
}

fn dependency_input(
    input: Result<
        InputDependencyKey,
        crate::analysis_kernel::incremental::InputDependencyDigestKindError,
    >,
    context: &str,
) -> CacheNode {
    CacheNode::DependencyInput(
        input.unwrap_or_else(|error| panic!("invalid {context} digest purpose: {error}")),
    )
}

fn append_lifecycle_dependencies(
    edges: &mut Vec<DependencyEdge>,
    from: &CacheNode,
    components: &[InputComponent],
) {
    edges.extend(components.iter().map(|component| {
        let (input, kind, shape, context) = if component.digest.kind == DigestKind::ToolInvocation {
            (
                InputDependencyKey::tool_invocation(
                    component.name.clone(),
                    component.digest.clone(),
                    component.status,
                ),
                DependencyKind::ToolInvocation,
                ShapeKind::Toolchain,
                "tool-invocation lifecycle dependency",
            )
        } else {
            (
                InputDependencyKey::language_lifecycle(
                    component.name.clone(),
                    component.digest.clone(),
                    component.status,
                ),
                DependencyKind::Lifecycle,
                ShapeKind::Lifecycle,
                "language-lifecycle dependency",
            )
        };
        dependency_edge(from, dependency_input(input, context), kind, shape)
    }));
}

fn provider_manifest_dependency_digest(
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
) -> Digest {
    input_snapshot
        .provider_schemas
        .iter()
        .find(|provider| provider.provider_id == manifest.id)
        .unwrap_or_else(|| {
            panic!(
                "input snapshot is missing provider manifest identity for `{}`",
                manifest.id
            )
        })
        .provider_manifest_digest
        .clone()
}

fn dependency_edge(
    _from: &CacheNode,
    to: CacheNode,
    kind: DependencyKind,
    required_shape: ShapeKind,
) -> DependencyEdge {
    DependencyEdge {
        from: relative_manifest_dependency_source(),
        to,
        kind,
        required_shape,
    }
}

fn upstream_layer_dependency(
    layer_kind: LayerKind,
    provider_id: &str,
    output_digest: Digest,
    status: InputComponentStatus,
) -> CacheNode {
    dependency_input(
        InputDependencyKey::upstream_layer(
            format!("{provider_id}/{}", layer_kind.label()),
            crate::analysis_kernel::incremental::dependency_layer_digest(output_digest),
            status,
        ),
        "upstream-layer dependency",
    )
}

fn language_cache_label(language: Language) -> &'static str {
    match language {
        Language::Go => "go",
        Language::TypeScript => "typescript",
        Language::Tsx => "tsx",
        Language::JavaScript => "javascript",
        Language::Jsx => "jsx",
        Language::Unknown => "unknown",
    }
}

fn sort_symbol_facts(symbols: &mut [SymbolFact]) {
    symbols.sort_by(|left, right| {
        fact_order_key(
            &left.stable_key,
            left.file,
            left.primary_span.as_ref(),
            &left.name,
        )
        .cmp(&fact_order_key(
            &right.stable_key,
            right.file,
            right.primary_span.as_ref(),
            &right.name,
        ))
    });
}

fn sort_definition_facts(definitions: &mut [DefinitionFact]) {
    definitions.sort_by(|left, right| {
        fact_order_key(
            &left.stable_key,
            left.file,
            left.primary_span.as_ref(),
            &left.name,
        )
        .cmp(&fact_order_key(
            &right.stable_key,
            right.file,
            right.primary_span.as_ref(),
            &right.name,
        ))
    });
}

fn sort_reference_facts(references: &mut [ReferenceFact]) {
    references.sort_by(|left, right| {
        fact_order_key(
            &left.stable_key,
            left.file,
            left.primary_span.as_ref(),
            &left.name,
        )
        .cmp(&fact_order_key(
            &right.stable_key,
            right.file,
            right.primary_span.as_ref(),
            &right.name,
        ))
    });
}

fn sorted_semantic_rows<T, K: Ord>(mut rows: Vec<T>, key: impl Fn(&T) -> K) -> Vec<T> {
    rows.sort_by_key(key);
    rows
}

fn scope_payload_sort_key(row: &ScopeFact) -> (String, Option<crate::core::FileId>) {
    (row.stable_key.clone(), row.file)
}

fn semantic_import_payload_sort_key(
    row: &SemanticImportFact,
) -> (String, Option<crate::core::FileId>, String) {
    (row.stable_key.clone(), row.file, row.import_path.clone())
}

fn export_payload_sort_key(row: &ExportFact) -> (String, Option<crate::core::FileId>, String) {
    (row.stable_key.clone(), row.file, row.export_name.clone())
}

fn alias_payload_sort_key(row: &AliasFact) -> (String, Option<crate::core::FileId>) {
    (row.stable_key.clone(), row.file)
}

fn resolution_payload_sort_key(row: &ResolutionFact) -> (String, Option<crate::core::FileId>) {
    (row.stable_key.clone(), row.file)
}

fn generated_symbol_payload_sort_key(
    row: &GeneratedSymbolFact,
) -> (String, Option<crate::core::FileId>) {
    (row.stable_key.clone(), row.file)
}

fn stable_export_payload_sort_key(row: &StableExportIdentity) -> (String, String) {
    (row.stable_key.clone(), row.symbol_stable_key.clone())
}

fn fact_order_key<'a>(
    stable_key: &'a str,
    file: Option<crate::core::FileId>,
    span: Option<&Span>,
    name: &'a str,
) -> (&'a str, Option<crate::core::FileId>, u32, u32, &'a str) {
    let (start_byte, end_byte) = span
        .map(|span| (span.start_byte, span.end_byte))
        .unwrap_or((u32::MAX, u32::MAX));
    (stable_key, file, start_byte, end_byte, name)
}

fn merge_language_output(
    derivation: &mut SymbolGraphDerivation,
    semantic: &mut SemanticIndexOutput,
    output: LanguageSymbolOutput,
) {
    derivation.diagnostics.extend(output.diagnostics);
    derivation
        .capability_support
        .extend(output.capability_support);
    semantic.extend(output.semantic);
}

fn capability_diagnostics(support: &[CapabilitySupport]) -> Vec<Diagnostic> {
    support
        .iter()
        .filter(|entry| entry.status != CapabilitySupportStatus::Supported)
        .flat_map(|entry| {
            entry
                .rules
                .iter()
                .map(|rule_id| capability_diagnostic(entry, rule_id))
        })
        .collect()
}

fn capability_diagnostic(entry: &CapabilitySupport, rule_id: &str) -> Diagnostic {
    let language = entry.language.map(language_name).unwrap_or("workspace");
    let status = capability_status_json(&entry.status);
    let docs_path = entry.docs_path.as_deref().unwrap_or(SYMBOL_FACTS_DOCS_PATH);
    let reason = entry
        .reason
        .as_deref()
        .unwrap_or("Symbol/reference provider support is unavailable.");
    let mut diagnostic = Diagnostic::error(
        "polint/capability",
        "<workspace>",
        TextRange::point(1, 1),
        format!(
            "Rule `{rule_id}` requested capability `{}` for {language}, but symbol graph provider support is {status}.",
            entry.capability
        ),
    )
    .with_evidence("rule", rule_id.to_string())
    .with_evidence("capability", entry.capability.clone())
    .with_evidence("language", language.to_string())
    .with_evidence("status", status.to_string())
    .with_evidence("reason", reason.to_string())
    .with_evidence("docs_path", docs_path.to_string())
    .with_help(format!(
        "Capability `{}` is recognized but the {language} symbol/reference provider is not available yet; see {docs_path}.",
        entry.capability
    ));
    if let Some(hint) = &entry.hint {
        diagnostic = diagnostic.with_evidence("hint", hint.clone());
    }
    diagnostic
}

fn sort_symbol_derivation(derivation: &mut SymbolGraphDerivation) {
    derivation.capability_support.sort_by(|left, right| {
        (
            left.capability.as_str(),
            left.language,
            left.rules.as_slice(),
            left.reason.as_deref(),
            left.hint.as_deref(),
            left.docs_path.as_deref(),
        )
            .cmp(&(
                right.capability.as_str(),
                right.language,
                right.rules.as_slice(),
                right.reason.as_deref(),
                right.hint.as_deref(),
                right.docs_path.as_deref(),
            ))
    });
    derivation.diagnostics.sort_by(|left, right| {
        (
            left.rule_id.as_str(),
            left.file.as_str(),
            left.range.start_line,
            left.range.start_col,
            left.message.as_str(),
            left.stable_fingerprint.as_str(),
        )
            .cmp(&(
                right.rule_id.as_str(),
                right.file.as_str(),
                right.range.start_line,
                right.range.start_col,
                right.message.as_str(),
                right.stable_fingerprint.as_str(),
            ))
    });
}

fn language_name(language: Language) -> &'static str {
    match language {
        Language::Go => "Go",
        Language::TypeScript => "TypeScript",
        Language::Tsx => "TSX",
        Language::JavaScript => "JavaScript",
        Language::Jsx => "JSX",
        Language::Unknown => "unknown",
    }
}

fn capability_status_json(status: &CapabilitySupportStatus) -> &'static str {
    match status {
        CapabilitySupportStatus::Supported => "supported",
        CapabilitySupportStatus::Unsupported => "unsupported",
        CapabilitySupportStatus::SetupMissing => "setup_missing",
    }
}

#[cfg(test)]
mod symbol_graph_derivation {
    use super::{SymbolGraphDerivation, derive_requested_symbols};
    use crate::analysis_kernel::incremental::{
        CacheNode, DependencyKind, Digest, DigestKind, InputComponentStatus, InputDependencyKind,
        InputSnapshot, ProviderOutputDependency,
    };
    use crate::analysis_kernel::{
        FactConfidence, FactFamily, FactPrecision, FactRef, ValidationStatus,
    };
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, CapabilitySupport, CapabilitySupportStatus, CapabilitySupportView,
        DefinitionFact, DefinitionId, DefinitionKind, FileId, ImportFact, ImportId, Language,
        ModuleNode, ModuleNodeId, ModuleNodeKind, ReferenceFact, ReferenceId, ReferenceKind,
        ResolutionPrecision, ResolutionStatus, ResolvedImportFact, ResolvedImportId, Span,
        SymbolFact, SymbolId, SymbolKind, SymbolNamespace, SymbolPrecision, SymbolResolutionStatus,
        span_from_byte_range,
    };
    use std::fs;
    use std::path::Path;

    type SymbolRows = Vec<(Language, String, SymbolPrecision)>;
    type ReferenceRows = Vec<(Language, String, SymbolResolutionStatus, SymbolPrecision)>;
    type SupportRows = Vec<(String, Option<Language>, CapabilitySupportStatus)>;
    type DeriveSnapshot = (SymbolRows, ReferenceRows, SupportRows, Vec<String>);

    fn loaded_config_for(root: &Path) -> crate::config::LoadedConfig {
        load_config(root).expect("default config loads")
    }

    fn add_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) -> FileId {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().expect("test file has parent")).expect("mkdirs");
        std::fs::write(&path, source).expect("write fixture file");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn stale_symbol_fact(file: FileId) -> SymbolFact {
        SymbolFact {
            id: SymbolId(999),
            language: Language::TypeScript,
            name: "stale".to_string(),
            qualified_name: "stale".to_string(),
            kind: SymbolKind::Unknown,
            namespace: SymbolNamespace::Unknown,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            primary_span: Some(Span::point(file, 1, 1)),
            is_exported: false,
            stable_key: "stale:symbol".to_string(),
            precision: SymbolPrecision::Unsupported,
        }
    }

    fn stale_definition_fact(file: FileId) -> DefinitionFact {
        DefinitionFact {
            id: DefinitionId(999),
            symbol: SymbolId(999),
            language: Language::TypeScript,
            name: "stale".to_string(),
            qualified_name: "stale".to_string(),
            kind: DefinitionKind::Unknown,
            namespace: SymbolNamespace::Unknown,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            primary_span: Some(Span::point(file, 1, 1)),
            is_primary: false,
            is_exported: false,
            stable_key: "stale:definition".to_string(),
            precision: SymbolPrecision::Unsupported,
        }
    }

    fn stale_reference_fact(file: FileId) -> ReferenceFact {
        ReferenceFact {
            id: ReferenceId(999),
            language: Language::TypeScript,
            name: "stale".to_string(),
            qualified_name: "stale".to_string(),
            kind: ReferenceKind::Unknown,
            namespace: SymbolNamespace::Unknown,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            primary_span: Some(Span::point(file, 1, 1)),
            target: None,
            candidates: Vec::new(),
            stable_key: "stale:reference".to_string(),
            status: SymbolResolutionStatus::Unsupported,
            precision: SymbolPrecision::Unsupported,
        }
    }

    fn symbol_graph_manifest() -> &'static crate::analysis_kernel::ProviderManifest {
        crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.symbol_graph")
            .expect("symbol graph provider manifest exists")
    }

    fn requested_symbol_plan() -> AnalysisPlan {
        AnalysisPlan::from_capability_names_for_test(&["symbols", "references"])
    }

    fn upstream_digests(label: &str) -> (Digest, Vec<Digest>) {
        (
            Digest::from_parts(DigestKind::ProviderOutput, "module_graph", &[label]),
            vec![
                Digest::from_parts(DigestKind::ProviderOutput, "go_syntax", &[label]),
                Digest::from_parts(DigestKind::ProviderOutput, "ts_syntax", &[label]),
            ],
        )
    }

    fn symbol_input_snapshot(
        loaded: &crate::config::LoadedConfig,
        db: &AnalysisDb,
        plan: &AnalysisPlan,
        config_digest: &str,
    ) -> InputSnapshot {
        let identity_sources = InputSnapshot::identity_sources_from_plan(loaded, plan);
        let requested_capabilities = plan.requested_capability_snapshots();
        assert!(!requested_capabilities.is_empty());
        assert_eq!(
            identity_sources.requested_capabilities,
            requested_capabilities
        );
        assert_eq!(
            identity_sources.analysis_requirements_identity,
            plan.analysis_requirements_digest()
        );

        InputSnapshot::from_run_inputs_with_plan(
            loaded,
            db,
            config_digest,
            "rule-digest",
            plan,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        )
    }

    fn derive_symbols_with_cache(
        db: &mut AnalysisDb,
        loaded: &crate::config::LoadedConfig,
        cache: &Cache,
        plan: &AnalysisPlan,
        config_digest: &str,
        upstream_label: &str,
    ) -> SymbolGraphDerivation {
        let snapshot = symbol_input_snapshot(loaded, db, plan, config_digest);
        let (module_graph_output_digest, syntax_output_digests) = upstream_digests(upstream_label);
        let syntax_outputs = syntax_output_digests
            .into_iter()
            .map(ProviderOutputDependency::present)
            .collect();
        super::derive_requested_symbols_with_cache_stats(
            db,
            loaded,
            plan,
            cache,
            &snapshot,
            symbol_graph_manifest(),
            module_graph_output_digest,
            syntax_outputs,
        )
    }

    fn collect_files(root: &Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        collect_files_into(root, &mut files);
        files.sort();
        files
    }

    fn collect_files_into(root: &Path, files: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries {
            let path = entry.expect("read cache entry").path();
            if path.is_dir() {
                collect_files_into(&path, files);
            } else {
                files.push(path);
            }
        }
    }

    fn first_layer_file(cache_root: &Path, category: &str) -> std::path::PathBuf {
        collect_files(&cache_root.join("layers").join(category))
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("expected layer cache {category} file"))
    }

    fn add_ts_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) -> FileId {
        add_file(db, root, relative_path, source)
    }

    fn span_for(file: FileId, source: &str, needle: &str) -> Span {
        let start = source
            .find(needle)
            .unwrap_or_else(|| panic!("missing fixture text {needle:?}"));
        span_from_byte_range(file, source, start, start + needle.len())
    }

    fn push_import(db: &mut AnalysisDb, file: FileId, source: &str, path: &str) -> ImportId {
        db.push_import(ImportFact {
            id: ImportId(0),
            file,
            package: None,
            path: path.to_string(),
            span: span_for(file, source, &format!("{path:?}")),
            language: Language::TypeScript,
        })
    }

    fn fixture_db(root: &Path, import_path: &str) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let app_source = format!(
            r#"
import {{ token as importedToken }} from "{import_path}";

export function answer() {{
    return importedToken;
}}
"#
        );
        let target_source = "export const token = 42;\n";
        let app = add_ts_file(&mut db, root, "src/app.ts", &app_source);
        let target = add_ts_file(&mut db, root, "src/target.ts", target_source);
        let import = push_import(&mut db, app, &app_source, import_path);
        db.replace_module_graph_facts(
            vec![ResolvedImportFact {
                id: ResolvedImportId(0),
                import,
                from_file: app,
                target_node: Some(ModuleNodeId(1)),
                status: ResolutionStatus::Resolved,
                precision: ResolutionPrecision::ExactFile,
                reason: None,
            }],
            vec![
                ModuleNode {
                    id: ModuleNodeId(0),
                    kind: ModuleNodeKind::File,
                    label: "src/app.ts".to_string(),
                    file: Some(app),
                    package: None,
                    language: Some(Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(1),
                    kind: ModuleNodeKind::File,
                    label: "src/target.ts".to_string(),
                    file: Some(target),
                    package: None,
                    language: Some(Language::TypeScript),
                },
            ],
            Vec::new(),
        );
        db
    }

    #[test]
    fn symbol_graph_metadata_records_provider_and_reuses_existing_stable_keys() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            Path::new("src/app.ts").to_path_buf(),
            "src/app.ts".to_string(),
            "export const value = 1;\n".to_string(),
        );
        let symbol = SymbolFact {
            id: SymbolId(7),
            language: Language::TypeScript,
            name: "value".to_string(),
            qualified_name: "value".to_string(),
            kind: SymbolKind::Variable,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            primary_span: Some(Span::point(file, 1, 14)),
            is_exported: true,
            stable_key: "symbol:key:value".to_string(),
            precision: SymbolPrecision::ExactSemantic,
        };
        let definition = DefinitionFact {
            id: DefinitionId(11),
            symbol: symbol.id,
            language: Language::TypeScript,
            name: "value".to_string(),
            qualified_name: "value".to_string(),
            kind: DefinitionKind::Declaration,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            primary_span: Some(Span::point(file, 1, 14)),
            is_primary: true,
            is_exported: true,
            stable_key: "definition:key:value".to_string(),
            precision: SymbolPrecision::ExactSemantic,
        };
        let reference = ReferenceFact {
            id: ReferenceId(13),
            language: Language::TypeScript,
            name: "value".to_string(),
            qualified_name: "value".to_string(),
            kind: ReferenceKind::Read,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            primary_span: Some(Span::point(file, 1, 14)),
            target: Some(symbol.id),
            candidates: Vec::new(),
            stable_key: "reference:key:value".to_string(),
            status: SymbolResolutionStatus::Resolved,
            precision: SymbolPrecision::ExactSemantic,
        };

        db.replace_symbol_graph_facts(vec![symbol], vec![definition], vec![reference]);

        let symbol_meta = db
            .metadata_for(FactRef::new(FactFamily::Symbol, 7))
            .expect("symbol metadata should be recorded");
        let definition_meta = db
            .metadata_for(FactRef::new(FactFamily::Definition, 11))
            .expect("definition metadata should be recorded");
        let reference_meta = db
            .metadata_for(FactRef::new(FactFamily::Reference, 13))
            .expect("reference metadata should be recorded");

        assert_eq!(symbol_meta.producer_id, "polint.symbol_graph");
        assert_eq!(symbol_meta.layer_id, "polint.symbol_graph");
        assert_eq!(symbol_meta.precision, FactPrecision::SetupAware);
        assert_eq!(symbol_meta.confidence, FactConfidence::High);
        assert_eq!(symbol_meta.validation, ValidationStatus::NativeTrusted);
        assert_eq!(symbol_meta.stable_key, "symbol:key:value");
        assert_eq!(definition_meta.stable_key, "definition:key:value");
        assert_eq!(reference_meta.stable_key, "reference:key:value");
    }

    #[test]
    fn symbol_graph_metadata_maps_low_confidence_precisions_without_mutating_status_fields() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            Path::new("src/app.ts").to_path_buf(),
            "src/app.ts".to_string(),
            "missing;\n".to_string(),
        );
        let reference = ReferenceFact {
            id: ReferenceId(23),
            language: Language::TypeScript,
            name: "missing".to_string(),
            qualified_name: "missing".to_string(),
            kind: ReferenceKind::Read,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            primary_span: Some(Span::point(file, 1, 1)),
            target: None,
            candidates: Vec::new(),
            stable_key: "reference:key:missing".to_string(),
            status: SymbolResolutionStatus::SetupMissing,
            precision: SymbolPrecision::SetupMissing,
        };

        db.replace_symbol_graph_facts(Vec::new(), Vec::new(), vec![reference]);

        let metadata = db
            .metadata_for(FactRef::new(FactFamily::Reference, 23))
            .expect("reference metadata should be recorded");

        assert_eq!(metadata.precision, FactPrecision::SetupMissing);
        assert_eq!(metadata.confidence, FactConfidence::Low);
        assert_eq!(
            db.references()[0].status,
            SymbolResolutionStatus::SetupMissing
        );
        assert_eq!(db.references()[0].precision, SymbolPrecision::SetupMissing);
    }

    #[test]
    fn provider_defaults_when_symbol_capabilities_are_not_requested() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        add_file(
            &mut db,
            temp.path(),
            "src/app.ts",
            "export const value = 1;\n",
        );

        let derivation = derive_requested_symbols(
            &mut db,
            &loaded_config_for(temp.path()),
            &AnalysisPlan::empty(),
        );

        assert!(derivation.diagnostics.is_empty());
        assert!(derivation.capability_support.is_empty());
        assert!(db.symbols().is_empty());
        assert!(db.definitions().is_empty());
        assert!(db.references().is_empty());
    }

    #[test]
    fn support_view_merges_symbol_provider_rows_in_order() {
        let derivation = SymbolGraphDerivation {
            diagnostics: Vec::new(),
            capability_support: vec![
                CapabilitySupport {
                    capability: "symbols".to_string(),
                    language: Some(Language::TypeScript),
                    status: CapabilitySupportStatus::Unsupported,
                    rules: vec!["local/symbols".to_string()],
                    reason: Some("symbol extraction pending".to_string()),
                    hint: None,
                    docs_path: Some("docs/facts/symbols-and-references.md".to_string()),
                },
                CapabilitySupport {
                    capability: "references".to_string(),
                    language: Some(Language::Go),
                    status: CapabilitySupportStatus::Unsupported,
                    rules: vec!["local/references".to_string()],
                    reason: Some("reference extraction pending".to_string()),
                    hint: None,
                    docs_path: Some("docs/facts/symbols-and-references.md".to_string()),
                },
            ],
            ..SymbolGraphDerivation::default()
        };
        let base = CapabilitySupportView::new(vec![CapabilitySupport {
            capability: "symbols".to_string(),
            language: Some(Language::TypeScript),
            status: CapabilitySupportStatus::Supported,
            rules: vec!["local/symbols".to_string()],
            reason: None,
            hint: None,
            docs_path: None,
        }]);

        let support = derivation.support_view(&base);

        assert_eq!(
            support
                .entries()
                .iter()
                .map(|entry| (
                    entry.capability.as_str(),
                    entry.language,
                    entry.status.clone(),
                ))
                .collect::<Vec<_>>(),
            vec![
                (
                    "symbols",
                    Some(Language::TypeScript),
                    CapabilitySupportStatus::Unsupported
                ),
                (
                    "references",
                    Some(Language::Go),
                    CapabilitySupportStatus::Unsupported
                ),
            ]
        );
    }

    #[test]
    fn requested_symbol_derivation_replaces_facts_deterministically() {
        fn derive_once() -> DeriveSnapshot {
            let temp = tempfile::tempdir().expect("tempdir");
            let mut db = AnalysisDb::new();
            let app = add_file(
                &mut db,
                temp.path(),
                "src/app.ts",
                "export const value = 1;\n",
            );
            add_file(&mut db, temp.path(), "lib/main.go", "package lib\n");
            db.replace_symbol_graph_facts(
                vec![stale_symbol_fact(app)],
                vec![stale_definition_fact(app)],
                vec![stale_reference_fact(app)],
            );

            let derivation = derive_requested_symbols(
                &mut db,
                &loaded_config_for(temp.path()),
                &AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]),
            );

            (
                db.symbols()
                    .iter()
                    .map(|symbol| (symbol.language, symbol.name.clone(), symbol.precision))
                    .collect(),
                db.references()
                    .iter()
                    .map(|reference| {
                        (
                            reference.language,
                            reference.name.clone(),
                            reference.status,
                            reference.precision,
                        )
                    })
                    .collect(),
                derivation
                    .capability_support
                    .iter()
                    .map(|entry| {
                        (
                            entry.capability.clone(),
                            entry.language,
                            entry.status.clone(),
                        )
                    })
                    .collect(),
                derivation
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.stable_fingerprint.clone())
                    .collect(),
            )
        }

        let first = derive_once();
        let second = derive_once();

        assert_eq!(first, second);
        assert!(first.0.iter().any(|(_, name, _)| name == "value"));
        assert!(first.0.iter().all(|(_, name, _)| name != "stale"));
        assert!(first.1.iter().all(|(_, name, _, _)| name != "stale"));
    }

    #[test]
    fn symbol_graph_dependency_edges_preserve_typed_input_identity_and_status() {
        let temp = tempfile::tempdir().expect("tempdir");
        let loaded = loaded_config_for(temp.path());
        let plan = requested_symbol_plan();
        let db = fixture_db(temp.path(), "./target");
        let snapshot = symbol_input_snapshot(&loaded, &db, &plan, "config");
        let analysis_settings = snapshot
            .analysis_settings_digest(crate::cache::keys::AnalysisSettingsScope::SymbolGraph)
            .clone();
        let go_lifecycle = super::lifecycle_component_digest(
            DigestKind::GoLifecycle,
            "symbol_graph_go_lifecycle",
            &snapshot.go_lifecycle.components,
        );
        let ts_js_lifecycle = super::lifecycle_component_digest(
            DigestKind::TsJsLifecycle,
            "symbol_graph_ts_js_lifecycle",
            &snapshot.ts_js_lifecycle.components,
        );
        let (module_graph_output, syntax_output_digests) = upstream_digests("typed");
        let syntax_dependencies = vec![
            ProviderOutputDependency::present(syntax_output_digests[0].clone()),
            ProviderOutputDependency::from_execution(
                "polint.ts.syntax",
                crate::analysis_kernel::incremental::ProviderExecutionOutcome::Skipped,
                None,
            ),
        ];
        let syntax_outputs = syntax_dependencies
            .iter()
            .map(|output| output.output_digest.clone())
            .collect::<Vec<_>>();
        let key = super::symbol_graph_layer_key(
            &db,
            symbol_graph_manifest(),
            analysis_settings.clone(),
            go_lifecycle,
            ts_js_lifecycle,
            module_graph_output.clone(),
            syntax_outputs,
        );
        let edges = super::symbol_graph_layer_dependency_edges(
            &db,
            &key,
            symbol_graph_manifest(),
            &module_graph_output,
            &syntax_dependencies,
            analysis_settings,
            &snapshot.go_lifecycle.components,
            &snapshot.ts_js_lifecycle.components,
            super::provider_manifest_dependency_digest(&snapshot, symbol_graph_manifest()),
        );

        let typed_inputs = edges
            .iter()
            .filter_map(|edge| match &edge.to {
                CacheNode::DependencyInput(input) => Some(input),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(typed_inputs.iter().any(|input| {
            input.kind == InputDependencyKind::SourceFile
                && input.stable_key == "src/app.ts"
                && input.digest.kind == DigestKind::SourceText
                && input.status == InputComponentStatus::Present
        }));
        assert!(typed_inputs.iter().any(|input| {
            input.kind == InputDependencyKind::AnalysisSetting
                && input.digest.kind == DigestKind::AnalysisSettings
                && input.status == InputComponentStatus::Present
        }));
        assert!(typed_inputs.iter().any(|input| {
            input.kind == InputDependencyKind::ProviderManifest
                && input.digest.kind == DigestKind::ProviderManifest
        }));
        assert!(typed_inputs.iter().any(|input| {
            input.kind == InputDependencyKind::ProviderSchema
                && input.digest.kind == DigestKind::ProviderManifest
        }));
        assert!(typed_inputs.iter().any(|input| {
            input.kind == InputDependencyKind::UpstreamLayer
                && input.stable_key.starts_with("polint.go.syntax/")
                && input.digest.kind == DigestKind::DependencyLayer
                && input.status == InputComponentStatus::Present
        }));
        assert!(typed_inputs.iter().any(|input| {
            input.kind == InputDependencyKind::UpstreamLayer
                && input.stable_key.starts_with("polint.ts.syntax/")
                && input.digest.kind == DigestKind::DependencyLayer
                && input.status == InputComponentStatus::Absent
        }));
        for component in snapshot
            .go_lifecycle
            .components
            .iter()
            .chain(&snapshot.ts_js_lifecycle.components)
        {
            assert!(typed_inputs.iter().any(|input| {
                input.stable_key == component.name
                    && input.digest == component.digest
                    && input.status == component.status
            }));
        }
        assert!(!edges.iter().any(|edge| matches!(
            edge.kind,
            DependencyKind::Rule | DependencyKind::Extension | DependencyKind::Model
        )));
    }

    mod symbol_graph_layer_cache {
        use super::*;

        #[test]
        fn symbol_graph_layer_reuses_warm_cache() {
            let temp = tempfile::tempdir().expect("tempdir");
            let loaded = loaded_config_for(temp.path());
            let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
            let plan = requested_symbol_plan();
            let mut first_db = fixture_db(temp.path(), "./target");
            let mut second_db = fixture_db(temp.path(), "./target");

            let first = derive_symbols_with_cache(
                &mut first_db,
                &loaded,
                &cache,
                &plan,
                "rule-only-full-config-changed",
                "stable",
            );
            let second = derive_symbols_with_cache(
                &mut second_db,
                &loaded,
                &cache,
                &plan,
                "config",
                "stable",
            );

            assert_eq!(first.cache_stats.misses, 1);
            assert_eq!(first.cache_stats.recomputes, 1);
            assert_eq!(first.cache_stats.writes, 1);
            assert_eq!(second.cache_stats.hits, 1);
            assert_eq!(second.cache_stats.verified_reuse, 1);
            assert_eq!(second.cache_stats.recomputes, 0);
            assert_eq!(first.output_digest, second.output_digest);
            assert_eq!(symbol_rows(&first_db), symbol_rows(&second_db));
            assert_eq!(reference_rows(&first_db), reference_rows(&second_db));
        }

        #[test]
        fn symbol_graph_layer_invalidates_on_import_or_module_digest_change() {
            let temp = tempfile::tempdir().expect("tempdir");
            let loaded = loaded_config_for(temp.path());
            let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
            let plan = requested_symbol_plan();
            let mut base_db = fixture_db(temp.path(), "./target");
            derive_symbols_with_cache(&mut base_db, &loaded, &cache, &plan, "config", "stable");

            let mut import_changed_db = fixture_db(temp.path(), "./other");
            let import_changed = derive_symbols_with_cache(
                &mut import_changed_db,
                &loaded,
                &cache,
                &plan,
                "config",
                "stable",
            );
            let mut module_changed_db = fixture_db(temp.path(), "./target");
            let module_changed = derive_symbols_with_cache(
                &mut module_changed_db,
                &loaded,
                &cache,
                &plan,
                "config",
                "changed",
            );

            assert_eq!(import_changed.cache_stats.misses, 1);
            assert_eq!(import_changed.cache_stats.recomputes, 1);
            assert_eq!(module_changed.cache_stats.misses, 1);
            assert_eq!(module_changed.cache_stats.recomputes, 1);
        }

        #[test]
        fn symbol_graph_layer_corrupt_cache_recomputes() {
            let temp = tempfile::tempdir().expect("tempdir");
            let loaded = loaded_config_for(temp.path());
            let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
            let plan = requested_symbol_plan();
            let mut first_db = fixture_db(temp.path(), "./target");
            derive_symbols_with_cache(&mut first_db, &loaded, &cache, &plan, "config", "stable");
            let manifest = first_layer_file(temp.path().join("cache").as_path(), "manifests");
            fs::write(manifest, "{broken").expect("corrupt symbol graph manifest");
            let mut second_db = fixture_db(temp.path(), "./target");

            let second = derive_symbols_with_cache(
                &mut second_db,
                &loaded,
                &cache,
                &plan,
                "config",
                "stable",
            );

            assert_eq!(second.cache_stats.invalid_evicted_reads, 1);
            assert_eq!(second.cache_stats.recomputes, 1);
        }

        #[test]
        fn symbol_graph_layer_disabled_cache_records_bypass_without_layer_files() {
            let temp = tempfile::tempdir().expect("tempdir");
            let loaded = loaded_config_for(temp.path());
            let cache_root = temp.path().join("cache").join("analysis");
            let cache = Cache::new(&cache_root, false);
            let plan = requested_symbol_plan();
            let mut db = fixture_db(temp.path(), "./target");

            let derivation =
                derive_symbols_with_cache(&mut db, &loaded, &cache, &plan, "config", "stable");

            assert_eq!(derivation.cache_stats.bypasses_disabled, 1);
            assert_eq!(derivation.cache_stats.recomputes, 1);
            assert!(!temp.path().join("cache").join("layers").exists());
            assert!(!db.symbols().is_empty());
        }
    }

    fn symbol_rows(db: &AnalysisDb) -> Vec<(String, String, SymbolKind, SymbolPrecision)> {
        db.symbols()
            .iter()
            .map(|symbol| {
                (
                    symbol.stable_key.clone(),
                    symbol.name.clone(),
                    symbol.kind,
                    symbol.precision,
                )
            })
            .collect()
    }

    fn reference_rows(
        db: &AnalysisDb,
    ) -> Vec<(
        String,
        String,
        ReferenceKind,
        SymbolResolutionStatus,
        SymbolPrecision,
    )> {
        db.references()
            .iter()
            .map(|reference| {
                (
                    reference.stable_key.clone(),
                    reference.name.clone(),
                    reference.kind,
                    reference.status,
                    reference.precision,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod semantic_layer_payload {
    use super::*;
    use crate::analysis_kernel::incremental::LayerCacheManifest;
    use crate::analysis_kernel::{FactFamily, FactRef};
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, FileId, ImportFact, ImportId, Language, ModuleNode, ModuleNodeId,
        ModuleNodeKind, ResolutionPrecision, ResolutionStatus, ResolvedImportFact,
        ResolvedImportId, Span, SymbolNamespace, span_from_byte_range,
    };
    use crate::symbol_graph::semantic::{
        ExportFact, ExportId, ExportKind, ScopeFact, ScopeId, ScopeKind, SemanticIndexOutput,
        SemanticStatus, StableExportId, StableExportIdentity,
    };
    use std::path::Path;

    fn add_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) -> FileId {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().expect("test file has parent")).expect("mkdirs");
        std::fs::write(&path, source).expect("write fixture file");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn span_for(file: FileId, source: &str, needle: &str) -> Span {
        let start = source
            .find(needle)
            .unwrap_or_else(|| panic!("missing fixture text {needle:?}"));
        span_from_byte_range(file, source, start, start + needle.len())
    }

    fn push_import(db: &mut AnalysisDb, file: FileId, source: &str, path: &str) -> ImportId {
        db.push_import(ImportFact {
            id: ImportId(0),
            file,
            package: None,
            path: path.to_string(),
            span: span_for(file, source, &format!("{path:?}")),
            language: Language::TypeScript,
        })
    }

    fn fixture_db(root: &Path) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let app_source = r#"
import { token as importedToken } from "./target";

export function answer() {
    return importedToken;
}
"#;
        let target_source = "export const token = 42;\n";
        let app = add_file(&mut db, root, "src/app.ts", app_source);
        let target = add_file(&mut db, root, "src/target.ts", target_source);
        let import = push_import(&mut db, app, app_source, "./target");
        db.replace_module_graph_facts(
            vec![ResolvedImportFact {
                id: ResolvedImportId(0),
                import,
                from_file: app,
                target_node: Some(ModuleNodeId(1)),
                status: ResolutionStatus::Resolved,
                precision: ResolutionPrecision::ExactFile,
                reason: None,
            }],
            vec![
                ModuleNode {
                    id: ModuleNodeId(0),
                    kind: ModuleNodeKind::File,
                    label: "src/app.ts".to_string(),
                    file: Some(app),
                    package: None,
                    language: Some(Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(1),
                    kind: ModuleNodeKind::File,
                    label: "src/target.ts".to_string(),
                    file: Some(target),
                    package: None,
                    language: Some(Language::TypeScript),
                },
            ],
            Vec::new(),
        );
        db
    }

    fn manifest() -> &'static crate::analysis_kernel::ProviderManifest {
        crate::analysis_kernel::AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.symbol_graph")
            .expect("symbol graph provider manifest exists")
    }

    fn layer_key() -> LayerKey {
        LayerKey::symbol_graph_layer_key(
            manifest(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Digest::from_parts(
                DigestKind::AnalysisSettings,
                "provider_analysis_settings",
                &["polint.symbol_graph", "base"],
            ),
            Digest::from_parts(DigestKind::GoLifecycle, "go", &["base"]),
            Digest::from_parts(DigestKind::TsJsLifecycle, "ts", &["base"]),
            Digest::from_parts(DigestKind::ProviderOutput, "module_graph", &["base"]),
            vec![Digest::from_parts(
                DigestKind::ProviderOutput,
                "ts_syntax",
                &["base"],
            )],
            semantic_provider_parameter_digest(),
        )
    }

    fn cache_manifest_for(payload: &SymbolGraphLayerPayload) -> LayerCacheManifest {
        let key = layer_key();
        let payload_digest =
            LayerCacheStore::payload_digest_for_json(payload).expect("payload digest");
        let output_digest = symbol_graph_output_digest_for_payload(payload, Some(&key));
        LayerCacheManifest::new(
            key,
            output_digest,
            payload_digest,
            Vec::new(),
            PrecisionTier::SetupAware,
            crate::analysis_kernel::incremental::ProviderValidationStatus::NativeTrusted,
            Vec::new(),
        )
    }

    fn scope(file: FileId, stable_key: &str) -> ScopeFact {
        ScopeFact {
            id: ScopeId(0),
            language: Language::TypeScript,
            file: Some(file),
            package: None,
            module: None,
            parent: None,
            scope_path: vec!["module".to_string()],
            kind: ScopeKind::Module,
            stable_key: stable_key.to_string(),
            status: SemanticStatus::Resolved,
        }
    }

    fn stable_export(
        file: FileId,
        export_id: ExportId,
        stable_key: &str,
        symbol_key: &str,
    ) -> StableExportIdentity {
        StableExportIdentity {
            id: StableExportId(0),
            export: export_id,
            language: Language::TypeScript,
            package_key: None,
            module_key: Some(format!("file:{}", file.0)),
            export_name: "answer".to_string(),
            namespace: SymbolNamespace::Value,
            symbol_stable_key: symbol_key.to_string(),
            generated_discriminator: None,
            stable_key: stable_key.to_string(),
            status: SemanticStatus::Resolved,
        }
    }

    fn payload_with_semantic_index(semantic_index: SemanticIndexOutput) -> SymbolGraphLayerPayload {
        SymbolGraphLayerPayload {
            schema: SYMBOL_GRAPH_LAYER_SCHEMA.to_string(),
            diagnostics: Vec::new(),
            capability_support: Vec::new(),
            symbols: Vec::new(),
            definitions: Vec::new(),
            references: Vec::new(),
            semantic_index,
        }
    }

    #[test]
    fn cold_payload_writes_semantic_rows() {
        let temp = tempfile::tempdir().expect("tempdir");
        let loaded = load_config(temp.path()).expect("config");
        let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
        let plan = AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]);
        let mut db = fixture_db(temp.path());
        let identity_sources = InputSnapshot::identity_sources_from_plan(&loaded, &plan);
        let requested_capabilities = plan.requested_capability_snapshots();
        assert!(!requested_capabilities.is_empty());
        assert_eq!(
            identity_sources.requested_capabilities,
            requested_capabilities
        );
        assert_eq!(
            identity_sources.analysis_requirements_identity,
            plan.analysis_requirements_digest()
        );
        let snapshot = InputSnapshot::from_run_inputs_with_plan(
            &loaded,
            &db,
            "config",
            "rule-digest",
            &plan,
            crate::analysis_kernel::AnalysisKernel::provider_manifests(),
        );
        let derivation = derive_requested_symbols_with_cache_stats(
            &mut db,
            &loaded,
            &plan,
            &cache,
            &snapshot,
            manifest(),
            Digest::from_parts(DigestKind::ProviderOutput, "module_graph", &["base"]),
            vec![ProviderOutputDependency::present(Digest::from_parts(
                DigestKind::ProviderOutput,
                "ts_syntax",
                &["base"],
            ))],
        );

        let payload = symbol_graph_layer_payload(&db, &derivation);

        assert!(!payload.semantic_index.scopes.is_empty());
        assert!(!payload.semantic_index.stable_exports.is_empty());
    }

    #[test]
    fn restore_replaces_semantic_rows_and_metadata() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            Path::new("src/app.ts").to_path_buf(),
            "src/app.ts".to_string(),
            "export const answer = 42;\n".to_string(),
        );
        let export = ExportFact {
            id: ExportId(0),
            language: Language::TypeScript,
            file: Some(file),
            package: None,
            module: None,
            scope: None,
            symbol: None,
            export_name: "answer".to_string(),
            namespace: SymbolNamespace::Value,
            kind: ExportKind::Named,
            stable_key: "semantic:export:answer".to_string(),
            status: SemanticStatus::Resolved,
        };
        let payload = payload_with_semantic_index(SemanticIndexOutput {
            scopes: vec![scope(file, "semantic:scope:module")],
            exports: vec![export],
            stable_exports: vec![stable_export(
                file,
                ExportId(0),
                "semantic:stable-export:answer",
                "symbol:answer",
            )],
            ..SemanticIndexOutput::default()
        });

        restore_symbol_graph_layer_payload(&mut db, &payload);

        assert_eq!(db.scopes().len(), 1);
        assert_eq!(db.exports().len(), 1);
        assert_eq!(db.stable_exports().len(), 1);
        assert!(
            db.metadata_for(FactRef::new(FactFamily::StableExport, 0))
                .is_some()
        );
    }

    #[test]
    fn validation_rejects_missing_keys_absolute_paths_and_stable_export_conflicts() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            Path::new("src/app.ts").to_path_buf(),
            "src/app.ts".to_string(),
            "export const answer = 42;\n".to_string(),
        );
        let valid_payload = payload_with_semantic_index(SemanticIndexOutput {
            scopes: vec![scope(file, "semantic:scope:module")],
            stable_exports: vec![stable_export(
                file,
                ExportId(0),
                "semantic:stable-export:answer",
                "symbol:answer",
            )],
            ..SemanticIndexOutput::default()
        });
        let valid_manifest = cache_manifest_for(&valid_payload);
        assert!(validate_symbol_graph_layer_payload(
            &valid_payload,
            &valid_manifest
        ));

        let mut empty_key_payload = valid_payload.clone();
        empty_key_payload.semantic_index.scopes[0]
            .stable_key
            .clear();
        assert!(!validate_symbol_graph_layer_payload(
            &empty_key_payload,
            &cache_manifest_for(&empty_key_payload)
        ));

        let mut absolute_path_payload = valid_payload.clone();
        absolute_path_payload.semantic_index.stable_exports[0].module_key =
            Some("/tmp/repo/src/app.ts".to_string());
        assert!(!validate_symbol_graph_layer_payload(
            &absolute_path_payload,
            &cache_manifest_for(&absolute_path_payload)
        ));

        let mut conflict_payload = valid_payload;
        conflict_payload
            .semantic_index
            .stable_exports
            .push(stable_export(
                file,
                ExportId(1),
                "semantic:stable-export:answer",
                "symbol:other",
            ));
        assert!(!validate_symbol_graph_layer_payload(
            &conflict_payload,
            &cache_manifest_for(&conflict_payload)
        ));
    }
}

#[cfg(test)]
mod semantic_cache_restore {
    use crate::analysis_kernel::{AnalysisKernel, KernelInput, KernelOutput};
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use crate::symbol_graph::semantic::SemanticStatus;
    use std::path::Path;

    fn requested_symbol_plan() -> AnalysisPlan {
        AnalysisPlan::from_capability_names_for_test(&["symbols", "references"])
    }

    fn run_kernel(root: &Path, cache: &Cache, plan: &AnalysisPlan) -> KernelOutput {
        let loaded = load_config(root).expect("default config loads");
        AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache,
            config_digest: "config",
            rule_digest: "rules",
            plan,
            parallel: false,
        })
        .expect("kernel run should succeed")
    }

    fn symbol_graph_output(
        output: &KernelOutput,
    ) -> &crate::analysis_kernel::incremental::ProviderOutputMeta {
        output
            .run_report
            .provider_outputs
            .iter()
            .find(|row| row.provider_id == "polint.symbol_graph")
            .expect("symbol graph provider output exists")
    }

    fn stable_export_keys(output: &KernelOutput) -> Vec<String> {
        let has_empty_key = output.db.stable_exports().iter().any(stable_key_is_empty);
        assert!(!has_empty_key);
        let mut keys = output
            .db
            .stable_exports()
            .iter()
            .map(|row| row.stable_key.clone())
            .collect::<Vec<_>>();
        keys.sort();
        keys
    }

    fn stable_key_is_empty(row: &crate::symbol_graph::semantic::StableExportIdentity) -> bool {
        row.stable_key.is_empty()
    }

    fn assert_warm_symbol_graph_reuse(output: &KernelOutput) {
        let warm = symbol_graph_output(output);
        assert_eq!(warm.cache_stats.hits, 1);
        assert_eq!(warm.cache_stats.verified_reuse, 1);
        assert_eq!(warm.cache_stats.recomputes, 0);
    }

    #[test]
    fn ts_stable_exports_survive_warm_cache_restore() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("src")).expect("create src");
        std::fs::write(
            temp.path().join("src/app.ts"),
            "import { token } from './tokens';\nexport function answer() { return token; }\n",
        )
        .expect("write app");
        std::fs::write(
            temp.path().join("src/tokens.ts"),
            "export const token = 42;\n",
        )
        .expect("write tokens");
        let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
        let plan = requested_symbol_plan();

        let cold = run_kernel(temp.path(), &cache, &plan);
        let warm = run_kernel(temp.path(), &cache, &plan);

        assert_warm_symbol_graph_reuse(&warm);
        let cold_keys = stable_export_keys(&cold);
        let warm_keys = stable_export_keys(&warm);
        assert!(!cold_keys.is_empty());
        assert_eq!(cold_keys, warm_keys);
    }

    #[test]
    fn go_stable_exports_survive_warm_cache_restore_when_setup_succeeds() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/cache\n\ngo 1.24\n",
        )
        .expect("write go.mod");
        std::fs::create_dir_all(temp.path().join("pkg")).expect("create pkg");
        std::fs::write(
            temp.path().join("pkg/cache.go"),
            "package pkg\n\nfunc Answer() int { return 42 }\n",
        )
        .expect("write go file");
        let cache = Cache::new(temp.path().join("cache").join("analysis"), true);
        let plan = requested_symbol_plan();

        let cold = run_kernel(temp.path(), &cache, &plan);
        let warm = run_kernel(temp.path(), &cache, &plan);

        assert_warm_symbol_graph_reuse(&warm);
        let cold_keys = stable_export_keys(&cold);
        let warm_keys = stable_export_keys(&warm);
        if cold_keys.is_empty() {
            assert!(cold.db.resolution_facts().iter().any(|row| {
                matches!(
                    row.status,
                    SemanticStatus::SetupMissing | SemanticStatus::Unsupported
                )
            }));
            assert!(cold.diagnostics.iter().any(|diagnostic| {
                diagnostic.rule_id == "polint/capability"
                    && diagnostic
                        .message
                        .contains("symbol graph provider support is setup_missing")
            }));
        } else {
            assert_eq!(cold_keys, warm_keys);
        }
    }
}
