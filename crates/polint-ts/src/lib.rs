use anyhow::{Context, Result};
use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;
use polint_core::{
    AnalysisDb, FunctionFact, FunctionId, ImportFact, JsxAttributeFact, Language, Span,
    StringLiteralFact, TsComponentFact, span_from_byte_range,
};
use polint_diagnostics::{Diagnostic, TextRange};

pub fn analyze(db: &mut AnalysisDb) -> Vec<Diagnostic> {
    let files: Vec<_> = db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
        .map(|file| file.id)
        .collect();

    let mut diagnostics = Vec::new();
    for file_id in files {
        match parse_ts_file(db, file_id) {
            Ok(mut file_diagnostics) => diagnostics.append(&mut file_diagnostics),
            Err(error) => {
                diagnostics.push(Diagnostic::error(
                    "parser/ts",
                    db.path_for(file_id),
                    TextRange::point(1, 1),
                    format!("Failed to parse TS/JS file: {error}"),
                ));
            }
        }
    }
    diagnostics
}

fn parse_ts_file(db: &mut AnalysisDb, file_id: polint_core::FileId) -> Result<Vec<Diagnostic>> {
    let file = db.file(file_id).context("missing source file")?.clone();
    let source = file.source.to_string();
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&file.path).unwrap_or_default();
    let parsed = Parser::new(&allocator, &source, source_type).parse();
    let mut diagnostics = Vec::new();

    for error in &parsed.errors {
        let range = error
            .labels
            .as_ref()
            .and_then(|labels| labels.first())
            .map(|label| {
                span_from_byte_range(file_id, &source, label.offset(), label.offset() + label.len())
                    .diagnostic_range()
            })
            .unwrap_or_else(|| TextRange::point(1, 1));

        diagnostics.push(Diagnostic::error(
            "parser/ts",
            db.path_for(file_id),
            range,
            format!("TS/JS parser reported a syntax error: {error}"),
        ));
    }

    if parsed.panicked && parsed.program.body.is_empty() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic::error(
            "parser/ts",
            db.path_for(file_id),
            TextRange::point(1, 1),
            "TS/JS parser reported a syntax error",
        ));
    }

    extract_imports(db, file_id, &source, file.language);
    extract_functions(db, file_id, &source, file.language);
    extract_string_literals(db, file_id, &source, file.language);
    extract_jsx_attributes(db, file_id, &source);
    Ok(diagnostics)
}

fn extract_imports(
    db: &mut AnalysisDb,
    file: polint_core::FileId,
    source: &str,
    language: Language,
) {
    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        let has_module_specifier = trimmed.starts_with("import ")
            || trimmed.contains("require(")
            || trimmed.starts_with("export *")
            || (trimmed.starts_with("export ") && trimmed.contains(" from "));
        let path = if has_module_specifier {
            module_specifier(trimmed)
        } else {
            None
        };
        if let Some(path) = path {
            db.push_import(ImportFact {
                id: polint_core::ImportId(0),
                file,
                package: None,
                path,
                span: Span::point(file, line_idx as u32 + 1, 1),
                language,
            });
        }
    }
}

fn extract_functions(
    db: &mut AnalysisDb,
    file: polint_core::FileId,
    source: &str,
    language: Language,
) {
    let line_starts = line_starts(source);
    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        let name = ts_function_name(trimmed);
        if let Some(name) = name {
            let start_byte = *line_starts.get(line_idx).unwrap_or(&0);
            let end_line = find_block_end(source, start_byte);
            let span = span_from_byte_range(file, source, start_byte, end_line.min(source.len()));
            let body = &source[start_byte..end_line.min(source.len())];
            let function = FunctionFact {
                id: FunctionId(0),
                file,
                name: name.clone(),
                span: span.clone(),
                language,
                is_test: name.contains("test") || name.contains("spec"),
                is_exported: trimmed.starts_with("export "),
                cyclomatic_complexity: cyclomatic_complexity(body),
                calls: calls(body),
            };
            let function_id = db.push_function(function);
            if is_component_name(&name) || body.contains("jsx(") || body.contains('<') {
                db.push_ts_component(TsComponentFact {
                    file,
                    function: Some(function_id),
                    name,
                    span,
                });
            }
        }
    }
}

fn extract_string_literals(
    db: &mut AnalysisDb,
    file: polint_core::FileId,
    source: &str,
    language: Language,
) {
    let bytes = source.as_bytes();
    let mut idx = 0;
    while idx < bytes.len() {
        let quote = bytes[idx];
        if quote != b'\'' && quote != b'"' && quote != b'`' {
            idx += 1;
            continue;
        }
        let start = idx;
        idx += 1;
        let mut value = String::new();
        while idx < bytes.len() {
            if bytes[idx] == b'\\' {
                idx += 2;
                continue;
            }
            if bytes[idx] == quote {
                let end = idx + 1;
                db.push_string_literal(StringLiteralFact {
                    file,
                    value,
                    span: span_from_byte_range(file, source, start, end),
                    language,
                });
                idx = end;
                break;
            }
            value.push(bytes[idx] as char);
            idx += 1;
        }
    }
}

fn extract_jsx_attributes(db: &mut AnalysisDb, file: polint_core::FileId, source: &str) {
    for (line_idx, line) in source.lines().enumerate() {
        if !line.contains('<') || !line.contains('=') {
            continue;
        }
        for part in line.split_whitespace() {
            if let Some(eq) = part.find('=') {
                let name = part[..eq]
                    .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_');
                if name.is_empty() || name.starts_with('<') {
                    continue;
                }
                let raw_value = part[eq + 1..]
                    .trim_matches(|ch| ch == '"' || ch == '\'' || ch == '{' || ch == '}');
                db.push_jsx_attribute(JsxAttributeFact {
                    file,
                    name: name.to_string(),
                    value: (!raw_value.is_empty()).then(|| raw_value.to_string()),
                    span: Span::point(file, line_idx as u32 + 1, 1),
                });
            }
        }
    }
}

fn module_specifier(line: &str) -> Option<String> {
    let quote_idx = line.find('"').or_else(|| line.find('\''))?;
    let quote = line.as_bytes()[quote_idx] as char;
    let rest = &line[quote_idx + 1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn ts_function_name(line: &str) -> Option<String> {
    let line = line.strip_prefix("export ").unwrap_or(line).trim_start();
    if let Some(rest) = line.strip_prefix("function ") {
        return rest.split('(').next().map(|name| name.trim().to_string());
    }
    if let Some(rest) = line.strip_prefix("async function ") {
        return rest.split('(').next().map(|name| name.trim().to_string());
    }
    if line.starts_with("const ") || line.starts_with("let ") || line.starts_with("var ") {
        let after_decl = line.split_once(' ')?.1;
        if after_decl.contains("=>") || after_decl.contains("function") {
            return after_decl
                .split(['=', ':'])
                .next()
                .map(|name| name.trim().to_string());
        }
    }
    if let Some(rest) = line.strip_prefix("class ") {
        return rest
            .split([' ', '{'])
            .next()
            .map(|name| name.trim().to_string());
    }
    None
}

fn is_component_name(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

fn find_block_end(source: &str, start: usize) -> usize {
    let mut depth = 0_i32;
    let mut saw_open = false;
    for (idx, ch) in source[start..].char_indices() {
        if ch == '{' {
            depth += 1;
            saw_open = true;
        } else if ch == '}' {
            depth -= 1;
        }
        if saw_open && depth <= 0 {
            return start + idx + ch.len_utf8();
        }
    }
    source.len()
}

fn cyclomatic_complexity(body: &str) -> u32 {
    1 + count_word(body, "if")
        + count_word(body, "for")
        + count_word(body, "while")
        + count_word(body, "case")
        + count_word(body, "catch")
        + body.matches("&&").count() as u32
        + body.matches("||").count() as u32
        + body.matches('?').count() as u32
}

fn count_word(body: &str, word: &str) -> u32 {
    body.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|part| *part == word)
        .count() as u32
}

fn calls(body: &str) -> Vec<String> {
    let keywords = ["if", "for", "while", "switch", "return", "function"];
    let mut calls = Vec::new();
    for token in body.split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')) {
        if token.is_empty() || keywords.contains(&token) {
            continue;
        }
        if body.contains(&format!("{token}(")) {
            calls.push(token.to_string());
        }
    }
    calls.sort();
    calls.dedup();
    calls
}

fn line_starts(source: &str) -> Vec<usize> {
    let mut starts = vec![0];
    for (idx, ch) in source.char_indices() {
        if ch == '\n' {
            starts.push(idx + 1);
        }
    }
    starts
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn analyze_source(path: &str, source: &str) -> (AnalysisDb, Vec<Diagnostic>) {
        let mut db = AnalysisDb::new();
        db.add_file(PathBuf::from(path), path.to_string(), source.to_string());
        let diagnostics = analyze(&mut db);
        (db, diagnostics)
    }

    #[test]
    fn extracts_function_names() {
        assert_eq!(ts_function_name("function run() {}").unwrap(), "run");
        assert_eq!(
            ts_function_name("const Button = () => <div />").unwrap(),
            "Button"
        );
    }

    #[test]
    fn extracts_module_specifier() {
        assert_eq!(module_specifier("import x from \"./x\";").unwrap(), "./x");
    }

    #[test]
    fn reports_oxc_parser_errors_as_parser_ts_diagnostics() {
        let (_db, diagnostics) = analyze_source("broken.ts", "export function Broken( {");

        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.rule_id == "parser/ts")
            .expect("expected parser/ts diagnostic");

        assert_eq!(diagnostic.file, "broken.ts");
        assert!(
            diagnostic
                .message
                .contains("TS/JS parser reported a syntax error")
        );
    }

    #[test]
    fn clean_ts_family_sources_do_not_emit_parser_ts() {
        let cases = [
            (
                "valid.ts",
                "export function ok(value: number) { return value + 1; }",
            ),
            (
                "component.tsx",
                "export function Button() { return <button aria-label=\"Save\">Save</button>; }",
            ),
            (
                "util.js",
                "export function ok(value) { return value + 1; }",
            ),
            (
                "view.jsx",
                "export function Button() { return <button aria-label=\"Save\">Save</button>; }",
            ),
        ];

        for (path, source) in cases {
            let (_db, diagnostics) = analyze_source(path, source);
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.rule_id != "parser/ts"),
                "{path} emitted parser/ts diagnostics: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn continues_best_effort_ast_extraction_after_oxc_parse_error() {
        let (db, diagnostics) = analyze_source(
            "recoverable.ts",
            "import x from \"./x\";\nexport function Broken( {",
        );

        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule_id == "parser/ts"),
            "expected parser/ts diagnostic"
        );
        assert!(
            db.imports().iter().any(|import| import.path == "./x"),
            "expected best-effort import fact after parse error"
        );
    }
}
