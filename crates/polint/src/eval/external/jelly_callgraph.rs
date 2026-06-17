use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use serde::Deserialize;

#[cfg(test)]
use crate::analysis_kernel::KernelOutput;
#[cfg(test)]
use crate::core::Span;
use crate::eval::adapter::{BenchmarkAdapter, PreparedCase, RawObservedOutput};
use crate::eval::model::{
    AssertionMode, EvaluationCase, ExpectedGraphEdge, ExpectedItem, FixtureArea, ObservedGraphEdge,
    ObservedItem,
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
        for file in graph.files.iter().chain(graph.entries.iter()) {
            let path = safe_join_workspace(&root, &case_dir.join(file))?;
            if path.is_file() && is_jelly_source_file(&path) {
                target_files.push(path);
            }
        }
        target_files.sort();
        target_files.dedup();
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
        // Jelly is demand-driven: it only emits call edges from functions reached
        // from the loaded modules' execution, so its oracle is reachability-pruned
        // (helloworld loads 81 files but emits edges from only 35 — the rest are
        // required but never invoked). polint analyzes every function body, so to
        // compare like-for-like we filter observed edges to those whose caller is
        // reachable. Reachability roots are module executions plus the functions
        // the value flow actually executed (host-invoked callbacks such as promise
        // handlers, which have no incoming call edge), propagated through resolved
        // calls. See `jelly_reachable_caller_spans`.
        let reachable_caller_spans = jelly_reachable_caller_spans(&output.db);
        // Diagnostic escape hatch: `POLINT_JELLY_NO_PRUNE=1` disables the
        // reachability filter so every resolved edge is observed. Diffing an FN set
        // against the pruned run partitions the misses into "computed-but-pruned"
        // (resolved by a provider, dropped here) vs "unresolved" (no provider finds
        // it). NOT part of scoring — for FN decomposition only.
        let no_prune = std::env::var_os("POLINT_JELLY_NO_PRUNE").is_some();
        let mut items = Vec::new();
        let mut seen_graph_edges = BTreeSet::new();
        for edge in crate::eval::observed::graph_edges_from_kernel_output(output) {
            if !no_prune
                && !reachable_caller_spans.contains(&(
                    edge.caller_span.file,
                    edge.caller_span.start_byte,
                    edge.caller_span.end_byte,
                ))
            {
                continue;
            }
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
            push_unique_jelly_graph_edge(
                &mut items,
                &mut seen_graph_edges,
                ObservedGraphEdge {
                    graph: "jelly.call_graph.call2fun".to_string(),
                    from: call_site,
                    to: target.clone(),
                    mode: AssertionMode::Exact,
                    partial_truth: false,
                    producer_id: Some(edge.producer_id.clone()),
                    provenance: Some(edge.provenance.clone()),
                    precision: Some(edge.precision.clone()),
                    status: Some(edge.status),
                },
            );
            push_unique_jelly_graph_edge(
                &mut items,
                &mut seen_graph_edges,
                ObservedGraphEdge {
                    graph: "jelly.call_graph.fun2fun".to_string(),
                    from: caller,
                    to: target,
                    mode: AssertionMode::Exact,
                    partial_truth: false,
                    producer_id: Some(edge.producer_id),
                    provenance: Some(edge.provenance),
                    precision: Some(edge.precision),
                    status: Some(edge.status),
                },
            );
        }
        items.extend(crate::eval::observed::call_graph_unknown_facts_from_kernel_output(output));
        Ok(items)
    }
}

/// Compute the set of function spans reachable under Jelly's demand-driven
/// semantics, keyed by `(file, start_byte, end_byte)`.
///
/// Reachability roots are:
/// - every synthetic TS/JS module function — Jelly module-evaluates all loaded
///   modules, so their top-level code is always reachable; and
/// - every function the value flow actually *executed* — the value flow only
///   emits `FunctionTokenFlow` edges from a body it walks, and it walks a body
///   only when that function is invoked. This captures host-invoked callbacks
///   (promise/iterator handlers) that have no incoming call edge, distinguishing
///   them from genuinely dead functions whose calls are resolved only by the
///   import-binding/direct resolver (e.g. `body-parser/lib/read.js` in
///   helloworld, never invoked from the entry).
///
/// Reachability then propagates through resolved call edges (both `call_targets`
/// and `refined_call_edges`), so cross-file targets such as express's
/// `createApplication` are reached via the resolved `express()` edge.
#[cfg(test)]
fn jelly_reachable_caller_spans(
    db: &crate::core::AnalysisDb,
) -> BTreeSet<(crate::core::FileId, u32, u32)> {
    use crate::analysis::calls::facts::CallAlgorithm;
    use crate::core::{FunctionId, is_synthetic_ts_js_module_function};

    let mut adjacency: BTreeMap<FunctionId, Vec<FunctionId>> = BTreeMap::new();
    let mut reachable: BTreeSet<FunctionId> = BTreeSet::new();
    let mut stack: Vec<FunctionId> = Vec::new();

    for target in db.call_targets() {
        if let Some(callee) = target.target_function {
            adjacency.entry(target.caller).or_default().push(callee);
        }
        // A `FunctionTokenFlow` edge is emitted only from a body the value flow
        // executed, so its caller was invoked — seed it as a root.
        if target.algorithm == CallAlgorithm::FunctionTokenFlow && reachable.insert(target.caller) {
            stack.push(target.caller);
        }
    }
    for edge in db.refined_call_edges() {
        if let Some(callee) = edge.target_function {
            adjacency.entry(edge.caller).or_default().push(callee);
        }
    }
    for function in db.functions() {
        if is_synthetic_ts_js_module_function(function) && reachable.insert(function.id) {
            stack.push(function.id);
        }
    }
    while let Some(function) = stack.pop() {
        if let Some(callees) = adjacency.get(&function) {
            for &callee in callees {
                if reachable.insert(callee) {
                    stack.push(callee);
                }
            }
        }
    }

    db.functions()
        .iter()
        .filter(|function| reachable.contains(&function.id))
        .map(|function| {
            (
                function.span.file,
                function.span.start_byte,
                function.span.end_byte,
            )
        })
        .collect()
}

fn push_unique_jelly_graph_edge(
    items: &mut Vec<ObservedItem>,
    seen: &mut BTreeSet<(String, String, String)>,
    edge: ObservedGraphEdge,
) {
    let key = (edge.graph.clone(), edge.from.clone(), edge.to.clone());
    if seen.insert(key) {
        items.push(ObservedItem::GraphEdge(edge));
    }
}

fn is_jelly_source_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts")
    )
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
    let case_dir = Path::new(&relative_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let expected: Vec<ExpectedItem> = jelly_expected_edges(&graph, case_dir)?
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

fn jelly_expected_edges(
    graph: &JellyCallgraph,
    case_dir: &Path,
) -> anyhow::Result<Vec<ExpectedGraphEdge>> {
    let mut edges = Vec::new();
    for [from, to] in &graph.fun2fun {
        edges.push(ExpectedGraphEdge {
            graph: "jelly.call_graph.fun2fun".to_string(),
            from: canonical_location(&graph.files, &graph.functions, *from, case_dir)?,
            to: canonical_location(&graph.files, &graph.functions, *to, case_dir)?,
            mode: AssertionMode::Exact,
            partial_truth: false,
        });
    }
    for [from, to] in &graph.call2fun {
        edges.push(ExpectedGraphEdge {
            graph: "jelly.call_graph.call2fun".to_string(),
            from: canonical_location(&graph.files, &graph.calls, *from, case_dir)?,
            to: canonical_location(&graph.files, &graph.functions, *to, case_dir)?,
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
    case_dir: &Path,
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
    let file = canonical_case_file_path(case_dir, file)?;
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

fn canonical_case_file_path(case_dir: &Path, file: &str) -> anyhow::Result<String> {
    let file = normalize_repo_relative_path("jelly graph file path", file)?;
    let joined = case_dir.join(file);
    normalize_repo_relative_path(
        "jelly resolved graph file path",
        &joined.to_string_lossy().replace('\\', "/"),
    )
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
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    use serde::Deserialize;
    use tempfile::tempdir;

    use super::*;
    use crate::analysis::identity::facts::{
        IdentityKind, IdentityRecord, IdentityRecordId, LanguageTag, compute_identity_stable_key,
        compute_signature_digest,
    };
    use crate::analysis::identity::render::jelly_span;
    use crate::core::{AnalysisDb, SourceFile, Span};
    use crate::eval::suite::{
        CaseSelector, ExpectedSource, ExpectedSourceFormat, LocalClonePolicy, SuiteCheckout,
        SuiteCheckoutStrategy, SuiteId, SuiteKind, SuiteScoring, SuiteTier,
    };
    use crate::ts::inventory::extract::extract_ts_inventory;
    use crate::ts::inventory::facts::{
        TsCallsiteInventoryKind, TsFunctionInventoryKind, TsInventoryStatus,
    };

    // ---- shared helpers for the points-to heap edge-resolution tests ---------

    /// Source text of a resolved call edge's target function (empty if unresolved).
    fn target_src(db: &AnalysisDb, t: &crate::analysis::calls::facts::CallTargetFact) -> String {
        t.target_function
            .and_then(|f| db.functions().iter().find(|x| x.id == f))
            .and_then(|x| {
                db.file(x.span.file).map(|file| {
                    file.source[x.span.start_byte as usize..x.span.end_byte as usize].to_string()
                })
            })
            .unwrap_or_default()
    }

    /// Source text of a resolved call edge's call site (empty if missing).
    fn site_src(db: &AnalysisDb, t: &crate::analysis::calls::facts::CallTargetFact) -> String {
        db.call_sites()
            .iter()
            .find(|s| s.id == t.site)
            .and_then(|s| {
                db.file(s.span.file).map(|file| {
                    file.source[s.span.start_byte as usize..s.span.end_byte as usize].to_string()
                })
            })
            .unwrap_or_default()
    }

    /// Does any resolved edge (by any algorithm) connect a site to a target, matched
    /// by source-text substrings?
    fn any_resolves(db: &AnalysisDb, site_needle: &str, target_needle: &str) -> bool {
        db.call_targets().iter().any(|t| {
            site_src(db, t).contains(site_needle) && target_src(db, t).contains(target_needle)
        })
    }

    /// Like [`any_resolves`] but only counts points-to heap edges
    /// (`CallAlgorithm::PointsTo`).
    fn heap_resolves(db: &AnalysisDb, site_needle: &str, target_needle: &str) -> bool {
        db.call_targets().iter().any(|t| {
            t.algorithm == crate::analysis::calls::facts::CallAlgorithm::PointsTo
                && site_src(db, t).contains(site_needle)
                && target_src(db, t).contains(target_needle)
        })
    }

    #[test]
    fn absent_clone_skips_gracefully() {
        let missing = tempdir().unwrap().path().join("missing");
        let enumeration = enumerate_jelly_callgraph_cases(&missing).unwrap();

        assert!(enumeration.cases.is_empty());
        assert!(enumeration.limitations[0].contains("local clone is absent"));
    }

    #[test]
    fn points_to_heap_generalizes_and_stays_precise_on_novel_code() {
        // Overfitting / soundness guard: NOVEL shapes not in the Jelly suite, with
        // both recall (should resolve) and PRECISION (must NOT resolve / must not
        // cross-contaminate) assertions. Catches the heap silently over-resolving
        // on out-of-distribution code.
        let dir = tempdir().unwrap();
        let root = dir.path();
        let src = concat!(
            "function leafA() { console.log('A'); }\n", // 1
            "function leafB() { console.log('B'); }\n", // 2
            "function unknownInput() {}\n",             // 3 (a real fn, but reached via unknown)
            // field sensitivity: two distinct objects, same property name.
            "const a = { run: leafA };\n", // 5
            "const b = { run: leafB };\n", // 6
            "a.run();\n",                  // 7  -> leafA ONLY
            "b.run();\n",                  // 8  -> leafB ONLY
            // novel closure-returning-closure depth (different from client1).
            "function wrap(f) { return function () { return function () { f(); }; }; }\n", // 10
            "wrap(leafA)()();\n", // 11 -> leafA
            // precision negative: callee from an unmodeled global -> resolves to nothing.
            "const ext = someGlobalThing();\n", // 13
            "ext.method();\n",                  // 14 -> NOTHING
            // precision negative: a genuinely unknowable computed key (from an
            // unmodeled source) -> no field resolution.
            "const holder = { x: leafA };\n", // 16
            "const k = someGlobalThing();\n", // 17
            "holder[k]();\n",                 // 18 -> NOTHING (key unknown)
        );
        std::fs::write(root.join("m.js"), src).unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        let fid = |needle: &str| {
            db.functions()
                .iter()
                .filter(|f| {
                    db.file(f.span.file).is_some_and(|file| {
                        file.source[f.span.start_byte as usize..f.span.end_byte as usize]
                            .contains(needle)
                    })
                })
                .min_by_key(|f| f.span.end_byte - f.span.start_byte)
                .map(|f| f.id)
        };
        let site_text = |id: crate::analysis::ids::CallSiteId| {
            db.call_sites().iter().find(|s| s.id == id).and_then(|s| {
                db.file(s.span.file).map(|file| {
                    file.source[s.span.start_byte as usize..s.span.end_byte as usize].to_string()
                })
            })
        };
        // All resolved (site-text, target-fid) pairs across BOTH edge sources.
        let edges: std::collections::BTreeSet<(String, crate::core::FunctionId)> = db
            .call_targets()
            .iter()
            .filter_map(|t| Some((site_text(t.site)?, t.target_function?)))
            .chain(
                db.refined_call_edges()
                    .iter()
                    .filter_map(|e| Some((site_text(e.site)?, e.target_function?))),
            )
            .collect();
        let resolves = |site: &str, target: crate::core::FunctionId| {
            edges.iter().any(|(s, t)| s.trim() == site && *t == target)
        };
        let (la, lb) = (fid("leafA").unwrap(), fid("leafB").unwrap());

        // RECALL on novel shapes.
        assert!(resolves("a.run()", la), "a.run() -> leafA");
        assert!(resolves("b.run()", lb), "b.run() -> leafB");
        // The captured param `f` is invoked inside the doubly-nested closure
        // (reached via `wrap(leafA)()()`): `f()` must resolve to the passed leafA.
        assert!(
            resolves("f()", la),
            "captured f() in the nested closure should reach leafA"
        );

        // PRECISION: field sensitivity must not cross-contaminate.
        assert!(
            !resolves("a.run()", lb),
            "a.run() must NOT reach leafB (field sensitivity)"
        );
        assert!(
            !resolves("b.run()", la),
            "b.run() must NOT reach leafA (field sensitivity)"
        );

        // PRECISION: unmodeled global and non-constant key resolve to NOTHING.
        let ext_targets = edges
            .iter()
            .filter(|(s, _)| s.trim() == "ext.method()")
            .count();
        assert_eq!(
            ext_targets, 0,
            "ext.method() on an unknown value must resolve to nothing"
        );
        // The non-const computed key must not dispatch to holder.x (leafA).
        assert!(
            !edges.iter().any(|(s, t)| s.contains("holder[") && *t == la),
            "non-constant computed key must not resolve to holder.x"
        );
    }

    #[test]
    fn object_literal_super_resolves() {
        // `super.m()` inside an object-literal method resolves against the object's
        // prototype, set via `Object.setPrototypeOf(o, p)` or `o.__proto__ = p`.
        // Two prior gaps: (1) object shorthand-method bodies were never lowered (the
        // MIR candidate span didn't match the property-span FunctionFact), so
        // `super.m1()` got no call site; (2) the value-flow didn't bind `super` for
        // object methods. A negative case guards against over-resolving `super` in
        // an object with no prototype.
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("o.js"),
            "var q1 = { m1() { console.log(\"q1.m1\"); } };\n\
             var q2 = { m2() { super.m1(); } };\n\
             Object.setPrototypeOf(q2, q1);\n\
             q2.m2();\n\
             var q3 = { m3() { super.m1(); } };\n\
             q3.__proto__ = q1;\n\
             q3.m3();\n\
             var lone = { z() { super.nope(); } };\n\
             lone.z();\n",
        )
        .unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        // setPrototypeOf path: q2.m2's super.m1() → q1.m1.
        assert!(
            any_resolves(db, "super.m1()", "console.log(\"q1.m1\")"),
            "super.m1() in an object method should resolve to the prototype's m1"
        );
        // PRECISION: an object with no prototype must NOT resolve super.nope().
        assert!(
            !any_resolves(db, "super.nope()", "z()"),
            "super in a prototype-less object must not resolve to the object itself"
        );
    }

    #[test]
    fn compound_receiver_method_this_resolves() {
        // `(o1 || o2).f()` resolves `f` against the union of o1/o2, and the method
        // body is walked with `this` = that union, so `this.g()` inside dispatches
        // to both g's. (receiver-callee-mixup.js.)
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("rc.js"),
            "const o1 = { f() { this.g(); }, g() { console.log(\"foo\"); } };\n\
             const o2 = { f() { this.g(); }, g() { console.log(\"bar\"); } };\n\
             (o1 || o2).f();\n\
             (o2 || o1).f();\n",
        )
        .unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        // `(o1 || o2)` short-circuits to o1 and `(o2 || o1)` to o2; each method body
        // is walked with `this` = its receiver, so `this.g()` reaches both g's.
        assert!(
            any_resolves(db, "this.g()", "console.log(\"foo\")")
                && any_resolves(db, "this.g()", "console.log(\"bar\")"),
            "this.g() should resolve to both o1.g and o2.g across the two compound calls"
        );
    }

    #[test]
    fn super_method_body_this_resolves() {
        // When `super.m1()` resolves, the super method's body is walked with `this`
        // bound to the calling receiver, so a `this.x()` inside it dispatches
        // against that object. (super3: `q2.m2(){ super.m1() }` →
        // `q1.m1(){ this.m3() }` with `this` = q2 → `this.m3()` → q2.m3.)
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("sm.js"),
            "var q1 = { m1() { this.m3(); } };\n\
             var q2 = {\n\
             \x20   m2() { super.m1(); },\n\
             \x20   m3() { console.log(\"q2.m3\"); },\n\
             };\n\
             Object.setPrototypeOf(q2, q1);\n\
             q2.m2();\n",
        )
        .unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        assert!(
            any_resolves(db, "this.m3()", "console.log(\"q2.m3\")"),
            "this.m3() in the super method should resolve to the receiver's m3"
        );
    }

    #[test]
    fn static_field_sequence_value_resolves() {
        // A field initialized to a comma-sequence takes its last operand as the
        // value, so `static p = (helper(), function(){})` makes `C.p` the trailing
        // function. (classes.js: `static staticProperty = (f1(), function(){});
        // C6.staticProperty()`.)
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("seq.js"),
            "function helper() {}\n\
             class C {\n\
             \x20   static p = (helper(), function() { console.log(\"v\"); });\n\
             }\n\
             C.p();\n",
        )
        .unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        assert!(
            any_resolves(db, "C.p()", "console.log(\"v\")"),
            "C.p() should resolve to the trailing function of the sequence initializer"
        );
    }

    #[test]
    fn super_constructor_argument_flows() {
        // `super(arg)` flows the subclass's argument into the superclass
        // constructor's parameter, so a parameter invoked in the superclass
        // constructor resolves to the passed value. (super.js: `class A
        // { constructor(x){ x() } } class B extends A { constructor(){ super(()=>…) } }`
        // → `x()` resolves to the passed arrow.)
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("sc.js"),
            "class A {\n\
             \x20   constructor(x) { x(); }\n\
             }\n\
             class B extends A {\n\
             \x20   constructor() { super(() => { console.log(\"cb\"); }); }\n\
             }\n\
             new B();\n",
        )
        .unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        assert!(
            any_resolves(db, "x()", "console.log(\"cb\")"),
            "x() in the superclass constructor should resolve to the arrow passed via super()"
        );
    }

    #[test]
    fn class_static_field_super_alias_resolves() {
        // `static g = super.f` aliases a static field to the superclass's static
        // member, so `Sub.g()` dispatches to it. (super.js: `static g = super.f;
        // B.g();` → A's static f arrow.)
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("alias.js"),
            "class A {\n\
             \x20   static f = () => { console.log(\"A.f\"); };\n\
             }\n\
             class B extends A {\n\
             \x20   static g = super.f;\n\
             }\n\
             B.g();\n",
        )
        .unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        assert!(
            any_resolves(db, "B.g()", "console.log(\"A.f\")"),
            "B.g() should resolve to A's static f via the super.f field alias"
        );
    }

    #[test]
    fn class_static_block_calls_resolve() {
        // A class static block (`static { … }`) executes at class-definition time
        // and is its own function in Jelly's model. Before this fix it was never
        // lowered, so direct calls inside it (`super.f()`, top-level `helper()`)
        // got no call site and never resolved. Now the block is emitted as a
        // span-named FunctionFact + MIR body, so its `super`/direct calls resolve.
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("statics.js"),
            "function helper() { console.log(\"helper\"); }\n\
             class A {\n\
             \x20   static f = () => { console.log(\"A.f\"); };\n\
             }\n\
             class B extends A {\n\
             \x20   static {\n\
             \x20       super.f();\n\
             \x20       helper();\n\
             \x20   }\n\
             }\n\
             new B();\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        // `super.f()` inside B's static block resolves to A's static `f` arrow.
        assert!(
            any_resolves(db, "super.f()", "console.log(\"A.f\")"),
            "super.f() in a static block should resolve to A's static f"
        );
        // A direct call to a top-level helper inside the static block resolves too.
        assert!(
            any_resolves(db, "helper()", "function helper"),
            "helper() in a static block should resolve to the top-level helper"
        );
    }

    #[test]
    fn class_field_initializer_calls_resolve() {
        // A class field initializer (`x = <expr>`) is its own function in Jelly's
        // model: calls in the initializer are attributed to it. Before this fix the
        // initializer was never lowered, so a call inside it got no call site.
        // Covers both a direct call buried in a sequence (static) and a `super` call
        // (instance) — the two field-init shapes in the Jelly suite (classes/super4).
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("fields.js"),
            "function helper() { console.log(\"helper\"); }\n\
             class A {\n\
             \x20   m() { console.log(\"A.m\"); }\n\
             }\n\
             class B extends A {\n\
             \x20   static prop = (helper(), function() {});\n\
             \x20   w = super.m();\n\
             }\n\
             new B();\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        // `helper()` inside the static field initializer's sequence resolves.
        assert!(
            any_resolves(db, "helper()", "function helper"),
            "helper() in a static field initializer should resolve"
        );
        // `super.m()` inside an instance field initializer resolves to A.m.
        assert!(
            any_resolves(db, "super.m()", "console.log(\"A.m\")"),
            "super.m() in an instance field initializer should resolve to A.m"
        );
    }

    #[test]
    fn throw_argument_calls_are_suppressed() {
        // A call lexically inside a `throw` argument sits on an error path the
        // demand-driven oracle does not exercise; resolving it produces false edges
        // (e.g. express's `gettype(fn)` in a middleware type-check throw). Such a
        // call must NOT resolve, while an identical call on the normal path does.
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("t.js"),
            "function throwhelper() {}\n\
             function normalhelper() {}\n\
             function f(x) {\n\
             \x20   if (!x) { throw new Error(\"bad \" + throwhelper()); }\n\
             \x20   normalhelper();\n\
             }\n\
             f(1);\n",
        )
        .unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        assert!(
            any_resolves(db, "normalhelper()", "function normalhelper"),
            "a call on the normal path must still resolve"
        );
        assert!(
            !any_resolves(db, "throwhelper()", "function throwhelper"),
            "a call inside a throw argument must be suppressed"
        );
    }

    #[test]
    fn absent_param_dead_branch_is_folded() {
        // array-flatten shape: a wrapper called without its `depth` arg takes the
        // `depth == null` branch; the `else` early-returns past dead code. With the
        // param known-absent the value flow folds the guard and never WALKS the dead
        // branch, so it emits NO value-flow (FunctionTokenFlow) edge there — while
        // the live branch does. (The points-to heap is flow-insensitive and may
        // still resolve the dead call; the fold operates at the value-flow layer.)
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("fl.js"),
            "function wrap(depth) {\n\
             \x20   var o = { live: () => { return 1; }, dead: () => { return 2; } };\n\
             \x20   if (depth == null) { o.live(); return; }\n\
             \x20   o.dead();\n\
             }\n\
             wrap();\n",
        )
        .unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        use crate::analysis::calls::facts::CallAlgorithm;
        let value_flow_resolves = |site: &str, tgt: &str| {
            db.call_targets().iter().any(|t| {
                t.algorithm == CallAlgorithm::FunctionTokenFlow
                    && site_src(db, t).contains(site)
                    && target_src(db, t).contains(tgt)
            })
        };
        assert!(
            value_flow_resolves("o.live()", "return 1"),
            "the live `depth == null` branch must resolve via value-flow"
        );
        assert!(
            !value_flow_resolves("o.dead()", "return 2"),
            "the dead `else` branch must be folded away (no value-flow walk)"
        );
    }

    #[test]
    fn points_to_heap_survives_adversarially_deep_nesting() {
        // Crash guard (C1): the harvester walks every JS/TS file in an arbitrary
        // repo, so deeply nested / generated input must not recurse the AST walk to
        // a stack overflow. `MAX_HARVEST_DEPTH` bounds the harvester's `expr` /
        // `const_key` recursion to a constant number of frames regardless of input
        // depth (it bails to an empty cell past the ceiling — sound, no edges).
        //
        // Run on an ample stack: the upstream oxc recursive-descent parser needs
        // stack proportional to nesting depth (a pre-existing, kernel-wide limit on
        // the default 2 MB test-thread stack), so we give the worker a generous
        // stack the way real analysis would. The point being tested is that the
        // *harvester* adds only bounded stack on top of that — the run completes
        // and still yields a usable db instead of aborting the process.
        let result = std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let dir = tempdir().unwrap();
                let root = dir.path();
                let depth = 2_000;
                // Single file (so parsing stays on this generous-stack thread; the
                // kernel fans multi-file parsing onto a small-stack worker pool).
                // The clean, resolvable call comes FIRST so it is harvested before
                // the pathological tail; then deep parens (expr recursion) and a
                // 2000-operand constant concatenation key (const_key recursion)
                // exercise the `MAX_HARVEST_DEPTH` guards without aborting.
                let mut src = String::from("function leaf() {}\nconst f = leaf;\nf();\n");
                src.push_str(&format!(
                    "const x = {}leaf{};\n",
                    "(".repeat(depth),
                    ")".repeat(depth)
                ));
                let concat = std::iter::repeat_n("\"a\"", depth)
                    .collect::<Vec<_>>()
                    .join(" + ");
                src.push_str(&format!("const obj = {{}};\nobj[{concat}];\n"));
                std::fs::write(root.join("m.js"), src).unwrap();

                // The core assertion is that this returns at all (no stack
                // overflow / abort). The kernel must also still produce a usable
                // db rather than crashing — `leaf` is extracted even though oxc
                // flags the pathologically deep tail and skips its value-flow.
                let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
                let db = &output.db;
                let leaf_analyzed = db.functions().iter().any(|f| {
                    db.file(f.span.file).is_some_and(|file| {
                        file.source[f.span.start_byte as usize..f.span.end_byte as usize]
                            .contains("leaf")
                    })
                });
                assert!(
                    leaf_analyzed,
                    "the kernel should still analyze the file's functions"
                );
            })
            .unwrap()
            .join();
        assert!(
            result.is_ok(),
            "deep-nesting analysis must not panic or overflow"
        );
    }

    #[test]
    fn points_to_heap_resolves_dynamic_property_and_cross_file_chains() {
        // End-to-end gate for the value-token heap (`js_points_to`): an instance
        // dynamic constant-key property call, and a cross-file factory-returns-object
        // chain, both of which the per-syntax recognizers miss. Resolved edges land
        // in `call_targets` (the heap is additive); the points-to edges are labeled
        // `CallAlgorithm::PointsTo`.
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("lib.js"),
            "function leaf() { console.log(\"leaf\"); }\nmodule.exports.make = function () { return { run: leaf }; };\n",
        )
        .unwrap();
        std::fs::write(
            root.join("main.js"),
            "const lib = require('./lib');\nconst o = lib.make();\nconst f = o.run;\nf();\nfunction Foo() {}\nconst g = function gg() { console.log(\"g\"); };\nconst p2 = \"p2\";\nconst inst = new Foo();\ninst[p2] = g;\ninst.p2();\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        // The instance dynamic-key call resolves only through the points-to heap.
        assert!(
            heap_resolves(db, "inst.p2()", "gg"),
            "inst.p2() should resolve to g via a PointsTo edge"
        );
        // The cross-file factory-returns-object chain resolves end to end.
        assert!(
            any_resolves(db, "f()", "function leaf"),
            "cross-file `const f = lib.make().run; f()` should resolve to leaf"
        );
    }

    #[test]
    fn points_to_heap_resolves_array_element_flow() {
        // Gate for the heap's array model (`js_points_to`): index-sensitive element
        // cells (`arr[const]` reads exactly its index), iteration callbacks
        // (`forEach`/`map`) binding the parameter to every element, `push` joining
        // the element set, and `for…of` binding the loop variable. All resolve
        // through the points-to heap (`CallAlgorithm::PointsTo`).
        let dir = tempdir().unwrap();
        let root = dir.path();
        // The *indirect* index form (`const y = arr[i]; y()`) is where the
        // recognizer's index-insensitive smear fails — the heap's index-sensitive
        // cells resolve it precisely. Iteration callbacks and `for…of` genuinely
        // visit every element, so binding to the summary stays precise.
        std::fs::write(
            root.join("arr.js"),
            "function a0() { console.log(\"a0\"); }\n\
             function a1() { console.log(\"a1\"); }\n\
             function pushed() { console.log(\"pushed\"); }\n\
             function looped() { console.log(\"looped\"); }\n\
             const xs = [a0, a1];\n\
             const y0 = xs[0];\n\
             y0();\n\
             xs.push(pushed);\n\
             xs.forEach(f => f());\n\
             const ys = [looped];\n\
             for (const g of ys) g();\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        // Index-sensitive read: y0 = xs[0] → a0, and it stays precise — it never
        // sees the *other* specific index a1.
        assert!(
            heap_resolves(db, "y0()", "function a0"),
            "the heap should resolve y0 = xs[0] to a0"
        );
        assert!(
            !heap_resolves(db, "y0()", "function a1"),
            "the heap must keep xs[0] precise — no smear to the index-1 element a1"
        );
        // `xs.forEach(f => f())`: the callback parameter is bound to every element
        // (both literal indices and the pushed value).
        assert!(
            heap_resolves(db, "f()", "function a0") && heap_resolves(db, "f()", "function a1"),
            "forEach should bind the callback parameter to the literal elements"
        );
        assert!(
            heap_resolves(db, "f()", "pushed"),
            "forEach should also see the pushed element (it joins %ARRAY_ALL)"
        );
        // `for (const g of ys) g()`: the loop variable is bound to the elements.
        assert!(
            heap_resolves(db, "g()", "looped"),
            "for…of should bind the loop variable to the array elements"
        );
    }

    #[test]
    fn value_flow_array_model_pop_and_index_stay_precise() {
        // Gate for the value-flow array model (`ts_value_flows` `CollectionTargets`),
        // mirroring `arrays.js`/`iterators.js`. The defect this guards against is the
        // old `%ALL`-smear where `pop`/`at` returned *every* element and a dynamic
        // index write polluted the value lane:
        //   * `arr.pop()` returns the appended (`push`) bucket only — NOT the literal
        //     numeric cells, and NOT a value written at an arbitrary index;
        //   * `arr.at(const)` reads exactly that numeric cell (no smear).
        // These resolve through the value flow (`CallAlgorithm::FunctionTokenFlow`).
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("arr.js"),
            "var arr = [\n\
             \x20 function lit0() { return 0; },\n\
             \x20 function lit1() { return 1; }\n\
             ];\n\
             arr[7] = function dynwrite() { return 3; };\n\
             arr.push(function appended() { return 2; });\n\
             var p = arr.pop();\n\
             p();\n\
             var picked = arr.at(0);\n\
             picked();\n",
        )
        .unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        use crate::analysis::calls::facts::CallAlgorithm;
        let value_flow_resolves = |site: &str, tgt: &str| {
            db.call_targets().iter().any(|t| {
                t.algorithm == CallAlgorithm::FunctionTokenFlow
                    && site_src(db, t).contains(site)
                    && target_src(db, t).contains(tgt)
            })
        };
        // `pop` reads the appended bucket only.
        assert!(
            value_flow_resolves("p()", "function appended"),
            "arr.pop() should resolve to the pushed (appended) element"
        );
        assert!(
            !value_flow_resolves("p()", "function lit0")
                && !value_flow_resolves("p()", "function lit1"),
            "arr.pop() must NOT smear to the literal numeric cells"
        );
        assert!(
            !value_flow_resolves("p()", "function dynwrite"),
            "arr.pop() must NOT surface an element written at an arbitrary index"
        );
        // `at(0)` is a specific-index read.
        assert!(
            value_flow_resolves("picked()", "function lit0"),
            "arr.at(0) should resolve to the index-0 element"
        );
        assert!(
            !value_flow_resolves("picked()", "function lit1"),
            "arr.at(0) must stay precise — no smear to the index-1 element"
        );
    }

    #[test]
    fn points_to_heap_resolves_mixin_merged_method() {
        // The express keystone: `mixin(app, proto)` (merge-descriptors) copies
        // proto's methods onto app, so `app.init()` resolves to the merged-in
        // function. Modeled as inheritance in the heap. Mirrors
        // `createApplication` → `app.init` in express/lib.
        let dir = tempdir().unwrap();
        let root = dir.path();
        // application.js: methods assigned on the chained-export alias object.
        std::fs::write(
            root.join("application.js"),
            "var app = exports = module.exports = {};\n\
             app.init = function init() { console.log(\"init\"); };\n",
        )
        .unwrap();
        // express.js: mixin the proto onto a fresh app function, then call init.
        std::fs::write(
            root.join("express.js"),
            "var mixin = require('merge-descriptors');\n\
             var proto = require('./application');\n\
             function createApplication() {\n\
               var app = function () {};\n\
               mixin(app, proto, false);\n\
               app.init();\n\
               return app;\n\
             }\n\
             createApplication();\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        assert!(
            heap_resolves(db, "app.init()", "function init"),
            "app.init() should resolve to the mixed-in init via inheritance"
        );
    }

    #[test]
    fn points_to_heap_resolves_object_assign_merge() {
        // `Object.assign(target, src)` merges src's methods onto target (modeled as
        // inheritance), so `target.m()` resolves. Guards against the regression where
        // evaluating the `Object` receiver lazily bound the name and disabled the
        // merge for the genuine global.
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("assign.js"),
            "function m() { console.log(\"m\"); }\n\
             const proto = {};\n\
             proto.m = m;\n\
             const t = {};\n\
             Object.assign(t, proto);\n\
             t.m();\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        assert!(
            heap_resolves(db, "t.m()", "function m"),
            "Object.assign(t, proto) should merge proto.m onto t so t.m() resolves"
        );
    }

    #[test]
    fn points_to_heap_resolves_object_coercion_identity() {
        // `Object(x)` returns `x` itself for an object argument, so a property
        // written through the coerced alias is visible on the original (and vice
        // versa). Resolves through the points-to heap.
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("oc.js"),
            "function g() { console.log(\"g\"); }\n\
             const o1 = {};\n\
             const o2 = Object(o1);\n\
             o2.g = g;\n\
             o1.g();\n\
             o2.g();\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        // The write through the `Object(o1)` alias is visible on both names.
        assert!(
            heap_resolves(db, "o2.g()", "function g"),
            "o2.g() should resolve to g (written directly on the alias)"
        );
        assert!(
            heap_resolves(db, "o1.g()", "function g"),
            "o1.g() should resolve to g (Object(o1) aliases o1, so o2.g = g lands on o1)"
        );
    }

    #[test]
    fn points_to_heap_object_coercion_respects_shadowing() {
        // A user binding that shadows the global `Object` must NOT be modeled as the
        // native coercion: the call resolves through the normal path to the user
        // function, whose body is walked (so the argument flows to its parameter).
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("shadow.js"),
            "function Object(cb) { cb(); }\n\
             function handler() { console.log(\"handler\"); }\n\
             Object(handler);\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        // The shadowed Object went through the real call path: its body was walked
        // and the argument bound to its parameter, so `cb()` resolves to handler.
        assert!(
            heap_resolves(db, "cb()", "function handler"),
            "a user function shadowing `Object` must be called normally (cb -> handler)"
        );
    }

    #[test]
    fn this_method_call_site_carries_member_callee() {
        // A `this.m()` call lowers to a callee with the `this.m` evidence, which is a
        // `Member` callee (its base `this` is lower-case, so it is NOT a by-name
        // `StaticMember`). This lets a resolved `this`-method edge line up with the
        // call site instead of falling back to the bare `"call"` evidence that no
        // resolved edge could match. Here the export-object method `outer` calls
        // `this.inner()`, which the value flow walks with `this` bound to `obj`.
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("m.js"),
            "var obj = {};\n\
             obj.outer = function outer() { this.inner(); };\n\
             obj.inner = function inner() { leaf(); };\n\
             function leaf() {}\n\
             obj.outer();\n",
        )
        .unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        assert!(
            any_resolves(db, "this.inner()", "function inner"),
            "this.inner() should resolve to obj.inner now that the site carries a Member callee"
        );
    }

    #[test]
    fn points_to_heap_resolves_constructor_return_override() {
        // A constructor that returns an object overrides the fresh instance (JS `new`
        // semantics). `new Bar()` yields the inner `baz` the constructor returns, so
        // `b()` resolves to `baz`.
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("ctor.js"),
            "function Bar() {\n\
            \x20 const x = function baz() { leaf(); };\n\
            \x20 return x;\n\
             }\n\
             function leaf() {}\n\
             const b = new Bar();\n\
             b();\n",
        )
        .unwrap();
        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        assert!(
            heap_resolves(db, "b()", "function baz"),
            "new Bar() should yield the returned baz so b() resolves to it"
        );
    }

    #[test]
    fn reachability_prunes_dead_code_but_keeps_invoked_callbacks() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // `deadFn` is loaded (required) but never invoked, so Jelly would not emit
        // edges from its body (even though it has a resolvable internal call). The
        // entry invokes `reached()` from top level and registers a promise handler
        // the value flow executes — a host-invoked callback with no incoming call
        // edge of its own.
        std::fs::write(
            root.join("dead.js"),
            "function deadFn() {\n  const local = () => {};\n  local();\n}\nmodule.exports = deadFn;\n",
        )
        .unwrap();
        std::fs::write(
            root.join("entry.js"),
            "require('./dead');\nfunction helper() {}\nfunction reached() { helper(); }\nreached();\nconst p = Promise.resolve(() => {});\np.then(function handler(v) { v(); });\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;

        // Find a function fact by a substring of its source text, picking the
        // tightest-spanning match (named function expressions passed as callbacks
        // get anonymous fact names, and the whole-file module function also
        // contains every needle).
        let find_fn = |needle: &str| {
            db.functions()
                .iter()
                .filter(|f| {
                    db.file(f.span.file).is_some_and(|file| {
                        file.source[f.span.start_byte as usize..f.span.end_byte as usize]
                            .contains(needle)
                    })
                })
                .min_by_key(|f| f.span.end_byte - f.span.start_byte)
                .unwrap_or_else(|| panic!("function containing `{needle}` not found"))
        };

        // Confirm the value flow only emits `FunctionTokenFlow` edges from bodies
        // it executes: the promise handler has such an edge, the never-invoked
        // `deadFn` does not (so it cannot be seeded as a reachability root).
        let token_flow_callers: BTreeSet<crate::core::FunctionId> = db
            .call_targets()
            .iter()
            .filter(|t| {
                t.algorithm == crate::analysis::calls::facts::CallAlgorithm::FunctionTokenFlow
            })
            .map(|t| t.caller)
            .collect();
        let dead_fn = find_fn("function deadFn");
        assert!(
            !token_flow_callers.contains(&dead_fn.id),
            "never-invoked deadFn must not be a value-flow (FunctionTokenFlow) caller"
        );
        assert!(
            token_flow_callers.contains(&find_fn("function handler(v)").id),
            "the executed promise handler must be a FunctionTokenFlow caller"
        );

        let reachable = jelly_reachable_caller_spans(db);
        let span_key =
            |f: &crate::core::FunctionFact| (f.span.file, f.span.start_byte, f.span.end_byte);
        let is_reachable = |needle: &str| reachable.contains(&span_key(find_fn(needle)));

        // Invoked from top level -> reachable.
        assert!(
            is_reachable("function reached"),
            "top-level-invoked `reached` must be reachable"
        );
        // Promise handler the value flow executed -> reachable (the whole point).
        assert!(
            is_reachable("function handler(v)"),
            "value-flow-invoked promise handler must be reachable"
        );
        // Module executions are always roots.
        assert!(
            reachable.iter().any(|&(file, start, _)| {
                start == 0
                    && db
                        .functions()
                        .iter()
                        .any(|f| f.span.file == file && f.span.start_byte == 0)
            }),
            "module-execution functions must be reachable roots"
        );
        // Loaded-but-never-invoked function -> NOT reachable (its FP gets pruned).
        assert!(
            !is_reachable("function deadFn"),
            "loaded-but-never-invoked deadFn must be pruned"
        );
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

        let edges = jelly_expected_edges(&graph, Path::new("")).unwrap();

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
        assert_eq!(
            match &enumeration.cases[0].expected[0] {
                ExpectedItem::GraphEdge(edge) => edge.from.as_str(),
                _ => unreachable!("Jelly cases only emit expected graph edges"),
            },
            "tests/micro/a.js:1:1:1:8"
        );
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
            r#"{"entries":["entry.js"],"files":["entry.js","helper.js","missing.js","package.json"],"functions":["0:1:1:1:20"],"calls":[],"fun2fun":[],"call2fun":[]}"#,
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

    #[test]
    fn jelly_normalized_graph_edges_are_set_semantics() {
        let mut items = Vec::new();
        let mut seen = BTreeSet::new();
        let edge = ObservedGraphEdge {
            graph: "jelly.call_graph.fun2fun".to_string(),
            from: "app.js:1:1:2:1".to_string(),
            to: "app.js:3:1:4:1".to_string(),
            mode: AssertionMode::Exact,
            partial_truth: false,
            producer_id: Some("first".to_string()),
            provenance: Some("direct".to_string()),
            precision: Some("heuristic".to_string()),
            status: None,
        };

        super::push_unique_jelly_graph_edge(&mut items, &mut seen, edge.clone());
        super::push_unique_jelly_graph_edge(&mut items, &mut seen, edge);

        assert_eq!(items.len(), 1);
    }

    #[test]
    fn ts_inventory_spans_fixture_covers_js01_forms_with_jelly_spans() {
        let fixture = repo_root().join("tests/eval-fixtures/jelly/ts-inventory-spans/repo");
        let forms = fixture.join("src/forms.ts");
        let source = std::fs::read_to_string(&forms).expect("read TS inventory span fixture");
        let mut db = AnalysisDb::new();
        let file_id = db.add_file(forms, "src/forms.ts".to_string(), source);
        let file = db.file(file_id).expect("fixture file");
        let inventory = extract_ts_inventory(file);
        let expectations = ts_inventory_span_expectations();
        let expected_spans = ts_inventory_span_oracle();
        assert_eq!(
            expected_spans.keys().cloned().collect::<BTreeSet<_>>(),
            expectations
                .iter()
                .map(|expected| expected.label.to_string())
                .collect::<BTreeSet<_>>(),
            "ts_inventory_spans: Jelly oracle labels must match named JS-01 expectations"
        );

        let mut matched_spans = BTreeMap::new();
        let mut unmatched = Vec::new();
        for expected in &expectations {
            if let Some(span) = expected.find_span(file, &inventory) {
                matched_spans.insert(
                    expected.label.to_string(),
                    render_inventory_span(file, expected.identity_kind(), span, expected.display),
                );
            } else {
                unmatched.push(format!(
                    "file=src/forms.ts span={} reason=missing named JS-01 inventory form `{}`",
                    expected.source_contains.unwrap_or(expected.display),
                    expected.label
                ));
            }
        }

        let mismatches = expected_spans
            .iter()
            .filter_map(|(label, expected_span)| match matched_spans.get(label) {
                Some(observed_span) if observed_span == expected_span => None,
                Some(observed_span) => Some(format!(
                    "file=src/forms.ts span={observed_span} reason=span mismatch for `{label}` expected={expected_span}"
                )),
                None => Some(format!(
                    "file=src/forms.ts span={expected_span} reason=missing oracle label `{label}`"
                )),
            })
            .collect::<Vec<_>>();
        let exact_matches = expected_spans.len() - mismatches.len();
        let ratio = exact_matches as f64 / expected_spans.len() as f64;
        assert!(
            ratio >= 0.99,
            "ts_inventory_spans Jelly oracle-span coverage ratio {ratio:.4} (exact {}/{}) below 0.99; unmatched: {unmatched:?}; mismatches: {mismatches:?}",
            exact_matches,
            expected_spans.len()
        );
        assert_eq!(matched_spans, expected_spans);
        assert!(
            matched_spans
                .values()
                .any(|span| spans_multiple_lines(span.as_str())),
            "ts_inventory_spans: expected at least one multi-line Jelly span, got {matched_spans:?}"
        );
    }

    #[derive(Deserialize)]
    struct JellySpanOracle {
        spans: BTreeMap<String, String>,
    }

    fn ts_inventory_span_oracle() -> BTreeMap<String, String> {
        let oracle_path =
            repo_root().join("tests/eval-fixtures/jelly/ts-inventory-spans/jelly-spans.toml");
        let oracle = std::fs::read_to_string(&oracle_path).expect("read Jelly span oracle fixture");
        toml::from_str::<JellySpanOracle>(&oracle)
            .expect("parse Jelly span oracle fixture")
            .spans
    }

    #[derive(Clone, Copy)]
    enum InventoryExpectationKind {
        Function(TsFunctionInventoryKind),
        Callsite(TsCallsiteInventoryKind),
    }

    struct InventoryExpectation {
        label: &'static str,
        kind: InventoryExpectationKind,
        display: &'static str,
        source_contains: Option<&'static str>,
        status_reason: Option<&'static str>,
    }

    impl InventoryExpectation {
        fn function(
            label: &'static str,
            kind: TsFunctionInventoryKind,
            display: &'static str,
            source_contains: Option<&'static str>,
        ) -> Self {
            Self {
                label,
                kind: InventoryExpectationKind::Function(kind),
                display,
                source_contains,
                status_reason: None,
            }
        }

        fn callsite(
            label: &'static str,
            kind: TsCallsiteInventoryKind,
            display: &'static str,
            source_contains: Option<&'static str>,
            status_reason: Option<&'static str>,
        ) -> Self {
            Self {
                label,
                kind: InventoryExpectationKind::Callsite(kind),
                display,
                source_contains,
                status_reason,
            }
        }

        fn identity_kind(&self) -> IdentityKind {
            match self.kind {
                InventoryExpectationKind::Function(_) => IdentityKind::Function,
                InventoryExpectationKind::Callsite(_) => IdentityKind::Callsite,
            }
        }

        fn find_span(
            &self,
            file: &SourceFile,
            inventory: &crate::ts::inventory::store::TsInventoryOutput,
        ) -> Option<Span> {
            match self.kind {
                InventoryExpectationKind::Function(kind) => inventory
                    .functions
                    .iter()
                    .find(|function| {
                        function.kind == kind
                            && function.display_name.as_deref() == Some(self.display)
                            && self.source_contains_matches(file, &function.span)
                    })
                    .map(|function| function.span.clone()),
                InventoryExpectationKind::Callsite(kind) => inventory
                    .callsites
                    .iter()
                    .find(|callsite| {
                        callsite.kind == kind
                            && callsite.display_name.as_deref() == Some(self.display)
                            && self.source_contains_matches(file, &callsite.span)
                            && self.status_reason_matches(&callsite.status)
                    })
                    .map(|callsite| callsite.span.clone()),
            }
        }

        fn source_contains_matches(&self, file: &SourceFile, span: &Span) -> bool {
            let Some(needle) = self.source_contains else {
                return true;
            };
            source_slice(file, span).contains(needle)
        }

        fn status_reason_matches(&self, status: &TsInventoryStatus) -> bool {
            match self.status_reason {
                Some(expected) => {
                    matches!(status, TsInventoryStatus::Unresolved { reason } if reason == expected)
                }
                None => matches!(status, TsInventoryStatus::Resolved),
            }
        }
    }

    fn ts_inventory_span_expectations() -> Vec<InventoryExpectation> {
        vec![
            InventoryExpectation::function(
                "function-declaration",
                TsFunctionInventoryKind::Declaration,
                "declaredMulti",
                Some("function declaredMulti"),
            ),
            InventoryExpectation::function(
                "function-expression",
                TsFunctionInventoryKind::FunctionExpression,
                "namedExpr",
                Some("function namedExpr"),
            ),
            InventoryExpectation::function(
                "arrow",
                TsFunctionInventoryKind::Arrow,
                "arrow",
                Some("=> declaredMulti(value)"),
            ),
            InventoryExpectation::function(
                "nested-arrow",
                TsFunctionInventoryKind::Arrow,
                "nestedArrow",
                Some("=> declaredMulti(\"nested\")"),
            ),
            InventoryExpectation::function(
                "method",
                TsFunctionInventoryKind::Method,
                "method",
                Some("method(next"),
            ),
            InventoryExpectation::function(
                "constructor",
                TsFunctionInventoryKind::Constructor,
                "constructor",
                Some("constructor(value"),
            ),
            InventoryExpectation::function(
                "accessor-get",
                TsFunctionInventoryKind::Accessor,
                "current",
                Some("get current"),
            ),
            InventoryExpectation::function(
                "accessor-set",
                TsFunctionInventoryKind::Accessor,
                "current",
                Some("set current"),
            ),
            InventoryExpectation::function(
                "class-static-block",
                TsFunctionInventoryKind::ClassStaticBlock,
                "static",
                Some("static {"),
            ),
            InventoryExpectation::callsite(
                "call",
                TsCallsiteInventoryKind::Call,
                "declaredMulti",
                None,
                None,
            ),
            InventoryExpectation::callsite(
                "new",
                TsCallsiteInventoryKind::New,
                "Box",
                Some("new Box"),
                None,
            ),
            InventoryExpectation::callsite(
                "tagged-template",
                TsCallsiteInventoryKind::TaggedTemplate,
                "tag",
                Some("tag`hello"),
                None,
            ),
            InventoryExpectation::callsite(
                "optional-call",
                TsCallsiteInventoryKind::OptionalCall,
                "maybe",
                Some("maybe?."),
                None,
            ),
            InventoryExpectation::callsite(
                "dynamic-import-static",
                TsCallsiteInventoryKind::DynamicImport,
                "import:./static",
                Some("import(\"./static\")"),
                None,
            ),
            InventoryExpectation::callsite(
                "dynamic-import-unresolved",
                TsCallsiteInventoryKind::DynamicImport,
                "import:<dynamic>",
                Some("import(dynamicSpecifier)"),
                Some("non-string dynamic import"),
            ),
            InventoryExpectation::callsite(
                "require",
                TsCallsiteInventoryKind::Require,
                "require",
                Some("require(\"pkg\")"),
                None,
            ),
        ]
    }

    fn render_inventory_span(
        file: &SourceFile,
        kind: IdentityKind,
        span: Span,
        display_name: &str,
    ) -> String {
        let language = LanguageTag::TypeScript;
        let record = IdentityRecord {
            id: IdentityRecordId(0),
            kind,
            file_id: file.id,
            span: span.clone(),
            language,
            package_or_module: Arc::from(file.relative_path.as_str()),
            container_path: Arc::from(file.relative_path.as_str()),
            display_name: Arc::from(display_name),
            signature_digest: compute_signature_digest(
                language,
                &file.relative_path,
                &file.relative_path,
                display_name,
                None,
                None,
            ),
            multiplicity: 1,
            stable_key: compute_identity_stable_key(
                kind,
                language,
                &file.relative_path,
                &file.relative_path,
                file.id,
                &span,
            ),
            originating_call_site_id: None,
            originating_call_target_id: None,
        };
        jelly_span::render(&record, file)
    }

    fn source_slice<'a>(file: &'a SourceFile, span: &Span) -> &'a str {
        &file.source[span.start_byte as usize..span.end_byte as usize]
    }

    fn spans_multiple_lines(span: &str) -> bool {
        let parts = span.rsplit(':').take(4).collect::<Vec<_>>();
        parts.len() == 4 && parts[3] != parts[1]
    }

    fn manifest(path: &str) -> SuiteManifest {
        SuiteManifest {
            schema_version: "polint-eval-suite-1".to_string(),
            id: SuiteId("jelly-callgraph-micro".to_string()),
            name: "Jelly JS/TS callgraph micro".to_string(),
            kind: SuiteKind::CallGraphPrecision,
            languages: vec!["javascript".to_string(), "typescript".to_string()],
            adapter_id: "jelly_callgraph_micro".to_string(),
            scoring_mode: crate::eval::suite::ScoringMode::OracleJelly,
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
