//! Layer/query key vocabulary shared by demand analysis and cache layers.

use serde::{Deserialize, Serialize};

use crate::digest::Digest;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerKind {
    SourceFiles,
    GoSyntax,
    TsSyntax,
    ModuleGraph,
    SymbolGraph,
    ModuleTopology,
    SemanticMir,
    Cfg,
    Calls,
    AbstractDomains,
    DirectSummaries,
    TypeValueAlias,
    DemandQuery,
    Metrics,
    Extension,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrecisionTier {
    Syntax,
    SetupAware,
    Exact,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct QueryKey {
    pub query_kind: String,
    pub query_version: String,
    pub parameter_digest: Digest,
    pub layer_digests: Vec<Digest>,
    pub budget_digest: Digest,
    pub precision_tier: PrecisionTier,
}

impl QueryKey {
    pub fn new(
        query_kind: impl Into<String>,
        query_version: impl Into<String>,
        parameter_digest: Digest,
        mut layer_digests: Vec<Digest>,
        budget_digest: Digest,
        precision_tier: PrecisionTier,
    ) -> Self {
        layer_digests.sort();
        Self {
            query_kind: query_kind.into(),
            query_version: query_version.into(),
            parameter_digest,
            layer_digests,
            budget_digest,
            precision_tier,
        }
    }
}
