#[cfg(test)]
mod symbol_graph_go {
    use super::*;
    use crate::local_db::LocalFactDb;
    use std::collections::BTreeMap;
    use polint_analysis::symbol_graph::{
        model::SymbolGraphOutput, LanguageSymbolOutput, SymbolGraphRequest,
    };
    use polint_analysis_api::{
        DefinitionFact, DefinitionKind, ReferenceFact, ReferenceKind, SymbolFact, SymbolKind,
        SymbolPrecision, SymbolResolutionStatus,
    };
    use polint_core::{FileId, StableKeyInterner, SymbolId};
    use std::path::Path;
    use polint_analysis::symbol_graph::SymbolCapabilityStatus;

    fn derive_go_fixture(
        files: &[(&str, &str)],
    ) -> Option<(SymbolGraphOutput, LanguageSymbolOutput)> {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/app\n\ngo 1.24.0\n",
        )
        .expect("write go.mod");
        let mut db = LocalFactDb::new();
        for (relative_path, source) in files {
            add_go_file(&mut db, temp.path(), relative_path, source);
        }
        let mut builder = SymbolGraphBuilder::new(StableKeyInterner::default());
        let output = derive_go_symbols(
            &mut builder,
            &db,
            &GoSymbolOptions {
                root: temp.path().to_path_buf(),
                settings: BTreeMap::new(),
                request: SymbolGraphRequest::new(true, true),
                reference_files: None,
            },
        );
        if output
            .capability_support
            .iter()
            .any(|entry| entry.status == SymbolCapabilityStatus::SetupMissing)
        {
            eprintln!(
                "skipping Go sidecar-backed symbol test; setup missing: {:#?}",
                output.capability_support
            );
            return None;
        }
        assert!(
            !output.capability_support.is_empty()
                && output
                    .capability_support
                    .iter()
                    .all(|entry| entry.status == SymbolCapabilityStatus::Supported),
            "expected supported Go symbol capabilities; support = {:#?}; diagnostics = {:#?}",
            output.capability_support,
            output.diagnostics
        );
        Some((builder.finish(), output))
    }

    fn add_go_file(db: &mut LocalFactDb, root: &Path, relative_path: &str, source: &str) {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().expect("fixture has parent")).expect("mkdirs");
        std::fs::write(&path, source).expect("write Go fixture");
        db.add_file(path, relative_path.to_string(), source.to_string());
    }


    fn symbol<'a>(symbols: &'a [SymbolFact], name: &str, kind: SymbolKind) -> &'a SymbolFact {
        symbols
            .iter()
            .find(|symbol| symbol.name == name && symbol.kind == kind)
            .unwrap_or_else(|| panic!("missing {kind:?} symbol {name}; symbols = {symbols:#?}"))
    }

    fn primary_definition(definitions: &[DefinitionFact], symbol_id: SymbolId) -> &DefinitionFact {
        definitions
            .iter()
            .find(|definition| definition.symbol == symbol_id && definition.is_primary)
            .unwrap_or_else(|| {
                panic!("missing definition for {symbol_id:?}; definitions = {definitions:#?}")
            })
    }

    fn resolved_reference(
        references: &[ReferenceFact],
        target: SymbolId,
        kind: ReferenceKind,
    ) -> &ReferenceFact {
        references
            .iter()
            .find(|reference| {
                reference.target == Some(target)
                    && reference.kind == kind
                    && reference.status == SymbolResolutionStatus::Resolved
            })
            .unwrap_or_else(|| {
                panic!("missing {kind:?} reference to {target:?}; references = {references:#?}")
            })
    }

    fn file_id(db: &LocalFactDb, relative_path: &str) -> FileId {
        db.files()
            .iter()
            .find(|file| file.relative_path == relative_path)
            .map(|file| file.id)
            .unwrap_or_else(|| panic!("missing file {relative_path}"))
    }

    #[test]
    fn go_function_definition_and_call_reference_are_exact_semantic() {
        let Some((graph, output)) = derive_go_fixture(&[(
            "main.go",
            r#"package app

func Build() int {
	return 41
}

func Use() int {
	return Build() + 1
}
"#,
        )]) else {
            return;
        };

        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        let build = symbol(&graph.symbols, "Build", SymbolKind::Function);
        assert_eq!(build.precision, SymbolPrecision::ExactSemantic);
        assert!(build.file.is_some());
        let definition = primary_definition(&graph.definitions, build.id);
        assert_eq!(definition.kind, DefinitionKind::Declaration);
        let reference = resolved_reference(&graph.references, build.id, ReferenceKind::Call);
        assert_eq!(reference.precision, SymbolPrecision::ExactSemantic);
    }

    #[test]
    fn go_method_and_field_selector_references_are_exact_semantic() {
        let Some((graph, output)) = derive_go_fixture(&[(
            "widget.go",
            r#"package app

type Widget struct {
	Name string
}

func (w Widget) Label() string {
	return w.Name
}

func Use(w Widget) string {
	return w.Label()
}
"#,
        )]) else {
            return;
        };

        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        let method = symbol(&graph.symbols, "Label", SymbolKind::Method);
        let field = symbol(&graph.symbols, "Name", SymbolKind::Field);
        let method_reference =
            resolved_reference(&graph.references, method.id, ReferenceKind::Call);
        let field_reference =
            resolved_reference(&graph.references, field.id, ReferenceKind::MemberAccess);
        assert_eq!(method_reference.precision, SymbolPrecision::ExactSemantic);
        assert_eq!(field_reference.precision, SymbolPrecision::ExactSemantic);
    }

    #[test]
    fn go_package_qualified_external_call_is_resolved_call_reference() {
        let Some((graph, output)) = derive_go_fixture(&[(
            "main.go",
            r#"package app

import "fmt"

func Use() {
	fmt.Println("ok")
}
"#,
        )]) else {
            return;
        };

        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        let println = symbol(&graph.symbols, "Println", SymbolKind::Function);
        assert_eq!(println.qualified_name, "fmt.Println");
        assert_eq!(println.file, None);
        let reference = resolved_reference(&graph.references, println.id, ReferenceKind::Call);
        assert_eq!(reference.name, "Println");
        assert_eq!(reference.precision, SymbolPrecision::ExactSemantic);
    }

    #[test]
    fn go_multi_module_monorepo_infers_module_roots_without_repo_go_mod() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("services/app")).expect("mkdir service");
        std::fs::create_dir_all(temp.path().join("libs/shared")).expect("mkdir lib");
        std::fs::write(
            temp.path().join("services/app/go.mod"),
            r#"module example.com/app

go 1.24

require example.com/shared v0.0.0
"#,
        )
        .expect("write app go.mod");
        std::fs::write(
            temp.path().join("libs/shared/go.mod"),
            r#"module example.com/shared

go 1.24
"#,
        )
        .expect("write shared go.mod");
        let mut db = LocalFactDb::new();
        add_go_file(
            &mut db,
            temp.path(),
            "services/app/main.go",
            r#"package app

import "example.com/shared"

func Use() string {
	return shared.Build()
}
"#,
        );
        add_go_file(
            &mut db,
            temp.path(),
            "libs/shared/shared.go",
            r#"package shared

func Build() string {
	return "ok"
}
"#,
        );
        let mut builder = SymbolGraphBuilder::new(StableKeyInterner::default());
        let output = derive_go_symbols(
            &mut builder,
            &db,
            &GoSymbolOptions {
                root: temp.path().to_path_buf(),
                settings: BTreeMap::new(),
                request: SymbolGraphRequest::new(true, true),
                reference_files: None,
            },
        );
        if output
            .capability_support
            .iter()
            .any(|entry| entry.status == SymbolCapabilityStatus::SetupMissing)
        {
            eprintln!(
                "skipping Go sidecar-backed monorepo test; setup missing: {:#?}",
                output.capability_support
            );
            return;
        }

        assert!(output.diagnostics.is_empty(), "{:#?}", output.diagnostics);
        let graph = builder.finish();
        assert!(
            graph.diagnostics.is_empty(),
            "monorepo derivation should not produce duplicate symbol diagnostics: {:#?}",
            graph.diagnostics
        );
        let build = symbol(&graph.symbols, "Build", SymbolKind::Function);
        let definition = primary_definition(&graph.definitions, build.id);
        assert_eq!(definition.file, Some(file_id(&db, "libs/shared/shared.go")));
        let reference = resolved_reference(&graph.references, build.id, ReferenceKind::Call);
        assert_eq!(reference.file, Some(file_id(&db, "services/app/main.go")));
    }

    #[test]
    fn unknown_go_reference_precision_is_unsupported() {
        assert_eq!(
            reference_precision("sidecar_typo"),
            SymbolPrecision::Unsupported
        );
    }

    #[test]
    fn go_package_objectpath_symbol_id_survives_unrelated_file_move() {
        let Some((first, _)) = derive_go_fixture(&[
            (
                "main.go",
                r#"package app

func Build() int {
	return 1
}
"#,
            ),
            ("unused/a.go", "package app\n\nconst Unused = 1\n"),
        ]) else {
            return;
        };
        let Some((second, _)) = derive_go_fixture(&[
            (
                "main.go",
                r#"package app

func Build() int {
	return 1
}
"#,
            ),
            ("other/a.go", "package app\n\nconst Unused = 1\n"),
        ]) else {
            return;
        };

        assert_eq!(
            symbol(&first.symbols, "Build", SymbolKind::Function).id,
            symbol(&second.symbols, "Build", SymbolKind::Function).id
        );
    }

    #[test]
    fn go_local_variable_id_is_stable_for_same_file_and_owner_chain() {
        let source = r#"package app

func Use() int {
	local := 41
	return local + 1
}
"#;
        let Some((first, _)) = derive_go_fixture(&[("main.go", source)]) else {
            return;
        };
        let Some((second, _)) = derive_go_fixture(&[("main.go", source)]) else {
            return;
        };

        assert_eq!(
            symbol(&first.symbols, "local", SymbolKind::Variable).id,
            symbol(&second.symbols, "local", SymbolKind::Variable).id
        );
    }


}
