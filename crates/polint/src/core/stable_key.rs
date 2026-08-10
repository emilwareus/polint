use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub(crate) struct StableKeyId(pub(crate) u32);

#[derive(Debug, Default)]
struct StableKeyInternerState {
    keys: Vec<Arc<str>>,
    ids: HashMap<Arc<str>, StableKeyId>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct StableKeyInterner {
    state: Arc<Mutex<StableKeyInternerState>>,
}

impl StableKeyInterner {
    pub(crate) fn intern(&self, key: String) -> StableKeyId {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(id) = state.ids.get(key.as_str()) {
            return *id;
        }

        let id = StableKeyId(
            u32::try_from(state.keys.len())
                .unwrap_or_else(|_| panic!("stable-key interner exhausted u32 ids")),
        );
        let key: Arc<str> = key.into();
        state.keys.push(Arc::clone(&key));
        state.ids.insert(key, id);
        id
    }

    pub(crate) fn resolve(&self, id: StableKeyId) -> Arc<str> {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        Arc::clone(
            state
                .keys
                .get(id.0 as usize)
                .unwrap_or_else(|| panic!("unknown stable-key id {}", id.0)),
        )
    }

    pub(crate) fn detached_clone(&self) -> Self {
        let state = self.state.lock().unwrap_or_else(|error| error.into_inner());
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
            state: Arc::new(Mutex::new(StableKeyInternerState { keys, ids })),
        }
    }
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
