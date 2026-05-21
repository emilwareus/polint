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
            stable_key: "domain:after-op:nilness".to_string(),
        };

        assert_eq!(fact.id.0, 7);
        assert_eq!(fact.status, DomainStatus::Present);
        assert_eq!(fact.precision, DomainPrecision::ExactLocal);
        assert_eq!(fact.stable_key, "domain:after-op:nilness");
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
