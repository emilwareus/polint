#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_parts_is_deterministic_and_kind_separated() {
        let first = Digest::from_parts(DigestKind::SourceText, "source_text", &["path", "hash"]);
        let second = Digest::from_parts(DigestKind::SourceText, "source_text", &["path", "hash"]);
        let config = Digest::from_parts(DigestKind::Config, "source_text", &["path", "hash"]);

        assert_eq!(first, second);
        assert_ne!(first, config);
    }

    #[test]
    fn from_unordered_sorts_input_digests_canonically() {
        let a = Digest::from_parts(DigestKind::SourceText, "file", &["a"]);
        let b = Digest::from_parts(DigestKind::SourceText, "file", &["b"]);

        assert_eq!(
            Digest::from_unordered(DigestKind::LayerOutput, "layer", vec![b.clone(), a.clone()]),
            Digest::from_unordered(DigestKind::LayerOutput, "layer", vec![a, b])
        );
    }

    #[test]
    fn serde_and_display_include_kind_and_value() {
        let digest = Digest::from_parts(DigestKind::SourceText, "source_text", &["path"]);
        let json = serde_json::to_string(&digest).expect("digest should serialize");

        assert!(json.contains("\"kind\""));
        assert!(json.contains("\"value\""));
        assert_eq!(digest.to_string(), format!("source_text:{}", digest.value));
    }
}
