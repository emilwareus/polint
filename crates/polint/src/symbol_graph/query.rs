use crate::core::{
    AnalysisDb, DefinitionFact, FileId, ReferenceFact, SymbolFact, SymbolId, SymbolResolutionStatus,
};

pub(crate) fn symbol_by_id(db: &AnalysisDb, id: SymbolId) -> Option<&SymbolFact> {
    db.symbol_by_id(id)
}

pub(crate) fn symbols_for_file(db: &AnalysisDb, file: FileId) -> impl Iterator<Item = &SymbolFact> {
    db.symbols_for_file(file)
}

pub(crate) fn symbols_by_name<'a>(
    db: &'a AnalysisDb,
    name: &'a str,
) -> impl Iterator<Item = &'a SymbolFact> {
    db.symbols_by_name(name)
}

pub(crate) fn definitions_for_symbol(
    db: &AnalysisDb,
    symbol: SymbolId,
) -> impl Iterator<Item = &DefinitionFact> {
    db.definitions_for_symbol(symbol)
}

pub(crate) fn references_to_symbol(
    db: &AnalysisDb,
    symbol: SymbolId,
) -> impl Iterator<Item = &ReferenceFact> {
    db.references_to_symbol(symbol)
}

pub(crate) fn references_for_file(
    db: &AnalysisDb,
    file: FileId,
) -> impl Iterator<Item = &ReferenceFact> {
    db.references_for_file(file)
}

pub(crate) fn unresolved_references(db: &AnalysisDb) -> impl Iterator<Item = &ReferenceFact> {
    references_with_status(db, SymbolResolutionStatus::Unresolved)
}

pub(crate) fn ambiguous_references(db: &AnalysisDb) -> impl Iterator<Item = &ReferenceFact> {
    references_with_status(db, SymbolResolutionStatus::Ambiguous)
}

fn references_with_status(
    db: &AnalysisDb,
    status: SymbolResolutionStatus,
) -> impl Iterator<Item = &ReferenceFact> {
    let mut references = db
        .references()
        .iter()
        .filter(move |reference| reference.status == status)
        .collect::<Vec<_>>();
    references.sort_by(|left, right| reference_order(left, right));
    references.into_iter()
}

fn reference_order(left: &ReferenceFact, right: &ReferenceFact) -> std::cmp::Ordering {
    (
        left.id,
        left.name.as_str(),
        span_key(left.primary_span.as_ref()),
        left.stable_key.as_str(),
    )
        .cmp(&(
            right.id,
            right.name.as_str(),
            span_key(right.primary_span.as_ref()),
            right.stable_key.as_str(),
        ))
}

fn span_key(span: Option<&crate::core::Span>) -> (u32, u32) {
    span.map(|span| (span.start_byte, span.end_byte))
        .unwrap_or((u32::MAX, u32::MAX))
}

#[cfg(test)]
mod symbol_graph_query {
    use crate::core::{
        AnalysisDb, DefinitionFact, DefinitionId, DefinitionKind, FileId, Language, ModuleNodeId,
        ReferenceFact, ReferenceId, ReferenceKind, Span, SymbolFact, SymbolId, SymbolKind,
        SymbolNamespace, SymbolPrecision, SymbolResolutionStatus,
    };
    use crate::sdk::facts::{FactView, References, Symbols};
    use crate::symbol_graph::query;
    use std::path::PathBuf;

    fn build_db() -> (AnalysisDb, FileId, FileId, SymbolId, SymbolId) {
        let mut db = AnalysisDb::new();
        let app_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function Button() { return theme; }\n".to_string(),
        );
        let theme_file = db.add_file(
            PathBuf::from("src/theme.ts"),
            "src/theme.ts".to_string(),
            "export const theme = {};\n".to_string(),
        );
        let button = SymbolId(20);
        let theme = SymbolId(10);

        db.replace_symbol_graph_facts(
            vec![
                symbol_fact(theme, "theme", theme_file, 1, SymbolKind::Constant),
                symbol_fact(button, "Button", app_file, 1, SymbolKind::Function),
                symbol_fact(SymbolId(30), "Button", app_file, 10, SymbolKind::Class),
            ],
            vec![definition_fact(
                DefinitionId(30),
                button,
                "Button",
                app_file,
                1,
            )],
            vec![
                reference_fact(
                    ReferenceId(60),
                    "ambiguous",
                    app_file,
                    44,
                    None,
                    vec![button, theme],
                    SymbolResolutionStatus::Ambiguous,
                ),
                reference_fact(
                    ReferenceId(50),
                    "missing",
                    app_file,
                    35,
                    None,
                    Vec::new(),
                    SymbolResolutionStatus::Unresolved,
                ),
                reference_fact(
                    ReferenceId(40),
                    "theme",
                    app_file,
                    28,
                    Some(theme),
                    Vec::new(),
                    SymbolResolutionStatus::Resolved,
                ),
            ],
        );

        (db, app_file, theme_file, button, theme)
    }

    fn symbol_fact(
        id: SymbolId,
        name: &str,
        file: FileId,
        col: u32,
        kind: SymbolKind,
    ) -> SymbolFact {
        SymbolFact {
            id,
            language: Language::TypeScript,
            name: name.to_string(),
            qualified_name: format!("src/app.ts::{name}"),
            kind,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: Some(ModuleNodeId(0)),
            owner: None,
            primary_span: Some(Span::point(file, 1, col)),
            is_exported: true,
            stable_key: format!("symbol:{name}:{}", id.0),
            precision: SymbolPrecision::ExactLocal,
        }
    }

    fn definition_fact(
        id: DefinitionId,
        symbol: SymbolId,
        name: &str,
        file: FileId,
        col: u32,
    ) -> DefinitionFact {
        DefinitionFact {
            id,
            symbol,
            language: Language::TypeScript,
            name: name.to_string(),
            qualified_name: format!("src/app.ts::{name}"),
            kind: DefinitionKind::Declaration,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: Some(ModuleNodeId(0)),
            owner: None,
            primary_span: Some(Span::point(file, 1, col)),
            is_primary: true,
            is_exported: true,
            stable_key: format!("definition:{name}:{}", id.0),
            precision: SymbolPrecision::ExactLocal,
        }
    }

    fn reference_fact(
        id: ReferenceId,
        name: &str,
        file: FileId,
        col: u32,
        target: Option<SymbolId>,
        candidates: Vec<SymbolId>,
        status: SymbolResolutionStatus,
    ) -> ReferenceFact {
        ReferenceFact {
            id,
            language: Language::TypeScript,
            name: name.to_string(),
            qualified_name: name.to_string(),
            kind: ReferenceKind::Read,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: Some(ModuleNodeId(0)),
            owner: None,
            primary_span: Some(Span::point(file, 1, col)),
            target,
            candidates,
            stable_key: format!("reference:{name}:{}", id.0),
            status,
            precision: match status {
                SymbolResolutionStatus::Resolved => SymbolPrecision::ExactLocal,
                SymbolResolutionStatus::Unresolved => SymbolPrecision::Unresolved,
                SymbolResolutionStatus::Ambiguous => SymbolPrecision::Ambiguous,
                SymbolResolutionStatus::SetupMissing => SymbolPrecision::SetupMissing,
                SymbolResolutionStatus::Unsupported => SymbolPrecision::Unsupported,
            },
        }
    }

    #[test]
    fn query_helpers_match_sdk_symbol_and_reference_views() {
        let (db, app_file, _theme_file, button, theme) = build_db();
        let symbols = Symbols::build(&db);
        let references = References::build(&db);

        assert!(std::ptr::eq(
            query::symbol_by_id(&db, button).unwrap(),
            symbols.get(button).unwrap()
        ));
        assert_eq!(
            query::symbols_for_file(&db, app_file)
                .map(|symbol| symbol.id)
                .collect::<Vec<_>>(),
            symbols
                .for_file(app_file)
                .map(|symbol| symbol.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            query::symbols_by_name(&db, "Button")
                .map(|symbol| symbol.id)
                .collect::<Vec<_>>(),
            symbols
                .by_name("Button")
                .map(|symbol| symbol.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            query::definitions_for_symbol(&db, button)
                .map(|definition| definition.id)
                .collect::<Vec<_>>(),
            symbols
                .definitions(button)
                .map(|definition| definition.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            query::references_to_symbol(&db, theme)
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            references
                .to(theme)
                .map(|reference| reference.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            query::references_for_file(&db, app_file)
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            references
                .for_file(app_file)
                .map(|reference| reference.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            query::unresolved_references(&db)
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            references
                .unresolved()
                .map(|reference| reference.id)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            query::ambiguous_references(&db)
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            references
                .ambiguous()
                .map(|reference| reference.id)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn query_helpers_return_deterministic_id_order() {
        let (db, app_file, _theme_file, _button, _theme) = build_db();

        assert_eq!(
            query::symbols_by_name(&db, "Button")
                .map(|symbol| symbol.id)
                .collect::<Vec<_>>(),
            vec![SymbolId(20), SymbolId(30)]
        );
        assert_eq!(
            query::references_for_file(&db, app_file)
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            vec![ReferenceId(40), ReferenceId(50), ReferenceId(60)]
        );
    }
}
