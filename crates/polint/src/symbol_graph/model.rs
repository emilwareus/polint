#[cfg(test)]
mod symbol_graph_builder {
    use crate::core::{
        DefinitionKind, FileId, Language, ReferenceKind, Span, SymbolKind, SymbolNamespace,
        SymbolPrecision, SymbolResolutionStatus,
    };
    use crate::symbol_graph::model::{
        DefinitionDraft, ReferenceDraft, SymbolDraft, SymbolGraphBuilder,
    };

    fn span(file: FileId, start_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte: start_byte + 5,
            start_line: 1,
            start_col: start_byte + 1,
            end_line: 1,
            end_col: start_byte + 6,
        }
    }

    fn symbol_draft(name: &str, file: FileId, start_byte: u32) -> SymbolDraft {
        SymbolDraft {
            language: Language::TypeScript,
            name: name.to_string(),
            qualified_name: format!("src/app.ts::{name}"),
            kind: SymbolKind::Function,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            module_key: Some("module:app".to_string()),
            package_key: Some("package:web".to_string()),
            file_key: Some("src/app.ts".to_string()),
            owner_chain: Vec::new(),
            primary_span: Some(span(file, start_byte)),
            is_exported: true,
            precision: SymbolPrecision::ExactLocal,
        }
    }

    fn definition_draft(name: &str, file: FileId, start_byte: u32) -> DefinitionDraft {
        DefinitionDraft {
            language: Language::TypeScript,
            name: name.to_string(),
            qualified_name: format!("src/app.ts::{name}"),
            kind: DefinitionKind::Declaration,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            file_key: "src/app.ts".to_string(),
            primary_span: Some(span(file, start_byte)),
            is_primary: true,
            is_exported: true,
            precision: SymbolPrecision::ExactLocal,
        }
    }

    fn reference_draft(name: &str, file: FileId, start_byte: u32) -> ReferenceDraft {
        ReferenceDraft {
            language: Language::TypeScript,
            name: name.to_string(),
            qualified_name: format!("src/app.ts::{name}"),
            kind: ReferenceKind::Read,
            namespace: SymbolNamespace::Value,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            file_key: "src/app.ts".to_string(),
            primary_span: Some(span(file, start_byte)),
            precision: SymbolPrecision::ExactLocal,
        }
    }

    #[test]
    fn output_order_is_deterministic_across_insertion_order() {
        let file = FileId(0);
        let mut left = SymbolGraphBuilder::new();
        let zeta = left.add_symbol(symbol_draft("zeta", file, 20));
        let alpha = left.add_symbol(symbol_draft("alpha", file, 10));
        left.add_definition(zeta, definition_draft("zeta", file, 20));
        left.add_definition(alpha, definition_draft("alpha", file, 10));
        left.add_reference(zeta, reference_draft("zeta", file, 80));
        left.add_reference(alpha, reference_draft("alpha", file, 70));

        let mut right = SymbolGraphBuilder::new();
        let alpha = right.add_symbol(symbol_draft("alpha", file, 10));
        let zeta = right.add_symbol(symbol_draft("zeta", file, 20));
        right.add_reference(alpha, reference_draft("alpha", file, 70));
        right.add_reference(zeta, reference_draft("zeta", file, 80));
        right.add_definition(alpha, definition_draft("alpha", file, 10));
        right.add_definition(zeta, definition_draft("zeta", file, 20));

        let left = left.finish();
        let right = right.finish();

        assert_eq!(
            left.symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "zeta"]
        );
        assert_eq!(
            left.symbols
                .iter()
                .map(|symbol| symbol.stable_key.as_str())
                .collect::<Vec<_>>(),
            right
                .symbols
                .iter()
                .map(|symbol| symbol.stable_key.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            left.references
                .iter()
                .map(|reference| reference.name.as_str())
                .collect::<Vec<_>>(),
            right
                .references
                .iter()
                .map(|reference| reference.name.as_str())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn id_collisions_emit_deterministic_internal_diagnostics() {
        fn constant_hash(_: &str) -> String {
            "0000000000000001".to_string()
        }

        let file = FileId(0);
        let mut builder = SymbolGraphBuilder::with_hash_for_test(constant_hash);
        let alpha = builder.add_symbol(symbol_draft("alpha", file, 10));
        let beta = builder.add_symbol(symbol_draft("beta", file, 20));
        let output = builder.finish();

        assert_eq!(alpha, beta);
        assert_eq!(
            output
                .symbols
                .iter()
                .map(|symbol| symbol.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "beta"]
        );
        assert_eq!(output.diagnostics[0].rule_id, "polint/internal");
        assert_eq!(output.diagnostics[0].evidence[0].label, "id");
    }

    #[test]
    fn precision_and_reference_status_are_preserved() {
        let file = FileId(0);
        let mut builder = SymbolGraphBuilder::new();
        let mut symbol = symbol_draft("alpha", file, 10);
        symbol.precision = SymbolPrecision::ExactSemantic;
        let alpha = builder.add_symbol(symbol);
        let mut definition = definition_draft("alpha", file, 10);
        definition.precision = SymbolPrecision::ModuleLinked;
        builder.add_definition(alpha, definition);
        builder.add_reference(alpha, reference_draft("alpha", file, 40));
        let mut ambiguous = reference_draft("ambiguous", file, 50);
        ambiguous.precision = SymbolPrecision::Ambiguous;
        builder.add_ambiguous_reference(vec![alpha], ambiguous);
        let mut unresolved = reference_draft("missing", file, 60);
        unresolved.precision = SymbolPrecision::Unresolved;
        builder.add_unresolved_reference(unresolved);
        builder.add_setup_missing_reference(Language::Go, file, "cmd/main.go", "go/types");
        builder.add_unsupported_reference(Language::Unknown, file, "README.md", "markdown");

        let output = builder.finish();

        assert_eq!(output.symbols[0].precision, SymbolPrecision::ExactSemantic);
        assert_eq!(
            output.definitions[0].precision,
            SymbolPrecision::ModuleLinked
        );
        assert_eq!(
            output
                .references
                .iter()
                .map(|reference| reference.status)
                .collect::<Vec<_>>(),
            vec![
                SymbolResolutionStatus::Resolved,
                SymbolResolutionStatus::Ambiguous,
                SymbolResolutionStatus::Unresolved,
                SymbolResolutionStatus::SetupMissing,
                SymbolResolutionStatus::Unsupported,
            ]
        );
    }
}
