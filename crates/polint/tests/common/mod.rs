//! Shared helpers for integration tests (`tests/*.rs`).

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|e| panic!("create parent dir for {}: {e}", path.display()));
    }
    fs::write(path, contents)
        .unwrap_or_else(|e| panic!("write fixture file {}: {e}", path.display()));
}

pub(crate) fn stdout_json(assert: assert_cmd::assert::Assert) -> serde_json::Value {
    let stdout = String::from_utf8(assert.get_output().stdout.clone())
        .expect("polint stdout should be valid UTF-8");
    serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("stdout was not parseable JSON: {error}\nstdout:\n{stdout}"))
}

pub(crate) fn stdout_string(assert: assert_cmd::assert::Assert) -> String {
    String::from_utf8(assert.get_output().stdout.clone())
        .expect("polint stdout should be valid UTF-8")
}

pub(crate) fn cache_json_count(root: &Path) -> usize {
    let cache_dir = root.join(".polint/cache");
    if !cache_dir.exists() {
        return 0;
    }
    fs::read_dir(&cache_dir)
        .unwrap_or_else(|e| panic!("read cache dir {}: {e}", cache_dir.display()))
        .filter(|entry| {
            entry
                .as_ref()
                .ok()
                .and_then(|entry| entry.path().extension().map(|ext| ext == "json"))
                .unwrap_or(false)
        })
        .count()
}

pub(crate) fn diagnostic_files(value: &serde_json::Value, rule_id: &str) -> Vec<String> {
    diagnostics(value)
        .iter()
        .filter(|diagnostic| diagnostic["rule_id"] == rule_id)
        .map(|diagnostic| {
            diagnostic["file"]
                .as_str()
                .unwrap_or_else(|| {
                    panic!("expected diagnostic.file to be a string, got: {diagnostic:#?}")
                })
                .to_string()
        })
        .collect()
}

pub(crate) fn diagnostics(value: &serde_json::Value) -> &[serde_json::Value] {
    value["diagnostics"].as_array().unwrap_or_else(|| {
        panic!("expected polint JSON report with diagnostics array, got: {value:?}")
    })
}

pub(crate) fn diagnostic_has_evidence(
    diagnostic: &serde_json::Value,
    label: &str,
    value: &str,
) -> bool {
    diagnostic["evidence"].as_array().is_some_and(|evidence| {
        evidence.iter().any(|item| {
            item["label"] == label
                && item["value"]
                    .as_str()
                    .is_some_and(|actual| actual.contains(value))
        })
    })
}

pub(crate) fn diagnostics_for_rule<'a>(
    value: &'a serde_json::Value,
    rule_id: &str,
) -> Vec<&'a serde_json::Value> {
    diagnostics(value)
        .iter()
        .filter(|diagnostic| diagnostic["rule_id"] == rule_id)
        .collect()
}

pub(crate) fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("polint crate should live under crates/")
        .to_path_buf()
}

pub(crate) fn example_rule_cmd(example: &str) -> Command {
    let manifest = repo_root()
        .join("examples")
        .join(example)
        .join(".polint/rules/Cargo.toml");
    let manifest_str = manifest
        .to_str()
        .unwrap_or_else(|| panic!("manifest path is not valid UTF-8: {}", manifest.display()))
        .to_string();
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()));
    command.args(["run", "--quiet", "--manifest-path", &manifest_str, "--"]);
    command
}

pub(crate) fn raw_color_rule_cmd() -> Command {
    example_rule_cmd("ts-design-tokens")
}

pub(crate) fn ts_complexity_rule_cmd() -> Command {
    example_rule_cmd("ts-complexity")
}

pub(crate) fn go_import_boundaries_rule_cmd() -> Command {
    example_rule_cmd("go-import-boundaries")
}

pub(crate) fn go_branch_obligations_rule_cmd() -> Command {
    example_rule_cmd("go-branch-obligations")
}

pub(crate) fn write_phase8_raw_color_fixture(root: &Path, severity: &str) {
    write_file(
        &root.join(".polint.toml"),
        &format!(
            r##"
[profiles.phase8]
rules = ["local/no-raw-colors"]

[[rules.config]]
id = "local/no-raw-colors"
severity = "{severity}"
files = ["**/*.tsx"]
"##
        ),
    );
    write_file(
        &root.join("component.tsx"),
        "export function Button() { return <button style={{ color: \"#ff00aa\" }}>Pay</button>; }\n",
    );
}

pub(crate) fn write_phase8_graph_fixture(root: &Path) {
    write_file(
        &root.join(".polint.toml"),
        r#"
[profiles.phase8]
rules = []
"#,
    );
    write_file(
        &root.join("main.go"),
        r#"
package main

import "fmt"

func Authorize() {
    validateUser()
    charge()
    fmt.Println("ok")
}

func validateUser() {}

func charge() {}
"#,
    );
}
