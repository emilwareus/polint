//! Map frontend analysis errors into the shared analysis error shape at the
//! composition boundary (orphan rules forbid `From` across foreign types).

use crate::analysis_neutral::error::AnalysisError;

#[cfg(test)]
pub(crate) fn from_go(error: crate::go::error::AnalysisError) -> AnalysisError {
    match error {
        crate::go::error::AnalysisError::MissingFactFamily { family } => {
            AnalysisError::MissingFactFamily { family }
        }
        crate::go::error::AnalysisError::InvalidFact { provider, reason } => {
            AnalysisError::InvalidFact { provider, reason }
        }
        crate::go::error::AnalysisError::CacheSchemaMismatch { schema } => {
            AnalysisError::CacheSchemaMismatch { schema }
        }
    }
}

pub(crate) fn from_ts(error: crate::ts::error::AnalysisError) -> AnalysisError {
    match error {
        crate::ts::error::AnalysisError::MissingFactFamily { family } => {
            AnalysisError::MissingFactFamily { family }
        }
        crate::ts::error::AnalysisError::InvalidFact { provider, reason } => {
            AnalysisError::InvalidFact { provider, reason }
        }
        crate::ts::error::AnalysisError::CacheSchemaMismatch { schema } => {
            AnalysisError::CacheSchemaMismatch { schema }
        }
    }
}
