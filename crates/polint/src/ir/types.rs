use serde::{Deserialize, Serialize};

use crate::ir::ids::TypeSetId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TypeShape {
    Primitive(String),
    Literal(String),
    Nullish(String),
    Callable { signature: String },
    Class { name: Option<String> },
    Object { shape_id: Option<String> },
    Module { module_key: String },
    Nominal { type_id: String },
    Structural { shape_id: String },
    Union(Vec<TypeSetId>),
    Intersection(Vec<TypeSetId>),
    GenericPlaceholder(String),
    Any,
    Unknown { reason: String },
    Unsupported { reason: String },
}
