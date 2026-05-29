use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::Deserialize;

#[cfg(test)]
use crate::analysis_kernel::KernelOutput;
#[cfg(test)]
use crate::core::Span;
use crate::eval::adapter::{BenchmarkAdapter, PreparedCase, RawObservedOutput};
#[cfg(test)]
use crate::eval::model::ObservedGraphEdge;
use crate::eval::model::{
    AssertionMode, EvaluationCase, ExpectedGraphEdge, ExpectedItem, FixtureArea, ObservedItem,
};
use crate::eval::report::CaseResult;
use crate::eval::runner::safe_join_workspace;
use crate::eval::suite::{SuiteLanguageSupport, SuiteManifest, normalize_repo_relative_path};

pub(crate) struct JellyCallgraphAdapter;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct JellyCallgraphEnumeration {
    pub(crate) cases: Vec<EvaluationCase>,
    pub(crate) limitations: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
struct JellyCallgraph {
    #[serde(default)]
    entries: Vec<String>,
    files: Vec<String>,
    functions: Vec<String>,
    calls: Vec<String>,
    #[serde(default)]
    fun2fun: Vec<[usize; 2]>,
    #[serde(default)]
    call2fun: Vec<[usize; 2]>,
}

impl BenchmarkAdapter for JellyCallgraphAdapter {
    fn adapter_id(&self) -> &'static str {
        "jelly_callgraph_micro"
    }

    fn language_support(&self, manifest: &SuiteManifest) -> SuiteLanguageSupport {
        manifest.language_support
    }

    fn enumerate_cases(&self, manifest: &SuiteManifest) -> anyhow::Result<Vec<EvaluationCase>> {
        Ok(enumerate_jelly_callgraph_cases(Path::new(&manifest.checkout.path))?.cases)
    }

    fn prepare_case(
        &self,
        manifest: &SuiteManifest,
        case: &EvaluationCase,
    ) -> anyhow::Result<PreparedCase> {
        let root = PathBuf::from(&manifest.checkout.path);
        let json_path = safe_join_workspace(&root, Path::new(&case.repo_path))?;
        let graph = parse_jelly_graph_file(&json_path)?;
        let case_dir = Path::new(&case.repo_path)
            .parent()
            .unwrap_or_else(|| Path::new(""));
        let mut target_files = Vec::new();
        for entry in &graph.entries {
            target_files.push(safe_join_workspace(&root, &case_dir.join(entry))?);
        }
        if target_files.is_empty() {
            target_files.push(json_path);
        }
        Ok(PreparedCase {
            case_id: case.case_id.clone(),
            workspace_root: root,
            target_files,
        })
    }

    fn normalize_observed(
        &self,
        _manifest: &SuiteManifest,
        _case: &EvaluationCase,
        raw: RawObservedOutput,
    ) -> anyhow::Result<Vec<ObservedItem>> {
        Ok(raw.observed)
    }

    fn suite_native_metrics(
        &self,
        _manifest: &SuiteManifest,
        results: &[CaseResult],
    ) -> anyhow::Result<BTreeMap<String, f64>> {
        let expected_edges = results
            .iter()
            .flat_map(|case| case.expected.iter())
            .filter(|item| matches!(item, ExpectedItem::GraphEdge(_)))
            .count();
        Ok(BTreeMap::from([
            (
                "jelly.callgraph_micro.case_count".to_string(),
                results.len() as f64,
            ),
            (
                "jelly.callgraph_micro.expected_edge_count".to_string(),
                expected_edges as f64,
            ),
        ]))
    }

    #[cfg(test)]
    fn normalize_kernel_output(
        &self,
        _manifest: &SuiteManifest,
        _case: &EvaluationCase,
        _prepared: &PreparedCase,
        output: &KernelOutput,
    ) -> anyhow::Result<Vec<ObservedItem>> {
        // Per D-05 the Jelly span string is produced ONLY by
        // `analysis::identity::render::jelly_span::render` — this adapter no
        // longer formats spans inline. We look up the IdentityRecord whose span
        // matches each edge span and render it; an edge whose span has no
        // identity record (or whose source file is missing) is dropped via the
        // existing `else { continue; }` short-circuit. The `db` borrow keeps the
        // lookup pure and explicit (WARNING #3 — no inline `format!()`).
        let files = output
            .db
            .files()
            .iter()
            .map(|file| (file.id, file))
            .collect::<BTreeMap<_, _>>();
        let records = output.db.identity_records();
        let render_span = |span: &Span| -> Option<String> {
            let record = records.iter().find(|record| record.span == *span)?;
            let source = files.get(&span.file)?;
            Some(crate::analysis::identity::render::jelly_span::render(
                record, source,
            ))
        };
        let mut items = Vec::new();
        for edge in crate::eval::observed::graph_edges_from_kernel_output(output) {
            let Some(call_site) = render_span(&edge.site_span) else {
                continue;
            };
            let Some(caller) = render_span(&edge.caller_span) else {
                continue;
            };
            let Some(target_span) = &edge.target_span else {
                continue;
            };
            let Some(target) = render_span(target_span) else {
                continue;
            };
            items.push(ObservedItem::GraphEdge(ObservedGraphEdge {
                graph: "jelly.call_graph.call2fun".to_string(),
                from: call_site,
                to: target.clone(),
                mode: AssertionMode::Exact,
                partial_truth: false,
                producer_id: Some(edge.producer_id.clone()),
                provenance: Some(edge.provenance.clone()),
                precision: Some(edge.precision.clone()),
                status: Some(edge.status),
            }));
            items.push(ObservedItem::GraphEdge(ObservedGraphEdge {
                graph: "jelly.call_graph.fun2fun".to_string(),
                from: caller,
                to: target,
                mode: AssertionMode::Exact,
                partial_truth: false,
                producer_id: Some(edge.producer_id),
                provenance: Some(edge.provenance),
                precision: Some(edge.precision),
                status: Some(edge.status),
            }));
        }
        items.extend(crate::eval::observed::call_graph_unknown_facts_from_kernel_output(output));
        Ok(items)
    }
}

pub(crate) fn enumerate_jelly_callgraph_cases(
    root: &Path,
) -> anyhow::Result<JellyCallgraphEnumeration> {
    if !root.exists() {
        return Ok(JellyCallgraphEnumeration {
            cases: Vec::new(),
            limitations: vec![format!(
                "Jelly local clone is absent at {}; suite can be planned but not executed",
                root.display()
            )],
        });
    }

    let mut files = Vec::new();
    collect_json_files(root, &root.join("tests"), &mut files)?;
    files.sort();

    let mut cases = Vec::new();
    for relative in files {
        if let Some(case) = jelly_case(root, relative)? {
            cases.push(case);
        }
    }

    Ok(JellyCallgraphEnumeration {
        cases,
        limitations: vec![
            "Jelly JSON files are suite-native call graph outputs, so this adapter scores edge agreement with that oracle rather than executing Jelly itself".to_string(),
            "Location identities follow Jelly's file-indexed source-location schema; consumers must normalize their own call graph output to the same identity format".to_string(),
        ],
    })
}

fn collect_json_files(root: &Path, current: &Path, files: &mut Vec<String>) -> anyhow::Result<()> {
    if !current.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if matches!(file_name.as_ref(), ".git" | "node_modules") {
            continue;
        }
        if path.is_dir() {
            collect_json_files(root, &path, files)?;
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
            let relative = path.strip_prefix(root)?;
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn jelly_case(root: &Path, relative_path: String) -> anyhow::Result<Option<EvaluationCase>> {
    let relative_path = normalize_repo_relative_path("jelly case path", &relative_path)?;
    let graph = match parse_jelly_graph_file(&root.join(&relative_path)) {
        Ok(graph) => graph,
        Err(_) => return Ok(None),
    };
    if graph.functions.is_empty() && graph.calls.is_empty() {
        return Ok(None);
    }
    let expected: Vec<ExpectedItem> = jelly_expected_edges(&graph)?
        .into_iter()
        .map(ExpectedItem::GraphEdge)
        .collect();
    if expected.is_empty() {
        return Ok(None);
    }

    Ok(Some(EvaluationCase {
        case_id: relative_path.clone(),
        area: FixtureArea::Graphs,
        repo_path: relative_path,
        expected,
        observed: Vec::new(),
    }))
}

fn parse_jelly_graph_file(path: &Path) -> anyhow::Result<JellyCallgraph> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read Jelly call graph {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("parse Jelly call graph {}", path.display()))
}

fn jelly_expected_edges(graph: &JellyCallgraph) -> anyhow::Result<Vec<ExpectedGraphEdge>> {
    let mut edges = Vec::new();
    for [from, to] in &graph.fun2fun {
        edges.push(ExpectedGraphEdge {
            graph: "jelly.call_graph.fun2fun".to_string(),
            from: canonical_location(&graph.files, &graph.functions, *from)?,
            to: canonical_location(&graph.files, &graph.functions, *to)?,
            mode: AssertionMode::Exact,
            partial_truth: false,
        });
    }
    for [from, to] in &graph.call2fun {
        edges.push(ExpectedGraphEdge {
            graph: "jelly.call_graph.call2fun".to_string(),
            from: canonical_location(&graph.files, &graph.calls, *from)?,
            to: canonical_location(&graph.files, &graph.functions, *to)?,
            mode: AssertionMode::Exact,
            partial_truth: false,
        });
    }
    Ok(edges)
}

fn canonical_location(
    files: &[String],
    locations: &[String],
    index: usize,
) -> anyhow::Result<String> {
    let raw = locations
        .get(index)
        .ok_or_else(|| anyhow::anyhow!("Jelly location index {index} out of bounds"))?;
    let mut parts = raw.split(':');
    let file_index = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("Jelly location `{raw}` has no file index"))?
        .parse::<usize>()
        .with_context(|| format!("parse Jelly file index in `{raw}`"))?;
    let file = files
        .get(file_index)
        .ok_or_else(|| anyhow::anyhow!("Jelly file index {file_index} out of bounds"))?;
    let start_line = next_location_part(&mut parts, raw, "start line")?;
    let start_col = next_location_part(&mut parts, raw, "start column")?;
    let end_line = next_location_part(&mut parts, raw, "end line")?;
    let end_col = next_location_part(&mut parts, raw, "end column")?;
    if parts.next().is_some() {
        bail!("Jelly location `{raw}` has too many parts");
    }
    Ok(format!(
        "{file}:{start_line}:{start_col}:{end_line}:{end_col}"
    ))
}

fn next_location_part<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
    raw: &str,
    field: &str,
) -> anyhow::Result<&'a str> {
    parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Jelly location `{raw}` missing {field}"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use tempfile::tempdir;

    use super::*;
    use crate::eval::suite::{
        CaseSelector, ExpectedSource, ExpectedSourceFormat, LocalClonePolicy, SuiteCheckout,
        SuiteCheckoutStrategy, SuiteId, SuiteKind, SuiteScoring, SuiteTier,
    };

    #[test]
    fn absent_clone_skips_gracefully() {
        let missing = tempdir().unwrap().path().join("missing");
        let enumeration = enumerate_jelly_callgraph_cases(&missing).unwrap();

        assert!(enumeration.cases.is_empty());
        assert!(enumeration.limitations[0].contains("local clone is absent"));
    }

    #[test]
    fn parses_jelly_fun_and_call_edges() {
        let graph = JellyCallgraph {
            entries: vec!["app.js".to_string()],
            files: vec!["app.js".to_string()],
            functions: vec!["0:1:1:4:1".to_string(), "0:2:1:2:16".to_string()],
            calls: vec!["0:3:1:3:9".to_string()],
            fun2fun: vec![[0, 1]],
            call2fun: vec![[0, 1]],
        };

        let edges = jelly_expected_edges(&graph).unwrap();

        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0].graph, "jelly.call_graph.fun2fun");
        assert_eq!(edges[0].from, "app.js:1:1:4:1");
        assert_eq!(edges[0].to, "app.js:2:1:2:16");
        assert_eq!(edges[1].graph, "jelly.call_graph.call2fun");
        assert_eq!(edges[1].from, "app.js:3:1:3:9");
    }

    #[test]
    fn enumerates_only_json_call_graph_cases_deterministically() {
        let root = tempdir().unwrap();
        let tests = root.path().join("tests/micro");
        std::fs::create_dir_all(&tests).unwrap();
        std::fs::write(tests.join("ignore.json"), r#"{"pattern":"not a graph"}"#).unwrap();
        std::fs::write(
            tests.join("b.json"),
            r#"{"entries":["b.js"],"files":["b.js"],"functions":["0:1:1:1:8","0:2:1:2:8"],"calls":["0:3:1:3:4"],"fun2fun":[[0,1]],"call2fun":[]}"#,
        )
        .unwrap();
        std::fs::write(
            tests.join("a.json"),
            r#"{"entries":["a.js"],"files":["a.js"],"functions":["0:1:1:1:8","0:2:1:2:8"],"calls":["0:3:1:3:4"],"fun2fun":[[0,1]],"call2fun":[[0,1]]}"#,
        )
        .unwrap();

        let enumeration = enumerate_jelly_callgraph_cases(root.path()).unwrap();

        assert_eq!(
            enumeration
                .cases
                .iter()
                .map(|case| case.case_id.as_str())
                .collect::<Vec<_>>(),
            ["tests/micro/a.json", "tests/micro/b.json"]
        );
        assert_eq!(enumeration.cases[0].expected.len(), 2);
    }

    #[test]
    fn local_jelly_expected_edge_parser_preserves_current_oracle_count() {
        let root = repo_root().join("research/evaluation-harness/repos/jelly");
        if !root.exists() {
            return;
        }
        let enumeration = enumerate_jelly_callgraph_cases(&root).unwrap();
        let expected_edges = enumeration
            .cases
            .iter()
            .flat_map(|case| case.expected.iter())
            .filter(|item| matches!(item, ExpectedItem::GraphEdge(_)))
            .count();

        assert_eq!(expected_edges, 1_479);
    }

    #[test]
    fn adapter_prepares_entry_files_inside_clone() {
        let root = tempdir().unwrap();
        let tests = root.path().join("tests/micro");
        std::fs::create_dir_all(&tests).unwrap();
        std::fs::write(
            tests.join("case.json"),
            r#"{"entries":["app.js"],"files":["app.js"],"functions":["0:1:1:1:8","0:2:1:2:8"],"calls":[],"fun2fun":[[0,1]],"call2fun":[]}"#,
        )
        .unwrap();
        std::fs::write(tests.join("app.js"), "function a(){}").unwrap();
        let manifest = manifest(root.path().to_string_lossy().as_ref());
        let adapter = JellyCallgraphAdapter;
        let case = EvaluationCase {
            case_id: "tests/micro/case.json".to_string(),
            area: FixtureArea::Graphs,
            repo_path: "tests/micro/case.json".to_string(),
            expected: Vec::new(),
            observed: Vec::new(),
        };

        let prepared = adapter.prepare_case(&manifest, &case).unwrap();

        assert_eq!(
            prepared.target_files,
            [root.path().join("tests/micro/app.js")]
        );
    }

    #[test]
    fn jelly_entry_extraction_selects_intended_files() {
        let root = tempdir().unwrap();
        let tests = root.path().join("tests/micro");
        std::fs::create_dir_all(&tests).unwrap();
        std::fs::write(tests.join("entry.js"), "function main() {}\n").unwrap();
        std::fs::write(tests.join("helper.js"), "function helper() {}\n").unwrap();
        std::fs::write(
            tests.join("entry.json"),
            r#"{"entries":["entry.js","helper.js"],"files":["entry.js","helper.js"],"functions":["0:1:1:1:20"],"calls":[],"fun2fun":[],"call2fun":[]}"#,
        )
        .unwrap();
        let manifest = manifest(root.path().to_string_lossy().as_ref());
        let case = EvaluationCase {
            case_id: "tests/micro/entry.json".to_string(),
            area: FixtureArea::Graphs,
            repo_path: "tests/micro/entry.json".to_string(),
            expected: Vec::new(),
            observed: Vec::new(),
        };

        let prepared = JellyCallgraphAdapter
            .prepare_case(&manifest, &case)
            .unwrap();

        assert_eq!(
            prepared
                .target_files
                .iter()
                .map(|path| path
                    .strip_prefix(root.path())
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"))
                .collect::<Vec<_>>(),
            vec!["tests/micro/entry.js", "tests/micro/helper.js"]
        );
    }

    #[test]
    fn suite_native_metrics_count_cases_and_expected_edges() {
        let adapter = JellyCallgraphAdapter;
        let manifest = manifest("research/evaluation-harness/repos/jelly");
        let case = CaseResult {
            case_id: "case".to_string(),
            area: FixtureArea::Graphs,
            expected: vec![ExpectedItem::GraphEdge(ExpectedGraphEdge {
                graph: "jelly.call_graph.fun2fun".to_string(),
                from: "app.js:1:1:1:8".to_string(),
                to: "app.js:2:1:2:8".to_string(),
                mode: AssertionMode::Exact,
                partial_truth: false,
            })],
            observed: Vec::new(),
            matches: Vec::new(),
            runtime: crate::eval::report::RuntimeObservation {
                budget_name: "eval-runner".to_string(),
                budget_passed: true,
                observed_runtime_ms: None,
            },
        };

        let metrics = adapter.suite_native_metrics(&manifest, &[case]).unwrap();

        assert_eq!(metrics["jelly.callgraph_micro.case_count"], 1.0);
        assert_eq!(metrics["jelly.callgraph_micro.expected_edge_count"], 1.0);
    }

    fn manifest(path: &str) -> SuiteManifest {
        SuiteManifest {
            schema_version: "polint-eval-suite-1".to_string(),
            id: SuiteId("jelly-callgraph-micro".to_string()),
            name: "Jelly JS/TS callgraph micro".to_string(),
            kind: SuiteKind::CallGraphPrecision,
            languages: vec!["javascript".to_string(), "typescript".to_string()],
            adapter_id: "jelly_callgraph_micro".to_string(),
            source_url: Some("https://github.com/cs-au-dk/jelly".to_string()),
            source_commit: Some("b799ed4f0d68c670fe398830aaa51dd5c628cf74".to_string()),
            license: "Apache-2.0".to_string(),
            language_support: SuiteLanguageSupport::Supported,
            checkout: SuiteCheckout {
                strategy: SuiteCheckoutStrategy::LocalClone,
                path: path.to_string(),
                ignored_by_git: true,
                local_clone_policy: LocalClonePolicy::AllowAbsolute,
            },
            expected: ExpectedSource {
                format: ExpectedSourceFormat::SuiteNative,
                path: "suite-native-jelly-callgraph".to_string(),
            },
            scoring: SuiteScoring {
                native: vec![
                    "jelly.callgraph_micro.case_count".to_string(),
                    "jelly.callgraph_micro.expected_edge_count".to_string(),
                ],
                unified: vec!["precision".to_string(), "recall".to_string()],
            },
            tiers: BTreeMap::from([(
                SuiteTier::Fast,
                CaseSelector {
                    enabled: true,
                    selector: "sample:balanced:20".to_string(),
                    max_cases: Some(20),
                    deterministic_seed: Some("jelly-callgraph-fast".to_string()),
                },
            )]),
        }
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|path| path.parent())
            .expect("workspace root")
            .to_path_buf()
    }
}
