use crate::analysis_plan::AnalysisPlan;
use crate::config::LoadedConfig;
use crate::core::{
    AnalysisDb, CapabilitySupport, CapabilitySupportStatus, FileId, Language, SourceFile,
};
use crate::symbol_graph::model::SymbolGraphBuilder;
use crate::symbol_graph::{LanguageSymbolOutput, SYMBOL_FACTS_DOCS_PATH};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const GO_SYMBOL_GRAPH_CAPABILITIES: &[&str] = &["symbols", "references"];
const GO_SYMBOL_SIDECAR_SCHEMA: &str = "polint-go-symbols-v1";

#[derive(Debug, Clone, PartialEq, Eq)]
struct GoSymbolConfig {
    package_patterns: Vec<String>,
    build_tags: Vec<String>,
    include_tests: bool,
}

#[derive(Debug, Clone)]
enum GoSidecarFailure {
    CommandUnavailable(String),
    CommandFailed(String),
    InvalidJson(String),
    InvalidPath(String),
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarOutput {
    schema: String,
    #[serde(default)]
    go_version: String,
    #[serde(default)]
    module_path: String,
    #[serde(default)]
    packages: Vec<GoSidecarPackage>,
    #[serde(default)]
    symbols: Vec<GoSidecarSymbol>,
    #[serde(default)]
    definitions: Vec<GoSidecarDefinition>,
    #[serde(default)]
    references: Vec<GoSidecarReference>,
    #[serde(default)]
    errors: Vec<GoSidecarPackageError>,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarPackage {
    id: String,
    path: String,
    name: String,
    #[serde(default)]
    module_path: String,
    test_variant: String,
    #[serde(default)]
    files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarSymbol {
    key: String,
    package_id: String,
    package_path: String,
    test_variant: String,
    #[serde(default)]
    file: String,
    #[serde(default)]
    owner_key: String,
    #[serde(default)]
    owner_chain: Vec<String>,
    name: String,
    qualified_name: String,
    namespace: String,
    kind: String,
    #[serde(default)]
    objectpath: String,
    span: GoSidecarSpan,
    #[serde(default)]
    exported: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarDefinition {
    symbol_key: String,
    package_id: String,
    #[serde(default)]
    file: String,
    name: String,
    kind: String,
    span: GoSidecarSpan,
    #[serde(default)]
    implicit: bool,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarReference {
    package_id: String,
    #[serde(default)]
    file: String,
    name: String,
    #[serde(default)]
    target_key: String,
    kind: String,
    span: GoSidecarSpan,
    precision: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarPackageError {
    package_id: String,
    package_path: String,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarSpan {
    start_byte: usize,
    end_byte: usize,
    start_line: u32,
    start_column: u32,
    end_line: u32,
    end_column: u32,
}

pub(crate) fn derive_go_symbols(
    builder: &mut SymbolGraphBuilder,
    db: &AnalysisDb,
    loaded: &LoadedConfig,
    plan: &AnalysisPlan,
) -> LanguageSymbolOutput {
    derive_go_symbols_with_runner(builder, db, loaded, plan, |config| {
        run_go_sidecar(loaded.root.as_path(), config)
    })
}

fn derive_go_symbols_with_runner(
    builder: &mut SymbolGraphBuilder,
    db: &AnalysisDb,
    loaded: &LoadedConfig,
    plan: &AnalysisPlan,
    runner: impl FnOnce(&GoSymbolConfig) -> Result<Vec<u8>, GoSidecarFailure>,
) -> LanguageSymbolOutput {
    let files = go_files(db);
    if files.is_empty() || !plan.requests_any_capability(GO_SYMBOL_GRAPH_CAPABILITIES) {
        return LanguageSymbolOutput::default();
    }

    if !loaded.root.join("go.mod").is_file() {
        return setup_missing_output(
            builder,
            &files,
            plan,
            "go.mod was not found at the repository root.",
            "Add a repository-root go.mod or configure Go symbol package loading in a future lifecycle hook.",
        );
    }

    let config = GoSymbolConfig::from_loaded(loaded);
    let stdout = match runner(&config) {
        Ok(stdout) => stdout,
        Err(error) => {
            return setup_missing_output(
                builder,
                &files,
                plan,
                &error.reason(),
                "Ensure Go is installed and the repository can be loaded with the configured package patterns.",
            );
        }
    };
    let sidecar = match parse_sidecar_output(&stdout).and_then(|output| validate_paths(output, db)) {
        Ok(output) => output,
        Err(error) => {
            return setup_missing_output(
                builder,
                &files,
                plan,
                &error.reason(),
                "Run go test ./... or adjust languages.go package_patterns/build_tags so Go packages load cleanly.",
            );
        }
    };

    let mut output = LanguageSymbolOutput::default();
    output
        .capability_support
        .extend(supported_language_support(plan));
    if !sidecar.errors.is_empty() {
        output.capability_support.extend(setup_support(
            plan,
            "Go packages loaded with errors; exact available facts were retained.",
            "Fix the reported Go package load errors for complete symbol/reference coverage.",
        ));
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

impl GoSymbolConfig {
    fn from_loaded(loaded: &LoadedConfig) -> Self {
        let settings = &loaded.config.languages.go;
        Self {
            package_patterns: string_or_array_setting(settings, "package_patterns", &["./..."]),
            build_tags: string_or_array_setting(settings, "build_tags", &[]),
            include_tests: settings
                .get("include_tests")
                .and_then(toml::Value::as_bool)
                .unwrap_or(true),
        }
    }
}

impl GoSidecarFailure {
    fn reason(&self) -> String {
        match self {
            Self::CommandUnavailable(message) => message.clone(),
            Self::CommandFailed(message) => message.clone(),
            Self::InvalidJson(message) => message.clone(),
            Self::InvalidPath(message) => message.clone(),
        }
    }
}

fn string_or_array_setting(
    settings: &BTreeMap<String, toml::Value>,
    key: &str,
    default: &[&str],
) -> Vec<String> {
    let Some(value) = settings.get(key) else {
        return default.iter().map(|value| (*value).to_string()).collect();
    };
    match value {
        toml::Value::String(value) => split_comma(value),
        toml::Value::Array(values) => values
            .iter()
            .filter_map(toml::Value::as_str)
            .flat_map(split_comma)
            .collect::<Vec<_>>(),
        _ => default.iter().map(|value| (*value).to_string()).collect(),
    }
}

fn split_comma(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn run_go_sidecar(root: &Path, config: &GoSymbolConfig) -> Result<Vec<u8>, GoSidecarFailure> {
    let sidecar_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/polint-go-symbols");
    let output = Command::new("go")
        .arg("run")
        .arg(sidecar_dir.as_os_str())
        .arg("symbols")
        .arg("--root")
        .arg(root.as_os_str())
        .arg("--patterns")
        .arg(config.package_patterns.join(","))
        .arg("--tests")
        .arg(config.include_tests.to_string())
        .arg("--build-tags")
        .arg(config.build_tags.join(","))
        .arg("--json")
        .current_dir(root)
        .env_remove("GOFLAGS")
        .output()
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                GoSidecarFailure::CommandUnavailable("go executable was not found.".to_string())
            } else {
                GoSidecarFailure::CommandFailed(format!("failed to start go sidecar: {error}"))
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let reason = if stderr.is_empty() {
            format!("go sidecar exited with status {}.", output.status)
        } else {
            format!("go sidecar exited with status {}: {stderr}", output.status)
        };
        return Err(GoSidecarFailure::CommandFailed(reason));
    }

    Ok(output.stdout)
}

fn parse_sidecar_output(stdout: &[u8]) -> Result<GoSidecarOutput, GoSidecarFailure> {
    let output: GoSidecarOutput = serde_json::from_slice(stdout).map_err(|error| {
        GoSidecarFailure::InvalidJson(format!("invalid Go symbol sidecar JSON: {error}"))
    })?;
    if output.schema != GO_SYMBOL_SIDECAR_SCHEMA {
        return Err(GoSidecarFailure::InvalidJson(format!(
            "unsupported Go symbol sidecar schema `{}`; expected `{GO_SYMBOL_SIDECAR_SCHEMA}`",
            output.schema
        )));
    }
    Ok(output)
}

fn validate_paths(
    mut output: GoSidecarOutput,
    db: &AnalysisDb,
) -> Result<GoSidecarOutput, GoSidecarFailure> {
    let file_ids = go_file_ids(db);
    for package in &mut output.packages {
        for file in &mut package.files {
            *file = validate_sidecar_path(file, &file_ids)?;
        }
    }
    for symbol in &mut output.symbols {
        if !symbol.file.is_empty() {
            symbol.file = validate_sidecar_path(&symbol.file, &file_ids)?;
        }
    }
    for definition in &mut output.definitions {
        if !definition.file.is_empty() {
            definition.file = validate_sidecar_path(&definition.file, &file_ids)?;
        }
    }
    for reference in &mut output.references {
        if !reference.file.is_empty() {
            reference.file = validate_sidecar_path(&reference.file, &file_ids)?;
        }
    }
    Ok(output)
}

fn go_file_ids(db: &AnalysisDb) -> BTreeMap<String, FileId> {
    db.files()
        .iter()
        .filter(|file| file.language == Language::Go)
        .map(|file| (file.relative_path.clone(), file.id))
        .collect()
}

fn validate_sidecar_path(
    raw_path: &str,
    file_ids: &BTreeMap<String, FileId>,
) -> Result<String, GoSidecarFailure> {
    let path = lexical_repo_relative(raw_path)?;
    if !file_ids.contains_key(&path) {
        return Err(GoSidecarFailure::InvalidPath(format!(
            "Go symbol sidecar file path `{path}` does not map to a discovered Go file."
        )));
    }
    Ok(path)
}

fn lexical_repo_relative(raw_path: &str) -> Result<String, GoSidecarFailure> {
    let path = Path::new(raw_path);
    if path.is_absolute() {
        return Err(GoSidecarFailure::InvalidPath(format!(
            "Go symbol sidecar file path `{raw_path}` is absolute and escapes repository."
        )));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(GoSidecarFailure::InvalidPath(format!(
                        "Go symbol sidecar file path `{raw_path}` escapes repository."
                    )));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(GoSidecarFailure::InvalidPath(format!(
                    "Go symbol sidecar file path `{raw_path}` escapes repository."
                )));
            }
        }
    }
    Ok(normalized.to_string_lossy().replace('\\', "/"))
}

fn setup_missing_output(
    builder: &mut SymbolGraphBuilder,
    files: &[&SourceFile],
    plan: &AnalysisPlan,
    reason: &str,
    hint: &str,
) -> LanguageSymbolOutput {
    if plan.requests_capability("references") {
        for file in files {
            builder.add_setup_missing_reference(
                file.language,
                file.id,
                file.relative_path.clone(),
                "<setup-missing>",
            );
        }
    }

    let mut output = LanguageSymbolOutput::default();
    output
        .capability_support
        .extend(setup_support(plan, reason, hint));
    output.capability_support.sort_by(|left, right| {
        (
            left.capability.as_str(),
            left.language,
            left.rules.as_slice(),
            left.reason.as_deref(),
        )
            .cmp(&(
                right.capability.as_str(),
                right.language,
                right.rules.as_slice(),
                right.reason.as_deref(),
            ))
    });
    output
}

fn setup_support(plan: &AnalysisPlan, reason: &str, hint: &str) -> Vec<CapabilitySupport> {
    GO_SYMBOL_GRAPH_CAPABILITIES
        .iter()
        .filter(|capability| plan.requests_capability(capability))
        .filter_map(|capability| {
            let base = plan
                .support_view()
                .entries()
                .iter()
                .find(|entry| entry.capability == *capability)?;
            Some(CapabilitySupport {
                capability: (*capability).to_string(),
                language: Some(Language::Go),
                status: CapabilitySupportStatus::SetupMissing,
                rules: base.rules.clone(),
                reason: Some(reason.to_string()),
                hint: Some(hint.to_string()),
                docs_path: Some(SYMBOL_FACTS_DOCS_PATH.to_string()),
            })
        })
        .collect()
}

fn supported_language_support(plan: &AnalysisPlan) -> Vec<CapabilitySupport> {
    GO_SYMBOL_GRAPH_CAPABILITIES
        .iter()
        .filter(|capability| plan.requests_capability(capability))
        .filter_map(|capability| {
            let base = plan
                .support_view()
                .entries()
                .iter()
                .find(|entry| entry.capability == *capability)?;
            Some(CapabilitySupport {
                capability: (*capability).to_string(),
                language: Some(Language::Go),
                status: CapabilitySupportStatus::Supported,
                rules: base.rules.clone(),
                reason: None,
                hint: None,
                docs_path: Some(SYMBOL_FACTS_DOCS_PATH.to_string()),
            })
        })
        .collect()
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
