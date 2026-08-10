//! Facade-owned host session for [`polint_analysis_api::ProviderCtx`].
//!
//! `ProviderCtx` only carries fact-db / digest contracts. Facade providers reach
//! cache/config/plan state through a thread-local owned session installed by the
//! composition root for the duration of scheduled provider execution.

use std::any::Any;
use std::cell::RefCell;

use crate::analysis::summaries::provider::SccClosureProviderOutput;
use crate::analysis_kernel::incremental::InputSnapshot;
use crate::analysis_plan::AnalysisPlan;
use crate::cache::Cache;
use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, CapabilitySupportView, StableKeyInterner};
use polint_analysis_api::{
    FactDatabase, FactFamily, FactMetaStore, FactStore, NullHostAttachment, ProviderHostServices,
};

thread_local! {
    static PROVIDER_HOST_SESSION: RefCell<Option<ProviderHostSession>> = const { RefCell::new(None) };
}

/// Owned host state cloned into each provider handle (cache is path metadata only).
pub(crate) struct ProviderHostSession {
    pub(crate) cache: Cache,
    pub(crate) loaded: LoadedConfig,
    pub(crate) input_snapshot: InputSnapshot,
    pub(crate) plan: AnalysisPlan,
    pub(crate) capability_support: CapabilitySupportView,
    pub(crate) scc_closure: Option<SccClosureProviderOutput>,
}

pub(crate) fn install_provider_host_session(session: ProviderHostSession) {
    PROVIDER_HOST_SESSION.with(|slot| {
        *slot.borrow_mut() = Some(session);
    });
}

pub(crate) fn take_provider_host_session() -> Option<ProviderHostSession> {
    PROVIDER_HOST_SESSION.with(|slot| slot.borrow_mut().take())
}

pub(crate) fn with_provider_host_session_mut<R>(
    f: impl FnOnce(&mut ProviderHostSession) -> R,
) -> R {
    PROVIDER_HOST_SESSION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let session = slot
            .as_mut()
            .expect("provider host session installed by analysis kernel");
        f(session)
    })
}

/// Placeholder host services object for `ProviderCtx::host`.
#[derive(Debug, Default)]
pub(crate) struct FacadeHostServices;

impl ProviderHostServices for FacadeHostServices {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

pub(crate) type FacadeHostAttachment = NullHostAttachment;

impl FactDatabase for AnalysisDb {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn store(&self, family: FactFamily) -> Option<&dyn FactStore> {
        self.fact_stores.get(&family).map(|entry| entry.as_store())
    }

    fn store_mut(&mut self, family: FactFamily) -> Option<&mut dyn FactStore> {
        self.fact_stores
            .get_mut(&family)
            .map(|entry| entry.as_store_mut())
    }

    fn fact_meta(&self) -> &FactMetaStore {
        &self.fact_meta
    }

    fn fact_meta_mut(&mut self) -> &mut FactMetaStore {
        &mut self.fact_meta
    }

    fn stable_key_interner(&self) -> StableKeyInterner {
        self.stable_keys.clone()
    }
}
