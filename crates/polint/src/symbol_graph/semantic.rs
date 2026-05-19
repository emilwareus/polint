use crate::analysis_kernel::{FactFamily, stable_key_from_parts};
use crate::core::{FileId, Language, ModuleNodeId, PackageId, Span, SymbolId, SymbolNamespace};
use crate::diagnostics::{Diagnostic, TextRange};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const SYMBOL_GRAPH_PRODUCER_ID: &str = "polint.symbol_graph";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct ScopeId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct SemanticImportId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct ExportId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct AliasId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct ResolutionId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct GeneratedSymbolId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct StableExportId(pub(crate) u64);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ScopeFact {
    pub(crate) id: ScopeId,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) package: Option<PackageId>,
    pub(crate) module: Option<ModuleNodeId>,
    pub(crate) parent: Option<ScopeId>,
    pub(crate) scope_path: Vec<String>,
    pub(crate) kind: ScopeKind,
    pub(crate) stable_key: String,
    pub(crate) status: SemanticStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SemanticImportFact {
    pub(crate) id: SemanticImportId,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) package: Option<PackageId>,
    pub(crate) module: Option<ModuleNodeId>,
    pub(crate) scope: Option<ScopeId>,
    pub(crate) import_path: String,
    pub(crate) local_name: Option<String>,
    pub(crate) imported_name: Option<String>,
    pub(crate) namespace: SymbolNamespace,
    pub(crate) kind: SemanticImportKind,
    pub(crate) stable_key: String,
    pub(crate) status: SemanticStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ExportFact {
    pub(crate) id: ExportId,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) package: Option<PackageId>,
    pub(crate) module: Option<ModuleNodeId>,
    pub(crate) scope: Option<ScopeId>,
    pub(crate) symbol: Option<SymbolId>,
    pub(crate) export_name: String,
    pub(crate) namespace: SymbolNamespace,
    pub(crate) kind: ExportKind,
    pub(crate) stable_key: String,
    pub(crate) status: SemanticStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AliasFact {
    pub(crate) id: AliasId,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) package: Option<PackageId>,
    pub(crate) module: Option<ModuleNodeId>,
    pub(crate) source_symbol_stable_key: String,
    pub(crate) target_symbol_stable_keys: Vec<String>,
    pub(crate) kind: AliasKind,
    pub(crate) stable_key: String,
    pub(crate) status: SemanticStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ResolutionFact {
    pub(crate) id: ResolutionId,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) package: Option<PackageId>,
    pub(crate) module: Option<ModuleNodeId>,
    pub(crate) source_stable_key: String,
    pub(crate) target_stable_keys: Vec<String>,
    pub(crate) step: ResolutionStepKind,
    pub(crate) stable_key: String,
    pub(crate) status: SemanticStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GeneratedSymbolFact {
    pub(crate) id: GeneratedSymbolId,
    pub(crate) language: Language,
    pub(crate) file: Option<FileId>,
    pub(crate) package: Option<PackageId>,
    pub(crate) module: Option<ModuleNodeId>,
    pub(crate) symbol_stable_key: String,
    pub(crate) source_stable_key: String,
    pub(crate) producer_id: String,
    pub(crate) generator: String,
    pub(crate) generated_discriminator: String,
    pub(crate) kind: GeneratedSymbolKind,
    pub(crate) span: Option<Span>,
    pub(crate) stable_key: String,
    pub(crate) status: SemanticStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StableExportIdentity {
    pub(crate) id: StableExportId,
    pub(crate) export: ExportId,
    pub(crate) language: Language,
    pub(crate) package_key: Option<String>,
    pub(crate) module_key: Option<String>,
    pub(crate) export_name: String,
    pub(crate) namespace: SymbolNamespace,
    pub(crate) symbol_stable_key: String,
    pub(crate) generated_discriminator: Option<String>,
    pub(crate) stable_key: String,
    pub(crate) status: SemanticStatus,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SemanticIndexOutput {
    pub(crate) scopes: Vec<ScopeFact>,
    pub(crate) semantic_imports: Vec<SemanticImportFact>,
    pub(crate) exports: Vec<ExportFact>,
    pub(crate) aliases: Vec<AliasFact>,
    pub(crate) resolutions: Vec<ResolutionFact>,
    pub(crate) generated_symbols: Vec<GeneratedSymbolFact>,
    pub(crate) stable_exports: Vec<StableExportIdentity>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AliasReexportClosureOutput {
    pub(crate) aliases: Vec<AliasFact>,
    pub(crate) resolutions: Vec<ResolutionFact>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct NativeGeneratedHooksOutput {
    pub(crate) generated_symbols: Vec<GeneratedSymbolFact>,
    pub(crate) resolutions: Vec<ResolutionFact>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SemanticIndexBuilder {
    scopes: Vec<ScopeFact>,
    semantic_imports: Vec<SemanticImportFact>,
    exports: Vec<ExportFact>,
    aliases: Vec<AliasFact>,
    resolutions: Vec<ResolutionFact>,
    generated_symbols: Vec<GeneratedSymbolFact>,
    stable_exports: Vec<StableExportIdentity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum ScopeKind {
    Workspace,
    Package,
    Module,
    File,
    Class,
    Function,
    Method,
    Block,
    Catch,
    Loop,
    Switch,
    Type,
    Namespace,
    Generated,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum SemanticImportKind {
    StaticNamed,
    StaticDefault,
    StaticNamespace,
    SideEffect,
    TypeOnly,
    CommonJsRequire,
    DynamicImport,
    GoDefault,
    GoNamed,
    GoDot,
    GoBlank,
    GoImplicit,
    EsNamed,
    EsDefault,
    EsNamespace,
    ReExport,
    CommonJs,
    Dynamic,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum ExportKind {
    Named,
    Default,
    Namespace,
    StarReexport,
    CommonJsModuleExports,
    CommonJsExportsProperty,
    ReExport,
    Star,
    Generated,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum AliasKind {
    Import,
    ImportAlias,
    Export,
    ReExport,
    Generated,
    TypeOnly,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum ResolutionStepKind {
    LexicalLookup,
    ImportAliasLookup,
    ModuleLookup,
    MemberLookup,
    GeneratedHintLookup,
    UnknownFallback,
    Lexical,
    ImportAlias,
    ExportAlias,
    Package,
    Module,
    Member,
    Generated,
    External,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum SemanticStatus {
    Resolved,
    Ambiguous,
    Unresolved,
    Cycle,
    Generated,
    Dynamic,
    External,
    SetupMissing,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum GeneratedSymbolKind {
    FrameworkEntrypoint,
    Route,
    TestHarness,
    BuildGenerated,
    ExtensionProvided,
    Unknown,
}

impl SemanticIndexBuilder {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add_scope(&mut self, mut fact: ScopeFact) -> ScopeId {
        let id = ScopeId(self.scopes.len() as u64);
        fact.id = id;
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
        self.scopes.push(fact);
        id
    }

    pub(crate) fn add_semantic_import(&mut self, mut fact: SemanticImportFact) -> SemanticImportId {
        let id = SemanticImportId(self.semantic_imports.len() as u64);
        fact.id = id;
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
        self.semantic_imports.push(fact);
        id
    }

    pub(crate) fn add_export_identity(&mut self, mut fact: ExportFact) -> ExportId {
        let id = ExportId(self.exports.len() as u64);
        fact.id = id;
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
        self.exports.push(fact);
        id
    }

    pub(crate) fn add_alias(&mut self, mut fact: AliasFact) -> AliasId {
        let id = AliasId(self.aliases.len() as u64);
        fact.id = id;
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
        self.aliases.push(fact);
        id
    }

    pub(crate) fn add_resolution(&mut self, mut fact: ResolutionFact) -> ResolutionId {
        let id = ResolutionId(self.resolutions.len() as u64);
        fact.id = id;
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
        self.resolutions.push(fact);
        id
    }

    pub(crate) fn add_generated_symbol(
        &mut self,
        mut fact: GeneratedSymbolFact,
    ) -> GeneratedSymbolId {
        let id = GeneratedSymbolId(self.generated_symbols.len() as u64);
        fact.id = id;
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
        self.generated_symbols.push(fact);
        id
    }

    pub(crate) fn add_stable_export(&mut self, mut fact: StableExportIdentity) -> StableExportId {
        let id = StableExportId(self.stable_exports.len() as u64);
        fact.id = id;
        if fact.stable_key.is_empty() {
            fact.stable_key = fact.computed_stable_key();
        }
        self.stable_exports.push(fact);
        id
    }

    pub(crate) fn finish(self) -> SemanticIndexOutput {
        SemanticIndexOutput {
            scopes: sort_rows(self.scopes, scope_sort_key),
            semantic_imports: sort_rows(self.semantic_imports, semantic_import_sort_key),
            exports: sort_rows(self.exports, export_sort_key),
            aliases: sort_rows(self.aliases, alias_sort_key),
            resolutions: sort_rows(self.resolutions, resolution_sort_key),
            generated_symbols: sort_rows(self.generated_symbols, generated_symbol_sort_key),
            stable_exports: sort_rows(self.stable_exports, stable_export_sort_key),
        }
    }
}

impl SemanticIndexOutput {
    pub(crate) fn extend(&mut self, other: Self) {
        self.scopes.extend(other.scopes);
        self.semantic_imports.extend(other.semantic_imports);
        self.exports.extend(other.exports);
        self.aliases.extend(other.aliases);
        self.resolutions.extend(other.resolutions);
        self.generated_symbols.extend(other.generated_symbols);
        self.stable_exports.extend(other.stable_exports);
    }
}

pub(crate) fn alias_reexport_closure(
    aliases: &[AliasFact],
    exports: &[ExportFact],
    stable_exports: &[StableExportIdentity],
) -> AliasReexportClosureOutput {
    let max_iterations = aliases.len() + exports.len() + stable_exports.len() + 1;
    let alias_targets = aliases
        .iter()
        .map(|alias| {
            (
                alias.source_symbol_stable_key.as_str(),
                alias
                    .target_symbol_stable_keys
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut closure_aliases = BTreeMap::<String, AliasFact>::new();
    let mut closure_resolutions = BTreeMap::<String, ResolutionFact>::new();

    for alias in aliases {
        let (targets, status) = resolve_alias_targets(alias, &alias_targets, max_iterations);
        let mut closure = alias.clone();
        closure.target_symbol_stable_keys = targets.into_iter().collect();
        closure.status = closure_status(alias.status, status);
        closure.stable_key = closure.computed_stable_key();
        let resolution = closure_resolution_for_alias(&closure);
        closure_resolutions.insert(resolution.stable_key.clone(), resolution);
        closure_aliases.insert(closure.stable_key.clone(), closure);
    }

    let stable_export_targets = stable_exports
        .iter()
        .map(|export| export.symbol_stable_key.clone())
        .collect::<BTreeSet<_>>();
    for export in exports
        .iter()
        .filter(|export| matches!(export.kind, ExportKind::Star | ExportKind::StarReexport))
    {
        let mut closure = AliasFact {
            id: AliasId(0),
            language: export.language,
            file: export.file,
            package: export.package,
            module: export.module,
            source_symbol_stable_key: export.stable_key.clone(),
            target_symbol_stable_keys: stable_export_targets.iter().cloned().collect(),
            kind: AliasKind::ReExport,
            stable_key: String::new(),
            status: SemanticStatus::Ambiguous,
        };
        closure.stable_key = closure.computed_stable_key();
        let resolution = closure_resolution_for_alias(&closure);
        closure_resolutions.insert(resolution.stable_key.clone(), resolution);
        closure_aliases.insert(closure.stable_key.clone(), closure);
    }

    AliasReexportClosureOutput {
        aliases: sort_rows(closure_aliases.into_values().collect(), alias_sort_key),
        resolutions: sort_rows(
            closure_resolutions.into_values().collect(),
            resolution_sort_key,
        ),
    }
}

pub(crate) fn emit_native_generated_symbol_hooks(
    semantic: &SemanticIndexOutput,
) -> NativeGeneratedHooksOutput {
    let mut generated = BTreeMap::<String, GeneratedSymbolFact>::new();
    let mut resolutions = BTreeMap::<String, ResolutionFact>::new();
    let mut diagnostics = BTreeMap::<String, Diagnostic>::new();

    for stable_export in &semantic.stable_exports {
        let Some(discriminator) = stable_export.generated_discriminator.as_ref() else {
            continue;
        };
        let mut row = GeneratedSymbolFact {
            id: GeneratedSymbolId(0),
            language: stable_export.language,
            file: None,
            package: None,
            module: None,
            symbol_stable_key: stable_export.symbol_stable_key.clone(),
            source_stable_key: stable_export.stable_key.clone(),
            producer_id: SYMBOL_GRAPH_PRODUCER_ID.to_string(),
            generator: SYMBOL_GRAPH_PRODUCER_ID.to_string(),
            generated_discriminator: discriminator.clone(),
            kind: GeneratedSymbolKind::Unknown,
            span: None,
            stable_key: String::new(),
            status: SemanticStatus::Generated,
        };
        row.stable_key = row.computed_stable_key();

        if let Some(existing) = generated.get(&row.stable_key) {
            if existing != &row {
                diagnostics
                    .entry(row.stable_key.clone())
                    .or_insert_with(|| generated_symbol_conflict_diagnostic(&row.stable_key));
            }
            continue;
        }

        let resolution = generated_hint_resolution(stable_export, &row);
        resolutions.insert(resolution.stable_key.clone(), resolution);
        generated.insert(row.stable_key.clone(), row);
    }

    NativeGeneratedHooksOutput {
        generated_symbols: sort_rows(generated.into_values().collect(), generated_symbol_sort_key),
        resolutions: sort_rows(resolutions.into_values().collect(), resolution_sort_key),
        diagnostics: diagnostics.into_values().collect(),
    }
}

fn generated_hint_resolution(
    stable_export: &StableExportIdentity,
    generated: &GeneratedSymbolFact,
) -> ResolutionFact {
    let mut resolution = ResolutionFact {
        id: ResolutionId(0),
        language: stable_export.language,
        file: None,
        package: None,
        module: None,
        source_stable_key: stable_export.stable_key.clone(),
        target_stable_keys: vec![generated.stable_key.clone()],
        step: ResolutionStepKind::GeneratedHintLookup,
        stable_key: String::new(),
        status: SemanticStatus::Generated,
    };
    resolution.stable_key = resolution.computed_stable_key();
    resolution
}

fn generated_symbol_conflict_diagnostic(stable_key: &str) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        "Generated symbol hook stable key has incompatible native rows.",
    )
    .with_evidence("family", FactFamily::GeneratedSymbol.label().to_string())
    .with_evidence("stable_key", stable_key.to_string())
    .with_evidence("reason", "duplicate_generated_stable_key".to_string())
}

fn resolve_alias_targets(
    alias: &AliasFact,
    alias_targets: &BTreeMap<&str, Vec<&str>>,
    max_iterations: usize,
) -> (BTreeSet<String>, SemanticStatus) {
    if !matches!(
        alias.status,
        SemanticStatus::Resolved | SemanticStatus::Generated
    ) {
        return (
            alias.target_symbol_stable_keys.iter().cloned().collect(),
            alias.status,
        );
    }

    let mut terminals = BTreeSet::new();
    let mut stack = alias
        .target_symbol_stable_keys
        .iter()
        .map(|target| (target.as_str(), 0usize))
        .collect::<Vec<_>>();
    let mut seen_edges = BTreeSet::<(String, String)>::new();
    let mut status = SemanticStatus::Resolved;

    while let Some((target, depth)) = stack.pop() {
        if depth >= max_iterations || target == alias.source_symbol_stable_key {
            status = SemanticStatus::Cycle;
            continue;
        }
        if let Some(next_targets) = alias_targets.get(target) {
            for next in next_targets {
                let edge = (target.to_string(), (*next).to_string());
                if !seen_edges.insert(edge) {
                    status = SemanticStatus::Cycle;
                    continue;
                }
                stack.push((next, depth + 1));
            }
        } else {
            terminals.insert(target.to_string());
        }
    }

    (terminals, status)
}

fn closure_status(original: SemanticStatus, resolved: SemanticStatus) -> SemanticStatus {
    if resolved == SemanticStatus::Cycle {
        SemanticStatus::Cycle
    } else {
        original
    }
}

fn closure_resolution_for_alias(alias: &AliasFact) -> ResolutionFact {
    let mut resolution = ResolutionFact {
        id: ResolutionId(0),
        language: alias.language,
        file: alias.file,
        package: alias.package,
        module: alias.module,
        source_stable_key: alias.source_symbol_stable_key.clone(),
        target_stable_keys: alias.target_symbol_stable_keys.clone(),
        step: ResolutionStepKind::ImportAliasLookup,
        stable_key: String::new(),
        status: alias.status,
    };
    resolution.stable_key = resolution.computed_stable_key();
    resolution
}

impl ScopeFact {
    pub(crate) fn computed_stable_key(&self) -> String {
        Self::stable_key_for(
            self.language,
            &self.scope_path,
            self.file.map(file_id_key),
            self.package.map(package_id_key),
            self.module.map(module_node_id_key),
            self.kind,
            self.status,
        )
    }

    pub(crate) fn stable_key_for(
        language: Language,
        scope_path: &[String],
        file_key: Option<String>,
        package_key: Option<String>,
        module_key: Option<String>,
        kind: ScopeKind,
        status: SemanticStatus,
    ) -> String {
        stable_key_from_parts(
            FactFamily::Scope,
            &[
                ("language", language_label(language).to_string()),
                ("scope_path", sorted_repeated_value(scope_path)),
                ("file", option_key(file_key)),
                ("package", option_key(package_key)),
                ("module", option_key(module_key)),
                ("kind", scope_kind_label(kind).to_string()),
                ("status", semantic_status_label(status).to_string()),
            ],
        )
    }
}

impl SemanticImportFact {
    pub(crate) fn computed_stable_key(&self) -> String {
        stable_key_from_parts(
            FactFamily::SemanticImport,
            &[
                ("language", language_label(self.language).to_string()),
                (
                    "file",
                    self.file.map(file_id_key).unwrap_or_else(none_value),
                ),
                (
                    "package",
                    self.package.map(package_id_key).unwrap_or_else(none_value),
                ),
                (
                    "module",
                    self.module
                        .map(module_node_id_key)
                        .unwrap_or_else(none_value),
                ),
                ("name", option_key(self.local_name.clone())),
                ("namespace", namespace_label(self.namespace).to_string()),
                ("kind", semantic_import_kind_label(self.kind).to_string()),
                ("status", semantic_status_label(self.status).to_string()),
            ],
        )
    }
}

impl ExportFact {
    pub(crate) fn computed_stable_key(&self) -> String {
        stable_key_from_parts(
            FactFamily::Export,
            &[
                ("language", language_label(self.language).to_string()),
                (
                    "file",
                    self.file.map(file_id_key).unwrap_or_else(none_value),
                ),
                (
                    "package",
                    self.package.map(package_id_key).unwrap_or_else(none_value),
                ),
                (
                    "module",
                    self.module
                        .map(module_node_id_key)
                        .unwrap_or_else(none_value),
                ),
                ("export_name", self.export_name.clone()),
                ("namespace", namespace_label(self.namespace).to_string()),
                ("kind", export_kind_label(self.kind).to_string()),
                ("status", semantic_status_label(self.status).to_string()),
            ],
        )
    }
}

impl AliasFact {
    pub(crate) fn computed_stable_key(&self) -> String {
        stable_key_from_parts(
            FactFamily::Alias,
            &[
                ("language", language_label(self.language).to_string()),
                (
                    "file",
                    self.file.map(file_id_key).unwrap_or_else(none_value),
                ),
                (
                    "package",
                    self.package.map(package_id_key).unwrap_or_else(none_value),
                ),
                (
                    "module",
                    self.module
                        .map(module_node_id_key)
                        .unwrap_or_else(none_value),
                ),
                ("symbol_stable_key", self.source_symbol_stable_key.clone()),
                ("kind", alias_kind_label(self.kind).to_string()),
                ("status", semantic_status_label(self.status).to_string()),
            ],
        )
    }
}

impl ResolutionFact {
    pub(crate) fn computed_stable_key(&self) -> String {
        stable_key_from_parts(
            FactFamily::Resolution,
            &[
                ("language", language_label(self.language).to_string()),
                (
                    "file",
                    self.file.map(file_id_key).unwrap_or_else(none_value),
                ),
                (
                    "package",
                    self.package.map(package_id_key).unwrap_or_else(none_value),
                ),
                (
                    "module",
                    self.module
                        .map(module_node_id_key)
                        .unwrap_or_else(none_value),
                ),
                ("symbol_stable_key", self.source_stable_key.clone()),
                ("kind", resolution_step_kind_label(self.step).to_string()),
                ("status", semantic_status_label(self.status).to_string()),
            ],
        )
    }
}

impl GeneratedSymbolFact {
    pub(crate) fn computed_stable_key(&self) -> String {
        stable_key_from_parts(
            FactFamily::GeneratedSymbol,
            &[
                ("language", language_label(self.language).to_string()),
                (
                    "file",
                    self.file.map(file_id_key).unwrap_or_else(none_value),
                ),
                (
                    "package",
                    self.package.map(package_id_key).unwrap_or_else(none_value),
                ),
                (
                    "module",
                    self.module
                        .map(module_node_id_key)
                        .unwrap_or_else(none_value),
                ),
                ("symbol_stable_key", self.symbol_stable_key.clone()),
                ("source_stable_key", self.source_stable_key.clone()),
                ("producer_id", self.producer_id.clone()),
                (
                    "generated_discriminator",
                    self.generated_discriminator.clone(),
                ),
                ("kind", generated_symbol_kind_label(self.kind).to_string()),
                ("status", semantic_status_label(self.status).to_string()),
            ],
        )
    }
}

impl StableExportIdentity {
    pub(crate) fn computed_stable_key(&self) -> String {
        stable_key_from_parts(
            FactFamily::StableExport,
            &[
                ("language", language_label(self.language).to_string()),
                ("package", option_key(self.package_key.clone())),
                ("module", option_key(self.module_key.clone())),
                ("export_name", self.export_name.clone()),
                ("namespace", namespace_label(self.namespace).to_string()),
                ("symbol_stable_key", self.symbol_stable_key.clone()),
                (
                    "generated_discriminator",
                    option_key(self.generated_discriminator.clone()),
                ),
                ("status", semantic_status_label(self.status).to_string()),
            ],
        )
    }
}

fn sorted_repeated_value(values: &[String]) -> String {
    let mut values = values
        .iter()
        .map(|value| value.replace('\\', "/"))
        .collect::<Vec<_>>();
    values.sort();
    values
        .into_iter()
        .map(|value| format!("{}:{value}", value.len()))
        .collect::<Vec<_>>()
        .join(",")
}

fn option_key(value: Option<String>) -> String {
    value.unwrap_or_else(none_value)
}

fn file_id_key(id: FileId) -> String {
    id.0.to_string()
}

fn package_id_key(id: PackageId) -> String {
    id.0.to_string()
}

fn module_node_id_key(id: ModuleNodeId) -> String {
    id.0.to_string()
}

fn none_value() -> String {
    "<none>".to_string()
}

fn language_label(language: Language) -> &'static str {
    match language {
        Language::Go => "go",
        Language::TypeScript => "typescript",
        Language::Tsx => "tsx",
        Language::JavaScript => "javascript",
        Language::Jsx => "jsx",
        Language::Unknown => "unknown",
    }
}

fn namespace_label(namespace: SymbolNamespace) -> &'static str {
    match namespace {
        SymbolNamespace::Value => "value",
        SymbolNamespace::Type => "type",
        SymbolNamespace::Namespace => "namespace",
        SymbolNamespace::Package => "package",
        SymbolNamespace::Module => "module",
        SymbolNamespace::Unknown => "unknown",
    }
}

fn scope_kind_label(kind: ScopeKind) -> &'static str {
    match kind {
        ScopeKind::Workspace => "workspace",
        ScopeKind::Package => "package",
        ScopeKind::Module => "module",
        ScopeKind::File => "file",
        ScopeKind::Class => "class",
        ScopeKind::Function => "function",
        ScopeKind::Method => "method",
        ScopeKind::Block => "block",
        ScopeKind::Catch => "catch",
        ScopeKind::Loop => "loop",
        ScopeKind::Switch => "switch",
        ScopeKind::Type => "type",
        ScopeKind::Namespace => "namespace",
        ScopeKind::Generated => "generated",
        ScopeKind::Unknown => "unknown",
    }
}

fn sort_rows<T, K: Ord>(mut rows: Vec<T>, key: impl Fn(&T) -> K) -> Vec<T> {
    rows.sort_by_key(key);
    rows
}

fn scope_sort_key(fact: &ScopeFact) -> (String, Vec<String>, Option<FileId>, ScopeKind) {
    (
        fact.stable_key.clone(),
        fact.scope_path.clone(),
        fact.file,
        fact.kind,
    )
}

fn semantic_import_sort_key(
    fact: &SemanticImportFact,
) -> (
    String,
    Option<FileId>,
    String,
    Option<String>,
    SemanticImportKind,
    SemanticStatus,
) {
    (
        fact.stable_key.clone(),
        fact.file,
        fact.import_path.clone(),
        fact.local_name.clone(),
        fact.kind,
        fact.status,
    )
}

fn export_sort_key(
    fact: &ExportFact,
) -> (
    String,
    Option<FileId>,
    String,
    String,
    ExportKind,
    SemanticStatus,
) {
    (
        fact.stable_key.clone(),
        fact.file,
        fact.export_name.clone(),
        namespace_label(fact.namespace).to_string(),
        fact.kind,
        fact.status,
    )
}

fn alias_sort_key(
    fact: &AliasFact,
) -> (
    String,
    Option<FileId>,
    String,
    Vec<String>,
    AliasKind,
    SemanticStatus,
) {
    (
        fact.stable_key.clone(),
        fact.file,
        fact.source_symbol_stable_key.clone(),
        fact.target_symbol_stable_keys.clone(),
        fact.kind,
        fact.status,
    )
}

fn resolution_sort_key(
    fact: &ResolutionFact,
) -> (
    String,
    Option<FileId>,
    String,
    Vec<String>,
    ResolutionStepKind,
    SemanticStatus,
) {
    (
        fact.stable_key.clone(),
        fact.file,
        fact.source_stable_key.clone(),
        fact.target_stable_keys.clone(),
        fact.step,
        fact.status,
    )
}

fn generated_symbol_sort_key(
    fact: &GeneratedSymbolFact,
) -> (
    String,
    Option<FileId>,
    String,
    String,
    GeneratedSymbolKind,
    SemanticStatus,
) {
    (
        fact.stable_key.clone(),
        fact.file,
        fact.symbol_stable_key.clone(),
        fact.generated_discriminator.clone(),
        fact.kind,
        fact.status,
    )
}

fn stable_export_sort_key(
    fact: &StableExportIdentity,
) -> (
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
    SemanticStatus,
) {
    (
        fact.stable_key.clone(),
        fact.package_key.clone(),
        fact.module_key.clone(),
        fact.export_name.clone(),
        namespace_label(fact.namespace).to_string(),
        fact.symbol_stable_key.clone(),
        fact.generated_discriminator.clone(),
        fact.status,
    )
}

fn semantic_import_kind_label(kind: SemanticImportKind) -> &'static str {
    match kind {
        SemanticImportKind::StaticNamed => "static_named",
        SemanticImportKind::StaticDefault => "static_default",
        SemanticImportKind::StaticNamespace => "static_namespace",
        SemanticImportKind::SideEffect => "side_effect",
        SemanticImportKind::TypeOnly => "type_only",
        SemanticImportKind::CommonJsRequire => "commonjs_require",
        SemanticImportKind::DynamicImport => "dynamic_import",
        SemanticImportKind::GoDefault => "go_default",
        SemanticImportKind::GoNamed => "go_named",
        SemanticImportKind::GoDot => "go_dot",
        SemanticImportKind::GoBlank => "go_blank",
        SemanticImportKind::GoImplicit => "go_implicit",
        SemanticImportKind::EsNamed => "es_named",
        SemanticImportKind::EsDefault => "es_default",
        SemanticImportKind::EsNamespace => "es_namespace",
        SemanticImportKind::ReExport => "re_export",
        SemanticImportKind::CommonJs => "common_js",
        SemanticImportKind::Dynamic => "dynamic",
        SemanticImportKind::Unknown => "unknown",
    }
}

fn export_kind_label(kind: ExportKind) -> &'static str {
    match kind {
        ExportKind::Named => "named",
        ExportKind::Default => "default",
        ExportKind::Namespace => "namespace",
        ExportKind::StarReexport => "star_reexport",
        ExportKind::CommonJsModuleExports => "commonjs_module_exports",
        ExportKind::CommonJsExportsProperty => "commonjs_exports_property",
        ExportKind::ReExport => "re_export",
        ExportKind::Star => "star",
        ExportKind::Generated => "generated",
        ExportKind::Unknown => "unknown",
    }
}

fn alias_kind_label(kind: AliasKind) -> &'static str {
    match kind {
        AliasKind::Import => "import",
        AliasKind::ImportAlias => "import_alias",
        AliasKind::Export => "export",
        AliasKind::ReExport => "re_export",
        AliasKind::Generated => "generated",
        AliasKind::TypeOnly => "type_only",
        AliasKind::Unknown => "unknown",
    }
}

fn resolution_step_kind_label(kind: ResolutionStepKind) -> &'static str {
    match kind {
        ResolutionStepKind::LexicalLookup => "LexicalLookup",
        ResolutionStepKind::ImportAliasLookup => "ImportAliasLookup",
        ResolutionStepKind::ModuleLookup => "ModuleLookup",
        ResolutionStepKind::MemberLookup => "MemberLookup",
        ResolutionStepKind::GeneratedHintLookup => "GeneratedHintLookup",
        ResolutionStepKind::UnknownFallback => "UnknownFallback",
        ResolutionStepKind::Lexical => "lexical",
        ResolutionStepKind::ImportAlias => "import_alias",
        ResolutionStepKind::ExportAlias => "export_alias",
        ResolutionStepKind::Package => "package",
        ResolutionStepKind::Module => "module",
        ResolutionStepKind::Member => "member",
        ResolutionStepKind::Generated => "generated",
        ResolutionStepKind::External => "external",
        ResolutionStepKind::Unsupported => "unsupported",
    }
}

fn semantic_status_label(status: SemanticStatus) -> &'static str {
    match status {
        SemanticStatus::Resolved => "resolved",
        SemanticStatus::Ambiguous => "ambiguous",
        SemanticStatus::Unresolved => "unresolved",
        SemanticStatus::Cycle => "cycle",
        SemanticStatus::Generated => "generated",
        SemanticStatus::Dynamic => "dynamic",
        SemanticStatus::External => "external",
        SemanticStatus::SetupMissing => "setup_missing",
        SemanticStatus::Unsupported => "unsupported",
    }
}

fn generated_symbol_kind_label(kind: GeneratedSymbolKind) -> &'static str {
    match kind {
        GeneratedSymbolKind::FrameworkEntrypoint => "framework_entrypoint",
        GeneratedSymbolKind::Route => "route",
        GeneratedSymbolKind::TestHarness => "test_harness",
        GeneratedSymbolKind::BuildGenerated => "build_generated",
        GeneratedSymbolKind::ExtensionProvided => "extension_provided",
        GeneratedSymbolKind::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{FileId, Language, ModuleNodeId, PackageId, SymbolNamespace};

    #[test]
    fn scope_stable_keys_are_deterministic_across_input_order() {
        let first = ScopeFact::stable_key_for(
            Language::TypeScript,
            &["module".to_string(), "function:handler".to_string()],
            Some("src/api.ts".to_string()),
            Some("pkg:web".to_string()),
            Some("mod:web".to_string()),
            ScopeKind::Function,
            SemanticStatus::Resolved,
        );
        let second = ScopeFact::stable_key_for(
            Language::TypeScript,
            &["function:handler".to_string(), "module".to_string()],
            Some("src/api.ts".to_string()),
            Some("pkg:web".to_string()),
            Some("mod:web".to_string()),
            ScopeKind::Function,
            SemanticStatus::Resolved,
        );

        assert_eq!(first, second);
    }

    #[test]
    fn stable_export_identity_key_includes_required_context() {
        let identity = StableExportIdentity {
            id: StableExportId(17),
            export: ExportId(3),
            language: Language::Go,
            package_key: Some("pkg:service".to_string()),
            module_key: Some("mod:service".to_string()),
            export_name: "Handler".to_string(),
            namespace: SymbolNamespace::Value,
            symbol_stable_key: "symbol:handler".to_string(),
            generated_discriminator: Some("route:/users".to_string()),
            stable_key: String::new(),
            status: SemanticStatus::Generated,
        };

        let stable_key = identity.computed_stable_key();

        assert!(stable_key.contains("language"));
        assert!(stable_key.contains("package"));
        assert!(stable_key.contains("module"));
        assert!(stable_key.contains("export_name"));
        assert!(stable_key.contains("namespace"));
        assert!(stable_key.contains("symbol_stable_key"));
        assert!(stable_key.contains("generated_discriminator"));
    }

    #[test]
    fn alias_status_supports_all_closure_outcomes() {
        let statuses = [
            SemanticStatus::Resolved,
            SemanticStatus::Ambiguous,
            SemanticStatus::Unresolved,
            SemanticStatus::Cycle,
            SemanticStatus::Generated,
            SemanticStatus::Dynamic,
            SemanticStatus::External,
            SemanticStatus::SetupMissing,
            SemanticStatus::Unsupported,
        ];

        let aliases = statuses
            .into_iter()
            .enumerate()
            .map(|(index, status)| AliasFact {
                id: AliasId(index as u64),
                language: Language::JavaScript,
                file: Some(FileId(0)),
                package: Some(PackageId(1)),
                module: Some(ModuleNodeId(2)),
                source_symbol_stable_key: format!("alias:{index}"),
                target_symbol_stable_keys: Vec::new(),
                kind: AliasKind::ReExport,
                stable_key: format!("alias-key:{index}"),
                status,
            })
            .collect::<Vec<_>>();

        assert_eq!(aliases.len(), 9);
    }

    #[test]
    fn semantic_index_builder_sorts_scope_rows_by_stable_key() {
        let mut builder = SemanticIndexBuilder::new();
        builder.add_scope(ScopeFact {
            id: ScopeId(0),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            package: None,
            module: None,
            parent: None,
            scope_path: vec!["src/app.ts".to_string(), "function@20-40".to_string()],
            kind: ScopeKind::Function,
            stable_key: "z-scope".to_string(),
            status: SemanticStatus::Resolved,
        });
        builder.add_scope(ScopeFact {
            id: ScopeId(0),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            package: None,
            module: None,
            parent: None,
            scope_path: vec!["src/app.ts".to_string(), "module@0-50".to_string()],
            kind: ScopeKind::Module,
            stable_key: "a-scope".to_string(),
            status: SemanticStatus::Resolved,
        });

        let output = builder.finish();

        assert_eq!(
            output
                .scopes
                .iter()
                .map(|scope| scope.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["a-scope", "z-scope"]
        );
    }

    #[test]
    fn semantic_index_builder_collects_all_row_families() {
        let mut builder = SemanticIndexBuilder::new();
        builder.add_semantic_import(SemanticImportFact {
            id: SemanticImportId(0),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            package: None,
            module: None,
            scope: None,
            import_path: "./target".to_string(),
            local_name: Some("target".to_string()),
            imported_name: Some("default".to_string()),
            namespace: SymbolNamespace::Value,
            kind: SemanticImportKind::EsDefault,
            stable_key: String::new(),
            status: SemanticStatus::Resolved,
        });
        let export = builder.add_export_identity(ExportFact {
            id: ExportId(0),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            package: None,
            module: None,
            scope: None,
            symbol: None,
            export_name: "target".to_string(),
            namespace: SymbolNamespace::Value,
            kind: ExportKind::Default,
            stable_key: String::new(),
            status: SemanticStatus::Resolved,
        });
        builder.add_alias(AliasFact {
            id: AliasId(0),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            package: None,
            module: None,
            source_symbol_stable_key: "alias:source".to_string(),
            target_symbol_stable_keys: vec!["alias:target".to_string()],
            kind: AliasKind::Import,
            stable_key: String::new(),
            status: SemanticStatus::Resolved,
        });
        builder.add_resolution(ResolutionFact {
            id: ResolutionId(0),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            package: None,
            module: None,
            source_stable_key: "ref:source".to_string(),
            target_stable_keys: vec!["symbol:target".to_string()],
            step: ResolutionStepKind::Lexical,
            stable_key: String::new(),
            status: SemanticStatus::Resolved,
        });
        builder.add_generated_symbol(GeneratedSymbolFact {
            id: GeneratedSymbolId(0),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            package: None,
            module: None,
            symbol_stable_key: "symbol:generated".to_string(),
            source_stable_key: "stable-export:generated".to_string(),
            producer_id: "polint.symbol_graph".to_string(),
            generator: "test".to_string(),
            generated_discriminator: "native".to_string(),
            kind: GeneratedSymbolKind::Unknown,
            span: None,
            stable_key: String::new(),
            status: SemanticStatus::Generated,
        });
        builder.add_stable_export(StableExportIdentity {
            id: StableExportId(0),
            export,
            language: Language::TypeScript,
            package_key: None,
            module_key: Some("src/target.ts".to_string()),
            export_name: "target".to_string(),
            namespace: SymbolNamespace::Value,
            symbol_stable_key: "symbol:target".to_string(),
            generated_discriminator: Some("native".to_string()),
            stable_key: String::new(),
            status: SemanticStatus::Resolved,
        });

        let output = builder.finish();

        assert_eq!(output.semantic_imports.len(), 1);
        assert_eq!(output.exports.len(), 1);
        assert_eq!(output.aliases.len(), 1);
        assert_eq!(output.resolutions.len(), 1);
        assert_eq!(output.generated_symbols.len(), 1);
        assert_eq!(output.stable_exports.len(), 1);
    }
}

#[cfg(test)]
mod alias_reexport_closure_tests {
    use super::*;
    use crate::core::{FileId, Language, SymbolNamespace};

    fn alias(source: &str, targets: &[&str], status: SemanticStatus) -> AliasFact {
        AliasFact {
            id: AliasId(0),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            package: None,
            module: None,
            source_symbol_stable_key: source.to_string(),
            target_symbol_stable_keys: targets.iter().map(|target| (*target).to_string()).collect(),
            kind: AliasKind::Import,
            stable_key: String::new(),
            status,
        }
    }

    fn star_export(stable_key: &str, status: SemanticStatus) -> ExportFact {
        ExportFact {
            id: ExportId(0),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            package: None,
            module: None,
            scope: None,
            symbol: None,
            export_name: "*".to_string(),
            namespace: SymbolNamespace::Module,
            kind: ExportKind::StarReexport,
            stable_key: stable_key.to_string(),
            status,
        }
    }

    fn stable_export(export_name: &str, symbol_key: &str) -> StableExportIdentity {
        StableExportIdentity {
            id: StableExportId(0),
            export: ExportId(0),
            language: Language::TypeScript,
            package_key: None,
            module_key: Some("src/mod.ts".to_string()),
            export_name: export_name.to_string(),
            namespace: SymbolNamespace::Value,
            symbol_stable_key: symbol_key.to_string(),
            generated_discriminator: Some("native".to_string()),
            stable_key: format!("stable-export:{export_name}"),
            status: SemanticStatus::Resolved,
        }
    }

    #[test]
    fn acyclic_import_alias_chains_resolve_to_deterministic_targets() {
        let output = super::alias_reexport_closure(
            &[
                alias("import:a", &["import:b"], SemanticStatus::Resolved),
                alias("import:b", &["symbol:c"], SemanticStatus::Resolved),
            ],
            &[],
            &[],
        );

        let closure_alias = output
            .aliases
            .iter()
            .find(|alias| alias.source_symbol_stable_key == "import:a")
            .expect("closure contains source alias");

        assert_eq!(closure_alias.status, SemanticStatus::Resolved);
        assert_eq!(closure_alias.target_symbol_stable_keys, vec!["symbol:c"]);
        assert!(output.resolutions.iter().any(|resolution| {
            resolution.source_stable_key == "import:a"
                && resolution.target_stable_keys == vec!["symbol:c".to_string()]
                && resolution.step == ResolutionStepKind::ImportAliasLookup
                && resolution.status == SemanticStatus::Resolved
        }));
    }

    #[test]
    fn reexport_cycles_terminate_and_emit_cycle_rows() {
        let output = super::alias_reexport_closure(
            &[
                alias("reexport:a", &["reexport:b"], SemanticStatus::Resolved),
                alias("reexport:b", &["reexport:a"], SemanticStatus::Resolved),
            ],
            &[],
            &[],
        );

        assert!(output.aliases.iter().any(|alias| {
            alias.source_symbol_stable_key == "reexport:a" && alias.status == SemanticStatus::Cycle
        }));
        assert!(output.resolutions.iter().any(|resolution| {
            resolution.source_stable_key == "reexport:a"
                && resolution.step == ResolutionStepKind::ImportAliasLookup
                && resolution.status == SemanticStatus::Cycle
        }));
    }

    #[test]
    fn star_exports_with_incomplete_targets_emit_ambiguous_closure_rows() {
        let output = super::alias_reexport_closure(
            &[],
            &[star_export(
                "export:*:src/star.ts",
                SemanticStatus::Unresolved,
            )],
            &[stable_export("known", "symbol:known")],
        );

        let alias = output
            .aliases
            .iter()
            .find(|alias| alias.source_symbol_stable_key == "export:*:src/star.ts")
            .expect("star export gets conservative closure alias");

        assert_eq!(alias.status, SemanticStatus::Ambiguous);
        assert_eq!(alias.target_symbol_stable_keys, vec!["symbol:known"]);
        assert!(output.resolutions.iter().any(|resolution| {
            resolution.source_stable_key == "export:*:src/star.ts"
                && resolution.status == SemanticStatus::Ambiguous
        }));
    }
}

#[cfg(test)]
mod native_generated_hooks {
    use super::*;
    use crate::analysis_kernel::{FactFamily, FactRef};
    use crate::core::{AnalysisDb, Language, SymbolNamespace};

    fn stable_export(
        language: Language,
        export_name: &str,
        generated: bool,
    ) -> StableExportIdentity {
        StableExportIdentity {
            id: StableExportId(0),
            export: ExportId(0),
            language,
            package_key: (language == Language::Go)
                .then(|| "go:package:example.com/app".to_string()),
            module_key: (language != Language::Go).then(|| "src/mod.ts".to_string()),
            export_name: export_name.to_string(),
            namespace: SymbolNamespace::Value,
            symbol_stable_key: format!("symbol:{export_name}"),
            generated_discriminator: generated.then(|| "native".to_string()),
            stable_key: format!("stable-export:{export_name}"),
            status: if generated {
                SemanticStatus::Generated
            } else {
                SemanticStatus::Resolved
            },
        }
    }

    #[test]
    fn ts_native_stable_exports_emit_generated_symbol_hooks_with_provenance() {
        let mut semantic = SemanticIndexOutput::default();
        semantic
            .stable_exports
            .push(stable_export(Language::TypeScript, "route", true));

        let output = super::emit_native_generated_symbol_hooks(&semantic);

        assert_eq!(output.generated_symbols.len(), 1);
        let generated = &output.generated_symbols[0];
        assert_eq!(generated.status, SemanticStatus::Generated);
        assert_eq!(generated.producer_id, "polint.symbol_graph");
        assert_eq!(generated.source_stable_key, "stable-export:route");
        assert_eq!(generated.generated_discriminator, "native");
        assert!(generated.stable_key.contains("polint.symbol_graph"));
        assert!(output.resolutions.iter().any(|resolution| {
            resolution.step == ResolutionStepKind::GeneratedHintLookup
                && resolution.status == SemanticStatus::Generated
                && resolution.source_stable_key == "stable-export:route"
        }));
    }

    #[test]
    fn go_generated_stable_exports_emit_generated_symbol_hooks() {
        let mut semantic = SemanticIndexOutput::default();
        semantic
            .stable_exports
            .push(stable_export(Language::Go, "Handler", true));

        let output = super::emit_native_generated_symbol_hooks(&semantic);

        assert_eq!(output.generated_symbols.len(), 1);
        let generated = &output.generated_symbols[0];
        assert_eq!(generated.language, Language::Go);
        assert_eq!(generated.status, SemanticStatus::Generated);
        assert_eq!(generated.producer_id, "polint.symbol_graph");
        assert_eq!(generated.source_stable_key, "stable-export:Handler");
    }

    #[test]
    fn generated_hooks_have_metadata_after_semantic_replacement() {
        let mut semantic = SemanticIndexOutput::default();
        semantic
            .stable_exports
            .push(stable_export(Language::TypeScript, "route", true));
        let output = super::emit_native_generated_symbol_hooks(&semantic);
        let mut db = AnalysisDb::new();
        db.add_file(
            std::path::PathBuf::from("src/mod.ts"),
            "src/mod.ts".to_string(),
            "export const route = 1;\n".to_string(),
        );

        db.replace_semantic_index_facts(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            output.resolutions,
            output.generated_symbols,
            semantic.stable_exports,
        );

        let metadata = db
            .metadata_for(FactRef::new(FactFamily::GeneratedSymbol, 0))
            .expect("generated hook metadata exists");

        assert_eq!(metadata.producer_id, "polint.symbol_graph");
        assert_eq!(db.generated_symbols()[0].producer_id, "polint.symbol_graph");
    }
}
