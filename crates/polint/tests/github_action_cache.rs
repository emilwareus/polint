//! `action.yml` is the published CI contract for polint's cache layout.
//!
//! Two properties have to hold together, and neither is visible from the Rust
//! code alone:
//!
//! 1. The analysis-side cache directories (`analysis`, `layers`, `derived`,
//!    `semantic-store`) are restored under a key that pins the polint version
//!    and the config/rule inputs. polint re-validates every artifact in them
//!    against current sources, so a restore can only ever save work, never
//!    substitute for validation.
//! 2. `rules-target` holds Cargo output for repo-local rule hosts. It is a
//!    compiler cache, restored under a key built from compiler inputs, and the
//!    action recompiles every rule package after restoring it so no cached
//!    rule-host binary can outlive the sources it was built from.
//!
//! Mixing the two - caching compiler output under the analysis key, widening a
//! restore-key fallback, or dropping the forced rebuild - is the failure this
//! file exists to catch. `crate::cache` pins the directory names these
//! assertions use; see `cache_layout_matches_the_github_action_contract`.

use std::fs;
use std::path::{Path, PathBuf};

use serde_norway::Value;

/// Cache directories whose contents polint validates against current sources.
/// Must match `CacheLayout`'s analysis-side directory names.
const ANALYSIS_CACHE_DIRS: &[&str] = &["analysis", "layers", "derived", "semantic-store"];

/// The only directory that may hold compiler output.
/// Must match `CacheLayout::rules_target_dir`.
const BUILD_CACHE_DIR: &str = "rules-target";

const RESOLVE_STEP: &str = "Resolve polint cache inputs";
const ANALYSIS_RESTORE_STEP: &str = "Restore polint analysis cache";
const ANALYSIS_SAVE_STEP: &str = "Save polint analysis cache";
const BUILD_RESTORE_STEP: &str = "Restore polint rule-host build cache";
const BUILD_SAVE_STEP: &str = "Save polint rule-host build cache";
const FORCE_REBUILD_STEP: &str = "Force a rebuild of repo-local rule sources";
const RUN_STEP: &str = "Run polint";

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("canonical repo root")
}

fn action() -> Value {
    let raw = fs::read_to_string(repo_root().join("action.yml")).expect("read action.yml");
    serde_norway::from_str(&raw).expect("action.yml parses as YAML")
}

fn steps(action: &Value) -> &[Value] {
    action["runs"]["steps"]
        .as_sequence()
        .expect("composite action has steps")
}

fn step_index(steps: &[Value], name: &str) -> usize {
    steps
        .iter()
        .position(|step| step["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("action.yml has no step named {name:?}"))
}

fn step<'a>(steps: &'a [Value], name: &str) -> &'a Value {
    &steps[step_index(steps, name)]
}

fn text<'a>(step: &'a Value, key: &str) -> &'a str {
    step[key]
        .as_str()
        .unwrap_or_else(|| panic!("step field {key:?} is not a string: {step:?}"))
}

/// Cache paths are a newline-separated block; a single path is a plain scalar.
fn cache_paths(step: &Value) -> Vec<String> {
    text(&step["with"], "path")
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

fn restore_keys(step: &Value) -> Vec<String> {
    step["with"]["restore-keys"]
        .as_str()
        .map(|keys| {
            keys.lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn expected_analysis_paths() -> Vec<String> {
    ANALYSIS_CACHE_DIRS
        .iter()
        .map(|dir| format!("${{{{ inputs.working-directory }}}}/.polint/cache/{dir}"))
        .collect()
}

#[test]
fn analysis_cache_covers_every_validated_directory_and_no_compiler_output() {
    let action = action();
    let steps = steps(&action);

    for name in [ANALYSIS_RESTORE_STEP, ANALYSIS_SAVE_STEP] {
        let paths = cache_paths(step(steps, name));
        assert_eq!(
            paths,
            expected_analysis_paths(),
            "{name} must cache exactly the source-validated cache directories"
        );
        assert!(
            !paths.iter().any(|path| path.contains(BUILD_CACHE_DIR)),
            "{name} must not treat compiler output as an analysis artifact"
        );
    }
}

#[test]
fn analysis_cache_key_pins_the_polint_version_and_current_rule_inputs() {
    let action = action();
    let steps = steps(&action);
    let restore = step(steps, ANALYSIS_RESTORE_STEP);
    let key = text(&restore["with"], "key");

    for required in [
        "inputs.cache-key-prefix",
        "runner.os",
        "steps.install.outputs.version",
        "/.polint.toml",
        "/.polint/rules/Cargo.toml",
        "/.polint/rules/src/**/*.rs",
    ] {
        assert!(
            key.contains(required),
            "analysis cache key must include {required}: {key}"
        );
    }

    // A fallback that crosses polint versions can only restore entries polint
    // will reject anyway - its own artifact keys carry the version - so the
    // only fallback allowed is the version-scoped one.
    let fallbacks = restore_keys(restore);
    assert_eq!(
        fallbacks.len(),
        1,
        "analysis cache should keep exactly one fallback: {fallbacks:?}"
    );
    assert!(
        fallbacks[0].contains("steps.install.outputs.version"),
        "analysis fallback must stay scoped to the installed polint version: {fallbacks:?}"
    );
    assert!(
        key.starts_with(&fallbacks[0]),
        "analysis fallback must be a prefix of the primary key: {fallbacks:?}"
    );
}

#[test]
fn rule_host_build_cache_is_its_own_entry_keyed_on_compiler_inputs() {
    let action = action();
    let steps = steps(&action);
    let expected_path =
        format!("${{{{ inputs.working-directory }}}}/.polint/cache/{BUILD_CACHE_DIR}");

    for name in [BUILD_RESTORE_STEP, BUILD_SAVE_STEP] {
        assert_eq!(
            cache_paths(step(steps, name)),
            vec![expected_path.clone()],
            "{name} must cache only the rule-host target directory"
        );
    }

    let key = text(&step(steps, BUILD_RESTORE_STEP)["with"], "key");
    for required in [
        "inputs.cache-key-prefix",
        "rules-build-v1",
        "runner.os",
        "runner.arch",
        "steps.install.outputs.version",
        "steps.cache-inputs.outputs.env-digest",
        "steps.cache-inputs.outputs.deps-digest",
    ] {
        assert!(
            key.contains(required),
            "rule-host build cache key must include {required}: {key}"
        );
    }
}

#[test]
fn rule_host_build_cache_fallback_only_relaxes_the_dependency_digest() {
    let action = action();
    let steps = steps(&action);
    let restore = step(steps, BUILD_RESTORE_STEP);
    let key = text(&restore["with"], "key");
    let fallbacks = restore_keys(restore);

    assert_eq!(
        fallbacks.len(),
        1,
        "the build cache must not stack broad fallbacks: {fallbacks:?}"
    );
    let fallback = &fallbacks[0];
    assert!(
        key.starts_with(fallback.as_str()),
        "build fallback must be a prefix of the primary key: {fallback}"
    );
    assert!(
        fallback.ends_with("-deps-"),
        "the dependency digest must be the only relaxed component: {fallback}"
    );
    for pinned in [
        "runner.os",
        "runner.arch",
        "steps.install.outputs.version",
        "steps.cache-inputs.outputs.env-digest",
    ] {
        assert!(
            fallback.contains(pinned),
            "build fallback must still pin {pinned}: {fallback}"
        );
    }
}

#[test]
fn restored_rule_sources_are_always_recompiled_before_polint_runs() {
    let action = action();
    let steps = steps(&action);

    let restore = step_index(steps, BUILD_RESTORE_STEP);
    let rebuild = step_index(steps, FORCE_REBUILD_STEP);
    let run = step_index(steps, RUN_STEP);
    assert!(
        restore < rebuild && rebuild < run,
        "the forced rebuild must sit between the build-cache restore and the polint run"
    );

    // The guarantee is that no restored rule-host binary is ever reused, so it
    // must not be conditional on how the cache was restored.
    let condition = text(step(steps, FORCE_REBUILD_STEP), "if");
    assert!(
        !condition.contains("cache-hit"),
        "the forced rebuild must not depend on the restore outcome: {condition}"
    );
    assert!(
        condition.contains("steps.cache-inputs.outputs.build-cache == 'true'"),
        "the forced rebuild must run whenever the build cache is in play: {condition}"
    );
}

#[test]
fn cache_entries_are_saved_only_when_their_own_key_missed() {
    let action = action();
    let steps = steps(&action);

    for (name, hit) in [
        (ANALYSIS_SAVE_STEP, "steps.cache-restore.outputs.cache-hit"),
        (
            BUILD_SAVE_STEP,
            "steps.rule-build-restore.outputs.cache-hit",
        ),
    ] {
        let condition = text(step(steps, name), "if");
        assert!(
            condition.contains(&format!("{hit} != 'true'")),
            "{name} must skip saving after an exact hit: {condition}"
        );
        assert!(
            condition.contains("always()"),
            "{name} must run even when polint reported findings: {condition}"
        );
    }
}

#[cfg(unix)]
mod resolve_script {
    use super::*;
    use std::collections::BTreeMap;
    use std::process::Command;

    struct Fixture {
        temp: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().expect("temp repo");
            let root = temp.path();
            write(
                &root.join(".polint.toml"),
                "[rules]\npaths = [\".polint/rules\"]\n",
            );
            write(
                &root.join(".polint/rules/Cargo.toml"),
                "[package]\nname = \"rules\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
            );
            write(&root.join(".polint/rules/src/main.rs"), "fn main() {}\n");
            Self { temp }
        }

        fn path(&self) -> &Path {
            self.temp.path()
        }

        /// Runs the resolve step's script the way the action does and returns
        /// the `GITHUB_OUTPUT` it emitted.
        fn resolve(&self, env: &[(&str, &str)]) -> BTreeMap<String, String> {
            let script = self.temp.path().join("resolve.sh");
            write(&script, &resolve_script_source());
            let output_file = self.temp.path().join("github-output");
            write(&output_file, "");
            let summary_file = self.temp.path().join("step-summary");
            write(&summary_file, "");

            let mut command = Command::new("bash");
            command
                .arg(&script)
                .current_dir(self.path())
                .env("GITHUB_OUTPUT", &output_file)
                .env("GITHUB_STEP_SUMMARY", &summary_file)
                .env("RUNNER_OS", "Linux")
                .env("RUNNER_ARCH", "X64")
                .env("POLINT_ACTION_CACHE_RULE_BUILDS", "true")
                .env("POLINT_ACTION_RULE_PATHS", "");
            for key in [
                "POLINT_CACHE_DIR",
                "POLINT_RULES_TARGET_DIR",
                "POLINT_RULES_PROFILE",
                "POLINT_RULES_TOOLCHAIN",
                "RUSTFLAGS",
                "CARGO_ENCODED_RUSTFLAGS",
            ] {
                command.env_remove(key);
            }
            for (key, value) in env {
                command.env(key, value);
            }

            let result = command.output().expect("run resolve script");
            assert!(
                result.status.success(),
                "resolve script failed: {}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            parse_github_output(&fs::read_to_string(&output_file).expect("read outputs"))
        }
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, contents).expect("write fixture file");
    }

    fn resolve_script_source() -> String {
        let action = action();
        text(step(steps(&action), RESOLVE_STEP), "run").to_string()
    }

    /// Understands both `key=value` lines and the heredoc form used for values
    /// that may span lines.
    fn parse_github_output(raw: &str) -> BTreeMap<String, String> {
        let mut parsed = BTreeMap::new();
        let mut lines = raw.lines();
        while let Some(line) = lines.next() {
            if line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                let (key, delimiter) = line
                    .split_once("<<")
                    .unwrap_or_else(|| panic!("unrecognized GITHUB_OUTPUT line: {line}"));
                let mut collected = Vec::new();
                for body in lines.by_ref() {
                    if body == delimiter {
                        break;
                    }
                    collected.push(body.to_string());
                }
                parsed.insert(key.to_string(), collected.join("\n"));
                continue;
            };
            parsed.insert(key.to_string(), value.to_string());
        }
        parsed
    }

    fn assert_digest(outputs: &BTreeMap<String, String>, key: &str) -> String {
        let digest = outputs
            .get(key)
            .unwrap_or_else(|| panic!("missing output {key}: {outputs:?}"));
        assert_eq!(
            digest.len(),
            64,
            "{key} should be a sha256 digest: {digest}"
        );
        assert!(
            digest.chars().all(|ch| ch.is_ascii_hexdigit()),
            "{key} should be hex: {digest}"
        );
        digest.clone()
    }

    #[test]
    fn every_action_script_is_valid_bash() {
        let action = action();
        for step in steps(&action) {
            let Some(script) = step["run"].as_str() else {
                continue;
            };
            let name = step["name"].as_str().unwrap_or("<unnamed>");
            let checked = Command::new("bash")
                .arg("-n")
                .arg("-c")
                .arg(script)
                .output()
                .expect("run bash -n");
            assert!(
                checked.status.success(),
                "step {name:?} is not valid bash: {}",
                String::from_utf8_lossy(&checked.stderr)
            );
        }
    }

    #[test]
    fn default_layout_emits_stable_build_cache_inputs() {
        let fixture = Fixture::new();
        let outputs = fixture.resolve(&[]);

        assert_eq!(
            outputs.get("analysis-cache").map(String::as_str),
            Some("true")
        );
        assert_eq!(outputs.get("build-cache").map(String::as_str), Some("true"));
        assert_eq!(
            outputs.get("rule-paths").map(String::as_str),
            Some(".polint/rules")
        );
        let env_digest = assert_digest(&outputs, "env-digest");
        let deps_digest = assert_digest(&outputs, "deps-digest");

        for dir in ANALYSIS_CACHE_DIRS.iter().chain([&BUILD_CACHE_DIR]) {
            assert!(
                fixture.path().join(".polint/cache").join(dir).is_dir(),
                "resolve step should create .polint/cache/{dir}"
            );
        }

        // Same inputs, same key: the digests cannot depend on run order,
        // timestamps, or directory iteration order.
        let repeated = fixture.resolve(&[]);
        assert_eq!(repeated.get("env-digest"), Some(&env_digest));
        assert_eq!(repeated.get("deps-digest"), Some(&deps_digest));
    }

    #[test]
    fn dependency_and_environment_changes_move_the_build_cache_key() {
        let fixture = Fixture::new();
        let baseline = fixture.resolve(&[]);
        let env_digest = assert_digest(&baseline, "env-digest");
        let deps_digest = assert_digest(&baseline, "deps-digest");

        write(
            &fixture.path().join(".polint/rules/Cargo.toml"),
            "[package]\nname = \"rules\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\npolint = \"0.2.0\"\n\n[workspace]\n",
        );
        let after_manifest = fixture.resolve(&[]);
        assert_ne!(
            after_manifest.get("deps-digest"),
            Some(&deps_digest),
            "a changed rule manifest must not reuse dependency builds"
        );
        assert_eq!(
            after_manifest.get("env-digest"),
            Some(&env_digest),
            "a changed manifest is not a changed build environment"
        );

        for (key, value) in [
            ("RUSTFLAGS", "-C target-cpu=native"),
            ("POLINT_RULES_PROFILE", "dev"),
        ] {
            let changed = fixture.resolve(&[(key, value)]);
            assert_ne!(
                changed.get("env-digest").map(String::as_str),
                Some(env_digest.as_str()),
                "{key} changes what the compiler emits and must move the key"
            );
        }

        // Rule source edits are deliberately absent from the build key: the
        // action recompiles rule packages on every run, so their compiled form
        // is never reused across sources.
        write(
            &fixture.path().join(".polint/rules/src/main.rs"),
            "fn main() { println!(\"changed\"); }\n",
        );
        let after_source = fixture.resolve(&[]);
        assert_eq!(
            after_source.get("deps-digest"),
            after_manifest.get("deps-digest"),
            "rule source edits are handled by the forced rebuild, not by the key"
        );
    }

    #[test]
    fn build_cache_is_skipped_when_its_assumptions_do_not_hold() {
        let fixture = Fixture::new();

        for (env, reason) in [
            (
                vec![("POLINT_RULES_TARGET_DIR", "/tmp/elsewhere")],
                "POLINT_RULES_TARGET_DIR",
            ),
            (
                vec![("POLINT_ACTION_CACHE_RULE_BUILDS", "false")],
                "cache-rule-builds",
            ),
        ] {
            let outputs = fixture.resolve(&env);
            assert_eq!(
                outputs.get("build-cache").map(String::as_str),
                Some("false"),
                "expected the build cache to be skipped for {reason}"
            );
            assert!(
                outputs
                    .get("build-cache-skipped")
                    .is_some_and(|skipped| skipped.contains(reason)),
                "skip reason should name {reason}: {outputs:?}"
            );
            assert_eq!(
                outputs.get("analysis-cache").map(String::as_str),
                Some("true"),
                "skipping compiler caching must not disable analysis caching"
            );
        }

        // A moved cache root is outside this action's contract entirely.
        let moved = fixture.resolve(&[("POLINT_CACHE_DIR", "/tmp/polint-cache")]);
        assert_eq!(
            moved.get("analysis-cache").map(String::as_str),
            Some("false")
        );
        assert_eq!(moved.get("build-cache").map(String::as_str), Some("false"));
    }

    #[test]
    fn non_default_rule_paths_require_an_explicit_input() {
        let fixture = Fixture::new();
        write(
            &fixture.path().join(".polint.toml"),
            "[rules]\npaths = [\".polint/rules\", \"tools/rules\"]\n",
        );

        let skipped = fixture.resolve(&[]);
        assert_eq!(
            skipped.get("build-cache").map(String::as_str),
            Some("false"),
            "an unrecognized [rules].paths must not silently cache a partial key"
        );

        write(
            &fixture.path().join("tools/rules/Cargo.toml"),
            "[package]\nname = \"more-rules\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
        );
        let configured =
            fixture.resolve(&[("POLINT_ACTION_RULE_PATHS", ".polint/rules\ntools/rules")]);
        assert_eq!(
            configured.get("build-cache").map(String::as_str),
            Some("true")
        );
        assert_eq!(
            configured.get("rule-paths").map(String::as_str),
            Some(".polint/rules\ntools/rules"),
            "every configured rule package must reach the forced-rebuild step"
        );
    }
}
