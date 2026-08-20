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
    assert_tree_excludes(
        &src.join("ts"),
        &["crate::go", "crate::analysis::", "crate::core::AnalysisDb"],
    );
}

#[test]
fn semantic_graph_facade_delegates_ts_builder_without_ts_implementation_imports() {
    let src = repo_root().join("crates/polint/src");
    let facade_dir = src.join("analysis/semantic_graph");
    assert!(
        !facade_dir.join("build.rs").exists(),
        "the composition facade must not own the TypeScript semantic-graph builder"
    );

    let facade =
        fs::read_to_string(facade_dir.join("mod.rs")).expect("read semantic-graph facade module");
    assert!(
        facade.contains("crate::ts::semantic_graph_build"),
        "the composition facade must narrowly re-export the TypeScript-owned builder"
    );
    assert_tree_excludes(
        &facade_dir,
        &[
            "crate::ts::binding::direct",
            "crate::ts::binding::facts",
            "crate::ts::inventory",
            "crate::ts::object_model",
            "crate::ts::parse",
            "crate::ts::scope",
            "crate::ts::semantic_graph::",
            "crate::ts::token_flow",
        ],
    );

    let ts_builder_path = src.join("ts/semantic_graph_build.rs");
    assert!(
        ts_builder_path.is_file(),
        "the TypeScript frontend must own semantic-graph projection"
    );
    let ts_builder = fs::read_to_string(ts_builder_path).expect("read TypeScript graph builder");
    for forbidden in ["crate::analysis::", "crate::core::AnalysisDb"] {
        assert!(
            !ts_builder.contains(forbidden),
            "the TypeScript graph builder must use neutral host contracts, not `{forbidden}`"
        );
    }
}

#[test]
fn go_rta_projection_is_separate_from_the_neutral_engine() {
    let src = repo_root().join("crates/polint/src");
    let neutral = src.join("analysis_neutral/solver/go_rta");
    let go = src.join("go/rta");

    for file in ["snapshot.rs", "dispatch.rs", "fixpoint.rs"] {
        assert!(
            neutral.join(file).is_file(),
            "the frontend-neutral RTA engine must own {file}"
        );
        assert!(
            !go.join(file).exists(),
            "the Go frontend must not own RTA algorithm file {file}"
        );
    }

    let mut go_files = Vec::new();
    collect_rs_files(&go, &mut go_files);
    let mut go_file_names = go_files
        .iter()
        .map(|path| {
            path.file_name()
                .expect("RTA file name")
                .to_string_lossy()
                .into_owned()
        })
        .collect::<Vec<_>>();
    go_file_names.sort();
    assert_eq!(
        go_file_names,
        ["inputs.rs", "mod.rs"],
        "the Go RTA directory is a projection adapter and facade only"
    );

    let snapshot = fs::read_to_string(neutral.join("snapshot.rs")).expect("read RTA snapshot");
    assert!(snapshot.contains("struct RtaInputs"));
    assert!(!snapshot.contains("struct GoRtaInputs"));

    let adapter = fs::read_to_string(go.join("inputs.rs")).expect("read Go RTA input adapter");
    assert!(adapter.contains("fn from_db"));
    assert!(adapter.contains("crate::go::semantic::facts"));
    assert!(!adapter.contains("crate::analysis::"));
    for algorithm in [
        "fn resolve_callsite",
        "fn solve_rta",
        "while !frontier.is_empty()",
    ] {
        assert!(
            !adapter.contains(algorithm),
            "the Go input adapter contains neutral algorithm logic: {algorithm}"
        );
    }

    let facade = fs::read_to_string(go.join("mod.rs")).expect("read Go RTA facade");
    assert!(facade.contains("analysis_neutral::solver::go_rta::solve_rta"));
    assert_tree_excludes(
        &neutral,
        &[
            "crate::go",
            "crate::core",
            "crate::analysis::",
            "AnalysisDb",
            "GoSemantic",
        ],
    );
    assert!(
        !src.join("analysis/solver/go_rta").exists(),
        "the composition-root solver must not duplicate the neutral RTA engine"
    );
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

#[test]
fn ci_uses_supported_supply_chain_and_existing_go_cache_inputs() {
    let root = repo_root();
    let workflow =
        fs::read_to_string(root.join(".github/workflows/ci.yml")).expect("read CI workflow");

    assert!(workflow.contains("command: check"));
    assert!(workflow.contains("command-arguments: all"));
    assert!(workflow.contains("arguments: \"\""));
    assert!(!workflow.contains("arguments: --all-features --locked"));
    assert!(workflow.contains("crates/polint/src/go-sidecar/polint-go-frontend/go.sum"));
    assert!(workflow.contains("crates/polint/src/go-sidecar/polint-go-symbols/go.sum"));
    assert!(!workflow.contains("crates/polint/go-sidecar/"));

    for path in [
        "crates/polint/src/go-sidecar/polint-go-frontend/go.sum",
        "crates/polint/src/go-sidecar/polint-go-symbols/go.sum",
    ] {
        assert!(
            root.join(path).is_file(),
            "CI cache input is missing: {path}"
        );
    }
}
