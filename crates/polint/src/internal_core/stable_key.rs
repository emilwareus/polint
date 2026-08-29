use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(
    Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, serde::Serialize, serde::Deserialize,
)]
pub struct StableKeyId(pub u32);

impl StableKeyId {
    pub const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }

    /// Test-only substring check against the process test interner.
    #[doc(hidden)]
    pub fn contains(self, needle: &str) -> bool {
        test_stable_key_interner().resolve(self).contains(needle)
    }
}

/// Interned key texts and their ids.
///
/// A [`StableKeyId`] is the index of its text in `keys`, so ids must be handed
/// out in insertion order and `keys` must stay densely packed: that invariant is
/// what makes `resolve` a vector index instead of a map lookup. Splitting this
/// state across shards would break it, because each shard would number its own
/// keys from zero.
#[derive(Debug, Default)]
struct StableKeyInternerState {
    keys: Vec<Arc<str>>,
    ids: HashMap<Arc<str>, StableKeyId>,
}

impl StableKeyInternerState {
    /// Appends `key`, returning the id that now indexes it.
    fn push(&mut self, key: Arc<str>) -> StableKeyId {
        let id = StableKeyId(
            u32::try_from(self.keys.len())
                .unwrap_or_else(|_| panic!("stable-key interner exhausted u32 ids")),
        );
        self.keys.push(Arc::clone(&key));
        self.ids.insert(key, id);
        id
    }
}

#[derive(Debug, Clone, Default)]
pub struct StableKeyInterner {
    state: Arc<RwLock<StableKeyInternerState>>,
}

impl StableKeyInterner {
    /// Returns the id for `key`, assigning a new one the first time it is seen.
    ///
    /// Text already interned is answered under a read lock and without
    /// allocating, so callers holding a `&str` pay nothing extra on the common
    /// path; the owned form is only materialized when the key is new.
    pub fn intern(&self, key: impl AsRef<str> + Into<String>) -> StableKeyId {
        if let Some(id) = self.read().ids.get(key.as_ref()) {
            return *id;
        }

        let key = key.into();
        let mut state = self.write();
        if let Some(id) = state.ids.get(key.as_str()) {
            return *id;
        }
        state.push(key.into())
    }

    /// Interns `key` and returns its text in the same lock acquisition.
    ///
    /// Callers that need both — every fact-metadata construction does, because
    /// the stable-key text is hashed into the payload digest — would otherwise
    /// take the lock twice for one key.
    pub(crate) fn intern_and_resolve(&self, key: &str) -> (StableKeyId, Arc<str>) {
        let mut state = self.write();
        if let Some((text, id)) = state.ids.get_key_value(key) {
            return (*id, Arc::clone(text));
        }
        let text: Arc<str> = Arc::from(key);
        let id = state.push(Arc::clone(&text));
        (id, text)
    }

    pub fn resolve(&self, id: StableKeyId) -> Arc<str> {
        let state = self.read();
        Arc::clone(
            state
                .keys
                .get(id.0 as usize)
                .unwrap_or_else(|| panic!("unknown stable-key id {}", id.0)),
        )
    }

    fn read(&self) -> RwLockReadGuard<'_, StableKeyInternerState> {
        self.state.read().unwrap_or_else(|error| error.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, StableKeyInternerState> {
        self.state
            .write()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub fn detached_clone(&self) -> Self {
        let state = self.read();
        let keys = state.keys.clone();
        let ids = keys
            .iter()
            .enumerate()
            .map(|(index, key)| {
                (
                    Arc::clone(key),
                    StableKeyId(
                        u32::try_from(index)
                            .unwrap_or_else(|_| panic!("stable-key interner exhausted u32 ids")),
                    ),
                )
            })
            .collect();
        Self {
            state: Arc::new(RwLock::new(StableKeyInternerState { keys, ids })),
        }
    }
}

/// Test-only helper: intern into the process-wide test interner.
#[doc(hidden)]
pub fn stable_key_for_test(key: &str) -> StableKeyId {
    test_stable_key_interner().intern(key)
}

/// Test-only process-wide interner for unit tests that lack an `AnalysisDb`.
#[doc(hidden)]
pub fn test_stable_key_interner() -> StableKeyInterner {
    use std::sync::OnceLock;

    static INTERNER: OnceLock<StableKeyInterner> = OnceLock::new();
    INTERNER.get_or_init(StableKeyInterner::default).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_text_reuses_an_id_and_resolves_exactly() {
        let interner = StableKeyInterner::default();

        let first = interner.intern("stable-key".to_string());
        let second = interner.intern("stable-key".to_string());

        assert_eq!(first, second);
        assert_eq!(interner.resolve(first).as_ref(), "stable-key");
    }

    #[test]
    fn ids_are_assigned_in_insertion_order_and_index_their_text() {
        let interner = StableKeyInterner::default();

        let ids = ["first", "second", "third", "second", "first"]
            .map(|key| interner.intern(key))
            .to_vec();

        assert_eq!(
            ids,
            [
                StableKeyId(0),
                StableKeyId(1),
                StableKeyId(2),
                StableKeyId(1),
                StableKeyId(0)
            ]
        );
        for (index, key) in ["first", "second", "third"].iter().enumerate() {
            let id = StableKeyId(u32::try_from(index).expect("small index"));
            assert_eq!(interner.resolve(id).as_ref(), *key);
        }
    }

    #[test]
    fn interning_with_the_text_agrees_with_interning_alone() {
        let interner = StableKeyInterner::default();
        let existing = interner.intern("existing");

        let (reused, reused_text) = interner.intern_and_resolve("existing");
        let (fresh, fresh_text) = interner.intern_and_resolve("fresh");

        assert_eq!(reused, existing);
        assert_eq!(reused_text.as_ref(), "existing");
        assert_eq!(fresh, StableKeyId(1));
        assert_eq!(fresh_text.as_ref(), "fresh");
        assert_eq!(interner.intern("fresh"), fresh);
    }

    #[test]
    fn detached_clone_does_not_share_future_allocations() {
        let interner = StableKeyInterner::default();
        let original = interner.intern("first".to_string());
        let detached = interner.detached_clone();

        let second = interner.intern("second".to_string());

        assert_eq!(detached.resolve(original).as_ref(), "first");
        assert_eq!(second, StableKeyId(1));
        assert_eq!(detached.intern("detached".to_string()), StableKeyId(1));
    }
}
