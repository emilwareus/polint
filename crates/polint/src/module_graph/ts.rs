use crate::core::{ResolutionStatus, UnresolvedReason};
use crate::module_graph::model::{ResolvedImportDraft, ResolverInput};

pub(crate) fn resolve_ts_import(input: ResolverInput<'_>) -> ResolvedImportDraft {
    let _ = (
        input.root,
        input.db.files().len(),
        input.owner_module,
        input.owner_package,
    );
    if !input.import.language.is_ts_family() {
        return ResolvedImportDraft::unsupported_language();
    }

    let mut draft = ResolvedImportDraft::unresolved(UnresolvedReason::NotFound);
    draft.status = ResolutionStatus::Unresolved;
    draft
}

#[cfg(test)]
mod tests {
    use super::resolve_ts_import;
    use crate::core::{
        AnalysisDb, ImportFact, ImportId, Language, ResolutionStatus, Span, UnresolvedReason,
    };
    use crate::module_graph::model::ResolverInput;
    use std::path::{Path, PathBuf};

    #[test]
    fn module_graph_resolver_contracts_ts_unmatched_import_is_not_found() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import missing from './missing';\n".to_string(),
        );
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "./missing".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
        });
        let import = &db.imports()[0];

        let draft = resolve_ts_import(ResolverInput {
            root: Path::new("."),
            db: &db,
            import,
            owner_module: None,
            owner_package: None,
        });

        assert_eq!(draft.status, ResolutionStatus::Unresolved);
        assert_eq!(draft.reason, Some(UnresolvedReason::NotFound));
    }
}
