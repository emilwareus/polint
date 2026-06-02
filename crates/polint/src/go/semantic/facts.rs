use crate::core::{FileId, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticPackageId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticFunctionId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticCallsiteId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticMethodSetId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct GoSemanticPackageErrorId(pub(crate) u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum GoSemanticFunctionKind {
    Function,
    Method,
    Init,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum GoSemanticCallStatus {
    ResolvedStatic,
    UnresolvedDynamic,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticPackageFact {
    pub(crate) id: GoSemanticPackageId,
    pub(crate) stable_key: String,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) package_name: String,
    pub(crate) module_path: String,
    pub(crate) files: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticFunctionFact {
    pub(crate) id: GoSemanticFunctionId,
    pub(crate) stable_key: String,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) name: String,
    pub(crate) qualified: String,
    pub(crate) signature: String,
    pub(crate) kind: GoSemanticFunctionKind,
    pub(crate) receiver: Option<String>,
    pub(crate) relative_file: Option<String>,
    pub(crate) file: Option<FileId>,
    pub(crate) span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticCallsiteFact {
    pub(crate) id: GoSemanticCallsiteId,
    pub(crate) stable_key: String,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) caller: String,
    pub(crate) static_callee: Option<String>,
    pub(crate) status: GoSemanticCallStatus,
    pub(crate) reason: Option<String>,
    pub(crate) relative_file: Option<String>,
    pub(crate) file: Option<FileId>,
    pub(crate) span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticMethodSetFact {
    pub(crate) id: GoSemanticMethodSetId,
    pub(crate) stable_key: String,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) type_name: String,
    pub(crate) methods: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoSemanticPackageErrorFact {
    pub(crate) id: GoSemanticPackageErrorId,
    pub(crate) stable_key: String,
    pub(crate) package_id: String,
    pub(crate) package_path: String,
    pub(crate) message: String,
}
