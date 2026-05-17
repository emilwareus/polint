use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, bail, ensure};
use serde::Deserialize;

use crate::eval::model::ExpectedItem;

const FIXTURE_SCHEMA_VERSION: &str = "polint-eval-fixture-1";
const FIXTURE_MANIFEST_FILE: &str = "expected.polint-eval.toml";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct NativeFixtureManifest {
    pub(crate) schema_version: String,
    pub(crate) case_id: String,
    pub(crate) area: crate::eval::model::FixtureArea,
    pub(crate) repo: FixtureRepo,
    #[serde(default)]
    pub(crate) expected: Vec<ExpectedItem>,
    pub(crate) budget: Option<FixtureBudget>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct FixtureRepo {
    pub(crate) path: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) struct FixtureBudget {
    pub(crate) max_runtime_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativeFixture {
    pub(crate) fixture_dir: PathBuf,
    pub(crate) repo_dir: PathBuf,
    pub(crate) manifest: NativeFixtureManifest,
}

pub(crate) fn load_native_fixture(fixture_dir: &Path) -> anyhow::Result<NativeFixture> {
    let fixture_dir = fixture_dir
        .canonicalize()
        .with_context(|| format!("canonicalize fixture dir {}", fixture_dir.display()))?;
    let manifest_path = fixture_dir.join(FIXTURE_MANIFEST_FILE);
    let manifest_toml = fs::read_to_string(&manifest_path)
        .with_context(|| format!("read fixture manifest {}", manifest_path.display()))?;
    let mut manifest: NativeFixtureManifest = toml::from_str(&manifest_toml)
        .with_context(|| format!("parse fixture manifest {}", manifest_path.display()))?;

    ensure!(
        manifest.schema_version == FIXTURE_SCHEMA_VERSION,
        "unsupported fixture schema_version `{}`; expected `{FIXTURE_SCHEMA_VERSION}`",
        manifest.schema_version
    );
    validate_manifest_relative_path("repo.path", &manifest.repo.path)?;
    normalize_expected_item_paths(&mut manifest.expected)?;

    let repo_dir = fixture_dir.join(Path::new(&manifest.repo.path));
    let repo_dir = repo_dir
        .canonicalize()
        .with_context(|| format!("canonicalize fixture repo {}", repo_dir.display()))?;
    ensure!(
        repo_dir.starts_with(&fixture_dir),
        "fixture repo path must stay inside fixture directory"
    );

    Ok(NativeFixture {
        fixture_dir,
        repo_dir,
        manifest,
    })
}

fn normalize_expected_item_paths(expected: &mut [ExpectedItem]) -> anyhow::Result<()> {
    for item in expected {
        if let ExpectedItem::Diagnostic(diagnostic) = item {
            diagnostic.relative_path = normalize_manifest_relative_path(
                "expected.diagnostic.relative_path",
                &diagnostic.relative_path,
            )?;
        }
    }
    Ok(())
}

fn normalize_manifest_relative_path(field: &str, value: &str) -> anyhow::Result<String> {
    validate_manifest_relative_path(field, value)?;
    Ok(value.replace('\\', "/"))
}

fn validate_manifest_relative_path(field: &str, value: &str) -> anyhow::Result<()> {
    if is_absolute_manifest_path(value) {
        bail!("{field} must be relative, not absolute");
    }
    if has_parent_dir_component(value) {
        bail!("{field} must not contain a parent directory component");
    }
    Ok(())
}

fn is_absolute_manifest_path(value: &str) -> bool {
    Path::new(value).is_absolute()
        || value.starts_with('\\')
        || value.as_bytes().get(1) == Some(&b':')
}

fn has_parent_dir_component(value: &str) -> bool {
    Path::new(value)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
        || value.split(['/', '\\']).any(|component| component == "..")
}

#[cfg(test)]
mod eval_fixture_manifest_tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::eval::model::{ExpectedItem, FixtureArea};

    fn write_fixture(root: &Path, manifest: &str) {
        fs::create_dir_all(root.join("repo/src")).unwrap();
        fs::write(root.join("repo/src/app.ts"), "export const answer = 42;\n").unwrap();
        fs::write(root.join("expected.polint-eval.toml"), manifest).unwrap();
    }

    #[test]
    fn eval_fixture_manifest_loads_native_fixture_from_expected_toml() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "repo"

[[expected]]
invariant = { name = "provider_order.0", value = "polint.source", mode = "exact" }
"#,
        );

        let fixture = load_native_fixture(&fixture_dir).unwrap();

        assert_eq!(fixture.manifest.schema_version, "polint-eval-fixture-1");
        assert_eq!(fixture.manifest.area, FixtureArea::Kernel);
        assert_eq!(fixture.manifest.case_id, "provider-order");
        assert_eq!(fixture.manifest.repo.path, "repo");
        assert_eq!(fixture.manifest.expected.len(), 1);
        assert!(matches!(
            fixture.manifest.expected.first(),
            Some(ExpectedItem::Invariant(_))
        ));
        assert_eq!(fixture.repo_dir, fixture.fixture_dir.join("repo"));
    }

    #[test]
    fn eval_fixture_manifest_rejects_absolute_repo_path() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "/tmp/polint-outside"
"#,
        );

        let err = load_native_fixture(&fixture_dir).unwrap_err();

        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn eval_fixture_manifest_rejects_parent_repo_path() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        fs::create_dir_all(&fixture_dir).unwrap();
        fs::write(
            fixture_dir.join("expected.polint-eval.toml"),
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "../outside"
"#,
        )
        .unwrap();

        let err = load_native_fixture(&fixture_dir).unwrap_err();

        assert!(err.to_string().contains("parent directory"));
    }

    #[test]
    fn eval_fixture_manifest_rejects_parent_expected_relative_path() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "repo"

[[expected]]
diagnostic = { rule_id = "local/rule", relative_path = "../outside.ts", line = 1, mode = "exact" }
"#,
        );

        let err = load_native_fixture(&fixture_dir).unwrap_err();

        assert!(err.to_string().contains("parent directory"));
    }

    #[test]
    fn eval_fixture_manifest_rejects_absolute_expected_relative_path() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "repo"

[[expected]]
diagnostic = { rule_id = "local/rule", relative_path = "/tmp/app.ts", line = 1, mode = "exact" }
"#,
        );

        let err = load_native_fixture(&fixture_dir).unwrap_err();

        assert!(err.to_string().contains("absolute"));
    }

    #[test]
    fn eval_fixture_manifest_normalizes_expected_relative_path_separators() {
        let temp = tempfile::tempdir().unwrap();
        let fixture_dir = temp
            .path()
            .join("tests/eval-fixtures/kernel/provider-order");
        write_fixture(
            &fixture_dir,
            r#"
schema_version = "polint-eval-fixture-1"
case_id = "provider-order"
area = "kernel"

[repo]
path = "repo"

[[expected]]
diagnostic = { rule_id = "local/rule", relative_path = "src\\app.ts", line = 1, mode = "exact" }
"#,
        );

        let fixture = load_native_fixture(&fixture_dir).unwrap();

        assert!(matches!(
            fixture.manifest.expected.first(),
            Some(ExpectedItem::Diagnostic(diagnostic)) if diagnostic.relative_path == "src/app.ts"
        ));
    }
}
