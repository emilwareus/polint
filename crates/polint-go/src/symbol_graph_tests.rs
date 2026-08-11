#[cfg(test)]
mod sidecar_semantic_output {
    use crate::symbol_graph::*;

    #[test]
    fn semantic_schema_defaults_missing_arrays_to_empty() {
        let output = parse_sidecar_output(
            br#"{
  "schema":"polint-go-symbols-semantic-1",
  "go_version":"go1.24.13",
  "packages":[],
  "symbols":[],
  "definitions":[],
  "references":[],
  "scopes":null,
  "imports":null,
  "exports":null,
  "resolution_steps":null,
  "errors":null
}"#,
        )
        .expect("semantic sidecar output parses");

        assert!(output.scopes.is_empty());
        assert!(output.imports.is_empty());
        assert!(output.exports.is_empty());
        assert!(output.resolution_steps.is_empty());
    }

    #[test]
    fn semantic_schema_parses_scopes_imports_exports_and_resolution_steps() {
        let output = parse_sidecar_output(
            br#"{
  "schema":"polint-go-symbols-semantic-1",
  "go_version":"go1.24.13",
  "packages":[],
  "symbols":[],
  "definitions":[],
  "references":[],
  "scopes":[{
    "key":"go:scope:package:example.com/app",
    "parent_key":"",
    "kind":"package",
    "package_path":"example.com/app",
    "file":"",
    "span":{"start_byte":0,"end_byte":0,"start_line":1,"start_column":1,"end_line":1,"end_column":1}
  }],
  "imports":[{
    "path":"fmt",
    "local_name":"named",
    "alias_kind":"named",
    "file":"main.go",
    "span":{"start_byte":8,"end_byte":11,"start_line":3,"start_column":8,"end_line":3,"end_column":11}
  }],
  "exports":[{
    "symbol_key":"go:package|package:example.com/app|name:Build",
    "export_name":"Build",
    "namespace":"value",
    "object_path":"Build",
    "package_path":"example.com/app",
    "generated":false
  }],
  "resolution_steps":[{
    "reference_key":"go:reference:main.go:Build",
    "step":"LexicalLookup",
    "status":"resolved",
    "target_key":"go:package|package:example.com/app|name:Build",
    "candidate_keys":["go:package|package:example.com/app|name:Build"]
  }],
  "errors":[]
}"#,
        )
        .expect("semantic sidecar output parses");

        assert_eq!(output.scopes[0].kind, "package");
        assert_eq!(output.imports[0].alias_kind, "named");
        assert_eq!(output.exports[0].object_path, "Build");
        assert_eq!(output.resolution_steps[0].candidate_keys.len(), 1);
    }
}

#[cfg(test)]
mod semantic_conversion {
    use crate::symbol_graph::*;
    use polint_analysis::symbol_graph::semantic::{
        AliasKind, ScopeKind, SemanticImportKind, SemanticStatus,
    };
    use polint_analysis_api::SourceFile;
    use polint_core::FileId;
    use polint_core::Language;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn source_file(relative_path: &str, source: &str) -> SourceFile {
        SourceFile {
            id: FileId(0),
            path: PathBuf::from(relative_path),
            relative_path: relative_path.to_string(),
            language: Language::Go,
            source: source.to_string().into(),
            content_hash: "test-hash".to_string(),
        }
    }

    fn derive(json: &[u8]) -> polint_analysis::symbol_graph::semantic::SemanticIndexOutput {
        let file = source_file("main.go", "package app\n");
        let files = BTreeMap::from([(file.relative_path.as_str(), &file)]);
        let sidecar = parse_sidecar_output(json).expect("sidecar fixture parses");

        derive_go_semantic_index(
            &polint_core::test_stable_key_interner(),
            &sidecar,
            &files,
            &BTreeMap::new(),
            &BTreeMap::new(),
        )
    }

    #[test]
    fn converts_go_scope_rows_with_parent_links() {
        let output = derive(
            br#"{
  "schema":"polint-go-symbols-semantic-1",
  "go_version":"go1.24.13",
  "packages":[],
  "symbols":[],
  "definitions":[],
  "references":[],
  "scopes":[
    {"key":"pkg","parent_key":"","kind":"package","package_path":"example.com/app","file":"","span":{"start_byte":0,"end_byte":0,"start_line":1,"start_column":1,"end_line":1,"end_column":1}},
    {"key":"file","parent_key":"pkg","kind":"file","package_path":"example.com/app","file":"main.go","span":{"start_byte":0,"end_byte":11,"start_line":1,"start_column":1,"end_line":1,"end_column":12}}
  ],
  "imports":[],
  "exports":[],
  "resolution_steps":[],
  "errors":[]
}"#,
        );

        let package = output
            .scopes
            .iter()
            .find(|scope| scope.kind == ScopeKind::Package)
            .expect("package scope");
        let file = output
            .scopes
            .iter()
            .find(|scope| scope.kind == ScopeKind::File)
            .expect("file scope");

        assert_eq!(file.parent, Some(package.id));
    }

    #[test]
    fn converts_go_import_alias_rows_with_honest_statuses() {
        let output = derive(
            br#"{
  "schema":"polint-go-symbols-semantic-1",
  "go_version":"go1.24.13",
  "packages":[],
  "symbols":[],
  "definitions":[],
  "references":[],
  "scopes":[],
  "imports":[
    {"path":"fmt","local_name":"named","alias_kind":"named","file":"main.go","span":{"start_byte":15,"end_byte":26,"start_line":3,"start_column":2,"end_line":3,"end_column":13}},
    {"path":"strings","local_name":"","alias_kind":"implicit","file":"main.go","span":{"start_byte":27,"end_byte":36,"start_line":4,"start_column":2,"end_line":4,"end_column":11}},
    {"path":"math","local_name":".","alias_kind":"dot","file":"main.go","span":{"start_byte":37,"end_byte":45,"start_line":5,"start_column":2,"end_line":5,"end_column":10}},
    {"path":"net/http/pprof","local_name":"_","alias_kind":"blank","file":"main.go","span":{"start_byte":46,"end_byte":64,"start_line":6,"start_column":2,"end_line":6,"end_column":20}}
  ],
  "exports":[],
  "resolution_steps":[
    {"reference_key":"go:import|file:main.go|path:fmt|local:named|span:15-26","step":"Package","status":"resolved","target_key":"fmt.Println","candidate_keys":["fmt.Println"]},
    {"reference_key":"go:import|file:main.go|path:math|local:.|span:37-45","step":"Package","status":"ambiguous","target_key":"","candidate_keys":["math.Max","math.Min"]}
  ],
  "errors":[]
}"#,
        );

        assert!(output.semantic_imports.iter().any(|fact| {
            fact.kind == SemanticImportKind::GoNamed && fact.status == SemanticStatus::Resolved
        }));
        assert!(output.semantic_imports.iter().any(|fact| {
            fact.kind == SemanticImportKind::GoDot && fact.status == SemanticStatus::Ambiguous
        }));
        assert!(output.semantic_imports.iter().any(|fact| {
            fact.kind == SemanticImportKind::GoBlank && fact.status == SemanticStatus::Resolved
        }));
        assert!(output.semantic_imports.iter().any(|fact| {
            fact.kind == SemanticImportKind::GoImplicit && fact.status == SemanticStatus::Resolved
        }));
        assert!(output.aliases.iter().any(|alias| {
            alias.kind == AliasKind::ImportAlias
                && alias.status == SemanticStatus::Resolved
                && alias
                    .target_symbol_stable_keys
                    .iter()
                    .map(|key| {
                        polint_core::test_stable_key_interner()
                            .resolve(*key)
                            .to_string()
                    })
                    .collect::<Vec<_>>()
                    == vec!["fmt.Println".to_string()]
        }));
        assert!(output.aliases.iter().any(|alias| {
            alias.kind == AliasKind::ImportAlias && alias.status == SemanticStatus::Ambiguous
        }));
    }

    #[test]
    fn converts_go_exports_to_stable_export_identities() {
        let output = derive(
            br#"{
  "schema":"polint-go-symbols-semantic-1",
  "go_version":"go1.24.13",
  "packages":[],
  "symbols":[],
  "definitions":[],
  "references":[],
  "scopes":[],
  "imports":[],
  "exports":[{
    "symbol_key":"go:package|package:example.com/app|name:Build",
    "export_name":"Build",
    "namespace":"value",
    "object_path":"Build",
    "package_path":"example.com/app",
    "generated":false
  }],
  "resolution_steps":[],
  "errors":[]
}"#,
        );

        assert!(output.stable_exports.iter().any(|export| {
            export.export_name == "Build"
                && export.package_key.as_deref() == Some("go:package:example.com/app")
                && polint_core::test_stable_key_interner()
                    .resolve(export.symbol_stable_key)
                    .as_ref()
                    == "go:package|package:example.com/app|name:Build"
                && export.generated_discriminator.as_deref() == Some("native")
        }));
    }
}

#[cfg(test)]
mod semantic_setup_missing {
    use crate::symbol_graph::*;
    use polint_analysis::symbol_graph::model::SymbolGraphBuilder;
    use polint_analysis::symbol_graph::semantic::{ResolutionStepKind, SemanticStatus};
    use polint_analysis_api::{ReferenceKind, SourceFile, SymbolResolutionStatus};
    use polint_core::FileId;
    use polint_core::Language;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn source_file(relative_path: &str, source: &str) -> SourceFile {
        SourceFile {
            id: FileId(0),
            path: PathBuf::from(relative_path),
            relative_path: relative_path.to_string(),
            language: Language::Go,
            source: source.to_string().into(),
            content_hash: "test-hash".to_string(),
        }
    }

    fn parse(json: &[u8]) -> GoSidecarOutput {
        parse_sidecar_output(json).expect("sidecar fixture parses")
    }

    #[test]
    fn setup_missing_files_get_unknown_fallback_semantic_rows() {
        let file = source_file("main.go", "package app\n");
        let files = vec![&file];

        let output = setup_missing_semantic_index_for_files(
            &polint_core::test_stable_key_interner(),
            &files,
        );

        assert!(output.resolutions.iter().any(|resolution| {
            resolution.step == ResolutionStepKind::UnknownFallback
                && resolution.status == SemanticStatus::SetupMissing
                && polint_core::test_stable_key_interner()
                    .resolve(resolution.source_stable_key)
                    .contains("main.go")
        }));
    }

    #[test]
    fn sidecar_reference_without_target_or_candidates_gets_unresolved_unknown_fallback() {
        let file = source_file("main.go", "package app\n");
        let files = BTreeMap::from([(file.relative_path.as_str(), &file)]);
        let sidecar = parse(
            br#"{
  "schema":"polint-go-symbols-semantic-1",
  "go_version":"go1.24.13",
  "packages":[],
  "symbols":[],
  "definitions":[],
  "references":[{
    "package_id":"example.com/app",
    "file":"main.go",
    "name":"Missing",
    "target_key":"",
    "kind":"call",
    "span":{"start_byte":12,"end_byte":19,"start_line":3,"start_column":2,"end_line":3,"end_column":9},
    "precision":"exact_semantic"
  }],
  "scopes":[],
  "imports":[],
  "exports":[],
  "resolution_steps":[],
  "errors":[]
}"#,
        );

        let output = derive_go_semantic_index(
            &polint_core::test_stable_key_interner(),
            &sidecar,
            &files,
            &BTreeMap::new(),
            &BTreeMap::new(),
        );

        assert!(output.resolutions.iter().any(|resolution| {
            resolution.step == ResolutionStepKind::UnknownFallback
                && resolution.status == SemanticStatus::Unresolved
                && polint_core::test_stable_key_interner()
                    .resolve(resolution.source_stable_key)
                    .contains("Missing")
        }));
    }

    #[test]
    fn sidecar_candidate_sets_become_ambiguous_public_references() {
        let file = source_file("main.go", "package app\nfunc Use() { Thing() }\n");
        let sidecar = parse(
            br#"{
  "schema":"polint-go-symbols-semantic-1",
  "go_version":"go1.24.13",
  "packages":[],
  "symbols":[
    {"key":"one","package_id":"example.com/app","package_path":"example.com/app","test_variant":"regular","file":"main.go","name":"Thing","qualified_name":"Thing","namespace":"value","kind":"function","span":{"start_byte":12,"end_byte":17,"start_line":2,"start_column":6,"end_line":2,"end_column":11},"exported":true},
    {"key":"two","package_id":"example.com/app","package_path":"example.com/app","test_variant":"regular","file":"main.go","name":"Thing","qualified_name":"Thing","namespace":"value","kind":"function","span":{"start_byte":18,"end_byte":23,"start_line":2,"start_column":12,"end_line":2,"end_column":17},"exported":true}
  ],
  "definitions":[],
  "references":[{
    "package_id":"example.com/app",
    "file":"main.go",
    "name":"Thing",
    "target_key":"",
    "kind":"call",
    "span":{"start_byte":26,"end_byte":31,"start_line":2,"start_column":20,"end_line":2,"end_column":25},
    "precision":"exact_semantic"
  }],
  "scopes":[],
  "imports":[],
  "exports":[],
  "resolution_steps":[{
    "reference_key":"example.com/app|main.go|Thing||call|26|31",
    "step":"UnknownFallback",
    "status":"ambiguous",
    "target_key":"",
    "candidate_keys":["one","two"]
  }],
  "errors":[]
}"#,
        );
        let mut builder = SymbolGraphBuilder::new(polint_core::StableKeyInterner::default());

        convert_sidecar_output(&mut builder, &analysis_db_with(file), &sidecar);
        let graph = builder.finish();

        assert!(graph.references.iter().any(|reference| {
            reference.kind == ReferenceKind::Call
                && reference.status == SymbolResolutionStatus::Ambiguous
                && reference.candidates.len() == 2
        }));
    }

    fn analysis_db_with(file: SourceFile) -> crate::local_db::LocalFactDb {
        let mut db = crate::local_db::LocalFactDb::new();
        db.add_file(
            file.path.clone(),
            file.relative_path.clone(),
            file.source.to_string(),
        );
        db
    }
}
