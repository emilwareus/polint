use super::store::RefinedCallOutput;
use crate::core::AnalysisDb;

pub(crate) fn derive_summary_assisted_refinements(_db: &AnalysisDb) -> RefinedCallOutput {
    RefinedCallOutput::empty()
}
