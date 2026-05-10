mod common;

use assert_cmd::Command;
use common::*;
use predicates::prelude::*;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn point_generated_rule_pack_at_local_polint(root: &Path) {
    let manifest_path = root.join(".polint/rules/Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let polint_path = repo_root()
        .join("crates/polint")
        .to_string_lossy()
        .replace('\\', "/");
    let dep_line = format!(r#"polint = {{ path = "{polint_path}" }}"#);
    let rewritten = manifest
        .lines()
        .map(|line| {
            if line.starts_with("polint = ") {
                dep_line.clone()
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(&manifest_path, format!("{rewritten}\n")).unwrap();
}

#[test]
fn top_level_help_only_lists_supported_public_commands() {
    let help = stdout_string(
        Command::cargo_bin("polint")
            .unwrap()
            .arg("--help")
            .assert()
            .success(),
    );

    for command in ["init", "add-skill", "new-rule", "check", "ignores"] {
        assert!(help.contains(command), "help should list {command}: {help}");
    }
    for command in ["explain", "test-rules", "profile-rules", "graph"] {
        assert!(
            !help.contains(command),
            "help should not expose internal command {command}: {help}"
        );
    }
}

fn write_plan_capability_rule_repo(root: &Path) {
    let polint_path = repo_root()
        .join("crates/polint")
        .to_string_lossy()
        .replace('\\', "/");
    write_file(
        &root.join(".polint.toml"),
        r#"
[workspace]
include = ["src/**"]
exclude = []

[rules]
paths = [".polint/rules"]
"#,
    );
    write_file(
        &root.join(".polint/rules/Cargo.toml"),
        &format!(
            r#"[package]
name = "polint-local-rules"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
polint = {{ path = "{polint_path}" }}

[workspace]
"#,
        ),
    );
    write_file(
        &root.join(".polint/rules/src/main.rs"),
        r#"use std::process::ExitCode;

use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/needs-imports",
    description = "Needs import facts.",
    severity = "warn"
)]
fn needs_imports(_ctx: &mut RuleCtx<'_>, imports: Imports<'_>) -> RuleResult {
    let _import_count = imports.iter().count();
    Ok(())
}

#[polint::rule(
    id = "local/needs-cfg",
    description = "Needs CFG facts.",
    severity = "warn"
)]
fn needs_cfg(_ctx: &mut RuleCtx<'_>, _cfg: Cfg<'_>) -> RuleResult {
    _ctx.warn(
        &Span::point(FileId(0), 1, 1),
        "this should not run while cfg is unsupported",
    );
    Ok(())
}

fn main() -> ExitCode {
    polint::runner::run_cli(vec![needs_imports(), needs_cfg()])
}
"#,
    );
    write_file(
        &root.join("src/component.ts"),
        r#"import { token } from "./token";

export const value = token;
"#,
    );
    write_file(&root.join("src/token.ts"), r#"export const token = "ok";"#);
}

fn write_cache_capability_rule_repo(root: &Path) {
    let polint_path = repo_root()
        .join("crates/polint")
        .to_string_lossy()
        .replace('\\', "/");
    write_file(
        &root.join(".polint.toml"),
        r#"
[workspace]
include = ["src/**"]
exclude = []

[rules]
paths = [".polint/rules"]
"#,
    );
    write_file(
        &root.join(".polint/rules/Cargo.toml"),
        &format!(
            r#"[package]
name = "polint-local-rules"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
polint = {{ path = "{polint_path}" }}

[workspace]
"#,
        ),
    );
    write_file(
        &root.join(".polint/rules/src/main.rs"),
        r#"use std::process::ExitCode;

use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/cache-capability",
    description = "Reads capability-gated facts.",
    severity = "warn"
)]
fn cache_capability(_ctx: &mut RuleCtx<'_>, literals: StringLiterals<'_>) -> RuleResult {
    let _literal_count = literals.iter().count();
    Ok(())
}

fn main() -> ExitCode {
    polint::runner::run_cli(vec![cache_capability()])
}
"#,
    );
    write_file(
        &root.join("src/component.tsx"),
        r##"export function Component() {
  return <div data-color="#fff">ok</div>;
}
"##,
    );
}

fn write_no_todo_rule_repo(root: &Path, require_reason: bool) {
    let polint_path = repo_root()
        .join("crates/polint")
        .to_string_lossy()
        .replace('\\', "/");
    write_file(
        &root.join(".polint.toml"),
        &format!(
            r#"
[workspace]
include = ["src/**"]
exclude = []

[rules]
paths = [".polint/rules"]

[ignores]
require_reason = {require_reason}

[[rules.config]]
id = "local/no-todo-literals"
severity = "warn"
files = ["src/**/*.ts"]
"#
        ),
    );
    write_file(
        &root.join(".polint/rules/Cargo.toml"),
        &format!(
            r#"[package]
name = "polint-local-rules"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
polint = {{ path = "{polint_path}" }}

[workspace]
"#,
        ),
    );
    write_file(
        &root.join(".polint/rules/src/main.rs"),
        r#"use std::process::ExitCode;

use polint::sdk::prelude::*;

#[polint::rule(
    id = "local/no-todo-literals",
    description = "Reject TODO string literals.",
    severity = "warn"
)]
fn no_todo_literals(ctx: &mut RuleCtx<'_>, literals: StringLiterals<'_>) -> RuleResult {
    let rule_id = ctx.rule_id().to_string();
    for literal in literals.iter() {
        let file = ctx.file_path(literal.file);
        if file_in_scope(ctx.options(), &file) && literal.value == "TODO" {
            ctx.report(Diagnostic::warning(
                rule_id.clone(),
                file,
                literal.span.diagnostic_range(),
                "TODO string literal is not allowed.",
            ));
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    polint::runner::run_cli(vec![no_todo_literals()])
}
"#,
    );
}

fn cache_json_file_names(root: &Path) -> BTreeSet<String> {
    fs::read_dir(root.join(".polint/cache"))
        .unwrap_or_else(|error| panic!("read cache dir: {error}"))
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            (path.extension().is_some_and(|ext| ext == "json")).then(|| {
                path.file_name()
                    .unwrap_or_else(|| panic!("cache path has no file name: {}", path.display()))
                    .to_string_lossy()
                    .to_string()
            })
        })
        .collect()
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
    assert!(temp.path().join(".polint/rules/src").exists());
    assert!(temp.path().join(".polint/cache").exists());
    let nested_gitignore = fs::read_to_string(temp.path().join(".polint/.gitignore")).unwrap();
    assert!(nested_gitignore.contains("cache/"), "{nested_gitignore:?}");
    let toolchain = fs::read_to_string(temp.path().join("rust-toolchain.toml")).unwrap();
    assert!(toolchain.contains("channel = \"1.95\""), "{toolchain:?}");
}

#[test]
fn init_does_not_overwrite_existing_rust_toolchain() {
    let temp = tempfile::tempdir().unwrap();
    let tc = temp.path().join("rust-toolchain.toml");
    fs::write(
        &tc,
        "# sentinel-toolchain\n[toolchain]\nchannel = \"1.94.0\"\n",
    )
    .unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success();

    assert_eq!(
        fs::read_to_string(&tc).unwrap(),
        "# sentinel-toolchain\n[toolchain]\nchannel = \"1.94.0\"\n"
    );
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
    assert!(temp.path().join(".polint/rules/src").exists());
    assert!(temp.path().join(".polint/cache").exists());
    assert!(temp.path().join(".polint/.gitignore").exists());
    assert_eq!(
        fs::read_to_string(config).unwrap(),
        "# sentinel\n[workspace]\ninclude = [\"src/**\"]\n"
    );
    let toolchain = fs::read_to_string(temp.path().join("rust-toolchain.toml")).unwrap();
    assert!(toolchain.contains("channel = \"1.95\""), "{toolchain:?}");
}

#[test]
fn init_appends_cache_to_existing_polint_gitignore() {
    let temp = tempfile::tempdir().unwrap();
    let polint_dir = temp.path().join(".polint");
    fs::create_dir_all(&polint_dir).unwrap();
    fs::write(polint_dir.join(".gitignore"), "# keep\nother/\n").unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success();

    let gi = fs::read_to_string(polint_dir.join(".gitignore")).unwrap();
    assert!(gi.contains("# keep"));
    assert!(gi.contains("other/"));
    assert!(gi.contains("cache/"));
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
        temp.path().join(".polint/rules/Cargo.toml").exists(),
        "expected one pack manifest at .polint/rules/Cargo.toml"
    );
    assert!(
        temp.path()
            .join(".polint/rules/src/branch_error_paths.rs")
            .exists()
    );
    assert!(temp.path().join(".polint/rules/src/main.rs").exists());
}

#[test]
fn new_rule_rejects_unsafe_rule_names_without_writing_outside_rules_dir() {
    for rule_name in ["../..", "nested/rule", "", "branch_error_paths"] {
        let temp = tempfile::tempdir().unwrap();

        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args(["new-rule", "go", rule_name])
            .assert()
            .failure()
            .stderr(predicate::str::contains("rule name must be"));

        assert!(
            !temp.path().join("Cargo.toml").exists(),
            "{rule_name:?} must not write Cargo.toml outside .polint/rules"
        );
        assert!(
            !temp.path().join("src/main.rs").exists(),
            "{rule_name:?} must not write src/main.rs outside .polint/rules"
        );
        assert!(
            !temp.path().join(".polint/rules/Cargo.toml").exists(),
            "{rule_name:?} must not write into .polint/rules directly"
        );
        assert!(
            !temp.path().join(".polint/rules/src/nested").exists(),
            "{rule_name:?} must not create stray module paths"
        );
        assert!(
            !temp
                .path()
                .join(".polint/rules/src/branch_error_paths.rs")
                .exists(),
            "{rule_name:?} must not create underscored module files"
        );
    }
}

#[test]
fn new_rule_rejects_existing_rule_without_overwriting_files() {
    let temp = tempfile::tempdir().unwrap();
    let rules = temp.path().join(".polint/rules");
    fs::create_dir_all(rules.join("src")).unwrap();
    let sentinel = "// sentinel module\n";
    fs::write(rules.join("src/demo.rs"), sentinel).unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["new-rule", "go", "demo"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rule already exists"));

    assert_eq!(
        fs::read_to_string(rules.join("src/demo.rs")).unwrap(),
        sentinel
    );
}

#[test]
fn add_skill_installs_claude_skill_non_interactively() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["add-skill", "--agent", "claude"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed Claude Code skill"));

    let skill = temp.path().join(".claude/skills/polint/SKILL.md");
    let contents = fs::read_to_string(skill).unwrap();
    assert!(contents.contains("name: polint"));
    assert!(contents.contains("polint check --fail-on none"));
    assert!(contents.contains("polint ignores --shortstat"));
    assert!(contents.contains("docs/schemas/polint-report-v1.json"));
    assert!(contents.contains("polint ships no built-in"));
    assert!(contents.contains("use polint::sdk::prelude::*;"));
    assert!(contents.contains("Do not implement `Rule` manually"));
}

#[test]
fn add_skill_installs_codex_skill_to_agents_skills_by_default() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["add-skill", "--agent", "codex"])
        .assert()
        .success()
        .stdout(predicate::str::contains(".agents/skills/polint/SKILL.md"));

    assert!(temp.path().join(".agents/skills/polint/SKILL.md").exists());
    assert!(!temp.path().join(".codex/skills/polint/SKILL.md").exists());
}

#[test]
fn add_skill_uses_existing_codex_skills_folder_when_present() {
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir_all(temp.path().join(".codex/skills")).unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["add-skill", "--agent", "codex"])
        .assert()
        .success()
        .stdout(predicate::str::contains(".codex/skills/polint/SKILL.md"));

    assert!(temp.path().join(".codex/skills/polint/SKILL.md").exists());
    assert!(!temp.path().join(".agents/skills/polint/SKILL.md").exists());
}

#[test]
fn add_skill_all_installs_claude_and_codex() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["add-skill", "--all"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Installed Claude Code skill"))
        .stdout(predicate::str::contains("Installed Codex skill"));

    assert!(temp.path().join(".claude/skills/polint/SKILL.md").exists());
    assert!(temp.path().join(".agents/skills/polint/SKILL.md").exists());
}

#[test]
fn add_skill_prompts_when_agent_is_not_provided() {
    let temp = tempfile::tempdir().unwrap();
    let mut command = Command::cargo_bin("polint").unwrap();
    command
        .current_dir(temp.path())
        .arg("add-skill")
        .write_stdin("1\n")
        .assert()
        .success()
        .stdout(predicate::str::contains("Install the polint skill"))
        .stdout(predicate::str::contains("Installed Claude Code skill"));

    assert!(temp.path().join(".claude/skills/polint/SKILL.md").exists());
    assert!(!temp.path().join(".agents/skills/polint/SKILL.md").exists());
}

#[test]
fn add_skill_rejects_existing_skill_unless_force_is_used() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["add-skill", "--agent", "claude"])
        .assert()
        .success();

    let skill = temp.path().join(".claude/skills/polint/SKILL.md");
    fs::write(&skill, "sentinel").unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["add-skill", "--agent", "claude"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("skill already exists"));
    assert_eq!(fs::read_to_string(&skill).unwrap(), "sentinel");

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["add-skill", "--agent", "claude", "--force"])
        .assert()
        .success();
    assert!(fs::read_to_string(skill).unwrap().contains("name: polint"));
}

#[cfg(unix)]
#[test]
fn add_skill_rejects_symlinked_agent_folder() {
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(outside.path(), temp.path().join(".claude")).unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["add-skill", "--agent", "claude"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "refusing to write through symlink",
        ));

    assert!(!outside.path().join("skills/polint/SKILL.md").exists());
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

    let rules = temp.path().join(".polint/rules");
    assert!(rules.join("Cargo.toml").exists());
    let manifest = fs::read_to_string(rules.join("Cargo.toml")).unwrap();
    let module = fs::read_to_string(rules.join("src/branch_error_paths.rs")).unwrap();
    let main = fs::read_to_string(rules.join("src/main.rs")).unwrap();
    assert!(manifest.contains("polint ="));
    assert!(main.contains("polint::runner::run_cli"));
    assert!(main.contains("branch_error_paths::branch_error_paths()"));
    assert!(module.contains("#[polint::rule("));
    assert!(module.contains("id = \"custom/branch-error-paths\""));
    assert!(module.contains("use polint::sdk::prelude::*;"));
    assert!(!module.contains("SourceFiles<'_>"));
    assert!(module.contains("tests: GoTests<'_>"));
    assert!(module.contains("branches: BranchObligations<'_>"));
    assert!(module.contains("tests.related_for_file(branch.file)"));
    assert!(module.contains("branches.iter()"));
    assert!(!module.contains("fn capabilities"));
    assert!(!module.contains("impl Rule"));
    assert!(!module.contains("crate::core::"));
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

    let rules = temp.path().join(".polint/rules");
    let module = fs::read_to_string(rules.join("src/no_raw_brand_colors.rs")).unwrap();
    let main = fs::read_to_string(rules.join("src/main.rs")).unwrap();
    assert!(main.contains("polint::runner::run_cli"));
    assert!(main.contains("no_raw_brand_colors::no_raw_brand_colors()"));
    assert!(module.contains("#[polint::rule("));
    assert!(module.contains("id = \"custom/no-raw-brand-colors\""));
    assert!(module.contains("use polint::sdk::prelude::*;"));
    assert!(!module.contains("SourceFiles<'_>"));
    assert!(module.contains("literals: StringLiterals<'_>"));
    assert!(module.contains("jsx: JsxAttributes<'_>"));
    assert!(module.contains("literals.iter().count()"));
    assert!(module.contains("jsx.iter().count()"));
    assert!(!module.contains("fn capabilities"));
    assert!(!module.contains("impl Rule"));
    assert!(!module.contains("crate::core::"));
}

#[test]
fn new_rule_generic_uses_sdk_query_helpers() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["new-rule", "generic", "domain-names"])
        .assert()
        .success();

    let rules = temp.path().join(".polint/rules");
    let module = fs::read_to_string(rules.join("src/domain_names.rs")).unwrap();
    let main = fs::read_to_string(rules.join("src/main.rs")).unwrap();
    assert!(main.contains("polint::runner::run_cli"));
    assert!(main.contains("domain_names::domain_names()"));
    assert!(module.contains("#[polint::rule("));
    assert!(module.contains("id = \"custom/domain-names\""));
    assert!(module.contains("use polint::sdk::prelude::*;"));
    assert!(!module.contains("SourceFiles<'_>"));
    assert!(module.contains("functions: Functions<'_>"));
    assert!(module.contains("functions.iter().count()"));
    assert!(!module.contains("fn capabilities"));
    assert!(!module.contains("impl Rule"));
    assert!(!module.contains("crate::core::"));
}

#[test]
fn rule_macro_rejects_unknown_fact_view_parameter() {
    let temp = tempfile::tempdir().unwrap();
    let polint_path = repo_root()
        .join("crates/polint")
        .to_string_lossy()
        .replace('\\', "/");

    write_file(
        &temp.path().join("Cargo.toml"),
        &format!(
            r#"[package]
name = "polint-macro-fail-smoke"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
polint = {{ path = "{polint_path}" }}
"#,
        ),
    );
    write_file(
        &temp.path().join("src/main.rs"),
        r#"use polint::sdk::prelude::*;
use std::process::ExitCode;

struct UserDefinedFacts<'a>(&'a ());

#[polint::rule(
    id = "local/invalid-facts",
    description = "Invalid facts.",
    severity = "warn"
)]
fn invalid_facts(
    ctx: &mut RuleCtx<'_>,
    facts: UserDefinedFacts<'_>,
) -> RuleResult {
    let _ = (ctx, facts);
    Ok(())
}

fn main() -> ExitCode {
    polint::runner::run_cli(vec![invalid_facts()])
}
"#,
    );

    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(temp.path())
        .args(["check", "--quiet"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "unsupported polint fact view parameter",
        ));
}

#[test]
fn rule_macro_rejects_shadowed_fact_view_names() {
    let temp = tempfile::tempdir().unwrap();
    let polint_path = repo_root()
        .join("crates/polint")
        .to_string_lossy()
        .replace('\\', "/");

    write_file(
        &temp.path().join("Cargo.toml"),
        &format!(
            r#"[package]
name = "polint-shadowed-facts-fail-smoke"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
polint = {{ path = "{polint_path}" }}
"#,
        ),
    );
    write_file(
        &temp.path().join("src/main.rs"),
        r#"use std::process::ExitCode;

use polint::sdk::prelude::{RuleCtx, RuleResult};

struct Imports<'a>(&'a ());

#[polint::rule(
    id = "local/shadowed-imports",
    description = "Shadowed imports.",
    severity = "warn"
)]
fn shadowed_imports(ctx: &mut RuleCtx<'_>, _imports: Imports<'_>) -> RuleResult {
    let _ = ctx;
    Ok(())
}

fn main() -> ExitCode {
    polint::runner::run_cli(vec![shadowed_imports()])
}
"#,
    );

    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(temp.path())
        .args(["check", "--quiet"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("mismatched types"));
}

#[test]
fn rule_macro_rejects_non_canonical_qualified_fact_view_paths() {
    let temp = tempfile::tempdir().unwrap();
    let polint_path = repo_root()
        .join("crates/polint")
        .to_string_lossy()
        .replace('\\', "/");

    write_file(
        &temp.path().join("Cargo.toml"),
        &format!(
            r#"[package]
name = "polint-qualified-facts-fail-smoke"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
polint = {{ path = "{polint_path}" }}
"#,
        ),
    );
    write_file(
        &temp.path().join("src/main.rs"),
        r#"use std::process::ExitCode;

use polint::sdk::prelude::{RuleCtx, RuleResult};

mod local {
    pub struct Imports<'a>(&'a ());
}

#[polint::rule(
    id = "local/qualified-imports",
    description = "Qualified imports.",
    severity = "warn"
)]
fn qualified_imports(ctx: &mut RuleCtx<'_>, _imports: local::Imports<'_>) -> RuleResult {
    let _ = ctx;
    Ok(())
}

fn main() -> ExitCode {
    polint::runner::run_cli(vec![qualified_imports()])
}
"#,
    );

    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(temp.path())
        .args(["check", "--quiet"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "fact-view parameters must use canonical polint SDK fact views",
        ));
}

#[test]
fn rule_macro_rejects_non_analyzable_rule_signatures() {
    let temp = tempfile::tempdir().unwrap();
    let polint_path = repo_root()
        .join("crates/polint")
        .to_string_lossy()
        .replace('\\', "/");

    write_file(
        &temp.path().join("Cargo.toml"),
        &format!(
            r#"[package]
name = "polint-signature-fail-smoke"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
polint = {{ path = "{polint_path}" }}
"#,
        ),
    );
    write_file(
        &temp.path().join("src/main.rs"),
        r#"use polint::sdk::prelude::*;
use std::process::ExitCode;

#[polint::rule(
    id = "local/async-rule",
    description = "Async rule.",
    severity = "warn"
)]
async fn async_rule(ctx: &mut RuleCtx<'_>) -> RuleResult {
    let _ = ctx;
    Ok(())
}

#[polint::rule(
    id = "local/value-return",
    description = "Value return.",
    severity = "warn"
)]
fn value_return(ctx: &mut RuleCtx<'_>) -> RuleResult<usize> {
    let _ = ctx;
    Ok(1)
}

fn main() -> ExitCode {
    polint::runner::run_cli(vec![async_rule(), value_return()])
}
"#,
    );

    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(temp.path())
        .args(["check", "--quiet"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("plain non-generic sync functions")
                .and(predicate::str::contains("RuleResult<()>")),
        );
}

#[test]
fn external_rule_cannot_manually_implement_rule_trait() {
    let temp = tempfile::tempdir().unwrap();
    let polint_path = repo_root()
        .join("crates/polint")
        .to_string_lossy()
        .replace('\\', "/");

    write_file(
        &temp.path().join("Cargo.toml"),
        &format!(
            r#"[package]
name = "polint-manual-rule-fail-smoke"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
polint = {{ path = "{polint_path}" }}
"#,
        ),
    );
    write_file(
        &temp.path().join("src/main.rs"),
        r#"use polint::sdk::prelude::*;

struct ManualRule;

impl Rule for ManualRule {}

fn main() {}
"#,
    );

    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(temp.path())
        .args(["check", "--quiet"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "expected trait, found struct `Rule`",
        ));
}

#[test]
fn external_prelude_does_not_export_manual_capability_types() {
    let temp = tempfile::tempdir().unwrap();
    let polint_path = repo_root()
        .join("crates/polint")
        .to_string_lossy()
        .replace('\\', "/");

    write_file(
        &temp.path().join("Cargo.toml"),
        &format!(
            r#"[package]
name = "polint-capabilities-prelude-fail-smoke"
version = "0.1.0"
edition = "2024"
publish = false

[dependencies]
polint = {{ path = "{polint_path}" }}
"#,
        ),
    );
    write_file(
        &temp.path().join("src/main.rs"),
        r#"use polint::sdk::prelude::*;

fn main() {
    let _ = Capabilities::new();
}
"#,
    );

    Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
        .current_dir(temp.path())
        .args(["check", "--quiet"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "use of undeclared type `Capabilities`",
        ));
}

#[test]
fn external_generated_rule_uses_sdk_facts_settings_and_reports_diagnostic() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["new-rule", "ts", "no-todo-literals"])
        .assert()
        .success();

    point_generated_rule_pack_at_local_polint(temp.path());
    write_file(
        &temp.path().join(".polint.toml"),
        r#"
[workspace]
include = ["src/**"]

[profiles.sdk]
rules = ["custom/no-todo-literals"]

[[rules.config]]
id = "custom/no-todo-literals"
severity = "warn"
files = ["src/**/*.ts"]
token = "TODO"
message = "Configured forbidden literal found."
"#,
    );
    write_file(
        &temp.path().join(".polint/rules/src/no_todo_literals.rs"),
        r#"use polint::sdk::prelude::*;

#[polint::rule(
    id = "custom/no-todo-literals",
    description = "Reject configured placeholder literals.",
    severity = "warn"
)]
pub(crate) fn no_todo_literals(
    ctx: &mut RuleCtx<'_>,
    literals: StringLiterals<'_>,
) -> RuleResult {
    let token = ctx
        .options()
        .settings
        .get("token")
        .and_then(|value| value.as_str())
        .unwrap_or("TODO")
        .to_string();
    let message = ctx
        .options()
        .settings
        .get("message")
        .and_then(|value| value.as_str())
        .unwrap_or("Forbidden literal found.")
        .to_string();
    let rule_id = ctx.rule_id().to_string();
    let mut diagnostics = Vec::new();

    for literal in literals.iter() {
        let file = ctx.file_path(literal.file);
        if file_in_scope(ctx.options(), &file) && literal.value == token {
            diagnostics.push(
                Diagnostic::warning(
                    rule_id.clone(),
                    file,
                    literal.span.diagnostic_range(),
                    message.clone(),
                )
                .with_evidence("token", token.clone()),
            );
        }
    }

    for diagnostic in diagnostics {
        ctx.report(diagnostic);
    }
    Ok(())
}
"#,
    );
    write_file(
        &temp.path().join("src/component.ts"),
        r#"export const status = "TODO";"#,
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "sdk",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    let diagnostics = diagnostics_for_rule(&json, "custom/no-todo-literals");
    assert_eq!(diagnostics.len(), 1, "{json:#?}");
    assert_eq!(diagnostics[0]["file"], "src/component.ts");
    assert_eq!(
        diagnostics[0]["message"],
        "Configured forbidden literal found."
    );
    assert!(diagnostic_has_evidence(diagnostics[0], "token", "TODO"));
}

#[test]
fn check_suppresses_next_line_ignore_and_ignores_reports_stats() {
    let temp = tempfile::tempdir().unwrap();
    write_no_todo_rule_repo(temp.path(), false);
    write_file(
        &temp.path().join("src/component.ts"),
        r#"// polint-ignore-next-line local/no-todo-literals -- legacy fixture
export const skipped = "TODO";
export const reported = "TODO";
"#,
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args(["check", "--format", "json", "--fail-on", "none"])
            .assert()
            .success(),
    );
    let diagnostics = diagnostics_for_rule(&json, "local/no-todo-literals");
    assert_eq!(diagnostics.len(), 1, "{json:#?}");
    assert_eq!(diagnostics[0]["range"]["start_line"], 3);

    let report = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args(["ignores", "--format", "json", "--filter", "local/*"])
            .assert()
            .success(),
    );
    assert_eq!(report["summary"]["directives"], 1, "{report:#?}");
    assert_eq!(report["summary"]["active"], 1);
    assert_eq!(report["summary"]["suppressed_diagnostics"], 1);
    assert_eq!(
        report["directives"][0]["suppressed"][0]["rule_id"],
        "local/no-todo-literals"
    );

    let shortstat = stdout_string(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args(["ignores", "--shortstat"])
            .assert()
            .success(),
    );
    assert!(shortstat.contains("1 directives, 1 active"), "{shortstat}");
}

#[test]
fn check_reports_missing_reason_when_config_requires_it_but_still_suppresses() {
    let temp = tempfile::tempdir().unwrap();
    write_no_todo_rule_repo(temp.path(), true);
    write_file(
        &temp.path().join("src/component.ts"),
        r#"// polint-ignore-next-line local/no-todo-literals
export const skipped = "TODO";
"#,
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args(["check", "--format", "json", "--fail-on", "none"])
            .assert()
            .success(),
    );
    assert!(
        diagnostics_for_rule(&json, "local/no-todo-literals").is_empty(),
        "{json:#?}"
    );
    let missing = diagnostics_for_rule(&json, "polint/ignore-missing-reason");
    assert_eq!(missing.len(), 1, "{json:#?}");
    assert_eq!(missing[0]["file"], "src/component.ts");

    let report = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args(["ignores", "--format", "json"])
            .assert()
            .success(),
    );
    assert_eq!(report["summary"]["missing_reasons"], 1);
    assert_eq!(report["summary"]["suppressed_diagnostics"], 1);
    assert_eq!(report["directives"][0]["status"], "missing_reason");
}

#[test]
fn check_reports_unused_and_malformed_ignore_directives() {
    let temp = tempfile::tempdir().unwrap();
    write_no_todo_rule_repo(temp.path(), false);
    write_file(
        &temp.path().join("src/component.ts"),
        r#"// polint-ignore-next-line local/no-todo-literals -- no finding here
export const ok = "DONE";
// polint-ignore-next-line
export const also_ok = "DONE";
"#,
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args(["check", "--format", "json", "--fail-on", "none"])
            .assert()
            .success(),
    );
    assert_eq!(
        diagnostics_for_rule(&json, "polint/unused-ignore").len(),
        1,
        "{json:#?}"
    );
    assert_eq!(
        diagnostics_for_rule(&json, "polint/malformed-ignore").len(),
        1,
        "{json:#?}"
    );
}

#[test]
fn check_with_no_bundled_rules_reports_no_policy_diagnostics() {
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

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args(["check", "--format", "json", "--fail-on", "none"])
            .assert()
            .success(),
    );
    assert_eq!(diagnostics(&json).len(), 0);
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
fn check_go_named_profile_uses_branch_and_test_facts() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r#"
[profiles.phase4]
rules = ["local/go-branch-obligations"]
"#,
    );
    write_file(
        &temp.path().join("payment.go"),
        include_str!("../../../tests/fixtures/go/failing/payment.go"),
    );

    let json = stdout_json(
        go_branch_obligations_rule_cmd()
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
        .find(|diagnostic| diagnostic["rule_id"] == "local/go-branch-obligations")
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
rules = ["local/go-import-boundaries"]

[[rules.config]]
id = "local/go-import-boundaries"
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
        go_import_boundaries_rule_cmd()
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
        .find(|diagnostic| diagnostic["rule_id"] == "local/go-import-boundaries")
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
fn check_ts_named_profile_uses_phase5_facts() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r##"
[profiles.phase5]
rules = ["local/ts-cyclomatic-complexity"]

[[rules.config]]
id = "local/ts-cyclomatic-complexity"
max = 3
files = ["**/*.tsx", "**/*.ts", "**/*.jsx", "**/*.js"]
"##,
    );
    write_file(
        &temp.path().join("component.tsx"),
        include_str!("../../../tests/fixtures/ts/failing/component.tsx"),
    );

    let json = stdout_json(
        ts_complexity_rule_cmd()
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
        .find(|diagnostic| diagnostic["rule_id"] == "local/ts-cyclomatic-complexity")
        .expect("failing TS fixture should emit TS complexity diagnostic");
    assert!(
        diagnostic_has_evidence(complexity, "complexity", ""),
        "complexity diagnostic should include complexity evidence: {complexity:#?}"
    );
}

#[test]
fn check_ts_design_token_example_reports_raw_colors() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r#"
[profiles.phase5]
rules = ["local/no-raw-colors"]

[[rules.config]]
id = "local/no-raw-colors"
files = ["**/*.tsx", "**/*.ts", "**/*.jsx", "**/*.js"]
"#,
    );
    write_file(
        &temp.path().join("Button.tsx"),
        include_str!("../../../examples/ts-design-tokens/Button.tsx"),
    );

    let json = stdout_json(
        raw_color_rule_cmd()
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
        .find(|diagnostic| diagnostic["rule_id"] == "local/no-raw-colors")
        .expect("TS design-token example should emit raw-color diagnostic");
    assert_eq!(diagnostic["file"], "Button.tsx");
}

#[test]
fn check_mixed_fixture_handles_go_and_ts_sources() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join("mixed/main.go"),
        include_str!("../../../tests/fixtures/mixed/main.go"),
    );
    write_file(
        &temp.path().join("mixed/view.ts"),
        include_str!("../../../tests/fixtures/mixed/view.ts"),
    );
    write_file(
        &temp.path().join(".polint.toml"),
        r#"
[workspace]
include = ["mixed/**"]
exclude = []

[profiles.mixed]
rules = []
"#,
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "mixed",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );
    assert!(
        json.get("version").and_then(|v| v.as_u64()) == Some(1) && diagnostics(&json).is_empty(),
        "check output should be polint JSON report: {json:#?}"
    );
}

#[test]
fn checked_in_examples_are_runnable_cli_fixtures() {
    let cases: &[(&str, &str, &str, &[&str])] = &[
        (
            "basic",
            "no_raw_colors.rs",
            "local/no-raw-colors",
            &["Button.tsx"],
        ),
        (
            "comment-ignores",
            "no_denied_literals.rs",
            "local/no-denied-literals",
            &["app.ts"],
        ),
        (
            "config-denied-literal",
            "no_denied_literals.rs",
            "local/no-denied-literals",
            &["query.ts"],
        ),
        (
            "custom-rule-go",
            "require_error_branch_tests.rs",
            "local/require-error-branch-tests",
            &["authorize.go"],
        ),
        (
            "custom-rule-ts",
            "no_product_hex_colors.rs",
            "local/no-product-hex-colors",
            &["Button.tsx"],
        ),
        (
            "go-branch-obligations",
            "go_branch_obligations.rs",
            "local/go-branch-obligations",
            &["authorize.go"],
        ),
        (
            "go-complexity",
            "go_complexity.rs",
            "local/go-cyclomatic-complexity",
            &["router.go"],
        ),
        (
            "go-import-boundaries",
            "go_import_boundaries.rs",
            "local/go-import-boundaries",
            &["handler.go"],
        ),
        (
            "go-test-quality",
            "go_test_quality.rs",
            "local/go-test-quality",
            &["payment_test.go"],
        ),
        (
            "ts-complexity",
            "ts_complexity.rs",
            "local/ts-cyclomatic-complexity",
            &["label.ts"],
        ),
        (
            "ts-design-tokens",
            "no_raw_colors.rs",
            "local/no-raw-colors",
            &["Button.tsx"],
        ),
    ];

    for (example, rule_module, expected_rule, expected_files) in cases {
        let example_dir = repo_root().join("examples").join(example);
        assert!(
            example_dir.join("README.md").exists(),
            "{example} should document how to run the example"
        );
        assert!(
            example_dir.join(".polint.toml").exists(),
            "{example} should be runnable as a mini repository"
        );
        assert!(
            example_dir
                .join(".polint/rules/src")
                .join(rule_module)
                .exists(),
            "{example} should contain its local rule implementation ({rule_module})"
        );

        let json = stdout_json(
            Command::cargo_bin("polint")
                .unwrap()
                .current_dir(&example_dir)
                .args([
                    "check",
                    "--format",
                    "json",
                    "--no-cache",
                    "--fail-on",
                    "none",
                ])
                .assert()
                .success(),
        );

        let actual_rules = diagnostics(&json)
            .iter()
            .filter_map(|diagnostic| diagnostic["rule_id"].as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            actual_rules,
            std::collections::BTreeSet::from([*expected_rule]),
            "{example} should emit only its one local rule: {json:#?}"
        );
        for file in *expected_files {
            assert!(
                diagnostics(&json)
                    .iter()
                    .any(|diagnostic| diagnostic["file"] == *file),
                "{example} should report a diagnostic in {file}: {json:#?}"
            );
        }
    }
}

#[test]
fn comment_ignores_example_reports_one_visible_and_one_ignored_diagnostic() {
    let example_dir = repo_root().join("examples/comment-ignores");

    let check_json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(&example_dir)
            .args([
                "check",
                "--format",
                "json",
                "--no-cache",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );
    let visible = diagnostics_for_rule(&check_json, "local/no-denied-literals");
    assert_eq!(visible.len(), 1, "{check_json:#?}");
    assert_eq!(visible[0]["range"]["start_line"], 6);
    assert!(
        diagnostics_for_rule(&check_json, "polint/ignore-missing-reason").is_empty(),
        "{check_json:#?}"
    );

    let ignores_json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(&example_dir)
            .args([
                "ignores",
                "--format",
                "json",
                "--filter",
                "local/no-denied-literals",
                "--no-cache",
            ])
            .assert()
            .success(),
    );
    assert_eq!(
        ignores_json["summary"]["directives"], 1,
        "{ignores_json:#?}"
    );
    assert_eq!(ignores_json["summary"]["active"], 1);
    assert_eq!(ignores_json["summary"]["suppressed_diagnostics"], 1);
    assert_eq!(ignores_json["directives"][0]["line"], 2);
    assert_eq!(
        ignores_json["directives"][0]["suppressed"][0]["rule_id"],
        "local/no-denied-literals"
    );

    let stat = stdout_string(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(&example_dir)
            .args(["ignores", "--stat", "--no-cache"])
            .assert()
            .success(),
    );
    assert!(stat.contains("1 directives, 1 active"), "{stat}");
    assert!(stat.contains("By rule"), "{stat}");

    let check_shortstat = stdout_string(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(&example_dir)
            .args(["check", "--shortstat", "--no-cache", "--fail-on", "none"])
            .assert()
            .success(),
    );
    assert!(
        check_shortstat
            .contains("Scanned 1 file; 1 diagnostics; 1 suppressed by 1 ignore directives"),
        "{check_shortstat}"
    );

    let check_stat = stdout_string(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(&example_dir)
            .args(["check", "--stat", "--no-cache", "--fail-on", "none"])
            .assert()
            .success(),
    );
    assert!(check_stat.contains("By language"), "{check_stat}");
    assert!(check_stat.contains("  ts: 1"), "{check_stat}");
    assert!(check_stat.contains("Ignores"), "{check_stat}");

    let check_json_with_stats = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(&example_dir)
            .args([
                "check",
                "--format",
                "json",
                "--stat",
                "--shortstat",
                "--no-cache",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );
    assert_eq!(
        diagnostics_for_rule(&check_json_with_stats, "local/no-denied-literals").len(),
        1,
        "{check_json_with_stats:#?}"
    );
}

#[test]
fn check_with_local_rule_host_respects_positional_paths() {
    let example_dir = repo_root().join("examples/config-denied-literal");

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(&example_dir)
            .args([
                "check",
                "query.ts",
                "--format",
                "json",
                "--no-cache",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );
    assert_eq!(
        diagnostic_files(&json, "local/no-denied-literals"),
        vec!["query.ts".to_string()]
    );

    let empty = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(&example_dir)
            .args([
                "check",
                "README.md",
                "--format",
                "json",
                "--no-cache",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );
    assert!(diagnostics(&empty).is_empty());
}

#[test]
fn checked_in_multiple_rules_example_uses_one_rule_pack_crate() {
    let example_dir = repo_root().join("examples/multiple-rules");
    assert!(
        example_dir.join("README.md").exists(),
        "multiple-rules should document the rule-pack structure"
    );
    assert!(
        example_dir.join(".polint.toml").exists(),
        "multiple-rules should be runnable as a mini repository"
    );
    assert!(
        example_dir.join(".polint/rules/Cargo.toml").exists(),
        "multiple-rules should use one Cargo manifest for its rule pack"
    );
    assert!(
        example_dir
            .join(".polint/rules/src/no_raw_colors.rs")
            .exists(),
        "multiple-rules should keep no-raw-colors in its own module"
    );
    assert!(
        example_dir
            .join(".polint/rules/src/go_import_boundaries.rs")
            .exists(),
        "multiple-rules should keep go-import-boundaries in its own module"
    );
    assert!(
        !example_dir
            .join(".polint/rules/no-raw-colors/Cargo.toml")
            .exists(),
        "multiple-rules should not create one Cargo package per rule"
    );

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(&example_dir)
            .args([
                "check",
                "--format",
                "json",
                "--no-cache",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    let actual_rules = diagnostics(&json)
        .iter()
        .filter_map(|diagnostic| diagnostic["rule_id"].as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        actual_rules,
        std::collections::BTreeSet::from(["local/go-import-boundaries", "local/no-raw-colors"]),
        "multiple-rules should emit diagnostics from both registered local rules: {json:#?}"
    );
    for file in ["Button.tsx", "handler.go"] {
        assert!(
            diagnostics(&json)
                .iter()
                .any(|diagnostic| diagnostic["file"] == file),
            "multiple-rules should report a diagnostic in {file}: {json:#?}"
        );
    }
}

#[test]
fn check_with_unknown_profile_is_an_error() {
    let example_dir = repo_root().join("examples/config-denied-literal");

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(&example_dir)
        .args(["check", "--profile", "missing", "--fail-on", "none"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "profile `missing` is not defined in .polint.toml",
        ));
}

#[test]
fn check_rejects_unknown_config_flag() {
    Command::cargo_bin("polint")
        .unwrap()
        .args(["check", "--config"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument '--config'"));
}

#[test]
fn check_ts_no_raw_colors_dedupes_real_jsx_attribute_literal() {
    let temp = tempfile::tempdir().unwrap();
    write_file(
        &temp.path().join(".polint.toml"),
        r##"
[profiles.phase6]
rules = ["local/no-raw-colors"]

[[rules.config]]
id = "local/no-raw-colors"
files = ["**/*.tsx"]
"##,
    );
    write_file(
        &temp.path().join("Button.tsx"),
        r##"
export function Button() {
  return <button data-color="#00ff00">Pay</button>;
}
"##,
    );

    let json = stdout_json(
        raw_color_rule_cmd()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase6",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    let raw_color_diagnostics = diagnostics_for_rule(&json, "local/no-raw-colors");
    assert_eq!(
        raw_color_diagnostics.len(),
        1,
        "JSX attribute literal should be reported once: {raw_color_diagnostics:#?}"
    );
    assert!(diagnostic_has_evidence(
        raw_color_diagnostics[0],
        "literal",
        "#00ff00"
    ));
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

    assert_eq!(diagnostics(&json).len(), 0);
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
rules = ["local/no-raw-colors"]

[[rules.config]]
id = "local/no-raw-colors"
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
        raw_color_rule_cmd()
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

    let diagnostic = diagnostics(&json)
        .iter()
        .find(|diagnostic| diagnostic["rule_id"] == "local/no-raw-colors")
        .unwrap();
    assert_eq!(diagnostic["severity"], "info");
}

mod capability_planning {
    use super::*;

    #[test]
    fn check_reports_unsupported_reserved_capability() {
        let temp = tempfile::tempdir().unwrap();
        write_plan_capability_rule_repo(temp.path());

        let json = stdout_json(
            Command::cargo_bin("polint")
                .unwrap()
                .current_dir(temp.path())
                .args(["check", "--format", "json", "--fail-on", "none"])
                .assert()
                .success(),
        );

        let diagnostic = diagnostics(&json)
            .iter()
            .find(|diagnostic| diagnostic["rule_id"] == "polint/capability")
            .unwrap_or_else(|| panic!("expected capability diagnostic: {json:#?}"));
        assert!(
            diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("unsupported capability `cfg`")),
            "{diagnostic:#?}"
        );
        assert!(
            diagnostic["help"]
                .as_str()
                .is_some_and(|help| help.contains("docs/roadmap/00_ROADMAP.md")),
            "{diagnostic:#?}"
        );
        assert!(diagnostic_has_evidence(
            diagnostic,
            "rule",
            "local/needs-cfg"
        ));
    }

    #[test]
    fn check_only_rule_keeps_selected_capability_diagnostic() {
        let temp = tempfile::tempdir().unwrap();
        write_plan_capability_rule_repo(temp.path());

        let json = stdout_json(
            Command::cargo_bin("polint")
                .unwrap()
                .current_dir(temp.path())
                .args([
                    "check",
                    "--format",
                    "json",
                    "--only-rule",
                    "local/needs-cfg",
                ])
                .assert()
                .failure(),
        );

        let diagnostics = diagnostics(&json);
        assert_eq!(diagnostics.len(), 1, "{json:#?}");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic["rule_id"], "polint/capability");
        assert!(
            diagnostic["message"]
                .as_str()
                .is_some_and(|message| message.contains("unsupported capability `cfg`")),
            "{diagnostic:#?}"
        );
        assert!(diagnostic_has_evidence(
            diagnostic,
            "rule",
            "local/needs-cfg"
        ));
    }

    #[test]
    fn capability_change_changes_cache_entries() {
        let temp = tempfile::tempdir().unwrap();
        write_cache_capability_rule_repo(temp.path());

        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args(["check", "--format", "json", "--fail-on", "none"])
            .assert()
            .success();
        let first = cache_json_file_names(temp.path());
        assert!(!first.is_empty(), "first check should write cache files");

        let rule_path = temp.path().join(".polint/rules/src/main.rs");
        let rule_source = fs::read_to_string(&rule_path).unwrap();
        fs::write(
            &rule_path,
            rule_source
                .replace("literals: StringLiterals<'_>", "jsx: JsxAttributes<'_>")
                .replace("literals.iter().count()", "jsx.iter().count()"),
        )
        .unwrap();

        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args(["check", "--format", "json", "--fail-on", "none"])
            .assert()
            .success();
        let second = cache_json_file_names(temp.path());

        assert!(
            second.difference(&first).next().is_some(),
            "changing only requested capabilities should create new cache entries\nfirst={first:#?}\nsecond={second:#?}"
        );
    }
}

#[test]
fn check_max_diagnostics_truncates_sorted_json_in_example_repo() {
    let root = repo_root().join("examples/ts-design-tokens");
    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(&root)
            .args([
                "check",
                "--format",
                "json",
                "--fail-on",
                "none",
                "--max-diagnostics",
                "1",
            ])
            .assert()
            .success(),
    );
    assert_eq!(diagnostics(&json).len(), 1);
}

#[test]
fn check_max_diagnostics_does_not_hide_failing_diagnostics() {
    let root = repo_root().join("examples/ts-design-tokens");
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(&root)
        .args([
            "check",
            "--format",
            "json",
            "--fail-on",
            "error",
            "--max-diagnostics",
            "0",
        ])
        .assert()
        .failure();
}

#[test]
fn check_only_rule_filters_out_diagnostics_when_pattern_matches_nothing() {
    let root = repo_root().join("examples/ts-design-tokens");
    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(&root)
            .args([
                "check",
                "--format",
                "json",
                "--fail-on",
                "none",
                "--only-rule",
                "absent/nope",
            ])
            .assert()
            .success(),
    );
    assert!(diagnostics(&json).is_empty());
}

#[test]
fn init_new_rule_and_check_json_smoke() {
    let temp = tempfile::tempdir().unwrap();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .arg("init")
        .assert()
        .success();
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["new-rule", "generic", "agent-smoke"])
        .assert()
        .success();
    point_generated_rule_pack_at_local_polint(temp.path());
    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args(["check", "--format", "json", "--fail-on", "none"])
        .assert()
        .success();
}

#[test]
fn cookbook_golden_empty_report_fragment_is_valid_polint_json() {
    let raw = include_str!("fixtures/cookbook-empty-report-fragment.json");
    let report: polint::sdk::prelude::PolintReport = serde_json::from_str(raw).unwrap();
    assert_eq!(report.version, 1);
    assert!(report.diagnostics.is_empty());
}

#[test]
fn rules_json_stdout_should_be_parseable_json() {
    let temp = tempfile::tempdir().unwrap();
    write_phase8_raw_color_fixture(temp.path(), "error");

    let json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase8",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    assert_eq!(diagnostics(&json).len(), 0);
}

#[test]
fn check_fail_on_warn_error_none_exit_codes_are_stable() {
    let temp = tempfile::tempdir().unwrap();
    write_phase8_raw_color_fixture(temp.path(), "warn");

    raw_color_rule_cmd()
        .current_dir(temp.path())
        .args([
            "check",
            "--profile",
            "phase8",
            "--format",
            "json",
            "--fail-on",
            "warn",
        ])
        .assert()
        .failure()
        .code(1);

    raw_color_rule_cmd()
        .current_dir(temp.path())
        .args([
            "check",
            "--profile",
            "phase8",
            "--format",
            "json",
            "--fail-on",
            "error",
        ])
        .assert()
        .success();

    raw_color_rule_cmd()
        .current_dir(temp.path())
        .args([
            "check",
            "--profile",
            "phase8",
            "--format",
            "json",
            "--fail-on",
            "none",
        ])
        .assert()
        .success();
}

#[test]
fn fatal_config_parse_error_exits_two() {
    let temp = tempfile::tempdir().unwrap();
    write_file(&temp.path().join(".polint.toml"), "[profiles.phase8\n");

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .arg("check")
        .assert()
        .failure()
        .code(2);
}

#[test]
fn sarif_no_cache_and_rule_paths_are_supported() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(".polint.toml"),
        r#"
[rules]
paths = [".polint/rules", "tools/policy-rules"]

[profiles.phase2]
rules = ["local/no-raw-colors"]
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("component.tsx"),
        "export const color = \"#ff00aa\";",
    )
    .unwrap();

    let sarif = stdout_json(
        raw_color_rule_cmd()
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
    assert!(sarif.pointer("/runs/0/tool/driver/rules").is_some());
    assert_eq!(
        sarif.pointer("/runs/0/results/0/ruleId").unwrap(),
        "local/no-raw-colors"
    );
}

#[test]
fn sarif_output_includes_ci_fields_and_honors_fail_threshold() {
    let temp = tempfile::tempdir().unwrap();
    write_phase8_raw_color_fixture(temp.path(), "warn");

    let sarif = stdout_json(
        raw_color_rule_cmd()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase8",
                "--format",
                "sarif",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    assert_eq!(sarif.pointer("/version").unwrap(), "2.1.0");
    assert_eq!(sarif.pointer("/runs/0/tool/driver/name").unwrap(), "polint");
    assert!(sarif.pointer("/runs/0/tool/driver/rules").is_some());
    assert_eq!(
        sarif.pointer("/runs/0/results/0/ruleId").unwrap(),
        "local/no-raw-colors"
    );
    assert!(
        sarif
            .pointer("/runs/0/results/0/message/text")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|message| message.contains("Raw color"))
    );
    assert!(
        sarif
            .pointer("/runs/0/results/0/fingerprints/polint~1v1")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|fingerprint| !fingerprint.is_empty())
    );
    assert_eq!(
        sarif
            .pointer("/runs/0/results/0/locations/0/physicalLocation/artifactLocation/uri")
            .unwrap(),
        "component.tsx"
    );

    raw_color_rule_cmd()
        .current_dir(temp.path())
        .args([
            "check",
            "--profile",
            "phase8",
            "--format",
            "sarif",
            "--fail-on",
            "warn",
        ])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn check_no_cache_does_not_create_cache_directory() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(".polint.toml"),
        r##"
[profiles.phase7]
rules = ["local/no-raw-colors"]
"##,
    )
    .unwrap();
    fs::write(
        temp.path().join("component.tsx"),
        "export const color = \"#ff00aa\";",
    )
    .unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args([
            "check",
            "--profile",
            "phase7",
            "--format",
            "json",
            "--no-cache",
            "--fail-on",
            "none",
        ])
        .assert()
        .success();

    assert!(!temp.path().join(".polint/cache").exists());
}

#[test]
fn check_no_cache_bypasses_cache_writes() {
    let temp = tempfile::tempdir().unwrap();
    fs::write(
        temp.path().join(".polint.toml"),
        r##"
[profiles.phase7]
rules = ["local/no-raw-colors"]
"##,
    )
    .unwrap();
    fs::write(
        temp.path().join("component.tsx"),
        "export const color = \"#ff00aa\";",
    )
    .unwrap();

    Command::cargo_bin("polint")
        .unwrap()
        .current_dir(temp.path())
        .args([
            "check",
            "--profile",
            "phase7",
            "--format",
            "json",
            "--no-cache",
            "--fail-on",
            "none",
        ])
        .assert()
        .success();

    assert_eq!(cache_json_count(temp.path()), 0);
}

fn write_phase7_cache_fixture(root: &Path) {
    fs::write(
        root.join(".polint.toml"),
        r##"
[profiles.phase7]
rules = ["local/no-raw-colors", "local/go-cyclomatic-complexity"]

[[rules.config]]
id = "local/no-raw-colors"
files = ["**/*.tsx"]
"##,
    )
    .unwrap();
    fs::write(
        root.join("component.tsx"),
        r##"
import { token } from "./tokens";

export function Button() {
  return <div data-color="#ff00aa">{token}</div>;
}
"##,
    )
    .unwrap();
    fs::write(
        root.join("payment.go"),
        r#"
package payment

func Authorize(user string) bool {
    return user != ""
}
"#,
    )
    .unwrap();
}

#[test]
fn check_cache_writes_fact_metadata() {
    let temp = tempfile::tempdir().unwrap();
    write_phase7_cache_fixture(temp.path());

    let _json = stdout_json(
        Command::cargo_bin("polint")
            .unwrap()
            .current_dir(temp.path())
            .args([
                "check",
                "--profile",
                "phase7",
                "--format",
                "json",
                "--fail-on",
                "none",
            ])
            .assert()
            .success(),
    );

    assert!(cache_json_count(temp.path()) >= 2);
    let cache_entry = fs::read_to_string(
        fs::read_dir(temp.path().join(".polint/cache"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path(),
    )
    .unwrap();
    assert!(!cache_entry.contains("export function Button"));
    assert!(!cache_entry.contains("package payment"));
}

#[test]
fn check_cached_output_is_deterministic_across_repeated_runs() {
    let temp = tempfile::tempdir().unwrap();
    write_phase7_cache_fixture(temp.path());
    let run = || {
        stdout_string(
            Command::cargo_bin("polint")
                .unwrap()
                .current_dir(temp.path())
                .args([
                    "check",
                    "--profile",
                    "phase7",
                    "--format",
                    "json",
                    "--fail-on",
                    "none",
                ])
                .assert()
                .success(),
        )
    };

    let first = run();
    let second = run();
    let third = run();

    assert_eq!(second, first);
    assert_eq!(third, first);
    assert!(cache_json_count(temp.path()) >= 2);
}

#[test]
fn check_parallel_cached_output_is_deterministic_across_repeated_runs() {
    let temp = tempfile::tempdir().unwrap();
    write_phase7_cache_fixture(temp.path());
    let run = || {
        stdout_string(
            Command::cargo_bin("polint")
                .unwrap()
                .current_dir(temp.path())
                .args([
                    "check",
                    "--profile",
                    "phase7",
                    "--format",
                    "json",
                    "--fail-on",
                    "none",
                ])
                .assert()
                .success(),
        )
    };

    let first = run();
    let second = run();
    let third = run();

    assert_eq!(second, first);
    assert_eq!(third, first);
    assert!(cache_json_count(temp.path()) >= 2);
}

#[test]
fn check_no_cache_bypasses_cache_reads_and_writes() {
    let temp = tempfile::tempdir().unwrap();
    write_phase7_cache_fixture(temp.path());
    let run = |no_cache: bool| {
        let mut args = vec![
            "check",
            "--profile",
            "phase7",
            "--format",
            "json",
            "--fail-on",
            "none",
        ];
        if no_cache {
            args.push("--no-cache");
        }
        stdout_string(
            Command::cargo_bin("polint")
                .unwrap()
                .current_dir(temp.path())
                .args(args)
                .assert()
                .success(),
        )
    };

    let cold = run(false);
    let cache_files_after_cold = cache_json_count(temp.path());
    let warm = run(false);
    let no_cache = run(true);
    let cache_files_after_no_cache = cache_json_count(temp.path());

    assert!(cache_files_after_cold >= 2);
    assert_eq!(warm, cold);
    assert_eq!(no_cache, cold);
    assert_eq!(cache_files_after_no_cache, cache_files_after_cold);
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
rules = ["local/no-raw-colors"]

[[rules.config]]
id = "local/no-raw-colors"
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
        raw_color_rule_cmd()
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

    let files = diagnostic_files(&json, "local/no-raw-colors");
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
rules = ["local/no-raw-colors"]
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
        raw_color_rule_cmd()
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

    let files = diagnostic_files(&json, "local/no-raw-colors");
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
        raw_color_rule_cmd()
            .current_dir(temp.path())
            .args(["check", "--format", "json", "--fail-on", "none"])
            .assert()
            .success(),
    );

    let files = diagnostic_files(&json, "local/no-raw-colors");
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
rules = ["local/no-raw-colors"]
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
        diagnostic_files(&first, "local/no-raw-colors"),
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
        raw_color_rule_cmd()
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
