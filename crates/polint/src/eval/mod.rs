#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 22 plan 01 defines the internal eval schema before later harness plans consume it"
    )
)]

pub(crate) mod fixtures;
pub(crate) mod matcher;
pub(crate) mod metrics;
pub(crate) mod model;
pub(crate) mod observed;
pub(crate) mod report;

#[cfg(test)]
mod semantic_rows {
    use crate::eval::matcher::{MatchOutcome, MatcherConfig, match_case};
    use crate::eval::model::{
        AssertionMode, ExpectedFact, ExpectedItem, ObservedFact, ObservedItem, ObservedStatus,
    };
    use serde_json::json;

    #[test]
    fn eval_expected_facts_accept_semantic_families_and_statuses() {
        let cases = crate::eval::model::SEMANTIC_FACT_FAMILIES
            .iter()
            .copied()
            .zip([
                "resolved",
                "dynamic",
                "resolved",
                "ambiguous",
                "unresolved",
                "generated",
                "external",
            ]);

        for (family, status) in cases {
            let manifest = format!(
                r#"
schema_version = "polint-eval-fixture-1"
case_id = "semantic-row-model"
area = "facts"

[repo]
path = "repo"

[[expected]]
fact = {{ family = "{family}", stable_key = "semantic:{family}", mode = "partial", producer_id = "polint.symbol_graph", precision = "setup_aware", status = "{status}" }}
"#
            );
            let parsed: crate::eval::fixtures::NativeFixtureManifest =
                toml::from_str(&manifest).expect("semantic expected fact should parse");
            assert_eq!(parsed.expected.len(), 1);
        }
    }

    #[test]
    fn observed_semantic_debug_rows_normalize_to_fact_rows_with_evidence() {
        let debug = json!({
            "semantic": {
                "scopes": [{
                    "family": "Scope",
                    "stable_key": "scope:src/app.ts:module",
                    "producer_id": "polint.symbol_graph",
                    "layer_id": "polint.symbol_graph",
                    "status": "resolved",
                    "metadata": { "precision": "setup_aware" },
                    "path": "src/app.ts",
                    "span": {
                        "start_byte": 0,
                        "end_byte": 12,
                        "start_line": 1,
                        "start_col": 1,
                        "end_line": 1,
                        "end_col": 13
                    }
                }],
                "imports": [],
                "exports": [],
                "aliases": [],
                "resolutions": [],
                "generated_symbols": [],
                "stable_exports": []
            }
        });

        let observed = crate::eval::observed::metadata_debug_facts_for_test(&debug);

        assert!(observed.iter().any(|item| match item {
            ObservedItem::Fact(fact) => {
                fact.family == "Scope"
                    && fact.stable_key == "scope:src/app.ts:module"
                    && fact.producer_id.as_deref() == Some("polint.symbol_graph")
                    && fact.provenance.as_deref() == Some("polint.symbol_graph")
                    && fact.precision.as_deref() == Some("setup_aware")
                    && fact.status == Some(ObservedStatus::Resolved)
                    && fact.payload.as_deref().is_some_and(|payload| {
                        payload.contains("\"path\":\"src/app.ts\"")
                            && payload.contains("\"start_byte\":0")
                    })
            }
            _ => false,
        }));
    }

    #[test]
    fn semantic_partial_and_unknown_status_matching_works() {
        let summaries = match_case(
            &[ExpectedItem::Fact(ExpectedFact {
                family: "Resolution".to_string(),
                stable_key: "handler".to_string(),
                mode: AssertionMode::Partial,
                producer_id: Some("polint.symbol_graph".to_string()),
                precision: Some("ambiguous".to_string()),
                status: Some(ObservedStatus::Ambiguous),
                false_positive_trap: false,
            })],
            &[ObservedItem::Fact(ObservedFact {
                family: "Resolution".to_string(),
                stable_key: "resolution:src/app.ts:handler:candidates".to_string(),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.symbol_graph".to_string()),
                provenance: Some("polint.symbol_graph".to_string()),
                precision: Some("ambiguous".to_string()),
                status: Some(ObservedStatus::Ambiguous),
                payload: None,
            })],
            MatcherConfig::default(),
        );

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].outcome, MatchOutcome::Unknown);
    }
}

#[cfg(test)]
mod topology_rows {
    use crate::core::{AnalysisDb, FileId};
    use crate::eval::matcher::{MatchOutcome, MatcherConfig, match_case};
    use crate::eval::model::{
        AssertionMode, ExpectedFact, ExpectedItem, FixtureArea, ObservedFact, ObservedItem,
        ObservedStatus, TOPOLOGY_FACT_FAMILIES,
    };
    use crate::module_graph::topology::{
        DependencyRequirementFact, DependencyRequirementId, ImportContextKind, ImportToPackageFact,
        ImportToPackageId, ImportToPackageStatus, RepoTopologyOverlayFact, RepoTopologyOverlayId,
        RepoTopologyOverlayKind, RequirementKind, ResolvedDependencyEdgeFact,
        ResolvedDependencyEdgeId, ResolvedDependencyKind, SourceSetFact, SourceSetId,
        SourceSetKind, TopologyOutput, TopologyPackageFact, TopologyPackageId, TopologyPackageKind,
        TopologyPrecision, TopologyStatus, WorkspaceRootFact, WorkspaceRootId, WorkspaceRootKind,
    };

    #[test]
    fn eval_expected_facts_accept_topology_families_and_statuses() {
        let families = TOPOLOGY_FACT_FAMILIES.join(",");
        assert!(families.contains("WorkspaceRoot"));
        assert!(families.contains("TopologyPackage"));
        assert!(families.contains("SourceSet"));
        assert!(families.contains("DependencyRequirement"));
        assert!(families.contains("ResolvedDependencyEdge"));
        assert!(families.contains("ImportToPackage"));
        assert!(families.contains("RepoTopologyOverlay"));

        let manifest = r#"
schema_version = "polint-eval-fixture-1"
case_id = "topology-row-model"
area = "module-topology"

[repo]
path = "repo"

[[expected]]
fact = { family = "ImportToPackage", stable_key = "import:outside", mode = "partial", producer_id = "polint.module_topology", precision = "heuristic", status = "outside_workspace" }

[[expected]]
fact = { family = "DependencyRequirement", stable_key = "requirement:undeclared", mode = "partial", producer_id = "polint.module_graph", precision = "heuristic", status = "undeclared" }

[[expected]]
fact = { family = "ResolvedDependencyEdge", stable_key = "edge:missing-lockfile", mode = "partial", producer_id = "polint.module_graph", precision = "unknown", status = "missing_lockfile" }
"#;

        let parsed: crate::eval::fixtures::NativeFixtureManifest =
            toml::from_str(manifest).expect("topology expected facts should parse");

        assert_eq!(parsed.area, FixtureArea::ModuleTopology);
        assert_eq!(parsed.expected.len(), 3);
    }

    #[test]
    fn observed_topology_rows_normalize_to_fact_rows_with_compact_payloads() {
        let mut db = AnalysisDb::new();
        db.replace_topology_facts(topology_output());

        let observed = crate::eval::observed::topology_facts_for_test(&db);

        for family in TOPOLOGY_FACT_FAMILIES {
            assert!(
                observed.iter().any(|item| matches!(
                    item,
                    ObservedItem::Fact(fact) if fact.family == *family
                )),
                "missing observed topology family {family}"
            );
        }
        assert!(observed.iter().any(|item| match item {
            ObservedItem::Fact(fact) => {
                fact.family == "ImportToPackage"
                    && fact.stable_key == "import:outside-workspace"
                    && fact.producer_id.as_deref() == Some("polint.module_topology")
                    && fact.precision.as_deref() == Some("heuristic")
                    && fact.status == Some(ObservedStatus::OutsideWorkspace)
                    && fact.payload.as_deref().is_some_and(|payload| {
                        payload.contains("source_set:set:source")
                            && payload.contains("from:pkg:web")
                            && !payload.contains(env!("CARGO_MANIFEST_DIR"))
                    })
            }
            _ => false,
        }));
    }

    #[test]
    fn topology_unknown_like_statuses_match_as_unknown_metrics() {
        let statuses = [
            ObservedStatus::MissingLockfile,
            ObservedStatus::Unsupported,
            ObservedStatus::Dynamic,
            ObservedStatus::Ambiguous,
            ObservedStatus::Undeclared,
            ObservedStatus::OutsideWorkspace,
        ];

        for status in statuses {
            let summaries = match_case(
                &[ExpectedItem::Fact(ExpectedFact {
                    family: "ImportToPackage".to_string(),
                    stable_key: "import".to_string(),
                    mode: AssertionMode::Partial,
                    producer_id: Some("polint.module_topology".to_string()),
                    precision: None,
                    status: Some(status),
                    false_positive_trap: false,
                })],
                &[ObservedItem::Fact(ObservedFact {
                    family: "ImportToPackage".to_string(),
                    stable_key: "import:row".to_string(),
                    mode: AssertionMode::Exact,
                    producer_id: Some("polint.module_topology".to_string()),
                    provenance: Some("polint.module_topology".to_string()),
                    precision: Some("heuristic".to_string()),
                    status: Some(status),
                    payload: None,
                })],
                MatcherConfig::default(),
            );

            assert_eq!(summaries[0].outcome, MatchOutcome::Unknown);
        }
    }

    fn topology_output() -> TopologyOutput {
        TopologyOutput {
            workspace_roots: vec![WorkspaceRootFact {
                id: WorkspaceRootId(0),
                kind: WorkspaceRootKind::JsWorkspace,
                root_path: "web".to_string(),
                manifest_path: Some("web/package.json".to_string()),
                language: None,
                stable_key: "root:web".to_string(),
                producer_id: "polint.module_graph",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Resolved,
            }],
            packages: vec![TopologyPackageFact {
                id: TopologyPackageId(0),
                workspace_root: Some(WorkspaceRootId(0)),
                package: None,
                module_node: None,
                kind: TopologyPackageKind::JsPackage,
                name: "web".to_string(),
                version: Some("1.0.0".to_string()),
                path: "web".to_string(),
                language: None,
                stable_key: "pkg:web".to_string(),
                producer_id: "polint.module_graph",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Resolved,
            }],
            source_sets: vec![SourceSetFact {
                id: SourceSetId(0),
                package: Some(TopologyPackageId(0)),
                root: Some(WorkspaceRootId(0)),
                kind: SourceSetKind::Source,
                path: "web/src/app.ts".to_string(),
                language: None,
                files: vec![FileId(0)],
                stable_key: "set:source".to_string(),
                producer_id: "polint.module_graph",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Present,
            }],
            dependency_requirements: vec![DependencyRequirementFact {
                id: DependencyRequirementId(0),
                from_package: Some(TopologyPackageId(0)),
                target_package: None,
                target_name: "@scope/lib".to_string(),
                version_requirement: Some("^1.0.0".to_string()),
                kind: RequirementKind::Runtime,
                manifest_path: Some("web/package.json".to_string()),
                stable_key: "requirement:@scope/lib".to_string(),
                producer_id: "polint.module_graph",
                precision: TopologyPrecision::ExactStatic,
                status: TopologyStatus::Present,
            }],
            resolved_dependency_edges: vec![ResolvedDependencyEdgeFact {
                id: ResolvedDependencyEdgeId(0),
                requirement: Some(DependencyRequirementId(0)),
                from_package: Some(TopologyPackageId(0)),
                to_package: None,
                package_name: "@scope/lib".to_string(),
                resolved_version: None,
                kind: ResolvedDependencyKind::External,
                stable_key: "edge:@scope/lib:missing-lockfile".to_string(),
                producer_id: "polint.module_graph",
                precision: TopologyPrecision::Unknown,
                status: TopologyStatus::MissingLockfile,
            }],
            import_to_package_edges: vec![ImportToPackageFact {
                id: ImportToPackageId(0),
                syntax_import: None,
                resolved_import: None,
                semantic_import_stable_key: Some("semantic:import:@scope/lib".to_string()),
                from_file: Some(FileId(0)),
                from_package: Some(TopologyPackageId(0)),
                to_package: None,
                target_node: None,
                from_package_stable_key: Some("pkg:web".to_string()),
                to_package_stable_key: None,
                source_set_stable_key: Some("set:source".to_string()),
                import_path: "@scope/lib".to_string(),
                context: ImportContextKind::Source,
                stable_key: "import:outside-workspace".to_string(),
                producer_id: "polint.module_topology",
                precision: TopologyPrecision::Heuristic,
                status: ImportToPackageStatus::OutsideWorkspace,
            }],
            overlays: vec![RepoTopologyOverlayFact {
                id: RepoTopologyOverlayId(0),
                root: Some(WorkspaceRootId(0)),
                package: Some(TopologyPackageId(0)),
                source_set: Some(SourceSetId(0)),
                kind: RepoTopologyOverlayKind::GeneratedZone,
                label: "generated".to_string(),
                path: Some("web/generated".to_string()),
                stable_key: "overlay:generated".to_string(),
                producer_id: "polint.module_graph",
                precision: TopologyPrecision::Heuristic,
                status: TopologyStatus::Present,
            }],
        }
    }
}

#[cfg(test)]
mod semantic_mir_rows {
    use crate::eval::matcher::{MatchOutcome, MatcherConfig, match_case};
    use crate::eval::model::{
        AssertionMode, ExpectedFact, ExpectedItem, FixtureArea, ObservedFact, ObservedItem,
        ObservedStatus, SEMANTIC_MIR_FACT_FAMILIES,
    };
    use serde_json::json;

    #[test]
    fn eval_expected_facts_accept_semantic_mir_families_and_partial_status() {
        assert_eq!(
            SEMANTIC_MIR_FACT_FAMILIES,
            ["MirBody", "MirOperation", "Place", "UnsupportedSemantic"]
        );

        let manifest = r#"
schema_version = "polint-eval-fixture-1"
case_id = "semantic-mir-row-model"
area = "semantic-mir"

[repo]
path = "repo"

[[expected]]
fact = { family = "MirBody", stable_key = "body:handler", mode = "partial", producer_id = "polint.semantic_mir", precision = "setup_aware", status = "resolved" }

[[expected]]
fact = { family = "MirOperation", stable_key = "op:branch", mode = "partial", producer_id = "polint.semantic_mir", precision = "heuristic", status = "partial" }

[[expected]]
fact = { family = "Place", stable_key = "root_kind:parameter", mode = "partial", producer_id = "polint.semantic_mir", precision = "setup_aware", status = "unknown" }

[[expected]]
fact = { family = "UnsupportedSemantic", stable_key = "unsupported:eval", mode = "partial", producer_id = "polint.semantic_mir", precision = "unsupported", status = "unsupported" }
"#;

        let parsed: crate::eval::fixtures::NativeFixtureManifest =
            toml::from_str(manifest).expect("semantic MIR expected facts should parse");

        assert_eq!(parsed.area, FixtureArea::SemanticMir);
        assert_eq!(parsed.expected.len(), 4);
    }

    #[test]
    fn observed_semantic_mir_debug_rows_normalize_to_fact_rows_with_compact_payloads() {
        let debug = json!({
            "mir": {
                "bodies": [{
                    "family": "MirBody",
                    "stable_key": "mir-body:src/app.ts:handler",
                    "producer_id": "polint.semantic_mir",
                    "layer_id": "polint.semantic_mir",
                    "status": "resolved",
                    "precision": "setup_aware",
                    "path": "src/app.ts",
                    "span": {
                        "start_byte": 0,
                        "end_byte": 16,
                        "start_line": 1,
                        "start_col": 1,
                        "end_line": 1,
                        "end_col": 17
                    },
                    "owner_function": 7
                }],
                "operations": [{
                    "family": "MirOperation",
                    "stable_key": "mir-op:src/app.ts:handler:branch",
                    "producer_id": "polint.semantic_mir",
                    "layer_id": "polint.semantic_mir",
                    "status": "partial",
                    "precision": "heuristic",
                    "path": "src/app.ts",
                    "span": {
                        "start_byte": 20,
                        "end_byte": 32,
                        "start_line": 2,
                        "start_col": 3,
                        "end_line": 2,
                        "end_col": 15
                    },
                    "owner_function": 7,
                    "operation_kind": "branch"
                }],
                "places": [{
                    "family": "Place",
                    "stable_key": "place:src/app.ts:handler:parameter:0:user",
                    "producer_id": "polint.semantic_mir",
                    "layer_id": "polint.semantic_mir",
                    "status": "unknown",
                    "precision": "setup_aware",
                    "path": "src/app.ts",
                    "owner_function": 7,
                    "place_root": "parameter:0:user",
                    "place_projections": ["property:name", "index_known:0"]
                }],
                "unsupported": [{
                    "family": "UnsupportedSemantic",
                    "stable_key": "unsupported:src/app.ts:eval",
                    "producer_id": "polint.semantic_mir",
                    "layer_id": "polint.semantic_mir",
                    "status": "unsupported",
                    "precision": "unsupported",
                    "path": "src/app.ts",
                    "span": {
                        "start_byte": 44,
                        "end_byte": 55,
                        "start_line": 4,
                        "start_col": 5,
                        "end_line": 4,
                        "end_col": 16
                    },
                    "owner_function": 7,
                    "unsupported_construct": "eval",
                    "conservative_action": "havoc_affected_places"
                }]
            }
        });

        let observed = crate::eval::observed::semantic_mir_facts_for_test(&debug);

        for family in SEMANTIC_MIR_FACT_FAMILIES {
            assert!(
                observed.iter().any(|item| matches!(
                    item,
                    ObservedItem::Fact(fact) if fact.family == *family
                )),
                "missing semantic MIR family {family}: {observed:#?}"
            );
        }
        let rendered = serde_json::to_string(&observed).unwrap();
        assert!(rendered.contains("path=src/app.ts"));
        assert!(rendered.contains("span=1:1-1:17"));
        assert!(rendered.contains("owner=7"));
        assert!(rendered.contains("kind=branch"));
        assert!(rendered.contains("root=parameter:0:user"));
        assert!(rendered.contains("projections=property:name>index_known:0"));
        assert!(rendered.contains("construct=eval"));
        assert!(rendered.contains("conservative_action=havoc_affected_places"));
        for forbidden in ["raw_source", "source_text", "tree_sitter", "oxc_ast", "/tmp/"] {
            assert!(!rendered.contains(forbidden), "unexpected payload leak: {forbidden}");
        }
    }

    #[test]
    fn semantic_mir_unknown_like_statuses_match_as_unknown_metrics() {
        for status in [
            ObservedStatus::Partial,
            ObservedStatus::Unknown,
            ObservedStatus::Unsupported,
        ] {
            let summaries = match_case(
                &[ExpectedItem::Fact(ExpectedFact {
                    family: "MirOperation".to_string(),
                    stable_key: "handler".to_string(),
                    mode: AssertionMode::Partial,
                    producer_id: Some("polint.semantic_mir".to_string()),
                    precision: None,
                    status: Some(status),
                    false_positive_trap: false,
                })],
                &[ObservedItem::Fact(ObservedFact {
                    family: "MirOperation".to_string(),
                    stable_key: "mir-op:handler:branch".to_string(),
                    mode: AssertionMode::Exact,
                    producer_id: Some("polint.semantic_mir".to_string()),
                    provenance: Some("kernel.metadata_debug_json.mir".to_string()),
                    precision: Some("heuristic".to_string()),
                    status: Some(status),
                    payload: None,
                })],
                MatcherConfig::default(),
            );

            assert_eq!(summaries[0].outcome, MatchOutcome::Unknown);
        }
    }
}
