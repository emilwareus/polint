use std::collections::BTreeMap;

use crate::analysis::calls::facts::{
    CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
    CallSyntaxKind, CallTargetFact, CallTargetStatus,
};
use crate::analysis::ids::CallTargetId;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{
    AnalysisDb, FileId, FunctionFact, FunctionId, ReferenceFact, ReferenceId, Span, SymbolFact,
    SymbolId, SymbolPrecision, SymbolResolutionStatus,
};
use crate::module_graph::topology::ImportToPackageStatus;
use crate::symbol_graph::semantic::SemanticStatus;

pub(crate) fn resolve_direct_call_targets(
    db: &AnalysisDb,
    sites: &[CallSiteFact],
) -> Vec<CallTargetFact> {
    let index = DirectIndex::new(db);
    let mut rows = Vec::new();

    for site in sites {
        let Some(reference) = index.reference_for_site(site) else {
            continue;
        };
        if !is_precise_resolved_reference(reference) {
            continue;
        }
        let Some(target_symbol) = reference.target else {
            continue;
        };
        let Some(symbol) = index.symbols_by_id.get(&target_symbol).copied() else {
            continue;
        };

        let algorithm = if index.is_import_binding(site, reference) {
            CallAlgorithm::ImportBinding
        } else if matches!(
            site.kind,
            CallSyntaxKind::StaticMember | CallSyntaxKind::Member
        ) {
            CallAlgorithm::StaticMember
        } else {
            CallAlgorithm::DirectReference
        };
        rows.push(CallTargetFact {
            id: CallTargetId(0),
            site: site.id,
            caller: site.caller,
            target_function: index.function_for_symbol(symbol),
            target_symbol: Some(target_symbol),
            edge_kind: edge_kind_for_site(site.kind),
            algorithm,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::NativeDirect,
            precision: CallPrecision::SetupAware,
            stable_key: target_stable_key(site, algorithm, symbol),
        });
    }

    rows.sort_by(|left, right| {
        (left.stable_key.as_str(), left.site).cmp(&(right.stable_key.as_str(), right.site))
    });
    rows.dedup_by(|left, right| left.stable_key == right.stable_key);
    for (index, row) in rows.iter_mut().enumerate() {
        row.id = CallTargetId(index as u64);
    }
    rows
}

struct DirectIndex<'db> {
    db: &'db AnalysisDb,
    references_by_id: BTreeMap<ReferenceId, &'db ReferenceFact>,
    symbols_by_id: BTreeMap<SymbolId, &'db SymbolFact>,
    functions_by_file_name: BTreeMap<(Option<FileId>, String), &'db FunctionFact>,
}

impl<'db> DirectIndex<'db> {
    fn new(db: &'db AnalysisDb) -> Self {
        Self {
            db,
            references_by_id: db
                .references()
                .iter()
                .map(|reference| (reference.id, reference))
                .collect(),
            symbols_by_id: db
                .symbols()
                .iter()
                .map(|symbol| (symbol.id, symbol))
                .collect(),
            functions_by_file_name: db
                .functions()
                .iter()
                .map(|function| ((Some(function.file), function.name.clone()), function))
                .collect(),
        }
    }

    fn reference_for_site(&self, site: &CallSiteFact) -> Option<&'db ReferenceFact> {
        match &site.callee {
            CallCallee::Identifier { reference, name }
            | CallCallee::Constructor {
                reference,
                name: Some(name),
            } => reference
                .and_then(|id| self.references_by_id.get(&id).copied())
                .or_else(|| self.unique_reference_by_site_name(site, name)),
            CallCallee::Member { property, .. }
                if matches!(
                    site.kind,
                    CallSyntaxKind::StaticMember | CallSyntaxKind::Member
                ) =>
            {
                self.unique_reference_by_site_name(site, property)
            }
            _ => None,
        }
    }

    fn unique_reference_by_site_name(
        &self,
        site: &CallSiteFact,
        name: &str,
    ) -> Option<&'db ReferenceFact> {
        let mut matches = self
            .db
            .references_for_file(site.file)
            .filter(|reference| reference.name == name)
            .filter(|reference| {
                reference
                    .primary_span
                    .as_ref()
                    .is_some_and(|span| spans_overlap_or_touch(span, &site.span))
            });
        let first = matches.next()?;
        if matches.next().is_none() {
            Some(first)
        } else {
            None
        }
    }

    fn is_import_binding(&self, site: &CallSiteFact, reference: &ReferenceFact) -> bool {
        matches!(reference.precision, SymbolPrecision::ModuleLinked)
            || self.semantic_import_matches(site, reference)
            || self.resolved_import_matches(site)
            || self.import_to_package_matches(site)
    }

    fn semantic_import_matches(&self, site: &CallSiteFact, reference: &ReferenceFact) -> bool {
        self.db.semantic_imports().iter().any(|import| {
            import.file == Some(site.file)
                && import.status == SemanticStatus::Resolved
                && (import.local_name.as_deref() == Some(reference.name.as_str())
                    || import.imported_name.as_deref() == Some(reference.name.as_str()))
        })
    }

    fn resolved_import_matches(&self, site: &CallSiteFact) -> bool {
        self.db
            .resolved_imports()
            .iter()
            .any(|import| import.from_file == site.file && import.target_node.is_some())
    }

    fn import_to_package_matches(&self, site: &CallSiteFact) -> bool {
        self.db.import_to_package_edges().iter().any(|edge| {
            edge.from_file == Some(site.file)
                && edge.target_node.is_some()
                && edge.status == ImportToPackageStatus::Resolved
        })
    }

    fn function_for_symbol(&self, symbol: &SymbolFact) -> Option<FunctionId> {
        self.functions_by_file_name
            .get(&(symbol.file, symbol.qualified_name.clone()))
            .or_else(|| {
                self.functions_by_file_name
                    .get(&(symbol.file, symbol.name.clone()))
            })
            .map(|function| function.id)
    }
}

fn is_precise_resolved_reference(reference: &ReferenceFact) -> bool {
    reference.status == SymbolResolutionStatus::Resolved
        && reference.target.is_some()
        && reference.target.is_none_or(|target| {
            reference
                .candidates
                .iter()
                .all(|candidate| *candidate == target)
        })
        && matches!(
            reference.precision,
            SymbolPrecision::ExactSemantic
                | SymbolPrecision::ExactLocal
                | SymbolPrecision::ModuleLinked
        )
}

fn edge_kind_for_site(kind: CallSyntaxKind) -> CallEdgeKind {
    match kind {
        CallSyntaxKind::Constructor | CallSyntaxKind::New => CallEdgeKind::Constructor,
        CallSyntaxKind::StaticMember => CallEdgeKind::StaticMember,
        CallSyntaxKind::Method | CallSyntaxKind::Member => CallEdgeKind::MethodDirect,
        _ => CallEdgeKind::Direct,
    }
}

fn spans_overlap_or_touch(left: &Span, right: &Span) -> bool {
    left.file == right.file
        && left.start_byte <= right.end_byte
        && right.start_byte <= left.end_byte
}

fn target_stable_key(site: &CallSiteFact, algorithm: CallAlgorithm, symbol: &SymbolFact) -> String {
    semantic_stable_key(
        FactFamily::CallTarget,
        &[
            ("site", site.stable_key.clone()),
            ("algorithm", format!("{algorithm:?}")),
            ("target", symbol.stable_key.clone()),
            ("provider", crate::core::CALLS_PROVIDER_ID.to_string()),
            ("schema", "calls-facts-1:1".to_string()),
            ("model", "absent".to_string()),
        ],
    )
    .into_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::resolve_direct_call_targets;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallSiteFact, CallSyntaxKind,
        CallTargetStatus,
    };
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId};
    use crate::core::{
        AnalysisDb, DefinitionFact, DefinitionId, DefinitionKind, FileId, FunctionFact, FunctionId,
        Language, ReferenceFact, ReferenceId, ReferenceKind, Span, SymbolFact, SymbolId,
        SymbolKind, SymbolNamespace, SymbolPrecision, SymbolResolutionStatus,
    };
    use crate::symbol_graph::semantic::{
        SemanticImportFact, SemanticImportId, SemanticImportKind, SemanticStatus,
    };

    #[test]
    fn lexical_function_call_with_resolved_reference_emits_direct_target() {
        let mut fixture = Fixture::new(Language::TypeScript, "src/caller.ts");
        let target_function = fixture.add_function("target", 10);
        let target_symbol = fixture.add_symbol("target", target_function, 10);
        let reference =
            fixture.add_reference("target", target_symbol, 3, SymbolPrecision::ExactLocal);
        fixture.store_symbols();

        let site = fixture.site(
            1,
            CallSyntaxKind::Function,
            CallCallee::Identifier {
                reference: Some(reference),
                name: "target".to_string(),
            },
            3,
        );
        let targets = resolve_direct_call_targets(&fixture.db, &[site]);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].algorithm, CallAlgorithm::DirectReference);
        assert_eq!(targets[0].status, CallTargetStatus::Resolved);
        assert_eq!(targets[0].target_symbol, Some(target_symbol));
        assert_eq!(targets[0].target_function, Some(target_function));
        assert_eq!(targets[0].edge_kind, CallEdgeKind::Direct);
    }

    #[test]
    fn imported_function_call_uses_import_binding_algorithm() {
        let mut fixture = Fixture::new(Language::TypeScript, "src/caller.ts");
        let target_function = fixture.add_function("makeThing", 20);
        let target_symbol = fixture.add_symbol("makeThing", target_function, 20);
        let reference =
            fixture.add_reference("makeThing", target_symbol, 4, SymbolPrecision::ModuleLinked);
        fixture.semantic_imports.push(SemanticImportFact {
            id: SemanticImportId(0),
            language: Language::TypeScript,
            file: Some(fixture.file),
            package: None,
            module: None,
            scope: None,
            import_path: "./factory".to_string(),
            local_name: Some("makeThing".to_string()),
            imported_name: Some("makeThing".to_string()),
            namespace: SymbolNamespace::Value,
            kind: SemanticImportKind::StaticNamed,
            stable_key: "semantic-import:makeThing".to_string(),
            status: SemanticStatus::Resolved,
        });
        fixture.store_symbols_and_semantic_imports();

        let site = fixture.site(
            2,
            CallSyntaxKind::Function,
            CallCallee::Identifier {
                reference: Some(reference),
                name: "makeThing".to_string(),
            },
            4,
        );
        let targets = resolve_direct_call_targets(&fixture.db, &[site]);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].algorithm, CallAlgorithm::ImportBinding);
        assert_eq!(targets[0].status, CallTargetStatus::Resolved);
        assert_eq!(targets[0].target_symbol, Some(target_symbol));
        assert_eq!(targets[0].target_function, Some(target_function));
    }

    #[test]
    fn static_member_requires_precise_semantic_reference() {
        let mut fixture = Fixture::new(Language::TypeScript, "src/caller.ts");
        let target_function = fixture.add_function("Service.run", 30);
        let target_symbol = fixture.add_symbol("Service.run", target_function, 30);
        let reference =
            fixture.add_reference("run", target_symbol, 5, SymbolPrecision::ExactSemantic);
        fixture.store_symbols();

        let resolved_site = fixture.site(
            3,
            CallSyntaxKind::StaticMember,
            CallCallee::Identifier {
                reference: Some(reference),
                name: "run".to_string(),
            },
            5,
        );
        let dynamic_member = fixture.site(
            4,
            CallSyntaxKind::Member,
            CallCallee::Member {
                base: crate::analysis::ids::PlaceId(7),
                property: "run".to_string(),
            },
            6,
        );
        let targets = resolve_direct_call_targets(&fixture.db, &[resolved_site, dynamic_member]);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].algorithm, CallAlgorithm::StaticMember);
        assert_eq!(targets[0].edge_kind, CallEdgeKind::StaticMember);
        assert_eq!(targets[0].target_symbol, Some(target_symbol));
    }

    struct Fixture {
        db: AnalysisDb,
        file: FileId,
        caller: FunctionId,
        symbols: Vec<SymbolFact>,
        definitions: Vec<DefinitionFact>,
        references: Vec<ReferenceFact>,
        semantic_imports: Vec<SemanticImportFact>,
    }

    impl Fixture {
        fn new(language: Language, path: &str) -> Self {
            let mut db = AnalysisDb::new();
            let file = db.add_source_file(
                PathBuf::from(path),
                path.to_string(),
                language,
                "".into(),
                "content".to_string(),
            );
            let caller = db.push_function(function(file, language, "caller", 1));
            Self {
                db,
                file,
                caller,
                symbols: Vec::new(),
                definitions: Vec::new(),
                references: Vec::new(),
                semantic_imports: Vec::new(),
            }
        }

        fn add_function(&mut self, name: &str, line: u32) -> FunctionId {
            self.db
                .push_function(function(self.file, Language::TypeScript, name, line))
        }

        fn add_symbol(&mut self, name: &str, function: FunctionId, line: u32) -> SymbolId {
            let id = SymbolId(self.symbols.len() as u64);
            self.symbols.push(SymbolFact {
                id,
                language: Language::TypeScript,
                name: name.to_string(),
                qualified_name: name.to_string(),
                kind: SymbolKind::Function,
                namespace: SymbolNamespace::Value,
                file: Some(self.file),
                package: None,
                module: None,
                owner: None,
                primary_span: Some(span(self.file, line)),
                is_exported: true,
                stable_key: format!("symbol:{name}"),
                precision: SymbolPrecision::ExactSemantic,
            });
            self.definitions.push(DefinitionFact {
                id: DefinitionId(id.0),
                symbol: id,
                language: Language::TypeScript,
                name: name.to_string(),
                qualified_name: name.to_string(),
                kind: DefinitionKind::Definition,
                namespace: SymbolNamespace::Value,
                file: Some(self.file),
                package: None,
                module: None,
                owner: None,
                primary_span: Some(span(self.file, line)),
                is_primary: true,
                is_exported: true,
                stable_key: format!("definition:{name}"),
                precision: SymbolPrecision::ExactSemantic,
            });
            assert_eq!(function.0, id.0 + 1);
            id
        }

        fn add_reference(
            &mut self,
            name: &str,
            target: SymbolId,
            line: u32,
            precision: SymbolPrecision,
        ) -> ReferenceId {
            let id = ReferenceId(self.references.len() as u64);
            self.references.push(ReferenceFact {
                id,
                language: Language::TypeScript,
                name: name.to_string(),
                qualified_name: name.to_string(),
                kind: ReferenceKind::Call,
                namespace: SymbolNamespace::Value,
                file: Some(self.file),
                package: None,
                module: None,
                owner: None,
                primary_span: Some(span(self.file, line)),
                target: Some(target),
                candidates: vec![target],
                stable_key: format!("reference:{name}:{}", id.0),
                status: SymbolResolutionStatus::Resolved,
                precision,
            });
            id
        }

        fn site(
            &self,
            id: u64,
            kind: CallSyntaxKind,
            callee: CallCallee,
            line: u32,
        ) -> CallSiteFact {
            CallSiteFact {
                id: CallSiteId(id),
                language: Language::TypeScript,
                file: self.file,
                caller: self.caller,
                owner_symbol: None,
                body: MirBodyId(0),
                operation: MirOpId(id),
                span: span(self.file, line),
                kind,
                callee,
                receiver: None,
                arguments: Vec::new(),
                result: None,
                status: CallTargetStatus::Unresolved,
                precision: CallPrecision::Conservative,
                stable_key: format!("call-site:{id}"),
            }
        }

        fn store_symbols(&mut self) {
            self.db.replace_symbol_graph_facts(
                self.symbols.clone(),
                self.definitions.clone(),
                self.references.clone(),
            );
        }

        fn store_symbols_and_semantic_imports(&mut self) {
            self.store_symbols();
            self.db.replace_semantic_index_facts(
                Vec::new(),
                self.semantic_imports.clone(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            );
        }
    }

    fn function(file: FileId, language: Language, name: &str, line: u32) -> FunctionFact {
        FunctionFact {
            id: FunctionId(999),
            file,
            name: name.to_string(),
            span: span(file, line),
            language,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        }
    }

    fn span(file: FileId, line: u32) -> Span {
        Span {
            file,
            start_byte: line * 10,
            end_byte: line * 10 + 5,
            start_line: line,
            start_col: 1,
            end_line: line,
            end_col: 6,
        }
    }
}
