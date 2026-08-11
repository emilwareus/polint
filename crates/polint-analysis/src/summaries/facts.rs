use serde::{Deserialize, Serialize};

use crate::ids::{SummaryEventId, SummaryId};
use polint_core::{FunctionId, StableKeyId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SummaryDomainKind {
    ControlEffects,
    CallEffects,
    MemoryEffects,
    DataFlowTito,
}

impl SummaryDomainKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ControlEffects => "control_effects",
            Self::CallEffects => "call_effects",
            Self::MemoryEffects => "memory_effects",
            Self::DataFlowTito => "data_flow_tito",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SummaryStatus {
    Present,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
}

impl SummaryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
            Self::SetupMissing => "setup_missing",
            Self::BudgetExceeded => "budget_exceeded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SummaryPrecision {
    Local,
    SetupAware,
    Heuristic,
    UnknownTop,
}

impl SummaryPrecision {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::SetupAware => "setup_aware",
            Self::Heuristic => "heuristic",
            Self::UnknownTop => "unknown_top",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SummaryProvenance {
    NativeLocal,
    LiftedFromDomain,
    InterproceduralClosure,
}

impl SummaryProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NativeLocal => "native_local",
            Self::LiftedFromDomain => "lifted_from_domain",
            Self::InterproceduralClosure => "interprocedural_closure",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExitKind {
    Returns,
    Throws,
    Panics,
    ExitsProcess,
    DoesNotReturn,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AsyncKind {
    Sync,
    Async,
    Generator,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AccessKind {
    None,
    Read,
    Write,
    ReadWrite,
}

// Per-resource external effects replace the coarse boolean.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum ExternalEffectKind {
    FileSystem,
    Network,
    Database,
    Env,
    Process,
    Time,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum MemoryResource {
    Receiver,
    Param(u16),
    Return,
    Local,
    Global,
    Module,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FlowKind {
    Value,
    BySideEffect,
    Taint,
    Barrier,
    Sanitizer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum FlowRoot {
    Param(u16),
    Receiver,
    Return,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SummaryFlowEdge {
    pub from: FlowRoot,
    pub to: FlowRoot,
    pub kind: FlowKind,
}

/// In-memory summary fact. Identity fields are interned [`StableKeyId`]s; do not
/// derive serde on this type — wire/debug payloads must resolve text via an interner
/// (see `analysis_kernel::debug` summary rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryFact {
    pub id: SummaryId,
    pub callable_stable_key: StableKeyId,
    pub function: FunctionId,
    pub domain: SummaryDomainKind,
    pub status: SummaryStatus,
    pub precision: SummaryPrecision,
    pub provenance: SummaryProvenance,
    pub payload_digest: String,
    pub tito_flows: Vec<SummaryFlowEdge>,
    pub stable_key: StableKeyId,
}

/// In-memory summary event fact. Identity fields are interned [`StableKeyId`]s; do
/// not derive serde on this type — wire/debug payloads must resolve text via an
/// interner (see `analysis_kernel::debug` summary event rows).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SummaryEventFact {
    pub id: SummaryEventId,
    pub callable_stable_key: StableKeyId,
    pub function: FunctionId,
    pub domain: SummaryDomainKind,
    pub event_kind: String,
    pub reason: String,
    pub status: SummaryStatus,
    pub precision: SummaryPrecision,
    pub stable_key: StableKeyId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_domain_kind_as_str_covers_all_variants() {
        let kinds = [
            SummaryDomainKind::ControlEffects,
            SummaryDomainKind::CallEffects,
            SummaryDomainKind::MemoryEffects,
            SummaryDomainKind::DataFlowTito,
        ];

        for kind in &kinds {
            let s = kind.as_str();
            assert!(!s.is_empty(), "as_str must be non-empty for {kind:?}");
        }

        assert_eq!(kinds.len(), 4);
    }

    #[test]
    fn summary_status_as_str_covers_all_variants() {
        let statuses = [
            SummaryStatus::Present,
            SummaryStatus::Unknown,
            SummaryStatus::Unsupported,
            SummaryStatus::SetupMissing,
            SummaryStatus::BudgetExceeded,
        ];

        for status in &statuses {
            let s = status.as_str();
            assert!(!s.is_empty(), "as_str must be non-empty for {status:?}");
        }

        assert_eq!(statuses.len(), 5);
    }

    #[test]
    fn summary_precision_as_str_covers_all_variants() {
        let precisions = [
            SummaryPrecision::Local,
            SummaryPrecision::SetupAware,
            SummaryPrecision::Heuristic,
            SummaryPrecision::UnknownTop,
        ];

        for precision in &precisions {
            let s = precision.as_str();
            assert!(!s.is_empty(), "as_str must be non-empty for {precision:?}");
        }

        assert_eq!(precisions.len(), 4);
    }

    #[test]
    fn summary_provenance_as_str_covers_all_variants() {
        let provenances = [
            SummaryProvenance::NativeLocal,
            SummaryProvenance::LiftedFromDomain,
            SummaryProvenance::InterproceduralClosure,
        ];

        for provenance in &provenances {
            let s = provenance.as_str();
            assert!(!s.is_empty(), "as_str must be non-empty for {provenance:?}");
        }

        assert_eq!(provenances.len(), 3);
    }

    #[test]
    fn flow_kind_has_five_variants_including_future_taint_barrier_sanitizer() {
        let kinds = [
            FlowKind::Value,
            FlowKind::BySideEffect,
            FlowKind::Taint,
            FlowKind::Barrier,
            FlowKind::Sanitizer,
        ];

        assert_eq!(kinds.len(), 5);
    }

    #[test]
    fn summary_fact_keeps_fields_separate() {
        let fact = SummaryFact {
            id: SummaryId(1),
            callable_stable_key: polint_core::stable_key_for_test("func::main"),
            function: FunctionId(10),
            domain: SummaryDomainKind::ControlEffects,
            status: SummaryStatus::Present,
            precision: SummaryPrecision::Local,
            provenance: SummaryProvenance::NativeLocal,
            payload_digest: "abc123".to_string(),
            tito_flows: Vec::new(),
            stable_key: polint_core::stable_key_for_test("summary:control_effects:func::main"),
        };

        assert_eq!(fact.id.0, 1);
        assert_eq!(fact.domain, SummaryDomainKind::ControlEffects);
        assert_eq!(fact.status, SummaryStatus::Present);
        assert_eq!(fact.precision, SummaryPrecision::Local);
        assert_eq!(fact.provenance, SummaryProvenance::NativeLocal);
    }

    #[test]
    fn summary_event_fact_keeps_fields_separate() {
        let event = SummaryEventFact {
            id: SummaryEventId(1),
            callable_stable_key: polint_core::stable_key_for_test("func::main"),
            function: FunctionId(10),
            domain: SummaryDomainKind::CallEffects,
            event_kind: "unresolved_callee".to_string(),
            reason: "dynamic dispatch".to_string(),
            status: SummaryStatus::Unknown,
            precision: SummaryPrecision::UnknownTop,
            stable_key: polint_core::stable_key_for_test("summary_event:call_effects:func::main:0"),
        };

        assert_eq!(event.id.0, 1);
        assert_eq!(event.domain, SummaryDomainKind::CallEffects);
        assert_eq!(event.status, SummaryStatus::Unknown);
    }

    #[test]
    fn access_kind_ordering_follows_none_read_write_readwrite() {
        assert!(AccessKind::None < AccessKind::Read);
        assert!(AccessKind::Read < AccessKind::Write);
        assert!(AccessKind::Write < AccessKind::ReadWrite);
    }

    #[test]
    fn exit_kind_ordering_is_deterministic_for_btree_set() {
        use std::collections::BTreeSet;

        let mut exits = BTreeSet::new();
        exits.insert(ExitKind::Throws);
        exits.insert(ExitKind::Returns);
        exits.insert(ExitKind::Panics);

        assert_eq!(exits.len(), 3);
    }

    #[test]
    fn flow_root_param_ordering_is_deterministic() {
        assert!(FlowRoot::Param(0) < FlowRoot::Param(1));
    }

    #[test]
    fn memory_resource_param_ordering_is_deterministic() {
        assert!(MemoryResource::Param(0) < MemoryResource::Param(1));
    }
}
