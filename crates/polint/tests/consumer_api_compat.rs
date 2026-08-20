//! Consumer API compatibility.
//!
//! This is an integration test, so it sees `polint` exactly as a repo-local rule pack
//! does: through `polint::sdk::prelude`, with no crate-internal access. Everything here
//! is a *compile-time* assertion — if this file compiles, the shapes downstream rule
//! packs rely on are still available. If it fails to compile, a real consumer breaks.
//!
//! The patterns below are taken from rule packs that exist in the wild, not invented.
//! A rule that computes its own span from byte offsets must build a `Span`; one that
//! reports at a computed line/column must build a `DiagnosticRange`. Both were briefly
//! made impossible by marking those types `#[non_exhaustive]`, which forbids struct
//! literals downstream while buying nothing — every constructor on them already takes
//! all fields positionally, so adding a field breaks callers either way.
//!
//! Enums stay `#[non_exhaustive]` on purpose: new languages and severities are
//! realistic, and there the attribute genuinely does buy room to grow.

use polint::sdk::prelude::*;

/// Rule packs build `Span` when they locate a match themselves rather than reading one
/// off a fact. Struct-literal construction must keep working.
#[test]
fn span_is_constructible_by_struct_literal() {
    let span = Span {
        file: FileId(0),
        start_byte: 0,
        end_byte: 4,
        start_line: 1,
        start_col: 1,
        end_line: 1,
        end_col: 5,
    };
    assert_eq!(span.start_byte, 0);
    assert_eq!(span.end_line, 1);
}

/// Reporting at a computed position requires building a range directly.
#[test]
fn diagnostic_range_is_constructible_by_struct_literal() {
    let range = DiagnosticRange {
        start_line: 3,
        start_col: 7,
        end_line: 3,
        end_col: 11,
    };
    assert_eq!(range.start_line, 3);
    assert_eq!(range.end_col, 11);
}

/// `RuleId` is a public newtype; wrapping a string must keep working.
#[test]
fn rule_id_is_constructible() {
    let id = RuleId("local/example".to_string());
    assert_eq!(id.0, "local/example");
}

/// Comparing and non-exhaustively matching `Language` are the two patterns real rule
/// packs use to scope themselves to one language. Neither may require a wildcard arm.
#[test]
fn language_supports_comparison_and_partial_matching() {
    let language = Language::Go;
    assert!(language == Language::Go);
    assert!(matches!(
        Language::TypeScript,
        Language::TypeScript | Language::Tsx
    ));
}

/// Severity is ordered, and rule packs compare rather than exhaustively match it.
#[test]
fn severity_supports_comparison() {
    assert!(Severity::Error >= Severity::Warn);
    assert!(Severity::Warn >= Severity::Info);
}

/// The diagnostic builder chain that every rule pack uses to report a finding.
#[test]
fn diagnostic_builder_chain_is_available() {
    let diagnostic = Diagnostic::error(
        "local/example".to_string(),
        "src/example.go".to_string(),
        DiagnosticRange {
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 2,
        },
        "example finding",
    )
    .with_evidence("kind", "example".to_string());

    assert_eq!(diagnostic.rule_id, "local/example");
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(diagnostic.evidence.len(), 1);
}
