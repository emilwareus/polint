//! TypeScript/JavaScript [`LanguageFrontend`] registered by the polint composition root.

use std::path::Path;

use polint_analysis_api::{
    DisabledAnalysisCache, PrecisionCeiling, ProviderCtx, ProviderRunResult,
};
use polint_core::{Language, LanguageId};
use polint_frontend_api::{AnalysisUnit, FrontendProfile, LanguageFrontend};

use crate::adapter::analyze_files_with_plan_options_and_cache_stats;

pub const FAMILY_TYPESCRIPT_JAVASCRIPT: &str = "typescript_javascript";

const TS_PRODUCES: &[&str] = &[
    "functions",
    "imports",
    "ts_components",
    "ts_classes",
    "string_literals",
    "jsx_attributes",
];

pub const TS_FRONTEND_PROFILE: FrontendProfile = FrontendProfile {
    name: "ts",
    family: FAMILY_TYPESCRIPT_JAVASCRIPT,
    produces: TS_PRODUCES,
    precision_ceiling: PrecisionCeiling::Syntax,
};

pub struct TsJsFrontend {
    id: LanguageId,
}

impl TsJsFrontend {
    pub fn new(id: LanguageId) -> Self {
        Self { id }
    }
}

impl LanguageFrontend for TsJsFrontend {
    fn id(&self) -> LanguageId {
        self.id
    }

    fn handles(&self, path: &Path) -> bool {
        Language::from_path(path).is_ts_family()
    }

    fn profile(&self) -> &'static FrontendProfile {
        &TS_FRONTEND_PROFILE
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
