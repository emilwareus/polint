use super::*;
use crate::analysis_plan::AnalysisPlan;
use crate::core::{AnalysisDb, TS_JS_MODULE_FUNCTION_NAME};
use crate::diagnostics::Diagnostic;
use crate::graph::ImportGraph;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static CACHE_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn analyze_source(path: &str, source: &str) -> (AnalysisDb, Vec<Diagnostic>) {
    let mut db = AnalysisDb::new();
    db.add_file(PathBuf::from(path), path.to_string(), source.to_string());
    let diagnostics = analyze(&mut db);
    (db, diagnostics)
}

fn db_with_ts_file(path: &str, source: &str) -> AnalysisDb {
    let mut db = AnalysisDb::new();
    db.add_file(PathBuf::from(path), path.to_string(), source.to_string());
    db
}

fn unique_cache_root() -> PathBuf {
    let sequence = CACHE_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "polint-ts-cache-{}-{nanos}-{sequence}",
        std::process::id()
    ))
}

fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files_into(root, &mut files);
    files.sort();
    files
}

fn collect_files_into(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries {
        let entry = entry.expect("read cache entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files_into(&path, files);
        } else {
            files.push(path);
        }
    }
}

fn first_layer_file(cache_root: &Path, category: &str) -> PathBuf {
    collect_files(&cache_root.join("layers").join(category))
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("expected layer cache {category} file"))
}

fn layer_cache_text(cache_root: &Path) -> String {
    collect_files(&cache_root.join("layers"))
        .into_iter()
        .map(|path| fs::read_to_string(path).expect("read layer cache file"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn corrupt_first_manifest_output_digest(cache_root: &Path) {
    let manifest_path = first_layer_file(cache_root, "manifests");
    let mut manifest_json: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read manifest"))
            .expect("manifest json");
    manifest_json["output_digest"]["value"] = serde_json::json!("deadbeef");
    fs::write(
        manifest_path,
        serde_json::to_vec(&manifest_json).expect("manifest serializes"),
    )
    .expect("write manifest");
}

fn assert_no_parser_diagnostics(diagnostics: &[Diagnostic]) {
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule_id != "parser/ts"),
        "unexpected parser diagnostics: {diagnostics:?}"
    );
}

#[test]
fn cache_writes_and_restores_ts_facts() {
    let cache_root = unique_cache_root();
    let cache = crate::cache::Cache::new(&cache_root, true);
    let source = r##"
import { token } from "./tokens";

export function Button() {
  return <div data-color="#ff00aa">{token}</div>;
}
"##;
    let mut first = db_with_ts_file("component.tsx", source);

    let first_diagnostics = analyze_with_cache(&mut first, &cache, "config", "rule");

    assert_no_parser_diagnostics(&first_diagnostics);
    assert!(cache_root.exists());
    let first_functions = first
        .functions()
        .iter()
        .map(|fact| fact.name.as_str())
        .collect::<Vec<_>>();

    let mut second = db_with_ts_file("component.tsx", source);
    let second_diagnostics = analyze_with_cache(&mut second, &cache, "config", "rule");
    let second_functions = second
        .functions()
        .iter()
        .map(|fact| fact.name.as_str())
        .collect::<Vec<_>>();

    assert_eq!(second_diagnostics, first_diagnostics);
    assert_eq!(second_functions, first_functions);
    assert_eq!(second.imports()[0].path, "./tokens");
    assert!(
        second
            .jsx_attributes()
            .iter()
            .any(|attribute| attribute.name == "data-color"
                && attribute.value.as_deref() == Some("#ff00aa"))
    );

    fs::remove_dir_all(cache_root).ok();
}

#[test]
fn ts_syntax_layer_cache_cold_warm() {
    let cache_root = unique_cache_root();
    let cache = crate::cache::Cache::new(cache_root.join("analysis"), true);
    let plan = AnalysisPlan::empty();
    let source = r#"
export function Button() {
  return "ok";
}
"#;
    let mut first = db_with_ts_file("component.ts", source);

    let first_result = analyze_with_plan_options_and_cache_stats(
        &mut first, &cache, "config", "rule", &plan, false,
    );

    assert_no_parser_diagnostics(&first_result.diagnostics);
    assert_eq!(first_result.cache_stats.misses, 1);
    assert_eq!(first_result.cache_stats.recomputes, 1);
    assert_eq!(first_result.cache_stats.writes, 1);
    assert_eq!(first_result.cache_stats.verified_reuse, 0);
    assert_eq!(first_result.cache_stats.quarantines, 0);
    assert!(cache_root.join("layers").exists());

    let mut second = db_with_ts_file("component.ts", source);
    let second_result = analyze_with_plan_options_and_cache_stats(
        &mut second,
        &cache,
        "config",
        "rule",
        &plan,
        false,
    );

    assert_no_parser_diagnostics(&second_result.diagnostics);
    assert_eq!(second_result.cache_stats.hits, 1);
    assert_eq!(second_result.cache_stats.verified_reuse, 1);
    assert_eq!(second_result.cache_stats.recomputes, 0);
    assert_eq!(second_result.cache_stats.writes, 0);

    fs::remove_dir_all(cache_root).ok();
}

#[test]
fn ts_syntax_layer_cache_corrupt() {
    let cache_root = unique_cache_root();
    let cache = crate::cache::Cache::new(cache_root.join("analysis"), true);
    let plan = AnalysisPlan::empty();
    let source = "// console.log('secret')\nexport function main() { return 'ok'; }\n";
    let mut first = db_with_ts_file("main.ts", source);

    let first_result = analyze_with_plan_options_and_cache_stats(
        &mut first, &cache, "config", "rule", &plan, false,
    );
    assert_no_parser_diagnostics(&first_result.diagnostics);

    let payload = first_layer_file(&cache_root, "blobs");
    fs::write(payload, "{not-json").expect("corrupt payload");

    let mut second = db_with_ts_file("main.ts", source);
    let second_result = analyze_with_plan_options_and_cache_stats(
        &mut second,
        &cache,
        "config",
        "changed-rule",
        &plan,
        false,
    );

    assert_no_parser_diagnostics(&second_result.diagnostics);
    assert_eq!(second_result.cache_stats.invalid_evicted_reads, 1);
    assert_eq!(second_result.cache_stats.recomputes, 1);
    assert_eq!(second_result.cache_stats.writes, 1);

    fs::remove_dir_all(cache_root).ok();
}

#[test]
fn ts_syntax_layer_cache_output_digest_mismatch_recomputes() {
    let cache_root = unique_cache_root();
    let cache = crate::cache::Cache::new(cache_root.join("analysis"), true);
    let plan = AnalysisPlan::empty();
    let source = "// console.log('secret')\nexport function main() { return 'ok'; }\n";
    let mut first = db_with_ts_file("main.ts", source);

    let first_result = analyze_with_plan_options_and_cache_stats(
        &mut first, &cache, "config", "rule", &plan, false,
    );
    assert_no_parser_diagnostics(&first_result.diagnostics);
    corrupt_first_manifest_output_digest(&cache_root);

    let mut second = db_with_ts_file("main.ts", source);
    let second_result = analyze_with_plan_options_and_cache_stats(
        &mut second,
        &cache,
        "config",
        "rule",
        &plan,
        false,
    );

    assert_no_parser_diagnostics(&second_result.diagnostics);
    assert_eq!(second_result.cache_stats.invalid_evicted_reads, 1);
    assert_eq!(second_result.cache_stats.recomputes, 1);
    assert_eq!(second_result.cache_stats.writes, 1);

    fs::remove_dir_all(cache_root).ok();
}

#[test]
fn ts_syntax_layer_cache_disabled_bypass() {
    let cache_root = unique_cache_root();
    let cache = crate::cache::Cache::new(cache_root.join("analysis"), false);
    let plan = AnalysisPlan::empty();
    let mut db = AnalysisDb::new();
    db.add_file(
        PathBuf::from("first.ts"),
        "first.ts".to_string(),
        "export const one = 1;\n".to_string(),
    );
    db.add_file(
        PathBuf::from("second.ts"),
        "second.ts".to_string(),
        "export const two = 2;\n".to_string(),
    );

    let result =
        analyze_with_plan_options_and_cache_stats(&mut db, &cache, "config", "rule", &plan, false);

    assert_no_parser_diagnostics(&result.diagnostics);
    assert_eq!(result.cache_stats.bypasses_disabled, 1);
    assert_eq!(result.cache_stats.recomputes, 1);
    assert_eq!(result.cache_stats.writes, 0);
    assert!(!cache_root.join("layers").exists());
}

#[test]
fn ts_syntax_layer_cache_payload_excludes_source_and_temp_paths() {
    let cache_root = unique_cache_root();
    let cache = crate::cache::Cache::new(cache_root.join("analysis"), true);
    let plan = AnalysisPlan::empty();
    let source = "// console.log('secret')\nexport function main() { return 'ok'; }\n";
    let mut db = db_with_ts_file("main.ts", source);

    let result =
        analyze_with_plan_options_and_cache_stats(&mut db, &cache, "config", "rule", &plan, false);

    assert_no_parser_diagnostics(&result.diagnostics);
    assert!(
        !collect_files(&cache_root.join("layers")).is_empty(),
        "expected syntax layer cache files"
    );
    let cache_text = layer_cache_text(&cache_root);
    assert!(!cache_text.contains("console.log"));
    assert!(!cache_text.contains(cache_root.to_string_lossy().as_ref()));

    fs::remove_dir_all(cache_root).ok();
}

#[test]
fn ts_parallel_analysis_matches_sequential() {
    let source = r##"
import { token } from "./tokens";

export function Button() {
  return <div data-color="#ff00aa">{token}</div>;
}
"##;
    let cache = crate::cache::Cache::new("", false);
    let mut sequential = db_with_ts_file("component.tsx", source);
    let mut parallel = db_with_ts_file("component.tsx", source);

    let sequential_diagnostics =
        analyze_with_options(&mut sequential, &cache, "config", "rule", false);
    let parallel_diagnostics = analyze_with_options(&mut parallel, &cache, "config", "rule", true);

    assert_eq!(parallel_diagnostics, sequential_diagnostics);
    assert_eq!(
        parallel
            .functions()
            .iter()
            .map(|fact| (&fact.name, fact.cyclomatic_complexity))
            .collect::<Vec<_>>(),
        sequential
            .functions()
            .iter()
            .map(|fact| (&fact.name, fact.cyclomatic_complexity))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        parallel
            .imports()
            .iter()
            .map(|fact| fact.path.as_str())
            .collect::<Vec<_>>(),
        sequential
            .imports()
            .iter()
            .map(|fact| fact.path.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        parallel
            .jsx_attributes()
            .iter()
            .map(|fact| (&fact.name, fact.value.as_deref()))
            .collect::<Vec<_>>(),
        sequential
            .jsx_attributes()
            .iter()
            .map(|fact| (&fact.name, fact.value.as_deref()))
            .collect::<Vec<_>>()
    );
}

#[test]
fn extracts_imports_and_export_from_specifiers_from_oxc_ast() {
    let source = r#"
import React from "react";
import { token } from "./tokens";
export { token } from "./tokens";
export * from "./shared";
"#;
    let (db, diagnostics) = analyze_source("imports.ts", source);
    assert_no_parser_diagnostics(&diagnostics);

    let paths: Vec<_> = db
        .imports()
        .iter()
        .map(|import| import.path.as_str())
        .collect();
    assert_eq!(paths, ["react", "./tokens", "./tokens", "./shared"]);
}

#[test]
fn dynamic_imports_string_literal_emit_import_fact() {
    let source = r#"
async function load() {
  return await import("./lazy");
}
"#;
    let (db, diagnostics) = analyze_source("dynamic.ts", source);
    assert_no_parser_diagnostics(&diagnostics);

    let paths: Vec<_> = db
        .imports()
        .iter()
        .map(|import| import.path.as_str())
        .collect();
    assert_eq!(paths, ["./lazy"]);
}

#[test]
fn dynamic_imports_expression_emit_dynamic_sentinel_import_fact() {
    let source = r#"
async function load(name: string) {
  return await import(name);
}
"#;
    let (db, diagnostics) = analyze_source("dynamic-expression.ts", source);
    assert_no_parser_diagnostics(&diagnostics);

    let paths: Vec<_> = db
        .imports()
        .iter()
        .map(|import| import.path.as_str())
        .collect();
    assert_eq!(paths, ["<dynamic>"]);
}

#[test]
fn dynamic_imports_inside_array_elements_emit_import_facts() {
    let source = r#"
const name = "./runtime";
const loaders = [import("./lazy"), import(name)];
"#;
    let (db, diagnostics) = analyze_source("dynamic-array.ts", source);
    assert_no_parser_diagnostics(&diagnostics);

    let paths: Vec<_> = db
        .imports()
        .iter()
        .map(|import| import.path.as_str())
        .collect();
    assert_eq!(paths, ["./lazy", "<dynamic>"]);
}

#[test]
fn extracts_functions_arrows_classes_methods_and_calls_from_oxc_ast() {
    let source = r#"
function helper(label: string) {
  track(label);
  return formatLabel(label);
}

export const Button = () => helper("ok");

export class Dialog {
  render() {
track("dialog");
return formatLabel("dialog");
  }
}
"#;
    let (db, diagnostics) = analyze_source("component.tsx", source);
    assert_no_parser_diagnostics(&diagnostics);

    let function_names: Vec<_> = db
        .functions()
        .iter()
        .filter(|function| function.name != TS_JS_MODULE_FUNCTION_NAME)
        .map(|function| function.name.as_str())
        .collect();
    assert_eq!(
        function_names,
        ["helper", "Button", "Dialog", "Dialog.render"]
    );

    let classes: Vec<_> = db.ts_classes().iter().collect();
    assert_eq!(classes.len(), 1);
    assert_eq!(classes[0].name, "Dialog");
    assert!(classes[0].is_exported);
    assert!(classes[0].is_component_like);

    let helper = db
        .functions()
        .iter()
        .find(|function| function.name == "helper")
        .expect("expected helper function");
    assert_eq!(helper.calls, ["formatLabel", "track"]);

    let button = db
        .functions()
        .iter()
        .find(|function| function.name == "Button")
        .expect("expected Button function");
    assert!(button.is_exported);
    assert_eq!(button.calls, ["helper"]);

    let dialog_constructor = db
        .functions()
        .iter()
        .find(|function| function.name == "Dialog")
        .expect("expected Dialog constructor function");
    assert!(dialog_constructor.is_exported);
    assert_eq!(dialog_constructor.calls, Vec::<String>::new());
    assert!(
        source[dialog_constructor.span.start_byte as usize
            ..dialog_constructor.span.end_byte as usize]
            .contains("class Dialog"),
        "class constructor function should use the class declaration span"
    );

    let render = db
        .functions()
        .iter()
        .find(|function| function.name == "Dialog.render")
        .expect("expected Dialog.render method");
    assert_eq!(render.calls, ["formatLabel", "track"]);
}

#[test]
fn extracts_commonjs_exported_function_expression_assignments() {
    let source = r#"
exports.cjsTarget = function cjsTarget(value: string): string {
  return normalize(value);
};

module.exports.otherTarget = function (value: string): string {
  return cjsTarget(value);
};

exports.aliasTarget = function implementationName(value: string): string {
  return otherTarget(value);
};

obj.notExported = function notExported(): void {};
"#;
    let (db, diagnostics) = analyze_source("cjs.ts", source);
    assert_no_parser_diagnostics(&diagnostics);

    let functions = db
        .functions()
        .iter()
        .filter(|function| function.is_exported)
        .map(|function| {
            (
                function.name.as_str(),
                function.is_exported,
                function
                    .calls
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        functions,
        [
            ("cjsTarget", true, vec!["normalize"]),
            ("otherTarget", true, vec!["cjsTarget"]),
            ("aliasTarget", true, vec!["otherTarget"])
        ]
    );
}

#[test]
fn detects_component_like_ts_facts_with_honest_heuristics() {
    let source = r#"
function helper() {
  return "ok";
}

function Button() {
  return label("ok");
}

const Link = () => <a href="/">Home</a>;

class Dialog {
  render() {
return <section />;
  }
}

class store {}
"#;
    let (db, diagnostics) = analyze_source("components.tsx", source);
    assert_no_parser_diagnostics(&diagnostics);

    let component_names: Vec<_> = db
        .ts_components()
        .iter()
        .map(|component| component.name.as_str())
        .collect();
    assert_eq!(component_names, ["Button", "Link", "Dialog"]);

    assert!(
        db.ts_components()
            .iter()
            .all(|component| component.name != "helper" && component.name != "store"),
        "lower-case helpers without JSX should not be component facts"
    );
    assert!(
        db.ts_classes()
            .iter()
            .any(|class| class.name == "Dialog" && class.is_component_like)
    );
    assert!(
        db.ts_classes()
            .iter()
            .any(|class| class.name == "store" && !class.is_component_like)
    );

    let production_source = include_str!("adapter.rs");
    assert!(
        production_source.contains("syntax-level component heuristic"),
        "component detection must identify itself as a heuristic"
    );
}

#[test]
fn reports_oxc_parser_errors_as_parser_ts_diagnostics() {
    let (_db, diagnostics) = analyze_source("broken.ts", "export function Broken( {");

    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.rule_id == "parser/ts")
        .expect("expected parser/ts diagnostic");

    assert_eq!(diagnostic.file, "broken.ts");
    assert!(
        diagnostic
            .message
            .contains("TS/JS parser reported a syntax error")
    );
}

#[test]
fn clean_ts_family_sources_do_not_emit_parser_ts() {
    let cases = [
        (
            "valid.ts",
            "export function ok(value: number) { return value + 1; }",
        ),
        (
            "component.tsx",
            "export function Button() { return <button aria-label=\"Save\">Save</button>; }",
        ),
        ("util.js", "export function ok(value) { return value + 1; }"),
        (
            "view.jsx",
            "export function Button() { return <button aria-label=\"Save\">Save</button>; }",
        ),
    ];

    for (path, source) in cases {
        let (_db, diagnostics) = analyze_source(path, source);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.rule_id != "parser/ts"),
            "{path} emitted parser/ts diagnostics: {diagnostics:?}"
        );
    }
}

#[test]
fn continues_best_effort_ast_extraction_after_oxc_parse_error() {
    let (db, diagnostics) =
        analyze_source("recoverable.ts", "import x from \"./x\";\nconst value = ;");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.rule_id == "parser/ts"),
        "expected parser/ts diagnostic"
    );
    assert!(
        db.imports().iter().any(|import| import.path == "./x"),
        "expected best-effort import fact after parse error"
    );
}

#[test]
fn parses_ts_source_from_shared_arc_without_full_source_clone() {
    let production_only = include_str!("adapter.rs");
    assert!(
        production_only.contains("Arc::clone(&file.source)"),
        "parse_ts_file should refcount Arc<str> (O(1)) instead of copying the buffer"
    );
    let forbidden = concat!("file.source", ".to_string()");
    assert!(
        !production_only.contains(forbidden),
        "parse_ts_file should not allocate a full String copy of the source"
    );
}

#[test]
fn source_type_comes_from_file_path_for_ts_family() {
    let production_source = include_str!("adapter.rs");
    let helper_signature = ["fn parse_source_type", "(path: &Path) -> SourceType"].concat();
    let source_type_from_path = ["SourceType::from_path", "(path).unwrap_or_default()"].concat();

    assert!(
        production_source.contains(&helper_signature),
        "expected parse_source_type helper"
    );
    assert!(
        production_source.contains(&source_type_from_path),
        "parse_source_type should derive SourceType from the file path"
    );
}

#[test]
fn ast_helpers_preserve_source_byte_spans() {
    let source = "\nimport x from \"./x\";\n";
    let (db, diagnostics) = analyze_source("spans.ts", source);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.rule_id != "parser/ts"),
        "unexpected parser diagnostics: {diagnostics:?}"
    );

    let import = db
        .imports()
        .iter()
        .find(|import| import.path == "./x")
        .expect("expected import fact for ./x");
    let start = source
        .find("\"./x\"")
        .expect("fixture should contain import");
    let end = start + "\"./x\"".len();
    assert_eq!(import.span.start_byte as usize, start);
    assert_eq!(import.span.end_byte as usize, end);

    let production_source = include_str!("adapter.rs");
    let span_helper = ["fn span_from", "_oxc"].concat();
    let span_conversion = [
        "span_from_byte_range(file, source, ",
        "span.start as usize, span.end as usize)",
    ]
    .concat();
    assert!(
        production_source.contains(&span_helper),
        "expected span_from_oxc helper"
    );
    assert!(
        production_source.contains(&span_conversion),
        "span_from_oxc should convert Oxc byte spans through core span conversion"
    );
}

#[test]
fn extracts_string_literals_and_static_templates_from_oxc_ast() {
    let source = r#"
const plain = "plain";
const single = 'single';
const staticTemplate = `static-template`;
const dynamicTemplate = `prefix-${name}`;
"#;
    let (db, diagnostics) = analyze_source("literals.ts", source);
    assert_no_parser_diagnostics(&diagnostics);

    let values: Vec<_> = db
        .string_literals()
        .iter()
        .map(|literal| literal.value.as_str())
        .collect();
    assert_eq!(values, ["plain", "single", "static-template", "prefix-"]);
    assert!(
        !values.iter().any(|value| value.contains("${")),
        "dynamic template expressions should not be recorded as exact strings: {values:?}"
    );
}

#[test]
fn extracts_ts_regex_literals_as_syntax_literals() {
    let source = r#"
const testIdPattern = /legacy-testid/;
const unsafePrefix = /^unsafe-/i;
"#;
    let (db, diagnostics) = analyze_source("regex-literals.ts", source);
    assert_no_parser_diagnostics(&diagnostics);

    let values: Vec<_> = db
        .string_literals()
        .iter()
        .map(|literal| literal.value.as_str())
        .collect();
    assert_eq!(values, ["/legacy-testid/", "/^unsafe-/i"]);
}

#[test]
fn denied_literal_rules_can_see_regex_literal_text() {
    use crate::core::{RuleCtx, RuleKind, RuleMeta, RuleOptions};
    use crate::diagnostics::Severity;
    use crate::sdk::facts::{FactView, StringLiterals};

    let source = r#"
const allowed = /legacy-testid/;
const denied = /^unsafe-/i;
"#;
    let (db, diagnostics) = analyze_source("regex-deny.ts", source);
    assert_no_parser_diagnostics(&diagnostics);
    let ctx = RuleCtx::new(
        &db,
        RuleMeta {
            id: "examples/config-query-no-literal".to_string(),
            description: "Deny configured syntax-level literals.".to_string(),
            severity: Severity::Error,
            kind: RuleKind::Check,
        },
        RuleOptions {
            deny: vec!["unsafe".to_string()],
            ..RuleOptions::default()
        },
    );

    let literals = StringLiterals::build(&db);
    let denied = literals
        .iter()
        .filter(|literal| {
            ctx.options()
                .deny
                .iter()
                .any(|deny| literal.value.contains(deny))
        })
        .map(|literal| literal.value.as_str())
        .collect::<Vec<_>>();

    assert_eq!(denied, vec!["/^unsafe-/i"]);
}

#[test]
fn extracts_jsx_attributes_from_oxc_ast() {
    let source = r#"
const View = ({ token }) => (
  <Button aria-label="Pay" data-token={token} disabled {...spread} />
);
"#;
    let (db, diagnostics) = analyze_source("component.tsx", source);
    assert_no_parser_diagnostics(&diagnostics);

    let attributes: Vec<_> = db
        .jsx_attributes()
        .iter()
        .map(|attribute| (attribute.name.as_str(), attribute.value.as_deref()))
        .collect();
    assert_eq!(
        attributes,
        [
            ("aria-label", Some("Pay")),
            ("data-token", Some("token")),
            ("disabled", None)
        ]
    );
}

#[test]
fn raw_color_literals_are_available_from_strings_and_jsx_attributes() {
    let source = r##"
const rawColor = "#ff00aa";
export function Button() {
  return <Button data-color="#00ff00" />;
}
"##;
    let (db, diagnostics) = analyze_source("raw-colors.tsx", source);
    assert_no_parser_diagnostics(&diagnostics);

    assert!(
        db.string_literals()
            .iter()
            .any(|literal| literal.value == "#ff00aa"),
        "expected raw color string literal fact"
    );
    assert!(
        db.jsx_attributes().iter().any(|attribute| {
            attribute.name == "data-color" && attribute.value.as_deref() == Some("#00ff00")
        }),
        "expected raw color JSX attribute fact"
    );
}

#[test]
fn quoted_jsx_attributes_are_available_as_string_literals() {
    let source = r##"
export function Button() {
  return <button data-color="#00ff00" />;
}
"##;
    let (db, diagnostics) = analyze_source("jsx-attribute-literal.tsx", source);
    assert_no_parser_diagnostics(&diagnostics);

    assert!(
        db.string_literals()
            .iter()
            .any(|literal| literal.value == "#00ff00"),
        "quoted JSX attribute values should be available to literal rules"
    );
}

#[test]
fn export_specifiers_mark_ts_facts_as_exported() {
    let source = r#"
const Button = () => null;
class Panel {}

export { Button, Panel };
"#;
    let (db, diagnostics) = analyze_source("exports.tsx", source);
    assert_no_parser_diagnostics(&diagnostics);

    let button = db
        .functions()
        .iter()
        .find(|function| function.name == "Button")
        .expect("expected Button function fact");
    assert!(button.is_exported);

    let panel = db
        .ts_classes()
        .iter()
        .find(|class| class.name == "Panel")
        .expect("expected Panel class fact");
    assert!(panel.is_exported);
    let panel_constructor = db
        .functions()
        .iter()
        .find(|function| function.name == "Panel")
        .expect("expected Panel constructor function fact");
    assert!(panel_constructor.is_exported);
}

#[test]
fn nested_calls_inside_regular_arguments_are_collected() {
    let source = r#"
export function render(error: unknown) {
  outer(inner());
  track(String(error));
}
"#;
    let (db, diagnostics) = analyze_source("calls.ts", source);
    assert_no_parser_diagnostics(&diagnostics);

    let render = db
        .functions()
        .iter()
        .find(|function| function.name == "render")
        .expect("expected render function");
    assert_eq!(render.calls, ["String", "inner", "outer", "track"]);
}

#[test]
fn referenced_default_exports_mark_ts_facts_as_exported() {
    let (function_db, function_diagnostics) = analyze_source(
        "default-function.tsx",
        r#"
const Button = () => null;
export default Button;
"#,
    );
    assert_no_parser_diagnostics(&function_diagnostics);
    let button = function_db
        .functions()
        .iter()
        .find(|function| function.name == "Button")
        .expect("expected Button function fact");
    assert!(button.is_exported);

    let (class_db, class_diagnostics) = analyze_source(
        "default-class.tsx",
        r#"
class Panel {}
export default Panel;
"#,
    );
    assert_no_parser_diagnostics(&class_diagnostics);
    let panel = class_db
        .ts_classes()
        .iter()
        .find(|class| class.name == "Panel")
        .expect("expected Panel class fact");
    assert!(panel.is_exported);
    let panel_constructor = class_db
        .functions()
        .iter()
        .find(|function| function.name == "Panel")
        .expect("expected Panel constructor function fact");
    assert!(panel_constructor.is_exported);
}

#[test]
fn calls_inside_expression_containers_are_collected() {
    let source = r#"
export function render() {
  const values = [load()];
  const obj = { value: format() };
  const client = new Client(createConfig());
  return <View label={labelText()}>{renderChild()}</View>;
}
"#;
    let (db, diagnostics) = analyze_source("containers.tsx", source);
    assert_no_parser_diagnostics(&diagnostics);

    let render = db
        .functions()
        .iter()
        .find(|function| function.name == "render")
        .expect("expected render function");
    assert_eq!(
        render.calls,
        [
            "Client",
            "createConfig",
            "format",
            "labelText",
            "load",
            "renderChild"
        ]
    );
}

#[test]
fn commonjs_require_calls_emit_import_facts() {
    let source = r#"
const config = require("./config");
const lib = require("@scope/lib");
export const value = config.value;
"#;
    let (db, diagnostics) = analyze_source("commonjs.js", source);
    assert_no_parser_diagnostics(&diagnostics);

    let paths: Vec<_> = db
        .imports()
        .iter()
        .map(|import| import.path.as_str())
        .collect();
    assert_eq!(paths, ["./config", "@scope/lib"]);
}

#[test]
fn commonjs_require_calls_inside_function_and_class_bodies_emit_import_facts() {
    let source = r#"
function loadConfig() {
  return require("./config");
}

const loadLazy = () => require("./lazy");

class Loader {
  load() {
return require("./method");
  }

  static fallback = require("./static");
}
"#;
    let (db, diagnostics) = analyze_source("commonjs-bodies.js", source);
    assert_no_parser_diagnostics(&diagnostics);

    let paths: Vec<_> = db
        .imports()
        .iter()
        .map(|import| import.path.as_str())
        .collect();
    assert_eq!(paths, ["./config", "./lazy", "./method", "./static"]);
}

#[test]
fn computes_ts_complexity_from_oxc_control_flow() {
    let source = r#"
export function authorize(input: Input) {
  if (input.ready && input.allowed || input.admin) {
approve();
  }

  for (const item of input.items) {
audit(item);
  }

  switch (input.kind) {
case "fast":
  fast();
  break;
default:
  fallback();
  }

  try {
risky(input.flag ? "yes" : "no");
  } catch (error) {
recover(error);
  }
}
"#;
    let (db, diagnostics) = analyze_source("complexity.ts", source);
    assert_no_parser_diagnostics(&diagnostics);

    let function = db
        .functions()
        .iter()
        .find(|function| function.name == "authorize")
        .expect("expected authorize function");

    assert_eq!(function.cyclomatic_complexity, 9);
}

#[test]
fn ts_complexity_does_not_count_words_inside_strings_or_comments() {
    let source = r#"
// if for while case catch && || ?
export function plain() {
  const text = "if for while case catch && || ?";
  return text;
}
"#;
    let (db, diagnostics) = analyze_source("plain.ts", source);
    assert_no_parser_diagnostics(&diagnostics);

    let function = db
        .functions()
        .iter()
        .find(|function| function.name == "plain")
        .expect("expected plain function");

    assert_eq!(function.cyclomatic_complexity, 1);
}

#[test]
fn ts_import_facts_feed_import_graph() {
    let source = r#"
import { tokens } from "./tokens";

export function renderView() {
  return tokens.primary;
}
"#;
    let (db, diagnostics) = analyze_source("src/view.ts", source);
    assert_no_parser_diagnostics(&diagnostics);

    let dot = ImportGraph::from_db(&db).to_dot();
    assert!(dot.contains("src/view.ts"), "{dot}");
    assert!(dot.contains("./tokens"), "{dot}");
}
