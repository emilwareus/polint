//! Local analysis errors for TS object-model store validation.

#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("missing fact family `{family}`")]
    MissingFactFamily { family: &'static str },
    #[error("invalid fact from `{provider}`: {reason}")]
    InvalidFact {
        provider: &'static str,
        reason: String,
    },
    #[error("cache schema mismatch: expected `{schema}`")]
    CacheSchemaMismatch { schema: String },
}
