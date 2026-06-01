use serde::Deserialize;
use std::collections::BTreeSet;

pub(crate) const GO_SEMANTIC_SCHEMA: &str = "polint-go-semantic-1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GoSemanticProtocolError {
    InvalidJson(String),
    UnsupportedSchema(String),
    UnknownKind(String),
    RowBeforeBegin(String),
    DuplicateBegin,
    DuplicateEnd,
    MissingBegin,
    MissingEnd,
}

impl std::fmt::Display for GoSemanticProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidJson(error) => write!(f, "invalid Go semantic NDJSON row: {error}"),
            Self::UnsupportedSchema(schema) => write!(
                f,
                "unsupported Go semantic schema `{schema}`; expected `{GO_SEMANTIC_SCHEMA}`"
            ),
            Self::UnknownKind(kind) => write!(f, "unknown Go semantic frame kind `{kind}`"),
            Self::RowBeforeBegin(kind) => {
                write!(
                    f,
                    "Go semantic frame `{kind}` appeared before session_begin"
                )
            }
            Self::DuplicateBegin => write!(f, "duplicate Go semantic session_begin frame"),
            Self::DuplicateEnd => write!(f, "duplicate Go semantic session_end frame"),
            Self::MissingBegin => write!(f, "missing Go semantic session_begin frame"),
            Self::MissingEnd => write!(f, "missing Go semantic session_end frame"),
        }
    }
}

impl std::error::Error for GoSemanticProtocolError {}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GoSemanticRawFrame {
    pub(crate) schema: String,
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) package_id: String,
    #[serde(default)]
    pub(crate) package_path: String,
    #[serde(default)]
    pub(crate) package_name: String,
    #[serde(default)]
    pub(crate) module_path: String,
    #[serde(default)]
    pub(crate) files: Vec<String>,
    #[serde(default)]
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) qualified: String,
    #[serde(default)]
    pub(crate) signature: String,
    #[serde(default)]
    pub(crate) stable_key: String,
    #[serde(default)]
    pub(crate) receiver: String,
    #[serde(default)]
    pub(crate) method: String,
    #[serde(default, rename = "type")]
    pub(crate) type_name: String,
    #[serde(default)]
    pub(crate) methods: Vec<String>,
    #[serde(default)]
    pub(crate) file: String,
    #[serde(default)]
    pub(crate) span: Option<GoSemanticSpan>,
    #[serde(default)]
    pub(crate) caller: String,
    #[serde(default)]
    pub(crate) static_callee: String,
    #[serde(default)]
    pub(crate) message: String,
    #[serde(default)]
    pub(crate) status: String,
    #[serde(default)]
    pub(crate) reason: String,
    #[serde(default)]
    pub(crate) go_version: String,
    #[serde(default)]
    pub(crate) x_tools_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct GoSemanticSpan {
    pub(crate) start_byte: u32,
    pub(crate) end_byte: u32,
    pub(crate) start_line: u32,
    #[serde(rename = "start_column")]
    pub(crate) start_col: u32,
    pub(crate) end_line: u32,
    #[serde(rename = "end_column")]
    pub(crate) end_col: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct GoSemanticOutput {
    pub(crate) go_version: String,
    pub(crate) x_tools_version: String,
    pub(crate) rows: Vec<GoSemanticRawFrame>,
}

#[derive(Debug, Clone)]
pub(crate) enum GoSemanticFrame {
    SessionBegin(GoSemanticRawFrame),
    Row(GoSemanticRawFrame),
    SessionEnd,
}

pub(crate) fn decode_ndjson(bytes: &[u8]) -> Result<GoSemanticOutput, GoSemanticProtocolError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|error| GoSemanticProtocolError::InvalidJson(error.to_string()))?;
    decode_ndjson_str(text)
}

pub(crate) fn decode_ndjson_str(text: &str) -> Result<GoSemanticOutput, GoSemanticProtocolError> {
    let allowed = allowed_kinds();
    let mut saw_begin = false;
    let mut saw_end = false;
    let mut go_version = String::new();
    let mut x_tools_version = String::new();
    let mut rows = Vec::new();

    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let frame: GoSemanticRawFrame = serde_json::from_str(line)
            .map_err(|error| GoSemanticProtocolError::InvalidJson(error.to_string()))?;
        if frame.schema != GO_SEMANTIC_SCHEMA {
            return Err(GoSemanticProtocolError::UnsupportedSchema(frame.schema));
        }
        if !allowed.contains(frame.kind.as_str()) {
            return Err(GoSemanticProtocolError::UnknownKind(frame.kind));
        }
        match classify_frame(frame)? {
            GoSemanticFrame::SessionBegin(frame) => {
                if saw_begin {
                    return Err(GoSemanticProtocolError::DuplicateBegin);
                }
                if saw_end {
                    return Err(GoSemanticProtocolError::DuplicateEnd);
                }
                saw_begin = true;
                go_version = frame.go_version.clone();
                x_tools_version = frame.x_tools_version.clone();
            }
            GoSemanticFrame::SessionEnd => {
                if !saw_begin {
                    return Err(GoSemanticProtocolError::MissingBegin);
                }
                if saw_end {
                    return Err(GoSemanticProtocolError::DuplicateEnd);
                }
                saw_end = true;
            }
            GoSemanticFrame::Row(frame) if !saw_begin => {
                return Err(GoSemanticProtocolError::RowBeforeBegin(frame.kind));
            }
            GoSemanticFrame::Row(frame) => rows.push(frame),
        }
    }

    if !saw_begin {
        return Err(GoSemanticProtocolError::MissingBegin);
    }
    if !saw_end {
        return Err(GoSemanticProtocolError::MissingEnd);
    }

    Ok(GoSemanticOutput {
        go_version,
        x_tools_version,
        rows,
    })
}

fn classify_frame(frame: GoSemanticRawFrame) -> Result<GoSemanticFrame, GoSemanticProtocolError> {
    match frame.kind.as_str() {
        "session_begin" => Ok(GoSemanticFrame::SessionBegin(frame)),
        "session_end" => Ok(GoSemanticFrame::SessionEnd),
        _ => Ok(GoSemanticFrame::Row(frame)),
    }
}

fn allowed_kinds() -> BTreeSet<&'static str> {
    [
        "session_begin",
        "session_end",
        "package",
        "function",
        "method",
        "receiver_type",
        "init_function",
        "method_set",
        "callsite",
        "type_fact",
        "package_error",
        "unsupported",
    ]
    .into_iter()
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn framed(row: &str) -> String {
        format!(
            "{{\"schema\":\"polint-go-semantic-1\",\"kind\":\"session_begin\",\"go_version\":\"go1.25.0\",\"x_tools_version\":\"v0.45.0\"}}\n{row}\n{{\"schema\":\"polint-go-semantic-1\",\"kind\":\"session_end\"}}\n"
        )
    }

    #[test]
    fn decode_ndjson_accepts_framed_rows() {
        let output = decode_ndjson_str(&framed(
            "{\"schema\":\"polint-go-semantic-1\",\"kind\":\"package\",\"package_id\":\"p\"}",
        ))
        .expect("framed output decodes");
        assert_eq!(output.rows.len(), 1);
    }

    #[test]
    fn decode_ndjson_rejects_invalid_json() {
        let err = decode_ndjson_str("{").unwrap_err();
        assert!(err.to_string().contains("invalid Go semantic NDJSON"));
    }

    #[test]
    fn decode_ndjson_rejects_unsupported_schema() {
        let err =
            decode_ndjson_str("{\"schema\":\"other\",\"kind\":\"session_begin\"}").unwrap_err();
        assert!(err.to_string().contains("unsupported Go semantic schema"));
    }

    #[test]
    fn decode_ndjson_rejects_unknown_frame_kind() {
        let err = decode_ndjson_str(&framed(
            "{\"schema\":\"polint-go-semantic-1\",\"kind\":\"mystery\"}",
        ))
        .unwrap_err();
        assert!(err.to_string().contains("unknown Go semantic frame kind"));
    }

    #[test]
    fn decode_ndjson_rejects_missing_terminator() {
        let err =
            decode_ndjson_str("{\"schema\":\"polint-go-semantic-1\",\"kind\":\"session_begin\"}\n")
                .unwrap_err();
        assert_eq!(err, GoSemanticProtocolError::MissingEnd);
    }
}
