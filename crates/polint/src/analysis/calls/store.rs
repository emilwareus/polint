#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
        CallSyntaxKind, CallTargetFact, CallTargetStatus, UnresolvedCallFact,
        UnresolvedCallReason,
    };
    use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId};
    use crate::core::{FileId, FunctionId, Language, Span, SymbolId};

    fn span() -> Span {
        Span::point(FileId(1), 1, 1)
    }

    fn site(id: u64, caller: u64, stable_key: &str) -> CallSiteFact {
        CallSiteFact {
            id: CallSiteId(id),
            language: Language::TypeScript,
            file: FileId(1),
            caller: FunctionId(caller),
            owner_symbol: Some(SymbolId(caller + 100)),
            body: MirBodyId(caller),
            operation: MirOpId(id),
            span: span(),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: None,
                name: stable_key.to_string(),
            },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::Exact,
            stable_key: stable_key.to_string(),
        }
    }

    fn target(id: u64, site: u64, caller: u64, stable_key: &str) -> CallTargetFact {
        CallTargetFact {
            id: CallTargetId(id),
            site: CallSiteId(site),
            caller: FunctionId(caller),
            target_function: Some(FunctionId(id + 10)),
            target_symbol: Some(SymbolId(id + 20)),
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::Native,
            precision: CallPrecision::Exact,
            stable_key: stable_key.to_string(),
        }
    }

    fn unresolved(site: u64, caller: u64, reason: UnresolvedCallReason, stable_key: &str) -> UnresolvedCallFact {
        UnresolvedCallFact {
            site: CallSiteId(site),
            caller: FunctionId(caller),
            status: CallTargetStatus::Unresolved,
            reason,
            algorithm: CallAlgorithm::SyntaxOnly,
            provenance: CallProvenance::MirShape,
            precision: CallPrecision::Unknown,
            stable_key: stable_key.to_string(),
        }
    }

    #[test]
    fn normalized_sorts_call_rows_without_dropping_duplicates() {
        let output = CallOutput {
            sites: vec![site(2, 1, "b"), site(1, 1, "a"), site(3, 1, "a")],
            targets: vec![target(2, 2, 1, "target-b"), target(1, 1, 1, "target-a")],
            unresolved: vec![
                unresolved(2, 1, UnresolvedCallReason::DynamicProperty, "unresolved-b"),
                unresolved(1, 1, UnresolvedCallReason::FunctionValue, "unresolved-a"),
            ],
        }
        .normalized();

        assert_eq!(
            output
                .sites
                .iter()
                .map(|fact| fact.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "a", "b"]
        );
        assert_eq!(
            output
                .targets
                .iter()
                .map(|fact| fact.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["target-a", "target-b"]
        );
        assert_eq!(
            output
                .unresolved
                .iter()
                .map(|fact| fact.stable_key.as_str())
                .collect::<Vec<_>>(),
            vec!["unresolved-a", "unresolved-b"]
        );
    }

    #[test]
    fn from_output_builds_deterministic_call_indexes() {
        let store = CallStore::from_output(CallOutput {
            sites: vec![site(2, 1, "site-b"), site(1, 1, "site-a")],
            targets: vec![target(2, 2, 1, "target-b"), target(1, 1, 1, "target-a")],
            unresolved: vec![unresolved(
                2,
                1,
                UnresolvedCallReason::DynamicProperty,
                "unresolved-b",
            )],
        })
        .expect("call output should be valid");

        assert_eq!(store.sites_by_caller(FunctionId(1)).len(), 2);
        assert_eq!(store.targets_by_site(CallSiteId(1))[0].stable_key, "target-a");
        assert_eq!(store.outgoing_by_function(FunctionId(1)).len(), 2);
        assert_eq!(store.outgoing_by_symbol(SymbolId(101)).len(), 2);
        assert_eq!(store.incoming_by_symbol(SymbolId(21))[0].stable_key, "target-a");
        assert_eq!(store.incoming_by_function(FunctionId(11))[0].stable_key, "target-a");
        assert_eq!(
            store.unresolved_by_reason(UnresolvedCallReason::DynamicProperty)[0].stable_key,
            "unresolved-b"
        );
        assert_eq!(
            store.unresolved_by_status(CallTargetStatus::Unresolved)[0].stable_key,
            "unresolved-b"
        );
    }

    #[test]
    fn from_output_rejects_targets_without_matching_sites() {
        let error = CallStore::from_output(CallOutput {
            sites: vec![site(1, 1, "site-a")],
            targets: vec![target(2, 99, 1, "dangling-target")],
            unresolved: Vec::new(),
        })
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("dangling call site CallSiteId(99)")
        );
    }
}
