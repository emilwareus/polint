use crate::analysis_kernel::{FactFamily, stable_key_from_parts};
use crate::core::{FileId, Language, ModuleNodeId, PackageId, Span, SymbolId, SymbolNamespace};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ScopeKind {
    Workspace,
    Package,
    Module,
    File,
    Class,
    Function,
    Block,
    Generated,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SemanticImportKind {
    GoDefault,
    GoNamed,
    GoDot,
    GoBlank,
    EsNamed,
    EsDefault,
    EsNamespace,
    ReExport,
    CommonJs,
    Dynamic,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ExportKind {
    Named,
    Default,
    Namespace,
    ReExport,
    Star,
    Generated,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum AliasKind {
    Import,
    Export,
    ReExport,
    Generated,
    TypeOnly,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ResolutionStepKind {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum GeneratedSymbolKind {
    FrameworkEntrypoint,
    Route,
    TestHarness,
    BuildGenerated,
    ExtensionProvided,
    Unknown,
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
        ScopeKind::Block => "block",
        ScopeKind::Generated => "generated",
        ScopeKind::Unknown => "unknown",
    }
}

fn semantic_import_kind_label(kind: SemanticImportKind) -> &'static str {
    match kind {
        SemanticImportKind::GoDefault => "go_default",
        SemanticImportKind::GoNamed => "go_named",
        SemanticImportKind::GoDot => "go_dot",
        SemanticImportKind::GoBlank => "go_blank",
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
        ExportKind::ReExport => "re_export",
        ExportKind::Star => "star",
        ExportKind::Generated => "generated",
        ExportKind::Unknown => "unknown",
    }
}

fn alias_kind_label(kind: AliasKind) -> &'static str {
    match kind {
        AliasKind::Import => "import",
        AliasKind::Export => "export",
        AliasKind::ReExport => "re_export",
        AliasKind::Generated => "generated",
        AliasKind::TypeOnly => "type_only",
        AliasKind::Unknown => "unknown",
    }
}

fn resolution_step_kind_label(kind: ResolutionStepKind) -> &'static str {
    match kind {
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
}
