use anyhow::{Context, Result};
use polint_core::{
    AnalysisDb, BranchId, BranchObligation, FileId, FunctionFact, FunctionId, ImportFact, Language,
    PackageFact, Span, TestFact, span_from_byte_range,
};
use polint_diagnostics::{Diagnostic, TextRange, fingerprint};
use tree_sitter::{Node, Parser};

pub fn analyze(db: &mut AnalysisDb) -> Vec<Diagnostic> {
    let files: Vec<_> = db
        .files()
        .iter()
        .filter(|file| file.language == Language::Go)
        .map(|file| file.id)
        .collect();

    let mut diagnostics = Vec::new();
    for file_id in files {
        match parse_go_file(db, file_id) {
            Ok(mut file_diagnostics) => diagnostics.append(&mut file_diagnostics),
            Err(error) => {
                diagnostics.push(Diagnostic::error(
                    "parser/go",
                    db.path_for(file_id),
                    TextRange::point(1, 1),
                    format!("Failed to parse Go file: {error}"),
                ));
            }
        }
    }
    diagnostics
}

fn parse_go_file(db: &mut AnalysisDb, file_id: FileId) -> Result<Vec<Diagnostic>> {
    let source = db
        .file(file_id)
        .context("missing source file")?
        .source
        .clone();
    let source = source.as_ref();

    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_go::LANGUAGE.into())?;
    let tree = parser
        .parse(source, None)
        .context("tree-sitter returned no parse tree")?;
    let root = tree.root_node();
    let mut diagnostics = Vec::new();

    if root.has_error() {
        diagnostics.push(parser_error_diagnostic(db, file_id, source, root));
    }

    extract_package(db, file_id, source, root);
    extract_imports(db, file_id, source, root);
    extract_functions(db, file_id, source, root);
    Ok(diagnostics)
}

fn parser_error_diagnostic(
    db: &AnalysisDb,
    file_id: FileId,
    source: &str,
    root: Node<'_>,
) -> Diagnostic {
    let range = first_error_node(root)
        .map(|node| node_span(file_id, source, node).diagnostic_range())
        .unwrap_or_else(|| TextRange::point(1, 1));

    Diagnostic::error(
        "parser/go",
        db.path_for(file_id),
        range,
        "Go parser reported a syntax error.",
    )
}

fn extract_package(db: &mut AnalysisDb, file: FileId, source: &str, root: Node<'_>) {
    let Some(package_clause) =
        first_named_descendant(root, &|node| node.kind() == "package_clause")
    else {
        return;
    };

    let Some(identifier) =
        first_named_descendant(package_clause, &|node| node.kind() == "package_identifier")
    else {
        return;
    };

    let Some(name) = node_text(source, identifier) else {
        return;
    };

    db.push_package(PackageFact {
        id: polint_core::PackageId(0),
        file,
        name: name.to_string(),
        span: node_span(file, source, identifier),
        language: Language::Go,
    });
}

fn node_span(file: FileId, source: &str, node: Node<'_>) -> Span {
    span_from_byte_range(file, source, node.start_byte(), node.end_byte())
}

fn node_text<'source>(source: &'source str, node: Node<'_>) -> Option<&'source str> {
    source.get(node.start_byte()..node.end_byte())
}

fn first_error_node(node: Node<'_>) -> Option<Node<'_>> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }

    for index in 0..node.named_child_count() as u32 {
        let Some(child) = node.named_child(index) else {
            continue;
        };
        if let Some(error) = first_error_node(child) {
            return Some(error);
        }
    }
    None
}

fn first_named_descendant<'tree, F>(node: Node<'tree>, predicate: &F) -> Option<Node<'tree>>
where
    F: Fn(Node<'tree>) -> bool,
{
    if predicate(node) {
        return Some(node);
    }

    walk_named_children(node, |child| {
        if let Some(descendant) = first_named_descendant(child, predicate) {
            return Some(descendant);
        }
        None
    })
}

fn walk_named_children<'tree, F>(node: Node<'tree>, mut visit: F) -> Option<Node<'tree>>
where
    F: FnMut(Node<'tree>) -> Option<Node<'tree>>,
{
    for index in 0..node.named_child_count() as u32 {
        let Some(child) = node.named_child(index) else {
            continue;
        };
        if let Some(found) = visit(child) {
            return Some(found);
        }
    }
    None
}

fn extract_imports(db: &mut AnalysisDb, file: FileId, source: &str, root: Node<'_>) {
    visit_named_descendants(root, &mut |node| {
        if node.kind() != "import_declaration" {
            return;
        }

        let mut specs = Vec::new();
        visit_named_descendants(node, &mut |descendant| {
            if descendant.kind() == "import_spec" {
                specs.push(descendant);
            }
        });

        if specs.is_empty() {
            push_import_from_node(db, file, source, node);
            return;
        }

        for spec in specs {
            push_import_from_node(db, file, source, spec);
        }
    });
}

fn push_import_from_node(db: &mut AnalysisDb, file: FileId, source: &str, node: Node<'_>) {
    let Some(path_node) = first_named_descendant(node, &is_go_string_literal) else {
        return;
    };
    let Some(path) = unquote_go_string_literal(source, path_node) else {
        return;
    };
    let package = import_alias(source, node, path_node);

    db.push_import(ImportFact {
        id: polint_core::ImportId(0),
        file,
        package,
        path,
        span: node_span(file, source, node),
        language: Language::Go,
    });
}

fn import_alias(source: &str, spec: Node<'_>, path: Node<'_>) -> Option<String> {
    let alias = source
        .get(spec.start_byte()..path.start_byte())?
        .trim()
        .strip_prefix("import")
        .unwrap_or_else(|| {
            source
                .get(spec.start_byte()..path.start_byte())
                .unwrap_or("")
        })
        .trim()
        .trim_matches('(')
        .trim();

    if alias.is_empty() {
        None
    } else {
        Some(alias.to_string())
    }
}

fn unquote_go_string_literal(source: &str, node: Node<'_>) -> Option<String> {
    let text = node_text(source, node)?.trim();
    if text.len() < 2 {
        return None;
    }

    let first = text.as_bytes()[0] as char;
    let last = text.as_bytes()[text.len() - 1] as char;
    if matches!((first, last), ('"', '"') | ('`', '`')) {
        Some(text[1..text.len() - 1].to_string())
    } else {
        None
    }
}

fn is_go_string_literal(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "interpreted_string_literal" | "raw_string_literal"
    )
}

fn extract_functions(db: &mut AnalysisDb, file: FileId, source: &str, root: Node<'_>) {
    let file_path = db.path_for(file);
    visit_named_descendants(root, &mut |node| {
        if !matches!(node.kind(), "function_declaration" | "method_declaration") {
            return;
        }

        let Some(simple_name) = declaration_name(source, node) else {
            return;
        };
        let body_node = node.child_by_field_name("body");
        let name = if node.kind() == "method_declaration" {
            receiver_type_name(source, node)
                .map(|receiver| format!("{receiver}.{simple_name}"))
                .unwrap_or_else(|| simple_name.clone())
        } else {
            simple_name.clone()
        };
        let span = node_span(file, source, node);
        let is_test = is_go_test_entry(&file_path, &simple_name, node, source);
        let fact = FunctionFact {
            id: FunctionId(0),
            file,
            name: name.clone(),
            span: span.clone(),
            language: Language::Go,
            is_test,
            is_exported: simple_name.chars().next().is_some_and(char::is_uppercase),
            cyclomatic_complexity: body_node
                .map(|body| go_cyclomatic_complexity(source, body))
                .unwrap_or(1),
            calls: body_node
                .map(|body| extract_calls(source, body))
                .unwrap_or_default(),
        };
        let function_id = db.push_function(fact);

        let body_lines: Vec<&str> = node_text(source, node)
            .unwrap_or_default()
            .lines()
            .collect();
        extract_branches(
            db,
            file,
            function_id,
            source,
            &body_lines,
            span.start_line.saturating_sub(1) as usize,
        );
        if is_test {
            if let Some(body) = body_node {
                db.push_test(go_test_fact(file, function_id, name, span, source, body));
            }
        }
    });
}

fn declaration_name(source: &str, node: Node<'_>) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|name| node_text(source, name))
        .map(str::to_string)
        .or_else(|| {
            first_named_descendant(node, &|child| {
                matches!(child.kind(), "identifier" | "field_identifier")
            })
            .and_then(|name| node_text(source, name))
            .map(str::to_string)
        })
}

fn receiver_type_name(source: &str, node: Node<'_>) -> Option<String> {
    let receiver = node.child_by_field_name("receiver")?;
    let receiver_text = node_text(source, receiver)?;
    let inner = receiver_text
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    let raw_type = inner
        .split_whitespace()
        .last()
        .unwrap_or(inner)
        .trim_start_matches('*')
        .trim_start_matches('&');
    let without_package = raw_type.rsplit('.').next().unwrap_or(raw_type);
    let without_generics = without_package
        .split_once('[')
        .map(|(name, _)| name)
        .unwrap_or(without_package);

    (!without_generics.is_empty()).then(|| without_generics.to_string())
}

fn is_go_test_entry(path: &str, name: &str, node: Node<'_>, source: &str) -> bool {
    if !path.ends_with("_test.go") || node.kind() != "function_declaration" {
        return false;
    }

    let expected_parameter = if name.starts_with("Test") {
        "*testing.T"
    } else if name.starts_with("Benchmark") {
        "*testing.B"
    } else if name.starts_with("Fuzz") {
        "*testing.F"
    } else {
        return false;
    };

    let Some(parameters) = node
        .child_by_field_name("parameters")
        .or_else(|| first_named_child(node, "parameter_list"))
    else {
        return false;
    };
    let normalized = node_text(source, parameters)
        .unwrap_or_default()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    normalized.contains(expected_parameter)
}

fn extract_calls(source: &str, body: Node<'_>) -> Vec<String> {
    let mut calls = Vec::new();
    visit_named_descendants(body, &mut |node| {
        if node.kind() != "call_expression" {
            return;
        }
        let Some(call) = call_callee(source, node) else {
            return;
        };
        calls.push(call);
    });
    calls.sort();
    calls.dedup();
    calls
}

fn call_callee(source: &str, node: Node<'_>) -> Option<String> {
    let function_node = node
        .child_by_field_name("function")
        .or_else(|| node.named_child(0))?;
    stable_call_name(source, function_node)
}

fn stable_call_name(source: &str, node: Node<'_>) -> Option<String> {
    let text = node_text(source, node)?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("");
    if text.is_empty() || text.starts_with("func") {
        None
    } else {
        Some(text)
    }
}

fn go_test_fact(
    file: FileId,
    function: FunctionId,
    name: String,
    span: Span,
    source: &str,
    body: Node<'_>,
) -> TestFact {
    TestFact {
        file,
        function: Some(function),
        name,
        span,
        evidence_terms: go_test_evidence_terms(source, body),
        assertion_count: go_assertion_count(source, body),
        subtest_count: go_subtest_count(source, body),
        table_rows: go_table_rows(source, body),
    }
}

fn go_subtest_count(source: &str, body: Node<'_>) -> u32 {
    let mut count = 0;
    visit_named_descendants(body, &mut |node| {
        if node.kind() == "call_expression" && call_callee(source, node).as_deref() == Some("t.Run")
        {
            count += 1;
        }
    });
    count
}

fn go_assertion_count(source: &str, body: Node<'_>) -> u32 {
    let mut count = 0;
    visit_named_descendants(body, &mut |node| match node.kind() {
        "call_expression" => {
            if call_callee(source, node).is_some_and(|callee| is_go_assertion_callee(&callee)) {
                count += 1;
            }
        }
        "if_statement" => {
            if is_simple_assertion_condition(source, node) {
                count += 1;
            }
        }
        _ => {}
    });
    count
}

fn is_go_assertion_callee(callee: &str) -> bool {
    matches!(callee, "t.Fatal" | "t.Fatalf" | "t.Error" | "t.Errorf")
        || callee.starts_with("require.")
        || callee.starts_with("assert.")
}

fn is_simple_assertion_condition(source: &str, node: Node<'_>) -> bool {
    let Some(condition) = if_condition_text(source, node) else {
        return false;
    };
    let normalized = condition
        .trim()
        .trim_matches('(')
        .trim_matches(')')
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();

    matches!(normalized.as_str(), "err==nil" | "err!=nil" | "got!=want")
}

fn if_condition_text<'source>(source: &'source str, node: Node<'_>) -> Option<&'source str> {
    if let Some(condition) = node.child_by_field_name("condition") {
        return node_text(source, condition);
    }

    let text = node_text(source, node)?.trim();
    let condition = text.strip_prefix("if")?.split('{').next()?.trim();
    condition.rsplit(';').next().map(str::trim)
}

fn go_table_rows(source: &str, body: Node<'_>) -> u32 {
    let mut rows = std::collections::BTreeSet::new();
    visit_named_descendants(body, &mut |node| {
        if !matches!(
            node.kind(),
            "literal_element" | "keyed_element" | "literal_value"
        ) {
            return;
        }
        let Some(text) = node_text(source, node).map(str::trim) else {
            return;
        };
        if text.starts_with('{')
            && text.ends_with('}')
            && text.contains(':')
            && !text.contains('\n')
        {
            rows.insert((node.start_byte(), node.end_byte()));
        }
    });
    rows.len() as u32
}

fn go_test_evidence_terms(source: &str, body: Node<'_>) -> Vec<String> {
    let mut terms = std::collections::BTreeSet::new();
    visit_named_descendants(body, &mut |node| match node.kind() {
        "identifier" | "field_identifier" | "package_identifier" => {
            if let Some(text) = node_text(source, node) {
                add_identifier_evidence(&mut terms, text);
            }
        }
        "interpreted_string_literal" | "raw_string_literal" => {
            if let Some(text) = unquote_go_string_literal(source, node) {
                add_string_evidence(&mut terms, source, node, &text);
            }
        }
        "nil" => {
            terms.insert("nil".to_string());
        }
        _ => {}
    });

    visit_named_descendants(body, &mut |node| {
        if node.kind() == "if_statement"
            && if_condition_text(source, node).is_some_and(|condition| condition.contains("nil"))
        {
            terms.insert("nil".to_string());
        }
    });

    terms.into_iter().collect()
}

fn add_identifier_evidence(terms: &mut std::collections::BTreeSet<String>, text: &str) {
    let lower = text.to_ascii_lowercase();
    if is_go_evidence_word(&lower) {
        terms.insert(lower);
    }
}

fn add_string_evidence(
    terms: &mut std::collections::BTreeSet<String>,
    source: &str,
    node: Node<'_>,
    text: &str,
) {
    let words = evidence_words(text);
    let is_case_or_subtest_name = string_literal_looks_like_test_case_name(source, node);
    let has_marker = words.iter().any(|word| is_go_evidence_word(word));
    for word in words {
        if is_go_evidence_word(&word)
            || (is_case_or_subtest_name && !has_marker && !word.contains('_'))
        {
            terms.insert(word);
        }
    }
}

fn evidence_words(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn is_go_evidence_word(word: &str) -> bool {
    matches!(
        word,
        "allowed" | "denied" | "err" | "error" | "fail" | "invalid" | "nil"
    )
}

fn string_literal_looks_like_test_case_name(source: &str, node: Node<'_>) -> bool {
    let line_start = source[..node.start_byte()]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let prefix = &source[line_start..node.start_byte()];
    prefix.contains("name:") || prefix.contains("t.Run(")
}

fn go_cyclomatic_complexity(_source: &str, body: Node<'_>) -> u32 {
    let mut complexity = 1;
    visit_named_descendants(body, &mut |node| match node.kind() {
        "if_statement" | "for_statement" => complexity += 1,
        "expression_case" | "type_case" | "communication_case" | "default_case" => {
            complexity += 1;
        }
        "binary_expression" => complexity += boolean_operator_count(node),
        _ => {}
    });
    complexity
}

fn boolean_operator_count(node: Node<'_>) -> u32 {
    let mut count = 0;
    for index in 0..node.child_count() as u32 {
        let Some(child) = node.child(index) else {
            continue;
        };
        if matches!(child.kind(), "&&" | "||") {
            count += 1;
        }
    }
    count
}

fn visit_named_descendants<'tree, F>(node: Node<'tree>, visit: &mut F)
where
    F: FnMut(Node<'tree>),
{
    visit(node);
    for index in 0..node.named_child_count() as u32 {
        let Some(child) = node.named_child(index) else {
            continue;
        };
        visit_named_descendants(child, visit);
    }
}

fn first_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    for index in 0..node.named_child_count() as u32 {
        let Some(child) = node.named_child(index) else {
            continue;
        };
        if child.kind() == kind {
            return Some(child);
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use super::*;
    use polint_graph::ImportGraph;
    use std::path::PathBuf;

    fn db_with_go_file(relative_path: &str, source: &str) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from(relative_path),
            relative_path.to_string(),
            source.to_string(),
        );
        db
    }

    #[test]
    fn reports_tree_sitter_parse_errors_with_stable_range() {
        let mut db = db_with_go_file("payment.go", "package payment\n\nfunc Broken( {\n");

        let diagnostics = analyze(&mut db);

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.rule_id, "parser/go");
        assert_eq!(diagnostic.file, "payment.go");
        assert_eq!(diagnostic.message, "Go parser reported a syntax error.");
        assert_ne!(diagnostic.range, TextRange::point(1, 1));
        assert!(diagnostic.range.start_line >= 3, "{:?}", diagnostic.range);
        assert!(
            (diagnostic.range.end_line, diagnostic.range.end_col)
                >= (diagnostic.range.start_line, diagnostic.range.start_col),
            "{:?}",
            diagnostic.range
        );
    }

    #[test]
    fn continues_best_effort_package_extraction_after_parse_error() {
        let mut db = db_with_go_file("payment.go", "package payment\n\nfunc Broken( {\n");

        let diagnostics = analyze(&mut db);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "parser/go");
        assert_eq!(db.packages().len(), 1);
        assert_eq!(db.packages()[0].name, "payment");
        assert_eq!(db.packages()[0].language, Language::Go);
    }

    #[test]
    fn extracts_go_package_name_from_tree_sitter() {
        let mut db = db_with_go_file("payment.go", "package payment\n\nfunc Authorize() {}\n");

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        assert_eq!(db.packages().len(), 1);
        let package = &db.packages()[0];
        assert_eq!(package.name, "payment");
        assert_eq!(package.language, Language::Go);
        assert_eq!(package.span.diagnostic_range().start_line, 1);
        assert_eq!(package.span.diagnostic_range().start_col, 9);
    }

    #[test]
    fn parser_foundation_covers_diagnostics_and_package_facts() {
        reports_tree_sitter_parse_errors_with_stable_range();
        continues_best_effort_package_extraction_after_parse_error();
        extracts_go_package_name_from_tree_sitter();
    }

    #[test]
    fn extracts_go_imports_from_tree_sitter() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

import "fmt"
import (
	alias "github.com/acme/aliased"
	. "github.com/acme/dot"
	_ "github.com/acme/sideeffect"
	"context"
)
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        let imports = db
            .imports()
            .iter()
            .map(|import| (import.package.as_deref(), import.path.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            imports,
            vec![
                (None, "fmt"),
                (Some("alias"), "github.com/acme/aliased"),
                (Some("."), "github.com/acme/dot"),
                (Some("_"), "github.com/acme/sideeffect"),
                (None, "context"),
            ]
        );
        assert!(
            db.imports()
                .iter()
                .all(|import| import.language == Language::Go)
        );
        assert_eq!(db.imports()[0].span.diagnostic_range().start_line, 3);
        assert_eq!(db.imports()[1].span.diagnostic_range().start_line, 5);
    }

    #[test]
    fn extracts_go_functions_methods_calls_and_complexity_from_tree_sitter() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

import "log"

type Service struct{}

func Authorize(ok bool, svc *Service) error {
	if ok && svc.Enabled() {
		svc.Charge()
		svc.Charge()
		log.Printf("authorized")
	}
	for _, item := range []int{1, 2} {
		process(item)
	}
	switch ok {
	case true:
		audit()
	default:
		fallback()
	}
	return nil
}

func (svc *Service) Charge() {}

func (svc *Service) Enabled() bool {
	return true
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        let functions = db.functions();
        assert_eq!(
            functions
                .iter()
                .map(|function| function.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Authorize", "Service.Charge", "Service.Enabled"]
        );

        let authorize = &functions[0];
        assert_eq!(authorize.language, Language::Go);
        assert!(authorize.is_exported);
        assert!(!authorize.is_test);
        assert_eq!(
            authorize.calls,
            vec![
                "audit".to_string(),
                "fallback".to_string(),
                "log.Printf".to_string(),
                "process".to_string(),
                "svc.Charge".to_string(),
                "svc.Enabled".to_string(),
            ]
        );
        assert_eq!(authorize.cyclomatic_complexity, 6);
        assert_eq!(authorize.span.diagnostic_range().start_line, 7);
        assert_eq!(authorize.span.diagnostic_range().end_line, 23);

        let method = &functions[1];
        assert_eq!(method.name, "Service.Charge");
        assert!(method.is_exported);
        assert_eq!(method.cyclomatic_complexity, 1);
    }

    #[test]
    fn go_import_facts_feed_import_graph() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

import "github.com/acme/authz"

func Authorize() {}
"#,
        );

        let diagnostics = analyze(&mut db);
        let dot = ImportGraph::from_db(&db).to_dot();

        assert!(diagnostics.is_empty());
        assert!(dot.contains("payment.go"), "{dot}");
        assert!(dot.contains("github.com/acme/authz"), "{dot}");
    }

    #[test]
    fn extracts_go_test_functions_subtests_and_table_evidence() {
        let mut db = db_with_go_file(
            "payment_test.go",
            r#"package payment

import "testing"

func TestAuthorize(t *testing.T) {
	cases := []struct {
		name string
		allowed bool
		wantErr bool
	}{
		{name: "allowed", allowed: true, wantErr: false},
		{name: "denied", allowed: false, wantErr: true},
		{name: "invalid token", allowed: false, wantErr: true},
	}

	for _, tt := range cases {
		t.Run(tt.name, func(t *testing.T) {
			err := Authorize(tt.allowed)
			if tt.wantErr && err == nil {
				t.Fatalf("expected error for %s", tt.name)
			}
			if !tt.wantErr && err != nil {
				t.Errorf("unexpected denied error: %v", err)
			}
		})
	}
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        assert_eq!(db.functions().len(), 1);
        assert!(db.functions()[0].is_test);
        assert_eq!(db.tests().len(), 1);
        let test = &db.tests()[0];
        assert_eq!(test.name, "TestAuthorize");
        assert_eq!(test.function, Some(db.functions()[0].id));
        assert_eq!(test.subtest_count, 1);
        assert_eq!(test.table_rows, 3);
        assert_eq!(test.assertion_count, 2);
        assert_eq!(
            test.evidence_terms,
            vec![
                "allowed".to_string(),
                "denied".to_string(),
                "err".to_string(),
                "error".to_string(),
                "invalid".to_string(),
                "nil".to_string(),
            ]
        );
    }

    #[test]
    fn does_not_mark_non_test_go_functions_as_tests() {
        let mut helper_db = db_with_go_file(
            "payment_test.go",
            r#"package payment

func TestHelper() {}
"#,
        );
        let mut non_test_file_db = db_with_go_file(
            "payment.go",
            r#"package payment

import "testing"

func TestAuthorize(t *testing.T) {}
"#,
        );

        let helper_diagnostics = analyze(&mut helper_db);
        let non_test_file_diagnostics = analyze(&mut non_test_file_db);

        assert!(helper_diagnostics.is_empty());
        assert!(non_test_file_diagnostics.is_empty());
        assert_eq!(helper_db.tests().len(), 0);
        assert!(!helper_db.functions()[0].is_test);
        assert_eq!(non_test_file_db.tests().len(), 0);
        assert!(!non_test_file_db.functions()[0].is_test);
    }

    #[test]
    fn go_assertion_evidence_counts_common_failure_calls() {
        let mut db = db_with_go_file(
            "payment_test.go",
            r#"package payment

import (
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestAssertions(t *testing.T) {
	err := Authorize(false)
	if err == nil {
		t.Fatal("expected error")
	}
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	got := "denied"
	want := "allowed"
	if got != want {
		t.Errorf("got %s want %s", got, want)
	}
	require.NoError(t, err)
	assert.Equal(t, want, got)
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        assert_eq!(db.tests().len(), 1);
        assert_eq!(db.tests()[0].assertion_count, 8);
        assert_eq!(
            db.tests()[0].evidence_terms,
            vec![
                "allowed".to_string(),
                "denied".to_string(),
                "err".to_string(),
                "error".to_string(),
                "nil".to_string(),
            ]
        );
    }
}
