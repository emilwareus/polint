//! Review changeset types for `polint review` diffs.
//!
//! Extracted from the core monolith without behaviour changes.

use serde::{Deserialize, Serialize};

/// How a path changed relative to the target ref, in a `polint review` diff.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChangeStatus {
    /// The file is new on the working side.
    Added,
    /// The file existed on both sides and its content changed.
    Modified,
    /// The file was removed on the working side.
    Deleted,
    /// The file was renamed; the carried path is the new-side path.
    Renamed,
}

/// One changed file in a `polint review` diff against the target ref.
///
/// Crate-internal: rule authors read changed files through the `ChangedFiles`
/// fact view and its `ChangedFileRef` items, never this struct directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ChangedFile {
    /// Repo-relative, `/`-normalized path, identical in form to `Diagnostic.file`.
    pub(crate) path: String,
    /// How this file changed relative to the target ref.
    pub(crate) status: ChangeStatus,
    /// New-side changed line ranges, inclusive and 1-based; empty for `Deleted`.
    pub(crate) new_line_ranges: Vec<(u32, u32)>,
}

/// The set of files changed in a `polint review` diff against the target ref.
///
/// Injected on the [`AnalysisDb`] by the host runner; read through the
/// `ChangedFiles` SDK fact view. Empty under `polint check`. Crate-internal:
/// it travels outer→host as a JSON cache file, so it derives `Serialize`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReviewChangeset {
    /// Changed files, sorted by `path` for deterministic output.
    pub(crate) files: Vec<ChangedFile>,
}
