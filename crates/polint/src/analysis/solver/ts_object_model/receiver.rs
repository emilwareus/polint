//! Receiver/`this` binding helpers for the JS/TS object-model solver.

use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::ids::SemanticNodeId;
use crate::ts::object_model::facts::TsReceiverBindingKind;

use super::inputs::TsObjectReceiverBinding;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReceiverBindingOutcome {
    pub(crate) receiver_by_callsite: BTreeMap<SemanticNodeId, SemanticNodeId>,
    pub(crate) evidence_by_callsite: BTreeMap<SemanticNodeId, Vec<String>>,
    pub(crate) deferred_reasons: BTreeSet<&'static str>,
}

pub(crate) fn resolve_receiver_bindings(
    bindings: &[TsObjectReceiverBinding],
) -> ReceiverBindingOutcome {
    let mut receiver_by_callsite = BTreeMap::new();
    let mut evidence_by_callsite: BTreeMap<SemanticNodeId, Vec<String>> = BTreeMap::new();
    let mut deferred_reasons = BTreeSet::new();

    for binding in bindings {
        match binding.kind {
            TsReceiverBindingKind::MethodCall
            | TsReceiverBindingKind::ConstructorInstance
            | TsReceiverBindingKind::BoundFunction
            | TsReceiverBindingKind::CallReceiver
            | TsReceiverBindingKind::ApplyReceiver => {
                let (Some(callsite), Some(receiver)) =
                    (binding.callsite_node, binding.receiver_object)
                else {
                    deferred_reasons.insert("ThisModelRequired");
                    continue;
                };
                receiver_by_callsite.insert(callsite, receiver);
                evidence_by_callsite
                    .entry(callsite)
                    .or_default()
                    .push(binding.stable_key.clone());
            }
            TsReceiverBindingKind::LexicalThis => {
                if let Some(callsite) = binding.callsite_node {
                    evidence_by_callsite
                        .entry(callsite)
                        .or_default()
                        .push(binding.stable_key.clone());
                }
            }
        }
    }

    ReceiverBindingOutcome {
        receiver_by_callsite,
        evidence_by_callsite,
        deferred_reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_call_binds_this_to_receiver_object() {
        let outcome = resolve_receiver_bindings(&[binding(
            TsReceiverBindingKind::MethodCall,
            Some(9),
            Some(10),
            "receiver:method",
        )]);

        assert_eq!(
            outcome.receiver_by_callsite.get(&SemanticNodeId(9)),
            Some(&SemanticNodeId(10))
        );
        assert_eq!(
            outcome.evidence_by_callsite[&SemanticNodeId(9)],
            vec!["receiver:method".to_string()]
        );
    }

    #[test]
    fn arrow_lexical_this_preserves_evidence_without_rebinding_receiver() {
        let outcome = resolve_receiver_bindings(&[binding(
            TsReceiverBindingKind::LexicalThis,
            Some(9),
            Some(10),
            "receiver:lexical",
        )]);

        assert!(
            !outcome
                .receiver_by_callsite
                .contains_key(&SemanticNodeId(9))
        );
        assert_eq!(
            outcome.evidence_by_callsite[&SemanticNodeId(9)],
            vec!["receiver:lexical".to_string()]
        );
    }

    #[test]
    fn constructor_bound_call_and_apply_bind_known_receivers() {
        for kind in [
            TsReceiverBindingKind::ConstructorInstance,
            TsReceiverBindingKind::BoundFunction,
            TsReceiverBindingKind::CallReceiver,
            TsReceiverBindingKind::ApplyReceiver,
        ] {
            let outcome =
                resolve_receiver_bindings(&[binding(kind, Some(9), Some(10), kind.as_str())]);
            assert_eq!(
                outcome.receiver_by_callsite.get(&SemanticNodeId(9)),
                Some(&SemanticNodeId(10)),
                "{kind:?} should bind a known receiver"
            );
        }
    }

    #[test]
    fn call_and_apply_defer_when_receiver_is_unknown() {
        let outcome = resolve_receiver_bindings(&[binding(
            TsReceiverBindingKind::CallReceiver,
            Some(9),
            None,
            "receiver:call",
        )]);

        assert!(outcome.deferred_reasons.contains("ThisModelRequired"));
        assert!(outcome.receiver_by_callsite.is_empty());
    }

    fn binding(
        kind: TsReceiverBindingKind,
        callsite: Option<u64>,
        receiver: Option<u64>,
        stable_key: &str,
    ) -> TsObjectReceiverBinding {
        TsObjectReceiverBinding {
            kind,
            callsite_node: callsite.map(SemanticNodeId),
            receiver_object: receiver.map(SemanticNodeId),
            stable_key: stable_key.to_string(),
        }
    }
}
