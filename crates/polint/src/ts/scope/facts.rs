#![allow(dead_code, reason = "kept for private internal consumers")]

use serde::{Deserialize, Serialize};

use crate::analysis::ids::{TsBindingId, TsScopeId};
use crate::core::{FileId, Span, StableKeyId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsScopeKind {
    Module,
    Function,
    Class,
    Block,
    Catch,
    Loop,
    Switch,
    Type,
    Namespace,
    Unknown,
}

impl TsScopeKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::Function => "function",
            Self::Class => "class",
            Self::Block => "block",
            Self::Catch => "catch",
            Self::Loop => "loop",
            Self::Switch => "switch",
            Self::Type => "type",
            Self::Namespace => "namespace",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsDeclarationKind {
    Var,
    Let,
    Const,
    FunctionDeclaration,
    FunctionExpression,
    ClassDeclaration,
    ClassExpression,
    Parameter,
    Destructuring,
    Catch,
    Import,
    ReExport,
    Alias,
    UnsupportedDynamic,
    Unknown,
}

impl TsDeclarationKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Var => "var",
            Self::Let => "let",
            Self::Const => "const",
            Self::FunctionDeclaration => "function_declaration",
            Self::FunctionExpression => "function_expression",
            Self::ClassDeclaration => "class_declaration",
            Self::ClassExpression => "class_expression",
            Self::Parameter => "parameter",
            Self::Destructuring => "destructuring",
            Self::Catch => "catch",
            Self::Import => "import",
            Self::ReExport => "re_export",
            Self::Alias => "alias",
            Self::UnsupportedDynamic => "unsupported_dynamic",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsBindingKind {
    Var,
    Let,
    Const,
    FunctionDeclaration,
    FunctionExpression,
    ClassDeclaration,
    ClassExpression,
    Parameter,
    Destructuring,
    Catch,
    Import,
    ReExport,
    Alias,
    NamespaceImport,
    DefaultImport,
    NamedImport,
    UnsupportedDynamic,
    Unknown,
}

impl TsBindingKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Var => "var",
            Self::Let => "let",
            Self::Const => "const",
            Self::FunctionDeclaration => "function_declaration",
            Self::FunctionExpression => "function_expression",
            Self::ClassDeclaration => "class_declaration",
            Self::ClassExpression => "class_expression",
            Self::Parameter => "parameter",
            Self::Destructuring => "destructuring",
            Self::Catch => "catch",
            Self::Import => "import",
            Self::ReExport => "re_export",
            Self::Alias => "alias",
            Self::NamespaceImport => "namespace_import",
            Self::DefaultImport => "default_import",
            Self::NamedImport => "named_import",
            Self::UnsupportedDynamic => "unsupported_dynamic",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsImportExportKind {
    None,
    ImportDefault,
    ImportNamed,
    ImportNamespace,
    SideEffectImport,
    CommonJsRequire,
    ExportNamed,
    ExportDefault,
    ReExportNamed,
    ReExportAll,
    Alias,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) enum TsBindingStatus {
    Present,
    Unresolved { reason: String },
    UnsupportedDynamic { reason: String },
    External { module: String },
}

impl TsBindingStatus {
    pub(crate) fn present() -> Self {
        Self::Present
    }

    pub(crate) fn unresolved(reason: impl Into<String>) -> Self {
        Self::Unresolved {
            reason: reason.into(),
        }
    }

    pub(crate) fn unsupported_dynamic(reason: impl Into<String>) -> Self {
        Self::UnsupportedDynamic {
            reason: reason.into(),
        }
    }

    pub(crate) fn external(module: impl Into<String>) -> Self {
        Self::External {
            module: module.into(),
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Unresolved { .. } => "unresolved",
            Self::UnsupportedDynamic { .. } => "unsupported_dynamic",
            Self::External { .. } => "external",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TsScopeFact {
    pub(crate) id: TsScopeId,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) stable_key: StableKeyId,
    pub(crate) parent_scope_key: Option<StableKeyId>,
    pub(crate) kind: TsScopeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TsBindingFact {
    pub(crate) id: TsBindingId,
    pub(crate) file: FileId,
    pub(crate) span: Span,
    pub(crate) stable_key: StableKeyId,
    pub(crate) scope_key: StableKeyId,
    pub(crate) parent_scope_key: Option<StableKeyId>,
    pub(crate) name: String,
    pub(crate) declaration_kind: TsDeclarationKind,
    pub(crate) binding_kind: TsBindingKind,
    pub(crate) import_export_kind: TsImportExportKind,
    pub(crate) module_source: Option<String>,
    pub(crate) imported_name: Option<String>,
    pub(crate) exported_name: Option<String>,
    pub(crate) inventory_function_key: Option<StableKeyId>,
    pub(crate) inventory_callsite_key: Option<StableKeyId>,
    pub(crate) status: TsBindingStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{FileId, Span};

    #[test]
    fn binding_status_covers_present_unresolved_unsupported_and_external() {
        let statuses = [
            TsBindingStatus::present(),
            TsBindingStatus::unresolved("missing import"),
            TsBindingStatus::unsupported_dynamic("computed property"),
            TsBindingStatus::external("./dep"),
        ];

        assert_eq!(
            statuses
                .iter()
                .map(TsBindingStatus::as_str)
                .collect::<Vec<_>>(),
            vec!["present", "unresolved", "unsupported_dynamic", "external"]
        );
    }

    #[test]
    fn binding_kind_model_represents_required_js_ts_declarations() {
        let kinds = [
            TsBindingKind::Var,
            TsBindingKind::Let,
            TsBindingKind::Const,
            TsBindingKind::FunctionDeclaration,
            TsBindingKind::FunctionExpression,
            TsBindingKind::ClassDeclaration,
            TsBindingKind::ClassExpression,
            TsBindingKind::Parameter,
            TsBindingKind::Destructuring,
            TsBindingKind::Catch,
            TsBindingKind::Import,
            TsBindingKind::ReExport,
            TsBindingKind::Alias,
            TsBindingKind::NamespaceImport,
            TsBindingKind::DefaultImport,
            TsBindingKind::NamedImport,
        ];

        assert_eq!(kinds.len(), 16);
        assert!(kinds.iter().all(|kind| !kind.as_str().is_empty()));
    }

    #[test]
    fn scope_and_binding_rows_keep_dense_ids_separate_from_stable_keys() {
        let interner = crate::core::StableKeyInterner::default();
        let span = Span::point(FileId(1), 1, 1);
        let scope = TsScopeFact {
            id: TsScopeId(99),
            file: FileId(1),
            span: span.clone(),
            stable_key: interner.intern("scope:module"),
            parent_scope_key: None,
            kind: TsScopeKind::Module,
        };
        let binding = TsBindingFact {
            id: TsBindingId(7),
            file: FileId(1),
            span,
            stable_key: interner.intern("binding:value"),
            scope_key: scope.stable_key,
            parent_scope_key: scope.parent_scope_key,
            name: "value".to_string(),
            declaration_kind: TsDeclarationKind::Const,
            binding_kind: TsBindingKind::Const,
            import_export_kind: TsImportExportKind::None,
            module_source: None,
            imported_name: None,
            exported_name: None,
            inventory_function_key: None,
            inventory_callsite_key: None,
            status: TsBindingStatus::present(),
        };

        assert_eq!(scope.id, TsScopeId(99));
        assert_eq!(binding.id, TsBindingId(7));
        assert_eq!(interner.resolve(binding.scope_key).as_ref(), "scope:module");
    }
}
