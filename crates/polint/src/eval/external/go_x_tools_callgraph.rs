use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[cfg(test)]
use crate::analysis_kernel::KernelOutput;
use crate::eval::adapter::{BenchmarkAdapter, PreparedCase, RawObservedOutput};
#[cfg(test)]
use crate::eval::model::ObservedGraphEdge;
use crate::eval::model::{
    AssertionMode, EvaluationCase, ExpectedGraphEdge, ExpectedItem, FixtureArea, ObservedItem,
};
use crate::eval::report::CaseResult;
use crate::eval::runner::safe_join_workspace;
use crate::eval::suite::{SuiteLanguageSupport, SuiteManifest, normalize_repo_relative_path};

pub(crate) struct GoXToolsCallgraphAdapter;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct GoXToolsCallgraphEnumeration {
    pub(crate) cases: Vec<EvaluationCase>,
    pub(crate) limitations: Vec<String>,
}

impl BenchmarkAdapter for GoXToolsCallgraphAdapter {
    fn adapter_id(&self) -> &'static str {
        "go_x_tools_rta_callgraph"
    }

    fn language_support(&self, manifest: &SuiteManifest) -> SuiteLanguageSupport {
        manifest.language_support
    }

    fn enumerate_cases(&self, manifest: &SuiteManifest) -> anyhow::Result<Vec<EvaluationCase>> {
        Ok(enumerate_go_x_tools_callgraph_cases(Path::new(&manifest.checkout.path))?.cases)
    }

    fn prepare_case(
        &self,
        manifest: &SuiteManifest,
        case: &EvaluationCase,
    ) -> anyhow::Result<PreparedCase> {
        let root = PathBuf::from(&manifest.checkout.path);
        let target = safe_join_workspace(&root, Path::new(&case.repo_path))?;
        Ok(PreparedCase {
            case_id: case.case_id.clone(),
            workspace_root: root,
            target_files: vec![target],
        })
    }

    #[cfg(test)]
    fn prepare_case_with_scratch(
        &self,
        manifest: &SuiteManifest,
        case: &EvaluationCase,
        scratch_root: &Path,
    ) -> anyhow::Result<PreparedCase> {
        materialize_go_x_tools_txtar_case(Path::new(&manifest.checkout.path), case, scratch_root)
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
                "go_x_tools.rta_callgraph.case_count".to_string(),
                results.len() as f64,
            ),
            (
                "go_x_tools.rta_callgraph.expected_edge_count".to_string(),
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
        // Per D-05 Go function/method names are rendered ONLY by
        // `analysis::identity::render::go_relstring::render`. This adapter routes
        // each edge endpoint through that renderer (the single source of truth)
        // and then projects to the RTA `WANT:` oracle key, which uses the bare
        // function name with the `main.` package prefix stripped.
        let rta_edges = output.db.go_semantic_rta_edges();
        if !rta_edges.is_empty() {
            return Ok(rta_edges
                .iter()
                .map(|edge| {
                    ObservedItem::GraphEdge(ObservedGraphEdge {
                        graph: format!(
                            "go.rta.call_graph.{}",
                            normalize_graph_segment(&edge.edge_kind)
                        ),
                        from: edge.caller.clone(),
                        to: edge.callee.clone(),
                        mode: AssertionMode::Exact,
                        partial_truth: false,
                        producer_id: Some("polint.go.semantic.rta".to_string()),
                        provenance: Some("x_tools_rta".to_string()),
                        precision: Some("OracleCompatible".to_string()),
                        status: Some(crate::eval::model::ObservedStatus::Resolved),
                    })
                })
                .collect());
        }

        let records = output.db.identity_records();
        let mut items = crate::eval::observed::graph_edges_from_kernel_output(output)
            .into_iter()
            .filter_map(|edge| {
                let graph_kind = go_rta_graph_kind(&edge.edge_kind, &edge.algorithm)?;
                Some(ObservedItem::GraphEdge(ObservedGraphEdge {
                    graph: format!("go.rta.call_graph.{graph_kind}"),
                    from: go_rta_oracle_identity(records, &edge.caller_span, &edge.caller_name),
                    to: edge
                        .target_span
                        .as_ref()
                        .map(|span| go_rta_oracle_identity(records, span, &edge.target_name))
                        .unwrap_or_else(|| go_x_tools_function_identity(&edge.target_name)),
                    mode: AssertionMode::Exact,
                    partial_truth: false,
                    producer_id: Some(edge.producer_id),
                    provenance: Some(edge.provenance),
                    precision: Some(edge.precision),
                    status: Some(edge.status),
                }))
            })
            .collect::<Vec<_>>();
        items.extend(crate::eval::observed::call_graph_unknown_facts_from_kernel_output(output));
        Ok(items)
    }
}

/// Projects a Go call-graph endpoint to the RTA `WANT:` oracle key.
///
/// The Go `RelString` form is produced by the single-source-of-truth renderer
/// (`analysis::identity::render::go_relstring::render`, D-05) for the identity
/// record whose span matches the edge endpoint. The RTA oracle uses bare names
/// with the `main.` package prefix dropped, so we derive that oracle key from
/// the record's display name; when no identity record matches the span we fall
/// back to the raw edge name with the same `main.` stripping.
#[cfg(test)]
fn go_rta_oracle_identity(
    records: &[crate::analysis::identity::facts::IdentityRecord],
    span: &crate::core::Span,
    fallback_name: &str,
) -> String {
    match records.iter().find(|record| record.span == *span) {
        Some(record) => {
            // Exercise the renderer as the single source of truth (D-05) and
            // assert it produced a non-empty RelString rather than discarding the
            // output. The RelString currently carries Go package-NAME qualification
            // (`foo.Bar`) after Plan 05.
            let rel_string = crate::analysis::identity::render::go_relstring::render(record);
            debug_assert!(
                !rel_string.is_empty(),
                "go_relstring render must be non-empty for {}",
                record.stable_key
            );
            // The x/tools RTA `WANT:` oracle uses bare names, so benchmark matching
            // intentionally stays on display_name even when RelString contains a
            // full semantic package import path.
            go_x_tools_function_identity(record.display_name.as_ref())
        }
        None => go_x_tools_function_identity(fallback_name),
    }
}

pub(crate) fn enumerate_go_x_tools_callgraph_cases(
    root: &Path,
) -> anyhow::Result<GoXToolsCallgraphEnumeration> {
    if !root.exists() {
        return Ok(GoXToolsCallgraphEnumeration {
            cases: Vec::new(),
            limitations: vec![format!(
                "Go x/tools local clone is absent at {}; suite can be planned but not executed",
                root.display()
            )],
        });
    }

    let rta_testdata = root.join("go/callgraph/rta/testdata");
    let mut archives = Vec::new();
    collect_txtar_files(root, &rta_testdata, &mut archives)?;
    archives.sort();

    let cases = archives
        .into_iter()
        .map(|relative| go_x_tools_case(root, relative))
        .collect::<anyhow::Result<Vec<_>>>()?
        .into_iter()
        .filter(|case| !case.expected.is_empty())
        .collect();

    Ok(GoXToolsCallgraphEnumeration {
        cases,
        limitations: vec![
            "Go x/tools RTA expectations are a Go-native call graph oracle for RTA examples, not a full product-level graph benchmark".to_string(),
            "Negative reachability and runtime-type assertions are preserved by the upstream suite but not scored by this call-edge adapter yet".to_string(),
        ],
    })
}

fn collect_txtar_files(root: &Path, current: &Path, files: &mut Vec<String>) -> anyhow::Result<()> {
    if !current.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_txtar_files(root, &path, files)?;
            continue;
        }
        if path.extension().and_then(|extension| extension.to_str()) == Some("txtar") {
            let relative = path.strip_prefix(root)?;
            files.push(relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn go_x_tools_case(root: &Path, relative_path: String) -> anyhow::Result<EvaluationCase> {
    let relative_path = normalize_repo_relative_path("go x/tools case path", &relative_path)?;
    let text = std::fs::read_to_string(root.join(&relative_path))?;
    let expected = parse_rta_want_edges(&text)
        .into_iter()
        .map(ExpectedItem::GraphEdge)
        .collect();

    Ok(EvaluationCase {
        case_id: relative_path.clone(),
        area: FixtureArea::Graphs,
        repo_path: relative_path,
        expected,
        observed: Vec::new(),
    })
}

#[cfg(test)]
pub(crate) fn materialize_go_x_tools_txtar_case(
    root: &Path,
    case: &EvaluationCase,
    scratch_root: &Path,
) -> anyhow::Result<PreparedCase> {
    let archive_path = safe_join_workspace(root, Path::new(&case.repo_path))?;
    let text = std::fs::read_to_string(&archive_path)?;
    let files = parse_txtar_files(&text)?;
    anyhow::ensure!(
        files.iter().any(|(path, _)| path == Path::new("go.mod")),
        "Go x/tools txtar case {} has no go.mod",
        case.repo_path
    );

    let case_dir = scratch_root.join(materialized_case_dir_name(&case.case_id));
    std::fs::create_dir_all(&case_dir)?;
    let mut target_files = Vec::new();
    for (relative, contents) in files {
        let path = safe_join_workspace(&case_dir, &relative)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(
            &path,
            adapted_go_x_tools_file_contents(&relative, &contents),
        )?;
        if path.extension().and_then(|extension| extension.to_str()) == Some("go") {
            target_files.push(path);
        }
    }
    target_files.sort();

    Ok(PreparedCase {
        case_id: case.case_id.clone(),
        workspace_root: case_dir,
        target_files,
    })
}

#[cfg(test)]
fn adapted_go_x_tools_file_contents(path: &Path, contents: &str) -> String {
    if path.extension().and_then(|extension| extension.to_str()) != Some("go") {
        return contents.to_string();
    }
    contents
        .lines()
        .map(|line| {
            if line.trim() == "func use(interface{})" {
                format!(
                    "{}func use(interface{{}}) {{}}",
                    line.chars()
                        .take_while(|ch| ch.is_whitespace())
                        .collect::<String>()
                )
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if contents.ends_with('\n') { "\n" } else { "" }
}

#[cfg(test)]
fn parse_txtar_files(text: &str) -> anyhow::Result<Vec<(PathBuf, String)>> {
    let mut files = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_contents = String::new();

    for line in text.lines() {
        if let Some(path) = txtar_marker_path(line) {
            if let Some(previous) = current_path.replace(path) {
                files.push((previous, std::mem::take(&mut current_contents)));
            }
            continue;
        }
        if current_path.is_some() {
            current_contents.push_str(line);
            current_contents.push('\n');
        }
    }
    if let Some(path) = current_path {
        files.push((path, current_contents));
    }

    anyhow::ensure!(!files.is_empty(), "txtar archive has no files");
    Ok(files)
}

#[cfg(test)]
fn txtar_marker_path(line: &str) -> Option<PathBuf> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix("-- ")?.strip_suffix(" --")?;
    let normalized = normalize_repo_relative_path("txtar file path", inner).ok()?;
    Some(PathBuf::from(normalized))
}

fn parse_rta_want_edges(text: &str) -> Vec<ExpectedGraphEdge> {
    let mut in_want = false;
    let mut edges = Vec::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line == "// WANT:" {
            in_want = true;
            continue;
        }
        if !in_want {
            continue;
        }
        let Some(line) = line.strip_prefix("//") else {
            continue;
        };
        let line = line.trim();
        if line.is_empty() || line.starts_with('!') || !line.starts_with("edge ") {
            continue;
        }
        let rest = line.trim_start_matches("edge ").trim();
        let Some((from, after_from)) = rest.split_once(" --") else {
            continue;
        };
        let Some((kind, to)) = after_from.split_once("--> ") else {
            continue;
        };
        edges.push(ExpectedGraphEdge {
            graph: format!("go.rta.call_graph.{}", normalize_graph_segment(kind)),
            from: from.trim().to_string(),
            to: to.trim().to_string(),
            mode: AssertionMode::Partial,
            partial_truth: true,
        });
    }

    edges
}

fn normalize_graph_segment(value: &str) -> String {
    value
        .trim_matches('-')
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

#[cfg(test)]
fn sanitize_case_id(case_id: &str) -> String {
    let sanitized = case_id
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if sanitized.is_empty() {
        "case".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
fn materialized_case_dir_name(case_id: &str) -> String {
    let hash = crate::cache::stable_hash(&[case_id]);
    format!("{}-{}", sanitize_case_id(case_id), &hash[..8])
}

#[cfg(test)]
fn go_rta_graph_kind(edge_kind: &str, algorithm: &str) -> Option<&'static str> {
    match (edge_kind, algorithm) {
        ("Method" | "MethodDirect", "GoCha" | "GoRta" | "GoVta" | "TypeHierarchy") => {
            Some("dynamic_method_call")
        }
        ("FunctionValue", _) | (_, "FunctionTokenFlow" | "PointsTo") => {
            Some("dynamic_function_call")
        }
        ("Method" | "MethodDirect", _) => Some("static_method_call"),
        ("Direct" | "Constructor" | "StaticMember" | "Synthetic" | "Spawn" | "Deferred", _) => {
            Some("static_function_call")
        }
        _ => None,
    }
}

#[cfg(test)]
fn go_x_tools_function_identity(name: &str) -> String {
    name.strip_prefix("main.").unwrap_or(name).to_string()
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
        let enumeration = enumerate_go_x_tools_callgraph_cases(&missing).unwrap();

        assert!(enumeration.cases.is_empty());
        assert!(enumeration.limitations[0].contains("local clone is absent"));
    }

    #[test]
    fn parses_rta_want_edges_from_txtar_comments() {
        let text = r#"
-- go.mod --
module example.com
-- main.go --
package main
// WANT:
//
//  edge main --dynamic function call--> init$1
//  reachable main
// !reachable B
"#;

        let edges = parse_rta_want_edges(text);

        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].graph, "go.rta.call_graph.dynamic_function_call");
        assert_eq!(edges[0].from, "main");
        assert_eq!(edges[0].to, "init$1");
    }

    #[test]
    fn enumerates_txtar_cases_deterministically() {
        let root = tempdir().unwrap();
        let testdata = root.path().join("go/callgraph/rta/testdata");
        std::fs::create_dir_all(&testdata).unwrap();
        std::fs::write(
            testdata.join("b.txtar"),
            "// WANT:\n// edge B --static function call--> C\n",
        )
        .unwrap();
        std::fs::write(
            testdata.join("a.txtar"),
            "// WANT:\n// edge A --dynamic function call--> B\n",
        )
        .unwrap();

        let enumeration = enumerate_go_x_tools_callgraph_cases(root.path()).unwrap();

        assert_eq!(
            enumeration
                .cases
                .iter()
                .map(|case| case.case_id.as_str())
                .collect::<Vec<_>>(),
            [
                "go/callgraph/rta/testdata/a.txtar",
                "go/callgraph/rta/testdata/b.txtar"
            ]
        );
        assert_eq!(enumeration.cases[0].expected.len(), 1);
    }

    #[test]
    fn local_go_x_tools_expected_edge_parser_preserves_current_oracle_count() {
        let root = repo_root().join("research/evaluation-harness/repos/golang-tools");
        if !root.exists() {
            return;
        }
        let enumeration = enumerate_go_x_tools_callgraph_cases(&root).unwrap();
        let expected_edges = enumeration
            .cases
            .iter()
            .flat_map(|case| case.expected.iter())
            .filter(|item| matches!(item, ExpectedItem::GraphEdge(_)))
            .count();

        assert_eq!(expected_edges, 37);
    }

    #[test]
    fn txtar_materialization_creates_valid_temp_repo() {
        let root = tempdir().unwrap();
        let testdata = root.path().join("go/callgraph/rta/testdata");
        std::fs::create_dir_all(&testdata).unwrap();
        std::fs::write(
            testdata.join("case.txtar"),
            "-- go.mod --\nmodule example.com\n-- main.go --\npackage main\nfunc main() {}\n",
        )
        .unwrap();
        let case = EvaluationCase {
            case_id: "go/callgraph/rta/testdata/case.txtar".to_string(),
            area: FixtureArea::Graphs,
            repo_path: "go/callgraph/rta/testdata/case.txtar".to_string(),
            expected: Vec::new(),
            observed: Vec::new(),
        };
        let scratch = tempdir().unwrap();

        let prepared =
            materialize_go_x_tools_txtar_case(root.path(), &case, scratch.path()).unwrap();

        assert!(prepared.workspace_root.join("go.mod").is_file());
        assert_eq!(prepared.target_files.len(), 1);
        assert!(prepared.target_files[0].ends_with("main.go"));
    }

    #[test]
    fn txtar_materialization_uses_distinct_dirs_for_colliding_sanitized_case_ids() {
        let root = tempdir().unwrap();
        let testdata = root.path().join("go/callgraph/rta/testdata/a");
        std::fs::create_dir_all(&testdata).unwrap();
        std::fs::write(
            testdata.join("b.txtar"),
            "-- go.mod --\nmodule example.com/one\n-- old.go --\npackage main\nfunc old() {}\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("go/callgraph/rta/testdata/a-b.txtar"),
            "-- go.mod --\nmodule example.com/two\n-- main.go --\npackage main\nfunc main() {}\n",
        )
        .unwrap();
        let scratch = tempdir().unwrap();
        let first = EvaluationCase {
            case_id: "go/callgraph/rta/testdata/a/b.txtar".to_string(),
            area: FixtureArea::Graphs,
            repo_path: "go/callgraph/rta/testdata/a/b.txtar".to_string(),
            expected: Vec::new(),
            observed: Vec::new(),
        };
        let second = EvaluationCase {
            case_id: "go/callgraph/rta/testdata/a-b.txtar".to_string(),
            area: FixtureArea::Graphs,
            repo_path: "go/callgraph/rta/testdata/a-b.txtar".to_string(),
            expected: Vec::new(),
            observed: Vec::new(),
        };

        let first_prepared =
            materialize_go_x_tools_txtar_case(root.path(), &first, scratch.path()).unwrap();
        let second_prepared =
            materialize_go_x_tools_txtar_case(root.path(), &second, scratch.path()).unwrap();

        assert_ne!(
            first_prepared.workspace_root,
            second_prepared.workspace_root
        );
        assert!(!second_prepared.workspace_root.join("old.go").exists());
        assert!(second_prepared.workspace_root.join("main.go").exists());
    }

    #[test]
    fn adapter_prepares_txtar_target_inside_clone() {
        let manifest = manifest("research/evaluation-harness/repos/golang-tools");
        let adapter = GoXToolsCallgraphAdapter;
        let case = EvaluationCase {
            case_id: "go/callgraph/rta/testdata/func.txtar".to_string(),
            area: FixtureArea::Graphs,
            repo_path: "go/callgraph/rta/testdata/func.txtar".to_string(),
            expected: Vec::new(),
            observed: Vec::new(),
        };

        let prepared = adapter.prepare_case(&manifest, &case).unwrap();

        assert_eq!(prepared.case_id, "go/callgraph/rta/testdata/func.txtar");
        assert_eq!(prepared.target_files.len(), 1);
    }

    #[test]
    fn suite_native_metrics_count_cases_and_expected_edges() {
        let adapter = GoXToolsCallgraphAdapter;
        let manifest = manifest("research/evaluation-harness/repos/golang-tools");
        let case = CaseResult {
            case_id: "case".to_string(),
            area: FixtureArea::Graphs,
            expected: vec![ExpectedItem::GraphEdge(ExpectedGraphEdge {
                graph: "go.rta.call_graph.static_function_call".to_string(),
                from: "main".to_string(),
                to: "A".to_string(),
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

        assert_eq!(metrics["go_x_tools.rta_callgraph.case_count"], 1.0);
        assert_eq!(metrics["go_x_tools.rta_callgraph.expected_edge_count"], 1.0);
    }

    fn manifest(path: &str) -> SuiteManifest {
        SuiteManifest {
            schema_version: "polint-eval-suite-1".to_string(),
            id: SuiteId("go-x-tools-rta-callgraph".to_string()),
            name: "Go x/tools RTA callgraph".to_string(),
            kind: SuiteKind::CallGraphPrecision,
            languages: vec!["go".to_string()],
            adapter_id: "go_x_tools_rta_callgraph".to_string(),
            scoring_mode: crate::eval::suite::ScoringMode::OracleRta,
            source_url: Some("https://github.com/golang/tools".to_string()),
            source_commit: Some("7743a285e3d261ca235408e013ec5c14cb5170e4".to_string()),
            license: "BSD-3-Clause".to_string(),
            language_support: SuiteLanguageSupport::Supported,
            checkout: SuiteCheckout {
                strategy: SuiteCheckoutStrategy::LocalClone,
                path: path.to_string(),
                ignored_by_git: true,
                local_clone_policy: LocalClonePolicy::RepoRelativeOnly,
            },
            expected: ExpectedSource {
                format: ExpectedSourceFormat::SuiteNative,
                path: "suite-native-go-x-tools-rta-callgraph".to_string(),
            },
            scoring: SuiteScoring {
                native: vec![
                    "go_x_tools.rta_callgraph.case_count".to_string(),
                    "go_x_tools.rta_callgraph.expected_edge_count".to_string(),
                ],
                unified: vec!["precision".to_string(), "recall".to_string()],
            },
            tiers: BTreeMap::from([(
                SuiteTier::Fast,
                CaseSelector {
                    enabled: true,
                    selector: "sample:balanced:6".to_string(),
                    max_cases: Some(6),
                    deterministic_seed: Some("go-x-tools-rta-fast".to_string()),
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
