//! Source-file spine shared by frontends and the host fact database.

use std::path::PathBuf;
use std::sync::Arc;

use polint_core::{FileId, Language};

/// A discovered source file loaded into the analysis database.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub id: FileId,
    pub path: PathBuf,
    pub relative_path: String,
    pub language: Language,
    pub source: Arc<str>,
    pub content_hash: String,
}
