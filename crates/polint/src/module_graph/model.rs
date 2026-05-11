use crate::core::{
    AnalysisDb, FileId, ImportId, Language, ModuleEdge, ModuleEdgeId, ModuleEdgeKind, ModuleNode,
    ModuleNodeId, ModuleNodeKind, PackageFact, ResolutionStatus,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default)]
pub(crate) struct ModuleGraphOutput {
    pub(crate) nodes: Vec<ModuleNode>,
    pub(crate) edges: Vec<ModuleEdge>,
}

#[derive(Debug, Clone)]
pub(crate) struct ModuleGraphBuilder {
    files: BTreeMap<FileId, FileNodeInfo>,
    nodes: Vec<ModuleNode>,
    file_nodes: BTreeMap<FileId, ModuleNodeId>,
    package_nodes: BTreeMap<String, ModuleNodeId>,
    module_nodes: BTreeMap<String, ModuleNodeId>,
    external_nodes: BTreeMap<(String, Option<Language>), ModuleNodeId>,
    edge_keys: BTreeSet<EdgeKey>,
    edges: Vec<ModuleEdge>,
}

#[derive(Debug, Clone)]
struct FileNodeInfo {
    relative_path: String,
    language: Language,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct EdgeKey {
    from: ModuleNodeId,
    to: ModuleNodeId,
    import: Option<ImportId>,
    resolved_import: Option<crate::core::ResolvedImportId>,
    kind: u8,
    status: u8,
}

impl ModuleGraphBuilder {
    pub(crate) fn new(db: &AnalysisDb) -> Self {
        let files = db
            .files()
            .iter()
            .map(|file| {
                (
                    file.id,
                    FileNodeInfo {
                        relative_path: file.relative_path.clone(),
                        language: file.language,
                    },
                )
            })
            .collect();

        Self {
            files,
            nodes: Vec::new(),
            file_nodes: BTreeMap::new(),
            package_nodes: BTreeMap::new(),
            module_nodes: BTreeMap::new(),
            external_nodes: BTreeMap::new(),
            edge_keys: BTreeSet::new(),
            edges: Vec::new(),
        }
    }

    pub(crate) fn ensure_file_node(&mut self, file: FileId) -> ModuleNodeId {
        if let Some(node) = self.file_nodes.get(&file).copied() {
            return node;
        }

        let Some(info) = self.files.get(&file) else {
            return self.insert_node(ModuleNode {
                id: ModuleNodeId(0),
                kind: ModuleNodeKind::File,
                label: format!("<unknown:{}>", file.0),
                file: Some(file),
                package: None,
                language: None,
            });
        };

        let node = self.insert_node(ModuleNode {
            id: ModuleNodeId(0),
            kind: ModuleNodeKind::File,
            label: info.relative_path.clone(),
            file: Some(file),
            package: None,
            language: Some(info.language),
        });
        self.file_nodes.insert(file, node);
        node
    }

    pub(crate) fn ensure_package_node(&mut self, package: &PackageFact) -> ModuleNodeId {
        let label = package_label(package.language, &package.name);
        if let Some(node) = self.package_nodes.get(&label).copied() {
            return node;
        }

        let node = self.insert_node(ModuleNode {
            id: ModuleNodeId(0),
            kind: ModuleNodeKind::Package,
            label: label.clone(),
            file: None,
            package: Some(package.id),
            language: Some(package.language),
        });
        self.package_nodes.insert(label, node);
        node
    }

    pub(crate) fn ensure_module_node(&mut self, label: impl Into<String>) -> ModuleNodeId {
        let label = label.into();
        if let Some(node) = self.module_nodes.get(&label).copied() {
            return node;
        }

        let node = self.insert_node(ModuleNode {
            id: ModuleNodeId(0),
            kind: ModuleNodeKind::Module,
            label: label.clone(),
            file: None,
            package: None,
            language: None,
        });
        self.module_nodes.insert(label, node);
        node
    }

    pub(crate) fn ensure_external_node(
        &mut self,
        label: impl Into<String>,
        language: Option<Language>,
    ) -> ModuleNodeId {
        let label = label.into();
        let key = (label.clone(), language);
        if let Some(node) = self.external_nodes.get(&key).copied() {
            return node;
        }

        let node = self.insert_node(ModuleNode {
            id: ModuleNodeId(0),
            kind: ModuleNodeKind::External,
            label,
            file: None,
            package: None,
            language,
        });
        self.external_nodes.insert(key, node);
        node
    }

    pub(crate) fn link_module_contains(&mut self, module: ModuleNodeId, child: ModuleNodeId) {
        self.link(
            module,
            child,
            None,
            None,
            ModuleEdgeKind::Contains,
            ResolutionStatus::Resolved,
        );
    }

    pub(crate) fn link_contains(&mut self, parent: ModuleNodeId, child: ModuleNodeId) {
        self.link(
            parent,
            child,
            None,
            None,
            ModuleEdgeKind::Contains,
            ResolutionStatus::Resolved,
        );
    }

    pub(crate) fn link_dependency(
        &mut self,
        from: ModuleNodeId,
        to: ModuleNodeId,
        import: Option<ImportId>,
        kind: ModuleEdgeKind,
    ) {
        let status = match kind {
            ModuleEdgeKind::DependsOn => ResolutionStatus::External,
            ModuleEdgeKind::Imports | ModuleEdgeKind::Contains => ResolutionStatus::Resolved,
        };
        self.link(from, to, import, None, kind, status);
    }

    pub(crate) fn link_resolved_import(
        &mut self,
        from: ModuleNodeId,
        to: ModuleNodeId,
        import: ImportId,
        resolved_import: crate::core::ResolvedImportId,
        kind: ModuleEdgeKind,
        status: ResolutionStatus,
    ) {
        self.link(from, to, Some(import), Some(resolved_import), kind, status);
    }

    pub(crate) fn finish(mut self) -> ModuleGraphOutput {
        self.nodes.sort_by(|left, right| {
            (left.id, left.label.as_str()).cmp(&(right.id, right.label.as_str()))
        });
        self.edges.sort_by(|left, right| {
            (
                left.from,
                left.to,
                edge_kind_rank(left.kind),
                left.import,
                left.resolved_import,
                status_rank(left.status),
            )
                .cmp(&(
                    right.from,
                    right.to,
                    edge_kind_rank(right.kind),
                    right.import,
                    right.resolved_import,
                    status_rank(right.status),
                ))
        });

        ModuleGraphOutput {
            nodes: self.nodes,
            edges: self.edges,
        }
    }

    fn insert_node(&mut self, mut node: ModuleNode) -> ModuleNodeId {
        let id = ModuleNodeId(self.nodes.len() as u64);
        node.id = id;
        self.nodes.push(node);
        id
    }

    fn link(
        &mut self,
        from: ModuleNodeId,
        to: ModuleNodeId,
        import: Option<ImportId>,
        resolved_import: Option<crate::core::ResolvedImportId>,
        kind: ModuleEdgeKind,
        status: ResolutionStatus,
    ) {
        let key = EdgeKey {
            from,
            to,
            import,
            resolved_import,
            kind: edge_kind_rank(kind),
            status: status_rank(status),
        };
        if !self.edge_keys.insert(key) {
            return;
        }

        self.edges.push(ModuleEdge {
            id: ModuleEdgeId(0),
            from,
            to,
            import,
            resolved_import,
            kind,
            status,
        });
    }
}

pub(crate) fn package_label(language: Language, name: &str) -> String {
    format!("{}:{name}", language_label(language))
}

fn language_label(language: Language) -> &'static str {
    match language {
        Language::Go => "go",
        Language::TypeScript => "ts",
        Language::Tsx => "tsx",
        Language::JavaScript => "js",
        Language::Jsx => "jsx",
        Language::Unknown => "unknown",
    }
}

fn edge_kind_rank(kind: ModuleEdgeKind) -> u8 {
    match kind {
        ModuleEdgeKind::Contains => 0,
        ModuleEdgeKind::Imports => 1,
        ModuleEdgeKind::DependsOn => 2,
    }
}

fn status_rank(status: ResolutionStatus) -> u8 {
    match status {
        ResolutionStatus::Resolved => 0,
        ResolutionStatus::External => 1,
        ResolutionStatus::Unresolved => 2,
        ResolutionStatus::SetupMissing => 3,
        ResolutionStatus::Dynamic => 4,
        ResolutionStatus::Unsupported => 5,
    }
}

pub(crate) fn sort_packages<'a>(
    packages: &'a [PackageFact],
    db: &AnalysisDb,
) -> Vec<&'a PackageFact> {
    let mut packages = packages.iter().collect::<Vec<_>>();
    packages.sort_by(|left, right| {
        (
            db.path_for(left.file),
            left.language,
            left.name.as_str(),
            left.id,
        )
            .cmp(&(
                db.path_for(right.file),
                right.language,
                right.name.as_str(),
                right.id,
            ))
    });
    packages
}

#[cfg(test)]
mod tests {
    use super::{ModuleGraphBuilder, ModuleNodeDraft, ResolvedImportDraft};
    use crate::core::{
        AnalysisDb, ImportFact, ImportId, Language, ModuleEdgeKind, ModuleNodeKind,
        ResolutionPrecision, ResolutionStatus, Span,
    };
    use std::path::PathBuf;

    #[test]
    fn module_graph_resolver_contracts_external_draft_links_dependency_edge() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import React from 'react';\n".to_string(),
        );
        db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "react".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
        });
        let import = &db.imports()[0];
        let mut builder = ModuleGraphBuilder::new(&db);
        let owner = builder.ensure_module_node(".");
        let draft = ResolvedImportDraft {
            target: Some(ModuleNodeDraft::external(
                "react",
                Some(Language::TypeScript),
            )),
            status: ResolutionStatus::External,
            precision: ResolutionPrecision::ExternalPackage,
            reason: None,
            edge_kind: Some(ModuleEdgeKind::DependsOn),
        };

        let fact = builder.apply_resolved_import_draft(import, owner, draft);
        let output = builder.finish();

        let external = output
            .nodes
            .iter()
            .find(|node| node.kind == ModuleNodeKind::External && node.label == "react")
            .map(|node| node.id)
            .expect("external node exists");
        assert_eq!(fact.target_node, Some(external));
        assert!(output.edges.iter().any(|edge| {
            edge.from == owner && edge.to == external && edge.kind == ModuleEdgeKind::DependsOn
        }));
    }
}
