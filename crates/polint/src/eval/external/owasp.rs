use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{Context, bail};

use crate::eval::adapter::{BenchmarkAdapter, PreparedCase, RawObservedOutput};
use crate::eval::matcher::MatchOutcome;
use crate::eval::model::{
    AssertionMode, EvaluationCase, ExpectedDiagnostic, ExpectedItem, FixtureArea,
    ObservedDiagnostic, ObservedItem, ObservedStatus,
};
use crate::eval::report::CaseResult;
use crate::eval::suite::{SuiteLanguageSupport, SuiteManifest};

pub(crate) struct OwaspExpectedResultsAdapter;

impl BenchmarkAdapter for OwaspExpectedResultsAdapter {
    fn adapter_id(&self) -> &'static str {
        "owasp_expected_results_csv"
    }

    fn language_support(&self, manifest: &SuiteManifest) -> SuiteLanguageSupport {
        manifest.language_support
    }

    fn enumerate_cases(&self, _manifest: &SuiteManifest) -> anyhow::Result<Vec<EvaluationCase>> {
        bail!("OWASP cases require expected-results CSV content; use parse_expected_results_csv")
    }

    fn prepare_case(
        &self,
        manifest: &SuiteManifest,
        case: &EvaluationCase,
    ) -> anyhow::Result<PreparedCase> {
        Ok(PreparedCase {
            case_id: case.case_id.clone(),
            workspace_root: PathBuf::from(&manifest.checkout.path),
            target_files: vec![PathBuf::from(&case.repo_path)],
        })
    }

    fn normalize_observed(
        &self,
        _manifest: &SuiteManifest,
        _case: &EvaluationCase,
        raw: RawObservedOutput,
    ) -> anyhow::Result<Vec<ObservedItem>> {
        Ok(raw.observed)
    }

    fn suite_native_metrics(
        &self,
        manifest: &SuiteManifest,
        results: &[CaseResult],
    ) -> anyhow::Result<BTreeMap<String, f64>> {
        ensure_adapter_only_mode(manifest)?;
        Ok(score_case_results(results).as_metric_map())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OwaspExpectedCase {
    pub(crate) test_name: String,
    pub(crate) category: String,
    pub(crate) vulnerable: bool,
    pub(crate) cwe: Option<u32>,
    pub(crate) full_category_name: Option<String>,
}

impl OwaspExpectedCase {
    pub(crate) fn to_evaluation_case(&self, suite_id: &str) -> EvaluationCase {
        EvaluationCase {
            case_id: self.test_name.clone(),
            area: FixtureArea::Diagnostics,
            repo_path: self.target_selector(),
            expected: vec![ExpectedItem::Diagnostic(self.to_expected_diagnostic())],
            observed: Vec::new(),
        }
        .with_suite_id(suite_id)
    }

    pub(crate) fn to_expected_diagnostic(&self) -> ExpectedDiagnostic {
        ExpectedDiagnostic {
            rule_id: self.rule_id(),
            relative_path: self.target_selector(),
            line: None,
            fingerprint: Some(self.fingerprint()),
            mode: if self.vulnerable {
                AssertionMode::Exact
            } else {
                AssertionMode::Forbidden
            },
            false_positive_trap: !self.vulnerable,
        }
    }

    pub(crate) fn synthetic_observed_diagnostic(&self) -> ObservedDiagnostic {
        ObservedDiagnostic {
            rule_id: self.rule_id(),
            relative_path: self.target_selector(),
            line: None,
            fingerprint: Some(self.fingerprint()),
            mode: AssertionMode::Exact,
            producer_id: Some("owasp.synthetic".to_string()),
            provenance: Some("synthetic_observed".to_string()),
            precision: Some("exact".to_string()),
            status: Some(ObservedStatus::Present),
        }
    }

    fn rule_id(&self) -> String {
        format!("owasp/{}", self.category)
    }

    fn target_selector(&self) -> String {
        self.test_name.clone()
    }

    fn fingerprint(&self) -> String {
        format!(
            "owasp:{}:{}:{}:{}",
            self.test_name,
            self.category,
            self.cwe
                .map_or_else(|| "cwe-unknown".to_string(), |cwe| { format!("cwe-{cwe}") }),
            if self.vulnerable { "vuln" } else { "safe" }
        )
    }
}

trait WithSuiteId {
    fn with_suite_id(self, suite_id: &str) -> Self;
}

impl WithSuiteId for EvaluationCase {
    fn with_suite_id(mut self, suite_id: &str) -> Self {
        self.repo_path = format!("{suite_id}/{}", self.repo_path);
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct OwaspNativeMetrics {
    pub(crate) true_positives: u64,
    pub(crate) false_negatives: u64,
    pub(crate) true_negatives: u64,
    pub(crate) false_positives: u64,
}

impl OwaspNativeMetrics {
    pub(crate) fn as_metric_map(&self) -> BTreeMap<String, f64> {
        [
            ("owasp.tp".to_string(), self.true_positives as f64),
            ("owasp.fn".to_string(), self.false_negatives as f64),
            ("owasp.tn".to_string(), self.true_negatives as f64),
            ("owasp.fp".to_string(), self.false_positives as f64),
            ("owasp.tpr".to_string(), self.tpr()),
            ("owasp.fpr".to_string(), self.fpr()),
            ("owasp.precision".to_string(), self.precision()),
            ("owasp.f_score".to_string(), self.f_score()),
            ("owasp.tpr_minus_fpr".to_string(), self.tpr() - self.fpr()),
        ]
        .into_iter()
        .collect()
    }

    fn tpr(&self) -> f64 {
        ratio(
            self.true_positives,
            self.true_positives + self.false_negatives,
        )
    }

    fn fpr(&self) -> f64 {
        ratio(
            self.false_positives,
            self.false_positives + self.true_negatives,
        )
    }

    fn precision(&self) -> f64 {
        ratio(
            self.true_positives,
            self.true_positives + self.false_positives,
        )
    }

    fn f_score(&self) -> f64 {
        let precision = self.precision();
        let recall = self.tpr();
        if precision == 0.0 && recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        }
    }
}

pub(crate) fn parse_expected_results_csv(raw: &str) -> anyhow::Result<Vec<OwaspExpectedCase>> {
    let rows = parse_csv_rows(raw)?;
    let (header, body) = rows
        .split_first()
        .context("OWASP expected-results CSV must include a header")?;
    let columns = OwaspColumns::from_header(header)?;
    let mut cases = Vec::new();

    for (index, row) in body.iter().enumerate() {
        if row.iter().all(|cell| cell.trim().is_empty()) {
            continue;
        }
        cases.push(columns.case_from_row(row).with_context(|| {
            format!(
                "parse OWASP expected-results row {}",
                index.saturating_add(2)
            )
        })?);
    }

    Ok(cases)
}

pub(crate) fn score_case_results(results: &[CaseResult]) -> OwaspNativeMetrics {
    let mut metrics = OwaspNativeMetrics::default();
    for summary in results.iter().flat_map(|case| case.matches.iter()) {
        match summary.outcome {
            MatchOutcome::TruePositive => metrics.true_positives += 1,
            MatchOutcome::FalseNegative => metrics.false_negatives += 1,
            MatchOutcome::TrueNegative => metrics.true_negatives += 1,
            MatchOutcome::FalsePositive | MatchOutcome::ForbiddenHit | MatchOutcome::TrapHit => {
                metrics.false_positives += 1;
            }
            MatchOutcome::Unconfirmed
            | MatchOutcome::Unknown
            | MatchOutcome::RuntimeBudgetPassed
            | MatchOutcome::RuntimeBudgetFailed => {}
        }
    }
    metrics
}

pub(crate) fn ensure_adapter_only_mode(manifest: &SuiteManifest) -> anyhow::Result<()> {
    anyhow::ensure!(
        manifest.language_support == SuiteLanguageSupport::AdapterOnly,
        "OWASP Java/Python suites must remain adapter_only until polint supports those languages"
    );
    Ok(())
}

fn ratio(numerator: u64, denominator: u64) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

struct OwaspColumns {
    test_name: usize,
    category: usize,
    vulnerable: usize,
    cwe: usize,
    full_category_name: Option<usize>,
}

impl OwaspColumns {
    fn from_header(header: &[String]) -> anyhow::Result<Self> {
        Ok(Self {
            test_name: required_column(header, "testname")?,
            category: required_column(header, "category")?,
            vulnerable: required_column(header, "realvulnerability")?,
            cwe: required_column(header, "cwe")?,
            full_category_name: optional_column(header, "fullcategoryname"),
        })
    }

    fn case_from_row(&self, row: &[String]) -> anyhow::Result<OwaspExpectedCase> {
        let test_name = required_cell(row, self.test_name, "test name")?;
        let category = required_cell(row, self.category, "category")?;
        let vulnerable = parse_bool(required_cell(row, self.vulnerable, "real vulnerability")?)?;
        let cwe = parse_cwe(row.get(self.cwe).map(String::as_str).unwrap_or_default())?;
        let full_category_name = self
            .full_category_name
            .and_then(|index| row.get(index))
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        Ok(OwaspExpectedCase {
            test_name: test_name.to_string(),
            category: category.to_string(),
            vulnerable,
            cwe,
            full_category_name,
        })
    }
}

fn required_column(header: &[String], normalized_name: &str) -> anyhow::Result<usize> {
    optional_column(header, normalized_name)
        .with_context(|| format!("missing required OWASP CSV column `{normalized_name}`"))
}

fn optional_column(header: &[String], normalized_name: &str) -> Option<usize> {
    header
        .iter()
        .position(|cell| normalize_header_cell(cell) == normalized_name)
}

fn normalize_header_cell(value: &str) -> String {
    value
        .trim_start_matches('\u{feff}')
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn required_cell<'a>(row: &'a [String], index: usize, field: &str) -> anyhow::Result<&'a str> {
    row.get(index)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .with_context(|| format!("missing required OWASP CSV field `{field}`"))
}

fn parse_bool(value: &str) -> anyhow::Result<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        other => bail!("unsupported OWASP truth value `{other}`"),
    }
}

fn parse_cwe(value: &str) -> anyhow::Result<Option<u32>> {
    let value = value.trim();
    if value.is_empty() {
        return Ok(None);
    }
    let value = value
        .strip_prefix("CWE-")
        .or_else(|| value.strip_prefix("cwe-"))
        .unwrap_or(value);
    Ok(Some(value.parse()?))
}

fn parse_csv_rows(raw: &str) -> anyhow::Result<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut chars = raw.chars().peekable();
    let mut in_quotes = false;

    while let Some(ch) = chars.next() {
        match ch {
            '"' if in_quotes && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => {
                row.push(std::mem::take(&mut cell));
            }
            '\n' if !in_quotes => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            '\r' if !in_quotes => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            _ => cell.push(ch),
        }
    }

    if in_quotes {
        bail!("unterminated quoted CSV field");
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::matcher::{MatcherConfig, match_case};
    use crate::eval::metrics::compute_metrics;
    use crate::eval::report::{MetricSummary, RuntimeObservation};

    #[test]
    fn owasp_parser_reads_expected_results_rows() {
        let cases = parse_expected_results_csv(
            "# test name,category,real vulnerability,cwe,Benchmark version: 1.2,Full Category Name\n\
             BenchmarkTest00001,pathtraver,true,22,,Path Traversal\n\
             BenchmarkTest00002,sqli,false,89,,SQL Injection\n",
        )
        .unwrap();

        assert_eq!(cases.len(), 2);
        assert_eq!(cases[0].test_name, "BenchmarkTest00001");
        assert_eq!(cases[0].category, "pathtraver");
        assert!(cases[0].vulnerable);
        assert_eq!(cases[0].cwe, Some(22));
        assert_eq!(
            cases[0].to_evaluation_case("owasp-java").repo_path,
            "owasp-java/BenchmarkTest00001"
        );
        assert_eq!(
            cases[1].full_category_name.as_deref(),
            Some("SQL Injection")
        );
        assert!(!cases[1].vulnerable);
    }

    #[test]
    fn owasp_parser_handles_quoted_cells_and_cwe_prefixes() {
        let cases = parse_expected_results_csv(
            "# test name,category,real vulnerability,cwe,Full Category Name\n\
             BenchmarkTest00003,\"cmdi\",yes,CWE-78,\"Command, Injection\"\n",
        )
        .unwrap();

        assert_eq!(cases[0].category, "cmdi");
        assert_eq!(cases[0].cwe, Some(78));
        assert_eq!(
            cases[0].full_category_name.as_deref(),
            Some("Command, Injection")
        );
    }

    #[test]
    fn owasp_synthetic_observed_rows_score_vulnerable_and_safe_cases() {
        let cases = parse_expected_results_csv(
            "# test name,category,real vulnerability,cwe\n\
             BenchmarkTest00001,pathtraver,true,22\n\
             BenchmarkTest00002,pathtraver,true,22\n\
             BenchmarkTest00003,sqli,false,89\n\
             BenchmarkTest00004,sqli,false,89\n",
        )
        .unwrap();

        let results = vec![
            result_for_case(&cases[0], true),
            result_for_case(&cases[1], false),
            result_for_case(&cases[2], false),
            result_for_case(&cases[3], true),
        ];
        let metrics = score_case_results(&results);
        let map = metrics.as_metric_map();
        let unified: MetricSummary = compute_metrics(
            &results
                .iter()
                .flat_map(|result| result.matches.iter().cloned())
                .collect::<Vec<_>>(),
        )
        .into();

        assert_eq!(metrics.true_positives, 1);
        assert_eq!(metrics.false_negatives, 1);
        assert_eq!(metrics.true_negatives, 1);
        assert_eq!(metrics.false_positives, 1);
        assert_eq!(map.get("owasp.tpr"), Some(&0.5));
        assert_eq!(map.get("owasp.fpr"), Some(&0.5));
        assert_eq!(map.get("owasp.tpr_minus_fpr"), Some(&0.0));
        assert_eq!(unified.true_positives, 1);
        assert_eq!(unified.true_negatives, 1);
        assert_eq!(unified.false_negatives, 1);
        assert_eq!(unified.false_positive_trap_hits, 1);
    }

    #[test]
    fn owasp_adapter_only_guard_rejects_supported_language_claims() {
        let adapter = OwaspExpectedResultsAdapter;
        let manifest = SuiteManifest {
            schema_version: "polint-eval-suite-1".to_string(),
            id: crate::eval::suite::SuiteId("owasp-java".to_string()),
            name: "OWASP Java".to_string(),
            kind: crate::eval::suite::SuiteKind::ScannerVulnerability,
            languages: vec!["java".to_string()],
            adapter_id: "owasp_expected_results_csv".to_string(),
            source_url: Some("https://github.com/OWASP-Benchmark/BenchmarkJava".to_string()),
            source_commit: Some("61b831658171".to_string()),
            license: "license-review-needed".to_string(),
            language_support: SuiteLanguageSupport::Supported,
            checkout: crate::eval::suite::SuiteCheckout {
                strategy: crate::eval::suite::SuiteCheckoutStrategy::LocalClone,
                path: "research/evaluation-harness/repos/BenchmarkJava".to_string(),
                ignored_by_git: true,
                local_clone_policy: crate::eval::suite::LocalClonePolicy::RepoRelativeOnly,
            },
            expected: crate::eval::suite::ExpectedSource {
                format: crate::eval::suite::ExpectedSourceFormat::OwaspExpectedResultsCsv,
                path: "expectedresults-1.2.csv".to_string(),
            },
            scoring: crate::eval::suite::SuiteScoring {
                native: vec!["owasp_confusion_matrix".to_string()],
                unified: vec!["precision".to_string()],
            },
            tiers: [(
                crate::eval::suite::SuiteTier::Fast,
                crate::eval::suite::CaseSelector {
                    enabled: true,
                    selector: "sample:balanced:10".to_string(),
                    max_cases: Some(10),
                    deterministic_seed: Some("owasp".to_string()),
                },
            )]
            .into_iter()
            .collect(),
        };

        assert_eq!(adapter.adapter_id(), "owasp_expected_results_csv");
        assert_eq!(
            adapter.language_support(&manifest),
            SuiteLanguageSupport::Supported
        );
        assert!(ensure_adapter_only_mode(&manifest).is_err());
    }

    fn result_for_case(case: &OwaspExpectedCase, observed: bool) -> CaseResult {
        let expected = vec![ExpectedItem::Diagnostic(case.to_expected_diagnostic())];
        let observed = if observed {
            vec![ObservedItem::Diagnostic(
                case.synthetic_observed_diagnostic(),
            )]
        } else {
            Vec::new()
        };
        let matches = match_case(&expected, &observed, MatcherConfig::default());
        CaseResult {
            case_id: case.test_name.clone(),
            area: FixtureArea::Diagnostics,
            expected,
            observed,
            matches,
            runtime: RuntimeObservation {
                budget_name: "owasp".to_string(),
                budget_passed: true,
                observed_runtime_ms: None,
            },
        }
    }
}
