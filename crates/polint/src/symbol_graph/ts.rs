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
    ScopeFact, ScopeId, ScopeKind, SemanticIndexBuilder, SemanticIndexOutput, SemanticStatus,
};
use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    BindingIdentifier, BindingPattern, Declaration, ExportDefaultDeclarationKind,
    ImportDeclarationSpecifier, ImportOrExportKind, ModuleExportName, Program, Statement,
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum ImportAliasTarget {
    Named(String),
    Default,
    Namespace,
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
    let _semantic_index = derive_ts_semantic_index(file, source, &parsed.program, scoping, nodes);
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
    _source: &str,
    _program: &Program<'_>,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
) -> SemanticIndexOutput {
    let mut builder = SemanticIndexBuilder::new();
    let mut ids_by_stable_key = BTreeMap::<String, ScopeId>::new();

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

    builder.finish()
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

fn reference_scope_stable_key(
    file: &SourceFile,
    scoping: &Scoping,
    nodes: &AstNodes<'_>,
    reference_id: OxcReferenceId,
) -> String {
    let reference = scoping.get_reference(reference_id);
    scope_stable_key(file, scoping, nodes, reference.scope_id())
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
                if let Some(symbol) = default_export_symbol(&export.declaration) {
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

fn default_export_symbol(declaration: &ExportDefaultDeclarationKind<'_>) -> Option<OxcSymbolId> {
    match declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            function.id.as_ref()?.symbol_id.get()
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => class.id.as_ref()?.symbol_id.get(),
        ExportDefaultDeclarationKind::TSInterfaceDeclaration(interface) => {
            interface.id.symbol_id.get()
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

export default function defaultThing() {
    return exportedValue;
}
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
    use super::{derive_ts_semantic_index, parse_source_type};
    use crate::core::{FileId, Language, SourceFile};
    use crate::symbol_graph::semantic::{ExportKind, SemanticImportKind, SemanticStatus};
    use oxc_allocator::Allocator;
    use oxc_parser::Parser;
    use oxc_semantic::SemanticBuilder;
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
        )
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
        assert!(kinds.contains(&ExportKind::Namespace));
        assert!(
            output
                .aliases
                .iter()
                .any(|alias| alias.status == SemanticStatus::Unresolved)
        );
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
