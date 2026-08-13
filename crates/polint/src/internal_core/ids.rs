use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($name:ident, $ty:ty) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[non_exhaustive]
        pub struct $name(pub $ty);

        impl $name {
            /// Construct from the raw integer identity.
            pub const fn from_raw(raw: $ty) -> Self {
                Self(raw)
            }

            /// Borrow the raw integer identity.
            pub const fn raw(self) -> $ty {
                self.0
            }
        }
    };
}

id_newtype!(FileId, u32);
id_newtype!(NodeId, u64);
id_newtype!(FunctionId, u64);
id_newtype!(PackageId, u64);
id_newtype!(BranchId, u64);
id_newtype!(ImportId, u64);
id_newtype!(ResolvedImportId, u64);
id_newtype!(ModuleNodeId, u64);
id_newtype!(ModuleEdgeId, u64);
id_newtype!(SymbolId, u64);
id_newtype!(DefinitionId, u64);
id_newtype!(ReferenceId, u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub struct RuleId(pub String);

impl RuleId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
