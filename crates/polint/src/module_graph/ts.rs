use crate::config::LoadedConfig;
use crate::core::{
    AnalysisDb, FileId, Language, ModuleEdgeKind, ModuleNodeId, ResolutionPrecision,
    ResolutionStatus, SourceFile, UnresolvedReason,
};
use crate::module_graph::formats::js_lockfile::{
    JsLockfileKind, JsLockfileManifest, JsLockfilePackage, JsPackageManager, parse_js_lockfile,
};
use crate::module_graph::formats::package_json::{PackageJsonManifest, parse_package_json};
use crate::module_graph::model::{ModuleNodeDraft, ResolvedImportDraft, ResolverInput};
use crate::module_graph::paths::{normalize_path, normalize_repo_relative};
use crate::module_graph::topology::{
    DependencyRequirementFact, DependencyRequirementId, RepoTopologyOverlayFact,
    RepoTopologyOverlayId, RepoTopologyOverlayKind, RequirementKind, ResolvedDependencyEdgeFact,
    ResolvedDependencyEdgeId, ResolvedDependencyKind, SourceSetFact, SourceSetId, SourceSetKind,
    TopologyOutput, TopologyPackageFact, TopologyPackageId, TopologyPackageKind, TopologyPrecision,
    TopologyStatus, WorkspaceRootFact, WorkspaceRootId, WorkspaceRootKind,
};
use crate::ts::DYNAMIC_IMPORT_SPECIFIER;
use oxc_resolver::{ResolveError, ResolveOptions, Resolver, TsconfigDiscovery};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
use std::cell::Cell;

#[cfg(test)]
thread_local! {
    static RESOLVER_CONTEXT_CONSTRUCTIONS: Cell<usize> = const { Cell::new(0) };
}

#[cfg(test)]
mod topology {
    use super::collect_ts_topology;
    use crate::config::load_config;
    use crate::core::{AnalysisDb, FileId};
    use crate::module_graph::topology::{
        RepoTopologyOverlayKind, SourceSetKind, TopologyPackageKind, TopologyPrecision,
        TopologyStatus, WorkspaceRootKind,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn collect_ts_topology_emits_js_workspace_and_member_packages() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","workspaces":["packages/*"]}"#,
        );
        write_fixture(
            temp.path(),
            "packages/ui/package.json",
            r#"{"name":"@acme/ui","version":"1.0.0"}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(
            &mut db,
            temp.path(),
            "packages/ui/src/index.ts",
            "export const ui = true;\n",
        );
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.workspace_roots.iter().any(|root| {
            root.kind == WorkspaceRootKind::JsWorkspace
                && root.root_path == "."
                && root.manifest_path.as_deref() == Some("package.json")
        }));
        assert!(output.packages.iter().any(|package| {
            package.kind == TopologyPackageKind::JsPackage
                && package.name == "@acme/ui"
                && package.path == "packages/ui"
        }));
        let root = output
            .workspace_roots
            .iter()
            .find(|root| root.root_path == ".")
            .expect("root workspace exists");
        let package = output
            .packages
            .iter()
            .find(|package| package.path == "packages/ui")
            .expect("workspace package exists");
        assert_eq!(package.workspace_root, Some(root.id));
        let source_set = output
            .source_sets
            .iter()
            .find(|source_set| source_set.path == "packages/ui/src/index.ts")
            .expect("workspace source set exists");
        assert_eq!(source_set.root, Some(root.id));
        assert_eq!(source_set.package, Some(package.id));
    }

    #[test]
    fn collect_ts_topology_expands_nested_workspace_globs_relative_to_manifest() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "web/package.json",
            r#"{"name":"web-root","workspaces":["packages/*"]}"#,
        );
        write_fixture(
            temp.path(),
            "web/packages/ui/package.json",
            r#"{"name":"@acme/ui","version":"1.0.0"}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(
            &mut db,
            temp.path(),
            "web/packages/ui/src/index.ts",
            "export const ui = true;\n",
        );
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.workspace_roots.iter().any(|root| {
            root.kind == WorkspaceRootKind::JsWorkspace && root.root_path == "web"
        }));
        assert!(output.packages.iter().any(|package| {
            package.kind == TopologyPackageKind::JsPackage
                && package.name == "@acme/ui"
                && package.path == "web/packages/ui"
        }));
    }

    #[test]
    fn collect_ts_topology_records_package_manager_and_lockfile_evidence() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"pnpm@9.0.0"}"#,
        );
        write_fixture(
            temp.path(),
            "pnpm-workspace.yaml",
            "packages:\n  - packages/*\n",
        );
        write_fixture(temp.path(), "package-lock.json", r#"{"lockfileVersion":3}"#);
        write_fixture(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
        write_fixture(temp.path(), "yarn.lock", "# yarn lockfile\n");
        write_fixture(temp.path(), "bun.lock", "# bun lockfile\n");
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);
        let labels = output
            .overlays
            .iter()
            .map(|overlay| overlay.label.as_str())
            .collect::<Vec<_>>();

        assert!(labels.contains(&"packageManager:pnpm@9.0.0"));
        assert!(labels.contains(&"pnpm-workspace.yaml:packages/*"));
        assert!(labels.contains(&"lockfile:package-lock.json:package-lock-v3"));
        assert!(labels.contains(&"lockfile:pnpm-lock.yaml:pnpm-lock-v9.0"));
        assert!(labels.contains(&"lockfile:yarn.lock:yarn-classic-v1"));
        assert!(labels.contains(&"lockfile:bun.lock:bun-lock-unknown"));
        assert!(labels.contains(&"package-manager:selected:pnpm:lockfile:pnpm-lock.yaml"));
        assert!(!labels.iter().any(|label| label.contains("bun.lockb")));
        let package_lock_overlay = output
            .overlays
            .iter()
            .find(|overlay| overlay.label == "lockfile:package-lock.json:package-lock-v3")
            .expect("package-lock overlay exists");
        assert_eq!(
            package_lock_overlay.precision,
            TopologyPrecision::ExactStatic
        );
        assert_eq!(package_lock_overlay.status, TopologyStatus::Present);
    }

    #[test]
    fn collect_ts_topology_classifies_ts_source_sets() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(temp.path(), "package.json", r#"{"name":"root"}"#);
        let mut db = AnalysisDb::new();
        let source = add_fixture_file(&mut db, temp.path(), "src/app.ts", "export {};\n");
        let test = add_fixture_file(&mut db, temp.path(), "src/app.test.ts", "export {};\n");
        let spec = add_fixture_file(&mut db, temp.path(), "src/app.spec.tsx", "export {};\n");
        let nested_test =
            add_fixture_file(&mut db, temp.path(), "src/__tests__/app.ts", "export {};\n");
        let generated =
            add_fixture_file(&mut db, temp.path(), "generated/client.ts", "export {};\n");
        let generated_named =
            add_fixture_file(&mut db, temp.path(), "src/api.generated.ts", "export {};\n");
        let vendor = add_fixture_file(
            &mut db,
            temp.path(),
            "node_modules/pkg/index.ts",
            "export {};\n",
        );
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(source_set_for_file(&output, source, SourceSetKind::Source));
        assert!(source_set_for_file(&output, test, SourceSetKind::Test));
        assert!(source_set_for_file(&output, spec, SourceSetKind::Test));
        assert!(source_set_for_file(
            &output,
            nested_test,
            SourceSetKind::Test
        ));
        assert!(source_set_for_file(
            &output,
            generated,
            SourceSetKind::Generated
        ));
        assert!(source_set_for_file(
            &output,
            generated_named,
            SourceSetKind::Generated
        ));
        assert!(source_set_for_file(&output, vendor, SourceSetKind::Vendor));
    }

    #[test]
    fn collect_ts_topology_records_tsconfig_alias_and_reference_evidence() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(temp.path(), "package.json", r#"{"name":"root"}"#);
        write_fixture(
            temp.path(),
            "tsconfig.json",
            r#"{
  "compilerOptions": {
    "baseUrl": ".",
    "paths": { "@/*": ["src/*"] },
    "rootDirs": ["src", "generated"]
  },
  "references": [{ "path": "./packages/ui" }]
}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.overlays.iter().any(|overlay| {
            overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                && overlay.label == "tsconfig:paths:@/*"
        }));
        assert!(output.overlays.iter().any(|overlay| {
            overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                && overlay.label == "tsconfig:baseUrl:."
        }));
        assert!(output.overlays.iter().any(|overlay| {
            overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                && overlay.label == "tsconfig:rootDirs:generated"
        }));
        assert!(output.overlays.iter().any(|overlay| {
            overlay.kind == RepoTopologyOverlayKind::SourceOfTruthDirectory
                && overlay.label == "tsconfig:reference:./packages/ui"
        }));
    }

    fn source_set_for_file(
        output: &crate::module_graph::topology::TopologyOutput,
        file: FileId,
        kind: SourceSetKind,
    ) -> bool {
        output
            .source_sets
            .iter()
            .any(|source_set| source_set.files == vec![file] && source_set.kind == kind)
    }

    fn write_fixture(root: &Path, relative_path: &str, source: &str) -> PathBuf {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("test fixture path has parent")).expect("mkdirs");
        fs::write(&path, source).expect("write fixture");
        path
    }

    fn add_fixture_file(
        db: &mut AnalysisDb,
        root: &Path,
        relative_path: &str,
        source: &str,
    ) -> FileId {
        let path = write_fixture(root, relative_path, source);
        db.add_file(path, relative_path.to_string(), source.to_string())
    }
}

#[cfg(test)]
mod dependency_topology {
    use super::collect_ts_topology;
    use crate::config::load_config;
    use crate::core::AnalysisDb;
    use crate::module_graph::topology::{
        RequirementKind, ResolvedDependencyKind, TopologyPrecision, TopologyStatus,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn collect_ts_topology_emits_declared_dependency_requirement_kinds() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{
  "name": "root",
  "dependencies": { "react": "^18.0.0", "@acme/workspace": "workspace:*" },
  "devDependencies": { "vitest": "^2.0.0" },
  "peerDependencies": { "typescript": "^5.0.0" },
  "optionalDependencies": { "fsevents": "^2.0.0" },
  "bundleDependencies": ["left-pad"]
}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);
        let requirements = output
            .dependency_requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.target_name.as_str(),
                    requirement.version_requirement.as_deref(),
                    requirement.kind,
                    requirement.status,
                )
            })
            .collect::<Vec<_>>();

        assert!(requirements.contains(&(
            "react",
            Some("^18.0.0"),
            RequirementKind::Direct,
            TopologyStatus::Present
        )));
        assert!(requirements.contains(&(
            "@acme/workspace",
            Some("workspace:*"),
            RequirementKind::Workspace,
            TopologyStatus::Present
        )));
        assert!(requirements.contains(&(
            "vitest",
            Some("^2.0.0"),
            RequirementKind::Dev,
            TopologyStatus::Present
        )));
        assert!(requirements.contains(&(
            "typescript",
            Some("^5.0.0"),
            RequirementKind::Peer,
            TopologyStatus::Present
        )));
        assert!(requirements.contains(&(
            "fsevents",
            Some("^2.0.0"),
            RequirementKind::Optional,
            TopologyStatus::Present
        )));
        assert!(requirements.contains(&(
            "left-pad",
            None,
            RequirementKind::Bundled,
            TopologyStatus::Present
        )));
    }

    #[test]
    fn collect_ts_topology_emits_package_lock_selected_versions() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "package-lock.json",
            r#"{
  "lockfileVersion": 3,
  "packages": {
    "": { "name": "root", "version": "1.0.0" },
    "node_modules/react": { "version": "18.2.0" }
  }
}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react"
                && edge.resolved_version.as_deref() == Some("18.2.0")
                && edge.kind == ResolvedDependencyKind::LockfileSelected
                && edge.precision == TopologyPrecision::ExactLockfile
                && edge.status == TopologyStatus::Resolved
                && edge.stable_key.contains("source=package-lock.json")
                && edge.stable_key.contains("schema=package-lock-v3")
        }));
    }

    #[test]
    fn collect_ts_topology_keeps_nested_package_lock_entries_distinct() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "package-lock.json",
            r#"{
  "lockfileVersion": 3,
  "packages": {
    "": { "name": "root", "version": "1.0.0" },
    "node_modules/react": { "version": "18.2.0" },
    "node_modules/plugin/node_modules/react": { "version": "18.2.0" }
  }
}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);
        let react_edges = output
            .resolved_dependency_edges
            .iter()
            .filter(|edge| {
                edge.package_name == "react" && edge.resolved_version.as_deref() == Some("18.2.0")
            })
            .collect::<Vec<_>>();

        assert_eq!(react_edges.len(), 2);
        assert_ne!(react_edges[0].stable_key, react_edges[1].stable_key);
        assert!(react_edges.iter().any(|edge| {
            edge.stable_key
                .contains("node_modules/plugin/node_modules/react")
        }));
    }

    #[test]
    fn collect_ts_topology_scopes_inherited_package_lock_entries_to_workspace_member() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"npm@10.0.0","workspaces":["packages/*"]}"#,
        );
        write_fixture(
            temp.path(),
            "packages/a/package.json",
            r#"{"name":"a","dependencies":{"react":"^17.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "packages/b/package.json",
            r#"{"name":"b","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "package-lock.json",
            r#"{
  "lockfileVersion": 3,
  "packages": {
    "": { "name": "root", "version": "1.0.0" },
    "node_modules/react": { "version": "19.0.0" },
    "packages/a": { "name": "a", "version": "1.0.0" },
    "packages/a/node_modules/react": { "version": "17.0.2" },
    "packages/b": { "name": "b", "version": "1.0.0" },
    "packages/b/node_modules/react": { "version": "18.2.0" }
  }
}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(
            &mut db,
            temp.path(),
            "packages/a/src/index.ts",
            "export {};\n",
        );
        add_fixture_file(
            &mut db,
            temp.path(),
            "packages/b/src/index.ts",
            "export {};\n",
        );
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);
        let package_a = output
            .packages
            .iter()
            .find(|package| package.path == "packages/a")
            .expect("package a exists");
        let package_b = output
            .packages
            .iter()
            .find(|package| package.path == "packages/b")
            .expect("package b exists");

        assert_eq!(
            lockfile_versions_for(&output, package_a.id, "react"),
            vec!["17.0.2".to_string()]
        );
        assert_eq!(
            lockfile_versions_for(&output, package_b.id, "react"),
            vec!["18.2.0".to_string()]
        );
    }

    #[test]
    fn collect_ts_topology_prefers_npm_shrinkwrap_over_package_lock() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"npm@10.0.0","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "package-lock.json",
            r#"{"lockfileVersion":3,"packages":{"node_modules/react":{"version":"18.2.0"}}}"#,
        );
        write_fixture(
            temp.path(),
            "npm-shrinkwrap.json",
            r#"{"lockfileVersion":3,"packages":{"node_modules/react":{"version":"18.3.0"}}}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react"
                && edge.resolved_version.as_deref() == Some("18.3.0")
                && edge.stable_key.contains("source=npm-shrinkwrap.json")
                && edge.precision == TopologyPrecision::ExactLockfile
                && edge.status == TopologyStatus::Resolved
        }));
        assert!(!output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react" && edge.resolved_version.as_deref() == Some("18.2.0")
        }));
    }

    #[test]
    fn collect_ts_topology_emits_pnpm_importer_selected_versions() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"pnpm@9.0.0","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "pnpm-lock.yaml",
            r#"
lockfileVersion: '9.0'
importers:
  .:
    dependencies:
      react:
        specifier: ^18.0.0
        version: 18.2.0
"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react"
                && edge.resolved_version.as_deref() == Some("18.2.0")
                && edge.stable_key.contains("source=pnpm-lock.yaml")
                && edge.stable_key.contains("schema=pnpm-lock-v9.0")
                && edge.precision == TopologyPrecision::ExactLockfile
                && edge.status == TopologyStatus::Resolved
        }));
    }

    #[test]
    fn collect_ts_topology_uses_pnpm_workspace_yaml_for_member_lockfile_selection() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"pnpm@9.0.0"}"#,
        );
        write_fixture(
            temp.path(),
            "pnpm-workspace.yaml",
            "packages:\n  - packages/*\n",
        );
        write_fixture(
            temp.path(),
            "packages/ui/package.json",
            r#"{"name":"ui","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "pnpm-lock.yaml",
            r#"
lockfileVersion: '9.0'
importers:
  packages/ui:
    dependencies:
      react:
        specifier: ^18.0.0
        version: 18.2.0
"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(
            &mut db,
            temp.path(),
            "packages/ui/src/index.ts",
            "export {};\n",
        );
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);
        let package = output
            .packages
            .iter()
            .find(|package| package.path == "packages/ui")
            .expect("workspace member package exists");

        assert!(output.workspace_roots.iter().any(|root| {
            root.root_path == "."
                && root.manifest_path.as_deref() == Some("package.json")
                && root.status == TopologyStatus::Present
        }));
        assert_eq!(
            lockfile_versions_for(&output, package.id, "react"),
            vec!["18.2.0".to_string()]
        );
    }

    #[test]
    fn collect_ts_topology_prefers_root_explicit_manager_over_stale_member_lockfile() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"pnpm@9.0.0"}"#,
        );
        write_fixture(
            temp.path(),
            "pnpm-workspace.yaml",
            "packages:\n  - packages/*\n",
        );
        write_fixture(
            temp.path(),
            "packages/ui/package.json",
            r#"{"name":"ui","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "packages/ui/package-lock.json",
            r#"{"lockfileVersion":3,"packages":{"node_modules/react":{"version":"19.0.0"}}}"#,
        );
        write_fixture(
            temp.path(),
            "pnpm-lock.yaml",
            r#"
lockfileVersion: '9.0'
importers:
  packages/ui:
    dependencies:
      react:
        specifier: ^18.0.0
        version: 18.2.0
"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(
            &mut db,
            temp.path(),
            "packages/ui/src/index.ts",
            "export {};\n",
        );
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);
        let package = output
            .packages
            .iter()
            .find(|package| package.path == "packages/ui")
            .expect("workspace member package exists");

        assert_eq!(
            lockfile_versions_for(&output, package.id, "react"),
            vec!["18.2.0".to_string()]
        );
        assert!(!output.resolved_dependency_edges.iter().any(|edge| {
            edge.from_package == Some(package.id)
                && edge.package_name == "react"
                && edge.resolved_version.as_deref() == Some("19.0.0")
        }));
    }

    #[test]
    fn collect_ts_topology_emits_yarn_classic_selected_versions() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"yarn@1.22.22","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "yarn.lock",
            r#"
react@^18.0.0:
  version "18.2.0"
  resolved "https://registry.yarnpkg.com/react/-/react-18.2.0.tgz"
"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react"
                && edge.resolved_version.as_deref() == Some("18.2.0")
                && edge.stable_key.contains("source=yarn.lock")
                && edge.stable_key.contains("schema=yarn-classic-v1")
                && edge.precision == TopologyPrecision::ExactLockfile
                && edge.status == TopologyStatus::Resolved
        }));
    }

    #[test]
    fn collect_ts_topology_emits_yarn_berry_selected_versions() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"yarn@4.0.0","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "yarn.lock",
            r#"
__metadata:
  version: 8
  cacheKey: 10
"react@npm:^18.0.0":
  version: 18.2.0
  resolution: "react@npm:18.2.0"
"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react"
                && edge.resolved_version.as_deref() == Some("18.2.0")
                && edge.stable_key.contains("source=yarn.lock")
                && edge.stable_key.contains("schema=yarn-berry-v8")
                && edge.precision == TopologyPrecision::ExactLockfile
                && edge.status == TopologyStatus::Resolved
        }));
    }

    #[test]
    fn collect_ts_topology_emits_bun_text_lock_selected_versions() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"bun@1.2.0","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "bun.lock",
            r#"{
  "lockfileVersion": 1,
  "packages": {
    "react": ["react@18.2.0", "", {}, "sha512-test"]
  }
}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react"
                && edge.resolved_version.as_deref() == Some("18.2.0")
                && edge.stable_key.contains("source=bun.lock")
                && edge.stable_key.contains("schema=bun-lock-v1")
                && edge.precision == TopologyPrecision::ExactLockfile
                && edge.status == TopologyStatus::Resolved
        }));
    }

    #[test]
    fn collect_ts_topology_reports_selected_lockfile_without_parseable_entries() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"bun@1.2.0","dependencies":{"uWebSockets.js":"^20.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "bun.lock",
            r#"{
  "lockfileVersion": 1,
  "packages": {
    "uWebSockets.js": [
      "git+https://github.com/uNetworking/uWebSockets.js.git#6609a88",
      "",
      {},
      "uNetworking-uWebSockets.js-6609a88"
    ]
  }
}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.stable_key.contains("js-lock-problem")
                && edge.stable_key.contains("source=bun.lock")
                && edge
                    .stable_key
                    .contains("reason=no-parseable-selected-lockfile-entries")
                && edge.status == TopologyStatus::Unsupported
                && edge.precision == TopologyPrecision::Unsupported
        }));
    }

    #[test]
    fn collect_ts_topology_marks_ambiguous_lockfiles_without_package_manager() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(temp.path(), "pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
        write_fixture(
            temp.path(),
            "yarn.lock",
            r#"
react@^18.0.0:
  version "18.2.0"
"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.stable_key
                .contains("multiple-lockfile-managers-without-packageManager")
                && edge.status == TopologyStatus::Ambiguous
                && edge.precision == TopologyPrecision::Unknown
        }));
    }

    #[test]
    fn collect_ts_topology_reports_missing_selected_manager_lockfile_on_conflict() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"pnpm@9.0.0","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "package-lock.json",
            r#"{"lockfileVersion":3,"packages":{"node_modules/react":{"version":"18.2.0"}}}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react"
                && edge.status == TopologyStatus::MissingLockfile
                && edge.stable_key.contains("source=pnpm-lock.yaml")
        }));
        assert!(!output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react" && edge.resolved_version.as_deref() == Some("18.2.0")
        }));
    }

    #[test]
    fn collect_ts_topology_ignores_bun_lockb_for_lockfile_selection() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","packageManager":"bun@1.2.0","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(temp.path(), "bun.lockb", "binary");
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react"
                && edge.status == TopologyStatus::MissingLockfile
                && edge.stable_key.contains("source=bun.lock")
        }));
        assert!(
            !output
                .overlays
                .iter()
                .any(|overlay| overlay.label.contains("bun.lockb"))
        );
    }

    #[test]
    fn collect_ts_topology_marks_missing_lockfile_when_declared_external_deps_have_no_lockfile() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","dependencies":{"react":"^18.0.0"}}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.package_name == "react"
                && edge.status == TopologyStatus::MissingLockfile
                && edge.precision == TopologyPrecision::Unknown
        }));
    }

    #[test]
    fn collect_ts_topology_emits_unsupported_package_lock_evidence_for_malformed_json() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(temp.path(), "package-lock.json", "{");
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.stable_key.contains("js-lock-unsupported")
                && edge.stable_key.contains("source=package-lock.json")
                && edge.stable_key.contains("reason=malformed-json")
                && edge.status == TopologyStatus::Unsupported
                && edge.precision == TopologyPrecision::Unsupported
        }));
        assert!(
            !output
                .resolved_dependency_edges
                .iter()
                .any(|edge| edge.status == TopologyStatus::MissingLockfile)
        );
    }

    #[test]
    fn collect_ts_topology_emits_unsupported_package_lock_evidence_for_v1() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(
            temp.path(),
            "package.json",
            r#"{"name":"root","dependencies":{"react":"^18.0.0"}}"#,
        );
        write_fixture(
            temp.path(),
            "package-lock.json",
            r#"{
  "lockfileVersion": 1,
  "dependencies": {
    "react": { "version": "18.2.0" }
  }
}"#,
        );
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);

        assert!(output.resolved_dependency_edges.iter().any(|edge| {
            edge.stable_key.contains("js-lock-unsupported")
                && edge.stable_key.contains("schema=package-lock-v1")
                && edge
                    .stable_key
                    .contains("reason=package-lock-v1-dependency-tree-is-not-supported")
                && edge.status == TopologyStatus::Unsupported
                && edge.precision == TopologyPrecision::Unsupported
        }));
    }

    #[test]
    fn collect_ts_topology_marks_malformed_package_json_unsupported() {
        let temp = tempfile::tempdir().expect("tempdir");
        write_fixture(temp.path(), "package.json", "{");
        let mut db = AnalysisDb::new();
        add_fixture_file(&mut db, temp.path(), "src/index.ts", "export {};\n");
        let loaded = load_config(temp.path()).expect("config loads");

        let output = collect_ts_topology(&loaded, &db, None);
        let package = output
            .packages
            .iter()
            .find(|package| package.path == ".")
            .expect("root package row exists");

        assert_eq!(package.precision, TopologyPrecision::Unknown);
        assert_eq!(package.status, TopologyStatus::Unsupported);
        assert!(output.overlays.iter().any(|overlay| {
            overlay.label == "package-json-unsupported:malformed-json"
                && overlay.path.as_deref() == Some("package.json")
                && overlay.precision == TopologyPrecision::Unknown
                && overlay.status == TopologyStatus::Unsupported
        }));
    }

    fn lockfile_versions_for(
        output: &crate::module_graph::topology::TopologyOutput,
        package_id: crate::module_graph::topology::TopologyPackageId,
        package_name: &str,
    ) -> Vec<String> {
        output
            .resolved_dependency_edges
            .iter()
            .filter(|edge| {
                edge.from_package == Some(package_id)
                    && edge.package_name == package_name
                    && edge.kind == ResolvedDependencyKind::LockfileSelected
                    && edge.precision == TopologyPrecision::ExactLockfile
            })
            .filter_map(|edge| edge.resolved_version.clone())
            .collect()
    }

    fn write_fixture(root: &Path, relative_path: &str, source: &str) -> PathBuf {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("test fixture path has parent")).expect("mkdirs");
        fs::write(&path, source).expect("write fixture");
        path
    }

    fn add_fixture_file(db: &mut AnalysisDb, root: &Path, relative_path: &str, source: &str) {
        let path = write_fixture(root, relative_path, source);
        db.add_file(path, relative_path.to_string(), source.to_string());
    }
}

#[derive(Debug)]
pub(crate) struct TsResolverContext {
    resolver: Resolver,
    root: PathBuf,
    file_by_absolute_normalized_path: BTreeMap<PathBuf, FileId>,
    path_aliases_by_config_dir: BTreeMap<PathBuf, Vec<String>>,
    pub(crate) owner_module: Option<ModuleNodeId>,
}

impl TsResolverContext {
    pub(crate) fn new(root: &Path, db: &AnalysisDb, owner_module: Option<ModuleNodeId>) -> Self {
        #[cfg(test)]
        RESOLVER_CONTEXT_CONSTRUCTIONS.with(|count| count.set(count.get() + 1));

        let root = normalize_path(root).unwrap_or_else(|| root.to_path_buf());
        let file_by_absolute_normalized_path = db
            .files()
            .iter()
            .filter_map(|file| {
                let absolute = if file.path.is_absolute() {
                    file.path.clone()
                } else {
                    root.join(&file.relative_path)
                };
                normalize_path(&absolute).map(|path| (path, file.id))
            })
            .collect();

        Self {
            resolver: Resolver::new(resolve_options()),
            path_aliases_by_config_dir: collect_ts_path_aliases(&root, db),
            root,
            file_by_absolute_normalized_path,
            owner_module,
        }
    }
}

pub(crate) fn resolve_ts_import(input: ResolverInput<'_>) -> ResolvedImportDraft {
    let _ = (input.root, input.owner_module, input.owner_package);
    if !input.import.language.is_ts_family() {
        return ResolvedImportDraft::unsupported_language();
    }
    if input.import.path == DYNAMIC_IMPORT_SPECIFIER {
        return ResolvedImportDraft {
            target: None,
            status: ResolutionStatus::Dynamic,
            precision: ResolutionPrecision::None,
            reason: Some(UnresolvedReason::DynamicExpression),
            edge_kind: None,
        };
    }

    let Some(context) = input.ts_resolver else {
        return ResolvedImportDraft::setup_missing();
    };
    let _owner_module = input.owner_module.or(context.owner_module);
    let Some(importer) = input.db.file(input.import.file) else {
        return ResolvedImportDraft::unresolved(UnresolvedReason::NotFound);
    };
    let importer_path = if importer.path.is_absolute() {
        importer.path.clone()
    } else {
        context.root.join(&importer.relative_path)
    };
    let Some(importer_path) = normalize_path(&importer_path) else {
        return ResolvedImportDraft::unresolved(UnresolvedReason::NotFound);
    };

    match context
        .resolver
        .resolve_file(&importer_path, input.import.path.as_str())
    {
        Ok(resolution) => resolved_path_draft(context, input, resolution.path()),
        Err(ResolveError::Builtin { resolved, .. }) => {
            external_draft(resolved, input.import.language)
        }
        Err(ResolveError::MatchedAliasNotFound(_, _)) => {
            ResolvedImportDraft::unresolved(UnresolvedReason::NotFound)
        }
        Err(ResolveError::NotFound(_)) => {
            if tsconfig_path_alias_matches(context, &importer_path, &input.import.path) {
                ResolvedImportDraft::unresolved(UnresolvedReason::NotFound)
            } else if is_external_package_specifier(&input.import.path) {
                external_draft(input.import.path.clone(), input.import.language)
            } else {
                ResolvedImportDraft::unresolved(UnresolvedReason::NotFound)
            }
        }
        Err(
            ResolveError::TsconfigNotFound(_)
            | ResolveError::TsconfigSelfReference(_)
            | ResolveError::TsconfigCircularExtend(_)
            | ResolveError::Json(_)
            | ResolveError::IOError(_),
        ) => ResolvedImportDraft::setup_missing(),
        Err(_) => {
            if is_external_package_specifier(&input.import.path) {
                external_draft(input.import.path.clone(), input.import.language)
            } else {
                ResolvedImportDraft::unresolved(UnresolvedReason::ResolverError)
            }
        }
    }
}

fn resolved_path_draft(
    context: &TsResolverContext,
    input: ResolverInput<'_>,
    resolved_path: &Path,
) -> ResolvedImportDraft {
    let Some(normalized_path) = normalize_path(resolved_path) else {
        return ResolvedImportDraft::unresolved(UnresolvedReason::NotFound);
    };
    if let Some(file) = context
        .file_by_absolute_normalized_path
        .get(&normalized_path)
        .copied()
    {
        return ResolvedImportDraft {
            target: Some(ModuleNodeDraft::file(
                file,
                input.db.path_for(file),
                input.import.language,
            )),
            status: ResolutionStatus::Resolved,
            precision: ResolutionPrecision::ExactFile,
            reason: None,
            edge_kind: Some(ModuleEdgeKind::Imports),
        };
    }

    if !normalized_path.starts_with(&context.root)
        || is_external_package_specifier(&input.import.path)
    {
        external_draft(input.import.path.clone(), input.import.language)
    } else {
        ResolvedImportDraft::unresolved(UnresolvedReason::NotFound)
    }
}

fn external_draft(label: String, language: Language) -> ResolvedImportDraft {
    ResolvedImportDraft {
        target: Some(ModuleNodeDraft::external(label, Some(language))),
        status: ResolutionStatus::External,
        precision: ResolutionPrecision::ExternalPackage,
        reason: None,
        edge_kind: Some(ModuleEdgeKind::DependsOn),
    }
}

pub(crate) fn collect_ts_topology(
    loaded: &LoadedConfig,
    db: &AnalysisDb,
    _resolver: Option<&TsResolverContext>,
) -> TopologyOutput {
    let mut output = TopologyOutput::default();
    let ts_files = db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
        .collect::<Vec<_>>();
    let package_manifests = collect_package_manifests(loaded, &ts_files);
    let workspace_roots = js_workspace_roots(loaded, &package_manifests);

    let mut root_ids_by_path = BTreeMap::new();
    for package_path in &workspace_roots {
        let id = WorkspaceRootId(output.workspace_roots.len() as u64);
        output.workspace_roots.push(WorkspaceRootFact {
            id,
            kind: WorkspaceRootKind::JsWorkspace,
            root_path: package_path.clone(),
            manifest_path: Some(package_manifest_path(package_path)),
            language: Some(Language::TypeScript),
            stable_key: format!("js-workspace:{package_path}"),
            producer_id: TS_TOPOLOGY_PROVIDER_ID,
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        });
        root_ids_by_path.insert(package_path.clone(), id);
    }

    let mut package_ids_by_path = BTreeMap::new();
    for (package_path, manifest) in &package_manifests {
        let (precision, status) = package_manifest_topology_state(manifest);
        let id = TopologyPackageId(output.packages.len() as u64);
        let workspace_root = workspace_root_for_package(package_path, &workspace_roots)
            .and_then(|root| root_ids_by_path.get(root).copied());
        output.packages.push(TopologyPackageFact {
            id,
            workspace_root,
            package: None,
            module_node: None,
            kind: TopologyPackageKind::JsPackage,
            name: manifest
                .name
                .clone()
                .unwrap_or_else(|| package_path.clone()),
            version: manifest.version.clone(),
            path: package_path.clone(),
            language: Some(Language::TypeScript),
            stable_key: format!(
                "js-package:{package_path}:{}",
                manifest.name.as_deref().unwrap_or("")
            ),
            producer_id: TS_TOPOLOGY_PROVIDER_ID,
            precision,
            status,
        });
        package_ids_by_path.insert(package_path.clone(), id);
    }

    let lockfile_selections = select_js_lockfiles(loaded, &package_manifests, &workspace_roots);

    for (package_path, manifest) in &package_manifests {
        emit_package_manager_overlays(&mut output, package_path, manifest);
        emit_package_manifest_unsupported_overlays(&mut output, package_path, manifest);
        emit_lockfile_overlays(&mut output, loaded, package_path);
        if let Some(selection) = lockfile_selections.get(package_path) {
            emit_lockfile_selection_overlay(&mut output, package_path, selection);
        }
        if let Some(package_id) = package_ids_by_path.get(package_path).copied() {
            emit_package_requirements(&mut output, package_id, package_path, manifest);
        }
    }
    for (package_path, package_id) in &package_ids_by_path {
        if let Some(selection) = lockfile_selections.get(package_path) {
            emit_js_lockfile_edges(&mut output, loaded, package_path, *package_id, selection);
        }
    }
    emit_pnpm_workspace_overlays(&mut output, loaded);
    emit_tsconfig_overlays(&mut output, loaded, &ts_files);

    for file in ts_files {
        let package_path = nearest_package_root_for_relative_path(loaded, &file.relative_path);
        let package = package_path
            .as_ref()
            .and_then(|path| package_ids_by_path.get(path).copied());
        let root = package_path
            .as_ref()
            .and_then(|path| workspace_root_for_package(path, &workspace_roots))
            .and_then(|path| root_ids_by_path.get(path).copied());
        let kind = classify_ts_source_set(file);
        output.source_sets.push(SourceSetFact {
            id: SourceSetId(output.source_sets.len() as u64),
            package,
            root,
            kind,
            path: file.relative_path.clone(),
            language: Some(file.language),
            files: vec![file.id],
            stable_key: format!(
                "ts-source-set:{}:{}",
                source_set_kind_label(kind),
                file.relative_path
            ),
            producer_id: TS_TOPOLOGY_PROVIDER_ID,
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        });
    }

    output.normalized()
}

const TS_TOPOLOGY_PROVIDER_ID: &str = "polint.module_graph";

#[derive(Debug, Clone, PartialEq, Eq)]
struct JsLockfileSelection {
    manager: Option<JsPackageManager>,
    root_path: String,
    lockfile: Option<DetectedJsLockfile>,
    status: JsLockfileSelectionStatus,
    reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DetectedJsLockfile {
    kind: JsLockfileKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsLockfileSelectionStatus {
    Selected,
    MissingLockfile,
    Ambiguous,
    Unsupported,
}

fn collect_package_manifests(
    loaded: &LoadedConfig,
    ts_files: &[&SourceFile],
) -> BTreeMap<String, PackageJsonManifest> {
    let mut package_paths = BTreeSet::new();
    if loaded.root.join("package.json").is_file() {
        package_paths.insert(".".to_string());
    }
    for file in ts_files {
        package_paths.extend(package_roots_for_relative_path(loaded, &file.relative_path));
    }

    let mut manifests = BTreeMap::new();
    for package_path in package_paths {
        if let Some(manifest) = read_package_manifest(loaded, &package_path) {
            for workspace in &manifest.workspaces {
                for member in expand_workspace_glob(&loaded.root, &package_path, workspace) {
                    if let Some(member_manifest) = read_package_manifest(loaded, &member) {
                        manifests.insert(member, member_manifest);
                    }
                }
            }
            manifests.insert(package_path, manifest);
        }
    }
    for workspace in root_pnpm_workspace_patterns(loaded) {
        for member in expand_workspace_glob(&loaded.root, ".", &workspace) {
            if let Some(member_manifest) = read_package_manifest(loaded, &member) {
                manifests.insert(member, member_manifest);
            }
        }
    }
    manifests
}

fn js_workspace_roots(
    loaded: &LoadedConfig,
    package_manifests: &BTreeMap<String, PackageJsonManifest>,
) -> BTreeSet<String> {
    let mut roots = package_manifests
        .iter()
        .filter(|(_, manifest)| !manifest.workspaces.is_empty())
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    if package_manifests.contains_key(".") && !root_pnpm_workspace_patterns(loaded).is_empty() {
        roots.insert(".".to_string());
    }
    roots
}

fn package_roots_for_relative_path(loaded: &LoadedConfig, relative_path: &str) -> Vec<String> {
    let Some(mut path) = Path::new(relative_path).parent().map(Path::to_path_buf) else {
        return Vec::new();
    };
    let mut package_roots = Vec::new();
    loop {
        if let Some(package_path) = normalize_repo_relative(path.to_string_lossy()) {
            let manifest_path = package_manifest_path(&package_path);
            if loaded.root.join(manifest_path).is_file() {
                package_roots.push(package_path);
            }
        }
        if !path.pop() {
            break;
        }
    }
    package_roots
}

fn read_package_manifest(loaded: &LoadedConfig, package_path: &str) -> Option<PackageJsonManifest> {
    let manifest_path = package_manifest_path(package_path);
    let contents = fs::read_to_string(loaded.root.join(&manifest_path)).ok()?;
    Some(parse_package_json(&manifest_path, &contents))
}

fn package_manifest_topology_state(
    manifest: &PackageJsonManifest,
) -> (TopologyPrecision, TopologyStatus) {
    if manifest.unsupported.is_empty() {
        (TopologyPrecision::ExactStatic, TopologyStatus::Present)
    } else {
        (TopologyPrecision::Unknown, TopologyStatus::Unsupported)
    }
}

fn package_manifest_path(package_path: &str) -> String {
    if package_path == "." {
        "package.json".to_string()
    } else {
        format!("{package_path}/package.json")
    }
}

fn nearest_package_root_for_relative_path(
    loaded: &LoadedConfig,
    relative_path: &str,
) -> Option<String> {
    let mut path = Path::new(relative_path).parent()?.to_path_buf();
    loop {
        let package_path = normalize_repo_relative(path.to_string_lossy())?;
        let manifest_path = package_manifest_path(&package_path);
        if loaded.root.join(manifest_path).is_file() {
            return Some(package_path);
        }
        if !path.pop() {
            return None;
        }
    }
}

fn expand_workspace_glob(root: &Path, package_path: &str, pattern: &str) -> Vec<String> {
    let Some(base) = pattern.strip_suffix("/*") else {
        return Vec::new();
    };
    let base_path = if package_path == "." {
        PathBuf::from(base)
    } else {
        Path::new(package_path).join(base)
    };
    let Ok(entries) = fs::read_dir(root.join(base_path)) else {
        return Vec::new();
    };
    let mut members = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_type().ok()?.is_dir().then_some(entry.path()))
        .filter(|path| path.join("package.json").is_file())
        .filter_map(|path| crate::module_graph::paths::normalize_repo_relative_path(root, &path))
        .collect::<Vec<_>>();
    members.sort();
    members
}

fn workspace_root_for_package<'a>(
    package_path: &str,
    workspace_roots: &'a BTreeSet<String>,
) -> Option<&'a String> {
    workspace_roots
        .iter()
        .filter(|root| {
            root.as_str() == "."
                || package_path == root.as_str()
                || package_path.starts_with(&format!("{root}/"))
        })
        .max_by_key(|root| root.len())
}

fn select_js_lockfiles(
    loaded: &LoadedConfig,
    package_manifests: &BTreeMap<String, PackageJsonManifest>,
    workspace_roots: &BTreeSet<String>,
) -> BTreeMap<String, JsLockfileSelection> {
    package_manifests
        .iter()
        .map(|(package_path, manifest)| {
            let root_path = lockfile_selection_root(
                loaded,
                package_path,
                manifest,
                package_manifests,
                workspace_roots,
            );
            let root_manifest = package_manifests.get(&root_path).unwrap_or(manifest);
            let selection = select_js_lockfile_at_root(loaded, &root_path, root_manifest);
            (package_path.clone(), selection)
        })
        .collect()
}

fn lockfile_selection_root(
    loaded: &LoadedConfig,
    package_path: &str,
    manifest: &PackageJsonManifest,
    package_manifests: &BTreeMap<String, PackageJsonManifest>,
    workspace_roots: &BTreeSet<String>,
) -> String {
    if manifest.package_manager.is_some() {
        return package_path.to_string();
    }
    let Some(workspace_root) = workspace_root_for_package(package_path, workspace_roots) else {
        return package_path.to_string();
    };
    if workspace_root == package_path {
        return package_path.to_string();
    }
    let Some(root_manifest) = package_manifests.get(workspace_root) else {
        return package_path.to_string();
    };
    if root_manifest.package_manager.is_some() {
        return workspace_root.clone();
    }
    if !detect_js_lockfiles(loaded, package_path).is_empty() {
        return package_path.to_string();
    }
    if !detect_js_lockfiles(loaded, workspace_root).is_empty() {
        workspace_root.clone()
    } else {
        package_path.to_string()
    }
}

fn select_js_lockfile_at_root(
    loaded: &LoadedConfig,
    root_path: &str,
    manifest: &PackageJsonManifest,
) -> JsLockfileSelection {
    let lockfiles = detect_js_lockfiles(loaded, root_path);
    if let Some(package_manager) = manifest.package_manager.as_deref() {
        return match parse_package_manager(package_manager) {
            Ok(manager) => match select_lockfile_for_manager(manager, &lockfiles) {
                Some(lockfile) => JsLockfileSelection {
                    manager: Some(manager),
                    root_path: root_path.to_string(),
                    lockfile: Some(lockfile),
                    status: JsLockfileSelectionStatus::Selected,
                    reason: None,
                },
                None => JsLockfileSelection {
                    manager: Some(manager),
                    root_path: root_path.to_string(),
                    lockfile: None,
                    status: JsLockfileSelectionStatus::MissingLockfile,
                    reason: Some(format!("missing {} lockfile", manager.label())),
                },
            },
            Err(reason) => JsLockfileSelection {
                manager: None,
                root_path: root_path.to_string(),
                lockfile: None,
                status: JsLockfileSelectionStatus::Unsupported,
                reason: Some(reason),
            },
        };
    }

    let managers = lockfiles
        .iter()
        .map(|lockfile| lockfile.kind.manager())
        .collect::<BTreeSet<_>>();
    match managers.len() {
        0 => JsLockfileSelection {
            manager: None,
            root_path: root_path.to_string(),
            lockfile: None,
            status: JsLockfileSelectionStatus::MissingLockfile,
            reason: Some("missing js lockfile".to_string()),
        },
        1 => {
            let manager = *managers.iter().next().expect("manager exists");
            JsLockfileSelection {
                manager: Some(manager),
                root_path: root_path.to_string(),
                lockfile: select_lockfile_for_manager(manager, &lockfiles),
                status: JsLockfileSelectionStatus::Selected,
                reason: None,
            }
        }
        _ => JsLockfileSelection {
            manager: None,
            root_path: root_path.to_string(),
            lockfile: None,
            status: JsLockfileSelectionStatus::Ambiguous,
            reason: Some(format!(
                "multiple lockfile managers without packageManager: {}",
                managers
                    .iter()
                    .map(|manager| manager.label())
                    .collect::<Vec<_>>()
                    .join(",")
            )),
        },
    }
}

fn parse_package_manager(value: &str) -> Result<JsPackageManager, String> {
    let name = value
        .split('@')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    match name.as_str() {
        "npm" => Ok(JsPackageManager::Npm),
        "pnpm" => Ok(JsPackageManager::Pnpm),
        "yarn" => Ok(JsPackageManager::Yarn),
        "bun" => Ok(JsPackageManager::Bun),
        "" => Err("empty packageManager".to_string()),
        other => Err(format!("unsupported packageManager {other}")),
    }
}

fn detect_js_lockfiles(loaded: &LoadedConfig, package_path: &str) -> Vec<DetectedJsLockfile> {
    [
        JsLockfileKind::NpmPackageLock,
        JsLockfileKind::NpmShrinkwrap,
        JsLockfileKind::Pnpm,
        JsLockfileKind::Yarn,
        JsLockfileKind::Bun,
    ]
    .into_iter()
    .filter(|kind| {
        loaded
            .root
            .join(lockfile_relative_path(package_path, *kind))
            .is_file()
    })
    .map(|kind| DetectedJsLockfile { kind })
    .collect()
}

fn select_lockfile_for_manager(
    manager: JsPackageManager,
    lockfiles: &[DetectedJsLockfile],
) -> Option<DetectedJsLockfile> {
    if manager == JsPackageManager::Npm {
        return lockfiles
            .iter()
            .find(|lockfile| lockfile.kind == JsLockfileKind::NpmShrinkwrap)
            .copied()
            .or_else(|| {
                lockfiles
                    .iter()
                    .find(|lockfile| lockfile.kind == JsLockfileKind::NpmPackageLock)
                    .copied()
            });
    }
    lockfiles
        .iter()
        .find(|lockfile| lockfile.kind.manager() == manager)
        .copied()
}

fn lockfile_relative_path(package_path: &str, kind: JsLockfileKind) -> String {
    package_relative_path(package_path, kind.file_name())
}

fn default_lockfile_name_for_manager(manager: Option<JsPackageManager>) -> &'static str {
    match manager {
        Some(JsPackageManager::Npm) => "package-lock.json",
        Some(JsPackageManager::Pnpm) => "pnpm-lock.yaml",
        Some(JsPackageManager::Yarn) => "yarn.lock",
        Some(JsPackageManager::Bun) => "bun.lock",
        None => "js-lockfile",
    }
}

fn importer_path_for_package(lockfile_root: &str, package_path: &str) -> String {
    if lockfile_root == package_path {
        ".".to_string()
    } else if lockfile_root == "." {
        package_path.to_string()
    } else {
        package_path
            .strip_prefix(&format!("{lockfile_root}/"))
            .unwrap_or(package_path)
            .to_string()
    }
}

fn stable_label_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '.' || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn emit_package_manager_overlays(
    output: &mut TopologyOutput,
    package_path: &str,
    manifest: &PackageJsonManifest,
) {
    if let Some(manager) = &manifest.package_manager {
        push_overlay(
            output,
            package_path,
            format!("packageManager:{manager}"),
            Some(package_manifest_path(package_path)),
            TopologyPrecision::ExactStatic,
            TopologyStatus::Present,
        );
    }
}

fn emit_package_manifest_unsupported_overlays(
    output: &mut TopologyOutput,
    package_path: &str,
    manifest: &PackageJsonManifest,
) {
    for unsupported in &manifest.unsupported {
        let reason = unsupported.reason.replace([':', ' '], "-");
        push_overlay(
            output,
            package_path,
            format!("package-json-unsupported:{reason}"),
            Some(unsupported.source_path.clone()),
            unsupported.precision,
            unsupported.status,
        );
    }
}

fn emit_lockfile_overlays(output: &mut TopologyOutput, loaded: &LoadedConfig, package_path: &str) {
    for lockfile in detect_js_lockfiles(loaded, package_path) {
        let relative_path = lockfile_relative_path(package_path, lockfile.kind);
        let manifest = fs::read_to_string(loaded.root.join(&relative_path))
            .map(|contents| parse_js_lockfile(lockfile.kind, &relative_path, &contents));
        let schema = manifest
            .as_ref()
            .map(|manifest| manifest.schema_label.clone())
            .unwrap_or_else(|_| format!("{}-unknown", lockfile.kind.manager().label()));
        let (precision, status) = manifest
            .as_ref()
            .map(|manifest| {
                if manifest.unsupported.is_empty() {
                    (TopologyPrecision::ExactStatic, TopologyStatus::Present)
                } else {
                    (TopologyPrecision::Unsupported, TopologyStatus::Unsupported)
                }
            })
            .unwrap_or((TopologyPrecision::Unknown, TopologyStatus::SetupMissing));
        push_overlay(
            output,
            package_path,
            format!("lockfile:{}:{schema}", lockfile.kind.file_name()),
            Some(relative_path),
            precision,
            status,
        );
    }
}

fn emit_lockfile_selection_overlay(
    output: &mut TopologyOutput,
    package_path: &str,
    selection: &JsLockfileSelection,
) {
    let manager = selection
        .manager
        .map(JsPackageManager::label)
        .unwrap_or("unknown");
    let source = selection
        .lockfile
        .map(|lockfile| lockfile.kind.file_name())
        .unwrap_or_else(|| default_lockfile_name_for_manager(selection.manager));
    let status = match selection.status {
        JsLockfileSelectionStatus::Selected => "selected",
        JsLockfileSelectionStatus::MissingLockfile => "missing",
        JsLockfileSelectionStatus::Ambiguous => "ambiguous",
        JsLockfileSelectionStatus::Unsupported => "unsupported",
    };
    let precision = match selection.status {
        JsLockfileSelectionStatus::Selected | JsLockfileSelectionStatus::MissingLockfile => {
            TopologyPrecision::ExactStatic
        }
        JsLockfileSelectionStatus::Ambiguous => TopologyPrecision::Unknown,
        JsLockfileSelectionStatus::Unsupported => TopologyPrecision::Unsupported,
    };
    let topology_status = match selection.status {
        JsLockfileSelectionStatus::Selected => TopologyStatus::Present,
        JsLockfileSelectionStatus::MissingLockfile => TopologyStatus::MissingLockfile,
        JsLockfileSelectionStatus::Ambiguous => TopologyStatus::Ambiguous,
        JsLockfileSelectionStatus::Unsupported => TopologyStatus::Unsupported,
    };
    push_overlay(
        output,
        package_path,
        format!("package-manager:{status}:{manager}:lockfile:{source}"),
        selection
            .lockfile
            .map(|lockfile| lockfile_relative_path(&selection.root_path, lockfile.kind)),
        precision,
        topology_status,
    );
}

fn emit_package_requirements(
    output: &mut TopologyOutput,
    package_id: TopologyPackageId,
    package_path: &str,
    manifest: &PackageJsonManifest,
) {
    for dependency in &manifest.dependencies {
        let kind = if dependency
            .version_requirement
            .as_deref()
            .is_some_and(|requirement| requirement.starts_with("workspace:"))
        {
            RequirementKind::Workspace
        } else {
            dependency.kind
        };
        output
            .dependency_requirements
            .push(DependencyRequirementFact {
                id: DependencyRequirementId(output.dependency_requirements.len() as u64),
                from_package: Some(package_id),
                target_package: None,
                target_name: dependency.target_name.clone(),
                version_requirement: dependency.version_requirement.clone(),
                kind,
                manifest_path: Some(package_manifest_path(package_path)),
                stable_key: format!(
                    "js-require:{package_path}:{}:{}:{}",
                    dependency.section,
                    dependency.target_name,
                    dependency.version_requirement.as_deref().unwrap_or("")
                ),
                producer_id: TS_TOPOLOGY_PROVIDER_ID,
                precision: dependency.precision,
                status: dependency.status,
            });
    }
}

fn emit_js_lockfile_edges(
    output: &mut TopologyOutput,
    loaded: &LoadedConfig,
    package_path: &str,
    package_id: TopologyPackageId,
    selection: &JsLockfileSelection,
) {
    match selection.status {
        JsLockfileSelectionStatus::Selected => {
            let Some(lockfile) = selection.lockfile else {
                return;
            };
            let relative_path = lockfile_relative_path(&selection.root_path, lockfile.kind);
            let Ok(contents) = fs::read_to_string(loaded.root.join(&relative_path)) else {
                emit_lockfile_problem_edge(
                    output,
                    package_path,
                    package_id,
                    lockfile.kind.file_name(),
                    "unreadable",
                    TopologyPrecision::Unknown,
                    TopologyStatus::SetupMissing,
                );
                return;
            };
            let manifest = parse_js_lockfile(lockfile.kind, &relative_path, &contents);
            let unsupported_count =
                emit_lockfile_unsupported_edges(output, package_path, package_id, &manifest);
            let selected_count = emit_selected_lockfile_package_edges(
                output,
                package_path,
                package_id,
                selection,
                &manifest,
            );
            if unsupported_count == 0
                && selected_count == 0
                && package_has_lockfile_requirements(output, package_id)
            {
                emit_lockfile_problem_edge(
                    output,
                    package_path,
                    package_id,
                    lockfile.kind.file_name(),
                    "no parseable selected lockfile entries",
                    TopologyPrecision::Unsupported,
                    TopologyStatus::Unsupported,
                );
            }
        }
        JsLockfileSelectionStatus::MissingLockfile => emit_missing_lockfile_edges(
            output,
            package_path,
            package_id,
            default_lockfile_name_for_manager(selection.manager),
        ),
        JsLockfileSelectionStatus::Ambiguous => emit_lockfile_problem_edge(
            output,
            package_path,
            package_id,
            "js-lockfile",
            selection.reason.as_deref().unwrap_or("ambiguous lockfiles"),
            TopologyPrecision::Unknown,
            TopologyStatus::Ambiguous,
        ),
        JsLockfileSelectionStatus::Unsupported => emit_lockfile_problem_edge(
            output,
            package_path,
            package_id,
            "packageManager",
            selection
                .reason
                .as_deref()
                .unwrap_or("unsupported package manager"),
            TopologyPrecision::Unsupported,
            TopologyStatus::Unsupported,
        ),
    }
}

fn emit_lockfile_unsupported_edges(
    output: &mut TopologyOutput,
    package_path: &str,
    package_id: TopologyPackageId,
    manifest: &JsLockfileManifest,
) -> usize {
    let mut count = 0;
    for unsupported in &manifest.unsupported {
        let reason = stable_label_fragment(&unsupported.reason);
        output
            .resolved_dependency_edges
            .push(ResolvedDependencyEdgeFact {
                id: ResolvedDependencyEdgeId(output.resolved_dependency_edges.len() as u64),
                requirement: None,
                from_package: Some(package_id),
                to_package: None,
                package_name: String::new(),
                resolved_version: None,
                kind: ResolvedDependencyKind::LockfileSelected,
                stable_key: format!(
                    "js-lock-unsupported:{package_path}:{}:source={}:schema={}:reason={reason}",
                    unsupported.source_path, unsupported.source_label, manifest.schema_label
                ),
                producer_id: TS_TOPOLOGY_PROVIDER_ID,
                precision: unsupported.precision,
                status: unsupported.status,
            });
        count += 1;
    }
    count
}

fn emit_selected_lockfile_package_edges(
    output: &mut TopologyOutput,
    package_path: &str,
    package_id: TopologyPackageId,
    selection: &JsLockfileSelection,
    manifest: &JsLockfileManifest,
) -> usize {
    let mut stable_keys = BTreeSet::new();
    let mut count = 0;
    for package in &manifest.packages {
        if !lockfile_package_applies_to_package(
            output,
            package_id,
            selection,
            package_path,
            package,
        ) {
            continue;
        }
        let stable_key = format!(
            "js-lock-selected:{package_path}:{}:{}:{}:source={}:schema={}",
            package.path,
            package.name,
            package.version,
            package.source_label,
            manifest.schema_label
        );
        if !stable_keys.insert(stable_key.clone()) {
            continue;
        }
        output
            .resolved_dependency_edges
            .push(ResolvedDependencyEdgeFact {
                id: ResolvedDependencyEdgeId(output.resolved_dependency_edges.len() as u64),
                requirement: requirement_id_for(output, package_id, &package.name),
                from_package: Some(package_id),
                to_package: None,
                package_name: package.name.clone(),
                resolved_version: Some(package.version.clone()),
                kind: ResolvedDependencyKind::LockfileSelected,
                stable_key,
                producer_id: TS_TOPOLOGY_PROVIDER_ID,
                precision: package.precision,
                status: package.status,
            });
        count += 1;
    }
    count
}

fn package_has_lockfile_requirements(
    output: &TopologyOutput,
    package_id: TopologyPackageId,
) -> bool {
    output.dependency_requirements.iter().any(|requirement| {
        requirement.from_package == Some(package_id)
            && requirement.kind != RequirementKind::Workspace
            && requirement.status == TopologyStatus::Present
    })
}

fn lockfile_package_applies_to_package(
    output: &TopologyOutput,
    package_id: TopologyPackageId,
    selection: &JsLockfileSelection,
    package_path: &str,
    package: &JsLockfilePackage,
) -> bool {
    let importer_path = importer_path_for_package(&selection.root_path, package_path);
    if let Some(package_importer_path) = package.importer_path.as_deref() {
        return package_importer_path == importer_path;
    }
    if selection.root_path == package_path {
        return true;
    }
    package_lock_path_matches_importer(&package.path, &importer_path, &package.name)
        && requirement_id_for(output, package_id, &package.name).is_some()
}

fn package_lock_path_matches_importer(
    package_path: &str,
    importer_path: &str,
    package_name: &str,
) -> bool {
    let package_entry = if importer_path == "." {
        package_path.strip_prefix("node_modules/")
    } else {
        let prefix = format!("{importer_path}/node_modules/");
        package_path.strip_prefix(&prefix)
    };
    package_entry.is_some_and(|entry| entry == package_name)
}

fn emit_lockfile_problem_edge(
    output: &mut TopologyOutput,
    package_path: &str,
    package_id: TopologyPackageId,
    source_label: &str,
    reason: &str,
    precision: TopologyPrecision,
    status: TopologyStatus,
) {
    let reason = stable_label_fragment(reason);
    output
        .resolved_dependency_edges
        .push(ResolvedDependencyEdgeFact {
            id: ResolvedDependencyEdgeId(output.resolved_dependency_edges.len() as u64),
            requirement: None,
            from_package: Some(package_id),
            to_package: None,
            package_name: String::new(),
            resolved_version: None,
            kind: ResolvedDependencyKind::LockfileSelected,
            stable_key: format!(
                "js-lock-problem:{package_path}:source={source_label}:reason={reason}"
            ),
            producer_id: TS_TOPOLOGY_PROVIDER_ID,
            precision,
            status,
        });
}

fn emit_missing_lockfile_edges(
    output: &mut TopologyOutput,
    package_path: &str,
    package_id: TopologyPackageId,
    source_label: &str,
) {
    let requirements = output
        .dependency_requirements
        .iter()
        .filter(|requirement| {
            requirement.from_package == Some(package_id)
                && requirement.kind != RequirementKind::Workspace
                && requirement.status == TopologyStatus::Present
        })
        .map(|requirement| {
            (
                requirement.id,
                requirement.target_name.clone(),
                requirement.version_requirement.clone(),
            )
        })
        .collect::<Vec<_>>();
    for (requirement_id, target_name, version_requirement) in requirements {
        output
            .resolved_dependency_edges
            .push(ResolvedDependencyEdgeFact {
                id: ResolvedDependencyEdgeId(output.resolved_dependency_edges.len() as u64),
                requirement: Some(requirement_id),
                from_package: Some(package_id),
                to_package: None,
                package_name: target_name.clone(),
                resolved_version: version_requirement.clone(),
                kind: ResolvedDependencyKind::LockfileSelected,
                stable_key: format!(
                    "js-lock-missing:{package_path}:{target_name}:{}:source={source_label}",
                    version_requirement.as_deref().unwrap_or("")
                ),
                producer_id: TS_TOPOLOGY_PROVIDER_ID,
                precision: TopologyPrecision::Unknown,
                status: TopologyStatus::MissingLockfile,
            });
    }
}

fn requirement_id_for(
    output: &TopologyOutput,
    package_id: TopologyPackageId,
    target_name: &str,
) -> Option<DependencyRequirementId> {
    output
        .dependency_requirements
        .iter()
        .find(|requirement| {
            requirement.from_package == Some(package_id) && requirement.target_name == target_name
        })
        .map(|requirement| requirement.id)
}

fn package_relative_path(package_path: &str, file_name: &str) -> String {
    if package_path == "." {
        file_name.to_string()
    } else {
        format!("{package_path}/{file_name}")
    }
}

fn emit_pnpm_workspace_overlays(output: &mut TopologyOutput, loaded: &LoadedConfig) {
    let relative_path = "pnpm-workspace.yaml";
    for workspace in root_pnpm_workspace_patterns(loaded) {
        push_overlay(
            output,
            ".",
            format!("pnpm-workspace.yaml:{workspace}"),
            Some(relative_path.to_string()),
            TopologyPrecision::Heuristic,
            TopologyStatus::Present,
        );
    }
}

fn root_pnpm_workspace_patterns(loaded: &LoadedConfig) -> Vec<String> {
    let relative_path = "pnpm-workspace.yaml";
    fs::read_to_string(loaded.root.join(relative_path))
        .map(|contents| parse_pnpm_workspace_packages(&contents))
        .unwrap_or_default()
}

fn parse_pnpm_workspace_packages(contents: &str) -> Vec<String> {
    let mut in_packages = false;
    let mut packages = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "packages:" {
            in_packages = true;
            continue;
        }
        if !in_packages {
            continue;
        }
        let Some(entry) = trimmed.strip_prefix('-') else {
            if !trimmed.is_empty() && !line.starts_with(' ') {
                break;
            }
            continue;
        };
        packages.push(entry.trim().trim_matches(['"', '\'']).to_string());
    }
    packages.sort();
    packages.dedup();
    packages
}

fn emit_tsconfig_overlays(
    output: &mut TopologyOutput,
    loaded: &LoadedConfig,
    ts_files: &[&SourceFile],
) {
    let mut configs = BTreeSet::new();
    for file in ts_files {
        let absolute = if file.path.is_absolute() {
            file.path.clone()
        } else {
            loaded.root.join(&file.relative_path)
        };
        if let Some(config) = nearest_tsconfig_path(&loaded.root, &absolute)
            && let Some(relative) =
                crate::module_graph::paths::normalize_repo_relative_path(&loaded.root, &config)
        {
            configs.insert(relative);
        }
    }
    for config in configs {
        let Some(value) = read_json_with_comments(&loaded.root.join(&config)) else {
            continue;
        };
        let Some(object) = value.as_object() else {
            continue;
        };
        if let Some(options) = object
            .get("compilerOptions")
            .and_then(serde_json::Value::as_object)
        {
            if let Some(base_url) = options.get("baseUrl").and_then(serde_json::Value::as_str) {
                push_overlay(
                    output,
                    ".",
                    format!("tsconfig:baseUrl:{base_url}"),
                    Some(config.clone()),
                    TopologyPrecision::ExactStatic,
                    TopologyStatus::Present,
                );
            }
            if let Some(paths) = options.get("paths").and_then(serde_json::Value::as_object) {
                for pattern in paths.keys() {
                    push_overlay(
                        output,
                        ".",
                        format!("tsconfig:paths:{pattern}"),
                        Some(config.clone()),
                        TopologyPrecision::ExactStatic,
                        TopologyStatus::Present,
                    );
                }
            }
            if let Some(root_dirs) = options
                .get("rootDirs")
                .and_then(serde_json::Value::as_array)
            {
                for root_dir in root_dirs.iter().filter_map(serde_json::Value::as_str) {
                    push_overlay(
                        output,
                        ".",
                        format!("tsconfig:rootDirs:{root_dir}"),
                        Some(config.clone()),
                        TopologyPrecision::ExactStatic,
                        TopologyStatus::Present,
                    );
                }
            }
        }
        if let Some(references) = object
            .get("references")
            .and_then(serde_json::Value::as_array)
        {
            for reference in references
                .iter()
                .filter_map(serde_json::Value::as_object)
                .filter_map(|reference| reference.get("path").and_then(serde_json::Value::as_str))
            {
                push_overlay(
                    output,
                    ".",
                    format!("tsconfig:reference:{reference}"),
                    Some(config.clone()),
                    TopologyPrecision::ExactStatic,
                    TopologyStatus::Present,
                );
            }
        }
    }
}

fn read_json_with_comments(path: &Path) -> Option<serde_json::Value> {
    let mut source = fs::read_to_string(path).ok()?;
    if let Some(stripped) = source.strip_prefix('\u{feff}') {
        source = stripped.to_string();
    }
    json_strip_comments::strip(&mut source).ok()?;
    serde_json::from_str(&source).ok()
}

fn classify_ts_source_set(file: &SourceFile) -> SourceSetKind {
    let path = file.relative_path.replace('\\', "/");
    if path.contains("/node_modules/") || path.starts_with("node_modules/") {
        return SourceSetKind::Vendor;
    }
    if path.contains("/generated/")
        || path.starts_with("generated/")
        || path.contains("/gen/")
        || path.starts_with("gen/")
        || path.contains(".generated.")
    {
        return SourceSetKind::Generated;
    }
    if path.contains("/__tests__/")
        || path.starts_with("__tests__/")
        || path.contains("/test/")
        || path.starts_with("test/")
        || path.contains("/tests/")
        || path.starts_with("tests/")
        || path.contains(".test.")
        || path.contains(".spec.")
    {
        return SourceSetKind::Test;
    }
    SourceSetKind::Source
}

fn source_set_kind_label(kind: SourceSetKind) -> &'static str {
    match kind {
        SourceSetKind::Source => "source",
        SourceSetKind::Test => "test",
        SourceSetKind::Generated => "generated",
        SourceSetKind::Vendor => "vendor",
        SourceSetKind::External => "external",
        SourceSetKind::Unknown => "unknown",
    }
}

fn push_overlay(
    output: &mut TopologyOutput,
    package_path: &str,
    label: String,
    path: Option<String>,
    precision: TopologyPrecision,
    status: TopologyStatus,
) {
    output.overlays.push(RepoTopologyOverlayFact {
        id: RepoTopologyOverlayId(output.overlays.len() as u64),
        root: None,
        package: None,
        source_set: None,
        kind: RepoTopologyOverlayKind::SourceOfTruthDirectory,
        stable_key: format!(
            "ts-overlay:{package_path}:{label}:{}",
            path.as_deref().unwrap_or("")
        ),
        label,
        path,
        producer_id: TS_TOPOLOGY_PROVIDER_ID,
        precision,
        status,
    });
}

fn is_external_package_specifier(specifier: &str) -> bool {
    !specifier.starts_with('.')
        && !specifier.starts_with('/')
        && !specifier.starts_with('#')
        && !specifier.starts_with("@/")
}

fn tsconfig_path_alias_matches(
    context: &TsResolverContext,
    importer_path: &Path,
    specifier: &str,
) -> bool {
    let Some(mut current) = importer_path.parent().and_then(normalize_path) else {
        return false;
    };
    loop {
        if let Some(patterns) = context.path_aliases_by_config_dir.get(&current) {
            return patterns
                .iter()
                .any(|pattern| ts_path_pattern_matches(pattern, specifier));
        }
        if current == context.root || !current.starts_with(&context.root) || !current.pop() {
            return false;
        }
    }
}

fn ts_path_pattern_matches(pattern: &str, specifier: &str) -> bool {
    let Some((prefix, suffix)) = pattern.split_once('*') else {
        return pattern == specifier;
    };
    specifier.starts_with(prefix) && specifier.ends_with(suffix)
}

fn collect_ts_path_aliases(root: &Path, db: &AnalysisDb) -> BTreeMap<PathBuf, Vec<String>> {
    let mut aliases = BTreeMap::new();
    for file in db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
    {
        let absolute = if file.path.is_absolute() {
            file.path.clone()
        } else {
            root.join(&file.relative_path)
        };
        let Some(config_path) = nearest_tsconfig_path(root, &absolute) else {
            continue;
        };
        let Some(config_dir) = config_path.parent().and_then(normalize_path) else {
            continue;
        };
        aliases
            .entry(config_dir)
            .or_insert_with(|| read_tsconfig_path_aliases(&config_path));
    }
    aliases
}

fn nearest_tsconfig_path(root: &Path, file_path: &Path) -> Option<PathBuf> {
    let root = normalize_path(root)?;
    let mut current = normalize_path(file_path.parent()?)?;
    loop {
        let candidate = current.join("tsconfig.json");
        if candidate.is_file() {
            return Some(candidate);
        }
        if current == root || !current.starts_with(&root) || !current.pop() {
            return None;
        }
    }
}

fn read_tsconfig_path_aliases(path: &Path) -> Vec<String> {
    let mut visited = BTreeSet::new();
    read_tsconfig_path_aliases_inner(path, &mut visited)
}

fn read_tsconfig_path_aliases_inner(path: &Path, visited: &mut BTreeSet<PathBuf>) -> Vec<String> {
    let Some(path) = normalize_path(path) else {
        return Vec::new();
    };
    if !visited.insert(path.clone()) {
        return Vec::new();
    }
    let Some(config) = read_tsconfig_alias_wire(&path) else {
        return Vec::new();
    };

    if let Some(paths) = config
        .compiler_options
        .as_ref()
        .and_then(|options| options.paths.as_ref())
    {
        return sorted_ts_path_aliases(paths.keys().cloned());
    }

    let Some(config_dir) = path.parent() else {
        return Vec::new();
    };
    let mut aliases = config
        .extends
        .into_iter()
        .flat_map(TsconfigExtendsWire::into_specifiers)
        .filter_map(|specifier| resolve_tsconfig_extends_path(config_dir, &specifier))
        .flat_map(|extended_path| read_tsconfig_path_aliases_inner(&extended_path, visited))
        .collect::<Vec<_>>();
    aliases.sort();
    aliases.dedup();
    aliases
}

fn read_tsconfig_alias_wire(path: &Path) -> Option<TsconfigAliasWire> {
    let Ok(mut source) = fs::read_to_string(path) else {
        return None;
    };
    if let Some(stripped) = source.strip_prefix('\u{feff}') {
        source = stripped.to_string();
    }
    if json_strip_comments::strip(&mut source).is_err() {
        return None;
    }
    serde_json::from_str::<TsconfigAliasWire>(&source).ok()
}

fn sorted_ts_path_aliases(paths: impl Iterator<Item = String>) -> Vec<String> {
    let mut aliases = paths.collect::<Vec<_>>();
    aliases.sort();
    aliases.dedup();
    aliases
}

fn resolve_tsconfig_extends_path(config_dir: &Path, specifier: &str) -> Option<PathBuf> {
    let specifier_path = Path::new(specifier);
    if specifier_path.is_absolute() {
        return resolve_tsconfig_file_candidate(specifier_path);
    }
    if specifier.starts_with('.') {
        return resolve_tsconfig_file_candidate(&config_dir.join(specifier_path));
    }
    resolve_package_tsconfig_extends_path(config_dir, specifier)
}

fn resolve_package_tsconfig_extends_path(config_dir: &Path, specifier: &str) -> Option<PathBuf> {
    let mut current = normalize_path(config_dir)?;
    loop {
        let candidate = current.join("node_modules").join(specifier);
        if let Some(resolved) = resolve_tsconfig_file_candidate(&candidate) {
            return Some(resolved);
        }
        if !current.pop() {
            return None;
        }
    }
}

fn resolve_tsconfig_file_candidate(base: &Path) -> Option<PathBuf> {
    let mut candidates = vec![base.to_path_buf()];
    if base.extension().and_then(|extension| extension.to_str()) != Some("json") {
        let mut with_json = base.as_os_str().to_owned();
        with_json.push(".json");
        candidates.push(PathBuf::from(with_json));
    }
    candidates.push(base.join("tsconfig.json"));

    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .and_then(|candidate| normalize_path(&candidate))
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum TsconfigExtendsWire {
    Single(String),
    Multiple(Vec<String>),
}

impl TsconfigExtendsWire {
    fn into_specifiers(self) -> Vec<String> {
        match self {
            Self::Single(specifier) => vec![specifier],
            Self::Multiple(specifiers) => specifiers,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TsconfigAliasWire {
    #[serde(default)]
    extends: Option<TsconfigExtendsWire>,
    compiler_options: Option<TsconfigCompilerOptionsWire>,
}

#[derive(Debug, Deserialize)]
struct TsconfigCompilerOptionsWire {
    paths: Option<BTreeMap<String, Vec<String>>>,
}

fn resolve_options() -> ResolveOptions {
    ResolveOptions {
        tsconfig: Some(TsconfigDiscovery::Auto),
        extensions: vec![
            ".ts".into(),
            ".tsx".into(),
            ".js".into(),
            ".jsx".into(),
            ".json".into(),
            ".node".into(),
        ],
        extension_alias: vec![
            (
                ".js".into(),
                vec![".js".into(), ".ts".into(), ".tsx".into()],
            ),
            (".jsx".into(), vec![".jsx".into(), ".tsx".into()]),
            (".mjs".into(), vec![".mjs".into(), ".mts".into()]),
            (".cjs".into(), vec![".cjs".into(), ".cts".into()]),
        ],
        condition_names: vec![
            "import".into(),
            "require".into(),
            "node".into(),
            "default".into(),
        ],
        main_fields: vec!["module".into(), "browser".into(), "main".into()],
        exports_fields: vec![vec!["exports".into()]],
        imports_fields: vec![vec!["imports".into()]],
        builtin_modules: true,
        symlinks: false,
        ..ResolveOptions::default()
    }
}

#[cfg(test)]
pub(crate) fn reset_resolver_context_construction_count_for_test() {
    RESOLVER_CONTEXT_CONSTRUCTIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(crate) fn resolver_context_construction_count_for_test() -> usize {
    RESOLVER_CONTEXT_CONSTRUCTIONS.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use super::{TsResolverContext, resolve_ts_import};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, ImportFact, ImportId, Language, ModuleEdgeKind, ModuleNodeId, ModuleNodeKind,
        ResolutionPrecision, ResolutionStatus, Span, UnresolvedReason,
    };
    use crate::module_graph::derive_requested_module_graph;
    use crate::module_graph::model::ResolverInput;
    use crate::ts::DYNAMIC_IMPORT_SPECIFIER;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn module_graph_resolver_contracts_ts_without_context_is_setup_missing() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import missing from './missing';\n".to_string(),
        );
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "./missing".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
        });
        let import = &db.imports()[0];

        let draft = resolve_ts_import(ResolverInput {
            root: Path::new("."),
            db: &db,
            import,
            ts_resolver: None,
            owner_module: None,
            owner_package: None,
        });

        assert_eq!(draft.status, ResolutionStatus::SetupMissing);
        assert_eq!(draft.reason, Some(UnresolvedReason::SetupMissing));
    }

    #[test]
    fn module_graph_ts_dynamic_resolution_marks_sentinel_as_dynamic() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("src/app.ts");
        fs::create_dir_all(path.parent().expect("test file has parent")).expect("mkdirs");
        fs::write(&path, "const mod = await import(name);\n").expect("write fixture file");
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            path,
            "src/app.ts".to_string(),
            "const mod = await import(name);\n".to_string(),
        );
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: DYNAMIC_IMPORT_SPECIFIER.to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
        });
        let import = &db.imports()[0];
        let context = TsResolverContext::new(temp.path(), &db, None);

        let draft = resolve_ts_import(ResolverInput {
            root: temp.path(),
            db: &db,
            import,
            ts_resolver: Some(&context),
            owner_module: None,
            owner_package: None,
        });

        assert_eq!(draft.status, ResolutionStatus::Dynamic);
        assert_eq!(draft.precision, ResolutionPrecision::None);
        assert_eq!(draft.reason, Some(UnresolvedReason::DynamicExpression));
        assert_eq!(draft.target, None);
    }

    type DeterminismSnapshot = (
        Vec<(ModuleNodeKind, String)>,
        Vec<(
            ResolutionStatus,
            ResolutionPrecision,
            Option<UnresolvedReason>,
            Option<String>,
        )>,
        Vec<(String, String, ModuleEdgeKind, ResolutionStatus)>,
    );

    fn write_fixture(root: &Path, relative_path: &str, source: &str) -> PathBuf {
        let path = root.join(relative_path);
        fs::create_dir_all(path.parent().expect("test fixture path has parent")).expect("mkdirs");
        fs::write(&path, source).expect("write fixture");
        path
    }

    fn add_fixture_file(
        db: &mut AnalysisDb,
        root: &Path,
        relative_path: &str,
        source: &str,
    ) -> crate::core::FileId {
        let path = write_fixture(root, relative_path, source);
        db.add_file(path, relative_path.to_string(), source.to_string())
    }

    fn push_import(db: &mut AnalysisDb, file: crate::core::FileId, path: &str, offset: u32) {
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: path.to_string(),
            span: Span {
                file,
                start_byte: offset,
                end_byte: offset + 1,
                start_line: 1,
                start_col: offset + 1,
                end_line: 1,
                end_col: offset + 2,
            },
            language: Language::TypeScript,
        });
    }

    fn build_determinism_db(root: &Path) -> AnalysisDb {
        write_fixture(
            root,
            "package.json",
            r#"{"name":"frontend","dependencies":{"react":"latest"}}"#,
        );
        write_fixture(
            root,
            "tsconfig.json",
            r#"{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}}}"#,
        );
        let mut db = AnalysisDb::new();
        let app = add_fixture_file(
            &mut db,
            root,
            "src/app.ts",
            r#"
import tokens from "@/tokens";
import React from "react";
const lazy = await import("./lazy");
const dynamic = await import(name);
"#,
        );
        add_fixture_file(
            &mut db,
            root,
            "src/tokens.ts",
            "export const tokens = {};\n",
        );
        add_fixture_file(&mut db, root, "src/lazy.ts", "export const lazy = true;\n");
        push_import(&mut db, app, "@/tokens", 0);
        push_import(&mut db, app, "react", 30);
        push_import(&mut db, app, "./lazy", 60);
        push_import(&mut db, app, DYNAMIC_IMPORT_SPECIFIER, 90);
        db
    }

    fn node_label(db: &AnalysisDb, id: ModuleNodeId) -> String {
        db.module_nodes()
            .iter()
            .find(|node| node.id == id)
            .map(|node| node.label.clone())
            .expect("node exists")
    }

    fn derive_snapshot(root: &Path) -> DeterminismSnapshot {
        let mut db = build_determinism_db(root);
        let config = load_config(root).expect("test config loads");
        derive_requested_module_graph(
            &mut db,
            &config,
            &AnalysisPlan::from_capability_names_for_test(&["resolved_imports", "module_graph"]),
        );

        let nodes = db
            .module_nodes()
            .iter()
            .map(|node| (node.kind, node.label.clone()))
            .collect::<Vec<_>>();
        let imports = db
            .resolved_imports()
            .iter()
            .map(|fact| {
                (
                    fact.status,
                    fact.precision,
                    fact.reason,
                    fact.target_node.map(|node| node_label(&db, node)),
                )
            })
            .collect::<Vec<_>>();
        let edges = db
            .module_edges()
            .iter()
            .map(|edge| {
                (
                    node_label(&db, edge.from),
                    node_label(&db, edge.to),
                    edge.kind,
                    edge.status,
                )
            })
            .collect::<Vec<_>>();

        (nodes, imports, edges)
    }

    #[test]
    fn module_graph_ts_determinism_repeated_provider_runs_match_exact_graph_rows() {
        let temp = tempfile::tempdir().expect("tempdir");

        let first = derive_snapshot(temp.path());
        let second = derive_snapshot(temp.path());

        assert_eq!(first, second);
        assert!(
            first
                .0
                .iter()
                .any(|(kind, label)| { *kind == ModuleNodeKind::Module && label == "frontend" })
        );
        assert!(first.2.iter().any(|(from, to, kind, status)| {
            from == "frontend"
                && to == "react"
                && *kind == ModuleEdgeKind::DependsOn
                && *status == ResolutionStatus::External
        }));
    }
}
