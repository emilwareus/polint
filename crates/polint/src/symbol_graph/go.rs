use crate::analysis_plan::AnalysisPlan;
use crate::config::LoadedConfig;
use crate::core::{
    AnalysisDb, CapabilitySupport, CapabilitySupportStatus, DefinitionKind, FileId, Language,
    ReferenceKind, SourceFile, Span, SymbolKind, SymbolNamespace, SymbolPrecision,
    span_from_byte_range,
};
use crate::diagnostics::{Diagnostic, TextRange};
use crate::symbol_graph::model::SymbolGraphBuilder;
use crate::symbol_graph::model::{DefinitionDraft, ReferenceDraft, SymbolDraft};
use crate::symbol_graph::{LanguageSymbolOutput, SYMBOL_FACTS_DOCS_PATH};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const GO_SYMBOL_GRAPH_CAPABILITIES: &[&str] = &["symbols", "references"];
const GO_SYMBOL_SIDECAR_SCHEMA: &str = "polint-go-symbols-v1";
const GO_SYMBOL_SIDECAR_ENV: &str = "POLINT_GO_SYMBOLS";
const EMBEDDED_GO_SIDECAR_FILES: &[(&str, &str)] = &[
    (
        "go.mod",
        include_str!("../../go-sidecar/polint-go-symbols/go.mod"),
    ),
    (
        "go.sum",
        include_str!("../../go-sidecar/polint-go-symbols/go.sum"),
    ),
    (
        "main.go",
        include_str!("../../go-sidecar/polint-go-symbols/main.go"),
    ),
    (
        "internal/symbols/emit.go",
        include_str!("../../go-sidecar/polint-go-symbols/internal/symbols/emit.go"),
    ),
];

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

#[derive(Debug, Clone)]
enum GoSidecarCommand {
    Binary(PathBuf),
    SourceDir(PathBuf),
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarOutput {
    schema: String,
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
    #[serde(default)]
    files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarSymbol {
    key: String,
    package_path: String,
    #[serde(default)]
    file: String,
    #[serde(default)]
    owner_chain: Vec<String>,
    name: String,
    qualified_name: String,
    namespace: String,
    kind: String,
    span: GoSidecarSpan,
    #[serde(default)]
    exported: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarDefinition {
    symbol_key: String,
    #[serde(default)]
    file: String,
    name: String,
    kind: String,
    span: GoSidecarSpan,
    #[serde(default)]
    primary: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarReference {
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
    let sidecar = match parse_sidecar_output(&stdout).and_then(|output| validate_paths(output, db))
    {
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
        .extend(supported_language_support(plan, &files));
    convert_sidecar_output(builder, db, &sidecar);
    output
        .diagnostics
        .extend(package_error_diagnostics(&sidecar.errors));

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
    let sidecar = resolve_go_sidecar()?;
    let mut command = match sidecar {
        GoSidecarCommand::Binary(path) => {
            let mut command = Command::new(path);
            command.current_dir(root);
            command
        }
        GoSidecarCommand::SourceDir(path) => {
            let mut command = Command::new("go");
            command
                .arg("run")
                .arg(".")
                .current_dir(path)
                .env("GOWORK", "off")
                .env("GOTOOLCHAIN", "local");
            command
        }
    };

    let output = command
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

fn resolve_go_sidecar() -> Result<GoSidecarCommand, GoSidecarFailure> {
    if let Ok(path) = std::env::var(GO_SYMBOL_SIDECAR_ENV)
        && !path.trim().is_empty()
    {
        return sidecar_command_for_path(PathBuf::from(path));
    }

    if let Some(path) = installed_sidecar_binary()? {
        return Ok(GoSidecarCommand::Binary(path));
    }

    materialize_embedded_go_sidecar().map(GoSidecarCommand::SourceDir)
}

fn sidecar_command_for_path(path: PathBuf) -> Result<GoSidecarCommand, GoSidecarFailure> {
    if path.is_file() {
        return Ok(GoSidecarCommand::Binary(path));
    }
    if path.join("go.mod").is_file() {
        return Ok(GoSidecarCommand::SourceDir(path));
    }
    Err(GoSidecarFailure::CommandFailed(format!(
        "{GO_SYMBOL_SIDECAR_ENV} must point to a polint-go-symbols binary or source directory."
    )))
}

fn installed_sidecar_binary() -> Result<Option<PathBuf>, GoSidecarFailure> {
    let executable = std::env::current_exe().map_err(|error| {
        GoSidecarFailure::CommandFailed(format!("failed to resolve current executable: {error}"))
    })?;
    let Some(directory) = executable.parent() else {
        return Ok(None);
    };
    let candidate = directory.join(sidecar_binary_name());
    Ok(candidate.is_file().then_some(candidate))
}

fn sidecar_binary_name() -> &'static str {
    if cfg!(windows) {
        "polint-go-symbols.exe"
    } else {
        "polint-go-symbols"
    }
}

fn materialize_embedded_go_sidecar() -> Result<PathBuf, GoSidecarFailure> {
    let hash = embedded_go_sidecar_hash();
    let parent = std::env::temp_dir()
        .join("polint-go-symbols")
        .join(env!("CARGO_PKG_VERSION"));
    let directory = parent.join(&hash);
    let marker = directory.join(".complete");
    if marker.is_file() {
        return Ok(directory);
    }
    if directory.exists() && embedded_go_sidecar_files_match(&directory) {
        fs::write(&marker, "").map_err(|error| {
            GoSidecarFailure::CommandFailed(format!(
                "failed to mark embedded Go sidecar directory `{}` complete: {error}",
                directory.display()
            ))
        })?;
        return Ok(directory);
    }
    if directory.exists() {
        fs::remove_dir_all(&directory).map_err(|error| {
            GoSidecarFailure::CommandFailed(format!(
                "failed to replace incomplete embedded Go sidecar directory `{}`: {error}",
                directory.display()
            ))
        })?;
    }

    fs::create_dir_all(&parent).map_err(|error| {
        GoSidecarFailure::CommandFailed(format!(
            "failed to create embedded Go sidecar cache `{}`: {error}",
            parent.display()
        ))
    })?;
    let staging = parent.join(format!(
        ".{hash}-{}-{}",
        std::process::id(),
        unique_materialization_suffix()
    ));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| {
            GoSidecarFailure::CommandFailed(format!(
                "failed to clear stale embedded Go sidecar staging directory `{}`: {error}",
                staging.display()
            ))
        })?;
    }

    write_embedded_go_sidecar_files(&staging)?;
    fs::write(staging.join(".complete"), "").map_err(|error| {
        GoSidecarFailure::CommandFailed(format!(
            "failed to write embedded Go sidecar completion marker `{}`: {error}",
            staging.display()
        ))
    })?;
    match fs::rename(&staging, &directory) {
        Ok(()) => Ok(directory),
        Err(_) if marker.is_file() => {
            let _ = fs::remove_dir_all(&staging);
            Ok(directory)
        }
        Err(error) => Err(GoSidecarFailure::CommandFailed(format!(
            "failed to publish embedded Go sidecar directory `{}`: {error}",
            directory.display()
        ))),
    }
}

fn embedded_go_sidecar_files_match(directory: &Path) -> bool {
    EMBEDDED_GO_SIDECAR_FILES
        .iter()
        .all(|(relative_path, contents)| {
            fs::read_to_string(directory.join(relative_path))
                .as_deref()
                .is_ok_and(|existing| existing == *contents)
        })
}

fn write_embedded_go_sidecar_files(directory: &Path) -> Result<(), GoSidecarFailure> {
    for (relative_path, contents) in EMBEDDED_GO_SIDECAR_FILES {
        let path = directory.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                GoSidecarFailure::CommandFailed(format!(
                    "failed to create embedded Go sidecar directory `{}`: {error}",
                    parent.display()
                ))
            })?;
        }
        fs::write(&path, contents).map_err(|error| {
            GoSidecarFailure::CommandFailed(format!(
                "failed to write embedded Go sidecar file `{}`: {error}",
                path.display()
            ))
        })?;
    }
    Ok(())
}

fn unique_materialization_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn embedded_go_sidecar_hash() -> String {
    let mut parts = Vec::new();
    parts.push(GO_SYMBOL_SIDECAR_SCHEMA.to_string());
    for (relative_path, contents) in EMBEDDED_GO_SIDECAR_FILES {
        parts.push(format!(
            "{relative_path}:{}",
            crate::cache::stable_hash(&[*contents])
        ));
    }
    let part_refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    crate::cache::stable_hash(&part_refs)
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
    if !plan
        .rules_for_capability_matching_files("references", files)
        .is_empty()
    {
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
        .extend(setup_support(plan, files, reason, hint));
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

fn setup_support(
    plan: &AnalysisPlan,
    files: &[&SourceFile],
    reason: &str,
    hint: &str,
) -> Vec<CapabilitySupport> {
    GO_SYMBOL_GRAPH_CAPABILITIES
        .iter()
        .filter(|capability| plan.requests_capability(capability))
        .filter_map(|capability| {
            let rules = plan.rules_for_capability_matching_files(capability, files);
            if rules.is_empty() {
                return None;
            }
            Some(CapabilitySupport {
                capability: (*capability).to_string(),
                language: Some(Language::Go),
                status: CapabilitySupportStatus::SetupMissing,
                rules,
                reason: Some(reason.to_string()),
                hint: Some(hint.to_string()),
                docs_path: Some(SYMBOL_FACTS_DOCS_PATH.to_string()),
            })
        })
        .collect()
}

fn supported_language_support(
    plan: &AnalysisPlan,
    files: &[&SourceFile],
) -> Vec<CapabilitySupport> {
    GO_SYMBOL_GRAPH_CAPABILITIES
        .iter()
        .filter(|capability| plan.requests_capability(capability))
        .filter_map(|capability| {
            let rules = plan.rules_for_capability_matching_files(capability, files);
            if rules.is_empty() {
                return None;
            }
            Some(CapabilitySupport {
                capability: (*capability).to_string(),
                language: Some(Language::Go),
                status: CapabilitySupportStatus::Supported,
                rules,
                reason: None,
                hint: None,
                docs_path: Some(SYMBOL_FACTS_DOCS_PATH.to_string()),
            })
        })
        .collect()
}

fn convert_sidecar_output(
    builder: &mut SymbolGraphBuilder,
    db: &AnalysisDb,
    sidecar: &GoSidecarOutput,
) {
    let files = go_files_by_path(db);
    let symbol_rows = sidecar
        .symbols
        .iter()
        .map(|symbol| (symbol.key.as_str(), symbol))
        .collect::<BTreeMap<_, _>>();
    let mut symbols = BTreeMap::new();

    for symbol in &sidecar.symbols {
        let id = builder.add_symbol(symbol_draft(symbol, &files));
        symbols.insert(symbol.key.clone(), id);
    }

    for definition in &sidecar.definitions {
        let Some(symbol) = symbols.get(&definition.symbol_key).copied() else {
            continue;
        };
        let source_symbol = symbol_rows.get(definition.symbol_key.as_str()).copied();
        builder.add_definition(symbol, definition_draft(definition, source_symbol, &files));
    }

    for reference in &sidecar.references {
        let draft = reference_draft(reference, &files);
        if let Some(target) = symbols.get(&reference.target_key).copied() {
            builder.add_reference(target, draft);
        } else {
            builder.add_unresolved_reference(draft);
        }
    }
}

fn go_files_by_path(db: &AnalysisDb) -> BTreeMap<&str, &SourceFile> {
    db.files()
        .iter()
        .filter(|file| file.language == Language::Go)
        .map(|file| (file.relative_path.as_str(), file))
        .collect()
}

fn symbol_draft(symbol: &GoSidecarSymbol, files: &BTreeMap<&str, &SourceFile>) -> SymbolDraft {
    let file = file_for_path(files, &symbol.file).map(|file| file.id);
    let primary_span = span_for_file(files, &symbol.file, &symbol.span);
    SymbolDraft {
        language: Language::Go,
        name: symbol.name.clone(),
        qualified_name: symbol.qualified_name.clone(),
        kind: symbol_kind(&symbol.kind),
        namespace: symbol_namespace(&symbol.namespace),
        file,
        package: None,
        module: None,
        owner: None,
        module_key: if symbol.package_path.is_empty() {
            None
        } else {
            Some(format!("go:module:{}", symbol.package_path))
        },
        package_key: Some(format!("go:sidecar:{}", symbol.key)),
        file_key: None,
        owner_chain: symbol.owner_chain.clone(),
        primary_span: if symbol.key.starts_with("go:local") {
            primary_span
        } else {
            None
        },
        is_exported: symbol.exported,
        precision: SymbolPrecision::ExactSemantic,
    }
}

fn definition_draft(
    definition: &GoSidecarDefinition,
    symbol: Option<&GoSidecarSymbol>,
    files: &BTreeMap<&str, &SourceFile>,
) -> DefinitionDraft {
    let file = file_for_path(files, &definition.file).map(|file| file.id);
    DefinitionDraft {
        language: Language::Go,
        name: definition.name.clone(),
        qualified_name: symbol
            .map(|symbol| symbol.qualified_name.clone())
            .unwrap_or_else(|| definition.name.clone()),
        kind: definition_kind(&definition.kind),
        namespace: symbol
            .map(|symbol| symbol_namespace(&symbol.namespace))
            .unwrap_or(SymbolNamespace::Unknown),
        file,
        package: None,
        module: None,
        owner: None,
        file_key: definition.file.clone(),
        primary_span: span_for_file(files, &definition.file, &definition.span),
        is_primary: definition.primary,
        is_exported: symbol.is_some_and(|symbol| symbol.exported),
        precision: SymbolPrecision::ExactSemantic,
    }
}

fn reference_draft(
    reference: &GoSidecarReference,
    files: &BTreeMap<&str, &SourceFile>,
) -> ReferenceDraft {
    let file = file_for_path(files, &reference.file).map(|file| file.id);
    ReferenceDraft {
        language: Language::Go,
        name: reference.name.clone(),
        qualified_name: reference.name.clone(),
        kind: reference_kind(&reference.kind),
        namespace: reference_namespace(&reference.kind),
        file,
        package: None,
        module: None,
        owner: None,
        file_key: reference.file.clone(),
        primary_span: span_for_file(files, &reference.file, &reference.span),
        precision: reference_precision(&reference.precision),
    }
}

fn file_for_path<'a>(files: &'a BTreeMap<&str, &SourceFile>, path: &str) -> Option<&'a SourceFile> {
    if path.is_empty() {
        None
    } else {
        files.get(path).copied()
    }
}

fn span_for_file(
    files: &BTreeMap<&str, &SourceFile>,
    path: &str,
    span: &GoSidecarSpan,
) -> Option<Span> {
    let file = file_for_path(files, path)?;
    Some(span_from_byte_range(
        file.id,
        file.source.as_ref(),
        span.start_byte,
        span.end_byte,
    ))
}

fn symbol_kind(kind: &str) -> SymbolKind {
    match kind {
        "package" => SymbolKind::Package,
        "function" => SymbolKind::Function,
        "method" => SymbolKind::Method,
        "variable" => SymbolKind::Variable,
        "constant" => SymbolKind::Constant,
        "type" => SymbolKind::TypeAlias,
        "field" => SymbolKind::Field,
        "parameter" => SymbolKind::Parameter,
        _ => SymbolKind::Unknown,
    }
}

fn symbol_namespace(namespace: &str) -> SymbolNamespace {
    match namespace {
        "value" => SymbolNamespace::Value,
        "type" => SymbolNamespace::Type,
        "package" => SymbolNamespace::Package,
        "module" => SymbolNamespace::Module,
        _ => SymbolNamespace::Unknown,
    }
}

fn definition_kind(kind: &str) -> DefinitionKind {
    match kind {
        "declaration" => DefinitionKind::Declaration,
        "definition" => DefinitionKind::Definition,
        "import" => DefinitionKind::Import,
        "export" => DefinitionKind::Export,
        "implicit" => DefinitionKind::Implicit,
        _ => DefinitionKind::Unknown,
    }
}

fn reference_kind(kind: &str) -> ReferenceKind {
    match kind {
        "read" => ReferenceKind::Read,
        "write" => ReferenceKind::Write,
        "read_write" => ReferenceKind::ReadWrite,
        "call" => ReferenceKind::Call,
        "type" => ReferenceKind::TypeUse,
        "package" => ReferenceKind::Import,
        "field" | "method" | "member" => ReferenceKind::MemberAccess,
        _ => ReferenceKind::Unknown,
    }
}

fn reference_namespace(kind: &str) -> SymbolNamespace {
    match kind {
        "type" => SymbolNamespace::Type,
        "package" => SymbolNamespace::Package,
        _ => SymbolNamespace::Value,
    }
}

fn reference_precision(precision: &str) -> SymbolPrecision {
    match precision {
        "exact_semantic" => SymbolPrecision::ExactSemantic,
        "exact_local" => SymbolPrecision::ExactLocal,
        "module_linked" => SymbolPrecision::ModuleLinked,
        "heuristic" => SymbolPrecision::Heuristic,
        "setup_missing" => SymbolPrecision::SetupMissing,
        "unsupported" => SymbolPrecision::Unsupported,
        _ => SymbolPrecision::Unsupported,
    }
}

fn package_error_diagnostics(errors: &[GoSidecarPackageError]) -> Vec<Diagnostic> {
    let mut diagnostics = errors
        .iter()
        .map(|error| {
            Diagnostic::error(
                "polint/capability",
                "<workspace>",
                TextRange::point(1, 1),
                format!(
                    "Go package `{}` loaded with errors while deriving symbols and references.",
                    error.package_path
                ),
            )
            .with_evidence("capability", "symbols".to_string())
            .with_evidence("language", "Go".to_string())
            .with_evidence("package_id", error.package_id.clone())
            .with_evidence("package_path", error.package_path.clone())
            .with_evidence("reason", error.message.clone())
            .with_evidence("docs_path", SYMBOL_FACTS_DOCS_PATH.to_string())
            .with_help(format!(
                "Fix Go package setup for `{}` to make symbol/reference coverage complete.",
                error.package_path
            ))
        })
        .collect::<Vec<_>>();
    diagnostics.sort_by(|left, right| {
        (
            left.file.as_str(),
            left.message.as_str(),
            left.stable_fingerprint.as_str(),
        )
            .cmp(&(
                right.file.as_str(),
                right.message.as_str(),
                right.stable_fingerprint.as_str(),
            ))
    });
    diagnostics
}

#[cfg(test)]
mod symbol_graph_go_setup {
    use super::*;
    use crate::core::{
        Capabilities, CapabilitySupportStatus, Rule, RuleMeta, RuleOptions, SymbolPrecision,
        SymbolResolutionStatus,
    };
    use crate::diagnostics::Severity;
    use std::collections::BTreeMap;
    use std::path::Path;

    fn add_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().expect("fixture has parent")).expect("mkdirs");
        std::fs::write(&path, source).expect("write fixture");
        db.add_file(path, relative_path.to_string(), source.to_string());
    }

    fn add_go_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) {
        add_file(db, root, relative_path, source);
    }

    fn loaded_config_for(root: &Path) -> LoadedConfig {
        crate::config::load_config(root).expect("config loads")
    }

    fn requested_plan() -> AnalysisPlan {
        AnalysisPlan::from_capability_names_for_test(&["symbols", "references"])
    }

    fn requested_plan_with_files(files: &[&str]) -> AnalysisPlan {
        let rule = Rule::from_parts(
            || RuleMeta {
                id: "local/needs-symbols".to_string(),
                description: "Needs symbols".to_string(),
                severity: Severity::Warn,
            },
            || Capabilities::new().references(),
            |_db, _ctx| Ok(()),
        );
        let mut options = BTreeMap::new();
        options.insert(
            "local/needs-symbols".to_string(),
            RuleOptions {
                files: files.iter().map(|file| (*file).to_string()).collect(),
                ..RuleOptions::default()
            },
        );
        AnalysisPlan::from_rules(&[rule], None, &options)
    }

    #[test]
    fn embedded_go_sidecar_sources_match_workspace_sources() {
        let workspace_sidecar =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/polint-go-symbols");
        for (relative_path, embedded) in EMBEDDED_GO_SIDECAR_FILES {
            let workspace = std::fs::read_to_string(workspace_sidecar.join(relative_path))
                .unwrap_or_else(|error| panic!("read workspace sidecar {relative_path}: {error}"));
            assert_eq!(
                workspace, *embedded,
                "embedded sidecar drifted at {relative_path}"
            );
        }
    }

    #[test]
    fn embedded_go_sidecar_keeps_go_1_24_minimum() {
        let go_mod = EMBEDDED_GO_SIDECAR_FILES
            .iter()
            .find_map(|(relative_path, contents)| (*relative_path == "go.mod").then_some(*contents))
            .expect("embedded go.mod exists");

        assert!(
            go_mod.lines().any(|line| line == "go 1.24.0"),
            "embedded sidecar should keep Go 1.24 as its minimum supported toolchain: {go_mod:?}"
        );
        assert!(
            go_mod
                .lines()
                .any(|line| line == "require golang.org/x/tools v0.42.0"),
            "embedded sidecar should stay on the Go 1.24-compatible x/tools line: {go_mod:?}"
        );
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

        let output = derive_go_symbols(
            &mut builder,
            &db,
            &loaded_config_for(temp.path()),
            &requested_plan(),
        );
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
    fn missing_go_mod_does_not_block_ts_only_symbol_rules() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package main\n");
        add_file(
            &mut db,
            temp.path(),
            "src/app.ts",
            "export function run() { return 1; }\n",
        );
        let mut builder = SymbolGraphBuilder::new();

        let output = derive_go_symbols(
            &mut builder,
            &db,
            &loaded_config_for(temp.path()),
            &requested_plan_with_files(&["src/**/*.ts"]),
        );
        let graph = builder.finish();

        assert!(
            output.capability_support.is_empty(),
            "{:#?}",
            output.capability_support
        );
        assert!(
            graph.references.is_empty(),
            "TS-only rules should not receive Go setup-missing reference placeholders: {:#?}",
            graph.references
        );
    }

    #[test]
    fn sidecar_command_failure_reports_setup_missing() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/app\n\ngo 1.24.0\n",
        )
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
                Err(GoSidecarFailure::CommandFailed(
                    "sidecar failed".to_string(),
                ))
            },
        );

        assert!(
            output.capability_support.iter().all(|entry| {
                entry.status == CapabilitySupportStatus::SetupMissing
                    && entry
                        .reason
                        .as_deref()
                        .is_some_and(|reason| reason.contains("sidecar failed"))
            }),
            "{:#?}",
            output.capability_support
        );
    }

    #[test]
    fn invalid_sidecar_json_reports_setup_missing() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/app\n\ngo 1.24.0\n",
        )
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
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/app\n\ngo 1.24.0\n",
        )
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
                    && entry
                        .reason
                        .as_deref()
                        .is_some_and(|reason| reason.contains("escapes repository"))
            }),
            "{:#?}",
            output.capability_support
        );
    }
}

#[cfg(test)]
mod symbol_graph_go {
    use super::*;
    use crate::core::{
        CapabilitySupportStatus, DefinitionFact, DefinitionKind, ReferenceFact, ReferenceKind,
        SymbolFact, SymbolId, SymbolKind, SymbolPrecision, SymbolResolutionStatus,
    };
    use crate::symbol_graph::model::SymbolGraphOutput;
    use std::path::Path;

    fn derive_go_fixture(
        files: &[(&str, &str)],
    ) -> Option<(SymbolGraphOutput, LanguageSymbolOutput)> {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/app\n\ngo 1.24.0\n",
        )
        .expect("write go.mod");
        let mut db = AnalysisDb::new();
        for (relative_path, source) in files {
            add_go_file(&mut db, temp.path(), relative_path, source);
        }
        let mut builder = SymbolGraphBuilder::new();
        let output = derive_go_symbols(
            &mut builder,
            &db,
            &loaded_config_for(temp.path()),
            &AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]),
        );
        if output
            .capability_support
            .iter()
            .any(|entry| entry.status == CapabilitySupportStatus::SetupMissing)
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
                    .all(|entry| entry.status == CapabilitySupportStatus::Supported),
            "expected supported Go symbol capabilities; support = {:#?}; diagnostics = {:#?}",
            output.capability_support,
            output.diagnostics
        );
        Some((builder.finish(), output))
    }

    fn add_go_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().expect("fixture has parent")).expect("mkdirs");
        std::fs::write(&path, source).expect("write Go fixture");
        db.add_file(path, relative_path.to_string(), source.to_string());
    }

    fn loaded_config_for(root: &Path) -> LoadedConfig {
        crate::config::load_config(root).expect("config loads")
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

    #[test]
    fn go_setup_missing_derivation_emits_capability_diagnostics() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package main\n");

        let derivation = crate::symbol_graph::derive_requested_symbols(
            &mut db,
            &loaded_config_for(temp.path()),
            &AnalysisPlan::from_capability_names_for_test(&["symbols", "references"]),
        );

        assert!(
            derivation.diagnostics.iter().any(|diagnostic| {
                diagnostic.rule_id == "polint/capability"
                    && diagnostic.evidence.iter().any(|evidence| {
                        evidence.label == "status" && evidence.value == "setup_missing"
                    })
            }),
            "{:#?}",
            derivation.diagnostics
        );
    }
}
