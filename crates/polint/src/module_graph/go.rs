use crate::core::{Language, ResolutionStatus, UnresolvedReason};
use crate::module_graph::model::{ResolvedImportDraft, ResolverInput};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub(crate) struct GoPackageIndex {
    packages: BTreeMap<String, ()>,
}

impl GoPackageIndex {
    fn is_empty(&self) -> bool {
        self.packages.is_empty()
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
    if metadata.is_empty() {
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
    use std::process::ExitStatus;
    use std::path::{Path, PathBuf};

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
        let source = add_go_file(&mut db, temp.path(), "internal/worker/worker.go", "package worker\n");
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
