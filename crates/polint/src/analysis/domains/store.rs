#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::cfg::ids::BasicBlockId;
    use crate::analysis::domains::facts::{
        DomainLocation, DomainObservationFact, DomainPrecision, DomainSlot, DomainStatus,
        DomainValue,
    };
    use crate::analysis::ids::{DomainObservationId, MirBodyId, MirOpId, PlaceId};
    use crate::analysis_kernel::{FactFamily, FactRef};
    use crate::core::AnalysisDb;

    fn observation(id: u64, stable_key: &str, status: DomainStatus) -> DomainObservationFact {
        DomainObservationFact {
            id: DomainObservationId(id),
            body: MirBodyId(1),
            block: Some(BasicBlockId(2)),
            operation: Some(MirOpId(id)),
            place: Some(PlaceId(4)),
            slot: DomainSlot::Nilness,
            location: DomainLocation::AfterOperation,
            value: DomainValue::Label(format!("{status:?}")),
            status,
            precision: DomainPrecision::Conservative,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn normalized_sorts_domain_rows_without_dropping_unknown_statuses() {
        let output = DomainOutput {
            observations: vec![
                observation(2, "domain:z", DomainStatus::Unsupported),
                observation(1, "domain:a", DomainStatus::Unknown),
                observation(3, "domain:m", DomainStatus::BudgetExceeded),
            ],
            events: Vec::new(),
        }
        .normalized();

        assert_eq!(
            output
                .observations
                .iter()
                .map(|fact| (fact.stable_key.as_str(), fact.status))
                .collect::<Vec<_>>(),
            vec![
                ("domain:a", DomainStatus::Unknown),
                ("domain:m", DomainStatus::BudgetExceeded),
                ("domain:z", DomainStatus::Unsupported),
            ]
        );
    }

    #[test]
    fn replace_abstract_domain_facts_removes_stale_rows_and_refreshes_metadata() {
        let mut db = AnalysisDb::new();
        db.replace_abstract_domain_facts(DomainOutput {
            observations: vec![observation(1, "domain:first", DomainStatus::Present)],
            events: Vec::new(),
        });
        db.replace_abstract_domain_facts(DomainOutput {
            observations: vec![observation(2, "domain:second", DomainStatus::Top)],
            events: Vec::new(),
        });

        assert_eq!(db.abstract_domain_observations().len(), 1);
        assert_eq!(
            db.abstract_domain_observations()[0].stable_key,
            "domain:second"
        );
        assert!(
            db.fact_meta()
                .get(FactRef::new(FactFamily::DomainObservation, 0))
                .is_some()
        );
        assert!(
            db.fact_meta()
                .get(FactRef::new(FactFamily::DomainObservation, 1))
                .is_none()
        );
    }
}
