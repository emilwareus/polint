use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum UnknownCategory {
    SetupMissing,
    UnsupportedSemantic,
    MissingFact,
    OutOfScope,
    GoPackagesLoadFailed,
    GoVersionUnsupported,
    GoSidecarTimeout,
    BudgetExceeded,
    Rejected,
    ModelMissing,
}

impl UnknownCategory {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::SetupMissing => "setup_missing",
            Self::UnsupportedSemantic => "unsupported_semantic",
            Self::MissingFact => "missing_fact",
            Self::OutOfScope => "out_of_scope",
            Self::GoPackagesLoadFailed => "go_packages_load_failed",
            Self::GoVersionUnsupported => "go_version_unsupported",
            Self::GoSidecarTimeout => "go_sidecar_timeout",
            Self::BudgetExceeded => "budget_exceeded",
            Self::Rejected => "rejected",
            Self::ModelMissing => "model_missing",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct UnknownSpan {
    pub(crate) line: u32,
    pub(crate) column: u32,
}

impl UnknownSpan {
    pub(crate) fn from_span(span: &crate::core::Span) -> Self {
        let range = span.diagnostic_range();
        Self {
            line: range.start_line,
            column: range.start_col,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct UnknownRow {
    pub(crate) category: UnknownCategory,
    pub(crate) capability: Option<String>,
    pub(crate) family: Option<String>,
    pub(crate) provider: String,
    pub(crate) file: String,
    pub(crate) span: Option<UnknownSpan>,
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
    pub(crate) precision: Option<String>,
    pub(crate) docs_path: Option<String>,
    pub(crate) suggested_artifact: Option<String>,
    pub(crate) source_stable_key: Option<String>,
    pub(crate) stable_sort_key: String,
}

impl UnknownRow {
    pub(crate) fn new(interner: &crate::core::StableKeyInterner, input: UnknownRowInput) -> Self {
        let stable_sort_key = stable_sort_key(
            interner,
            input.file.as_str(),
            input.span.as_ref(),
            input.category,
            input.capability.as_deref(),
            input.reason.as_deref(),
            input.source_stable_key.as_deref(),
        );
        Self {
            category: input.category,
            capability: input.capability,
            family: input.family,
            provider: input.provider,
            file: input.file,
            span: input.span,
            status: input.status,
            reason: input.reason,
            precision: input.precision,
            docs_path: input.docs_path,
            suggested_artifact: input.suggested_artifact,
            source_stable_key: input.source_stable_key,
            stable_sort_key,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnknownRowInput {
    pub(crate) category: UnknownCategory,
    pub(crate) capability: Option<String>,
    pub(crate) family: Option<String>,
    pub(crate) provider: String,
    pub(crate) file: String,
    pub(crate) span: Option<UnknownSpan>,
    pub(crate) status: String,
    pub(crate) reason: Option<String>,
    pub(crate) precision: Option<String>,
    pub(crate) docs_path: Option<String>,
    pub(crate) suggested_artifact: Option<String>,
    pub(crate) source_stable_key: Option<String>,
}

pub(crate) fn normalize_rows(mut rows: Vec<UnknownRow>) -> Vec<UnknownRow> {
    rows.sort_by(|left, right| left.stable_sort_key.cmp(&right.stable_sort_key));
    rows.dedup_by(|left, right| left.stable_sort_key == right.stable_sort_key);
    rows
}

fn stable_sort_key(
    interner: &crate::core::StableKeyInterner,
    file: &str,
    span: Option<&UnknownSpan>,
    category: UnknownCategory,
    capability: Option<&str>,
    reason: Option<&str>,
    source_stable_key: Option<&str>,
) -> String {
    let (line, column) = span.map_or((0, 0), |span| (span.line, span.column));
    crate::analysis_kernel::stable_key_text_from_parts(
        interner,
        crate::analysis_kernel::FactFamily::UnsupportedSemantic,
        &[
            ("file", file.to_string()),
            ("line", line.to_string()),
            ("column", column.to_string()),
            ("category", category.as_str().to_string()),
            ("capability", capability.unwrap_or_default().to_string()),
            ("reason", reason.unwrap_or_default().to_string()),
            ("source", source_stable_key.unwrap_or_default().to_string()),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn category_labels_are_stable_snake_case() {
        assert_eq!(UnknownCategory::SetupMissing.as_str(), "setup_missing");
        assert_eq!(
            UnknownCategory::GoPackagesLoadFailed.as_str(),
            "go_packages_load_failed"
        );
        assert_eq!(
            UnknownCategory::GoSidecarTimeout.as_str(),
            "go_sidecar_timeout"
        );
    }

    #[test]
    fn rows_sort_deterministically_by_stable_sort_key() {
        let mut rows = vec![row("b.ts", 3, "z"), row("a.ts", 2, "a")];

        rows = normalize_rows(rows);

        assert_eq!(rows[0].file, "a.ts");
        assert_eq!(rows[1].file, "b.ts");
    }

    fn row(file: &str, line: u32, source: &str) -> UnknownRow {
        UnknownRow::new(
            &crate::core::AnalysisDb::new().stable_key_interner(),
            UnknownRowInput {
                category: UnknownCategory::SetupMissing,
                capability: Some("references".to_string()),
                family: Some("Reference".to_string()),
                provider: "polint.symbol_graph".to_string(),
                file: file.to_string(),
                span: Some(UnknownSpan { line, column: 1 }),
                status: "setup_missing".to_string(),
                reason: Some("test".to_string()),
                precision: Some("setup_missing".to_string()),
                docs_path: None,
                suggested_artifact: None,
                source_stable_key: Some(source.to_string()),
            },
        )
    }
}
