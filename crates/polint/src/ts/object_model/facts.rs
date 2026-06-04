#![allow(
    dead_code,
    reason = "Phase 50 introduces private TS object-model facts before all solver consumers land"
)]

use serde::{Deserialize, Serialize};

use crate::analysis::ids::{TsInventoryCallsiteId, TsInventoryFunctionId};
use crate::core::{FileId, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsObjectAllocationId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsPropertyWriteId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsPropertyReadId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsReceiverBindingId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsPrototypeLinkId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TsObjectAllocationKind {
    ObjectLiteral,
    ArrayLiteral,
    FunctionObject,
    ClassObject,
    ClassInstance,
    PrototypeObject,
    ModuleObject,
    NativeFunctionObject,
}

impl TsObjectAllocationKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ObjectLiteral => "object_literal",
            Self::ArrayLiteral => "array_literal",
            Self::FunctionObject => "function_object",
            Self::ClassObject => "class_object",
            Self::ClassInstance => "class_instance",
            Self::PrototypeObject => "prototype_object",
            Self::ModuleObject => "module_object",
            Self::NativeFunctionObject => "native_function_object",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TsPropertyKeyKind {
    Static,
    Private,
    StringLiteral,
    NumericLiteral,
    ComputedBucket,
    UnknownBucket,
}

impl TsPropertyKeyKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Static => "static",
            Self::Private => "private",
            Self::StringLiteral => "string_literal",
            Self::NumericLiteral => "numeric_literal",
            Self::ComputedBucket => "computed_bucket",
            Self::UnknownBucket => "unknown_bucket",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct TsPropertyKey {
    pub(crate) kind: TsPropertyKeyKind,
    pub(crate) value: Option<String>,
}

impl TsPropertyKey {
    pub(crate) fn stable_label(&self) -> String {
        match self.value.as_deref() {
            Some(value) => format!("{}:{value}", self.kind.as_str()),
            None => self.kind.as_str().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsObjectModelStatus {
    Resolved,
    Unknown { reason: String },
    Unsupported { reason: String },
    BudgetExceeded { reason: String },
}

impl TsObjectModelStatus {
    pub(crate) fn resolved() -> Self {
        Self::Resolved
    }

    pub(crate) fn unknown(reason: impl Into<String>) -> Self {
        Self::Unknown {
            reason: reason.into(),
        }
    }

    pub(crate) fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported {
            reason: reason.into(),
        }
    }

    pub(crate) fn budget_exceeded(reason: impl Into<String>) -> Self {
        Self::BudgetExceeded {
            reason: reason.into(),
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Unknown { .. } => "unknown",
            Self::Unsupported { .. } => "unsupported",
            Self::BudgetExceeded { .. } => "budget_exceeded",
        }
    }

    pub(crate) fn reason(&self) -> Option<&str> {
        match self {
            Self::Resolved => None,
            Self::Unknown { reason }
            | Self::Unsupported { reason }
            | Self::BudgetExceeded { reason } => Some(reason.as_str()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TsReceiverBindingKind {
    MethodCall,
    LexicalThis,
    ConstructorInstance,
    BoundFunction,
    CallReceiver,
    ApplyReceiver,
}

impl TsReceiverBindingKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::MethodCall => "method_call",
            Self::LexicalThis => "lexical_this",
            Self::ConstructorInstance => "constructor_instance",
            Self::BoundFunction => "bound_function",
            Self::CallReceiver => "call_receiver",
            Self::ApplyReceiver => "apply_receiver",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TsPrototypeLinkKind {
    ClassPrototype,
    ClassExtends,
    ConstructorPrototypeProperty,
    SetPrototypeOf,
    ProtoProperty,
}

impl TsPrototypeLinkKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ClassPrototype => "class_prototype",
            Self::ClassExtends => "class_extends",
            Self::ConstructorPrototypeProperty => "constructor_prototype_property",
            Self::SetPrototypeOf => "set_prototype_of",
            Self::ProtoProperty => "proto_property",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TsObjectAllocationFact {
    pub(crate) id: TsObjectAllocationId,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) stable_key: String,
    pub(crate) lexical_parent_key: Option<String>,
    pub(crate) inventory_function: Option<TsInventoryFunctionId>,
    pub(crate) inventory_function_stable_key: Option<String>,
    pub(crate) inventory_callsite: Option<TsInventoryCallsiteId>,
    pub(crate) inventory_callsite_stable_key: Option<String>,
    pub(crate) kind: TsObjectAllocationKind,
    pub(crate) status: TsObjectModelStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TsPropertyWriteFact {
    pub(crate) id: TsPropertyWriteId,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) stable_key: String,
    pub(crate) base_object_stable_key: String,
    pub(crate) property_key: TsPropertyKey,
    pub(crate) value_function: Option<TsInventoryFunctionId>,
    pub(crate) value_function_stable_key: Option<String>,
    pub(crate) value_object_stable_key: Option<String>,
    pub(crate) status: TsObjectModelStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TsPropertyReadFact {
    pub(crate) id: TsPropertyReadId,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) stable_key: String,
    pub(crate) base_object_stable_key: String,
    pub(crate) property_key: TsPropertyKey,
    pub(crate) destination_stable_key: Option<String>,
    pub(crate) callsite: Option<TsInventoryCallsiteId>,
    pub(crate) callsite_stable_key: Option<String>,
    pub(crate) status: TsObjectModelStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TsReceiverBindingFact {
    pub(crate) id: TsReceiverBindingId,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) stable_key: String,
    pub(crate) kind: TsReceiverBindingKind,
    pub(crate) callsite: Option<TsInventoryCallsiteId>,
    pub(crate) callsite_stable_key: Option<String>,
    pub(crate) callee_function: Option<TsInventoryFunctionId>,
    pub(crate) callee_function_stable_key: Option<String>,
    pub(crate) receiver_object_stable_key: Option<String>,
    pub(crate) receiver_place_stable_key: Option<String>,
    pub(crate) lexical_parent_key: Option<String>,
    pub(crate) status: TsObjectModelStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TsPrototypeLinkFact {
    pub(crate) id: TsPrototypeLinkId,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) stable_key: String,
    pub(crate) kind: TsPrototypeLinkKind,
    pub(crate) object_stable_key: String,
    pub(crate) prototype_stable_key: String,
    pub(crate) property_key: Option<TsPropertyKey>,
    pub(crate) status: TsObjectModelStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_kind_labels_are_stable() {
        let variants = [
            (TsObjectAllocationKind::ObjectLiteral, "object_literal"),
            (TsObjectAllocationKind::ArrayLiteral, "array_literal"),
            (TsObjectAllocationKind::FunctionObject, "function_object"),
            (TsObjectAllocationKind::ClassObject, "class_object"),
            (TsObjectAllocationKind::ClassInstance, "class_instance"),
            (TsObjectAllocationKind::PrototypeObject, "prototype_object"),
            (TsObjectAllocationKind::ModuleObject, "module_object"),
            (
                TsObjectAllocationKind::NativeFunctionObject,
                "native_function_object",
            ),
        ];

        assert_eq!(
            variants
                .iter()
                .map(|(kind, _)| kind.as_str())
                .collect::<Vec<_>>(),
            variants.iter().map(|(_, label)| *label).collect::<Vec<_>>()
        );
    }

    #[test]
    fn property_key_kind_labels_are_distinct_and_stable() {
        let labels = [
            TsPropertyKeyKind::Static.as_str(),
            TsPropertyKeyKind::Private.as_str(),
            TsPropertyKeyKind::StringLiteral.as_str(),
            TsPropertyKeyKind::NumericLiteral.as_str(),
            TsPropertyKeyKind::ComputedBucket.as_str(),
            TsPropertyKeyKind::UnknownBucket.as_str(),
        ];

        assert_eq!(labels[0], "static");
        assert_eq!(labels[4], "computed_bucket");
        assert_eq!(labels[5], "unknown_bucket");
        assert_eq!(
            labels
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            6
        );
    }

    #[test]
    fn property_key_stable_label_includes_kind_and_optional_value() {
        let exact = TsPropertyKey {
            kind: TsPropertyKeyKind::StringLiteral,
            value: Some("target".to_string()),
        };
        let computed = TsPropertyKey {
            kind: TsPropertyKeyKind::ComputedBucket,
            value: None,
        };

        assert_eq!(exact.stable_label(), "string_literal:target");
        assert_eq!(computed.stable_label(), "computed_bucket");
    }

    #[test]
    fn status_carries_explicit_unknown_and_unsupported_reasons() {
        assert_eq!(TsObjectModelStatus::resolved().as_str(), "resolved");
        assert_eq!(
            TsObjectModelStatus::unknown("computed property").reason(),
            Some("computed property")
        );
        assert_eq!(
            TsObjectModelStatus::unsupported("reflective prototype mutation").reason(),
            Some("reflective prototype mutation")
        );
        assert_eq!(
            TsObjectModelStatus::budget_exceeded("property bucket cap").as_str(),
            "budget_exceeded"
        );
    }
}
