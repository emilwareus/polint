use serde::{Deserialize, Serialize};

pub(crate) const SEMANTIC_FACT_FAMILIES: &[&str] = &[
    "Scope",
    "SemanticImport",
    "Export",
    "Alias",
    "Resolution",
    "GeneratedSymbol",
    "StableExport",
];

pub(crate) const TOPOLOGY_FACT_FAMILIES: &[&str] = &[
    "WorkspaceRoot",
    "TopologyPackage",
    "SourceSet",
    "DependencyRequirement",
    "ResolvedDependencyEdge",
    "ImportToPackage",
    "RepoTopologyOverlay",
];

pub(crate) const SEMANTIC_MIR_FACT_FAMILIES: &[&str] =
    &["MirBody", "MirOperation", "Place", "UnsupportedSemantic"];

pub(crate) const CFG_FACT_FAMILIES: &[&str] = &[
    "CfgFunction",
    "CfgNode",
    "BasicBlock",
    "CfgEdge",
    "CfgReachability",
    "CfgDominator",
    "CfgPostDominator",
    "CfgControlDependence",
    "UnsupportedControlFlow",
];

pub(crate) const CALL_FACT_FAMILIES: &[&str] = &["CallSite", "CallTarget", "UnresolvedCall"];

pub(crate) const REFINED_CALL_FACT_FAMILIES: &[&str] = &["RefinedCallEdge"];

pub(crate) const ABSTRACT_DOMAIN_FACT_FAMILIES: &[&str] = &["DomainObservation", "DomainEvent"];

pub(crate) const DIRECT_SUMMARY_FACT_FAMILIES: &[&str] = &[
    "summary_control",
    "summary_call",
    "summary_memory",
    "summary_tito",
    "summary_event",
];

#[cfg(test)]
#[allow(
    dead_code,
    reason = "Data-flow eval fact families document the internal fixture vocabulary."
)]
pub(crate) const DATA_FLOW_FACT_FAMILIES: &[&str] = &[
    "DataFlowNode",
    "DataFlowEdge",
    "DataFlowModel",
    "DataFlowBudget",
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct EvaluationSuite {
    pub(crate) schema_version: String,
    pub(crate) suite_id: String,
    pub(crate) cases: Vec<EvaluationCase>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct EvaluationCase {
    pub(crate) case_id: String,
    pub(crate) area: FixtureArea,
    pub(crate) repo_path: String,
    pub(crate) expected: Vec<ExpectedItem>,
    pub(crate) observed: Vec<ObservedItem>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvaluationMode {
    PolintBaseline,
    PolintAgentAdapted,
    ImportedScanner,
    LocallyReproducedScanner,
    AdapterOnly,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FixtureArea {
    Kernel,
    Provenance,
    Cache,
    Extension,
    #[serde(rename = "semantic-index")]
    SemanticIndex,
    #[serde(rename = "module-topology")]
    ModuleTopology,
    #[serde(rename = "semantic-mir")]
    SemanticMir,
    Cfg,
    #[serde(rename = "direct-calls")]
    DirectCalls,
    #[serde(rename = "refined-calls")]
    RefinedCalls,
    #[serde(rename = "abstract-domains")]
    AbstractDomains,
    #[serde(rename = "direct-summaries")]
    DirectSummaries,
    #[serde(rename = "framework-entrypoints")]
    FrameworkEntrypoints,
    #[serde(rename = "go-rta")]
    GoRta,
    #[serde(rename = "polyglot-canary")]
    PolyglotCanary,
    #[serde(rename = "data-flow")]
    DataFlow,
    Evidence,
    Facts,
    Graphs,
    Paths,
    Diagnostics,
    Invariants,
    Budgets,
    Promotion,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AssertionMode {
    Exact,
    Tolerant,
    Partial,
    Forbidden,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ExpectedItem {
    Diagnostic(ExpectedDiagnostic),
    Fact(ExpectedFact),
    GraphEdge(ExpectedGraphEdge),
    Path(ExpectedPath),
    Invariant(ExpectedInvariant),
    RuntimeBudget(ExpectedRuntimeBudget),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObservedItem {
    Diagnostic(ObservedDiagnostic),
    Fact(ObservedFact),
    GraphEdge(ObservedGraphEdge),
    Path(ObservedPath),
    Invariant(ObservedInvariant),
    RuntimeBudget(ObservedRuntimeBudget),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExpectedDiagnostic {
    pub(crate) rule_id: String,
    pub(crate) relative_path: String,
    pub(crate) line: Option<u32>,
    pub(crate) fingerprint: Option<String>,
    pub(crate) mode: AssertionMode,
    #[serde(default)]
    pub(crate) false_positive_trap: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ObservedDiagnostic {
    pub(crate) rule_id: String,
    pub(crate) relative_path: String,
    pub(crate) line: Option<u32>,
    pub(crate) fingerprint: Option<String>,
    pub(crate) mode: AssertionMode,
    pub(crate) producer_id: Option<String>,
    pub(crate) provenance: Option<String>,
    pub(crate) precision: Option<String>,
    pub(crate) status: Option<ObservedStatus>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExpectedFact {
    pub(crate) family: String,
    pub(crate) stable_key: String,
    pub(crate) mode: AssertionMode,
    pub(crate) producer_id: Option<String>,
    pub(crate) precision: Option<String>,
    pub(crate) status: Option<ObservedStatus>,
    #[serde(default)]
    pub(crate) false_positive_trap: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ObservedFact {
    pub(crate) family: String,
    pub(crate) stable_key: String,
    pub(crate) mode: AssertionMode,
    pub(crate) producer_id: Option<String>,
    pub(crate) provenance: Option<String>,
    pub(crate) precision: Option<String>,
    pub(crate) status: Option<ObservedStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) payload: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExpectedGraphEdge {
    pub(crate) graph: String,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) mode: AssertionMode,
    pub(crate) partial_truth: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ObservedGraphEdge {
    pub(crate) graph: String,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) mode: AssertionMode,
    pub(crate) partial_truth: bool,
    pub(crate) producer_id: Option<String>,
    pub(crate) provenance: Option<String>,
    pub(crate) precision: Option<String>,
    pub(crate) status: Option<ObservedStatus>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExpectedPath {
    pub(crate) path_id: String,
    pub(crate) nodes: Vec<String>,
    pub(crate) mode: AssertionMode,
    pub(crate) partial_truth: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ObservedPath {
    pub(crate) path_id: String,
    pub(crate) nodes: Vec<String>,
    pub(crate) mode: AssertionMode,
    pub(crate) partial_truth: bool,
    pub(crate) producer_id: Option<String>,
    pub(crate) provenance: Option<String>,
    pub(crate) precision: Option<String>,
    pub(crate) status: Option<ObservedStatus>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExpectedInvariant {
    pub(crate) name: String,
    pub(crate) value: String,
    pub(crate) mode: AssertionMode,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ObservedInvariant {
    pub(crate) name: String,
    pub(crate) value: String,
    pub(crate) mode: AssertionMode,
    pub(crate) producer_id: Option<String>,
    pub(crate) provenance: Option<String>,
    pub(crate) precision: Option<String>,
    pub(crate) status: Option<ObservedStatus>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ExpectedRuntimeBudget {
    pub(crate) name: String,
    pub(crate) max_runtime_ms: u64,
    pub(crate) mode: AssertionMode,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct ObservedRuntimeBudget {
    pub(crate) name: String,
    pub(crate) budget_passed: bool,
    pub(crate) observed_runtime_ms: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ObservedStatus {
    Present,
    Resolved,
    Partial,
    Top,
    Unknown,
    Unresolved,
    Ambiguous,
    Dynamic,
    SetupMissing,
    MissingLockfile,
    Unsupported,
    External,
    Cycle,
    Generated,
    Undeclared,
    OutsideWorkspace,
    BudgetExceeded,
    Rejected,
    Accepted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_model_represents_expected_and_observed_item_kinds() {
        let expected = vec![
            ExpectedItem::Diagnostic(ExpectedDiagnostic {
                rule_id: "local/no-raw-colors".to_string(),
                relative_path: "src/button.tsx".to_string(),
                line: Some(12),
                fingerprint: Some("diag-fp".to_string()),
                mode: AssertionMode::Exact,
                false_positive_trap: false,
            }),
            ExpectedItem::Fact(ExpectedFact {
                family: "symbols".to_string(),
                stable_key: "symbol:Button".to_string(),
                mode: AssertionMode::Partial,
                producer_id: Some("polint.ts.syntax".to_string()),
                precision: Some("syntactic".to_string()),
                status: Some(ObservedStatus::Present),
                false_positive_trap: false,
            }),
            ExpectedItem::GraphEdge(ExpectedGraphEdge {
                graph: "module".to_string(),
                from: "src/button.tsx".to_string(),
                to: "src/theme.ts".to_string(),
                mode: AssertionMode::Tolerant,
                partial_truth: true,
            }),
            ExpectedItem::Path(ExpectedPath {
                path_id: "route-to-sink".to_string(),
                nodes: vec!["handler".to_string(), "sink".to_string()],
                mode: AssertionMode::Partial,
                partial_truth: true,
            }),
            ExpectedItem::Invariant(ExpectedInvariant {
                name: "provider_order_stable".to_string(),
                value: "true".to_string(),
                mode: AssertionMode::Exact,
            }),
            ExpectedItem::RuntimeBudget(ExpectedRuntimeBudget {
                name: "fast-ci".to_string(),
                max_runtime_ms: 500,
                mode: AssertionMode::Exact,
            }),
        ];
        let observed = vec![
            ObservedItem::Diagnostic(ObservedDiagnostic {
                rule_id: "local/no-raw-colors".to_string(),
                relative_path: "src/button.tsx".to_string(),
                line: Some(12),
                fingerprint: Some("diag-fp".to_string()),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.ts.syntax".to_string()),
                provenance: Some("native".to_string()),
                precision: Some("syntactic".to_string()),
                status: Some(ObservedStatus::Present),
            }),
            ObservedItem::Fact(ObservedFact {
                family: "symbols".to_string(),
                stable_key: "symbol:Button".to_string(),
                mode: AssertionMode::Partial,
                producer_id: Some("polint.ts.syntax".to_string()),
                provenance: Some("metadata-sidecar".to_string()),
                precision: Some("syntactic".to_string()),
                status: Some(ObservedStatus::Present),
                payload: None,
            }),
            ObservedItem::GraphEdge(ObservedGraphEdge {
                graph: "module".to_string(),
                from: "src/button.tsx".to_string(),
                to: "src/theme.ts".to_string(),
                mode: AssertionMode::Tolerant,
                partial_truth: true,
                producer_id: Some("polint.module_graph".to_string()),
                provenance: Some("derived".to_string()),
                precision: Some("syntactic".to_string()),
                status: Some(ObservedStatus::Present),
            }),
            ObservedItem::Path(ObservedPath {
                path_id: "route-to-sink".to_string(),
                nodes: vec!["handler".to_string(), "sink".to_string()],
                mode: AssertionMode::Partial,
                partial_truth: true,
                producer_id: Some("polint.paths".to_string()),
                provenance: Some("derived".to_string()),
                precision: Some("partial".to_string()),
                status: Some(ObservedStatus::Unknown),
            }),
            ObservedItem::Invariant(ObservedInvariant {
                name: "provider_order_stable".to_string(),
                value: "true".to_string(),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.eval".to_string()),
                provenance: Some("fixture".to_string()),
                precision: Some("exact".to_string()),
                status: Some(ObservedStatus::Accepted),
            }),
            ObservedItem::RuntimeBudget(ObservedRuntimeBudget {
                name: "fast-ci".to_string(),
                budget_passed: true,
                observed_runtime_ms: Some(412),
            }),
        ];

        let suite = EvaluationSuite {
            schema_version: "polint-eval-internal-1".to_string(),
            suite_id: "eval-schema".to_string(),
            cases: vec![EvaluationCase {
                case_id: "all-item-kinds".to_string(),
                area: FixtureArea::Kernel,
                repo_path: "fixtures/kernel/repo".to_string(),
                expected,
                observed,
            }],
        };

        assert_eq!(suite.cases[0].expected.len(), 6);
        assert_eq!(suite.cases[0].observed.len(), 6);
    }

    #[test]
    fn eval_model_serializes_modes_as_snake_case_strings() {
        assert_eq!(
            serde_json::to_string(&AssertionMode::Exact).unwrap(),
            "\"exact\""
        );
        assert_eq!(
            serde_json::to_string(&AssertionMode::Tolerant).unwrap(),
            "\"tolerant\""
        );
        assert_eq!(
            serde_json::to_string(&AssertionMode::Partial).unwrap(),
            "\"partial\""
        );
        assert_eq!(
            serde_json::to_string(&AssertionMode::Forbidden).unwrap(),
            "\"forbidden\""
        );
    }

    #[test]
    fn eval_model_observed_items_carry_normalized_identity_and_statuses() {
        let observed = vec![
            ObservedItem::Diagnostic(ObservedDiagnostic {
                rule_id: "local/no-raw-colors".to_string(),
                relative_path: "src/button.tsx".to_string(),
                line: Some(12),
                fingerprint: Some("diag-fp".to_string()),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.ts.syntax".to_string()),
                provenance: Some("native".to_string()),
                precision: Some("syntactic".to_string()),
                status: Some(ObservedStatus::Unknown),
            }),
            ObservedItem::Fact(ObservedFact {
                family: "symbols".to_string(),
                stable_key: "symbol:Button".to_string(),
                mode: AssertionMode::Partial,
                producer_id: Some("polint.symbol_graph".to_string()),
                provenance: Some("metadata-sidecar".to_string()),
                precision: Some("setup_aware".to_string()),
                status: Some(ObservedStatus::SetupMissing),
                payload: None,
            }),
            ObservedItem::Fact(ObservedFact {
                family: "call_graph".to_string(),
                stable_key: "call:dynamic".to_string(),
                mode: AssertionMode::Forbidden,
                producer_id: None,
                provenance: None,
                precision: None,
                status: Some(ObservedStatus::Unsupported),
                payload: None,
            }),
        ];

        let json = serde_json::to_string_pretty(&observed).unwrap();

        assert!(json.contains("src/button.tsx"));
        assert!(json.contains("diag-fp"));
        assert!(json.contains("symbols"));
        assert!(json.contains("symbol:Button"));
        assert!(json.contains("polint.symbol_graph"));
        assert!(json.contains("metadata-sidecar"));
        assert!(json.contains("setup_aware"));
        assert!(json.contains("unknown"));
        assert!(json.contains("setup_missing"));
        assert!(json.contains("unsupported"));
    }

    #[test]
    fn eval_model_eval_module_stays_crate_private_in_lib_rs() {
        let lib_rs =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")).unwrap();

        assert_eq!(lib_rs.matches("pub(crate) mod eval;").count(), 1);
        assert!(!lib_rs.contains("pub mod eval"));
    }

    #[test]
    fn eval_model_runtime_budgets_use_distinct_expected_and_observed_shapes() {
        let expected = ExpectedRuntimeBudget {
            name: "fast-ci".to_string(),
            max_runtime_ms: 500,
            mode: AssertionMode::Exact,
        };
        let observed = ObservedRuntimeBudget {
            name: "fast-ci".to_string(),
            budget_passed: false,
            observed_runtime_ms: Some(725),
        };

        assert_eq!(expected.max_runtime_ms, 500);
        assert!(!observed.budget_passed);
        assert_eq!(observed.observed_runtime_ms, Some(725));
    }
}
