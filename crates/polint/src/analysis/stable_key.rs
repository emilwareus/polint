#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::ids::PlaceId;
    use crate::analysis_kernel::FactFamily;

    #[test]
    fn semantic_stable_key_sorts_parts_normalizes_backslashes_and_includes_family() {
        let first = semantic_stable_key(
            FactFamily::Function,
            &[
                ("path", "src\\main.go".to_string()),
                ("name", "handler".to_string()),
            ],
        );
        let second = semantic_stable_key(
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
            FactFamily::Function,
            &[("place", "local:handler:value".to_string())],
        );

        assert!(!key.as_str().contains(&PlaceId(42).0.to_string()));
        assert_eq!(
            key.into_string(),
            semantic_stable_key(
                FactFamily::Function,
                &[("place", "local:handler:value".to_string())]
            )
            .into_string()
        );
    }
}
