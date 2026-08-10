    #[test]
    fn module_relationship_core_contract_stores_relationship_facts_with_stable_ids() {
        let mut db = AnalysisDb::new();
        let from_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import { Button } from './button';\nimport React from 'react';\n".to_string(),
        );
        let target_file = db.add_file(
            PathBuf::from("src/button.ts"),
            "src/button.ts".to_string(),
            "export function Button() {}\n".to_string(),
        );
        let span = test_span(from_file, 1);
        let local_import = db.push_import(ImportFact {
            id: ImportId(99),
            file: from_file,
            package: None,
            path: "./button".to_string(),
            span: span.clone(),
            language: Language::TypeScript,
        });
        let external_import = db.push_import(ImportFact {
            id: ImportId(99),
            file: from_file,
            package: None,
            path: "react".to_string(),
            span,
            language: Language::TypeScript,
        });

        db.replace_module_graph_facts(
            vec![
                ResolvedImportFact {
                    id: ResolvedImportId(99),
                    import: local_import,
                    from_file,
                    target_node: Some(ModuleNodeId(1)),
                    status: ResolutionStatus::Resolved,
                    precision: ResolutionPrecision::ExactFile,
                    reason: None,
                },
                ResolvedImportFact {
                    id: ResolvedImportId(99),
                    import: external_import,
                    from_file,
                    target_node: Some(ModuleNodeId(2)),
                    status: ResolutionStatus::External,
                    precision: ResolutionPrecision::ExternalPackage,
                    reason: None,
                },
            ],
            vec![
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::File,
                    label: "src/app.ts".to_string(),
                    file: Some(from_file),
                    package: None,
                    language: Some(Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::File,
                    label: "src/button.ts".to_string(),
                    file: Some(target_file),
                    package: None,
                    language: Some(Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(99),
                    kind: ModuleNodeKind::External,
                    label: "react".to_string(),
                    file: None,
                    package: None,
                    language: Some(Language::TypeScript),
                },
            ],
            vec![
                ModuleEdge {
                    id: ModuleEdgeId(99),
                    from: ModuleNodeId(0),
                    to: ModuleNodeId(1),
                    import: Some(local_import),
                    resolved_import: Some(ResolvedImportId(0)),
                    kind: ModuleEdgeKind::Imports,
                    status: ResolutionStatus::Resolved,
                },
                ModuleEdge {
                    id: ModuleEdgeId(99),
                    from: ModuleNodeId(0),
                    to: ModuleNodeId(2),
                    import: Some(external_import),
                    resolved_import: Some(ResolvedImportId(1)),
                    kind: ModuleEdgeKind::DependsOn,
                    status: ResolutionStatus::External,
                },
            ],
        );

        assert_eq!(db.resolved_imports()[0].id, ResolvedImportId(0));
        assert_eq!(db.resolved_imports()[1].id, ResolvedImportId(1));
        assert_eq!(db.module_nodes()[0].id, ModuleNodeId(0));
        assert_eq!(db.module_nodes()[1].id, ModuleNodeId(1));
        assert_eq!(db.module_nodes()[2].id, ModuleNodeId(2));
        assert_eq!(db.module_edges()[0].id, ModuleEdgeId(0));
        assert_eq!(db.module_edges()[1].id, ModuleEdgeId(1));
        assert_eq!(
            db.module_edges()[1].resolved_import,
            Some(ResolvedImportId(1))
        );
    }

    #[test]
    fn module_relationship_core_contract_remaps_relationship_ids_when_normalizing_inputs() {
        let mut db = AnalysisDb::new();
        let from_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import { Button } from './button';\n".to_string(),
        );
        let target_file = db.add_file(
            PathBuf::from("src/button.ts"),
            "src/button.ts".to_string(),
            "export function Button() {}\n".to_string(),
        );
        let import = db.push_import(ImportFact {
            id: ImportId(99),
            file: from_file,
            package: None,
            path: "./button".to_string(),
            span: test_span(from_file, 1),
            language: Language::TypeScript,
        });

        db.replace_module_graph_facts(
            vec![ResolvedImportFact {
                id: ResolvedImportId(40),
                import,
                from_file,
                target_node: Some(ModuleNodeId(42)),
                status: ResolutionStatus::Resolved,
                precision: ResolutionPrecision::ExactFile,
                reason: None,
            }],
            vec![
                ModuleNode {
                    id: ModuleNodeId(41),
                    kind: ModuleNodeKind::File,
                    label: "src/app.ts".to_string(),
                    file: Some(from_file),
                    package: None,
                    language: Some(Language::TypeScript),
                },
                ModuleNode {
                    id: ModuleNodeId(42),
                    kind: ModuleNodeKind::File,
                    label: "src/button.ts".to_string(),
                    file: Some(target_file),
                    package: None,
                    language: Some(Language::TypeScript),
                },
            ],
            vec![ModuleEdge {
                id: ModuleEdgeId(43),
                from: ModuleNodeId(41),
                to: ModuleNodeId(42),
                import: Some(import),
                resolved_import: Some(ResolvedImportId(40)),
                kind: ModuleEdgeKind::Imports,
                status: ResolutionStatus::Resolved,
            }],
        );

        assert_eq!(db.resolved_imports()[0].target_node, Some(ModuleNodeId(1)));
        assert_eq!(db.module_edges()[0].from, ModuleNodeId(0));
        assert_eq!(db.module_edges()[0].to, ModuleNodeId(1));
        assert_eq!(
            db.module_edges()[0].resolved_import,
            Some(ResolvedImportId(0))
        );
    }

    #[test]
    fn symbol_fact_contract_preserves_provider_ids_and_indexes_queries() {
        let mut db = AnalysisDb::new();
        let app_file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function Button() { return theme; }\n".to_string(),
        );
        let theme_file = db.add_file(
            PathBuf::from("src/theme.ts"),
            "src/theme.ts".to_string(),
            "export const theme = {};\n".to_string(),
        );

        let interner = db.stable_key_interner();
        db.replace_symbol_graph_facts(
            vec![
                SymbolFact {
                    id: SymbolId(0xfeed_beef),
                    language: Language::TypeScript,
                    name: "Button".to_string(),
                    qualified_name: "src/app.ts::Button".to_string(),
                    kind: SymbolKind::Function,
                    namespace: SymbolNamespace::Value,
                    file: Some(app_file),
                    package: None,
                    module: Some(ModuleNodeId(10)),
                    owner: None,
                    primary_span: Some(test_span(app_file, 1)),
                    is_exported: true,
                    stable_key: interner
                        .intern("ts|src/app.ts|value|function|Button|1:1".to_string()),
                    precision: SymbolPrecision::ExactLocal,
                },
                SymbolFact {
                    id: SymbolId(0xabc0_1234),
                    language: Language::TypeScript,
                    name: "theme".to_string(),
                    qualified_name: "src/theme.ts::theme".to_string(),
                    kind: SymbolKind::Constant,
                    namespace: SymbolNamespace::Value,
                    file: Some(theme_file),
                    package: None,
                    module: Some(ModuleNodeId(11)),
                    owner: None,
                    primary_span: Some(test_span(theme_file, 1)),
                    is_exported: true,
                    stable_key: interner
                        .intern("ts|src/theme.ts|value|constant|theme|1:1".to_string()),
                    precision: SymbolPrecision::ModuleLinked,
                },
            ],
            vec![DefinitionFact {
                id: DefinitionId(0x1010_2020),
                symbol: SymbolId(0xfeed_beef),
                language: Language::TypeScript,
                name: "Button".to_string(),
                qualified_name: "src/app.ts::Button".to_string(),
                kind: DefinitionKind::Declaration,
                namespace: SymbolNamespace::Value,
                file: Some(app_file),
                package: None,
                module: Some(ModuleNodeId(10)),
                owner: None,
                primary_span: Some(test_span(app_file, 1)),
                is_primary: true,
                is_exported: true,
                stable_key: interner.intern("ts|src/app.ts|definition|Button|1:1".to_string()),
                precision: SymbolPrecision::ExactLocal,
            }],
            vec![
                ReferenceFact {
                    id: ReferenceId(0x3030_4040),
                    language: Language::TypeScript,
                    name: "theme".to_string(),
                    qualified_name: "src/theme.ts::theme".to_string(),
                    kind: ReferenceKind::Read,
                    namespace: SymbolNamespace::Value,
                    file: Some(app_file),
                    package: None,
                    module: Some(ModuleNodeId(10)),
                    owner: Some(SymbolId(0xfeed_beef)),
                    primary_span: Some(test_span(app_file, 1)),
                    target: Some(SymbolId(0xabc0_1234)),
                    candidates: Vec::new(),
                    stable_key: interner
                        .intern("ts|src/app.ts|reference|theme|1:28".to_string()),
                    status: SymbolResolutionStatus::Resolved,
                    precision: SymbolPrecision::ModuleLinked,
                },
                ReferenceFact {
                    id: ReferenceId(0x5050_6060),
                    language: Language::TypeScript,
                    name: "missing".to_string(),
                    qualified_name: "missing".to_string(),
                    kind: ReferenceKind::Read,
                    namespace: SymbolNamespace::Value,
                    file: Some(app_file),
                    package: None,
                    module: Some(ModuleNodeId(10)),
                    owner: Some(SymbolId(0xfeed_beef)),
                    primary_span: Some(test_span(app_file, 2)),
                    target: None,
                    candidates: vec![SymbolId(0xfeed_beef), SymbolId(0xabc0_1234)],
                    stable_key: interner
                        .intern("ts|src/app.ts|reference|missing|2:1".to_string()),
                    status: SymbolResolutionStatus::Ambiguous,
                    precision: SymbolPrecision::Ambiguous,
                },
            ],
        );

        assert_eq!(db.symbols()[0].id, SymbolId(0xfeed_beef));
        assert_eq!(db.definitions()[0].id, DefinitionId(0x1010_2020));
        assert_eq!(db.references()[0].id, ReferenceId(0x3030_4040));
        assert_eq!(
            db.symbol_by_id(SymbolId(0xabc0_1234))
                .map(|symbol| symbol.name.as_str()),
            Some("theme")
        );
        assert_eq!(
            db.symbols_for_file(app_file)
                .map(|symbol| symbol.id)
                .collect::<Vec<_>>(),
            vec![SymbolId(0xfeed_beef)]
        );
        assert_eq!(
            db.symbols_by_name("Button")
                .map(|symbol| symbol.id)
                .collect::<Vec<_>>(),
            vec![SymbolId(0xfeed_beef)]
        );
        assert_eq!(
            db.definitions_for_symbol(SymbolId(0xfeed_beef))
                .map(|definition| definition.id)
                .collect::<Vec<_>>(),
            vec![DefinitionId(0x1010_2020)]
        );
        assert_eq!(
            db.definition_for_symbol(SymbolId(0xfeed_beef))
                .map(|definition| definition.id),
            Some(DefinitionId(0x1010_2020))
        );
        assert_eq!(
            db.references_to_symbol(SymbolId(0xabc0_1234))
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            vec![ReferenceId(0x3030_4040)]
        );
        assert_eq!(
            db.references_for_file(app_file)
                .map(|reference| reference.id)
                .collect::<Vec<_>>(),
            vec![ReferenceId(0x3030_4040), ReferenceId(0x5050_6060)]
        );

        let precision_statuses = [
            SymbolPrecision::ExactSemantic,
            SymbolPrecision::ExactLocal,
            SymbolPrecision::ModuleLinked,
            SymbolPrecision::Heuristic,
            SymbolPrecision::Unresolved,
            SymbolPrecision::Ambiguous,
            SymbolPrecision::SetupMissing,
            SymbolPrecision::Unsupported,
        ];
        assert_eq!(precision_statuses.len(), 8);

        let resolution_statuses = [
            SymbolResolutionStatus::Resolved,
            SymbolResolutionStatus::Unresolved,
            SymbolResolutionStatus::Ambiguous,
            SymbolResolutionStatus::SetupMissing,
            SymbolResolutionStatus::Unsupported,
        ];
        assert_eq!(resolution_statuses.len(), 5);

        let capabilities = Capabilities::new().references();
        assert!(capabilities.references);
        assert!(capabilities.symbols);
    }

    #[test]
    fn module_relationship_core_contract_statuses_are_representable() {
        let statuses = [
            ResolutionStatus::Resolved,
            ResolutionStatus::External,
            ResolutionStatus::Unresolved,
            ResolutionStatus::SetupMissing,
            ResolutionStatus::Dynamic,
            ResolutionStatus::Unsupported,
        ];
        let reasons = [
            UnresolvedReason::NotFound,
            UnresolvedReason::SetupMissing,
            UnresolvedReason::DynamicExpression,
            UnresolvedReason::UnsupportedLanguage,
            UnresolvedReason::UnsupportedImport,
            UnresolvedReason::ResolverError,
            UnresolvedReason::OutsideWorkspace,
        ];

        assert!(matches!(statuses[0], ResolutionStatus::Resolved));
        assert!(matches!(statuses[1], ResolutionStatus::External));
        assert!(matches!(statuses[2], ResolutionStatus::Unresolved));
        assert!(matches!(statuses[3], ResolutionStatus::SetupMissing));
        assert!(matches!(statuses[4], ResolutionStatus::Dynamic));
        assert!(matches!(statuses[5], ResolutionStatus::Unsupported));
        assert_eq!(reasons.len(), 7);
    }

    #[test]
    fn module_relationship_core_contract_public_enums_match_with_wildcard() {
        fn status_name(status: ResolutionStatus) -> &'static str {
            match status {
                ResolutionStatus::Resolved => "resolved",
                _ => "not-resolved",
            }
        }

        fn node_kind_name(kind: ModuleNodeKind) -> &'static str {
            match kind {
                ModuleNodeKind::File => "file",
                _ => "other",
            }
        }

        fn edge_kind_name(kind: ModuleEdgeKind) -> &'static str {
            match kind {
                ModuleEdgeKind::Imports => "imports",
                _ => "other",
            }
        }

        fn precision_name(precision: ResolutionPrecision) -> &'static str {
            match precision {
                ResolutionPrecision::ExactFile => "exact-file",
                _ => "other",
            }
        }

        fn reason_name(reason: UnresolvedReason) -> &'static str {
            match reason {
                UnresolvedReason::NotFound => "not-found",
                _ => "other",
            }
        }

        assert_eq!(status_name(ResolutionStatus::Resolved), "resolved");
        assert_eq!(node_kind_name(ModuleNodeKind::Package), "other");
        assert_eq!(edge_kind_name(ModuleEdgeKind::Contains), "other");
        assert_eq!(
            precision_name(ResolutionPrecision::ExternalPackage),
            "other"
        );
        assert_eq!(reason_name(UnresolvedReason::SetupMissing), "other");
    }

    #[test]
    fn analysis_db_exposes_all_phase3_fact_families() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/component.tsx"),
            "src/component.tsx".to_string(),
            "export function Button() { return <button aria-label=\"Save\" /> }\n".to_string(),
        );
        let span = test_span(file, 1);
        let function = db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "Button".to_string(),
            span: span.clone(),
            language: Language::Tsx,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: vec!["render".to_string()],
        });
        let import = db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: Some("react".to_string()),
            path: "react".to_string(),
            span: span.clone(),
            language: Language::Tsx,
        });
        let branch = db.push_branch(BranchObligation {
            id: BranchId(99),
            function: Some(function),
            file,
            decision_span: span.clone(),
            condition_text: "enabled".to_string(),
            edge_label: "true".to_string(),
            is_error_path: false,
            stable_fingerprint: "branch".to_string(),
        });
        db.push_test(TestFact {
            file,
            function: Some(function),
            name: "Button test".to_string(),
            span: span.clone(),
            evidence_terms: vec!["render".to_string()],
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_coverage(CoverageFact {
            branch,
            covered: Some(true),
            source: "synthetic-coverage".to_string(),
        });
        db.push_ts_component(TsComponentFact {
            file,
            function: Some(function),
            name: "Button".to_string(),
            span: span.clone(),
        });
        db.push_string_literal(StringLiteralFact {
            file,
            value: "Save".to_string(),
            span: span.clone(),
            language: Language::Tsx,
        });
        db.push_jsx_attribute(JsxAttributeFact {
            file,
            name: "aria-label".to_string(),
            value: Some("Save".to_string()),
            span,
        });

        assert_eq!(db.files()[0].id, file);
        assert_eq!(db.functions()[0].id, function);
        assert_eq!(db.imports()[0].id, import);
        assert_eq!(db.branches()[0].id, branch);
        assert_eq!(db.tests()[0].name, "Button test");
        assert_eq!(db.coverage()[0].covered, Some(true));
        assert_eq!(db.ts_components()[0].name, "Button");
        assert_eq!(db.string_literals()[0].value, "Save");
        assert_eq!(db.jsx_attributes()[0].name, "aria-label");
    }

    #[test]
    fn span_from_byte_range_handles_utf8_newlines_and_empty_ranges() {
        let source = "aé\nβ\n";
        let file = FileId(7);

        let utf8 = span_from_byte_range(file, source, 1, 3);
        assert_eq!(utf8.diagnostic_range(), diagnostic_range(1, 2, 1, 3));

        let newline = span_from_byte_range(file, source, 3, 4);
        assert_eq!(newline.diagnostic_range(), diagnostic_range(1, 3, 2, 1));

        let empty = span_from_byte_range(file, source, 4, 4);
        assert_eq!(empty.diagnostic_range(), diagnostic_range(2, 1, 2, 1));

        let clamped = span_from_byte_range(file, source, source.len() + 10, source.len() + 20);
        assert_eq!(clamped.start_byte as usize, source.len());
        assert_eq!(clamped.end_byte as usize, source.len());
        assert_eq!(clamped.diagnostic_range(), diagnostic_range(3, 1, 3, 1));
    }

    #[test]
    fn rule_pattern_matches_prefix() {
        assert!(rule_id_matches("examples/*", "examples/ts-no-raw-colors"));
        assert!(!rule_id_matches("custom/*", "examples/ts-no-raw-colors"));
    }

    #[test]
    fn extension_facts_are_sidecar_metadata_and_rejections_are_audit_only() {
        let mut db = AnalysisDb::new();
        db.replace_extension_facts(ExtensionOutput {
            activations: vec![ExtensionActivationRow {
                extension_id: "demo".to_string(),
                provider_id: Some("routes".to_string()),
                status: crate::analysis::extensions::manifest::ExtensionActivationStatus::Active,
                diagnostic_count: 0,
                output_digest_inputs: Vec::new(),
                diagnostic_digest: "empty".to_string(),
            }],
            accepted: vec![AcceptedExtensionFact {
                extension_id: "demo".to_string(),
                provider_id: "routes".to_string(),
                fact_family: "extension.routes".to_string(),
                stable_key: crate::core::stable_key_for_test("route:/a"),
                binding_refs: vec!["file:src/app.ts".to_string()],
                precision: ExtensionFactPrecision::Heuristic,
                confidence: ExtensionFactConfidence::Medium,
                status: crate::analysis::extensions::sinks::ExtensionFactStatus::Accepted,
                evidence: vec!["fixture".to_string()],
                payload_labels: vec!["method=GET".to_string()],
                payload_digest: "payload".to_string(),
            }],
            rejected: vec![RejectedExtensionFact {
                extension_id: "demo".to_string(),
                provider_id: "routes".to_string(),
                fact_family: "extension.routes".to_string(),
                stable_key: crate::core::stable_key_for_test("route:/bad"),
                reason:
                    crate::analysis::extensions::validate::ExtensionRejectionReason::NativeConflict,
                evidence: vec!["fixture".to_string()],
            }],
        });

        assert_eq!(db.extension_facts().len(), 1);
        assert_eq!(db.extension_activations().len(), 1);
        assert_eq!(db.rejected_extension_facts().len(), 1);
        let metadata = db
            .metadata_for(FactRef::new(FactFamily::ExtensionFact, 0))
            .expect("extension metadata exists");
        assert_eq!(metadata.producer_id, "polint.extension.demo.routes");
        assert_eq!(metadata.layer_id, "polint.extension.demo.routes");
        assert_eq!(metadata.precision, FactPrecision::Heuristic);
        assert_eq!(metadata.validation, ValidationStatus::SchemaValidated);
    }

    #[test]
    fn evidence_exact_rows_do_not_exceed_setup_aware_metadata_ceiling() {
        let mut db = AnalysisDb::new();
        db.replace_evidence_facts(crate::analysis::evidence::store::EvidenceOutput {
            nodes: vec![EvidenceNodeFact {
                id: crate::analysis::ids::EvidenceNodeId(0),
                kind: crate::analysis::evidence::facts::EvidenceNodeKind::Operation,
                language: Language::Go,
                file: None,
                function: None,
                body: None,
                operation: None,
                cfg_node: None,
                place: None,
                symbol: None,
                reference: None,
                call_site: None,
                span: None,
                status: EvidenceStatus::Present,
                precision: EvidencePrecision::Exact,
                provenance: EvidenceProvenance::Native,
                validation: EvidenceValidation::Native,
                confidence: EvidenceConfidence::High,
                compact_label: None,
                source_fact_stable_keys: Vec::new(),
                stable_key: crate::core::stable_key_for_test("evidence:node:exact"),
            }],
            ..crate::analysis::evidence::store::EvidenceOutput::empty()
        })
        .expect("valid evidence output");

        let metadata = db
            .metadata_for(FactRef::new(FactFamily::EvidenceNode, 0))
            .expect("evidence metadata exists");
        assert_eq!(metadata.producer_id, "polint.evidence");
        assert_eq!(metadata.precision, FactPrecision::SetupAware);
    }

    proptest! {
        #[test]
        fn span_from_byte_range_is_monotonic_for_char_boundaries(source in "\\PC*") {
            let mut offsets: Vec<usize> = source.char_indices().map(|(idx, _)| idx).collect();
            offsets.push(source.len());

            for start in &offsets {
                for end in offsets.iter().filter(|end| *end >= start) {
                    let span = span_from_byte_range(FileId(0), &source, *start, *end);
                    let range = span.diagnostic_range();
                    prop_assert!(
                        (range.end_line, range.end_col) >= (range.start_line, range.start_col),
                        "range {range:?} from offsets {start}..{end} in {source:?}"
                    );
                }
            }
        }
    }
