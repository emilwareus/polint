use crate::core::{AnalysisDb, FileId, FunctionId};
use petgraph::dot::{Config, Dot};
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::BTreeMap;

/// Look up `key` in `nodes`, otherwise allocate a single owned `String` for the
/// petgraph node weight. Avoids the redundant double-clone pattern that
/// `entry(...).or_insert_with(...)` produces when the key and the new node
/// label are the same string.
fn ensure_node<'k>(
    nodes: &mut BTreeMap<&'k str, NodeIndex>,
    graph: &mut DiGraph<String, ()>,
    key: &'k str,
) -> NodeIndex {
    if let Some(&idx) = nodes.get(key) {
        return idx;
    }
    let idx = graph.add_node(key.to_string());
    nodes.insert(key, idx);
    idx
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ImportGraph {
    graph: DiGraph<String, ()>,
}

impl ImportGraph {
    pub(crate) fn from_db(db: &AnalysisDb) -> Self {
        let mut graph = DiGraph::<String, ()>::new();
        let mut nodes: BTreeMap<&str, NodeIndex> = BTreeMap::new();

        for file in db.files() {
            ensure_node(&mut nodes, &mut graph, file.relative_path.as_str());
        }

        for import in db.imports() {
            let from_path = db
                .file(import.file)
                .map_or("<unknown>", |file| file.relative_path.as_str());
            let from_idx = ensure_node(&mut nodes, &mut graph, from_path);
            let to_idx = ensure_node(&mut nodes, &mut graph, import.path.as_str());
            graph.add_edge(from_idx, to_idx, ());
        }

        Self { graph }
    }

    pub(crate) fn to_dot(&self) -> String {
        format!(
            "{:?}",
            Dot::with_config(&self.graph, &[Config::EdgeNoLabel])
        )
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct FunctionGraph {
    graph: DiGraph<String, ()>,
}

impl FunctionGraph {
    pub(crate) fn from_db(db: &AnalysisDb, function_name: &str) -> Self {
        let mut graph = DiGraph::<String, ()>::new();
        let mut nodes = BTreeMap::<&str, NodeIndex>::new();

        for function in db
            .functions()
            .iter()
            .filter(|function| function.name == function_name)
        {
            let root = ensure_node(&mut nodes, &mut graph, function.name.as_str());
            for call in &function.calls {
                let call_idx = ensure_node(&mut nodes, &mut graph, call.as_str());
                graph.add_edge(root, call_idx, ());
            }
        }

        Self { graph }
    }

    pub(crate) fn to_dot(&self) -> String {
        format!(
            "{:?}",
            Dot::with_config(&self.graph, &[Config::EdgeNoLabel])
        )
    }
}

// Placeholders for internal graph experiments; do not expose before the SDK view
// and user value are finished.
#[expect(
    dead_code,
    reason = "Reserved for internal CFG experiments; not a public surface."
)]
pub(crate) fn cfg_to_dot(_db: &AnalysisDb, _function: FunctionId) -> String {
    "digraph cfg {\n  \"entry\" -> \"exit\";\n}\n".to_string()
}

#[expect(
    dead_code,
    reason = "Reserved for future graph export; ImportGraph uses paths directly today."
)]
pub(crate) fn file_node_label(db: &AnalysisDb, file: FileId) -> String {
    db.path_for(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{FunctionFact, FunctionId, ImportFact, ImportId, Language, Span};
    use std::path::PathBuf;

    fn add_file(db: &mut AnalysisDb, relative_path: &str) -> FileId {
        db.add_file(
            PathBuf::from(relative_path),
            relative_path.to_string(),
            "package main\n".to_string(),
        )
    }

    fn span(file: FileId) -> Span {
        Span::point(file, 1, 1)
    }

    #[test]
    fn import_graph_dot_is_deterministic() {
        let mut db = AnalysisDb::new();
        let main = add_file(&mut db, "cmd/main.go");
        let worker = add_file(&mut db, "internal/worker.go");
        db.push_import(ImportFact {
            id: ImportId(99),
            file: main,
            package: None,
            path: "fmt".to_string(),
            span: span(main),
            language: Language::Go,
        });
        db.push_import(ImportFact {
            id: ImportId(99),
            file: worker,
            package: None,
            path: "context".to_string(),
            span: span(worker),
            language: Language::Go,
        });

        let dot = ImportGraph::from_db(&db).to_dot();
        let repeated = ImportGraph::from_db(&db).to_dot();

        assert_eq!(dot, repeated);
        assert!(dot.contains("digraph"));
        assert!(dot.contains("cmd/main.go"));
        assert!(dot.contains("internal/worker.go"));
        assert!(dot.contains("fmt"));
        assert!(dot.contains("context"));
    }

    #[test]
    fn function_graph_dot_includes_available_calls() {
        let mut db = AnalysisDb::new();
        let file = add_file(&mut db, "cmd/main.go");
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "Authorize".to_string(),
            span: span(file),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: vec!["charge".to_string(), "validateUser".to_string()],
        });

        let dot = FunctionGraph::from_db(&db, "Authorize").to_dot();

        assert!(dot.contains("digraph"));
        assert!(dot.contains("Authorize"));
        assert!(dot.contains("charge"));
        assert!(dot.contains("validateUser"));
    }

    #[test]
    fn function_graph_dot_empty_match_is_valid_dot() {
        let db = AnalysisDb::new();
        let dot = FunctionGraph::from_db(&db, "Missing").to_dot();

        assert!(dot.contains("digraph"));
        assert!(!dot.contains("Missing"));
    }
}
