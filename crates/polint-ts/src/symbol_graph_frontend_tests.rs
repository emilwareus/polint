#[cfg(test)]
mod symbol_graph_ts_local_symbols {
    use super::derive_ts_symbols;
    use super::TsSymbolOptions;
    use polint_analysis::LocalAnalysisDb;
    use polint_analysis::symbol_graph::{model::SymbolGraphBuilder, SymbolGraphRequest};
    use polint_analysis_api::{DefinitionFact, SymbolFact, SymbolKind, SymbolNamespace, SymbolPrecision};
    use polint_core::{FileId, Language};
    use std::fs;
    use std::path::Path;

    fn add_file(db: &mut LocalAnalysisDb, root: &Path, relative_path: &str, source: &str) -> FileId {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("test file has parent")).expect("create fixture");
        fs::write(&path, source).expect("write fixture");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn derive_ts_symbol_facts(
        source: &str,
    ) -> (
        polint_core::StableKeyInterner,
        Vec<SymbolFact>,
        Vec<DefinitionFact>,
    ) {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = LocalAnalysisDb::new();
        add_file(&mut db, temp.path(), "src/component.ts", source);
        let interner = db.stable_key_interner();
        let mut builder = SymbolGraphBuilder::new(interner.clone());

        derive_ts_symbols(
            &mut builder,
            &db,
            TsSymbolOptions {
                request: SymbolGraphRequest::new(true, false),
                resolved_imports: Vec::new(),
                module_nodes: Vec::new(),
            },
        );
        let output = builder.finish();
        (interner, output.symbols, output.definitions)
    }

    fn symbol<'a>(symbols: &'a [SymbolFact], name: &str) -> &'a SymbolFact {
        symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap_or_else(|| panic!("missing symbol `{name}` in {symbols:#?}"))
    }

    #[test]
    fn extracts_exported_and_local_symbols_from_oxc_semantic_data() {
        let (_interner, symbols, _definitions) = derive_ts_symbol_facts(
            r#"
export function exportedFn(param: string) {
    const localValue = param;
    return localValue;
}

export class Widget {}
"#,
        );

        let exported_fn = symbol(&symbols, "exportedFn");
        assert_eq!(exported_fn.language, Language::TypeScript);
        assert_eq!(exported_fn.kind, SymbolKind::Function);
        assert_eq!(exported_fn.namespace, SymbolNamespace::Value);
        assert!(exported_fn.is_exported);
        assert_eq!(exported_fn.precision, SymbolPrecision::ExactLocal);

        let widget = symbol(&symbols, "Widget");
        assert_eq!(widget.kind, SymbolKind::Class);
        assert!(widget.is_exported);

        let local_value = symbol(&symbols, "localValue");
        assert_eq!(local_value.kind, SymbolKind::Constant);
        assert!(!local_value.is_exported);

        let parameter = symbol(&symbols, "param");
        assert_eq!(parameter.kind, SymbolKind::Parameter);
    }

    #[test]
    fn declaration_merging_emits_one_symbol_with_multiple_definitions() {
        let (_interner, symbols, definitions) = derive_ts_symbol_facts(
            r#"
export interface MergeMe {
    one: string;
}

export interface MergeMe {
    two: string;
}
"#,
        );

        let merged = symbol(&symbols, "MergeMe");
        assert_eq!(merged.kind, SymbolKind::Interface);
        assert_eq!(merged.namespace, SymbolNamespace::Type);
        assert!(merged.is_exported);

        let merged_definitions = definitions
            .iter()
            .filter(|definition| definition.symbol == merged.id)
            .count();
        assert_eq!(merged_definitions, 2);
    }

    #[test]
    fn stable_symbol_ids_are_deterministic_across_repeated_extraction() {
        let source = "export const answer = 42;\n";
        let (first_interner, first_symbols, first_definitions) = derive_ts_symbol_facts(source);
        let (second_interner, second_symbols, second_definitions) = derive_ts_symbol_facts(source);

        let first = first_symbols
            .iter()
            .map(|symbol| {
                (
                    symbol.name.as_str(),
                    symbol.id,
                    first_interner.resolve(symbol.stable_key),
                )
            })
            .collect::<Vec<_>>();
        let second = second_symbols
            .iter()
            .map(|symbol| {
                (
                    symbol.name.as_str(),
                    symbol.id,
                    second_interner.resolve(symbol.stable_key),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(first, second);

        let first = first_definitions
            .iter()
            .map(|definition| {
                (
                    definition.name.as_str(),
                    definition.id,
                    definition.symbol,
                    first_interner.resolve(definition.stable_key),
                )
            })
            .collect::<Vec<_>>();
        let second = second_definitions
            .iter()
            .map(|definition| {
                (
                    definition.name.as_str(),
                    definition.id,
                    definition.symbol,
                    second_interner.resolve(definition.stable_key),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(first, second);
    }

    #[test]
    fn uses_repo_relative_file_keys_without_source_snippets() {
        let (interner, symbols, _definitions) =
            derive_ts_symbol_facts("export const answer = 42;\n");
        let answer = symbol(&symbols, "answer");
        let stable_key = interner.resolve(answer.stable_key);

        assert!(stable_key.contains("src/component.ts"));
        assert!(!stable_key.contains("export const answer"));
    }
}


#[cfg(test)]
mod symbol_graph_ts_references {
    use super::derive_ts_symbols;
    use super::TsSymbolOptions;
    use polint_analysis::LocalAnalysisDb;
    use polint_analysis::symbol_graph::{model::SymbolGraphBuilder, SymbolGraphRequest};
    use polint_analysis_api::{ReferenceFact, ReferenceKind, SymbolFact, SymbolPrecision, SymbolResolutionStatus};
    use polint_core::FileId;
    use std::fs;
    use std::path::Path;

    fn add_file(db: &mut LocalAnalysisDb, root: &Path, relative_path: &str, source: &str) -> FileId {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("test file has parent")).expect("create fixture");
        fs::write(&path, source).expect("write fixture");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn derive_ts_reference_facts(
        source: &str,
    ) -> (
        polint_core::StableKeyInterner,
        Vec<SymbolFact>,
        Vec<ReferenceFact>,
    ) {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = LocalAnalysisDb::new();
        add_file(&mut db, temp.path(), "src/component.ts", source);
        let interner = db.stable_key_interner();
        let mut builder = SymbolGraphBuilder::new(interner.clone());

        derive_ts_symbols(
            &mut builder,
            &db,
            TsSymbolOptions {
                request: SymbolGraphRequest::new(true, true),
                resolved_imports: Vec::new(),
                module_nodes: Vec::new(),
            },
        );
        let output = builder.finish();
        (interner, output.symbols, output.references)
    }

    fn symbol<'a>(symbols: &'a [SymbolFact], name: &str) -> &'a SymbolFact {
        symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .unwrap_or_else(|| panic!("missing symbol `{name}` in {symbols:#?}"))
    }

    fn references_to<'a>(
        symbols: &[SymbolFact],
        references: &'a [ReferenceFact],
        name: &str,
    ) -> Vec<&'a ReferenceFact> {
        let target = symbol(symbols, name).id;
        references
            .iter()
            .filter(|reference| reference.target == Some(target))
            .collect()
    }

    #[test]
    fn extracts_resolved_reference_kinds_from_oxc_semantic_data() {
        let (_interner, symbols, references) = derive_ts_reference_facts(
            r#"
type Model = { value: number };

function helper(input: Model) {
    return input.value;
}

let count = 0;
count = helper({ value: count });
"#,
        );

        assert!(references_to(&symbols, &references, "helper").iter().any(
            |reference| reference.kind == ReferenceKind::Call
                && reference.status == SymbolResolutionStatus::Resolved
                && reference.precision == SymbolPrecision::ExactLocal
        ));
        assert!(
            references_to(&symbols, &references, "Model")
                .iter()
                .any(|reference| reference.kind == ReferenceKind::TypeUse)
        );
        assert!(
            references_to(&symbols, &references, "input")
                .iter()
                .any(|reference| reference.kind == ReferenceKind::Read)
        );
        assert!(
            references_to(&symbols, &references, "count")
                .iter()
                .any(|reference| reference.kind == ReferenceKind::Write)
        );
    }

    #[test]
    fn emits_visible_unresolved_references_from_root_unresolved_set() {
        let (_interner, _symbols, references) = derive_ts_reference_facts(
            r#"
export function run() {
    return missingValue + anotherMissing();
}
"#,
        );

        assert!(references.iter().any(|reference| {
            reference.name == "missingValue"
                && reference.target.is_none()
                && reference.status == SymbolResolutionStatus::Unresolved
                && reference.precision == SymbolPrecision::Unresolved
        }));
        assert!(references.iter().any(|reference| {
            reference.name == "anotherMissing"
                && reference.kind == ReferenceKind::Call
                && reference.status == SymbolResolutionStatus::Unresolved
        }));
    }

    #[test]
    fn stable_reference_ids_are_deterministic_across_repeated_extraction() {
        let source = "const value = 1;\nexport const doubled = value + value;\n";
        let (first_interner, _first_symbols, first_references) = derive_ts_reference_facts(source);
        let (second_interner, _second_symbols, second_references) =
            derive_ts_reference_facts(source);

        let first = first_references
            .iter()
            .map(|reference| {
                (
                    reference.name.as_str(),
                    reference.id,
                    reference.target,
                    reference.status,
                    first_interner.resolve(reference.stable_key),
                )
            })
            .collect::<Vec<_>>();
        let second = second_references
            .iter()
            .map(|reference| {
                (
                    reference.name.as_str(),
                    reference.id,
                    reference.target,
                    reference.status,
                    second_interner.resolve(reference.stable_key),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(first, second);
    }
}
