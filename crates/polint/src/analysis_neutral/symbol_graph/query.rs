use crate::analysis_api::{
    DefinitionFact, FactDatabase, ReferenceFact, SymbolFact, SymbolResolutionStatus,
};
use crate::internal_core::{FileId, Span, SymbolId};

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
    let mut references = db
        .references()
        .iter()
        .filter(move |reference| reference.file == Some(file))
        .collect::<Vec<_>>();
    references.sort_by(|left, right| reference_order(db, left, right));
    references.into_iter()
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
    use crate::analysis_api::{
        DefinitionKind, ReferenceKind, SymbolKind, SymbolNamespace, SymbolPrecision,
    };
    use crate::analysis_neutral::LocalAnalysisDb;
    use crate::internal_core::{Language, ModuleNodeId, Span, StableKeyInterner};
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
        let button = SymbolId::from_raw(20);
        let theme = SymbolId::from_raw(10);
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
                    SymbolId::from_raw(30),
                    "Button",
                    app_file,
                    10,
                    SymbolKind::Class,
                ),
            ],
            vec![definition_fact(
                &interner,
                crate::internal_core::DefinitionId::from_raw(30),
                button,
                "Button",
                app_file,
                1,
            )],
            vec![
                reference_fact(
                    &interner,
                    crate::internal_core::ReferenceId::from_raw(60),
                    "ambiguous",
                    (app_file, 44),
                    None,
                    vec![button, theme],
                    SymbolResolutionStatus::Ambiguous,
                ),
                reference_fact(
                    &interner,
                    crate::internal_core::ReferenceId::from_raw(50),
                    "missing",
                    (app_file, 35),
                    None,
                    Vec::new(),
                    SymbolResolutionStatus::Unresolved,
                ),
                reference_fact(
                    &interner,
                    crate::internal_core::ReferenceId::from_raw(40),
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
        SymbolFact::new(
            id,
            Language::TypeScript,
            name.to_string(),
            format!("src/app.ts::{name}"),
            kind,
            SymbolNamespace::Value,
            Some(file),
            None,
            Some(ModuleNodeId::from_raw(0)),
            None,
            Some(Span::point(file, 1, col)),
            true,
            interner.intern(format!("symbol:{name}:{}", id.0)),
            SymbolPrecision::ExactLocal,
        )
    }
    fn definition_fact(
        interner: &StableKeyInterner,
        id: crate::internal_core::DefinitionId,
        symbol: SymbolId,
        name: &str,
        file: FileId,
        col: u32,
    ) -> DefinitionFact {
        DefinitionFact::new(
            id,
            symbol,
            Language::TypeScript,
            name.to_string(),
            format!("src/app.ts::{name}"),
            DefinitionKind::Declaration,
            SymbolNamespace::Value,
            Some(file),
            None,
            Some(ModuleNodeId::from_raw(0)),
            None,
            Some(Span::point(file, 1, col)),
            true,
            true,
            interner.intern(format!("definition:{name}:{}", id.0)),
            SymbolPrecision::ExactLocal,
        )
    }
    fn reference_fact(
        interner: &StableKeyInterner,
        id: crate::internal_core::ReferenceId,
        name: &str,
        location: (FileId, u32),
        target: Option<SymbolId>,
        candidates: Vec<SymbolId>,
        status: SymbolResolutionStatus,
    ) -> ReferenceFact {
        let (file, col) = location;
        ReferenceFact::new(
            id,
            Language::TypeScript,
            name.to_string(),
            name.to_string(),
            ReferenceKind::Read,
            SymbolNamespace::Value,
            Some(file),
            None,
            Some(ModuleNodeId::from_raw(0)),
            None,
            Some(Span::point(file, 1, col)),
            target,
            candidates,
            interner.intern(format!("reference:{name}:{}", id.0)),
            status,
            match status {
                SymbolResolutionStatus::Resolved => SymbolPrecision::ExactLocal,
                SymbolResolutionStatus::Unresolved => SymbolPrecision::Unresolved,
                SymbolResolutionStatus::Ambiguous => SymbolPrecision::Ambiguous,
                SymbolResolutionStatus::SetupMissing => SymbolPrecision::SetupMissing,
                SymbolResolutionStatus::Unsupported => SymbolPrecision::Unsupported,
                _ => SymbolPrecision::Unresolved,
            },
        )
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
            vec![SymbolId::from_raw(20), SymbolId::from_raw(30)]
        );
        assert_eq!(definitions_for_symbol(&db, button).count(), 1);
        assert_eq!(
            references_to_symbol(&db, theme)
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            vec![crate::internal_core::ReferenceId::from_raw(40)]
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
                crate::internal_core::ReferenceId::from_raw(40),
                crate::internal_core::ReferenceId::from_raw(50),
                crate::internal_core::ReferenceId::from_raw(60)
            ]
        );
        assert_eq!(
            unresolved_references(&db)
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            vec![crate::internal_core::ReferenceId::from_raw(50)]
        );
        assert_eq!(
            ambiguous_references(&db)
                .map(|row| row.id)
                .collect::<Vec<_>>(),
            vec![crate::internal_core::ReferenceId::from_raw(60)]
        );
    }
}
