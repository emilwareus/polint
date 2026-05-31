#![allow(
    dead_code,
    reason = "Phase 45 wires private TS direct binding analysis into semantic graph across sequential plans"
)]

use std::collections::BTreeMap;

use crate::analysis::ids::TsDirectBindingId;
use crate::ts::binding::facts::{
    TsDirectBindingFact, TsDirectBindingKind, TsDirectBindingReason, TsDirectBindingStatus,
};
use crate::ts::binding::store::TsDirectBindingOutput;
use crate::ts::inventory::facts::{
    TsCallsiteInventoryKind, TsInventoryCallsiteFact, TsInventoryFunctionFact, TsInventoryStatus,
};
use crate::ts::inventory::store::TsInventoryOutput;
use crate::ts::scope::facts::{TsBindingFact, TsBindingKind, TsBindingStatus};
use crate::ts::scope::store::TsScopeOutput;

pub(crate) fn resolve_direct_bindings(
    inventory: &TsInventoryOutput,
    scope: &TsScopeOutput,
) -> TsDirectBindingOutput {
    let index = DirectBindingIndex::new(inventory, scope);
    let mut rows = Vec::new();

    for callsite in &inventory.callsites {
        if let Some(reason) = unresolved_reason_for_callsite(callsite, &index) {
            rows.push(unresolved_row(callsite, reason));
            continue;
        }

        let Some(display_name) = callsite.display_name.as_deref() else {
            rows.push(unresolved_row(
                callsite,
                TsDirectBindingReason::LocalBindingMissing,
            ));
            continue;
        };
        let (lookup_name, kind) = lookup_name_and_kind(display_name);
        if let Some(target) = index.function_by_name.get(lookup_name).copied() {
            rows.push(resolved_row(callsite, target, None, kind));
            continue;
        }
        if let Some(alias) = index.alias_by_name.get(lookup_name).copied()
            && let Some(target_name) = alias.imported_name.as_deref()
            && let Some(target) = index.function_by_name.get(target_name).copied()
        {
            rows.push(resolved_row(
                callsite,
                target,
                Some(alias),
                TsDirectBindingKind::LocalAlias,
            ));
            continue;
        }

        rows.push(unresolved_row(
            callsite,
            TsDirectBindingReason::LocalBindingMissing,
        ));
    }

    TsDirectBindingOutput { bindings: rows }.normalized()
}

struct DirectBindingIndex<'a> {
    function_by_name: BTreeMap<&'a str, &'a TsInventoryFunctionFact>,
    alias_by_name: BTreeMap<&'a str, &'a TsBindingFact>,
    unsupported_by_name: BTreeMap<&'a str, &'a TsBindingFact>,
}

impl<'a> DirectBindingIndex<'a> {
    fn new(inventory: &'a TsInventoryOutput, scope: &'a TsScopeOutput) -> Self {
        Self {
            function_by_name: inventory
                .functions
                .iter()
                .filter_map(|function| {
                    function
                        .display_name
                        .as_deref()
                        .map(|name| (name, function))
                })
                .collect(),
            alias_by_name: scope
                .bindings
                .iter()
                .filter(|binding| binding.binding_kind == TsBindingKind::Alias)
                .map(|binding| (binding.name.as_str(), binding))
                .collect(),
            unsupported_by_name: scope
                .bindings
                .iter()
                .filter(|binding| binding.binding_kind == TsBindingKind::UnsupportedDynamic)
                .map(|binding| (binding.name.as_str(), binding))
                .collect(),
        }
    }
}

fn unresolved_reason_for_callsite(
    callsite: &TsInventoryCallsiteFact,
    index: &DirectBindingIndex<'_>,
) -> Option<TsDirectBindingReason> {
    if matches!(callsite.kind, TsCallsiteInventoryKind::DynamicImport)
        && matches!(callsite.status, TsInventoryStatus::Unresolved { .. })
    {
        return Some(TsDirectBindingReason::NonStringDynamicImport);
    }
    if callsite.display_name.is_none() {
        return Some(TsDirectBindingReason::ComputedProperty);
    }
    let display = callsite.display_name.as_deref()?;
    let (lookup_name, _) = lookup_name_and_kind(display);
    index
        .unsupported_by_name
        .get(lookup_name)
        .and_then(|binding| match &binding.status {
            TsBindingStatus::UnsupportedDynamic { reason } if reason.contains("function-token") => {
                Some(TsDirectBindingReason::TokenFlowRequired)
            }
            TsBindingStatus::UnsupportedDynamic { reason } if reason.contains("property-flow") => {
                Some(TsDirectBindingReason::PropertyFlowRequired)
            }
            TsBindingStatus::UnsupportedDynamic { reason } if reason.contains("prototype") => {
                Some(TsDirectBindingReason::PrototypeModelRequired)
            }
            TsBindingStatus::UnsupportedDynamic { reason } if reason.contains("receiver") => {
                Some(TsDirectBindingReason::ThisModelRequired)
            }
            _ => None,
        })
}

fn lookup_name_and_kind(display_name: &str) -> (&str, TsDirectBindingKind) {
    display_name
        .rsplit_once('.')
        .map(|(_, property)| (property, TsDirectBindingKind::NamespaceMember))
        .unwrap_or((display_name, TsDirectBindingKind::LocalFunction))
}

fn resolved_row(
    callsite: &TsInventoryCallsiteFact,
    target: &TsInventoryFunctionFact,
    binding: Option<&TsBindingFact>,
    kind: TsDirectBindingKind,
) -> TsDirectBindingFact {
    TsDirectBindingFact {
        id: TsDirectBindingId(0),
        callsite: callsite.id,
        callsite_stable_key: callsite.stable_key.clone(),
        target_function: Some(target.id),
        target_function_stable_key: Some(target.stable_key.clone()),
        scope_binding: binding.map(|binding| binding.id),
        scope_binding_stable_key: binding.map(|binding| binding.stable_key.clone()),
        resolved_import: None,
        module_node: None,
        kind,
        status: TsDirectBindingStatus::Resolved,
        reason: None,
        stable_key: direct_binding_key(
            &callsite.stable_key,
            target.stable_key.as_str(),
            kind,
            TsDirectBindingStatus::Resolved,
            None,
        ),
    }
}

fn unresolved_row(
    callsite: &TsInventoryCallsiteFact,
    reason: TsDirectBindingReason,
) -> TsDirectBindingFact {
    TsDirectBindingFact {
        id: TsDirectBindingId(0),
        callsite: callsite.id,
        callsite_stable_key: callsite.stable_key.clone(),
        target_function: None,
        target_function_stable_key: None,
        scope_binding: None,
        scope_binding_stable_key: None,
        resolved_import: None,
        module_node: None,
        kind: TsDirectBindingKind::LocalFunction,
        status: TsDirectBindingStatus::Unresolved,
        reason: Some(reason),
        stable_key: direct_binding_key(
            &callsite.stable_key,
            "<unresolved>",
            TsDirectBindingKind::LocalFunction,
            TsDirectBindingStatus::Unresolved,
            Some(reason),
        ),
    }
}

fn direct_binding_key(
    callsite_key: &str,
    target_key: &str,
    kind: TsDirectBindingKind,
    status: TsDirectBindingStatus,
    reason: Option<TsDirectBindingReason>,
) -> String {
    format!(
        "ts_direct_binding|callsite={callsite_key}|target={target_key}|kind={kind:?}|status={}|reason={}",
        status.as_str(),
        reason.map(TsDirectBindingReason::as_str).unwrap_or("none")
    )
}
