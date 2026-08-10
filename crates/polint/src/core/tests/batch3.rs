    #[test]
    fn topology_storage_replaces_rows_and_rebuilds_metadata() {
        let mut db = AnalysisDb::new();
        db.replace_topology_facts(topology_output("first"));
        db.replace_topology_facts(topology_output("second"));

        assert_eq!(db.workspace_roots().len(), 1);
        assert_eq!(db.workspace_roots()[0].id.0, 0);
        assert_eq!(
            db.resolve_stable_key(db.workspace_roots()[0].stable_key).as_ref(),
            "second:root"
        );
        assert_eq!(db.topology_packages()[0].id.0, 0);
        assert_eq!(db.source_sets()[0].id.0, 0);
        assert_eq!(db.dependency_requirements()[0].id.0, 0);
        assert_eq!(db.resolved_dependency_edges()[0].id.0, 0);
        assert_eq!(db.import_to_package_edges()[0].id.0, 0);
        assert_eq!(db.repo_topology_overlays()[0].id.0, 0);
        assert!(
            db.metadata_for(FactRef::new(FactFamily::WorkspaceRoot, 1))
                .is_none()
        );
        assert!(
            db.metadata_for(FactRef::new(FactFamily::WorkspaceRoot, 0))
                .is_some()
        );
    }

    #[test]
    fn topology_storage_replaces_import_to_package_edges_only() {
        let mut db = AnalysisDb::new();
        db.replace_topology_facts(topology_output("base"));
        let mut edges = topology_output("updated").import_to_package_edges;
        edges[0].id = crate::module_graph::topology::ImportToPackageId(42);

        db.replace_import_to_package_facts(edges);

        assert_eq!(
            db.resolve_stable_key(db.workspace_roots()[0].stable_key).as_ref(),
            "base:root"
        );
        assert_eq!(
            db.resolve_stable_key(db.topology_packages()[0].stable_key)
                .as_ref(),
            "base:package"
        );
        assert_eq!(
            db.resolve_stable_key(db.source_sets()[0].stable_key).as_ref(),
            "base:source-set"
        );
        assert_eq!(
            db.resolve_stable_key(db.dependency_requirements()[0].stable_key)
                .as_ref(),
            "base:requirement"
        );
        assert_eq!(
            db.resolve_stable_key(db.resolved_dependency_edges()[0].stable_key)
                .as_ref(),
            "base:resolved"
        );
        assert_eq!(
            db.resolve_stable_key(db.repo_topology_overlays()[0].stable_key)
                .as_ref(),
            "base:overlay"
        );
        assert_eq!(
            db.resolve_stable_key(db.import_to_package_edges()[0].stable_key)
                .as_ref(),
            "updated:import-to-package"
        );
        assert_eq!(db.import_to_package_edges()[0].id.0, 0);
    }

    #[test]
    fn topology_storage_records_provider_metadata_for_every_row() {
        let mut db = AnalysisDb::new();
        db.replace_topology_facts(topology_output("meta"));

        for family in [
            FactFamily::WorkspaceRoot,
            FactFamily::TopologyPackage,
            FactFamily::SourceSet,
            FactFamily::DependencyRequirement,
            FactFamily::ResolvedDependencyEdge,
            FactFamily::RepoTopologyOverlay,
        ] {
            let metadata = db
                .metadata_for(FactRef::new(family, 0))
                .expect("topology metadata exists");
            assert_eq!(metadata.producer_id, MODULE_GRAPH_PROVIDER_ID);
        }

        let metadata = db
            .metadata_for(FactRef::new(FactFamily::ImportToPackage, 0))
            .expect("import-to-package metadata exists");
        assert_eq!(metadata.producer_id, MODULE_TOPOLOGY_PROVIDER_ID);
    }

    #[test]
    fn source_file_metadata_records_provider_and_stable_key_inputs() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src\\main.go"),
            "src\\main.go".to_string(),
            "package main\n".to_string(),
        );

        let metadata = db
            .metadata_for(FactRef::new(FactFamily::SourceFile, u64::from(file.0)))
            .expect("source metadata should be recorded");

        assert_eq!(metadata.producer_id, "polint.source");
        assert_eq!(metadata.layer_id, "polint.source");
        assert_eq!(metadata.precision, FactPrecision::Exact);
        assert_eq!(metadata.confidence, FactConfidence::High);
        assert_eq!(metadata.validation, ValidationStatus::NativeTrusted);
        assert!(metadata.stable_key.contains("4:path=11:src/main.go"));
        assert!(metadata.stable_key.contains("12:content_hash="));
        assert!(
            db.fact_meta_mut_for_test()
                .get(FactRef::new(FactFamily::SourceFile, u64::from(file.0)))
                .is_some()
        );
    }

    #[test]
    fn syntax_metadata_uses_language_specific_producers() {
        let mut db = AnalysisDb::new();
        let go_file = db.add_file(
            PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\nimport \"fmt\"\n".to_string(),
        );
        let ts_file = db.add_file(
            PathBuf::from("src/component.tsx"),
            "src/component.tsx".to_string(),
            "export class Button {}".to_string(),
        );
        let go_span = test_span(go_file, 1);
        let ts_span = test_span(ts_file, 1);

        let import = db.push_import(ImportFact {
            id: ImportId(999),
            file: go_file,
            package: None,
            path: "fmt".to_string(),
            span: go_span,
            language: Language::Go,
        });
        db.push_ts_class(TsClassFact {
            file: ts_file,
            name: "Button".to_string(),
            span: ts_span,
            is_exported: true,
            is_component_like: true,
        });

        let import_meta = db
            .metadata_for(FactRef::new(FactFamily::Import, import.0))
            .expect("import metadata should be recorded");
        let class_meta = db
            .metadata_for(FactRef::new(FactFamily::TsClass, 0))
            .expect("TS class metadata should be recorded");

        assert_eq!(import_meta.producer_id, "polint.go.syntax");
        assert_eq!(import_meta.precision, FactPrecision::Syntax);
        assert_eq!(class_meta.producer_id, "polint.ts.syntax");
        assert_eq!(class_meta.precision, FactPrecision::Syntax);
    }

    #[test]
    fn restore_file_facts_recreates_metadata_for_cached_syntax_facts() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/component.tsx"),
            "src/component.tsx".to_string(),
            "export function Button() { return <button aria-label=\"Save\" /> }\n".to_string(),
        );
        let span = test_span(file, 1);

        db.restore_file_facts(
            file,
            CachedFileFacts {
                packages: vec![PackageFact {
                    id: PackageId(99),
                    file,
                    name: "main".to_string(),
                    span: span.clone(),
                    language: Language::Go,
                }],
                functions: vec![FunctionFact {
                    id: FunctionId(99),
                    file,
                    name: "Button".to_string(),
                    span: span.clone(),
                    language: Language::Tsx,
                    is_test: false,
                    is_exported: true,
                    cyclomatic_complexity: 1,
                    calls: vec!["render".to_string()],
                }],
                imports: vec![ImportFact {
                    id: ImportId(99),
                    file,
                    package: Some("react".to_string()),
                    path: "react".to_string(),
                    span: span.clone(),
                    language: Language::Tsx,
                }],
                branches: vec![BranchObligation {
                    id: BranchId(99),
                    function: Some(FunctionId(99)),
                    file,
                    decision_span: span.clone(),
                    condition_text: "enabled".to_string(),
                    edge_label: "true".to_string(),
                    is_error_path: false,
                    stable_fingerprint: "branch".to_string(),
                }],
                tests: vec![TestFact {
                    file,
                    function: Some(FunctionId(99)),
                    name: "TestButton".to_string(),
                    span: span.clone(),
                    evidence_terms: vec!["render".to_string()],
                    assertion_count: 1,
                    subtest_count: 0,
                    subtest_names: Vec::new(),
                    table_rows: 0,
                }],
                coverage: vec![CoverageFact {
                    branch: BranchId(99),
                    covered: Some(true),
                    source: "synthetic".to_string(),
                }],
                ts_components: vec![TsComponentFact {
                    file,
                    function: Some(FunctionId(99)),
                    name: "Button".to_string(),
                    span: span.clone(),
                }],
                ts_classes: vec![TsClassFact {
                    file,
                    name: "Dialog".to_string(),
                    span: span.clone(),
                    is_exported: true,
                    is_component_like: false,
                }],
                string_literals: vec![StringLiteralFact {
                    file,
                    value: "Save".to_string(),
                    span: span.clone(),
                    language: Language::Tsx,
                }],
                jsx_attributes: vec![JsxAttributeFact {
                    file,
                    name: "aria-label".to_string(),
                    value: Some("Save".to_string()),
                    span,
                }],
            },
        );

        for family in [
            FactFamily::Package,
            FactFamily::Function,
            FactFamily::Import,
            FactFamily::BranchObligation,
            FactFamily::Test,
            FactFamily::Coverage,
            FactFamily::TsComponent,
            FactFamily::TsClass,
            FactFamily::StringLiteral,
            FactFamily::JsxAttribute,
        ] {
            assert!(
                db.metadata_for(FactRef::new(family, 0)).is_some(),
                "missing restored metadata for {family:?}"
            );
        }
    }

    #[test]
    fn capability_support_view_reports_status_for_capability() {
        let view = CapabilitySupportView::new(vec![CapabilitySupport {
            capability: "imports".to_string(),
            language: Some(Language::Go),
            status: CapabilitySupportStatus::Supported,
            rules: vec!["examples/imports".to_string()],
            reason: None,
            hint: None,
            docs_path: None,
        }]);

        assert_eq!(
            view.status_for("imports"),
            Some(CapabilitySupportStatus::Supported)
        );
        assert!(view.status_for("cfg").is_none());
        assert_eq!(view.entries().len(), 1);
    }

    #[test]
    fn capability_support_defaults_empty_for_rule_ctx_constructor() {
        let db = AnalysisDb::new();
        let ctx = RuleCtx::new(
            &db,
            RuleMeta {
                id: "examples/support".to_string(),
                description: "Support view constructor test".to_string(),
                severity: Severity::Warn,
                kind: RuleKind::Check,
            },
            RuleOptions::default(),
        );

        assert!(ctx.capability_support().entries().is_empty());
    }

    #[test]
    fn policy_preview_capabilities_have_distinct_names() {
        let capabilities = Capabilities::new()
            .events()
            .calls()
            .control_flow()
            .dataflow()
            .cfg()
            .call_graph();

        assert_eq!(
            capabilities.requested_names().collect::<Vec<_>>(),
            vec![
                "events",
                "calls",
                "control_flow",
                "cfg",
                "call_graph",
                "dataflow"
            ]
        );
    }

    #[test]
    fn capability_support_runner_supplies_view_to_rules() {
        let rule = Rule::from_parts(
            || RuleMeta {
                id: "examples/support-probe".to_string(),
                description: "Support probe".to_string(),
                severity: Severity::Warn,
                kind: RuleKind::Check,
            },
            || Capabilities::new().imports(),
            |_db, ctx| {
                if ctx.capability_support().status_for("imports")
                    == Some(CapabilitySupportStatus::Supported)
                {
                    ctx.report(Diagnostic::warning(
                        ctx.rule_id(),
                        "<workspace>",
                        DiagnosticRange::point(1, 1),
                        "imports are supported",
                    ));
                }
                Ok(())
            },
        );

        let db = AnalysisDb::new();
        let rules = vec![rule];
        let support_view = CapabilitySupportView::new(vec![CapabilitySupport {
            capability: "imports".to_string(),
            language: Some(Language::Go),
            status: CapabilitySupportStatus::Supported,
            rules: vec!["examples/support-probe".to_string()],
            reason: None,
            hint: None,
            docs_path: None,
        }]);

        let diagnostics = run_rules_with_capability_support(
            &db,
            &rules,
            &BTreeMap::new(),
            None,
            false,
            &support_view,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "imports are supported");
    }

    #[test]
    fn run_rules_skips_rules_with_blocking_capabilities() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::report("examples/needs-cfg", Severity::Warn, "cfg")
                .with_capabilities(Capabilities::new().cfg())
                .into_rule(),
            TestRule::panic("examples/needs-dataflow")
                .with_capabilities(Capabilities::new().dataflow())
                .into_rule(),
            TestRule::report("examples/imports", Severity::Warn, "imports")
                .with_capabilities(Capabilities::new().imports())
                .into_rule(),
        ];
        let support_view = CapabilitySupportView::new(vec![
            CapabilitySupport {
                capability: "cfg".to_string(),
                language: Some(Language::Go),
                status: CapabilitySupportStatus::Unsupported,
                rules: vec!["examples/needs-cfg".to_string()],
                reason: None,
                hint: None,
                docs_path: None,
            },
            CapabilitySupport {
                capability: "dataflow".to_string(),
                language: Some(Language::Go),
                status: CapabilitySupportStatus::SetupMissing,
                rules: vec!["examples/needs-dataflow".to_string()],
                reason: None,
                hint: None,
                docs_path: None,
            },
            CapabilitySupport {
                capability: "imports".to_string(),
                language: Some(Language::Go),
                status: CapabilitySupportStatus::Supported,
                rules: vec!["examples/imports".to_string()],
                reason: None,
                hint: None,
                docs_path: None,
            },
        ]);

        let diagnostics = run_rules_with_capability_support(
            &db,
            &rules,
            &BTreeMap::new(),
            None,
            false,
            &support_view,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "examples/imports");
    }

    #[test]
    fn cached_file_facts_round_trip_remaps_ids() {
        let mut source_db = AnalysisDb::new();
        let source_file = source_db.add_file(
            PathBuf::from("src/payment.go"),
            "src/payment.go".to_string(),
            "package payments\nfunc Authorize() {}".to_string(),
        );
        let function = source_db.push_function(FunctionFact {
            id: FunctionId(999),
            file: source_file,
            name: "Authorize".to_string(),
            span: test_span(source_file, 2),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 2,
            calls: vec!["audit".to_string()],
        });
        let branch = source_db.push_branch(BranchObligation {
            id: BranchId(999),
            function: Some(function),
            file: source_file,
            decision_span: test_span(source_file, 3),
            condition_text: "err != nil".to_string(),
            edge_label: "true".to_string(),
            is_error_path: true,
            stable_fingerprint: "branch".to_string(),
        });
        source_db.push_coverage(CoverageFact {
            branch,
            covered: Some(false),
            source: "static".to_string(),
        });
        source_db.push_test(TestFact {
            file: source_file,
            function: Some(function),
            name: "TestAuthorize".to_string(),
            span: test_span(source_file, 5),
            evidence_terms: vec!["Authorize".to_string()],
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });

        let cached = source_db.facts_for_file(source_file);

        let mut restored_db = AnalysisDb::new();
        let target_file = restored_db.add_file(
            PathBuf::from("src/payment.go"),
            "src/payment.go".to_string(),
            "package payments\nfunc Authorize() {}".to_string(),
        );
        let existing_function = restored_db.push_function(FunctionFact {
            id: FunctionId(999),
            file: target_file,
            name: "Existing".to_string(),
            span: test_span(target_file, 1),
            language: Language::Go,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        restored_db.push_branch(BranchObligation {
            id: BranchId(999),
            function: Some(existing_function),
            file: target_file,
            decision_span: test_span(target_file, 1),
            condition_text: "existing".to_string(),
            edge_label: "true".to_string(),
            is_error_path: false,
            stable_fingerprint: "existing".to_string(),
        });

        restored_db.restore_file_facts(target_file, cached);

        let restored_function = restored_db
            .functions()
            .iter()
            .find(|fact| fact.name == "Authorize")
            .unwrap();
        let restored_branch = restored_db
            .branches()
            .iter()
            .find(|fact| fact.stable_fingerprint == "branch")
            .unwrap();
        assert_ne!(restored_function.id, function);
        assert_eq!(restored_branch.function, Some(restored_function.id));
        assert_eq!(
            restored_db.coverage().last().unwrap().branch,
            restored_branch.id
        );
        assert_eq!(
            restored_db.tests().last().unwrap().function,
            Some(restored_function.id)
        );
    }

    #[test]
    fn cached_file_analysis_does_not_include_source_text() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/secret.go"),
            "src/secret.go".to_string(),
            "package main\nconst token = \"super-secret-full-source\"".to_string(),
        );
        db.push_package(PackageFact {
            id: PackageId(999),
            file,
            name: "main".to_string(),
            span: test_span(file, 1),
            language: Language::Go,
        });

        let cached = CachedFileAnalysis {
            schema: "go-facts-v1".to_string(),
            diagnostics: Vec::new(),
            facts: db.facts_for_file(file),
        };
        let serialized = format!("{cached:?}");

        assert!(!serialized.contains("super-secret-full-source"));
        assert!(!serialized.contains("source"));
        assert!(!serialized.contains("ast"));
        assert!(!serialized.contains("tree"));
    }

    #[test]
    fn analysis_db_exposes_ts_class_facts() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/component.tsx"),
            "src/component.tsx".to_string(),
            "export class Button {}".to_string(),
        );
        let first_span = test_span(file, 1);
        let second_span = test_span(file, 5);

        db.push_ts_class(TsClassFact {
            file,
            name: "Button".to_string(),
            span: first_span.clone(),
            is_exported: true,
            is_component_like: true,
        });
        db.push_ts_class(TsClassFact {
            file,
            name: "Store".to_string(),
            span: second_span.clone(),
            is_exported: false,
            is_component_like: false,
        });

        let classes = db.ts_classes();
        assert_eq!(classes.len(), 2);
        assert_eq!(classes[0].file, file);
        assert_eq!(classes[0].name, "Button");
        assert_eq!(classes[0].span, first_span);
        assert!(classes[0].is_exported);
        assert!(classes[0].is_component_like);
        assert_eq!(classes[1].file, file);
        assert_eq!(classes[1].name, "Store");
        assert_eq!(classes[1].span, second_span);
        assert!(!classes[1].is_exported);
        assert!(!classes[1].is_component_like);
    }

    #[test]
    fn fact_view_exposes_ts_classes() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/dialog.tsx"),
            "src/dialog.tsx".to_string(),
            "class Dialog {}".to_string(),
        );
        let span = test_span(file, 1);
        db.push_ts_class(TsClassFact {
            file,
            name: "Dialog".to_string(),
            span,
            is_exported: false,
            is_component_like: true,
        });

        let classes = TsClasses::build(&db);

        assert_eq!(classes.all().len(), 1);
        assert_eq!(classes.all()[0].name, db.ts_classes()[0].name);
        assert_eq!(classes.all()[0].span, db.ts_classes()[0].span);
    }

