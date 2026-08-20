#![allow(dead_code, reason = "kept for private internal consumers")]

use serde::{Deserialize, Serialize};

use crate::internal_core::{FileId, Span, StableKeyId};
use crate::ts::ids::{TsBindingId, TsScopeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TsScopeKind {
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
    pub fn as_str(self) -> &'static str {
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
pub enum TsDeclarationKind {
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
    pub fn as_str(self) -> &'static str {
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
pub enum TsBindingKind {
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
    pub fn as_str(self) -> &'static str {
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
pub enum TsImportExportKind {
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
pub enum TsBindingStatus {
    Present,
    Unresolved { reason: String },
    UnsupportedDynamic { reason: String },
    External { module: String },
}

impl TsBindingStatus {
    pub fn present() -> Self {
        Self::Present
    }

    pub fn unresolved(reason: impl Into<String>) -> Self {
        Self::Unresolved {
            reason: reason.into(),
        }
    }

    pub fn unsupported_dynamic(reason: impl Into<String>) -> Self {
        Self::UnsupportedDynamic {
            reason: reason.into(),
        }
    }

    pub fn external(module: impl Into<String>) -> Self {
        Self::External {
            module: module.into(),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Unresolved { .. } => "unresolved",
            Self::UnsupportedDynamic { .. } => "unsupported_dynamic",
            Self::External { .. } => "external",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsScopeFact {
    pub id: TsScopeId,
    pub file: FileId,
    pub span: Span,
    pub stable_key: StableKeyId,
    pub parent_scope_key: Option<StableKeyId>,
    pub kind: TsScopeKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsBindingFact {
    pub id: TsBindingId,
    pub file: FileId,
    pub span: Span,
    pub stable_key: StableKeyId,
    pub scope_key: StableKeyId,
    pub parent_scope_key: Option<StableKeyId>,
    pub name: String,
    pub declaration_kind: TsDeclarationKind,
    pub binding_kind: TsBindingKind,
    pub import_export_kind: TsImportExportKind,
    pub module_source: Option<String>,
    pub imported_name: Option<String>,
    pub exported_name: Option<String>,
    pub inventory_function_key: Option<StableKeyId>,
    pub inventory_callsite_key: Option<StableKeyId>,
    pub status: TsBindingStatus,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::internal_core::{FileId, Span};

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
        let interner = crate::internal_core::StableKeyInterner::default();
        let span = Span::point(FileId::from_raw(1), 1, 1);
        let scope = TsScopeFact {
            id: TsScopeId(99),
            file: FileId::from_raw(1),
            span: span.clone(),
            stable_key: interner.intern("scope:module"),
            parent_scope_key: None,
            kind: TsScopeKind::Module,
        };
        let binding = TsBindingFact {
            id: TsBindingId(7),
            file: FileId::from_raw(1),
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
