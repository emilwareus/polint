#![allow(
    dead_code,
    reason = "Phase 45 expands private TS direct binding store indexes across sequential plans"
)]

use std::collections::BTreeMap;

use crate::analysis::ids::TsDirectBindingId;
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

pub(crate) fn ts_direct_binding_output_digest(output: &TsDirectBindingOutput) -> Digest {
    let mut parts = vec![format!(
        "parameters={}",
        ts_direct_binding_provider_parameter_digest()
    )];
    parts.extend(output.bindings.iter().map(|binding| {
        format!(
            "binding={} callsite={} target={} scope={} import={} module={} kind={:?} status={} reason={}",
            binding.stable_key,
            binding.callsite_stable_key,
            binding
                .target_function_stable_key
                .as_deref()
                .unwrap_or("none"),
            binding
                .scope_binding_stable_key
                .as_deref()
                .unwrap_or("none"),
            binding
                .resolved_import
                .map(|id| id.0.to_string())
                .unwrap_or_else(|| "none".to_string()),
            binding
                .module_node
                .map(|id| id.0.to_string())
                .unwrap_or_else(|| "none".to_string()),
            binding.kind,
            binding.status.as_str(),
            binding
                .reason
                .map(TsDirectBindingReason::as_str)
                .unwrap_or("none"),
        )
    }));
    if output.bindings.is_empty() {
        parts.push("ts_direct_binding_output=empty".to_string());
    }
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderOutput,
        "ts_direct_binding_output",
        &refs,
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
    bindings_by_callsite_stable_key: BTreeMap<String, Vec<usize>>,
    bindings_by_target_function_stable_key: BTreeMap<String, Vec<usize>>,
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
                .bindings_by_callsite_stable_key
                .entry(binding.callsite_stable_key.clone())
                .or_default()
                .push(index);
            if let Some(target_function) = binding.target_function_stable_key.clone() {
                store
                    .bindings_by_target_function_stable_key
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

    pub(crate) fn bindings_by_callsite_stable_key(
        &self,
        callsite_stable_key: &str,
    ) -> Vec<&TsDirectBindingFact> {
        self.binding_refs(
            self.bindings_by_callsite_stable_key
                .get(callsite_stable_key),
        )
    }

    pub(crate) fn bindings_by_target_function_stable_key(
        &self,
        target_function_stable_key: &str,
    ) -> Vec<&TsDirectBindingFact> {
        self.binding_refs(
            self.bindings_by_target_function_stable_key
                .get(target_function_stable_key),
        )
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
        resolved.target_function_stable_key = Some("function:7".to_string());
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
            store.bindings_by_callsite_stable_key("callsite:3")[0].stable_key,
            "binding:resolved"
        );
        assert_eq!(
            store.bindings_by_target_function_stable_key("function:7")[0].stable_key,
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
    fn store_indexes_global_output_by_stable_keys_not_dense_ids() {
        let mut first = binding(
            "binding:first",
            TsDirectBindingId(10),
            TsInventoryCallsiteId(0),
        );
        first.callsite_stable_key = "file:a:callsite:0".to_string();
        first.target_function = Some(TsInventoryFunctionId(0));
        first.target_function_stable_key = Some("file:a:function:0".to_string());

        let mut second = binding(
            "binding:second",
            TsDirectBindingId(11),
            TsInventoryCallsiteId(0),
        );
        second.callsite_stable_key = "file:b:callsite:0".to_string();
        second.target_function = Some(TsInventoryFunctionId(0));
        second.target_function_stable_key = Some("file:b:function:0".to_string());

        let store = TsDirectBindingStore::from_output(TsDirectBindingOutput {
            bindings: vec![first, second],
        });

        assert_eq!(
            store.bindings_by_callsite_stable_key("file:a:callsite:0")[0].stable_key,
            "binding:first"
        );
        assert_eq!(
            store.bindings_by_callsite_stable_key("file:b:callsite:0")[0].stable_key,
            "binding:second"
        );
        assert_eq!(
            store.bindings_by_target_function_stable_key("file:a:function:0")[0].stable_key,
            "binding:first"
        );
        assert_eq!(
            store.bindings_by_target_function_stable_key("file:b:function:0")[0].stable_key,
            "binding:second"
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

    #[test]
    fn output_digest_changes_for_binding_rows() {
        let empty = TsDirectBindingOutput::default();
        let populated = TsDirectBindingOutput {
            bindings: vec![binding(
                "binding:resolved",
                TsDirectBindingId(10),
                TsInventoryCallsiteId(3),
            )],
        }
        .normalized();

        assert_ne!(
            ts_direct_binding_output_digest(&empty),
            ts_direct_binding_output_digest(&populated)
        );
    }

    #[test]
    fn output_digest_changes_for_source_scope_module_and_status_inputs() {
        let base = TsDirectBindingOutput {
            bindings: vec![binding(
                "binding:resolved",
                TsDirectBindingId(10),
                TsInventoryCallsiteId(3),
            )],
        };
        let base_digest = ts_direct_binding_output_digest(&base);

        let mut changed_source = base.clone();
        changed_source.bindings[0].callsite_stable_key = "callsite:changed-source".to_string();
        assert_ne!(
            base_digest,
            ts_direct_binding_output_digest(&changed_source)
        );

        let mut changed_scope = base.clone();
        changed_scope.bindings[0].scope_binding_stable_key =
            Some("scope:changed-binding".to_string());
        assert_ne!(base_digest, ts_direct_binding_output_digest(&changed_scope));

        let mut changed_module = base.clone();
        changed_module.bindings[0].resolved_import = Some(ResolvedImportId(44));
        changed_module.bindings[0].module_node = Some(ModuleNodeId(45));
        assert_ne!(
            base_digest,
            ts_direct_binding_output_digest(&changed_module)
        );

        let mut changed_status = base;
        changed_status.bindings[0].status = TsDirectBindingStatus::Unresolved;
        changed_status.bindings[0].reason = Some(TsDirectBindingReason::ImportedBindingUnresolved);
        assert_ne!(
            base_digest,
            ts_direct_binding_output_digest(&changed_status)
        );
    }

    #[test]
    fn output_digest_preserves_hit_for_noop_row_order_change() {
        let first = TsDirectBindingOutput {
            bindings: vec![
                binding("binding:a", TsDirectBindingId(10), TsInventoryCallsiteId(3)),
                binding("binding:b", TsDirectBindingId(11), TsInventoryCallsiteId(4)),
            ],
        };
        let second = TsDirectBindingOutput {
            bindings: vec![
                binding("binding:b", TsDirectBindingId(11), TsInventoryCallsiteId(4)),
                binding("binding:a", TsDirectBindingId(10), TsInventoryCallsiteId(3)),
            ],
        };

        assert_eq!(
            ts_direct_binding_output_digest(&first),
            ts_direct_binding_output_digest(&second)
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
