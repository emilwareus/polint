#![allow(dead_code, reason = "kept for private internal consumers")]

use serde::{Deserialize, Serialize};

use crate::internal_core::{ModuleNodeId, ResolvedImportId, StableKeyId};
use crate::ts::ids::{
    TsBindingId, TsDirectBindingId, TsInventoryCallsiteId, TsInventoryFunctionId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TsDirectBindingStatus {
    Resolved,
    Unresolved,
    Unsupported,
    External,
}

impl TsDirectBindingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Unresolved => "unresolved",
            Self::Unsupported => "unsupported",
            Self::External => "external",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TsDirectBindingReason {
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
    pub fn as_str(self) -> &'static str {
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

    /// Whether this unresolved direct-binding reason is eligible for the
    /// function-token solver handoff. Property/prototype/receiver modeling stays
    /// honestly unresolved until its own analysis family exists.
    pub fn is_function_token_handoff(self) -> bool {
        matches!(self, Self::TokenFlowRequired)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TsDirectBindingKind {
    LocalFunction,
    LocalAlias,
    NamespaceMember,
    ImportedNamed,
    ImportedDefault,
    ImportedNamespaceMember,
    ReExportedAlias,
    CommonJsRequireMember,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsDirectBindingFact {
    pub id: TsDirectBindingId,
    pub callsite: TsInventoryCallsiteId,
    pub callsite_stable_key: StableKeyId,
    pub target_function: Option<TsInventoryFunctionId>,
    pub target_function_stable_key: Option<StableKeyId>,
    pub scope_binding: Option<TsBindingId>,
    pub scope_binding_stable_key: Option<StableKeyId>,
    pub resolved_import: Option<ResolvedImportId>,
    pub module_node: Option<ModuleNodeId>,
    pub kind: TsDirectBindingKind,
    pub status: TsDirectBindingStatus,
    pub reason: Option<TsDirectBindingReason>,
    pub stable_key: StableKeyId,
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
    fn only_token_flow_reason_enters_function_token_handoff() {
        assert!(TsDirectBindingReason::TokenFlowRequired.is_function_token_handoff());

        let non_token_model_reasons = [
            TsDirectBindingReason::PropertyFlowRequired,
            TsDirectBindingReason::PrototypeModelRequired,
            TsDirectBindingReason::ThisModelRequired,
        ];
        assert!(
            non_token_model_reasons
                .iter()
                .all(|reason| !reason.is_function_token_handoff())
        );
        assert_eq!(
            TsDirectBindingReason::TokenFlowRequired.as_str(),
            "token_flow_required"
        );
        assert_eq!(
            TsDirectBindingReason::PropertyFlowRequired.as_str(),
            "property_flow_required"
        );
        assert_eq!(
            TsDirectBindingReason::PrototypeModelRequired.as_str(),
            "prototype_model_required"
        );
        assert_eq!(
            TsDirectBindingReason::ThisModelRequired.as_str(),
            "this_model_required"
        );
    }

    #[test]
    fn binding_fact_references_existing_identities_without_module_payload_copy() {
        let interner = crate::internal_core::StableKeyInterner::default();
        let fact = TsDirectBindingFact {
            id: TsDirectBindingId(1),
            callsite: TsInventoryCallsiteId(2),
            callsite_stable_key: interner.intern("callsite:f"),
            target_function: Some(TsInventoryFunctionId(3)),
            target_function_stable_key: Some(interner.intern("function:f")),
            scope_binding: Some(TsBindingId(4)),
            scope_binding_stable_key: Some(interner.intern("binding:f")),
            resolved_import: Some(ResolvedImportId::from_raw(5)),
            module_node: Some(ModuleNodeId::from_raw(6)),
            kind: TsDirectBindingKind::ImportedNamed,
            status: TsDirectBindingStatus::Resolved,
            reason: None,
            stable_key: interner.intern("direct:f"),
        };

        assert_eq!(fact.resolved_import, Some(ResolvedImportId::from_raw(5)));
        assert_eq!(fact.module_node, Some(ModuleNodeId::from_raw(6)));
        assert_eq!(fact.status.as_str(), "resolved");
    }
}
