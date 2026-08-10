#[derive(Debug, thiserror::Error)]
pub(crate) enum AnalysisError {
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

impl From<polint_go::error::AnalysisError> for AnalysisError {
    fn from(error: polint_go::error::AnalysisError) -> Self {
        match error {
            polint_go::error::AnalysisError::MissingFactFamily { family } => {
                Self::MissingFactFamily { family }
            }
            polint_go::error::AnalysisError::InvalidFact { provider, reason } => {
                Self::InvalidFact { provider, reason }
            }
            polint_go::error::AnalysisError::CacheSchemaMismatch { schema } => {
                Self::CacheSchemaMismatch { schema }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analysis_error_formats_missing_fact_family() {
        let error = AnalysisError::MissingFactFamily { family: "Place" };

        assert_eq!(error.to_string(), "missing semantic fact family `Place`");
    }

    #[test]
    fn analysis_error_formats_invalid_fact() {
        let error = AnalysisError::InvalidFact {
            provider: "polint.semantic_mir",
            reason: "dangling place reference".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "invalid semantic fact from `polint.semantic_mir`: dangling place reference"
        );
    }

    #[test]
    fn analysis_error_formats_cache_schema_mismatch() {
        let error = AnalysisError::CacheSchemaMismatch {
            schema: "semantic-mir-v0".to_string(),
        };

        assert_eq!(
            error.to_string(),
            "semantic cache schema mismatch: expected `semantic-mir-v0`"
        );
    }
}
