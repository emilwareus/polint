use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::calls::facts::{
    CallSiteFact, CallTargetFact, CallTargetStatus, UnresolvedCallFact, UnresolvedCallReason,
};
use crate::analysis::error::AnalysisError;
use crate::analysis::ids::CallSiteId;
use crate::core::{FunctionId, SymbolId};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct CallOutput {
    pub(crate) sites: Vec<CallSiteFact>,
    pub(crate) targets: Vec<CallTargetFact>,
    pub(crate) unresolved: Vec<UnresolvedCallFact>,
}

impl CallOutput {
    pub(crate) fn empty() -> Self {
        Self::default()
    }

    pub(crate) fn normalized(mut self) -> Self {
        self.sites.sort_by(|left, right| {
            (left.stable_key.as_str(), left.id).cmp(&(right.stable_key.as_str(), right.id))
        });
        self.targets.sort_by(|left, right| {
            (left.site, left.stable_key.as_str(), left.id).cmp(&(
                right.site,
                right.stable_key.as_str(),
                right.id,
            ))
        });
        self.unresolved.sort_by(|left, right| {
            (
                left.site,
                left.reason,
                left.status,
                left.stable_key.as_str(),
            )
                .cmp(&(
                    right.site,
                    right.reason,
                    right.status,
                    right.stable_key.as_str(),
                ))
        });
        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CallStore {
    output: CallOutput,
    sites_by_caller: BTreeMap<FunctionId, Vec<usize>>,
    targets_by_site: BTreeMap<CallSiteId, Vec<usize>>,
    outgoing_by_function: BTreeMap<FunctionId, Vec<usize>>,
    outgoing_by_symbol: BTreeMap<SymbolId, Vec<usize>>,
    incoming_by_symbol: BTreeMap<SymbolId, Vec<usize>>,
    incoming_by_function: BTreeMap<FunctionId, Vec<usize>>,
    unresolved_by_reason: BTreeMap<UnresolvedCallReason, Vec<usize>>,
    unresolved_by_status: BTreeMap<CallTargetStatus, Vec<usize>>,
}

impl CallStore {
    pub(crate) fn from_output(output: CallOutput) -> Result<Self, AnalysisError> {
        let output = output.normalized();
        let mut site_ids = BTreeSet::new();
        let mut site_owner_by_id = BTreeMap::new();

        for site in &output.sites {
            site_ids.insert(site.id);
            if let Some(owner_symbol) = site.owner_symbol {
                site_owner_by_id.insert(site.id, owner_symbol);
            }
        }

        for target in &output.targets {
            if !site_ids.contains(&target.site) {
                return Err(AnalysisError::InvalidFact {
                    provider: "polint.calls",
                    reason: format!(
                        "dangling call site {:?} for target `{}`",
                        target.site, target.stable_key
                    ),
                });
            }
        }

        for unresolved in &output.unresolved {
            if !site_ids.contains(&unresolved.site) {
                return Err(AnalysisError::InvalidFact {
                    provider: "polint.calls",
                    reason: format!(
                        "dangling call site {:?} for unresolved call `{}`",
                        unresolved.site, unresolved.stable_key
                    ),
                });
            }
        }

        let mut store = Self {
            output,
            ..Self::default()
        };

        for (index, site) in store.output.sites.iter().enumerate() {
            store
                .sites_by_caller
                .entry(site.caller)
                .or_default()
                .push(index);
        }

        for (index, target) in store.output.targets.iter().enumerate() {
            store
                .targets_by_site
                .entry(target.site)
                .or_default()
                .push(index);
            store
                .outgoing_by_function
                .entry(target.caller)
                .or_default()
                .push(index);
            if let Some(owner_symbol) = site_owner_by_id.get(&target.site).copied() {
                store
                    .outgoing_by_symbol
                    .entry(owner_symbol)
                    .or_default()
                    .push(index);
            }
            if let Some(target_symbol) = target.target_symbol {
                store
                    .incoming_by_symbol
                    .entry(target_symbol)
                    .or_default()
                    .push(index);
            }
            if let Some(target_function) = target.target_function {
                store
                    .incoming_by_function
                    .entry(target_function)
                    .or_default()
                    .push(index);
            }
        }

        for (index, unresolved) in store.output.unresolved.iter().enumerate() {
            store
                .unresolved_by_reason
                .entry(unresolved.reason)
                .or_default()
                .push(index);
            store
                .unresolved_by_status
                .entry(unresolved.status)
                .or_default()
                .push(index);
        }

        Ok(store)
    }

    pub(crate) fn sites(&self) -> &[CallSiteFact] {
        &self.output.sites
    }

    pub(crate) fn targets(&self) -> &[CallTargetFact] {
        &self.output.targets
    }

    pub(crate) fn unresolved(&self) -> &[UnresolvedCallFact] {
        &self.output.unresolved
    }

    pub(crate) fn sites_by_caller(&self, caller: FunctionId) -> Vec<&CallSiteFact> {
        self.site_refs(self.sites_by_caller.get(&caller))
    }

    pub(crate) fn targets_by_site(&self, site: CallSiteId) -> Vec<&CallTargetFact> {
        self.target_refs(self.targets_by_site.get(&site))
    }

    pub(crate) fn outgoing_by_function(&self, caller: FunctionId) -> Vec<&CallTargetFact> {
        self.target_refs(self.outgoing_by_function.get(&caller))
    }

    pub(crate) fn outgoing_by_symbol(&self, caller_symbol: SymbolId) -> Vec<&CallTargetFact> {
        self.target_refs(self.outgoing_by_symbol.get(&caller_symbol))
    }

    pub(crate) fn incoming_by_symbol(&self, target_symbol: SymbolId) -> Vec<&CallTargetFact> {
        self.target_refs(self.incoming_by_symbol.get(&target_symbol))
    }

    pub(crate) fn incoming_by_function(&self, target_function: FunctionId) -> Vec<&CallTargetFact> {
        self.target_refs(self.incoming_by_function.get(&target_function))
    }

    pub(crate) fn unresolved_by_reason(
        &self,
        reason: UnresolvedCallReason,
    ) -> Vec<&UnresolvedCallFact> {
        self.unresolved_refs(self.unresolved_by_reason.get(&reason))
    }

    pub(crate) fn unresolved_by_status(
        &self,
        status: CallTargetStatus,
    ) -> Vec<&UnresolvedCallFact> {
        self.unresolved_refs(self.unresolved_by_status.get(&status))
    }

    fn site_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&CallSiteFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.sites[index])
                .collect()
        })
    }

    fn target_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&CallTargetFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.targets[index])
                .collect()
        })
    }

    fn unresolved_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&UnresolvedCallFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.unresolved[index])
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
        CallSyntaxKind, CallTargetFact, CallTargetStatus, UnresolvedCallFact, UnresolvedCallReason,
    };
    use crate::analysis::ids::{CallSiteId, CallTargetId, MirBodyId, MirOpId};
    use crate::core::{FileId, FunctionId, Language, Span, SymbolId};

    fn span() -> Span {
        Span::point(FileId(1), 1, 1)
    }

    fn site(id: u64, caller: u64, stable_key: &str) -> CallSiteFact {
        CallSiteFact {
            in_throw: false,
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

    fn unresolved(
        site: u64,
        caller: u64,
        reason: UnresolvedCallReason,
        stable_key: &str,
    ) -> UnresolvedCallFact {
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

        assert_eq!(store.sites().len(), 2);
        assert_eq!(store.targets().len(), 2);
        assert_eq!(store.unresolved().len(), 1);
        assert_eq!(store.sites_by_caller(FunctionId(1)).len(), 2);
        assert_eq!(
            store.targets_by_site(CallSiteId(1))[0].stable_key,
            "target-a"
        );
        assert_eq!(store.outgoing_by_function(FunctionId(1)).len(), 2);
        assert_eq!(store.outgoing_by_symbol(SymbolId(101)).len(), 2);
        assert_eq!(
            store.incoming_by_symbol(SymbolId(21))[0].stable_key,
            "target-a"
        );
        assert_eq!(
            store.incoming_by_function(FunctionId(11))[0].stable_key,
            "target-a"
        );
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

    #[test]
    fn empty_output_builds_empty_store() {
        let store = CallStore::from_output(CallOutput::empty()).expect("empty output is valid");

        assert!(store.sites().is_empty());
        assert!(store.targets().is_empty());
        assert!(store.unresolved().is_empty());
    }
}
