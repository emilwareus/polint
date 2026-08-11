//! Map frontend analysis errors into the shared analysis error shape at the
//! composition boundary (orphan rules forbid `From` across foreign types).

use polint_analysis::error::AnalysisError;

#[cfg(test)]
pub(crate) fn from_go(error: polint_go::error::AnalysisError) -> AnalysisError {
    match error {
        polint_go::error::AnalysisError::MissingFactFamily { family } => {
            AnalysisError::MissingFactFamily { family }
        }
        polint_go::error::AnalysisError::InvalidFact { provider, reason } => {
            AnalysisError::InvalidFact { provider, reason }
        }
        polint_go::error::AnalysisError::CacheSchemaMismatch { schema } => {
            AnalysisError::CacheSchemaMismatch { schema }
        }
    }
}

pub(crate) fn from_ts(error: polint_ts::error::AnalysisError) -> AnalysisError {
    match error {
        polint_ts::error::AnalysisError::MissingFactFamily { family } => {
            AnalysisError::MissingFactFamily { family }
        }
        polint_ts::error::AnalysisError::InvalidFact { provider, reason } => {
            AnalysisError::InvalidFact { provider, reason }
        }
        polint_ts::error::AnalysisError::CacheSchemaMismatch { schema } => {
            AnalysisError::CacheSchemaMismatch { schema }
        }
    }
}
