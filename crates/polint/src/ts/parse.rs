//! Shared TS/JS parse entrypoint.
//!
//! Every Oxc parse site in this crate must go through [`parse_ts_source`]. When Oxc
//! recovers from a syntax error the returned AST is partial: consumers must record
//! an unsupported fact rather than treating missing facts as absences.

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_parser::{Parser, ParserReturn};
use oxc_span::SourceType;
use std::path::Path;

use crate::analysis_api::SourceFile;

/// Construct name for unsupported-semantic rows recorded on recoverable Oxc errors.
pub const PARSER_RECOVERY_CONSTRUCT: &str = "parser recovery";

/// Status reason secondary extractors use when they continue on a partial AST.
pub const PARTIAL_AST_REASON: &str = "partial AST from parse errors";

/// Owned parse-error summary taken from Oxc diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub message: String,
    pub start_byte: Option<usize>,
    pub end_byte: Option<usize>,
}

/// Result of [`parse_ts_source`].
///
/// `fully_parsed == false` means the AST is partial (recoverable errors) or empty
/// (catastrophic panic). Downstream analysis may continue, but must declare the gap.
pub struct ParsedTsSource<'a> {
    pub raw: ParserReturn<'a>,
    pub errors: Vec<ParseError>,
    pub fully_parsed: bool,
}

impl<'a> ParsedTsSource<'a> {
    pub fn program(&self) -> &Program<'a> {
        &self.raw.program
    }

    /// True when Oxc aborted and left an empty program body.
    pub fn is_catastrophic(&self) -> bool {
        self.raw.panicked && self.raw.program.body.is_empty()
    }
}

/// Derive Oxc [`SourceType`] from the file path. All parse sites must use this.
pub fn source_type(path: &Path) -> SourceType {
    SourceType::from_path(path).unwrap_or_default()
}

/// The only sanctioned way to parse TS/JS in this crate.
pub fn parse_ts_source<'a>(
    allocator: &'a Allocator,
    path: &Path,
    source: &'a str,
) -> ParsedTsSource<'a> {
    let raw = Parser::new(allocator, source, source_type(path)).parse();
    let errors = raw
        .errors
        .iter()
        .map(|error| {
            let (start_byte, end_byte) = error
                .labels
                .as_ref()
                .and_then(|labels| labels.first())
                .map(|label| {
                    let start = label.offset();
                    (Some(start), Some(start + label.len()))
                })
                .unwrap_or((None, None));
            ParseError {
                message: error.to_string(),
                start_byte,
                end_byte,
            }
        })
        .collect::<Vec<_>>();
    let fully_parsed = errors.is_empty() && !(raw.panicked && raw.program.body.is_empty());
    ParsedTsSource {
        raw,
        errors,
        fully_parsed,
    }
}

/// Parse a [`SourceFile`] through [`parse_ts_source`].
pub fn parse_ts_file<'a>(allocator: &'a Allocator, file: &'a SourceFile) -> ParsedTsSource<'a> {
    parse_ts_source(allocator, &file.path, file.source.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_allocator::Allocator;
    use std::path::Path;

    #[test]
    fn recoverable_syntax_error_marks_ast_partial() {
        let allocator = Allocator::default();
        let parsed = parse_ts_source(
            &allocator,
            Path::new("recoverable.tsx"),
            "const x = <div></span>;",
        );
        assert!(!parsed.errors.is_empty(), "expected oxc syntax errors");
        assert!(
            !parsed.program().body.is_empty(),
            "recoverable parse should keep a non-empty body"
        );
        assert!(!parsed.is_catastrophic());
        assert!(!parsed.fully_parsed);
    }

    #[test]
    fn catastrophic_parse_marks_ast_partial_and_empty() {
        let allocator = Allocator::default();
        let parsed = parse_ts_source(
            &allocator,
            Path::new("broken.ts"),
            "import x from \"./x\";\nconst value = ;",
        );
        assert!(!parsed.fully_parsed);
        assert!(!parsed.errors.is_empty());
        // This fixture panics with an empty body; imports may still appear via module_record.
        assert!(parsed.is_catastrophic());
    }

    #[test]
    fn clean_source_is_fully_parsed() {
        let allocator = Allocator::default();
        let parsed = parse_ts_source(
            &allocator,
            Path::new("ok.ts"),
            "export function ok(value: number) { return value + 1; }",
        );
        assert!(parsed.fully_parsed);
        assert!(parsed.errors.is_empty());
        assert!(!parsed.is_catastrophic());
    }

    #[test]
    fn every_ts_parse_site_routes_through_the_shared_helper() {
        let src_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ts");
        let mut parser_new_relpaths = Vec::new();
        for entry in walkdir_rs_files(&src_root) {
            let source = std::fs::read_to_string(&entry).expect("read rust source");
            if !source.contains("Parser::new(") {
                continue;
            }
            let rel = entry
                .strip_prefix(&src_root)
                .expect("file under src")
                .to_path_buf();
            parser_new_relpaths.push(rel);
        }
        assert_eq!(
            parser_new_relpaths,
            vec![Path::new("parse.rs").to_path_buf()],
            "Oxc Parser::new must live only in crate::ts::parse::parse_ts_source; found {parser_new_relpaths:?}"
        );
    }

    fn walkdir_rs_files(root: &Path) -> Vec<std::path::PathBuf> {
        let mut files = Vec::new();
        fn walk(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, files);
                } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                    files.push(path);
                }
            }
        }
        walk(root, &mut files);
        files.sort();
        files
    }
}
