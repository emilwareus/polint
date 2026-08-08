use crate::analysis::calls::facts::{CallSiteFact, CallTargetFact, UnresolvedCallFact};
use crate::analysis::cfg::facts::{
    BasicBlockFact, CfgEdgeFact, CfgFunctionFact, CfgNodeFact, ControlDependenceFact,
    DominatorFact, PostDominatorFact, ReachabilityFact, UnsupportedControlFlowFact,
};
use crate::analysis::data_flow::facts::{
    DataFlowBudgetFact, DataFlowConfidence, DataFlowEdgeFact, DataFlowModelFact, DataFlowNodeFact,
    DataFlowPrecision, DataFlowStatus, DataFlowValidation,
};
use crate::analysis::data_flow::provider::DATA_FLOW_PROVIDER_ID;
use crate::analysis::domains::facts::{DomainEventFact, DomainObservationFact};
use crate::analysis::evidence::facts::{
    EvidenceBundleFact, EvidenceEdgeFact, EvidenceNodeFact, EvidenceOmittedRegionFact,
    EvidencePathFact, EvidencePrecision, EvidenceReplayKeyFact, EvidenceSliceFact,
    EvidenceUnknownFact,
};
use crate::analysis::evidence::provider::EVIDENCE_PROVIDER_ID;
use crate::analysis::mir::body::MirBody;
use crate::analysis::mir::op::{MirOperation, UnsupportedSemanticFact};
use crate::analysis::places::PlaceFact;
use crate::analysis::refined_calls::facts::RefinedCallEdgeFact;
use crate::analysis::refined_calls::provider::REFINED_CALLS_PROVIDER_ID;
use crate::analysis_kernel::{
    FactConfidence, FactFamily, FactMeta, FactPrecision, FactRef, resolution_metadata,
    resolution_status_metadata, stable_key_from_parts, symbol_metadata,
};
use crate::symbol_graph::semantic::SemanticStatus;
use std::collections::{BTreeMap, BTreeSet};

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

use labels::*;
use metadata::*;

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

impl AnalysisDb {
    pub fn facts_for_file(&self, file: FileId) -> CachedFileFacts {
        let branch_ids = self
            .branches
            .iter()
            .filter(|branch| branch.file == file)
            .map(|branch| branch.id)
            .collect::<BTreeSet<_>>();
        CachedFileFacts {
            packages: self
                .packages
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            functions: self
                .functions
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            imports: self
                .imports
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            branches: self
                .branches
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            tests: self
                .tests
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            coverage: self
                .coverage
                .iter()
                .filter(|fact| branch_ids.contains(&fact.branch))
                .cloned()
                .collect(),
            ts_components: self
                .ts_components
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            ts_classes: self
                .ts_classes
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            string_literals: self
                .string_literals
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
            jsx_attributes: self
                .jsx_attributes
                .iter()
                .filter(|fact| fact.file == file)
                .cloned()
                .collect(),
        }
    }

    pub fn restore_file_facts(&mut self, file: FileId, facts: CachedFileFacts) {
        let mut function_ids = BTreeMap::new();
        let mut branch_ids = BTreeMap::new();

        for mut package in facts.packages {
            package.file = file;
            package.span.file = file;
            self.push_package(package);
        }

        for mut function in facts.functions {
            let cached_id = function.id;
            function.file = file;
            function.span.file = file;
            let restored_id = self.push_function(function);
            function_ids.insert(cached_id, restored_id);
        }

        for mut import in facts.imports {
            import.file = file;
            import.span.file = file;
            self.push_import(import);
        }

        for mut branch in facts.branches {
            let cached_id = branch.id;
            branch.file = file;
            branch.function = branch
                .function
                .and_then(|function| function_ids.get(&function).copied());
            branch.decision_span.file = file;
            let restored_id = self.push_branch(branch);
            branch_ids.insert(cached_id, restored_id);
        }

        for mut test in facts.tests {
            test.file = file;
            test.function = test
                .function
                .and_then(|function| function_ids.get(&function).copied());
            test.span.file = file;
            self.push_test(test);
        }

        for mut coverage in facts.coverage {
            if let Some(branch) = branch_ids.get(&coverage.branch).copied() {
                coverage.branch = branch;
                self.push_coverage(coverage);
            }
        }

        for mut component in facts.ts_components {
            component.file = file;
            component.function = component
                .function
                .and_then(|function| function_ids.get(&function).copied());
            component.span.file = file;
            self.push_ts_component(component);
        }

        for mut class in facts.ts_classes {
            class.file = file;
            class.span.file = file;
            self.push_ts_class(class);
        }

        for mut literal in facts.string_literals {
            literal.file = file;
            literal.span.file = file;
            self.push_string_literal(literal);
        }

        for mut attribute in facts.jsx_attributes {
            attribute.file = file;
            attribute.span.file = file;
            self.push_jsx_attribute(attribute);
        }
    }

    fn record_fact_meta(&mut self, family: FactFamily, run_id: u64, meta: FactMeta) {
        let reference = FactRef::new(family, run_id);
        let _insert = self.fact_meta.insert(reference, meta);
        debug_assert!(self.metadata_for(reference).is_some());
    }

    fn finish_fact_meta_insertions(&mut self, families: &[FactFamily]) {
        for family in families {
            self.fact_meta.finish_family_insertions(*family);
        }
    }

    pub(crate) fn finish_all_fact_meta_insertions(&mut self) {
        self.fact_meta.finish_all_insertions();
    }

    fn package_metadata(&self, fact: &PackageFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Package,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([]),
        )
    }

    fn function_metadata(&self, fact: &FunctionFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Function,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([
                ("is_test", fact.is_test.to_string()),
                ("is_exported", fact.is_exported.to_string()),
                (
                    "cyclomatic_complexity",
                    fact.cyclomatic_complexity.to_string(),
                ),
                ("calls", fact.calls.join("\n")),
            ]),
        )
    }

    fn import_metadata(&self, fact: &ImportFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Import,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("import_path", fact.path.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([(
                "package",
                fact.package.clone().unwrap_or_else(|| "none".to_string()),
            )]),
        )
    }

    fn branch_metadata(&self, fact: &BranchObligation) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::BranchObligation,
            GO_SYNTAX_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("stable_fingerprint", fact.stable_fingerprint.clone()),
            ]),
            stable_parts([
                ("function", option_function_id(fact.function)),
                ("span", span_metadata_value(&fact.decision_span)),
                ("condition_text", fact.condition_text.clone()),
                ("edge_label", fact.edge_label.clone()),
                ("is_error_path", fact.is_error_path.to_string()),
            ]),
        )
    }

    fn test_metadata(&self, fact: &TestFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::Test,
            GO_SYNTAX_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([
                ("function", option_function_id(fact.function)),
                ("evidence_terms", fact.evidence_terms.join("\n")),
                ("assertion_count", fact.assertion_count.to_string()),
                ("subtest_count", fact.subtest_count.to_string()),
                ("subtest_names", fact.subtest_names.join("\n")),
                ("table_rows", fact.table_rows.to_string()),
            ]),
        )
    }

    fn coverage_metadata(&self, fact: &CoverageFact) -> FactMeta {
        let branch = self.branches.iter().find(|branch| branch.id == fact.branch);
        let (path, branch_fingerprint, precision, confidence) = if let Some(branch) = branch {
            (
                self.path_for(branch.file),
                branch.stable_fingerprint.clone(),
                FactPrecision::SetupAware,
                FactConfidence::Medium,
            )
        } else {
            (
                "<unknown>".to_string(),
                format!("unresolved:{}", fact.branch.0),
                FactPrecision::Unsupported,
                FactConfidence::Low,
            )
        };

        fact_meta_from_parts(
            FactFamily::Coverage,
            branch
                .map(|branch| syntax_provider_for_file(self.file(branch.file)))
                .unwrap_or(GO_SYNTAX_PROVIDER_ID),
            precision,
            confidence,
            stable_parts([
                ("path", path),
                ("branch_fingerprint", branch_fingerprint),
                ("source", fact.source.clone()),
            ]),
            stable_parts([
                ("branch", fact.branch.0.to_string()),
                ("covered", option_bool(fact.covered)),
            ]),
        )
    }

    fn file_metric_metadata(&self, fact: &FileMetricFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::FileMetric,
            METRICS_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([("file_key", self.source_file_key(fact.file))]),
            stable_parts([
                ("language", language_label(fact.language).to_string()),
                ("line_count", fact.line_count.to_string()),
                (
                    "non_empty_line_count",
                    fact.non_empty_line_count.to_string(),
                ),
                ("byte_count", fact.byte_count.to_string()),
                ("function_count", fact.function_count.to_string()),
            ]),
        )
    }

    fn function_metric_metadata(&self, fact: &FunctionMetricFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::FunctionMetric,
            METRICS_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                (
                    "function_key",
                    self.function_key(fact.function, &fact.name, &fact.span),
                ),
                ("metric_name", FUNCTION_SIZE_METRIC_NAME.to_string()),
            ]),
            stable_parts([
                ("file_key", self.source_file_key(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("line_count", fact.line_count.to_string()),
                ("byte_count", fact.byte_count.to_string()),
            ]),
        )
    }

    fn complexity_metric_metadata(&self, fact: &ComplexityMetricFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::ComplexityMetric,
            METRICS_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                (
                    "function_key",
                    self.function_key(fact.function, &fact.name, &fact.span),
                ),
                ("metric_name", CYCLOMATIC_COMPLEXITY_METRIC_NAME.to_string()),
            ]),
            stable_parts([
                ("file_key", self.source_file_key(fact.file)),
                ("language", language_label(fact.language).to_string()),
                (
                    "cyclomatic_complexity",
                    fact.cyclomatic_complexity.to_string(),
                ),
            ]),
        )
    }

    fn module_node_metadata(&self, node: &ModuleNode) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::ModuleNode,
            MODULE_GRAPH_PROVIDER_ID,
            FactPrecision::SetupAware,
            FactConfidence::High,
            stable_parts([
                ("kind", module_node_kind_label(node.kind).to_string()),
                ("label", node.label.clone()),
                ("path", option_file_path(self, node.file)),
                (
                    "package_key",
                    node.package
                        .map(|package| self.fact_stable_key(FactFamily::Package, package.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "language",
                    node.language
                        .map(|language| language_label(language).to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
            stable_parts([("id", node.id.0.to_string())]),
        )
    }

    fn resolved_import_metadata(&self, fact: &ResolvedImportFact) -> FactMeta {
        let (precision, confidence) = resolution_metadata(fact.precision, fact.status);
        fact_meta_from_parts(
            FactFamily::ResolvedImport,
            MODULE_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            stable_parts([
                (
                    "import_key",
                    self.fact_stable_key(FactFamily::Import, fact.import.0),
                ),
                ("from_path", self.path_for(fact.from_file)),
                (
                    "target_node_key",
                    fact.target_node
                        .map(|node| self.fact_stable_key(FactFamily::ModuleNode, node.0))
                        .unwrap_or_else(none_value),
                ),
                ("status", resolution_status_label(fact.status).to_string()),
                (
                    "precision",
                    resolution_precision_label(fact.precision).to_string(),
                ),
                (
                    "reason",
                    fact.reason
                        .map(|reason| unresolved_reason_label(reason).to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("import", fact.import.0.to_string()),
                ("from_file", u64::from(fact.from_file.0).to_string()),
                (
                    "target_node",
                    fact.target_node
                        .map(|node| node.0.to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn module_edge_metadata(&self, edge: &ModuleEdge) -> FactMeta {
        let (precision, confidence) = resolution_status_metadata(edge.status);
        fact_meta_from_parts(
            FactFamily::ModuleEdge,
            MODULE_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            stable_parts([
                (
                    "from_node_key",
                    self.fact_stable_key(FactFamily::ModuleNode, edge.from.0),
                ),
                (
                    "to_node_key",
                    self.fact_stable_key(FactFamily::ModuleNode, edge.to.0),
                ),
                (
                    "import_key",
                    edge.import
                        .map(|import| self.fact_stable_key(FactFamily::Import, import.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "resolved_import_key",
                    edge.resolved_import
                        .map(|resolved| {
                            self.fact_stable_key(FactFamily::ResolvedImport, resolved.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                ("kind", module_edge_kind_label(edge.kind).to_string()),
                ("status", resolution_status_label(edge.status).to_string()),
            ]),
            stable_parts([
                ("id", edge.id.0.to_string()),
                ("from", edge.from.0.to_string()),
                ("to", edge.to.0.to_string()),
            ]),
        )
    }

    fn symbol_fact_metadata(&self, fact: &SymbolFact) -> FactMeta {
        let (precision, confidence) = symbol_metadata(fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Symbol,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("qualified_name", fact.qualified_name.clone()),
                (
                    "precision",
                    symbol_precision_label(fact.precision).to_string(),
                ),
                ("path", option_file_path(self, fact.file)),
                (
                    "span",
                    option_span_metadata_value(fact.primary_span.as_ref()),
                ),
            ]),
        )
    }

    fn definition_fact_metadata(&self, fact: &DefinitionFact) -> FactMeta {
        let (precision, confidence) = symbol_metadata(fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Definition,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("symbol", fact.symbol.0.to_string()),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("qualified_name", fact.qualified_name.clone()),
                (
                    "precision",
                    symbol_precision_label(fact.precision).to_string(),
                ),
                ("path", option_file_path(self, fact.file)),
                (
                    "span",
                    option_span_metadata_value(fact.primary_span.as_ref()),
                ),
            ]),
        )
    }

    fn reference_fact_metadata(&self, fact: &ReferenceFact) -> FactMeta {
        let (precision, confidence) = symbol_metadata(fact.precision);
        fact_meta_from_stable_key(
            FactFamily::Reference,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("id", fact.id.0.to_string()),
                ("language", language_label(fact.language).to_string()),
                ("name", fact.name.clone()),
                ("qualified_name", fact.qualified_name.clone()),
                (
                    "precision",
                    symbol_precision_label(fact.precision).to_string(),
                ),
                (
                    "status",
                    symbol_resolution_status_label(fact.status).to_string(),
                ),
                ("path", option_file_path(self, fact.file)),
                (
                    "span",
                    option_span_metadata_value(fact.primary_span.as_ref()),
                ),
                (
                    "target",
                    fact.target
                        .map(|target| target.0.to_string())
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn semantic_fact_metadata(
        &self,
        family: FactFamily,
        stable_key: &str,
        status: SemanticStatus,
    ) -> FactMeta {
        let (precision, confidence) = semantic_status_metadata(status);
        fact_meta_from_stable_key(
            family,
            SYMBOL_GRAPH_PROVIDER_ID,
            precision,
            confidence,
            stable_key.to_string(),
            stable_parts([("status", semantic_status_label(status).to_string())]),
        )
    }

    fn mir_body_metadata(&self, body: &MirBody) -> FactMeta {
        let (precision, confidence) = mir_status_metadata(body.status);
        fact_meta_from_stable_key(
            FactFamily::MirBody,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            body.stable_key.clone(),
            stable_parts([
                ("status", mir_status_label(body.status).to_string()),
                ("language", language_label(body.language).to_string()),
                ("file_key", self.source_file_key(body.file)),
                (
                    "function_key",
                    self.function_key(body.function, "", &body.span),
                ),
                ("owner_stable_key", body.owner_stable_key.clone()),
                (
                    "package",
                    body.package
                        .map(|package| package.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                (
                    "module",
                    body.module
                        .map(|module| module.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                ("span", span_metadata_value(&body.span)),
            ]),
        )
    }

    fn mir_operation_metadata(&self, operation: &MirOperation) -> FactMeta {
        let (precision, confidence) = mir_status_metadata(operation.status);
        fact_meta_from_stable_key(
            FactFamily::MirOperation,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            operation.stable_key.clone(),
            stable_parts([
                ("status", mir_status_label(operation.status).to_string()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, operation.body.0),
                ),
                ("ordinal", operation.ordinal.to_string()),
                ("span", span_metadata_value(&operation.span)),
            ]),
        )
    }

    fn place_metadata(&self, place: &PlaceFact) -> FactMeta {
        let (precision, confidence) = place_status_metadata(place.status);
        fact_meta_from_stable_key(
            FactFamily::Place,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            place.stable_key.clone(),
            stable_parts([
                ("status", place_status_label(place.status).to_string()),
                ("language", language_label(place.language).to_string()),
                ("path", option_file_path(self, place.file)),
                ("function", option_function_id(place.function)),
                ("projection_count", place.projections.len().to_string()),
            ]),
        )
    }

    fn unsupported_semantic_metadata(&self, row: &UnsupportedSemanticFact) -> FactMeta {
        let (precision, confidence) = mir_status_metadata(row.status);
        fact_meta_from_stable_key(
            FactFamily::UnsupportedSemantic,
            SEMANTIC_MIR_PROVIDER_ID,
            precision,
            confidence,
            row.stable_key.clone(),
            stable_parts([
                ("status", mir_status_label(row.status).to_string()),
                ("language", language_label(row.language).to_string()),
                ("path", self.path_for(row.file)),
                ("span", span_metadata_value(&row.span)),
                ("construct", row.construct.clone()),
                ("source_evidence", row.source_evidence.clone()),
                (
                    "body_key",
                    row.body
                        .map(|body| self.fact_stable_key(FactFamily::MirBody, body.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    row.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                (
                    "affected_places",
                    row.affected_places
                        .iter()
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
            ]),
        )
    }

    fn call_site_metadata(&self, fact: &CallSiteFact) -> FactMeta {
        let (precision, confidence) = call_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CallSite,
            CALLS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                ("kind", call_syntax_kind_label(fact.kind).to_string()),
                ("language", language_label(fact.language).to_string()),
                ("file_key", self.source_file_key(fact.file)),
                ("caller_key", self.function_key(fact.caller, "", &fact.span)),
                (
                    "owner_symbol_key",
                    fact.owner_symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                (
                    "operation_key",
                    self.fact_stable_key(FactFamily::MirOperation, fact.operation.0),
                ),
                ("span", span_metadata_value(&fact.span)),
            ]),
        )
    }

    fn call_target_metadata(&self, fact: &CallTargetFact) -> FactMeta {
        let (precision, confidence) = call_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CallTarget,
            CALLS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                (
                    "algorithm",
                    call_algorithm_label(fact.algorithm).to_string(),
                ),
                (
                    "edge_kind",
                    call_edge_kind_label(fact.edge_kind).to_string(),
                ),
                (
                    "reason",
                    fact.reason
                        .map(call_unresolved_reason_label)
                        .map(str::to_string)
                        .unwrap_or_else(none_value),
                ),
                (
                    "site_key",
                    self.fact_stable_key(FactFamily::CallSite, fact.site.0),
                ),
                (
                    "caller_key",
                    self.fact_stable_key(FactFamily::Function, fact.caller.0),
                ),
                (
                    "target_function_key",
                    fact.target_function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "target_symbol_key",
                    fact.target_symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn refined_call_edge_metadata(&self, fact: &RefinedCallEdgeFact) -> FactMeta {
        let (precision, status_confidence) = call_status_metadata(fact.status, fact.precision);
        let confidence = refined_call_confidence_metadata(fact.confidence, status_confidence);
        let validation = refined_call_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::RefinedCallEdge,
            REFINED_CALLS_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                (
                    "algorithm",
                    call_algorithm_label(fact.algorithm).to_string(),
                ),
                (
                    "edge_kind",
                    call_edge_kind_label(fact.edge_kind).to_string(),
                ),
                ("tier", refined_call_tier_label(fact.tier).to_string()),
                (
                    "validation",
                    refined_call_validation_label(fact.validation).to_string(),
                ),
                (
                    "reason",
                    fact.reason
                        .map(call_unresolved_reason_label)
                        .map(str::to_string)
                        .unwrap_or_else(none_value),
                ),
                (
                    "site_key",
                    self.fact_stable_key(FactFamily::CallSite, fact.site.0),
                ),
                (
                    "base_target_key",
                    fact.base_target
                        .map(|target| self.fact_stable_key(FactFamily::CallTarget, target.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "caller_key",
                    self.fact_stable_key(FactFamily::Function, fact.caller.0),
                ),
                (
                    "target_function_key",
                    fact.target_function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "target_symbol_key",
                    fact.target_symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "synthetic_target",
                    fact.synthetic_target.clone().unwrap_or_else(none_value),
                ),
                ("evidence", fact.evidence.join("\n")),
                ("inputs", fact.input_stable_keys.join("\n")),
            ]),
        )
    }

    fn data_flow_node_metadata(&self, fact: &DataFlowNodeFact) -> FactMeta {
        let model = fact
            .model
            .and_then(|id| self.data_flow_models.iter().find(|model| model.id == id));
        let (status, data_flow_precision, data_flow_confidence, data_flow_validation, model_key) =
            model.map_or(
                (
                    DataFlowStatus::Present,
                    DataFlowPrecision::Syntax,
                    DataFlowConfidence::High,
                    DataFlowValidation::Native,
                    none_value(),
                ),
                |model| {
                    (
                        model.status,
                        model.precision,
                        model.confidence,
                        model.validation,
                        model.stable_key.clone(),
                    )
                },
            );
        let (precision, status_confidence) = data_flow_status_metadata(status, data_flow_precision);
        let confidence = data_flow_confidence_metadata(data_flow_confidence, status_confidence);
        let validation = data_flow_validation_metadata(data_flow_validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::DataFlowNode,
            DATA_FLOW_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("status", data_flow_status_label(status).to_string()),
                (
                    "precision",
                    data_flow_precision_label(data_flow_precision).to_string(),
                ),
                (
                    "validation",
                    data_flow_validation_label(data_flow_validation).to_string(),
                ),
                ("language", language_label(fact.language).to_string()),
                (
                    "file_key",
                    fact.file
                        .map(|file| self.source_file_key(file))
                        .unwrap_or_else(none_value),
                ),
                (
                    "function_key",
                    fact.function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "place_key",
                    fact.place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "symbol_key",
                    fact.symbol
                        .map(|symbol| self.fact_stable_key(FactFamily::Symbol, symbol.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "reference_key",
                    fact.reference
                        .map(|reference| self.fact_stable_key(FactFamily::Reference, reference.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "call_site_key",
                    fact.call_site
                        .map(|site| self.fact_stable_key(FactFamily::CallSite, site.0))
                        .unwrap_or_else(none_value),
                ),
                ("model_key", model_key),
                ("span", option_span_metadata_value(fact.span.as_ref())),
            ]),
        )
    }

    fn data_flow_edge_metadata(&self, fact: &DataFlowEdgeFact) -> FactMeta {
        let (precision, status_confidence) = data_flow_status_metadata(fact.status, fact.precision);
        let confidence = data_flow_confidence_metadata(fact.confidence, status_confidence);
        let validation = data_flow_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::DataFlowEdge,
            DATA_FLOW_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("algorithm", format!("{:?}", fact.algorithm)),
                ("status", data_flow_status_label(fact.status).to_string()),
                (
                    "precision",
                    data_flow_precision_label(fact.precision).to_string(),
                ),
                (
                    "validation",
                    data_flow_validation_label(fact.validation).to_string(),
                ),
                (
                    "from_key",
                    self.fact_stable_key(FactFamily::DataFlowNode, fact.from.0),
                ),
                (
                    "to_key",
                    self.fact_stable_key(FactFamily::DataFlowNode, fact.to.0),
                ),
                (
                    "call_site_key",
                    fact.call_site
                        .map(|site| self.fact_stable_key(FactFamily::CallSite, site.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "call_target_key",
                    fact.call_target
                        .map(|target| self.fact_stable_key(FactFamily::CallTarget, target.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "refined_call_key",
                    fact.refined_call
                        .map(|edge| self.fact_stable_key(FactFamily::RefinedCallEdge, edge.0))
                        .unwrap_or_else(none_value),
                ),
                ("evidence", fact.evidence.join("\n")),
                ("inputs", fact.input_stable_keys.join("\n")),
            ]),
        )
    }

    fn data_flow_model_metadata(&self, fact: &DataFlowModelFact) -> FactMeta {
        let (precision, status_confidence) = data_flow_status_metadata(fact.status, fact.precision);
        let confidence = data_flow_confidence_metadata(fact.confidence, status_confidence);
        let validation = data_flow_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::DataFlowModel,
            DATA_FLOW_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("language", language_label(fact.language).to_string()),
                ("provider_id", fact.provider_id.clone()),
                ("model_id", fact.model_id.clone().unwrap_or_else(none_value)),
                (
                    "source_key",
                    fact.source_stable_key.clone().unwrap_or_else(none_value),
                ),
                ("evidence", fact.evidence.join("\n")),
                ("payload_labels", fact.payload_labels.join("\n")),
            ]),
        )
    }

    fn data_flow_budget_metadata(&self, fact: &DataFlowBudgetFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::DataFlowBudget,
            DATA_FLOW_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("reason", format!("{:?}", fact.reason)),
                ("status", data_flow_status_label(fact.status).to_string()),
                ("limit", fact.limit.to_string()),
                ("observed", fact.observed.to_string()),
            ]),
        )
    }

    fn evidence_node_metadata(&self, fact: &EvidenceNodeFact) -> FactMeta {
        let (precision, status_confidence) = evidence_status_metadata(fact.status, fact.precision);
        let confidence = evidence_confidence_metadata(fact.confidence, status_confidence);
        let validation = evidence_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::EvidenceNode,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("status", evidence_status_label(fact.status).to_string()),
                (
                    "precision",
                    evidence_precision_label(fact.precision).to_string(),
                ),
                (
                    "provenance",
                    evidence_provenance_label(fact.provenance).to_string(),
                ),
                (
                    "validation",
                    evidence_validation_label(fact.validation).to_string(),
                ),
                ("language", language_label(fact.language).to_string()),
                (
                    "file_key",
                    fact.file
                        .map(|file| self.source_file_key(file))
                        .unwrap_or_else(none_value),
                ),
                (
                    "function_key",
                    fact.function
                        .map(|function| self.fact_stable_key(FactFamily::Function, function.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "place_key",
                    fact.place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    fact.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                ("span", option_span_metadata_value(fact.span.as_ref())),
                ("sources", fact.source_fact_stable_keys.join("\n")),
            ]),
        )
    }

    fn evidence_edge_metadata(&self, fact: &EvidenceEdgeFact) -> FactMeta {
        let (precision, status_confidence) = evidence_status_metadata(fact.status, fact.precision);
        let confidence = evidence_confidence_metadata(fact.confidence, status_confidence);
        let validation = evidence_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::EvidenceEdge,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("kind", format!("{:?}", fact.kind)),
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                (
                    "precision",
                    evidence_precision_label(fact.precision).to_string(),
                ),
                (
                    "provenance",
                    evidence_provenance_label(fact.provenance).to_string(),
                ),
                (
                    "validation",
                    evidence_validation_label(fact.validation).to_string(),
                ),
                (
                    "from_key",
                    self.fact_stable_key(FactFamily::EvidenceNode, fact.from.0),
                ),
                (
                    "to_key",
                    self.fact_stable_key(FactFamily::EvidenceNode, fact.to.0),
                ),
                (
                    "call_site_key",
                    fact.call_site
                        .map(|site| self.fact_stable_key(FactFamily::CallSite, site.0))
                        .unwrap_or_else(none_value),
                ),
                (
                    "summary_key",
                    fact.summary_stable_key.clone().unwrap_or_else(none_value),
                ),
                ("sources", fact.source_fact_stable_keys.join("\n")),
            ]),
        )
    }

    fn evidence_bundle_metadata(&self, fact: &EvidenceBundleFact) -> FactMeta {
        let (precision, status_confidence) = evidence_status_metadata(fact.status, fact.precision);
        let confidence = evidence_confidence_metadata(fact.confidence, status_confidence);
        let validation = evidence_validation_metadata(fact.validation);
        fact_meta_from_stable_key_with_validation(
            FactFamily::EvidenceBundle,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            validation,
            fact.stable_key.clone(),
            stable_parts([
                ("diagnostic_key", fact.diagnostic_stable_key.clone()),
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                (
                    "precision",
                    evidence_precision_label(fact.precision).to_string(),
                ),
                ("selected_paths", fact.selected_paths.len().to_string()),
                ("selected_slices", fact.selected_slices.len().to_string()),
                (
                    "replay_key",
                    fact.replay_key.clone().unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn evidence_path_metadata(&self, fact: &EvidencePathFact) -> FactMeta {
        let (precision, confidence) =
            evidence_status_metadata(fact.status, EvidencePrecision::Heuristic);
        fact_meta_from_stable_key(
            FactFamily::EvidencePath,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                ("rank", fact.rank.to_string()),
                ("node_count", fact.nodes.len().to_string()),
                ("edge_count", fact.edges.len().to_string()),
                ("hidden_node_count", fact.hidden_node_count.to_string()),
            ]),
        )
    }

    fn evidence_slice_metadata(&self, fact: &EvidenceSliceFact) -> FactMeta {
        let (precision, confidence) =
            evidence_status_metadata(fact.status, EvidencePrecision::Heuristic);
        fact_meta_from_stable_key(
            FactFamily::EvidenceSlice,
            EVIDENCE_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("status", evidence_status_label(fact.status).to_string()),
                ("root_count", fact.root_nodes.len().to_string()),
                ("node_count", fact.nodes.len().to_string()),
                ("edge_count", fact.edges.len().to_string()),
            ]),
        )
    }

    fn evidence_unknown_metadata(&self, fact: &EvidenceUnknownFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::EvidenceUnknown,
            EVIDENCE_PROVIDER_ID,
            FactPrecision::Unresolved,
            FactConfidence::Low,
            fact.stable_key.clone(),
            stable_parts([
                ("reason", format!("{:?}", fact.reason)),
                ("message", fact.message.clone()),
                ("sources", fact.source_fact_stable_keys.join("\n")),
            ]),
        )
    }

    fn evidence_omitted_region_metadata(&self, fact: &EvidenceOmittedRegionFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::EvidenceOmittedRegion,
            EVIDENCE_PROVIDER_ID,
            FactPrecision::Unresolved,
            FactConfidence::Low,
            fact.stable_key.clone(),
            stable_parts([
                ("reason", format!("{:?}", fact.reason)),
                ("hidden_node_count", fact.hidden_node_count.to_string()),
                ("hidden_edge_count", fact.hidden_edge_count.to_string()),
                (
                    "budget_label",
                    fact.budget_label.clone().unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn evidence_replay_key_metadata(&self, fact: &EvidenceReplayKeyFact) -> FactMeta {
        fact_meta_from_stable_key(
            FactFamily::EvidenceReplayKey,
            EVIDENCE_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            fact.stable_key.clone(),
            stable_parts([
                ("query_mode", format!("{:?}", fact.query_mode)),
                ("graph_schema", fact.graph_schema.clone()),
                ("max_paths", fact.query_budget.max_paths.to_string()),
                ("max_nodes", fact.query_budget.max_nodes.to_string()),
                ("max_edges", fact.query_budget.max_edges.to_string()),
                ("max_depth", fact.query_budget.max_depth.to_string()),
                ("ranking", format!("{:?}", fact.ranking)),
                ("renderer", format!("{:?}", fact.renderer)),
                ("upstream", fact.upstream_digest_keys.join("\n")),
            ]),
        )
    }

    fn unresolved_call_metadata(&self, fact: &UnresolvedCallFact) -> FactMeta {
        let (precision, confidence) = call_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::UnresolvedCall,
            CALLS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", call_status_label(fact.status).to_string()),
                (
                    "precision",
                    call_precision_label(fact.precision).to_string(),
                ),
                (
                    "algorithm",
                    call_algorithm_label(fact.algorithm).to_string(),
                ),
                (
                    "reason",
                    call_unresolved_reason_label(fact.reason).to_string(),
                ),
                (
                    "site_key",
                    self.fact_stable_key(FactFamily::CallSite, fact.site.0),
                ),
                (
                    "caller_key",
                    self.fact_stable_key(FactFamily::Function, fact.caller.0),
                ),
            ]),
        )
    }

    fn domain_observation_metadata(&self, fact: &DomainObservationFact) -> FactMeta {
        let (precision, confidence) = domain_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::DomainObservation,
            POLINT_ABSTRACT_DOMAINS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", fact.status.as_str().to_string()),
                ("precision", fact.precision.as_str().to_string()),
                ("slot", fact.slot.as_str().to_string()),
                ("location", fact.location.as_str().to_string()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                (
                    "block",
                    fact.block
                        .map(|block| block.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    fact.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
                (
                    "place_key",
                    fact.place
                        .map(|place| self.fact_stable_key(FactFamily::Place, place.0))
                        .unwrap_or_else(none_value),
                ),
                ("value", fact.value.stable_parts().join("\n")),
            ]),
        )
    }

    fn domain_event_metadata(&self, fact: &DomainEventFact) -> FactMeta {
        let (precision, confidence) = domain_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::DomainEvent,
            POLINT_ABSTRACT_DOMAINS_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", fact.status.as_str().to_string()),
                ("precision", fact.precision.as_str().to_string()),
                (
                    "slot",
                    fact.slot
                        .map(|slot| slot.as_str().to_string())
                        .unwrap_or_else(none_value),
                ),
                ("reason", fact.reason.clone()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                (
                    "block",
                    fact.block
                        .map(|block| block.0.to_string())
                        .unwrap_or_else(none_value),
                ),
                (
                    "operation_key",
                    fact.operation
                        .map(|operation| {
                            self.fact_stable_key(FactFamily::MirOperation, operation.0)
                        })
                        .unwrap_or_else(none_value),
                ),
            ]),
        )
    }

    fn cfg_function_metadata(&self, fact: &CfgFunctionFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgFunction,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                (
                    "body_key",
                    self.fact_stable_key(FactFamily::MirBody, fact.body.0),
                ),
                ("language", language_label(fact.language).to_string()),
                ("path", self.path_for(fact.file)),
                ("span", span_metadata_value(&fact.span)),
            ]),
        )
    }

    fn cfg_node_metadata(&self, fact: &CfgNodeFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgNode,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("kind", cfg_node_kind_label(fact.kind).to_string()),
                (
                    "function_key",
                    self.fact_stable_key(FactFamily::CfgFunction, fact.cfg_function.0),
                ),
                ("operation_ordinal", fact.operation_ordinal.to_string()),
                ("span", option_span_metadata_value(fact.span.as_ref())),
            ]),
        )
    }

    fn cfg_block_metadata(&self, fact: &BasicBlockFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::BasicBlock,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("kind", basic_block_kind_label(fact.kind).to_string()),
                (
                    "function_key",
                    self.fact_stable_key(FactFamily::CfgFunction, fact.cfg_function.0),
                ),
                ("reachable", fact.reachable.to_string()),
                ("reverse_postorder", fact.reverse_postorder.to_string()),
            ]),
        )
    }

    fn cfg_edge_metadata(&self, fact: &CfgEdgeFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgEdge,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("kind", cfg_edge_kind_label(fact.kind).to_string()),
                (
                    "function_key",
                    self.fact_stable_key(FactFamily::CfgFunction, fact.cfg_function.0),
                ),
                ("from_block", fact.from_block.0.to_string()),
                ("to_block", fact.to_block.0.to_string()),
            ]),
        )
    }

    fn cfg_reachability_metadata(&self, fact: &ReachabilityFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgReachability,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("block", fact.block.0.to_string()),
                ("reachable", fact.reachable.to_string()),
            ]),
        )
    }

    fn cfg_dominator_metadata(&self, fact: &DominatorFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgDominator,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("dominator", fact.dominator.0.to_string()),
                ("dominated", fact.dominated.0.to_string()),
                ("immediate", fact.immediate.to_string()),
            ]),
        )
    }

    fn cfg_postdominator_metadata(&self, fact: &PostDominatorFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgPostDominator,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("postdominator", fact.postdominator.0.to_string()),
                ("postdominated", fact.postdominated.0.to_string()),
                ("immediate", fact.immediate.to_string()),
            ]),
        )
    }

    fn cfg_control_dependence_metadata(&self, fact: &ControlDependenceFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::CfgControlDependence,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("view", cfg_view_label(fact.view).to_string()),
                ("edge", fact.controlling_edge.0.to_string()),
                (
                    "edge_kind",
                    cfg_edge_kind_label(fact.controlling_edge_kind).to_string(),
                ),
                ("controlled_block", fact.controlled_block.0.to_string()),
            ]),
        )
    }

    fn unsupported_control_flow_metadata(&self, fact: &UnsupportedControlFlowFact) -> FactMeta {
        let (precision, confidence) = cfg_status_metadata(fact.status, fact.precision);
        fact_meta_from_stable_key(
            FactFamily::UnsupportedControlFlow,
            CFG_PROVIDER_ID,
            precision,
            confidence,
            fact.stable_key.clone(),
            stable_parts([
                ("status", cfg_status_label(fact.status).to_string()),
                ("precision", cfg_precision_label(fact.precision).to_string()),
                ("language", language_label(fact.language).to_string()),
                ("path", self.path_for(fact.file)),
                ("span", span_metadata_value(&fact.span)),
                ("construct", fact.construct.clone()),
                ("source_evidence", fact.source_evidence.clone()),
            ]),
        )
    }

    fn fact_stable_key(&self, family: FactFamily, run_id: u64) -> String {
        self.metadata_for(FactRef::new(family, run_id))
            .map(|metadata| metadata.stable_key.clone())
            .unwrap_or_else(|| format!("<missing:{}:{run_id}>", family.label()))
    }

    fn source_file_key(&self, file: FileId) -> String {
        self.metadata_for(FactRef::new(FactFamily::SourceFile, u64::from(file.0)))
            .map(|metadata| metadata.stable_key.clone())
            .unwrap_or_else(|| self.path_for(file).replace('\\', "/"))
    }

    fn option_source_file_key(&self, file: Option<FileId>) -> String {
        file.map(|file| self.source_file_key(file))
            .unwrap_or_else(none_value)
    }

    fn function_key(&self, function: FunctionId, name: &str, span: &Span) -> String {
        self.metadata_for(FactRef::new(FactFamily::Function, function.0))
            .map(|metadata| metadata.stable_key.clone())
            .unwrap_or_else(|| {
                stable_key_from_parts(
                    FactFamily::Function,
                    &[
                        ("path", self.path_for(span.file)),
                        ("name", name.to_string()),
                        ("span", span_metadata_value(span)),
                    ],
                )
            })
    }

    fn ts_component_metadata(&self, fact: &TsComponentFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::TsComponent,
            TS_SYNTAX_PROVIDER_ID,
            FactPrecision::Heuristic,
            FactConfidence::Medium,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([("function", option_function_id(fact.function))]),
        )
    }

    fn ts_class_metadata(&self, fact: &TsClassFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::TsClass,
            TS_SYNTAX_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([
                ("is_exported", fact.is_exported.to_string()),
                ("is_component_like", fact.is_component_like.to_string()),
            ]),
        )
    }

    fn string_literal_metadata(&self, fact: &StringLiteralFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::StringLiteral,
            syntax_provider_for_language(fact.language),
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("language", language_label(fact.language).to_string()),
                ("value", fact.value.clone()),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([]),
        )
    }

    fn jsx_attribute_metadata(&self, fact: &JsxAttributeFact) -> FactMeta {
        fact_meta_from_parts(
            FactFamily::JsxAttribute,
            TS_SYNTAX_PROVIDER_ID,
            FactPrecision::Syntax,
            FactConfidence::High,
            stable_parts([
                ("path", self.path_for(fact.file)),
                ("name", fact.name.clone()),
                ("value", option_string(fact.value.as_deref())),
                ("span", span_metadata_value(&fact.span)),
            ]),
            stable_parts([]),
        )
    }
}

fn option_file_path(db: &AnalysisDb, file: Option<FileId>) -> String {
    file.map(|file| db.path_for(file))
        .unwrap_or_else(none_value)
}

#[cfg(test)]
mod tests {
    use super::rule::line_col;
    use super::*;
    use crate::analysis::evidence::facts::{
        EvidenceConfidence, EvidenceProvenance, EvidenceStatus, EvidenceValidation,
    };
    use crate::analysis::extensions::sinks::{ExtensionFactConfidence, ExtensionFactPrecision};
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId, UnsupportedId};
    use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
    use crate::analysis::mir::op::{
        AssignMode, ConservativeAction, MirOperation, MirOperationKind, MirValue,
        UnsupportedDomain, UnsupportedPrecision, UnsupportedSemanticFact,
    };
    use crate::analysis::places::{PlaceFact, PlaceRoot, PlaceStatus};
    use crate::analysis_kernel::{
        FactConfidence, FactFamily, FactPrecision, FactRef, ValidationStatus,
    };
    use crate::diagnostics::{Diagnostic, Severity, TextRange as DiagnosticRange};
    use crate::rule_error::RuleResult;
    use crate::sdk::facts::{
        BranchObligations, FactView, Functions, GoTests, Imports, JsxAttributes, Packages,
        SourceFiles, StringLiterals, TsClasses, TsComponents,
    };
    use crate::symbol_graph::semantic::{
        AliasFact, ExportFact, GeneratedSymbolFact, ResolutionFact, ScopeFact, ScopeId, ScopeKind,
        SemanticImportFact, SemanticStatus, StableExportIdentity,
    };
    use anyhow::anyhow;
    use proptest::prelude::*;
    use std::thread;
    use std::time::Duration;

    #[derive(Clone, Copy)]
    enum TestRuleBehavior {
        Report,
        Error,
        Panic,
        MetaPanic,
    }

    #[derive(Clone, Copy)]
    struct TestRule {
        id: &'static str,
        capabilities: Capabilities,
        severity: Severity,
        message: &'static str,
        fingerprint: &'static str,
        delay: Duration,
        behavior: TestRuleBehavior,
    }

    #[test]
    fn analysis_db_solver_budget_status_tracks_not_run_and_replacements() {
        let mut db = AnalysisDb::new();

        assert_eq!(db.solver_budget_status(), BudgetStatus::NotRun);
        assert!(db.solver_budget_reasons().is_empty());

        db.replace_solver_facts(SolverOutput::default())
            .expect("within-budget solver facts");
        assert_eq!(db.solver_budget_status(), BudgetStatus::WithinBudget);
        assert!(db.solver_budget_reasons().is_empty());

        db.replace_solver_facts(SolverOutput {
            budget_status: BudgetStatus::BudgetExceeded,
            budget_reasons: BTreeSet::from(["solver.max_steps".to_string()]),
            ..SolverOutput::default()
        })
        .expect("budget-exceeded solver facts");
        assert_eq!(db.solver_budget_status(), BudgetStatus::BudgetExceeded);
        assert_eq!(
            db.solver_budget_reasons(),
            &BTreeSet::from(["solver.max_steps".to_string()])
        );
    }

    impl TestRule {
        fn report(id: &'static str, severity: Severity, fingerprint: &'static str) -> Self {
            Self {
                id,
                capabilities: Capabilities::new().syntax(),
                severity,
                message: "test diagnostic",
                fingerprint,
                delay: Duration::ZERO,
                behavior: TestRuleBehavior::Report,
            }
        }

        fn with_capabilities(mut self, capabilities: Capabilities) -> Self {
            self.capabilities = capabilities;
            self
        }

        fn with_message(mut self, message: &'static str) -> Self {
            self.message = message;
            self
        }

        fn with_delay(mut self, delay: Duration) -> Self {
            self.delay = delay;
            self
        }

        fn error(id: &'static str) -> Self {
            Self {
                id,
                capabilities: Capabilities::new(),
                severity: Severity::Error,
                message: "rule returned an error",
                fingerprint: "error",
                delay: Duration::ZERO,
                behavior: TestRuleBehavior::Error,
            }
        }

        fn panic(id: &'static str) -> Self {
            Self {
                id,
                capabilities: Capabilities::new(),
                severity: Severity::Error,
                message: "rule panicked",
                fingerprint: "panic",
                delay: Duration::ZERO,
                behavior: TestRuleBehavior::Panic,
            }
        }

        fn meta_panic() -> Self {
            Self {
                id: "examples/meta-panic",
                capabilities: Capabilities::new(),
                severity: Severity::Error,
                message: "metadata panicked",
                fingerprint: "meta-panic",
                delay: Duration::ZERO,
                behavior: TestRuleBehavior::MetaPanic,
            }
        }

        fn into_rule(self) -> Rule {
            let meta_rule = self;
            let capabilities_rule = self;
            let run_rule = self;
            Rule::from_parts(
                move || meta_rule.meta(),
                move || capabilities_rule.capabilities,
                move |_db, ctx| run_rule.run(ctx),
            )
        }

        fn meta(self) -> RuleMeta {
            if matches!(self.behavior, TestRuleBehavior::MetaPanic) {
                panic!("intentional metadata panic");
            }

            RuleMeta {
                id: self.id.to_string(),
                description: format!("Test rule {}", self.id),
                severity: self.severity,
                kind: RuleKind::Check,
            }
        }

        fn run(self, ctx: &mut RuleCtx<'_>) -> RuleResult {
            if !self.delay.is_zero() {
                thread::sleep(self.delay);
            }

            match self.behavior {
                TestRuleBehavior::Report => {
                    ctx.report(
                        Diagnostic::new(
                            self.id,
                            self.severity,
                            "src/main.go",
                            DiagnosticRange::point(1, 1),
                            self.message,
                        )
                        .with_fingerprint(self.fingerprint),
                    );
                    Ok(())
                }
                TestRuleBehavior::Error => Err(anyhow!("intentional rule error").into()),
                TestRuleBehavior::Panic => panic!("intentional rule panic"),
                TestRuleBehavior::MetaPanic => panic!("intentional metadata panic"),
            }
        }
    }

    fn test_span(file: FileId, line: u32) -> Span {
        Span {
            file,
            start_byte: 0,
            end_byte: 1,
            start_line: line,
            start_col: 1,
            end_line: line,
            end_col: 2,
        }
    }

    fn test_scope(name: &str, file: FileId, status: SemanticStatus) -> ScopeFact {
        let scope_path = vec![name.to_string()];
        ScopeFact {
            id: ScopeId(99),
            language: Language::TypeScript,
            file: Some(file),
            package: None,
            module: None,
            parent: None,
            stable_key: ScopeFact::stable_key_for(
                Language::TypeScript,
                &scope_path,
                Some(format!("file:{}", file.0)),
                None,
                None,
                ScopeKind::Function,
                status,
            ),
            scope_path,
            kind: ScopeKind::Function,
            status,
        }
    }

    fn test_mir_body(id: u64, file: FileId, stable_key: &str) -> MirBody {
        MirBody {
            id: MirBodyId(id),
            language: Language::TypeScript,
            file,
            function: FunctionId(id),
            package: None,
            module: None,
            owner_stable_key: format!("function:{stable_key}"),
            span: test_span(file, 1),
            stable_key: stable_key.to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn test_place(id: u64, file: FileId, stable_key: &str) -> PlaceFact {
        PlaceFact {
            id: PlaceId(id),
            language: Language::TypeScript,
            file: Some(file),
            function: Some(FunctionId(0)),
            root: PlaceRoot::Local {
                function: FunctionId(0),
                name: stable_key.to_string(),
            },
            projections: Vec::new(),
            stable_key: stable_key.to_string(),
            status: PlaceStatus::Resolved,
        }
    }

    fn test_mir_operation(
        id: u64,
        body: MirBodyId,
        place: PlaceId,
        value: PlaceId,
        stable_key: &str,
    ) -> MirOperation {
        MirOperation {
            id: MirOpId(id),
            body,
            ordinal: id as u32,
            span: test_span(FileId(0), 1),
            kind: MirOperationKind::Assign {
                place,
                value: MirValue::Place(value),
                mode: AssignMode::Overwrite,
            },
            stable_key: stable_key.to_string(),
            status: MirStatus::Resolved,
        }
    }

    fn test_unsupported(stable_key: &str) -> UnsupportedSemanticFact {
        UnsupportedSemanticFact {
            id: UnsupportedId(
                stable_key
                    .bytes()
                    .fold(0_u64, |sum, byte| sum + u64::from(byte)),
            ),
            body: None,
            operation: None,
            language: Language::TypeScript,
            file: FileId(0),
            span: test_span(FileId(0), 1),
            construct: "dynamic-property".to_string(),
            source_evidence: "target[key]".to_string(),
            affected_places: Vec::new(),
            affected_domains: vec![UnsupportedDomain::Mir],
            conservative_action: ConservativeAction::HavocAffectedPlaces,
            precision: UnsupportedPrecision::Unsupported,
            status: MirStatus::Unsupported,
            stable_key: stable_key.to_string(),
        }
    }

    fn test_call_site(
        id: u64,
        file: FileId,
        caller: FunctionId,
        stable_key: &str,
    ) -> crate::analysis::calls::facts::CallSiteFact {
        use crate::analysis::calls::facts::{
            CallCallee, CallPrecision, CallSiteFact, CallSyntaxKind, CallTargetStatus,
        };

        CallSiteFact {
            in_throw: false,
            id: CallSiteId(id),
            language: Language::TypeScript,
            file,
            caller,
            owner_symbol: Some(SymbolId(caller.0 + 100)),
            body: MirBodyId(id),
            operation: MirOpId(id),
            span: test_span(file, 1),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: None,
                name: stable_key.to_string(),
            },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::Exact,
            stable_key: stable_key.to_string(),
        }
    }

    fn test_call_target(
        id: u64,
        site: CallSiteId,
        caller: FunctionId,
        stable_key: &str,
    ) -> crate::analysis::calls::facts::CallTargetFact {
        use crate::analysis::calls::facts::{
            CallAlgorithm, CallEdgeKind, CallPrecision, CallProvenance, CallTargetFact,
            CallTargetStatus,
        };

        CallTargetFact {
            id: crate::analysis::ids::CallTargetId(id),
            site,
            caller,
            target_function: Some(FunctionId(id + 10)),
            target_symbol: Some(SymbolId(id + 20)),
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::Exact,
            stable_key: stable_key.to_string(),
        }
    }

    fn test_unresolved_call(
        site: CallSiteId,
        caller: FunctionId,
        stable_key: &str,
    ) -> crate::analysis::calls::facts::UnresolvedCallFact {
        use crate::analysis::calls::facts::{
            CallAlgorithm, CallPrecision, CallProvenance, CallTargetStatus, UnresolvedCallFact,
            UnresolvedCallReason,
        };

        UnresolvedCallFact {
            site,
            caller,
            status: CallTargetStatus::Unresolved,
            reason: UnresolvedCallReason::FunctionValue,
            algorithm: CallAlgorithm::SyntaxOnly,
            provenance: CallProvenance::MirShape,
            precision: CallPrecision::Unknown,
            stable_key: stable_key.to_string(),
        }
    }

    mod call_fact_storage {
        use super::*;
        use crate::analysis::calls::store::CallOutput;

        #[test]
        fn replace_call_facts_removes_stale_rows_from_previous_run() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { first(); second(); }\n".to_string(),
            );
            let first = CallOutput {
                sites: vec![test_call_site(1, file, FunctionId(1), "call-site:first")],
                targets: vec![test_call_target(
                    1,
                    CallSiteId(1),
                    FunctionId(1),
                    "call-target:first",
                )],
                unresolved: vec![test_unresolved_call(
                    CallSiteId(1),
                    FunctionId(1),
                    "unresolved:first",
                )],
            };
            let second = CallOutput {
                sites: vec![test_call_site(2, file, FunctionId(2), "call-site:second")],
                targets: Vec::new(),
                unresolved: Vec::new(),
            };

            db.replace_call_facts(first).expect("first call replace");
            assert!(db.call_store().is_some());
            assert_eq!(db.call_sites_by_caller(FunctionId(1)).len(), 1);
            assert_eq!(db.call_targets_by_site(CallSiteId(1)).len(), 1);
            assert_eq!(db.outgoing_calls_by_function(FunctionId(1)).len(), 1);
            assert_eq!(db.outgoing_calls_by_symbol(SymbolId(101)).len(), 1);
            assert_eq!(db.incoming_calls_by_symbol(SymbolId(21)).len(), 1);
            assert_eq!(db.incoming_calls_by_function(FunctionId(11)).len(), 1);
            assert_eq!(
                db.unresolved_calls_by_reason(
                    crate::analysis::calls::facts::UnresolvedCallReason::FunctionValue,
                )
                .len(),
                1
            );
            assert_eq!(
                db.unresolved_calls_by_status(
                    crate::analysis::calls::facts::CallTargetStatus::Unresolved,
                )
                .len(),
                1
            );

            db.replace_call_facts(second).expect("second call replace");

            assert_eq!(db.call_sites()[0].stable_key, "call-site:second");
            assert!(db.call_targets().is_empty());
            assert!(db.unresolved_calls().is_empty());
        }
    }

    mod ts_object_model_storage {
        use super::*;
        use crate::ts::object_model::facts::{
            TsObjectAllocationFact, TsObjectAllocationId, TsObjectAllocationKind,
            TsObjectModelStatus, TsPropertyKey, TsPropertyKeyKind, TsPropertyReadFact,
            TsPropertyReadId, TsPropertyWriteFact, TsPropertyWriteId, TsPrototypeLinkFact,
            TsPrototypeLinkId, TsPrototypeLinkKind, TsReceiverBindingFact, TsReceiverBindingId,
            TsReceiverBindingKind,
        };
        use crate::ts::object_model::store::TsObjectModelOutput;

        #[test]
        fn replace_ts_object_model_facts_removes_stale_rows_from_previous_run() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "const holder = { target() {} }; holder.target();\n".to_string(),
            );

            db.replace_ts_object_model_facts(full_output(file, "first"))
                .expect("first object-model replace");
            assert_eq!(db.ts_object_allocations().len(), 1);
            assert_eq!(db.ts_property_writes().len(), 1);
            assert_eq!(db.ts_property_reads().len(), 1);
            assert_eq!(db.ts_receiver_bindings().len(), 1);
            assert_eq!(db.ts_prototype_links().len(), 1);
            assert!(
                db.ts_object_model_store()
                    .expect("object-model store")
                    .allocation_by_stable_key("object:first")
                    .is_some()
            );

            db.replace_ts_object_model_facts(allocation_only_output(file, "second"))
                .expect("second object-model replace");

            assert_eq!(db.ts_object_allocations().len(), 1);
            assert_eq!(db.ts_object_allocations()[0].id, TsObjectAllocationId(0));
            assert_eq!(db.ts_object_allocations()[0].stable_key, "object:second");
            assert!(db.ts_property_writes().is_empty());
            assert!(db.ts_property_reads().is_empty());
            assert!(db.ts_receiver_bindings().is_empty());
            assert!(db.ts_prototype_links().is_empty());
            let store = db.ts_object_model_store().expect("object-model store");
            assert!(store.allocation_by_stable_key("object:first").is_none());
            assert!(store.allocation_by_stable_key("object:second").is_some());
        }

        #[test]
        fn replace_ts_object_model_facts_rejects_duplicate_stable_keys() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "const holder = {};\n".to_string(),
            );

            let error = db
                .replace_ts_object_model_facts(TsObjectModelOutput {
                    allocations: vec![
                        allocation(file, "object:dup", 1),
                        allocation(file, "object:dup", 2),
                    ],
                    property_writes: Vec::new(),
                    property_reads: Vec::new(),
                    receiver_bindings: Vec::new(),
                    prototype_links: Vec::new(),
                })
                .expect_err("duplicate stable key should be rejected");

            assert_eq!(
                error.to_string(),
                "invalid semantic fact from `polint.ts.object_model`: duplicate object allocation stable key `object:dup`"
            );
        }

        fn full_output(file: FileId, suffix: &str) -> TsObjectModelOutput {
            TsObjectModelOutput {
                allocations: vec![allocation(file, &format!("object:{suffix}"), 10)],
                property_writes: vec![property_write(file, &format!("write:{suffix}"), suffix)],
                property_reads: vec![property_read(file, &format!("read:{suffix}"), suffix)],
                receiver_bindings: vec![receiver_binding(file, &format!("receiver:{suffix}"))],
                prototype_links: vec![prototype_link(file, &format!("prototype:{suffix}"), suffix)],
            }
        }

        fn allocation_only_output(file: FileId, suffix: &str) -> TsObjectModelOutput {
            TsObjectModelOutput {
                allocations: vec![allocation(file, &format!("object:{suffix}"), 20)],
                property_writes: Vec::new(),
                property_reads: Vec::new(),
                receiver_bindings: Vec::new(),
                prototype_links: Vec::new(),
            }
        }

        fn allocation(file: FileId, stable_key: &str, id: u64) -> TsObjectAllocationFact {
            TsObjectAllocationFact {
                id: TsObjectAllocationId(id),
                file,
                span: test_span(file, 1),
                stable_key: stable_key.to_string(),
                lexical_parent_key: Some("scope:module".to_string()),
                inventory_function: None,
                inventory_function_stable_key: None,
                inventory_callsite: None,
                inventory_callsite_stable_key: None,
                kind: TsObjectAllocationKind::ObjectLiteral,
                status: TsObjectModelStatus::resolved(),
            }
        }

        fn property_write(file: FileId, stable_key: &str, suffix: &str) -> TsPropertyWriteFact {
            TsPropertyWriteFact {
                id: TsPropertyWriteId(99),
                file,
                span: test_span(file, 2),
                stable_key: stable_key.to_string(),
                base_object_stable_key: format!("object:{suffix}"),
                property_key: property_key(),
                value_function: None,
                value_function_stable_key: Some(format!("function:{suffix}")),
                value_object_stable_key: None,
                status: TsObjectModelStatus::resolved(),
            }
        }

        fn property_read(file: FileId, stable_key: &str, suffix: &str) -> TsPropertyReadFact {
            TsPropertyReadFact {
                id: TsPropertyReadId(99),
                file,
                span: test_span(file, 3),
                stable_key: stable_key.to_string(),
                base_object_stable_key: format!("object:{suffix}"),
                property_key: property_key(),
                destination_stable_key: Some(format!("place:{suffix}")),
                callsite: None,
                callsite_stable_key: Some(format!("callsite:{suffix}")),
                status: TsObjectModelStatus::resolved(),
            }
        }

        fn receiver_binding(file: FileId, stable_key: &str) -> TsReceiverBindingFact {
            TsReceiverBindingFact {
                id: TsReceiverBindingId(99),
                file,
                span: test_span(file, 4),
                stable_key: stable_key.to_string(),
                kind: TsReceiverBindingKind::MethodCall,
                callsite: None,
                callsite_stable_key: Some("callsite:first".to_string()),
                callee_function: None,
                callee_function_stable_key: Some("function:first".to_string()),
                receiver_object_stable_key: Some("object:first".to_string()),
                receiver_place_stable_key: Some("place:holder".to_string()),
                lexical_parent_key: Some("scope:module".to_string()),
                status: TsObjectModelStatus::resolved(),
            }
        }

        fn prototype_link(file: FileId, stable_key: &str, suffix: &str) -> TsPrototypeLinkFact {
            TsPrototypeLinkFact {
                id: TsPrototypeLinkId(99),
                file,
                span: test_span(file, 5),
                stable_key: stable_key.to_string(),
                kind: TsPrototypeLinkKind::ClassPrototype,
                object_stable_key: format!("object:{suffix}"),
                prototype_stable_key: format!("object:{suffix}:prototype"),
                property_key: None,
                status: TsObjectModelStatus::resolved(),
            }
        }

        fn property_key() -> TsPropertyKey {
            TsPropertyKey {
                kind: TsPropertyKeyKind::Static,
                value: Some("target".to_string()),
            }
        }
    }

    mod call_fact_metadata {
        use super::*;
        use crate::analysis::calls::facts::{CallPrecision, CallTargetStatus};
        use crate::analysis::calls::store::CallOutput;

        #[test]
        fn replace_call_facts_records_metadata_provider_and_family_labels() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { run(); }\n".to_string(),
            );

            db.replace_call_facts(CallOutput {
                sites: vec![test_call_site(0, file, FunctionId(1), "call-site:metadata")],
                targets: vec![test_call_target(
                    0,
                    CallSiteId(0),
                    FunctionId(1),
                    "call-target:metadata",
                )],
                unresolved: vec![test_unresolved_call(
                    CallSiteId(0),
                    FunctionId(1),
                    "unresolved:metadata",
                )],
            })
            .expect("call replace");

            for family in [
                FactFamily::CallSite,
                FactFamily::CallTarget,
                FactFamily::UnresolvedCall,
            ] {
                let metadata = db
                    .metadata_for(FactRef::new(family, 0))
                    .expect("call metadata exists");
                assert_eq!(metadata.producer_id, "polint.calls");
                assert_eq!(metadata.layer_id, "polint.calls");
                assert_eq!(metadata.validation, ValidationStatus::NativeTrusted);
                assert!(matches!(
                    family.label(),
                    "CallSite" | "CallTarget" | "UnresolvedCall"
                ));
            }
        }

        #[test]
        fn call_metadata_maps_unknown_statuses_to_non_exact_precision() {
            let mut db = AnalysisDb::new();
            let file = db.add_file(
                PathBuf::from("src/app.ts"),
                "src/app.ts".to_string(),
                "function app() { target[key](); }\n".to_string(),
            );
            let mut site = test_call_site(0, file, FunctionId(1), "call-site:unsupported");
            site.status = CallTargetStatus::Unsupported;
            site.precision = CallPrecision::Unsupported;
            let mut target =
                test_call_target(0, CallSiteId(0), FunctionId(1), "call-target:setup-missing");
            target.status = CallTargetStatus::SetupMissing;
            target.precision = CallPrecision::Unknown;
            let unresolved =
                test_unresolved_call(CallSiteId(0), FunctionId(1), "unresolved:unknown");

            db.replace_call_facts(CallOutput {
                sites: vec![site],
                targets: vec![target],
                unresolved: vec![unresolved],
            })
            .expect("call replace");

            assert_ne!(
                db.metadata_for(FactRef::new(FactFamily::CallSite, 0))
                    .expect("call site metadata exists")
                    .precision,
                FactPrecision::Exact
            );
            assert_ne!(
                db.metadata_for(FactRef::new(FactFamily::CallTarget, 0))
                    .expect("call target metadata exists")
                    .precision,
                FactPrecision::Exact
            );
            assert_ne!(
                db.metadata_for(FactRef::new(FactFamily::UnresolvedCall, 0))
                    .expect("unresolved call metadata exists")
                    .precision,
                FactPrecision::Exact
            );
        }
    }

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
