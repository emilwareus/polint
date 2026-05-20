use std::collections::{BTreeMap, BTreeSet};

use crate::core::{ResolutionPrecision, ResolutionStatus, SymbolPrecision};

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
    #[expect(
        dead_code,
        reason = "MIR metadata families are introduced before provider wiring in later Phase 28 plans."
    )]
    MirStatement,
    #[expect(
        dead_code,
        reason = "MIR metadata families are introduced before provider wiring in later Phase 28 plans."
    )]
    MirTerminator,
    UnsupportedSemantic,
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
            Self::MirStatement => "MirStatement",
            Self::MirTerminator => "MirTerminator",
            Self::UnsupportedSemantic => "UnsupportedSemantic",
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
    pub(crate) stable_key: String,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct StableKeyConflict {
    pub(crate) family: FactFamily,
    pub(crate) stable_key: String,
    pub(crate) existing: FactRef,
    pub(crate) incoming: FactRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FactMetaInsert {
    Inserted,
    Idempotent,
    Conflict {
        family: FactFamily,
        stable_key: String,
        existing: FactRef,
        incoming: FactRef,
    },
}

#[derive(Debug, Default, Clone)]
pub(crate) struct FactMetaStore {
    rows: BTreeMap<FactRef, FactMeta>,
    stable_key_owners: BTreeMap<(FactFamily, String), StableKeyOwner>,
    stable_key_conflicts: BTreeSet<StableKeyConflict>,
}

impl FactMetaStore {
    pub(crate) fn insert(&mut self, reference: FactRef, meta: FactMeta) -> FactMetaInsert {
        let owner_key = (reference.family, meta.stable_key.clone());
        let result = if let Some(owner) = self.stable_key_owners.get(&owner_key) {
            if owner.reference == reference && owner.payload_digest == meta.payload_digest {
                FactMetaInsert::Idempotent
            } else {
                let conflict = StableKeyConflict {
                    family: reference.family,
                    stable_key: meta.stable_key.clone(),
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
            self.stable_key_owners.insert(
                owner_key,
                StableKeyOwner {
                    reference,
                    payload_digest: meta.payload_digest.clone(),
                },
            );
            FactMetaInsert::Inserted
        };

        self.rows.insert(reference, meta);
        result
    }

    pub(crate) fn remove_family(&mut self, family: FactFamily) {
        self.rows
            .retain(|reference, _metadata| reference.family != family);
        self.stable_key_owners
            .retain(|(owner_family, _stable_key), _owner| *owner_family != family);
        self.stable_key_conflicts
            .retain(|conflict| conflict.family != family);
    }

    #[cfg(test)]
    pub(crate) fn remove_for_test(&mut self, reference: FactRef) -> Option<FactMeta> {
        self.rows.remove(&reference)
    }

    pub(crate) fn get(&self, reference: FactRef) -> Option<&FactMeta> {
        self.rows.get(&reference)
    }

    pub(crate) fn stable_key_conflicts(&self) -> impl Iterator<Item = &StableKeyConflict> {
        self.stable_key_conflicts.iter()
    }

    #[cfg(test)]
    pub(crate) fn stable_key_owner(
        &self,
        family: FactFamily,
        stable_key: &str,
    ) -> Option<&StableKeyOwner> {
        self.stable_key_owners
            .get(&(family, stable_key.to_string()))
    }

    pub(crate) fn rows(&self) -> impl Iterator<Item = (FactRef, &FactMeta)> {
        self.rows
            .iter()
            .map(|(reference, metadata)| (*reference, metadata))
    }
}

pub(crate) fn stable_key_from_parts(family: FactFamily, parts: &[(&str, String)]) -> String {
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
    key
}

pub(crate) fn resolution_metadata(
    precision: ResolutionPrecision,
    status: ResolutionStatus,
) -> (FactPrecision, FactConfidence) {
    if matches!(status, ResolutionStatus::Dynamic) {
        return (FactPrecision::Heuristic, FactConfidence::Low);
    }

    match precision {
        ResolutionPrecision::ExactFile => (FactPrecision::Exact, FactConfidence::High),
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
        SymbolPrecision::ExactSemantic => (FactPrecision::Exact, FactConfidence::High),
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

    #[test]
    fn fact_ref_keeps_run_local_id_separate_from_stable_key() {
        let reference = FactRef::new(FactFamily::Import, 7);
        let metadata = FactMeta {
            stable_key: "import:stable".to_string(),
            producer_id: "polint.go.syntax",
            layer_id: "polint.go.syntax",
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: "payload".to_string(),
        };

        assert_eq!(reference.run_id, 7);
        assert_eq!(metadata.stable_key, "import:stable");
    }

    #[test]
    fn stable_key_from_parts_sorts_and_normalizes_length_prefixed_parts() {
        let first = stable_key_from_parts(
            FactFamily::Import,
            &[
                ("path", "src\\main.go".to_string()),
                ("import_path", "fmt".to_string()),
            ],
        );
        let second = stable_key_from_parts(
            FactFamily::Import,
            &[
                ("import_path", "fmt".to_string()),
                ("path", "src/main.go".to_string()),
            ],
        );

        assert_eq!(first, second);
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

        store.insert(first_ref, test_meta("import:key", "payload:a"));
        let result = store.insert(second_ref, test_meta("import:key", "payload:a"));

        assert_eq!(
            result,
            FactMetaInsert::Conflict {
                family: FactFamily::Import,
                stable_key: "import:key".to_string(),
                existing: first_ref,
                incoming: second_ref,
            }
        );
        assert_eq!(store.stable_key_conflicts().count(), 1);
        assert_eq!(
            store
                .stable_key_owner(FactFamily::Import, "import:key")
                .map(|owner| owner.reference),
            Some(first_ref)
        );
    }

    #[test]
    fn stable_key_conflict_insert_records_conflicts_deterministically() {
        let mut store = FactMetaStore::default();
        let import_owner_ref = FactRef::new(FactFamily::Import, 8);
        let import_conflict_ref = FactRef::new(FactFamily::Import, 3);

        store.insert(import_owner_ref, test_meta("import:key", "payload:a"));
        let result = store.insert(import_conflict_ref, test_meta("import:key", "payload:b"));
        store.insert(import_conflict_ref, test_meta("import:key", "payload:b"));

        let conflicts = store.stable_key_conflicts().collect::<Vec<_>>();

        assert_eq!(
            result,
            FactMetaInsert::Conflict {
                family: FactFamily::Import,
                stable_key: "import:key".to_string(),
                existing: import_owner_ref,
                incoming: import_conflict_ref,
            }
        );
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].family, FactFamily::Import);
        assert_eq!(conflicts[0].stable_key, "import:key");
        assert_eq!(conflicts[0].existing, import_owner_ref);
        assert_eq!(conflicts[0].incoming, import_conflict_ref);
    }

    fn test_meta(stable_key: &str, payload_digest: &str) -> FactMeta {
        FactMeta {
            stable_key: stable_key.to_string(),
            producer_id: "polint.go.syntax",
            layer_id: "polint.go.syntax",
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: payload_digest.to_string(),
        }
    }
}
