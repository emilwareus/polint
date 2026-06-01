#![allow(
    dead_code,
    reason = "Phase 45 wires private TS direct binding facts into semantic graph consumers across sequential plans"
)]

use serde::{Deserialize, Serialize};

use crate::analysis::ids::{
    TsBindingId, TsDirectBindingId, TsInventoryCallsiteId, TsInventoryFunctionId,
};
use crate::core::{ModuleNodeId, ResolvedImportId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsDirectBindingStatus {
    Resolved,
    Unresolved,
    Unsupported,
    External,
}

impl TsDirectBindingStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Unresolved => "unresolved",
            Self::Unsupported => "unsupported",
            Self::External => "external",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsDirectBindingReason {
    LocalBindingMissing,
    ImportedBindingUnresolved,
    NonStringDynamicImport,
    ComputedProperty,
    Eval,
    ExternalPackageUnresolved,
    UnsupportedNativeCallback,
    TokenFlowRequired,
    PropertyFlowRequired,
    PrototypeModelRequired,
    ThisModelRequired,
}

impl TsDirectBindingReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::LocalBindingMissing => "local_binding_missing",
            Self::ImportedBindingUnresolved => "imported_binding_unresolved",
            Self::NonStringDynamicImport => "non_string_dynamic_import",
            Self::ComputedProperty => "computed_property",
            Self::Eval => "eval",
            Self::ExternalPackageUnresolved => "external_package_unresolved",
            Self::UnsupportedNativeCallback => "unsupported_native_callback",
            Self::TokenFlowRequired => "token_flow_required",
            Self::PropertyFlowRequired => "property_flow_required",
            Self::PrototypeModelRequired => "prototype_model_required",
            Self::ThisModelRequired => "this_model_required",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsDirectBindingKind {
    LocalFunction,
    LocalAlias,
    NamespaceMember,
    ImportedNamed,
    ImportedDefault,
    ImportedNamespaceMember,
    ReExportedAlias,
    CommonJsRequireMember,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TsDirectBindingFact {
    pub(crate) id: TsDirectBindingId,
    pub(crate) callsite: TsInventoryCallsiteId,
    pub(crate) callsite_stable_key: String,
    pub(crate) target_function: Option<TsInventoryFunctionId>,
    pub(crate) target_function_stable_key: Option<String>,
    pub(crate) scope_binding: Option<TsBindingId>,
    pub(crate) scope_binding_stable_key: Option<String>,
    pub(crate) resolved_import: Option<ResolvedImportId>,
    pub(crate) module_node: Option<ModuleNodeId>,
    pub(crate) kind: TsDirectBindingKind,
    pub(crate) status: TsDirectBindingStatus,
    pub(crate) reason: Option<TsDirectBindingReason>,
    pub(crate) stable_key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_reason_model_covers_phase_45_static_boundary() {
        let reasons = [
            TsDirectBindingReason::LocalBindingMissing,
            TsDirectBindingReason::ImportedBindingUnresolved,
            TsDirectBindingReason::NonStringDynamicImport,
            TsDirectBindingReason::ComputedProperty,
            TsDirectBindingReason::Eval,
            TsDirectBindingReason::ExternalPackageUnresolved,
            TsDirectBindingReason::UnsupportedNativeCallback,
            TsDirectBindingReason::TokenFlowRequired,
            TsDirectBindingReason::PropertyFlowRequired,
            TsDirectBindingReason::PrototypeModelRequired,
            TsDirectBindingReason::ThisModelRequired,
        ];

        assert_eq!(reasons.len(), 11);
        assert!(reasons.iter().all(|reason| !reason.as_str().is_empty()));
    }

    #[test]
    fn binding_fact_references_existing_identities_without_module_payload_copy() {
        let fact = TsDirectBindingFact {
            id: TsDirectBindingId(1),
            callsite: TsInventoryCallsiteId(2),
            callsite_stable_key: "callsite:f".to_string(),
            target_function: Some(TsInventoryFunctionId(3)),
            target_function_stable_key: Some("function:f".to_string()),
            scope_binding: Some(TsBindingId(4)),
            scope_binding_stable_key: Some("binding:f".to_string()),
            resolved_import: Some(ResolvedImportId(5)),
            module_node: Some(ModuleNodeId(6)),
            kind: TsDirectBindingKind::ImportedNamed,
            status: TsDirectBindingStatus::Resolved,
            reason: None,
            stable_key: "direct:f".to_string(),
        };

        assert_eq!(fact.resolved_import, Some(ResolvedImportId(5)));
        assert_eq!(fact.module_node, Some(ModuleNodeId(6)));
        assert_eq!(fact.status.as_str(), "resolved");
    }
}
