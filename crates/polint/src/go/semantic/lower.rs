use std::collections::BTreeMap;

use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, FileId, Language, Span};
use crate::go::semantic::facts::{
    GoSemanticCallStatus, GoSemanticCallsiteFact, GoSemanticCallsiteId, GoSemanticFunctionFact,
    GoSemanticFunctionId, GoSemanticFunctionKind, GoSemanticMethodSetFact, GoSemanticMethodSetId,
    GoSemanticPackageErrorFact, GoSemanticPackageErrorId, GoSemanticPackageFact,
    GoSemanticPackageId,
};
use crate::go::semantic::protocol::{GoSemanticOutput, GoSemanticRawFrame, GoSemanticSpan};
use crate::go::semantic::store::GoSemanticFactsOutput;
use crate::go::semantic::validate::validate_relative_path;

struct LoweredLocation {
    relative_file: Option<String>,
    file: Option<FileId>,
    span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GoSemanticLowerError {
    InvalidPath(String),
}

impl std::fmt::Display for GoSemanticLowerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidPath(reason) => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for GoSemanticLowerError {}

pub(crate) fn lower_go_semantic(
    db: &AnalysisDb,
    output: &GoSemanticOutput,
) -> Result<GoSemanticFactsOutput, GoSemanticLowerError> {
    let files = db
        .files()
        .iter()
        .filter(|file| file.language == Language::Go)
        .map(|file| (file.relative_path.as_str(), file.id))
        .collect::<BTreeMap<_, _>>();
    let mut lowered = GoSemanticFactsOutput::default();

    for row in &output.rows {
        match row.kind.as_str() {
            "package" => lowered.packages.push(lower_package(row, &files)?),
            "function" | "method" | "init_function" => {
                lowered.functions.push(lower_function(row, &files)?);
            }
            "callsite" => lowered.callsites.push(lower_callsite(row, &files)?),
            "method_set" => lowered.method_sets.push(lower_method_set(row)),
            "package_error" => lowered.package_errors.push(lower_package_error(row)),
            "receiver_type" | "unsupported" | "type_fact" => {}
            _ => {}
        }
    }

    Ok(lowered.normalized())
}

fn lower_package(
    row: &GoSemanticRawFrame,
    files: &BTreeMap<&str, FileId>,
) -> Result<GoSemanticPackageFact, GoSemanticLowerError> {
    for file in &row.files {
        validate_relative_path(file)
            .map_err(|error| GoSemanticLowerError::InvalidPath(error.to_string()))?;
        if !files.contains_key(file.as_str()) {
            return Err(GoSemanticLowerError::InvalidPath(format!(
                "Go semantic sidecar file path `{file}` was not discovered as an in-repository Go file"
            )));
        }
    }
    Ok(GoSemanticPackageFact {
        id: GoSemanticPackageId(0),
        stable_key: row_stable_key(row, "package"),
        package_id: row.package_id.clone(),
        package_path: row.package_path.clone(),
        package_name: row.package_name.clone(),
        module_path: row.module_path.clone(),
        files: row.files.clone(),
    })
}

fn lower_function(
    row: &GoSemanticRawFrame,
    files: &BTreeMap<&str, FileId>,
) -> Result<GoSemanticFunctionFact, GoSemanticLowerError> {
    let location = lower_optional_file_span(row, files)?;
    Ok(GoSemanticFunctionFact {
        id: GoSemanticFunctionId(0),
        stable_key: row_stable_key(row, row.kind.as_str()),
        package_id: row.package_id.clone(),
        package_path: row.package_path.clone(),
        name: row.name.clone(),
        qualified: row.qualified.clone(),
        signature: row.signature.clone(),
        kind: match row.kind.as_str() {
            "method" => GoSemanticFunctionKind::Method,
            "init_function" => GoSemanticFunctionKind::Init,
            _ => GoSemanticFunctionKind::Function,
        },
        receiver: non_empty(row.receiver.as_str()),
        relative_file: location.relative_file,
        file: location.file,
        span: location.span,
    })
}

fn lower_callsite(
    row: &GoSemanticRawFrame,
    files: &BTreeMap<&str, FileId>,
) -> Result<GoSemanticCallsiteFact, GoSemanticLowerError> {
    let location = lower_optional_file_span(row, files)?;
    Ok(GoSemanticCallsiteFact {
        id: GoSemanticCallsiteId(0),
        stable_key: row_stable_key(row, "callsite"),
        package_id: row.package_id.clone(),
        package_path: row.package_path.clone(),
        caller: row.caller.clone(),
        static_callee: non_empty(row.static_callee.as_str()),
        status: match row.status.as_str() {
            "resolved_static" => GoSemanticCallStatus::ResolvedStatic,
            "unsupported" => GoSemanticCallStatus::Unsupported,
            _ => GoSemanticCallStatus::UnresolvedDynamic,
        },
        reason: non_empty(row.reason.as_str()),
        relative_file: location.relative_file,
        file: location.file,
        span: location.span,
    })
}

fn lower_method_set(row: &GoSemanticRawFrame) -> GoSemanticMethodSetFact {
    GoSemanticMethodSetFact {
        id: GoSemanticMethodSetId(0),
        stable_key: row_stable_key(row, "method_set"),
        package_id: row.package_id.clone(),
        package_path: row.package_path.clone(),
        type_name: row.type_name.clone(),
        methods: row.methods.clone(),
    }
}

fn lower_package_error(row: &GoSemanticRawFrame) -> GoSemanticPackageErrorFact {
    GoSemanticPackageErrorFact {
        id: GoSemanticPackageErrorId(0),
        stable_key: row_stable_key(row, "package_error"),
        package_id: row.package_id.clone(),
        package_path: row.package_path.clone(),
        message: row.message.clone(),
    }
}

fn lower_optional_file_span(
    row: &GoSemanticRawFrame,
    files: &BTreeMap<&str, FileId>,
) -> Result<LoweredLocation, GoSemanticLowerError> {
    if row.file.is_empty() {
        return Ok(LoweredLocation {
            relative_file: None,
            file: None,
            span: None,
        });
    }
    validate_relative_path(&row.file)
        .map_err(|error| GoSemanticLowerError::InvalidPath(error.to_string()))?;
    let Some(&file) = files.get(row.file.as_str()) else {
        return Err(GoSemanticLowerError::InvalidPath(format!(
            "Go semantic sidecar file path `{}` was not discovered as an in-repository Go file",
            row.file
        )));
    };
    let span = row.span.as_ref().map(|span| to_span(file, span));
    Ok(LoweredLocation {
        relative_file: Some(row.file.clone()),
        file: Some(file),
        span,
    })
}

fn to_span(file: FileId, span: &GoSemanticSpan) -> Span {
    Span {
        file,
        start_byte: span.start_byte,
        end_byte: span.end_byte,
        start_line: span.start_line,
        start_col: span.start_col,
        end_line: span.end_line,
        end_col: span.end_col,
    }
}

fn row_stable_key(row: &GoSemanticRawFrame, kind: &str) -> String {
    if !row.stable_key.is_empty() {
        return row.stable_key.clone();
    }
    semantic_stable_key(
        FactFamily::SemanticImport,
        &[
            ("go_kind", kind.to_string()),
            ("package", row.package_path.clone()),
            ("name", row.qualified.clone()),
            ("file", row.file.clone()),
            ("caller", row.caller.clone()),
            ("message", row.message.clone()),
        ],
    )
    .into_string()
}

fn non_empty(value: &str) -> Option<String> {
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::go::semantic::protocol::{GO_SEMANTIC_SCHEMA, decode_ndjson_str};
    use std::path::PathBuf;

    #[test]
    fn lower_accepts_in_repository_path() {
        let db = db_with_go_file("main.go");
        let output = decode_ndjson_str(
            r#"{"schema":"polint-go-semantic-1","kind":"session_begin"}
{"schema":"polint-go-semantic-1","kind":"function","package_id":"example.com/p","package_path":"example.com/p","name":"F","qualified":"example.com/p.F","stable_key":"fn","file":"main.go","span":{"start_byte":1,"end_byte":2,"start_line":1,"start_column":1,"end_line":1,"end_column":2}}
{"schema":"polint-go-semantic-1","kind":"session_end"}
"#,
        )
        .expect("valid protocol");
        let lowered = lower_go_semantic(&db, &output).expect("lowered");
        assert_eq!(lowered.functions[0].file, Some(FileId(0)));
    }

    #[test]
    fn lower_rejects_absolute_repo_escaping_path() {
        let db = db_with_go_file("main.go");
        let outside = if cfg!(windows) {
            r"C:\tmp\outside.go"
        } else {
            "/tmp/outside.go"
        };
        let outside_json = serde_json::to_string(outside).expect("path serializes as JSON");
        let output = decode_ndjson_str(&format!(
            r#"{{"schema":"{GO_SEMANTIC_SCHEMA}","kind":"session_begin"}}
{{"schema":"{GO_SEMANTIC_SCHEMA}","kind":"function","package_id":"example.com/p","package_path":"example.com/p","name":"F","qualified":"example.com/p.F","stable_key":"fn","file":{outside_json}}}
{{"schema":"{GO_SEMANTIC_SCHEMA}","kind":"session_end"}}
"#,
        ))
        .expect("valid protocol");
        let err = lower_go_semantic(&db, &output).unwrap_err();
        assert!(err.to_string().contains("escapes repository"));
    }

    #[test]
    fn lower_rejects_undiscovered_in_repository_path() {
        let db = db_with_go_file("main.go");
        let output = decode_ndjson_str(&format!(
            r#"{{"schema":"{GO_SEMANTIC_SCHEMA}","kind":"session_begin"}}
{{"schema":"{GO_SEMANTIC_SCHEMA}","kind":"function","package_id":"example.com/p","package_path":"example.com/p","name":"F","qualified":"example.com/p.F","stable_key":"fn","file":"deleted.go"}}
{{"schema":"{GO_SEMANTIC_SCHEMA}","kind":"session_end"}}
"#,
        ))
        .expect("valid protocol");
        let err = lower_go_semantic(&db, &output).unwrap_err();
        assert!(err.to_string().contains("was not discovered"));
    }

    #[test]
    fn lower_rejects_package_files_not_discovered_by_polint() {
        let db = db_with_go_file("main.go");
        let output = decode_ndjson_str(&format!(
            r#"{{"schema":"{GO_SEMANTIC_SCHEMA}","kind":"session_begin"}}
{{"schema":"{GO_SEMANTIC_SCHEMA}","kind":"package","package_id":"example.com/p","package_path":"example.com/p","files":["main.go","other.go"]}}
{{"schema":"{GO_SEMANTIC_SCHEMA}","kind":"session_end"}}
"#,
        ))
        .expect("valid protocol");
        let err = lower_go_semantic(&db, &output).unwrap_err();
        assert!(err.to_string().contains("other.go"));
    }

    #[test]
    fn lower_preserves_package_load_errors() {
        let db = db_with_go_file("main.go");
        let output = decode_ndjson_str(
            r#"{"schema":"polint-go-semantic-1","kind":"session_begin"}
{"schema":"polint-go-semantic-1","kind":"package_error","package_id":"example.com/p","package_path":"example.com/p","message":"load failed"}
{"schema":"polint-go-semantic-1","kind":"session_end"}
"#,
        )
        .expect("valid protocol");
        let lowered = lower_go_semantic(&db, &output).expect("lowered");
        assert_eq!(lowered.package_errors[0].message, "load failed");
    }

    #[test]
    fn lower_package_error_fallback_stable_key_includes_message() {
        let db = db_with_go_file("main.go");
        let output = decode_ndjson_str(
            r#"{"schema":"polint-go-semantic-1","kind":"session_begin"}
{"schema":"polint-go-semantic-1","kind":"package_error","package_id":"example.com/p","package_path":"example.com/p","message":"first"}
{"schema":"polint-go-semantic-1","kind":"package_error","package_id":"example.com/p","package_path":"example.com/p","message":"second"}
{"schema":"polint-go-semantic-1","kind":"session_end"}
"#,
        )
        .expect("valid protocol");
        let lowered = lower_go_semantic(&db, &output).expect("lowered");
        assert_ne!(
            lowered.package_errors[0].stable_key,
            lowered.package_errors[1].stable_key
        );
    }

    fn db_with_go_file(path: &str) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from(path),
            path.to_string(),
            "package main\n".to_string(),
        );
        db
    }
}
