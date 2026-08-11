use polint_analysis_api::{
    DefinitionFact, FactDatabase, ReferenceFact, SymbolFact, SymbolResolutionStatus,
};
use polint_core::{FileId, Span, SymbolId};

pub fn symbol_by_id(db: &dyn FactDatabase, id: SymbolId) -> Option<&SymbolFact> {
    db.symbols().iter().find(|symbol| symbol.id == id)
}

pub fn symbols_for_file(db: &dyn FactDatabase, file: FileId) -> impl Iterator<Item = &SymbolFact> {
    db.symbols()
        .iter()
        .filter(move |symbol| symbol.file == Some(file))
}

pub fn symbols_by_name<'a>(
    db: &'a dyn FactDatabase,
    name: &str,
) -> impl Iterator<Item = &'a SymbolFact> {
    db.symbols()
        .iter()
        .filter(move |symbol| symbol.name == name)
}

pub fn definitions_for_symbol(
    db: &dyn FactDatabase,
    symbol: SymbolId,
) -> impl Iterator<Item = &DefinitionFact> {
    db.definitions()
        .iter()
        .filter(move |definition| definition.symbol == symbol)
}

pub fn references_to_symbol(
    db: &dyn FactDatabase,
    symbol: SymbolId,
) -> impl Iterator<Item = &ReferenceFact> {
    db.references()
        .iter()
        .filter(move |reference| reference.target == Some(symbol))
}

pub fn references_for_file(
    db: &dyn FactDatabase,
    file: FileId,
) -> impl Iterator<Item = &ReferenceFact> {
    db.references()
        .iter()
        .filter(move |reference| reference.file == Some(file))
}

pub fn unresolved_references(db: &dyn FactDatabase) -> impl Iterator<Item = &ReferenceFact> {
    references_with_status(db, SymbolResolutionStatus::Unresolved)
}

pub fn ambiguous_references(db: &dyn FactDatabase) -> impl Iterator<Item = &ReferenceFact> {
    references_with_status(db, SymbolResolutionStatus::Ambiguous)
}

fn references_with_status(
    db: &dyn FactDatabase,
    status: SymbolResolutionStatus,
) -> impl Iterator<Item = &ReferenceFact> {
    let mut references = db
        .references()
        .iter()
        .filter(move |reference| reference.status == status)
        .collect::<Vec<_>>();
    references.sort_by(|left, right| reference_order(db, left, right));
    references.into_iter()
}

fn reference_order(
    db: &dyn FactDatabase,
    left: &ReferenceFact,
    right: &ReferenceFact,
) -> std::cmp::Ordering {
    let interner = db.stable_key_interner();
    let left_stable_key = interner.resolve(left.stable_key);
    let right_stable_key = interner.resolve(right.stable_key);
    (
        left.id,
        left.name.as_str(),
        span_key(left.primary_span.as_ref()),
        left_stable_key.as_ref(),
    )
        .cmp(&(
            right.id,
            right.name.as_str(),
            span_key(right.primary_span.as_ref()),
            right_stable_key.as_ref(),
        ))
}

fn span_key(span: Option<&Span>) -> (u32, u32) {
    span.map(|span| (span.start_byte, span.end_byte))
        .unwrap_or((u32::MAX, u32::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LocalAnalysisDb;
    use polint_analysis_api::{
        DefinitionKind, ReferenceKind, SymbolKind, SymbolNamespace, SymbolPrecision,
    };
    use polint_core::{Language, ModuleNodeId, Span, StableKeyInterner};
    use std::path::PathBuf;

    fn build_db() -> (LocalAnalysisDb, FileId, SymbolId, SymbolId) {
        let mut db = LocalAnalysisDb::new();
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
        let interner = db.stable_key_interner();
        let button = SymbolId(20);
        let theme = SymbolId(10);
        db.replace_symbol_graph_facts(
            vec![
                symbol_fact(
                    &interner,
                    theme,
                    "theme",
                    theme_file,
                    1,
                    SymbolKind::Constant,
                ),
                symbol_fact(
                    &interner,
                    button,
                    "Button",
                    app_file,
                    1,
                    SymbolKind::Function,
                ),
                symbol_fact(
                    &interner,
                    SymbolId(30),
                    "Button",
                    app_file,
                    10,
                    SymbolKind::Class,
                ),
            ],
            vec![definition_fact(
                &interner,
                polint_core::DefinitionId(30),
                button,
                "Button",
                app_file,
                1,
            )],
            vec![
                reference_fact(
                    &interner,
                    polint_core::ReferenceId(60),
                    "ambiguous",
                    (app_file, 44),
                    None,
                    vec![button, theme],
                    SymbolResolutionStatus::Ambiguous,
                ),
                reference_fact(
                    &interner,
                    polint_core::ReferenceId(50),
                    "missing",
                    (app_file, 35),
                    None,
                    Vec::new(),
                    SymbolResolutionStatus::Unresolved,
                ),
                reference_fact(
                    &interner,
                    polint_core::ReferenceId(40),
                    "theme",
                    (app_file, 28),
                    Some(theme),
                    Vec::new(),
                    SymbolResolutionStatus::Resolved,
                ),
            ],
        );
        (db, app_file, button, theme)
    }

    fn symbol_fact(
        interner: &StableKeyInterner,
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
            stable_key: interner.intern(format!("symbol:{name}:{}", id.0)),
            precision: SymbolPrecision::ExactLocal,
        }
    }
    fn definition_fact(
        interner: &StableKeyInterner,
        id: polint_core::DefinitionId,
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
            stable_key: interner.intern(format!("definition:{name}:{}", id.0)),
            precision: SymbolPrecision::ExactLocal,
        }
    }
    fn reference_fact(
        interner: &StableKeyInterner,
        id: polint_core::ReferenceId,
        name: &str,
        location: (FileId, u32),
        target: Option<SymbolId>,
        candidates: Vec<SymbolId>,
        status: SymbolResolutionStatus,
    ) -> ReferenceFact {
        let (file, col) = location;
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
            stable_key: interner.intern(format!("reference:{name}:{}", id.0)),
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
    fn query_helpers_return_rows_by_identity_and_file() {
        let (db, app_file, button, theme) = build_db();
        assert_eq!(
            symbol_by_id(&db, button).map(|row| row.name.as_str()),
            Some("Button")
        );
        assert_eq!(symbols_for_file(&db, app_file).count(), 2);
        assert_eq!(
            symbols_by_name(&db, "Button")
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            vec![SymbolId(20), SymbolId(30)]
        );
        assert_eq!(definitions_for_symbol(&db, button).count(), 1);
        assert_eq!(
            references_to_symbol(&db, theme)
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            vec![polint_core::ReferenceId(40)]
        );
    }

    #[test]
    fn query_helpers_return_deterministic_status_and_file_order() {
        let (db, app_file, _button, _theme) = build_db();
        assert_eq!(
            references_for_file(&db, app_file)
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            vec![
                polint_core::ReferenceId(40),
                polint_core::ReferenceId(50),
                polint_core::ReferenceId(60)
            ]
        );
        assert_eq!(
            unresolved_references(&db)
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            vec![polint_core::ReferenceId(50)]
        );
        assert_eq!(
            ambiguous_references(&db)
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            vec![polint_core::ReferenceId(60)]
        );
    }
}
