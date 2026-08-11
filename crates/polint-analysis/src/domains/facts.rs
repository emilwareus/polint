use polint_core::StableKeyId;
use serde::{Deserialize, Serialize};

use crate::cfg::ids::BasicBlockId;
use crate::ids::{DomainEventId, DomainObservationId, MirBodyId, MirOpId, PlaceId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DomainSlot {
    Reachability,
    Nilness,
    Truthiness,
    Constants,
    Strings,
    Initializedness,
}

impl DomainSlot {
    pub fn as_str(self) -> &'static str {
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
pub enum DomainLocation {
    FunctionEntry,
    BlockEntry,
    BeforeOperation,
    AfterOperation,
    BlockExit,
}

impl DomainLocation {
    pub fn as_str(self) -> &'static str {
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
pub enum DomainValue {
    Label(String),
    DigestParts(Vec<String>),
    TopReason(String),
}

impl DomainValue {
    pub fn stable_parts(&self) -> Vec<String> {
        match self {
            Self::Label(value) => vec![format!("label={value}")],
            Self::DigestParts(parts) => parts.clone(),
            Self::TopReason(reason) => vec![format!("top_reason={reason}")],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DomainStatus {
    Present,
    Top,
    Unknown,
    Unsupported,
    SetupMissing,
    BudgetExceeded,
}

impl DomainStatus {
    pub fn as_str(self) -> &'static str {
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
pub enum DomainPrecision {
    ExactLocal,
    SetupAware,
    Conservative,
    Heuristic,
    Unknown,
    Unsupported,
}

impl DomainPrecision {
    pub fn as_str(self) -> &'static str {
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
pub struct DomainObservationFact {
    pub id: DomainObservationId,
    pub body: MirBodyId,
    pub block: Option<BasicBlockId>,
    pub operation: Option<MirOpId>,
    pub place: Option<PlaceId>,
    pub slot: DomainSlot,
    pub location: DomainLocation,
    pub value: DomainValue,
    pub status: DomainStatus,
    pub precision: DomainPrecision,
    pub stable_key: StableKeyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainEventFact {
    pub id: DomainEventId,
    pub body: MirBodyId,
    pub block: Option<BasicBlockId>,
    pub operation: Option<MirOpId>,
    pub slot: Option<DomainSlot>,
    pub status: DomainStatus,
    pub precision: DomainPrecision,
    pub reason: String,
    pub stable_key: StableKeyId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cfg::ids::BasicBlockId;
    use crate::ids::{DomainObservationId, MirBodyId, MirOpId, PlaceId};

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
            stable_key: polint_core::stable_key_for_test("domain:after-op:nilness"),
        };

        assert_eq!(fact.id.0, 7);
        assert_eq!(fact.status, DomainStatus::Present);
        assert_eq!(fact.precision, DomainPrecision::ExactLocal);
        assert_eq!(
            polint_core::test_stable_key_interner()
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
