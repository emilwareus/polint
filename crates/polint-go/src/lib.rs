use anyhow::{Context, Result};
use polint_core::{
    AnalysisDb, BranchId, BranchObligation, FileId, FunctionFact, FunctionId, ImportFact, Language,
    PackageFact, Span, StringLiteralFact, TestFact, span_from_byte_range,
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
    extract_string_literals(db, file_id, source, root);
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

fn extract_string_literals(db: &mut AnalysisDb, file: FileId, source: &str, root: Node<'_>) {
    visit_named_descendants(root, &mut |node| {
        if !is_go_string_literal(node) || is_inside_go_import(node) {
            return;
        }

        let Some(value) = unquote_go_string_literal(source, node) else {
            return;
        };

        db.push_string_literal(StringLiteralFact {
            file,
            value,
            span: node_span(file, source, node),
            language: Language::Go,
        });
    });
}

fn is_inside_go_import(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(parent.kind(), "import_spec" | "import_declaration") {
            return true;
        }
        current = parent.parent();
    }
    false
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

        if let Some(body) = body_node {
            extract_branches(
                db,
                file,
                function_id,
                &name,
                function_result_contains_error(source, node),
                source,
                body,
            );
        }
        if is_test && let Some(body) = body_node {
            db.push_test(go_test_fact(file, function_id, name, span, source, body));
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
    let Some(parameters) = normalized
        .strip_prefix('(')
        .and_then(|parameters| parameters.strip_suffix(')'))
    else {
        return false;
    };

    if parameters.contains(',') {
        return false;
    }

    parameters == expected_parameter
        || parameters
            .strip_suffix(expected_parameter)
            .is_some_and(|name| {
                !name.is_empty()
                    && name
                        .chars()
                        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            })
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
        if node.kind() != "literal_value" || !is_anonymous_struct_table(source, node) {
            return;
        }

        for index in 0..node.named_child_count() as u32 {
            let Some(row) = node.named_child(index) else {
                continue;
            };
            let Some(text) = node_text(source, row).map(str::trim) else {
                continue;
            };
            if matches!(
                row.kind(),
                "literal_element" | "keyed_element" | "literal_value"
            ) && text.starts_with('{')
                && text.ends_with('}')
                && text.contains(':')
            {
                rows.insert((row.start_byte(), row.end_byte()));
            }
        }
    });
    rows.len() as u32
}

fn is_anonymous_struct_table(source: &str, literal_value: Node<'_>) -> bool {
    let Some(parent) = literal_value
        .parent()
        .filter(|parent| parent.kind() == "composite_literal")
    else {
        return false;
    };
    let Some(type_text) = source.get(parent.start_byte()..literal_value.start_byte()) else {
        return false;
    };
    let compact = type_text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    compact.contains("[]struct") || compact.contains("[...]struct")
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
    function_name: &str,
    function_returns_error: bool,
    source: &str,
    body: Node<'_>,
) {
    visit_named_descendants(body, &mut |node| match node.kind() {
        "if_statement" => push_if_branches(
            db,
            file,
            function,
            function_name,
            function_returns_error,
            source,
            node,
        ),
        "expression_switch_statement" | "type_switch_statement" => {
            push_switch_branch(db, file, function, function_name, source, node);
        }
        "expression_case" | "type_case" | "communication_case" => push_case_branch(
            db,
            BranchTarget {
                file,
                function,
                function_name,
            },
            function_returns_error,
            source,
            node,
            "case",
        ),
        "default_case" => push_case_branch(
            db,
            BranchTarget {
                file,
                function,
                function_name,
            },
            function_returns_error,
            source,
            node,
            "default",
        ),
        "for_statement" => push_for_branch(
            db,
            file,
            function,
            function_name,
            function_returns_error,
            source,
            node,
        ),
        "select_statement" => push_select_branch(db, file, function, function_name, source, node),
        _ => {}
    });
}

fn push_if_branches(
    db: &mut AnalysisDb,
    file: FileId,
    function: FunctionId,
    function_name: &str,
    function_returns_error: bool,
    source: &str,
    node: Node<'_>,
) {
    let decision = node.child_by_field_name("condition").unwrap_or(node);
    let condition = if_condition_text(source, node)
        .or_else(|| node_text(source, decision))
        .unwrap_or("if")
        .trim()
        .to_string();
    let span = node_span(file, source, decision);
    let true_body = node
        .child_by_field_name("consequence")
        .or_else(|| node.child_by_field_name("body"));
    let false_body = node.child_by_field_name("alternative");
    let true_is_error_path = branch_body_returns_error(source, true_body, function_returns_error)
        || condition_implies_error_edge(&condition, "true");
    let false_is_error_path = branch_body_returns_error(source, false_body, function_returns_error)
        || condition_implies_error_edge(&condition, "false");
    push_branch(
        db,
        BranchTarget {
            file,
            function,
            function_name,
        },
        span.clone(),
        condition.clone(),
        "true",
        true_is_error_path,
    );
    push_branch(
        db,
        BranchTarget {
            file,
            function,
            function_name,
        },
        span,
        condition.clone(),
        "false",
        false_is_error_path,
    );
}

fn push_switch_branch(
    db: &mut AnalysisDb,
    file: FileId,
    function: FunctionId,
    function_name: &str,
    source: &str,
    node: Node<'_>,
) {
    let Some((start, end)) = switch_decision_range(source, node) else {
        return;
    };
    let condition = trimmed_text(source, start, end)
        .unwrap_or("switch")
        .to_string();
    push_branch(
        db,
        BranchTarget {
            file,
            function,
            function_name,
        },
        trimmed_span(file, source, start, end),
        condition.clone(),
        "switch",
        is_go_error_path_heuristic(source, &condition, Some(node), false),
    );
}

fn push_case_branch(
    db: &mut AnalysisDb,
    target: BranchTarget<'_>,
    function_returns_error: bool,
    source: &str,
    node: Node<'_>,
    edge_label: &str,
) {
    let (start, end) = if edge_label == "default" {
        (node.start_byte(), node.end_byte())
    } else {
        case_header_range(source, node)
    };
    let condition = if edge_label == "default" {
        "default".to_string()
    } else {
        trimmed_text(source, start, end)
            .unwrap_or("case")
            .to_string()
    };
    push_branch(
        db,
        target,
        trimmed_span(target.file, source, start, end),
        condition.clone(),
        edge_label,
        is_go_error_path_heuristic(source, &condition, Some(node), function_returns_error),
    );
}

fn push_for_branch(
    db: &mut AnalysisDb,
    file: FileId,
    function: FunctionId,
    function_name: &str,
    function_returns_error: bool,
    source: &str,
    node: Node<'_>,
) {
    if let Some(range) = direct_range_clause(node) {
        let condition = node_text(source, range)
            .unwrap_or("range")
            .trim()
            .to_string();
        push_branch(
            db,
            BranchTarget {
                file,
                function,
                function_name,
            },
            node_span(file, source, range),
            condition.clone(),
            "range",
            is_go_error_path_heuristic(source, &condition, Some(node), function_returns_error),
        );
        return;
    }

    let (start, end) = statement_header_range(source, node);
    let condition = trimmed_text(source, start, end)
        .unwrap_or("for")
        .to_string();
    push_branch(
        db,
        BranchTarget {
            file,
            function,
            function_name,
        },
        trimmed_span(file, source, start, end),
        condition.clone(),
        "loop",
        is_go_error_path_heuristic(source, &condition, Some(node), function_returns_error),
    );
}

fn direct_range_clause(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("clause")
        .filter(|child| child.kind() == "range_clause")
        .or_else(|| first_named_child(node, "range_clause"))
}

fn push_select_branch(
    db: &mut AnalysisDb,
    file: FileId,
    function: FunctionId,
    function_name: &str,
    source: &str,
    node: Node<'_>,
) {
    let end = node.start_byte().saturating_add("select".len());
    push_branch(
        db,
        BranchTarget {
            file,
            function,
            function_name,
        },
        span_from_byte_range(file, source, node.start_byte(), end),
        "select".to_string(),
        "select",
        false,
    );
}

fn switch_decision_range(source: &str, node: Node<'_>) -> Option<(usize, usize)> {
    if node.kind() == "expression_switch_statement"
        && let Some(value) = node.child_by_field_name("value")
    {
        return Some((value.start_byte(), value.end_byte()));
    }

    let (start, end) = statement_header_range(source, node);
    let header = source.get(start..end)?;
    let switch_offset = header.find("switch")? + "switch".len();
    let after_switch = start + switch_offset;
    let rest = source.get(after_switch..end)?;
    let leading = rest.len() - rest.trim_start().len();
    let decision_start = after_switch + leading;
    let trimmed_rest = source.get(decision_start..end)?.trim_end();
    if trimmed_rest.is_empty() {
        return Some((start, end));
    }
    Some((decision_start, decision_start + trimmed_rest.len()))
}

fn case_header_range(source: &str, node: Node<'_>) -> (usize, usize) {
    if let Some(case_text) = source.get(node.start_byte()..node.end_byte())
        && let Some(colon_offset) = case_delimiter_colon(case_text)
    {
        return (node.start_byte(), node.start_byte() + colon_offset + 1);
    }

    let Some(after_node) = source.get(node.end_byte()..node.end_byte().saturating_add(16)) else {
        return (node.start_byte(), node.end_byte());
    };
    let colon_offset = after_node.find(':').filter(|offset| {
        after_node[..*offset]
            .chars()
            .all(|character| character.is_whitespace())
    });

    let end = colon_offset
        .map(|offset| node.end_byte() + offset + 1)
        .unwrap_or_else(|| node.end_byte());
    (node.start_byte(), end)
}

fn case_delimiter_colon(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut paren_depth = 0_u32;
    let mut bracket_depth = 0_u32;
    let mut brace_depth = 0_u32;

    while index < bytes.len() {
        match bytes[index] {
            b'"' | b'\'' => index = skip_quoted_literal(bytes, index)?,
            b'`' => index = skip_raw_string_literal(bytes, index)?,
            b'(' => paren_depth += 1,
            b')' => paren_depth = paren_depth.saturating_sub(1),
            b'[' => bracket_depth += 1,
            b']' => bracket_depth = bracket_depth.saturating_sub(1),
            b'{' => brace_depth += 1,
            b'}' => brace_depth = brace_depth.saturating_sub(1),
            b':' if paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 => {
                if bytes.get(index + 1) != Some(&b'=') {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn skip_quoted_literal(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = *bytes.get(start)?;
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            byte if byte == quote => return Some(index),
            _ => index += 1,
        }
    }
    None
}

fn skip_raw_string_literal(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'`' {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn statement_header_range(source: &str, node: Node<'_>) -> (usize, usize) {
    let body_start = node
        .child_by_field_name("body")
        .map(|body| body.start_byte())
        .or_else(|| {
            source
                .get(node.start_byte()..node.end_byte())
                .and_then(|text| text.find('{'))
                .map(|offset| node.start_byte() + offset)
        })
        .unwrap_or_else(|| node.end_byte());

    (node.start_byte(), body_start)
}

fn trimmed_text(source: &str, start: usize, end: usize) -> Option<&str> {
    source.get(start..end).map(str::trim)
}

fn trimmed_span(file: FileId, source: &str, start: usize, end: usize) -> Span {
    let Some(text) = source.get(start..end) else {
        return span_from_byte_range(file, source, start, end);
    };
    let leading = text.len() - text.trim_start().len();
    let trailing = text.len() - text.trim_end().len();
    span_from_byte_range(file, source, start + leading, end - trailing)
}

#[derive(Clone, Copy)]
struct BranchTarget<'name> {
    file: FileId,
    function: FunctionId,
    function_name: &'name str,
}

fn push_branch(
    db: &mut AnalysisDb,
    target: BranchTarget<'_>,
    decision_span: Span,
    condition_text: String,
    edge_label: &str,
    is_error_path: bool,
) -> BranchId {
    let start_line = decision_span.start_line.to_string();
    let start_col = decision_span.start_col.to_string();
    let normalized_condition = normalize_branch_condition(&condition_text);
    let stable_fingerprint = fingerprint(&[
        &db.path_for(target.file),
        target.function_name,
        &start_line,
        &start_col,
        &normalized_condition,
        edge_label,
    ]);
    db.push_branch(BranchObligation {
        id: BranchId(0),
        function: Some(target.function),
        file: target.file,
        decision_span,
        condition_text,
        edge_label: edge_label.to_string(),
        is_error_path,
        stable_fingerprint,
    })
}

fn normalize_branch_condition(condition_text: &str) -> String {
    condition_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn function_result_contains_error(source: &str, node: Node<'_>) -> bool {
    if node
        .child_by_field_name("result")
        .and_then(|result| node_text(source, result))
        .is_some_and(contains_error_word)
    {
        return true;
    }

    let (start, end) = statement_header_range(source, node);
    let Some(header) = trimmed_text(source, start, end) else {
        return false;
    };
    header
        .rsplit_once(')')
        .is_some_and(|(_, result)| contains_error_word(result))
}

// Syntax-only heuristic: this flags obvious Go error branches, not exact path coverage.
fn is_go_error_path_heuristic(
    source: &str,
    condition_text: &str,
    branch_node: Option<Node<'_>>,
    function_returns_error: bool,
) -> bool {
    condition_implies_error_edge(condition_text, "true")
        || branch_body_returns_error(source, branch_node, function_returns_error)
}

fn condition_implies_error_edge(condition_text: &str, edge_label: &str) -> bool {
    let compact = condition_text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    match edge_label {
        "true" => {
            compact.contains("err!=nil")
                || compact.contains("errors.is(")
                || compact.contains("errors.as(")
                || condition_text.to_ascii_lowercase().contains("error")
        }
        "false" => compact.contains("err==nil"),
        _ => false,
    }
}

fn branch_body_returns_error(
    source: &str,
    branch_node: Option<Node<'_>>,
    function_returns_error: bool,
) -> bool {
    function_returns_error
        && branch_node.is_some_and(|node| subtree_returns_error_looking_value(source, node))
}

fn subtree_returns_error_looking_value(source: &str, node: Node<'_>) -> bool {
    let mut returns_error = false;
    visit_named_descendants(node, &mut |descendant| {
        if returns_error || descendant.kind() != "return_statement" {
            return;
        }
        returns_error = return_statement_looks_error(source, descendant);
    });
    returns_error
}

fn return_statement_looks_error(source: &str, node: Node<'_>) -> bool {
    let Some(returned) = node_text(source, node)
        .map(str::trim)
        .and_then(|text| text.strip_prefix("return"))
        .map(str::trim)
    else {
        return false;
    };

    returned
        .split(',')
        .map(str::trim)
        .any(error_value_looks_error)
}

fn error_value_looks_error(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let trimmed = lower.trim_matches(|character: char| !character.is_ascii_alphanumeric());
    !matches!(trimmed, "" | "nil" | "true" | "false")
        && (trimmed == "err"
            || trimmed.starts_with("err")
            || trimmed.contains("error")
            || trimmed.contains("errors.")
            || trimmed.contains("fmt.errorf"))
}

fn contains_error_word(text: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|word| word == "error")
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
    fn extracts_go_string_literals_for_sdk_rules() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

const status = "blocked"

func Validate() {
	message := "invalid empty payment"
	token := `legacy-token`
	_, _, _ = status, message, token
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        let literals = db
            .string_literals()
            .iter()
            .map(|literal| (literal.value.as_str(), literal.language))
            .collect::<Vec<_>>();
        assert_eq!(
            literals,
            vec![
                ("blocked", Language::Go),
                ("invalid empty payment", Language::Go),
                ("legacy-token", Language::Go),
            ]
        );
    }

    #[test]
    fn does_not_duplicate_go_import_paths_as_string_literals() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

import "net/http"

func Validate() {
	_ = "blocked"
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        let literal_values = db
            .string_literals()
            .iter()
            .map(|literal| literal.value.as_str())
            .collect::<Vec<_>>();
        assert_eq!(literal_values, vec!["blocked"]);
        assert!(
            !literal_values.contains(&"net/http"),
            "import paths should remain ImportFact-only"
        );
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
        let mut extra_param_db = db_with_go_file(
            "payment_test.go",
            r#"package payment

import "testing"

func TestAuthorize(t *testing.T, extra int) {}
"#,
        );
        let mut near_match_db = db_with_go_file(
            "payment_test.go",
            r#"package payment

import "testing"

func TestAuthorize(t *testing.TB) {}
"#,
        );
        let mut unnamed_param_db = db_with_go_file(
            "payment_test.go",
            r#"package payment

import "testing"

func TestAuthorize(*testing.T) {}
"#,
        );

        let helper_diagnostics = analyze(&mut helper_db);
        let non_test_file_diagnostics = analyze(&mut non_test_file_db);
        let extra_param_diagnostics = analyze(&mut extra_param_db);
        let near_match_diagnostics = analyze(&mut near_match_db);
        let unnamed_param_diagnostics = analyze(&mut unnamed_param_db);

        assert!(helper_diagnostics.is_empty());
        assert!(non_test_file_diagnostics.is_empty());
        assert!(extra_param_diagnostics.is_empty());
        assert!(near_match_diagnostics.is_empty());
        assert!(unnamed_param_diagnostics.is_empty());
        assert_eq!(helper_db.tests().len(), 0);
        assert!(!helper_db.functions()[0].is_test);
        assert_eq!(non_test_file_db.tests().len(), 0);
        assert!(!non_test_file_db.functions()[0].is_test);
        assert_eq!(extra_param_db.tests().len(), 0);
        assert!(!extra_param_db.functions()[0].is_test);
        assert_eq!(near_match_db.tests().len(), 0);
        assert!(!near_match_db.functions()[0].is_test);
        assert_eq!(unnamed_param_db.tests().len(), 1);
        assert!(unnamed_param_db.functions()[0].is_test);
    }

    #[test]
    fn counts_multiline_go_table_rows_without_nested_literals() {
        let mut db = db_with_go_file(
            "payment_test.go",
            r#"package payment

import "testing"

type charge struct {
	ID string
}

func TestAuthorize(t *testing.T) {
	cases := []struct {
		name string
		charges []charge
		wantErr bool
	}{
		{
			name: "allowed",
			charges: []charge{
				{ID: "one"},
				{ID: "two"},
			},
			wantErr: false,
		},
		{
			name: "denied",
			charges: nil,
			wantErr: true,
		},
	}

	for _, tt := range cases {
		t.Run(tt.name, func(t *testing.T) {
			if tt.wantErr {
				t.Fatal("denied")
			}
		})
	}
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        assert_eq!(db.tests().len(), 1);
        assert_eq!(db.tests()[0].table_rows, 2);
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

    #[test]
    fn extracts_go_branch_obligations_from_control_flow() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

func Authorize(ok bool, kind string, value any, items []int, ch <-chan int) error {
	if ok {
		approve()
	} else {
		deny()
	}

	switch kind {
	case "card", "bank":
		approve()
	default:
		deny()
	}

	switch typed := value.(type) {
	case string:
		_ = typed
	default:
		deny()
	}

	for i := 0; i < len(items); i++ {
		_ = i
	}
	for _, item := range items {
		_ = item
	}

	select {
	case <-ch:
		approve()
	default:
		deny()
	}

	return nil
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        assert_eq!(db.functions().len(), 1);
        let function = db.functions()[0].id;
        assert!(
            db.branches()
                .iter()
                .all(|branch| branch.function == Some(function))
        );

        let branches = db
            .branches()
            .iter()
            .map(|branch| {
                (
                    branch.edge_label.as_str(),
                    branch.condition_text.as_str(),
                    branch.decision_span.start_line,
                )
            })
            .collect::<Vec<_>>();

        assert!(
            branches.contains(&("true", "ok", 4)),
            "missing if true branch: {branches:?}"
        );
        assert!(
            branches.contains(&("false", "ok", 4)),
            "missing if false branch: {branches:?}"
        );
        assert!(
            branches.contains(&("switch", "kind", 10)),
            "missing expression switch branch: {branches:?}"
        );
        assert!(
            branches.contains(&("case", r#"case "card", "bank":"#, 11)),
            "missing expression case branch: {branches:?}"
        );
        assert!(
            branches.contains(&("default", "default", 13)),
            "missing expression default branch: {branches:?}"
        );
        assert!(
            branches.contains(&("switch", "typed := value.(type)", 17)),
            "missing type switch branch: {branches:?}"
        );
        assert!(
            branches.contains(&("case", "case string:", 18)),
            "missing type case branch: {branches:?}"
        );
        assert!(
            branches.contains(&("loop", "for i := 0; i < len(items); i++", 24)),
            "missing ordinary for branch: {branches:?}"
        );
        assert!(
            branches.contains(&("range", "_, item := range items", 27)),
            "missing range branch: {branches:?}"
        );
        assert!(
            branches.contains(&("select", "select", 31)),
            "missing select branch: {branches:?}"
        );
        assert!(
            branches.contains(&("case", "case <-ch:", 32)),
            "missing select communication case branch: {branches:?}"
        );
    }

    #[test]
    fn branch_spans_come_from_tree_sitter_nodes() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

func Authorize(ok bool, kind string) {
	if ok {
		approve()
	}
	switch kind {
	case "card":
		approve()
	default:
		deny()
	}
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        let branches = db.branches();
        let if_true = branches
            .iter()
            .find(|branch| branch.edge_label == "true" && branch.condition_text == "ok")
            .expect("if true branch exists");
        assert_eq!(if_true.decision_span.start_line, 4);
        assert_eq!(if_true.decision_span.start_col, 5);
        assert_eq!(if_true.decision_span.end_col, 7);
        assert_ne!(if_true.decision_span.start_col, 1);

        let switch = branches
            .iter()
            .find(|branch| branch.edge_label == "switch" && branch.condition_text == "kind")
            .expect("switch branch exists");
        assert_eq!(switch.decision_span.start_line, 7);
        assert_eq!(switch.decision_span.start_col, 9);
        assert_eq!(switch.decision_span.end_col, 13);

        let case = branches
            .iter()
            .find(|branch| branch.edge_label == "case")
            .expect("case branch exists");
        assert_eq!(case.decision_span.start_line, 8);
        assert_eq!(case.decision_span.start_col, 2);
        assert_eq!(case.decision_span.end_col, 14);
    }

    #[test]
    fn marks_basic_go_error_paths_heuristically() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

import "errors"

var ErrDenied error

func Authorize(err error, target error, ok bool, shouldReject bool) error {
	if err != nil {
		return err
	}
	if err == nil {
		return ErrDenied
	}
	if errors.Is(err, target) {
		return err
	}
	if ok {
		return nil
	}
	if shouldReject {
		return ErrDenied
	}
	return nil
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        assert!(branch_for(&db, "err != nil", "true").is_error_path);
        assert!(branch_for(&db, "err == nil", "true").is_error_path);
        assert!(branch_for(&db, "errors.Is(err, target)", "true").is_error_path);
        assert!(branch_for(&db, "shouldReject", "true").is_error_path);
        assert!(!branch_for(&db, "ok", "true").is_error_path);
    }

    #[test]
    fn classifies_if_error_paths_by_edge_and_body() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

var ErrDenied error

func Authorize(err error, ok bool) error {
	if err != nil {
		return err
	}
	if ok {
		return nil
	} else {
		return ErrDenied
	}
	if err == nil {
		return nil
	}
	return err
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        assert!(branch_for(&db, "err != nil", "true").is_error_path);
        assert!(!branch_for(&db, "err != nil", "false").is_error_path);
        assert!(!branch_for(&db, "ok", "true").is_error_path);
        assert!(branch_for(&db, "ok", "false").is_error_path);
        assert!(!branch_for(&db, "err == nil", "true").is_error_path);
        assert!(branch_for(&db, "err == nil", "false").is_error_path);
    }

    #[test]
    fn marks_case_and_loop_error_returns_without_marking_whole_switch() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

var ErrInvalid error

func Authorize(kind string, items []int) error {
	switch kind {
	case "invalid":
		return ErrInvalid
	default:
		return nil
	}
	for i := 0; i < len(items); i++ {
		return ErrInvalid
	}
	return nil
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        assert!(!branch_for(&db, "kind", "switch").is_error_path);
        assert!(branch_for(&db, r#"case "invalid":"#, "case").is_error_path);
        assert!(!branch_for(&db, "default", "default").is_error_path);
        assert!(branch_for(&db, "for i := 0; i < len(items); i++", "loop").is_error_path);
    }

    #[test]
    fn ordinary_for_ignores_nested_range_clause() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

func Process(items []int) {
	for i := 0; i < 1; i++ {
		for _, item := range items {
			_ = item
		}
	}
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        let loop_branches = db
            .branches()
            .iter()
            .filter(|branch| branch.edge_label == "loop")
            .collect::<Vec<_>>();
        let range_branches = db
            .branches()
            .iter()
            .filter(|branch| branch.edge_label == "range")
            .collect::<Vec<_>>();
        assert_eq!(loop_branches.len(), 1, "{loop_branches:?}");
        assert_eq!(range_branches.len(), 1, "{range_branches:?}");
        assert_eq!(loop_branches[0].condition_text, "for i := 0; i < 1; i++");
        assert_eq!(range_branches[0].condition_text, "_, item := range items");
        assert!(
            range_branches[0].decision_span.start_line > loop_branches[0].decision_span.start_line
        );
    }

    #[test]
    fn case_headers_keep_colons_inside_literals_and_short_declarations() {
        let mut db = db_with_go_file(
            "payment.go",
            r#"package payment

func Process(kind string, ch <-chan string) {
	switch kind {
	case "bad:token":
		deny()
	case map[string]int{"a:b": 1}["a:b"]:
		deny()
	default:
		allow()
	}

	select {
	case msg := <-ch:
		_ = msg
	default:
		return
	}
}
"#,
        );

        let diagnostics = analyze(&mut db);

        assert!(diagnostics.is_empty());
        let case_headers = db
            .branches()
            .iter()
            .filter(|branch| branch.edge_label == "case")
            .map(|branch| branch.condition_text.as_str())
            .collect::<Vec<_>>();
        assert!(
            case_headers.contains(&r#"case "bad:token":"#),
            "{case_headers:?}"
        );
        assert!(
            case_headers.contains(&r#"case map[string]int{"a:b": 1}["a:b"]:"#),
            "{case_headers:?}"
        );
        assert!(
            case_headers.contains(&"case msg := <-ch:"),
            "{case_headers:?}"
        );
    }

    #[test]
    fn branch_fingerprints_are_stable_for_same_source() {
        let source = r#"package payment

func Authorize(err error, ok bool) error {
	if err != nil {
		return err
	}
	if ok {
		return nil
	}
	return nil
}
"#;
        let first = analyzed_go_file("payment.go", source);
        let second = analyzed_go_file("payment.go", source);

        let first_fingerprints = branch_fingerprints(&first);
        let second_fingerprints = branch_fingerprints(&second);

        assert_eq!(first_fingerprints, second_fingerprints);
        assert!(
            first_fingerprints
                .iter()
                .all(|fingerprint| !fingerprint.is_empty())
        );
    }

    #[test]
    fn branch_fingerprints_do_not_use_branch_ids() {
        let source = r#"package payment

func Authorize(err error) error {
	if err != nil {
		return err
	}
	return nil
}
"#;
        let baseline = analyzed_go_file("payment.go", source);
        let baseline_branch = branch_for(&baseline, "err != nil", "true");

        let mut shifted = AnalysisDb::new();
        shifted.add_file(
            PathBuf::from("other.go"),
            "other.go".to_string(),
            r#"package payment

func Other(ok bool) {
	if ok {
		return
	}
}
"#
            .to_string(),
        );
        shifted.add_file(
            PathBuf::from("payment.go"),
            "payment.go".to_string(),
            source.to_string(),
        );

        let diagnostics = analyze(&mut shifted);

        assert!(diagnostics.is_empty());
        let shifted_branch = shifted
            .branches()
            .iter()
            .find(|branch| {
                shifted.path_for(branch.file) == "payment.go"
                    && branch.condition_text == "err != nil"
                    && branch.edge_label == "true"
            })
            .expect("shifted branch exists");

        assert_ne!(baseline_branch.id, shifted_branch.id);
        assert_eq!(
            baseline_branch.stable_fingerprint,
            shifted_branch.stable_fingerprint
        );
    }

    fn analyzed_go_file(relative_path: &str, source: &str) -> AnalysisDb {
        let mut db = db_with_go_file(relative_path, source);
        let diagnostics = analyze(&mut db);
        assert!(diagnostics.is_empty());
        db
    }

    fn branch_for<'db>(
        db: &'db AnalysisDb,
        condition_text: &str,
        edge_label: &str,
    ) -> &'db BranchObligation {
        db.branches()
            .iter()
            .find(|branch| {
                branch.condition_text == condition_text && branch.edge_label == edge_label
            })
            .expect("branch exists")
    }

    fn branch_fingerprints(db: &AnalysisDb) -> Vec<String> {
        db.branches()
            .iter()
            .map(|branch| branch.stable_fingerprint.clone())
            .collect()
    }
}
