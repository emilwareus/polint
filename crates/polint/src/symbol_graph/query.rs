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
            vec![definition_fact(DefinitionId(30), button, "Button", app_file, 1)],
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
