use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::Path;

fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

fn stdout_json(assert: assert_cmd::assert::Assert) -> serde_json::Value {
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    serde_json::from_str(&stdout)
        .unwrap_or_else(|error| panic!("stdout was not parseable JSON: {error}\nstdout:\n{stdout}"))
}

fn diagnostic_files(value: &serde_json::Value, rule_id: &str) -> Vec<String> {
    value
        .as_array()
        .unwrap()
        .iter()
        .filter(|diagnostic| diagnostic["rule_id"] == rule_id)
        .map(|diagnostic| diagnostic["file"].as_str().unwrap().to_string())
        .collect()
}

fn diagnostics(value: &serde_json::Value) -> &[serde_json::Value] {
    value.as_array().unwrap()
}

fn diagnostic_has_evidence(diagnostic: &serde_json::Value, label: &str, value: &str) -> bool {
    diagnostic["evidence"].as_array().is_some_and(|evidence| {
        evidence.iter().any(|item| {
            item["label"] == label
                && item["value"]
                    .as_str()
                    .is_some_and(|actual| actual.contains(value))
        })
    })
}

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
fn init_does_not_overwrite_existing_config() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join(".polint.toml");
    fs::write(&config, "# sentinel\n[workspace]\ninclude = [\"src/**\"]\n").unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success();

    assert!(temp.path().join(".polint/rules").exists());
    assert_eq!(
        fs::read_to_string(config).unwrap(),
        "# sentinel\n[workspace]\ninclude = [\"src/**\"]\n"
    );
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
fn new_rule_go_creates_sdk_oriented_skeleton() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["new-rule", "go", "branch-error-paths"])
        .assert()
        .success();

    let rule_dir = temp.path().join(".polint/rules/branch-error-paths");
    assert!(rule_dir.join("Cargo.toml").exists());
    let lib = fs::read_to_string(rule_dir.join("src/lib.rs")).unwrap();
    assert!(lib.contains("id: \"custom/branch-error-paths\""));
    assert!(lib.contains(".go_tests().branch_obligations()"));
}

#[test]
fn new_rule_ts_creates_sdk_oriented_skeleton() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["new-rule", "ts", "no-raw-brand-colors"])
        .assert()
        .success();

    let rule_dir = temp.path().join(".polint/rules/no-raw-brand-colors");
    assert!(rule_dir.join("Cargo.toml").exists());
    let lib = fs::read_to_string(rule_dir.join("src/lib.rs")).unwrap();
    assert!(lib.contains("id: \"custom/no-raw-brand-colors\""));
    assert!(lib.contains(".string_literals().jsx_attributes()"));
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
fn check_reports_go_parser_diagnostic_for_invalid_source() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r#"
[profiles.phase4]
rules = []
"#,
    );
    write_file(
        &temp.path().join("broken.go"),
        "package broken\nfunc Broken( {\n",
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase4",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    let diagnostic = diagnostics(&json)
        .iter()
        .find(|diagnostic| {
            diagnostic["rule_id"] == "parser/go" && diagnostic["file"] == "broken.go"
        })
        .expect("invalid Go source should emit parser/go for broken.go");
    assert!(
        diagnostic["message"]
            .as_str()
            .is_some_and(|message| message.contains("syntax error")),
        "parser/go diagnostic should mention syntax error: {diagnostic:#?}"
    );
}

#[test]
fn check_clean_go_fixture_has_no_parser_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r#"
[profiles.phase4]
rules = []
"#,
    );
    write_file(
        &temp.path().join("payment.go"),
        include_str!("../../../tests/fixtures/go/clean/payment.go"),
    );
    write_file(
        &temp.path().join("payment_test.go"),
        include_str!("../../../tests/fixtures/go/clean/payment_test.go"),
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase4",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    assert!(
        !diagnostics(&json)
            .iter()
            .any(|diagnostic| diagnostic["rule_id"] == "parser/go"),
        "clean Go fixtures should not emit parser/go diagnostics: {json:#?}"
    );
}

#[test]
fn check_go_full_profile_uses_branch_and_test_facts() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r#"
[profiles.phase4]
rules = ["examples/go-branch-obligations"]
"#,
    );
    write_file(
        &temp.path().join("payment.go"),
        include_str!("../../../tests/fixtures/go/failing/payment.go"),
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase4",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    let diagnostic = diagnostics(&json)
        .iter()
        .find(|diagnostic| diagnostic["rule_id"] == "examples/go-branch-obligations")
        .expect("failing Go fixture should emit branch-obligation diagnostic");
    assert!(
        diagnostic_has_evidence(diagnostic, "edge", ""),
        "branch diagnostic should include edge evidence: {diagnostic:#?}"
    );
    assert!(
        diagnostic["help"]
            .as_str()
            .is_some_and(|help| help.contains("heuristic")),
        "branch diagnostic help should disclose heuristic behavior: {diagnostic:#?}"
    );
}

#[test]
fn check_go_import_boundary_uses_import_facts() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r#"
[profiles.phase4]
rules = ["examples/go-import-boundaries"]

[[rules.config]]
id = "examples/go-import-boundaries"
files = ["**/*.go"]

[rules.config.forbidden_imports]
"**/*.go" = ["net/http"]
"#,
    );
    write_file(
        &temp.path().join("main.go"),
        "package main\nimport \"net/http\"\nfunc main() { _ = http.MethodGet }\n",
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase4",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    let diagnostic = diagnostics(&json)
        .iter()
        .find(|diagnostic| diagnostic["rule_id"] == "examples/go-import-boundaries")
        .expect("forbidden Go import should emit import-boundary diagnostic");
    assert!(
        diagnostic_has_evidence(diagnostic, "import", "net/http"),
        "import-boundary diagnostic should include net/http evidence: {diagnostic:#?}"
    );
}

#[test]
fn check_reports_ts_parser_diagnostic_for_invalid_source() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r#"
[profiles.phase5]
rules = []
"#,
    );
    write_file(&temp.path().join("broken.ts"), "export function Broken( {");

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase5",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    let diagnostic = diagnostics(&json)
        .iter()
        .find(|diagnostic| {
            diagnostic["rule_id"] == "parser/ts" && diagnostic["file"] == "broken.ts"
        })
        .expect("invalid TS source should emit parser/ts for broken.ts");
    assert!(
        diagnostic["message"]
            .as_str()
            .is_some_and(|message| { message.contains("TS/JS parser reported a syntax error") }),
        "parser/ts diagnostic should mention syntax error: {diagnostic:#?}"
    );
}

#[test]
fn check_clean_ts_fixture_has_no_parser_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r#"
[profiles.phase5]
rules = []
"#,
    );
    write_file(
        &temp.path().join("component.tsx"),
        include_str!("../../../tests/fixtures/ts/clean/component.tsx"),
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase5",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    assert!(
        !diagnostics(&json)
            .iter()
            .any(|diagnostic| diagnostic["rule_id"] == "parser/ts"),
        "clean TS fixture should not emit parser/ts diagnostics: {json:#?}"
    );
}

#[test]
fn check_ts_full_profile_uses_phase5_facts() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r##"
[profiles.phase5]
rules = ["examples/ts-cyclomatic-complexity", "examples/ts-no-raw-colors", "examples/config-query-no-literal"]

[[rules.config]]
id = "examples/ts-cyclomatic-complexity"
max = 3
files = ["**/*.tsx", "**/*.ts", "**/*.jsx", "**/*.js"]

[[rules.config]]
id = "examples/ts-no-raw-colors"
files = ["**/*.tsx", "**/*.ts", "**/*.jsx", "**/*.js"]

[[rules.config]]
id = "examples/config-query-no-literal"
deny = ["legacy-testid"]
files = ["**/*.tsx", "**/*.ts", "**/*.jsx", "**/*.js"]
"##,
    );
    write_file(
        &temp.path().join("component.tsx"),
        include_str!("../../../tests/fixtures/ts/failing/component.tsx"),
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase5",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    let complexity = diagnostics(&json)
        .iter()
        .find(|diagnostic| diagnostic["rule_id"] == "examples/ts-cyclomatic-complexity")
        .expect("failing TS fixture should emit TS complexity diagnostic");
    assert!(
        diagnostic_has_evidence(complexity, "complexity", ""),
        "complexity diagnostic should include complexity evidence: {complexity:#?}"
    );

    let raw_color = diagnostics(&json)
        .iter()
        .find(|diagnostic| diagnostic["rule_id"] == "examples/ts-no-raw-colors")
        .expect("failing TS fixture should emit raw-color diagnostic");
    assert_eq!(raw_color["file"], "component.tsx");

    let denied_literal = diagnostics(&json)
        .iter()
        .find(|diagnostic| diagnostic["rule_id"] == "examples/config-query-no-literal")
        .expect("failing TS fixture should emit denied literal diagnostic");
    assert!(
        denied_literal["message"]
            .as_str()
            .is_some_and(|message| message.contains("legacy-testid")),
        "denied literal diagnostic should mention legacy-testid: {denied_literal:#?}"
    );
}

#[test]
fn check_ts_design_token_example_reports_raw_colors() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r#"
[profiles.phase5]
rules = ["examples/ts-no-raw-colors"]

[[rules.config]]
id = "examples/ts-no-raw-colors"
files = ["**/*.tsx", "**/*.ts", "**/*.jsx", "**/*.js"]
"#,
    );
    write_file(
        &temp.path().join("Button.tsx"),
        include_str!("../../../examples/ts-design-tokens/Button.tsx"),
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase5",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    let diagnostic = diagnostics(&json)
        .iter()
        .find(|diagnostic| diagnostic["rule_id"] == "examples/ts-no-raw-colors")
        .expect("TS design-token example should emit raw-color diagnostic");
    assert_eq!(diagnostic["file"], "Button.tsx");
}

#[test]
fn check_json_without_config_is_parseable() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("component.tsx"),
        "export function Button() { return <button style={{ color: \"#ff00aa\" }}>Pay</button>; }",
    )
    .unwrap();

    let assert = Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", "--format", "json", "--fail-on", "none"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Config not found").not());
    let json = stdout_json(assert);

    assert!(
        json.as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| { diagnostic["rule_id"] == "examples/ts-no-raw-colors" })
    );
}

#[test]
fn check_human_without_config_suggests_init() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join("component.tsx"),
        "export const label = \"ok\";",
    )
    .unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", "--fail-on", "none"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Config not found. Run `polint init` to create .polint.toml.",
        ));
}

#[test]
fn profile_and_severity_override_affect_json_and_exit_code() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(".polint.toml"),
        r#"
[profiles.phase2]
rules = ["examples/ts-no-raw-colors"]

[[rules.config]]
id = "examples/ts-no-raw-colors"
severity = "info"
files = ["**/*.tsx"]
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("component.tsx"),
        "export const color = \"#ff00aa\";",
    )
    .unwrap();

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase2",
                "--format",
                "json",
                "--fail-on",
                "warn",
            ])
            .assert()
            .success(),
    );

    let diagnostic = json
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| diagnostic["rule_id"] == "examples/ts-no-raw-colors")
        .unwrap();
    assert_eq!(diagnostic["severity"], "info");
}

#[test]
fn sarif_no_cache_and_rule_paths_are_supported() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(".polint.toml"),
        r#"
[rules]
paths = [".polint/rules", "tools/polint-rules"]

[profiles.phase2]
rules = ["examples/ts-no-raw-colors"]
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("component.tsx"),
        "export const color = \"#ff00aa\";",
    )
    .unwrap();

    let sarif = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase2",
                "--format",
                "sarif",
                "--no-cache",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    assert_eq!(sarif.pointer("/runs/0/tool/driver/name").unwrap(), "polint");
    assert_eq!(
        sarif.pointer("/runs/0/results/0/ruleId").unwrap(),
        "examples/ts-no-raw-colors"
    );
}

#[test]
fn discovery_detects_all_supported_extensions() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(".polint.toml"),
        r#"
[workspace]
include = ["src/**"]
exclude = []

[profiles.phase2]
rules = ["examples/ts-no-raw-colors"]

[[rules.config]]
id = "examples/ts-no-raw-colors"
severity = "error"
files = ["**/*.ts", "**/*.tsx", "**/*.js", "**/*.jsx"]
"#,
    )
    .unwrap();
    for path in ["src/a.ts", "src/b.tsx", "src/c.js", "src/d.jsx"] {
        write_file(&temp.path().join(path), "export const color = \"#ff00aa\";");
    }
    write_file(
        &temp.path().join("src/main.go"),
        "package main\nfunc main() {}\n",
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase2",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    let files = diagnostic_files(&json, "examples/ts-no-raw-colors");
    assert!(files.contains(&"src/a.ts".to_string()));
    assert!(files.contains(&"src/b.tsx".to_string()));
    assert!(files.contains(&"src/c.js".to_string()));
    assert!(files.contains(&"src/d.jsx".to_string()));
    assert!(!files.contains(&"src/main.go".to_string()));
}

#[test]
fn discovery_respects_gitignore_include_exclude_and_supported_extensions() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join(".gitignore"), "ignored.tsx\n").unwrap();
    fs::write(
        temp.path().join(".polint.toml"),
        r#"
[workspace]
include = ["src/**"]
exclude = ["src/excluded.tsx"]

[profiles.phase2]
rules = ["examples/ts-no-raw-colors"]
"#,
    )
    .unwrap();
    write_file(
        &temp.path().join("src/included.tsx"),
        "export const color = \"#ff00aa\";",
    );
    write_file(
        &temp.path().join("src/excluded.tsx"),
        "export const color = \"#ff00aa\";",
    );
    write_file(
        &temp.path().join("ignored.tsx"),
        "export const color = \"#ff00aa\";",
    );
    write_file(
        &temp.path().join("src/ignored.tsx"),
        "export const color = \"#ff00aa\";",
    );
    write_file(&temp.path().join("src/notes.txt"), "#ff00aa\n");

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase2",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    let files = diagnostic_files(&json, "examples/ts-no-raw-colors");
    assert!(files.contains(&"src/included.tsx".to_string()));
    assert!(!files.contains(&"src/excluded.tsx".to_string()));
    assert!(!files.contains(&"ignored.tsx".to_string()));
    assert!(!files.contains(&"src/ignored.tsx".to_string()));
    assert!(!files.contains(&"src/notes.txt".to_string()));
}

#[test]
fn discovery_respects_default_excludes() {
    let temp = tempfile::tempdir().unwrap();
    for path in [
        "src/included.tsx",
        "vendor/vendor.tsx",
        "node_modules/pkg/index.tsx",
        "target/generated.tsx",
        "src/generated.pb.go",
    ] {
        write_file(&temp.path().join(path), "export const color = \"#ff00aa\";");
    }

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args(["check", "--format", "json", "--fail-on", "none"])
            .assert()
            .success(),
    );

    let files = diagnostic_files(&json, "examples/ts-no-raw-colors");
    assert!(files.contains(&"src/included.tsx".to_string()));
    assert!(!files.contains(&"vendor/vendor.tsx".to_string()));
    assert!(!files.contains(&"node_modules/pkg/index.tsx".to_string()));
    assert!(!files.contains(&"target/generated.tsx".to_string()));
    assert!(!files.contains(&"src/generated.pb.go".to_string()));
}

#[test]
fn check_json_output_is_deterministic_across_repeated_runs() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(temp.path().join(".gitignore"), "src/ignored.ts\n").unwrap();
    fs::write(
        temp.path().join(".polint.toml"),
        r#"
[workspace]
include = ["src/**"]
exclude = ["src/excluded.ts"]

[profiles.phase3]
rules = ["examples/ts-no-raw-colors"]
"#,
    )
    .unwrap();
    write_file(
        &temp.path().join("src/z.tsx"),
        "export function Button() { return <div style={{ color: \"#ffffff\" }} />; }",
    );
    write_file(
        &temp.path().join("src/excluded.ts"),
        "export const excluded = \"#333333\";",
    );
    write_file(
        &temp.path().join("src/ignored.ts"),
        "export const ignored = \"#444444\";",
    );
    write_file(
        &temp.path().join("src/a.ts"),
        "export const accent = \"#111111\";",
    );

    let first = phase3_check_json(temp.path());
    let second = phase3_check_json(temp.path());
    let third = phase3_check_json(temp.path());

    assert_eq!(second, first);
    assert_eq!(third, first);
    assert_eq!(
        diagnostic_files(&first, "examples/ts-no-raw-colors"),
        ["src/a.ts", "src/z.tsx"]
    );
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

fn phase3_check_json(root: &Path) -> serde_json::Value {
    stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(root)
            .args([
                "check",
                "--profile",
                "phase3",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    )
}
