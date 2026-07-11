use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::{self, IsTerminal};
use std::sync::Arc;

/// Public URL of [`crate::diagnostics::PolintReport`] JSON Schema (v1); embedded in `--format json` when present.
pub const POLINT_REPORT_JSON_SCHEMA_V1_URL: &str =
    "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-report-v1.json";
/// Public URL of [`crate::diagnostics::AiFriendlyReport`] JSON Schema (v1); embedded in `--format ai-friendly` files.
pub(crate) const POLINT_AI_FRIENDLY_JSON_SCHEMA_V1_URL: &str = "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-ai-friendly-v1.json";

pub(crate) const AI_FRIENDLY_EXAMPLE_LIMIT: usize = 10;
pub(crate) const STRUCTURED_EVIDENCE_MAX_PATHS: usize = 64;
pub(crate) const STRUCTURED_EVIDENCE_MAX_EDGES_PER_PATH: usize = 512;
pub(crate) const STRUCTURED_EVIDENCE_MAX_UNKNOWNS: usize = 512;
pub(crate) const STRUCTURED_EVIDENCE_MAX_OMITTED_REGIONS: usize = 512;
pub(crate) const STRUCTURED_EVIDENCE_MAX_NODES_PER_PATH: usize = 4096;
pub(crate) const STRUCTURED_EVIDENCE_MAX_STRING_CHARS: usize = 4096;
const EVIDENCE_STATUSES: &[&str] = &[
    "Present",
    "Partial",
    "Unknown",
    "Unsupported",
    "SetupMissing",
    "BudgetExceeded",
    "Rejected",
];
const EVIDENCE_PRECISIONS: &[&str] = &[
    "Exact",
    "SetupAware",
    "Syntax",
    "Conservative",
    "Heuristic",
    "Unknown",
];
const EVIDENCE_PROVENANCES: &[&str] = &[
    "Native",
    "Summary",
    "Extension",
    "Model",
    "Query",
    "Synthetic",
];
const EVIDENCE_VALIDATIONS: &[&str] = &[
    "Native",
    "ReferentiallyValidated",
    "ExtensionValidated",
    "BudgetValidated",
    "RendererValidated",
    "Rejected",
];
const EVIDENCE_CONFIDENCES: &[&str] = &["High", "Medium", "Low"];
const EVIDENCE_EDGE_KINDS: &[&str] = &[
    "DataValue",
    "DataTaint",
    "DataAddress",
    "Control",
    "Call",
    "Return",
    "ParameterIn",
    "ParameterOut",
    "Summary",
    "Model",
    "Alias",
    "Unknown",
    "ExplanationOnly",
];
const EVIDENCE_EXPANSION_STATES: &[&str] = &["none", "expandable", "opaque", "external_model"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warn,
    Error,
}

impl Severity {
    pub fn is_at_least(self, threshold: Severity) -> bool {
        self >= threshold
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Info => f.write_str("info"),
            Severity::Warn => f.write_str("warn"),
            Severity::Error => f.write_str("error"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl TextRange {
    pub fn point(line: u32, col: u32) -> Self {
        Self {
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: col,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Label {
    pub range: TextRange,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Suggestion {
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Fix {
    pub message: String,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct Evidence {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StructuredEvidenceV1 {
    value: serde_json::Value,
}

impl StructuredEvidenceV1 {
    pub(crate) fn try_from_value(value: serde_json::Value) -> Result<Self, String> {
        validate_structured_evidence_v1(&value)?;
        Ok(Self { value })
    }

    pub(crate) fn as_value(&self) -> &serde_json::Value {
        &self.value
    }
}

impl Serialize for StructuredEvidenceV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.value.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for StructuredEvidenceV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        Self::try_from_value(value).map_err(de::Error::custom)
    }
}

fn validate_structured_evidence_v1(value: &serde_json::Value) -> Result<(), String> {
    let object = expect_object(value, "evidence_v1")?;
    expect_allowed_keys(
        object,
        "evidence_v1",
        &[
            "version",
            "bundle",
            "paths",
            "unknowns",
            "omitted_regions",
            "limits",
        ],
    )?;
    expect_u64_field(object, "version", "evidence_v1").and_then(|version| {
        if version == 1 {
            Ok(())
        } else {
            Err("evidence_v1.version must be 1".to_string())
        }
    })?;
    validate_evidence_bundle(expect_field(object, "bundle", "evidence_v1")?)?;
    validate_evidence_paths(expect_field(object, "paths", "evidence_v1")?)?;
    validate_evidence_unknowns(expect_field(object, "unknowns", "evidence_v1")?)?;
    validate_evidence_omitted_regions(expect_field(object, "omitted_regions", "evidence_v1")?)?;
    validate_evidence_limits(expect_field(object, "limits", "evidence_v1")?)?;
    validate_evidence_count_invariants(object)?;
    Ok(())
}

fn validate_evidence_bundle(value: &serde_json::Value) -> Result<(), String> {
    let object = expect_object(value, "evidence_v1.bundle")?;
    expect_allowed_keys(
        object,
        "evidence_v1.bundle",
        &[
            "id",
            "stable_key",
            "diagnostic_stable_key",
            "status",
            "precision",
            "provenance",
            "validation",
            "confidence",
            "replay_key",
        ],
    )?;
    expect_u64_field(object, "id", "evidence_v1.bundle")?;
    expect_bounded_string_field(object, "stable_key", "evidence_v1.bundle")?;
    expect_bounded_string_field(object, "diagnostic_stable_key", "evidence_v1.bundle")?;
    expect_enum_field(object, "status", "evidence_v1.bundle", EVIDENCE_STATUSES)?;
    expect_enum_field(
        object,
        "precision",
        "evidence_v1.bundle",
        EVIDENCE_PRECISIONS,
    )?;
    expect_enum_field(
        object,
        "provenance",
        "evidence_v1.bundle",
        EVIDENCE_PROVENANCES,
    )?;
    expect_enum_field(
        object,
        "validation",
        "evidence_v1.bundle",
        EVIDENCE_VALIDATIONS,
    )?;
    expect_enum_field(
        object,
        "confidence",
        "evidence_v1.bundle",
        EVIDENCE_CONFIDENCES,
    )?;
    expect_optional_bounded_string_field(object, "replay_key", "evidence_v1.bundle")?;
    Ok(())
}

fn validate_evidence_paths(value: &serde_json::Value) -> Result<(), String> {
    let paths = expect_array(value, "evidence_v1.paths")?;
    expect_len_at_most(
        paths.len(),
        STRUCTURED_EVIDENCE_MAX_PATHS,
        "evidence_v1.paths",
    )?;
    for (index, path) in paths.iter().enumerate() {
        let path_name = format!("evidence_v1.paths[{index}]");
        let object = expect_object(path, &path_name)?;
        expect_allowed_keys(
            object,
            &path_name,
            &[
                "id",
                "stable_key",
                "rank",
                "status",
                "hidden_node_count",
                "nodes",
                "edges",
                "omitted_regions",
                "total_edges",
                "rendered_edges",
                "edges_truncated",
            ],
        )?;
        expect_u64_field(object, "id", &path_name)?;
        expect_bounded_string_field(object, "stable_key", &path_name)?;
        expect_u64_field(object, "rank", &path_name)?;
        expect_enum_field(object, "status", &path_name, EVIDENCE_STATUSES)?;
        expect_u64_field(object, "hidden_node_count", &path_name)?;
        validate_u64_array(
            expect_field(object, "nodes", &path_name)?,
            &format!("{path_name}.nodes"),
            STRUCTURED_EVIDENCE_MAX_NODES_PER_PATH,
        )?;
        validate_evidence_edges(expect_field(object, "edges", &path_name)?, &path_name)?;
        validate_u64_array(
            expect_field(object, "omitted_regions", &path_name)?,
            &format!("{path_name}.omitted_regions"),
            STRUCTURED_EVIDENCE_MAX_OMITTED_REGIONS,
        )?;
        expect_u64_field(object, "total_edges", &path_name)?;
        expect_u64_field(object, "rendered_edges", &path_name)?;
        expect_bool_field(object, "edges_truncated", &path_name)?;
    }
    Ok(())
}

fn validate_evidence_edges(value: &serde_json::Value, path_name: &str) -> Result<(), String> {
    let edges = expect_array(value, &format!("{path_name}.edges"))?;
    expect_len_at_most(
        edges.len(),
        STRUCTURED_EVIDENCE_MAX_EDGES_PER_PATH,
        &format!("{path_name}.edges"),
    )?;
    for (index, edge) in edges.iter().enumerate() {
        let edge_name = format!("{path_name}.edges[{index}]");
        let object = expect_object(edge, &edge_name)?;
        expect_allowed_keys(
            object,
            &edge_name,
            &[
                "id",
                "stable_key",
                "kind",
                "status",
                "precision",
                "provenance",
                "validation",
                "confidence",
                "summary_stable_key",
                "expansion",
                "location",
            ],
        )?;
        expect_u64_field(object, "id", &edge_name)?;
        expect_bounded_string_field(object, "stable_key", &edge_name)?;
        expect_enum_field(object, "kind", &edge_name, EVIDENCE_EDGE_KINDS)?;
        expect_enum_field(object, "status", &edge_name, EVIDENCE_STATUSES)?;
        expect_enum_field(object, "precision", &edge_name, EVIDENCE_PRECISIONS)?;
        expect_enum_field(object, "provenance", &edge_name, EVIDENCE_PROVENANCES)?;
        expect_enum_field(object, "validation", &edge_name, EVIDENCE_VALIDATIONS)?;
        expect_enum_field(object, "confidence", &edge_name, EVIDENCE_CONFIDENCES)?;
        expect_optional_bounded_string_field(object, "summary_stable_key", &edge_name)?;
        validate_evidence_expansion(expect_field(object, "expansion", &edge_name)?, &edge_name)?;
        if let Some(location) = object.get("location") {
            validate_evidence_location(location, &format!("{edge_name}.location"))?;
        }
    }
    Ok(())
}

fn validate_evidence_expansion(value: &serde_json::Value, edge_name: &str) -> Result<(), String> {
    let object = expect_object(value, &format!("{edge_name}.expansion"))?;
    expect_allowed_keys(
        object,
        &format!("{edge_name}.expansion"),
        &["state", "key", "reason", "model"],
    )?;
    expect_enum_field(
        object,
        "state",
        &format!("{edge_name}.expansion"),
        EVIDENCE_EXPANSION_STATES,
    )?;
    for key in ["key", "reason", "model"] {
        expect_optional_bounded_string_field(object, key, &format!("{edge_name}.expansion"))?;
    }
    Ok(())
}

fn validate_evidence_location(value: &serde_json::Value, name: &str) -> Result<(), String> {
    let object = expect_object(value, name)?;
    expect_allowed_keys(object, name, &["uri", "range"])?;
    expect_bounded_string_field(object, "uri", name)?;
    validate_text_range(
        expect_field(object, "range", name)?,
        &format!("{name}.range"),
    )
}

fn validate_text_range(value: &serde_json::Value, name: &str) -> Result<(), String> {
    let object = expect_object(value, name)?;
    expect_allowed_keys(
        object,
        name,
        &["start_line", "start_col", "end_line", "end_col"],
    )?;
    for key in ["start_line", "start_col", "end_line", "end_col"] {
        expect_u64_field(object, key, name)?;
    }
    Ok(())
}

fn validate_evidence_unknowns(value: &serde_json::Value) -> Result<(), String> {
    let unknowns = expect_array(value, "evidence_v1.unknowns")?;
    expect_len_at_most(
        unknowns.len(),
        STRUCTURED_EVIDENCE_MAX_UNKNOWNS,
        "evidence_v1.unknowns",
    )?;
    for (index, unknown) in unknowns.iter().enumerate() {
        let name = format!("evidence_v1.unknowns[{index}]");
        let object = expect_object(unknown, &name)?;
        expect_allowed_keys(object, &name, &["stable_key", "reason", "message", "edge"])?;
        expect_bounded_string_field(object, "stable_key", &name)?;
        expect_bounded_string_field(object, "reason", &name)?;
        expect_bounded_string_field(object, "message", &name)?;
        expect_optional_u64_field(object, "edge", &name)?;
    }
    Ok(())
}

fn validate_evidence_omitted_regions(value: &serde_json::Value) -> Result<(), String> {
    let regions = expect_array(value, "evidence_v1.omitted_regions")?;
    expect_len_at_most(
        regions.len(),
        STRUCTURED_EVIDENCE_MAX_OMITTED_REGIONS,
        "evidence_v1.omitted_regions",
    )?;
    for (index, region) in regions.iter().enumerate() {
        let name = format!("evidence_v1.omitted_regions[{index}]");
        let object = expect_object(region, &name)?;
        expect_allowed_keys(
            object,
            &name,
            &[
                "id",
                "stable_key",
                "reason",
                "hidden_node_count",
                "hidden_edge_count",
                "budget_label",
            ],
        )?;
        expect_u64_field(object, "id", &name)?;
        expect_bounded_string_field(object, "stable_key", &name)?;
        expect_bounded_string_field(object, "reason", &name)?;
        expect_u64_field(object, "hidden_node_count", &name)?;
        expect_u64_field(object, "hidden_edge_count", &name)?;
        expect_optional_bounded_string_field(object, "budget_label", &name)?;
    }
    Ok(())
}

fn validate_evidence_limits(value: &serde_json::Value) -> Result<(), String> {
    let object = expect_object(value, "evidence_v1.limits")?;
    expect_allowed_keys(
        object,
        "evidence_v1.limits",
        &[
            "max_paths",
            "max_edges_per_path",
            "max_unknowns",
            "max_omitted_regions",
            "total_paths",
            "rendered_paths",
            "paths_truncated",
            "total_unknowns",
            "rendered_unknowns",
            "unknowns_truncated",
            "total_omitted_regions",
            "rendered_omitted_regions",
            "omitted_regions_truncated",
        ],
    )?;
    for key in [
        "max_paths",
        "max_edges_per_path",
        "max_unknowns",
        "max_omitted_regions",
        "total_paths",
        "rendered_paths",
        "total_unknowns",
        "rendered_unknowns",
        "total_omitted_regions",
        "rendered_omitted_regions",
    ] {
        expect_u64_field(object, key, "evidence_v1.limits")?;
    }
    for key in [
        "paths_truncated",
        "unknowns_truncated",
        "omitted_regions_truncated",
    ] {
        expect_bool_field(object, key, "evidence_v1.limits")?;
    }
    Ok(())
}

fn validate_evidence_count_invariants(
    object: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let paths = expect_array(
        expect_field(object, "paths", "evidence_v1")?,
        "evidence_v1.paths",
    )?;
    let unknowns = expect_array(
        expect_field(object, "unknowns", "evidence_v1")?,
        "evidence_v1.unknowns",
    )?;
    let omitted_regions = expect_array(
        expect_field(object, "omitted_regions", "evidence_v1")?,
        "evidence_v1.omitted_regions",
    )?;
    let limits = expect_object(
        expect_field(object, "limits", "evidence_v1")?,
        "evidence_v1.limits",
    )?;

    expect_count_consistency(CountConsistency {
        name: "paths",
        max: expect_u64_field(limits, "max_paths", "evidence_v1.limits")?,
        total: expect_u64_field(limits, "total_paths", "evidence_v1.limits")?,
        rendered: expect_u64_field(limits, "rendered_paths", "evidence_v1.limits")?,
        actual_rendered: paths.len() as u64,
        truncated: expect_bool_value_field(limits, "paths_truncated", "evidence_v1.limits")?,
    })?;
    expect_count_consistency(CountConsistency {
        name: "unknowns",
        max: expect_u64_field(limits, "max_unknowns", "evidence_v1.limits")?,
        total: expect_u64_field(limits, "total_unknowns", "evidence_v1.limits")?,
        rendered: expect_u64_field(limits, "rendered_unknowns", "evidence_v1.limits")?,
        actual_rendered: unknowns.len() as u64,
        truncated: expect_bool_value_field(limits, "unknowns_truncated", "evidence_v1.limits")?,
    })?;
    expect_count_consistency(CountConsistency {
        name: "omitted_regions",
        max: expect_u64_field(limits, "max_omitted_regions", "evidence_v1.limits")?,
        total: expect_u64_field(limits, "total_omitted_regions", "evidence_v1.limits")?,
        rendered: expect_u64_field(limits, "rendered_omitted_regions", "evidence_v1.limits")?,
        actual_rendered: omitted_regions.len() as u64,
        truncated: expect_bool_value_field(
            limits,
            "omitted_regions_truncated",
            "evidence_v1.limits",
        )?,
    })?;

    let max_edges = expect_u64_field(limits, "max_edges_per_path", "evidence_v1.limits")?;
    for (index, path) in paths.iter().enumerate() {
        let path_name = format!("evidence_v1.paths[{index}]");
        let path_object = expect_object(path, &path_name)?;
        let edges = expect_array(
            expect_field(path_object, "edges", &path_name)?,
            &format!("{path_name}.edges"),
        )?;
        expect_count_consistency(CountConsistency {
            name: "edges",
            max: max_edges,
            total: expect_u64_field(path_object, "total_edges", &path_name)?,
            rendered: expect_u64_field(path_object, "rendered_edges", &path_name)?,
            actual_rendered: edges.len() as u64,
            truncated: expect_bool_value_field(path_object, "edges_truncated", &path_name)?,
        })
        .map_err(|error| format!("{path_name}.{error}"))?;
    }
    Ok(())
}

struct CountConsistency {
    name: &'static str,
    max: u64,
    total: u64,
    rendered: u64,
    actual_rendered: u64,
    truncated: bool,
}

fn expect_count_consistency(counts: CountConsistency) -> Result<(), String> {
    if counts.rendered != counts.actual_rendered {
        return Err(format!(
            "{} rendered count {} does not match rendered array length {}",
            counts.name, counts.rendered, counts.actual_rendered
        ));
    }
    if counts.rendered > counts.max {
        return Err(format!(
            "{} rendered count {} exceeds max {}",
            counts.name, counts.rendered, counts.max
        ));
    }
    if counts.total < counts.rendered {
        return Err(format!(
            "{} total count {} is less than rendered count {}",
            counts.name, counts.total, counts.rendered
        ));
    }
    let should_be_truncated = counts.total > counts.rendered;
    if counts.truncated != should_be_truncated {
        return Err(format!(
            "{} truncation flag {} does not match total/rendered counts",
            counts.name, counts.truncated
        ));
    }
    Ok(())
}

fn validate_u64_array(value: &serde_json::Value, name: &str, max_len: usize) -> Result<(), String> {
    let values = expect_array(value, name)?;
    expect_len_at_most(values.len(), max_len, name)?;
    for item in values {
        if !item.is_u64() {
            return Err(format!("{name} entries must be unsigned integers"));
        }
    }
    Ok(())
}

fn expect_object<'a>(
    value: &'a serde_json::Value,
    name: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, String> {
    value
        .as_object()
        .ok_or_else(|| format!("{name} must be an object"))
}

fn expect_array<'a>(
    value: &'a serde_json::Value,
    name: &str,
) -> Result<&'a [serde_json::Value], String> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{name} must be an array"))
}

fn expect_field<'a>(
    object: &'a serde_json::Map<String, serde_json::Value>,
    key: &str,
    object_name: &str,
) -> Result<&'a serde_json::Value, String> {
    object
        .get(key)
        .ok_or_else(|| format!("{object_name}.{key} is required"))
}

fn expect_allowed_keys(
    object: &serde_json::Map<String, serde_json::Value>,
    object_name: &str,
    allowed: &[&str],
) -> Result<(), String> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!(
                "{object_name}.{key} is not a supported evidence_v1 field"
            ));
        }
    }
    Ok(())
}

fn expect_len_at_most(len: usize, max: usize, name: &str) -> Result<(), String> {
    if len <= max {
        Ok(())
    } else {
        Err(format!("{name} has {len} entries; maximum is {max}"))
    }
}

fn expect_u64_field(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    object_name: &str,
) -> Result<u64, String> {
    expect_field(object, key, object_name)?
        .as_u64()
        .ok_or_else(|| format!("{object_name}.{key} must be an unsigned integer"))
}

fn expect_optional_u64_field(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    object_name: &str,
) -> Result<(), String> {
    match object.get(key) {
        Some(value) if value.is_null() || value.is_u64() => Ok(()),
        Some(_) => Err(format!(
            "{object_name}.{key} must be an unsigned integer or null"
        )),
        None => Ok(()),
    }
}

fn expect_bool_field(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    object_name: &str,
) -> Result<(), String> {
    if expect_field(object, key, object_name)?.is_boolean() {
        Ok(())
    } else {
        Err(format!("{object_name}.{key} must be a boolean"))
    }
}

fn expect_bool_value_field(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    object_name: &str,
) -> Result<bool, String> {
    expect_field(object, key, object_name)?
        .as_bool()
        .ok_or_else(|| format!("{object_name}.{key} must be a boolean"))
}

fn expect_bounded_string_field(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    object_name: &str,
) -> Result<(), String> {
    let value = expect_field(object, key, object_name)?
        .as_str()
        .ok_or_else(|| format!("{object_name}.{key} must be a string"))?;
    expect_bounded_string(value, &format!("{object_name}.{key}"))
}

fn expect_enum_field(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    object_name: &str,
    allowed: &[&str],
) -> Result<(), String> {
    let value = expect_field(object, key, object_name)?
        .as_str()
        .ok_or_else(|| format!("{object_name}.{key} must be a string"))?;
    expect_bounded_string(value, &format!("{object_name}.{key}"))?;
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "{object_name}.{key} must be one of: {}",
            allowed.join(", ")
        ))
    }
}

fn expect_optional_bounded_string_field(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    object_name: &str,
) -> Result<(), String> {
    match object.get(key) {
        Some(value) if value.is_null() => Ok(()),
        Some(value) => {
            let string = value
                .as_str()
                .ok_or_else(|| format!("{object_name}.{key} must be a string or null"))?;
            expect_bounded_string(string, &format!("{object_name}.{key}"))
        }
        None => Ok(()),
    }
}

fn expect_bounded_string(value: &str, name: &str) -> Result<(), String> {
    let char_count = value.chars().count();
    if char_count <= STRUCTURED_EVIDENCE_MAX_STRING_CHARS {
        Ok(())
    } else {
        Err(format!(
            "{name} has {char_count} characters; maximum is {STRUCTURED_EVIDENCE_MAX_STRING_CHARS}",
        ))
    }
}

/// Construct diagnostics with `Diagnostic::new`, `Diagnostic::error`,
/// `Diagnostic::warning`, `Diagnostic::info`, and the fluent helpers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Diagnostic {
    pub rule_id: String,
    pub severity: Severity,
    pub file: String,
    pub range: TextRange,
    pub message: String,
    pub labels: Vec<Label>,
    pub help: Option<String>,
    pub evidence: Vec<Evidence>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) evidence_v1: Option<StructuredEvidenceV1>,
    #[serde(skip)]
    pub(crate) evidence_bundle: Option<EvidenceBundleRef>,
    pub suggestions: Vec<Suggestion>,
    pub fix: Option<Fix>,
    pub stable_fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EvidenceBundleRef {
    pub(crate) id: crate::analysis::ids::EvidenceBundleId,
}

#[derive(Deserialize)]
struct DiagnosticWire {
    rule_id: String,
    severity: Severity,
    file: String,
    range: TextRange,
    message: String,
    #[serde(default)]
    labels: Vec<Label>,
    #[serde(default)]
    help: Option<String>,
    #[serde(default)]
    evidence: Vec<Evidence>,
    #[serde(default)]
    evidence_v1: Option<StructuredEvidenceV1>,
    #[serde(default)]
    suggestions: Vec<Suggestion>,
    #[serde(default)]
    fix: Option<Fix>,
    #[serde(default)]
    stable_fingerprint: String,
}

impl<'de> Deserialize<'de> for Diagnostic {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = DiagnosticWire::deserialize(deserializer)?;
        let stable_fingerprint = if wire.stable_fingerprint.is_empty() {
            diagnostic_fingerprint(&wire.rule_id, &wire.file, wire.range, &wire.message)
        } else {
            wire.stable_fingerprint
        };

        Ok(Self {
            rule_id: wire.rule_id,
            severity: wire.severity,
            file: wire.file,
            range: wire.range,
            message: wire.message,
            labels: wire.labels,
            help: wire.help,
            evidence: wire.evidence,
            evidence_v1: wire.evidence_v1,
            evidence_bundle: None,
            suggestions: wire.suggestions,
            fix: wire.fix,
            stable_fingerprint,
        })
    }
}

impl Diagnostic {
    pub fn new(
        rule_id: impl Into<String>,
        severity: Severity,
        file: impl Into<String>,
        range: TextRange,
        message: impl Into<String>,
    ) -> Self {
        let rule_id = rule_id.into();
        let file = file.into();
        let message = message.into();
        let stable_fingerprint = diagnostic_fingerprint(&rule_id, &file, range, &message);
        Self {
            rule_id,
            severity,
            file,
            range,
            message,
            labels: Vec::new(),
            help: None,
            evidence: Vec::new(),
            evidence_v1: None,
            evidence_bundle: None,
            suggestions: Vec::new(),
            fix: None,
            stable_fingerprint,
        }
    }

    pub fn error(
        rule_id: impl Into<String>,
        file: impl Into<String>,
        range: TextRange,
        message: impl Into<String>,
    ) -> Self {
        Self::new(rule_id, Severity::Error, file, range, message)
    }

    pub fn warning(
        rule_id: impl Into<String>,
        file: impl Into<String>,
        range: TextRange,
        message: impl Into<String>,
    ) -> Self {
        Self::new(rule_id, Severity::Warn, file, range, message)
    }

    pub fn info(
        rule_id: impl Into<String>,
        file: impl Into<String>,
        range: TextRange,
        message: impl Into<String>,
    ) -> Self {
        Self::new(rule_id, Severity::Info, file, range, message)
    }

    pub fn with_label(mut self, range: TextRange, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            range,
            message: message.into(),
        });
        self
    }

    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    pub fn with_evidence(mut self, label: impl Into<String>, value: impl Into<String>) -> Self {
        self.evidence.push(Evidence {
            label: label.into(),
            value: value.into(),
        });
        self
    }

    #[allow(
        dead_code,
        reason = "Internal bundle refs are attached by current render plumbing; tests verify fingerprint compatibility before rule-facing APIs exist."
    )]
    pub(crate) fn with_evidence_bundle_ref(
        mut self,
        id: crate::analysis::ids::EvidenceBundleId,
    ) -> Self {
        self.evidence_bundle = Some(EvidenceBundleRef { id });
        self
    }

    #[allow(
        dead_code,
        reason = "Structured evidence attachment is wired for current renderers and exercised through diagnostics tests before production call sites attach bundles."
    )]
    pub(crate) fn with_structured_evidence_v1(mut self, value: StructuredEvidenceV1) -> Self {
        self.evidence_v1 = Some(value);
        self
    }

    #[allow(
        dead_code,
        reason = "Internal bundle refs are read by current render plumbing; tests verify the side-table bridge without making it public SDK."
    )]
    pub(crate) fn evidence_bundle_ref(&self) -> Option<EvidenceBundleRef> {
        self.evidence_bundle
    }

    pub fn with_suggestion(mut self, message: impl Into<String>) -> Self {
        self.suggestions.push(Suggestion {
            message: message.into(),
        });
        self
    }

    pub fn with_fix(mut self, message: impl Into<String>, replacement: Option<String>) -> Self {
        self.fix = Some(Fix {
            message: message.into(),
            replacement,
        });
        self
    }

    pub fn with_fingerprint(mut self, stable_fingerprint: impl Into<String>) -> Self {
        self.stable_fingerprint = stable_fingerprint.into();
        self
    }
}

pub(crate) fn extension_setup_diagnostic(
    failure_kind: &str,
    extension_id: &str,
    provider_id: Option<&str>,
    summary: &str,
) -> Diagnostic {
    let provider_label = provider_id.unwrap_or("<handshake>");
    Diagnostic::warning(
        "polint/extension",
        "",
        TextRange::point(1, 1),
        format!("Extension provider setup failed: {failure_kind}"),
    )
    .with_evidence("extension_id", extension_id)
    .with_evidence("provider_id", provider_label)
    .with_evidence("failure_kind", failure_kind)
    .with_evidence("summary", bounded_extension_summary(summary))
}

fn bounded_extension_summary(summary: &str) -> String {
    summary
        .replace('\\', "/")
        .lines()
        .take(4)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(512)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Github,
    Json,
    Sarif,
    AiFriendly,
}

/// Metadata embedded in `--format json` output (`PolintReport.tool`).
#[derive(Debug, Clone, Copy)]
pub struct JsonReportMeta<'a> {
    pub tool_name: &'a str,
    pub tool_version: &'a str,
}

/// When to emit ANSI colors for `--format human` (honors `NO_COLOR` when [`ColorChoice::Auto`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

impl ColorChoice {
    pub fn use_ansi_colors(self) -> bool {
        match self {
            ColorChoice::Never => false,
            ColorChoice::Always => true,
            ColorChoice::Auto => {
                io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RenderOpts<'a> {
    pub json: JsonReportMeta<'a>,
    pub color: ColorChoice,
    /// Indexed by `Diagnostic.file`-style relative paths for human code snippets.
    pub sources: Option<&'a BTreeMap<String, Arc<str>>>,
}

/// Tool identity in a [`PolintReport`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolintToolInfo {
    pub name: String,
    pub version: String,
}

/// Versioned JSON report for `--format json` and repo-local rule host subprocesses.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolintReport {
    pub version: u32,
    pub tool: PolintToolInfo,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AiFriendlyReport {
    pub(crate) version: u32,
    pub(crate) schema: String,
    pub(crate) tool: PolintToolInfo,
    pub(crate) generated_at: String,
    pub(crate) summary: AiFriendlySummary,
    pub(crate) examples: Vec<AiFriendlyExample>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) truncation: Option<AiFriendlyTruncation>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AiFriendlySummary {
    pub(crate) total_diagnostics: usize,
    pub(crate) rules_triggered: usize,
    pub(crate) by_severity: BTreeMap<String, usize>,
    pub(crate) by_rule: Vec<AiFriendlyRuleSummary>,
    pub(crate) examples_limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AiFriendlyRuleSummary {
    pub(crate) rule_id: String,
    pub(crate) total: usize,
    pub(crate) by_severity: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AiFriendlyExample {
    pub(crate) rule_id: String,
    pub(crate) severity: Severity,
    pub(crate) file: String,
    pub(crate) range: TextRange,
    pub(crate) message: String,
    pub(crate) labels: Vec<Label>,
    pub(crate) help: Option<String>,
    pub(crate) evidence: Vec<Evidence>,
    pub(crate) suggestions: Vec<Suggestion>,
    pub(crate) fix: Option<Fix>,
    pub(crate) stable_fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AiFriendlyTruncation {
    pub(crate) total_diagnostics: usize,
    pub(crate) included_diagnostics: usize,
    pub(crate) reason: String,
}

#[derive(Serialize)]
struct PolintReportWire<'a> {
    version: u32,
    schema: &'static str,
    tool: PolintToolWire<'a>,
    diagnostics: &'a [Diagnostic],
}

#[derive(Serialize)]
struct PolintToolWire<'a> {
    name: &'a str,
    version: &'a str,
}

/// Parse stdout from a `polint-local-rules check --format json` process.
pub fn diagnostics_from_json_report(s: &str) -> Result<Vec<Diagnostic>, serde_json::Error> {
    let report: PolintReport = serde_json::from_str(s)?;
    Ok(report.diagnostics)
}

/// Parse public diagnostics from an external rule-host JSON process.
pub(crate) fn diagnostics_from_public_json_report(
    s: &str,
) -> Result<Vec<Diagnostic>, serde_json::Error> {
    let mut diagnostics = diagnostics_from_json_report(s)?;
    for diagnostic in &mut diagnostics {
        diagnostic.evidence_v1 = None;
        diagnostic.evidence_bundle = None;
    }
    Ok(diagnostics)
}

pub(crate) fn build_ai_friendly_report(
    diagnostics: &[Diagnostic],
    persisted_diagnostics: &[Diagnostic],
    json_meta: JsonReportMeta<'_>,
    generated_at: impl Into<String>,
) -> AiFriendlyReport {
    let summary = ai_friendly_summary(diagnostics);
    let examples = ai_friendly_examples(diagnostics, &summary.by_rule);
    let truncation = if persisted_diagnostics.len() < diagnostics.len() {
        Some(AiFriendlyTruncation {
            total_diagnostics: diagnostics.len(),
            included_diagnostics: persisted_diagnostics.len(),
            reason: "--max-diagnostics limited persisted diagnostics".to_string(),
        })
    } else {
        None
    };
    AiFriendlyReport {
        version: 1,
        schema: POLINT_AI_FRIENDLY_JSON_SCHEMA_V1_URL.to_string(),
        tool: PolintToolInfo {
            name: json_meta.tool_name.to_string(),
            version: json_meta.tool_version.to_string(),
        },
        generated_at: generated_at.into(),
        summary,
        examples,
        truncation,
        diagnostics: persisted_diagnostics.to_vec(),
    }
}

fn ai_friendly_summary(diagnostics: &[Diagnostic]) -> AiFriendlySummary {
    let mut by_severity = empty_severity_counts();
    let mut by_rule: BTreeMap<String, RuleSummaryDraft> = BTreeMap::new();
    for diagnostic in diagnostics {
        increment_severity(&mut by_severity, diagnostic.severity);
        let draft = by_rule
            .entry(diagnostic.rule_id.clone())
            .or_insert_with(|| RuleSummaryDraft::new(diagnostic.rule_id.clone()));
        draft.total += 1;
        increment_severity(&mut draft.by_severity, diagnostic.severity);
        draft.highest_severity = draft
            .highest_severity
            .min(severity_sort_rank(diagnostic.severity));
    }

    let mut by_rule: Vec<RuleSummaryDraft> = by_rule.into_values().collect();
    by_rule.sort_by(|left, right| {
        (
            left.highest_severity,
            std::cmp::Reverse(left.total),
            left.rule_id.as_str(),
        )
            .cmp(&(
                right.highest_severity,
                std::cmp::Reverse(right.total),
                right.rule_id.as_str(),
            ))
    });

    AiFriendlySummary {
        total_diagnostics: diagnostics.len(),
        rules_triggered: by_rule.len(),
        by_severity,
        by_rule: by_rule
            .into_iter()
            .map(|draft| AiFriendlyRuleSummary {
                rule_id: draft.rule_id,
                total: draft.total,
                by_severity: draft.by_severity,
            })
            .collect(),
        examples_limit: AI_FRIENDLY_EXAMPLE_LIMIT,
    }
}

#[derive(Debug, Clone)]
struct RuleSummaryDraft {
    rule_id: String,
    total: usize,
    by_severity: BTreeMap<String, usize>,
    highest_severity: u8,
}

impl RuleSummaryDraft {
    fn new(rule_id: String) -> Self {
        Self {
            rule_id,
            total: 0,
            by_severity: empty_severity_counts(),
            highest_severity: u8::MAX,
        }
    }
}

fn ai_friendly_examples(
    diagnostics: &[Diagnostic],
    by_rule: &[AiFriendlyRuleSummary],
) -> Vec<AiFriendlyExample> {
    let mut examples = BTreeMap::<String, &Diagnostic>::new();
    for diagnostic in diagnostics {
        examples
            .entry(diagnostic.rule_id.clone())
            .and_modify(|current| {
                if example_sort_key(diagnostic) < example_sort_key(current) {
                    *current = diagnostic;
                }
            })
            .or_insert(diagnostic);
    }

    by_rule
        .iter()
        .filter_map(|summary| examples.get(&summary.rule_id).copied())
        .take(AI_FRIENDLY_EXAMPLE_LIMIT)
        .map(AiFriendlyExample::from)
        .collect()
}

fn example_sort_key(diagnostic: &Diagnostic) -> (u8, &str, u32, u32, &str, &str) {
    (
        severity_sort_rank(diagnostic.severity),
        diagnostic.file.as_str(),
        diagnostic.range.start_line,
        diagnostic.range.start_col,
        diagnostic.message.as_str(),
        diagnostic.stable_fingerprint.as_str(),
    )
}

impl From<&Diagnostic> for AiFriendlyExample {
    fn from(diagnostic: &Diagnostic) -> Self {
        Self {
            rule_id: diagnostic.rule_id.clone(),
            severity: diagnostic.severity,
            file: diagnostic.file.clone(),
            range: diagnostic.range,
            message: diagnostic.message.clone(),
            labels: diagnostic.labels.clone(),
            help: diagnostic.help.clone(),
            evidence: diagnostic.evidence.clone(),
            suggestions: diagnostic.suggestions.clone(),
            fix: diagnostic.fix.clone(),
            stable_fingerprint: diagnostic.stable_fingerprint.clone(),
        }
    }
}

fn empty_severity_counts() -> BTreeMap<String, usize> {
    BTreeMap::from([
        ("error".to_string(), 0),
        ("warn".to_string(), 0),
        ("info".to_string(), 0),
    ])
}

fn increment_severity(counts: &mut BTreeMap<String, usize>, severity: Severity) {
    *counts
        .entry(severity_count_key(severity).to_string())
        .or_default() += 1;
}

fn severity_count_key(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}

fn severity_sort_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 0,
        Severity::Warn => 1,
        Severity::Info => 2,
    }
}

pub(crate) fn render_ai_friendly_stdout(report: &AiFriendlyReport, json_path: &str) -> String {
    let total = report.summary.total_diagnostics;
    let rules = report.summary.rules_triggered;
    let mut out = String::new();
    out.push_str(&format!(
        "polint: {total} {} across {rules} {}. Full JSON: {json_path}\n",
        plural(total, "diagnostic", "diagnostics"),
        plural(rules, "rule", "rules")
    ));

    if let Some(truncation) = &report.truncation {
        out.push_str(&format!(
            "Persisted diagnostics were limited: {} of {} included ({})\n",
            truncation.included_diagnostics, truncation.total_diagnostics, truncation.reason
        ));
    }

    out.push_str("\nBy rule\n");
    if report.summary.by_rule.is_empty() {
        out.push_str("  none\n");
    } else {
        for rule in &report.summary.by_rule {
            out.push_str(&format!(
                "  {} {}: {}\n",
                dominant_rule_severity(rule),
                rule.rule_id,
                rule.total
            ));
        }
    }

    out.push_str("\nExamples, max 10\n");
    if report.examples.is_empty() {
        out.push_str("  none\n");
    } else {
        for example in &report.examples {
            push_ai_friendly_example(&mut out, example);
        }
    }

    out.push_str(
        "\nJSON format: versioned object with `summary`, `examples`, and `diagnostics`.\n",
    );
    out.push_str("Do not read the whole file into an AI prompt. Query it with bounded commands:\n");
    out.push_str(&format!("  jq '.summary.by_rule' {json_path}\n"));
    if let Some(example) = report.examples.first() {
        let rule_id = jq_string_literal(&example.rule_id);
        let file = jq_string_literal(&example.file);
        out.push_str(&format!(
            "  jq '[.diagnostics[] | select(.rule_id=={rule_id})][0:20]' {json_path}\n"
        ));
        out.push_str(&format!(
            "  jq '.diagnostics[] | select(.file=={file}) | {{rule_id, range, message}}' {json_path} | head -c 12000\n"
        ));
    } else {
        out.push_str(&format!(
            "  jq '[.diagnostics[] | select(.rule_id==\"local/example\")][0:20]' {json_path}\n"
        ));
        out.push_str(&format!(
            "  jq '.diagnostics[] | select(.file==\"src/example.ts\") | {{rule_id, range, message}}' {json_path} | head -c 12000\n"
        ));
    }
    out
}

fn jq_string_literal(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn push_ai_friendly_example(out: &mut String, example: &AiFriendlyExample) {
    out.push_str(&format!(
        "  {}[{}]: {}\n    --> {}:{}\n",
        severity_label(example.severity),
        example.rule_id,
        example.message,
        example.file,
        format_range(example.range)
    ));
    for label in &example.labels {
        out.push_str(&format!(
            "    label {}: {}\n",
            format_range(label.range),
            label.message
        ));
    }
    for evidence in &example.evidence {
        out.push_str(&format!(
            "    evidence {}: {}\n",
            evidence.label, evidence.value
        ));
    }
    for suggestion in &example.suggestions {
        out.push_str(&format!("    suggestion: {}\n", suggestion.message));
    }
    if let Some(fix) = &example.fix {
        out.push_str(&format!("    fix: {}\n", fix.message));
        if let Some(replacement) = &fix.replacement {
            out.push_str(&format!("      replacement: {replacement}\n"));
        }
    }
    if let Some(help) = &example.help {
        out.push_str(&format!("    help: {help}\n"));
    }
    out.push_str(&format!(
        "    fingerprint: {}\n\n",
        example.stable_fingerprint
    ));
}

fn severity_label(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warn => "warn",
        Severity::Info => "info",
    }
}

fn dominant_rule_severity(rule: &AiFriendlyRuleSummary) -> &'static str {
    if rule.by_severity.get("error").copied().unwrap_or(0) > 0 {
        "error"
    } else if rule.by_severity.get("warn").copied().unwrap_or(0) > 0 {
        "warn"
    } else {
        "info"
    }
}

fn plural(count: usize, singular: &'static str, plural: &'static str) -> &'static str {
    if count == 1 { singular } else { plural }
}

pub(crate) fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|a, b| {
        (
            a.file.as_str(),
            a.range.start_line,
            a.range.start_col,
            diagnostic_sort_priority(a),
            a.rule_id.as_str(),
            a.message.as_str(),
            a.stable_fingerprint.as_str(),
        )
            .cmp(&(
                b.file.as_str(),
                b.range.start_line,
                b.range.start_col,
                diagnostic_sort_priority(b),
                b.rule_id.as_str(),
                b.message.as_str(),
                b.stable_fingerprint.as_str(),
            ))
    });
}

fn diagnostic_sort_priority(diagnostic: &Diagnostic) -> u8 {
    if diagnostic.rule_id == "polint/capability" {
        0
    } else {
        1
    }
}

pub(crate) fn dedupe_diagnostics(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut diagnostics = diagnostics;
    sort_diagnostics(&mut diagnostics);
    let mut seen = BTreeSet::new();
    diagnostics
        .into_iter()
        .filter(|diagnostic| seen.insert(diagnostic.stable_fingerprint.clone()))
        .collect()
}

/// Dedupe, optionally filter by `rule_id` pattern (see [`crate::core::rule_id_matches`]),
/// then sort for deterministic ordering.
pub(crate) fn apply_report_filters(
    diagnostics: Vec<Diagnostic>,
    only_rule: Option<&str>,
) -> Vec<Diagnostic> {
    let mut diagnostics = dedupe_diagnostics(diagnostics);
    if let Some(pattern) = only_rule {
        diagnostics.retain(|diagnostic| diagnostic_matches_rule_pattern(pattern, diagnostic));
    }
    sort_diagnostics(&mut diagnostics);
    diagnostics
}

fn diagnostic_matches_rule_pattern(pattern: &str, diagnostic: &Diagnostic) -> bool {
    crate::core::rule_id_matches(pattern, &diagnostic.rule_id)
        || diagnostic.rule_id.starts_with("parser/")
        || (diagnostic.rule_id == "polint/capability"
            && diagnostic.evidence.iter().any(|evidence| {
                evidence.label == "rule" && crate::core::rule_id_matches(pattern, &evidence.value)
            }))
        || (diagnostic.rule_id.starts_with("polint/")
            && diagnostic.evidence.iter().any(|evidence| {
                evidence.label == "selectors"
                    && evidence.value.split(',').map(str::trim).any(|selector| {
                        !selector.is_empty()
                            && (crate::core::rule_id_matches(pattern, selector)
                                || crate::core::rule_id_matches(selector, pattern))
                    })
            }))
}

/// Truncate diagnostics for rendering only. Exit status must be computed before this cap.
pub(crate) fn limit_report_diagnostics(
    mut diagnostics: Vec<Diagnostic>,
    max_diagnostics: Option<usize>,
) -> Vec<Diagnostic> {
    if let Some(max) = max_diagnostics {
        diagnostics.truncate(max);
    }
    diagnostics
}

#[cfg(test)]
pub(crate) fn render(
    format: OutputFormat,
    diagnostics: &[Diagnostic],
    opts: RenderOpts<'_>,
) -> String {
    render_with_sarif_help(format, diagnostics, opts, None)
}

pub(crate) fn render_with_sarif_help(
    format: OutputFormat,
    diagnostics: &[Diagnostic],
    opts: RenderOpts<'_>,
    sarif_rule_help_uri: Option<&BTreeMap<String, String>>,
) -> String {
    match format {
        OutputFormat::Human => render_human(diagnostics, opts.color, opts.sources),
        OutputFormat::Github => render_github(diagnostics),
        OutputFormat::Json => render_json(diagnostics, opts.json),
        OutputFormat::Sarif => render_sarif(diagnostics, sarif_rule_help_uri),
        OutputFormat::AiFriendly => {
            let report = build_ai_friendly_report(diagnostics, diagnostics, opts.json, "unknown");
            render_ai_friendly_stdout(&report, ".polint/output/latest.json")
        }
    }
}

fn render_github(diagnostics: &[Diagnostic]) -> String {
    let mut output = String::new();
    for diagnostic in diagnostics {
        output.push_str("::");
        output.push_str(github_command(diagnostic.severity));
        output.push_str(" file=");
        output.push_str(&escape_github_property(&diagnostic.file));
        output.push_str(",line=");
        output.push_str(&github_line(diagnostic.range.start_line).to_string());
        output.push_str(",col=");
        output.push_str(&github_line(diagnostic.range.start_col).to_string());
        output.push_str(",endLine=");
        output.push_str(&github_line(diagnostic.range.end_line).to_string());
        output.push_str(",endColumn=");
        output.push_str(&github_line(diagnostic.range.end_col).to_string());
        output.push_str(",title=");
        output.push_str(&escape_github_property(&diagnostic.rule_id));
        output.push_str("::");
        output.push_str(&escape_github_message(&diagnostic.message));
        output.push('\n');
    }
    output
}

fn github_command(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warn => "warning",
        Severity::Info => "notice",
    }
}

fn github_line(value: u32) -> u32 {
    value.max(1)
}

fn escape_github_property(value: &str) -> String {
    escape_github_message(value)
        .replace(':', "%3A")
        .replace(',', "%2C")
}

fn escape_github_message(value: &str) -> String {
    value
        .replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn render_json(diagnostics: &[Diagnostic], json_meta: JsonReportMeta<'_>) -> String {
    let wire = PolintReportWire {
        version: 1,
        schema: POLINT_REPORT_JSON_SCHEMA_V1_URL,
        tool: PolintToolWire {
            name: json_meta.tool_name,
            version: json_meta.tool_version,
        },
        diagnostics,
    };
    serde_json::to_string_pretty(&wire).unwrap_or_else(|_| "{}".to_string())
}

struct HumanPalette {
    reset: &'static str,
    bold: &'static str,
    dim: &'static str,
    red: &'static str,
    yellow: &'static str,
    cyan: &'static str,
    green: &'static str,
    magenta: &'static str,
}

impl HumanPalette {
    fn new(color: bool) -> Self {
        if color {
            Self {
                reset: "\x1b[0m",
                bold: "\x1b[1m",
                dim: "\x1b[2m",
                red: "\x1b[1;31m",
                yellow: "\x1b[1;33m",
                cyan: "\x1b[1;36m",
                green: "\x1b[1;32m",
                magenta: "\x1b[1;35m",
            }
        } else {
            Self {
                reset: "",
                bold: "",
                dim: "",
                red: "",
                yellow: "",
                cyan: "",
                green: "",
                magenta: "",
            }
        }
    }

    fn severity_paint(&self, severity: Severity) -> &'static str {
        match severity {
            Severity::Error => self.red,
            Severity::Warn => self.yellow,
            Severity::Info => self.cyan,
        }
    }
}

fn lookup_source<'a>(sources: &'a BTreeMap<String, Arc<str>>, file: &str) -> Option<&'a str> {
    if let Some(s) = sources.get(file) {
        return Some(s.as_ref());
    }
    let trimmed = file.trim_start_matches("./");
    if let Some(s) = sources.get(trimmed) {
        return Some(s.as_ref());
    }
    let with_dot = format!("./{trimmed}");
    sources.get(&with_dot).map(|a| a.as_ref())
}

/// 1-based column: return byte index at that column (first column is 1).
fn byte_idx_for_one_based_col(line: &str, col_1based: u32) -> usize {
    if col_1based <= 1 {
        return 0;
    }
    let mut c = 1_u32;
    for (idx, _) in line.char_indices() {
        if c == col_1based {
            return idx;
        }
        c = c.saturating_add(1);
    }
    line.len()
}

fn gutter_width(last_line_no: u32) -> usize {
    let d = last_line_no.max(1).ilog10() as usize + 1;
    d.max(3)
}

fn push_underline_row(
    out: &mut String,
    p: &HumanPalette,
    gutter_w: usize,
    start_col_1based: u32,
    line: &str,
    start_byte: usize,
    end_byte: usize,
) {
    out.push(' ');
    out.push_str(p.dim);
    for _ in 0..gutter_w {
        out.push(' ');
    }
    out.push_str(p.reset);
    out.push(' ');
    out.push_str(p.dim);
    out.push('|');
    out.push_str(p.reset);
    out.push(' ');
    out.push_str(p.red);
    let pad = (start_col_1based.saturating_sub(1)) as usize;
    for _ in 0..pad {
        out.push(' ');
    }
    let end_byte = end_byte.max(start_byte);
    let slice = line.get(start_byte..end_byte).unwrap_or("");
    let n = slice.chars().count().max(1);
    for _ in 0..n {
        out.push('^');
    }
    out.push_str(p.reset);
    out.push('\n');
}

fn push_code_snippet(out: &mut String, p: &HumanPalette, source: &str, range: TextRange) {
    let lines: Vec<&str> = source.lines().collect();
    if lines.is_empty() || range.start_line < 1 {
        return;
    }
    let start_idx = (range.start_line - 1) as usize;
    if start_idx >= lines.len() {
        return;
    }
    let end_idx = (range.end_line.saturating_sub(1) as usize)
        .min(lines.len() - 1)
        .max(start_idx);
    let w = gutter_width((end_idx + 1) as u32);

    out.push_str("  ");
    out.push_str(p.dim);
    for _ in 0..w {
        out.push(' ');
    }
    out.push('|');
    out.push_str(p.reset);
    out.push('\n');

    for (rel_idx, content) in lines[start_idx..=end_idx].iter().enumerate() {
        let line_no = (start_idx + rel_idx + 1) as u32;
        out.push(' ');
        out.push_str(p.dim);
        out.push_str(&format!("{:>width$}", line_no, width = w));
        out.push_str(p.reset);
        out.push(' ');
        out.push_str(p.dim);
        out.push('|');
        out.push_str(p.reset);
        out.push(' ');
        out.push_str(content);
        out.push('\n');
    }

    if range.start_line == range.end_line {
        let line = lines[start_idx];
        let start_b = byte_idx_for_one_based_col(line, range.start_col);
        let end_b = if range.end_col > range.start_col {
            byte_idx_for_one_based_col(line, range.end_col)
        } else {
            (start_b
                + line[start_b..]
                    .chars()
                    .next()
                    .map(|ch| ch.len_utf8())
                    .unwrap_or(1))
            .min(line.len())
        };
        push_underline_row(out, p, w, range.start_col, line, start_b, end_b);
    } else {
        let first = lines[start_idx];
        let start_b = byte_idx_for_one_based_col(first, range.start_col);
        push_underline_row(out, p, w, range.start_col, first, start_b, first.len());
        if end_idx > start_idx {
            let last = lines[end_idx];
            let end_b = byte_idx_for_one_based_col(last, range.end_col.max(1));
            push_underline_row(out, p, w, 1, last, 0, end_b);
        }
    }
}

/// Header line for human output: counts per `rule_id`, grouped by severity (errors, then warnings, then infos). Rule ids sorted lexicographically within each group.
fn format_human_summary_line(diagnostics: &[Diagnostic], p: &HumanPalette) -> String {
    let mut errors: BTreeMap<String, u32> = BTreeMap::new();
    let mut warns: BTreeMap<String, u32> = BTreeMap::new();
    let mut infos: BTreeMap<String, u32> = BTreeMap::new();
    for d in diagnostics {
        match d.severity {
            Severity::Error => *errors.entry(d.rule_id.clone()).or_insert(0) += 1,
            Severity::Warn => *warns.entry(d.rule_id.clone()).or_insert(0) += 1,
            Severity::Info => *infos.entry(d.rule_id.clone()).or_insert(0) += 1,
        }
    }

    fn push_group(
        parts: &mut Vec<String>,
        p: &HumanPalette,
        sev: Severity,
        count: u32,
        rules: &BTreeMap<String, u32>,
    ) {
        if rules.is_empty() {
            return;
        }
        let label = if count == 1 {
            match sev {
                Severity::Error => "error",
                Severity::Warn => "warning",
                Severity::Info => "info",
            }
        } else {
            match sev {
                Severity::Error => "errors",
                Severity::Warn => "warnings",
                Severity::Info => "infos",
            }
        };
        let breakdown: Vec<String> = rules.iter().map(|(id, n)| format!("{id} ({n})")).collect();
        parts.push(format!(
            "{}{} {} — {}{}",
            p.severity_paint(sev),
            count,
            label,
            breakdown.join(", "),
            p.reset
        ));
    }

    let err_n: u32 = errors.values().sum();
    let warn_n: u32 = warns.values().sum();
    let info_n: u32 = infos.values().sum();

    let mut parts = Vec::new();
    push_group(&mut parts, p, Severity::Error, err_n, &errors);
    push_group(&mut parts, p, Severity::Warn, warn_n, &warns);
    push_group(&mut parts, p, Severity::Info, info_n, &infos);

    let mut out = String::new();
    out.push_str(&format!("{}Summary:{} ", p.bold, p.reset));
    out.push_str(&parts.join("; "));
    out.push('\n');
    out
}

pub(crate) fn render_human(
    diagnostics: &[Diagnostic],
    color: ColorChoice,
    sources: Option<&BTreeMap<String, Arc<str>>>,
) -> String {
    let p = HumanPalette::new(color.use_ansi_colors());
    if diagnostics.is_empty() {
        return format!("{}No diagnostics.{}\n", p.dim, p.reset);
    }

    let mut out = String::new();
    out.push_str(&format_human_summary_line(diagnostics, &p));
    out.push('\n');
    for diagnostic in diagnostics {
        let sev_word = match diagnostic.severity {
            Severity::Error => "error",
            Severity::Warn => "warn",
            Severity::Info => "info",
        };
        out.push_str(&format!(
            "{}{}{}{}[{}{}{}]: {}{}\n  {}{}{}{} {}{}{}:{}\n",
            p.severity_paint(diagnostic.severity),
            sev_word,
            p.reset,
            p.bold,
            p.magenta,
            diagnostic.rule_id,
            p.reset,
            p.bold,
            diagnostic.message,
            p.reset,
            p.cyan,
            "-->",
            p.reset,
            p.bold,
            diagnostic.file,
            p.reset,
            format_range(diagnostic.range),
        ));
        if let Some(map) = sources
            && let Some(src) = lookup_source(map, &diagnostic.file)
        {
            push_code_snippet(&mut out, &p, src, diagnostic.range);
        }
        for label in &diagnostic.labels {
            out.push_str(&format!(
                "  {}label{} {}: {}\n",
                p.dim,
                p.reset,
                format_range(label.range),
                label.message
            ));
        }
        if !diagnostic.evidence.is_empty() {
            for evidence in &diagnostic.evidence {
                out.push_str(&format!(
                    "  {}evidence{} {}: {}\n",
                    p.dim, p.reset, evidence.label, evidence.value
                ));
            }
        }
        for suggestion in &diagnostic.suggestions {
            out.push_str(&format!(
                "  {}suggestion:{} {}\n",
                p.dim, p.reset, suggestion.message
            ));
        }
        if let Some(fix) = &diagnostic.fix {
            out.push_str(&format!("  {}fix:{} {}\n", p.yellow, p.reset, fix.message));
            if let Some(replacement) = &fix.replacement {
                out.push_str(&format!(
                    "    {}replacement:{} {}\n",
                    p.green, p.reset, replacement
                ));
            }
        }
        if let Some(help) = &diagnostic.help {
            out.push_str(&format!("  {}help:{} {}\n", p.cyan, p.reset, help));
        }
        out.push_str(&format!(
            "  {}fingerprint: {}{}\n\n",
            p.dim, diagnostic.stable_fingerprint, p.reset
        ));
    }
    out
}

fn format_range(range: TextRange) -> String {
    format!(
        "{}:{}-{}:{}",
        range.start_line, range.start_col, range.end_line, range.end_col
    )
}

pub(crate) fn render_sarif(
    diagnostics: &[Diagnostic],
    rule_help_uri: Option<&BTreeMap<String, String>>,
) -> String {
    #[derive(Serialize)]
    struct SarifLog {
        version: &'static str,
        #[serde(rename = "$schema")]
        schema: &'static str,
        runs: Vec<SarifRun>,
    }

    #[derive(Serialize)]
    struct SarifRun {
        tool: SarifTool,
        results: Vec<SarifResult>,
    }

    #[derive(Serialize)]
    struct SarifTool {
        driver: SarifDriver,
    }

    #[derive(Serialize)]
    struct SarifDriver {
        name: &'static str,
        #[serde(rename = "informationUri")]
        information_uri: &'static str,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        rules: Vec<SarifReportingRule>,
    }

    #[derive(Serialize)]
    struct SarifReportingRule {
        id: String,
        #[serde(rename = "shortDescription")]
        short_description: SarifMessage,
        #[serde(rename = "fullDescription", skip_serializing_if = "Option::is_none")]
        full_description: Option<SarifMessage>,
        #[serde(rename = "helpUri", skip_serializing_if = "Option::is_none")]
        help_uri: Option<String>,
        properties: SarifProperties,
    }

    #[derive(Serialize)]
    struct SarifProperties {
        tags: Vec<String>,
    }

    #[derive(Serialize)]
    struct SarifResult {
        #[serde(rename = "ruleId")]
        rule_id: String,
        level: &'static str,
        message: SarifMessage,
        fingerprints: SarifFingerprints,
        locations: Vec<SarifLocation>,
        #[serde(rename = "relatedLocations", skip_serializing_if = "Vec::is_empty")]
        related_locations: Vec<SarifRelatedLocation>,
        #[serde(rename = "codeFlows", skip_serializing_if = "Vec::is_empty")]
        code_flows: Vec<SarifCodeFlow>,
        #[serde(skip_serializing_if = "Option::is_none")]
        fixes: Option<Vec<SarifFix>>,
    }

    #[derive(Serialize)]
    struct SarifFingerprints {
        #[serde(rename = "polint/v1")]
        polint_v1: String,
    }

    #[derive(Serialize)]
    struct SarifMessage {
        text: String,
    }

    #[derive(Serialize)]
    struct SarifLocation {
        #[serde(rename = "physicalLocation")]
        physical_location: SarifPhysicalLocation,
    }

    #[derive(Serialize)]
    struct SarifRelatedLocation {
        #[serde(rename = "physicalLocation")]
        physical_location: SarifPhysicalLocation,
        message: SarifMessage,
    }

    #[derive(Serialize)]
    struct SarifCodeFlow {
        #[serde(rename = "threadFlows")]
        thread_flows: Vec<SarifThreadFlow>,
    }

    #[derive(Serialize)]
    struct SarifThreadFlow {
        locations: Vec<SarifThreadFlowLocation>,
    }

    #[derive(Serialize)]
    struct SarifThreadFlowLocation {
        location: SarifLocationWithMessage,
    }

    #[derive(Serialize)]
    struct SarifLocationWithMessage {
        #[serde(rename = "physicalLocation")]
        physical_location: SarifPhysicalLocation,
        message: SarifMessage,
    }

    #[derive(Serialize)]
    struct SarifPhysicalLocation {
        #[serde(rename = "artifactLocation")]
        artifact_location: SarifArtifactLocation,
        region: SarifRegion,
    }

    #[derive(Serialize)]
    struct SarifArtifactLocation {
        uri: String,
    }

    #[derive(Serialize)]
    struct SarifRegion {
        #[serde(rename = "startLine")]
        start_line: u32,
        #[serde(rename = "startColumn")]
        start_column: u32,
        #[serde(rename = "endLine")]
        end_line: u32,
        #[serde(rename = "endColumn")]
        end_column: u32,
    }

    #[derive(Serialize)]
    struct SarifFix {
        description: SarifMessage,
        #[serde(rename = "artifactChanges")]
        artifact_changes: Vec<SarifArtifactChange>,
    }

    #[derive(Serialize)]
    struct SarifArtifactChange {
        #[serde(rename = "artifactLocation")]
        artifact_location: SarifArtifactLocation,
        replacements: Vec<SarifReplacement>,
    }

    #[derive(Serialize)]
    struct SarifReplacement {
        #[serde(rename = "deletedRegion")]
        deleted_region: SarifRegion,
        #[serde(rename = "insertedContent")]
        inserted_content: SarifArtifactContent,
    }

    #[derive(Serialize)]
    struct SarifArtifactContent {
        text: String,
    }

    fn region_from(range: TextRange) -> SarifRegion {
        SarifRegion {
            start_line: range.start_line,
            start_column: range.start_col,
            end_line: range.end_line,
            end_column: range.end_col,
        }
    }

    fn physical(uri: &str, range: TextRange) -> SarifPhysicalLocation {
        SarifPhysicalLocation {
            artifact_location: SarifArtifactLocation {
                uri: uri.to_string(),
            },
            region: region_from(range),
        }
    }

    let mut rule_descriptions: BTreeMap<String, String> = BTreeMap::new();
    for diagnostic in diagnostics {
        rule_descriptions
            .entry(diagnostic.rule_id.clone())
            .or_insert_with(|| diagnostic.message.clone());
    }
    let rules: Vec<SarifReportingRule> = rule_descriptions
        .into_iter()
        .map(|(id, text)| {
            let help_uri = rule_help_uri.and_then(|m| m.get(&id).cloned());
            SarifReportingRule {
                id,
                short_description: SarifMessage { text: text.clone() },
                full_description: Some(SarifMessage { text }),
                help_uri,
                properties: SarifProperties {
                    tags: vec!["polint".to_string()],
                },
            }
        })
        .collect();

    let results: Vec<SarifResult> = diagnostics
        .iter()
        .map(|diagnostic| {
            let uri = diagnostic.file.as_str();
            let related_locations: Vec<SarifRelatedLocation> = diagnostic
                .labels
                .iter()
                .map(|label| SarifRelatedLocation {
                    physical_location: physical(uri, label.range),
                    message: SarifMessage {
                        text: label.message.clone(),
                    },
                })
                .collect();

            let fixes = diagnostic.fix.as_ref().and_then(|fix| {
                fix.replacement.as_ref().map(|replacement| {
                    vec![SarifFix {
                        description: SarifMessage {
                            text: fix.message.clone(),
                        },
                        artifact_changes: vec![SarifArtifactChange {
                            artifact_location: SarifArtifactLocation {
                                uri: uri.to_string(),
                            },
                            replacements: vec![SarifReplacement {
                                deleted_region: region_from(diagnostic.range),
                                inserted_content: SarifArtifactContent {
                                    text: replacement.clone(),
                                },
                            }],
                        }],
                    }]
                })
            });
            let code_flows = diagnostic
                .evidence_v1
                .as_ref()
                .map_or_else(Vec::new, |value| {
                    let locations = crate::analysis::evidence::render::sarif_thread_flow_steps(
                        value.as_value(),
                    )
                    .into_iter()
                    .filter_map(|step| {
                        let location = step.location?;
                        Some(SarifThreadFlowLocation {
                            location: SarifLocationWithMessage {
                                physical_location: physical(&location.uri, location.range),
                                message: SarifMessage { text: step.message },
                            },
                        })
                    })
                    .collect::<Vec<_>>();
                    if locations.is_empty() {
                        Vec::new()
                    } else {
                        vec![SarifCodeFlow {
                            thread_flows: vec![SarifThreadFlow { locations }],
                        }]
                    }
                });

            SarifResult {
                rule_id: diagnostic.rule_id.clone(),
                level: match diagnostic.severity {
                    Severity::Info => "note",
                    Severity::Warn => "warning",
                    Severity::Error => "error",
                },
                message: SarifMessage {
                    text: diagnostic.message.clone(),
                },
                fingerprints: SarifFingerprints {
                    polint_v1: diagnostic.stable_fingerprint.clone(),
                },
                locations: vec![SarifLocation {
                    physical_location: physical(uri, diagnostic.range),
                }],
                related_locations,
                code_flows,
                fixes,
            }
        })
        .collect();

    let log = SarifLog {
        version: "2.1.0",
        schema: "https://json.schemastore.org/sarif-2.1.0.json",
        runs: vec![SarifRun {
            tool: SarifTool {
                driver: SarifDriver {
                    name: "polint",
                    information_uri: "https://github.com/emilwareus/polint",
                    rules,
                },
            },
            results,
        }],
    };

    serde_json::to_string_pretty(&log).unwrap_or_else(|_| "{}".to_string())
}

pub(crate) fn fingerprint(parts: &[&str]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in part.as_bytes() {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub(crate) fn diagnostic_fingerprint(
    rule_id: &str,
    file: &str,
    range: TextRange,
    message: &str,
) -> String {
    let start_line = range.start_line.to_string();
    let start_col = range.start_col.to_string();
    let end_line = range.end_line.to_string();
    let end_col = range.end_col.to_string();
    fingerprint(&[
        rule_id,
        file,
        &start_line,
        &start_col,
        &end_line,
        &end_col,
        message,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn test_opts() -> RenderOpts<'static> {
        RenderOpts {
            json: JsonReportMeta {
                tool_name: "polint",
                tool_version: env!("CARGO_PKG_VERSION"),
            },
            color: ColorChoice::Never,
            sources: None,
        }
    }

    fn structured(value: serde_json::Value) -> StructuredEvidenceV1 {
        StructuredEvidenceV1::try_from_value(value).expect("valid structured evidence")
    }

    fn minimal_structured_evidence_value() -> serde_json::Value {
        serde_json::json!({
            "version": 1,
            "bundle": {
                "id": 0,
                "stable_key": "bundle:test",
                "diagnostic_stable_key": "diag:test",
                "status": "Partial",
                "precision": "SetupAware",
                "provenance": "Query",
                "validation": "RendererValidated",
                "confidence": "Medium",
                "replay_key": null
            },
            "paths": [],
            "unknowns": [],
            "omitted_regions": [],
            "limits": {
                "max_paths": 5,
                "max_edges_per_path": 96,
                "max_unknowns": 32,
                "max_omitted_regions": 32,
                "total_paths": 0,
                "rendered_paths": 0,
                "paths_truncated": false,
                "total_unknowns": 0,
                "rendered_unknowns": 0,
                "unknowns_truncated": false,
                "total_omitted_regions": 0,
                "rendered_omitted_regions": 0,
                "omitted_regions_truncated": false
            }
        })
    }

    #[test]
    fn sorting_is_deterministic() {
        let mut diagnostics = vec![
            Diagnostic::warning("b", "b.go", TextRange::point(2, 1), "b"),
            Diagnostic::warning("a", "a.go", TextRange::point(1, 1), "a"),
        ];

        sort_diagnostics(&mut diagnostics);

        assert_eq!(diagnostics[0].file, "a.go");
        assert_eq!(diagnostics[1].file, "b.go");
    }

    #[test]
    fn diagnostic_can_carry_internal_bundle_without_fingerprint_change() {
        let base = Diagnostic::warning("rule", "src/lib.rs", TextRange::point(1, 1), "message");
        let fingerprint = base.stable_fingerprint.clone();

        let diagnostic = base.with_evidence_bundle_ref(crate::analysis::ids::EvidenceBundleId(42));

        assert_eq!(diagnostic.stable_fingerprint, fingerprint);
        assert_eq!(
            diagnostic.evidence_bundle_ref().expect("bundle").id,
            crate::analysis::ids::EvidenceBundleId(42)
        );
    }

    #[test]
    fn structured_evidence_serializes_without_breaking_scalar_evidence() {
        let diagnostic =
            Diagnostic::warning("rule", "src/lib.rs", TextRange::point(1, 1), "message")
                .with_evidence("symbol", "unsafe_api")
                .with_structured_evidence_v1(structured(serde_json::json!({
                    "version": 1,
                    "bundle": {
                        "id": 0,
                        "stable_key": "bundle:test",
                        "diagnostic_stable_key": "diag:test",
                        "status": "Partial",
                        "precision": "SetupAware",
                        "provenance": "Query",
                        "validation": "RendererValidated",
                        "confidence": "Medium",
                        "replay_key": null
                    },
                    "paths": [],
                    "unknowns": [],
                    "omitted_regions": [],
                    "limits": {
                        "max_paths": 5,
                        "max_edges_per_path": 96,
                        "max_unknowns": 32,
                        "max_omitted_regions": 32,
                        "total_paths": 0,
                        "rendered_paths": 0,
                        "paths_truncated": false,
                        "total_unknowns": 0,
                        "rendered_unknowns": 0,
                        "unknowns_truncated": false,
                        "total_omitted_regions": 0,
                        "rendered_omitted_regions": 0,
                        "omitted_regions_truncated": false
                    }
                })));

        let value = serde_json::to_value(&diagnostic).expect("diagnostic serializes");

        assert_eq!(value["evidence"][0]["label"], "symbol");
        assert_eq!(value["evidence_v1"]["version"], 1);
        assert_eq!(
            diagnostic.stable_fingerprint,
            diagnostic_fingerprint("rule", "src/lib.rs", TextRange::point(1, 1), "message")
        );
    }

    #[test]
    fn diagnostic_rejects_arbitrary_structured_evidence_json() {
        let value = serde_json::json!({
            "rule_id": "rule",
            "severity": "warn",
            "file": "src/lib.rs",
            "range": {
                "start_line": 1,
                "start_col": 1,
                "end_line": 1,
                "end_col": 1
            },
            "message": "message",
            "evidence_v1": {
                "version": 1,
                "unbounded": [{"anything": ["goes"]}]
            }
        });

        let result = serde_json::from_value::<Diagnostic>(value);

        assert!(result.is_err());
    }

    #[test]
    fn diagnostic_rejects_untyped_structured_evidence_taxonomy_values() {
        let value = serde_json::json!({
            "version": 1,
            "bundle": {
                "id": 0,
                "stable_key": "bundle:test",
                "diagnostic_stable_key": "diag:test",
                "status": "Pretend",
                "precision": "SetupAware",
                "provenance": "Query",
                "validation": "RendererValidated",
                "confidence": "Medium",
                "replay_key": null
            },
            "paths": [],
            "unknowns": [],
            "omitted_regions": [],
            "limits": {
                "max_paths": 5,
                "max_edges_per_path": 96,
                "max_unknowns": 32,
                "max_omitted_regions": 32,
                "total_paths": 0,
                "rendered_paths": 0,
                "paths_truncated": false,
                "total_unknowns": 0,
                "rendered_unknowns": 0,
                "unknowns_truncated": false,
                "total_omitted_regions": 0,
                "rendered_omitted_regions": 0,
                "omitted_regions_truncated": false
            }
        });

        let err = StructuredEvidenceV1::try_from_value(value).unwrap_err();

        assert!(err.contains("evidence_v1.bundle.status"));
    }

    #[test]
    fn public_json_report_strips_structured_evidence_from_local_rule_output() {
        let diagnostic = Diagnostic::warning(
            "local/rule",
            "src/lib.rs",
            TextRange::point(1, 1),
            "message",
        )
        .with_structured_evidence_v1(structured(minimal_structured_evidence_value()));
        let json = render(OutputFormat::Json, &[diagnostic], test_opts());

        let diagnostics = diagnostics_from_public_json_report(&json).expect("public diagnostics");

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0].evidence_v1.is_none(),
            "external rule-host JSON must not inject internal structured evidence"
        );
    }

    #[test]
    fn structured_evidence_string_limit_counts_characters_like_schema() {
        let mut value = minimal_structured_evidence_value();
        value["bundle"]["stable_key"] = serde_json::Value::String("é".repeat(4096));

        StructuredEvidenceV1::try_from_value(value).expect("4096 unicode chars are schema-valid");
    }

    #[test]
    fn structured_evidence_rejects_inconsistent_rendered_counts() {
        let mut value = minimal_structured_evidence_value();
        value["limits"]["rendered_paths"] = serde_json::json!(1);

        let err = StructuredEvidenceV1::try_from_value(value).unwrap_err();

        assert!(err.contains("paths rendered count"));
    }

    fn contract_diagnostic() -> Diagnostic {
        Diagnostic::error(
            "project/rule",
            "src/lib.rs",
            TextRange {
                start_line: 10,
                start_col: 4,
                end_line: 10,
                end_col: 12,
            },
            "policy failed",
        )
        .with_label(
            TextRange {
                start_line: 11,
                start_col: 2,
                end_line: 11,
                end_col: 8,
            },
            "related expression",
        )
        .with_evidence("symbol", "unsafe_api")
        .with_suggestion("Prefer safe_api here")
        .with_fix("Replace unsafe_api", Some("safe_api()".to_string()))
        .with_help("Use the safe wrapper before crossing this boundary.")
        .with_fingerprint("fingerprint-123")
    }

    #[test]
    fn render_human_summary_line_groups_counts_by_severity_and_rule_id() {
        let d1 = Diagnostic::error("local/a", "a.go", TextRange::point(1, 1), "m");
        let d2 = Diagnostic::error("local/a", "b.go", TextRange::point(1, 1), "m");
        let d3 = Diagnostic::error("parser/go", "c.go", TextRange::point(1, 1), "m");
        let d4 = Diagnostic::warning("local/b", "d.go", TextRange::point(1, 1), "m");
        let d5 = Diagnostic::info("local/c", "e.go", TextRange::point(1, 1), "m");
        let d6 = Diagnostic::info("local/c", "f.go", TextRange::point(1, 1), "m");
        let rendered = render_human(&[d1, d2, d3, d4, d5, d6], ColorChoice::Never, None);
        assert!(
            rendered.starts_with("Summary:"),
            "unexpected prefix: {rendered:?}"
        );
        assert!(rendered.contains("3 errors"));
        assert!(rendered.contains("local/a (2)"));
        assert!(rendered.contains("parser/go (1)"));
        assert!(rendered.contains("1 warning"));
        assert!(rendered.contains("local/b (1)"));
        assert!(rendered.contains("2 infos"));
        assert!(rendered.contains("local/c (2)"));
    }

    #[test]
    fn only_rule_keeps_polint_ignore_health_diagnostics_for_matching_selectors() {
        let diagnostics = vec![
            Diagnostic::warning(
                "polint/unused-ignore",
                "src/a.ts",
                TextRange::point(1, 1),
                "unused",
            )
            .with_evidence("selectors", "local/*"),
            Diagnostic::warning(
                "polint/unused-ignore",
                "src/b.ts",
                TextRange::point(1, 1),
                "unused",
            )
            .with_evidence("selectors", "other/rule"),
            Diagnostic::warning(
                "local/no-todo",
                "src/c.ts",
                TextRange::point(1, 1),
                "finding",
            ),
        ];

        let filtered = apply_report_filters(diagnostics, Some("local/no-todo"));
        let rule_ids = filtered
            .iter()
            .map(|diagnostic| diagnostic.rule_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(rule_ids, ["polint/unused-ignore", "local/no-todo"]);
    }

    #[test]
    fn only_rule_keeps_parser_diagnostics() {
        let diagnostics = vec![
            Diagnostic::error("parser/ts", "src/a.ts", TextRange::point(1, 1), "parse"),
            Diagnostic::warning(
                "local/no-todo",
                "src/b.ts",
                TextRange::point(1, 1),
                "finding",
            ),
        ];

        let filtered = apply_report_filters(diagnostics, Some("local/no-todo"));
        let rule_ids = filtered
            .iter()
            .map(|diagnostic| diagnostic.rule_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(rule_ids, ["parser/ts", "local/no-todo"]);
    }

    #[test]
    fn render_human_always_color_includes_escape_sequences() {
        let diagnostic = Diagnostic::error("r", "f.go", TextRange::point(1, 1), "oops");
        let rendered = render_human(&[diagnostic], ColorChoice::Always, None);
        assert!(
            rendered.contains("\x1b["),
            "expected ANSI SGR sequences: {rendered:?}"
        );
    }

    #[test]
    fn render_human_snapshot_includes_contract_fields() {
        insta::assert_snapshot!(
            render(
                OutputFormat::Human,
                &[contract_diagnostic()],
                test_opts(),
            ),
            @r###"
        Summary: 1 error — project/rule (1)

        error[project/rule]: policy failed
          --> src/lib.rs:10:4-10:12
          label 11:2-11:8: related expression
          evidence symbol: unsafe_api
          suggestion: Prefer safe_api here
          fix: Replace unsafe_api
            replacement: safe_api()
          help: Use the safe wrapper before crossing this boundary.
          fingerprint: fingerprint-123

        "###,
        );
    }

    #[test]
    fn render_human_includes_snippet_when_sources_map_provided() {
        let source_text: String = (1..=12)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut sources = BTreeMap::new();
        sources.insert(
            "src/lib.rs".to_string(),
            Arc::from(source_text.into_boxed_str()),
        );
        let opts = RenderOpts {
            json: JsonReportMeta {
                tool_name: "polint",
                tool_version: env!("CARGO_PKG_VERSION"),
            },
            color: ColorChoice::Never,
            sources: Some(&sources),
        };
        insta::assert_snapshot!(
            render(OutputFormat::Human, &[contract_diagnostic()], opts),
            @r###"
        Summary: 1 error — project/rule (1)

        error[project/rule]: policy failed
          --> src/lib.rs:10:4-10:12
             |
          10 | line 10
             |    ^^^^
          label 11:2-11:8: related expression
          evidence symbol: unsafe_api
          suggestion: Prefer safe_api here
          fix: Replace unsafe_api
            replacement: safe_api()
          help: Use the safe wrapper before crossing this boundary.
          fingerprint: fingerprint-123

        "###,
        );
    }

    #[test]
    fn render_json_snapshot_is_stable() {
        let rendered = render(OutputFormat::Json, &[contract_diagnostic()], test_opts());
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed["version"], 1);
        assert_eq!(
            parsed["schema"].as_str().unwrap(),
            POLINT_REPORT_JSON_SCHEMA_V1_URL
        );
        assert_eq!(parsed["tool"]["name"], "polint");
        assert_eq!(parsed["tool"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(parsed["diagnostics"].as_array().unwrap().len(), 1);

        let normalized = rendered.replace(env!("CARGO_PKG_VERSION"), "<PKG_VERSION>");
        insta::assert_snapshot!(normalized, @r###"
        {
          "version": 1,
          "schema": "https://raw.githubusercontent.com/emilwareus/polint/main/docs/schemas/polint-report-v1.json",
          "tool": {
            "name": "polint",
            "version": "<PKG_VERSION>"
          },
          "diagnostics": [
            {
              "rule_id": "project/rule",
              "severity": "error",
              "file": "src/lib.rs",
              "range": {
                "start_line": 10,
                "start_col": 4,
                "end_line": 10,
                "end_col": 12
              },
              "message": "policy failed",
              "labels": [
                {
                  "range": {
                    "start_line": 11,
                    "start_col": 2,
                    "end_line": 11,
                    "end_col": 8
                  },
                  "message": "related expression"
                }
              ],
              "help": "Use the safe wrapper before crossing this boundary.",
              "evidence": [
                {
                  "label": "symbol",
                  "value": "unsafe_api"
                }
              ],
              "suggestions": [
                {
                  "message": "Prefer safe_api here"
                }
              ],
              "fix": {
                "message": "Replace unsafe_api",
                "replacement": "safe_api()"
              },
              "stable_fingerprint": "fingerprint-123"
            }
          ]
        }
        "###);
    }

    #[test]
    fn ai_friendly_report_counts_rules_and_caps_examples() {
        let mut diagnostics = Vec::new();
        for index in 0..12 {
            diagnostics.push(Diagnostic::warning(
                format!("local/rule-{index:02}"),
                format!("src/{index:02}.ts"),
                TextRange::point(index + 1, 1),
                "policy failed",
            ));
        }
        diagnostics.push(Diagnostic::error(
            "local/rule-05",
            "src/error.ts",
            TextRange::point(1, 1),
            "more important",
        ));
        sort_diagnostics(&mut diagnostics);

        let persisted = diagnostics[..3].to_vec();
        let report = build_ai_friendly_report(&diagnostics, &persisted, test_opts().json, "123456");

        assert_eq!(report.version, 1);
        assert_eq!(report.schema, POLINT_AI_FRIENDLY_JSON_SCHEMA_V1_URL);
        assert_eq!(report.summary.total_diagnostics, 13);
        assert_eq!(report.summary.rules_triggered, 12);
        assert_eq!(report.summary.by_severity["error"], 1);
        assert_eq!(report.summary.by_severity["warn"], 12);
        assert_eq!(report.summary.by_rule[0].rule_id, "local/rule-05");
        assert_eq!(report.summary.by_rule[0].total, 2);
        assert_eq!(report.examples.len(), AI_FRIENDLY_EXAMPLE_LIMIT);
        assert_eq!(report.examples[0].message, "more important");
        assert!(report.examples[0].labels.is_empty());
        assert!(report.examples[0].evidence.is_empty());
        assert_eq!(report.diagnostics.len(), 3);
        assert_eq!(report.truncation.as_ref().unwrap().total_diagnostics, 13);
    }

    #[test]
    fn render_ai_friendly_stdout_is_compact_and_query_oriented() {
        let diagnostics = vec![contract_diagnostic()];
        let report =
            build_ai_friendly_report(&diagnostics, &diagnostics, test_opts().json, "123456");
        let rendered = render_ai_friendly_stdout(&report, ".polint/output/latest.json");

        assert!(rendered.contains("polint: 1 diagnostic across 1 rule"));
        assert!(rendered.contains("Full JSON: .polint/output/latest.json"));
        assert!(rendered.contains("error[project/rule]: policy failed"));
        assert!(rendered.contains("--> src/lib.rs:10:4-10:12"));
        assert!(rendered.contains("label 11:2-11:8: related expression"));
        assert!(rendered.contains("evidence symbol: unsafe_api"));
        assert!(rendered.contains("suggestion: Prefer safe_api here"));
        assert!(rendered.contains("fix: Replace unsafe_api"));
        assert!(rendered.contains("help: Use the safe wrapper before crossing this boundary."));
        assert!(rendered.contains("jq '.summary.by_rule' .polint/output/latest.json"));
        assert!(
            rendered.contains(
                "jq '[.diagnostics[] | select(.rule_id==\"project/rule\")][0:20]' .polint/output/latest.json"
            ),
            "{rendered}"
        );
        assert!(
            rendered.contains(
                "jq '.diagnostics[] | select(.file==\"src/lib.rs\") | {rule_id, range, message}' .polint/output/latest.json | head -c 12000"
            ),
            "{rendered}"
        );
        assert!(rendered.contains("Do not read the whole file into an AI prompt"));
        assert!(!rendered.contains("stable_fingerprint"));
    }

    #[test]
    fn render_github_uses_workflow_annotations() {
        let diagnostic = Diagnostic::warning(
            "project/rule,one",
            "src/lib,one.rs",
            TextRange {
                start_line: 10,
                start_col: 4,
                end_line: 10,
                end_col: 12,
            },
            "policy % failed\nuse safe_api",
        );

        insta::assert_snapshot!(
            render(OutputFormat::Github, &[diagnostic], test_opts()),
            @"::warning file=src/lib%2Cone.rs,line=10,col=4,endLine=10,endColumn=12,title=project/rule%2Cone::policy %25 failed%0Ause safe_api
        "
        );
    }

    #[test]
    fn diagnostics_from_json_report_round_trips() {
        let input = vec![contract_diagnostic()];
        let json = render(OutputFormat::Json, &input, test_opts());
        let output = diagnostics_from_json_report(&json).unwrap();
        assert_eq!(output, input);
    }

    #[test]
    fn polint_report_json_schema_file_tracks_repo() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/schemas/polint-report-v1.json");
        let raw = std::fs::read_to_string(&path).expect("polint-report schema should exist");
        let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(
            value["$id"].as_str().unwrap(),
            POLINT_REPORT_JSON_SCHEMA_V1_URL
        );
        assert!(
            value["$defs"]["diagnostic"]["properties"]
                .get("evidence_v1")
                .is_some()
        );
    }

    #[test]
    fn render_sarif_includes_rule_help_uri_from_map() {
        let mut help = BTreeMap::new();
        help.insert(
            "project/rule".to_string(),
            "https://example.invalid/rules/project-rule".to_string(),
        );
        let rendered = render_with_sarif_help(
            OutputFormat::Sarif,
            &[contract_diagnostic()],
            RenderOpts {
                json: JsonReportMeta {
                    tool_name: "polint",
                    tool_version: env!("CARGO_PKG_VERSION"),
                },
                color: ColorChoice::Never,
                sources: None,
            },
            Some(&help),
        );
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        assert_eq!(
            parsed
                .pointer("/runs/0/tool/driver/rules/0/helpUri")
                .unwrap(),
            "https://example.invalid/rules/project-rule"
        );
        assert_eq!(
            parsed
                .pointer("/runs/0/tool/driver/rules/0/properties/tags/0")
                .unwrap(),
            "polint"
        );
        assert!(parsed.pointer("/runs/0/tool/driver/rules/0/tags").is_none());
    }

    #[test]
    fn render_sarif_projects_structured_evidence_to_code_flows() {
        let diagnostic = Diagnostic::warning(
            "project/rule",
            "src/lib.rs",
            TextRange::point(7, 3),
            "policy failed",
        )
        .with_structured_evidence_v1(structured(serde_json::json!({
            "version": 1,
            "bundle": {
                "id": 0,
                "stable_key": "bundle:diag",
                "diagnostic_stable_key": "diag:1",
                "status": "Partial",
                "precision": "SetupAware",
                "provenance": "Query",
                "validation": "RendererValidated",
                "confidence": "Medium",
                "replay_key": "replay:bundle"
            },
            "paths": [{
                "id": 0,
                "stable_key": "path:summary",
                "rank": 0,
                "status": "Partial",
                "hidden_node_count": 0,
                "nodes": [0, 1],
                "edges": [{
                    "id": 0,
                    "stable_key": "edge:summary",
                    "kind": "Summary",
                    "status": "Partial",
                    "precision": "SetupAware",
                    "provenance": "Summary",
                    "validation": "ReferentiallyValidated",
                    "confidence": "Medium",
                    "summary_stable_key": "summary:tito",
                    "expansion": {"state": "opaque", "reason": "summary_status=Unknown"},
                    "location": {
                        "uri": "src/evidence.rs",
                        "range": {
                            "start_line": 3,
                            "start_col": 5,
                            "end_line": 3,
                            "end_col": 11
                        }
                    }
                }],
                "omitted_regions": [],
                "total_edges": 1,
                "rendered_edges": 1,
                "edges_truncated": false
            }],
            "unknowns": [{
                "stable_key": "unknown:summary",
                "reason": "OpaqueSummary",
                "message": "opaque summary",
                "edge": 0
            }],
            "omitted_regions": [{
                "id": 0,
                "stable_key": "omitted:compact",
                "reason": "CompactRendering",
                "hidden_node_count": 1,
                "hidden_edge_count": 0,
                "budget_label": "test"
            }],
            "limits": {
                "max_paths": 5,
                "max_edges_per_path": 96,
                "max_unknowns": 32,
                "max_omitted_regions": 32,
                "total_paths": 1,
                "rendered_paths": 1,
                "paths_truncated": false,
                "total_unknowns": 1,
                "rendered_unknowns": 1,
                "unknowns_truncated": false,
                "total_omitted_regions": 1,
                "rendered_omitted_regions": 1,
                "omitted_regions_truncated": false
            }
        })));

        let rendered = render_sarif(&[diagnostic], None);
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
        let code_flows = &parsed["runs"][0]["results"][0]["codeFlows"];

        assert_eq!(code_flows.as_array().expect("code flows").len(), 1);
        let text = serde_json::to_string(code_flows).unwrap();
        assert!(text.contains("Partial"));
        assert!(text.contains("unknown"));
        assert!(text.contains("omitted"));
    }

    #[test]
    fn render_sarif_omits_code_flows_without_evidence_step_locations() {
        let diagnostic = Diagnostic::warning(
            "project/rule",
            "src/lib.rs",
            TextRange::point(7, 3),
            "policy failed",
        )
        .with_structured_evidence_v1(structured(serde_json::json!({
            "version": 1,
            "bundle": {
                "id": 0,
                "stable_key": "bundle:diag",
                "diagnostic_stable_key": "diag:1",
                "status": "Partial",
                "precision": "SetupAware",
                "provenance": "Query",
                "validation": "RendererValidated",
                "confidence": "Medium",
                "replay_key": "replay:bundle"
            },
            "paths": [{
                "id": 0,
                "stable_key": "path:summary",
                "rank": 0,
                "status": "Partial",
                "hidden_node_count": 0,
                "nodes": [0, 1],
                "edges": [{
                    "id": 0,
                    "stable_key": "edge:summary",
                    "kind": "Summary",
                    "status": "Partial",
                    "precision": "SetupAware",
                    "provenance": "Summary",
                    "validation": "ReferentiallyValidated",
                    "confidence": "Medium",
                    "summary_stable_key": "summary:tito",
                    "expansion": {"state": "opaque", "reason": "summary_status=Unknown"}
                }],
                "omitted_regions": [],
                "total_edges": 1,
                "rendered_edges": 1,
                "edges_truncated": false
            }],
            "unknowns": [],
            "omitted_regions": [],
            "limits": {
                "max_paths": 5,
                "max_edges_per_path": 96,
                "max_unknowns": 32,
                "max_omitted_regions": 32,
                "total_paths": 1,
                "rendered_paths": 1,
                "paths_truncated": false,
                "total_unknowns": 0,
                "rendered_unknowns": 0,
                "unknowns_truncated": false,
                "total_omitted_regions": 0,
                "rendered_omitted_regions": 0,
                "omitted_regions_truncated": false
            }
        })));

        let rendered = render_sarif(&[diagnostic], None);
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();

        assert!(parsed.pointer("/runs/0/results/0/codeFlows").is_none());
    }

    #[test]
    fn render_sarif_uses_evidence_step_locations_when_available() {
        let diagnostic = Diagnostic::warning(
            "project/rule",
            "src/diagnostic.rs",
            TextRange::point(7, 3),
            "policy failed",
        )
        .with_structured_evidence_v1(structured(serde_json::json!({
            "version": 1,
            "bundle": {
                "id": 0,
                "stable_key": "bundle:diag",
                "diagnostic_stable_key": "diag:1",
                "status": "Partial",
                "precision": "SetupAware",
                "provenance": "Query",
                "validation": "RendererValidated",
                "confidence": "Medium",
                "replay_key": "replay:bundle"
            },
            "paths": [{
                "id": 0,
                "stable_key": "path:summary",
                "rank": 0,
                "status": "Partial",
                "hidden_node_count": 0,
                "nodes": [0, 1],
                "edges": [{
                    "id": 0,
                    "stable_key": "edge:summary",
                    "kind": "Summary",
                    "status": "Partial",
                    "precision": "SetupAware",
                    "provenance": "Summary",
                    "validation": "ReferentiallyValidated",
                    "confidence": "Medium",
                    "summary_stable_key": "summary:tito",
                    "expansion": {"state": "opaque", "reason": "summary_status=Unknown"},
                    "location": {
                        "uri": "src/evidence.rs",
                        "range": {
                            "start_line": 3,
                            "start_col": 5,
                            "end_line": 3,
                            "end_col": 11
                        }
                    }
                }],
                "omitted_regions": [],
                "total_edges": 1,
                "rendered_edges": 1,
                "edges_truncated": false
            }],
            "unknowns": [],
            "omitted_regions": [],
            "limits": {
                "max_paths": 5,
                "max_edges_per_path": 96,
                "max_unknowns": 32,
                "max_omitted_regions": 32,
                "total_paths": 1,
                "rendered_paths": 1,
                "paths_truncated": false,
                "total_unknowns": 0,
                "rendered_unknowns": 0,
                "unknowns_truncated": false,
                "total_omitted_regions": 0,
                "rendered_omitted_regions": 0,
                "omitted_regions_truncated": false
            }
        })));

        let rendered = render_sarif(&[diagnostic], None);
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(
            parsed.pointer("/runs/0/results/0/codeFlows/0/threadFlows/0/locations/0/location/physicalLocation/artifactLocation/uri"),
            Some(&serde_json::json!("src/evidence.rs"))
        );
        assert_eq!(
            parsed.pointer("/runs/0/results/0/codeFlows/0/threadFlows/0/locations/0/location/physicalLocation/region/startLine"),
            Some(&serde_json::json!(3))
        );
        assert_eq!(
            parsed
                .pointer(
                    "/runs/0/results/0/codeFlows/0/threadFlows/0/locations/0/location/message/text"
                )
                .and_then(serde_json::Value::as_str)
                .map(|message| message.contains("confidence=Medium")),
            Some(true)
        );
    }

    #[test]
    fn render_sarif_snapshot_includes_ci_fields() {
        let rendered = render(OutputFormat::Sarif, &[contract_diagnostic()], test_opts());
        let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();

        assert_eq!(parsed.pointer("/version").unwrap(), "2.1.0");
        assert_eq!(
            parsed.pointer("/runs/0/tool/driver/name").unwrap(),
            "polint"
        );
        assert_eq!(
            parsed.pointer("/runs/0/tool/driver/rules/0/id").unwrap(),
            "project/rule"
        );
        assert_eq!(
            parsed.pointer("/runs/0/results/0/ruleId").unwrap(),
            "project/rule"
        );
        assert_eq!(parsed.pointer("/runs/0/results/0/level").unwrap(), "error");
        assert_eq!(
            parsed.pointer("/runs/0/results/0/message/text").unwrap(),
            "policy failed"
        );
        assert_eq!(
            parsed
                .pointer("/runs/0/results/0/fingerprints/polint~1v1")
                .unwrap(),
            "fingerprint-123"
        );
        assert_eq!(
            parsed
                .pointer("/runs/0/results/0/locations/0/physicalLocation/artifactLocation/uri")
                .unwrap(),
            "src/lib.rs"
        );
        assert_eq!(
            parsed
                .pointer("/runs/0/results/0/relatedLocations/0/physicalLocation/region/startLine",)
                .and_then(|v| v.as_u64()),
            Some(11)
        );
        assert!(parsed
            .pointer("/runs/0/results/0/fixes/0/artifactChanges/0/replacements/0/insertedContent/text")
            .is_some());

        insta::assert_snapshot!(rendered, @r###"
        {
          "version": "2.1.0",
          "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
          "runs": [
            {
              "tool": {
                "driver": {
                  "name": "polint",
                  "informationUri": "https://github.com/emilwareus/polint",
                  "rules": [
                    {
                      "id": "project/rule",
                      "shortDescription": {
                        "text": "policy failed"
                      },
                      "fullDescription": {
                        "text": "policy failed"
                      },
                      "properties": {
                        "tags": [
                          "polint"
                        ]
                      }
                    }
                  ]
                }
              },
              "results": [
                {
                  "ruleId": "project/rule",
                  "level": "error",
                  "message": {
                    "text": "policy failed"
                  },
                  "fingerprints": {
                    "polint/v1": "fingerprint-123"
                  },
                  "locations": [
                    {
                      "physicalLocation": {
                        "artifactLocation": {
                          "uri": "src/lib.rs"
                        },
                        "region": {
                          "startLine": 10,
                          "startColumn": 4,
                          "endLine": 10,
                          "endColumn": 12
                        }
                      }
                    }
                  ],
                  "relatedLocations": [
                    {
                      "physicalLocation": {
                        "artifactLocation": {
                          "uri": "src/lib.rs"
                        },
                        "region": {
                          "startLine": 11,
                          "startColumn": 2,
                          "endLine": 11,
                          "endColumn": 8
                        }
                      },
                      "message": {
                        "text": "related expression"
                      }
                    }
                  ],
                  "fixes": [
                    {
                      "description": {
                        "text": "Replace unsafe_api"
                      },
                      "artifactChanges": [
                        {
                          "artifactLocation": {
                            "uri": "src/lib.rs"
                          },
                          "replacements": [
                            {
                              "deletedRegion": {
                                "startLine": 10,
                                "startColumn": 4,
                                "endLine": 10,
                                "endColumn": 12
                              },
                              "insertedContent": {
                                "text": "safe_api()"
                              }
                            }
                          ]
                        }
                      ]
                    }
                  ]
                }
              ]
            }
          ]
        }
        "###);
    }

    #[test]
    fn diagnostic_deserializes_missing_phase3_fields_with_computed_fingerprint() {
        let diagnostic: Diagnostic = serde_json::from_str(
            r#"{
                "rule_id": "project/rule",
                "severity": "warn",
                "file": "src/lib.rs",
                "range": {
                    "start_line": 4,
                    "start_col": 2,
                    "end_line": 4,
                    "end_col": 9
                },
                "message": "policy failed"
            }"#,
        )
        .unwrap();

        let range = TextRange {
            start_line: 4,
            start_col: 2,
            end_line: 4,
            end_col: 9,
        };

        assert_eq!(diagnostic.rule_id, "project/rule");
        assert_eq!(diagnostic.severity, Severity::Warn);
        assert_eq!(diagnostic.file, "src/lib.rs");
        assert_eq!(diagnostic.range, range);
        assert_eq!(diagnostic.message, "policy failed");
        assert!(diagnostic.labels.is_empty());
        assert!(diagnostic.help.is_none());
        assert!(diagnostic.evidence.is_empty());
        assert!(diagnostic.suggestions.is_empty());
        assert!(diagnostic.fix.is_none());
        assert_eq!(
            diagnostic.stable_fingerprint,
            diagnostic_fingerprint("project/rule", "src/lib.rs", range, "policy failed")
        );
    }

    #[test]
    fn render_empty_human_output_is_stable() {
        assert_eq!(
            render(OutputFormat::Human, &[], test_opts()),
            "No diagnostics.\n"
        );
    }

    #[test]
    fn render_empty_json_report_is_stable() {
        let rendered = render(OutputFormat::Json, &[], test_opts());
        let parsed: PolintReport = serde_json::from_str(&rendered).unwrap();
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.tool.name, "polint");
        assert!(parsed.diagnostics.is_empty());
    }

    #[test]
    fn diagnostic_builders_cover_labels_suggestions_fixes_evidence_and_help() {
        let range = TextRange {
            start_line: 10,
            start_col: 4,
            end_line: 10,
            end_col: 12,
        };
        let label_range = TextRange {
            start_line: 11,
            start_col: 2,
            end_line: 11,
            end_col: 8,
        };

        let diagnostic = Diagnostic::error("project/rule", "src/lib.rs", range, "policy failed")
            .with_label(label_range, "related expression")
            .with_evidence("symbol", "unsafe_api")
            .with_suggestion("Prefer safe_api here")
            .with_fix("Replace unsafe_api", Some("safe_api".to_string()))
            .with_help("Use the safe wrapper before crossing this boundary.");

        assert_eq!(diagnostic.rule_id, "project/rule");
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.file, "src/lib.rs");
        assert_eq!(diagnostic.range, range);
        assert_eq!(diagnostic.message, "policy failed");
        assert_eq!(
            diagnostic.labels,
            vec![Label {
                range: label_range,
                message: "related expression".to_string()
            }]
        );
        assert_eq!(
            diagnostic.evidence,
            vec![Evidence {
                label: "symbol".to_string(),
                value: "unsafe_api".to_string()
            }]
        );
        assert_eq!(
            diagnostic.suggestions,
            vec![Suggestion {
                message: "Prefer safe_api here".to_string()
            }]
        );
        assert_eq!(
            diagnostic.fix,
            Some(Fix {
                message: "Replace unsafe_api".to_string(),
                replacement: Some("safe_api".to_string())
            })
        );
        assert_eq!(
            diagnostic.help,
            Some("Use the safe wrapper before crossing this boundary.".to_string())
        );
        assert!(!diagnostic.stable_fingerprint.is_empty());
    }

    #[test]
    fn fingerprint_includes_rule_file_full_range_and_message() {
        let range = TextRange {
            start_line: 4,
            start_col: 2,
            end_line: 4,
            end_col: 9,
        };
        let baseline = Diagnostic::warning("project/rule", "src/lib.rs", range, "policy failed")
            .stable_fingerprint;

        let changed_start_line = Diagnostic::warning(
            "project/rule",
            "src/lib.rs",
            TextRange {
                start_line: 5,
                ..range
            },
            "policy failed",
        )
        .stable_fingerprint;
        let changed_start_col = Diagnostic::warning(
            "project/rule",
            "src/lib.rs",
            TextRange {
                start_col: 3,
                ..range
            },
            "policy failed",
        )
        .stable_fingerprint;
        let changed_end_line = Diagnostic::warning(
            "project/rule",
            "src/lib.rs",
            TextRange {
                end_line: 5,
                ..range
            },
            "policy failed",
        )
        .stable_fingerprint;
        let changed_end_col = Diagnostic::warning(
            "project/rule",
            "src/lib.rs",
            TextRange {
                end_col: 10,
                ..range
            },
            "policy failed",
        )
        .stable_fingerprint;
        let changed_file =
            Diagnostic::warning("project/rule", "src/main.rs", range, "policy failed")
                .stable_fingerprint;
        let changed_rule =
            Diagnostic::warning("project/other", "src/lib.rs", range, "policy failed")
                .stable_fingerprint;
        let changed_message =
            Diagnostic::warning("project/rule", "src/lib.rs", range, "different message")
                .stable_fingerprint;

        for changed in [
            changed_start_line,
            changed_start_col,
            changed_end_line,
            changed_end_col,
            changed_file,
            changed_rule,
            changed_message,
        ] {
            assert_ne!(baseline, changed);
        }

        let changed_non_identity_fields =
            Diagnostic::error("project/rule", "src/lib.rs", range, "policy failed")
                .with_label(range, "related")
                .with_evidence("name", "value")
                .with_suggestion("try another expression")
                .with_fix("replace it", Some("replacement".to_string()))
                .with_help("extra context");

        assert_eq!(baseline, changed_non_identity_fields.stable_fingerprint);
    }

    #[test]
    fn dedupe_diagnostics_collapses_same_fingerprint_after_sorting() {
        let duplicate = Diagnostic::warning("project/rule", "b.go", TextRange::point(2, 1), "b");
        let unique = Diagnostic::warning("project/rule", "a.go", TextRange::point(1, 1), "a");

        let diagnostics = dedupe_diagnostics(vec![duplicate.clone(), unique.clone(), duplicate]);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].stable_fingerprint, unique.stable_fingerprint);
        assert_eq!(diagnostics[1].file, "b.go");
    }

    #[test]
    fn dedupe_diagnostics_removes_non_adjacent_duplicate_fingerprints() {
        let first =
            Diagnostic::warning("rule/a", "a.go", TextRange::point(1, 1), "first duplicate")
                .with_fingerprint("same-fingerprint");
        let unique = Diagnostic::warning("rule/b", "b.go", TextRange::point(1, 1), "unique")
            .with_fingerprint("unique-fingerprint");
        let second =
            Diagnostic::warning("rule/c", "c.go", TextRange::point(1, 1), "second duplicate")
                .with_fingerprint("same-fingerprint");

        let diagnostics = dedupe_diagnostics(vec![second, unique.clone(), first.clone()]);

        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0], first);
        assert_eq!(diagnostics[1], unique);
    }

    fn diagnostic_from_index(index: usize) -> Diagnostic {
        let file = format!("src/{:02}.rs", index % 5);
        let rule_id = format!("rule/{:02}", index % 7);
        let line = (index % 11) as u32 + 1;
        let col = (index % 13) as u32 + 1;
        Diagnostic::warning(
            rule_id,
            file,
            TextRange {
                start_line: line,
                start_col: col,
                end_line: line + ((index % 3) as u32),
                end_col: col + ((index % 5) as u32),
            },
            format!("message {index:02}"),
        )
    }

    fn sorted_keys(
        diagnostics: &mut [Diagnostic],
    ) -> Vec<(String, u32, u32, String, String, String)> {
        sort_diagnostics(diagnostics);
        diagnostics
            .iter()
            .map(|diagnostic| {
                (
                    diagnostic.file.clone(),
                    diagnostic.range.start_line,
                    diagnostic.range.start_col,
                    diagnostic.rule_id.clone(),
                    diagnostic.message.clone(),
                    diagnostic.stable_fingerprint.clone(),
                )
            })
            .collect()
    }

    proptest! {
        #[test]
        fn sort_diagnostics_is_input_order_independent(order in proptest::collection::vec(0usize..30, 1..80)) {
            let mut first: Vec<_> = order.iter().copied().map(diagnostic_from_index).collect();
            let mut second: Vec<_> = order.iter().rev().copied().map(diagnostic_from_index).collect();

            prop_assert_eq!(sorted_keys(&mut first), sorted_keys(&mut second));
        }
    }
}
