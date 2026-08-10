use crate::core::StableKeyId;
use serde::{Deserialize, Serialize};

use crate::analysis::cfg::ids::BasicBlockId;
use crate::analysis::ids::{DomainEventId, DomainObservationId, MirBodyId, MirOpId, PlaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DomainSlot {
    Reachability,
    Nilness,
    Truthiness,
    Constants,
    Strings,
    Initializedness,
}

impl DomainSlot {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Reachability => "reachability",
            Self::Nilness => "nilness",
            Self::Truthiness => "truthiness",
            Self::Constants => "constants",
            Self::Strings => "strings",
            Self::Initializedness => "initializedness",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DomainLocation {
    FunctionEntry,
    BlockEntry,
    BeforeOperation,
    AfterOperation,
    BlockExit,
}

impl DomainLocation {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::FunctionEntry => "function_entry",
            Self::BlockEntry => "block_entry",
            Self::BeforeOperation => "before_operation",
            Self::AfterOperation => "after_operation",
            Self::BlockExit => "block_exit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DomainValue {
    Label(String),
    DigestParts(Vec<String>),
    TopReason(String),
}

impl DomainValue {
    pub(crate) fn stable_parts(&self) -> Vec<String> {
        match self {
            Self::Label(value) => vec![format!("label={value}")],
            Self::DigestParts(parts) => parts.clone(),
            Self::TopReason(reason) => vec![format!("top_reason={reason}")],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DomainStatus {
    Present,
    Top,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
}

impl DomainStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Top => "top",
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
            Self::SetupMissing => "setup_missing",
            Self::BudgetExceeded => "budget_exceeded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum DomainPrecision {
    ExactLocal,
    SetupAware,
    Conservative,
    Heuristic,
    Unknown,
    Unsupported,
}

impl DomainPrecision {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ExactLocal => "exact_local",
            Self::SetupAware => "setup_aware",
            Self::Conservative => "conservative",
            Self::Heuristic => "heuristic",
            Self::Unknown => "unknown",
            Self::Unsupported => "unsupported",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DomainObservationFact {
    pub(crate) id: DomainObservationId,
    pub(crate) body: MirBodyId,
    pub(crate) block: Option<BasicBlockId>,
    pub(crate) operation: Option<MirOpId>,
    pub(crate) place: Option<PlaceId>,
    pub(crate) slot: DomainSlot,
    pub(crate) location: DomainLocation,
    pub(crate) value: DomainValue,
    pub(crate) status: DomainStatus,
    pub(crate) precision: DomainPrecision,
    pub(crate) stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DomainEventFact {
    pub(crate) id: DomainEventId,
    pub(crate) body: MirBodyId,
    pub(crate) block: Option<BasicBlockId>,
    pub(crate) operation: Option<MirOpId>,
    pub(crate) slot: Option<DomainSlot>,
    pub(crate) status: DomainStatus,
    pub(crate) precision: DomainPrecision,
    pub(crate) reason: String,
    pub(crate) stable_key: StableKeyId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::ids::BasicBlockId;
    use crate::analysis::ids::{DomainObservationId, MirBodyId, MirOpId, PlaceId};

    #[test]
    fn domain_observation_fact_keeps_status_precision_and_stable_key_separate() {
        let fact = DomainObservationFact {
            id: DomainObservationId(7),
            body: MirBodyId(1),
            block: Some(BasicBlockId(2)),
            operation: Some(MirOpId(3)),
            place: Some(PlaceId(4)),
            slot: DomainSlot::Nilness,
            location: DomainLocation::AfterOperation,
            value: DomainValue::Label("nil".to_string()),
            status: DomainStatus::Present,
            precision: DomainPrecision::ExactLocal,
            stable_key: crate::core::stable_key_for_test("domain:after-op:nilness"),
        };

        assert_eq!(fact.id.0, 7);
        assert_eq!(fact.status, DomainStatus::Present);
        assert_eq!(fact.precision, DomainPrecision::ExactLocal);
        assert_eq!(
            crate::core::test_stable_key_interner()
                .resolve(fact.stable_key)
                .as_ref(),
            "domain:after-op:nilness"
        );
    }

    #[test]
    fn domain_statuses_preserve_unknown_top_and_setup_rows() {
        let statuses = [
            DomainStatus::Present,
            DomainStatus::Top,
            DomainStatus::Unknown,
            DomainStatus::Unsupported,
            DomainStatus::SetupMissing,
            DomainStatus::BudgetExceeded,
        ];

        assert_eq!(statuses.len(), 6);
    }
}
