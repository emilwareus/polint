use crate::core::{Language, ResolutionStatus, UnresolvedReason};
use crate::module_graph::model::{ResolvedImportDraft, ResolverInput};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct GoPackageIndex {
    packages: BTreeMap<String, ()>,
}

impl GoPackageIndex {
    fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}

pub(crate) fn resolve_go_import(
    input: ResolverInput<'_>,
    metadata: &GoPackageIndex,
) -> ResolvedImportDraft {
    let _ = (
        input.root,
        input.db.files().len(),
        input.owner_module,
        input.owner_package,
    );
    if input.import.language != Language::Go {
        return ResolvedImportDraft::unsupported_language();
    }
    if metadata.is_empty() {
        return ResolvedImportDraft::setup_missing();
    }

    let mut draft = ResolvedImportDraft::unresolved(UnresolvedReason::NotFound);
    draft.status = ResolutionStatus::Unresolved;
    draft
}

#[cfg(test)]
mod tests {
    use super::{GoPackageIndex, resolve_go_import};
    use crate::core::{
        AnalysisDb, ImportFact, ImportId, Language, ResolutionStatus, Span, UnresolvedReason,
    };
    use crate::module_graph::model::ResolverInput;
    use std::path::{Path, PathBuf};

    #[test]
    fn module_graph_resolver_contracts_go_without_metadata_is_setup_missing() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("main.go"),
            "main.go".to_string(),
            "package main\nimport \"example.com/project/pkg\"\n".to_string(),
        );
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "example.com/project/pkg".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::Go,
        });
        let import = &db.imports()[0];

        let draft = resolve_go_import(
            ResolverInput {
                root: Path::new("."),
                db: &db,
                import,
                owner_module: None,
                owner_package: None,
            },
            &GoPackageIndex::default(),
        );

        assert_eq!(draft.status, ResolutionStatus::SetupMissing);
        assert_eq!(draft.reason, Some(UnresolvedReason::SetupMissing));
    }
}
