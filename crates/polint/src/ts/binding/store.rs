#![allow(
    dead_code,
    reason = "Phase 45 expands private TS direct binding store indexes across sequential plans"
)]

use std::collections::BTreeMap;

use crate::analysis::ids::{TsDirectBindingId, TsInventoryCallsiteId, TsInventoryFunctionId};
use crate::analysis_kernel::incremental::{Digest, DigestKind};
use crate::core::ModuleNodeId;
use crate::ts::binding::facts::{TsDirectBindingFact, TsDirectBindingReason};

pub(crate) const TS_DIRECT_BINDING_SCHEMA_LABEL: &str = "ts-direct-binding-facts-1";

pub(crate) fn ts_direct_binding_provider_parameter_digest() -> Digest {
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "ts_direct_binding_provider_parameters",
        &[
            TS_DIRECT_BINDING_SCHEMA_LABEL,
            "output=ts_direct_bindings",
            "output=direct_binding_statuses",
            "output=direct_binding_unresolved_reasons",
            "input=ts_syntax_output",
            "input=ts_inventory_output",
            "input=ts_scope_output",
            "input=module_graph_output",
            "lifecycle=tsconfig",
            "lifecycle=jsconfig",
            "lifecycle=tsconfig_extends",
            "lifecycle=package_json",
            "lifecycle=workspace_packages",
            "lifecycle=lockfile",
            "config=polint_config",
            "provider=ts_direct_binding_v1",
            "schema=ts_direct_binding_schema_v1",
        ],
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TsDirectBindingOutput {
    pub(crate) bindings: Vec<TsDirectBindingFact>,
}

impl TsDirectBindingOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.bindings.sort_by(|left, right| {
            (left.stable_key.as_str(), left.callsite_stable_key.as_str()).cmp(&(
                right.stable_key.as_str(),
                right.callsite_stable_key.as_str(),
            ))
        });
        for (index, binding) in self.bindings.iter_mut().enumerate() {
            binding.id = TsDirectBindingId(index as u64);
        }
        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct TsDirectBindingStore {
    output: TsDirectBindingOutput,
    bindings_by_callsite: BTreeMap<TsInventoryCallsiteId, Vec<usize>>,
    bindings_by_target_function: BTreeMap<TsInventoryFunctionId, Vec<usize>>,
    bindings_by_unresolved_reason: BTreeMap<TsDirectBindingReason, Vec<usize>>,
    bindings_by_module_node: BTreeMap<ModuleNodeId, Vec<usize>>,
    bindings_by_stable_key: BTreeMap<String, usize>,
}

impl TsDirectBindingStore {
    pub(crate) fn from_output(output: TsDirectBindingOutput) -> Self {
        let output = output.normalized();
        let mut store = Self {
            output,
            ..Self::default()
        };

        for (index, binding) in store.output.bindings.iter().enumerate() {
            store
                .bindings_by_callsite
                .entry(binding.callsite)
                .or_default()
                .push(index);
            if let Some(target_function) = binding.target_function {
                store
                    .bindings_by_target_function
                    .entry(target_function)
                    .or_default()
                    .push(index);
            }
            if let Some(reason) = binding.reason {
                store
                    .bindings_by_unresolved_reason
                    .entry(reason)
                    .or_default()
                    .push(index);
            }
            if let Some(module_node) = binding.module_node {
                store
                    .bindings_by_module_node
                    .entry(module_node)
                    .or_default()
                    .push(index);
            }
            store
                .bindings_by_stable_key
                .insert(binding.stable_key.clone(), index);
        }

        store
    }

    pub(crate) fn bindings(&self) -> &[TsDirectBindingFact] {
        &self.output.bindings
    }

    pub(crate) fn bindings_by_callsite(
        &self,
        callsite: TsInventoryCallsiteId,
    ) -> Vec<&TsDirectBindingFact> {
        self.binding_refs(self.bindings_by_callsite.get(&callsite))
    }

    pub(crate) fn bindings_by_target_function(
        &self,
        target_function: TsInventoryFunctionId,
    ) -> Vec<&TsDirectBindingFact> {
        self.binding_refs(self.bindings_by_target_function.get(&target_function))
    }

    pub(crate) fn bindings_by_unresolved_reason(
        &self,
        reason: TsDirectBindingReason,
    ) -> Vec<&TsDirectBindingFact> {
        self.binding_refs(self.bindings_by_unresolved_reason.get(&reason))
    }

    pub(crate) fn bindings_by_module_node(
        &self,
        module_node: ModuleNodeId,
    ) -> Vec<&TsDirectBindingFact> {
        self.binding_refs(self.bindings_by_module_node.get(&module_node))
    }

    pub(crate) fn binding_by_stable_key(&self, stable_key: &str) -> Option<&TsDirectBindingFact> {
        self.bindings_by_stable_key
            .get(stable_key)
            .map(|index| &self.output.bindings[*index])
    }

    fn binding_refs(&self, indexes: Option<&Vec<usize>>) -> Vec<&TsDirectBindingFact> {
        indexes.map_or_else(Vec::new, |indexes| {
            indexes
                .iter()
                .map(|&index| &self.output.bindings[index])
                .collect()
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::analysis::ids::{
        TsBindingId, TsDirectBindingId, TsInventoryCallsiteId, TsInventoryFunctionId,
    };
    use crate::analysis_kernel::incremental::{Digest, DigestKind};
    use crate::core::{ModuleNodeId, ResolvedImportId};
    use crate::ts::binding::facts::{
        TsDirectBindingKind, TsDirectBindingReason, TsDirectBindingStatus,
    };

    use super::*;

    #[test]
    fn normalized_assigns_dense_ids_after_stable_key_sort() {
        let output = TsDirectBindingOutput {
            bindings: vec![
                binding("binding:b", TsDirectBindingId(22), TsInventoryCallsiteId(2)),
                binding("binding:a", TsDirectBindingId(11), TsInventoryCallsiteId(1)),
            ],
        }
        .normalized();

        assert_eq!(
            output
                .bindings
                .iter()
                .map(|binding| (binding.stable_key.as_str(), binding.id.0))
                .collect::<Vec<_>>(),
            vec![("binding:a", 0), ("binding:b", 1)]
        );
    }

    #[test]
    fn store_indexes_callsite_target_reason_module_and_stable_key() {
        let mut resolved = binding(
            "binding:resolved",
            TsDirectBindingId(10),
            TsInventoryCallsiteId(3),
        );
        resolved.target_function = Some(TsInventoryFunctionId(7));
        resolved.module_node = Some(ModuleNodeId(9));

        let mut unresolved = binding(
            "binding:unresolved",
            TsDirectBindingId(11),
            TsInventoryCallsiteId(4),
        );
        unresolved.status = TsDirectBindingStatus::Unresolved;
        unresolved.reason = Some(TsDirectBindingReason::ImportedBindingUnresolved);

        let store = TsDirectBindingStore::from_output(TsDirectBindingOutput {
            bindings: vec![unresolved, resolved],
        });

        assert_eq!(store.bindings().len(), 2);
        assert_eq!(
            store.bindings_by_callsite(TsInventoryCallsiteId(3))[0].stable_key,
            "binding:resolved"
        );
        assert_eq!(
            store.bindings_by_target_function(TsInventoryFunctionId(7))[0].stable_key,
            "binding:resolved"
        );
        assert_eq!(
            store.bindings_by_module_node(ModuleNodeId(9))[0].stable_key,
            "binding:resolved"
        );
        assert_eq!(
            store.bindings_by_unresolved_reason(TsDirectBindingReason::ImportedBindingUnresolved)
                [0]
            .stable_key,
            "binding:unresolved"
        );
        assert_eq!(
            store
                .binding_by_stable_key("binding:resolved")
                .map(|binding| binding.callsite),
            Some(TsInventoryCallsiteId(3))
        );
    }

    #[test]
    fn provider_parameter_digest_names_phase_45_d12_inputs() {
        assert_eq!(
            ts_direct_binding_provider_parameter_digest(),
            Digest::from_parts(
                DigestKind::ProviderParameters,
                "ts_direct_binding_provider_parameters",
                &[
                    "ts-direct-binding-facts-1",
                    "output=ts_direct_bindings",
                    "output=direct_binding_statuses",
                    "output=direct_binding_unresolved_reasons",
                    "input=ts_syntax_output",
                    "input=ts_inventory_output",
                    "input=ts_scope_output",
                    "input=module_graph_output",
                    "lifecycle=tsconfig",
                    "lifecycle=jsconfig",
                    "lifecycle=tsconfig_extends",
                    "lifecycle=package_json",
                    "lifecycle=workspace_packages",
                    "lifecycle=lockfile",
                    "config=polint_config",
                    "provider=ts_direct_binding_v1",
                    "schema=ts_direct_binding_schema_v1",
                ],
            )
        );
    }

    fn binding(
        stable_key: &str,
        id: TsDirectBindingId,
        callsite: TsInventoryCallsiteId,
    ) -> TsDirectBindingFact {
        TsDirectBindingFact {
            id,
            callsite,
            callsite_stable_key: format!("callsite:{}", callsite.0),
            target_function: None,
            target_function_stable_key: None,
            scope_binding: Some(TsBindingId(1)),
            scope_binding_stable_key: Some("scope:binding".to_string()),
            resolved_import: Some(ResolvedImportId(2)),
            module_node: None,
            kind: TsDirectBindingKind::ImportedNamed,
            status: TsDirectBindingStatus::Resolved,
            reason: None,
            stable_key: stable_key.to_string(),
        }
    }
}
