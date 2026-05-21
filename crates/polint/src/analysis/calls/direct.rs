use crate::analysis::calls::facts::{CallSiteFact, CallTargetFact};
use crate::core::AnalysisDb;

pub(crate) fn resolve_direct_call_targets(
    _db: &AnalysisDb,
    _sites: &[CallSiteFact],
) -> Vec<CallTargetFact> {
    Vec::new()
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
        AnalysisDb, DefinitionFact, DefinitionId, DefinitionKind, FileId, FunctionFact,
        FunctionId, Language, ReferenceFact, ReferenceId, ReferenceKind, Span, SymbolFact,
        SymbolId, SymbolKind, SymbolNamespace, SymbolPrecision, SymbolResolutionStatus,
    };
    use crate::symbol_graph::semantic::{
        SemanticImportFact, SemanticImportId, SemanticImportKind, SemanticStatus,
    };

    #[test]
    fn lexical_function_call_with_resolved_reference_emits_direct_target() {
        let mut fixture = Fixture::new(Language::TypeScript, "src/caller.ts");
        let target_function = fixture.add_function("target", 10);
        let target_symbol = fixture.add_symbol("target", target_function, 10);
        let reference = fixture.add_reference("target", target_symbol, 3, SymbolPrecision::ExactLocal);
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
            self.db.push_function(function(self.file, Language::TypeScript, name, line))
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
