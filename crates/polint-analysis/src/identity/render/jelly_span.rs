//! Jelly span renderer (D-05, D-06, D-08, D-12).
//!
//! Projects an [`IdentityRecord`] plus the borrowed [`SourceFile`] into Jelly's
//! oracle span shape `file:start_line:start_col:end_line:end_col` with **1-based
//! line, 1-based column, half-open end column**.
//!
//! CRLF normalization (D-12) happens here, at renderer time — never at file
//! load. The on-disk `Span` byte offsets and `SourceFile.source` are left
//! byte-true so v1.2 facts are unchanged. The renderer collapses `\r\n` -> `\n`
//! into a scratch byte-offset map, translates the span's byte offsets through
//! it, and re-derives line/column from the post-normalization byte text. A
//! `\r\n`-encoded checkout and an `\n`-encoded checkout of the same logical
//! source therefore produce byte-identical Jelly span strings (D-13, D-25).

use crate::identity::facts::IdentityRecord;
use polint_analysis_api::SourceFile;

/// Renders the Jelly oracle span string for an identity record against its
/// source file (D-08).
pub fn render(identity: &IdentityRecord, source: &SourceFile) -> String {
    let relative_path = source.relative_path.replace('\\', "/");
    let (start_line, start_col, end_line, end_col) = line_columns(
        &source.source,
        identity.span.start_byte,
        identity.span.end_byte,
    );
    format!("{relative_path}:{start_line}:{start_col}:{end_line}:{end_col}")
}

/// Computes `(start_line, start_col, end_line, end_col)` for a byte range after
/// CRLF normalization. Lines and columns are 1-based; the end column is
/// half-open (D-08).
///
/// The single linear pass over the source bytes (O(n)) builds the
/// pre-normalization -> post-normalization offset translation implicitly while
/// counting lines/columns, so there is no quadratic scan and the only allocation
/// is the returned tuple (T-42-02-04).
fn line_columns(text: &str, start_byte: u32, end_byte: u32) -> (u32, u32, u32, u32) {
    let bytes = text.as_bytes();
    let start = start_byte as usize;
    let end = end_byte as usize;

    let mut line = 1u32;
    let mut col = 1u32;

    let mut start_line = 1u32;
    let mut start_col = 1u32;
    let mut end_line = 1u32;
    let mut end_col = 1u32;

    let mut index = 0usize;
    let mut captured_start = false;
    let mut captured_end = false;

    while index <= bytes.len() {
        if !captured_start && index >= start {
            start_line = line;
            start_col = col;
            captured_start = true;
        }
        if !captured_end && index >= end {
            end_line = line;
            end_col = col;
            captured_end = true;
        }
        if captured_start && captured_end {
            break;
        }
        if index == bytes.len() {
            break;
        }

        let byte = bytes[index];
        if byte == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            // `\r\n` collapses to a single `\n` (one logical newline). Advance
            // two source bytes but count one line break.
            line += 1;
            col = 1;
            index += 2;
        } else if byte == b'\n' || byte == b'\r' {
            line += 1;
            col = 1;
            index += 1;
        } else {
            // Advance by one UTF-8 character so multi-byte characters count as a
            // single column. We step over continuation bytes here.
            let char_len = utf8_char_len(byte);
            col += 1;
            index += char_len;
        }
    }

    if !captured_start {
        start_line = line;
        start_col = col;
    }
    if !captured_end {
        end_line = line;
        end_col = col;
    }

    (start_line, start_col, end_line, end_col)
}

fn utf8_char_len(first_byte: u8) -> usize {
    match first_byte {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        // Continuation or invalid lead byte: advance one to make progress.
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::*;
    use crate::identity::facts::{
        IdentityKind, IdentityRecord, IdentityRecordId, LanguageTag, compute_identity_stable_key,
        compute_signature_digest,
    };
    use polint_core::{FileId, Language, Span};

    fn source_file(relative_path: &str, text: &str) -> SourceFile {
        SourceFile::new(
            FileId::from_raw(1),
            PathBuf::from(relative_path),
            relative_path.to_string(),
            Language::TypeScript,
            Arc::from(text),
            "test".to_string(),
        )
    }

    fn byte_span(text: &str, needle: &str) -> Span {
        let start = text.find(needle).expect("needle present") as u32;
        let end = start + needle.len() as u32;
        let mut span = Span::point(FileId::from_raw(1), 0, 0);
        span.start_byte = start;
        span.end_byte = end;
        span
    }

    fn record(span: Span) -> IdentityRecord {
        let language = LanguageTag::TypeScript;
        IdentityRecord {
            id: IdentityRecordId(0),
            kind: IdentityKind::Function,
            file_id: FileId::from_raw(1),
            span: span.clone(),
            language,
            package_or_module: Arc::from("src/foo.ts"),
            container_path: Arc::from("foo"),
            display_name: Arc::from("foo"),
            signature_digest: compute_signature_digest(
                language,
                "src/foo.ts",
                "foo",
                "foo",
                None,
                None,
            ),
            multiplicity: 1,
            stable_key: polint_core::stable_key_for_test(&compute_identity_stable_key(
                IdentityKind::Function,
                language,
                "src/foo.ts",
                "foo",
                FileId::from_raw(1),
                &span,
            )),
            originating_call_site_id: None,
            originating_call_target_id: None,
        }
    }

    /// Multi-line LF function body. The function spans line 1 column 1 through
    /// the end byte just before the trailing newline of line 3.
    const MULTI_LINE_LF: &str = "function foo() {\n  return 1;\n}\n";

    #[test]
    fn basic_five_colon_format() {
        // A function spanning from the start of the file to the first byte of
        // line 3 renders 1-based line/column with a half-open end column.
        let text = MULTI_LINE_LF;
        let mut span = Span::point(FileId::from_raw(1), 0, 0);
        span.start_byte = 0;
        // End just past the closing brace `}` at the start of line 3.
        span.end_byte = text.find('}').expect("brace") as u32 + 1;
        let rendered = render(&record(span), &source_file("foo.ts", text));
        assert_eq!(rendered, "foo.ts:1:1:3:2");
    }

    #[test]
    fn forward_slash_path_normalization() {
        let text = "const a = 1;\n";
        let span = byte_span(text, "a");
        let rendered = render(&record(span), &source_file("src\\nested\\foo.ts", text));
        assert!(rendered.starts_with("src/nested/foo.ts:"));
        assert!(!rendered.contains('\\'));
    }

    #[test]
    fn crlf_and_lf_produce_byte_identical_output() {
        let lf = MULTI_LINE_LF;
        let crlf = "function foo() {\r\n  return 1;\r\n}\r\n";

        // The logical span is the whole `return 1;` statement. Byte offsets
        // differ between the two encodings because CRLF adds one `\r` per line,
        // but the renderer must re-derive identical line/column after collapse.
        let lf_span = byte_span(lf, "return 1;");
        let crlf_span = byte_span(crlf, "return 1;");

        let lf_rendered = render(&record(lf_span), &source_file("foo.ts", lf));
        let crlf_rendered = render(&record(crlf_span), &source_file("foo.ts", crlf));

        assert_eq!(lf_rendered, crlf_rendered);
        assert_eq!(lf_rendered, "foo.ts:2:3:2:12");
    }

    #[test]
    fn multi_line_function_line_counts_match_post_normalization() {
        // A genuinely multi-line function: the end line must be 3 in both
        // encodings, proving line counts shift correctly under CRLF collapse.
        let lf = MULTI_LINE_LF;
        let crlf = "function foo() {\r\n  return 1;\r\n}\r\n";

        let mut lf_span = Span::point(FileId::from_raw(1), 0, 0);
        lf_span.start_byte = 0;
        lf_span.end_byte = lf.find('}').expect("brace") as u32 + 1;

        let mut crlf_span = Span::point(FileId::from_raw(1), 0, 0);
        crlf_span.start_byte = 0;
        crlf_span.end_byte = crlf.find('}').expect("brace") as u32 + 1;

        let lf_rendered = render(&record(lf_span), &source_file("foo.ts", lf));
        let crlf_rendered = render(&record(crlf_span), &source_file("foo.ts", crlf));

        assert_eq!(lf_rendered, crlf_rendered);
        assert_eq!(lf_rendered, "foo.ts:1:1:3:2");
    }

    #[test]
    fn no_absolute_host_path_substrings() {
        // T-42-02-02: workspace-relative input must never produce absolute
        // host-path prefixes in renderer output.
        let text = "const a = 1;\n";
        let span = byte_span(text, "a");
        let output = render(&record(span), &source_file("src/foo.ts", text));
        assert!(!output.starts_with("/Users/"));
        assert!(!output.starts_with("/home/"));
        assert!(!output.contains(":\\"));
    }

    #[test]
    fn render_takes_only_identity_record_and_source_file() {
        // Signature lock (D-06): the renderer is a pure function of an
        // IdentityRecord plus a borrowed SourceFile, with no kernel handle.
        let render_fn: fn(&IdentityRecord, &SourceFile) -> String = render;
        let _ = render_fn;
    }
}
