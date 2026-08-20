//! Core diagnostic types shared across the workspace.
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::fmt;

const STRUCTURED_EVIDENCE_MAX_PATHS: usize = 64;
const STRUCTURED_EVIDENCE_MAX_EDGES_PER_PATH: usize = 512;
const STRUCTURED_EVIDENCE_MAX_UNKNOWNS: usize = 512;
const STRUCTURED_EVIDENCE_MAX_OMITTED_REGIONS: usize = 512;
const STRUCTURED_EVIDENCE_MAX_NODES_PER_PATH: usize = 4096;
const STRUCTURED_EVIDENCE_MAX_STRING_CHARS: usize = 4096;
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
#[non_exhaustive]
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
// Deliberately NOT `#[non_exhaustive]`: rule packs legitimately construct these when they
// compute their own ranges, and every constructor here takes all fields positionally, so
// the attribute would forbid struct literals without buying any room to add a field.
pub struct TextRange {
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl TextRange {
    pub fn new(start_line: u32, start_col: u32, end_line: u32, end_col: u32) -> Self {
        Self {
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    pub fn point(line: u32, col: u32) -> Self {
        Self::new(line, col, line, col)
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

impl Evidence {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

/// Opaque evidence-bundle identity (foundational; not an analysis fact).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EvidenceBundleId(pub u64);

impl EvidenceBundleId {
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct StructuredEvidenceV1 {
    value: serde_json::Value,
}

impl StructuredEvidenceV1 {
    /// Validates `value` against the `evidence_v1` schema and wraps it.
    pub fn try_from_value(value: serde_json::Value) -> Result<Self, String> {
        validate_structured_evidence_v1(&value)?;
        Ok(Self { value })
    }

    /// Borrows the validated JSON value.
    pub fn as_value(&self) -> &serde_json::Value {
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
    /// Bounded structured provenance envelope when produced by policy queries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_v1: Option<StructuredEvidenceV1>,
    #[serde(skip)]
    pub evidence_bundle: Option<EvidenceBundleRef>,
    pub suggestions: Vec<Suggestion>,
    pub fix: Option<Fix>,
    pub stable_fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceBundleRef {
    pub id: EvidenceBundleId,
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
    pub fn with_evidence_bundle_ref(mut self, id: EvidenceBundleId) -> Self {
        self.evidence_bundle = Some(EvidenceBundleRef { id });
        self
    }

    pub fn with_structured_evidence_v1(mut self, value: StructuredEvidenceV1) -> Self {
        self.evidence_v1 = Some(value);
        self
    }

    #[allow(
        dead_code,
        reason = "Internal bundle refs are read by current render plumbing; tests verify the side-table bridge without making it public SDK."
    )]
    pub fn evidence_bundle_ref(&self) -> Option<EvidenceBundleRef> {
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

    /// Full constructor for host/wire conversion (fields are otherwise `non_exhaustive`).
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        rule_id: impl Into<String>,
        severity: Severity,
        file: impl Into<String>,
        range: TextRange,
        message: impl Into<String>,
        labels: Vec<Label>,
        help: Option<String>,
        evidence: Vec<Evidence>,
        evidence_v1: Option<StructuredEvidenceV1>,
        evidence_bundle: Option<EvidenceBundleRef>,
        suggestions: Vec<Suggestion>,
        fix: Option<Fix>,
        stable_fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            rule_id: rule_id.into(),
            severity,
            file: file.into(),
            range,
            message: message.into(),
            labels,
            help,
            evidence,
            evidence_v1,
            evidence_bundle,
            suggestions,
            fix,
            stable_fingerprint: stable_fingerprint.into(),
        }
    }
}

pub fn fingerprint(parts: &[&str]) -> String {
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

pub fn diagnostic_fingerprint(
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

impl Label {
    /// Constructs a diagnostic label.
    pub fn new(range: TextRange, message: String) -> Self {
        Self { range, message }
    }
}

impl Suggestion {
    /// Constructs a diagnostic suggestion.
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

impl Fix {
    /// Constructs a diagnostic fix.
    pub fn new(message: String, replacement: Option<String>) -> Self {
        Self {
            message,
            replacement,
        }
    }
}
