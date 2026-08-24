//! `action.yml` is the published CI contract for polint's cache layout.
//!
//! Three properties have to hold together, and none is visible from the Rust
//! code alone:
//!
//! 1. The analysis-side cache directories (`analysis`, `layers`, `derived`,
//!    `semantic-store`) are restored under a key that pins the polint version
//!    and the config/rule inputs. polint re-validates every artifact in them
//!    against current sources, so a restore can only ever save work, never
//!    substitute for validation.
//! 2. `rules-target` and `extensions-target` hold Cargo output. They are
//!    compiler caches, restored under a key built from compiler inputs, and the
//!    action removes every repo-local package's own output from them before
//!    saving, so no restored entry can carry a rule-host binary at all.
//! 3. A cache entry is only written after a run that finished. `actions/cache`
//!    never overwrites an existing key, so a torn target directory saved once
//!    is permanent for that key.
//!
//! Mixing the two roles - caching compiler output under the analysis key,
//! widening a restore-key fallback, saving after a failed build - is the failure
//! this file exists to catch. `crate::cache` pins the directory names and their
//! roles; see `cache_layout_matches_the_github_action_contract`.

use std::fs;
use std::path::{Path, PathBuf};

use serde_norway::Value;

/// Cache directories whose contents polint validates against current sources.
/// Must match the source-validated categories in `CacheLayout`.
const ANALYSIS_CACHE_DIRS: &[&str] = &["analysis", "layers", "derived", "semantic-store"];

/// The directories that may hold compiler output.
/// Must match the compiler-output categories in `CacheLayout`.
const BUILD_CACHE_DIRS: &[&str] = &["rules-target", "extensions-target"];

const RESOLVE_SCRIPT: &str = "scripts/action/resolve-cache-inputs.sh";
const PREPARE_SAVE_SCRIPT: &str = "scripts/action/prepare-build-cache-save.sh";

const RESOLVE_STEP: &str = "Resolve polint cache inputs";
const ANALYSIS_RESTORE_STEP: &str = "Restore polint analysis cache";
const ANALYSIS_SAVE_STEP: &str = "Save polint analysis cache";
const BUILD_RESTORE_STEP: &str = "Restore polint rule-host build cache";
const BUILD_SAVE_STEP: &str = "Save polint rule-host build cache";
const PREPARE_SAVE_STEP: &str = "Prepare the polint rule-host build cache for saving";
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

fn expected_paths(dirs: &[&str]) -> Vec<String> {
    dirs.iter()
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
            expected_paths(ANALYSIS_CACHE_DIRS),
            "{name} must cache exactly the source-validated cache directories"
        );
        for build_dir in BUILD_CACHE_DIRS {
            assert!(
                !paths.iter().any(|path| path.contains(build_dir)),
                "{name} must not treat compiler output as an analysis artifact"
            );
        }
    }
}

#[test]
fn analysis_cache_key_pins_the_polint_version_and_resolved_rule_inputs() {
    let action = action();
    let steps = steps(&action);
    let restore = step(steps, ANALYSIS_RESTORE_STEP);
    let key = text(&restore["with"], "key");

    for required in [
        "inputs.cache-key-prefix",
        "runner.os",
        "steps.install.outputs.version",
        // The digest is computed from the *resolved* rule packages, so a
        // repository with custom `[rules].paths` is covered too.
        "steps.cache-inputs.outputs.analysis-digest",
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

    for name in [BUILD_RESTORE_STEP, BUILD_SAVE_STEP] {
        assert_eq!(
            cache_paths(step(steps, name)),
            expected_paths(BUILD_CACHE_DIRS),
            "{name} must cache exactly the compiler-output directories"
        );
    }

    let key = text(&step(steps, BUILD_RESTORE_STEP)["with"], "key");
    for required in [
        "inputs.cache-key-prefix",
        "rules-build-v2",
        "runner.os",
        "runner.arch",
        "steps.cache-inputs.outputs.env-digest",
        "steps.cache-inputs.outputs.deps-digest",
    ] {
        assert!(
            key.contains(required),
            "rule-host build cache key must include {required}: {key}"
        );
    }

    // The installed CLI does not compile the rule host - the rule package's own
    // manifest and lockfile pin the `polint` library it links - so pinning the
    // CLI version here would only throw away reusable dependency builds on
    // every polint release.
    assert!(
        !key.contains("steps.install.outputs.version"),
        "the polint CLI version does not change compiled artifacts and must not partition them: {key}"
    );
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
        "steps.cache-inputs.outputs.env-digest",
    ] {
        assert!(
            fallback.contains(pinned),
            "build fallback must still pin {pinned}: {fallback}"
        );
    }
}

#[test]
fn the_build_cache_is_pruned_between_the_run_and_the_save() {
    let action = action();
    let steps = steps(&action);

    let restore = step_index(steps, BUILD_RESTORE_STEP);
    let run = step_index(steps, RUN_STEP);
    let prepare = step_index(steps, PREPARE_SAVE_STEP);
    let save = step_index(steps, BUILD_SAVE_STEP);
    assert!(
        restore < run && run < prepare && prepare < save,
        "the prune must observe the finished run and gate the save that follows it"
    );

    // The prune is what makes a restored entry unable to carry a rule host, so
    // it must not be conditional on how the cache was restored.
    let condition = text(step(steps, PREPARE_SAVE_STEP), "if");
    assert!(
        condition.contains("steps.cache-inputs.outputs.build-cache == 'true'"),
        "the prune must run whenever the build cache is in play: {condition}"
    );
}

#[test]
fn the_build_cache_is_saved_only_after_a_run_that_finished() {
    let action = action();
    let steps = steps(&action);
    let condition = text(step(steps, BUILD_SAVE_STEP), "if");

    assert!(
        condition.contains("steps.rule-build-save.outputs.save == 'true'"),
        "the build cache must only be saved when the prune step vouched for it: {condition}"
    );
    assert!(
        condition.contains("steps.rule-build-restore.outputs.cache-hit != 'true'"),
        "an exact hit is already stored under this key: {condition}"
    );

    // The prune step reads polint's exit code, so the gate itself has to see it.
    let prepare = step(steps, PREPARE_SAVE_STEP);
    assert_eq!(
        prepare["env"]["POLINT_ACTION_EXIT_CODE"].as_str(),
        Some("${{ steps.run-polint.outputs.exit-code }}"),
        "the save gate must be derived from the polint run, not from always()"
    );
}

#[test]
fn the_analysis_cache_is_saved_even_when_the_run_failed() {
    let action = action();
    let steps = steps(&action);
    let condition = text(step(steps, ANALYSIS_SAVE_STEP), "if");

    // Analysis artifacts are validated per entry on read, so a partial one costs
    // a miss, never a wrong answer. Keeping the save unconditional means a run
    // that ends in findings still warms the next one.
    assert!(
        condition.contains("always()"),
        "{ANALYSIS_SAVE_STEP} must run even when polint reported findings: {condition}"
    );
    assert!(
        condition.contains("steps.cache-restore.outputs.cache-hit != 'true'"),
        "{ANALYSIS_SAVE_STEP} must skip saving after an exact hit: {condition}"
    );
}

#[test]
fn the_action_runs_the_checked_in_cache_scripts() {
    let action = action();
    let steps = steps(&action);
    let root = repo_root();

    for (step_name, script) in [
        (RESOLVE_STEP, RESOLVE_SCRIPT),
        (PREPARE_SAVE_STEP, PREPARE_SAVE_SCRIPT),
    ] {
        let name = script.rsplit('/').next().expect("script file name");
        let run = text(step(steps, step_name), "run");
        assert!(run.contains(name), "{step_name} must run {script}: {run}");
        assert!(
            root.join(script).is_file(),
            "{script} is referenced by action.yml but missing from the repository"
        );
        assert_eq!(
            step(steps, step_name)["env"]["POLINT_ACTION_SCRIPT_DIR"].as_str(),
            Some("${{ github.action_path }}/scripts/action"),
            "{step_name} must resolve its script from the action checkout"
        );
    }
}

#[cfg(unix)]
mod scripts {
    use super::*;
    use std::collections::BTreeMap;
    use std::process::Command;

    struct Fixture {
        temp: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().expect("temp repo");
            let fixture = Self { temp };
            fixture.write(".polint.toml", "[rules]\npaths = [\".polint/rules\"]\n");
            fixture.write_package(".polint/rules", "rules");
            fixture
        }

        fn path(&self) -> &Path {
            self.temp.path()
        }

        fn write(&self, relative: &str, contents: &str) {
            write(&self.path().join(relative), contents);
        }

        fn write_package(&self, relative: &str, name: &str) {
            self.write(
                &format!("{relative}/Cargo.toml"),
                &format!(
                    "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n"
                ),
            );
            self.write(&format!("{relative}/src/main.rs"), "fn main() {}\n");
        }

        /// Runs the resolve script the way the action does and returns the
        /// `GITHUB_OUTPUT` it emitted.
        fn resolve(&self, env: &[(&str, &str)]) -> BTreeMap<String, String> {
            let output_file = self.path().join("github-output");
            write(&output_file, "");
            let summary_file = self.path().join("step-summary");
            write(&summary_file, "");

            let mut command = Command::new("bash");
            command
                .arg(repo_root().join(RESOLVE_SCRIPT))
                .current_dir(self.path())
                .env("GITHUB_OUTPUT", &output_file)
                .env("GITHUB_STEP_SUMMARY", &summary_file)
                .env("RUNNER_OS", "Linux")
                .env("RUNNER_ARCH", "X64")
                .env(
                    "POLINT_ACTION_SCRIPT_DIR",
                    repo_root().join("scripts/action"),
                )
                .env("POLINT_ACTION_STATE_DIR", self.path().join("state"))
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
                "resolve script must never fail the job: {}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            parse_github_output(&fs::read_to_string(&output_file).expect("read outputs"))
        }

        fn rule_packages(&self, outputs: &BTreeMap<String, String>) -> Vec<String> {
            let file = outputs
                .get("rule-packages-file")
                .expect("rule-packages-file output");
            fs::read_to_string(file)
                .expect("read covered package list")
                .lines()
                .map(str::to_string)
                .filter(|line| !line.is_empty())
                .collect()
        }
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, contents).expect("write fixture file");
    }

    /// GitHub outputs are `key=value` lines. The scripts deliberately emit no
    /// heredoc form: a fixed delimiter is forgeable by any value that contains
    /// it, so nothing multi-line is ever written here.
    fn parse_github_output(raw: &str) -> BTreeMap<String, String> {
        let mut parsed = BTreeMap::new();
        for line in raw.lines() {
            if line.is_empty() {
                continue;
            }
            let (key, value) = line
                .split_once('=')
                .unwrap_or_else(|| panic!("unrecognized GITHUB_OUTPUT line: {line}"));
            assert!(
                !key.contains("<<"),
                "outputs must not use a heredoc delimiter: {line}"
            );
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

    fn assert_skipped(outputs: &BTreeMap<String, String>, needle: &str) {
        assert_eq!(
            outputs.get("build-cache").map(String::as_str),
            Some("false"),
            "expected the build cache to be skipped for {needle}: {outputs:?}"
        );
        assert!(
            outputs
                .get("build-cache-skipped")
                .is_some_and(|skipped| skipped.contains(needle)),
            "skip reason should name {needle}: {outputs:?}"
        );
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

        for script in [RESOLVE_SCRIPT, PREPARE_SAVE_SCRIPT] {
            let checked = Command::new("bash")
                .arg("-n")
                .arg(repo_root().join(script))
                .output()
                .expect("run bash -n");
            assert!(
                checked.status.success(),
                "{script} is not valid bash: {}",
                String::from_utf8_lossy(&checked.stderr)
            );
        }
    }

    #[test]
    fn default_layout_emits_stable_cache_inputs() {
        let fixture = Fixture::new();
        let outputs = fixture.resolve(&[]);

        assert_eq!(
            outputs.get("analysis-cache").map(String::as_str),
            Some("true")
        );
        assert_eq!(outputs.get("build-cache").map(String::as_str), Some("true"));
        assert_eq!(fixture.rule_packages(&outputs), vec![".polint/rules"]);
        assert_eq!(
            outputs.get("rules-profile").map(String::as_str),
            Some("release"),
            "polint builds rule hosts in release unless POLINT_RULES_PROFILE says otherwise"
        );
        let env_digest = assert_digest(&outputs, "env-digest");
        let deps_digest = assert_digest(&outputs, "deps-digest");
        let analysis_digest = assert_digest(&outputs, "analysis-digest");

        for dir in ANALYSIS_CACHE_DIRS.iter().chain(BUILD_CACHE_DIRS.iter()) {
            assert!(
                fixture.path().join(".polint/cache").join(dir).is_dir(),
                "resolve step should create .polint/cache/{dir}"
            );
        }
        assert!(
            !fixture.path().join(".polint/cache/review").exists(),
            "review is per-run scratch and is not part of any cache entry"
        );

        // Same inputs, same key: the digests cannot depend on run order,
        // timestamps, or directory iteration order.
        let repeated = fixture.resolve(&[]);
        assert_eq!(repeated.get("env-digest"), Some(&env_digest));
        assert_eq!(repeated.get("deps-digest"), Some(&deps_digest));
        assert_eq!(repeated.get("analysis-digest"), Some(&analysis_digest));
    }

    #[test]
    fn dependency_and_environment_changes_move_the_build_cache_key() {
        let fixture = Fixture::new();
        let baseline = fixture.resolve(&[]);
        let env_digest = assert_digest(&baseline, "env-digest");
        let deps_digest = assert_digest(&baseline, "deps-digest");

        fixture.write(
            ".polint/rules/Cargo.toml",
            "[package]\nname = \"rules\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\npolint = \"0.2.0\"\n\n[workspace]\n",
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
        // rule packages are pruned out of every saved entry, so their compiled
        // form is never restored in the first place.
        fixture.write(".polint/rules/src/main.rs", "fn main() { let _ = 1; }\n");
        let after_source = fixture.resolve(&[]);
        assert_eq!(
            after_source.get("deps-digest"),
            after_manifest.get("deps-digest"),
            "rule source edits are handled by pruning, not by the build key"
        );
        assert_ne!(
            after_source.get("analysis-digest"),
            after_manifest.get("analysis-digest"),
            "rule source edits belong in the analysis key"
        );
    }

    /// The action used to line-scan for a literal `[rules]` header, which read
    /// every other valid spelling of the same key as "unset" and quietly cached
    /// a key that covered the wrong packages.
    #[test]
    fn configured_rule_paths_are_read_in_every_supported_toml_spelling() {
        let fixture = Fixture::new();
        fixture.write_package("tools/rules", "more-rules");

        for config in [
            "[rules]\npaths = [\".polint/rules\", \"tools/rules\"]\n",
            "rules.paths = [\".polint/rules\", \"tools/rules\"]\n",
            "rules = { paths = [\".polint/rules\", \"tools/rules\"] }\n",
            "[rules]\npaths = [\n  \".polint/rules\",\n  \"tools/rules\",\n]\n",
            "[rules] # paths = [\"decoy\"]\npaths = [\n  '.polint/rules', # first\n  'tools/rules',\n]\n",
        ] {
            fixture.write(".polint.toml", config);
            let outputs = fixture.resolve(&[]);
            assert_eq!(
                outputs.get("build-cache").map(String::as_str),
                Some("true"),
                "expected a resolvable config: {config:?} -> {outputs:?}"
            );
            assert_eq!(
                fixture.rule_packages(&outputs),
                vec![".polint/rules", "tools/rules"],
                "every configured rule package must reach the prune step: {config:?}"
            );
        }
    }

    #[test]
    fn a_rules_table_without_paths_still_means_the_default_package() {
        let fixture = Fixture::new();
        for config in [
            "[workspace]\ninclude = [\"a\"]\n",
            "[[rules.config]]\nid = \"local/x\"\nseverity = \"error\"\n",
            "rules = { config = [] }\n",
        ] {
            fixture.write(".polint.toml", config);
            let outputs = fixture.resolve(&[]);
            assert_eq!(
                fixture.rule_packages(&outputs),
                vec![".polint/rules"],
                "a config without [rules].paths uses polint's default: {config:?}"
            );
        }
    }

    /// A config shape the parser cannot decode byte for byte must not be read as
    /// "the default": it has to fail safe, keeping the analysis cache and
    /// skipping the entry whose key would then cover the wrong packages.
    #[test]
    fn an_undecodable_rules_paths_skips_only_the_build_cache() {
        let fixture = Fixture::new();
        for (config, reason) in [
            ("[rules]\npaths = [\"\"\"multi\"\"\"]\n", "multi-line"),
            ("[rules]\npaths = [\".polint\\\\rules\"]\n", "escapes"),
            ("[[rules]]\npaths = [\".polint/rules\"]\n", "[[rules]]"),
            ("[rules]\npaths = \".polint/rules\"\n", "array literal"),
            (
                "[rules]\npaths = [\".polint/rules\"]\n[other]\nx = 1\n[rules]\ny = 2\n",
                "duplicate",
            ),
        ] {
            fixture.write(".polint.toml", config);
            let outputs = fixture.resolve(&[]);
            assert_skipped(&outputs, reason);
            assert_eq!(
                outputs.get("analysis-cache").map(String::as_str),
                Some("true"),
                "an unreadable [rules].paths must not disable the validated cache: {config:?}"
            );
        }
    }

    /// Reading a config costs time proportional to its size, so there is a
    /// bound past which the answer is not worth waiting for. Past it the reader
    /// says so instead of stalling the step.
    #[test]
    fn an_oversized_config_is_refused_rather_than_parsed() {
        let fixture = Fixture::new();
        let mut config = String::from("[rules]\npaths = [\".polint/rules\"]\n");
        while config.len() < 200_000 {
            config.push_str("\n[[rules.config]]\nid = \"local/filler\"\nseverity = \"error\"\n");
        }
        fixture.write(".polint.toml", &config);

        let outputs = fixture.resolve(&[]);
        assert_skipped(&outputs, "too large");
        assert_eq!(
            outputs.get("analysis-cache").map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn an_empty_rules_paths_list_has_no_rule_package_to_cache() {
        let fixture = Fixture::new();
        fixture.write(".polint.toml", "[rules]\npaths = []\n");
        let outputs = fixture.resolve(&[]);
        assert_skipped(&outputs, "no repo-local rule package found");
    }

    #[test]
    fn rule_paths_that_leave_the_working_directory_are_refused() {
        let fixture = Fixture::new();
        fixture.write_package("tools/rules", "more-rules");

        // A real rule package that simply is not in this repository. Only the
        // physical path shows it: the symlink looks like any other entry.
        let outside = tempfile::tempdir().expect("outside package");
        write(
            &outside.path().join("Cargo.toml"),
            "[package]\nname = \"outside-rules\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
        );
        std::os::unix::fs::symlink(outside.path(), fixture.path().join("escape"))
            .expect("symlink outside the working directory");

        for (paths, reason) in [
            ("/etc", "not relative"),
            ("../outside", ".. component"),
            (".polint/rules/../../outside", ".. component"),
            ("~/rules", "not relative"),
            ("C:/rules", "not relative"),
            (".polint\\rules", "relative POSIX path"),
            ("escape", "outside the working directory"),
            ("tools", "no Cargo.toml"),
            ("does/not/exist", "not a directory"),
        ] {
            let outputs = fixture.resolve(&[("POLINT_ACTION_RULE_PATHS", paths)]);
            assert_skipped(&outputs, reason);
            assert_eq!(
                outputs.get("analysis-cache").map(String::as_str),
                Some("true"),
                "a refused rule path must not disable the validated cache"
            );
        }

        // Blank and whitespace-only entries are dropped rather than refused, so
        // a trailing separator in the input is not an error.
        let padded = fixture.resolve(&[(
            "POLINT_ACTION_RULE_PATHS",
            "  .polint/rules  ,\n\n   \n, tools/rules\n",
        )]);
        assert_eq!(
            padded.get("build-cache").map(String::as_str),
            Some("true"),
            "blank entries around valid paths are not an error: {padded:?}"
        );
        assert_eq!(
            fixture.rule_packages(&padded),
            vec![".polint/rules", "tools/rules"]
        );
    }

    #[test]
    fn every_configured_rule_package_must_carry_a_manifest() {
        let fixture = Fixture::new();
        fs::create_dir_all(fixture.path().join("tools/rules")).expect("create empty package dir");
        fixture.write(
            ".polint.toml",
            "[rules]\npaths = [\".polint/rules\", \"tools/rules\"]\n",
        );

        let outputs = fixture.resolve(&[]);
        assert_skipped(&outputs, "tools/rules has no Cargo.toml");
    }

    /// Pointing `POLINT_CACHE_DIR` at the layout the action already caches is
    /// not a move, and used to disable caching anyway.
    #[test]
    fn a_cache_dir_override_that_resolves_to_the_default_keeps_caching() {
        let fixture = Fixture::new();
        let absolute = fixture.path().join(".polint/cache");

        for value in [
            absolute.to_str().expect("utf-8 fixture path"),
            ".polint/cache",
            "./.polint/./cache",
            ".polint/cache/",
        ] {
            let outputs = fixture.resolve(&[("POLINT_CACHE_DIR", value)]);
            assert_eq!(
                outputs.get("analysis-cache").map(String::as_str),
                Some("true"),
                "POLINT_CACHE_DIR={value} resolves to the default layout: {outputs:?}"
            );
            assert_eq!(
                outputs.get("build-cache").map(String::as_str),
                Some("true"),
                "POLINT_CACHE_DIR={value} resolves to the default layout: {outputs:?}"
            );
        }
    }

    #[test]
    fn a_moved_cache_root_is_outside_the_action_contract() {
        let fixture = Fixture::new();
        let elsewhere = fixture.path().join("somewhere-else");
        let outputs = fixture.resolve(&[(
            "POLINT_CACHE_DIR",
            elsewhere.to_str().expect("utf-8 fixture path"),
        )]);

        assert_eq!(
            outputs.get("analysis-cache").map(String::as_str),
            Some("false")
        );
        assert_skipped(&outputs, "POLINT_CACHE_DIR moves the cache root");
    }

    #[test]
    fn a_rules_target_override_is_judged_against_the_cache_root() {
        let fixture = Fixture::new();

        for value in ["rules-target", "./rules-target"] {
            let outputs = fixture.resolve(&[("POLINT_RULES_TARGET_DIR", value)]);
            assert_eq!(
                outputs.get("build-cache").map(String::as_str),
                Some("true"),
                "POLINT_RULES_TARGET_DIR={value} is the directory the action already caches"
            );
        }

        let elsewhere = fixture.path().join("other-target");
        let outputs = fixture.resolve(&[(
            "POLINT_RULES_TARGET_DIR",
            elsewhere.to_str().expect("utf-8 fixture path"),
        )]);
        assert_skipped(&outputs, "POLINT_RULES_TARGET_DIR moves");
        assert_eq!(
            outputs.get("analysis-cache").map(String::as_str),
            Some("true"),
            "a moved rule-host target directory says nothing about analysis artifacts"
        );
    }

    #[test]
    fn opting_out_of_build_caching_keeps_the_analysis_cache() {
        let fixture = Fixture::new();
        let outputs = fixture.resolve(&[("POLINT_ACTION_CACHE_RULE_BUILDS", "false")]);

        assert_skipped(&outputs, "cache-rule-builds is not true");
        assert_eq!(
            outputs.get("analysis-cache").map(String::as_str),
            Some("true")
        );
    }

    #[test]
    fn repo_local_extension_packages_join_the_build_cache() {
        let fixture = Fixture::new();
        fixture.write_package(".polint/extensions/demo", "demo-extension");

        let outputs = fixture.resolve(&[]);
        let extensions = fs::read_to_string(
            outputs
                .get("extension-packages-file")
                .expect("extension-packages-file output"),
        )
        .expect("read extension package list");

        assert_eq!(extensions.trim(), ".polint/extensions/demo");
        assert_eq!(outputs.get("build-cache").map(String::as_str), Some("true"));
    }
}

/// The prune step is what makes a restored build cache unable to carry a rule
/// host. These tests build real crates, because the property only exists if
/// Cargo agrees: the rule package's own output must be gone and its dependency
/// units must survive.
#[cfg(unix)]
mod prune {
    use super::*;
    use std::collections::BTreeMap;
    use std::process::Command;

    fn cargo() -> String {
        std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string())
    }

    struct BuiltFixture {
        temp: tempfile::TempDir,
    }

    impl BuiltFixture {
        /// A rule package with a path dependency, built into the cache layout
        /// the action saves. The dependency stands in for the `polint` library
        /// and its tree: the expensive half that pruning must keep.
        fn new() -> Self {
            let temp = tempfile::tempdir().expect("temp repo");
            let root = temp.path();
            write(
                &root.join("vendor/rule-support/Cargo.toml"),
                "[package]\nname = \"rule-support\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
            );
            write(
                &root.join("vendor/rule-support/src/lib.rs"),
                "pub fn support() -> u8 { 7 }\n",
            );
            write(
                &root.join(".polint/rules/Cargo.toml"),
                "[package]\nname = \"repo-rules\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nrule-support = { path = \"../../vendor/rule-support\" }\n\n[workspace]\n",
            );
            write(
                &root.join(".polint/rules/src/main.rs"),
                "fn main() { println!(\"{}\", rule_support::support()); }\n",
            );

            let fixture = Self { temp };
            fixture.build();
            fixture
        }

        fn path(&self) -> &Path {
            self.temp.path()
        }

        fn target(&self) -> PathBuf {
            self.path().join(".polint/cache/rules-target")
        }

        fn build(&self) {
            let built = Command::new(cargo())
                .args([
                    "build",
                    "--release",
                    "--offline",
                    "--manifest-path",
                    ".polint/rules/Cargo.toml",
                ])
                .current_dir(self.path())
                .env("CARGO_TARGET_DIR", self.target())
                .output()
                .expect("run cargo build");
            assert!(
                built.status.success(),
                "fixture rule package must build: {}",
                String::from_utf8_lossy(&built.stderr)
            );
        }

        fn prepare_save(&self, exit_code: &str, max_size_mb: &str) -> BTreeMap<String, String> {
            let output_file = self.path().join("github-output");
            write(&output_file, "");
            let packages = self.path().join("rule-packages");
            write(&packages, ".polint/rules\n");
            let extensions = self.path().join("extension-packages");
            write(&extensions, "");

            let result = Command::new("bash")
                .arg(repo_root().join(PREPARE_SAVE_SCRIPT))
                .current_dir(self.path())
                .env("GITHUB_OUTPUT", &output_file)
                .env("GITHUB_STEP_SUMMARY", self.path().join("step-summary"))
                .env("POLINT_ACTION_EXIT_CODE", exit_code)
                .env("POLINT_ACTION_RULE_PACKAGES_FILE", &packages)
                .env("POLINT_ACTION_EXTENSION_PACKAGES_FILE", &extensions)
                .env("POLINT_ACTION_RULES_PROFILE", "release")
                .env("POLINT_ACTION_MAX_SIZE_MB", max_size_mb)
                .output()
                .expect("run prepare-build-cache-save script");
            assert!(
                result.status.success(),
                "the prune step must never fail the job: {}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            );
            parse_outputs(&fs::read_to_string(&output_file).expect("read outputs"))
        }

        fn artifacts_matching(&self, needle: &str) -> Vec<PathBuf> {
            let mut found = Vec::new();
            collect(&self.target(), &mut found);
            found.retain(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.contains(needle))
            });
            found
        }
    }

    fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                out.push(path.clone());
                collect(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, contents).expect("write fixture file");
    }

    fn parse_outputs(raw: &str) -> BTreeMap<String, String> {
        raw.lines()
            .filter(|line| !line.is_empty())
            .map(|line| {
                let (key, value) = line
                    .split_once('=')
                    .unwrap_or_else(|| panic!("unrecognized GITHUB_OUTPUT line: {line}"));
                (key.to_string(), value.to_string())
            })
            .collect()
    }

    #[test]
    fn pruning_removes_the_rule_host_and_keeps_its_dependency_builds() {
        let fixture = BuiltFixture::new();
        assert!(
            !fixture.artifacts_matching("repo-rules").is_empty()
                && !fixture.artifacts_matching("repo_rules").is_empty(),
            "the fixture build should produce rule-host output to prune"
        );
        let dependency_before = fixture.artifacts_matching("rule_support").len();
        assert!(
            dependency_before > 0,
            "the fixture build should produce dependency output to keep"
        );

        let outputs = fixture.prepare_save("1", "");

        assert_eq!(outputs.get("save").map(String::as_str), Some("true"));
        assert!(
            fixture.artifacts_matching("repo-rules").is_empty()
                && fixture.artifacts_matching("repo_rules").is_empty(),
            "no output built from repo-local rule sources may enter the cache: {:?}",
            fixture.artifacts_matching("repo")
        );
        assert_eq!(
            fixture.artifacts_matching("rule_support").len(),
            dependency_before,
            "dependency builds are the reason the cache exists and must survive"
        );
        assert!(
            fixture
                .artifacts_matching("incremental")
                .iter()
                .all(|path| !path.is_dir()),
            "incremental state only speeds up a recompile that always happens anyway"
        );

        // And Cargo agrees: the next build recompiles the rule package alone.
        fixture.build();
        assert!(
            !fixture.artifacts_matching("repo-rules").is_empty(),
            "the rule host is rebuilt from the sources in the checkout"
        );
    }

    #[test]
    fn a_run_that_did_not_finish_never_writes_a_cache_entry() {
        let fixture = BuiltFixture::new();

        for (exit_code, reason) in [
            ("2", "polint exited 2"),
            ("101", "polint exited 101"),
            ("", "polint did not run"),
        ] {
            let outputs = fixture.prepare_save(exit_code, "");
            assert_eq!(
                outputs.get("save").map(String::as_str),
                Some("false"),
                "exit code {exit_code:?} means the target directory describes an interrupted build"
            );
            assert_eq!(
                outputs.get("save-skipped").map(String::as_str),
                Some(reason)
            );
        }

        // Nothing was pruned either, because nothing is being saved.
        assert!(
            !fixture.artifacts_matching("repo-rules").is_empty(),
            "a refused save should leave the working directory alone"
        );
    }

    #[test]
    fn a_size_ceiling_refuses_the_save_and_still_reports_the_size() {
        let fixture = BuiltFixture::new();
        let outputs = fixture.prepare_save("0", "0");

        assert_eq!(outputs.get("save").map(String::as_str), Some("false"));
        assert!(
            outputs
                .get("save-skipped")
                .is_some_and(|reason| reason.contains("ceiling")),
            "the refusal must name the ceiling it hit: {outputs:?}"
        );
        assert!(
            outputs
                .get("size-mb")
                .is_some_and(|size| size.parse::<u64>().is_ok()),
            "the measured size is reported whether or not it is saved: {outputs:?}"
        );
    }
}
