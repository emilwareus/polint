//! Source-file spine shared by frontends and the host fact database.

use std::path::PathBuf;
use std::sync::Arc;

use crate::internal_core::{FileId, Language, SourceTextIndex, Span, span_from_byte_range};

/// A discovered source file loaded into the analysis database.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SourceFile {
    pub id: FileId,
    pub path: PathBuf,
    pub relative_path: String,
    pub language: Language,
    pub source: Arc<str>,
    pub content_hash: String,
    source_index: SourceTextIndex,
}
impl SourceFile {
    /// Constructs a discovered source file from its complete fields.
    pub fn new(
        id: FileId,
        path: PathBuf,
        relative_path: String,
        language: Language,
        source: Arc<str>,
        content_hash: String,
    ) -> Self {
        let source_index = SourceTextIndex::new(&source);
        Self {
            id,
            path,
            relative_path,
            language,
            source,
            content_hash,
            source_index,
        }
    }

    pub(crate) fn clone_with_id(&self, id: FileId) -> Self {
        Self {
            id,
            path: self.path.clone(),
            relative_path: self.relative_path.clone(),
            language: self.language,
            source: Arc::clone(&self.source),
            content_hash: self.content_hash.clone(),
            source_index: self.source_index.clone(),
        }
    }

    pub(crate) fn span_from_byte_range(&self, start_byte: usize, end_byte: usize) -> Span {
        span_from_byte_range(
            self.id,
            &self.source,
            &self.source_index,
            start_byte,
            end_byte,
        )
    }

    pub(crate) fn byte_count(&self) -> usize {
        self.source_index.byte_count()
    }

    pub(crate) fn line_count(&self) -> usize {
        self.source_index.line_count()
    }

    pub(crate) fn non_empty_line_count(&self) -> usize {
        self.source_index.non_empty_line_count(&self.source)
    }
}
