use serde::{Deserialize, Serialize};

use crate::cache::stable_hash;
use crate::eval::suite::{CaseSelector, SuiteManifest, SuiteTier, normalize_repo_relative_path};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) struct TierSelection {
    pub(crate) suite_id: String,
    pub(crate) tier: SuiteTier,
    pub(crate) selector: String,
    pub(crate) seed: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source_commit: Option<String>,
    pub(crate) selected_case_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) limitations: Vec<String>,
}

pub(crate) fn select_case_ids(
    manifest: &SuiteManifest,
    tier: SuiteTier,
    candidate_ids: &[String],
) -> anyhow::Result<TierSelection> {
    let selector = manifest.tiers.get(&tier).ok_or_else(|| {
        anyhow::anyhow!("suite {} does not define tier {:?}", manifest.id.0, tier)
    })?;
    anyhow::ensure!(
        selector.enabled,
        "suite {} tier {:?} is disabled",
        manifest.id.0,
        tier
    );

    let mut unique_ids = unique_sorted_candidate_ids(candidate_ids)?;
    let limit = selector_limit(selector)?;
    let mut limitations = Vec::new();

    if selector.selector == "all" {
        if let Some(limit) = limit
            && unique_ids.len() > limit
        {
            limitations.push(format!(
                "tier {:?} selector all was capped from {} to {} cases",
                tier,
                unique_ids.len(),
                limit
            ));
            unique_ids.truncate(limit);
        }
    } else if selector.selector.starts_with("sample:balanced:") {
        let sample_size = sample_size(&selector.selector)?;
        let cap = limit.map_or(sample_size, |max_cases| max_cases.min(sample_size));
        unique_ids = deterministic_sample(manifest, selector, &unique_ids, cap);
        if candidate_ids.len() > unique_ids.len() {
            limitations.push(format!(
                "tier {:?} uses deterministic smoke subset {} of {} cases",
                tier,
                unique_ids.len(),
                candidate_ids.len()
            ));
        }
    } else {
        anyhow::bail!("unsupported case selector {}", selector.selector);
    }

    Ok(TierSelection {
        suite_id: manifest.id.0.clone(),
        tier,
        selector: selector.selector.clone(),
        seed: selector
            .deterministic_seed
            .clone()
            .unwrap_or_else(|| "default".to_string()),
        source_commit: manifest.source_commit.clone(),
        selected_case_ids: unique_ids,
        limitations,
    })
}

fn unique_sorted_candidate_ids(candidate_ids: &[String]) -> anyhow::Result<Vec<String>> {
    let mut ids = Vec::with_capacity(candidate_ids.len());
    for id in candidate_ids {
        ids.push(normalize_repo_relative_path("case_id", id)?);
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn selector_limit(selector: &CaseSelector) -> anyhow::Result<Option<usize>> {
    if let Some(max_cases) = selector.max_cases {
        anyhow::ensure!(max_cases > 0, "max_cases must be greater than zero");
    }
    Ok(selector.max_cases)
}

fn sample_size(selector: &str) -> anyhow::Result<usize> {
    let size = selector
        .strip_prefix("sample:balanced:")
        .ok_or_else(|| anyhow::anyhow!("unsupported selector {selector}"))?
        .parse::<usize>()?;
    anyhow::ensure!(size > 0, "sample size must be greater than zero");
    Ok(size)
}

fn deterministic_sample(
    manifest: &SuiteManifest,
    selector: &CaseSelector,
    candidate_ids: &[String],
    sample_size: usize,
) -> Vec<String> {
    let seed = selector.deterministic_seed.as_deref().unwrap_or("default");
    let source_commit = manifest
        .source_commit
        .as_deref()
        .unwrap_or("no-source-commit");
    let mut ranked: Vec<_> = candidate_ids
        .iter()
        .map(|case_id| {
            (
                stable_hash(&[&manifest.id.0, source_commit, seed, case_id]),
                case_id.clone(),
            )
        })
        .collect();
    ranked.sort();
    let mut selected: Vec<_> = ranked
        .into_iter()
        .take(sample_size.min(candidate_ids.len()))
        .map(|(_, case_id)| case_id)
        .collect();
    selected.sort();
    selected
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::eval::suite::{
        ExpectedSource, ExpectedSourceFormat, LocalClonePolicy, SuiteCheckout,
        SuiteCheckoutStrategy, SuiteId, SuiteKind, SuiteLanguageSupport, SuiteScoring,
    };

    #[test]
    fn same_inputs_select_same_cases() {
        let manifest = manifest();
        let candidates = (0..50)
            .map(|index| format!("cases/case-{index}.js"))
            .collect::<Vec<_>>();

        let left = select_case_ids(&manifest, SuiteTier::Fast, &candidates).unwrap();
        let right = select_case_ids(&manifest, SuiteTier::Fast, &candidates).unwrap();

        assert_eq!(left, right);
        assert_eq!(left.selected_case_ids.len(), 5);
        assert_eq!(left.source_commit.as_deref(), Some("abc123"));
        assert!(
            left.limitations
                .iter()
                .any(|limitation| limitation.contains("deterministic smoke subset"))
        );
    }

    #[test]
    fn selector_all_sorts_and_deduplicates() {
        let manifest = manifest();
        let selection = select_case_ids(
            &manifest,
            SuiteTier::Release,
            &["b.js".to_string(), "a.js".to_string(), "b.js".to_string()],
        )
        .unwrap();

        assert_eq!(selection.selected_case_ids, ["a.js", "b.js"]);
        assert!(selection.limitations.is_empty());
    }

    #[test]
    fn rejects_unsafe_case_ids() {
        let manifest = manifest();
        let err =
            select_case_ids(&manifest, SuiteTier::Fast, &["../escape.js".to_string()]).unwrap_err();

        assert!(err.to_string().contains("parent directory"));
    }

    fn manifest() -> SuiteManifest {
        SuiteManifest {
            schema_version: "polint-eval-suite-1".to_string(),
            id: SuiteId("tier-suite".to_string()),
            name: "Tier suite".to_string(),
            kind: SuiteKind::ScannerVulnerability,
            languages: vec!["javascript".to_string()],
            adapter_id: "tier".to_string(),
            scoring_mode: crate::eval::suite::ScoringMode::WholeRepo,
            source_url: Some("https://example.test/tier".to_string()),
            source_commit: Some("abc123".to_string()),
            license: "test".to_string(),
            language_support: SuiteLanguageSupport::Supported,
            checkout: SuiteCheckout {
                strategy: SuiteCheckoutStrategy::LocalClone,
                path: "research/evaluation-harness/repos/tier".to_string(),
                ignored_by_git: true,
                local_clone_policy: LocalClonePolicy::RepoRelativeOnly,
            },
            expected: ExpectedSource {
                format: ExpectedSourceFormat::SuiteNative,
                path: "expected.json".to_string(),
            },
            scoring: SuiteScoring {
                native: Vec::new(),
                unified: vec!["precision".to_string(), "recall".to_string()],
            },
            tiers: BTreeMap::from([
                (
                    SuiteTier::Fast,
                    CaseSelector {
                        enabled: true,
                        selector: "sample:balanced:5".to_string(),
                        max_cases: Some(5),
                        deterministic_seed: Some("determinism-seed".to_string()),
                    },
                ),
                (
                    SuiteTier::Release,
                    CaseSelector {
                        enabled: true,
                        selector: "all".to_string(),
                        max_cases: None,
                        deterministic_seed: None,
                    },
                ),
            ]),
        }
    }
}
