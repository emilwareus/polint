use super::{FAMILY_GO, FAMILY_TYPESCRIPT_JAVASCRIPT, GoFrontend, TsJsFrontend};
use crate::core::Language;

pub(crate) use polint_core::{
    LANGUAGE_IDS_GO, LANGUAGE_IDS_GO_AND_TS, LANGUAGE_IDS_NONE, LANGUAGE_IDS_TS, LanguageId,
};
pub(crate) use polint_frontend_api::FrontendRegistry;

/// Default host composition: Go then TypeScript/JavaScript.
///
/// Adding a language is one frontend module plus one `register` line here.
pub(crate) fn build_default_registry() -> FrontendRegistry {
    let mut registry = FrontendRegistry::new();
    registry.register(|id| Box::new(GoFrontend::new(id)));
    registry.register(|id| Box::new(TsJsFrontend::new(id)));
    debug_assert_eq!(registry.by_name("go").map(|f| f.id()), Some(LanguageId::GO));
    debug_assert_eq!(registry.by_name("ts").map(|f| f.id()), Some(LanguageId::TS));
    registry
}

pub(crate) trait FrontendRegistryExt {
    fn id_for_public_language(&self, language: Language) -> Option<LanguageId>;
    fn public_language_for(&self, id: LanguageId) -> Option<Language>;
}

impl FrontendRegistryExt for FrontendRegistry {
    fn id_for_public_language(&self, language: Language) -> Option<LanguageId> {
        let path = match language {
            Language::Go => std::path::Path::new("x.go"),
            Language::TypeScript | Language::Tsx | Language::JavaScript | Language::Jsx => {
                std::path::Path::new("x.ts")
            }
            Language::Unknown => return None,
            _ => return None,
        };
        self.scheduled_for(path)
            .first()
            .map(|frontend| frontend.id())
    }

    fn public_language_for(&self, id: LanguageId) -> Option<Language> {
        let frontend = self.get(id)?;
        match frontend.profile().family {
            FAMILY_GO => Some(Language::Go),
            FAMILY_TYPESCRIPT_JAVASCRIPT => Some(Language::TypeScript),
            _ => None,
        }
    }
}
