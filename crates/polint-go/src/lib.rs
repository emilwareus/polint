use anyhow::{Context, Result};
use polint_core::{
    AnalysisDb, BranchId, BranchObligation, FileId, FunctionFact, FunctionId, ImportFact, Language,
    Span, TestFact, span_from_byte_range,
};
use polint_diagnostics::{Diagnostic, TextRange, fingerprint};
use tree_sitter::Parser;

pub fn analyze(db: &mut AnalysisDb) -> Vec<Diagnostic> {
    let files: Vec<_> = db
        .files()
        .iter()
        .filter(|file| file.language == Language::Go)
        .cloned()
        .collect();

    let mut diagnostics = Vec::new();
    for file in files {
        if let Err(error) = parse_go_file(db, file.id) {
            diagnostics.push(Diagnostic::error(
                "parser/go",
                db.path_for(file.id),
                TextRange::point(1, 1),
                format!("Failed to parse Go file: {error}"),
            ));
        }
    }
    diagnostics
}

fn parse_go_file(db: &mut AnalysisDb, file_id: FileId) -> Result<()> {
    let file = db.file(file_id).context("missing source file")?.clone();
    let source = file.source.to_string();

    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_go::LANGUAGE.into())?;
    let tree = parser
        .parse(&source, None)
        .context("tree-sitter returned no parse tree")?;
    if tree.root_node().has_error() {
        // Keep extracting best-effort facts. The CLI also reports this parser diagnostic.
    }

    extract_imports(db, file_id, &source);
    extract_functions_and_tests(db, file_id, &source);
    Ok(())
}

fn extract_imports(db: &mut AnalysisDb, file: FileId, source: &str) {
    let mut in_block = false;
    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("import (") {
            in_block = true;
            continue;
        }
        if in_block && trimmed.starts_with(')') {
            in_block = false;
            continue;
        }
        let import_path = if in_block {
            quoted(trimmed)
        } else if let Some(rest) = trimmed.strip_prefix("import ") {
            quoted(rest)
        } else {
            None
        };

        if let Some(path) = import_path {
            let span = Span::point(file, line_idx as u32 + 1, 1);
            db.push_import(ImportFact {
                id: polint_core::ImportId(0),
                file,
                package: None,
                path,
                span,
                language: Language::Go,
            });
        }
    }
}

fn extract_functions_and_tests(db: &mut AnalysisDb, file: FileId, source: &str) {
    let line_starts = line_starts(source);
    let lines: Vec<&str> = source.lines().collect();
    let mut idx = 0;
    while idx < lines.len() {
        let trimmed = lines[idx].trim_start();
        if !trimmed.starts_with("func ") {
            idx += 1;
            continue;
        }

        let start_line = idx;
        let end_line = find_function_end(&lines, idx);
        let start_byte = *line_starts.get(start_line).unwrap_or(&0);
        let end_byte = line_starts
            .get(end_line + 1)
            .copied()
            .unwrap_or(source.len());
        let body = &source[start_byte..end_byte.min(source.len())];
        let name = function_name(trimmed).unwrap_or_else(|| "<anonymous>".to_string());
        let span = span_from_byte_range(file, source, start_byte, end_byte.min(source.len()));
        let is_test = name.starts_with("Test") && db.path_for(file).ends_with("_test.go");
        let fact = FunctionFact {
            id: FunctionId(0),
            file,
            name: name.clone(),
            span: span.clone(),
            language: Language::Go,
            is_test,
            is_exported: name.chars().next().is_some_and(char::is_uppercase),
            cyclomatic_complexity: cyclomatic_complexity(body),
            calls: calls(body),
        };
        let function_id = db.push_function(fact);

        extract_branches(
            db,
            file,
            function_id,
            source,
            &lines[start_line..=end_line],
            start_line,
        );
        if is_test {
            db.push_test(TestFact {
                file,
                function: Some(function_id),
                name,
                span,
                evidence_terms: evidence_terms(body),
                assertion_count: assertion_count(body),
                subtest_count: body.matches("t.Run(").count() as u32,
                table_rows: table_rows(body),
            });
        }

        idx = end_line + 1;
    }
}

fn extract_branches(
    db: &mut AnalysisDb,
    file: FileId,
    function: FunctionId,
    source: &str,
    lines: &[&str],
    base_line: usize,
) {
    for (offset, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let line_no = base_line + offset + 1;
        if let Some(condition) = trimmed.strip_prefix("if ") {
            let condition = condition
                .split('{')
                .next()
                .unwrap_or(condition)
                .trim()
                .to_string();
            push_branch(
                db,
                file,
                function,
                source,
                line_no,
                condition.clone(),
                "true",
            );
            push_branch(db, file, function, source, line_no, condition, "false");
        } else if trimmed.starts_with("case ") {
            let label = trimmed.trim_end_matches(':').to_string();
            push_branch(db, file, function, source, line_no, label, "case");
        } else if trimmed.starts_with("default:") {
            push_branch(
                db,
                file,
                function,
                source,
                line_no,
                "default".to_string(),
                "default",
            );
        } else if trimmed.starts_with("for ") || trimmed == "for {" {
            push_branch(
                db,
                file,
                function,
                source,
                line_no,
                trimmed.to_string(),
                "loop",
            );
        } else if trimmed.starts_with("select ") || trimmed == "select {" {
            push_branch(
                db,
                file,
                function,
                source,
                line_no,
                "select".to_string(),
                "select",
            );
        }
    }
}

fn push_branch(
    db: &mut AnalysisDb,
    file: FileId,
    function: FunctionId,
    _source: &str,
    line_no: usize,
    condition_text: String,
    edge_label: &str,
) -> BranchId {
    let is_error_path = condition_text.contains("err != nil")
        || condition_text.contains("err==nil")
        || condition_text.contains("error");
    let stable_fingerprint = fingerprint(&[
        &db.path_for(file),
        &function.0.to_string(),
        &line_no.to_string(),
        &condition_text,
        edge_label,
    ]);
    db.push_branch(BranchObligation {
        id: BranchId(0),
        function: Some(function),
        file,
        decision_span: Span::point(file, line_no as u32, 1),
        condition_text,
        edge_label: edge_label.to_string(),
        is_error_path,
        stable_fingerprint,
    })
}

fn quoted(input: &str) -> Option<String> {
    let start = input.find('"')?;
    let rest = &input[start + 1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn function_name(signature: &str) -> Option<String> {
    let after_func = signature.trim_start_matches("func ").trim_start();
    let name_part = if after_func.starts_with('(') {
        let receiver_end = after_func.find(')')?;
        after_func[receiver_end + 1..].trim_start()
    } else {
        after_func
    };
    let end = name_part.find('(')?;
    Some(name_part[..end].trim().to_string())
}

fn find_function_end(lines: &[&str], start: usize) -> usize {
    let mut depth = 0_i32;
    let mut saw_open = false;
    for (idx, line) in lines.iter().enumerate().skip(start) {
        for ch in line.chars() {
            if ch == '{' {
                depth += 1;
                saw_open = true;
            } else if ch == '}' {
                depth -= 1;
            }
        }
        if saw_open && depth <= 0 {
            return idx;
        }
    }
    lines.len().saturating_sub(1)
}

fn cyclomatic_complexity(body: &str) -> u32 {
    1 + count_word(body, "if")
        + count_word(body, "for")
        + count_word(body, "range")
        + count_word(body, "case")
        + count_word(body, "default")
        + body.matches("&&").count() as u32
        + body.matches("||").count() as u32
}

fn count_word(body: &str, word: &str) -> u32 {
    body.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .filter(|part| *part == word)
        .count() as u32
}

fn calls(body: &str) -> Vec<String> {
    let keywords = ["if", "for", "switch", "select", "return", "func"];
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

fn evidence_terms(body: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for marker in ["err", "error", "invalid", "denied", "nil", "fail"] {
        if body.contains(marker) {
            terms.push(marker.to_string());
        }
    }
    terms
}

fn assertion_count(body: &str) -> u32 {
    [
        "if got != want",
        "if err != nil",
        "if err == nil",
        "require.",
        "assert.",
        "t.Fatal",
        "t.Errorf",
        "t.Fatalf",
    ]
    .iter()
    .map(|needle| body.matches(needle).count() as u32)
    .sum()
}

fn table_rows(body: &str) -> u32 {
    body.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            trimmed.starts_with('{') && trimmed.contains(':')
        })
        .count() as u32
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

    #[test]
    fn extracts_go_function_name() {
        assert_eq!(
            function_name("func Authorize() error {").unwrap(),
            "Authorize"
        );
        assert_eq!(function_name("func (s *Svc) Run() {").unwrap(), "Run");
    }

    #[test]
    fn counts_complexity() {
        assert_eq!(
            cyclomatic_complexity("if a { for _, x := range xs { _ = x } }"),
            4
        );
    }
}
