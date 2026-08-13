//! Language-frontend contracts for polint.
//!
//! Depends on `polint-core` and `polint-analysis-api`. Must not import concrete frontends.

use std::path::Path;

use crate::analysis_api::{PrecisionCeiling, ProviderCtx, ProviderRunResult, SourceFile};
use crate::internal_core::LanguageId;

/// Declared fact families and precision for a language frontend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrontendProfile {
    /// Stable name used for sorting (`go`, `ts`). Never sort by [`LanguageId`].
    pub name: &'static str,
    /// Language-family tag for comparisons that used to call `Language::is_ts_family`.
    pub family: &'static str,
    pub produces: &'static [&'static str],
    pub precision_ceiling: PrecisionCeiling,
}

/// Analysis input for a frontend.
pub struct AnalysisUnit<'a> {
    pub files: &'a [&'a SourceFile],
    pub root: &'a Path,
}

/// Pluggable language (or index) frontend.
pub trait LanguageFrontend: Send + Sync {
    fn id(&self) -> LanguageId;
    fn handles(&self, path: &Path) -> bool;
    fn profile(&self) -> &'static FrontendProfile;
    fn analyze(&self, ctx: &mut ProviderCtx<'_>, unit: &AnalysisUnit<'_>) -> ProviderRunResult;
}

/// Composition container for language frontends (no default builder).
pub struct FrontendRegistry {
    frontends: Vec<Box<dyn LanguageFrontend>>,
}

impl FrontendRegistry {
    pub fn new() -> Self {
        Self {
            frontends: Vec::new(),
        }
    }

    /// Register a frontend; id is assigned in registration order.
    pub fn register<F>(&mut self, make: F) -> LanguageId
    where
        F: FnOnce(LanguageId) -> Box<dyn LanguageFrontend>,
    {
        let id = LanguageId::from_raw(
            u16::try_from(self.frontends.len()).expect("language frontend registry overflow"),
        );
        self.frontends.push(make(id));
        id
    }

    pub fn get(&self, id: LanguageId) -> Option<&dyn LanguageFrontend> {
        self.frontends
            .get(usize::from(id.raw()))
            .map(|frontend| frontend.as_ref())
    }

    pub fn by_name(&self, name: &str) -> Option<&dyn LanguageFrontend> {
        self.frontends
            .iter()
            .find(|frontend| frontend.profile().name == name)
            .map(|frontend| frontend.as_ref())
    }

    /// Frontends that claim `path`, ordered by stable profile name (not LanguageId).
    pub fn scheduled_for(&self, path: &Path) -> Vec<&dyn LanguageFrontend> {
        let mut matched: Vec<&dyn LanguageFrontend> = self
            .frontends
            .iter()
            .map(|frontend| frontend.as_ref())
            .filter(|frontend| frontend.handles(path))
            .collect();
        matched.sort_by_key(|frontend| frontend.profile().name);
        matched
    }
}

impl Default for FrontendRegistry {
    fn default() -> Self {
        Self::new()
    }
}
