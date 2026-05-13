pub(crate) mod go;
pub(crate) mod model;
pub(crate) mod query;
pub(crate) mod stable_id;
pub(crate) mod ts;

use crate::analysis_plan::AnalysisPlan;
use crate::config::LoadedConfig;
use crate::core::{
    AnalysisDb, CapabilitySupport, CapabilitySupportStatus, CapabilitySupportView, Language,
};
use crate::diagnostics::{Diagnostic, TextRange};
use model::SymbolGraphBuilder;

const SYMBOL_GRAPH_CAPABILITIES: &[&str] = &["symbols", "references"];
const SYMBOL_FACTS_DOCS_PATH: &str = "docs/facts/symbols-and-references.md";

#[derive(Debug, Clone, Default)]
pub(crate) struct SymbolGraphDerivation {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) capability_support: Vec<CapabilitySupport>,
}

impl SymbolGraphDerivation {
    pub(crate) fn support_view(&self, base: &CapabilitySupportView) -> CapabilitySupportView {
        let mut entries = base.entries().to_vec();
        for override_entry in &self.capability_support {
            if let Some(existing) = entries.iter_mut().find(|entry| {
                entry.capability == override_entry.capability
                    && entry.language == override_entry.language
            }) {
                *existing = override_entry.clone();
            } else {
                entries.push(override_entry.clone());
            }
        }
        CapabilitySupportView::new(entries)
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LanguageSymbolOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) capability_support: Vec<CapabilitySupport>,
}

pub(crate) fn derive_requested_symbols(
    db: &mut AnalysisDb,
    loaded: &LoadedConfig,
    plan: &AnalysisPlan,
) -> SymbolGraphDerivation {
    if !plan.requests_any_capability(SYMBOL_GRAPH_CAPABILITIES) {
        return SymbolGraphDerivation::default();
    }

    let mut builder = SymbolGraphBuilder::new();
    let mut derivation = SymbolGraphDerivation::default();

    merge_language_output(
        &mut derivation,
        ts::derive_ts_symbols(&mut builder, db, loaded, plan),
    );
    merge_language_output(
        &mut derivation,
        go::derive_go_symbols(&mut builder, db, loaded, plan),
    );

    let output = builder.finish();
    db.replace_symbol_graph_facts(output.symbols, output.definitions, output.references);
    derivation.diagnostics.extend(output.diagnostics);
    derivation
        .diagnostics
        .extend(capability_diagnostics(&derivation.capability_support));
    sort_symbol_derivation(&mut derivation);
    derivation
}

fn merge_language_output(derivation: &mut SymbolGraphDerivation, output: LanguageSymbolOutput) {
    derivation.diagnostics.extend(output.diagnostics);
    derivation
        .capability_support
        .extend(output.capability_support);
}

fn capability_diagnostics(support: &[CapabilitySupport]) -> Vec<Diagnostic> {
    support
        .iter()
        .filter(|entry| entry.status != CapabilitySupportStatus::Supported)
        .flat_map(|entry| {
            entry
                .rules
                .iter()
                .map(|rule_id| capability_diagnostic(entry, rule_id))
        })
        .collect()
}

fn capability_diagnostic(entry: &CapabilitySupport, rule_id: &str) -> Diagnostic {
    let language = entry.language.map(language_name).unwrap_or("workspace");
    let status = capability_status_json(&entry.status);
    let docs_path = entry.docs_path.as_deref().unwrap_or(SYMBOL_FACTS_DOCS_PATH);
    let reason = entry
        .reason
        .as_deref()
        .unwrap_or("Symbol/reference provider support is unavailable.");
    let mut diagnostic = Diagnostic::error(
        "polint/capability",
        "<workspace>",
        TextRange::point(1, 1),
        format!(
            "Rule `{rule_id}` requested capability `{}` for {language}, but symbol graph provider support is {status}.",
            entry.capability
        ),
    )
    .with_evidence("rule", rule_id.to_string())
    .with_evidence("capability", entry.capability.clone())
    .with_evidence("language", language.to_string())
    .with_evidence("status", status.to_string())
    .with_evidence("reason", reason.to_string())
    .with_evidence("docs_path", docs_path.to_string())
    .with_help(format!(
        "Capability `{}` is recognized but the {language} symbol/reference provider is not available yet; see {docs_path}.",
        entry.capability
    ));
    if let Some(hint) = &entry.hint {
        diagnostic = diagnostic.with_evidence("hint", hint.clone());
    }
    diagnostic
}

fn sort_symbol_derivation(derivation: &mut SymbolGraphDerivation) {
    derivation.capability_support.sort_by(|left, right| {
        (
            left.capability.as_str(),
            left.language,
            left.rules.as_slice(),
            left.reason.as_deref(),
            left.hint.as_deref(),
            left.docs_path.as_deref(),
        )
            .cmp(&(
                right.capability.as_str(),
                right.language,
                right.rules.as_slice(),
                right.reason.as_deref(),
                right.hint.as_deref(),
                right.docs_path.as_deref(),
            ))
    });
    derivation.diagnostics.sort_by(|left, right| {
        (
            left.rule_id.as_str(),
            left.file.as_str(),
            left.range.start_line,
            left.range.start_col,
            left.message.as_str(),
            left.stable_fingerprint.as_str(),
        )
            .cmp(&(
                right.rule_id.as_str(),
                right.file.as_str(),
                right.range.start_line,
                right.range.start_col,
                right.message.as_str(),
                right.stable_fingerprint.as_str(),
            ))
    });
}

fn language_name(language: Language) -> &'static str {
    match language {
        Language::Go => "Go",
        Language::TypeScript => "TypeScript",
        Language::Tsx => "TSX",
        Language::JavaScript => "JavaScript",
        Language::Jsx => "JSX",
        Language::Unknown => "unknown",
    }
}

fn capability_status_json(status: &CapabilitySupportStatus) -> &'static str {
    match status {
        CapabilitySupportStatus::Supported => "supported",
        CapabilitySupportStatus::Unsupported => "unsupported",
        CapabilitySupportStatus::SetupMissing => "setup_missing",
    }
}

#[cfg(test)]
mod symbol_graph_derivation {
    use super::{SymbolGraphDerivation, derive_requested_symbols};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, CapabilitySupport, CapabilitySupportStatus, CapabilitySupportView,
        DefinitionFact, DefinitionId, DefinitionKind, FileId, Language, ReferenceFact, ReferenceId,
        ReferenceKind, Span, SymbolFact, SymbolId, SymbolKind, SymbolNamespace, SymbolPrecision,
        SymbolResolutionStatus,
    };
    use std::path::Path;

    type SymbolRows = Vec<(Language, String, SymbolPrecision)>;
    type ReferenceRows = Vec<(Language, String, SymbolResolutionStatus, SymbolPrecision)>;
    type SupportRows = Vec<(String, Option<Language>, CapabilitySupportStatus)>;
    type DeriveSnapshot = (SymbolRows, ReferenceRows, SupportRows, Vec<String>);

    fn loaded_config_for(root: &Path) -> crate::config::LoadedConfig {
        load_config(root).expect("default config loads")
    }

    fn add_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) -> FileId {
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

        let derivation = derive_requested_symbols(
            &mut db,
            &loaded_config_for(temp.path()),
            &AnalysisPlan::empty(),
        );

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
            support
                .entries()
                .iter()
                .map(|entry| (
                    entry.capability.as_str(),
                    entry.language,
                    entry.status.clone(),
                ))
                .collect::<Vec<_>>(),
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
        fn derive_once() -> DeriveSnapshot {
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
                db.symbols()
                    .iter()
                    .map(|symbol| (symbol.language, symbol.name.clone(), symbol.precision))
                    .collect(),
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
                    .map(|entry| {
                        (
                            entry.capability.clone(),
                            entry.language,
                            entry.status.clone(),
                        )
                    })
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
        assert!(first.0.iter().any(|(_, name, _)| name == "value"));
        assert!(first.0.iter().all(|(_, name, _)| name != "stale"));
        assert!(first.1.iter().all(|(_, name, _, _)| name != "stale"));
    }
}
