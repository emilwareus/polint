use crate::go::lifecycle::GoAnalysisConfig;

pub(crate) fn go_semantic_lifecycle_digest(config: &GoAnalysisConfig) -> String {
    let mut parts = vec![
        format!("include_tests={}", config.include_tests),
        format!("offline={}", config.offline),
    ];
    parts.extend(
        config
            .module_roots
            .iter()
            .map(|root| format!("module_root={root}")),
    );
    parts.extend(
        config
            .package_patterns
            .iter()
            .map(|pattern| format!("package_pattern={pattern}")),
    );
    parts.extend(
        config
            .build_tags
            .iter()
            .map(|tag| format!("build_tag={tag}")),
    );
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    crate::cache::stable_hash(&refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_digest_changes_when_build_tags_change() {
        let mut first = config();
        let mut second = config();
        second.build_tags.push("integration".to_string());

        assert_ne!(
            go_semantic_lifecycle_digest(&first),
            go_semantic_lifecycle_digest(&second)
        );
        first.build_tags.push("integration".to_string());
        assert_eq!(
            go_semantic_lifecycle_digest(&first),
            go_semantic_lifecycle_digest(&second)
        );
    }

    fn config() -> GoAnalysisConfig {
        GoAnalysisConfig {
            module_roots: vec![".".to_string()],
            package_patterns: vec!["./...".to_string()],
            build_tags: Vec::new(),
            include_tests: true,
            offline: false,
            files_without_module_root: Vec::new(),
        }
    }
}
