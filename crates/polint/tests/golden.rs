//! Characterization harness: real CLI vs committed normalized goldens.
//!
//! Cases come from `tests/golden-corpus/inputs.toml`: each example rule pack is
//! paired with its parent example directory (`--format json`). Scale suite
//! checkouts are optional and skip loudly when missing. Set
//! `POLINT_UPDATE_GOLDENS=1` to rewrite goldens from current behaviour.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::Command as AssertCommand;
use serde_json::Value;

const INPUTS_REL: &str = "tests/golden-corpus/inputs.toml";
const GOLDEN_ROOT_REL: &str = "tests/golden";
const OUTPUTS_REL: &str = "tests/golden/outputs";
const UPDATE_ENV: &str = "POLINT_UPDATE_GOLDENS";
const STRIPPED_VERSION: &str = "<stripped>";
const FORMAT_JSON: &str = "json";
const EXPECTED_INPUTS_SCHEMA: &str = "polint-golden-corpus-inputs-1";

#[derive(Debug, Clone)]
struct GoldenCase {
    id: String,
    target_rel: String,
    format: &'static str,
    /// When true, a missing target directory skips the case with a stderr note.
    optional_target: bool,
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("polint crate should live under crates/")
        .to_path_buf()
}

fn polint_cmd() -> AssertCommand {
    let mut command = AssertCommand::cargo_bin("polint").unwrap();
    command.env(
        "CARGO_TARGET_DIR",
        repo_root().join("target/polint-cli-test-cargo"),
    );
    command.env(
        "POLINT_RULES_TARGET_DIR",
        repo_root().join("target/polint-cli-test-rules"),
    );
    command.env("POLINT_RULES_PROFILE", "dev");
    command
}

fn load_toml(path: &Path) -> toml::Value {
    let raw = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
    toml::from_str(&raw).unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
}

fn string_array<'a>(table: &'a toml::Value, key: &str, path: &Path) -> Vec<&'a str> {
    let Some(toml::Value::Array(items)) = table.get(key) else {
        panic!("{}: missing string array `{key}`", path.display());
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .unwrap_or_else(|| panic!("{}: `{key}` entries must be strings", path.display()))
        })
        .collect()
}

fn example_case_id(example_name: &str, format: &str) -> String {
    format!("examples/{example_name}/{format}")
}

fn scale_case_id(suite_id: &str, format: &str) -> String {
    format!("scale/{suite_id}/{format}")
}

fn golden_path(root: &Path, case_id: &str) -> PathBuf {
    root.join(OUTPUTS_REL).join(format!("{case_id}.json"))
}

fn discover_cases(root: &Path) -> Vec<GoldenCase> {
    let inputs_path = root.join(INPUTS_REL);
    let inputs = load_toml(&inputs_path);
    let schema = inputs
        .get("schema_version")
        .and_then(toml::Value::as_str)
        .unwrap_or_else(|| panic!("{}: missing schema_version", inputs_path.display()));
    assert_eq!(schema, EXPECTED_INPUTS_SCHEMA);

    let mut cases = Vec::new();

    for pack_rel in string_array(&inputs, "example_rule_packs", &inputs_path) {
        let pack = Path::new(pack_rel);
        let example_dir = pack.parent().and_then(Path::parent).unwrap_or_else(|| {
            panic!("rule pack path should be examples/<name>/.polint/rules: {pack_rel}")
        });
        let example_name = example_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_else(|| panic!("example directory name utf-8: {pack_rel}"));
        let target_rel = example_dir.to_string_lossy().replace('\\', "/");
        cases.push(GoldenCase {
            id: example_case_id(example_name, FORMAT_JSON),
            target_rel,
            format: FORMAT_JSON,
            optional_target: false,
        });
    }

    for manifest_rel in string_array(&inputs, "scale_suite_manifests", &inputs_path) {
        let suite = load_toml(&root.join(manifest_rel));
        let suite_id = suite
            .get("id")
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("{manifest_rel}: missing id"));
        let checkout = suite
            .get("checkout")
            .and_then(toml::Value::as_table)
            .and_then(|table| table.get("path"))
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| panic!("{manifest_rel}: missing checkout.path"));
        cases.push(GoldenCase {
            id: scale_case_id(suite_id, FORMAT_JSON),
            target_rel: checkout.replace('\\', "/"),
            format: FORMAT_JSON,
            optional_target: true,
        });
    }

    cases.sort_by(|left, right| left.id.cmp(&right.id));
    cases
}

fn update_goldens_enabled() -> bool {
    matches!(
        std::env::var(UPDATE_ENV).as_deref(),
        Ok("1") | Ok("true") | Ok("TRUE") | Ok("yes") | Ok("YES")
    )
}

fn run_check(target: &Path, format: &str) -> String {
    let assert = polint_cmd()
        .current_dir(target)
        .args([
            "check",
            "--format",
            format,
            "--fail-on",
            "none",
            "--color",
            "never",
            "--no-cache",
        ])
        .assert()
        .success();
    String::from_utf8(assert.get_output().stdout.clone())
        .expect("polint stdout should be valid UTF-8")
}

fn fingerprint_of(diagnostic: &Value) -> String {
    diagnostic
        .get("stable_fingerprint")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("diagnostic missing stable_fingerprint: {diagnostic}"))
        .to_string()
}

fn sort_value_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut ordered = serde_json::Map::new();
            let mut keys: Vec<_> = map.keys().cloned().collect();
            keys.sort();
            for key in keys {
                let child = map.get(&key).expect("key present").clone();
                ordered.insert(key, sort_value_keys(child));
            }
            Value::Object(ordered)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sort_value_keys).collect()),
        other => other,
    }
}

fn rewrite_path_string(text: &str, roots: &[PathBuf]) -> String {
    let mut out = text.replace('\\', "/");
    let mut root_strs: Vec<String> = roots
        .iter()
        .map(|root| root.to_string_lossy().replace('\\', "/"))
        .filter(|root| !root.is_empty())
        .collect();
    root_strs.sort_by_key(|root| std::cmp::Reverse(root.len()));
    for root_str in &root_strs {
        if let Some(rest) = out.strip_prefix(root_str) {
            out = rest.trim_start_matches('/').to_string();
        }
        let with_sep = format!("{root_str}/");
        out = out.replace(&with_sep, "");
    }
    for marker in ["/private/var/folders/", "/var/folders/", "/tmp/"] {
        if let Some(idx) = out.find(marker) {
            out = format!("<temp>{}", &out[idx + marker.len() - 1..]);
        }
    }
    out
}

fn strip_volatile_strings(value: &mut Value, roots: &[PathBuf]) {
    match value {
        Value::Object(map) => {
            const DROP_KEYS: &[&str] = &[
                "generated_at",
                "duration_ms",
                "elapsed_ms",
                "elapsed",
                "timing",
                "timings",
                "wall_time_ms",
                "cpu_time_ms",
                "threads",
                "thread_count",
                "hostname",
                "machine",
                "machine_name",
            ];
            for key in DROP_KEYS {
                map.remove(*key);
            }
            if let Some(tool) = map.get_mut("tool")
                && let Some(obj) = tool.as_object_mut()
                && obj.contains_key("version")
            {
                obj.insert(
                    "version".to_string(),
                    Value::String(STRIPPED_VERSION.to_string()),
                );
            }
            for child in map.values_mut() {
                strip_volatile_strings(child, roots);
            }
        }
        Value::Array(items) => {
            for item in items {
                strip_volatile_strings(item, roots);
            }
        }
        Value::String(text) => {
            *text = rewrite_path_string(text, roots);
        }
        _ => {}
    }
}

/// Normalize a polint JSON report for golden comparison.
fn normalize_report(raw: &str, case_root: &Path, repo_root: &Path) -> String {
    let mut value: Value = serde_json::from_str(raw)
        .unwrap_or_else(|err| panic!("CLI stdout was not JSON: {err}\n{raw}"));

    let roots = vec![
        case_root
            .canonicalize()
            .unwrap_or_else(|_| case_root.to_path_buf()),
        repo_root
            .canonicalize()
            .unwrap_or_else(|_| repo_root.to_path_buf()),
    ];
    strip_volatile_strings(&mut value, &roots);

    if let Some(diagnostics) = value.get_mut("diagnostics").and_then(Value::as_array_mut) {
        diagnostics.sort_by_key(fingerprint_of);
    }

    // Compact JSON keeps the committed corpus inside the PR line budget while
    // still using sorted object keys for byte-stable comparison.
    let ordered = sort_value_keys(value);
    let mut compact = serde_json::to_string(&ordered)
        .unwrap_or_else(|err| panic!("serialize normalized report: {err}"));
    compact.push('\n');
    compact
}

#[derive(Debug, Default)]
struct DiagnosticSetDiff {
    lost: Vec<(String, String)>,
    added: Vec<(String, String)>,
}

fn diagnostic_label(diagnostic: &Value) -> String {
    let rule = diagnostic
        .get("rule_id")
        .and_then(Value::as_str)
        .unwrap_or("<missing-rule>");
    let file = diagnostic
        .get("file")
        .and_then(Value::as_str)
        .unwrap_or("<missing-file>");
    let message = diagnostic
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("<missing-message>");
    format!("{rule} @ {file}: {message}")
}

fn set_diff_diagnostics(expected: &Value, actual: &Value) -> DiagnosticSetDiff {
    let expected_diags = expected
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let actual_diags = actual
        .get("diagnostics")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let expected_map: BTreeMap<String, String> = expected_diags
        .iter()
        .map(|diag| (fingerprint_of(diag), diagnostic_label(diag)))
        .collect();
    let actual_map: BTreeMap<String, String> = actual_diags
        .iter()
        .map(|diag| (fingerprint_of(diag), diagnostic_label(diag)))
        .collect();

    let mut diff = DiagnosticSetDiff::default();
    for (fp, label) in &expected_map {
        if !actual_map.contains_key(fp) {
            diff.lost.push((fp.clone(), label.clone()));
        }
    }
    for (fp, label) in &actual_map {
        if !expected_map.contains_key(fp) {
            diff.added.push((fp.clone(), label.clone()));
        }
    }
    diff
}

fn format_set_diff(case_id: &str, diff: &DiagnosticSetDiff) -> String {
    let mut out = format!("golden mismatch for `{case_id}` — diagnostic set difference:\n");
    if diff.lost.is_empty() && diff.added.is_empty() {
        out.push_str("  (diagnostic fingerprints match; other normalized report fields differ)\n");
        return out;
    }
    if !diff.lost.is_empty() {
        out.push_str("  lost diagnostics (present in golden, missing from actual):\n");
        for (fp, label) in &diff.lost {
            out.push_str(&format!("    - {fp}  {label}\n"));
        }
    }
    if !diff.added.is_empty() {
        out.push_str("  new diagnostics (present in actual, missing from golden):\n");
        for (fp, label) in &diff.added {
            out.push_str(&format!("    + {fp}  {label}\n"));
        }
    }
    out
}

fn assert_normalized_matches(case_id: &str, expected_raw: &str, actual_raw: &str) {
    if expected_raw == actual_raw {
        return;
    }
    let expected: Value = serde_json::from_str(expected_raw).unwrap_or(Value::Null);
    let actual: Value = serde_json::from_str(actual_raw).unwrap_or(Value::Null);
    let diff = set_diff_diagnostics(&expected, &actual);
    let summary = format_set_diff(case_id, &diff);
    panic!(
        "{summary}\n--- expected (normalized) ---\n{expected_raw}\n--- actual (normalized) ---\n{actual_raw}"
    );
}

fn ensure_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|err| panic!("create {}: {err}", parent.display()));
    }
}

fn run_case(root: &Path, case: &GoldenCase, update: bool) {
    let target = root.join(&case.target_rel);
    if !target.is_dir() {
        if case.optional_target {
            eprintln!(
                "GOLDEN SKIP (loud): case `{}` — target missing at {}; run `make fetch-scale-repos`",
                case.id,
                target.display()
            );
            return;
        }
        panic!(
            "golden case `{}` target missing: {}",
            case.id,
            target.display()
        );
    }

    let raw = run_check(&target, case.format);
    let normalized = normalize_report(&raw, &target, root);
    let path = golden_path(root, &case.id);

    if update {
        ensure_parent(&path);
        fs::write(&path, &normalized)
            .unwrap_or_else(|err| panic!("write golden {}: {err}", path.display()));
        return;
    }

    let expected = fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!(
            "missing golden for `{}` at {} ({err}); regenerate with {UPDATE_ENV}=1",
            case.id,
            path.display()
        )
    });
    assert_normalized_matches(&case.id, &expected, &normalized);
}

#[test]
fn characterization_goldens_match_cli() {
    let root = repo_root();
    assert!(
        root.join(GOLDEN_ROOT_REL).is_dir(),
        "missing {}",
        root.join(GOLDEN_ROOT_REL).display()
    );
    let cases = discover_cases(&root);
    assert!(
        cases.iter().any(|case| case.id.starts_with("examples/")),
        "expected at least one example characterization case"
    );
    assert!(
        cases.iter().any(|case| case.id.starts_with("scale/")),
        "expected scale characterization cases (optional checkout)"
    );
    let update = update_goldens_enabled();
    let mut ran = 0usize;
    let mut skipped = 0usize;
    for case in &cases {
        if case.optional_target && !root.join(&case.target_rel).is_dir() {
            eprintln!(
                "GOLDEN SKIP (loud): case `{}` — target missing at {}; run `make fetch-scale-repos`",
                case.id,
                root.join(&case.target_rel).display()
            );
            skipped += 1;
            continue;
        }
        run_case(&root, case, update);
        ran += 1;
    }
    assert!(
        ran > 0,
        "no golden cases ran; example targets must be present in a clean checkout"
    );
    eprintln!("golden harness: ran {ran} case(s), skipped {skipped} optional case(s)");
}

#[test]
fn normalize_sorts_diagnostics_by_stable_fingerprint() {
    let root = repo_root();
    let raw = r#"{
      "version": 1,
      "tool": {"name": "polint", "version": "9.9.9"},
      "diagnostics": [
        {"rule_id": "b", "file": "z.go", "message": "second", "stable_fingerprint": "bbbb"},
        {"rule_id": "a", "file": "a.go", "message": "first", "stable_fingerprint": "aaaa"}
      ],
      "duration_ms": 12,
      "hostname": "box"
    }"#;
    let normalized = normalize_report(raw, &root, &root);
    let value: Value = serde_json::from_str(&normalized).unwrap();
    assert_eq!(value["tool"]["version"], STRIPPED_VERSION);
    assert!(value.get("duration_ms").is_none());
    assert!(value.get("hostname").is_none());
    let fps: Vec<_> = value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(fingerprint_of)
        .collect();
    assert_eq!(fps, vec!["aaaa".to_string(), "bbbb".to_string()]);
}

#[test]
fn diagnostic_set_diff_names_lost_fingerprints() {
    let expected: Value = serde_json::json!({
        "diagnostics": [
            {
                "rule_id": "local/example",
                "file": "a.ts",
                "message": "keep me",
                "stable_fingerprint": "deadbeef"
            },
            {
                "rule_id": "local/example",
                "file": "b.ts",
                "message": "lost finding",
                "stable_fingerprint": "cafebabe"
            }
        ]
    });
    let actual: Value = serde_json::json!({
        "diagnostics": [
            {
                "rule_id": "local/example",
                "file": "a.ts",
                "message": "keep me",
                "stable_fingerprint": "deadbeef"
            }
        ]
    });
    let diff = set_diff_diagnostics(&expected, &actual);
    assert_eq!(diff.lost.len(), 1);
    assert_eq!(diff.lost[0].0, "cafebabe");
    assert!(diff.lost[0].1.contains("lost finding"));
    assert!(diff.added.is_empty());
    let rendered = format_set_diff("examples/demo/json", &diff);
    assert!(
        rendered.contains("lost diagnostics"),
        "failure text must call out lost diagnostics:\n{rendered}"
    );
    assert!(
        rendered.contains("cafebabe"),
        "failure text must name the fingerprint:\n{rendered}"
    );
}

#[test]
fn example_golden_cases_cover_inventory_rule_packs() {
    let root = repo_root();
    let inputs = load_toml(&root.join(INPUTS_REL));
    let packs: BTreeSet<String> =
        string_array(&inputs, "example_rule_packs", &root.join(INPUTS_REL))
            .into_iter()
            .map(str::to_string)
            .collect();
    let cases = discover_cases(&root);
    let example_case_count = cases
        .iter()
        .filter(|case| case.id.starts_with("examples/"))
        .count();
    assert_eq!(
        example_case_count,
        packs.len(),
        "every inventoried example rule pack must have a characterization case"
    );
    for pack in &packs {
        let name = Path::new(pack)
            .parent()
            .and_then(Path::parent)
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap();
        let id = example_case_id(name, FORMAT_JSON);
        assert!(
            cases.iter().any(|case| case.id == id),
            "missing golden case for pack {pack} (expected id {id})"
        );
        if !update_goldens_enabled() {
            let path = golden_path(&root, &id);
            assert!(
                path.is_file(),
                "committed golden missing for `{id}` at {}",
                path.display()
            );
        }
    }
}

#[test]
fn scale_cases_are_optional_and_discovered() {
    let root = repo_root();
    let cases = discover_cases(&root);
    let scale: Vec<_> = cases
        .iter()
        .filter(|case| case.id.starts_with("scale/"))
        .collect();
    assert_eq!(scale.len(), 3, "expected three scale suite cases");
    assert!(scale.iter().all(|case| case.optional_target));
}

#[test]
fn golden_updates_require_explicit_env_flag() {
    // Default CI / developer runs must compare, never rewrite. Only an explicit
    // opt-in rewrites goldens; workflows are scanned below so CI cannot arm it.
    if std::env::var_os(UPDATE_ENV).is_some() {
        eprintln!(
            "note: {UPDATE_ENV} is set in this process; update path is armed for this run only"
        );
        return;
    }
    assert!(
        !update_goldens_enabled(),
        "{UPDATE_ENV} must be unset (or not a truthy value) for characterization compares"
    );
}

#[test]
fn ci_workflows_never_set_golden_update_env() {
    let workflows = repo_root().join(".github/workflows");
    let entries = fs::read_dir(&workflows).unwrap_or_else(|err| {
        panic!("read {}: {err}", workflows.display());
    });
    let mut scanned = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("yml")
            && path.extension().and_then(|ext| ext.to_str()) != Some("yaml")
        {
            continue;
        }
        let raw = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!("read {}: {err}", path.display());
        });
        assert!(
            !raw.contains(UPDATE_ENV),
            "{} must not set or mention {UPDATE_ENV}; golden regeneration is opt-in only",
            path.display()
        );
        scanned += 1;
    }
    assert!(
        scanned > 0,
        "expected to scan at least one workflow under {}",
        workflows.display()
    );
}
