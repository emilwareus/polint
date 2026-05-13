use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, Language, SourceFile};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
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
    cleanup_path: Option<PathBuf>,
}

impl GoWorkspaceEnv {
    pub(crate) fn value(&self) -> &str {
        &self.value
    }
}

impl Drop for GoWorkspaceEnv {
    fn drop(&mut self) {
        if let Some(path) = &self.cleanup_path {
            let _ = fs::remove_file(path);
        }
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
        Ok(Self {
            module_roots,
            package_patterns: string_or_array_setting(settings, "package_patterns", &["./..."]),
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
    if checked_in.is_file() {
        return Ok(GoWorkspaceEnv {
            value: checked_in.to_string_lossy().to_string(),
            cleanup_path: None,
        });
    }
    if needs_synthetic_workspace(root, module_roots) {
        let path = write_synthetic_go_work(root, module_roots)?;
        return Ok(GoWorkspaceEnv {
            value: path.to_string_lossy().to_string(),
            cleanup_path: Some(path),
        });
    }
    Ok(GoWorkspaceEnv {
        value: "off".to_string(),
        cleanup_path: None,
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
        if current.join("go.mod").is_file() {
            return module_root_relative_path(root, &current);
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
    if module_root == "." {
        root.join("go.mod").is_file()
    } else {
        root.join(module_root).join("go.mod").is_file()
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
    !root.join("go.mod").is_file()
}

fn write_synthetic_go_work(
    root: &Path,
    module_roots: &[String],
) -> Result<PathBuf, GoLifecycleError> {
    let path = std::env::temp_dir().join(format!(
        "polint-go-work-{}-{}.work",
        std::process::id(),
        unique_suffix()
    ));
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
        contents.push_str(&quote_go_work_path(&path));
        contents.push('\n');
    }
    contents.push_str(")\n");
    fs::write(&path, contents).map_err(|source| {
        error(format!(
            "failed to write temporary Go workspace `{}`: {source}",
            path.display()
        ))
    })?;
    Ok(path)
}

fn synthetic_go_work_version(root: &Path, module_roots: &[String]) -> String {
    let mut version = MIN_SYNTHETIC_GO_WORK_VERSION.to_string();
    for module_root in module_roots {
        let go_mod = if module_root == "." {
            root.join("go.mod")
        } else {
            root.join(module_root).join("go.mod")
        };
        let Some(module_version) = go_mod_version(&go_mod) else {
            continue;
        };
        if go_version_is_greater(&module_version, &version) {
            version = module_version;
        }
    }
    version
}

fn go_mod_version(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
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

fn quote_go_work_path(path: &Path) -> String {
    format!("{:?}", path.to_string_lossy().replace('\\', "/"))
}

fn unique_suffix() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
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
}
