//! Source-file spine shared by frontends and the host fact database.

use std::path::PathBuf;
use std::sync::Arc;

use crate::internal_core::{FileId, Language};

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
        Self {
            id,
            path,
            relative_path,
            language,
            source,
            content_hash,
        }
    }
}
