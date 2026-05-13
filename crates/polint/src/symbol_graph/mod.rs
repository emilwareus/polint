pub(crate) mod model;
pub(crate) mod query;
pub(crate) mod stable_id;

#[cfg(test)]
mod symbol_graph_derivation {
    use super::{SymbolGraphDerivation, derive_requested_symbols};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, CapabilitySupport, CapabilitySupportStatus, CapabilitySupportView,
        DefinitionFact, DefinitionId, DefinitionKind, FileId, Language, ReferenceFact,
        ReferenceId, ReferenceKind, Span, SymbolFact, SymbolId, SymbolKind, SymbolNamespace,
        SymbolPrecision, SymbolResolutionStatus,
    };
    use std::path::Path;

    fn loaded_config_for(root: &Path) -> crate::config::LoadedConfig {
        load_config(root).expect("default config loads")
    }

    fn add_file(
        db: &mut AnalysisDb,
        root: &Path,
        relative_path: &str,
        source: &str,
    ) -> FileId {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().expect("test file has parent")).expect("mkdirs");
        std::fs::write(&path, source).expect("write fixture file");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn stale_symbol_fact(file: FileId) -> SymbolFact {
        SymbolFact {
            id: SymbolId(999),
            language: Language::TypeScript,
            name: "stale".to_string(),
            qualified_name: "stale".to_string(),
            kind: SymbolKind::Unknown,
            namespace: SymbolNamespace::Unknown,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            primary_span: Some(Span::point(file, 1, 1)),
            is_exported: false,
            stable_key: "stale:symbol".to_string(),
            precision: SymbolPrecision::Unsupported,
        }
    }

    fn stale_definition_fact(file: FileId) -> DefinitionFact {
        DefinitionFact {
            id: DefinitionId(999),
            symbol: SymbolId(999),
            language: Language::TypeScript,
            name: "stale".to_string(),
            qualified_name: "stale".to_string(),
            kind: DefinitionKind::Unknown,
            namespace: SymbolNamespace::Unknown,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            primary_span: Some(Span::point(file, 1, 1)),
            is_primary: false,
            is_exported: false,
            stable_key: "stale:definition".to_string(),
            precision: SymbolPrecision::Unsupported,
        }
    }

    fn stale_reference_fact(file: FileId) -> ReferenceFact {
        ReferenceFact {
            id: ReferenceId(999),
            language: Language::TypeScript,
            name: "stale".to_string(),
            qualified_name: "stale".to_string(),
            kind: ReferenceKind::Unknown,
            namespace: SymbolNamespace::Unknown,
            file: Some(file),
            package: None,
            module: None,
            owner: None,
            primary_span: Some(Span::point(file, 1, 1)),
            target: None,
            candidates: Vec::new(),
            stable_key: "stale:reference".to_string(),
            status: SymbolResolutionStatus::Unsupported,
            precision: SymbolPrecision::Unsupported,
        }
    }

    #[test]
    fn provider_defaults_when_symbol_capabilities_are_not_requested() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        add_file(
            &mut db,
            temp.path(),
            "src/app.ts",
            "export const value = 1;\n",
        );

        let derivation =
            derive_requested_symbols(&mut db, &loaded_config_for(temp.path()), &AnalysisPlan::empty());

        assert!(derivation.diagnostics.is_empty());
        assert!(derivation.capability_support.is_empty());
        assert!(db.symbols().is_empty());
        assert!(db.definitions().is_empty());
        assert!(db.references().is_empty());
    }

    #[test]
    fn support_view_merges_symbol_provider_rows_in_order() {
        let derivation = SymbolGraphDerivation {
            diagnostics: Vec::new(),
            capability_support: vec![
                CapabilitySupport {
                    capability: "symbols".to_string(),
                    language: Some(Language::TypeScript),
                    status: CapabilitySupportStatus::Unsupported,
                    rules: vec!["local/symbols".to_string()],
                    reason: Some("symbol extraction pending".to_string()),
                    hint: None,
                    docs_path: Some("docs/facts/symbols-and-references.md".to_string()),
                },
                CapabilitySupport {
                    capability: "references".to_string(),
                    language: Some(Language::Go),
                    status: CapabilitySupportStatus::Unsupported,
                    rules: vec!["local/references".to_string()],
                    reason: Some("reference extraction pending".to_string()),
                    hint: None,
                    docs_path: Some("docs/facts/symbols-and-references.md".to_string()),
                },
            ],
        };
        let base = CapabilitySupportView::new(vec![CapabilitySupport {
            capability: "symbols".to_string(),
            language: Some(Language::TypeScript),
            status: CapabilitySupportStatus::Supported,
            rules: vec!["local/symbols".to_string()],
            reason: None,
            hint: None,
            docs_path: None,
        }]);

        let support = derivation.support_view(&base);

        assert_eq!(
            support.entries().iter().map(|entry| (
                entry.capability.as_str(),
                entry.language,
                entry.status.clone(),
            )).collect::<Vec<_>>(),
            vec![
                (
                    "symbols",
                    Some(Language::TypeScript),
                    CapabilitySupportStatus::Unsupported
                ),
                (
                    "references",
                    Some(Language::Go),
                    CapabilitySupportStatus::Unsupported
                ),
            ]
        );
    }

    #[test]
    fn requested_symbol_derivation_replaces_facts_deterministically() {
        fn derive_once() -> (
            Vec<(Language, String, SymbolResolutionStatus, SymbolPrecision)>,
            Vec<(String, Option<Language>, CapabilitySupportStatus)>,
            Vec<String>,
        ) {
            let temp = tempfile::tempdir().expect("tempdir");
            let mut db = AnalysisDb::new();
            let app = add_file(
                &mut db,
                temp.path(),
                "src/app.ts",
                "export const value = 1;\n",
            );
            add_file(&mut db, temp.path(), "lib/main.go", "package lib\n");
            db.replace_symbol_graph_facts(
                vec![stale_symbol_fact(app)],
                vec![stale_definition_fact(app)],
                vec![stale_reference_fact(app)],
            );

            let derivation = derive_requested_symbols(
                &mut db,
                &loaded_config_for(temp.path()),
                &AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]),
            );

            (
                db.references()
                    .iter()
                    .map(|reference| {
                        (
                            reference.language,
                            reference.name.clone(),
                            reference.status,
                            reference.precision,
                        )
                    })
                    .collect(),
                derivation
                    .capability_support
                    .iter()
                    .map(|entry| (entry.capability.clone(), entry.language, entry.status.clone()))
                    .collect(),
                derivation
                    .diagnostics
                    .iter()
                    .map(|diagnostic| diagnostic.stable_fingerprint.clone())
                    .collect(),
            )
        }

        let first = derive_once();
        let second = derive_once();

        assert_eq!(first, second);
        assert!(first.0.iter().all(|(_, name, status, precision)| {
            name == "<unsupported>"
                && *status == SymbolResolutionStatus::Unsupported
                && *precision == SymbolPrecision::Unsupported
        }));
    }
}
