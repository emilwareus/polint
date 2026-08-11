use std::collections::BTreeMap;

use crate::entrypoints::facts::UnresolvedFrameworkFact;

/// Merge unresolved framework facts from Go and TS/JS recognizers.
/// Deduplicates by stable_key (keeps first occurrence), sorts by stable key.
pub fn merge_unresolved(
    interner: &polint_core::StableKeyInterner,
    go: Vec<UnresolvedFrameworkFact>,
    ts: Vec<UnresolvedFrameworkFact>,
) -> Vec<UnresolvedFrameworkFact> {
    let mut by_key = BTreeMap::new();

    // Insert Go facts first, then TS/JS facts; first occurrence wins per dedup rule
    for fact in go.into_iter().chain(ts) {
        by_key
            .entry(interner.resolve(fact.stable_key))
            .or_insert(fact);
    }

    by_key.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entrypoints::facts::{EntrypointPrecision, UnresolvedFrameworkReason};
    use crate::ids::UnresolvedFrameworkId;
    use polint_core::{FileId, Language, Span};

    fn make_unresolved(
        language: Language,
        stable_key: &str,
        framework_id: &str,
    ) -> UnresolvedFrameworkFact {
        UnresolvedFrameworkFact {
            id: UnresolvedFrameworkId(0),
            language,
            file: FileId(1),
            span: Span::point(FileId(1), 1, 1),
            framework_id: framework_id.to_string(),
            reason: UnresolvedFrameworkReason::UnsupportedFrameworkVersion,
            evidence: "import detected".to_string(),
            scope_description: "test scope".to_string(),
            precision: EntrypointPrecision::Conservative,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: polint_core::stable_key_for_test(stable_key),
        }
    }

    #[test]
    fn merge_combines_go_and_ts_facts() {
        let go = vec![make_unresolved(Language::Go, "go-fact-1", "go.gin")];
        let ts = vec![make_unresolved(
            Language::TypeScript,
            "ts-fact-1",
            "ts.fastify",
        )];

        let merged = merge_unresolved(&polint_core::test_stable_key_interner(), go, ts);

        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn merge_deduplicates_by_stable_key_keeping_first() {
        let go = vec![make_unresolved(Language::Go, "shared-key", "go.gin")];
        let ts = vec![make_unresolved(
            Language::TypeScript,
            "shared-key",
            "ts.gin",
        )];

        let merged = merge_unresolved(&polint_core::test_stable_key_interner(), go, ts);

        assert_eq!(merged.len(), 1);
        // First occurrence (Go) wins
        assert_eq!(merged[0].language, Language::Go);
        assert_eq!(merged[0].framework_id, "go.gin");
    }

    #[test]
    fn merge_sorts_by_stable_key() {
        let go = vec![make_unresolved(Language::Go, "z-key", "go.gin")];
        let ts = vec![
            make_unresolved(Language::TypeScript, "m-key", "ts.fastify"),
            make_unresolved(Language::TypeScript, "a-key", "ts.koa"),
        ];

        let merged = merge_unresolved(&polint_core::test_stable_key_interner(), go, ts);

        let interner = polint_core::test_stable_key_interner();
        let keys: Vec<_> = merged
            .iter()
            .map(|fact| interner.resolve(fact.stable_key))
            .collect();
        assert_eq!(
            keys,
            vec![
                std::sync::Arc::<str>::from("a-key"),
                std::sync::Arc::<str>::from("m-key"),
                std::sync::Arc::<str>::from("z-key"),
            ]
        );
    }

    #[test]
    fn merge_handles_empty_inputs() {
        let merged = merge_unresolved(
            &polint_core::test_stable_key_interner(),
            Vec::new(),
            Vec::new(),
        );
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_handles_one_empty_input() {
        let go = vec![make_unresolved(Language::Go, "fact-1", "go.gin")];

        let merged = merge_unresolved(&polint_core::test_stable_key_interner(), go, Vec::new());

        assert_eq!(merged.len(), 1);
        assert_eq!(
            polint_core::test_stable_key_interner()
                .resolve(merged[0].stable_key)
                .as_ref(),
            "fact-1"
        );
    }
}
