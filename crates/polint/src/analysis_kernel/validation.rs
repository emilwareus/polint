use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Debug;

use serde::Serialize;

use crate::analysis_kernel::{
    FactFamily, FactPrecision, FactRef, PrecisionCeiling, ProviderManifest,
};
use crate::analysis::validate::validate_semantic_mir;
use crate::core::{
    AnalysisDb, BranchId, FileId, FunctionId, ImportId, ModuleNodeId, PackageId, ResolvedImportId,
    Span, SymbolId,
};
use crate::diagnostics::{Diagnostic, TextRange};
use crate::module_graph::topology::{
    DependencyRequirementId, ImportToPackageStatus, ResolvedDependencyKind, SourceSetId,
    TopologyPackageId, TopologyPrecision, TopologyStatus, WorkspaceRootId,
};
use crate::symbol_graph::semantic::{ExportId, ScopeId, SemanticStatus};

const SYMBOL_GRAPH_PROVIDER_ID: &str = "polint.symbol_graph";
const SEMANTIC_EVIDENCE_ORDER: (&str, &str, &str) = ("family", "stable_key", "reason");

pub(crate) fn validate_fact_metadata(
    db: &AnalysisDb,
    manifests: &[ProviderManifest],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let ids = IdSets::from_db(db);
    let manifests_by_id = manifests
        .iter()
        .map(|manifest| (manifest.id, *manifest))
        .collect::<BTreeMap<_, _>>();

    validate_missing_metadata(db, &mut diagnostics);
    validate_stable_key_conflicts(db, &mut diagnostics);
    validate_references(db, &ids, &mut diagnostics);
    validate_spans(db, &ids.files, &mut diagnostics);
    validate_semantic_index(db, &ids, &mut diagnostics);
    validate_topology_facts(db, &ids, &mut diagnostics);
    validate_metadata_providers(db, &manifests_by_id, &mut diagnostics);
    validate_precision_ceilings(db, &manifests_by_id, &mut diagnostics);

    diagnostics.sort_by(diagnostic_order);
    diagnostics
}

#[cfg(test)]
mod semantic_mir {
    use super::validate_fact_metadata;
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId, UnsupportedId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{
        ConservativeAction, MirOperation, MirOperationKind, MirValue, UnsupportedPrecision,
        UnsupportedSemanticFact,
    };
    use crate::analysis::places::{PlaceFact, PlaceProjection, PlaceRoot, PlaceStatus};
    use crate::analysis_kernel::{
        AnalysisKernel, FactConfidence, FactFamily, FactMeta, FactPrecision, FactRef,
        ValidationStatus,
    };
    use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span};
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[test]
    fn semantic_mir_validation_reports_malformed_rows_with_required_evidence() {
        let mut db = base_db();
        db.replace_semantic_mir(MirOutput {
            bodies: vec![
                body(0, FunctionId(0), span(FileId(0), 0, 20), "body:dup"),
                body(1, FunctionId(99), span(FileId(0), 0, 999), "body:dup"),
            ],
            places: vec![
                place(
                    0,
                    FunctionId(99),
                    vec![PlaceProjection::IndexUnknown {
                        evidence: String::new(),
                    }],
                    "place:bad",
                ),
                call_return_place(1, "place:return"),
            ],
            operations: vec![MirOperation {
                id: MirOpId(0),
                body: MirBodyId(0),
                ordinal: 0,
                span: span(FileId(0), 0, 20),
                kind: MirOperationKind::Call {
                    site: CallSiteId(0),
                    callee: MirValue::Unknown {
                        evidence: "dynamic".to_string(),
                    },
                    arguments: vec![PlaceId(0)],
                    return_place: PlaceId(1),
                },
                stable_key: "op:call".to_string(),
                status: MirStatus::Partial,
            }],
            unsupported: vec![UnsupportedSemanticFact {
                id: UnsupportedId(0),
                body: Some(MirBodyId(0)),
                operation: Some(MirOpId(0)),
                language: Language::TypeScript,
                file: FileId(0),
                span: span(FileId(0), 0, 20),
                construct: String::new(),
                source_evidence: String::new(),
                affected_places: Vec::new(),
                affected_domains: Vec::new(),
                conservative_action: ConservativeAction::HavocAffectedPlaces,
                precision: UnsupportedPrecision::Unsupported,
                status: MirStatus::Unsupported,
                stable_key: "unsupported:bad".to_string(),
            }],
        })
        .expect("semantic rows should store for validation");

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let semantic_mir = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .message
                    .starts_with("Semantic MIR validation failed")
            })
            .collect::<Vec<_>>();

        assert!(
            semantic_mir.len() >= 5,
            "expected semantic MIR diagnostics: {diagnostics:#?}"
        );
        assert!(semantic_mir.iter().all(|diagnostic| {
            let labels = evidence_labels(diagnostic);
            labels.contains("family")
                && labels.contains("stable_key")
                && labels.contains("field")
                && labels.contains("reason")
        }));
    }

    #[test]
    fn semantic_mir_validation_rejects_exact_provider_precision() {
        let mut db = base_db();
        db.replace_semantic_mir(MirOutput {
            bodies: vec![body(0, FunctionId(0), span(FileId(0), 0, 20), "body:ok")],
            places: Vec::new(),
            operations: Vec::new(),
            unsupported: Vec::new(),
        })
        .expect("semantic rows should store");
        db.fact_meta_mut_for_test()
            .remove_for_test(FactRef::new(FactFamily::MirBody, 0));
        db.fact_meta_mut_for_test().insert(
            FactRef::new(FactFamily::MirBody, 0),
            FactMeta {
                stable_key: "body:ok".to_string(),
                producer_id: "polint.semantic_mir",
                layer_id: "polint.semantic_mir",
                precision: FactPrecision::Exact,
                confidence: FactConfidence::High,
                validation: ValidationStatus::NativeTrusted,
                payload_digest: "payload:exact-mir".to_string(),
            },
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .starts_with("Semantic MIR validation failed")
                    && diagnostic.evidence.iter().any(|evidence| {
                        evidence.label == "reason"
                            && evidence.value.contains("precision ceiling")
                    })
            }),
            "expected semantic MIR precision ceiling diagnostic: {diagnostics:#?}"
        );
    }

    fn base_db() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function app() { return 1; }\n".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "app".to_string(),
            span: span(file, 0, 33),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db
    }

    fn body(id: u64, function: FunctionId, span: Span, stable_key: &str) -> MirBody {
        MirBody {
            id: MirBodyId(id),
            language: Language::TypeScript,
            file: span.file,
            function,
            package: None,
            module: None,
            owner_stable_key: format!("function:{}", function.0),
            span,
            stable_key: stable_key.to_string(),
            status: MirStatus::Partial,
        }
    }

    fn place(
        id: u64,
        function: FunctionId,
        projections: Vec<PlaceProjection>,
        stable_key: &str,
    ) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            function: Some(function),
            root: PlaceRoot::Local {
                function,
                name: "value".to_string(),
            },
            projections,
            stable_key: stable_key.to_string(),
            status: PlaceStatus::Partial,
        }
    }

    fn call_return_place(id: u64, stable_key: &str) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::TypeScript,
            file: Some(FileId(0)),
            function: Some(FunctionId(0)),
            root: PlaceRoot::CallReturn {
                call: CallSiteId(0),
            },
            projections: Vec::new(),
            stable_key: stable_key.to_string(),
            status: PlaceStatus::Partial,
        }
    }

    fn span(file: FileId, start_byte: u32, end_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte,
            start_line: 1,
            start_col: start_byte + 1,
            end_line: 1,
            end_col: end_byte + 1,
        }
    }

    fn evidence_labels(diagnostic: &crate::diagnostics::Diagnostic) -> BTreeSet<&str> {
        diagnostic
            .evidence
            .iter()
            .map(|evidence| evidence.label.as_str())
            .collect()
    }
}

#[cfg(test)]
mod semantic_index {
    use super::validate_fact_metadata;
    use crate::analysis_kernel::{
        AnalysisKernel, FactConfidence, FactFamily, FactMeta, FactPrecision, FactRef,
        ValidationStatus,
    };
    use crate::core::{
        AnalysisDb, FileId, Language, Span, SymbolFact, SymbolId, SymbolKind, SymbolNamespace,
        SymbolPrecision,
    };
    use crate::symbol_graph::semantic::{
        ExportFact, ExportId, ExportKind, GeneratedSymbolFact, GeneratedSymbolId,
        GeneratedSymbolKind, SemanticStatus, StableExportId, StableExportIdentity,
    };
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[test]
    fn semantic_validation_reports_malformed_generated_rows_with_evidence() {
        let mut db = semantic_db();
        db.replace_semantic_index_facts(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![GeneratedSymbolFact {
                id: GeneratedSymbolId(99),
                language: Language::TypeScript,
                file: Some(FileId(404)),
                package: None,
                module: None,
                symbol_stable_key: "symbol:answer".to_string(),
                source_stable_key: String::new(),
                producer_id: String::new(),
                generator: "test".to_string(),
                generated_discriminator: String::new(),
                kind: GeneratedSymbolKind::BuildGenerated,
                span: Some(span(FileId(404), 0, 999)),
                stable_key: "generated:bad".to_string(),
                status: SemanticStatus::Resolved,
            }],
            Vec::new(),
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let semantic_diagnostics = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .message
                    .starts_with("Semantic index validation failed")
            })
            .collect::<Vec<_>>();

        assert!(
            semantic_diagnostics.len() >= 4,
            "expected generated-row validation diagnostics: {diagnostics:#?}"
        );
        assert!(
            semantic_diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule_id == "polint/internal")
        );
        assert!(semantic_diagnostics.iter().all(|diagnostic| {
            let labels = evidence_labels(diagnostic);
            labels.contains("family") && labels.contains("stable_key") && labels.contains("reason")
        }));
    }

    #[test]
    fn semantic_validation_rejects_symbol_graph_exact_semantic_metadata() {
        let mut db = semantic_db();
        db.replace_semantic_index_facts(
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![GeneratedSymbolFact {
                id: GeneratedSymbolId(0),
                language: Language::TypeScript,
                file: Some(FileId(0)),
                package: None,
                module: None,
                symbol_stable_key: "symbol:answer".to_string(),
                source_stable_key: "symbol:answer".to_string(),
                producer_id: "polint.symbol_graph".to_string(),
                generator: "test".to_string(),
                generated_discriminator: "entrypoint".to_string(),
                kind: GeneratedSymbolKind::BuildGenerated,
                span: Some(span(FileId(0), 0, 1)),
                stable_key: "generated:answer".to_string(),
                status: SemanticStatus::Generated,
            }],
            Vec::new(),
        );
        db.fact_meta_mut_for_test()
            .remove_for_test(FactRef::new(FactFamily::GeneratedSymbol, 0));
        db.fact_meta_mut_for_test().insert(
            FactRef::new(FactFamily::GeneratedSymbol, 0),
            FactMeta {
                stable_key: "generated:answer".to_string(),
                producer_id: "polint.symbol_graph",
                layer_id: "polint.symbol_graph",
                precision: FactPrecision::Exact,
                confidence: FactConfidence::High,
                validation: ValidationStatus::NativeTrusted,
                payload_digest: "payload:exact-generated".to_string(),
            },
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .starts_with("Semantic index validation failed")
                    && diagnostic.evidence.iter().any(|evidence| {
                        evidence.label == "reason"
                            && evidence.value.contains("provider precision ceiling")
                    })
                    && diagnostic.evidence.iter().any(|evidence| {
                        evidence.label == "family" && evidence.value == "GeneratedSymbol"
                    })
                    && diagnostic.evidence.iter().any(|evidence| {
                        evidence.label == "stable_key" && evidence.value == "generated:answer"
                    })
            }),
            "expected semantic precision ceiling diagnostic: {diagnostics:#?}"
        );
    }

    #[test]
    fn semantic_validation_rejects_missing_stable_export_symbol_key() {
        let mut db = semantic_db();
        db.replace_semantic_index_facts(
            Vec::new(),
            Vec::new(),
            vec![ExportFact {
                id: ExportId(0),
                language: Language::TypeScript,
                file: Some(FileId(0)),
                package: None,
                module: None,
                scope: None,
                symbol: None,
                export_name: "answer".to_string(),
                namespace: SymbolNamespace::Value,
                kind: ExportKind::Named,
                stable_key: "export:answer".to_string(),
                status: SemanticStatus::Resolved,
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![StableExportIdentity {
                id: StableExportId(0),
                export: ExportId(0),
                language: Language::TypeScript,
                package_key: None,
                module_key: Some("src/app.ts".to_string()),
                export_name: "answer".to_string(),
                namespace: SymbolNamespace::Value,
                symbol_stable_key: "symbol:missing".to_string(),
                generated_discriminator: Some("native".to_string()),
                stable_key: "stable-export:answer".to_string(),
                status: SemanticStatus::Resolved,
            }],
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic
                    .message
                    .starts_with("Semantic index validation failed")
                    && diagnostic.evidence.iter().any(|evidence| {
                        evidence.label == "reason"
                            && evidence
                                .value
                                .contains("StableExportIdentity.symbol_stable_key does not exist")
                    })
            }),
            "expected missing stable export symbol diagnostic: {diagnostics:#?}"
        );
    }

    fn semantic_db() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export const answer = 1;\n".to_string(),
        );
        db.replace_symbol_graph_facts(
            vec![SymbolFact {
                id: SymbolId(0),
                language: Language::TypeScript,
                name: "answer".to_string(),
                qualified_name: "answer".to_string(),
                kind: SymbolKind::Constant,
                namespace: SymbolNamespace::Value,
                file: Some(file),
                package: None,
                module: None,
                owner: None,
                primary_span: Some(span(file, 13, 19)),
                is_exported: true,
                stable_key: "symbol:answer".to_string(),
                precision: SymbolPrecision::ExactLocal,
            }],
            Vec::new(),
            Vec::new(),
        );
        db
    }

    fn span(file: FileId, start_byte: u32, end_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte,
            start_line: 1,
            start_col: start_byte + 1,
            end_line: 1,
            end_col: end_byte + 1,
        }
    }

    fn evidence_labels(diagnostic: &crate::diagnostics::Diagnostic) -> BTreeSet<&str> {
        diagnostic
            .evidence
            .iter()
            .map(|evidence| evidence.label.as_str())
            .collect()
    }
}

#[cfg(test)]
mod topology {
    use super::validate_fact_metadata;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::core::{
        AnalysisDb, FileId, ImportFact, ImportId, Language, ModuleNodeId, ResolutionPrecision,
        ResolutionStatus, ResolvedImportFact, ResolvedImportId, Span,
    };
    use crate::module_graph::topology::{
        DependencyRequirementFact, DependencyRequirementId, ImportContextKind, ImportToPackageFact,
        ImportToPackageId, ImportToPackageStatus, RequirementKind, ResolvedDependencyEdgeFact,
        ResolvedDependencyEdgeId, ResolvedDependencyKind, SourceSetFact, SourceSetId,
        SourceSetKind, TopologyOutput, TopologyPackageFact, TopologyPackageId, TopologyPackageKind,
        TopologyPrecision, TopologyStatus, WorkspaceRootFact, WorkspaceRootId, WorkspaceRootKind,
    };
    use crate::symbol_graph::semantic::{
        SemanticImportFact, SemanticImportId, SemanticImportKind, SemanticStatus,
    };
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[test]
    fn topology_validation_reports_invalid_refs_and_malformed_paths() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import './target';\n".to_string(),
        );
        let import = db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "./target".to_string(),
            span: span(file),
            language: Language::TypeScript,
        });
        db.replace_module_graph_facts(
            vec![ResolvedImportFact {
                id: ResolvedImportId(0),
                import,
                from_file: file,
                target_node: None,
                status: ResolutionStatus::Unresolved,
                precision: ResolutionPrecision::None,
                reason: None,
            }],
            Vec::new(),
            Vec::new(),
        );
        db.replace_topology_facts(TopologyOutput {
            workspace_roots: vec![root("/absolute", "root:absolute")],
            packages: vec![package(
                "package:bad",
                Some(WorkspaceRootId(404)),
                Some(ModuleNodeId(404)),
                "../escape",
                TopologyPrecision::ExactStatic,
                TopologyStatus::Present,
            )],
            source_sets: vec![SourceSetFact {
                id: SourceSetId(0),
                package: Some(TopologyPackageId(404)),
                root: Some(WorkspaceRootId(404)),
                kind: SourceSetKind::Source,
                path: r"src\app.ts".to_string(),
                language: Some(Language::TypeScript),
                files: vec![FileId(404)],
                stable_key: "source-set:bad".to_string(),
                producer_id: "test",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Present,
            }],
            import_to_package_edges: vec![import_edge(
                "import-to-package:bad",
                Some(ImportId(404)),
                Some(ResolvedImportId(404)),
                Some("semantic:missing".to_string()),
                Some(FileId(404)),
                ImportToPackageStatus::Resolved,
                TopologyPrecision::ExactStatic,
            )],
            ..TopologyOutput::default()
        });

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let topology = topology_diagnostics(&diagnostics);

        assert!(
            topology.len() >= 6,
            "expected topology validation diagnostics: {diagnostics:#?}"
        );
        assert!(topology.iter().all(|diagnostic| {
            let labels = evidence_labels(diagnostic);
            labels.contains("family")
                && labels.contains("stable_key")
                && labels.contains("field")
                && labels.contains("reason")
        }));
    }

    #[test]
    fn topology_stable_key_conflicts_reject_nonidentical_duplicate_rows() {
        let mut db = AnalysisDb::new();
        db.replace_topology_facts(TopologyOutput {
            workspace_roots: vec![root(".", "root:repo")],
            packages: vec![
                package(
                    "package:dup",
                    Some(WorkspaceRootId(0)),
                    None,
                    "src/a",
                    TopologyPrecision::ExactStatic,
                    TopologyStatus::Present,
                ),
                package(
                    "package:dup",
                    Some(WorkspaceRootId(0)),
                    None,
                    "src/b",
                    TopologyPrecision::ExactStatic,
                    TopologyStatus::Present,
                ),
            ],
            ..TopologyOutput::default()
        });

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert!(topology_diagnostics(&diagnostics).iter().any(|diagnostic| {
            diagnostic.evidence.iter().any(|evidence| {
                evidence.label == "reason" && evidence.value.contains("stable_key_conflict")
            })
        }));
    }

    #[test]
    fn topology_validation_rejects_unsupported_or_dynamic_rows_claiming_exactness() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "import React from 'react';\n".to_string(),
        );
        let import = db.push_import(ImportFact {
            id: ImportId(99),
            file,
            package: None,
            path: "react".to_string(),
            span: span(file),
            language: Language::TypeScript,
        });
        db.replace_semantic_index_facts(
            Vec::new(),
            vec![SemanticImportFact {
                id: SemanticImportId(0),
                language: Language::TypeScript,
                file: Some(file),
                package: None,
                module: None,
                scope: None,
                import_path: "react".to_string(),
                local_name: None,
                imported_name: None,
                namespace: crate::core::SymbolNamespace::Value,
                kind: SemanticImportKind::DynamicImport,
                stable_key: "semantic:react".to_string(),
                status: SemanticStatus::Dynamic,
            }],
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        db.replace_topology_facts(TopologyOutput {
            dependency_requirements: vec![DependencyRequirementFact {
                id: DependencyRequirementId(0),
                from_package: None,
                target_package: None,
                target_name: "react".to_string(),
                version_requirement: Some("^18".to_string()),
                kind: RequirementKind::Runtime,
                manifest_path: Some("package.json".to_string()),
                stable_key: "requirement:react".to_string(),
                producer_id: "test",
                precision: TopologyPrecision::ExactLockfile,
                status: TopologyStatus::Unsupported,
            }],
            resolved_dependency_edges: vec![ResolvedDependencyEdgeFact {
                id: ResolvedDependencyEdgeId(0),
                requirement: Some(DependencyRequirementId(0)),
                from_package: None,
                to_package: None,
                package_name: "react".to_string(),
                resolved_version: None,
                kind: ResolvedDependencyKind::Unknown,
                stable_key: "resolved:react".to_string(),
                producer_id: "test",
                precision: TopologyPrecision::ExactLockfile,
                status: TopologyStatus::Unsupported,
            }],
            import_to_package_edges: vec![import_edge(
                "import-to-package:dynamic",
                Some(import),
                None,
                Some("semantic:react".to_string()),
                Some(file),
                ImportToPackageStatus::Dynamic,
                TopologyPrecision::ExactStatic,
            )],
            ..TopologyOutput::default()
        });

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let reasons = topology_diagnostics(&diagnostics)
            .iter()
            .flat_map(|diagnostic| diagnostic.evidence.iter())
            .filter(|evidence| evidence.label == "reason")
            .map(|evidence| evidence.value.as_str())
            .collect::<Vec<_>>();

        assert!(
            reasons
                .iter()
                .any(|reason| reason.contains("unsupported_or_dynamic_exactness")),
            "expected exactness rejection: {diagnostics:#?}"
        );
    }

    #[test]
    fn topology_validation_accepts_exact_lockfile_for_lockfile_dependency_rows() {
        let mut db = AnalysisDb::new();
        db.replace_topology_facts(TopologyOutput {
            resolved_dependency_edges: [
                ResolvedDependencyKind::Lockfile,
                ResolvedDependencyKind::LockfileSelected,
                ResolvedDependencyKind::ChecksumEvidence,
            ]
            .into_iter()
            .enumerate()
            .map(|(index, kind)| ResolvedDependencyEdgeFact {
                id: ResolvedDependencyEdgeId(index as u64),
                requirement: None,
                from_package: None,
                to_package: None,
                package_name: format!("package-{index}"),
                resolved_version: Some("1.0.0".to_string()),
                kind,
                stable_key: format!("resolved:package-{index}"),
                producer_id: "test",
                precision: TopologyPrecision::ExactLockfile,
                status: TopologyStatus::Resolved,
            })
            .collect(),
            ..TopologyOutput::default()
        });

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert!(
            topology_diagnostics(&diagnostics).is_empty(),
            "expected exact lockfile rows to validate cleanly: {diagnostics:#?}"
        );
    }

    fn root(path: &str, stable_key: &str) -> WorkspaceRootFact {
        WorkspaceRootFact {
            id: WorkspaceRootId(0),
            kind: WorkspaceRootKind::Repository,
            root_path: path.to_string(),
            manifest_path: None,
            language: None,
            stable_key: stable_key.to_string(),
            producer_id: "test",
            precision: TopologyPrecision::ExactStatic,
            status: TopologyStatus::Present,
        }
    }

    fn package(
        stable_key: &str,
        workspace_root: Option<WorkspaceRootId>,
        module_node: Option<ModuleNodeId>,
        path: &str,
        precision: TopologyPrecision,
        status: TopologyStatus,
    ) -> TopologyPackageFact {
        TopologyPackageFact {
            id: TopologyPackageId(0),
            workspace_root,
            package: None,
            module_node,
            kind: TopologyPackageKind::Workspace,
            name: stable_key.to_string(),
            version: None,
            path: path.to_string(),
            language: Some(Language::TypeScript),
            stable_key: stable_key.to_string(),
            producer_id: "test",
            precision,
            status,
        }
    }

    fn import_edge(
        stable_key: &str,
        syntax_import: Option<ImportId>,
        resolved_import: Option<ResolvedImportId>,
        semantic_import_stable_key: Option<String>,
        from_file: Option<FileId>,
        status: ImportToPackageStatus,
        precision: TopologyPrecision,
    ) -> ImportToPackageFact {
        ImportToPackageFact {
            id: ImportToPackageId(0),
            syntax_import,
            resolved_import,
            semantic_import_stable_key,
            from_file,
            from_package: Some(TopologyPackageId(404)),
            to_package: None,
            target_node: Some(ModuleNodeId(404)),
            from_package_stable_key: None,
            to_package_stable_key: None,
            source_set_stable_key: None,
            import_path: "react".to_string(),
            context: ImportContextKind::Source,
            stable_key: stable_key.to_string(),
            producer_id: "test",
            precision,
            status,
        }
    }

    fn span(file: FileId) -> Span {
        Span {
            file,
            start_byte: 0,
            end_byte: 1,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 2,
        }
    }

    fn topology_diagnostics(
        diagnostics: &[crate::diagnostics::Diagnostic],
    ) -> Vec<&crate::diagnostics::Diagnostic> {
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.starts_with("Topology validation failed"))
            .collect()
    }

    fn evidence_labels(diagnostic: &crate::diagnostics::Diagnostic) -> BTreeSet<&str> {
        diagnostic
            .evidence
            .iter()
            .map(|evidence| evidence.label.as_str())
            .collect()
    }
}

#[derive(Debug, Default)]
pub(crate) struct IdSets {
    files: BTreeSet<FileId>,
    packages: BTreeSet<PackageId>,
    functions: BTreeSet<FunctionId>,
    branches: BTreeSet<BranchId>,
    imports: BTreeSet<ImportId>,
    resolved_imports: BTreeSet<ResolvedImportId>,
    module_nodes: BTreeSet<ModuleNodeId>,
    workspace_roots: BTreeSet<WorkspaceRootId>,
    topology_packages: BTreeSet<TopologyPackageId>,
    source_sets: BTreeSet<SourceSetId>,
    dependency_requirements: BTreeSet<DependencyRequirementId>,
    symbols: BTreeSet<SymbolId>,
    scopes: BTreeSet<ScopeId>,
    exports: BTreeSet<ExportId>,
    semantic_import_stable_keys: BTreeSet<String>,
    symbol_stable_keys: BTreeSet<String>,
    reference_stable_keys: BTreeSet<String>,
}

impl IdSets {
    fn from_db(db: &AnalysisDb) -> Self {
        Self {
            files: db.files().iter().map(|fact| fact.id).collect(),
            packages: db.packages().iter().map(|fact| fact.id).collect(),
            functions: db.functions().iter().map(|fact| fact.id).collect(),
            branches: db.branches().iter().map(|fact| fact.id).collect(),
            imports: db.imports().iter().map(|fact| fact.id).collect(),
            resolved_imports: db.resolved_imports().iter().map(|fact| fact.id).collect(),
            module_nodes: db.module_nodes().iter().map(|fact| fact.id).collect(),
            workspace_roots: db.workspace_roots().iter().map(|fact| fact.id).collect(),
            topology_packages: db.topology_packages().iter().map(|fact| fact.id).collect(),
            source_sets: db.source_sets().iter().map(|fact| fact.id).collect(),
            dependency_requirements: db
                .dependency_requirements()
                .iter()
                .map(|fact| fact.id)
                .collect(),
            symbols: db.symbols().iter().map(|fact| fact.id).collect(),
            scopes: db.scopes().iter().map(|fact| fact.id).collect(),
            exports: db.exports().iter().map(|fact| fact.id).collect(),
            semantic_import_stable_keys: db
                .semantic_imports()
                .iter()
                .map(|fact| fact.stable_key.clone())
                .collect(),
            symbol_stable_keys: db
                .symbols()
                .iter()
                .map(|fact| fact.stable_key.clone())
                .collect(),
            reference_stable_keys: db
                .references()
                .iter()
                .map(|fact| fact.stable_key.clone())
                .collect(),
        }
    }
}

fn validate_missing_metadata(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    for missing in db.missing_fact_metadata() {
        diagnostics.push(
            internal_diagnostic(format!(
                "Fact metadata missing for {}#{}.",
                missing.family.label(),
                missing.run_id
            ))
            .with_evidence("family", missing.family.label())
            .with_evidence(
                "fact_ref",
                fact_ref_value(FactRef::new(missing.family, missing.run_id)),
            ),
        );
    }
}

fn validate_stable_key_conflicts(db: &AnalysisDb, diagnostics: &mut Vec<Diagnostic>) {
    for conflict in db.fact_meta().stable_key_conflicts() {
        diagnostics.push(
            internal_diagnostic(format!(
                "Fact metadata stable key conflict detected for {} stable key.",
                conflict.family.label()
            ))
            .with_evidence("family", conflict.family.label())
            .with_evidence("stable_key", conflict.stable_key.clone())
            .with_evidence("existing_ref", fact_ref_value(conflict.existing))
            .with_evidence("incoming_ref", fact_ref_value(conflict.incoming)),
        );
    }
}

fn validate_metadata_providers(
    db: &AnalysisDb,
    manifests_by_id: &BTreeMap<&'static str, ProviderManifest>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (reference, metadata) in db.fact_meta().rows() {
        if !manifests_by_id.contains_key(metadata.producer_id) {
            diagnostics.push(provider_manifest_diagnostic(
                reference,
                "producer_id",
                metadata.producer_id,
            ));
        }
        if !manifests_by_id.contains_key(metadata.layer_id) {
            diagnostics.push(provider_manifest_diagnostic(
                reference,
                "layer_id",
                metadata.layer_id,
            ));
        }
    }
}

fn validate_references(db: &AnalysisDb, ids: &IdSets, diagnostics: &mut Vec<Diagnostic>) {
    for fact in db.functions() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::Function,
            fact.id.0,
            "FunctionFact.file",
            fact.file,
        );
    }
    for fact in db.packages() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::Package,
            fact.id.0,
            "PackageFact.file",
            fact.file,
        );
    }
    for fact in db.imports() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::Import,
            fact.id.0,
            "ImportFact.file",
            fact.file,
        );
    }
    for fact in db.branches() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::BranchObligation,
            fact.id.0,
            "BranchObligation.file",
            fact.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.functions,
            FactFamily::BranchObligation,
            fact.id.0,
            "BranchObligation.function",
            fact.function,
        );
    }
    for (run_id, fact) in db.tests().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::Test,
            run_id as u64,
            "TestFact.file",
            fact.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.functions,
            FactFamily::Test,
            run_id as u64,
            "TestFact.function",
            fact.function,
        );
    }
    for (run_id, fact) in db.coverage().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.branches,
            FactFamily::Coverage,
            run_id as u64,
            "CoverageFact.branch",
            fact.branch,
        );
    }
    for (run_id, fact) in db.ts_components().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::TsComponent,
            run_id as u64,
            "TsComponentFact.file",
            fact.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.functions,
            FactFamily::TsComponent,
            run_id as u64,
            "TsComponentFact.function",
            fact.function,
        );
    }
    for (run_id, fact) in db.ts_classes().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::TsClass,
            run_id as u64,
            "TsClassFact.file",
            fact.file,
        );
    }
    for (run_id, fact) in db.string_literals().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::StringLiteral,
            run_id as u64,
            "StringLiteralFact.file",
            fact.file,
        );
    }
    for (run_id, fact) in db.jsx_attributes().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::JsxAttribute,
            run_id as u64,
            "JsxAttributeFact.file",
            fact.file,
        );
    }
    for (run_id, fact) in db.file_metrics().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::FileMetric,
            run_id as u64,
            "FileMetricFact.file",
            fact.file,
        );
    }
    for (run_id, fact) in db.function_metrics().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::FunctionMetric,
            run_id as u64,
            "FunctionMetricFact.file",
            fact.file,
        );
        check_ref(
            diagnostics,
            &ids.functions,
            FactFamily::FunctionMetric,
            run_id as u64,
            "FunctionMetricFact.function",
            fact.function,
        );
    }
    for (run_id, fact) in db.complexity_metrics().iter().enumerate() {
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::ComplexityMetric,
            run_id as u64,
            "ComplexityMetricFact.file",
            fact.file,
        );
        check_ref(
            diagnostics,
            &ids.functions,
            FactFamily::ComplexityMetric,
            run_id as u64,
            "ComplexityMetricFact.function",
            fact.function,
        );
    }
    for fact in db.resolved_imports() {
        check_ref(
            diagnostics,
            &ids.imports,
            FactFamily::ResolvedImport,
            fact.id.0,
            "ResolvedImportFact.import",
            fact.import,
        );
        check_ref(
            diagnostics,
            &ids.files,
            FactFamily::ResolvedImport,
            fact.id.0,
            "ResolvedImportFact.from_file",
            fact.from_file,
        );
        check_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::ResolvedImport,
            fact.id.0,
            "ResolvedImportFact.target_node",
            fact.target_node,
        );
    }
    for node in db.module_nodes() {
        check_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::ModuleNode,
            node.id.0,
            "ModuleNode.file",
            node.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::ModuleNode,
            node.id.0,
            "ModuleNode.package",
            node.package,
        );
    }
    for edge in db.module_edges() {
        check_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::ModuleEdge,
            edge.id.0,
            "ModuleEdge.from",
            edge.from,
        );
        check_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::ModuleEdge,
            edge.id.0,
            "ModuleEdge.to",
            edge.to,
        );
        check_optional_ref(
            diagnostics,
            &ids.imports,
            FactFamily::ModuleEdge,
            edge.id.0,
            "ModuleEdge.import",
            edge.import,
        );
        check_optional_ref(
            diagnostics,
            &ids.resolved_imports,
            FactFamily::ModuleEdge,
            edge.id.0,
            "ModuleEdge.resolved_import",
            edge.resolved_import,
        );
    }
    for symbol in db.symbols() {
        check_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::Symbol,
            symbol.id.0,
            "SymbolFact.file",
            symbol.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::Symbol,
            symbol.id.0,
            "SymbolFact.package",
            symbol.package,
        );
        check_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::Symbol,
            symbol.id.0,
            "SymbolFact.module",
            symbol.module,
        );
        check_optional_ref(
            diagnostics,
            &ids.symbols,
            FactFamily::Symbol,
            symbol.id.0,
            "SymbolFact.owner",
            symbol.owner,
        );
    }
    for definition in db.definitions() {
        check_ref(
            diagnostics,
            &ids.symbols,
            FactFamily::Definition,
            definition.id.0,
            "DefinitionFact.symbol",
            definition.symbol,
        );
        check_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::Definition,
            definition.id.0,
            "DefinitionFact.file",
            definition.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::Definition,
            definition.id.0,
            "DefinitionFact.package",
            definition.package,
        );
        check_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::Definition,
            definition.id.0,
            "DefinitionFact.module",
            definition.module,
        );
        check_optional_ref(
            diagnostics,
            &ids.symbols,
            FactFamily::Definition,
            definition.id.0,
            "DefinitionFact.owner",
            definition.owner,
        );
    }
    for reference in db.references() {
        check_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::Reference,
            reference.id.0,
            "ReferenceFact.file",
            reference.file,
        );
        check_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::Reference,
            reference.id.0,
            "ReferenceFact.package",
            reference.package,
        );
        check_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::Reference,
            reference.id.0,
            "ReferenceFact.module",
            reference.module,
        );
        check_optional_ref(
            diagnostics,
            &ids.symbols,
            FactFamily::Reference,
            reference.id.0,
            "ReferenceFact.owner",
            reference.owner,
        );
        check_optional_ref(
            diagnostics,
            &ids.symbols,
            FactFamily::Reference,
            reference.id.0,
            "ReferenceFact.target",
            reference.target,
        );
        for candidate in &reference.candidates {
            check_ref(
                diagnostics,
                &ids.symbols,
                FactFamily::Reference,
                reference.id.0,
                "ReferenceFact.candidates",
                *candidate,
            );
        }
    }
}

fn validate_spans(db: &AnalysisDb, file_ids: &BTreeSet<FileId>, diagnostics: &mut Vec<Diagnostic>) {
    for fact in db.functions() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::Function,
                run_id: fact.id.0,
                field: "FunctionFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for fact in db.packages() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::Package,
                run_id: fact.id.0,
                field: "PackageFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for fact in db.imports() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::Import,
                run_id: fact.id.0,
                field: "ImportFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for fact in db.branches() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::BranchObligation,
                run_id: fact.id.0,
                field: "BranchObligation.decision_span",
                owner_file: Some(fact.file),
                span: &fact.decision_span,
            },
        );
    }
    for (run_id, fact) in db.tests().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::Test,
                run_id: run_id as u64,
                field: "TestFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.ts_components().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::TsComponent,
                run_id: run_id as u64,
                field: "TsComponentFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.ts_classes().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::TsClass,
                run_id: run_id as u64,
                field: "TsClassFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.string_literals().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::StringLiteral,
                run_id: run_id as u64,
                field: "StringLiteralFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.jsx_attributes().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::JsxAttribute,
                run_id: run_id as u64,
                field: "JsxAttributeFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.function_metrics().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::FunctionMetric,
                run_id: run_id as u64,
                field: "FunctionMetricFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for (run_id, fact) in db.complexity_metrics().iter().enumerate() {
        check_span(
            db,
            file_ids,
            diagnostics,
            SpanCheck {
                family: FactFamily::ComplexityMetric,
                run_id: run_id as u64,
                field: "ComplexityMetricFact.span",
                owner_file: Some(fact.file),
                span: &fact.span,
            },
        );
    }
    for symbol in db.symbols() {
        if let Some(span) = &symbol.primary_span {
            check_span(
                db,
                file_ids,
                diagnostics,
                SpanCheck {
                    family: FactFamily::Symbol,
                    run_id: symbol.id.0,
                    field: "SymbolFact.primary_span",
                    owner_file: symbol.file,
                    span,
                },
            );
        }
    }
    for definition in db.definitions() {
        if let Some(span) = &definition.primary_span {
            check_span(
                db,
                file_ids,
                diagnostics,
                SpanCheck {
                    family: FactFamily::Definition,
                    run_id: definition.id.0,
                    field: "DefinitionFact.primary_span",
                    owner_file: definition.file,
                    span,
                },
            );
        }
    }
    for reference in db.references() {
        if let Some(span) = &reference.primary_span {
            check_span(
                db,
                file_ids,
                diagnostics,
                SpanCheck {
                    family: FactFamily::Reference,
                    run_id: reference.id.0,
                    field: "ReferenceFact.primary_span",
                    owner_file: reference.file,
                    span,
                },
            );
        }
    }
}

fn validate_precision_ceilings(
    db: &AnalysisDb,
    manifests_by_id: &BTreeMap<&'static str, ProviderManifest>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for (reference, metadata) in db.fact_meta().rows() {
        let Some(manifest) = manifests_by_id.get(metadata.producer_id) else {
            continue;
        };
        if precision_within_ceiling(metadata.precision, manifest.precision_ceiling) {
            continue;
        }
        diagnostics.push(
            internal_diagnostic(format!(
                "Fact metadata precision ceiling violated for {}#{}.",
                reference.family.label(),
                reference.run_id
            ))
            .with_evidence("producer_id", metadata.producer_id)
            .with_evidence("family", reference.family.label())
            .with_evidence("precision", precision_label(metadata.precision))
            .with_evidence("ceiling", ceiling_label(manifest.precision_ceiling)),
        );
    }
}

fn validate_semantic_index(db: &AnalysisDb, ids: &IdSets, diagnostics: &mut Vec<Diagnostic>) {
    let semantic_keys = semantic_reference_keys(db, ids);

    for scope in db.scopes() {
        check_semantic_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::Scope,
            scope.stable_key.as_str(),
            "ScopeFact.file",
            scope.file,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::Scope,
            scope.stable_key.as_str(),
            "ScopeFact.package",
            scope.package,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::Scope,
            scope.stable_key.as_str(),
            "ScopeFact.module",
            scope.module,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.scopes,
            FactFamily::Scope,
            scope.stable_key.as_str(),
            "ScopeFact.parent",
            scope.parent,
        );
    }

    for import in db.semantic_imports() {
        check_semantic_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::SemanticImport,
            import.stable_key.as_str(),
            "SemanticImportFact.file",
            import.file,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::SemanticImport,
            import.stable_key.as_str(),
            "SemanticImportFact.package",
            import.package,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::SemanticImport,
            import.stable_key.as_str(),
            "SemanticImportFact.module",
            import.module,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.scopes,
            FactFamily::SemanticImport,
            import.stable_key.as_str(),
            "SemanticImportFact.scope",
            import.scope,
        );
    }

    for export in db.exports() {
        check_semantic_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::Export,
            export.stable_key.as_str(),
            "ExportFact.file",
            export.file,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::Export,
            export.stable_key.as_str(),
            "ExportFact.package",
            export.package,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::Export,
            export.stable_key.as_str(),
            "ExportFact.module",
            export.module,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.scopes,
            FactFamily::Export,
            export.stable_key.as_str(),
            "ExportFact.scope",
            export.scope,
        );
        if !semantic_status_allows_missing_references(export.status) {
            check_semantic_optional_ref(
                diagnostics,
                &ids.symbols,
                FactFamily::Export,
                export.stable_key.as_str(),
                "ExportFact.symbol",
                export.symbol,
            );
        }
    }

    for alias in db.aliases() {
        check_semantic_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::Alias,
            alias.stable_key.as_str(),
            "AliasFact.file",
            alias.file,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::Alias,
            alias.stable_key.as_str(),
            "AliasFact.package",
            alias.package,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::Alias,
            alias.stable_key.as_str(),
            "AliasFact.module",
            alias.module,
        );
        if !semantic_status_allows_missing_references(alias.status) {
            check_semantic_key_ref(
                diagnostics,
                &semantic_keys,
                FactFamily::Alias,
                alias.stable_key.as_str(),
                "AliasFact.source_symbol_stable_key",
                alias.source_symbol_stable_key.as_str(),
            );
            for target in &alias.target_symbol_stable_keys {
                check_semantic_key_ref(
                    diagnostics,
                    &semantic_keys,
                    FactFamily::Alias,
                    alias.stable_key.as_str(),
                    "AliasFact.target_symbol_stable_keys",
                    target.as_str(),
                );
            }
        }
    }

    for resolution in db.resolution_facts() {
        check_semantic_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::Resolution,
            resolution.stable_key.as_str(),
            "ResolutionFact.file",
            resolution.file,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::Resolution,
            resolution.stable_key.as_str(),
            "ResolutionFact.package",
            resolution.package,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::Resolution,
            resolution.stable_key.as_str(),
            "ResolutionFact.module",
            resolution.module,
        );
        if !semantic_status_allows_missing_references(resolution.status) {
            check_semantic_key_ref(
                diagnostics,
                &semantic_keys,
                FactFamily::Resolution,
                resolution.stable_key.as_str(),
                "ResolutionFact.source_stable_key",
                resolution.source_stable_key.as_str(),
            );
            for target in &resolution.target_stable_keys {
                check_semantic_key_ref(
                    diagnostics,
                    &semantic_keys,
                    FactFamily::Resolution,
                    resolution.stable_key.as_str(),
                    "ResolutionFact.target_stable_keys",
                    target.as_str(),
                );
            }
        }
    }

    for generated in db.generated_symbols() {
        check_semantic_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::GeneratedSymbol,
            generated.stable_key.as_str(),
            "GeneratedSymbolFact.file",
            generated.file,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::GeneratedSymbol,
            generated.stable_key.as_str(),
            "GeneratedSymbolFact.package",
            generated.package,
        );
        check_semantic_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::GeneratedSymbol,
            generated.stable_key.as_str(),
            "GeneratedSymbolFact.module",
            generated.module,
        );
        check_semantic_key_ref(
            diagnostics,
            &semantic_keys,
            FactFamily::GeneratedSymbol,
            generated.stable_key.as_str(),
            "GeneratedSymbolFact.source_stable_key",
            generated.source_stable_key.as_str(),
        );
        if generated.producer_id != SYMBOL_GRAPH_PROVIDER_ID {
            diagnostics.push(semantic_diagnostic(
                FactFamily::GeneratedSymbol,
                generated.stable_key.as_str(),
                format!("GeneratedSymbolFact.producer_id must be {SYMBOL_GRAPH_PROVIDER_ID}"),
            ));
        }
        if generated.generated_discriminator.is_empty() {
            diagnostics.push(semantic_diagnostic(
                FactFamily::GeneratedSymbol,
                generated.stable_key.as_str(),
                "GeneratedSymbolFact.generated_discriminator is empty",
            ));
        }
        if generated.status != SemanticStatus::Generated {
            diagnostics.push(semantic_diagnostic(
                FactFamily::GeneratedSymbol,
                generated.stable_key.as_str(),
                "GeneratedSymbolFact.status must be Generated",
            ));
        }
        if let Some(span) = &generated.span
            && let Some(reason) = span_failure_reason(db, &ids.files, generated.file, span)
        {
            diagnostics.push(semantic_diagnostic(
                FactFamily::GeneratedSymbol,
                generated.stable_key.as_str(),
                format!("GeneratedSymbolFact.span {reason}"),
            ));
        }
    }

    for stable_export in db.stable_exports() {
        check_semantic_ref(
            diagnostics,
            &ids.exports,
            FactFamily::StableExport,
            stable_export.stable_key.as_str(),
            "StableExportIdentity.export",
            stable_export.export,
        );
        if stable_export.status != SemanticStatus::Generated
            && !ids
                .symbol_stable_keys
                .contains(stable_export.symbol_stable_key.as_str())
            && !semantic_keys.contains(stable_export.symbol_stable_key.as_str())
        {
            diagnostics.push(semantic_diagnostic(
                FactFamily::StableExport,
                stable_export.stable_key.as_str(),
                format!(
                    "StableExportIdentity.symbol_stable_key does not exist: {}",
                    stable_export.symbol_stable_key
                ),
            ));
        }
    }

    for (reference, metadata) in db.fact_meta().rows() {
        if is_semantic_fact_family(reference.family)
            && metadata.producer_id == SYMBOL_GRAPH_PROVIDER_ID
            && metadata.precision == FactPrecision::Exact
        {
            diagnostics.push(semantic_diagnostic(
                reference.family,
                metadata.stable_key.as_str(),
                "provider precision ceiling exceeded: semantic rows from polint.symbol_graph are setup-aware, not exact",
            ));
        }
    }
}

fn validate_topology_facts(db: &AnalysisDb, ids: &IdSets, diagnostics: &mut Vec<Diagnostic>) {
    check_topology_family_stable_keys(
        diagnostics,
        FactFamily::WorkspaceRoot,
        db.workspace_roots(),
        |row| row.stable_key.as_str(),
    );
    check_topology_family_stable_keys(
        diagnostics,
        FactFamily::TopologyPackage,
        db.topology_packages(),
        |row| row.stable_key.as_str(),
    );
    check_topology_family_stable_keys(
        diagnostics,
        FactFamily::SourceSet,
        db.source_sets(),
        |row| row.stable_key.as_str(),
    );
    check_topology_family_stable_keys(
        diagnostics,
        FactFamily::DependencyRequirement,
        db.dependency_requirements(),
        |row| row.stable_key.as_str(),
    );
    check_topology_family_stable_keys(
        diagnostics,
        FactFamily::ResolvedDependencyEdge,
        db.resolved_dependency_edges(),
        |row| row.stable_key.as_str(),
    );
    check_topology_family_stable_keys(
        diagnostics,
        FactFamily::ImportToPackage,
        db.import_to_package_edges(),
        |row| row.stable_key.as_str(),
    );
    check_topology_family_stable_keys(
        diagnostics,
        FactFamily::RepoTopologyOverlay,
        db.repo_topology_overlays(),
        |row| row.stable_key.as_str(),
    );

    for root in db.workspace_roots() {
        check_topology_path(
            diagnostics,
            FactFamily::WorkspaceRoot,
            root.stable_key.as_str(),
            "WorkspaceRootFact.root_path",
            root.root_path.as_str(),
        );
        check_topology_optional_path(
            diagnostics,
            FactFamily::WorkspaceRoot,
            root.stable_key.as_str(),
            "WorkspaceRootFact.manifest_path",
            root.manifest_path.as_deref(),
        );
        check_topology_precision(
            diagnostics,
            FactFamily::WorkspaceRoot,
            root.stable_key.as_str(),
            "WorkspaceRootFact.precision",
            root.precision,
            root.status,
            false,
        );
    }

    for package in db.topology_packages() {
        check_topology_optional_ref(
            diagnostics,
            &ids.workspace_roots,
            FactFamily::TopologyPackage,
            package.stable_key.as_str(),
            "TopologyPackageFact.workspace_root",
            package.workspace_root,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.packages,
            FactFamily::TopologyPackage,
            package.stable_key.as_str(),
            "TopologyPackageFact.package",
            package.package,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::TopologyPackage,
            package.stable_key.as_str(),
            "TopologyPackageFact.module_node",
            package.module_node,
        );
        check_topology_path(
            diagnostics,
            FactFamily::TopologyPackage,
            package.stable_key.as_str(),
            "TopologyPackageFact.path",
            package.path.as_str(),
        );
        check_topology_precision(
            diagnostics,
            FactFamily::TopologyPackage,
            package.stable_key.as_str(),
            "TopologyPackageFact.precision",
            package.precision,
            package.status,
            false,
        );
    }

    for source_set in db.source_sets() {
        check_topology_optional_ref(
            diagnostics,
            &ids.topology_packages,
            FactFamily::SourceSet,
            source_set.stable_key.as_str(),
            "SourceSetFact.package",
            source_set.package,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.workspace_roots,
            FactFamily::SourceSet,
            source_set.stable_key.as_str(),
            "SourceSetFact.root",
            source_set.root,
        );
        for file in &source_set.files {
            check_topology_ref(
                diagnostics,
                &ids.files,
                FactFamily::SourceSet,
                source_set.stable_key.as_str(),
                "SourceSetFact.files",
                *file,
            );
        }
        check_topology_path(
            diagnostics,
            FactFamily::SourceSet,
            source_set.stable_key.as_str(),
            "SourceSetFact.path",
            source_set.path.as_str(),
        );
        check_topology_precision(
            diagnostics,
            FactFamily::SourceSet,
            source_set.stable_key.as_str(),
            "SourceSetFact.precision",
            source_set.precision,
            source_set.status,
            false,
        );
    }

    for requirement in db.dependency_requirements() {
        check_topology_optional_ref(
            diagnostics,
            &ids.topology_packages,
            FactFamily::DependencyRequirement,
            requirement.stable_key.as_str(),
            "DependencyRequirementFact.from_package",
            requirement.from_package,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.topology_packages,
            FactFamily::DependencyRequirement,
            requirement.stable_key.as_str(),
            "DependencyRequirementFact.target_package",
            requirement.target_package,
        );
        check_topology_optional_path(
            diagnostics,
            FactFamily::DependencyRequirement,
            requirement.stable_key.as_str(),
            "DependencyRequirementFact.manifest_path",
            requirement.manifest_path.as_deref(),
        );
        check_topology_precision(
            diagnostics,
            FactFamily::DependencyRequirement,
            requirement.stable_key.as_str(),
            "DependencyRequirementFact.precision",
            requirement.precision,
            requirement.status,
            false,
        );
    }

    for edge in db.resolved_dependency_edges() {
        check_topology_optional_ref(
            diagnostics,
            &ids.dependency_requirements,
            FactFamily::ResolvedDependencyEdge,
            edge.stable_key.as_str(),
            "ResolvedDependencyEdgeFact.requirement",
            edge.requirement,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.topology_packages,
            FactFamily::ResolvedDependencyEdge,
            edge.stable_key.as_str(),
            "ResolvedDependencyEdgeFact.from_package",
            edge.from_package,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.topology_packages,
            FactFamily::ResolvedDependencyEdge,
            edge.stable_key.as_str(),
            "ResolvedDependencyEdgeFact.to_package",
            edge.to_package,
        );
        check_resolved_dependency_precision(diagnostics, edge);
    }

    for edge in db.import_to_package_edges() {
        check_topology_optional_ref(
            diagnostics,
            &ids.imports,
            FactFamily::ImportToPackage,
            edge.stable_key.as_str(),
            "ImportToPackageFact.syntax_import",
            edge.syntax_import,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.resolved_imports,
            FactFamily::ImportToPackage,
            edge.stable_key.as_str(),
            "ImportToPackageFact.resolved_import",
            edge.resolved_import,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.files,
            FactFamily::ImportToPackage,
            edge.stable_key.as_str(),
            "ImportToPackageFact.from_file",
            edge.from_file,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.topology_packages,
            FactFamily::ImportToPackage,
            edge.stable_key.as_str(),
            "ImportToPackageFact.from_package",
            edge.from_package,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.topology_packages,
            FactFamily::ImportToPackage,
            edge.stable_key.as_str(),
            "ImportToPackageFact.to_package",
            edge.to_package,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.module_nodes,
            FactFamily::ImportToPackage,
            edge.stable_key.as_str(),
            "ImportToPackageFact.target_node",
            edge.target_node,
        );
        if let Some(key) = edge.semantic_import_stable_key.as_deref()
            && !ids.semantic_import_stable_keys.contains(key)
        {
            diagnostics.push(topology_diagnostic(
                FactFamily::ImportToPackage,
                edge.stable_key.as_str(),
                "ImportToPackageFact.semantic_import_stable_key",
                "semantic_import_stable_key_missing",
            ));
        }
        if edge.status == ImportToPackageStatus::Resolved && edge.to_package_stable_key.is_none() {
            diagnostics.push(topology_diagnostic(
                FactFamily::ImportToPackage,
                edge.stable_key.as_str(),
                "ImportToPackageFact.to_package_stable_key",
                "resolved_row_missing_to_package_stable_key",
            ));
        }
        if edge.status == ImportToPackageStatus::Undeclared
            && declared_requirement_exists(edge, db.dependency_requirements())
        {
            diagnostics.push(topology_diagnostic(
                FactFamily::ImportToPackage,
                edge.stable_key.as_str(),
                "ImportToPackageFact.status",
                "undeclared_row_has_matching_dependency_requirement",
            ));
        }
        check_import_to_package_precision(diagnostics, edge);
    }

    for overlay in db.repo_topology_overlays() {
        check_topology_optional_ref(
            diagnostics,
            &ids.workspace_roots,
            FactFamily::RepoTopologyOverlay,
            overlay.stable_key.as_str(),
            "RepoTopologyOverlayFact.root",
            overlay.root,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.topology_packages,
            FactFamily::RepoTopologyOverlay,
            overlay.stable_key.as_str(),
            "RepoTopologyOverlayFact.package",
            overlay.package,
        );
        check_topology_optional_ref(
            diagnostics,
            &ids.source_sets,
            FactFamily::RepoTopologyOverlay,
            overlay.stable_key.as_str(),
            "RepoTopologyOverlayFact.source_set",
            overlay.source_set,
        );
        check_topology_optional_path(
            diagnostics,
            FactFamily::RepoTopologyOverlay,
            overlay.stable_key.as_str(),
            "RepoTopologyOverlayFact.path",
            overlay.path.as_deref(),
        );
        check_topology_precision(
            diagnostics,
            FactFamily::RepoTopologyOverlay,
            overlay.stable_key.as_str(),
            "RepoTopologyOverlayFact.precision",
            overlay.precision,
            overlay.status,
            false,
        );
    }
}

fn check_topology_family_stable_keys<T: Serialize>(
    diagnostics: &mut Vec<Diagnostic>,
    family: FactFamily,
    rows: &[T],
    stable_key: impl Fn(&T) -> &str,
) {
    let mut seen = BTreeMap::<String, serde_json::Value>::new();
    for row in rows {
        let key = stable_key(row).to_string();
        let normalized = normalized_topology_row(row);
        if let Some(existing) = seen.get(&key) {
            if existing != &normalized {
                diagnostics.push(topology_diagnostic(
                    family,
                    key.as_str(),
                    "stable_key",
                    "stable_key_conflict",
                ));
            } else {
                diagnostics.push(topology_diagnostic(
                    family,
                    key.as_str(),
                    "stable_key",
                    "duplicate_stable_key",
                ));
            }
        } else {
            seen.insert(key, normalized);
        }
    }
}

fn normalized_topology_row<T: Serialize>(row: &T) -> serde_json::Value {
    let mut value = serde_json::to_value(row).unwrap_or(serde_json::Value::Null);
    if let serde_json::Value::Object(object) = &mut value {
        object.remove("id");
    }
    value
}

fn check_topology_ref<T>(
    diagnostics: &mut Vec<Diagnostic>,
    valid_ids: &BTreeSet<T>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    value: T,
) where
    T: Copy + Debug + Ord,
{
    if valid_ids.contains(&value) {
        return;
    }
    diagnostics.push(topology_diagnostic(
        family,
        stable_key,
        field,
        format!("reference_missing:{value:?}"),
    ));
}

fn check_topology_optional_ref<T>(
    diagnostics: &mut Vec<Diagnostic>,
    valid_ids: &BTreeSet<T>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    value: Option<T>,
) where
    T: Copy + Debug + Ord,
{
    let Some(value) = value else {
        return;
    };
    check_topology_ref(diagnostics, valid_ids, family, stable_key, field, value);
}

fn check_topology_path(
    diagnostics: &mut Vec<Diagnostic>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    path: &str,
) {
    if repo_relative_path_is_valid(path) {
        return;
    }
    diagnostics.push(topology_diagnostic(
        family,
        stable_key,
        field,
        "malformed_repo_relative_path",
    ));
}

fn check_topology_optional_path(
    diagnostics: &mut Vec<Diagnostic>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    path: Option<&str>,
) {
    let Some(path) = path else {
        return;
    };
    check_topology_path(diagnostics, family, stable_key, field, path);
}

fn repo_relative_path_is_valid(path: &str) -> bool {
    if path.is_empty() || path == "." {
        return true;
    }
    if path.contains('\\') || path.starts_with('/') || path.contains(':') {
        return false;
    }
    !path.split('/').any(|component| component == "..")
}

fn check_resolved_dependency_precision(
    diagnostics: &mut Vec<Diagnostic>,
    edge: &crate::module_graph::topology::ResolvedDependencyEdgeFact,
) {
    let exact_lockfile_allowed = matches!(
        edge.kind,
        ResolvedDependencyKind::Lockfile
            | ResolvedDependencyKind::LockfileSelected
            | ResolvedDependencyKind::ChecksumEvidence
    );
    if edge.precision == TopologyPrecision::ExactLockfile && !exact_lockfile_allowed {
        diagnostics.push(topology_diagnostic(
            FactFamily::ResolvedDependencyEdge,
            edge.stable_key.as_str(),
            "ResolvedDependencyEdgeFact.precision",
            "exact_lockfile_requires_lockfile_or_checksum_row",
        ));
    }
    check_topology_precision(
        diagnostics,
        FactFamily::ResolvedDependencyEdge,
        edge.stable_key.as_str(),
        "ResolvedDependencyEdgeFact.precision",
        edge.precision,
        edge.status,
        exact_lockfile_allowed,
    );
}

fn check_import_to_package_precision(
    diagnostics: &mut Vec<Diagnostic>,
    edge: &crate::module_graph::topology::ImportToPackageFact,
) {
    if matches!(
        edge.status,
        ImportToPackageStatus::Dynamic
            | ImportToPackageStatus::Unsupported
            | ImportToPackageStatus::SetupMissing
    ) && matches!(
        edge.precision,
        TopologyPrecision::ExactStatic | TopologyPrecision::ExactLockfile
    ) {
        diagnostics.push(topology_diagnostic(
            FactFamily::ImportToPackage,
            edge.stable_key.as_str(),
            "ImportToPackageFact.precision",
            "unsupported_or_dynamic_exactness",
        ));
    }
    if edge.precision == TopologyPrecision::ExactLockfile {
        diagnostics.push(topology_diagnostic(
            FactFamily::ImportToPackage,
            edge.stable_key.as_str(),
            "ImportToPackageFact.precision",
            "exact_lockfile_requires_lockfile_or_checksum_row",
        ));
    }
}

fn check_topology_precision(
    diagnostics: &mut Vec<Diagnostic>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    precision: TopologyPrecision,
    status: TopologyStatus,
    exact_lockfile_allowed: bool,
) {
    if precision == TopologyPrecision::ExactLockfile && !exact_lockfile_allowed {
        diagnostics.push(topology_diagnostic(
            family,
            stable_key,
            field,
            "exact_lockfile_requires_lockfile_or_checksum_row",
        ));
    }
    if status == TopologyStatus::Unsupported
        && matches!(
            precision,
            TopologyPrecision::ExactStatic | TopologyPrecision::ExactLockfile
        )
    {
        diagnostics.push(topology_diagnostic(
            family,
            stable_key,
            field,
            "unsupported_or_dynamic_exactness",
        ));
    }
}

fn declared_requirement_exists(
    edge: &crate::module_graph::topology::ImportToPackageFact,
    requirements: &[crate::module_graph::topology::DependencyRequirementFact],
) -> bool {
    let target = external_package_name(&edge.import_path);
    requirements.iter().any(|requirement| {
        requirement.target_name == target
            && edge
                .from_package
                .is_none_or(|package| requirement.from_package == Some(package))
    })
}

fn external_package_name(path: &str) -> &str {
    if let Some(stripped) = path.strip_prefix('@') {
        let mut parts = stripped.split('/');
        let Some(scope_name) = parts.next() else {
            return path;
        };
        let Some(package_name) = parts.next() else {
            return path;
        };
        let end = 1 + scope_name.len() + 1 + package_name.len();
        &path[..end]
    } else {
        path.split('/').next().unwrap_or(path)
    }
}

fn topology_diagnostic(
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    reason: impl Into<String>,
) -> Diagnostic {
    internal_diagnostic(format!(
        "Topology validation failed for {} stable key.",
        family.label()
    ))
    .with_evidence("family", family.label())
    .with_evidence("stable_key", stable_key.to_string())
    .with_evidence("field", field)
    .with_evidence("reason", reason.into())
}

fn is_semantic_fact_family(family: FactFamily) -> bool {
    matches!(
        family,
        FactFamily::Scope
            | FactFamily::SemanticImport
            | FactFamily::Export
            | FactFamily::Alias
            | FactFamily::Resolution
            | FactFamily::GeneratedSymbol
            | FactFamily::StableExport
    )
}

fn semantic_reference_keys(db: &AnalysisDb, ids: &IdSets) -> BTreeSet<String> {
    let mut keys = ids.symbol_stable_keys.clone();
    keys.extend(ids.reference_stable_keys.iter().cloned());
    for scope in db.scopes() {
        keys.insert(scope.stable_key.clone());
    }
    for import in db.semantic_imports() {
        keys.insert(import.stable_key.clone());
    }
    for export in db.exports() {
        keys.insert(export.stable_key.clone());
    }
    for alias in db.aliases() {
        keys.insert(alias.stable_key.clone());
    }
    for resolution in db.resolution_facts() {
        keys.insert(resolution.stable_key.clone());
    }
    for generated in db.generated_symbols() {
        keys.insert(generated.stable_key.clone());
    }
    for stable_export in db.stable_exports() {
        keys.insert(stable_export.stable_key.clone());
    }
    keys
}

fn semantic_status_allows_missing_references(status: SemanticStatus) -> bool {
    matches!(
        status,
        SemanticStatus::Generated
            | SemanticStatus::External
            | SemanticStatus::Unresolved
            | SemanticStatus::Dynamic
            | SemanticStatus::SetupMissing
            | SemanticStatus::Unsupported
    )
}

fn check_semantic_ref<T>(
    diagnostics: &mut Vec<Diagnostic>,
    valid_ids: &BTreeSet<T>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    value: T,
) where
    T: Copy + Debug + Ord,
{
    if valid_ids.contains(&value) {
        return;
    }
    diagnostics.push(semantic_diagnostic(
        family,
        stable_key,
        format!("{field} does not exist: {value:?}"),
    ));
}

fn check_semantic_optional_ref<T>(
    diagnostics: &mut Vec<Diagnostic>,
    valid_ids: &BTreeSet<T>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    value: Option<T>,
) where
    T: Copy + Debug + Ord,
{
    let Some(value) = value else {
        return;
    };
    check_semantic_ref(diagnostics, valid_ids, family, stable_key, field, value);
}

fn check_semantic_key_ref(
    diagnostics: &mut Vec<Diagnostic>,
    valid_keys: &BTreeSet<String>,
    family: FactFamily,
    stable_key: &str,
    field: &'static str,
    value: &str,
) {
    if !value.is_empty() && valid_keys.contains(value) {
        return;
    }
    diagnostics.push(semantic_diagnostic(
        family,
        stable_key,
        format!("{field} does not exist: {value}"),
    ));
}

fn semantic_diagnostic(
    family: FactFamily,
    stable_key: &str,
    reason: impl Into<String>,
) -> Diagnostic {
    let (family_label, stable_key_label, reason_label) = SEMANTIC_EVIDENCE_ORDER;
    internal_diagnostic(format!(
        "Semantic index validation failed for {} stable key.",
        family.label()
    ))
    .with_evidence(family_label, family.label())
    .with_evidence(stable_key_label, stable_key.to_string())
    .with_evidence(reason_label, reason.into())
}

fn check_ref<T>(
    diagnostics: &mut Vec<Diagnostic>,
    valid_ids: &BTreeSet<T>,
    family: FactFamily,
    run_id: u64,
    field: &'static str,
    value: T,
) where
    T: Copy + Debug + Ord,
{
    if valid_ids.contains(&value) {
        return;
    }
    diagnostics.push(reference_diagnostic(family, run_id, field, value));
}

fn check_optional_ref<T>(
    diagnostics: &mut Vec<Diagnostic>,
    valid_ids: &BTreeSet<T>,
    family: FactFamily,
    run_id: u64,
    field: &'static str,
    value: Option<T>,
) where
    T: Copy + Debug + Ord,
{
    let Some(value) = value else {
        return;
    };
    check_ref(diagnostics, valid_ids, family, run_id, field, value);
}

fn reference_diagnostic<T: Debug>(
    family: FactFamily,
    run_id: u64,
    field: &'static str,
    value: T,
) -> Diagnostic {
    internal_diagnostic(format!(
        "Fact metadata reference validation failed for {}#{}.",
        family.label(),
        run_id
    ))
    .with_evidence("family", family.label())
    .with_evidence("fact_ref", fact_ref_value(FactRef::new(family, run_id)))
    .with_evidence("field", field)
    .with_evidence("value", format!("{value:?}"))
}

fn provider_manifest_diagnostic(
    reference: FactRef,
    field: &'static str,
    value: &'static str,
) -> Diagnostic {
    internal_diagnostic(format!(
        "Fact metadata provider manifest missing for {}#{}.",
        reference.family.label(),
        reference.run_id
    ))
    .with_evidence("family", reference.family.label())
    .with_evidence("fact_ref", fact_ref_value(reference))
    .with_evidence("field", field)
    .with_evidence("value", value)
}

struct SpanCheck<'a> {
    family: FactFamily,
    run_id: u64,
    field: &'static str,
    owner_file: Option<FileId>,
    span: &'a Span,
}

fn check_span(
    db: &AnalysisDb,
    file_ids: &BTreeSet<FileId>,
    diagnostics: &mut Vec<Diagnostic>,
    check: SpanCheck<'_>,
) {
    let Some(reason) = span_failure_reason(db, file_ids, check.owner_file, check.span) else {
        return;
    };
    diagnostics.push(
        internal_diagnostic(format!(
            "Fact metadata span validation failed for {}#{}.",
            check.family.label(),
            check.run_id
        ))
        .with_evidence("family", check.family.label())
        .with_evidence(
            "fact_ref",
            fact_ref_value(FactRef::new(check.family, check.run_id)),
        )
        .with_evidence("field", check.field)
        .with_evidence("reason", reason)
        .with_evidence("span_file", format!("{:?}", check.span.file))
        .with_evidence("owner_file", owner_file_value(check.owner_file))
        .with_evidence("start_byte", check.span.start_byte.to_string())
        .with_evidence("end_byte", check.span.end_byte.to_string()),
    );
}

fn span_failure_reason(
    db: &AnalysisDb,
    file_ids: &BTreeSet<FileId>,
    owner_file: Option<FileId>,
    span: &Span,
) -> Option<String> {
    if !file_ids.contains(&span.file) {
        return Some("span file does not exist".to_string());
    }
    if let Some(owner_file) = owner_file
        && owner_file != span.file
    {
        return Some("span file does not match owning file".to_string());
    }
    if span.start_byte > span.end_byte {
        return Some("start_byte exceeds end_byte".to_string());
    }
    let source_len = db.file(span.file).map(|file| file.source.len() as u32)?;
    if span.end_byte > source_len {
        return Some(format!("end_byte exceeds source length {source_len}"));
    }
    None
}

fn precision_within_ceiling(precision: FactPrecision, ceiling: PrecisionCeiling) -> bool {
    match ceiling {
        PrecisionCeiling::Exact => true,
        PrecisionCeiling::Syntax => matches!(
            precision,
            FactPrecision::Syntax
                | FactPrecision::Heuristic
                | FactPrecision::Unresolved
                | FactPrecision::Ambiguous
                | FactPrecision::SetupMissing
                | FactPrecision::Unsupported
        ),
        PrecisionCeiling::SetupAware => true,
    }
}

fn internal_diagnostic(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        message,
    )
}

fn fact_ref_value(reference: FactRef) -> String {
    format!("{}#{}", reference.family.label(), reference.run_id)
}

fn owner_file_value(owner_file: Option<FileId>) -> String {
    owner_file.map_or_else(|| "none".to_string(), |file| format!("{file:?}"))
}

fn precision_label(precision: FactPrecision) -> &'static str {
    match precision {
        FactPrecision::Exact => "exact",
        FactPrecision::Syntax => "syntax",
        FactPrecision::SetupAware => "setup_aware",
        FactPrecision::Heuristic => "heuristic",
        FactPrecision::Unresolved => "unresolved",
        FactPrecision::Ambiguous => "ambiguous",
        FactPrecision::SetupMissing => "setup_missing",
        FactPrecision::Unsupported => "unsupported",
    }
}

fn ceiling_label(ceiling: PrecisionCeiling) -> &'static str {
    match ceiling {
        PrecisionCeiling::Exact => "exact",
        PrecisionCeiling::Syntax => "syntax",
        PrecisionCeiling::SetupAware => "setup_aware",
    }
}

fn diagnostic_order(left: &Diagnostic, right: &Diagnostic) -> std::cmp::Ordering {
    (
        left.rule_id.as_str(),
        left.file.as_str(),
        left.range.start_line,
        left.range.start_col,
        left.message.as_str(),
        evidence_order_key(left),
        left.stable_fingerprint.as_str(),
    )
        .cmp(&(
            right.rule_id.as_str(),
            right.file.as_str(),
            right.range.start_line,
            right.range.start_col,
            right.message.as_str(),
            evidence_order_key(right),
            right.stable_fingerprint.as_str(),
        ))
}

fn evidence_order_key(diagnostic: &Diagnostic) -> String {
    diagnostic
        .evidence
        .iter()
        .map(|evidence| format!("{}={}", evidence.label, evidence.value))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::validate_fact_metadata;
    use crate::analysis_kernel::{
        AnalysisKernel, FactConfidence, FactFamily, FactMeta, FactPrecision, FactRef,
        ValidationStatus,
    };
    use crate::core::{
        AnalysisDb, BranchId, BranchObligation, ComplexityMetricFact, FileId, FileMetricFact,
        FunctionFact, FunctionId, FunctionMetricFact, Language, ModuleNode, ModuleNodeId,
        ModuleNodeKind, PackageId, Span, TestFact, TsComponentFact,
    };
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[test]
    fn metadata_validation_conflict_records_render_internal_diagnostics_with_evidence() {
        let mut db = AnalysisDb::new();
        let existing = FactRef::new(FactFamily::Import, 1);
        let incoming = FactRef::new(FactFamily::Import, 2);

        db.fact_meta_mut_for_test().insert(
            existing,
            test_meta(FactFamily::Import, "import:key", "payload:a"),
        );
        db.fact_meta_mut_for_test().insert(
            incoming,
            test_meta(FactFamily::Import, "import:key", "payload:b"),
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "polint/internal");
        assert!(
            diagnostics[0]
                .message
                .starts_with("Fact metadata stable key conflict detected")
        );
        assert_eq!(
            evidence_labels(&diagnostics[0]),
            BTreeSet::from(["existing_ref", "family", "incoming_ref", "stable_key",])
        );
    }

    #[test]
    fn metadata_validation_span_failures_are_reported_deterministically() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "abc".to_string(),
        );
        let other_file = db.add_file(
            PathBuf::from("src/other.ts"),
            "src/other.ts".to_string(),
            "abcdef".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "too_long".to_string(),
            span: span(file, 0, 4),
            language: Language::TypeScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "reversed".to_string(),
            span: span(file, 2, 1),
            language: Language::TypeScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "wrong_file".to_string(),
            span: span(other_file, 0, 1),
            language: Language::TypeScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let messages = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();

        assert_eq!(messages.len(), 3);
        assert!(
            messages
                .iter()
                .all(|message| message.starts_with("Fact metadata span validation failed"))
        );
    }

    #[test]
    fn metadata_validation_reference_failures_cover_current_focused_fields() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.tsx"),
            "src/app.tsx".to_string(),
            "export function Button() { return null; }\n".to_string(),
        );
        db.push_branch(BranchObligation {
            id: BranchId(99),
            function: Some(FunctionId(404)),
            file,
            decision_span: span(file, 0, 1),
            condition_text: "enabled".to_string(),
            edge_label: "true".to_string(),
            is_error_path: false,
            stable_fingerprint: "branch:key".to_string(),
        });
        db.push_test(TestFact {
            file,
            function: Some(FunctionId(405)),
            name: "TestButton".to_string(),
            span: span(file, 0, 1),
            evidence_terms: Vec::new(),
            assertion_count: 0,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_ts_component(TsComponentFact {
            file,
            function: Some(FunctionId(406)),
            name: "Button".to_string(),
            span: span(file, 0, 1),
        });
        db.replace_module_graph_facts(
            Vec::new(),
            vec![ModuleNode {
                id: ModuleNodeId(99),
                kind: ModuleNodeKind::File,
                label: "missing".to_string(),
                file: Some(FileId(404)),
                package: Some(PackageId(405)),
                language: Some(Language::Tsx),
            }],
            Vec::new(),
        );
        db.replace_metric_facts(
            vec![FileMetricFact {
                file: FileId(406),
                language: Language::Tsx,
                line_count: 1,
                non_empty_line_count: 1,
                byte_count: 1,
                function_count: 0,
            }],
            vec![FunctionMetricFact {
                function: FunctionId(407),
                file: FileId(407),
                name: "Button".to_string(),
                span: span(file, 0, 1),
                language: Language::Tsx,
                line_count: 1,
                byte_count: 1,
            }],
            vec![ComplexityMetricFact {
                function: FunctionId(408),
                file: FileId(408),
                name: "Button".to_string(),
                span: span(file, 0, 1),
                language: Language::Tsx,
                cyclomatic_complexity: 1,
            }],
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let evidence_values = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .message
                    .starts_with("Fact metadata reference validation failed")
            })
            .flat_map(|diagnostic| {
                diagnostic
                    .evidence
                    .iter()
                    .filter(|evidence| evidence.label == "field")
                    .map(|evidence| evidence.value.as_str())
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(
            evidence_values,
            BTreeSet::from([
                "BranchObligation.function",
                "ComplexityMetricFact.file",
                "ComplexityMetricFact.function",
                "FileMetricFact.file",
                "FunctionMetricFact.file",
                "FunctionMetricFact.function",
                "ModuleNode.file",
                "ModuleNode.package",
                "TestFact.function",
                "TsComponentFact.function",
            ])
        );
    }

    #[test]
    fn metadata_validation_precision_ceiling_violations_name_provider_family_and_precision() {
        let mut db = AnalysisDb::new();
        db.fact_meta_mut_for_test().insert(
            FactRef::new(FactFamily::FileMetric, 0),
            FactMeta {
                stable_key: "metric:key".to_string(),
                producer_id: "polint.metrics",
                layer_id: "polint.metrics",
                precision: FactPrecision::Exact,
                confidence: FactConfidence::High,
                validation: ValidationStatus::NativeTrusted,
                payload_digest: "payload:a".to_string(),
            },
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .starts_with("Fact metadata precision ceiling violated")
        );
        assert_eq!(
            evidence_labels(&diagnostics[0]),
            BTreeSet::from(["ceiling", "family", "precision", "producer_id"])
        );
    }

    #[test]
    fn metadata_validation_reports_unknown_producer_and_layer_ids() {
        let mut db = AnalysisDb::new();
        db.fact_meta_mut_for_test().insert(
            FactRef::new(FactFamily::FileMetric, 0),
            FactMeta {
                stable_key: "metric:key".to_string(),
                producer_id: "polint.unknown_producer",
                layer_id: "polint.unknown_layer",
                precision: FactPrecision::Syntax,
                confidence: FactConfidence::High,
                validation: ValidationStatus::NativeTrusted,
                payload_digest: "payload:a".to_string(),
            },
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let provider_fields = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .message
                    .starts_with("Fact metadata provider manifest missing")
            })
            .flat_map(|diagnostic| {
                diagnostic
                    .evidence
                    .iter()
                    .filter(|evidence| evidence.label == "field")
                    .map(|evidence| evidence.value.as_str())
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(provider_fields, BTreeSet::from(["layer_id", "producer_id"]));
    }

    fn test_meta(family: FactFamily, stable_key: &str, payload_digest: &str) -> FactMeta {
        FactMeta {
            stable_key: stable_key.to_string(),
            producer_id: match family {
                FactFamily::SourceFile => "polint.source",
                FactFamily::FileMetric => "polint.metrics",
                _ => "polint.go.syntax",
            },
            layer_id: "polint.go.syntax",
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: payload_digest.to_string(),
        }
    }

    fn span(file: FileId, start_byte: u32, end_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte,
            start_line: 1,
            start_col: start_byte + 1,
            end_line: 1,
            end_col: end_byte + 1,
        }
    }

    fn evidence_labels(diagnostic: &crate::diagnostics::Diagnostic) -> BTreeSet<&str> {
        diagnostic
            .evidence
            .iter()
            .map(|evidence| evidence.label.as_str())
            .collect()
    }
}
