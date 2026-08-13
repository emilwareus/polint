#![allow(dead_code, reason = "kept for private internal consumers")]

use serde::{Deserialize, Serialize};

use crate::internal_core::{FileId, Span, StableKeyId};
use crate::ts::ids::{TsInventoryCallsiteId, TsInventoryFunctionId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TsObjectAllocationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TsPropertyWriteId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TsPropertyReadId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TsReceiverBindingId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TsPrototypeLinkId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TsObjectAllocationKind {
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
    pub fn as_str(self) -> &'static str {
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
pub enum TsPropertyKeyKind {
    Static,
    Private,
    StringLiteral,
    NumericLiteral,
    ComputedBucket,
    UnknownBucket,
}

impl TsPropertyKeyKind {
    pub fn as_str(self) -> &'static str {
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
pub struct TsPropertyKey {
    pub kind: TsPropertyKeyKind,
    pub value: Option<String>,
}

impl TsPropertyKey {
    pub fn stable_label(&self) -> String {
        match self.value.as_deref() {
            Some(value) => format!("{}:{value}", self.kind.as_str()),
            None => self.kind.as_str().to_string(),
        }
    }

    pub fn field_label(&self) -> String {
        match (self.kind, self.value.as_deref()) {
            (
                TsPropertyKeyKind::Static
                | TsPropertyKeyKind::StringLiteral
                | TsPropertyKeyKind::NumericLiteral,
                Some(value),
            ) => format!("exact:{value}"),
            _ => self.stable_label(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TsObjectModelStatus {
    Resolved,
    Unknown { reason: String },
    Unsupported { reason: String },
    BudgetExceeded { reason: String },
}

impl TsObjectModelStatus {
    pub fn resolved() -> Self {
        Self::Resolved
    }

    pub fn unknown(reason: impl Into<String>) -> Self {
        Self::Unknown {
            reason: reason.into(),
        }
    }

    pub fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported {
            reason: reason.into(),
        }
    }

    pub fn budget_exceeded(reason: impl Into<String>) -> Self {
        Self::BudgetExceeded {
            reason: reason.into(),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Unknown { .. } => "unknown",
            Self::Unsupported { .. } => "unsupported",
            Self::BudgetExceeded { .. } => "budget_exceeded",
        }
    }

    pub fn reason(&self) -> Option<&str> {
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
pub enum TsReceiverBindingKind {
    MethodCall,
    LexicalThis,
    ConstructorInstance,
    BoundFunction,
    CallReceiver,
    ApplyReceiver,
}

impl TsReceiverBindingKind {
    pub fn as_str(self) -> &'static str {
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
pub enum TsPrototypeLinkKind {
    ClassPrototype,
    ClassExtends,
    ConstructorPrototypeProperty,
    SetPrototypeOf,
    ProtoProperty,
}

impl TsPrototypeLinkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClassPrototype => "class_prototype",
            Self::ClassExtends => "class_extends",
            Self::ConstructorPrototypeProperty => "constructor_prototype_property",
            Self::SetPrototypeOf => "set_prototype_of",
            Self::ProtoProperty => "proto_property",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsObjectAllocationFact {
    pub id: TsObjectAllocationId,
    pub file: FileId,
    pub span: Span,
    pub stable_key: StableKeyId,
    pub lexical_parent_key: Option<StableKeyId>,
    pub inventory_function: Option<TsInventoryFunctionId>,
    pub inventory_function_stable_key: Option<StableKeyId>,
    pub inventory_callsite: Option<TsInventoryCallsiteId>,
    pub inventory_callsite_stable_key: Option<StableKeyId>,
    pub kind: TsObjectAllocationKind,
    pub status: TsObjectModelStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsPropertyWriteFact {
    pub id: TsPropertyWriteId,
    pub file: FileId,
    pub span: Span,
    pub stable_key: StableKeyId,
    pub base_object_stable_key: StableKeyId,
    pub property_key: TsPropertyKey,
    pub value_function: Option<TsInventoryFunctionId>,
    pub value_function_stable_key: Option<StableKeyId>,
    pub value_object_stable_key: Option<StableKeyId>,
    pub status: TsObjectModelStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsPropertyReadFact {
    pub id: TsPropertyReadId,
    pub file: FileId,
    pub span: Span,
    pub stable_key: StableKeyId,
    pub base_object_stable_key: StableKeyId,
    pub property_key: TsPropertyKey,
    pub destination_stable_key: Option<StableKeyId>,
    pub callsite: Option<TsInventoryCallsiteId>,
    pub callsite_stable_key: Option<StableKeyId>,
    pub status: TsObjectModelStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsReceiverBindingFact {
    pub id: TsReceiverBindingId,
    pub file: FileId,
    pub span: Span,
    pub stable_key: StableKeyId,
    pub kind: TsReceiverBindingKind,
    pub callsite: Option<TsInventoryCallsiteId>,
    pub callsite_stable_key: Option<StableKeyId>,
    pub callee_function: Option<TsInventoryFunctionId>,
    pub callee_function_stable_key: Option<StableKeyId>,
    pub receiver_object_stable_key: Option<StableKeyId>,
    pub receiver_place_stable_key: Option<StableKeyId>,
    pub lexical_parent_key: Option<StableKeyId>,
    pub status: TsObjectModelStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsPrototypeLinkFact {
    pub id: TsPrototypeLinkId,
    pub file: FileId,
    pub span: Span,
    pub stable_key: StableKeyId,
    pub kind: TsPrototypeLinkKind,
    pub object_stable_key: StableKeyId,
    pub prototype_stable_key: StableKeyId,
    pub property_key: Option<TsPropertyKey>,
    pub status: TsObjectModelStatus,
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
