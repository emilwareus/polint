use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
fn init_creates_config() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized polint config"));

    assert!(temp.path().join(".polint.toml").exists());
    assert!(temp.path().join(".polint/rules").exists());
}

#[test]
fn new_rule_creates_skeleton() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["new-rule", "go", "branch-error-paths"])
        .assert()
        .success();

    assert!(
        temp.path()
            .join(".polint/rules/branch-error-paths/Cargo.toml")
            .exists()
    );
    assert!(
        temp.path()
            .join(".polint/rules/branch-error-paths/src/lib.rs")
            .exists()
    );
}

#[test]
fn check_reports_ts_raw_color() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success();
    fs::write(
        temp.path().join("component.tsx"),
        "export function Button() { return <button style={{ color: \"#ff00aa\" }}>Pay</button>; }",
    )
    .unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", "--format", "json"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("examples/ts-no-raw-colors"));
}

#[test]
fn check_clean_repo_succeeds() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success();
    fs::write(
        temp.path().join("component.tsx"),
        "export const label = \"ok\";",
    )
    .unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .success()
        .stdout(predicate::str::contains("No diagnostics"));
}
