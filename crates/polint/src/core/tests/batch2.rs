    mod summary_fact_metadata {
        use super::*;
        use crate::analysis::ids::{SummaryEventId, SummaryId};
        use crate::analysis::summaries::facts::{
            SummaryDomainKind, SummaryPrecision, SummaryProvenance, SummaryStatus,
        };

        #[test]
        fn fast_summary_fact_digest_matches_generic_metadata_digest() {
            let interner = crate::core::test_stable_key_interner();
            let fact = SummaryFact {
                id: SummaryId(0),
                callable_stable_key: interner.intern("callable\\key"),
                function: FunctionId(1),
                domain: SummaryDomainKind::CallEffects,
                status: SummaryStatus::Present,
                precision: SummaryPrecision::SetupAware,
                provenance: SummaryProvenance::InterproceduralClosure,
                payload_digest: "payload\\digest".to_string(),
                tito_flows: Vec::new(),
                stable_key: interner.intern("summary:key"),
            };

            let callable = interner.resolve(fact.callable_stable_key).to_string();
            let stable_key = interner.resolve(fact.stable_key).to_string();
            let generic = metadata_payload_digest(
                &stable_key,
                &stable_parts([
                    ("status", fact.status.as_str().to_string()),
                    ("precision", fact.precision.as_str().to_string()),
                    ("domain", fact.domain.as_str().to_string()),
                    ("callable", callable),
                    ("provenance", fact.provenance.as_str().to_string()),
                    ("payload_digest", fact.payload_digest.clone()),
                ]),
            );

            assert_eq!(summary_fact_payload_metadata_digest(&interner, &fact), generic);
        }

        #[test]
        fn fast_summary_event_digest_matches_generic_metadata_digest() {
            let interner = crate::core::test_stable_key_interner();
            let fact = SummaryEventFact {
                id: SummaryEventId(0),
                callable_stable_key: interner.intern("callable\\key"),
                function: FunctionId(1),
                domain: SummaryDomainKind::CallEffects,
                event_kind: "unresolved\\callee".to_string(),
                reason: "dynamic\\target".to_string(),
                status: SummaryStatus::Unknown,
                precision: SummaryPrecision::UnknownTop,
                stable_key: interner.intern("summary:event:key"),
            };

            let callable = interner.resolve(fact.callable_stable_key).to_string();
            let stable_key = interner.resolve(fact.stable_key).to_string();
            let generic = metadata_payload_digest(
                &stable_key,
                &stable_parts([
                    ("status", fact.status.as_str().to_string()),
                    ("precision", fact.precision.as_str().to_string()),
                    ("domain", fact.domain.as_str().to_string()),
                    ("callable", callable),
                    ("event_kind", fact.event_kind.clone()),
                    ("reason", fact.reason.clone()),
                ]),
            );

            assert_eq!(summary_event_payload_metadata_digest(&interner, &fact), generic);
        }

        #[test]
        fn lower_hex_u64_is_zero_padded_and_lowercase() {
            assert_eq!(lower_hex_u64(0), "0000000000000000");
            assert_eq!(lower_hex_u64(0x0123_4567_89ab_cdef), "0123456789abcdef");
        }
    }

    mod data_flow_fact_metadata {
        use super::*;
        use crate::analysis::data_flow::facts::{
            DataFlowModelKind, DataFlowNodeKind, DataFlowProvenance,
        };

        #[test]
        fn model_backed_node_metadata_uses_model_precision_and_payload() {
            let mut db = AnalysisDb::new();

            db.replace_data_flow_facts(data_flow_output_with_model("model:source:first"))
                .expect("first data-flow replace");
            let first_metadata = db
                .metadata_for(FactRef::new(FactFamily::DataFlowNode, 0))
                .expect("node metadata")
                .clone();

            db.replace_data_flow_facts(data_flow_output_with_model("model:source:second"))
                .expect("second data-flow replace");
            let second_metadata = db
                .metadata_for(FactRef::new(FactFamily::DataFlowNode, 0))
                .expect("node metadata")
                .clone();

            assert_eq!(first_metadata.precision, FactPrecision::SetupAware);
            assert_eq!(
                first_metadata.validation,
                ValidationStatus::ReferentiallyValidated
            );
            assert_ne!(
                first_metadata.payload_digest,
                second_metadata.payload_digest
            );
        }

        fn data_flow_output_with_model(model_key: &str) -> DataFlowOutput {
            DataFlowOutput {
                nodes: vec![DataFlowNodeFact {
                    id: crate::analysis::ids::DataFlowNodeId(10),
                    kind: DataFlowNodeKind::Source,
                    language: Language::TypeScript,
                    file: None,
                    function: None,
                    body: None,
                    operation: None,
                    cfg_node: None,
                    place: None,
                    symbol: None,
                    reference: None,
                    call_site: None,
                    model: Some(crate::analysis::ids::DataFlowModelId(20)),
                    span: None,
                    stable_key: crate::core::stable_key_for_test("node:source"),
                }],
                edges: Vec::new(),
                models: vec![DataFlowModelFact {
                    id: crate::analysis::ids::DataFlowModelId(20),
                    kind: DataFlowModelKind::Source,
                    language: Language::TypeScript,
                    provider_id: "test".to_string(),
                    model_id: None,
                    source_stable_key: None,
                    status: DataFlowStatus::Present,
                    precision: DataFlowPrecision::SetupAware,
                    validation: DataFlowValidation::ReferentiallyValidated,
                    confidence: DataFlowConfidence::High,
                    provenance: DataFlowProvenance::Native,
                    evidence: Vec::new(),
                    payload_labels: Vec::new(),
                    stable_key: crate::core::stable_key_for_test(model_key),
                }],
                budgets: Vec::new(),
            }
        }
    }

    mod type_value_alias_metadata {
        use super::*;
        use crate::analysis::ids::{AbstractValueId, ValueFactId};
        use crate::analysis::values::facts::{
            ValueKind, ValuePrecision, ValueProvenance, ValueStatus, ValueSubject,
        };
        use crate::analysis::values::store::ValueOutput;

        #[test]
        fn exact_local_value_metadata_stays_within_setup_aware_provider_ceiling() {
            let mut db = AnalysisDb::new();
            db.replace_type_value_alias_facts(TypeValueAliasOutput {
                values: ValueOutput {
                    values: vec![ValueFact {
                        id: ValueFactId(0),
                        subject: ValueSubject::Synthetic("literal".to_string()),
                        value: AbstractValueId(0),
                        kind: ValueKind::String("\"ok\"".to_string()),
                        language: Language::TypeScript,
                        file: None,
                        function: None,
                        body: None,
                        precision: ValuePrecision::ExactLocal,
                        status: ValueStatus::Present,
                        provenance: ValueProvenance::Native,
                        stable_key: crate::core::stable_key_for_test("value:literal"),
                    }],
                    allocations: Vec::new(),
                },
                ..TypeValueAliasOutput::default()
            });

            let metadata = db
                .metadata_for(FactRef::new(FactFamily::Value, 0))
                .expect("value metadata exists");

            assert_eq!(metadata.precision, FactPrecision::SetupAware);
        }
    }

    mod semantic_mir_storage {
        use super::*;

        #[test]
        fn replace_semantic_mir_removes_stale_rows_from_prior_run() {
            let mut db = AnalysisDb::new();
            let interner = db.stable_key_interner();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let first = MirOutput {
                bodies: vec![test_mir_body(&interner, 9, file, "body:first")],
                places: vec![test_place(&interner, 9, file, "place:first")],
                operations: vec![test_mir_operation(&interner,
                    9,
                    MirBodyId(9),
                    PlaceId(9),
                    PlaceId(9),
                    "op:first",
                )],
                unsupported: vec![test_unsupported(&interner, "unsupported:first")],
                ..MirOutput::default()
};
            let second = MirOutput {
                bodies: vec![test_mir_body(&interner, 4, file, "body:second")],
                places: vec![test_place(&interner, 4, file, "place:second")],
                operations: vec![test_mir_operation(&interner,
                    4,
                    MirBodyId(4),
                    PlaceId(4),
                    PlaceId(4),
                    "op:second",
                )],
                unsupported: Vec::new(),
                ..MirOutput::default()
};

            db.replace_semantic_mir(first).expect("first MIR replace");
            db.replace_semantic_mir(second).expect("second MIR replace");

            assert_eq!(
                db.mir_bodies()
                    .iter()
                    .map(|body| (body.id, interner.resolve(body.stable_key).to_string()))
                    .collect::<Vec<_>>(),
                vec![(MirBodyId(0), "body:second".to_string())]
            );
            assert_eq!(interner.resolve(db.mir_operations()[0].stable_key).as_ref(), "op:second");
            assert_eq!(interner.resolve(db.mir_places()[0].stable_key).as_ref(), "place:second");
            assert!(db.unsupported_semantics().is_empty());
        }

        #[test]
        fn replace_semantic_mir_reassigns_ids_by_stable_key_order() {
            let mut db = AnalysisDb::new();
            let interner = db.stable_key_interner();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let output = MirOutput {
                bodies: vec![
                    test_mir_body(&interner, 20, file, "body:z"),
                    test_mir_body(&interner, 10, file, "body:a"),
                ],
                places: vec![
                    test_place(&interner, 20, file, "place:z"),
                    test_place(&interner, 10, file, "place:a"),
                ],
                operations: vec![
                    test_mir_operation(&interner, 20, MirBodyId(20), PlaceId(20), PlaceId(10), "op:z"),
                    test_mir_operation(&interner, 10, MirBodyId(10), PlaceId(10), PlaceId(20), "op:a"),
                ],
                unsupported: vec![
                    test_unsupported(&interner, "unsupported:z"),
                    test_unsupported(&interner, "unsupported:a"),
                ],
                ..MirOutput::default()
};

            db.replace_semantic_mir(output)
                .expect("semantic MIR replace");

            assert_eq!(
                db.mir_bodies()
                    .iter()
                    .map(|body| (body.id, interner.resolve(body.stable_key).to_string()))
                    .collect::<Vec<_>>(),
                vec![(MirBodyId(0), "body:a".to_string()), (MirBodyId(1), "body:z".to_string())]
            );
            assert_eq!(
                db.mir_operations()
                    .iter()
                    .map(|operation| (operation.id, operation.body, interner.resolve(operation.stable_key).to_string()))
                    .collect::<Vec<_>>(),
                vec![
                    (MirOpId(0), MirBodyId(0), "op:a".to_string()),
                    (MirOpId(1), MirBodyId(1), "op:z".to_string()),
                ]
            );
            assert_eq!(
                db.mir_places()
                    .iter()
                    .map(|place| (place.id, interner.resolve(place.stable_key).to_string()))
                    .collect::<Vec<_>>(),
                vec![(PlaceId(0), "place:a".to_string()), (PlaceId(1), "place:z".to_string())]
            );
            assert_eq!(
                db.unsupported_semantics()
                    .iter()
                    .map(|row| (row.id, interner.resolve(row.stable_key).to_string()))
                    .collect::<Vec<_>>(),
                vec![
                    (UnsupportedId(0), "unsupported:a".to_string()),
                    (UnsupportedId(1), "unsupported:z".to_string()),
                ]
            );
            let store = db.semantic_store().expect("semantic store exists");
            assert_eq!(
                store
                    .mir_body(MirBodyId(1))
                    .map(|body| interner.resolve(body.stable_key).to_string()),
                Some("body:z".to_string())
            );
            assert_eq!(
                store
                    .mir_operation(MirOpId(0))
                    .map(|operation| interner.resolve(operation.stable_key).to_string()),
                Some("op:a".to_string())
            );
            assert_eq!(
                store
                    .place(PlaceId(0))
                    .map(|place| interner.resolve(place.stable_key).to_string()),
                Some("place:a".to_string())
            );
            assert_eq!(
                store
                    .unsupported_semantic(UnsupportedId(1))
                    .map(|row| interner.resolve(row.stable_key).to_string()),
                Some("unsupported:z".to_string())
            );
        }

        #[test]
        fn replace_semantic_mir_rejects_dangling_operation_references() {
            let mut db = AnalysisDb::new();
            let interner = db.stable_key_interner();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let output = MirOutput {
                bodies: vec![test_mir_body(&interner, 0, file, "body:a")],
                places: vec![test_place(&interner, 0, file, "place:a")],
                operations: vec![test_mir_operation(&interner,
                    0,
                    MirBodyId(99),
                    PlaceId(0),
                    PlaceId(0),
                    "op:dangling",
                )],
                unsupported: Vec::new(),
                ..MirOutput::default()
};

            let error = db
                .replace_semantic_mir(output)
                .expect_err("dangling MIR body reference should fail");

            assert!(error.to_string().contains("dangling MIR operation body"));
        }
    }

    mod semantic_mir_metadata {
        use super::*;

        fn replace_with_semantic_rows(db: &mut AnalysisDb, file: FileId) {
            let interner = db.stable_key_interner();
            db.replace_semantic_mir(MirOutput {
                bodies: vec![test_mir_body(&interner, 2, file, "body:metadata")],
                places: vec![test_place(&interner, 2, file, "place:metadata")],
                operations: vec![test_mir_operation(&interner,
                    2,
                    MirBodyId(2),
                    PlaceId(2),
                    PlaceId(2),
                    "op:metadata",
                )],
                unsupported: vec![test_unsupported(&interner, "unsupported:metadata")],
                ..MirOutput::default()
})
            .expect("semantic MIR replace");
        }

        #[test]
        fn replace_semantic_mir_records_metadata_for_every_stored_row() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );

            replace_with_semantic_rows(&mut db, file);

            for family in [
                FactFamily::MirBody,
                FactFamily::MirOperation,
                FactFamily::Place,
                FactFamily::UnsupportedSemantic,
            ] {
                let metadata = db
                    .metadata_for(FactRef::new(family, 0))
                    .expect("semantic MIR metadata exists");
                assert_eq!(metadata.producer_id, "polint.semantic_mir");
                assert_eq!(metadata.layer_id, "polint.semantic_mir");
                assert_eq!(metadata.validation, ValidationStatus::NativeTrusted);
                assert_ne!(metadata.precision, FactPrecision::Exact);
            }
        }

        #[test]
        fn semantic_mir_missing_metadata_reports_rows_when_refresh_is_bypassed() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );

            replace_with_semantic_rows(&mut db, file);
            for family in [
                FactFamily::MirBody,
                FactFamily::MirOperation,
                FactFamily::Place,
                FactFamily::UnsupportedSemantic,
            ] {
                db.remove_fact_metadata_for_test(FactRef::new(family, 0));
            }

            assert_eq!(
                db.missing_fact_metadata(),
                vec![
                    MissingFactMeta {
                        family: FactFamily::MirBody,
                        run_id: 0,
                    },
                    MissingFactMeta {
                        family: FactFamily::MirOperation,
                        run_id: 0,
                    },
                    MissingFactMeta {
                        family: FactFamily::Place,
                        run_id: 0,
                    },
                    MissingFactMeta {
                        family: FactFamily::UnsupportedSemantic,
                        run_id: 0,
                    },
                ]
            );
        }

        #[test]
        fn semantic_mir_metadata_maps_unknown_and_unsupported_to_low_precision() {
            let mut db = AnalysisDb::new();
            let interner = db.stable_key_interner();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let mut body = test_mir_body(&interner, 1, file, "body:unknown");
            body.status = MirStatus::Unknown;
            let mut place = test_place(&interner, 1, file, "place:partial");
            place.status = PlaceStatus::Partial;

            db.replace_semantic_mir(MirOutput {
                bodies: vec![body],
                places: vec![place],
                operations: Vec::new(),
                unsupported: vec![test_unsupported(&interner, "unsupported:metadata")],
                ..MirOutput::default()
})
            .expect("semantic MIR replace");

            assert_eq!(
                db.metadata_for(FactRef::new(FactFamily::MirBody, 0))
                    .map(|metadata| (metadata.precision, metadata.confidence)),
                Some((FactPrecision::Unresolved, FactConfidence::Low))
            );
            assert_eq!(
                db.metadata_for(FactRef::new(FactFamily::Place, 0))
                    .map(|metadata| (metadata.precision, metadata.confidence)),
                Some((FactPrecision::Heuristic, FactConfidence::Medium))
            );
            assert_eq!(
                db.metadata_for(FactRef::new(FactFamily::UnsupportedSemantic, 0))
                    .map(|metadata| (metadata.precision, metadata.confidence)),
                Some((FactPrecision::Unsupported, FactConfidence::Low))
            );
        }
    }

    mod semantic_mir_storage_public_boundary {
        use std::fs;
        use std::path::Path;

        const FORBIDDEN_PUBLIC_TOKENS: &[&str] = &[
            "MirBody",
            "MirOperation",
            "PlaceFact",
            "SemanticStore",
            "UnsupportedSemanticFact",
            "polint.semantic_mir",
            "semantic-mir-facts",
        ];

        fn assert_no_forbidden_tokens(label: &str, source: &str) {
            for token in FORBIDDEN_PUBLIC_TOKENS {
                assert!(
                    !source.contains(token),
                    "{label} leaked private semantic MIR token `{token}`"
                );
            }
        }

        #[test]
        fn sdk_runner_and_bench_sources_do_not_leak_semantic_mir_storage() {
            let sources = [
                ("sdk/mod.rs", include_str!("../../sdk/mod.rs")),
                ("sdk/facts.rs", include_str!("../../sdk/facts.rs")),
                ("runner/mod.rs", include_str!("../../runner/mod.rs")),
                ("lib.rs", include_str!("../../lib.rs")),
            ];

            for (label, source) in sources {
                assert_no_forbidden_tokens(label, source);
            }
        }

        #[test]
        fn crate_root_keeps_analysis_module_crate_private_and_out_of_bench() {
            let lib = include_str!("../../lib.rs");
            assert!(lib.contains("pub(crate) mod analysis;"));

            let bench_surface = lib.split("pub mod _bench").nth(1).unwrap_or_default();
            assert!(!bench_surface.contains("pub mod analysis"));
            assert!(!bench_surface.contains("pub use crate::analysis"));
            assert_no_forbidden_tokens("_bench", bench_surface);
        }

        #[test]
        fn docs_and_readme_do_not_advertise_private_semantic_mir_facts() {
            let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
            let repo_root = manifest_dir
                .parent()
                .and_then(Path::parent)
                .expect("workspace root");
            let docs_root = repo_root.join("docs/facts");
            let mut sources = vec![(
                "README.md".to_string(),
                fs::read_to_string(repo_root.join("README.md")).expect("README.md"),
            )];

            for entry in fs::read_dir(&docs_root).expect("docs/facts exists") {
                let entry = entry.expect("docs/facts entry");
                if entry.file_type().expect("docs/facts file type").is_file() {
                    sources.push((
                        entry.path().display().to_string(),
                        fs::read_to_string(entry.path()).expect("docs/facts source"),
                    ));
                }
            }

            for (label, source) in sources {
                assert_no_forbidden_tokens(&label, &source);
            }
        }
    }

    fn topology_output(prefix: &str) -> crate::module_graph::topology::TopologyOutput {
        use crate::module_graph::topology::{
            DependencyRequirementFact, DependencyRequirementId, ImportContextKind,
            ImportToPackageFact, ImportToPackageId, ImportToPackageStatus, RepoTopologyOverlayFact,
            RepoTopologyOverlayId, RepoTopologyOverlayKind, ResolvedDependencyEdgeFact,
            ResolvedDependencyEdgeId, ResolvedDependencyKind, SourceSetFact, SourceSetId,
            SourceSetKind, TopologyOutput, TopologyPackageFact, TopologyPackageId,
            TopologyPackageKind, TopologyPrecision, TopologyStatus, WorkspaceRootFact,
            WorkspaceRootId, WorkspaceRootKind,
        };

        TopologyOutput {
            workspace_roots: vec![WorkspaceRootFact {
                id: WorkspaceRootId(99),
                kind: WorkspaceRootKind::Repository,
                root_path: ".".to_string(),
                manifest_path: None,
                language: None,
                stable_key: stable_key_for_test(&format!("{prefix}:root")),
                producer_id: "test",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Present,
            }],
            packages: vec![TopologyPackageFact {
                id: TopologyPackageId(99),
                workspace_root: Some(WorkspaceRootId(99)),
                package: None,
                module_node: None,
                kind: TopologyPackageKind::Workspace,
                name: format!("{prefix}-package"),
                version: None,
                path: ".".to_string(),
                language: Some(Language::TypeScript),
                stable_key: stable_key_for_test(&format!("{prefix}:package")),
                producer_id: "test",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Present,
            }],
            source_sets: vec![SourceSetFact {
                id: SourceSetId(99),
                package: Some(TopologyPackageId(99)),
                root: Some(WorkspaceRootId(99)),
                kind: SourceSetKind::Source,
                path: "src".to_string(),
                language: Some(Language::TypeScript),
                files: vec![FileId(0)],
                stable_key: stable_key_for_test(&format!("{prefix}:source-set")),
                producer_id: "test",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Present,
            }],
            dependency_requirements: vec![DependencyRequirementFact {
                id: DependencyRequirementId(99),
                from_package: Some(TopologyPackageId(99)),
                target_package: None,
                target_name: "react".to_string(),
                version_requirement: Some("^18".to_string()),
                kind: crate::module_graph::topology::RequirementKind::Runtime,
                manifest_path: Some("package.json".to_string()),
                stable_key: stable_key_for_test(&format!("{prefix}:requirement")),
                producer_id: "test",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Present,
            }],
            resolved_dependency_edges: vec![ResolvedDependencyEdgeFact {
                id: ResolvedDependencyEdgeId(99),
                requirement: Some(DependencyRequirementId(99)),
                from_package: Some(TopologyPackageId(99)),
                to_package: None,
                package_name: "react".to_string(),
                resolved_version: Some("18.2.0".to_string()),
                kind: ResolvedDependencyKind::Lockfile,
                stable_key: stable_key_for_test(&format!("{prefix}:resolved")),
                producer_id: "test",
                precision: TopologyPrecision::ExactLockfile,
                status: TopologyStatus::Resolved,
            }],
            import_to_package_edges: vec![ImportToPackageFact {
                id: ImportToPackageId(99),
                syntax_import: None,
                resolved_import: None,
                semantic_import_stable_key: None,
                from_file: Some(FileId(0)),
                from_package: Some(TopologyPackageId(99)),
                to_package: None,
                target_node: None,
                from_package_stable_key: Some(stable_key_for_test(&format!(
                    "{prefix}:package"
                ))),
                to_package_stable_key: None,
                source_set_stable_key: Some(stable_key_for_test(&format!(
                    "{prefix}:source-set"
                ))),
                import_path: "react".to_string(),
                context: ImportContextKind::Source,
                stable_key: stable_key_for_test(&format!("{prefix}:import-to-package")),
                producer_id: "test",
                precision: TopologyPrecision::ExactStatic,
                status: ImportToPackageStatus::Resolved,
            }],
            overlays: vec![RepoTopologyOverlayFact {
                id: RepoTopologyOverlayId(99),
                root: Some(WorkspaceRootId(99)),
                package: Some(TopologyPackageId(99)),
                source_set: Some(SourceSetId(99)),
                kind: RepoTopologyOverlayKind::OwnershipZone,
                label: "team-platform".to_string(),
                path: Some("src".to_string()),
                stable_key: stable_key_for_test(&format!("{prefix}:overlay")),
                producer_id: "test",
                precision: TopologyPrecision::Heuristic,
                status: TopologyStatus::Present,
            }],
        }
    }

