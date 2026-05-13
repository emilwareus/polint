use crate::analysis_plan::AnalysisPlan;
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, Language, SourceFile};
use crate::symbol_graph::model::SymbolGraphBuilder;
use crate::symbol_graph::{LanguageSymbolOutput, unsupported_language_support};

pub(crate) fn derive_go_symbols(
    builder: &mut SymbolGraphBuilder,
    db: &AnalysisDb,
    _loaded: &LoadedConfig,
    plan: &AnalysisPlan,
) -> LanguageSymbolOutput {
    let files = go_files(db);
    if files.is_empty() {
        return LanguageSymbolOutput::default();
    }

    let mut output = LanguageSymbolOutput::default();
    output
        .capability_support
        .extend(unsupported_language_support(
            plan,
            Language::Go,
            "Go symbol and reference extraction is not implemented in this plan.",
            "Typed Go package providers are promoted by the follow-up symbol extraction plan.",
        ));

    if plan.requests_capability("references") {
        for file in files {
            builder.add_unsupported_reference(
                file.language,
                file.id,
                file.relative_path.clone(),
                "<unsupported>",
            );
        }
    }

    output
}

fn go_files(db: &AnalysisDb) -> Vec<&SourceFile> {
    let mut files = db
        .files()
        .iter()
        .filter(|file| file.language == Language::Go)
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files
}

#[cfg(test)]
mod symbol_graph_go_setup {
    use super::*;
    use crate::core::{CapabilitySupportStatus, SymbolPrecision, SymbolResolutionStatus};
    use std::path::Path;

    fn add_go_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().expect("fixture has parent")).expect("mkdirs");
        std::fs::write(&path, source).expect("write Go fixture");
        db.add_file(path, relative_path.to_string(), source.to_string());
    }

    fn loaded_config_for(root: &Path) -> LoadedConfig {
        crate::config::load_config(root).expect("config loads")
    }

    fn requested_plan() -> AnalysisPlan {
        AnalysisPlan::from_capability_names_for_test(&["symbols", "references"])
    }

    #[test]
    fn go_symbol_config_parses_string_and_array_settings() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join(".polint.toml"),
            r#"
[languages.go]
package_patterns = ["./cmd/...", "./pkg/..."]
build_tags = "enterprise,polint"
include_tests = false
"#,
        )
        .expect("write config");

        let config = GoSymbolConfig::from_loaded(&loaded_config_for(temp.path()));

        assert_eq!(
            config,
            GoSymbolConfig {
                package_patterns: vec!["./cmd/...".to_string(), "./pkg/...".to_string()],
                build_tags: vec!["enterprise".to_string(), "polint".to_string()],
                include_tests: false,
            }
        );
    }

    #[test]
    fn missing_go_mod_reports_setup_missing_for_requested_capabilities() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package main\n");
        let mut builder = SymbolGraphBuilder::new();

        let output =
            derive_go_symbols(&mut builder, &db, &loaded_config_for(temp.path()), &requested_plan());
        let graph = builder.finish();

        assert_eq!(
            output
                .capability_support
                .iter()
                .map(|entry| (entry.capability.as_str(), entry.status.clone()))
                .collect::<Vec<_>>(),
            vec![
                ("references", CapabilitySupportStatus::SetupMissing),
                ("symbols", CapabilitySupportStatus::SetupMissing),
            ]
        );
        assert!(graph.references.iter().all(|reference| {
            reference.status == SymbolResolutionStatus::SetupMissing
                && reference.precision == SymbolPrecision::SetupMissing
        }));
    }

    #[test]
    fn sidecar_command_failure_reports_setup_missing() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module example.com/app\n\ngo 1.25.0\n")
            .expect("write go.mod");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package main\n");
        let mut builder = SymbolGraphBuilder::new();

        let output = derive_go_symbols_with_runner(
            &mut builder,
            &db,
            &loaded_config_for(temp.path()),
            &requested_plan(),
            |_config| Err(GoSidecarFailure::CommandFailed("sidecar failed".to_string())),
        );

        assert!(
            output.capability_support.iter().all(|entry| {
                entry.status == CapabilitySupportStatus::SetupMissing
                    && entry.reason.as_deref().is_some_and(|reason| reason.contains("sidecar failed"))
            }),
            "{:#?}",
            output.capability_support
        );
    }

    #[test]
    fn invalid_sidecar_json_reports_setup_missing() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module example.com/app\n\ngo 1.25.0\n")
            .expect("write go.mod");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package main\n");
        let mut builder = SymbolGraphBuilder::new();

        let output = derive_go_symbols_with_runner(
            &mut builder,
            &db,
            &loaded_config_for(temp.path()),
            &requested_plan(),
            |_config| Ok(br#"{"schema":"wrong"}"#.to_vec()),
        );

        assert!(
            output.capability_support.iter().all(|entry| {
                entry.status == CapabilitySupportStatus::SetupMissing
                    && entry.reason.as_deref().is_some_and(|reason| {
                        reason.contains("invalid Go symbol sidecar JSON")
                            || reason.contains("unsupported Go symbol sidecar schema")
                    })
            }),
            "{:#?}",
            output.capability_support
        );
    }

    #[test]
    fn repo_escaping_sidecar_file_path_reports_setup_missing() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module example.com/app\n\ngo 1.25.0\n")
            .expect("write go.mod");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package main\n");
        let mut builder = SymbolGraphBuilder::new();

        let output = derive_go_symbols_with_runner(
            &mut builder,
            &db,
            &loaded_config_for(temp.path()),
            &requested_plan(),
            |_config| {
                Ok(
                    br#"{
  "schema":"polint-go-symbols-v1",
  "go_version":"go1.26.2",
  "packages":[],
  "symbols":[{
    "key":"bad",
    "package_id":"example.com/app",
    "package_path":"example.com/app",
    "test_variant":"regular",
    "file":"../outside.go",
    "name":"Bad",
    "qualified_name":"Bad",
    "namespace":"value",
    "kind":"function",
    "span":{"start_byte":0,"end_byte":3,"start_line":1,"start_column":1,"end_line":1,"end_column":4},
    "exported":true
  }],
  "definitions":[],
  "references":[]
}"#
                    .to_vec(),
                )
            },
        );

        assert!(
            output.capability_support.iter().all(|entry| {
                entry.status == CapabilitySupportStatus::SetupMissing
                    && entry.reason.as_deref().is_some_and(|reason| reason.contains("escapes repository"))
            }),
            "{:#?}",
            output.capability_support
        );
    }
}
