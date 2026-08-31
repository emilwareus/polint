use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};

use crate::internal_core::diagnostic::TextRange as DiagnosticRange;
use crate::internal_core::ids::FileId;

#[derive(Debug, Clone)]
pub(crate) struct SourceTextIndex {
    line_starts: Arc<[u32]>,
    byte_count: usize,
    line_count: usize,
    non_empty_line_count: Arc<OnceLock<usize>>,
}

impl SourceTextIndex {
    pub(crate) fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            source
                .bytes()
                .enumerate()
                .filter_map(|(offset, byte)| (byte == b'\n').then_some((offset + 1) as u32)),
        );
        let line_count = if source.is_empty() {
            0
        } else {
            line_starts.len() - usize::from(source.ends_with('\n'))
        };
        Self {
            line_starts: line_starts.into(),
            byte_count: source.len(),
            line_count,
            non_empty_line_count: Arc::new(OnceLock::new()),
        }
    }

    pub(crate) fn byte_count(&self) -> usize {
        self.byte_count
    }

    pub(crate) fn line_count(&self) -> usize {
        self.line_count
    }

    pub(crate) fn non_empty_line_count(&self, source: &str) -> usize {
        *self.non_empty_line_count.get_or_init(|| {
            source
                .lines()
                .filter(|line| !line.trim().is_empty())
                .count()
        })
    }

    fn line_col(&self, source: &str, byte_offset: usize) -> (u32, u32) {
        let limit = byte_offset.min(source.len());
        let limit_u32 = limit as u32;
        let line_index = match self.line_starts.binary_search(&limit_u32) {
            Ok(index) => index,
            Err(0) => 0,
            Err(index) => index - 1,
        };
        let line_start = self.line_starts[line_index] as usize;
        let column = source[line_start..]
            .char_indices()
            .take_while(|(offset, _)| line_start + offset < limit)
            .count()
            + 1;
        ((line_index + 1) as u32, column as u32)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
// Deliberately NOT `#[non_exhaustive]`: rule packs legitimately construct these when they
// compute their own ranges, and every constructor here takes all fields positionally, so
// the attribute would forbid struct literals without buying any room to add a field.
pub struct TextRange {
    pub start_byte: u32,
    pub end_byte: u32,
}

impl TextRange {
    pub const fn new(start_byte: u32, end_byte: u32) -> Self {
        Self {
            start_byte,
            end_byte,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// Deliberately NOT `#[non_exhaustive]`: rule packs legitimately construct these when they
// compute their own ranges, and every constructor here takes all fields positionally, so
// the attribute would forbid struct literals without buying any room to add a field.
pub struct Span {
    pub file: FileId,
    pub start_byte: u32,
    pub end_byte: u32,
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

impl Span {
    pub fn new(
        file: FileId,
        start_byte: u32,
        end_byte: u32,
        start_line: u32,
        start_col: u32,
        end_line: u32,
        end_col: u32,
    ) -> Self {
        Self {
            file,
            start_byte,
            end_byte,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    pub fn point(file: FileId, line: u32, col: u32) -> Self {
        Self::new(file, 0, 0, line, col, line, col)
    }

    pub fn diagnostic_range(&self) -> DiagnosticRange {
        DiagnosticRange {
            start_line: self.start_line,
            start_col: self.start_col,
            end_line: self.end_line,
            end_col: self.end_col,
        }
    }
}

/// Build a [`Span`] from UTF-8 byte offsets using a source's retained line index.
pub(crate) fn span_from_byte_range(
    file: FileId,
    source: &str,
    index: &SourceTextIndex,
    start_byte: usize,
    end_byte: usize,
) -> Span {
    let start_byte = start_byte.min(source.len());
    let end_byte = end_byte.min(source.len()).max(start_byte);
    let (start_line, start_col) = index.line_col(source, start_byte);
    let (end_line, end_col) = index.line_col(source, end_byte);
    Span::new(
        file,
        start_byte as u32,
        end_byte as u32,
        start_line,
        start_col,
        end_line,
        end_col,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(source: &str, start: usize, end: usize) -> Span {
        let index = SourceTextIndex::new(source);
        span_from_byte_range(FileId::from_raw(0), source, &index, start, end)
    }

    #[test]
    fn indexed_span_preserves_unicode_character_columns() {
        let span = span("aé中\nx", 2, 5);

        assert_eq!((span.start_line, span.start_col, span.end_col), (1, 3, 4));
    }

    #[test]
    fn indexed_span_preserves_crlf_line_and_column_semantics() {
        let span = span("a\r\nb", 2, 3);

        assert_eq!(
            (span.start_line, span.start_col, span.end_line, span.end_col),
            (1, 3, 2, 1)
        );
    }

    #[test]
    fn indexed_span_handles_eof_offsets() {
        let span = span("abc", 3, usize::MAX);

        assert_eq!(
            (
                span.start_byte,
                span.end_byte,
                span.start_line,
                span.start_col
            ),
            (3, 3, 1, 4)
        );
    }

    #[test]
    fn indexed_span_handles_empty_source() {
        let span = span("", 1, 2);

        assert_eq!(
            (
                span.start_byte,
                span.end_byte,
                span.start_line,
                span.start_col
            ),
            (0, 0, 1, 1)
        );
    }

    #[test]
    fn indexed_span_counts_a_non_character_boundary_like_the_legacy_scan() {
        let span = span("aé", 2, 2);

        assert_eq!((span.start_col, span.end_col), (3, 3));
    }

    #[test]
    fn source_metrics_match_rust_line_semantics_and_memoize_unicode_whitespace() {
        for source in ["", "a", "a\n", "a\r\nb", "\u{2003}\nvalue\r\n\n"] {
            let index = SourceTextIndex::new(source);

            assert_eq!(index.byte_count(), source.len());
            assert_eq!(index.line_count(), source.lines().count());
            assert_eq!(
                index.non_empty_line_count(source),
                source
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .count()
            );
            assert_eq!(
                index.non_empty_line_count(source),
                source
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .count()
            );
        }
    }
}
