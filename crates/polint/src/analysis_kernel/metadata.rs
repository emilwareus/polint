use std::collections::{BTreeMap, HashMap, HashSet};

use crate::core::{
    ResolutionPrecision, ResolutionStatus, StableKeyId, StableKeyInterner, SymbolPrecision,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FactFamily {
    SourceFile,
    Package,
    Function,
    Import,
    BranchObligation,
    Test,
    Coverage,
    TsComponent,
    TsClass,
    StringLiteral,
    JsxAttribute,
    ResolvedImport,
    ModuleNode,
    ModuleEdge,
    WorkspaceRoot,
    TopologyPackage,
    SourceSet,
    DependencyRequirement,
    ResolvedDependencyEdge,
    ImportToPackage,
    RepoTopologyOverlay,
    Scope,
    SemanticImport,
    Export,
    Alias,
    Resolution,
    GeneratedSymbol,
    StableExport,
    Symbol,
    Definition,
    Reference,
    FileMetric,
    FunctionMetric,
    ComplexityMetric,
    Place,
    MirBody,
    MirOperation,
    CfgFunction,
    CfgNode,
    BasicBlock,
    CfgEdge,
    CfgReachability,
    CfgDominator,
    CfgPostDominator,
    CfgControlDependence,
    UnsupportedControlFlow,
    CallSite,
    CallTarget,
    UnresolvedCall,
    RefinedCallEdge,
    DataFlowNode,
    DataFlowEdge,
    DataFlowModel,
    DataFlowBudget,
    EvidenceNode,
    EvidenceEdge,
    EvidenceBundle,
    EvidencePath,
    EvidenceSlice,
    EvidenceUnknown,
    EvidenceOmittedRegion,
    EvidenceReplayKey,
    DomainObservation,
    DomainEvent,
    SummaryControl,
    SummaryCall,
    SummaryMemory,
    SummaryTito,
    SummaryEvent,
    ExtensionFact,
    Entrypoint,
    TrustBoundary,
    DispatchEdge,
    UnresolvedFramework,
    Type,
    NarrowedType,
    Value,
    AllocationToken,
    AccessPath,
    PointsToConstraint,
    PointsToSet,
    AliasAnswer,
    AdaptationModel,
    /// Unified-solver derived-edge facts (GRAPH-04). Each row tags a
    /// solver-derived edge whose `DerivedEdgeProvenance` records its contributing
    /// fact IDs (total-ordered by stable ID), producing `ConstraintKind`, and the
    /// monotonic solver step. The provider that emits these into the kernel lands in
    /// Plan 03; the fact family + stable-key tagging are produced here in Plan 02.
    SolverDerivedEdge,
    #[expect(
        dead_code,
        reason = "Type/value alias event rows are reserved for later plans."
    )]
    TypeValueAliasEvent,
    #[expect(
        dead_code,
        reason = "MIR metadata families are introduced before provider wiring in later plans."
    )]
    MirStatement,
    #[expect(
        dead_code,
        reason = "MIR metadata families are introduced before provider wiring in later plans."
    )]
    MirTerminator,
    UnsupportedSemantic,
    /// Registry key for [`crate::go::semantic::store::GoSemanticStore`].
    GoSemantic,
    /// Registry key for [`crate::ts::object_model::store::TsObjectModelStore`].
    TsObjectModel,
    /// Registry key for [`crate::analysis::identity::store::IdentityStore`].
    Identity,
    /// Registry key for [`crate::analysis::reachability::store::ReachabilityStore`].
    Reachability,
    /// Registry key for [`crate::analysis::semantic_graph::store::SemanticGraphStore`].
    SemanticGraph,
}

impl FactFamily {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::SourceFile => "SourceFile",
            Self::Package => "Package",
            Self::Function => "Function",
            Self::Import => "Import",
            Self::BranchObligation => "BranchObligation",
            Self::Test => "Test",
            Self::Coverage => "Coverage",
            Self::TsComponent => "TsComponent",
            Self::TsClass => "TsClass",
            Self::StringLiteral => "StringLiteral",
            Self::JsxAttribute => "JsxAttribute",
            Self::ResolvedImport => "ResolvedImport",
            Self::ModuleNode => "ModuleNode",
            Self::ModuleEdge => "ModuleEdge",
            Self::WorkspaceRoot => "WorkspaceRoot",
            Self::TopologyPackage => "TopologyPackage",
            Self::SourceSet => "SourceSet",
            Self::DependencyRequirement => "DependencyRequirement",
            Self::ResolvedDependencyEdge => "ResolvedDependencyEdge",
            Self::ImportToPackage => "ImportToPackage",
            Self::RepoTopologyOverlay => "RepoTopologyOverlay",
            Self::Scope => "Scope",
            Self::SemanticImport => "SemanticImport",
            Self::Export => "Export",
            Self::Alias => "Alias",
            Self::Resolution => "Resolution",
            Self::GeneratedSymbol => "GeneratedSymbol",
            Self::StableExport => "StableExport",
            Self::Symbol => "Symbol",
            Self::Definition => "Definition",
            Self::Reference => "Reference",
            Self::FileMetric => "FileMetric",
            Self::FunctionMetric => "FunctionMetric",
            Self::ComplexityMetric => "ComplexityMetric",
            Self::Place => "Place",
            Self::MirBody => "MirBody",
            Self::MirOperation => "MirOperation",
            Self::CfgFunction => "CfgFunction",
            Self::CfgNode => "CfgNode",
            Self::BasicBlock => "BasicBlock",
            Self::CfgEdge => "CfgEdge",
            Self::CfgReachability => "CfgReachability",
            Self::CfgDominator => "CfgDominator",
            Self::CfgPostDominator => "CfgPostDominator",
            Self::CfgControlDependence => "CfgControlDependence",
            Self::UnsupportedControlFlow => "UnsupportedControlFlow",
            Self::CallSite => "CallSite",
            Self::CallTarget => "CallTarget",
            Self::UnresolvedCall => "UnresolvedCall",
            Self::RefinedCallEdge => "RefinedCallEdge",
            Self::DataFlowNode => "DataFlowNode",
            Self::DataFlowEdge => "DataFlowEdge",
            Self::DataFlowModel => "DataFlowModel",
            Self::DataFlowBudget => "DataFlowBudget",
            Self::EvidenceNode => "EvidenceNode",
            Self::EvidenceEdge => "EvidenceEdge",
            Self::EvidenceBundle => "EvidenceBundle",
            Self::EvidencePath => "EvidencePath",
            Self::EvidenceSlice => "EvidenceSlice",
            Self::EvidenceUnknown => "EvidenceUnknown",
            Self::EvidenceOmittedRegion => "EvidenceOmittedRegion",
            Self::EvidenceReplayKey => "EvidenceReplayKey",
            Self::DomainObservation => "DomainObservation",
            Self::DomainEvent => "DomainEvent",
            Self::SummaryControl => "SummaryControl",
            Self::SummaryCall => "SummaryCall",
            Self::SummaryMemory => "SummaryMemory",
            Self::SummaryTito => "SummaryTito",
            Self::SummaryEvent => "SummaryEvent",
            Self::ExtensionFact => "ExtensionFact",
            Self::Entrypoint => "Entrypoint",
            Self::TrustBoundary => "TrustBoundary",
            Self::DispatchEdge => "DispatchEdge",
            Self::UnresolvedFramework => "UnresolvedFramework",
            Self::Type => "Type",
            Self::NarrowedType => "NarrowedType",
            Self::Value => "Value",
            Self::AllocationToken => "AllocationToken",
            Self::AccessPath => "AccessPath",
            Self::PointsToConstraint => "PointsToConstraint",
            Self::PointsToSet => "PointsToSet",
            Self::AliasAnswer => "AliasAnswer",
            Self::AdaptationModel => "AdaptationModel",
            Self::SolverDerivedEdge => "SolverDerivedEdge",
            Self::TypeValueAliasEvent => "TypeValueAliasEvent",
            Self::MirStatement => "MirStatement",
            Self::MirTerminator => "MirTerminator",
            Self::UnsupportedSemantic => "UnsupportedSemantic",
            Self::GoSemantic => "GoSemantic",
            Self::TsObjectModel => "TsObjectModel",
            Self::Identity => "Identity",
            Self::Reachability => "Reachability",
            Self::SemanticGraph => "SemanticGraph",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct FactRef {
    pub(crate) family: FactFamily,
    pub(crate) run_id: u64,
}

impl FactRef {
    pub(crate) fn new(family: FactFamily, run_id: u64) -> Self {
        Self { family, run_id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MissingFactMeta {
    pub(crate) family: FactFamily,
    pub(crate) run_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FactMeta {
    pub(crate) stable_key: StableKeyId,
    pub(crate) producer_id: &'static str,
    pub(crate) layer_id: &'static str,
    pub(crate) precision: FactPrecision,
    pub(crate) confidence: FactConfidence,
    pub(crate) validation: ValidationStatus,
    pub(crate) payload_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FactPrecision {
    Exact,
    Syntax,
    SetupAware,
    Heuristic,
    Unresolved,
    Ambiguous,
    SetupMissing,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FactConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(
    dead_code,
    reason = "The validation vocabulary is broader than the native trusted path used before extension providers land."
)]
pub(crate) enum ValidationStatus {
    NativeTrusted,
    SchemaValidated,
    ReferentiallyValidated,
    SpanValidated,
    StableKeyValidated,
    ConflictRejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StableKeyOwner {
    pub(crate) reference: FactRef,
    pub(crate) payload_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct StableKeyConflict {
    pub(crate) family: FactFamily,
    pub(crate) stable_key: StableKeyId,
    pub(crate) existing: FactRef,
    pub(crate) incoming: FactRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FactMetaInsert {
    Inserted,
    Idempotent,
    Conflict {
        family: FactFamily,
        stable_key: StableKeyId,
        existing: FactRef,
        incoming: FactRef,
    },
}

#[derive(Debug, Default, Clone)]
struct FactFamilyRows {
    dense: Vec<Option<FactMeta>>,
    sparse: BTreeMap<u64, FactMeta>,
}

impl FactFamilyRows {
    const DENSE_GAP_LIMIT: usize = 1024;

    fn insert(&mut self, run_id: u64, meta: FactMeta) {
        if let Ok(index) = usize::try_from(run_id)
            && index <= self.dense.len().saturating_add(Self::DENSE_GAP_LIMIT)
        {
            if self.dense.len() <= index {
                self.dense.resize_with(index + 1, || None);
            }
            self.dense[index] = Some(meta);
            self.sparse.remove(&run_id);
        } else {
            self.sparse.insert(run_id, meta);
        }
    }

    #[cfg(test)]
    fn remove(&mut self, run_id: u64) -> Option<FactMeta> {
        if let Ok(index) = usize::try_from(run_id)
            && let Some(row) = self.dense.get_mut(index)
        {
            return row.take();
        }
        self.sparse.remove(&run_id)
    }

    fn get(&self, run_id: u64) -> Option<&FactMeta> {
        if let Ok(index) = usize::try_from(run_id)
            && let Some(row) = self.dense.get(index)
        {
            return row.as_ref();
        }
        self.sparse.get(&run_id)
    }

    fn rows(&self) -> impl Iterator<Item = (u64, &FactMeta)> {
        self.dense
            .iter()
            .enumerate()
            .filter_map(|(run_id, metadata)| {
                metadata.as_ref().map(|metadata| (run_id as u64, metadata))
            })
            .chain(
                self.sparse
                    .iter()
                    .map(|(run_id, metadata)| (*run_id, metadata)),
            )
    }
}

#[derive(Debug, Default, Clone)]
pub(crate) struct FactMetaStore {
    rows: BTreeMap<FactFamily, FactFamilyRows>,
    stable_key_owners: BTreeMap<FactFamily, HashMap<StableKeyId, StableKeyOwner>>,
    stable_key_conflicts: HashSet<StableKeyConflict>,
}

impl FactMetaStore {
    pub(crate) fn insert(&mut self, reference: FactRef, meta: FactMeta) -> FactMetaInsert {
        let family_owners = self.stable_key_owners.entry(reference.family).or_default();
        let result = if let Some(owner) = family_owners.get(&meta.stable_key) {
            if owner.reference == reference && owner.payload_digest == meta.payload_digest {
                FactMetaInsert::Idempotent
            } else {
                let conflict = StableKeyConflict {
                    family: reference.family,
                    stable_key: meta.stable_key,
                    existing: owner.reference,
                    incoming: reference,
                };
                self.stable_key_conflicts.insert(conflict.clone());
                FactMetaInsert::Conflict {
                    family: conflict.family,
                    stable_key: conflict.stable_key,
                    existing: conflict.existing,
                    incoming: conflict.incoming,
                }
            }
        } else {
            family_owners.insert(
                meta.stable_key,
                StableKeyOwner {
                    reference,
                    payload_digest: meta.payload_digest.clone(),
                },
            );
            FactMetaInsert::Inserted
        };

        self.rows
            .entry(reference.family)
            .or_default()
            .insert(reference.run_id, meta);
        result
    }

    pub(crate) fn remove_family(&mut self, family: FactFamily) {
        self.rows.remove(&family);
        self.stable_key_owners.remove(&family);
        self.stable_key_conflicts
            .retain(|conflict| conflict.family != family);
    }

    pub(crate) fn finish_family_insertions(&mut self, family: FactFamily) {
        self.stable_key_owners.remove(&family);
    }

    pub(crate) fn finish_all_insertions(&mut self) {
        self.stable_key_owners.clear();
    }

    #[cfg(test)]
    pub(crate) fn remove_for_test(&mut self, reference: FactRef) -> Option<FactMeta> {
        self.rows
            .get_mut(&reference.family)
            .and_then(|rows| rows.remove(reference.run_id))
    }

    pub(crate) fn get(&self, reference: FactRef) -> Option<&FactMeta> {
        self.rows
            .get(&reference.family)
            .and_then(|rows| rows.get(reference.run_id))
    }

    pub(crate) fn stable_key_conflicts(&self) -> impl Iterator<Item = &StableKeyConflict> {
        self.stable_key_conflicts.iter()
    }

    #[cfg(test)]
    pub(crate) fn stable_key_owner(
        &self,
        family: FactFamily,
        stable_key: StableKeyId,
    ) -> Option<&StableKeyOwner> {
        self.stable_key_owners
            .get(&family)
            .and_then(|owners| owners.get(&stable_key))
    }

    pub(crate) fn rows(&self) -> impl Iterator<Item = (FactRef, &FactMeta)> {
        self.rows.iter().flat_map(|(family, rows)| {
            rows.rows()
                .map(move |(run_id, metadata)| (FactRef::new(*family, run_id), metadata))
        })
    }
}

pub(crate) fn stable_key_from_parts(
    interner: &StableKeyInterner,
    family: FactFamily,
    parts: &[(&str, String)],
) -> StableKeyId {
    let mut normalized = parts
        .iter()
        .map(|(label, value)| (*label, value.replace('\\', "/")))
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.0.cmp(right.0));

    let mut key = length_prefixed(family.label());
    for (label, value) in normalized {
        key.push('|');
        key.push_str(&length_prefixed(label));
        key.push('=');
        key.push_str(&length_prefixed(&value));
    }
    interner.intern(key)
}

pub(crate) fn stable_key_text_from_parts(
    interner: &StableKeyInterner,
    family: FactFamily,
    parts: &[(&str, String)],
) -> String {
    interner
        .resolve(stable_key_from_parts(interner, family, parts))
        .to_string()
}

pub(crate) fn resolution_metadata(
    precision: ResolutionPrecision,
    status: ResolutionStatus,
) -> (FactPrecision, FactConfidence) {
    if matches!(status, ResolutionStatus::Dynamic) {
        return (FactPrecision::Heuristic, FactConfidence::Low);
    }

    match precision {
        ResolutionPrecision::ExactFile => (FactPrecision::SetupAware, FactConfidence::High),
        ResolutionPrecision::Package | ResolutionPrecision::ExternalPackage => {
            (FactPrecision::SetupAware, FactConfidence::High)
        }
        ResolutionPrecision::Heuristic => (FactPrecision::Heuristic, FactConfidence::Medium),
        ResolutionPrecision::None => resolution_status_metadata(status),
    }
}

pub(crate) fn resolution_status_metadata(
    status: ResolutionStatus,
) -> (FactPrecision, FactConfidence) {
    match status {
        ResolutionStatus::Resolved | ResolutionStatus::External => {
            (FactPrecision::SetupAware, FactConfidence::High)
        }
        ResolutionStatus::Unresolved => (FactPrecision::Unresolved, FactConfidence::Low),
        ResolutionStatus::SetupMissing => (FactPrecision::SetupMissing, FactConfidence::Low),
        ResolutionStatus::Dynamic => (FactPrecision::Heuristic, FactConfidence::Low),
        ResolutionStatus::Unsupported => (FactPrecision::Unsupported, FactConfidence::Low),
    }
}

pub(crate) fn symbol_metadata(precision: SymbolPrecision) -> (FactPrecision, FactConfidence) {
    match precision {
        SymbolPrecision::ExactSemantic => (FactPrecision::SetupAware, FactConfidence::High),
        SymbolPrecision::ExactLocal => (FactPrecision::Syntax, FactConfidence::High),
        SymbolPrecision::ModuleLinked => (FactPrecision::SetupAware, FactConfidence::High),
        SymbolPrecision::Heuristic => (FactPrecision::Heuristic, FactConfidence::Medium),
        SymbolPrecision::Unresolved => (FactPrecision::Unresolved, FactConfidence::Low),
        SymbolPrecision::Ambiguous => (FactPrecision::Ambiguous, FactConfidence::Low),
        SymbolPrecision::SetupMissing => (FactPrecision::SetupMissing, FactConfidence::Low),
        SymbolPrecision::Unsupported => (FactPrecision::Unsupported, FactConfidence::Low),
    }
}

fn length_prefixed(value: &str) -> String {
    format!("{}:{value}", value.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::stable_key_for_test;

    #[test]
    fn fact_ref_keeps_run_local_id_separate_from_stable_key() {
        let reference = FactRef::new(FactFamily::Import, 7);
        let metadata = FactMeta {
            stable_key: stable_key_for_test("import:stable"),
            producer_id: "polint.go.syntax",
            layer_id: "polint.go.syntax",
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: "payload".to_string(),
        };

        assert_eq!(reference.run_id, 7);
        assert_eq!(metadata.stable_key, stable_key_for_test("import:stable"));
    }

    #[test]
    fn stable_key_from_parts_sorts_and_normalizes_length_prefixed_parts() {
        let interner = crate::core::AnalysisDb::new().stable_key_interner();
        let first = stable_key_from_parts(
            &interner,
            FactFamily::Import,
            &[
                ("path", "src\\main.go".to_string()),
                ("import_path", "fmt".to_string()),
            ],
        );
        let second = stable_key_from_parts(
            &interner,
            FactFamily::Import,
            &[
                ("import_path", "fmt".to_string()),
                ("path", "src/main.go".to_string()),
            ],
        );

        assert_eq!(first, second);
        let first = interner.resolve(first);
        assert!(first.contains("6:Import"));
        assert!(first.contains("4:path=11:src/main.go"));
    }

    #[test]
    fn fact_meta_store_rows_are_deterministically_ordered() {
        let mut store = FactMetaStore::default();
        let later = FactRef::new(FactFamily::Import, 10);
        let earlier = FactRef::new(FactFamily::Import, 2);

        store.insert(later, test_meta("function", "function"));
        store.insert(earlier, test_meta("import", "import"));

        let rows = store.rows().collect::<Vec<_>>();

        assert_eq!(rows[0].0, earlier);
        assert_eq!(rows[1].0, later);
    }

    #[test]
    fn stable_key_conflict_insert_is_idempotent_when_same_reference_payload_repeats() {
        let mut store = FactMetaStore::default();
        let reference = FactRef::new(FactFamily::Import, 7);

        let first = store.insert(reference, test_meta("import:key", "payload:a"));
        let second = store.insert(reference, test_meta("import:key", "payload:a"));

        assert_eq!(first, FactMetaInsert::Inserted);
        assert_eq!(second, FactMetaInsert::Idempotent);
    }

    #[test]
    fn stable_key_conflict_insert_records_duplicate_reference_for_same_payload() {
        let mut store = FactMetaStore::default();
        let first_ref = FactRef::new(FactFamily::Import, 1);
        let second_ref = FactRef::new(FactFamily::Import, 2);
        let key = stable_key_for_test("import:key");

        store.insert(first_ref, test_meta("import:key", "payload:a"));
        let result = store.insert(second_ref, test_meta("import:key", "payload:a"));

        assert_eq!(
            result,
            FactMetaInsert::Conflict {
                family: FactFamily::Import,
                stable_key: key,
                existing: first_ref,
                incoming: second_ref,
            }
        );
        assert_eq!(store.stable_key_conflicts().count(), 1);
        assert_eq!(
            store
                .stable_key_owner(FactFamily::Import, key)
                .map(|owner| owner.reference),
            Some(first_ref)
        );
    }

    #[test]
    fn finish_family_insertions_keeps_rows_and_conflicts_but_drops_owner_index() {
        let mut store = FactMetaStore::default();
        let first_ref = FactRef::new(FactFamily::Import, 1);
        let second_ref = FactRef::new(FactFamily::Import, 2);
        let key = stable_key_for_test("import:key");

        store.insert(first_ref, test_meta("import:key", "payload:a"));
        store.insert(second_ref, test_meta("import:key", "payload:b"));
        store.finish_family_insertions(FactFamily::Import);

        assert!(store.get(first_ref).is_some());
        assert!(store.get(second_ref).is_some());
        assert_eq!(store.stable_key_conflicts().count(), 1);
        assert!(store.stable_key_owner(FactFamily::Import, key).is_none());
    }

    #[test]
    fn remove_family_drops_rows_owners_and_conflicts_for_that_family_only() {
        let mut store = FactMetaStore::default();
        let import_ref = FactRef::new(FactFamily::Import, 1);
        let duplicate_import_ref = FactRef::new(FactFamily::Import, 2);
        let function_ref = FactRef::new(FactFamily::Function, 1);
        let shared = stable_key_for_test("shared:key");
        let function_key = stable_key_for_test("function:key");

        store.insert(import_ref, test_meta("shared:key", "payload:a"));
        store.insert(duplicate_import_ref, test_meta("shared:key", "payload:b"));
        store.insert(function_ref, test_meta("function:key", "payload:c"));

        store.remove_family(FactFamily::Import);

        assert!(store.get(import_ref).is_none());
        assert!(store.get(duplicate_import_ref).is_none());
        assert!(store.stable_key_owner(FactFamily::Import, shared).is_none());
        assert_eq!(store.stable_key_conflicts().count(), 0);
        assert!(store.get(function_ref).is_some());
        assert!(
            store
                .stable_key_owner(FactFamily::Function, function_key)
                .is_some()
        );
    }

    #[test]
    fn fact_meta_store_accepts_sparse_high_run_ids_without_dense_resize() {
        let mut store = FactMetaStore::default();
        let high_ref = FactRef::new(FactFamily::Import, u64::MAX);
        let low_ref = FactRef::new(FactFamily::Import, 0);
        let high_key = stable_key_for_test("import:high");
        let low_key = stable_key_for_test("import:low");

        store.insert(high_ref, test_meta("import:high", "payload:high"));
        store.insert(low_ref, test_meta("import:low", "payload:low"));

        assert_eq!(store.get(high_ref).unwrap().stable_key, high_key);
        assert_eq!(store.get(low_ref).unwrap().stable_key, low_key);
        assert_eq!(
            store
                .rows()
                .map(|(reference, metadata)| (reference, metadata.stable_key))
                .collect::<Vec<_>>(),
            vec![(low_ref, low_key), (high_ref, high_key)]
        );
    }

    #[test]
    fn stable_key_conflict_insert_records_conflicts_deterministically() {
        let mut store = FactMetaStore::default();
        let import_owner_ref = FactRef::new(FactFamily::Import, 8);
        let import_conflict_ref = FactRef::new(FactFamily::Import, 3);
        let key = stable_key_for_test("import:key");

        store.insert(import_owner_ref, test_meta("import:key", "payload:a"));
        let result = store.insert(import_conflict_ref, test_meta("import:key", "payload:b"));
        store.insert(import_conflict_ref, test_meta("import:key", "payload:b"));

        let conflicts = store.stable_key_conflicts().collect::<Vec<_>>();

        assert_eq!(
            result,
            FactMetaInsert::Conflict {
                family: FactFamily::Import,
                stable_key: key,
                existing: import_owner_ref,
                incoming: import_conflict_ref,
            }
        );
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].family, FactFamily::Import);
        assert_eq!(conflicts[0].stable_key, key);
        assert_eq!(conflicts[0].existing, import_owner_ref);
        assert_eq!(conflicts[0].incoming, import_conflict_ref);
    }

    fn test_meta(stable_key: &str, payload_digest: &str) -> FactMeta {
        FactMeta {
            stable_key: stable_key_for_test(stable_key),
            producer_id: "polint.go.syntax",
            layer_id: "polint.go.syntax",
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: payload_digest.to_string(),
        }
    }
}
