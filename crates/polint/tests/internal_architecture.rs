use std::fs;
use std::path::{Path, PathBuf};

const REMOVED_PACKAGES: &[&str] = &[
    "polint-core",
    "polint-ir",
    "polint-analysis-api",
    "polint-frontend-api",
    "polint-analysis",
    "polint-go",
    "polint-ts",
];

#[test]
fn workspace_has_only_two_publishable_product_packages() {
    let root = repo_root();
    let manifest = fs::read_to_string(root.join("Cargo.toml")).expect("read workspace manifest");
    let lock = fs::read_to_string(root.join("Cargo.lock")).expect("read workspace lockfile");
    for package in REMOVED_PACKAGES {
        assert!(
            !manifest.contains(&format!("crates/{package}")),
            "removed internal package remains a workspace member: {package}"
        );
        assert!(
            !lock.contains(&format!("name = \"{package}\"")),
            "removed internal package remains in Cargo.lock: {package}"
        );
        assert!(
            !root.join("crates").join(package).exists(),
            "removed internal package directory remains: {package}"
        );
    }
}

#[test]
fn internal_dependency_directions_are_acyclic() {
    let src = repo_root().join("crates/polint/src");
    assert_tree_excludes(
        &src.join("internal_core"),
        &[
            "crate::ir",
            "crate::analysis_api",
            "crate::analysis_neutral",
            "crate::frontend_api",
            "crate::go",
            "crate::ts",
        ],
    );
    assert_tree_excludes(
        &src.join("ir"),
        &[
            "crate::analysis_api",
            "crate::analysis_neutral",
            "crate::frontend_api",
            "crate::go",
            "crate::ts",
        ],
    );
    assert_tree_excludes(
        &src.join("analysis_api"),
        &[
            "crate::analysis_neutral",
            "crate::frontend_api",
            "crate::go",
            "crate::ts",
        ],
    );
    assert_tree_excludes(
        &src.join("frontend_api"),
        &["crate::analysis_neutral", "crate::go", "crate::ts"],
    );
    assert_tree_excludes(
        &src.join("analysis_neutral"),
        &["crate::frontend_api", "crate::go", "crate::ts"],
    );
    assert_tree_excludes(&src.join("go"), &["crate::ts"]);
    assert_tree_excludes(&src.join("ts"), &["crate::go"]);
}

fn assert_tree_excludes(root: &Path, forbidden: &[&str]) {
    let mut files = Vec::new();
    collect_rs_files(root, &mut files);
    for file in files {
        let source = fs::read_to_string(&file).expect("read Rust source");
        for needle in forbidden {
            assert!(
                !source.contains(needle),
                "{} crosses internal dependency direction with `{needle}`",
                file.display()
            );
        }
    }
}

fn collect_rs_files(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(root).unwrap_or_else(|error| panic!("read {}: {error}", root.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical repo root")
}

#[test]
fn language_features_are_isolated_and_default_to_both() {
    let root = repo_root();
    let manifest =
        fs::read_to_string(root.join("crates/polint/Cargo.toml")).expect("read polint manifest");
    let parsed: toml::Value = toml::from_str(&manifest).expect("parse polint manifest");
    let features = parsed["features"].as_table().expect("features table");
    assert_eq!(
        features["default"],
        toml::Value::Array(vec!["lang-go".into(), "lang-typescript".into()])
    );
    assert_eq!(
        features["all-languages"],
        toml::Value::Array(vec!["lang-go".into(), "lang-typescript".into()])
    );

    let dependencies = parsed["dependencies"]
        .as_table()
        .expect("dependencies table");
    for dependency in ["tree-sitter", "tree-sitter-go"] {
        assert_eq!(dependencies[dependency]["optional"].as_bool(), Some(true));
        assert!(
            features["lang-go"]
                .as_array()
                .unwrap()
                .contains(&format!("dep:{dependency}").into())
        );
    }
    for dependency in [
        "oxc_allocator",
        "oxc_ast",
        "oxc_parser",
        "oxc_resolver",
        "oxc_semantic",
        "oxc_span",
    ] {
        assert_eq!(dependencies[dependency]["optional"].as_bool(), Some(true));
        assert!(
            features["lang-typescript"]
                .as_array()
                .unwrap()
                .contains(&format!("dep:{dependency}").into())
        );
    }
}
