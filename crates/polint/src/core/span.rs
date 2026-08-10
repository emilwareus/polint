pub use polint_core::{Span, TextRange};

// Preserve Span::diagnostic_range returning facade diagnostics::TextRange alias.
// polint_core::Span::diagnostic_range returns polint_core::DiagnosticRange, which is the
// same shape as crate::diagnostics::TextRange after re-export.
