use petgraph::dot::{Config, Dot};
use petgraph::graph::{DiGraph, NodeIndex};
use polint_core::{AnalysisDb, FileId, FunctionId};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct ImportGraph {
    graph: DiGraph<String, String>,
}

impl ImportGraph {
    pub fn from_db(db: &AnalysisDb) -> Self {
        let mut graph = DiGraph::<String, String>::new();
        let mut nodes: BTreeMap<String, NodeIndex> = BTreeMap::new();

        for file in db.files() {
            nodes
                .entry(file.relative_path.clone())
                .or_insert_with(|| graph.add_node(file.relative_path.clone()));
        }

        for import in db.imports() {
            let from = db.path_for(import.file);
            let from_idx = *nodes
                .entry(from.clone())
                .or_insert_with(|| graph.add_node(from.clone()));
            let to_idx = *nodes
                .entry(import.path.clone())
                .or_insert_with(|| graph.add_node(import.path.clone()));
            graph.add_edge(from_idx, to_idx, import.path.clone());
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
    graph: DiGraph<String, String>,
}

impl FunctionGraph {
    pub fn from_db(db: &AnalysisDb, function_name: &str) -> Self {
        let mut graph = DiGraph::<String, String>::new();
        let mut nodes = BTreeMap::<String, NodeIndex>::new();

        for function in db
            .functions()
            .iter()
            .filter(|function| function.name == function_name)
        {
            let root = *nodes
                .entry(function.name.clone())
                .or_insert_with(|| graph.add_node(function.name.clone()));
            for call in &function.calls {
                let call_idx = *nodes
                    .entry(call.clone())
                    .or_insert_with(|| graph.add_node(call.clone()));
                graph.add_edge(root, call_idx, "calls".to_string());
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

pub fn cfg_to_dot(_db: &AnalysisDb, _function: FunctionId) -> String {
    "digraph cfg {\n  \"entry\" -> \"exit\";\n}\n".to_string()
}

pub fn file_node_label(db: &AnalysisDb, file: FileId) -> String {
    db.path_for(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use polint_core::{FunctionFact, FunctionId, ImportFact, ImportId, Language, Span};
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
