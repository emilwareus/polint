#![cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Phase 22 plan 01 defines the internal eval schema before later harness plans consume it"
    )
)]

pub(crate) mod fixtures;
pub(crate) mod matcher;
pub(crate) mod metrics;
pub(crate) mod model;
pub(crate) mod observed;
pub(crate) mod report;

#[cfg(test)]
mod semantic_rows {
    use crate::eval::matcher::{MatchOutcome, MatcherConfig, match_case};
    use crate::eval::model::{
        AssertionMode, ExpectedFact, ExpectedItem, ObservedFact, ObservedItem, ObservedStatus,
    };
    use serde_json::json;

    #[test]
    fn eval_expected_facts_accept_semantic_families_and_statuses() {
        let cases = crate::eval::model::SEMANTIC_FACT_FAMILIES
            .iter()
            .copied()
            .zip([
                "resolved",
                "dynamic",
                "resolved",
                "ambiguous",
                "unresolved",
                "generated",
                "external",
            ]);

        for (family, status) in cases {
            let manifest = format!(
                r#"
schema_version = "polint-eval-fixture-1"
case_id = "semantic-row-model"
area = "facts"

[repo]
path = "repo"

[[expected]]
fact = {{ family = "{family}", stable_key = "semantic:{family}", mode = "partial", producer_id = "polint.symbol_graph", precision = "setup_aware", status = "{status}" }}
"#
            );
            let parsed: crate::eval::fixtures::NativeFixtureManifest =
                toml::from_str(&manifest).expect("semantic expected fact should parse");
            assert_eq!(parsed.expected.len(), 1);
        }
    }

    #[test]
    fn observed_semantic_debug_rows_normalize_to_fact_rows_with_evidence() {
        let debug = json!({
            "semantic": {
                "scopes": [{
                    "family": "Scope",
                    "stable_key": "scope:src/app.ts:module",
                    "producer_id": "polint.symbol_graph",
                    "layer_id": "polint.symbol_graph",
                    "status": "resolved",
                    "metadata": { "precision": "setup_aware" },
                    "path": "src/app.ts",
                    "span": {
                        "start_byte": 0,
                        "end_byte": 12,
                        "start_line": 1,
                        "start_col": 1,
                        "end_line": 1,
                        "end_col": 13
                    }
                }],
                "imports": [],
                "exports": [],
                "aliases": [],
                "resolutions": [],
                "generated_symbols": [],
                "stable_exports": []
            }
        });

        let observed = crate::eval::observed::metadata_debug_facts_for_test(&debug);

        assert!(observed.iter().any(|item| match item {
            ObservedItem::Fact(fact) => {
                fact.family == "Scope"
                    && fact.stable_key == "scope:src/app.ts:module"
                    && fact.producer_id.as_deref() == Some("polint.symbol_graph")
                    && fact.provenance.as_deref() == Some("polint.symbol_graph")
                    && fact.precision.as_deref() == Some("setup_aware")
                    && fact.status == Some(ObservedStatus::Resolved)
                    && fact.payload.as_deref().is_some_and(|payload| {
                        payload.contains("\"path\":\"src/app.ts\"")
                            && payload.contains("\"start_byte\":0")
                    })
            }
            _ => false,
        }));
    }

    #[test]
    fn semantic_partial_and_unknown_status_matching_works() {
        let summaries = match_case(
            &[ExpectedItem::Fact(ExpectedFact {
                family: "Resolution".to_string(),
                stable_key: "handler".to_string(),
                mode: AssertionMode::Partial,
                producer_id: Some("polint.symbol_graph".to_string()),
                precision: Some("ambiguous".to_string()),
                status: Some(ObservedStatus::Ambiguous),
                false_positive_trap: false,
            })],
            &[ObservedItem::Fact(ObservedFact {
                family: "Resolution".to_string(),
                stable_key: "resolution:src/app.ts:handler:candidates".to_string(),
                mode: AssertionMode::Exact,
                producer_id: Some("polint.symbol_graph".to_string()),
                provenance: Some("polint.symbol_graph".to_string()),
                precision: Some("ambiguous".to_string()),
                status: Some(ObservedStatus::Ambiguous),
                payload: None,
            })],
            MatcherConfig::default(),
        );

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].outcome, MatchOutcome::Unknown);
    }
}
