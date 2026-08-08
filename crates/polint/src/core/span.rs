use super::ids::FileId;
use crate::diagnostics::TextRange as DiagnosticRange;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct TextRange {
    pub start_byte: u32,
    pub end_byte: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
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
    pub fn point(file: FileId, line: u32, col: u32) -> Self {
        Self {
            file,
            start_byte: 0,
            end_byte: 0,
            start_line: line,
            start_col: col,
            end_line: line,
            end_col: col,
        }
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
