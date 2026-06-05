use serde::{Deserialize, Serialize};

/// Source language for a repo-local adaptation model fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ModelLanguage {
    Go,
    #[serde(rename = "javascript")]
    JavaScript,
    #[serde(rename = "typescript")]
    TypeScript,
    Jsx,
    Tsx,
}

impl ModelLanguage {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Go => "go",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Jsx => "jsx",
            Self::Tsx => "tsx",
        }
    }
}

/// Honest confidence tier for accepted model facts.
///
/// Deliberately excludes `Exact`: repo-local adaptation models are source-evident
/// heuristics/setup-aware facts unless a later internal evidence contract promotes a
/// stricter ceiling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ModelConfidence {
    SetupAware,
    Heuristic,
    Conservative,
}

impl ModelConfidence {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SetupAware => "setup_aware",
            Self::Heuristic => "heuristic",
            Self::Conservative => "conservative",
        }
    }
}

/// Loaded TOML model fact after schema parsing and path normalization.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct LoadedModelFact {
    pub(crate) model_path: String,
    pub(crate) source_pattern: String,
    pub(crate) target_pattern: String,
    pub(crate) confidence: ModelConfidence,
    pub(crate) language: ModelLanguage,
    pub(crate) scope: String,
    pub(crate) evidence: Vec<String>,
    pub(crate) stable_key: String,
}

impl LoadedModelFact {
    pub(crate) fn digest_parts(&self) -> Vec<String> {
        let mut parts = vec![
            format!("model_path={}", self.model_path),
            format!("source_pattern={}", self.source_pattern),
            format!("target_pattern={}", self.target_pattern),
            format!("confidence={}", self.confidence.as_str()),
            format!("language={}", self.language.as_str()),
            format!("scope={}", self.scope),
            format!("stable_key={}", self.stable_key),
        ];
        parts.extend(self.evidence.iter().map(|item| format!("evidence={item}")));
        parts
    }
}

/// Accepted model fact. Later plans lower this into `ConstraintKind::ModelEdge`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct AcceptedModelFact {
    pub(crate) fact: LoadedModelFact,
}

/// Rejected model fact with a deterministic reason.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct RejectedModelFact {
    pub(crate) fact: LoadedModelFact,
    pub(crate) reason: RejectionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RejectionReason {
    EmptySourcePattern,
    EmptyTargetPattern,
    EmptyScope,
    EmptyEvidence,
    NonResolvingSource,
    NonResolvingTarget,
    BroadSourcePattern,
    BroadTargetPattern,
    OracleShapedTarget,
    BudgetExceeded,
}

impl RejectionReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::EmptySourcePattern => "empty_source_pattern",
            Self::EmptyTargetPattern => "empty_target_pattern",
            Self::EmptyScope => "empty_scope",
            Self::EmptyEvidence => "empty_evidence",
            Self::NonResolvingSource => "non_resolving_source",
            Self::NonResolvingTarget => "non_resolving_target",
            Self::BroadSourcePattern => "broad_source_pattern",
            Self::BroadTargetPattern => "broad_target_pattern",
            Self::OracleShapedTarget => "oracle_shaped_target",
            Self::BudgetExceeded => "budget_exceeded",
        }
    }
}
