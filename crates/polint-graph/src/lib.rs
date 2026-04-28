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
