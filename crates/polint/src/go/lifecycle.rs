use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, Language, SourceFile};
use crate::module_graph::paths::{
    TOPOLOGY_MANIFEST_MAX_BYTES, read_repo_file_to_string_with_limit, repo_file_exists,
    repo_relative_existing_path,
};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const MIN_SYNTHETIC_GO_WORK_VERSION: &str = "1.24";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoAnalysisConfig {
    pub(crate) module_roots: Vec<String>,
    pub(crate) package_patterns: Vec<String>,
    pub(crate) build_tags: Vec<String>,
    pub(crate) include_tests: bool,
    pub(crate) files_without_module_root: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct GoLifecycleError {
    reason: String,
}

impl GoLifecycleError {
    pub(crate) fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Debug)]
pub(crate) struct GoWorkspaceEnv {
    value: String,
    _temp_file: Option<tempfile::NamedTempFile>,
}

impl GoWorkspaceEnv {
    pub(crate) fn value(&self) -> &str {
        &self.value
    }
}

impl GoAnalysisConfig {
    pub(crate) fn from_loaded(
        loaded: &LoadedConfig,
        db: &AnalysisDb,
    ) -> Result<Self, GoLifecycleError> {
        let files = go_files(db);
        Self::from_loaded_files(loaded, &files)
    }

    pub(crate) fn from_loaded_files(
        loaded: &LoadedConfig,
        files: &[&SourceFile],
    ) -> Result<Self, GoLifecycleError> {
        let settings = &loaded.config.languages.go;
        let configured_roots = configured_module_roots(settings)?;
        let (module_roots, files_without_module_root) = if let Some(configured_roots) =
            configured_roots
        {
            let files_without_module_root = files_not_under_module_roots(files, &configured_roots);
            (configured_roots, files_without_module_root)
        } else {
            infer_go_module_roots(&loaded.root, files)
        };
        let package_patterns = validate_package_patterns(string_or_array_setting(
            settings,
            "package_patterns",
            &["./..."],
        ))?;
        Ok(Self {
            module_roots,
            package_patterns,
            build_tags: string_or_array_setting(settings, "build_tags", &[]),
            include_tests: settings
                .get("include_tests")
                .and_then(toml::Value::as_bool)
                .unwrap_or(true),
            files_without_module_root,
        })
    }

    pub(crate) fn missing_module_roots(&self, root: &Path) -> Vec<String> {
        self.module_roots
            .iter()
            .filter(|module_root| !module_root_has_go_mod(root, module_root))
            .cloned()
            .collect()
    }

    pub(crate) fn rooted_package_patterns(&self) -> Vec<String> {
        rooted_package_patterns(&self.module_roots, &self.package_patterns)
    }
}

pub(crate) fn workspace_env(
    root: &Path,
    module_roots: &[String],
) -> Result<GoWorkspaceEnv, GoLifecycleError> {
    let checked_in = root.join("go.work");
    if repo_file_exists(root, "go.work")
        && go_work_covers_module_roots(root, &checked_in, module_roots)
    {
        return Ok(GoWorkspaceEnv {
            value: checked_in.to_string_lossy().to_string(),
            _temp_file: None,
        });
    }
    if needs_synthetic_workspace(root, module_roots) {
        let file = write_synthetic_go_work(root, module_roots)?;
        let value = file.path().to_string_lossy().to_string();
        return Ok(GoWorkspaceEnv {
            value,
            _temp_file: Some(file),
        });
    }
    Ok(GoWorkspaceEnv {
        value: "off".to_string(),
        _temp_file: None,
    })
}

pub(crate) fn go_files(db: &AnalysisDb) -> Vec<&SourceFile> {
    let mut files = db
        .files()
        .iter()
        .filter(|file| file.language == Language::Go)
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    files
}

fn configured_module_roots(
    settings: &BTreeMap<String, toml::Value>,
) -> Result<Option<Vec<String>>, GoLifecycleError> {
    let values = string_or_array_value(
        settings
            .get("module_roots")
            .or_else(|| settings.get("module_root")),
        &[],
    );
    if values.is_empty() {
        return Ok(None);
    }
    let mut roots = BTreeSet::new();
    for value in values {
        roots.insert(normalize_module_root(&value)?);
    }
    Ok(Some(roots.into_iter().collect()))
}

fn normalize_module_root(raw: &str) -> Result<String, GoLifecycleError> {
    let raw = raw.trim();
    if raw.is_empty() || raw == "." {
        return Ok(".".to_string());
    }
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(error(format!(
            "Go module root `{raw}` must be relative to the repository root."
        )));
    }
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(error(format!(
                        "Go module root `{raw}` escapes the repository root."
                    )));
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(error(format!(
                    "Go module root `{raw}` escapes the repository root."
                )));
            }
        }
    }
    if normalized.as_os_str().is_empty() {
        Ok(".".to_string())
    } else {
        Ok(normalized.to_string_lossy().replace('\\', "/"))
    }
}

fn infer_go_module_roots(root: &Path, files: &[&SourceFile]) -> (Vec<String>, Vec<String>) {
    let mut module_roots = BTreeSet::new();
    let mut files_without_module_root = Vec::new();
    for file in files {
        if let Some(module_root) = nearest_go_module_root(root, &file.relative_path) {
            module_roots.insert(module_root);
        } else {
            files_without_module_root.push(file.relative_path.clone());
        }
    }
    (
        module_roots.into_iter().collect(),
        files_without_module_root,
    )
}

fn nearest_go_module_root(root: &Path, relative_path: &str) -> Option<String> {
    let file_path = root.join(relative_path);
    let mut current = file_path.parent()?.to_path_buf();
    loop {
        if let Some(module_root) = module_root_relative_path(root, &current)
            && repo_file_exists(root, module_root_manifest_path(&module_root, "go.mod"))
        {
            return Some(module_root);
        }
        if current == root || !current.pop() {
            return None;
        }
    }
}

fn module_root_relative_path(root: &Path, module_root: &Path) -> Option<String> {
    let relative = module_root.strip_prefix(root).ok()?;
    if relative.as_os_str().is_empty() {
        Some(".".to_string())
    } else {
        Some(relative.to_string_lossy().replace('\\', "/"))
    }
}

fn module_root_has_go_mod(root: &Path, module_root: &str) -> bool {
    repo_file_exists(root, module_root_manifest_path(module_root, "go.mod"))
}

fn module_root_manifest_path(module_root: &str, file_name: &str) -> String {
    if module_root == "." {
        file_name.to_string()
    } else {
        format!("{module_root}/{file_name}")
    }
}

fn files_not_under_module_roots(files: &[&SourceFile], module_roots: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|file| {
            !module_roots
                .iter()
                .any(|module_root| file_is_under_module_root(&file.relative_path, module_root))
        })
        .map(|file| file.relative_path.clone())
        .collect()
}

fn file_is_under_module_root(relative_path: &str, module_root: &str) -> bool {
    module_root == "."
        || relative_path == module_root
        || relative_path
            .strip_prefix(module_root)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn string_or_array_setting(
    settings: &BTreeMap<String, toml::Value>,
    key: &str,
    default: &[&str],
) -> Vec<String> {
    string_or_array_value(settings.get(key), default)
}

fn string_or_array_value(value: Option<&toml::Value>, default: &[&str]) -> Vec<String> {
    let Some(value) = value else {
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

fn validate_package_patterns(patterns: Vec<String>) -> Result<Vec<String>, GoLifecycleError> {
    for pattern in &patterns {
        if pattern.starts_with('-') {
            return Err(error(format!(
                "Go package pattern `{pattern}` must not start with `-` because it would be interpreted as a go list flag."
            )));
        }
    }
    Ok(patterns)
}

fn split_comma(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn rooted_package_patterns(module_roots: &[String], patterns: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut rooted = Vec::new();
    for module_root in module_roots {
        for pattern in patterns {
            let rooted_pattern = rooted_package_pattern(module_root, pattern);
            if seen.insert(rooted_pattern.clone()) {
                rooted.push(rooted_pattern);
            }
        }
    }
    rooted
}

fn rooted_package_pattern(module_root: &str, pattern: &str) -> String {
    if module_root == "." {
        return pattern.to_string();
    }
    let base = format!("./{}", module_root.trim_start_matches("./"));
    if pattern == "." {
        return base;
    }
    if let Some(suffix) = pattern.strip_prefix("./") {
        if suffix.is_empty() {
            return base;
        }
        return format!("{base}/{suffix}");
    }
    pattern.to_string()
}

fn needs_synthetic_workspace(root: &Path, module_roots: &[String]) -> bool {
    if module_roots.len() != 1 || module_roots[0] != "." {
        return true;
    }
    !repo_file_exists(root, "go.mod")
}

pub(crate) fn go_work_covers_module_roots(
    root: &Path,
    go_work: &Path,
    module_roots: &[String],
) -> bool {
    let Some(relative_path) = repo_relative_existing_path(root, go_work) else {
        return false;
    };
    let Ok(contents) =
        read_repo_file_to_string_with_limit(root, &relative_path, TOPOLOGY_MANIFEST_MAX_BYTES)
    else {
        return false;
    };
    let roots = parse_go_work_use_roots(root, &contents);
    module_roots
        .iter()
        .all(|module_root| roots.contains(module_root))
}

fn parse_go_work_use_roots(root: &Path, contents: &str) -> BTreeSet<String> {
    let mut roots = BTreeSet::new();
    let mut in_use_block = false;
    for raw_line in contents.lines() {
        let line = strip_go_line_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if in_use_block {
            let (entry, block_done) = split_go_work_block_line(line);
            if let Some(module_root) = go_work_module_root(root, entry) {
                roots.insert(module_root);
            }
            if block_done {
                in_use_block = false;
            }
            continue;
        }
        let Some(rest) = line.strip_prefix("use") else {
            continue;
        };
        if !rest
            .chars()
            .next()
            .is_none_or(|ch| ch.is_ascii_whitespace() || ch == '(')
        {
            continue;
        }
        let rest = rest.trim_start();
        if let Some(block_rest) = rest.strip_prefix('(') {
            let (entry, block_done) = split_go_work_block_line(block_rest);
            if let Some(module_root) = go_work_module_root(root, entry) {
                roots.insert(module_root);
            }
            in_use_block = !block_done;
        } else if let Some(module_root) = go_work_module_root(root, rest) {
            roots.insert(module_root);
        }
    }
    roots
}

fn strip_go_line_comment(line: &str) -> &str {
    line.split_once("//")
        .map(|(before, _)| before)
        .unwrap_or(line)
}

fn split_go_work_block_line(line: &str) -> (&str, bool) {
    if let Some((before, _)) = line.split_once(')') {
        (before.trim(), true)
    } else {
        (line, false)
    }
}

fn go_work_module_root(root: &Path, value: &str) -> Option<String> {
    let token = first_go_work_path_token(value)?;
    let module_path = parse_go_work_path_token(token)?;
    let path = Path::new(&module_path);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    };
    module_root_relative_path(root, &absolute)
}

fn first_go_work_path_token(value: &str) -> Option<&str> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    if value.starts_with('"') {
        let mut escaped = false;
        for (index, ch) in value.char_indices().skip(1) {
            if ch == '"' && !escaped {
                return Some(&value[..=index]);
            }
            escaped = ch == '\\' && !escaped;
            if ch != '\\' {
                escaped = false;
            }
        }
        return None;
    }
    if let Some(rest) = value.strip_prefix('`') {
        return rest.find('`').map(|index| &value[..=index + 1]);
    }
    value.split_whitespace().next()
}

fn parse_go_work_path_token(token: &str) -> Option<String> {
    if token.starts_with('"') {
        serde_json::from_str::<String>(token).ok()
    } else if token.starts_with('`') && token.ends_with('`') {
        Some(token[1..token.len() - 1].to_string())
    } else {
        Some(token.to_string())
    }
}

fn write_synthetic_go_work(
    root: &Path,
    module_roots: &[String],
) -> Result<tempfile::NamedTempFile, GoLifecycleError> {
    let directory = root.parent().unwrap_or_else(|| Path::new("."));
    let mut file = tempfile::Builder::new()
        .prefix("polint-go-work-")
        .suffix(".work")
        .tempfile_in(directory)
        .map_err(|source| {
            error(format!(
                "failed to create temporary Go workspace in `{}`: {source}",
                directory.display()
            ))
        })?;
    let path = file.path().to_path_buf();
    let work_dir = path.parent().unwrap_or(directory);
    let mut contents = format!(
        "go {}\n\nuse (\n",
        synthetic_go_work_version(root, module_roots)
    );
    for module_root in module_roots {
        let path = if module_root == "." {
            root.to_path_buf()
        } else {
            root.join(module_root)
        };
        contents.push('\t');
        contents.push_str(&quote_go_work_path(&go_work_use_path(work_dir, &path)));
        contents.push('\n');
    }
    contents.push_str(")\n");
    file.write_all(contents.as_bytes()).map_err(|source| {
        error(format!(
            "failed to write temporary Go workspace `{}`: {source}",
            path.display()
        ))
    })?;
    file.flush().map_err(|source| {
        error(format!(
            "failed to flush temporary Go workspace `{}`: {source}",
            path.display()
        ))
    })?;
    Ok(file)
}

fn go_work_use_path(work_dir: &Path, module_path: &Path) -> String {
    module_path
        .strip_prefix(work_dir)
        .unwrap_or(module_path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn synthetic_go_work_version(root: &Path, module_roots: &[String]) -> String {
    let mut version = MIN_SYNTHETIC_GO_WORK_VERSION.to_string();
    for module_root in module_roots {
        let Some(module_version) = go_mod_version(root, module_root) else {
            continue;
        };
        if go_version_is_greater(&module_version, &version) {
            version = module_version;
        }
    }
    version
}

fn go_mod_version(root: &Path, module_root: &str) -> Option<String> {
    let contents = read_repo_file_to_string_with_limit(
        root,
        module_root_manifest_path(module_root, "go.mod"),
        TOPOLOGY_MANIFEST_MAX_BYTES,
    )
    .ok()?;
    contents.lines().find_map(|line| {
        let line = line.trim();
        let version = line.strip_prefix("go ")?;
        version.split_whitespace().next().map(str::to_string)
    })
}

fn go_version_is_greater(left: &str, right: &str) -> bool {
    let mut left = left.split('.').map(|part| part.parse::<u64>().unwrap_or(0));
    let mut right = right
        .split('.')
        .map(|part| part.parse::<u64>().unwrap_or(0));
    loop {
        match (left.next(), right.next()) {
            (Some(left), Some(right)) if left != right => return left > right,
            (Some(left), None) if left != 0 => return true,
            (None, Some(right)) if right != 0 => return false,
            (Some(_), Some(_)) | (Some(_), None) | (None, Some(_)) => {}
            (None, None) => return false,
        }
    }
}

fn quote_go_work_path(path: &str) -> String {
    format!("{path:?}")
}

fn error(reason: String) -> GoLifecycleError {
    GoLifecycleError { reason }
}

pub(crate) fn command_with_go_env(
    root: &Path,
    module_roots: &[String],
) -> Result<(Command, GoWorkspaceEnv), GoLifecycleError> {
    let workspace = workspace_env(root, module_roots)?;
    let mut command = Command::new("go");
    command
        .current_dir(root)
        .env("GOWORK", workspace.value())
        .env_remove("GOFLAGS");
    Ok((command, workspace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synthetic_go_work_version_uses_one_24_as_floor() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.mod"),
            "module example.com/root\n\ngo 1.23\n",
        )
        .expect("write go.mod");

        assert_eq!(
            synthetic_go_work_version(temp.path(), &[".".to_string()]),
            "1.24"
        );
    }

    #[test]
    fn synthetic_go_work_version_tracks_newer_module_directives() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("services/app")).expect("mkdir service");
        std::fs::create_dir_all(temp.path().join("libs/shared")).expect("mkdir lib");
        std::fs::write(
            temp.path().join("services/app/go.mod"),
            "module example.com/app\n\ngo 1.25\n",
        )
        .expect("write service go.mod");
        std::fs::write(
            temp.path().join("libs/shared/go.mod"),
            "module example.com/shared\n\ngo 1.24\n",
        )
        .expect("write shared go.mod");

        assert_eq!(
            synthetic_go_work_version(
                temp.path(),
                &["services/app".to_string(), "libs/shared".to_string()]
            ),
            "1.25"
        );
    }

    #[test]
    fn workspace_env_uses_checked_in_go_work_when_it_covers_roots() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join("go.work"),
            "go 1.24\n\nuse ./services/app\n",
        )
        .expect("write go.work");

        let workspace =
            workspace_env(temp.path(), &["services/app".to_string()]).expect("workspace env");

        assert_eq!(
            workspace.value(),
            temp.path().join("go.work").to_string_lossy().as_ref()
        );
    }

    #[test]
    fn workspace_env_uses_synthetic_go_work_when_checked_in_go_work_misses_roots() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(temp.path().join("services/app")).expect("mkdir service");
        std::fs::write(
            temp.path().join("services/app/go.mod"),
            "module example.com/app\n\ngo 1.24\n",
        )
        .expect("write service go.mod");
        std::fs::write(temp.path().join("go.work"), "go 1.24\n\nuse ./tools/only\n")
            .expect("write go.work");

        let checked_in = temp.path().join("go.work").to_string_lossy().to_string();
        let workspace =
            workspace_env(temp.path(), &["services/app".to_string()]).expect("workspace env");
        let generated_path = workspace.value().to_string();
        let generated = std::fs::read_to_string(&generated_path).expect("read generated go.work");

        assert_ne!(generated_path, checked_in);
        assert!(
            !generated.contains(&temp.path().to_string_lossy().to_string()),
            "{generated}"
        );
        assert!(generated.contains("services/app"), "{generated}");
    }

    #[test]
    fn go_analysis_config_rejects_package_patterns_that_start_with_dash() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            temp.path().join(".polint.toml"),
            "[languages.go]\npackage_patterns = [\"-json\"]\n",
        )
        .expect("write config");
        let mut db = crate::core::AnalysisDb::new();
        let source = "package main\n";
        let path = temp.path().join("main.go");
        std::fs::write(&path, source).expect("write go file");
        db.add_file(path, "main.go".to_string(), source.to_string());
        let loaded = crate::config::load_config(temp.path()).expect("config loads");

        let error = GoAnalysisConfig::from_loaded(&loaded, &db).expect_err("pattern is rejected");

        assert!(
            error.reason().contains(
                "must not start with `-` because it would be interpreted as a go list flag"
            )
        );
    }
}
