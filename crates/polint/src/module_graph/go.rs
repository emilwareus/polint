use crate::core::{AnalysisDb, FileId, Language, ResolutionStatus, UnresolvedReason};
use crate::module_graph::model::{ResolvedImportDraft, ResolverInput};
use crate::module_graph::paths;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

#[derive(Debug, Clone)]
pub(crate) struct GoPackageIndex {
    by_import_path: BTreeMap<String, GoPackageMetadata>,
    file_to_import_path: BTreeMap<FileId, String>,
    module_path: Option<String>,
    setup_missing_reason: Option<String>,
}

impl Default for GoPackageIndex {
    fn default() -> Self {
        Self::setup_missing("Go package metadata was not loaded.")
    }
}

impl GoPackageIndex {
    pub(crate) fn load(root: &Path, db: &AnalysisDb) -> Self {
        Self::load_with_runner(root, db, run_go_list)
    }

    fn load_with_runner(
        root: &Path,
        db: &AnalysisDb,
        run: impl FnOnce(&Path) -> GoCommandOutput,
    ) -> Self {
        if !root.join("go.mod").is_file() {
            return Self::setup_missing("go.mod was not found at the repository root.");
        }

        let output = run(root);
        if !output.status.success() {
            return Self::setup_missing(go_list_failure_reason(&output));
        }

        Self::from_go_list_stdout(root, db, &output.stdout)
    }

    fn from_go_list_stdout(root: &Path, db: &AnalysisDb, stdout: &[u8]) -> Self {
        let mut packages = Vec::new();
        let stream = serde_json::Deserializer::from_slice(stdout).into_iter::<GoListPackage>();
        for parsed in stream {
            match parsed {
                Ok(package) => packages.push(package),
                Err(error) => {
                    return Self::setup_missing(format!(
                        "go list -json ./... failed to parse output: {error}"
                    ));
                }
            }
        }
        packages.sort_by(|left, right| left.import_path.cmp(&right.import_path));

        let file_ids = db
            .files()
            .iter()
            .filter_map(|file| {
                paths::normalize_repo_relative(&file.relative_path)
                    .map(|relative_path| (relative_path, file.id))
            })
            .collect::<BTreeMap<_, _>>();

        let mut index = Self {
            by_import_path: BTreeMap::new(),
            file_to_import_path: BTreeMap::new(),
            module_path: None,
            setup_missing_reason: None,
        };
        for package in packages {
            if package.import_path.is_empty() {
                continue;
            }
            let metadata = GoPackageMetadata::from_go_list_package(root, &file_ids, package);
            if index.module_path.is_none() {
                index.module_path.clone_from(&metadata.module_path);
            }
            for file in &metadata.files {
                index
                    .file_to_import_path
                    .insert(*file, metadata.import_path.clone());
            }
            index
                .by_import_path
                .insert(metadata.import_path.clone(), metadata);
        }
        index
    }

    fn setup_missing(reason: impl Into<String>) -> Self {
        Self {
            by_import_path: BTreeMap::new(),
            file_to_import_path: BTreeMap::new(),
            module_path: None,
            setup_missing_reason: Some(reason.into()),
        }
    }

    pub(crate) fn setup_missing_reason(&self) -> Option<&str> {
        self.setup_missing_reason.as_deref()
    }

    pub(crate) fn is_setup_missing(&self) -> bool {
        self.setup_missing_reason.is_some()
    }

    pub(crate) fn module_path(&self) -> Option<&String> {
        self.module_path.as_ref()
    }

    pub(crate) fn package(&self, import_path: &str) -> Option<&GoPackageMetadata> {
        self.by_import_path.get(import_path)
    }

    pub(crate) fn import_paths(&self) -> impl Iterator<Item = &str> {
        self.by_import_path.keys().map(String::as_str)
    }

    pub(crate) fn import_path_for_file(&self, file: FileId) -> Option<&str> {
        self.file_to_import_path.get(&file).map(String::as_str)
    }

    #[cfg(test)]
    fn load_with_runner_for_test(
        root: &Path,
        db: &AnalysisDb,
        run: impl FnOnce() -> GoCommandOutput,
    ) -> Self {
        Self::load_with_runner(root, db, |_| run())
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GoPackageMetadata {
    import_path: String,
    name: Option<String>,
    files: Vec<FileId>,
    standard: bool,
    module_path: Option<String>,
    module_version: Option<String>,
}

impl GoPackageMetadata {
    fn from_go_list_package(
        root: &Path,
        file_ids: &BTreeMap<String, FileId>,
        package: GoListPackage,
    ) -> Self {
        let files = mapped_package_files(root, file_ids, &package)
            .into_iter()
            .collect();
        let module_path = package
            .module
            .as_ref()
            .and_then(|module| module.path.clone());
        let module_version = package
            .module
            .as_ref()
            .and_then(|module| module.version.clone());
        Self {
            import_path: package.import_path,
            name: package.name,
            files,
            standard: package.standard,
            module_path,
            module_version,
        }
    }

    pub(crate) fn import_path(&self) -> &str {
        &self.import_path
    }

    pub(crate) fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub(crate) fn files(&self) -> impl Iterator<Item = FileId> + '_ {
        self.files.iter().copied()
    }

    pub(crate) fn standard(&self) -> bool {
        self.standard
    }

    pub(crate) fn module_path(&self) -> Option<&str> {
        self.module_path.as_deref()
    }

    pub(crate) fn module_version(&self) -> Option<&str> {
        self.module_version.as_deref()
    }
}

#[derive(Debug, Deserialize)]
struct GoListPackage {
    #[serde(rename = "ImportPath", default)]
    import_path: String,
    #[serde(rename = "Name", default)]
    name: Option<String>,
    #[serde(rename = "Dir", default)]
    dir: Option<PathBuf>,
    #[serde(rename = "GoFiles", default)]
    go_files: Vec<PathBuf>,
    #[serde(rename = "TestGoFiles", default)]
    test_go_files: Vec<PathBuf>,
    #[serde(rename = "CompiledGoFiles", default)]
    compiled_go_files: Vec<PathBuf>,
    #[serde(rename = "Standard", default)]
    standard: bool,
    #[serde(rename = "Module", default)]
    module: Option<GoListModule>,
}

#[derive(Debug, Deserialize)]
struct GoListModule {
    #[serde(rename = "Path", default)]
    path: Option<String>,
    #[serde(rename = "Version", default)]
    version: Option<String>,
}

pub(crate) struct GoCommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl From<std::process::Output> for GoCommandOutput {
    fn from(output: std::process::Output) -> Self {
        Self {
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        }
    }
}

fn run_go_list(root: &Path) -> GoCommandOutput {
    Command::new("go")
        .current_dir(root)
        .env_remove("GOFLAGS")
        .args(["list", "-json", "./..."])
        .output()
        .map(Into::into)
        .unwrap_or_else(|error| GoCommandOutput {
            status: failed_exit_status(),
            stdout: Vec::new(),
            stderr: error.to_string().into_bytes(),
        })
}

fn mapped_package_files(
    root: &Path,
    file_ids: &BTreeMap<String, FileId>,
    package: &GoListPackage,
) -> BTreeSet<FileId> {
    let Some(dir) = &package.dir else {
        return BTreeSet::new();
    };
    package
        .go_files
        .iter()
        .chain(package.test_go_files.iter())
        .chain(package.compiled_go_files.iter())
        .filter_map(|entry| map_package_file(root, dir, entry, file_ids))
        .collect()
}

fn map_package_file(
    root: &Path,
    dir: &Path,
    entry: &Path,
    file_ids: &BTreeMap<String, FileId>,
) -> Option<FileId> {
    let absolute_dir = if dir.is_absolute() {
        dir.to_path_buf()
    } else {
        root.join(dir)
    };
    let absolute_path = if entry.is_absolute() {
        entry.to_path_buf()
    } else {
        absolute_dir.join(entry)
    };
    let relative_path = paths::normalize_repo_relative_path(root, &absolute_path)?;
    file_ids.get(&relative_path).copied()
}

fn go_list_failure_reason(output: &GoCommandOutput) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        format!("go list -json ./... failed: {}", output.status)
    } else {
        format!("go list -json ./... failed: {stderr}")
    }
}

fn failed_exit_status() -> ExitStatus {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(1 << 8)
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::ExitStatusExt;
        ExitStatus::from_raw(1)
    }
}

pub(crate) fn resolve_go_import(
    input: ResolverInput<'_>,
    metadata: &GoPackageIndex,
) -> ResolvedImportDraft {
    let _ = (
        input.root,
        input.db.files().len(),
        input.owner_module,
        input.owner_package,
    );
    if input.import.language != Language::Go {
        return ResolvedImportDraft::unsupported_language();
    }
    if metadata.is_setup_missing() {
        return ResolvedImportDraft::setup_missing();
    }

    let mut draft = ResolvedImportDraft::unresolved(UnresolvedReason::NotFound);
    draft.status = ResolutionStatus::Unresolved;
    draft
}

#[cfg(test)]
mod tests {
    use super::{GoCommandOutput, GoPackageIndex, resolve_go_import};
    use crate::core::{
        AnalysisDb, ImportFact, ImportId, Language, ResolutionStatus, Span, UnresolvedReason,
    };
    use crate::module_graph::model::ResolverInput;
    use std::cell::Cell;
    use std::path::{Path, PathBuf};
    use std::process::ExitStatus;

    #[test]
    fn module_graph_resolver_contracts_go_without_metadata_is_setup_missing() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("main.go"),
            "main.go".to_string(),
            "package main\nimport \"example.com/project/pkg\"\n".to_string(),
        );
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "example.com/project/pkg".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::Go,
        });
        let import = &db.imports()[0];

        let draft = resolve_go_import(
            ResolverInput {
                root: Path::new("."),
                db: &db,
                import,
                ts_resolver: None,
                owner_module: None,
                owner_package: None,
            },
            &GoPackageIndex::default(),
        );

        assert_eq!(draft.status, ResolutionStatus::SetupMissing);
        assert_eq!(draft.reason, Some(UnresolvedReason::SetupMissing));
    }

    #[test]
    fn module_graph_go_metadata_missing_go_mod_is_setup_missing_without_running_go() {
        let temp = tempfile::tempdir().expect("tempdir");
        let db = AnalysisDb::new();
        let command_ran = Cell::new(false);

        let index = GoPackageIndex::load_with_runner_for_test(temp.path(), &db, || {
            command_ran.set(true);
            successful_go_output(b"{}")
        });

        assert!(!command_ran.get());
        assert_eq!(
            index.setup_missing_reason(),
            Some("go.mod was not found at the repository root.")
        );
    }

    #[test]
    fn module_graph_go_metadata_nonzero_go_list_is_setup_missing() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module example.com/app\n")
            .expect("write go.mod");
        let db = AnalysisDb::new();

        let index = GoPackageIndex::load_with_runner_for_test(temp.path(), &db, || {
            failed_go_output(b"package load failed")
        });

        assert_eq!(
            index.setup_missing_reason(),
            Some("go list -json ./... failed: package load failed")
        );
    }

    #[test]
    fn module_graph_go_metadata_parses_json_stream_sorted_by_import_path() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module example.com/app\n")
            .expect("write go.mod");
        let mut db = AnalysisDb::new();
        add_go_file(&mut db, temp.path(), "internal/z/z.go", "package z\n");
        add_go_file(&mut db, temp.path(), "internal/a/a.go", "package a\n");
        let dir_z = temp.path().join("internal/z");
        let dir_a = temp.path().join("internal/a");
        let stdout = format!(
            r#"{{"ImportPath":"example.com/app/internal/z","Name":"z","Dir":{},"GoFiles":["z.go"],"Standard":false,"Module":{{"Path":"example.com/app"}}}}
{{"ImportPath":"example.com/app/internal/a","Name":"a","Dir":{},"GoFiles":["a.go"],"Standard":false,"Module":{{"Path":"example.com/app"}}}}
"#,
            serde_json::to_string(&dir_z.to_string_lossy()).expect("serialize dir"),
            serde_json::to_string(&dir_a.to_string_lossy()).expect("serialize dir"),
        );

        let index = GoPackageIndex::load_with_runner_for_test(temp.path(), &db, || {
            successful_go_output(stdout.as_bytes())
        });

        assert_eq!(
            index.import_paths().collect::<Vec<_>>(),
            vec!["example.com/app/internal/a", "example.com/app/internal/z"]
        );
        assert_eq!(
            index.module_path().map(String::as_str),
            Some("example.com/app")
        );
    }

    #[test]
    fn module_graph_go_metadata_maps_go_files_to_analysis_file_ids() {
        let temp = tempfile::tempdir().expect("tempdir");
        std::fs::write(temp.path().join("go.mod"), "module example.com/app\n")
            .expect("write go.mod");
        let mut db = AnalysisDb::new();
        let source = add_go_file(
            &mut db,
            temp.path(),
            "internal/worker/worker.go",
            "package worker\n",
        );
        let test = add_go_file(
            &mut db,
            temp.path(),
            "internal/worker/worker_test.go",
            "package worker\n",
        );
        let ignored = temp.path().join("internal/worker/generated.go");
        let dir = temp.path().join("internal/worker");
        let stdout = format!(
            r#"{{"ImportPath":"example.com/app/internal/worker","Name":"worker","Dir":{},"GoFiles":["worker.go"],"TestGoFiles":["worker_test.go"],"CompiledGoFiles":["generated.go"],"Standard":false,"Module":{{"Path":"example.com/app"}}}}
"#,
            serde_json::to_string(&dir.to_string_lossy()).expect("serialize dir"),
        );
        std::fs::write(ignored, "package worker\n").expect("write ignored fixture");

        let index = GoPackageIndex::load_with_runner_for_test(temp.path(), &db, || {
            successful_go_output(stdout.as_bytes())
        });
        let package = index
            .package("example.com/app/internal/worker")
            .expect("package metadata exists");

        assert_eq!(package.files().collect::<Vec<_>>(), vec![source, test]);
    }

    fn add_go_file(
        db: &mut AnalysisDb,
        root: &Path,
        relative_path: &str,
        source: &str,
    ) -> crate::core::FileId {
        let path = root.join(relative_path);
        std::fs::create_dir_all(path.parent().expect("test fixture has parent"))
            .expect("create parent dirs");
        std::fs::write(&path, source).expect("write fixture");
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn successful_go_output(stdout: &[u8]) -> GoCommandOutput {
        GoCommandOutput {
            status: successful_status(),
            stdout: stdout.to_vec(),
            stderr: Vec::new(),
        }
    }

    fn failed_go_output(stderr: &[u8]) -> GoCommandOutput {
        GoCommandOutput {
            status: failed_status(),
            stdout: Vec::new(),
            stderr: stderr.to_vec(),
        }
    }

    fn successful_status() -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(0)
        }
    }

    fn failed_status() -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            ExitStatus::from_raw(1 << 8)
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;
            ExitStatus::from_raw(1)
        }
    }
}
