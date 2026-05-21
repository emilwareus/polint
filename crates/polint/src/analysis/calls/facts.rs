#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId, PlaceId};
    use crate::core::{FileId, FunctionId, Language, Span, SymbolId};
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use std::fmt::Debug;
    use std::hash::Hash;

    fn assert_small_id_contract<T>()
    where
        T: Debug
            + Clone
            + Copy
            + PartialEq
            + Eq
            + PartialOrd
            + Ord
            + Hash
            + Serialize
            + DeserializeOwned,
    {
    }

    #[test]
    fn call_target_id_is_copy_ordered_hashable_serializable_and_distinct_from_call_site_id() {
        assert_small_id_contract::<CallTargetId>();

        let site = CallSiteId(7);
        let target = CallTargetId(7);

        assert_eq!(site.0, target.0);
    }

    #[test]
    fn call_facts_keep_dense_ids_and_stable_keys_separate() {
        let site = CallSiteFact {
            id: CallSiteId(1),
            language: Language::TypeScript,
            file: FileId(2),
            caller: FunctionId(3),
            owner_symbol: Some(SymbolId(4)),
            body: MirBodyId(5),
            operation: MirOpId(6),
            span: Span::point(FileId(2), 10, 20),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: None,
                name: "run".to_string(),
            },
            receiver: None,
            arguments: vec![PlaceId(7)],
            result: Some(PlaceId(8)),
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::Exact,
            stable_key: "call-site:test".to_string(),
        };

        let target = CallTargetFact {
            id: CallTargetId(9),
            site: site.id,
            caller: site.caller,
            target_function: Some(FunctionId(10)),
            target_symbol: Some(SymbolId(11)),
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::Exact,
            stable_key: "call-target:test".to_string(),
        };

        assert_ne!(site.stable_key, target.stable_key);
        assert_eq!(site.id, target.site);
    }

    #[test]
    fn call_status_and_reason_vocabulary_covers_direct_and_unknown_forms() {
        let statuses = [
            CallTargetStatus::Resolved,
            CallTargetStatus::Ambiguous,
            CallTargetStatus::Unresolved,
            CallTargetStatus::Unsupported,
            CallTargetStatus::SetupMissing,
        ];
        let reasons = [
            UnresolvedCallReason::FunctionValue,
            UnresolvedCallReason::DynamicProperty,
            UnresolvedCallReason::InterfaceDispatch,
            UnresolvedCallReason::Eval,
            UnresolvedCallReason::CallApplyBind,
            UnresolvedCallReason::FrameworkDispatch,
        ];

        assert_eq!(statuses.len(), 5);
        assert_eq!(reasons.len(), 6);
    }
}
