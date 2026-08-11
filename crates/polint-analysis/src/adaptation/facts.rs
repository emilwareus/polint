use serde::{Deserialize, Serialize};

use polint_core::{StableKeyId, StableKeyInterner};

/// Source language for a repo-local adaptation model fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelLanguage {
    Go,
    #[serde(rename = "javascript")]
    JavaScript,
    #[serde(rename = "typescript")]
    TypeScript,
    Jsx,
    Tsx,
}

impl ModelLanguage {
    pub fn as_str(self) -> &'static str {
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
pub enum ModelConfidence {
    SetupAware,
    Heuristic,
    Conservative,
}

impl ModelConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SetupAware => "setup_aware",
            Self::Heuristic => "heuristic",
            Self::Conservative => "conservative",
        }
    }
}

/// Loaded TOML model fact after schema parsing and path normalization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LoadedModelFact {
    pub model_path: String,
    pub source_pattern: String,
    pub target_pattern: String,
    pub confidence: ModelConfidence,
    pub language: ModelLanguage,
    pub scope: String,
    pub evidence: Vec<String>,
    pub stable_key: StableKeyId,
}

impl LoadedModelFact {
    pub fn digest_parts(&self, interner: &StableKeyInterner) -> Vec<String> {
        let mut parts = vec![
            format!("model_path={}", self.model_path),
            format!("source_pattern={}", self.source_pattern),
            format!("target_pattern={}", self.target_pattern),
            format!("confidence={}", self.confidence.as_str()),
            format!("language={}", self.language.as_str()),
            format!("scope={}", self.scope),
            format!("stable_key={}", interner.resolve(self.stable_key)),
        ];
        parts.extend(self.evidence.iter().map(|item| format!("evidence={item}")));
        parts
    }

    pub fn sort_key<'a>(&'a self, interner: &'a StableKeyInterner) -> impl Ord + 'a {
        (
            self.model_path.as_str(),
            self.source_pattern.as_str(),
            self.target_pattern.as_str(),
            self.confidence,
            self.language,
            self.scope.as_str(),
            interner.resolve(self.stable_key),
            self.evidence.as_slice(),
        )
    }
}

/// Accepted model fact. Later plans lower this into `ConstraintKind::ModelEdge`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AcceptedModelFact {
    pub fact: LoadedModelFact,
}

/// Rejected model fact with a deterministic reason.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RejectedModelFact {
    pub fact: LoadedModelFact,
    pub reason: RejectionReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
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
    pub fn as_str(self) -> &'static str {
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
