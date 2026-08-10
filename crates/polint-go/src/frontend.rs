//! Go [`LanguageFrontend`] registered by the polint composition root.

use std::path::Path;

use polint_analysis_api::{
    DisabledAnalysisCache, PrecisionCeiling, ProviderCtx, ProviderRunResult,
};
use polint_core::{Language, LanguageId};
use polint_frontend_api::{AnalysisUnit, FrontendProfile, LanguageFrontend};

use crate::adapter::analyze_files_with_plan_options_and_cache_stats;

pub const FAMILY_GO: &str = "go";

const GO_PRODUCES: &[&str] = &[
    "packages",
    "functions",
    "imports",
    "go_tests",
    "branch_obligations",
];

pub const GO_FRONTEND_PROFILE: FrontendProfile = FrontendProfile {
    name: "go",
    family: FAMILY_GO,
    produces: GO_PRODUCES,
    precision_ceiling: PrecisionCeiling::Syntax,
};

pub struct GoFrontend {
    id: LanguageId,
}

impl GoFrontend {
    pub fn new(id: LanguageId) -> Self {
        Self { id }
    }
}

impl LanguageFrontend for GoFrontend {
    fn id(&self) -> LanguageId {
        self.id
    }

    fn handles(&self, path: &Path) -> bool {
        Language::from_path(path) == Language::Go
    }

    fn profile(&self) -> &'static FrontendProfile {
        &GO_FRONTEND_PROFILE
    }

    fn analyze(&self, ctx: &mut ProviderCtx<'_>, unit: &AnalysisUnit<'_>) -> ProviderRunResult {
        let _ = unit.root;
        let cache = ctx.host.analysis_cache().unwrap_or(&DisabledAnalysisCache);
        analyze_files_with_plan_options_and_cache_stats(
            ctx.facts,
            unit.files,
            cache,
            ctx.config_digest,
            ctx.rule_digest,
            ctx.host.plan_digest(),
            ctx.parallel,
        )
    }
}
