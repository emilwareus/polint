    #[test]
    fn rule_ctx_exposes_sdk_query_helpers() {
        let mut db = AnalysisDb::new();
        let go_file = db.add_file(
            PathBuf::from("src/payment.go"),
            "src/payment.go".to_string(),
            "package payment\n".to_string(),
        );
        let ts_file = db.add_file(
            PathBuf::from("src/component.tsx"),
            "src/component.tsx".to_string(),
            "export function Button() { return <button aria-label=\"Pay\">Pay</button>; }"
                .to_string(),
        );
        let go_span = test_span(go_file, 1);
        let ts_span = test_span(ts_file, 1);

        db.push_package(PackageFact::new(PackageId::from_raw(99), go_file, "payment".to_string(), go_span.clone(), Language::Go));
        let go_function = db.push_function(FunctionFact::new(FunctionId::from_raw(99), go_file, "Charge".to_string(), go_span.clone(), Language::Go, false, true, 3, vec!["authorize".to_string()]));
        let ts_function = db.push_function(FunctionFact::new(FunctionId::from_raw(99), ts_file, "Button".to_string(), ts_span.clone(), Language::Tsx, false, true, 1, vec!["render".to_string()]));
        db.push_import(ImportFact::new(ImportId::from_raw(99), go_file, None, "context".to_string(), go_span.clone(), Language::Go));
        db.push_branch(BranchObligation::new(BranchId::from_raw(99), Some(go_function), go_file, go_span.clone(), "err != nil".to_string(), "true".to_string(), true, "branch".to_string()));
        db.push_test(TestFact::new(go_file, Some(go_function), "TestCharge".to_string(), go_span, vec!["err".to_string()], 1, 0, Vec::new(), 0));
        db.push_ts_component(TsComponentFact::new(ts_file, Some(ts_function), "Button".to_string(), ts_span.clone()));
        db.push_ts_class(TsClassFact::new(ts_file, "Dialog".to_string(), ts_span.clone(), true, true));
        db.push_string_literal(StringLiteralFact::new(ts_file, "Pay".to_string(), ts_span.clone(), Language::Tsx));
        db.push_jsx_attribute(JsxAttributeFact::new(ts_file, "aria-label".to_string(), Some("Pay".to_string()), ts_span));

        let packages = Packages::build(&db);
        let files = SourceFiles::build(&db);
        let functions = Functions::build(&db);
        let imports = Imports::build(&db);
        let branches = BranchObligations::build(&db);
        let tests = GoTests::build(&db);
        let components = TsComponents::build(&db);
        let classes = TsClasses::build(&db);
        let literals = StringLiterals::build(&db);
        let jsx = JsxAttributes::build(&db);

        assert_eq!(packages.all()[0].name, "payment");
        assert_eq!(branches.all()[0].condition_text, "err != nil");
        assert_eq!(files.get(go_file).unwrap().relative_path, "src/payment.go");
        assert_eq!(
            functions
                .for_file(go_file)
                .map(|function| function.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Charge"]
        );
        assert_eq!(
            imports
                .for_file(go_file)
                .map(|import| import.path.as_str())
                .collect::<Vec<_>>(),
            vec!["context"]
        );
        assert_eq!(branches.for_file(go_file).count(), 1);
        assert_eq!(
            tests
                .for_file(go_file)
                .map(|test| test.name.as_str())
                .collect::<Vec<_>>(),
            vec!["TestCharge"]
        );
        assert_eq!(
            components
                .for_file(ts_file)
                .map(|component| component.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Button"]
        );
        assert_eq!(
            classes
                .for_file(ts_file)
                .map(|class| class.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Dialog"]
        );
        assert_eq!(
            literals
                .for_file(ts_file)
                .map(|literal| literal.value.as_str())
                .collect::<Vec<_>>(),
            vec!["Pay"]
        );
        assert_eq!(
            jsx.for_file(ts_file)
                .map(|attribute| attribute.name.as_str())
                .collect::<Vec<_>>(),
            vec!["aria-label"]
        );
    }

    #[test]
    fn rule_ctx_import_edges_preserve_analysis_order() {
        let mut db = AnalysisDb::new();
        let first_file = db.add_file(
            PathBuf::from("src/first.go"),
            "src/first.go".to_string(),
            "package first\n".to_string(),
        );
        let second_file = db.add_file(
            PathBuf::from("src/second.go"),
            "src/second.go".to_string(),
            "package second\n".to_string(),
        );

        db.push_import(ImportFact::new(ImportId::from_raw(99), second_file, None, "fmt".to_string(), test_span(second_file, 1), Language::Go));
        db.push_import(ImportFact::new(ImportId::from_raw(99), first_file, None, "strings".to_string(), test_span(first_file, 1), Language::Go));

        let imports = Imports::build(&db);

        assert_eq!(
            imports
                .edges()
                .map(|(file, import)| (file.relative_path.as_str(), import.path.as_str()))
                .collect::<Vec<_>>(),
            vec![("src/second.go", "fmt"), ("src/first.go", "strings")]
        );
    }

    #[test]
    fn rule_ctx_go_tests_for_related_file_matches_companion_tests() {
        let mut db = AnalysisDb::new();
        let production_file = db.add_file(
            PathBuf::from("src/payments/payment.go"),
            "src/payments/payment.go".to_string(),
            "package payments\n".to_string(),
        );
        let companion_file = db.add_file(
            PathBuf::from("src/payments/payment_test.go"),
            "src/payments/payment_test.go".to_string(),
            "package payments\n".to_string(),
        );
        let unrelated_file = db.add_file(
            PathBuf::from("src/users/payment_test.go"),
            "src/users/payment_test.go".to_string(),
            "package users\n".to_string(),
        );

        db.push_test(TestFact::new(production_file, None, "TestInline".to_string(), test_span(production_file, 1), Vec::new(), 1, 0, Vec::new(), 0));
        db.push_test(TestFact::new(companion_file, None, "TestPayment".to_string(), test_span(companion_file, 1), Vec::new(), 1, 0, Vec::new(), 0));
        db.push_test(TestFact::new(unrelated_file, None, "TestUserPayment".to_string(), test_span(unrelated_file, 1), Vec::new(), 1, 0, Vec::new(), 0));

        let tests = GoTests::build(&db);

        assert_eq!(
            tests
                .related_for_file(production_file)
                .iter()
                .map(|test| test.name.as_str())
                .collect::<Vec<_>>(),
            vec!["TestInline", "TestPayment"]
        );
        assert_eq!(
            tests
                .related_for_file(companion_file)
                .iter()
                .map(|test| test.name.as_str())
                .collect::<Vec<_>>(),
            vec!["TestPayment"]
        );
    }

    #[test]
    fn capabilities_expose_ts_classes() {
        assert!(!Capabilities::new().ts_classes);
        let capabilities = Capabilities::new().ts_classes();
        assert!(capabilities.ts_classes);
    }

    fn diagnostic_range(
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> DiagnosticRange {
        DiagnosticRange::new(start_line, start_col, end_line, end_col)
    }

    #[test]
    fn line_col_counts_utf8_boundaries() {
        assert_eq!(line_col("a\nbc", 3), (2, 2));
    }

    #[test]
    fn registry_exposes_capability_declarations() {
        let mut registry = RuleRegistry::new();
        registry.register(
            TestRule::report("examples/capabilities", Severity::Warn, "capabilities")
                .with_capabilities(Capabilities::new().imports().coverage_facts())
                .into_rule(),
        );

        let capabilities = registry.rules()[0].capabilities();
        assert!(capabilities.imports);
        assert!(capabilities.coverage_facts);
        assert!(!capabilities.dataflow);
        assert!(!capabilities.jsx_attributes);
    }

    #[test]
    fn run_rules_filters_enabled_patterns_and_applies_severity_override() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::report("examples/allowed", Severity::Warn, "allowed").into_rule(),
            TestRule::report("custom/blocked", Severity::Error, "blocked").into_rule(),
        ];
        let mut options = BTreeMap::new();
        options.insert(
            "examples/allowed".to_string(),
            RuleOptions {
                severity: Some(Severity::Error),
                ..RuleOptions::default()
            },
        );
        let enabled = BTreeSet::from(["examples/*".to_string()]);

        let diagnostics = run_rules(&db, &rules, &options, Some(&enabled), false);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "examples/allowed");
        assert_eq!(diagnostics[0].severity, Severity::Error);
    }

    #[test]
    fn run_rules_none_selection_runs_all_and_empty_selection_runs_none() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::report("examples/one", Severity::Warn, "one").into_rule(),
            TestRule::report("examples/two", Severity::Warn, "two").into_rule(),
        ];

        let all = run_rules(&db, &rules, &BTreeMap::new(), None, false);
        assert_eq!(all.len(), 2);

        let empty = BTreeSet::new();
        let none = run_rules(&db, &rules, &BTreeMap::new(), Some(&empty), false);
        assert!(none.is_empty());
    }

    #[test]
    fn run_rules_contains_rule_errors_and_panics() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::error("examples/error").into_rule(),
            TestRule::panic("examples/panic").into_rule(),
        ];

        let diagnostics = run_rules(&db, &rules, &BTreeMap::new(), None, false);

        assert_eq!(
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.rule_id.as_str())
                .collect::<Vec<_>>(),
            vec!["internal/examples/error", "internal/examples/panic"]
        );
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.file == "<workspace>")
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("intentional rule error"))
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("rule panicked"))
        );
    }

    #[test]
    fn run_rules_contains_meta_panics() {
        let db = AnalysisDb::new();
        let rules = vec![TestRule::meta_panic().into_rule()];

        let diagnostics = run_rules(&db, &rules, &BTreeMap::new(), None, false);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "internal/unknown");
        assert_eq!(diagnostics[0].file, "<workspace>");
        assert!(diagnostics[0].message.contains("rule metadata panicked"));
    }

    #[test]
    fn run_rules_parallel_matches_sequential() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::report("examples/duplicate", Severity::Warn, "duplicate")
                .with_message("same diagnostic")
                .with_delay(Duration::from_millis(50))
                .into_rule(),
            TestRule::report("examples/duplicate", Severity::Error, "duplicate")
                .with_message("same diagnostic")
                .into_rule(),
        ];

        let sequential = run_rules(&db, &rules, &BTreeMap::new(), None, false);
        let parallel = run_rules(&db, &rules, &BTreeMap::new(), None, true);

        assert_eq!(parallel, sequential);
    }

    #[test]
    fn run_rules_dedupes_duplicate_fingerprints() {
        let db = AnalysisDb::new();
        let rules = vec![
            TestRule::report("examples/duplicate-a", Severity::Warn, "same-fingerprint")
                .into_rule(),
            TestRule::report("examples/duplicate-b", Severity::Error, "same-fingerprint")
                .into_rule(),
        ];

        let diagnostics = run_rules(&db, &rules, &BTreeMap::new(), None, false);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].stable_fingerprint, "same-fingerprint");
    }

    #[test]
    fn analysis_db_assigns_deterministic_ids_and_preserves_shared_source() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\n".to_string(),
        );
        let span = test_span(file, 1);

        let function = db.push_function(FunctionFact::new(FunctionId::from_raw(99), file, "main".to_string(), span.clone(), Language::Go, false, false, 1, Vec::new()));
        let import = db.push_import(ImportFact::new(ImportId::from_raw(99), file, None, "fmt".to_string(), span.clone(), Language::Go));
        let branch = db.push_branch(BranchObligation::new(BranchId::from_raw(99), Some(function), file, span, "err != nil".to_string(), "true".to_string(), true, "branch".to_string()));

        assert_eq!(file, FileId::from_raw(0));
        assert_eq!(function, FunctionId::from_raw(0));
        assert_eq!(import, ImportId::from_raw(0));
        assert_eq!(branch, BranchId::from_raw(0));

        let stored = db.file(file).expect("source file exists");
        let shared: Arc<str> = Arc::clone(&stored.source);
        assert_eq!(&*shared, "package main\n");
    }

    #[test]
    fn analysis_db_assigns_package_ids_deterministically() {
        let mut db = AnalysisDb::new();
        let first_file = db.add_file(
            PathBuf::from("payment.go"),
            "payment.go".to_string(),
            "package payment\n".to_string(),
        );
        let second_file = db.add_file(
            PathBuf::from("billing.go"),
            "billing.go".to_string(),
            "package billing\n".to_string(),
        );

        let first = db.push_package(PackageFact::new(PackageId::from_raw(99), first_file, "payment".to_string(), test_span(first_file, 1), Language::Go));
        let second = db.push_package(PackageFact::new(PackageId::from_raw(99), second_file, "billing".to_string(), test_span(second_file, 1), Language::Go));

        assert_eq!(first, PackageId::from_raw(0));
        assert_eq!(second, PackageId::from_raw(1));
        assert_eq!(db.packages()[0].id, PackageId::from_raw(0));
        assert_eq!(db.packages()[1].id, PackageId::from_raw(1));
    }

    #[test]
    fn analysis_db_exposes_package_facts() {
        let mut db = AnalysisDb::new();
        let first_file = db.add_file(
            PathBuf::from("payment.go"),
            "payment.go".to_string(),
            "package payment\n".to_string(),
        );
        let second_file = db.add_file(
            PathBuf::from("billing.go"),
            "billing.go".to_string(),
            "package billing\n".to_string(),
        );
        let first_span = test_span(first_file, 1);
        let second_span = test_span(second_file, 1);

        db.push_package(PackageFact::new(PackageId::from_raw(99), first_file, "payment".to_string(), first_span.clone(), Language::Go));
        db.push_package(PackageFact::new(PackageId::from_raw(99), second_file, "billing".to_string(), second_span.clone(), Language::Go));

        assert_eq!(db.packages().len(), 2);
        assert_eq!(db.packages()[0].file, first_file);
        assert_eq!(db.packages()[0].name, "payment");
        assert_eq!(db.packages()[0].span, first_span);
        assert_eq!(db.packages()[0].language, Language::Go);
        assert_eq!(db.packages()[1].file, second_file);
        assert_eq!(db.packages()[1].name, "billing");
        assert_eq!(db.packages()[1].span, second_span);
        assert_eq!(db.packages()[1].language, Language::Go);
    }

    #[test]
    fn semantic_index_storage_replaces_rows_and_rebuilds_metadata() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function handler() {}\n".to_string(),
        );
        let stale = test_scope("stale", file, SemanticStatus::Resolved);
        let beta = test_scope("bravo", file, SemanticStatus::Resolved);
        let alpha = test_scope("alpha", file, SemanticStatus::SetupMissing);

        db.replace_semantic_index_facts(
            vec![stale],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        db.replace_semantic_index_facts(
            vec![beta, alpha],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(
            db.scopes()
                .iter()
                .map(|scope| (scope.id.0, scope.scope_path.as_slice()))
                .collect::<Vec<_>>(),
            vec![
                (0, &["alpha".to_string()][..]),
                (1, &["bravo".to_string()][..]),
            ]
        );
        assert!(
            db.metadata_for(FactRef::new(FactFamily::Scope, 0))
                .is_some()
        );
        assert!(
            db.metadata_for(FactRef::new(FactFamily::Scope, 2))
                .is_none()
        );
    }

    #[test]
    fn semantic_index_storage_reports_missing_metadata_when_refresh_is_bypassed() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function handler() {}\n".to_string(),
        );

        db.replace_semantic_index_facts(vec![test_scope("root", file, SemanticStatus::Resolved)],
            Vec::<SemanticImportFact>::new(),
            Vec::<ExportFact>::new(),
            Vec::<AliasFact>::new(),
            Vec::<ResolutionFact>::new(),
            Vec::<GeneratedSymbolFact>::new(),
            Vec::<StableExportIdentity>::new(),
        );
        db.remove_fact_metadata_for_test(FactRef::new(FactFamily::Scope, 0));

        assert_eq!(
            db.missing_fact_metadata(),
            vec![MissingFactMeta {
                family: FactFamily::Scope,
                run_id: 0,
            }]
        );
    }

