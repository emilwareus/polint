//! Local analysis errors for Go semantic store validation.

#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("missing semantic fact family `{family}`")]
    MissingFactFamily { family: &'static str },
    #[error("invalid semantic fact from `{provider}`: {reason}")]
    InvalidFact {
        provider: &'static str,
        reason: String,
    },
    #[error("semantic cache schema mismatch: expected `{schema}`")]
    CacheSchemaMismatch { schema: String },
}
