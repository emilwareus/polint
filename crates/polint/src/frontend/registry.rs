use super::{FAMILY_GO, FAMILY_TYPESCRIPT_JAVASCRIPT, GoFrontend, LanguageFrontend, TsJsFrontend};
use crate::core::Language;

pub(crate) use polint_core::{
    LANGUAGE_IDS_GO, LANGUAGE_IDS_GO_AND_TS, LANGUAGE_IDS_NONE, LANGUAGE_IDS_TS, LanguageId,
};

/// Composition root for language frontends.
pub(crate) struct FrontendRegistry {
    frontends: Vec<Box<dyn LanguageFrontend>>,
}

impl FrontendRegistry {
    pub(crate) fn new() -> Self {
        Self {
            frontends: Vec::new(),
        }
    }

    /// Register a frontend; id is assigned in registration order.
    pub(crate) fn register<F>(&mut self, make: F) -> LanguageId
    where
        F: FnOnce(LanguageId) -> Box<dyn LanguageFrontend>,
    {
        let id = LanguageId::from_raw(
            u16::try_from(self.frontends.len()).expect("language frontend registry overflow"),
        );
        self.frontends.push(make(id));
        id
    }

    pub(crate) fn get(&self, id: LanguageId) -> Option<&dyn LanguageFrontend> {
        self.frontends
            .get(usize::from(id.raw()))
            .map(|frontend| frontend.as_ref())
    }

    pub(crate) fn by_name(&self, name: &str) -> Option<&dyn LanguageFrontend> {
        self.frontends
            .iter()
            .find(|frontend| frontend.profile().name == name)
            .map(|frontend| frontend.as_ref())
    }

    /// Frontends that claim `path`, ordered by stable profile name (not LanguageId).
    pub(crate) fn scheduled_for(&self, path: &std::path::Path) -> Vec<&dyn LanguageFrontend> {
        let mut matched: Vec<&dyn LanguageFrontend> = self
            .frontends
            .iter()
            .map(|frontend| frontend.as_ref())
            .filter(|frontend| frontend.handles(path))
            .collect();
        matched.sort_by_key(|frontend| frontend.profile().name);
        matched
    }

    pub(crate) fn id_for_public_language(&self, language: Language) -> Option<LanguageId> {
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

    pub(crate) fn public_language_for(&self, id: LanguageId) -> Option<Language> {
        let frontend = self.get(id)?;
        match frontend.profile().family {
            FAMILY_GO => Some(Language::Go),
            FAMILY_TYPESCRIPT_JAVASCRIPT => Some(Language::TypeScript),
            _ => None,
        }
    }
}

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
