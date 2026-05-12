#[cfg(test)]
mod symbol_graph_stable_ids {
    use crate::core::{
        DefinitionId, Language, ReferenceId, Span, SymbolId, SymbolKind, SymbolNamespace,
    };
    use crate::symbol_graph::stable_id::{
        StableDefinitionKey, StableReferenceKey, StableSymbolKey, definition_id_from_key,
        reference_id_from_key, symbol_id_from_key,
    };

    fn span(start_byte: u32, end_byte: u32) -> Span {
        Span {
            file: crate::core::FileId(7),
            start_byte,
            end_byte,
            start_line: 3,
            start_col: 5,
            end_line: 3,
            end_col: 9,
        }
    }

    fn button_key() -> StableSymbolKey {
        StableSymbolKey::new(
            Language::TypeScript,
            Some("module:ui".to_string()),
            Some("package:web".to_string()),
            Some("src/button.ts".to_string()),
            vec!["Button".to_string()],
            SymbolNamespace::Value,
            SymbolKind::Function,
            "render".to_string(),
            Some(span(10, 16)),
        )
    }

    #[test]
    fn identical_symbol_keys_produce_identical_ids() {
        let left = button_key();
        let right = button_key();

        assert_eq!(symbol_id_from_key(&left), symbol_id_from_key(&right));
    }

    #[test]
    fn symbol_id_changes_when_semantic_namespace_changes() {
        let mut changed = button_key();
        changed.set_namespace(SymbolNamespace::Type);

        assert_ne!(symbol_id_from_key(&button_key()), symbol_id_from_key(&changed));
    }

    #[test]
    fn symbol_id_changes_when_kind_changes() {
        let mut changed = button_key();
        changed.set_kind(SymbolKind::Class);

        assert_ne!(symbol_id_from_key(&button_key()), symbol_id_from_key(&changed));
    }

    #[test]
    fn definition_id_changes_when_definition_span_changes() {
        let symbol = button_key();
        let first = StableDefinitionKey::new(symbol.clone(), "src/button.ts", span(10, 16));
        let second = StableDefinitionKey::new(symbol, "src/button.ts", span(20, 26));

        assert_ne!(definition_id_from_key(&first), definition_id_from_key(&second));
    }

    #[test]
    fn reference_id_changes_when_reference_span_changes() {
        let target = button_key();
        let first = StableReferenceKey::resolved(target.clone(), "src/app.ts", span(30, 36));
        let second = StableReferenceKey::resolved(target, "src/app.ts", span(40, 46));

        assert_ne!(reference_id_from_key(&first), reference_id_from_key(&second));
    }

    #[test]
    fn stable_keys_are_debug_safe_and_length_prefixed() {
        let key = button_key();
        let encoded = key.stable_key();

        assert!(encoded.contains("10:src/button.ts"));
        assert!(!encoded.contains("function Button()"));
    }

    #[test]
    fn stable_ids_use_core_newtypes() {
        let symbol = symbol_id_from_key(&button_key());
        let definition = definition_id_from_key(&StableDefinitionKey::new(
            button_key(),
            "src/button.ts",
            span(10, 16),
        ));
        let reference = reference_id_from_key(&StableReferenceKey::unresolved(
            Language::TypeScript,
            "src/app.ts",
            "missing",
            span(30, 36),
        ));

        assert!(matches!(symbol, SymbolId(_)));
        assert!(matches!(definition, DefinitionId(_)));
        assert!(matches!(reference, ReferenceId(_)));
    }
}
