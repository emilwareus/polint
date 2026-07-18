use std::mem::size_of;
use std::time::Instant;

use serde::Deserialize;

pub(crate) const GO_SEMANTIC_SCHEMA: &str = "polint-go-semantic-2";
pub(crate) const GO_SEMANTIC_MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
pub(crate) const GO_SEMANTIC_MAX_FRAME_BYTES: usize = 1024 * 1024;
const GO_SEMANTIC_MAX_FRAMES: usize = 250_002;
const GO_SEMANTIC_MAX_ROWS: usize = 250_000;
const GO_SEMANTIC_MAX_FIELD_BYTES: usize = 256 * 1024;
const GO_SEMANTIC_MAX_ARRAY_ITEMS: usize = 65_536;
const GO_SEMANTIC_MAX_DECODED_ALLOCATION_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GoSemanticProtocolError {
    OutputTooLarge {
        max_bytes: usize,
    },
    FrameTooLarge {
        line: usize,
        max_bytes: usize,
    },
    TooManyFrames {
        max_frames: usize,
    },
    TooManyRows {
        max_rows: usize,
    },
    FieldTooLarge {
        field: &'static str,
        max_bytes: usize,
    },
    ArrayTooLarge {
        field: &'static str,
        max_items: usize,
    },
    AllocationLimitExceeded {
        max_bytes: usize,
    },
    AllocationFailed {
        resource: &'static str,
    },
    DeadlineExceeded,
    InvalidJson(String),
    UnsupportedSchema(String),
    UnknownKind(String),
    RowBeforeBegin(String),
    RowAfterEnd(String),
    DuplicateBegin,
    DuplicateEnd,
    MissingBegin,
    MissingEnd,
}

impl std::fmt::Display for GoSemanticProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OutputTooLarge { max_bytes } => {
                write!(f, "Go semantic NDJSON output exceeds {max_bytes} bytes")
            }
            Self::FrameTooLarge { line, max_bytes } => write!(
                f,
                "Go semantic NDJSON frame on line {line} exceeds {max_bytes} bytes"
            ),
            Self::TooManyFrames { max_frames } => {
                write!(f, "Go semantic NDJSON output exceeds {max_frames} frames")
            }
            Self::TooManyRows { max_rows } => {
                write!(f, "Go semantic NDJSON output exceeds {max_rows} rows")
            }
            Self::FieldTooLarge { field, max_bytes } => {
                write!(f, "Go semantic field `{field}` exceeds {max_bytes} bytes")
            }
            Self::ArrayTooLarge { field, max_items } => {
                write!(f, "Go semantic array `{field}` exceeds {max_items} items")
            }
            Self::AllocationLimitExceeded { max_bytes } => write!(
                f,
                "Go semantic decoded data exceeds the {max_bytes}-byte allocation budget"
            ),
            Self::AllocationFailed { resource } => {
                write!(f, "could not allocate bounded Go semantic {resource}")
            }
            Self::DeadlineExceeded => {
                write!(
                    f,
                    "Go semantic NDJSON decoding exceeded its operation deadline"
                )
            }
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
            Self::RowAfterEnd(kind) => {
                write!(f, "Go semantic frame `{kind}` appeared after session_end")
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
    pub(crate) callee: String,
    #[serde(default)]
    pub(crate) edge_kind: String,
    #[serde(default)]
    pub(crate) static_callee: String,
    #[serde(default)]
    pub(crate) function: String,
    #[serde(default)]
    pub(crate) interface_type: String,
    #[serde(default)]
    pub(crate) callsite_stable_key: String,
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

#[derive(Debug, Clone, Copy)]
struct GoSemanticProtocolLimits {
    output_bytes: usize,
    frame_bytes: usize,
    frames: usize,
    rows: usize,
    field_bytes: usize,
    array_items: usize,
    decoded_allocation_bytes: usize,
}

impl Default for GoSemanticProtocolLimits {
    fn default() -> Self {
        Self {
            output_bytes: GO_SEMANTIC_MAX_OUTPUT_BYTES,
            frame_bytes: GO_SEMANTIC_MAX_FRAME_BYTES,
            frames: GO_SEMANTIC_MAX_FRAMES,
            rows: GO_SEMANTIC_MAX_ROWS,
            field_bytes: GO_SEMANTIC_MAX_FIELD_BYTES,
            array_items: GO_SEMANTIC_MAX_ARRAY_ITEMS,
            decoded_allocation_bytes: GO_SEMANTIC_MAX_DECODED_ALLOCATION_BYTES,
        }
    }
}

#[derive(Debug)]
struct DecodedAllocationBudget {
    used: usize,
    max: usize,
}

impl DecodedAllocationBudget {
    fn new(max: usize) -> Self {
        Self { used: 0, max }
    }

    fn reserve(&mut self, bytes: usize) -> Result<(), GoSemanticProtocolError> {
        let Some(used) = self.used.checked_add(bytes) else {
            return Err(GoSemanticProtocolError::AllocationLimitExceeded {
                max_bytes: self.max,
            });
        };
        if used > self.max {
            return Err(GoSemanticProtocolError::AllocationLimitExceeded {
                max_bytes: self.max,
            });
        }
        self.used = used;
        Ok(())
    }

    fn remaining(&self) -> usize {
        self.max.saturating_sub(self.used)
    }
}

pub(crate) fn decode_ndjson(bytes: &[u8]) -> Result<GoSemanticOutput, GoSemanticProtocolError> {
    decode_ndjson_before(bytes, None)
}

pub(crate) fn decode_ndjson_until(
    bytes: &[u8],
    deadline: Instant,
) -> Result<GoSemanticOutput, GoSemanticProtocolError> {
    decode_ndjson_before(bytes, Some(deadline))
}

fn decode_ndjson_before(
    bytes: &[u8],
    deadline: Option<Instant>,
) -> Result<GoSemanticOutput, GoSemanticProtocolError> {
    check_decode_deadline(deadline)?;
    if bytes.len() > GO_SEMANTIC_MAX_OUTPUT_BYTES {
        return Err(GoSemanticProtocolError::OutputTooLarge {
            max_bytes: GO_SEMANTIC_MAX_OUTPUT_BYTES,
        });
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|error| GoSemanticProtocolError::InvalidJson(error.to_string()))?;
    check_decode_deadline(deadline)?;
    decode_ndjson_str_with_limits(text, GoSemanticProtocolLimits::default(), deadline)
}

pub(crate) fn decode_ndjson_str(text: &str) -> Result<GoSemanticOutput, GoSemanticProtocolError> {
    decode_ndjson_str_with_limits(text, GoSemanticProtocolLimits::default(), None)
}

fn decode_ndjson_str_with_limits(
    text: &str,
    limits: GoSemanticProtocolLimits,
    deadline: Option<Instant>,
) -> Result<GoSemanticOutput, GoSemanticProtocolError> {
    check_decode_deadline(deadline)?;
    if text.len() > limits.output_bytes {
        return Err(GoSemanticProtocolError::OutputTooLarge {
            max_bytes: limits.output_bytes,
        });
    }

    let mut saw_begin = false;
    let mut saw_end = false;
    let mut go_version = String::new();
    let mut x_tools_version = String::new();
    let mut rows = Vec::new();
    let mut frame_count = 0_usize;
    let mut row_count = 0_usize;
    let mut allocation_budget = DecodedAllocationBudget::new(limits.decoded_allocation_bytes);

    for (line_index, line) in text.lines().enumerate() {
        check_decode_deadline(deadline)?;
        if line.len() > limits.frame_bytes {
            return Err(GoSemanticProtocolError::FrameTooLarge {
                line: line_index + 1,
                max_bytes: limits.frame_bytes,
            });
        }
        if line.trim().is_empty() {
            continue;
        }

        frame_count = frame_count.saturating_add(1);
        if frame_count > limits.frames {
            return Err(GoSemanticProtocolError::TooManyFrames {
                max_frames: limits.frames,
            });
        }

        let frame: GoSemanticRawFrame = serde_json::from_str(line)
            .map_err(|error| GoSemanticProtocolError::InvalidJson(error.to_string()))?;
        check_decode_deadline(deadline)?;
        validate_frame_limits(&frame, limits, &mut allocation_budget)?;
        if frame.schema != GO_SEMANTIC_SCHEMA {
            return Err(GoSemanticProtocolError::UnsupportedSchema(frame.schema));
        }
        if !is_allowed_kind(&frame.kind) {
            return Err(GoSemanticProtocolError::UnknownKind(frame.kind));
        }
        match classify_frame(frame) {
            GoSemanticFrame::SessionBegin(frame) => {
                if saw_begin {
                    return Err(GoSemanticProtocolError::DuplicateBegin);
                }
                if saw_end {
                    return Err(GoSemanticProtocolError::DuplicateEnd);
                }
                saw_begin = true;
                go_version = frame.go_version;
                x_tools_version = frame.x_tools_version;
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
            GoSemanticFrame::Row(frame) if saw_end => {
                return Err(GoSemanticProtocolError::RowAfterEnd(frame.kind));
            }
            GoSemanticFrame::Row(frame) => {
                row_count = row_count.saturating_add(1);
                if row_count > limits.rows {
                    return Err(GoSemanticProtocolError::TooManyRows {
                        max_rows: limits.rows,
                    });
                }
                reserve_row_slot(&mut rows, &mut allocation_budget)?;
                rows.push(frame);
            }
        }
    }

    if !saw_begin {
        return Err(GoSemanticProtocolError::MissingBegin);
    }
    if !saw_end {
        return Err(GoSemanticProtocolError::MissingEnd);
    }
    check_decode_deadline(deadline)?;

    Ok(GoSemanticOutput {
        go_version,
        x_tools_version,
        rows,
    })
}

fn check_decode_deadline(deadline: Option<Instant>) -> Result<(), GoSemanticProtocolError> {
    if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
        Err(GoSemanticProtocolError::DeadlineExceeded)
    } else {
        Ok(())
    }
}

fn reserve_row_slot(
    rows: &mut Vec<GoSemanticRawFrame>,
    allocation_budget: &mut DecodedAllocationBudget,
) -> Result<(), GoSemanticProtocolError> {
    if rows.len() < rows.capacity() {
        return Ok(());
    }

    let slot_bytes = size_of::<GoSemanticRawFrame>();
    let affordable_slots = allocation_budget.remaining() / slot_bytes;
    if affordable_slots == 0 {
        return Err(GoSemanticProtocolError::AllocationLimitExceeded {
            max_bytes: allocation_budget.max,
        });
    }
    let requested_slots = if rows.capacity() == 0 {
        64
    } else {
        rows.capacity()
    }
    .min(affordable_slots);
    allocation_budget.reserve(requested_slots.saturating_mul(slot_bytes))?;

    let old_capacity = rows.capacity();
    rows.try_reserve_exact(requested_slots).map_err(|_| {
        GoSemanticProtocolError::AllocationFailed {
            resource: "row storage",
        }
    })?;
    let actual_slots = rows.capacity().saturating_sub(old_capacity);
    let extra_slots = actual_slots.saturating_sub(requested_slots);
    allocation_budget.reserve(extra_slots.saturating_mul(size_of::<GoSemanticRawFrame>()))
}

fn validate_frame_limits(
    frame: &GoSemanticRawFrame,
    limits: GoSemanticProtocolLimits,
    allocation_budget: &mut DecodedAllocationBudget,
) -> Result<(), GoSemanticProtocolError> {
    for (field, value) in frame.string_fields() {
        if value.len() > limits.field_bytes {
            return Err(GoSemanticProtocolError::FieldTooLarge {
                field,
                max_bytes: limits.field_bytes,
            });
        }
        allocation_budget.reserve(value.capacity())?;
    }
    allocation_budget.reserve(frame.files.capacity().saturating_mul(size_of::<String>()))?;
    validate_string_array("files", &frame.files, limits, allocation_budget)?;
    allocation_budget.reserve(frame.methods.capacity().saturating_mul(size_of::<String>()))?;
    validate_string_array("methods", &frame.methods, limits, allocation_budget)
}

fn validate_string_array(
    field: &'static str,
    values: &[String],
    limits: GoSemanticProtocolLimits,
    allocation_budget: &mut DecodedAllocationBudget,
) -> Result<(), GoSemanticProtocolError> {
    if values.len() > limits.array_items {
        return Err(GoSemanticProtocolError::ArrayTooLarge {
            field,
            max_items: limits.array_items,
        });
    }
    for value in values {
        if value.len() > limits.field_bytes {
            return Err(GoSemanticProtocolError::FieldTooLarge {
                field,
                max_bytes: limits.field_bytes,
            });
        }
        allocation_budget.reserve(value.capacity())?;
    }
    Ok(())
}

impl GoSemanticRawFrame {
    fn string_fields(&self) -> [(&'static str, &String); 26] {
        [
            ("schema", &self.schema),
            ("kind", &self.kind),
            ("package_id", &self.package_id),
            ("package_path", &self.package_path),
            ("package_name", &self.package_name),
            ("module_path", &self.module_path),
            ("name", &self.name),
            ("qualified", &self.qualified),
            ("signature", &self.signature),
            ("stable_key", &self.stable_key),
            ("receiver", &self.receiver),
            ("method", &self.method),
            ("type", &self.type_name),
            ("file", &self.file),
            ("caller", &self.caller),
            ("callee", &self.callee),
            ("edge_kind", &self.edge_kind),
            ("static_callee", &self.static_callee),
            ("function", &self.function),
            ("interface_type", &self.interface_type),
            ("callsite_stable_key", &self.callsite_stable_key),
            ("message", &self.message),
            ("status", &self.status),
            ("reason", &self.reason),
            ("go_version", &self.go_version),
            ("x_tools_version", &self.x_tools_version),
        ]
    }
}

fn classify_frame(frame: GoSemanticRawFrame) -> GoSemanticFrame {
    match frame.kind.as_str() {
        "session_begin" => GoSemanticFrame::SessionBegin(frame),
        "session_end" => GoSemanticFrame::SessionEnd,
        _ => GoSemanticFrame::Row(frame),
    }
}

fn is_allowed_kind(kind: &str) -> bool {
    matches!(
        kind,
        "session_begin"
            | "session_end"
            | "package"
            | "function"
            | "method"
            | "receiver_type"
            | "init_function"
            | "method_set"
            | "callsite"
            | "type_fact"
            | "package_error"
            | "unsupported"
            | "address_taken"
            | "instantiated_type"
            | "dynamic_dispatch"
            | "rta_edge"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn framed(row: &str) -> String {
        format!(
            "{{\"schema\":\"polint-go-semantic-2\",\"kind\":\"session_begin\",\"go_version\":\"go1.25.0\",\"x_tools_version\":\"v0.45.0\"}}\n{row}\n{{\"schema\":\"polint-go-semantic-2\",\"kind\":\"session_end\"}}\n"
        )
    }

    #[test]
    fn decode_ndjson_accepts_framed_rows() {
        let output = decode_ndjson_str(&framed(
            "{\"schema\":\"polint-go-semantic-2\",\"kind\":\"package\",\"package_id\":\"p\"}",
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
            "{\"schema\":\"polint-go-semantic-2\",\"kind\":\"mystery\"}",
        ))
        .unwrap_err();
        assert!(err.to_string().contains("unknown Go semantic frame kind"));
    }

    #[test]
    fn decode_ndjson_rejects_missing_terminator() {
        let err =
            decode_ndjson_str("{\"schema\":\"polint-go-semantic-2\",\"kind\":\"session_begin\"}\n")
                .unwrap_err();
        assert_eq!(err, GoSemanticProtocolError::MissingEnd);
    }

    #[test]
    fn decode_ndjson_honors_an_expired_absolute_deadline() {
        let error = decode_ndjson_until(
            framed("{\"schema\":\"polint-go-semantic-2\",\"kind\":\"package\"}").as_bytes(),
            Instant::now(),
        )
        .expect_err("an expired operation must not begin protocol decoding");

        assert_eq!(error, GoSemanticProtocolError::DeadlineExceeded);
    }

    #[test]
    fn decode_ndjson_rejects_rows_after_session_end() {
        let err = decode_ndjson_str(
            "{\"schema\":\"polint-go-semantic-2\",\"kind\":\"session_begin\"}\n\
             {\"schema\":\"polint-go-semantic-2\",\"kind\":\"session_end\"}\n\
             {\"schema\":\"polint-go-semantic-2\",\"kind\":\"package\",\"package_id\":\"p\"}\n",
        )
        .unwrap_err();
        assert_eq!(
            err,
            GoSemanticProtocolError::RowAfterEnd("package".to_string())
        );
    }

    #[test]
    fn decode_ndjson_rejects_total_bytes_before_json_decode() {
        let limits = GoSemanticProtocolLimits {
            output_bytes: 3,
            ..GoSemanticProtocolLimits::default()
        };
        let err = decode_ndjson_str_with_limits("not-json", limits, None).unwrap_err();
        assert_eq!(
            err,
            GoSemanticProtocolError::OutputTooLarge { max_bytes: 3 }
        );
    }

    #[test]
    fn decode_ndjson_rejects_frame_bytes_before_json_decode() {
        let limits = GoSemanticProtocolLimits {
            output_bytes: 64,
            frame_bytes: 3,
            ..GoSemanticProtocolLimits::default()
        };
        let err = decode_ndjson_str_with_limits("\n{{{{", limits, None).unwrap_err();
        assert_eq!(
            err,
            GoSemanticProtocolError::FrameTooLarge {
                line: 2,
                max_bytes: 3,
            }
        );
    }

    #[test]
    fn decode_ndjson_rejects_excess_frames() {
        let limits = GoSemanticProtocolLimits {
            frames: 1,
            ..GoSemanticProtocolLimits::default()
        };
        let input = concat!(
            "{\"schema\":\"polint-go-semantic-2\",\"kind\":\"session_begin\"}\n",
            "{\"schema\":\"polint-go-semantic-2\",\"kind\":\"session_end\"}\n",
        );
        let err = decode_ndjson_str_with_limits(input, limits, None).unwrap_err();
        assert_eq!(
            err,
            GoSemanticProtocolError::TooManyFrames { max_frames: 1 }
        );
    }

    #[test]
    fn decode_ndjson_rejects_excess_rows() {
        let limits = GoSemanticProtocolLimits {
            rows: 1,
            ..GoSemanticProtocolLimits::default()
        };
        let input = concat!(
            "{\"schema\":\"polint-go-semantic-2\",\"kind\":\"session_begin\"}\n",
            "{\"schema\":\"polint-go-semantic-2\",\"kind\":\"package\",\"package_id\":\"a\"}\n",
            "{\"schema\":\"polint-go-semantic-2\",\"kind\":\"package\",\"package_id\":\"b\"}\n",
            "{\"schema\":\"polint-go-semantic-2\",\"kind\":\"session_end\"}\n",
        );
        let err = decode_ndjson_str_with_limits(input, limits, None).unwrap_err();
        assert_eq!(err, GoSemanticProtocolError::TooManyRows { max_rows: 1 });
    }

    #[test]
    fn decode_ndjson_rejects_oversized_string_field() {
        let package_id = "x".repeat(GO_SEMANTIC_MAX_FIELD_BYTES + 1);
        let row = format!(
            "{{\"schema\":\"{GO_SEMANTIC_SCHEMA}\",\"kind\":\"package\",\"package_id\":\"{package_id}\"}}"
        );
        let err = decode_ndjson_str(&framed(&row)).unwrap_err();
        assert_eq!(
            err,
            GoSemanticProtocolError::FieldTooLarge {
                field: "package_id",
                max_bytes: GO_SEMANTIC_MAX_FIELD_BYTES,
            }
        );
    }

    #[test]
    fn decode_ndjson_rejects_oversized_array() {
        let files = std::iter::repeat_n("\"\"", GO_SEMANTIC_MAX_ARRAY_ITEMS + 1)
            .collect::<Vec<_>>()
            .join(",");
        let row = format!(
            "{{\"schema\":\"{GO_SEMANTIC_SCHEMA}\",\"kind\":\"package\",\"files\":[{files}]}}"
        );
        let err = decode_ndjson_str(&framed(&row)).unwrap_err();
        assert_eq!(
            err,
            GoSemanticProtocolError::ArrayTooLarge {
                field: "files",
                max_items: GO_SEMANTIC_MAX_ARRAY_ITEMS,
            }
        );
    }

    #[test]
    fn decode_ndjson_rejects_decoded_allocation_budget() {
        let limits = GoSemanticProtocolLimits {
            decoded_allocation_bytes: 1,
            ..GoSemanticProtocolLimits::default()
        };
        let input = "{\"schema\":\"polint-go-semantic-2\",\"kind\":\"session_begin\"}\n";
        let err = decode_ndjson_str_with_limits(input, limits, None).unwrap_err();
        assert_eq!(
            err,
            GoSemanticProtocolError::AllocationLimitExceeded { max_bytes: 1 }
        );
    }
}
