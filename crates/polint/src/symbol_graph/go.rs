use crate::analysis_plan::AnalysisPlan;
use crate::config::LoadedConfig;
use crate::core::{
    AnalysisDb, CapabilitySupport, CapabilitySupportStatus, DefinitionKind, FileId, Language,
    ReferenceKind, SourceFile, Span, SymbolId, SymbolKind, SymbolNamespace, SymbolPrecision,
    span_from_byte_range,
};
use crate::diagnostics::{Diagnostic, TextRange};
use crate::go::lifecycle::{self, GoAnalysisConfig};
use crate::symbol_graph::model::SymbolGraphBuilder;
use crate::symbol_graph::model::{DefinitionDraft, ReferenceDraft, SymbolDraft};
use crate::symbol_graph::semantic::{
    AliasFact, AliasId, AliasKind, ExportFact, ExportId, ExportKind, ResolutionFact, ResolutionId,
    ResolutionStepKind, ScopeFact, ScopeId, ScopeKind, SemanticImportFact, SemanticImportId,
    SemanticImportKind, SemanticIndexBuilder, SemanticIndexOutput, SemanticStatus, StableExportId,
    StableExportIdentity,
};
use crate::symbol_graph::stable_id::{StableReferenceKey, StableSymbolKey};
use crate::symbol_graph::{LanguageSymbolOutput, SYMBOL_FACTS_DOCS_PATH};
use serde::{Deserialize, Deserializer};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const GO_SYMBOL_GRAPH_CAPABILITIES: &[&str] = &["symbols", "references"];
const GO_SYMBOL_SIDECAR_SCHEMA: &str = "polint-go-symbols-semantic-1";
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
    #[serde(default, deserialize_with = "null_as_default_vec")]
    packages: Vec<GoSidecarPackage>,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    symbols: Vec<GoSidecarSymbol>,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    definitions: Vec<GoSidecarDefinition>,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    references: Vec<GoSidecarReference>,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    scopes: Vec<GoSidecarScope>,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    imports: Vec<GoSidecarImport>,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    exports: Vec<GoSidecarExport>,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    resolution_steps: Vec<GoSidecarResolutionStep>,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    errors: Vec<GoSidecarPackageError>,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarPackage {
    #[serde(default, deserialize_with = "null_as_default_vec")]
    files: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarSymbol {
    key: String,
    package_path: String,
    #[serde(default)]
    file: String,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    owner_chain: Vec<String>,
    name: String,
    qualified_name: String,
    namespace: String,
    kind: String,
    span: GoSidecarSpan,
    #[serde(default)]
    exported: bool,
}

fn null_as_default_vec<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Option::<Vec<T>>::deserialize(deserializer)?.unwrap_or_default())
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
struct GoSidecarScope {
    key: String,
    #[serde(default)]
    parent_key: String,
    kind: String,
    package_path: String,
    #[serde(default)]
    file: String,
    span: GoSidecarSpan,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarImport {
    path: String,
    #[serde(default)]
    local_name: String,
    alias_kind: String,
    #[serde(default)]
    file: String,
    span: GoSidecarSpan,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarExport {
    symbol_key: String,
    export_name: String,
    namespace: String,
    object_path: String,
    package_path: String,
    #[serde(default)]
    generated: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct GoSidecarResolutionStep {
    reference_key: String,
    step: String,
    status: String,
    #[serde(default)]
    target_key: String,
    #[serde(default, deserialize_with = "null_as_default_vec")]
    candidate_keys: Vec<String>,
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
    runner: impl FnOnce(&GoAnalysisConfig) -> Result<Vec<u8>, GoSidecarFailure>,
) -> LanguageSymbolOutput {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let files = lifecycle::go_files(db);
    if files.is_empty() || !plan.requests_any_capability(GO_SYMBOL_GRAPH_CAPABILITIES) {
        return LanguageSymbolOutput::default();
    }

    let config = match GoAnalysisConfig::from_loaded_files(loaded, &files) {
        Ok(config) => config,
        Err(error) => {
            return setup_missing_output(
                interner,
                builder,
                &files,
                plan,
                error.reason(),
                "Configure languages.go.module_roots with repository-relative Go module roots.",
            );
        }
    };
    let uncovered_files = files_matching_paths(&files, &config.files_without_module_root);
    if !uncovered_files.is_empty() {
        let missing = setup_missing_output(
            interner,
            builder,
            &uncovered_files,
            plan,
            "some Go files are not under a go.mod module root.",
            "Move those files under a Go module or set languages.go.module_roots in .polint.toml.",
        );
        if !missing.capability_support.is_empty() {
            return missing;
        }
    }
    let missing_roots = config.missing_module_roots(&loaded.root);
    if !missing_roots.is_empty() {
        return setup_missing_output(
            interner,
            builder,
            &files,
            plan,
            &format!(
                "configured Go module roots are missing go.mod: {}.",
                missing_roots.join(", ")
            ),
            "Check languages.go.module_roots in .polint.toml.",
        );
    }

    let stdout = match runner(&config) {
        Ok(stdout) => stdout,
        Err(error) => {
            return setup_missing_output(
                interner,
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
                interner,
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
    output
        .semantic
        .extend(convert_sidecar_output(builder, db, &sidecar));
    output
        .diagnostics
        .extend(package_error_diagnostics(&sidecar.errors));

    output
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

fn files_matching_paths<'a>(
    files: &'a [&'a SourceFile],
    relative_paths: &[String],
) -> Vec<&'a SourceFile> {
    let relative_paths = relative_paths.iter().collect::<BTreeSet<_>>();
    files
        .iter()
        .copied()
        .filter(|file| relative_paths.contains(&file.relative_path))
        .collect()
}

fn run_go_sidecar(root: &Path, config: &GoAnalysisConfig) -> Result<Vec<u8>, GoSidecarFailure> {
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

    lifecycle::apply_go_offline_env(&mut command, config.offline);
    let output = command
        .arg("symbols")
        .arg("--root")
        .arg(root.as_os_str())
        .arg("--module-roots")
        .arg(config.module_roots.join(","))
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
    for scope in &mut output.scopes {
        if !scope.file.is_empty() {
            scope.file = validate_sidecar_path(&scope.file, &file_ids)?;
        }
    }
    for import in &mut output.imports {
        if !import.file.is_empty() {
            import.file = validate_sidecar_path(&import.file, &file_ids)?;
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
    interner: &crate::core::StableKeyInterner,
    builder: &mut SymbolGraphBuilder,
    files: &[&SourceFile],
    plan: &AnalysisPlan,
    reason: &str,
    hint: &str,
) -> LanguageSymbolOutput {
    let mut output = LanguageSymbolOutput::default();
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
        output
            .semantic
            .extend(setup_missing_semantic_index_for_files(interner, files));
    }

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
) -> SemanticIndexOutput {
    let interner_handle = db.stable_key_interner();
    let interner = &interner_handle;
    let files = go_files_by_path(db);
    let symbol_rows = sidecar
        .symbols
        .iter()
        .map(|symbol| (symbol.key.as_str(), symbol))
        .collect::<BTreeMap<_, _>>();
    let mut symbols = BTreeMap::new();
    let mut symbol_stable_keys = BTreeMap::new();
    let mut symbol_key_inputs = BTreeMap::new();
    let mut reference_stable_keys = BTreeMap::new();
    let resolution_steps = sidecar
        .resolution_steps
        .iter()
        .map(|step| (step.reference_key.as_str(), step))
        .collect::<BTreeMap<_, _>>();

    for symbol in &sidecar.symbols {
        let stable_key_input = go_stable_symbol_key(symbol, &files);
        let stable_key = stable_key_input.stable_key();
        let id = builder.add_symbol(symbol_draft(symbol, &files));
        symbols.insert(symbol.key.clone(), id);
        symbol_stable_keys.insert(symbol.key.clone(), stable_key);
        symbol_key_inputs.insert(symbol.key.clone(), stable_key_input);
    }

    for definition in &sidecar.definitions {
        let Some(symbol) = symbols.get(&definition.symbol_key).copied() else {
            continue;
        };
        let source_symbol = symbol_rows.get(definition.symbol_key.as_str()).copied();
        builder.add_definition(symbol, definition_draft(definition, source_symbol, &files));
    }

    for reference in &sidecar.references {
        let mut draft = reference_draft(reference, &files);
        reference_stable_keys.insert(
            go_reference_key(reference),
            go_stable_reference_key(reference, &files, &symbol_key_inputs),
        );
        if let Some(target) = symbols.get(&reference.target_key).copied() {
            builder.add_reference(target, draft);
        } else {
            let candidates = resolution_steps
                .get(go_reference_key(reference).as_str())
                .map(|step| semantic_candidate_symbol_ids(step, &symbols))
                .unwrap_or_default();
            if candidates.len() > 1 {
                draft.precision = SymbolPrecision::Ambiguous;
                builder.add_ambiguous_reference(candidates, draft);
            } else if let Some(candidate) = candidates.first().copied() {
                builder.add_reference(candidate, draft);
            } else {
                draft.precision = SymbolPrecision::Unresolved;
                builder.add_unresolved_reference(draft);
            }
        }
    }

    derive_go_semantic_index(
        interner,
        sidecar,
        &files,
        &symbol_stable_keys,
        &reference_stable_keys,
    )
}

fn setup_missing_semantic_index_for_files(
    interner: &crate::core::StableKeyInterner,
    files: &[&SourceFile],
) -> SemanticIndexOutput {
    let mut builder = SemanticIndexBuilder::new();
    for file in files {
        let source_key = format!("go:setup-missing|file:{}", file.relative_path);
        builder.add_resolution(
            interner,
            ResolutionFact {
                id: ResolutionId(0),
                language: Language::Go,
                file: Some(file.id),
                package: None,
                module: None,
                source_stable_key: source_key.clone(),
                target_stable_keys: Vec::new(),
                step: ResolutionStepKind::UnknownFallback,
                stable_key: format!("{source_key}|step:UnknownFallback|status:setup_missing"),
                status: SemanticStatus::SetupMissing,
            },
        );
    }
    builder.finish()
}

fn derive_go_semantic_index(
    interner: &crate::core::StableKeyInterner,
    sidecar: &GoSidecarOutput,
    files: &BTreeMap<&str, &SourceFile>,
    symbol_stable_keys: &BTreeMap<String, String>,
    reference_stable_keys: &BTreeMap<String, String>,
) -> SemanticIndexOutput {
    let mut builder = SemanticIndexBuilder::new();
    let mut scope_ids = BTreeMap::new();

    for scope in &sidecar.scopes {
        let id = builder.add_scope(
            interner,
            ScopeFact {
                id: ScopeId(0),
                language: Language::Go,
                file: file_for_path(files, &scope.file).map(|file| file.id),
                package: None,
                module: None,
                parent: scope_ids.get(scope.parent_key.as_str()).copied(),
                scope_path: go_scope_path(scope),
                kind: go_scope_kind(&scope.kind),
                stable_key: scope.key.clone(),
                status: SemanticStatus::Resolved,
            },
        );
        scope_ids.insert(scope.key.as_str(), id);
    }

    let resolution_steps = sidecar
        .resolution_steps
        .iter()
        .map(|step| (step.reference_key.as_str(), step))
        .collect::<BTreeMap<_, _>>();

    for import in &sidecar.imports {
        let source_key = go_import_stable_key(import);
        let resolution = resolution_steps.get(source_key.as_str()).copied();
        let status = go_import_status(import, resolution);
        builder.add_semantic_import(
            interner,
            SemanticImportFact {
                id: SemanticImportId(0),
                language: Language::Go,
                file: file_for_path(files, &import.file).map(|file| file.id),
                package: None,
                module: None,
                scope: None,
                import_path: import.path.clone(),
                local_name: go_import_local_name(import),
                imported_name: None,
                namespace: SymbolNamespace::Package,
                kind: go_import_kind(&import.alias_kind),
                stable_key: source_key.clone(),
                status,
            },
        );
        if let Some(alias) = go_import_alias_fact(import, resolution, status, symbol_stable_keys) {
            builder.add_alias(interner, alias);
        }
    }

    for export in &sidecar.exports {
        let export_id = builder.add_export_identity(
            interner,
            ExportFact {
                id: ExportId(0),
                language: Language::Go,
                file: None,
                package: None,
                module: None,
                scope: None,
                symbol: None,
                export_name: export.export_name.clone(),
                namespace: symbol_namespace(&export.namespace),
                kind: ExportKind::Named,
                stable_key: go_export_stable_key(export),
                status: if export.generated {
                    SemanticStatus::Generated
                } else {
                    SemanticStatus::Resolved
                },
            },
        );
        builder.add_stable_export(
            interner,
            StableExportIdentity {
                id: StableExportId(0),
                export: export_id,
                language: Language::Go,
                package_key: Some(format!("go:package:{}", export.package_path)),
                module_key: None,
                export_name: export.export_name.clone(),
                namespace: symbol_namespace(&export.namespace),
                symbol_stable_key: mapped_symbol_key(symbol_stable_keys, &export.symbol_key),
                generated_discriminator: Some("native".to_string()),
                stable_key: go_stable_export_key(export),
                status: SemanticStatus::Resolved,
            },
        );
    }

    for step in &sidecar.resolution_steps {
        let target_stable_keys = mapped_candidate_keys(step, symbol_stable_keys);
        builder.add_resolution(
            interner,
            ResolutionFact {
                id: ResolutionId(0),
                language: Language::Go,
                file: None,
                package: None,
                module: None,
                source_stable_key: mapped_reference_key(reference_stable_keys, &step.reference_key),
                target_stable_keys,
                step: go_resolution_step_kind(&step.step),
                stable_key: go_resolution_stable_key(step),
                status: semantic_status(&step.status),
            },
        );
    }
    for reference in &sidecar.references {
        let reference_key = go_reference_key(reference);
        if resolution_steps.contains_key(reference_key.as_str()) {
            continue;
        }
        if reference.target_key.is_empty() {
            builder.add_resolution(
                interner,
                ResolutionFact {
                    id: ResolutionId(0),
                    language: Language::Go,
                    file: file_for_path(files, &reference.file).map(|file| file.id),
                    package: None,
                    module: None,
                    source_stable_key: mapped_reference_key(reference_stable_keys, &reference_key),
                    target_stable_keys: Vec::new(),
                    step: ResolutionStepKind::UnknownFallback,
                    stable_key: go_reference_unknown_fallback_key(reference),
                    status: SemanticStatus::Unresolved,
                },
            );
        }
    }

    builder.finish()
}

fn go_files_by_path(db: &AnalysisDb) -> BTreeMap<&str, &SourceFile> {
    db.files()
        .iter()
        .filter(|file| file.language == Language::Go)
        .map(|file| (file.relative_path.as_str(), file))
        .collect()
}

fn go_scope_path(scope: &GoSidecarScope) -> Vec<String> {
    [
        format!("package:{}", scope.package_path),
        format!("file:{}", scope.file),
        format!("kind:{}", scope.kind),
        format!("span:{}-{}", scope.span.start_byte, scope.span.end_byte),
        scope.key.clone(),
    ]
    .into_iter()
    .filter(|part| !part.ends_with(':'))
    .collect()
}

fn go_scope_kind(kind: &str) -> ScopeKind {
    match kind {
        "package" => ScopeKind::Package,
        "file" => ScopeKind::File,
        "function" => ScopeKind::Function,
        "method" => ScopeKind::Method,
        "block" => ScopeKind::Block,
        "type" => ScopeKind::Type,
        _ => ScopeKind::Unknown,
    }
}

fn go_import_kind(alias_kind: &str) -> SemanticImportKind {
    match alias_kind {
        "named" => SemanticImportKind::GoNamed,
        "dot" => SemanticImportKind::GoDot,
        "blank" => SemanticImportKind::GoBlank,
        "implicit" => SemanticImportKind::GoImplicit,
        _ => SemanticImportKind::Unknown,
    }
}

fn go_import_status(
    import: &GoSidecarImport,
    resolution: Option<&GoSidecarResolutionStep>,
) -> SemanticStatus {
    if import.alias_kind == "dot" {
        let candidates = resolution
            .map(normalized_candidate_keys)
            .unwrap_or_default();
        if candidates.len() == 1 {
            SemanticStatus::Resolved
        } else {
            SemanticStatus::Ambiguous
        }
    } else {
        resolution
            .map(|step| semantic_status(&step.status))
            .unwrap_or(SemanticStatus::Resolved)
    }
}

fn go_import_local_name(import: &GoSidecarImport) -> Option<String> {
    if import.local_name.is_empty() {
        None
    } else {
        Some(import.local_name.clone())
    }
}

fn go_import_alias_fact(
    import: &GoSidecarImport,
    resolution: Option<&GoSidecarResolutionStep>,
    status: SemanticStatus,
    symbol_stable_keys: &BTreeMap<String, String>,
) -> Option<AliasFact> {
    if import.alias_kind == "blank" || import.alias_kind == "implicit" {
        return None;
    }
    let source_key = go_import_stable_key(import);
    Some(AliasFact {
        id: AliasId(0),
        language: Language::Go,
        file: None,
        package: None,
        module: None,
        source_symbol_stable_key: source_key.clone(),
        target_symbol_stable_keys: resolution
            .map(|step| mapped_candidate_keys(step, symbol_stable_keys))
            .unwrap_or_default(),
        kind: AliasKind::ImportAlias,
        stable_key: format!("{source_key}|alias"),
        status,
    })
}

fn go_import_stable_key(import: &GoSidecarImport) -> String {
    format!(
        "go:import|file:{}|path:{}|local:{}|span:{}-{}",
        import.file, import.path, import.local_name, import.span.start_byte, import.span.end_byte
    )
}

fn go_export_stable_key(export: &GoSidecarExport) -> String {
    format!(
        "go:export|package:{}|namespace:{}|name:{}|object_path:{}",
        export.package_path, export.namespace, export.export_name, export.object_path
    )
}

fn go_stable_export_key(export: &GoSidecarExport) -> String {
    format!(
        "{}|symbol:{}|discriminator:native",
        go_export_stable_key(export),
        export.symbol_key
    )
}

fn go_resolution_stable_key(step: &GoSidecarResolutionStep) -> String {
    format!(
        "go:resolution|reference:{}|step:{}|status:{}",
        step.reference_key, step.step, step.status
    )
}

fn go_reference_key(reference: &GoSidecarReference) -> String {
    format!(
        "{}|{}|{}|{}|{}|{}|{}",
        reference.package_id,
        reference.file,
        reference.name,
        reference.target_key,
        reference.kind,
        reference.span.start_byte,
        reference.span.end_byte
    )
}

fn go_reference_unknown_fallback_key(reference: &GoSidecarReference) -> String {
    format!(
        "go:resolution|reference:{}|step:UnknownFallback|status:unresolved",
        go_reference_key(reference)
    )
}

fn semantic_candidate_symbol_ids(
    step: &GoSidecarResolutionStep,
    symbols: &BTreeMap<String, SymbolId>,
) -> Vec<SymbolId> {
    normalized_candidate_keys(step)
        .into_iter()
        .filter_map(|key| symbols.get(&key).copied())
        .collect()
}

fn go_resolution_step_kind(step: &str) -> ResolutionStepKind {
    match step {
        "LexicalLookup" | "lexical" => ResolutionStepKind::LexicalLookup,
        "ImportAliasLookup" | "import_alias" => ResolutionStepKind::ImportAliasLookup,
        "Package" | "package" => ResolutionStepKind::Package,
        "ModuleLookup" | "module" => ResolutionStepKind::ModuleLookup,
        "MemberLookup" | "member" => ResolutionStepKind::MemberLookup,
        "GeneratedHintLookup" | "generated" => ResolutionStepKind::GeneratedHintLookup,
        "UnknownFallback" | "unknown_fallback" => ResolutionStepKind::UnknownFallback,
        "external" => ResolutionStepKind::External,
        "unsupported" => ResolutionStepKind::Unsupported,
        _ => ResolutionStepKind::UnknownFallback,
    }
}

fn semantic_status(status: &str) -> SemanticStatus {
    match status {
        "resolved" => SemanticStatus::Resolved,
        "ambiguous" => SemanticStatus::Ambiguous,
        "unresolved" => SemanticStatus::Unresolved,
        "cycle" => SemanticStatus::Cycle,
        "generated" => SemanticStatus::Generated,
        "dynamic" => SemanticStatus::Dynamic,
        "external" => SemanticStatus::External,
        "setup_missing" => SemanticStatus::SetupMissing,
        "unsupported" => SemanticStatus::Unsupported,
        _ => SemanticStatus::Unresolved,
    }
}

fn normalized_candidate_keys(step: &GoSidecarResolutionStep) -> Vec<String> {
    let mut keys = step.candidate_keys.clone();
    if !step.target_key.is_empty() {
        keys.push(step.target_key.clone());
    }
    keys.sort();
    keys.dedup();
    keys
}

fn mapped_candidate_keys(
    step: &GoSidecarResolutionStep,
    symbol_stable_keys: &BTreeMap<String, String>,
) -> Vec<String> {
    if symbol_stable_keys.is_empty() {
        return normalized_candidate_keys(step);
    }
    let mut keys = normalized_candidate_keys(step)
        .into_iter()
        .filter_map(|key| mapped_candidate_key(symbol_stable_keys, key))
        .collect::<Vec<_>>();
    keys.sort();
    keys.dedup();
    keys
}

fn mapped_candidate_key(
    symbol_stable_keys: &BTreeMap<String, String>,
    key: String,
) -> Option<String> {
    if let Some(mapped) = symbol_stable_keys.get(&key) {
        Some(mapped.clone())
    } else if key.starts_with("go:builtin|") {
        None
    } else {
        Some(key)
    }
}

fn mapped_symbol_key(symbol_stable_keys: &BTreeMap<String, String>, key: &str) -> String {
    symbol_stable_keys
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

fn mapped_reference_key(reference_stable_keys: &BTreeMap<String, String>, key: &str) -> String {
    reference_stable_keys
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.to_string())
}

fn go_stable_symbol_key(
    symbol: &GoSidecarSymbol,
    files: &BTreeMap<&str, &SourceFile>,
) -> StableSymbolKey {
    StableSymbolKey::new(
        Language::Go,
        (!symbol.package_path.is_empty()).then(|| format!("go:module:{}", symbol.package_path)),
        Some(format!("go:sidecar:{}", symbol.key)),
        None,
        symbol.owner_chain.clone(),
        symbol_namespace(&symbol.namespace),
        symbol_kind(&symbol.kind),
        symbol.name.clone(),
        if symbol.key.starts_with("go:local") {
            span_for_file(files, &symbol.file, &symbol.span)
        } else {
            None
        },
    )
}

fn go_stable_reference_key(
    reference: &GoSidecarReference,
    files: &BTreeMap<&str, &SourceFile>,
    symbol_key_inputs: &BTreeMap<String, StableSymbolKey>,
) -> String {
    let span = span_for_file(files, &reference.file, &reference.span)
        .unwrap_or_else(|| Span::point(FileId(u32::MAX), 1, 1));
    symbol_key_inputs
        .get(&reference.target_key)
        .cloned()
        .map_or_else(
            || {
                StableReferenceKey::unresolved(
                    Language::Go,
                    reference.file.clone(),
                    reference.name.clone(),
                    span.clone(),
                )
            },
            |target| StableReferenceKey::resolved(target, reference.file.clone(), span.clone()),
        )
        .stable_key()
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
mod sidecar_semantic_output {
    use super::*;

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
    use super::*;
    use crate::core::{FileId, SourceFile};
    use crate::symbol_graph::semantic::{AliasKind, ScopeKind, SemanticImportKind, SemanticStatus};
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

    fn derive(json: &[u8]) -> crate::symbol_graph::semantic::SemanticIndexOutput {
        let file = source_file("main.go", "package app\n");
        let files = BTreeMap::from([(file.relative_path.as_str(), &file)]);
        let sidecar = parse_sidecar_output(json).expect("sidecar fixture parses");

        derive_go_semantic_index(
            &crate::core::AnalysisDb::new().stable_key_interner(),
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
                && alias.target_symbol_stable_keys == vec!["fmt.Println".to_string()]
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
                && export.symbol_stable_key == "go:package|package:example.com/app|name:Build"
                && export.generated_discriminator.as_deref() == Some("native")
        }));
    }
}

#[cfg(test)]
mod semantic_setup_missing {
    use super::*;
    use crate::core::{FileId, ReferenceKind, SourceFile, SymbolResolutionStatus};
    use crate::symbol_graph::semantic::{ResolutionStepKind, SemanticStatus};
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
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &files,
        );

        assert!(output.resolutions.iter().any(|resolution| {
            resolution.step == ResolutionStepKind::UnknownFallback
                && resolution.status == SemanticStatus::SetupMissing
                && resolution.source_stable_key.contains("main.go")
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
            &crate::core::AnalysisDb::new().stable_key_interner(),
            &sidecar,
            &files,
            &BTreeMap::new(),
            &BTreeMap::new(),
        );

        assert!(output.resolutions.iter().any(|resolution| {
            resolution.step == ResolutionStepKind::UnknownFallback
                && resolution.status == SemanticStatus::Unresolved
                && resolution.source_stable_key.contains("Missing")
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
        let mut builder = SymbolGraphBuilder::new(crate::core::StableKeyInterner::default());

        convert_sidecar_output(&mut builder, &analysis_db_with(file), &sidecar);
        let graph = builder.finish();

        assert!(graph.references.iter().any(|reference| {
            reference.kind == ReferenceKind::Call
                && reference.status == SymbolResolutionStatus::Ambiguous
                && reference.candidates.len() == 2
        }));
    }

    fn analysis_db_with(file: SourceFile) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        db.add_file(
            file.path.clone(),
            file.relative_path.clone(),
            file.source.to_string(),
        );
        db
    }
}

#[cfg(test)]
mod symbol_graph_go_setup {
    use super::*;
    use crate::core::{
        Capabilities, CapabilitySupportStatus, Rule, RuleKind, RuleMeta, RuleOptions,
        SymbolPrecision, SymbolResolutionStatus,
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
                kind: RuleKind::Check,
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
                .map(str::trim)
                .any(|line| line == "golang.org/x/tools v0.42.0"),
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
module_roots = ["cmd/service", "libs/platform"]
package_patterns = ["./cmd/...", "./pkg/..."]
build_tags = "enterprise,polint"
include_tests = false
"#,
        )
        .expect("write config");

        let files = Vec::new();
        let config =
            GoAnalysisConfig::from_loaded_files(&loaded_config_for(temp.path()), &files).unwrap();

        assert_eq!(
            config,
            GoAnalysisConfig {
                module_roots: vec!["cmd/service".to_string(), "libs/platform".to_string(),],
                package_patterns: vec!["./cmd/...".to_string(), "./pkg/...".to_string()],
                build_tags: vec!["enterprise".to_string(), "polint".to_string()],
                include_tests: false,
                offline: false,
                files_without_module_root: Vec::new(),
            }
        );
    }

    #[test]
    fn go_symbol_config_infers_nearest_module_roots_for_monorepos() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("services/payments")).expect("mkdirs");
        std::fs::create_dir_all(temp.path().join("libs/money")).expect("mkdirs");
        std::fs::write(
            temp.path().join("services/payments/go.mod"),
            "module example.com/payments\n\ngo 1.24\n",
        )
        .expect("write service go.mod");
        std::fs::write(
            temp.path().join("libs/money/go.mod"),
            "module example.com/money\n\ngo 1.24\n",
        )
        .expect("write lib go.mod");
        let mut db = AnalysisDb::new();
        add_go_file(
            &mut db,
            temp.path(),
            "services/payments/main.go",
            "package payments\n",
        );
        add_go_file(
            &mut db,
            temp.path(),
            "libs/money/money.go",
            "package money\n",
        );
        let files = lifecycle::go_files(&db);

        let config =
            GoAnalysisConfig::from_loaded_files(&loaded_config_for(temp.path()), &files).unwrap();

        assert_eq!(
            config.module_roots,
            vec!["libs/money".to_string(), "services/payments".to_string()]
        );
        assert!(config.files_without_module_root.is_empty());
    }

    #[test]
    fn go_symbol_config_treats_files_outside_configured_roots_as_uncovered() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join(".polint.toml"),
            r#"
[languages.go]
module_roots = ["services/payments"]
"#,
        )
        .expect("write config");
        let mut db = AnalysisDb::new();
        add_go_file(
            &mut db,
            temp.path(),
            "services/payments/main.go",
            "package payments\n",
        );
        add_go_file(
            &mut db,
            temp.path(),
            "services/ledger/main.go",
            "package ledger\n",
        );
        let files = lifecycle::go_files(&db);

        let config =
            GoAnalysisConfig::from_loaded_files(&loaded_config_for(temp.path()), &files).unwrap();

        assert_eq!(config.module_roots, vec!["services/payments".to_string()]);
        assert_eq!(
            config.files_without_module_root,
            vec!["services/ledger/main.go".to_string()]
        );
    }

    #[test]
    fn missing_go_mod_reports_setup_missing_for_requested_capabilities() {
        let temp = tempfile::tempdir().expect("tempdir");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "main.go", "package main\n");
        let mut builder = SymbolGraphBuilder::new(crate::core::StableKeyInterner::default());

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
        let mut builder = SymbolGraphBuilder::new(crate::core::StableKeyInterner::default());

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
        let mut builder = SymbolGraphBuilder::new(crate::core::StableKeyInterner::default());

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
        let mut builder = SymbolGraphBuilder::new(crate::core::StableKeyInterner::default());

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
    fn sidecar_null_sequence_fields_parse_as_empty_vectors() {
        let output = parse_sidecar_output(
            br#"{
  "schema":"polint-go-symbols-semantic-1",
  "go_version":"go1.24.13",
  "packages":[{"files":null}],
  "symbols":null,
  "definitions":null,
  "references":null,
  "scopes":null,
  "imports":null,
  "exports":null,
  "resolution_steps":null,
  "errors":null
}"#,
        )
        .expect("sidecar output parses");

        assert_eq!(output.packages.len(), 1);
        assert!(output.packages[0].files.is_empty());
        assert!(output.symbols.is_empty());
        assert!(output.definitions.is_empty());
        assert!(output.references.is_empty());
        assert!(output.scopes.is_empty());
        assert!(output.imports.is_empty());
        assert!(output.exports.is_empty());
        assert!(output.resolution_steps.is_empty());
        assert!(output.errors.is_empty());
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
        let mut builder = SymbolGraphBuilder::new(crate::core::StableKeyInterner::default());

        let output = derive_go_symbols_with_runner(
            &mut builder,
            &db,
            &loaded_config_for(temp.path()),
            &requested_plan(),
            |_config| {
                Ok(
                    br#"{
  "schema":"polint-go-symbols-semantic-1",
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
        let mut builder = SymbolGraphBuilder::new(crate::core::StableKeyInterner::default());
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

    fn file_id(db: &AnalysisDb, relative_path: &str) -> FileId {
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
        let mut db = AnalysisDb::new();
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
        let mut builder = SymbolGraphBuilder::new(crate::core::StableKeyInterner::default());
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
