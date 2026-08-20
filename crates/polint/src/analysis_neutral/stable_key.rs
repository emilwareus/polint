use crate::analysis_api::{FactFamily, stable_key_text_from_parts};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StableFactKey(String);

impl StableFactKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

pub fn semantic_stable_key(
    interner: &crate::internal_core::StableKeyInterner,
    family: FactFamily,
    parts: &[(&str, String)],
) -> StableFactKey {
    StableFactKey(stable_key_text_from_parts(interner, family, parts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_api::FactFamily;
    use crate::analysis_neutral::ids::PlaceId;

    #[test]
    fn semantic_stable_key_sorts_parts_normalizes_backslashes_and_includes_family() {
        let first = semantic_stable_key(
            &crate::internal_core::test_stable_key_interner(),
            FactFamily::Function,
            &[
                ("path", "src\\main.go".to_string()),
                ("name", "handler".to_string()),
            ],
        );
        let second = semantic_stable_key(
            &crate::internal_core::test_stable_key_interner(),
            FactFamily::Function,
            &[
                ("name", "handler".to_string()),
                ("path", "src/main.go".to_string()),
            ],
        );

        assert_eq!(first, second);
        assert!(first.as_str().contains("8:Function"));
        assert!(first.as_str().contains("4:path=11:src/main.go"));
    }

    #[test]
    fn semantic_stable_key_does_not_require_dense_run_local_ids() {
        let key = semantic_stable_key(
            &crate::internal_core::test_stable_key_interner(),
            FactFamily::Function,
            &[("place", "local:handler:value".to_string())],
        );

        assert!(!key.as_str().contains(&PlaceId(42).0.to_string()));
        assert_eq!(
            key.into_string(),
            semantic_stable_key(
                &crate::internal_core::test_stable_key_interner(),
                FactFamily::Function,
                &[("place", "local:handler:value".to_string())]
            )
            .into_string()
        );
    }
}
