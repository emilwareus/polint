//! Erased provider-owned fact container contracts.

use std::any::Any;
use std::fmt;

use crate::metadata::FactFamily;

/// Erased provider-owned fact container.
pub trait FactStore: Any + Send + Sync {
    fn family(&self) -> FactFamily;
    fn clear(&mut self);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn clone_box(&self) -> Box<dyn FactStore>;
}

/// Cloneable / debug wrapper for host registries that derive `Clone` / `Debug`.
pub struct FactStoreEntry(Box<dyn FactStore>);

impl FactStoreEntry {
    pub fn new(store: impl FactStore + 'static) -> Self {
        Self(Box::new(store))
    }

    pub fn as_store(&self) -> &dyn FactStore {
        self.0.as_ref()
    }

    pub fn as_store_mut(&mut self) -> &mut dyn FactStore {
        self.0.as_mut()
    }
}

impl Clone for FactStoreEntry {
    fn clone(&self) -> Self {
        Self(self.0.clone_box())
    }
}

impl fmt::Debug for FactStoreEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FactStoreEntry")
            .field("family", &self.0.family())
            .finish_non_exhaustive()
    }
}
