pub use crate::internal_core::{Span, TextRange};

// Preserve Span::diagnostic_range returning facade diagnostics::TextRange alias.
// crate::internal_core::Span::diagnostic_range returns crate::internal_core::DiagnosticRange, which is the
// same shape as crate::diagnostics::TextRange after re-export.
