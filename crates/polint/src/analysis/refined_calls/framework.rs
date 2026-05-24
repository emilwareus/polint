use super::store::RefinedCallOutput;
use crate::core::AnalysisDb;

pub(crate) fn derive_framework_refinements(_db: &AnalysisDb) -> RefinedCallOutput {
    RefinedCallOutput::empty()
}
