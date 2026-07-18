use crate::analysis_kernel::{FactFamily, stable_key_from_parts};
use crate::analysis_plan::AnalysisPlan;
use crate::config::LoadedConfig;
use crate::core::{
    AnalysisDb, CapabilitySupport, CapabilitySupportStatus, DefinitionKind, FileId, Language,
    ModuleNodeId, ReferenceKind, ResolutionStatus, ResolvedImportFact, SourceFile, Span,
    SymbolId as PolintSymbolId, SymbolKind, SymbolNamespace, SymbolPrecision, span_from_byte_range,
};
use crate::diagnostics::{Diagnostic, TextRange};
use crate::symbol_graph::LanguageSymbolOutput;
use crate::symbol_graph::model::{
    DefinitionDraft, ReferenceDraft, SymbolDraft, SymbolGraphBuilder,
};
use crate::symbol_graph::semantic::{
    AliasFact, AliasId, AliasKind, ExportFact, ExportId, ExportKind, ResolutionFact, ResolutionId,
    ResolutionStepKind, ScopeFact, ScopeId, ScopeKind, SemanticImportFact, SemanticImportId,
    SemanticImportKind, SemanticIndexBuilder, SemanticIndexOutput, SemanticStatus, StableExportId,
    StableExportIdentity,
};
use crate::symbol_graph::stable_id::{StableReferenceKey, StableSymbolKey};
use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    Argument, AssignmentTarget, BindingIdentifier, BindingPattern, Declaration,
    ExportDefaultDeclarationKind, Expression, ImportDeclarationSpecifier, ImportOrExportKind,
    ModuleExportName, Program, Statement,
};
use oxc_parser::Parser;
use oxc_semantic::{
    AstNodes, Reference, ReferenceFlags, ReferenceId as OxcReferenceId, Scoping, SemanticBuilder,
    SymbolFlags, SymbolId as OxcSymbolId,
};
use oxc_span::{GetSpan, SourceType};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const TS_SYMBOL_GRAPH_CAPABILITIES: &[&str] = &["symbols", "references"];
const SYMBOL_FACTS_DOCS_PATH: &str = "docs/facts/symbols-and-references.md";

#[derive(Debug, Clone)]
struct TsFileSymbolSummary {
    file: FileId,
    import_aliases: Vec<ImportAlias>,
    exports: BTreeMap<String, Vec<PolintSymbolId>>,
}

impl TsFileSymbolSummary {
    fn new(file: FileId) -> Self {
        Self {
            file,
            import_aliases: Vec::new(),
            exports: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct ImportAlias {
    file: FileId,
    language: Language,
    file_key: String,
    import_path: String,
    import_source_span: Span,
    local_name: String,
    local_span: Span,
    target: ImportAliasTarget,
    namespace: SymbolNamespace,
}

#[derive(Debug, Clone)]
struct ExportSymbolIdentity {
    stable_key: String,
    namespace: SymbolNamespace,
}

type EmittedExports = BTreeMap<String, ExportId>;
type EmittedSemanticImports = BTreeMap<(String, Option<ScopeId>), SemanticImportId>;
type EmittedAliases = BTreeMap<String, AliasId>;

#[derive(Debug, Default)]
struct TsSemanticEmissions {
    imports: EmittedSemanticImports,
    exports: EmittedExports,
    stable_exports: BTreeSet<String>,
    aliases: EmittedAliases,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImportAliasTarget {
    Named(String),
    Default,
    Namespace,
}

#[derive(Debug, Clone, Copy)]
enum ReExportTarget<'a> {
    Named {
        module: &'a str,
        imported_name: &'a str,
    },
    Module {
        module: &'a str,
    },
}

pub(crate) fn derive_ts_symbols(
    builder: &mut SymbolGraphBuilder,
    db: &AnalysisDb,
    _loaded: &LoadedConfig,
    plan: &AnalysisPlan,
) -> LanguageSymbolOutput {
    let files = ts_files(db);
    if files.is_empty() {
        return LanguageSymbolOutput::default();
    }

    let mut output = LanguageSymbolOutput::default();
    let requests_symbols = plan.requests_capability("symbols");
    let requests_references = plan.requests_capability("references");
    let languages = files
        .iter()
        .map(|file| file.language)
        .collect::<BTreeSet<_>>();
    for language in languages {
        output
            .capability_support
            .extend(supported_language_support(plan, language));
    }

    if !requests_symbols && !requests_references {
        return output;
    }

    let mut summaries = Vec::new();
    for file in files {
        summaries.push(derive_ts_file_symbols(
            builder,
            &mut output,
            file,
            requests_references,
        ));
    }
    if requests_references {
        add_import_alias_references(builder, db, &summaries);
    }

    output
}

fn ts_files(db: &AnalysisDb) -> Vec<&SourceFile> {
    let mut files = db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files
}

fn supported_language_support(plan: &AnalysisPlan, language: Language) -> Vec<CapabilitySupport> {
    TS_SYMBOL_GRAPH_CAPABILITIES
        .iter()
        .filter(|capability| plan.requests_capability(capability))
        .filter_map(|capability| {
            let base = plan
                .support_view()
                .entries()
                .iter()
                .find(|entry| entry.capability == *capability)?;
            Some(CapabilitySupport {
                capability: (*capability).to_string(),
                language: Some(language),
                status: CapabilitySupportStatus::Supported,
                rules: base.rules.clone(),
                reason: None,
                hint: None,
                docs_path: Some(SYMBOL_FACTS_DOCS_PATH.to_string()),
            })
        })
        .collect()
}

fn derive_ts_file_symbols(
    builder: &mut SymbolGraphBuilder,
    output: &mut LanguageSymbolOutput,
    file: &SourceFile,
    requests_references: bool,
) -> TsFileSymbolSummary {
    let source = file.source.as_ref();
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, parse_source_type(&file.path)).parse();
    output
        .diagnostics
        .extend(oxc_diagnostics(file, &parsed.errors, "parser/ts"));

    if parsed.panicked && parsed.program.body.is_empty() {
        output.diagnostics.push(Diagnostic::error(
            "parser/ts",
            file.relative_path.clone(),
            TextRange::point(1, 1),
            "TS/JS parser reported a syntax error",
        ));
        return TsFileSymbolSummary::new(file.id);
    }

    let import_aliases = collect_import_aliases(file, source, &parsed.program);
    let semantic_return = SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&parsed.program);
    output.diagnostics.extend(oxc_diagnostics(
        file,
        &semantic_return.errors,
        "semantic/ts",
    ));

    let semantic = semantic_return.semantic;
    let scoping = semantic.scoping();
    let nodes = semantic.nodes();
    output.semantic.extend(derive_ts_semantic_index(
        file,
        source,
        &parsed.program,
        scoping,
        nodes,
        requests_references,
    ));
    let export_names = collect_export_names(&parsed.program, scoping);
    let parameter_symbols = collect_parameter_symbols(nodes);
    let mut summary = TsFileSymbolSummary {
        file: file.id,
        import_aliases,
        exports: BTreeMap::new(),
    };
    let mut symbol_ids = BTreeMap::<OxcSymbolId, PolintSymbolId>::new();
    let mut oxc_symbols = scoping.symbol_ids().collect::<Vec<_>>();
    oxc_symbols.sort_by(|left, right| {
        let left_span = scoping.symbol_span(*left);
        let right_span = scoping.symbol_span(*right);
        (
            left_span.start,
            left_span.end,
            scoping.symbol_name(*left),
            left.index(),
        )
            .cmp(&(
                right_span.start,
                right_span.end,
                scoping.symbol_name(*right),
                right.index(),
            ))
    });

    for oxc_symbol in oxc_symbols {
        let name = scoping.symbol_name(oxc_symbol).to_string();
        let flags = scoping.symbol_flags(oxc_symbol);
        let kind = symbol_kind(flags, parameter_symbols.contains(&oxc_symbol));
        let namespace = symbol_namespace(flags);
        let owner_chain = owner_chain(scoping, nodes, scoping.symbol_scope_id(oxc_symbol));
        let qualified_name = qualified_name(&file.relative_path, &owner_chain, &name);
        let primary_span = span_from_oxc(file.id, source, scoping.symbol_span(oxc_symbol));
        let is_exported = export_names.contains_key(&oxc_symbol);

        let symbol = builder.add_symbol(SymbolDraft {
            language: file.language,
            name: name.clone(),
            qualified_name: qualified_name.clone(),
            kind,
            namespace,
            file: Some(file.id),
            package: None,
            module: None,
            owner: None,
            module_key: None,
            package_key: None,
            file_key: Some(file.relative_path.clone()),
            owner_chain,
            primary_span: Some(primary_span),
            is_exported,
            precision: SymbolPrecision::ExactLocal,
        });
        symbol_ids.insert(oxc_symbol, symbol);
        if let Some(names) = export_names.get(&oxc_symbol) {
            for export_name in names {
                summary
                    .exports
                    .entry(export_name.clone())
                    .or_default()
                    .push(symbol);
            }
        }

        add_symbol_definitions(
            builder,
            file,
            source,
            scoping,
            oxc_symbol,
            symbol,
            name,
            qualified_name,
            namespace,
            is_exported,
        );
    }

    if requests_references {
        add_ts_references(builder, file, source, scoping, nodes, &symbol_ids);
    }

    summary
}

fn derive_ts_semantic_index(
    file: &SourceFile,
    source: &str,
    program: &Program<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    requests_references: bool,
) -> SemanticIndexOutput {
    let mut builder = SemanticIndexBuilder::new();
    let mut ids_by_stable_key = BTreeMap::<String, ScopeId>::new();
    let export_symbol_identities = export_symbol_identities(file, source, program, scoping, nodes);

    for scope_id in scoping.scope_descendants_from_root() {
        let node_id = scoping.get_node_id(scope_id);
        let kind = scope_kind_for_ast(nodes.kind(node_id));
        let stable_key = scope_stable_key(file, scoping, nodes, scope_id);
        if ids_by_stable_key.contains_key(&stable_key) {
            continue;
        }
        let parent = scoping.scope_parent_id(scope_id).and_then(|parent| {
            let parent_key = scope_stable_key(file, scoping, nodes, parent);
            ids_by_stable_key.get(&parent_key).copied()
        });
        let id = builder.add_scope(ScopeFact {
            id: ScopeId(0),
            language: file.language,
            file: Some(file.id),
            package: None,
            module: None,
            parent,
            scope_path: scope_path_for_scope(file, scoping, nodes, scope_id),
            kind,
            stable_key: stable_key.clone(),
            status: SemanticStatus::Resolved,
        });
        ids_by_stable_key.insert(stable_key, id);
    }

    let module_scope_id = ids_by_stable_key
        .get(&scope_stable_key(
            file,
            scoping,
            nodes,
            scoping.root_scope_id(),
        ))
        .copied();
    let mut semantic_nodes = nodes
        .iter_enumerated()
        .filter_map(|(node_id, node)| {
            let kind = semantic_scope_kind_for_ast(node.kind())?;
            let span = node.kind().span();
            Some((span.start, span.end, node_id, kind))
        })
        .collect::<Vec<_>>();
    semantic_nodes.sort_by_key(|(start, end, _, kind)| (*start, *end, *kind));

    for (_, _, node_id, kind) in semantic_nodes {
        let scope_path = scope_path_for_node(file, nodes, node_id);
        let stable_key = ScopeFact::stable_key_for(
            file.language,
            &scope_path,
            Some(file.relative_path.clone()),
            None,
            None,
            kind,
            SemanticStatus::Resolved,
        );
        if ids_by_stable_key.contains_key(&stable_key) {
            continue;
        }
        let parent = nearest_semantic_scope_parent(file, nodes, node_id, &ids_by_stable_key)
            .or(module_scope_id);
        let id = builder.add_scope(ScopeFact {
            id: ScopeId(0),
            language: file.language,
            file: Some(file.id),
            package: None,
            module: None,
            parent,
            scope_path,
            kind,
            stable_key: stable_key.clone(),
            status: SemanticStatus::Resolved,
        });
        ids_by_stable_key.insert(stable_key, id);
    }

    add_ts_semantic_import_export_rows(
        &mut builder,
        file,
        source,
        program,
        module_scope_id,
        scoping,
        &export_symbol_identities,
    );
    if requests_references {
        add_ts_semantic_resolution_rows(&mut builder, file, source, scoping, nodes);
    }

    builder.finish()
}

fn export_symbol_identities(
    file: &SourceFile,
    source: &str,
    program: &Program<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) -> BTreeMap<(OxcSymbolId, String), ExportSymbolIdentity> {
    let export_names = collect_export_names(program, scoping);
    let parameter_symbols = collect_parameter_symbols(nodes);
    let mut identities = BTreeMap::new();

    for (oxc_symbol, export_names) in export_names {
        let stable_key =
            ts_symbol_stable_key(file, source, scoping, nodes, oxc_symbol, &parameter_symbols);
        let namespace = symbol_namespace(scoping.symbol_flags(oxc_symbol));
        for export_name in export_names {
            identities.insert(
                (oxc_symbol, export_name),
                ExportSymbolIdentity {
                    stable_key: stable_key.clone(),
                    namespace,
                },
            );
        }
    }

    identities
}

fn ts_symbol_stable_key(
    file: &SourceFile,
    source: &str,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    oxc_symbol: OxcSymbolId,
    parameter_symbols: &BTreeSet<OxcSymbolId>,
) -> String {
    ts_symbol_stable_key_input(file, source, scoping, nodes, oxc_symbol, parameter_symbols)
        .stable_key()
}

fn ts_symbol_stable_key_input(
    file: &SourceFile,
    source: &str,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    oxc_symbol: OxcSymbolId,
    parameter_symbols: &BTreeSet<OxcSymbolId>,
) -> StableSymbolKey {
    let name = scoping.symbol_name(oxc_symbol).to_string();
    let flags = scoping.symbol_flags(oxc_symbol);
    let kind = symbol_kind(flags, parameter_symbols.contains(&oxc_symbol));
    let namespace = symbol_namespace(flags);
    let owner_chain = owner_chain(scoping, nodes, scoping.symbol_scope_id(oxc_symbol));
    let primary_span = span_from_oxc(file.id, source, scoping.symbol_span(oxc_symbol));

    StableSymbolKey::new(
        file.language,
        None,
        None,
        Some(file.relative_path.clone()),
        owner_chain,
        namespace,
        kind,
        name,
        Some(primary_span),
    )
}

fn add_ts_semantic_import_export_rows(
    builder: &mut SemanticIndexBuilder,
    file: &SourceFile,
    source: &str,
    program: &Program<'_>,
    module_scope: Option<ScopeId>,
    scoping: &Scoping,
    export_symbol_identities: &BTreeMap<(OxcSymbolId, String), ExportSymbolIdentity>,
) {
    let mut emissions = TsSemanticEmissions::default();
    for statement in &program.body {
        match statement {
            Statement::ImportDeclaration(import) => {
                add_import_declaration_rows(builder, &mut emissions, file, module_scope, import);
            }
            Statement::ExportNamedDeclaration(export) => {
                add_export_named_rows(
                    builder,
                    file,
                    module_scope,
                    scoping,
                    export_symbol_identities,
                    &mut emissions,
                    export,
                );
            }
            Statement::ExportDefaultDeclaration(export) => {
                let identity =
                    default_export_symbol(&export.declaration, scoping).and_then(|symbol| {
                        export_symbol_identities.get(&(symbol, "default".to_string()))
                    });
                add_export_row(
                    builder,
                    &mut emissions,
                    file,
                    module_scope,
                    "default".to_string(),
                    identity
                        .map(|identity| identity.namespace)
                        .unwrap_or(SymbolNamespace::Value),
                    ExportKind::Default,
                    identity.map(|identity| identity.stable_key.clone()),
                    SemanticStatus::Resolved,
                );
                collect_commonjs_from_export_default(
                    builder,
                    &mut emissions,
                    file,
                    source,
                    module_scope,
                    export,
                );
            }
            Statement::ExportAllDeclaration(export) => {
                let export_name = export
                    .exported
                    .as_ref()
                    .map(module_export_name_text)
                    .unwrap_or_else(|| module_export_source_name(export.source.value.as_str()));
                let is_type = matches!(export.export_kind, ImportOrExportKind::Type);
                let (namespace, kind, alias_kind) = if export.exported.is_some() {
                    (
                        if is_type {
                            SymbolNamespace::Type
                        } else {
                            SymbolNamespace::Namespace
                        },
                        ExportKind::Namespace,
                        if is_type { "type" } else { "namespace" },
                    )
                } else {
                    (
                        if is_type {
                            SymbolNamespace::Type
                        } else {
                            SymbolNamespace::Module
                        },
                        ExportKind::StarReexport,
                        if is_type { "type-star" } else { "star" },
                    )
                };
                let target_stable_key = ts_reexport_target_stable_key(ReExportTarget::Module {
                    module: export.source.value.as_str(),
                });
                add_export_row(
                    builder,
                    &mut emissions,
                    file,
                    module_scope,
                    export_name.clone(),
                    namespace,
                    kind,
                    None,
                    SemanticStatus::Unresolved,
                );
                add_alias_row(
                    builder,
                    &mut emissions,
                    file,
                    format!("reexport:{alias_kind}:{export_name}"),
                    vec![target_stable_key],
                    AliasKind::ReExport,
                    SemanticStatus::Unresolved,
                );
            }
            Statement::ExpressionStatement(statement) => {
                collect_commonjs_from_expression(
                    builder,
                    &mut emissions,
                    file,
                    source,
                    module_scope,
                    &statement.expression,
                );
            }
            Statement::VariableDeclaration(variable) => {
                for declarator in &variable.declarations {
                    if let Some(init) = &declarator.init {
                        collect_commonjs_from_expression(
                            builder,
                            &mut emissions,
                            file,
                            source,
                            module_scope,
                            init,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

fn add_import_declaration_rows(
    builder: &mut SemanticIndexBuilder,
    emissions: &mut TsSemanticEmissions,
    file: &SourceFile,
    module_scope: Option<ScopeId>,
    import: &oxc_ast::ast::ImportDeclaration<'_>,
) {
    let import_path = import.source.value.to_string();
    let status = import_status_for_path(&import_path);
    let Some(specifiers) = &import.specifiers else {
        add_import_row(
            builder,
            emissions,
            file,
            module_scope,
            import_path,
            None,
            None,
            SymbolNamespace::Module,
            SemanticImportKind::SideEffect,
            status,
        );
        return;
    };

    for specifier in specifiers {
        match specifier {
            ImportDeclarationSpecifier::ImportDefaultSpecifier(default) => {
                let local_name = default.local.name.to_string();
                add_import_row(
                    builder,
                    emissions,
                    file,
                    module_scope,
                    import_path.clone(),
                    Some(local_name.clone()),
                    Some("default".to_string()),
                    SymbolNamespace::Value,
                    SemanticImportKind::StaticDefault,
                    status,
                );
                add_import_lookup_rows(builder, file, &local_name, &import_path, status);
            }
            ImportDeclarationSpecifier::ImportNamespaceSpecifier(namespace) => {
                let local_name = namespace.local.name.to_string();
                add_import_row(
                    builder,
                    emissions,
                    file,
                    module_scope,
                    import_path.clone(),
                    Some(local_name.clone()),
                    Some("*".to_string()),
                    SymbolNamespace::Namespace,
                    SemanticImportKind::StaticNamespace,
                    status,
                );
                add_import_lookup_rows(builder, file, &local_name, &import_path, status);
            }
            ImportDeclarationSpecifier::ImportSpecifier(named) => {
                let is_type_import = matches!(import.import_kind, ImportOrExportKind::Type)
                    || matches!(named.import_kind, ImportOrExportKind::Type);
                let local_name = named.local.name.to_string();
                add_import_row(
                    builder,
                    emissions,
                    file,
                    module_scope,
                    import_path.clone(),
                    Some(local_name.clone()),
                    Some(module_export_name_text(&named.imported)),
                    if is_type_import {
                        SymbolNamespace::Type
                    } else {
                        SymbolNamespace::Value
                    },
                    if is_type_import {
                        SemanticImportKind::TypeOnly
                    } else {
                        SemanticImportKind::StaticNamed
                    },
                    status,
                );
                add_alias_row(
                    builder,
                    emissions,
                    file,
                    format!("import:{local_name}"),
                    Vec::new(),
                    if is_type_import {
                        AliasKind::TypeOnly
                    } else {
                        AliasKind::Import
                    },
                    status,
                );
                add_import_lookup_rows(builder, file, &local_name, &import_path, status);
            }
        }
    }
}

fn add_export_named_rows(
    builder: &mut SemanticIndexBuilder,
    file: &SourceFile,
    module_scope: Option<ScopeId>,
    scoping: &Scoping,
    export_symbol_identities: &BTreeMap<(OxcSymbolId, String), ExportSymbolIdentity>,
    emissions: &mut TsSemanticEmissions,
    export: &oxc_ast::ast::ExportNamedDeclaration<'_>,
) {
    if let Some(source) = &export.source {
        for specifier in &export.specifiers {
            let is_type = matches!(export.export_kind, ImportOrExportKind::Type)
                || matches!(specifier.export_kind, ImportOrExportKind::Type);
            let namespace = if is_type {
                SymbolNamespace::Type
            } else {
                SymbolNamespace::Value
            };
            let export_name = module_export_name_text(&specifier.exported);
            let imported_name = module_export_name_text(&specifier.local);
            let target_stable_key = ts_reexport_target_stable_key(ReExportTarget::Named {
                module: source.value.as_str(),
                imported_name: &imported_name,
            });
            add_export_row(
                builder,
                emissions,
                file,
                module_scope,
                export_name.clone(),
                namespace,
                ExportKind::ReExport,
                None,
                SemanticStatus::Unresolved,
            );
            add_alias_row(
                builder,
                emissions,
                file,
                format!(
                    "reexport:{}:{export_name}",
                    if is_type { "type" } else { "value" }
                ),
                vec![target_stable_key],
                AliasKind::ReExport,
                SemanticStatus::Unresolved,
            );
        }
        return;
    }

    if let Some(declaration) = &export.declaration {
        let mut declarations = Vec::new();
        collect_declaration_symbols(declaration, &mut declarations);
        for (symbol, name) in declarations {
            let identity = export_symbol_identities.get(&(symbol, name.clone()));
            add_export_row(
                builder,
                emissions,
                file,
                module_scope,
                name.clone(),
                identity
                    .map(|identity| identity.namespace)
                    .unwrap_or_else(|| symbol_namespace(scoping.symbol_flags(symbol))),
                ExportKind::Named,
                identity.map(|identity| identity.stable_key.clone()),
                SemanticStatus::Resolved,
            );
        }
    }
    for specifier in &export.specifiers {
        let export_name = module_export_name_text(&specifier.exported);
        let identity = module_export_name_symbol(&specifier.local, scoping)
            .and_then(|symbol| export_symbol_identities.get(&(symbol, export_name.clone())));
        let namespace = if matches!(export.export_kind, ImportOrExportKind::Type)
            || matches!(specifier.export_kind, ImportOrExportKind::Type)
        {
            SymbolNamespace::Type
        } else {
            identity
                .map(|identity| identity.namespace)
                .unwrap_or(SymbolNamespace::Value)
        };
        add_export_row(
            builder,
            emissions,
            file,
            module_scope,
            export_name.clone(),
            namespace,
            ExportKind::Named,
            identity.map(|identity| identity.stable_key.clone()),
            SemanticStatus::Resolved,
        );
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "semantic import rows mirror the normalized internal fact fields"
)]
fn add_import_row(
    builder: &mut SemanticIndexBuilder,
    emissions: &mut TsSemanticEmissions,
    file: &SourceFile,
    scope: Option<ScopeId>,
    import_path: String,
    local_name: Option<String>,
    imported_name: Option<String>,
    namespace: SymbolNamespace,
    kind: SemanticImportKind,
    status: SemanticStatus,
) -> SemanticImportId {
    let stable_key = ts_semantic_import_stable_key(
        file,
        &import_path,
        local_name.as_deref(),
        imported_name.as_deref(),
        namespace,
        kind,
        status,
    );
    let emission_key = (stable_key.clone(), scope);
    if let Some(existing) = emissions.imports.get(&emission_key) {
        return *existing;
    }
    let import = builder.add_semantic_import(SemanticImportFact {
        id: SemanticImportId(0),
        language: file.language,
        file: Some(file.id),
        package: None,
        module: None,
        scope,
        import_path,
        local_name,
        imported_name,
        namespace,
        kind,
        stable_key,
        status,
    });
    emissions.imports.insert(emission_key, import);
    import
}

#[expect(
    clippy::too_many_arguments,
    reason = "semantic export rows mirror the normalized internal fact fields"
)]
fn add_export_row(
    builder: &mut SemanticIndexBuilder,
    emissions: &mut TsSemanticEmissions,
    file: &SourceFile,
    scope: Option<ScopeId>,
    export_name: String,
    namespace: SymbolNamespace,
    kind: ExportKind,
    symbol_stable_key: Option<String>,
    status: SemanticStatus,
) -> ExportId {
    let stable_key = ts_export_stable_key(file, &export_name, namespace, kind, status);
    let export = if let Some(existing) = emissions.exports.get(&stable_key) {
        *existing
    } else {
        let export = builder.add_export_identity(ExportFact {
            id: ExportId(0),
            language: file.language,
            file: Some(file.id),
            package: None,
            module: None,
            scope,
            symbol: None,
            export_name: export_name.clone(),
            namespace,
            kind,
            stable_key: stable_key.clone(),
            status,
        });
        emissions.exports.insert(stable_key, export);
        export
    };
    if status == SemanticStatus::Resolved
        && matches!(
            kind,
            ExportKind::Named | ExportKind::Default | ExportKind::Namespace
        )
        && let Some(symbol_stable_key) = symbol_stable_key
    {
        let identity = stable_export_identity(
            file,
            export,
            &export_name,
            namespace,
            symbol_stable_key,
            status,
        );
        if emissions.stable_exports.insert(identity.stable_key.clone()) {
            builder.add_stable_export(identity);
        }
    }
    export
}

fn add_alias_row(
    builder: &mut SemanticIndexBuilder,
    emissions: &mut TsSemanticEmissions,
    file: &SourceFile,
    source_symbol_stable_key: String,
    target_symbol_stable_keys: Vec<String>,
    kind: AliasKind,
    status: SemanticStatus,
) -> AliasId {
    let stable_key = ts_alias_stable_key(
        file,
        &source_symbol_stable_key,
        &target_symbol_stable_keys,
        kind,
        status,
    );
    if let Some(existing) = emissions.aliases.get(&stable_key) {
        return *existing;
    }
    let emission_key = stable_key.clone();
    let alias = builder.add_alias(AliasFact {
        id: AliasId(0),
        language: file.language,
        file: Some(file.id),
        package: None,
        module: None,
        source_symbol_stable_key,
        target_symbol_stable_keys,
        kind,
        stable_key,
        status,
    });
    emissions.aliases.insert(emission_key, alias);
    alias
}

fn add_resolution_row(
    builder: &mut SemanticIndexBuilder,
    file: &SourceFile,
    source_stable_key: String,
    target_stable_keys: Vec<String>,
    step: ResolutionStepKind,
    status: SemanticStatus,
) -> ResolutionId {
    let stable_key =
        ts_resolution_stable_key(file, &source_stable_key, &target_stable_keys, step, status);
    builder.add_resolution(ResolutionFact {
        id: ResolutionId(0),
        language: file.language,
        file: Some(file.id),
        package: None,
        module: None,
        source_stable_key,
        target_stable_keys,
        step,
        stable_key,
        status,
    })
}

fn add_import_lookup_rows(
    builder: &mut SemanticIndexBuilder,
    file: &SourceFile,
    local_name: &str,
    import_path: &str,
    status: SemanticStatus,
) {
    let source = format!("{}::import::{local_name}", file.relative_path);
    add_resolution_row(
        builder,
        file,
        source.clone(),
        Vec::new(),
        ResolutionStepKind::ImportAliasLookup,
        status,
    );
    add_resolution_row(
        builder,
        file,
        source,
        vec![format!("module:{import_path}")],
        ResolutionStepKind::ModuleLookup,
        status,
    );
}

fn stable_export_identity(
    file: &SourceFile,
    export: ExportId,
    export_name: &str,
    namespace: SymbolNamespace,
    symbol_stable_key: String,
    status: SemanticStatus,
) -> StableExportIdentity {
    let mut identity = StableExportIdentity {
        id: StableExportId(0),
        export,
        language: file.language,
        package_key: None,
        module_key: Some(file.relative_path.clone()),
        export_name: export_name.to_string(),
        namespace,
        symbol_stable_key,
        generated_discriminator: Some("native".to_string()),
        stable_key: String::new(),
        status,
    };
    identity.stable_key = identity.computed_stable_key();
    identity
}

fn ts_semantic_import_stable_key(
    file: &SourceFile,
    import_path: &str,
    local_name: Option<&str>,
    imported_name: Option<&str>,
    namespace: SymbolNamespace,
    kind: SemanticImportKind,
    status: SemanticStatus,
) -> String {
    stable_key_from_parts(
        FactFamily::SemanticImport,
        &[
            ("language", format!("{:?}", file.language)),
            ("file", file.relative_path.clone()),
            ("import_path", import_path.to_string()),
            ("local_name", local_name.unwrap_or("<none>").to_string()),
            (
                "imported_name",
                imported_name.unwrap_or("<none>").to_string(),
            ),
            ("namespace", format!("{namespace:?}")),
            ("kind", format!("{kind:?}")),
            ("status", format!("{status:?}")),
        ],
    )
}

fn ts_export_stable_key(
    file: &SourceFile,
    export_name: &str,
    namespace: SymbolNamespace,
    kind: ExportKind,
    status: SemanticStatus,
) -> String {
    stable_key_from_parts(
        FactFamily::Export,
        &[
            ("language", format!("{:?}", file.language)),
            ("file", file.relative_path.clone()),
            ("export_name", export_name.to_string()),
            ("namespace", format!("{namespace:?}")),
            ("kind", format!("{kind:?}")),
            ("status", format!("{status:?}")),
        ],
    )
}

fn ts_reexport_target_stable_key(target: ReExportTarget<'_>) -> String {
    match target {
        ReExportTarget::Named {
            module,
            imported_name,
        } => stable_key_from_parts(
            FactFamily::Export,
            &[
                ("target_kind", "named".to_string()),
                ("module", module.to_string()),
                ("imported_name", imported_name.to_string()),
            ],
        ),
        ReExportTarget::Module { module } => stable_key_from_parts(
            FactFamily::Export,
            &[
                ("target_kind", "module".to_string()),
                ("module", module.to_string()),
            ],
        ),
    }
}

fn ts_alias_stable_key(
    file: &SourceFile,
    source_symbol_stable_key: &str,
    target_symbol_stable_keys: &[String],
    kind: AliasKind,
    status: SemanticStatus,
) -> String {
    let mut targets = target_symbol_stable_keys.to_vec();
    targets.sort();
    targets.dedup();
    stable_key_from_parts(
        FactFamily::Alias,
        &[
            ("language", format!("{:?}", file.language)),
            ("file", file.relative_path.clone()),
            ("source", source_symbol_stable_key.to_string()),
            ("targets", targets.join("\n")),
            ("kind", format!("{kind:?}")),
            ("status", format!("{status:?}")),
        ],
    )
}

fn ts_resolution_stable_key(
    file: &SourceFile,
    source_stable_key: &str,
    target_stable_keys: &[String],
    step: ResolutionStepKind,
    status: SemanticStatus,
) -> String {
    let mut targets = target_stable_keys.to_vec();
    targets.sort();
    targets.dedup();
    stable_key_from_parts(
        FactFamily::Resolution,
        &[
            ("language", format!("{:?}", file.language)),
            ("file", file.relative_path.clone()),
            ("source", source_stable_key.to_string()),
            ("targets", targets.join("\n")),
            ("step", format!("{step:?}")),
            ("status", format!("{status:?}")),
        ],
    )
}

fn collect_commonjs_from_export_default(
    builder: &mut SemanticIndexBuilder,
    emissions: &mut TsSemanticEmissions,
    file: &SourceFile,
    source: &str,
    module_scope: Option<ScopeId>,
    export: &oxc_ast::ast::ExportDefaultDeclaration<'_>,
) {
    if let ExportDefaultDeclarationKind::CallExpression(call) = &export.declaration {
        collect_commonjs_from_call(builder, emissions, file, source, module_scope, call);
    }
}

fn collect_commonjs_from_expression(
    builder: &mut SemanticIndexBuilder,
    emissions: &mut TsSemanticEmissions,
    file: &SourceFile,
    source: &str,
    module_scope: Option<ScopeId>,
    expression: &Expression<'_>,
) {
    match expression {
        Expression::CallExpression(call) => {
            collect_commonjs_from_call(builder, emissions, file, source, module_scope, call);
            for argument in &call.arguments {
                if let Some(expression) = argument.as_expression() {
                    collect_commonjs_from_expression(
                        builder,
                        emissions,
                        file,
                        source,
                        module_scope,
                        expression,
                    );
                }
            }
        }
        Expression::ImportExpression(import) => {
            let (import_path, status) = match &import.source {
                Expression::StringLiteral(path) => (
                    path.value.to_string(),
                    import_status_for_path(path.value.as_str()),
                ),
                _ => ("<dynamic>".to_string(), SemanticStatus::Dynamic),
            };
            add_import_row(
                builder,
                emissions,
                file,
                module_scope,
                import_path,
                None,
                None,
                SymbolNamespace::Module,
                SemanticImportKind::DynamicImport,
                status,
            );
        }
        Expression::AssignmentExpression(assignment) => {
            if let Some(kind) = commonjs_export_kind(&assignment.left) {
                add_export_row(
                    builder,
                    emissions,
                    file,
                    module_scope,
                    commonjs_export_name(&assignment.left),
                    SymbolNamespace::Value,
                    kind,
                    None,
                    SemanticStatus::Unsupported,
                );
            }
            collect_commonjs_from_expression(
                builder,
                emissions,
                file,
                source,
                module_scope,
                &assignment.right,
            );
        }
        Expression::AwaitExpression(expression) => {
            collect_commonjs_from_expression(
                builder,
                emissions,
                file,
                source,
                module_scope,
                &expression.argument,
            );
        }
        Expression::ParenthesizedExpression(expression) => {
            collect_commonjs_from_expression(
                builder,
                emissions,
                file,
                source,
                module_scope,
                &expression.expression,
            );
        }
        Expression::TSAsExpression(expression) => {
            collect_commonjs_from_expression(
                builder,
                emissions,
                file,
                source,
                module_scope,
                &expression.expression,
            );
        }
        Expression::TSSatisfiesExpression(expression) => {
            collect_commonjs_from_expression(
                builder,
                emissions,
                file,
                source,
                module_scope,
                &expression.expression,
            );
        }
        Expression::TSNonNullExpression(expression) => {
            collect_commonjs_from_expression(
                builder,
                emissions,
                file,
                source,
                module_scope,
                &expression.expression,
            );
        }
        Expression::TSTypeAssertion(expression) => {
            collect_commonjs_from_expression(
                builder,
                emissions,
                file,
                source,
                module_scope,
                &expression.expression,
            );
        }
        _ => {
            let _ = source;
        }
    }
}

fn collect_commonjs_from_call(
    builder: &mut SemanticIndexBuilder,
    emissions: &mut TsSemanticEmissions,
    file: &SourceFile,
    _source: &str,
    module_scope: Option<ScopeId>,
    call: &oxc_ast::ast::CallExpression<'_>,
) {
    if callee_text(&call.callee).as_deref() != Some("require") {
        return;
    }
    let (import_path, status) = match call.arguments.first() {
        Some(Argument::StringLiteral(path)) => (
            path.value.to_string(),
            import_status_for_path(path.value.as_str()),
        ),
        _ => ("<dynamic>".to_string(), SemanticStatus::Dynamic),
    };
    add_import_row(
        builder,
        emissions,
        file,
        module_scope,
        import_path,
        None,
        None,
        SymbolNamespace::Module,
        SemanticImportKind::CommonJsRequire,
        status,
    );
}

fn commonjs_export_kind(target: &AssignmentTarget<'_>) -> Option<ExportKind> {
    match target {
        AssignmentTarget::StaticMemberExpression(member)
            if expression_text(&member.object).as_deref() == Some("module")
                && member.property.name == "exports" =>
        {
            Some(ExportKind::CommonJsModuleExports)
        }
        AssignmentTarget::StaticMemberExpression(member)
            if expression_text(&member.object).as_deref() == Some("exports") =>
        {
            Some(ExportKind::CommonJsExportsProperty)
        }
        _ => None,
    }
}

fn commonjs_export_name(target: &AssignmentTarget<'_>) -> String {
    match target {
        AssignmentTarget::StaticMemberExpression(member)
            if expression_text(&member.object).as_deref() == Some("exports") =>
        {
            member.property.name.to_string()
        }
        _ => "module.exports".to_string(),
    }
}

fn import_status_for_path(path: &str) -> SemanticStatus {
    if path == "<dynamic>" {
        SemanticStatus::Dynamic
    } else if path.starts_with('.') || path.starts_with('/') || path.starts_with('#') {
        SemanticStatus::Unresolved
    } else {
        SemanticStatus::External
    }
}

fn module_export_source_name(path: &str) -> String {
    format!("* from {path}")
}

fn callee_text(expression: &Expression<'_>) -> Option<String> {
    expression_text(expression)
}

fn expression_text(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => expression_text(&member.object)
            .map(|object| format!("{object}.{}", member.property.name)),
        Expression::ParenthesizedExpression(expression) => expression_text(&expression.expression),
        Expression::TSAsExpression(expression) => expression_text(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => expression_text(&expression.expression),
        Expression::TSNonNullExpression(expression) => expression_text(&expression.expression),
        Expression::TSTypeAssertion(expression) => expression_text(&expression.expression),
        _ => None,
    }
}

fn scope_stable_key(
    file: &SourceFile,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    scope_id: oxc_semantic::ScopeId,
) -> String {
    let kind = scope_kind_for_ast(nodes.kind(scoping.get_node_id(scope_id)));
    ScopeFact::stable_key_for(
        file.language,
        &scope_path_for_scope(file, scoping, nodes, scope_id),
        Some(file.relative_path.clone()),
        None,
        None,
        kind,
        SemanticStatus::Resolved,
    )
}

#[cfg(test)]
fn reference_scope_stable_key(
    file: &SourceFile,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    reference_id: OxcReferenceId,
) -> String {
    let reference = scoping.get_reference(reference_id);
    scope_stable_key(file, scoping, nodes, reference.scope_id())
}

fn add_ts_semantic_resolution_rows(
    builder: &mut SemanticIndexBuilder,
    file: &SourceFile,
    source: &str,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) {
    let parameter_symbols = collect_parameter_symbols(nodes);
    let mut resolved = scoping
        .symbol_ids()
        .flat_map(|symbol| {
            scoping
                .get_resolved_reference_ids(symbol)
                .iter()
                .map(move |reference| (symbol, *reference))
        })
        .collect::<Vec<_>>();
    resolved.sort_by(|left, right| {
        reference_order_key(scoping, nodes, left.1)
            .cmp(&reference_order_key(scoping, nodes, right.1))
    });

    for (symbol, reference_id) in resolved {
        let reference = scoping.get_reference(reference_id);
        let target_key =
            ts_symbol_stable_key_input(file, source, scoping, nodes, symbol, &parameter_symbols);
        add_resolution_row(
            builder,
            file,
            semantic_resolved_reference_key(file, source, nodes, reference, target_key.clone()),
            vec![target_key.stable_key()],
            ResolutionStepKind::LexicalLookup,
            SemanticStatus::Resolved,
        );
    }

    let mut unresolved = scoping
        .root_unresolved_references_ids()
        .flat_map(|references| references.collect::<Vec<_>>())
        .collect::<Vec<_>>();
    unresolved.sort_by(|left, right| {
        reference_order_key(scoping, nodes, *left).cmp(&reference_order_key(scoping, nodes, *right))
    });
    unresolved.dedup();

    for reference_id in unresolved {
        let reference = scoping.get_reference(reference_id);
        let name = reference_name(nodes, reference);
        add_resolution_row(
            builder,
            file,
            semantic_unresolved_reference_key(file, source, nodes, reference, &name),
            Vec::new(),
            ResolutionStepKind::UnknownFallback,
            SemanticStatus::Unresolved,
        );
    }
}

fn semantic_resolved_reference_key(
    file: &SourceFile,
    source: &str,
    nodes: &AstNodes<'_>,
    reference: &Reference,
    target: StableSymbolKey,
) -> String {
    StableReferenceKey::resolved(
        target,
        file.relative_path.clone(),
        span_from_oxc(file.id, source, nodes.kind(reference.node_id()).span()),
    )
    .stable_key()
}

fn semantic_unresolved_reference_key(
    file: &SourceFile,
    source: &str,
    nodes: &AstNodes<'_>,
    reference: &Reference,
    name: &str,
) -> String {
    StableReferenceKey::unresolved(
        file.language,
        file.relative_path.clone(),
        name.to_string(),
        span_from_oxc(file.id, source, nodes.kind(reference.node_id()).span()),
    )
    .stable_key()
}

fn scope_path_for_scope(
    file: &SourceFile,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    scope_id: oxc_semantic::ScopeId,
) -> Vec<String> {
    let mut ancestors = scoping.scope_ancestors(scope_id).collect::<Vec<_>>();
    ancestors.reverse();
    let mut path = vec![file.relative_path.clone()];
    path.extend(ancestors.into_iter().map(|scope| {
        let kind = nodes.kind(scoping.get_node_id(scope));
        scope_path_segment(kind)
    }));
    path
}

fn scope_path_for_node(
    file: &SourceFile,
    nodes: &AstNodes<'_>,
    node_id: oxc_semantic::NodeId,
) -> Vec<String> {
    let mut ancestors = nodes
        .ancestors_enumerated(node_id)
        .filter_map(|(_, node)| semantic_scope_kind_for_ast(node.kind()).map(|_| node.kind()))
        .collect::<Vec<_>>();
    ancestors.reverse();
    let mut path = vec![file.relative_path.clone()];
    path.extend(ancestors.into_iter().map(scope_path_segment));
    path.push(scope_path_segment(nodes.kind(node_id)));
    path
}

fn nearest_semantic_scope_parent(
    file: &SourceFile,
    nodes: &AstNodes<'_>,
    node_id: oxc_semantic::NodeId,
    ids_by_stable_key: &BTreeMap<String, ScopeId>,
) -> Option<ScopeId> {
    nodes.ancestors_enumerated(node_id).find_map(|(_, node)| {
        let kind = semantic_scope_kind_for_ast(node.kind())?;
        let scope_path = scope_path_for_node(file, nodes, node.id());
        let stable_key = ScopeFact::stable_key_for(
            file.language,
            &scope_path,
            Some(file.relative_path.clone()),
            None,
            None,
            kind,
            SemanticStatus::Resolved,
        );
        ids_by_stable_key.get(&stable_key).copied()
    })
}

fn semantic_scope_kind_for_ast(kind: AstKind<'_>) -> Option<ScopeKind> {
    match kind {
        AstKind::Program(_) => Some(ScopeKind::Module),
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => Some(ScopeKind::Function),
        AstKind::Class(_) => Some(ScopeKind::Class),
        AstKind::BlockStatement(_) => Some(ScopeKind::Block),
        AstKind::CatchClause(_) => Some(ScopeKind::Catch),
        AstKind::ForStatement(_) | AstKind::ForInStatement(_) | AstKind::ForOfStatement(_) => {
            Some(ScopeKind::Loop)
        }
        AstKind::SwitchStatement(_) => Some(ScopeKind::Switch),
        AstKind::TSInterfaceDeclaration(_)
        | AstKind::TSTypeAliasDeclaration(_)
        | AstKind::TSEnumDeclaration(_) => Some(ScopeKind::Type),
        AstKind::TSModuleDeclaration(_) => Some(ScopeKind::Namespace),
        _ => None,
    }
}

fn scope_kind_for_ast(kind: AstKind<'_>) -> ScopeKind {
    semantic_scope_kind_for_ast(kind).unwrap_or(ScopeKind::Block)
}

fn scope_path_segment(kind: AstKind<'_>) -> String {
    let span = kind.span();
    format!("{}@{}-{}", ast_kind_label(kind), span.start, span.end)
}

fn add_ts_references(
    builder: &mut SymbolGraphBuilder,
    file: &SourceFile,
    source: &str,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    symbol_ids: &BTreeMap<OxcSymbolId, PolintSymbolId>,
) {
    let mut resolved = symbol_ids
        .iter()
        .flat_map(|(oxc_symbol, target)| {
            scoping
                .get_resolved_reference_ids(*oxc_symbol)
                .iter()
                .map(|reference| (*reference, *target))
        })
        .collect::<Vec<_>>();
    resolved.sort_by(|left, right| {
        reference_order_key(scoping, nodes, left.0)
            .cmp(&reference_order_key(scoping, nodes, right.0))
    });

    for (reference_id, target) in resolved {
        let reference = scoping.get_reference(reference_id);
        let draft = reference_draft(
            file,
            source,
            nodes,
            reference,
            reference_name(nodes, reference),
            SymbolPrecision::ExactLocal,
        );
        builder.add_reference(target, draft);
    }

    let mut unresolved = scoping
        .root_unresolved_references_ids()
        .flat_map(|references| references.collect::<Vec<_>>())
        .collect::<Vec<_>>();
    unresolved.sort_by(|left, right| {
        reference_order_key(scoping, nodes, *left).cmp(&reference_order_key(scoping, nodes, *right))
    });
    unresolved.dedup();

    for reference_id in unresolved {
        let reference = scoping.get_reference(reference_id);
        let draft = reference_draft(
            file,
            source,
            nodes,
            reference,
            reference_name(nodes, reference),
            SymbolPrecision::Unresolved,
        );
        builder.add_unresolved_reference(draft);
    }
}

fn add_import_alias_references(
    builder: &mut SymbolGraphBuilder,
    db: &AnalysisDb,
    summaries: &[TsFileSymbolSummary],
) {
    let exports_by_file = summaries
        .iter()
        .map(|summary| (summary.file, summary.exports.clone()))
        .collect::<BTreeMap<_, _>>();
    let imports_by_key = db
        .imports()
        .iter()
        .map(|import| {
            (
                (import.file, import.path.clone(), import.span.start_byte),
                import,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let resolved_by_import = db
        .resolved_imports()
        .iter()
        .map(|resolved| (resolved.import, resolved))
        .collect::<BTreeMap<_, _>>();
    let file_by_node = db
        .module_nodes()
        .iter()
        .filter_map(|node| node.file.map(|file| (node.id, file)))
        .collect::<BTreeMap<ModuleNodeId, FileId>>();

    for alias in summaries
        .iter()
        .flat_map(|summary| summary.import_aliases.iter())
    {
        let key = (
            alias.file,
            alias.import_path.clone(),
            alias.import_source_span.start_byte,
        );
        let Some(import) = imports_by_key.get(&key) else {
            let draft = import_alias_reference_draft(alias, SymbolPrecision::SetupMissing);
            builder.add_setup_missing_reference_draft(draft);
            continue;
        };
        let Some(resolved) = resolved_by_import.get(&import.id) else {
            let draft = import_alias_reference_draft(alias, SymbolPrecision::SetupMissing);
            builder.add_setup_missing_reference_draft(draft);
            continue;
        };

        add_import_alias_reference(builder, alias, resolved, &file_by_node, &exports_by_file);
    }
}

fn add_import_alias_reference(
    builder: &mut SymbolGraphBuilder,
    alias: &ImportAlias,
    resolved: &ResolvedImportFact,
    file_by_node: &BTreeMap<ModuleNodeId, FileId>,
    exports_by_file: &BTreeMap<FileId, BTreeMap<String, Vec<PolintSymbolId>>>,
) {
    match resolved.status {
        ResolutionStatus::Resolved => {
            let Some(target_file) = resolved
                .target_node
                .and_then(|target_node| file_by_node.get(&target_node).copied())
            else {
                let draft = import_alias_reference_draft(alias, SymbolPrecision::Unresolved);
                builder.add_unresolved_reference(draft);
                return;
            };
            let candidates = import_alias_candidates(alias, target_file, exports_by_file);
            match candidates.as_slice() {
                [target] => {
                    let draft = import_alias_reference_draft(alias, SymbolPrecision::ModuleLinked);
                    builder.add_reference(*target, draft);
                }
                [] => {
                    let draft = import_alias_reference_draft(alias, SymbolPrecision::Unresolved);
                    builder.add_unresolved_reference(draft);
                }
                _ => {
                    let draft = import_alias_reference_draft(alias, SymbolPrecision::Ambiguous);
                    builder.add_ambiguous_reference(candidates, draft);
                }
            }
        }
        ResolutionStatus::SetupMissing => {
            let draft = import_alias_reference_draft(alias, SymbolPrecision::SetupMissing);
            builder.add_setup_missing_reference_draft(draft);
        }
        ResolutionStatus::Unsupported => {
            let draft = import_alias_reference_draft(alias, SymbolPrecision::Unsupported);
            builder.add_unsupported_reference_draft(draft);
        }
        ResolutionStatus::External | ResolutionStatus::Unresolved | ResolutionStatus::Dynamic => {
            let draft = import_alias_reference_draft(alias, SymbolPrecision::Unresolved);
            builder.add_unresolved_reference(draft);
        }
    }
}

fn import_alias_candidates(
    alias: &ImportAlias,
    target_file: FileId,
    exports_by_file: &BTreeMap<FileId, BTreeMap<String, Vec<PolintSymbolId>>>,
) -> Vec<PolintSymbolId> {
    let Some(exports) = exports_by_file.get(&target_file) else {
        return Vec::new();
    };
    let mut candidates = match &alias.target {
        ImportAliasTarget::Named(name) => exports.get(name).cloned().unwrap_or_default(),
        ImportAliasTarget::Default => exports.get("default").cloned().unwrap_or_default(),
        ImportAliasTarget::Namespace => exports
            .values()
            .flat_map(|symbols| symbols.iter().copied())
            .collect::<Vec<_>>(),
    };
    candidates.sort_unstable();
    candidates.dedup();
    candidates
}

fn import_alias_reference_draft(alias: &ImportAlias, precision: SymbolPrecision) -> ReferenceDraft {
    ReferenceDraft {
        language: alias.language,
        name: alias.local_name.clone(),
        qualified_name: format!("{}::{}", alias.file_key, alias.local_name),
        kind: ReferenceKind::Import,
        namespace: alias.namespace,
        file: Some(alias.file),
        package: None,
        module: None,
        owner: None,
        file_key: alias.file_key.clone(),
        primary_span: Some(alias.local_span.clone()),
        precision,
    }
}

fn reference_order_key(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    reference_id: OxcReferenceId,
) -> (u32, u32, String) {
    let reference = scoping.get_reference(reference_id);
    let span = nodes.kind(reference.node_id()).span();
    (span.start, span.end, reference_name(nodes, reference))
}

fn reference_draft(
    file: &SourceFile,
    source: &str,
    nodes: &AstNodes<'_>,
    reference: &Reference,
    name: String,
    precision: SymbolPrecision,
) -> ReferenceDraft {
    let flags = reference.flags();
    ReferenceDraft {
        language: file.language,
        name: name.clone(),
        qualified_name: format!("{}::{name}", file.relative_path),
        kind: reference_kind(flags, nodes, reference),
        namespace: reference_namespace(flags),
        file: Some(file.id),
        package: None,
        module: None,
        owner: None,
        file_key: file.relative_path.clone(),
        primary_span: Some(span_from_oxc(
            file.id,
            source,
            nodes.kind(reference.node_id()).span(),
        )),
        precision,
    }
}

fn reference_name(nodes: &AstNodes<'_>, reference: &Reference) -> String {
    match nodes.kind(reference.node_id()) {
        AstKind::IdentifierReference(identifier) => identifier.name.to_string(),
        AstKind::BindingIdentifier(identifier) => identifier.name.to_string(),
        AstKind::IdentifierName(identifier) => identifier.name.to_string(),
        kind => {
            let span = kind.span();
            format!("<reference:{}-{}>", span.start, span.end)
        }
    }
}

fn reference_kind(
    flags: ReferenceFlags,
    nodes: &AstNodes<'_>,
    reference: &Reference,
) -> ReferenceKind {
    if flags.is_type() || flags.is_value_as_type() {
        ReferenceKind::TypeUse
    } else if is_call_reference(nodes, reference.node_id()) {
        ReferenceKind::Call
    } else if flags.is_read_write() {
        ReferenceKind::ReadWrite
    } else if flags.is_write() {
        ReferenceKind::Write
    } else if flags.is_read() {
        ReferenceKind::Read
    } else {
        ReferenceKind::Unknown
    }
}

fn is_call_reference(nodes: &AstNodes<'_>, node_id: oxc_semantic::NodeId) -> bool {
    matches!(
        nodes.parent_kind(node_id),
        AstKind::CallExpression(_) | AstKind::NewExpression(_)
    )
}

fn reference_namespace(flags: ReferenceFlags) -> SymbolNamespace {
    if flags.is_namespace() {
        SymbolNamespace::Namespace
    } else if flags.is_type() || flags.is_value_as_type() {
        SymbolNamespace::Type
    } else if flags.is_value() {
        SymbolNamespace::Value
    } else {
        SymbolNamespace::Unknown
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "definition rows need the Oxc symbol context and normalized polint identity"
)]
fn add_symbol_definitions(
    builder: &mut SymbolGraphBuilder,
    file: &SourceFile,
    source: &str,
    scoping: &Scoping,
    oxc_symbol: OxcSymbolId,
    symbol: PolintSymbolId,
    name: String,
    qualified_name: String,
    namespace: SymbolNamespace,
    is_exported: bool,
) {
    let redeclarations = scoping.symbol_redeclarations(oxc_symbol);
    if redeclarations.is_empty() {
        let flags = scoping.symbol_flags(oxc_symbol);
        let span = span_from_oxc(file.id, source, scoping.symbol_span(oxc_symbol));
        builder.add_definition(
            symbol,
            definition_draft(
                file,
                name,
                qualified_name,
                namespace,
                flags,
                Some(span),
                true,
                is_exported,
            ),
        );
        return;
    }

    for (index, redeclaration) in redeclarations.iter().enumerate() {
        builder.add_definition(
            symbol,
            definition_draft(
                file,
                name.clone(),
                qualified_name.clone(),
                namespace,
                redeclaration.flags,
                Some(span_from_oxc(file.id, source, redeclaration.span)),
                index == 0,
                is_exported,
            ),
        );
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "definition drafts mirror the public fact identity and span fields"
)]
fn definition_draft(
    file: &SourceFile,
    name: String,
    qualified_name: String,
    namespace: SymbolNamespace,
    flags: SymbolFlags,
    primary_span: Option<Span>,
    is_primary: bool,
    is_exported: bool,
) -> DefinitionDraft {
    DefinitionDraft {
        language: file.language,
        name,
        qualified_name,
        kind: definition_kind(flags, is_exported),
        namespace,
        file: Some(file.id),
        package: None,
        module: None,
        owner: None,
        file_key: file.relative_path.clone(),
        primary_span,
        is_primary,
        is_exported,
        precision: SymbolPrecision::ExactLocal,
    }
}

fn definition_kind(flags: SymbolFlags, is_exported: bool) -> DefinitionKind {
    if flags.is_import() {
        DefinitionKind::Import
    } else if is_exported {
        DefinitionKind::Export
    } else {
        DefinitionKind::Declaration
    }
}

fn symbol_kind(flags: SymbolFlags, is_parameter: bool) -> SymbolKind {
    if flags.is_import() {
        SymbolKind::Import
    } else if is_parameter {
        SymbolKind::Parameter
    } else if flags.is_interface() {
        SymbolKind::Interface
    } else if flags.is_type_alias() {
        SymbolKind::TypeAlias
    } else if flags.is_enum_member() {
        SymbolKind::EnumMember
    } else if flags.is_enum() {
        SymbolKind::Enum
    } else if flags.is_class() {
        SymbolKind::Class
    } else if flags.is_function() {
        SymbolKind::Function
    } else if flags.is_const_variable() {
        SymbolKind::Constant
    } else if flags.is_namespace_module() || flags.is_value_module() {
        SymbolKind::Namespace
    } else if flags.is_variable() {
        SymbolKind::Variable
    } else {
        SymbolKind::Unknown
    }
}

fn symbol_namespace(flags: SymbolFlags) -> SymbolNamespace {
    if flags.is_namespace_module() || flags.is_value_module() {
        SymbolNamespace::Namespace
    } else if flags.is_type() && !flags.is_value() {
        SymbolNamespace::Type
    } else if flags.is_value() {
        SymbolNamespace::Value
    } else if flags.is_type() {
        SymbolNamespace::Type
    } else {
        SymbolNamespace::Unknown
    }
}

fn collect_parameter_symbols(nodes: &AstNodes<'_>) -> BTreeSet<OxcSymbolId> {
    nodes
        .iter_enumerated()
        .filter_map(|(node_id, node)| {
            let AstKind::BindingIdentifier(identifier) = node.kind() else {
                return None;
            };
            let is_parameter = nodes.ancestor_kinds(node_id).any(|kind| {
                matches!(
                    kind,
                    AstKind::FormalParameter(_)
                        | AstKind::FormalParameterRest(_)
                        | AstKind::CatchParameter(_)
                        | AstKind::TSThisParameter(_)
                )
            });
            is_parameter.then(|| identifier.symbol_id.get()).flatten()
        })
        .collect()
}

fn owner_chain(
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    scope_id: oxc_semantic::ScopeId,
) -> Vec<String> {
    let mut scopes = scoping.scope_ancestors(scope_id).collect::<Vec<_>>();
    scopes.reverse();
    scopes
        .into_iter()
        .filter_map(|scope| {
            let node_id = scoping.get_node_id(scope);
            let kind = nodes.kind(node_id);
            if matches!(kind, AstKind::Program(_)) {
                return None;
            }
            let span = kind.span();
            Some(format!(
                "{}@{}-{}",
                ast_kind_label(kind),
                span.start,
                span.end
            ))
        })
        .collect()
}

fn ast_kind_label(kind: AstKind<'_>) -> &'static str {
    match kind {
        AstKind::Function(_) => "function",
        AstKind::ArrowFunctionExpression(_) => "arrow",
        AstKind::Class(_) => "class",
        AstKind::BlockStatement(_) => "block",
        AstKind::CatchClause(_) => "catch",
        AstKind::ForStatement(_) => "for",
        AstKind::ForInStatement(_) => "for-in",
        AstKind::ForOfStatement(_) => "for-of",
        AstKind::SwitchStatement(_) => "switch",
        AstKind::TSInterfaceDeclaration(_) => "interface",
        AstKind::TSTypeAliasDeclaration(_) => "type",
        AstKind::TSEnumDeclaration(_) => "enum",
        _ => "scope",
    }
}

fn qualified_name(file_key: &str, owner_chain: &[String], name: &str) -> String {
    if owner_chain.is_empty() {
        format!("{file_key}::{name}")
    } else {
        format!("{file_key}::{}::{name}", owner_chain.join("::"))
    }
}

fn collect_import_aliases(
    file: &SourceFile,
    source: &str,
    program: &Program<'_>,
) -> Vec<ImportAlias> {
    let mut aliases = Vec::new();

    for statement in &program.body {
        let Statement::ImportDeclaration(import) = statement else {
            continue;
        };
        let Some(specifiers) = &import.specifiers else {
            continue;
        };
        let import_path = import.source.value.to_string();
        let import_source_span = span_from_oxc(file.id, source, import.source.span);

        for specifier in specifiers {
            match specifier {
                // default import alias
                ImportDeclarationSpecifier::ImportDefaultSpecifier(default) => {
                    aliases.push(ImportAlias {
                        file: file.id,
                        language: file.language,
                        file_key: file.relative_path.clone(),
                        import_path: import_path.clone(),
                        import_source_span: import_source_span.clone(),
                        local_name: default.local.name.to_string(),
                        local_span: span_from_oxc(file.id, source, default.local.span),
                        target: ImportAliasTarget::Default,
                        namespace: SymbolNamespace::Value,
                    });
                }
                // namespace import alias
                ImportDeclarationSpecifier::ImportNamespaceSpecifier(namespace) => {
                    aliases.push(ImportAlias {
                        file: file.id,
                        language: file.language,
                        file_key: file.relative_path.clone(),
                        import_path: import_path.clone(),
                        import_source_span: import_source_span.clone(),
                        local_name: namespace.local.name.to_string(),
                        local_span: span_from_oxc(file.id, source, namespace.local.span),
                        target: ImportAliasTarget::Namespace,
                        namespace: SymbolNamespace::Namespace,
                    });
                }
                ImportDeclarationSpecifier::ImportSpecifier(named) => {
                    let is_type_import = matches!(import.import_kind, ImportOrExportKind::Type)
                        || matches!(named.import_kind, ImportOrExportKind::Type);
                    aliases.push(ImportAlias {
                        file: file.id,
                        language: file.language,
                        file_key: file.relative_path.clone(),
                        import_path: import_path.clone(),
                        import_source_span: import_source_span.clone(),
                        local_name: named.local.name.to_string(),
                        local_span: span_from_oxc(file.id, source, named.local.span),
                        target: ImportAliasTarget::Named(module_export_name_text(&named.imported)),
                        namespace: if is_type_import {
                            SymbolNamespace::Type
                        } else {
                            SymbolNamespace::Value
                        },
                    });
                }
            }
        }
    }

    aliases
}

fn collect_export_names(
    program: &Program<'_>,
    scoping: &Scoping,
) -> BTreeMap<OxcSymbolId, BTreeSet<String>> {
    let mut exports = BTreeMap::<OxcSymbolId, BTreeSet<String>>::new();

    for statement in &program.body {
        match statement {
            Statement::ExportNamedDeclaration(export) => {
                if let Some(declaration) = &export.declaration {
                    let mut declarations = Vec::new();
                    collect_declaration_symbols(declaration, &mut declarations);
                    for (symbol, name) in declarations {
                        exports.entry(symbol).or_default().insert(name);
                    }
                }

                if export.source.is_none() {
                    for specifier in &export.specifiers {
                        if let Some(symbol) = module_export_name_symbol(&specifier.local, scoping) {
                            exports
                                .entry(symbol)
                                .or_default()
                                .insert(module_export_name_text(&specifier.exported));
                        }
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let Some(symbol) = default_export_symbol(&export.declaration, scoping) {
                    exports
                        .entry(symbol)
                        .or_default()
                        .insert("default".to_string());
                }
            }
            _ => {}
        }
    }

    exports
}

fn collect_declaration_symbols(
    declaration: &Declaration<'_>,
    symbols: &mut Vec<(OxcSymbolId, String)>,
) {
    match declaration {
        Declaration::VariableDeclaration(declaration) => {
            for declarator in &declaration.declarations {
                collect_binding_pattern_symbols(&declarator.id, symbols);
            }
        }
        Declaration::FunctionDeclaration(function) => {
            if let Some(id) = &function.id {
                collect_binding_identifier_symbol(id, symbols);
            }
        }
        Declaration::ClassDeclaration(class) => {
            if let Some(id) = &class.id {
                collect_binding_identifier_symbol(id, symbols);
            }
        }
        Declaration::TSTypeAliasDeclaration(type_alias) => {
            collect_binding_identifier_symbol(&type_alias.id, symbols);
        }
        Declaration::TSInterfaceDeclaration(interface) => {
            collect_binding_identifier_symbol(&interface.id, symbols);
        }
        Declaration::TSEnumDeclaration(enumeration) => {
            collect_binding_identifier_symbol(&enumeration.id, symbols);
        }
        Declaration::TSModuleDeclaration(_)
        | Declaration::TSGlobalDeclaration(_)
        | Declaration::TSImportEqualsDeclaration(_) => {}
    }
}

fn collect_binding_pattern_symbols(
    pattern: &BindingPattern<'_>,
    symbols: &mut Vec<(OxcSymbolId, String)>,
) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            collect_binding_identifier_symbol(identifier, symbols);
        }
        BindingPattern::ObjectPattern(pattern) => {
            for property in &pattern.properties {
                collect_binding_pattern_symbols(&property.value, symbols);
            }
            if let Some(rest) = &pattern.rest {
                collect_binding_pattern_symbols(&rest.argument, symbols);
            }
        }
        BindingPattern::ArrayPattern(pattern) => {
            for element in pattern.elements.iter().flatten() {
                collect_binding_pattern_symbols(element, symbols);
            }
            if let Some(rest) = &pattern.rest {
                collect_binding_pattern_symbols(&rest.argument, symbols);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            collect_binding_pattern_symbols(&pattern.left, symbols);
        }
    }
}

fn collect_binding_identifier_symbol(
    identifier: &BindingIdentifier<'_>,
    symbols: &mut Vec<(OxcSymbolId, String)>,
) {
    if let Some(symbol) = identifier.symbol_id.get() {
        symbols.push((symbol, identifier.name.to_string()));
    }
}

fn default_export_symbol(
    declaration: &ExportDefaultDeclarationKind<'_>,
    scoping: &Scoping,
) -> Option<OxcSymbolId> {
    match declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            function.id.as_ref()?.symbol_id.get()
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => class.id.as_ref()?.symbol_id.get(),
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(interface) => {
            interface.id.symbol_id.get()
        }
        _ => declaration
            .as_expression()
            .and_then(|expression| expression_symbol(expression, scoping)),
    }
}

fn expression_symbol(expression: &Expression<'_>, scoping: &Scoping) -> Option<OxcSymbolId> {
    match expression {
        Expression::Identifier(identifier) => identifier
            .reference_id
            .get()
            .and_then(|reference| scoping.get_reference(reference).symbol_id()),
        Expression::ParenthesizedExpression(expression) => {
            expression_symbol(&expression.expression, scoping)
        }
        Expression::TSAsExpression(expression) => {
            expression_symbol(&expression.expression, scoping)
        }
        Expression::TSSatisfiesExpression(expression) => {
            expression_symbol(&expression.expression, scoping)
        }
        Expression::TSNonNullExpression(expression) => {
            expression_symbol(&expression.expression, scoping)
        }
        Expression::TSTypeAssertion(expression) => {
            expression_symbol(&expression.expression, scoping)
        }
        _ => None,
    }
}

fn module_export_name_symbol(
    name: &ModuleExportName<'_>,
    scoping: &Scoping,
) -> Option<OxcSymbolId> {
    match name {
        ModuleExportName::IdentifierReference(identifier) => identifier
            .reference_id
            .get()
            .and_then(|reference| scoping.get_reference(reference).symbol_id()),
        ModuleExportName::IdentifierName(_) | ModuleExportName::StringLiteral(_) => None,
    }
}

fn module_export_name_text(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(literal) => literal.value.to_string(),
    }
}

fn parse_source_type(path: &Path) -> SourceType {
    SourceType::from_path(path).unwrap_or_default()
}

fn span_from_oxc(file: crate::core::FileId, source: &str, span: oxc_span::Span) -> Span {
    span_from_byte_range(file, source, span.start as usize, span.end as usize)
}

fn oxc_diagnostics(
    file: &SourceFile,
    diagnostics: &[impl std::fmt::Display],
    rule_id: &str,
) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .map(|error| {
            Diagnostic::error(
                rule_id,
                file.relative_path.clone(),
                TextRange::point(1, 1),
                format!("TS/JS semantic provider reported an error: {error}"),
            )
        })
        .collect()
}

#[cfg(test)]
mod symbol_graph_ts_local_symbols {
    use super::derive_ts_symbols;
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, DefinitionFact, FileId, Language, SymbolFact, SymbolKind, SymbolNamespace,
        SymbolPrecision,
    };
    use crate::symbol_graph::model::SymbolGraphBuilder;
    use std::fs;
    use std::path::Path;

    fn add_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) -> FileId {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("test file has parent")).expect("create fixture");
        fs::write(&path, source).expect("write fixture");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn derive_ts_symbol_facts(source: &str) -> (Vec<SymbolFact>, Vec<DefinitionFact>) {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        add_file(&mut db, temp.path(), "src/component.ts", source);
        let loaded = load_config(temp.path()).expect("default config loads");
        let plan = AnalysisPlan::from_capability_names_for_test(&["symbols"]);
        let mut builder = SymbolGraphBuilder::new();

        derive_ts_symbols(&mut builder, &db, &loaded, &plan);
        let output = builder.finish();
        (output.symbols, output.definitions)
    }

    fn symbol<'a>(symbols: &'a [SymbolFact], name: &str) -> &'a SymbolFact {
        symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap_or_else(|| panic!("missing symbol `{name}` in {symbols:#?}"))
    }

    #[test]
    fn extracts_exported_and_local_symbols_from_oxc_semantic_data() {
        let (symbols, _definitions) = derive_ts_symbol_facts(
            r#"
export function exportedFn(param: string) {
    const localValue = param;
    return localValue;
}

export class Widget {}
"#,
        );

        let exported_fn = symbol(&symbols, "exportedFn");
        assert_eq!(exported_fn.language, Language::TypeScript);
        assert_eq!(exported_fn.kind, SymbolKind::Function);
        assert_eq!(exported_fn.namespace, SymbolNamespace::Value);
        assert!(exported_fn.is_exported);
        assert_eq!(exported_fn.precision, SymbolPrecision::ExactLocal);

        let widget = symbol(&symbols, "Widget");
        assert_eq!(widget.kind, SymbolKind::Class);
        assert!(widget.is_exported);

        let local_value = symbol(&symbols, "localValue");
        assert_eq!(local_value.kind, SymbolKind::Constant);
        assert!(!local_value.is_exported);

        let parameter = symbol(&symbols, "param");
        assert_eq!(parameter.kind, SymbolKind::Parameter);
    }

    #[test]
    fn declaration_merging_emits_one_symbol_with_multiple_definitions() {
        let (symbols, definitions) = derive_ts_symbol_facts(
            r#"
export interface MergeMe {
    one: string;
}

export interface MergeMe {
    two: string;
}
"#,
        );

        let merged = symbol(&symbols, "MergeMe");
        assert_eq!(merged.kind, SymbolKind::Interface);
        assert_eq!(merged.namespace, SymbolNamespace::Type);
        assert!(merged.is_exported);

        let merged_definitions = definitions
            .iter()
            .filter(|definition| definition.symbol == merged.id)
            .count();
        assert_eq!(merged_definitions, 2);
    }

    #[test]
    fn stable_symbol_ids_are_deterministic_across_repeated_extraction() {
        let source = "export const answer = 42;\n";
        let (first_symbols, first_definitions) = derive_ts_symbol_facts(source);
        let (second_symbols, second_definitions) = derive_ts_symbol_facts(source);

        let first = first_symbols
            .iter()
            .map(|symbol| (symbol.name.as_str(), symbol.id, symbol.stable_key.as_str()))
            .collect::<Vec<_>>();
        let second = second_symbols
            .iter()
            .map(|symbol| (symbol.name.as_str(), symbol.id, symbol.stable_key.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(first, second);

        let first = first_definitions
            .iter()
            .map(|definition| {
                (
                    definition.name.as_str(),
                    definition.id,
                    definition.symbol,
                    definition.stable_key.as_str(),
                )
            })
            .collect::<Vec<_>>();
        let second = second_definitions
            .iter()
            .map(|definition| {
                (
                    definition.name.as_str(),
                    definition.id,
                    definition.symbol,
                    definition.stable_key.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(first, second);
    }

    #[test]
    fn uses_repo_relative_file_keys_without_source_snippets() {
        let (symbols, _definitions) = derive_ts_symbol_facts("export const answer = 42;\n");
        let answer = symbol(&symbols, "answer");

        assert!(answer.stable_key.contains("src/component.ts"));
        assert!(!answer.stable_key.contains("export const answer"));
    }
}

#[cfg(test)]
mod symbol_graph_ts_references {
    use super::derive_ts_symbols;
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, FileId, ReferenceFact, ReferenceKind, SymbolFact, SymbolPrecision,
        SymbolResolutionStatus,
    };
    use crate::symbol_graph::model::SymbolGraphBuilder;
    use std::fs;
    use std::path::Path;

    fn add_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) -> FileId {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("test file has parent")).expect("create fixture");
        fs::write(&path, source).expect("write fixture");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn derive_ts_reference_facts(source: &str) -> (Vec<SymbolFact>, Vec<ReferenceFact>) {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        add_file(&mut db, temp.path(), "src/component.ts", source);
        let loaded = load_config(temp.path()).expect("default config loads");
        let plan = AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]);
        let mut builder = SymbolGraphBuilder::new();

        derive_ts_symbols(&mut builder, &db, &loaded, &plan);
        let output = builder.finish();
        (output.symbols, output.references)
    }

    fn symbol<'a>(symbols: &'a [SymbolFact], name: &str) -> &'a SymbolFact {
        symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap_or_else(|| panic!("missing symbol `{name}` in {symbols:#?}"))
    }

    fn references_to<'a>(
        symbols: &[SymbolFact],
        references: &'a [ReferenceFact],
        name: &str,
    ) -> Vec<&'a ReferenceFact> {
        let target = symbol(symbols, name).id;
        references
            .iter()
            .filter(|reference| reference.target == Some(target))
            .collect()
    }

    #[test]
    fn extracts_resolved_reference_kinds_from_oxc_semantic_data() {
        let (symbols, references) = derive_ts_reference_facts(
            r#"
type Model = { value: number };

function helper(input: Model) {
    return input.value;
}

let count = 0;
count = helper({ value: count });
"#,
        );

        assert!(references_to(&symbols, &references, "helper").iter().any(
            |reference| reference.kind == ReferenceKind::Call
                && reference.status == SymbolResolutionStatus::Resolved
                && reference.precision == SymbolPrecision::ExactLocal
        ));
        assert!(
            references_to(&symbols, &references, "Model")
                .iter()
                .any(|reference| reference.kind == ReferenceKind::TypeUse)
        );
        assert!(
            references_to(&symbols, &references, "input")
                .iter()
                .any(|reference| reference.kind == ReferenceKind::Read)
        );
        assert!(
            references_to(&symbols, &references, "count")
                .iter()
                .any(|reference| reference.kind == ReferenceKind::Write)
        );
    }

    #[test]
    fn emits_visible_unresolved_references_from_root_unresolved_set() {
        let (_symbols, references) = derive_ts_reference_facts(
            r#"
export function run() {
    return missingValue + anotherMissing();
}
"#,
        );

        assert!(references.iter().any(|reference| {
            reference.name == "missingValue"
                && reference.target.is_none()
                && reference.status == SymbolResolutionStatus::Unresolved
                && reference.precision == SymbolPrecision::Unresolved
        }));
        assert!(references.iter().any(|reference| {
            reference.name == "anotherMissing"
                && reference.kind == ReferenceKind::Call
                && reference.status == SymbolResolutionStatus::Unresolved
        }));
    }

    #[test]
    fn stable_reference_ids_are_deterministic_across_repeated_extraction() {
        let source = "const value = 1;\nexport const doubled = value + value;\n";
        let (_first_symbols, first_references) = derive_ts_reference_facts(source);
        let (_second_symbols, second_references) = derive_ts_reference_facts(source);

        let first = first_references
            .iter()
            .map(|reference| {
                (
                    reference.name.as_str(),
                    reference.id,
                    reference.target,
                    reference.status,
                    reference.stable_key.as_str(),
                )
            })
            .collect::<Vec<_>>();
        let second = second_references
            .iter()
            .map(|reference| {
                (
                    reference.name.as_str(),
                    reference.id,
                    reference.target,
                    reference.status,
                    reference.stable_key.as_str(),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(first, second);
    }
}

#[cfg(test)]
mod symbol_graph_ts_import_links {
    use super::derive_ts_symbols;
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, FileId, ImportFact, ImportId, Language, ModuleNode, ModuleNodeId,
        ModuleNodeKind, ReferenceFact, ReferenceKind, ResolutionPrecision, ResolutionStatus,
        ResolvedImportFact, SymbolFact, SymbolPrecision, SymbolResolutionStatus, UnresolvedReason,
        span_from_byte_range,
    };
    use crate::symbol_graph::model::SymbolGraphBuilder;
    use std::fs;
    use std::path::Path;

    fn add_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) -> FileId {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("test file has parent")).expect("create fixture");
        fs::write(&path, source).expect("write fixture");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn span_for(file: FileId, source: &str, needle: &str, occurrence: usize) -> crate::core::Span {
        let start = source
            .match_indices(needle)
            .nth(occurrence)
            .map(|(index, _)| index)
            .unwrap_or_else(|| panic!("missing occurrence {occurrence} of {needle:?}"));
        span_from_byte_range(file, source, start, start + needle.len())
    }

    fn push_import(
        db: &mut AnalysisDb,
        file: FileId,
        source: &str,
        path: &str,
        occurrence: usize,
    ) -> ImportId {
        db.push_import(ImportFact {
            id: ImportId(0),
            file,
            package: None,
            path: path.to_string(),
            span: span_for(file, source, &format!("{path:?}"), occurrence),
            language: Language::TypeScript,
        })
    }

    fn file_node(id: ModuleNodeId, label: &str, file: FileId) -> ModuleNode {
        ModuleNode {
            id,
            kind: ModuleNodeKind::File,
            label: label.to_string(),
            file: Some(file),
            package: None,
            language: Some(Language::TypeScript),
        }
    }

    fn derive_symbol_reference_facts(
        db: &AnalysisDb,
        root: &Path,
    ) -> (Vec<SymbolFact>, Vec<ReferenceFact>) {
        let loaded = load_config(root).expect("default config loads");
        let plan = AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]);
        let mut builder = SymbolGraphBuilder::new();

        derive_ts_symbols(&mut builder, db, &loaded, &plan);
        let output = builder.finish();
        (output.symbols, output.references)
    }

    fn exported_symbol<'a>(symbols: &'a [SymbolFact], name: &str) -> &'a SymbolFact {
        symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.is_exported)
            .unwrap_or_else(|| panic!("missing exported symbol `{name}` in {symbols:#?}"))
    }

    #[test]
    fn links_named_default_and_namespace_imports_through_module_graph_targets() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        let source = r#"
import defaultThing, { exportedValue as localValue } from "./target";
import * as targetModule from "./target";

export const used = localValue + defaultThing() + targetModule.exportedValue;
"#;
        let target = r#"
export const exportedValue = 1;

const defaultThing = function() {
    return exportedValue;
};
export default defaultThing;
"#;
        let source_file = add_file(&mut db, temp.path(), "src/source.ts", source);
        let target_file = add_file(&mut db, temp.path(), "src/target.ts", target);
        let first_import = push_import(&mut db, source_file, source, "./target", 0);
        let second_import = push_import(&mut db, source_file, source, "./target", 1);
        let source_node = ModuleNodeId(0);
        let target_node = ModuleNodeId(1);
        db.replace_module_graph_facts(
            vec![
                ResolvedImportFact {
                    id: crate::core::ResolvedImportId(0),
                    import: first_import,
                    from_file: source_file,
                    target_node: Some(target_node),
                    status: ResolutionStatus::Resolved,
                    precision: ResolutionPrecision::ExactFile,
                    reason: None,
                },
                ResolvedImportFact {
                    id: crate::core::ResolvedImportId(0),
                    import: second_import,
                    from_file: source_file,
                    target_node: Some(target_node),
                    status: ResolutionStatus::Resolved,
                    precision: ResolutionPrecision::ExactFile,
                    reason: None,
                },
            ],
            vec![
                file_node(source_node, "src/source.ts", source_file),
                file_node(target_node, "src/target.ts", target_file),
            ],
            vec![],
        );

        let (symbols, references) = derive_symbol_reference_facts(&db, temp.path());
        let exported_value = exported_symbol(&symbols, "exportedValue");
        let default_thing = exported_symbol(&symbols, "defaultThing");

        assert!(references.iter().any(|reference| {
            reference.name == "localValue"
                && reference.kind == ReferenceKind::Import
                && reference.target == Some(exported_value.id)
                && reference.status == SymbolResolutionStatus::Resolved
                && reference.precision == SymbolPrecision::ModuleLinked
        }));
        assert!(references.iter().any(|reference| {
            reference.name == "defaultThing"
                && reference.kind == ReferenceKind::Import
                && reference.target == Some(default_thing.id)
                && reference.status == SymbolResolutionStatus::Resolved
                && reference.precision == SymbolPrecision::ModuleLinked
        }));
        assert!(references.iter().any(|reference| {
            reference.name == "targetModule"
                && reference.kind == ReferenceKind::Import
                && reference.status == SymbolResolutionStatus::Ambiguous
                && reference.candidates.contains(&exported_value.id)
                && reference.candidates.contains(&default_thing.id)
        }));
    }

    #[test]
    fn emits_visible_unresolved_import_alias_references() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        let source = r#"
import { missing } from "./missing";

export const used = missing;
"#;
        let source_file = add_file(&mut db, temp.path(), "src/source.ts", source);
        let import = push_import(&mut db, source_file, source, "./missing", 0);
        let source_node = ModuleNodeId(0);
        db.replace_module_graph_facts(
            vec![ResolvedImportFact {
                id: crate::core::ResolvedImportId(0),
                import,
                from_file: source_file,
                target_node: None,
                status: ResolutionStatus::Unresolved,
                precision: ResolutionPrecision::None,
                reason: Some(UnresolvedReason::NotFound),
            }],
            vec![file_node(source_node, "src/source.ts", source_file)],
            vec![],
        );

        let (_symbols, references) = derive_symbol_reference_facts(&db, temp.path());

        assert!(references.iter().any(|reference| {
            reference.name == "missing"
                && reference.kind == ReferenceKind::Import
                && reference.target.is_none()
                && reference.status == SymbolResolutionStatus::Unresolved
                && reference.precision == SymbolPrecision::Unresolved
        }));
    }
}

#[cfg(test)]
mod semantic_scopes {
    use super::{derive_ts_semantic_index, parse_source_type, reference_scope_stable_key};
    use crate::core::{FileId, Language, SourceFile};
    use crate::symbol_graph::semantic::ScopeKind;
    use oxc_allocator::Allocator;
    use oxc_ast::AstKind;
    use oxc_parser::Parser;
    use oxc_semantic::SemanticBuilder;
    use std::path::PathBuf;

    fn source_file(source: &str) -> SourceFile {
        SourceFile {
            id: FileId(0),
            path: PathBuf::from("src/scopes.ts"),
            relative_path: "src/scopes.ts".to_string(),
            language: Language::TypeScript,
            source: source.to_string().into(),
            content_hash: "test-hash".to_string(),
        }
    }

    #[test]
    fn emits_module_function_block_class_catch_loop_switch_type_and_namespace_scopes() {
        let source = r#"
namespace App {
    export interface Model { value: string }
    export type Alias = Model;
    export enum Choice { One }
}

class Widget {
    render() {
        for (const item of [1]) {
            switch (item) {
                case 1: break;
            }
        }
        try {
            throw new Error();
        } catch (err) {
            const local = err;
        }
    }
}

"#;
        let file = source_file(source);
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, parse_source_type(&file.path)).parse();
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;

        let output = derive_ts_semantic_index(
            &file,
            source,
            &parsed.program,
            semantic.scoping(),
            semantic.nodes(),
            false,
        );
        let kinds = output
            .scopes
            .iter()
            .map(|scope| scope.kind)
            .collect::<Vec<_>>();

        assert!(kinds.contains(&ScopeKind::Module));
        assert!(kinds.contains(&ScopeKind::Function));
        assert!(kinds.contains(&ScopeKind::Block));
        assert!(kinds.contains(&ScopeKind::Class));
        assert!(kinds.contains(&ScopeKind::Catch));
        assert!(kinds.contains(&ScopeKind::Loop));
        assert!(kinds.contains(&ScopeKind::Switch));
        assert!(kinds.contains(&ScopeKind::Type));
        assert!(kinds.contains(&ScopeKind::Namespace));
    }

    #[test]
    fn reference_scope_stable_key_uses_the_enclosing_oxc_scope_path() {
        let source = r#"
function outer(value: number) {
    {
        const value = 1;
        return value;
    }
}
"#;
        let file = source_file(source);
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, parse_source_type(&file.path)).parse();
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
        let reference = semantic
            .nodes()
            .iter()
            .find_map(|node| {
                let AstKind::IdentifierReference(identifier) = node.kind() else {
                    return None;
                };
                (identifier.name == "value")
                    .then(|| identifier.reference_id.get())
                    .flatten()
            })
            .expect("fixture has a value reference");

        let stable_key =
            reference_scope_stable_key(&file, semantic.scoping(), semantic.nodes(), reference);

        assert!(stable_key.contains("src/scopes.ts"));
        assert!(stable_key.contains("block"));
    }
}

#[cfg(test)]
mod semantic_imports_exports {
    use super::{
        ReExportTarget, derive_ts_file_symbols, derive_ts_semantic_index, parse_source_type,
        ts_reexport_target_stable_key,
    };
    use crate::analysis_kernel::{AnalysisKernel, validation::validate_fact_metadata};
    use crate::core::{AnalysisDb, FileId, Language, SourceFile, SymbolNamespace};
    use crate::symbol_graph::semantic::{
        ExportKind, SemanticImportKind, SemanticIndexOutput, SemanticStatus,
    };
    use crate::symbol_graph::{LanguageSymbolOutput, model::SymbolGraphBuilder};
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_semantic::SemanticBuilder;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn derive(source: &str) -> crate::symbol_graph::semantic::SemanticIndexOutput {
        let file = SourceFile {
            id: FileId(0),
            path: PathBuf::from("src/imports.ts"),
            relative_path: "src/imports.ts".to_string(),
            language: Language::TypeScript,
            source: source.to_string().into(),
            content_hash: "test-hash".to_string(),
        };
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, parse_source_type(&file.path)).parse();
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;

        derive_ts_semantic_index(
            &file,
            source,
            &parsed.program,
            semantic.scoping(),
            semantic.nodes(),
            false,
        )
    }

    fn derive_and_install(
        source: &str,
    ) -> (
        LanguageSymbolOutput,
        crate::analysis_kernel::validation::FactValidationReport,
    ) {
        let mut db = AnalysisDb::new();
        let file_id = db.add_file(
            PathBuf::from("src/imports.ts"),
            "src/imports.ts".to_string(),
            source.to_string(),
        );
        let file = db.file(file_id).expect("fixture file exists").clone();
        let mut builder = SymbolGraphBuilder::new();
        let mut output = LanguageSymbolOutput::default();

        derive_ts_file_symbols(&mut builder, &mut output, &file, false);
        let symbols = builder.finish();
        db.replace_symbol_graph_facts(symbols.symbols, symbols.definitions, symbols.references);
        let SemanticIndexOutput {
            scopes,
            semantic_imports,
            exports,
            aliases,
            resolutions,
            generated_symbols,
            stable_exports,
        } = output.semantic.clone();
        db.replace_semantic_index_facts(
            scopes,
            semantic_imports,
            exports,
            aliases,
            resolutions,
            generated_symbols,
            stable_exports,
        );
        let validation = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        (output, validation)
    }

    #[test]
    fn emits_static_import_rows_for_named_default_namespace_side_effect_and_type_only_forms() {
        let output = derive(
            r#"
import defaultThing from "pkg";
import { named as local, type TypeName } from "./mod";
import * as ns from "./ns";
import "./side-effect";
"#,
        );
        let kinds = output
            .semantic_imports
            .iter()
            .map(|fact| fact.kind)
            .collect::<Vec<_>>();

        assert!(kinds.contains(&SemanticImportKind::StaticDefault));
        assert!(kinds.contains(&SemanticImportKind::StaticNamed));
        assert!(kinds.contains(&SemanticImportKind::StaticNamespace));
        assert!(kinds.contains(&SemanticImportKind::SideEffect));
        assert!(kinds.contains(&SemanticImportKind::TypeOnly));
    }

    #[test]
    fn side_effect_import_stable_keys_include_import_path() {
        let output = derive(
            r#"
import "./setup-a";
import "./setup-b";
"#,
        );
        let side_effect_keys = output
            .semantic_imports
            .iter()
            .filter(|fact| fact.kind == SemanticImportKind::SideEffect)
            .map(|fact| fact.stable_key.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(side_effect_keys.len(), 2);
    }

    #[test]
    fn repeated_import_expressions_emit_one_semantic_identity_each() {
        let output = derive(
            r#"
import "./setup";
import "./setup";
const first = require("pkg");
const second = require("pkg");
const lazyFirst = import("./chunk");
const lazySecond = import("./chunk");
"#,
        );
        let keys = output
            .semantic_imports
            .iter()
            .map(|fact| fact.stable_key.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(keys.len(), output.semantic_imports.len());
        assert_eq!(
            output
                .semantic_imports
                .iter()
                .filter(|fact| fact.import_path == "./setup")
                .count(),
            1
        );
        assert_eq!(
            output
                .semantic_imports
                .iter()
                .filter(|fact| fact.import_path == "pkg")
                .count(),
            1
        );
        assert_eq!(
            output
                .semantic_imports
                .iter()
                .filter(|fact| fact.import_path == "./chunk")
                .count(),
            1
        );
    }

    #[test]
    fn emits_export_rows_for_named_default_star_reexport_and_reexport_specifiers() {
        let output = derive(
            r#"
const local = 1;
export { local as renamed };
export default function main() {}
export * from "./star";
export { named as reexported } from "./mod";
"#,
        );
        let kinds = output
            .exports
            .iter()
            .map(|fact| fact.kind)
            .collect::<Vec<_>>();

        assert!(kinds.contains(&ExportKind::Named));
        assert!(kinds.contains(&ExportKind::Default));
        assert!(kinds.contains(&ExportKind::StarReexport));
        assert!(kinds.contains(&ExportKind::ReExport));
        assert!(
            output
                .aliases
                .iter()
                .any(|alias| alias.status == SemanticStatus::Unresolved)
        );
    }

    #[test]
    fn declaration_merging_emits_one_type_export_identity() {
        let output = derive(
            r#"
export interface MergeMe {
    first: string;
}

export interface MergeMe {
    second: string;
}

export const value = 1;
"#,
        );
        let exports = output
            .exports
            .iter()
            .filter(|fact| fact.export_name == "MergeMe")
            .collect::<Vec<_>>();
        let stable_exports = output
            .stable_exports
            .iter()
            .filter(|fact| fact.export_name == "MergeMe")
            .collect::<Vec<_>>();

        assert_eq!(exports.len(), 1);
        assert_eq!(stable_exports.len(), 1);
        assert_eq!(exports[0].namespace, SymbolNamespace::Type);
        assert_eq!(stable_exports[0].namespace, SymbolNamespace::Type);
        assert_eq!(stable_exports[0].export, exports[0].id);
        assert!(output.exports.iter().any(|fact| {
            fact.export_name == "value" && fact.namespace == SymbolNamespace::Value
        }));
        assert_eq!(
            output
                .exports
                .iter()
                .map(|fact| fact.stable_key.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            output.exports.len()
        );
        assert_eq!(
            output
                .stable_exports
                .iter()
                .map(|fact| fact.stable_key.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            output.stable_exports.len()
        );
    }

    #[test]
    fn default_interface_export_uses_type_namespace() {
        let output = derive(
            r#"
export default interface DefaultShape {
    value: string;
}
"#,
        );
        let export = output
            .exports
            .iter()
            .find(|fact| fact.export_name == "default")
            .expect("default export exists");
        let stable_export = output
            .stable_exports
            .iter()
            .find(|fact| fact.export_name == "default")
            .expect("default stable export exists");

        assert_eq!(export.namespace, SymbolNamespace::Type);
        assert_eq!(stable_export.namespace, SymbolNamespace::Type);
        assert_eq!(stable_export.export, export.id);
    }

    #[test]
    fn source_type_reexports_use_type_namespace() {
        let output = derive(
            r#"
export type { Foo as TypeFoo } from "./types";
export { type Bar as TypeBar } from "./types";
"#,
        );
        let reexports = output
            .exports
            .iter()
            .filter(|fact| matches!(fact.export_name.as_str(), "TypeFoo" | "TypeBar"))
            .collect::<Vec<_>>();

        assert_eq!(reexports.len(), 2);
        assert!(reexports.iter().all(|fact| {
            fact.namespace == SymbolNamespace::Type && fact.kind == ExportKind::ReExport
        }));
    }

    #[test]
    fn classifies_source_reexport_forms_without_stable_targets() {
        let output = derive(
            r#"
export { value as renamed } from "./value";
export type { Shape as RenamedShape } from "./types";
export * from "./star";
export type * from "./type-star";
export * as namespaceExport from "./namespace";
"#,
        );
        let expected = [
            ("renamed", SymbolNamespace::Value, ExportKind::ReExport),
            ("RenamedShape", SymbolNamespace::Type, ExportKind::ReExport),
            (
                "* from ./star",
                SymbolNamespace::Module,
                ExportKind::StarReexport,
            ),
            (
                "* from ./type-star",
                SymbolNamespace::Type,
                ExportKind::StarReexport,
            ),
            (
                "namespaceExport",
                SymbolNamespace::Namespace,
                ExportKind::Namespace,
            ),
        ];

        for (name, namespace, kind) in expected {
            let export = output
                .exports
                .iter()
                .find(|fact| fact.export_name == name)
                .unwrap_or_else(|| panic!("missing export `{name}` in {output:#?}"));
            assert_eq!(export.namespace, namespace);
            assert_eq!(export.kind, kind);
            assert_eq!(export.status, SemanticStatus::Unresolved);
        }
        let alias_targets = output
            .aliases
            .iter()
            .flat_map(|alias| alias.target_symbol_stable_keys.iter().cloned())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            alias_targets,
            BTreeSet::from([
                ts_reexport_target_stable_key(ReExportTarget::Named {
                    module: "./types",
                    imported_name: "Shape",
                }),
                ts_reexport_target_stable_key(ReExportTarget::Named {
                    module: "./value",
                    imported_name: "value",
                }),
                ts_reexport_target_stable_key(ReExportTarget::Module {
                    module: "./namespace",
                }),
                ts_reexport_target_stable_key(ReExportTarget::Module { module: "./star" }),
                ts_reexport_target_stable_key(ReExportTarget::Module {
                    module: "./type-star",
                }),
            ])
        );
        assert!(
            output
                .aliases
                .iter()
                .all(|alias| alias.kind == crate::symbol_graph::semantic::AliasKind::ReExport)
        );
        assert_eq!(
            output
                .exports
                .iter()
                .map(|fact| fact.stable_key.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            output.exports.len()
        );
        assert!(output.stable_exports.is_empty());
    }

    #[test]
    fn type_and_value_exports_with_one_name_keep_distinct_stable_keys() {
        let (output, validation) = derive_and_install(
            r#"
interface SharedType {
    value: string;
}
const sharedValue = 1;
export type { SharedType as Shared };
export { sharedValue as Shared };
"#,
        );
        assert!(
            output.diagnostics.is_empty(),
            "valid type/value declaration pairing must parse cleanly: {:#?}",
            output.diagnostics
        );
        assert!(validation.diagnostics.is_empty());
        let exports = output
            .semantic
            .exports
            .iter()
            .filter(|fact| fact.export_name == "Shared")
            .collect::<Vec<_>>();
        assert_eq!(exports.len(), 2);
        assert_ne!(exports[0].stable_key, exports[1].stable_key);
        assert!(
            exports
                .iter()
                .any(|fact| fact.namespace == SymbolNamespace::Type)
        );
        assert!(
            exports
                .iter()
                .any(|fact| fact.namespace == SymbolNamespace::Value)
        );
        assert_eq!(
            output
                .semantic
                .stable_exports
                .iter()
                .filter(|fact| fact.export_name == "Shared")
                .count(),
            2
        );
    }

    #[test]
    fn duplicate_export_targets_install_without_metadata_conflicts() {
        let (output, validation) = derive_and_install(
            r#"
const left = 1;
const right = 2;
const defaultValue = 3;
export { left as shared, right as shared };
export { first as shared } from "./first";
export { second as shared } from "./second";
export { defaultValue as default };
export default defaultValue;
"#,
        );
        let exports = output
            .semantic
            .exports
            .iter()
            .filter(|fact| fact.export_name == "shared")
            .collect::<Vec<_>>();
        let alias_targets = output
            .semantic
            .aliases
            .iter()
            .flat_map(|alias| alias.target_symbol_stable_keys.iter().cloned())
            .collect::<BTreeSet<_>>();

        assert_eq!(exports.len(), 2, "local and re-export kinds stay distinct");
        assert_eq!(
            exports
                .iter()
                .map(|fact| fact.stable_key.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            exports.len()
        );
        assert_eq!(
            output
                .semantic
                .stable_exports
                .iter()
                .filter(|fact| fact.export_name == "shared")
                .count(),
            2
        );
        assert_eq!(
            output
                .semantic
                .exports
                .iter()
                .filter(|fact| fact.export_name == "default")
                .count(),
            2,
            "named and default export kinds remain distinct"
        );
        assert_eq!(
            output
                .semantic
                .stable_exports
                .iter()
                .filter(|fact| fact.export_name == "default")
                .count(),
            1,
            "the persisted stable-export identity is kind-independent"
        );
        assert_eq!(alias_targets.len(), 2);
        assert!(output.diagnostics.iter().all(|diagnostic| {
            !matches!(
                diagnostic.rule_id.as_str(),
                "polint/internal" | "polint/capability"
            )
        }));
        assert!(
            validation.diagnostics.is_empty(),
            "recovered semantic facts must remain valid: {:#?}",
            validation.diagnostics
        );
    }

    #[test]
    fn multi_file_semantic_ids_preserve_scope_and_export_ownership() {
        let mut db = AnalysisDb::new();
        let files = [
            (
                "src/a.ts",
                r#"
import { beta } from "./b";
export const alpha = beta;
"#,
            ),
            (
                "src/b.ts",
                r#"
export const beta = 2;
"#,
            ),
        ]
        .into_iter()
        .map(|(path, source)| {
            let id = db.add_file(PathBuf::from(path), path.to_string(), source.to_string());
            db.file(id).expect("fixture file exists").clone()
        })
        .collect::<Vec<_>>();
        let mut symbol_builder = SymbolGraphBuilder::new();
        let mut output = LanguageSymbolOutput::default();
        for file in &files {
            derive_ts_file_symbols(&mut symbol_builder, &mut output, file, false);
        }
        assert!(
            output.diagnostics.is_empty(),
            "multi-file fixture must parse cleanly: {:#?}",
            output.diagnostics
        );

        let symbols = symbol_builder.finish();
        db.replace_symbol_graph_facts(symbols.symbols, symbols.definitions, symbols.references);
        let SemanticIndexOutput {
            scopes,
            semantic_imports,
            exports,
            aliases,
            resolutions,
            generated_symbols,
            stable_exports,
        } = output.semantic;
        db.replace_semantic_index_facts(
            scopes,
            semantic_imports,
            exports,
            aliases,
            resolutions,
            generated_symbols,
            stable_exports,
        );

        let validation = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        assert!(
            validation.diagnostics.is_empty(),
            "multi-file semantic facts must remain valid: {:#?}",
            validation.diagnostics
        );
        assert_eq!(db.exports().len(), 2);
        assert_eq!(db.stable_exports().len(), 2);

        for import in db.semantic_imports() {
            let scope = import
                .scope
                .and_then(|id| db.scopes().iter().find(|scope| scope.id == id))
                .expect("scoped import retains its module scope");
            assert_eq!(scope.file, import.file);
        }
        for export in db.exports() {
            let scope = export
                .scope
                .and_then(|id| db.scopes().iter().find(|scope| scope.id == id))
                .expect("scoped export retains its module scope");
            assert_eq!(scope.file, export.file);
        }
        for stable_export in db.stable_exports() {
            let export = db
                .exports()
                .iter()
                .find(|export| export.id == stable_export.export)
                .expect("stable export retains its export row");
            let module_key = stable_export
                .module_key
                .as_deref()
                .expect("TypeScript stable export retains its source file key");
            let file = db
                .files()
                .iter()
                .find(|file| file.relative_path == module_key)
                .expect("stable export source file exists");

            assert_eq!(export.file, Some(file.id));
            assert_eq!(export.export_name, stable_export.export_name);
            assert_eq!(export.namespace, stable_export.namespace);
            assert_eq!(export.language, stable_export.language);
        }
    }

    #[test]
    fn canonical_reexport_targets_separate_named_and_module_vocabularies() {
        let output = derive(
            r#"
export { path as namedTarget } from "module";
export * from "path";
"#,
        );
        let named_target = ts_reexport_target_stable_key(ReExportTarget::Named {
            module: "module",
            imported_name: "path",
        });
        let module_target =
            ts_reexport_target_stable_key(ReExportTarget::Module { module: "path" });
        let emitted_targets = output
            .aliases
            .iter()
            .flat_map(|alias| alias.target_symbol_stable_keys.iter().cloned())
            .collect::<BTreeSet<_>>();

        assert_ne!(named_target, module_target);
        assert_eq!(
            emitted_targets,
            BTreeSet::from([named_target, module_target])
        );
        assert_eq!(output.aliases.len(), 2);
        assert_eq!(output.exports.len(), 2);
    }

    #[test]
    fn star_reexport_alias_stable_keys_include_reexport_target() {
        let output = derive(
            r#"
export * from "./a";
export * from "./a";
export * from "./b";
"#,
        );
        let reexport_keys = output
            .aliases
            .iter()
            .filter(|alias| alias.source_symbol_stable_key.starts_with("reexport:star:"))
            .map(|alias| alias.stable_key.as_str())
            .collect::<BTreeSet<_>>();
        let star_exports = output
            .exports
            .iter()
            .filter(|export| export.kind == ExportKind::StarReexport)
            .count();

        assert_eq!(reexport_keys.len(), 2);
        assert_eq!(star_exports, 2);
    }

    #[test]
    fn emits_conservative_rows_for_commonjs_and_dynamic_import_forms() {
        let output = derive(
            r#"
const req = require(moduleName);
module.exports = req;
exports.named = req;
const lazy = import(moduleName);
"#,
        );

        assert!(output.semantic_imports.iter().any(|fact| {
            fact.kind == SemanticImportKind::CommonJsRequire
                && fact.status == SemanticStatus::Dynamic
        }));
        assert!(output.semantic_imports.iter().any(|fact| {
            fact.kind == SemanticImportKind::DynamicImport && fact.status == SemanticStatus::Dynamic
        }));
        assert!(output.exports.iter().any(|fact| {
            fact.kind == ExportKind::CommonJsModuleExports
                && fact.status == SemanticStatus::Unsupported
        }));
        assert!(output.exports.iter().any(|fact| {
            fact.kind == ExportKind::CommonJsExportsProperty
                && fact.status == SemanticStatus::Unsupported
        }));
    }
}

#[cfg(test)]
mod semantic_resolution {
    use super::{derive_ts_file_symbols, derive_ts_semantic_index, parse_source_type};
    use crate::core::{FileId, Language, SourceFile, SymbolFact};
    use crate::symbol_graph::semantic::{ResolutionStepKind, SemanticIndexOutput, SemanticStatus};
    use crate::symbol_graph::{LanguageSymbolOutput, model::SymbolGraphBuilder};
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_semantic::SemanticBuilder;
    use std::path::PathBuf;

    fn derive(source: &str) -> crate::symbol_graph::semantic::SemanticIndexOutput {
        let file = SourceFile {
            id: FileId(0),
            path: PathBuf::from("src/resolution.ts"),
            relative_path: "src/resolution.ts".to_string(),
            language: Language::TypeScript,
            source: source.to_string().into(),
            content_hash: "test-hash".to_string(),
        };
        let allocator = Allocator::default();
        let parsed = Parser::new(&allocator, source, parse_source_type(&file.path)).parse();
        let semantic = SemanticBuilder::new().build(&parsed.program).semantic;

        derive_ts_semantic_index(
            &file,
            source,
            &parsed.program,
            semantic.scoping(),
            semantic.nodes(),
            true,
        )
    }

    fn derive_symbols_and_semantic(source: &str) -> (Vec<SymbolFact>, SemanticIndexOutput) {
        let file = SourceFile {
            id: FileId(0),
            path: PathBuf::from("src/resolution.ts"),
            relative_path: "src/resolution.ts".to_string(),
            language: Language::TypeScript,
            source: source.to_string().into(),
            content_hash: "test-hash".to_string(),
        };
        let mut builder = SymbolGraphBuilder::new();
        let mut output = LanguageSymbolOutput::default();

        derive_ts_file_symbols(&mut builder, &mut output, &file, false);
        let symbol_output = builder.finish();

        (symbol_output.symbols, output.semantic)
    }

    #[test]
    fn local_lexical_references_record_resolved_lookup_steps() {
        let output = derive(
            r#"
const value = 1;
export const doubled = value + value;
"#,
        );

        assert!(output.resolutions.iter().any(|resolution| {
            resolution.step == ResolutionStepKind::LexicalLookup
                && resolution.status == SemanticStatus::Resolved
                && resolution.source_stable_key.contains("value")
        }));
    }

    #[test]
    fn import_alias_references_record_alias_and_module_lookup_steps() {
        let output = derive(
            r#"
import { thing as localThing } from "./thing";
export const used = localThing;
"#,
        );

        assert!(
            output
                .resolutions
                .iter()
                .any(|resolution| resolution.step == ResolutionStepKind::ImportAliasLookup)
        );
        assert!(
            output
                .resolutions
                .iter()
                .any(|resolution| resolution.step == ResolutionStepKind::ModuleLookup)
        );
    }

    #[test]
    fn unresolved_references_record_unknown_fallback_steps() {
        let output = derive("export const value = missingGlobal;\n");

        assert!(output.resolutions.iter().any(|resolution| {
            resolution.step == ResolutionStepKind::UnknownFallback
                && resolution.status == SemanticStatus::Unresolved
                && resolution.source_stable_key.contains("missingGlobal")
        }));
    }

    #[test]
    fn stable_export_identities_use_native_discriminator() {
        let output = derive("export const value = 1;\n");

        assert!(output.stable_exports.iter().any(|identity| {
            identity.export_name == "value"
                && identity.generated_discriminator.as_deref() == Some("native")
                && identity.symbol_stable_key.contains("value")
        }));
    }

    #[test]
    fn stable_export_symbol_key_matches_exported_symbol_fact_key() {
        let (symbols, semantic) = derive_symbols_and_semantic("export const value = 1;\n");
        let exported_symbol = symbols
            .iter()
            .find(|symbol| symbol.name == "value" && symbol.is_exported)
            .expect("exported symbol exists");
        let stable_export = semantic
            .stable_exports
            .iter()
            .find(|identity| identity.export_name == "value")
            .expect("stable export exists");

        assert_eq!(stable_export.symbol_stable_key, exported_symbol.stable_key);
    }

    #[test]
    fn symbols_only_semantic_derivation_does_not_emit_reference_keyed_resolutions() {
        let (_symbols, semantic) =
            derive_symbols_and_semantic("const value = 1;\nexport const doubled = value;\n");

        assert!(semantic.resolutions.is_empty());
    }
}
