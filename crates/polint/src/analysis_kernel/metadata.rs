use crate::core::{ResolutionPrecision, ResolutionStatus, SymbolPrecision};

pub(crate) use crate::analysis_api::{
    FactConfidence, FactFamily, FactMeta, FactMetaStore, FactPrecision, FactRef, MissingFactMeta,
    ValidationStatus, stable_key_from_parts, stable_key_text_from_parts,
};

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
        _ => (FactPrecision::Unsupported, FactConfidence::Low),
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
        _ => (FactPrecision::Unsupported, FactConfidence::Low),
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
        _ => (FactPrecision::Unsupported, FactConfidence::Low),
    }
}
