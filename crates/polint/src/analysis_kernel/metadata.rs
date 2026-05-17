use std::collections::BTreeMap;

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
    Symbol,
    Definition,
    Reference,
    FileMetric,
    FunctionMetric,
    ComplexityMetric,
}

const FACT_FAMILY_VOCABULARY: &[FactFamily] = &[
    FactFamily::SourceFile,
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
    FactFamily::ResolvedImport,
    FactFamily::ModuleNode,
    FactFamily::ModuleEdge,
    FactFamily::Symbol,
    FactFamily::Definition,
    FactFamily::Reference,
    FactFamily::FileMetric,
    FactFamily::FunctionMetric,
    FactFamily::ComplexityMetric,
];

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
            Self::Symbol => "Symbol",
            Self::Definition => "Definition",
            Self::Reference => "Reference",
            Self::FileMetric => "FileMetric",
            Self::FunctionMetric => "FunctionMetric",
            Self::ComplexityMetric => "ComplexityMetric",
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

const FACT_PRECISION_VOCABULARY: &[FactPrecision] = &[
    FactPrecision::Exact,
    FactPrecision::Syntax,
    FactPrecision::SetupAware,
    FactPrecision::Heuristic,
    FactPrecision::Unresolved,
    FactPrecision::Ambiguous,
    FactPrecision::SetupMissing,
    FactPrecision::Unsupported,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FactConfidence {
    High,
    Medium,
    Low,
}

const FACT_CONFIDENCE_VOCABULARY: &[FactConfidence] = &[
    FactConfidence::High,
    FactConfidence::Medium,
    FactConfidence::Low,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ValidationStatus {
    NativeTrusted,
    SchemaValidated,
    ReferentiallyValidated,
    SpanValidated,
    StableKeyValidated,
    ConflictRejected,
}

const VALIDATION_STATUS_VOCABULARY: &[ValidationStatus] = &[
    ValidationStatus::NativeTrusted,
    ValidationStatus::SchemaValidated,
    ValidationStatus::ReferentiallyValidated,
    ValidationStatus::SpanValidated,
    ValidationStatus::StableKeyValidated,
    ValidationStatus::ConflictRejected,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FactMetaInsert {
    pub(crate) reference: FactRef,
    pub(crate) meta: FactMeta,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct FactMetaStore {
    rows: BTreeMap<FactRef, FactMeta>,
}

impl FactMetaStore {
    pub(crate) fn insert(&mut self, insert: FactMetaInsert) -> Option<FactMeta> {
        self.rows.insert(insert.reference, insert.meta)
    }

    pub(crate) fn remove_family(&mut self, family: FactFamily) {
        self.rows
            .retain(|reference, _metadata| reference.family != family);
    }

    #[cfg(test)]
    pub(crate) fn remove_for_test(&mut self, reference: FactRef) -> Option<FactMeta> {
        self.rows.remove(&reference)
    }

    pub(crate) fn get(&self, reference: FactRef) -> Option<&FactMeta> {
        self.rows.get(&reference)
    }

    #[cfg(test)]
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

pub(super) fn metadata_vocabulary_weight() -> usize {
    FACT_FAMILY_VOCABULARY
        .iter()
        .map(|family| family.label().len())
        .sum::<usize>()
        + FACT_PRECISION_VOCABULARY.len()
        + FACT_CONFIDENCE_VOCABULARY.len()
        + VALIDATION_STATUS_VOCABULARY.len()
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

        store.insert(FactMetaInsert {
            reference: later,
            meta: test_meta("function"),
        });
        store.insert(FactMetaInsert {
            reference: earlier,
            meta: test_meta("import"),
        });

        let rows = store.rows().collect::<Vec<_>>();

        assert_eq!(rows[0].0, earlier);
        assert_eq!(rows[1].0, later);
    }

    fn test_meta(stable_key: &str) -> FactMeta {
        FactMeta {
            stable_key: stable_key.to_string(),
            producer_id: "polint.go.syntax",
            layer_id: "polint.go.syntax",
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: stable_key.to_string(),
        }
    }
}
