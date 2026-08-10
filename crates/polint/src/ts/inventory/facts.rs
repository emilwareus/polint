use serde::{Deserialize, Serialize};

use crate::analysis::ids::{TsInventoryCallsiteId, TsInventoryFunctionId};
use crate::core::{FileId, Span, StableKeyId};

#[allow(
    dead_code,
    reason = " defines private inventory rows before extraction/store consumers land"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsFunctionInventoryKind {
    Declaration,
    FunctionExpression,
    Arrow,
    Method,
    Constructor,
    Accessor,
    ClassStaticBlock,
}

#[allow(
    dead_code,
    reason = " defines private inventory rows before extraction/store consumers land"
)]
impl TsFunctionInventoryKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Declaration => "declaration",
            Self::FunctionExpression => "function_expression",
            Self::Arrow => "arrow",
            Self::Method => "method",
            Self::Constructor => "constructor",
            Self::Accessor => "accessor",
            Self::ClassStaticBlock => "class_static_block",
        }
    }
}

#[allow(
    dead_code,
    reason = " defines private inventory rows before extraction/store consumers land"
)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsCallsiteInventoryKind {
    Call,
    New,
    TaggedTemplate,
    OptionalCall,
    DynamicImport,
    Require,
}

#[allow(
    dead_code,
    reason = " defines private inventory rows before extraction/store consumers land"
)]
impl TsCallsiteInventoryKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Call => "call",
            Self::New => "new",
            Self::TaggedTemplate => "tagged_template",
            Self::OptionalCall => "optional_call",
            Self::DynamicImport => "dynamic_import",
            Self::Require => "require",
        }
    }
}

#[allow(
    dead_code,
    reason = " defines private inventory rows before extraction/store consumers land"
)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsInventoryStatus {
    Resolved,
    Unresolved { reason: String },
    Unsupported { reason: String },
}

#[allow(
    dead_code,
    reason = " defines private inventory rows before extraction/store consumers land"
)]
impl TsInventoryStatus {
    pub(crate) fn resolved() -> Self {
        Self::Resolved
    }

    pub(crate) fn unresolved(reason: impl Into<String>) -> Self {
        Self::Unresolved {
            reason: reason.into(),
        }
    }

    pub(crate) fn unsupported(reason: impl Into<String>) -> Self {
        Self::Unsupported {
            reason: reason.into(),
        }
    }
}

#[allow(
    dead_code,
    reason = " defines private inventory rows before extraction/store consumers land"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TsInventoryFunctionFact {
    pub(crate) id: TsInventoryFunctionId,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) stable_key: StableKeyId,
    pub(crate) lexical_parent_key: Option<StableKeyId>,
    pub(crate) display_name: Option<String>,
    pub(crate) kind: TsFunctionInventoryKind,
    pub(crate) status: TsInventoryStatus,
}

#[allow(
    dead_code,
    reason = " defines private inventory rows before extraction/store consumers land"
)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TsInventoryCallsiteFact {
    pub(crate) id: TsInventoryCallsiteId,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) stable_key: StableKeyId,
    pub(crate) lexical_parent_key: Option<StableKeyId>,
    pub(crate) display_name: Option<String>,
    pub(crate) kind: TsCallsiteInventoryKind,
    pub(crate) status: TsInventoryStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{FileId, Span};

    #[test]
    fn function_inventory_kind_labels_are_stable() {
        let variants = [
            (TsFunctionInventoryKind::Declaration, "declaration"),
            (
                TsFunctionInventoryKind::FunctionExpression,
                "function_expression",
            ),
            (TsFunctionInventoryKind::Arrow, "arrow"),
            (TsFunctionInventoryKind::Method, "method"),
            (TsFunctionInventoryKind::Constructor, "constructor"),
            (TsFunctionInventoryKind::Accessor, "accessor"),
            (
                TsFunctionInventoryKind::ClassStaticBlock,
                "class_static_block",
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
    fn callsite_inventory_kind_labels_are_stable() {
        let variants = [
            (TsCallsiteInventoryKind::Call, "call"),
            (TsCallsiteInventoryKind::New, "new"),
            (TsCallsiteInventoryKind::TaggedTemplate, "tagged_template"),
            (TsCallsiteInventoryKind::OptionalCall, "optional_call"),
            (TsCallsiteInventoryKind::DynamicImport, "dynamic_import"),
            (TsCallsiteInventoryKind::Require, "require"),
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
    fn fact_rows_keep_dense_ids_separate_from_stable_keys() {
        let interner = crate::core::StableKeyInterner::default();
        let span = Span::point(FileId(3), 10, 4);
        let function = TsInventoryFunctionFact {
            id: TsInventoryFunctionId(99),
            file: FileId(3),
            span: span.clone(),
            stable_key: interner.intern("file=src/a.ts|span=10:4|kind=arrow"),
            lexical_parent_key: Some(interner.intern("file=src/a.ts|scope=module")),
            display_name: Some("handler".to_string()),
            kind: TsFunctionInventoryKind::Arrow,
            status: TsInventoryStatus::resolved(),
        };
        let callsite = TsInventoryCallsiteFact {
            id: TsInventoryCallsiteId(7),
            file: FileId(3),
            span,
            stable_key: interner.intern("file=src/a.ts|span=11:2|kind=call"),
            lexical_parent_key: Some(function.stable_key),
            display_name: Some("handler".to_string()),
            kind: TsCallsiteInventoryKind::Call,
            status: TsInventoryStatus::unresolved("dynamic callee"),
        };

        assert_eq!(function.id, TsInventoryFunctionId(99));
        assert_eq!(callsite.id, TsInventoryCallsiteId(7));
        assert!(interner.resolve(function.stable_key).contains("kind=arrow"));
        assert!(matches!(
            callsite.status,
            TsInventoryStatus::Unresolved { .. }
        ));
    }

    #[test]
    fn unsupported_status_carries_a_reason() {
        assert_eq!(
            TsInventoryStatus::unsupported("private field call"),
            TsInventoryStatus::Unsupported {
                reason: "private field call".to_string()
            }
        );
    }
}
