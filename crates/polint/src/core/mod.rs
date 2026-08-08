#[cfg(test)]
use crate::analysis::data_flow::facts::{
    DataFlowConfidence, DataFlowModelFact, DataFlowNodeFact, DataFlowPrecision, DataFlowStatus,
    DataFlowValidation,
};
#[cfg(test)]
use crate::analysis::evidence::facts::{EvidenceNodeFact, EvidencePrecision};

#[cfg(test)]
use crate::analysis::data_flow::store::DataFlowOutput;
#[cfg(test)]
use crate::analysis::extensions::store::{
    AcceptedExtensionFact, ExtensionActivationRow, ExtensionOutput, RejectedExtensionFact,
};
#[cfg(test)]
use crate::analysis::solver::budget::BudgetStatus;
#[cfg(test)]
use crate::analysis::solver::store::SolverOutput;
#[cfg(test)]
use crate::analysis::types::store::TypeValueAliasOutput;
#[cfg(test)]
use std::collections::{BTreeMap, BTreeSet};
#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
use crate::analysis_kernel::MissingFactMeta;
#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
use crate::analysis::summaries::facts::{SummaryEventFact, SummaryFact};
#[cfg(test)]
use crate::analysis::values::facts::ValueFact;

pub(super) const SOURCE_PROVIDER_ID: &str = "polint.source";
pub(super) const GO_SYNTAX_PROVIDER_ID: &str = "polint.go.syntax";
pub(super) const TS_SYNTAX_PROVIDER_ID: &str = "polint.ts.syntax";
pub(super) const MODULE_GRAPH_PROVIDER_ID: &str = "polint.module_graph";
pub(super) const MODULE_TOPOLOGY_PROVIDER_ID: &str = "polint.module_topology";
pub(super) const SYMBOL_GRAPH_PROVIDER_ID: &str = "polint.symbol_graph";
pub(crate) const SEMANTIC_MIR_PROVIDER_ID: &str = "polint.semantic_mir";
pub(crate) const CFG_PROVIDER_ID: &str = "polint.cfg";
pub(crate) const CALLS_PROVIDER_ID: &str = "polint.calls";
pub(crate) const POLINT_ABSTRACT_DOMAINS_PROVIDER_ID: &str = "polint.abstract_domains";
pub(crate) const POLINT_DIRECT_SUMMARIES_PROVIDER_ID: &str = "polint.direct_summaries";
pub(crate) const ENTRYPOINTS_PROVIDER_ID: &str = "polint.entrypoints";
pub(super) const METRICS_PROVIDER_ID: &str = "polint.metrics";
pub(super) const FUNCTION_SIZE_METRIC_NAME: &str = "function_size";
pub(super) const CYCLOMATIC_COMPLEXITY_METRIC_NAME: &str = "cyclomatic_complexity";

#[cfg(test)]
use metadata::*;

mod capability;
mod db;
mod facts;
mod ids;
mod labels;
mod lang;
mod metadata;
mod review;
pub(crate) mod rule;
mod span;

pub use facts::{
    BranchObligation, ComplexityMetricFact, CoverageFact, DefinitionFact, DefinitionKind,
    FileMetricFact, FunctionFact, FunctionMetricFact, ImportFact, JsxAttributeFact, ModuleEdge,
    ModuleEdgeKind, ModuleNode, ModuleNodeKind, PackageFact, ReferenceFact, ReferenceKind,
    ResolutionPrecision, ResolutionStatus, ResolvedImportFact, SourceFile, StringLiteralFact,
    SymbolFact, SymbolKind, SymbolNamespace, SymbolPrecision, SymbolResolutionStatus, TestFact,
    TsClassFact, TsComponentFact, UnresolvedReason,
};
pub(crate) use facts::{
    CachedFileAnalysis, CachedFileFacts, TS_JS_MODULE_FUNCTION_NAME,
    is_synthetic_ts_js_module_function,
};
pub use ids::{
    BranchId, DefinitionId, FileId, FunctionId, ImportId, ModuleEdgeId, ModuleNodeId, NodeId,
    PackageId, ReferenceId, ResolvedImportId, RuleId, SymbolId,
};
pub use lang::Language;
pub use span::{Span, TextRange};

pub use review::ChangeStatus;
pub(crate) use review::{ChangedFile, ReviewChangeset};

pub use capability::{
    Capabilities, CapabilitySupport, CapabilitySupportStatus, CapabilitySupportView,
};
pub use db::AnalysisDb;
#[cfg(test)]
pub(crate) use rule::RuleRegistry;
pub use rule::{Rule, RuleConfigValue, RuleCtx, RuleKind, RuleMeta, RuleOptions};
pub(crate) use rule::{
    rule_id_matches, run_rules, run_rules_with_capability_support, span_from_byte_range,
};

#[cfg(test)]
mod tests {
    include!("tests/batch1.rs");

    mod summary_fact_metadata {
        use super::*;
        use crate::analysis::ids::{SummaryEventId, SummaryId};
        use crate::analysis::summaries::facts::{
            SummaryDomainKind, SummaryPrecision, SummaryProvenance, SummaryStatus,
        };

        #[test]
        fn fast_summary_fact_digest_matches_generic_metadata_digest() {
            let fact = SummaryFact {
                id: SummaryId(0),
                callable_stable_key: "callable\\key".to_string(),
                function: FunctionId(1),
                domain: SummaryDomainKind::CallEffects,
                status: SummaryStatus::Present,
                precision: SummaryPrecision::SetupAware,
                provenance: SummaryProvenance::InterproceduralClosure,
                payload_digest: "payload\\digest".to_string(),
                tito_flows: Vec::new(),
                stable_key: "summary:key".to_string(),
            };

            let generic = metadata_payload_digest(
                &fact.stable_key,
                &stable_parts([
                    ("status", fact.status.as_str().to_string()),
                    ("precision", fact.precision.as_str().to_string()),
                    ("domain", fact.domain.as_str().to_string()),
                    ("callable", fact.callable_stable_key.clone()),
                    ("provenance", fact.provenance.as_str().to_string()),
                    ("payload_digest", fact.payload_digest.clone()),
                ]),
            );

            assert_eq!(summary_fact_payload_metadata_digest(&fact), generic);
        }

        #[test]
        fn fast_summary_event_digest_matches_generic_metadata_digest() {
            let fact = SummaryEventFact {
                id: SummaryEventId(0),
                callable_stable_key: "callable\\key".to_string(),
                function: FunctionId(1),
                domain: SummaryDomainKind::CallEffects,
                event_kind: "unresolved\\callee".to_string(),
                reason: "dynamic\\target".to_string(),
                status: SummaryStatus::Unknown,
                precision: SummaryPrecision::UnknownTop,
                stable_key: "summary:event:key".to_string(),
            };

            let generic = metadata_payload_digest(
                &fact.stable_key,
                &stable_parts([
                    ("status", fact.status.as_str().to_string()),
                    ("precision", fact.precision.as_str().to_string()),
                    ("domain", fact.domain.as_str().to_string()),
                    ("callable", fact.callable_stable_key.clone()),
                    ("event_kind", fact.event_kind.clone()),
                    ("reason", fact.reason.clone()),
                ]),
            );

            assert_eq!(summary_event_payload_metadata_digest(&fact), generic);
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
                    stable_key: "node:source".to_string(),
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
                    stable_key: model_key.to_string(),
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
                        stable_key: "value:literal".to_string(),
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
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let first = MirOutput {
                bodies: vec![test_mir_body(9, file, "body:first")],
                places: vec![test_place(9, file, "place:first")],
                operations: vec![test_mir_operation(
                    9,
                    MirBodyId(9),
                    PlaceId(9),
                    PlaceId(9),
                    "op:first",
                )],
                unsupported: vec![test_unsupported("unsupported:first")],
            };
            let second = MirOutput {
                bodies: vec![test_mir_body(4, file, "body:second")],
                places: vec![test_place(4, file, "place:second")],
                operations: vec![test_mir_operation(
                    4,
                    MirBodyId(4),
                    PlaceId(4),
                    PlaceId(4),
                    "op:second",
                )],
                unsupported: Vec::new(),
            };

            db.replace_semantic_mir(first).expect("first MIR replace");
            db.replace_semantic_mir(second).expect("second MIR replace");

            assert_eq!(
                db.mir_bodies()
                    .iter()
                    .map(|body| (body.id, body.stable_key.as_str()))
                    .collect::<Vec<_>>(),
                vec![(MirBodyId(0), "body:second")]
            );
            assert_eq!(db.mir_operations()[0].stable_key, "op:second");
            assert_eq!(db.mir_places()[0].stable_key, "place:second");
            assert!(db.unsupported_semantics().is_empty());
        }

        #[test]
        fn replace_semantic_mir_reassigns_ids_by_stable_key_order() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let output = MirOutput {
                bodies: vec![
                    test_mir_body(20, file, "body:z"),
                    test_mir_body(10, file, "body:a"),
                ],
                places: vec![
                    test_place(20, file, "place:z"),
                    test_place(10, file, "place:a"),
                ],
                operations: vec![
                    test_mir_operation(20, MirBodyId(20), PlaceId(20), PlaceId(10), "op:z"),
                    test_mir_operation(10, MirBodyId(10), PlaceId(10), PlaceId(20), "op:a"),
                ],
                unsupported: vec![
                    test_unsupported("unsupported:z"),
                    test_unsupported("unsupported:a"),
                ],
            };

            db.replace_semantic_mir(output)
                .expect("semantic MIR replace");

            assert_eq!(
                db.mir_bodies()
                    .iter()
                    .map(|body| (body.id, body.stable_key.as_str()))
                    .collect::<Vec<_>>(),
                vec![(MirBodyId(0), "body:a"), (MirBodyId(1), "body:z")]
            );
            assert_eq!(
                db.mir_operations()
                    .iter()
                    .map(|operation| (operation.id, operation.body, operation.stable_key.as_str()))
                    .collect::<Vec<_>>(),
                vec![
                    (MirOpId(0), MirBodyId(0), "op:a"),
                    (MirOpId(1), MirBodyId(1), "op:z"),
                ]
            );
            assert_eq!(
                db.mir_places()
                    .iter()
                    .map(|place| (place.id, place.stable_key.as_str()))
                    .collect::<Vec<_>>(),
                vec![(PlaceId(0), "place:a"), (PlaceId(1), "place:z")]
            );
            assert_eq!(
                db.unsupported_semantics()
                    .iter()
                    .map(|row| (row.id, row.stable_key.as_str()))
                    .collect::<Vec<_>>(),
                vec![
                    (UnsupportedId(0), "unsupported:a"),
                    (UnsupportedId(1), "unsupported:z"),
                ]
            );
            let store = db.semantic_store().expect("semantic store exists");
            assert_eq!(
                store
                    .mir_body(MirBodyId(1))
                    .map(|body| body.stable_key.as_str()),
                Some("body:z")
            );
            assert_eq!(
                store
                    .mir_operation(MirOpId(0))
                    .map(|operation| operation.stable_key.as_str()),
                Some("op:a")
            );
            assert_eq!(
                store
                    .place(PlaceId(0))
                    .map(|place| place.stable_key.as_str()),
                Some("place:a")
            );
            assert_eq!(
                store
                    .unsupported_semantic(UnsupportedId(1))
                    .map(|row| row.stable_key.as_str()),
                Some("unsupported:z")
            );
        }

        #[test]
        fn replace_semantic_mir_rejects_dangling_operation_references() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let output = MirOutput {
                bodies: vec![test_mir_body(0, file, "body:a")],
                places: vec![test_place(0, file, "place:a")],
                operations: vec![test_mir_operation(
                    0,
                    MirBodyId(99),
                    PlaceId(0),
                    PlaceId(0),
                    "op:dangling",
                )],
                unsupported: Vec::new(),
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
            db.replace_semantic_mir(MirOutput {
                bodies: vec![test_mir_body(2, file, "body:metadata")],
                places: vec![test_place(2, file, "place:metadata")],
                operations: vec![test_mir_operation(
                    2,
                    MirBodyId(2),
                    PlaceId(2),
                    PlaceId(2),
                    "op:metadata",
                )],
                unsupported: vec![test_unsupported("unsupported:metadata")],
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
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { return 1; }\n".to_string(),
            );
            let mut body = test_mir_body(1, file, "body:unknown");
            body.status = MirStatus::Unknown;
            let mut place = test_place(1, file, "place:partial");
            place.status = PlaceStatus::Partial;

            db.replace_semantic_mir(MirOutput {
                bodies: vec![body],
                places: vec![place],
                operations: Vec::new(),
                unsupported: vec![test_unsupported("unsupported:metadata")],
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
                ("sdk/mod.rs", include_str!("../sdk/mod.rs")),
                ("sdk/facts.rs", include_str!("../sdk/facts.rs")),
                ("runner/mod.rs", include_str!("../runner/mod.rs")),
                ("lib.rs", include_str!("../lib.rs")),
            ];

            for (label, source) in sources {
                assert_no_forbidden_tokens(label, source);
            }
        }

        #[test]
        fn crate_root_keeps_analysis_module_crate_private_and_out_of_bench() {
            let lib = include_str!("../lib.rs");
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
                stable_key: format!("{prefix}:root"),
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
                stable_key: format!("{prefix}:package"),
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
                stable_key: format!("{prefix}:source-set"),
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
                stable_key: format!("{prefix}:requirement"),
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
                stable_key: format!("{prefix}:resolved"),
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
                from_package_stable_key: Some(format!("{prefix}:package")),
                to_package_stable_key: None,
                source_set_stable_key: Some(format!("{prefix}:source-set")),
                import_path: "react".to_string(),
                context: ImportContextKind::Source,
                stable_key: format!("{prefix}:import-to-package"),
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
                stable_key: format!("{prefix}:overlay"),
                producer_id: "test",
                precision: TopologyPrecision::Heuristic,
                status: TopologyStatus::Present,
            }],
        }
    }

    #[test]
    fn topology_storage_replaces_rows_and_rebuilds_metadata() {
        let mut db = AnalysisDb::new();
        db.replace_topology_facts(topology_output("first"));
        db.replace_topology_facts(topology_output("second"));

        assert_eq!(db.workspace_roots().len(), 1);
        assert_eq!(db.workspace_roots()[0].id.0, 0);
        assert_eq!(db.workspace_roots()[0].stable_key, "second:root");
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

        assert_eq!(db.workspace_roots()[0].stable_key, "base:root");
        assert_eq!(db.topology_packages()[0].stable_key, "base:package");
        assert_eq!(db.source_sets()[0].stable_key, "base:source-set");
        assert_eq!(
            db.dependency_requirements()[0].stable_key,
            "base:requirement"
        );
        assert_eq!(
            db.resolved_dependency_edges()[0].stable_key,
            "base:resolved"
        );
        assert_eq!(db.repo_topology_overlays()[0].stable_key, "base:overlay");
        assert_eq!(
            db.import_to_package_edges()[0].stable_key,
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

        db.push_package(PackageFact {
            id: PackageId(99),
            file: go_file,
            name: "payment".to_string(),
            span: go_span.clone(),
            language: Language::Go,
        });
        let go_function = db.push_function(FunctionFact {
            id: FunctionId(99),
            file: go_file,
            name: "Charge".to_string(),
            span: go_span.clone(),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 3,
            calls: vec!["authorize".to_string()],
        });
        let ts_function = db.push_function(FunctionFact {
            id: FunctionId(99),
            file: ts_file,
            name: "Button".to_string(),
            span: ts_span.clone(),
            language: Language::Tsx,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: vec!["render".to_string()],
        });
        db.push_import(ImportFact {
            id: ImportId(99),
            file: go_file,
            package: None,
            path: "context".to_string(),
            span: go_span.clone(),
            language: Language::Go,
        });
        db.push_branch(BranchObligation {
            id: BranchId(99),
            function: Some(go_function),
            file: go_file,
            decision_span: go_span.clone(),
            condition_text: "err != nil".to_string(),
            edge_label: "true".to_string(),
            is_error_path: true,
            stable_fingerprint: "branch".to_string(),
        });
        db.push_test(TestFact {
            file: go_file,
            function: Some(go_function),
            name: "TestCharge".to_string(),
            span: go_span,
            evidence_terms: vec!["err".to_string()],
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_ts_component(TsComponentFact {
            file: ts_file,
            function: Some(ts_function),
            name: "Button".to_string(),
            span: ts_span.clone(),
        });
        db.push_ts_class(TsClassFact {
            file: ts_file,
            name: "Dialog".to_string(),
            span: ts_span.clone(),
            is_exported: true,
            is_component_like: true,
        });
        db.push_string_literal(StringLiteralFact {
            file: ts_file,
            value: "Pay".to_string(),
            span: ts_span.clone(),
            language: Language::Tsx,
        });
        db.push_jsx_attribute(JsxAttributeFact {
            file: ts_file,
            name: "aria-label".to_string(),
            value: Some("Pay".to_string()),
            span: ts_span,
        });

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

        db.push_import(ImportFact {
            id: ImportId(99),
            file: second_file,
            package: None,
            path: "fmt".to_string(),
            span: test_span(second_file, 1),
            language: Language::Go,
        });
        db.push_import(ImportFact {
            id: ImportId(99),
            file: first_file,
            package: None,
            path: "strings".to_string(),
            span: test_span(first_file, 1),
            language: Language::Go,
        });

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

        db.push_test(TestFact {
            file: production_file,
            function: None,
            name: "TestInline".to_string(),
            span: test_span(production_file, 1),
            evidence_terms: Vec::new(),
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_test(TestFact {
            file: companion_file,
            function: None,
            name: "TestPayment".to_string(),
            span: test_span(companion_file, 1),
            evidence_terms: Vec::new(),
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_test(TestFact {
            file: unrelated_file,
            function: None,
            name: "TestUserPayment".to_string(),
            span: test_span(unrelated_file, 1),
            evidence_terms: Vec::new(),
            assertion_count: 1,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });

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
        DiagnosticRange {
            start_line,
            start_col,
            end_line,
            end_col,
        }
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

        let function = db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "main".to_string(),
            span: span.clone(),
            language: Language::Go,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        let import = db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "fmt".to_string(),
            span: span.clone(),
            language: Language::Go,
        });
        let branch = db.push_branch(BranchObligation {
            id: BranchId(99),
            function: Some(function),
            file,
            decision_span: span,
            condition_text: "err != nil".to_string(),
            edge_label: "true".to_string(),
            is_error_path: true,
            stable_fingerprint: "branch".to_string(),
        });

        assert_eq!(file, FileId(0));
        assert_eq!(function, FunctionId(0));
        assert_eq!(import, ImportId(0));
        assert_eq!(branch, BranchId(0));

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

        let first = db.push_package(PackageFact {
            id: PackageId(99),
            file: first_file,
            name: "payment".to_string(),
            span: test_span(first_file, 1),
            language: Language::Go,
        });
        let second = db.push_package(PackageFact {
            id: PackageId(99),
            file: second_file,
            name: "billing".to_string(),
            span: test_span(second_file, 1),
            language: Language::Go,
        });

        assert_eq!(first, PackageId(0));
        assert_eq!(second, PackageId(1));
        assert_eq!(db.packages()[0].id, PackageId(0));
        assert_eq!(db.packages()[1].id, PackageId(1));
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

        db.push_package(PackageFact {
            id: PackageId(99),
            file: first_file,
            name: "payment".to_string(),
            span: first_span.clone(),
            language: Language::Go,
        });
        db.push_package(PackageFact {
            id: PackageId(99),
            file: second_file,
            name: "billing".to_string(),
            span: second_span.clone(),
            language: Language::Go,
        });

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

        db.replace_semantic_index_facts(
            vec![test_scope("root", file, SemanticStatus::Resolved)],
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
                    stable_key: "ts|src/app.ts|value|function|Button|1:1".to_string(),
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
                    stable_key: "ts|src/theme.ts|value|constant|theme|1:1".to_string(),
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
                stable_key: "ts|src/app.ts|definition|Button|1:1".to_string(),
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
                    stable_key: "ts|src/app.ts|reference|theme|1:28".to_string(),
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
                    stable_key: "ts|src/app.ts|reference|missing|2:1".to_string(),
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
                stable_key: "route:/a".to_string(),
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
                stable_key: "route:/bad".to_string(),
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
                stable_key: "evidence:node:exact".to_string(),
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
}
