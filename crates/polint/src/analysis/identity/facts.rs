use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::analysis::ids::{CallSiteId, CallTargetId};
use crate::core::{FileId, Span};

/// Dense run-local handle for an identity record.
///
/// Per D-01 this newtype lives here, alongside the records it identifies, rather
/// than in `analysis::ids` which is reserved for the legacy raw-integer IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub(crate) struct IdentityRecordId(pub(crate) u64);

/// Closed taxonomy of identity record kinds (D-02).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum IdentityKind {
    Function,
    Callsite,
}

/// Closed, serde-stable language tag for identity facts.
///
/// Mirrors the supported `core::Language` analysis surface but is typed
/// specifically for identity facts so the digest payloads never depend on the
/// broader `Language` enum's `Unknown`/`Tsx`/`Jsx` shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LanguageTag {
    Go,
    TypeScript,
    JavaScript,
}

impl LanguageTag {
    /// Stable lowercase label used in digest payloads and stable keys.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Go => "go",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
        }
    }
}

/// Length-prefixed two-pass FNV-1a 16-byte signature digest (D-03).
///
/// FNV-1a was the deliberate no-new-dependency choice (T-42-SC); each field
/// component is length-prefixed before hashing so the digest is deterministic and
/// cross-platform byte-identical. Rendered as 32 lowercase hex characters through
/// serde. The byte array is the canonical form; hex is only the wire/serde
/// representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct SignatureDigest(pub(crate) [u8; 16]);

impl Serialize for SignatureDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&encode_hex(&self.0))
    }
}

impl<'de> Deserialize<'de> for SignatureDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        let bytes = decode_hex(&hex).map_err(serde::de::Error::custom)?;
        Ok(Self(bytes))
    }
}

/// One identity record for a function or callsite (D-02, D-04).
///
/// Records reference the v1.2 `analysis::calls` IDs by composition through
/// `originating_call_site_id` / `originating_call_target_id`; the call facts are
/// never mutated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct IdentityRecord {
    pub(crate) id: IdentityRecordId,
    pub(crate) kind: IdentityKind,
    pub(crate) file_id: FileId,
    pub(crate) span: Span,
    pub(crate) language: LanguageTag,
    #[serde(with = "arc_str_serde")]
    pub(crate) package_or_module: Arc<str>,
    #[serde(with = "arc_str_serde")]
    pub(crate) container_path: Arc<str>,
    #[serde(with = "arc_str_serde")]
    pub(crate) display_name: Arc<str>,
    pub(crate) signature_digest: SignatureDigest,
    pub(crate) multiplicity: u32,
    pub(crate) stable_key: String,
    pub(crate) originating_call_site_id: Option<CallSiteId>,
    pub(crate) originating_call_target_id: Option<CallTargetId>,
}

impl IdentityRecord {
    /// Returns a clone with `multiplicity` overwritten — used by dedup so the
    /// canonical record keeps every field except the merge counter (D-10).
    pub(crate) fn clone_with_multiplicity(&self, multiplicity: u32) -> Self {
        Self {
            multiplicity,
            ..self.clone()
        }
    }
}

/// Serde adapter so `Arc<str>` fields round-trip through plain JSON strings
/// without requiring the workspace `serde` `rc` feature (which is not enabled).
mod arc_str_serde {
    use std::sync::Arc;

    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(value: &Arc<str>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(value)
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<Arc<str>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(Arc::from(value.as_str()))
    }
}

/// Computes the 16-byte signature digest from the identity's semantic fields
/// (D-03).
///
/// Every component is length-prefixed before hashing so that distinct field
/// tuples can never collide by sliding a separator (T-42-01). The hash is a pure
/// integer computation so the output bytes are byte-identical across platforms
/// (D-25).
pub(crate) fn compute_signature_digest(
    language: LanguageTag,
    package_or_module: &str,
    container_path: &str,
    display_name: &str,
    parameter_shape: Option<&str>,
    return_shape: Option<&str>,
) -> SignatureDigest {
    let mut input = Vec::new();
    push_length_prefixed(&mut input, language.as_str().as_bytes());
    push_length_prefixed(&mut input, package_or_module.as_bytes());
    push_length_prefixed(&mut input, container_path.as_bytes());
    push_length_prefixed(&mut input, display_name.as_bytes());
    push_optional(&mut input, parameter_shape);
    push_optional(&mut input, return_shape);
    SignatureDigest(digest_16(&input))
}

/// Builds the deterministic, boundary-disambiguated stable key for an identity
/// record. Mirrors the `analysis::calls::*::stable_key` shape: single line, no
/// whitespace, explicit `|` separators.
pub(crate) fn compute_identity_stable_key(
    kind: IdentityKind,
    language: LanguageTag,
    package_or_module: &str,
    container_path: &str,
    file_id: FileId,
    span: &Span,
) -> String {
    format!(
        "identity|{}|{}|{}|{}|{}|{}..{}",
        identity_kind_label(kind),
        language.as_str(),
        escape_field(package_or_module),
        escape_field(container_path),
        file_id.0,
        span.start_byte,
        span.end_byte,
    )
}

fn identity_kind_label(kind: IdentityKind) -> &'static str {
    match kind {
        IdentityKind::Function => "function",
        IdentityKind::Callsite => "callsite",
    }
}

/// Escapes the `|` separator so a value containing the separator cannot forge a
/// different field boundary.
fn escape_field(value: &str) -> String {
    value.replace('\\', "\\\\").replace('|', "\\|")
}

fn push_length_prefixed(buffer: &mut Vec<u8>, bytes: &[u8]) {
    buffer.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    buffer.extend_from_slice(bytes);
}

fn push_optional(buffer: &mut Vec<u8>, value: Option<&str>) {
    match value {
        Some(value) => {
            buffer.push(1);
            push_length_prefixed(buffer, value.as_bytes());
        }
        None => buffer.push(0),
    }
}

/// Deterministic 16-byte digest built from two domain-separated FNV-1a passes
/// over the length-prefixed input. Pure integer math keeps the output
/// byte-identical across Linux and macOS (D-25). This avoids introducing a new
/// third-party hashing dependency while preserving collision resistance at repo
/// scale (the input is already length-prefixed and domain-separated).
fn digest_16(input: &[u8]) -> [u8; 16] {
    let low = fnv1a64_with_seed(0xcbf2_9ce4_8422_2325, input);
    let high = fnv1a64_with_seed(0x84222325_cbf29ce4 ^ low, input);
    let mut bytes = [0u8; 16];
    bytes[..8].copy_from_slice(&low.to_be_bytes());
    bytes[8..].copy_from_slice(&high.to_be_bytes());
    bytes
}

fn fnv1a64_with_seed(seed: u64, input: &[u8]) -> u64 {
    let mut hash = seed;
    for byte in input {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex(hex: &str) -> Result<[u8; 16], String> {
    if hex.len() != 32 {
        return Err(format!(
            "signature digest must be 32 hex chars, found {}",
            hex.len()
        ));
    }
    let mut bytes = [0u8; 16];
    let chars = hex.as_bytes();
    for (index, byte) in bytes.iter_mut().enumerate() {
        let high = hex_value(chars[index * 2])?;
        let low = hex_value(chars[index * 2 + 1])?;
        *byte = (high << 4) | low;
    }
    Ok(bytes)
}

fn hex_value(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        other => Err(format!("invalid hex character `{}`", other as char)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;
    use serde::de::DeserializeOwned;
    use std::fmt::Debug;
    use std::hash::Hash;

    fn sample_record() -> IdentityRecord {
        IdentityRecord {
            id: IdentityRecordId(0),
            kind: IdentityKind::Function,
            file_id: FileId(3),
            span: Span::point(FileId(3), 4, 5),
            language: LanguageTag::Go,
            package_or_module: Arc::from("example.com/pkg"),
            container_path: Arc::from("pkg.Type"),
            display_name: Arc::from("Method"),
            signature_digest: compute_signature_digest(
                LanguageTag::Go,
                "example.com/pkg",
                "pkg.Type",
                "Method",
                None,
                None,
            ),
            multiplicity: 1,
            stable_key: "identity|function|go|example.com/pkg|pkg.Type|3|4..5".to_string(),
            originating_call_site_id: None,
            originating_call_target_id: None,
        }
    }

    fn assert_copy_ord_hash<T>()
    where
        T: Debug
            + Clone
            + Copy
            + PartialEq
            + Eq
            + PartialOrd
            + Ord
            + Hash
            + Serialize
            + DeserializeOwned,
    {
    }

    #[test]
    fn identity_record_round_trips_through_serde_json_with_stable_fields() {
        let record = sample_record();
        let json = serde_json::to_string(&record).expect("serialize");
        let restored: IdentityRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(record, restored);
    }

    #[test]
    fn signature_digest_round_trips_as_32_char_lowercase_hex() {
        let digest = compute_signature_digest(LanguageTag::Go, "pkg", "Type", "Method", None, None);
        let json = serde_json::to_string(&digest).expect("serialize");
        // JSON string includes surrounding quotes.
        assert_eq!(json.len(), 34);
        let inner = json.trim_matches('"');
        assert_eq!(inner.len(), 32);
        assert!(inner.chars().all(|character| character.is_ascii_hexdigit()));
        assert!(
            inner
                .chars()
                .all(|character| !character.is_ascii_uppercase())
        );
        let restored: SignatureDigest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(digest, restored);
    }

    #[test]
    fn identity_kind_and_language_tag_are_copy_ord_hash() {
        assert_copy_ord_hash::<IdentityKind>();
        assert_copy_ord_hash::<LanguageTag>();
        assert_copy_ord_hash::<IdentityRecordId>();

        let mut kinds = vec![IdentityKind::Callsite, IdentityKind::Function];
        kinds.sort();
        assert_eq!(kinds, vec![IdentityKind::Function, IdentityKind::Callsite]);

        let mut tags = vec![
            LanguageTag::TypeScript,
            LanguageTag::Go,
            LanguageTag::JavaScript,
        ];
        tags.sort();
        assert_eq!(
            tags,
            vec![
                LanguageTag::Go,
                LanguageTag::TypeScript,
                LanguageTag::JavaScript,
            ]
        );
    }

    #[test]
    fn signature_digest_is_byte_identical_for_identical_inputs() {
        let first =
            compute_signature_digest(LanguageTag::Go, "pkg", "Type", "Method", Some("()"), None);
        let second =
            compute_signature_digest(LanguageTag::Go, "pkg", "Type", "Method", Some("()"), None);
        assert_eq!(first, second);
    }

    #[test]
    fn signature_digest_disambiguates_field_boundaries() {
        let left = compute_signature_digest(LanguageTag::Go, "a", "b/c", "fn", None, None);
        let right = compute_signature_digest(LanguageTag::Go, "a/b", "c", "fn", None, None);
        assert_ne!(left, right);
    }

    #[test]
    fn signature_digest_is_not_all_zero_for_real_input() {
        let digest = compute_signature_digest(LanguageTag::Go, "pkg", "Type", "Method", None, None);
        assert_ne!(digest, SignatureDigest([0u8; 16]));
    }

    #[test]
    fn stable_key_disambiguates_field_boundaries() {
        let left = compute_identity_stable_key(
            IdentityKind::Function,
            LanguageTag::Go,
            "a",
            "b/c",
            FileId(1),
            &Span::point(FileId(1), 1, 1),
        );
        let right = compute_identity_stable_key(
            IdentityKind::Function,
            LanguageTag::Go,
            "a/b",
            "c",
            FileId(1),
            &Span::point(FileId(1), 1, 1),
        );
        assert_ne!(left, right);
    }

    #[test]
    fn stable_key_disambiguates_separator_in_field_value() {
        let left = compute_identity_stable_key(
            IdentityKind::Function,
            LanguageTag::Go,
            "a|b",
            "c",
            FileId(1),
            &Span::point(FileId(1), 1, 1),
        );
        let right = compute_identity_stable_key(
            IdentityKind::Function,
            LanguageTag::Go,
            "a",
            "b|c",
            FileId(1),
            &Span::point(FileId(1), 1, 1),
        );
        assert_ne!(left, right);
    }

    #[test]
    fn clone_with_multiplicity_only_changes_multiplicity() {
        let record = sample_record();
        let bumped = record.clone_with_multiplicity(7);
        assert_eq!(bumped.multiplicity, 7);
        assert_eq!(record.clone_with_multiplicity(record.multiplicity), record);
    }
}
