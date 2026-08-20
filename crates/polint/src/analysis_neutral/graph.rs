use crate::analysis_neutral::AnalysisHost;
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
pub struct ImportGraph {
    graph: DiGraph<String, ()>,
}

impl ImportGraph {
    pub fn from_db(db: &(impl AnalysisHost + ?Sized)) -> Self {
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

    pub fn to_dot(&self) -> String {
        format!(
            "{:?}",
            Dot::with_config(&self.graph, &[Config::EdgeNoLabel])
        )
    }
}

#[derive(Debug, Clone, Default)]
pub struct FunctionGraph {
    graph: DiGraph<String, ()>,
}

impl FunctionGraph {
    pub fn from_db(db: &(impl AnalysisHost + ?Sized), function_name: &str) -> Self {
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

    pub fn to_dot(&self) -> String {
        format!(
            "{:?}",
            Dot::with_config(&self.graph, &[Config::EdgeNoLabel])
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_api::{FactDatabase, FunctionFact, ImportFact};
    use crate::analysis_neutral::LocalAnalysisDb;
    use crate::internal_core::{FileId, FunctionId, ImportId, Language, Span};
    use std::path::PathBuf;

    fn add_file(db: &mut LocalAnalysisDb, relative_path: &str) -> FileId {
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
        let mut db = LocalAnalysisDb::new();
        let main = add_file(&mut db, "cmd/main.go");
        let worker = add_file(&mut db, "internal/worker.go");
        db.push_import(ImportFact::new(
            ImportId::from_raw(99),
            main,
            None,
            "fmt".to_string(),
            span(main),
            Language::Go,
        ));
        db.push_import(ImportFact::new(
            ImportId::from_raw(99),
            worker,
            None,
            "context".to_string(),
            span(worker),
            Language::Go,
        ));

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
        let mut db = LocalAnalysisDb::new();
        let file = add_file(&mut db, "cmd/main.go");
        db.push_function(FunctionFact::new(
            FunctionId::from_raw(99),
            file,
            "Authorize".to_string(),
            span(file),
            Language::Go,
            false,
            true,
            1,
            vec!["charge".to_string(), "validateUser".to_string()],
        ));

        let dot = FunctionGraph::from_db(&db, "Authorize").to_dot();

        assert!(dot.contains("digraph"));
        assert!(dot.contains("Authorize"));
        assert!(dot.contains("charge"));
        assert!(dot.contains("validateUser"));
    }

    #[test]
    fn function_graph_dot_empty_match_is_valid_dot() {
        let db = LocalAnalysisDb::new();
        let dot = FunctionGraph::from_db(&db, "Missing").to_dot();

        assert!(dot.contains("digraph"));
        assert!(!dot.contains("Missing"));
    }
}
