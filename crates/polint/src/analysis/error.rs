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
