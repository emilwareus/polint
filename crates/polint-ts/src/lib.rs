use anyhow::{Context, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, ArrowFunctionExpression, BindingPattern, Class, ClassElement,
    Declaration, ExportDefaultDeclarationKind, Expression, ForStatementInit, ForStatementLeft,
    Function, FunctionBody, JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXChild,
    JSXElement, JSXExpression, JSXFragment, MethodDefinition, ObjectPropertyKind, Program,
    PropertyKey, Statement, TemplateLiteral, VariableDeclarator,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use polint_core::{
    AnalysisDb, FileId, FunctionFact, FunctionId, ImportFact, JsxAttributeFact, Language, Span,
    StringLiteralFact, TsClassFact, TsComponentFact, span_from_byte_range,
};
use polint_diagnostics::{Diagnostic, TextRange};
use std::path::Path;

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

fn parse_ts_file(db: &mut AnalysisDb, file_id: FileId) -> Result<Vec<Diagnostic>> {
    let file = db.file(file_id).context("missing source file")?.clone();
    let source = file.source.as_ref();
    let allocator = Allocator::default();
    let source_type = parse_source_type(&file.path);
    let parsed = Parser::new(&allocator, source, source_type).parse();
    let mut diagnostics = Vec::new();

    for error in &parsed.errors {
        let range = error
            .labels
            .as_ref()
            .and_then(|labels| labels.first())
            .map(|label| {
                span_from_byte_range(
                    file_id,
                    source,
                    label.offset(),
                    label.offset() + label.len(),
                )
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

    let import_count = db.imports().len();
    extract_from_program(db, file_id, source, file.language, &parsed.program);
    if db.imports().len() == import_count {
        let mut module_requests = parsed
            .module_record
            .requested_modules
            .iter()
            .flat_map(|(path, requests)| {
                requests
                    .iter()
                    .map(move |request| (request.span.start, request.statement_span, path.as_str()))
            })
            .collect::<Vec<_>>();
        module_requests.sort_by_key(|(start, _, _)| *start);

        for (_, statement_span, path) in module_requests {
            push_module_import(
                db,
                file_id,
                path,
                span_from_oxc(file_id, source, statement_span),
                file.language,
            );
        }
    }
    Ok(diagnostics)
}

fn parse_source_type(path: &Path) -> SourceType {
    SourceType::from_path(path).unwrap_or_default()
}

fn extract_from_program(
    db: &mut AnalysisDb,
    file_id: FileId,
    source: &str,
    language: Language,
    program: &Program<'_>,
) {
    extract_imports_and_exports(db, file_id, source, language, program);
    extract_declarations(db, file_id, source, language, program);
    extract_literals_and_jsx(db, file_id, source, language, program);
}

fn span_from_oxc(file: FileId, source: &str, span: oxc_span::Span) -> Span {
    span_from_byte_range(file, source, span.start as usize, span.end as usize)
}

fn extract_imports_and_exports(
    db: &mut AnalysisDb,
    file: FileId,
    source: &str,
    language: Language,
    program: &Program<'_>,
) {
    for statement in &program.body {
        match statement {
            Statement::ImportDeclaration(declaration) => {
                push_module_import(
                    db,
                    file,
                    declaration.source.value.as_str(),
                    span_from_oxc(file, source, declaration.source.span),
                    language,
                );
            }
            Statement::ExportAllDeclaration(declaration) => {
                push_module_import(
                    db,
                    file,
                    declaration.source.value.as_str(),
                    span_from_oxc(file, source, declaration.source.span),
                    language,
                );
            }
            Statement::ExportNamedDeclaration(declaration) => {
                if let Some(module_source) = &declaration.source {
                    push_module_import(
                        db,
                        file,
                        module_source.value.as_str(),
                        span_from_oxc(file, source, module_source.span),
                        language,
                    );
                }
            }
            _ => {}
        }
    }
}

fn push_module_import(
    db: &mut AnalysisDb,
    file: FileId,
    path: &str,
    span: Span,
    language: Language,
) {
    db.push_import(ImportFact {
        id: polint_core::ImportId(0),
        file,
        package: None,
        path: path.to_string(),
        span,
        language,
    });
}

#[derive(Clone, Copy)]
struct TsAstCtx<'a> {
    file: FileId,
    source: &'a str,
    language: Language,
}

fn extract_declarations(
    db: &mut AnalysisDb,
    file: FileId,
    source: &str,
    language: Language,
    program: &Program<'_>,
) {
    let ctx = TsAstCtx {
        file,
        source,
        language,
    };
    for statement in &program.body {
        match statement {
            Statement::FunctionDeclaration(function) => {
                if let Some(name) = function.id.as_ref().map(|id| id.name.to_string()) {
                    let is_component_like =
                        is_component_like_name(&name) || function_returns_jsx(function);
                    push_ts_function(
                        db,
                        ctx,
                        name,
                        function.span,
                        false,
                        function_body_calls(function.body.as_deref()),
                        is_component_like,
                    );
                }
            }
            Statement::VariableDeclaration(variable) => {
                for declarator in &variable.declarations {
                    extract_variable_declarator(db, file, source, language, declarator, false);
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(name) = class.id.as_ref().map(|id| id.name.to_string()) {
                    push_ts_class(db, file, source, language, name, class, false);
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(declaration) = &export.declaration {
                    extract_declaration(db, file, source, language, declaration, true);
                }
            }
            Statement::ExportDefaultDeclaration(export) => match &export.declaration {
                ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                    if let Some(name) = function.id.as_ref().map(|id| id.name.to_string()) {
                        let is_component_like =
                            is_component_like_name(&name) || function_returns_jsx(function);
                        push_ts_function(
                            db,
                            ctx,
                            name,
                            function.span,
                            true,
                            function_body_calls(function.body.as_deref()),
                            is_component_like,
                        );
                    }
                }
                ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                    if let Some(name) = class.id.as_ref().map(|id| id.name.to_string()) {
                        push_ts_class(db, file, source, language, name, class, true);
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

fn extract_declaration(
    db: &mut AnalysisDb,
    file: FileId,
    source: &str,
    language: Language,
    declaration: &Declaration<'_>,
    is_exported: bool,
) {
    let ctx = TsAstCtx {
        file,
        source,
        language,
    };
    match declaration {
        Declaration::FunctionDeclaration(function) => {
            if let Some(name) = function.id.as_ref().map(|id| id.name.to_string()) {
                let is_component_like =
                    is_component_like_name(&name) || function_returns_jsx(function);
                push_ts_function(
                    db,
                    ctx,
                    name,
                    function.span,
                    is_exported,
                    function_body_calls(function.body.as_deref()),
                    is_component_like,
                );
            }
        }
        Declaration::VariableDeclaration(variable) => {
            for declarator in &variable.declarations {
                extract_variable_declarator(db, file, source, language, declarator, is_exported);
            }
        }
        Declaration::ClassDeclaration(class) => {
            if let Some(name) = class.id.as_ref().map(|id| id.name.to_string()) {
                push_ts_class(db, file, source, language, name, class, is_exported);
            }
        }
        _ => {}
    }
}

fn extract_variable_declarator(
    db: &mut AnalysisDb,
    file: FileId,
    source: &str,
    language: Language,
    declarator: &VariableDeclarator<'_>,
    is_exported: bool,
) {
    let ctx = TsAstCtx {
        file,
        source,
        language,
    };
    let BindingPattern::BindingIdentifier(name) = &declarator.id else {
        return;
    };
    let Some(init) = &declarator.init else {
        return;
    };

    let name = name.name.to_string();
    match init {
        Expression::ArrowFunctionExpression(function) => {
            let is_component_like = is_component_like_name(&name) || arrow_returns_jsx(function);
            push_ts_function(
                db,
                ctx,
                name,
                declarator.span,
                is_exported,
                function_body_calls(Some(&function.body)),
                is_component_like,
            );
        }
        Expression::FunctionExpression(function) => {
            let is_component_like = is_component_like_name(&name) || function_returns_jsx(function);
            push_ts_function(
                db,
                ctx,
                name,
                declarator.span,
                is_exported,
                function_body_calls(function.body.as_deref()),
                is_component_like,
            );
        }
        Expression::ClassExpression(class) => {
            push_ts_class(db, file, source, language, name, class, is_exported);
        }
        _ => {}
    }
}

fn push_ts_function(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    name: String,
    span: oxc_span::Span,
    is_exported: bool,
    mut calls: Vec<String>,
    is_component_like: bool,
) -> FunctionId {
    calls.sort();
    calls.dedup();
    let span = span_from_oxc(ctx.file, ctx.source, span);
    let function_id = db.push_function(FunctionFact {
        id: FunctionId(0),
        file: ctx.file,
        name: name.clone(),
        span: span.clone(),
        language: ctx.language,
        is_test: name.contains("test") || name.contains("spec"),
        is_exported,
        cyclomatic_complexity: 1,
        calls,
    });

    if is_component_like {
        // syntax-level component heuristic: PascalCase or JSX-returning syntax only.
        db.push_ts_component(TsComponentFact {
            file: ctx.file,
            function: Some(function_id),
            name,
            span,
        });
    }

    function_id
}

fn push_ts_class(
    db: &mut AnalysisDb,
    file: FileId,
    source: &str,
    language: Language,
    name: String,
    class: &Class<'_>,
    is_exported: bool,
) {
    let is_component_like = is_component_like_name(&name);
    let span = span_from_oxc(file, source, class.span);
    db.push_ts_class(TsClassFact {
        file,
        name: name.clone(),
        span: span.clone(),
        is_exported,
        is_component_like,
    });

    if is_component_like {
        // syntax-level component heuristic: PascalCase classes are component-like only.
        db.push_ts_component(TsComponentFact {
            file,
            function: None,
            name: name.clone(),
            span,
        });
    }

    for element in &class.body.body {
        if let ClassElement::MethodDefinition(method) = element
            && let Some(method_name) = method_name(method)
        {
            push_ts_function(
                db,
                TsAstCtx {
                    file,
                    source,
                    language,
                },
                format!("{name}.{method_name}"),
                method.span,
                is_exported,
                function_body_calls(method.value.body.as_deref()),
                false,
            );
        }
    }
}

fn method_name(method: &MethodDefinition<'_>) -> Option<String> {
    match &method.key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

fn expression_calls(expression: &Expression<'_>) -> Vec<String> {
    let mut calls = Vec::new();
    collect_calls_from_expression(expression, &mut calls);
    calls.sort();
    calls.dedup();
    calls
}

fn function_body_calls(body: Option<&FunctionBody<'_>>) -> Vec<String> {
    let mut calls = Vec::new();
    if let Some(body) = body {
        for statement in &body.statements {
            collect_calls_from_statement(statement, &mut calls);
        }
    }
    calls.sort();
    calls.dedup();
    calls
}

fn collect_calls_from_statement(statement: &Statement<'_>, calls: &mut Vec<String>) {
    match statement {
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_calls_from_statement(statement, calls);
            }
        }
        Statement::ExpressionStatement(statement) => {
            calls.extend(expression_calls(&statement.expression));
        }
        Statement::ReturnStatement(statement) => {
            if let Some(argument) = &statement.argument {
                calls.extend(expression_calls(argument));
            }
        }
        Statement::IfStatement(statement) => {
            calls.extend(expression_calls(&statement.test));
            collect_calls_from_statement(&statement.consequent, calls);
            if let Some(alternate) = &statement.alternate {
                collect_calls_from_statement(alternate, calls);
            }
        }
        Statement::DoWhileStatement(statement) => {
            collect_calls_from_statement(&statement.body, calls);
            calls.extend(expression_calls(&statement.test));
        }
        Statement::WhileStatement(statement) => {
            calls.extend(expression_calls(&statement.test));
            collect_calls_from_statement(&statement.body, calls);
        }
        Statement::ForStatement(statement) => {
            if let Some(init) = &statement.init {
                collect_calls_from_for_init(init, calls);
            }
            if let Some(test) = &statement.test {
                calls.extend(expression_calls(test));
            }
            if let Some(update) = &statement.update {
                calls.extend(expression_calls(update));
            }
            collect_calls_from_statement(&statement.body, calls);
        }
        Statement::ForInStatement(statement) => {
            collect_calls_from_for_left(&statement.left, calls);
            calls.extend(expression_calls(&statement.right));
            collect_calls_from_statement(&statement.body, calls);
        }
        Statement::ForOfStatement(statement) => {
            collect_calls_from_for_left(&statement.left, calls);
            calls.extend(expression_calls(&statement.right));
            collect_calls_from_statement(&statement.body, calls);
        }
        Statement::SwitchStatement(statement) => {
            calls.extend(expression_calls(&statement.discriminant));
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    calls.extend(expression_calls(test));
                }
                for statement in &case.consequent {
                    collect_calls_from_statement(statement, calls);
                }
            }
        }
        Statement::ThrowStatement(statement) => {
            calls.extend(expression_calls(&statement.argument));
        }
        Statement::TryStatement(statement) => {
            for statement in &statement.block.body {
                collect_calls_from_statement(statement, calls);
            }
            if let Some(handler) = &statement.handler {
                for statement in &handler.body.body {
                    collect_calls_from_statement(statement, calls);
                }
            }
            if let Some(finalizer) = &statement.finalizer {
                for statement in &finalizer.body {
                    collect_calls_from_statement(statement, calls);
                }
            }
        }
        Statement::VariableDeclaration(variable) => {
            for declarator in &variable.declarations {
                if let Some(init) = &declarator.init {
                    collect_calls_from_expression(init, calls);
                }
            }
        }
        _ => {}
    }
}

fn collect_calls_from_for_init(init: &ForStatementInit<'_>, calls: &mut Vec<String>) {
    if let ForStatementInit::VariableDeclaration(variable) = init {
        for declarator in &variable.declarations {
            if let Some(init) = &declarator.init {
                collect_calls_from_expression(init, calls);
            }
        }
    }
}

fn collect_calls_from_for_left(left: &ForStatementLeft<'_>, calls: &mut Vec<String>) {
    if let ForStatementLeft::VariableDeclaration(variable) = left {
        for declarator in &variable.declarations {
            if let Some(init) = &declarator.init {
                collect_calls_from_expression(init, calls);
            }
        }
    }
}

fn collect_calls_from_expression(expression: &Expression<'_>, calls: &mut Vec<String>) {
    match expression {
        Expression::CallExpression(call) => {
            if let Some(name) = callee_text(&call.callee) {
                calls.push(name);
            }
            for argument in &call.arguments {
                collect_calls_from_argument(argument, calls);
            }
        }
        Expression::StaticMemberExpression(member) => {
            collect_calls_from_expression(&member.object, calls);
        }
        Expression::ArrayExpression(_) => {}
        Expression::AssignmentExpression(expression) => {
            collect_calls_from_expression(&expression.right, calls);
        }
        Expression::BinaryExpression(expression) => {
            collect_calls_from_expression(&expression.left, calls);
            collect_calls_from_expression(&expression.right, calls);
        }
        Expression::ConditionalExpression(expression) => {
            collect_calls_from_expression(&expression.test, calls);
            collect_calls_from_expression(&expression.consequent, calls);
            collect_calls_from_expression(&expression.alternate, calls);
        }
        Expression::LogicalExpression(expression) => {
            collect_calls_from_expression(&expression.left, calls);
            collect_calls_from_expression(&expression.right, calls);
        }
        Expression::ParenthesizedExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Expression::SequenceExpression(expression) => {
            for expression in &expression.expressions {
                collect_calls_from_expression(expression, calls);
            }
        }
        Expression::UnaryExpression(expression) => {
            collect_calls_from_expression(&expression.argument, calls);
        }
        Expression::UpdateExpression(_) => {}
        Expression::TSAsExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Expression::TSSatisfiesExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Expression::TSNonNullExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Expression::TSTypeAssertion(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        _ => {}
    }
}

fn collect_calls_from_argument(argument: &Argument<'_>, calls: &mut Vec<String>) {
    if let Argument::SpreadElement(spread) = argument {
        collect_calls_from_expression(&spread.argument, calls);
    }
}

fn callee_text(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => {
            callee_text(&member.object).map(|object| format!("{object}.{}", member.property.name))
        }
        Expression::ParenthesizedExpression(expression) => callee_text(&expression.expression),
        Expression::TSAsExpression(expression) => callee_text(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => callee_text(&expression.expression),
        Expression::TSNonNullExpression(expression) => callee_text(&expression.expression),
        Expression::TSTypeAssertion(expression) => callee_text(&expression.expression),
        _ => None,
    }
}

fn is_component_like_name(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_uppercase)
}

fn function_returns_jsx(function: &Function<'_>) -> bool {
    function
        .body
        .as_deref()
        .is_some_and(function_body_returns_jsx)
}

fn arrow_returns_jsx(function: &ArrowFunctionExpression<'_>) -> bool {
    function.get_expression().is_some_and(expression_is_jsx)
        || function_body_returns_jsx(&function.body)
}

fn function_body_returns_jsx(body: &FunctionBody<'_>) -> bool {
    body.statements.iter().any(statement_returns_jsx)
}

fn statement_returns_jsx(statement: &Statement<'_>) -> bool {
    match statement {
        Statement::BlockStatement(block) => block.body.iter().any(statement_returns_jsx),
        Statement::ReturnStatement(statement) => {
            statement.argument.as_ref().is_some_and(expression_is_jsx)
        }
        Statement::IfStatement(statement) => {
            statement_returns_jsx(&statement.consequent)
                || statement
                    .alternate
                    .as_ref()
                    .is_some_and(statement_returns_jsx)
        }
        _ => false,
    }
}

fn expression_is_jsx(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::JSXElement(_) | Expression::JSXFragment(_) => true,
        Expression::ParenthesizedExpression(expression) => {
            expression_is_jsx(&expression.expression)
        }
        Expression::TSAsExpression(expression) => expression_is_jsx(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => expression_is_jsx(&expression.expression),
        Expression::TSNonNullExpression(expression) => expression_is_jsx(&expression.expression),
        Expression::TSTypeAssertion(expression) => expression_is_jsx(&expression.expression),
        Expression::ConditionalExpression(expression) => {
            expression_is_jsx(&expression.consequent) || expression_is_jsx(&expression.alternate)
        }
        _ => false,
    }
}

fn extract_literals_and_jsx(
    db: &mut AnalysisDb,
    file: FileId,
    source: &str,
    language: Language,
    program: &Program<'_>,
) {
    let ctx = TsAstCtx {
        file,
        source,
        language,
    };
    for directive in &program.directives {
        push_string_literal_from_oxc(
            db,
            ctx,
            directive.expression.value.to_string(),
            directive.span,
        );
    }
    for statement in &program.body {
        walk_statement_for_literals(db, ctx, statement);
    }
}

fn walk_statement_for_literals(db: &mut AnalysisDb, ctx: TsAstCtx<'_>, statement: &Statement<'_>) {
    match statement {
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                walk_statement_for_literals(db, ctx, statement);
            }
        }
        Statement::ExpressionStatement(statement) => {
            walk_expression_for_literals(db, ctx, &statement.expression);
        }
        Statement::ReturnStatement(statement) => {
            if let Some(argument) = &statement.argument {
                walk_expression_for_literals(db, ctx, argument);
            }
        }
        Statement::IfStatement(statement) => {
            walk_expression_for_literals(db, ctx, &statement.test);
            walk_statement_for_literals(db, ctx, &statement.consequent);
            if let Some(alternate) = &statement.alternate {
                walk_statement_for_literals(db, ctx, alternate);
            }
        }
        Statement::DoWhileStatement(statement) => {
            walk_statement_for_literals(db, ctx, &statement.body);
            walk_expression_for_literals(db, ctx, &statement.test);
        }
        Statement::WhileStatement(statement) => {
            walk_expression_for_literals(db, ctx, &statement.test);
            walk_statement_for_literals(db, ctx, &statement.body);
        }
        Statement::ForStatement(statement) => {
            if let Some(init) = &statement.init {
                walk_for_init_for_literals(db, ctx, init);
            }
            if let Some(test) = &statement.test {
                walk_expression_for_literals(db, ctx, test);
            }
            if let Some(update) = &statement.update {
                walk_expression_for_literals(db, ctx, update);
            }
            walk_statement_for_literals(db, ctx, &statement.body);
        }
        Statement::ForInStatement(statement) => {
            walk_for_left_for_literals(db, ctx, &statement.left);
            walk_expression_for_literals(db, ctx, &statement.right);
            walk_statement_for_literals(db, ctx, &statement.body);
        }
        Statement::ForOfStatement(statement) => {
            walk_for_left_for_literals(db, ctx, &statement.left);
            walk_expression_for_literals(db, ctx, &statement.right);
            walk_statement_for_literals(db, ctx, &statement.body);
        }
        Statement::SwitchStatement(statement) => {
            walk_expression_for_literals(db, ctx, &statement.discriminant);
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    walk_expression_for_literals(db, ctx, test);
                }
                for statement in &case.consequent {
                    walk_statement_for_literals(db, ctx, statement);
                }
            }
        }
        Statement::ThrowStatement(statement) => {
            walk_expression_for_literals(db, ctx, &statement.argument);
        }
        Statement::TryStatement(statement) => {
            for statement in &statement.block.body {
                walk_statement_for_literals(db, ctx, statement);
            }
            if let Some(handler) = &statement.handler {
                for statement in &handler.body.body {
                    walk_statement_for_literals(db, ctx, statement);
                }
            }
            if let Some(finalizer) = &statement.finalizer {
                for statement in &finalizer.body {
                    walk_statement_for_literals(db, ctx, statement);
                }
            }
        }
        Statement::WithStatement(statement) => {
            walk_expression_for_literals(db, ctx, &statement.object);
            walk_statement_for_literals(db, ctx, &statement.body);
        }
        Statement::LabeledStatement(statement) => {
            walk_statement_for_literals(db, ctx, &statement.body);
        }
        Statement::VariableDeclaration(variable) => {
            walk_variable_declaration_for_literals(db, ctx, variable);
        }
        Statement::FunctionDeclaration(function) => {
            walk_function_for_literals(db, ctx, function);
        }
        Statement::ClassDeclaration(class) => {
            walk_class_for_literals(db, ctx, class);
        }
        Statement::ImportDeclaration(declaration) => {
            push_string_literal_from_oxc(
                db,
                ctx,
                declaration.source.value.to_string(),
                declaration.source.span,
            );
        }
        Statement::ExportAllDeclaration(declaration) => {
            push_string_literal_from_oxc(
                db,
                ctx,
                declaration.source.value.to_string(),
                declaration.source.span,
            );
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(source) = &declaration.source {
                push_string_literal_from_oxc(db, ctx, source.value.to_string(), source.span);
            }
            if let Some(declaration) = &declaration.declaration {
                walk_declaration_for_literals(db, ctx, declaration);
            }
        }
        Statement::ExportDefaultDeclaration(declaration) => {
            walk_export_default_for_literals(db, ctx, &declaration.declaration);
        }
        _ => {}
    }
}

fn walk_expression_for_literals(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    expression: &Expression<'_>,
) {
    match expression {
        Expression::StringLiteral(literal) => {
            push_string_literal_from_oxc(db, ctx, literal.value.to_string(), literal.span);
        }
        Expression::TemplateLiteral(template) => {
            push_template_literal_from_oxc(db, ctx, template);
        }
        Expression::TaggedTemplateExpression(tagged) => {
            walk_expression_for_literals(db, ctx, &tagged.tag);
            push_template_literal_from_oxc(db, ctx, &tagged.quasi);
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                walk_array_element_for_literals(db, ctx, element);
            }
        }
        Expression::ArrowFunctionExpression(function) => {
            walk_function_body_for_literals(db, ctx, &function.body);
        }
        Expression::AssignmentExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.right);
        }
        Expression::AwaitExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.argument);
        }
        Expression::BinaryExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.left);
            walk_expression_for_literals(db, ctx, &expression.right);
        }
        Expression::CallExpression(call) => {
            walk_expression_for_literals(db, ctx, &call.callee);
            for argument in &call.arguments {
                walk_argument_for_literals(db, ctx, argument);
            }
        }
        Expression::ClassExpression(class) => {
            walk_class_for_literals(db, ctx, class);
        }
        Expression::ConditionalExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.test);
            walk_expression_for_literals(db, ctx, &expression.consequent);
            walk_expression_for_literals(db, ctx, &expression.alternate);
        }
        Expression::FunctionExpression(function) => {
            walk_function_for_literals(db, ctx, function);
        }
        Expression::ImportExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.source);
        }
        Expression::LogicalExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.left);
            walk_expression_for_literals(db, ctx, &expression.right);
        }
        Expression::NewExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.callee);
            for argument in &expression.arguments {
                walk_argument_for_literals(db, ctx, argument);
            }
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        walk_property_key_for_literals(db, ctx, &property.key);
                        walk_expression_for_literals(db, ctx, &property.value);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        walk_expression_for_literals(db, ctx, &spread.argument);
                    }
                }
            }
        }
        Expression::ParenthesizedExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        Expression::SequenceExpression(expression) => {
            for expression in &expression.expressions {
                walk_expression_for_literals(db, ctx, expression);
            }
        }
        Expression::UnaryExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.argument);
        }
        Expression::JSXElement(element) => {
            extract_jsx_element_attributes(db, ctx, element);
        }
        Expression::JSXFragment(fragment) => {
            walk_jsx_fragment_for_literals(db, ctx, fragment);
        }
        Expression::TSAsExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        Expression::TSSatisfiesExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        Expression::TSNonNullExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        Expression::TSTypeAssertion(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        Expression::TSInstantiationExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        Expression::ComputedMemberExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.object);
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        Expression::StaticMemberExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.object);
        }
        Expression::PrivateFieldExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.object);
        }
        _ => {}
    }
}

fn push_string_literal_from_oxc(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    value: String,
    span: oxc_span::Span,
) {
    db.push_string_literal(StringLiteralFact {
        file: ctx.file,
        value,
        span: span_from_oxc(ctx.file, ctx.source, span),
        language: ctx.language,
    });
}

fn extract_jsx_element_attributes(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    element: &JSXElement<'_>,
) {
    for item in &element.opening_element.attributes {
        match item {
            JSXAttributeItem::Attribute(attribute) => {
                if let Some(name) = jsx_attribute_name(&attribute.name) {
                    db.push_jsx_attribute(JsxAttributeFact {
                        file: ctx.file,
                        name,
                        value: attribute.value.as_ref().and_then(jsx_attribute_value),
                        span: span_from_oxc(ctx.file, ctx.source, attribute.span),
                    });
                }
                if let Some(value) = &attribute.value {
                    walk_jsx_attribute_value_for_literals(db, ctx, value);
                }
            }
            JSXAttributeItem::SpreadAttribute(spread) => {
                walk_expression_for_literals(db, ctx, &spread.argument);
            }
        }
    }
    for child in &element.children {
        walk_jsx_child_for_literals(db, ctx, child);
    }
}

fn jsx_attribute_name(name: &JSXAttributeName<'_>) -> Option<String> {
    match name {
        JSXAttributeName::Identifier(identifier) => Some(identifier.name.to_string()),
        JSXAttributeName::NamespacedName(name) => {
            Some(format!("{}:{}", name.namespace.name, name.name.name))
        }
    }
}

fn jsx_attribute_value(value: &JSXAttributeValue<'_>) -> Option<String> {
    match value {
        JSXAttributeValue::StringLiteral(literal) => Some(literal.value.to_string()),
        JSXAttributeValue::ExpressionContainer(container) => {
            jsx_expression_static_value(&container.expression)
        }
        JSXAttributeValue::Element(_) | JSXAttributeValue::Fragment(_) => None,
    }
}

fn push_template_literal_from_oxc(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    template: &TemplateLiteral<'_>,
) {
    if template.expressions.is_empty() {
        push_string_literal_from_oxc(db, ctx, template_literal_value(template), template.span);
    } else {
        for quasi in &template.quasis {
            let value = template_element_value(quasi);
            if !value.is_empty() {
                push_string_literal_from_oxc(db, ctx, value, quasi.span);
            }
        }
        for expression in &template.expressions {
            walk_expression_for_literals(db, ctx, expression);
        }
    }
}

fn template_literal_value(template: &TemplateLiteral<'_>) -> String {
    template
        .quasis
        .iter()
        .map(template_element_value)
        .collect::<Vec<_>>()
        .join("")
}

fn template_element_value(element: &oxc_ast::ast::TemplateElement<'_>) -> String {
    element
        .value
        .cooked
        .as_ref()
        .unwrap_or(&element.value.raw)
        .to_string()
}

fn walk_declaration_for_literals(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    declaration: &Declaration<'_>,
) {
    match declaration {
        Declaration::VariableDeclaration(variable) => {
            walk_variable_declaration_for_literals(db, ctx, variable);
        }
        Declaration::FunctionDeclaration(function) => {
            walk_function_for_literals(db, ctx, function);
        }
        Declaration::ClassDeclaration(class) => {
            walk_class_for_literals(db, ctx, class);
        }
        _ => {}
    }
}

fn walk_export_default_for_literals(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    declaration: &ExportDefaultDeclarationKind<'_>,
) {
    match declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            walk_function_for_literals(db, ctx, function);
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            walk_class_for_literals(db, ctx, class);
        }
        ExportDefaultDeclarationKind::StringLiteral(literal) => {
            push_string_literal_from_oxc(db, ctx, literal.value.to_string(), literal.span);
        }
        ExportDefaultDeclarationKind::TemplateLiteral(template) => {
            push_template_literal_from_oxc(db, ctx, template);
        }
        ExportDefaultDeclarationKind::JSXElement(element) => {
            extract_jsx_element_attributes(db, ctx, element);
        }
        ExportDefaultDeclarationKind::JSXFragment(fragment) => {
            walk_jsx_fragment_for_literals(db, ctx, fragment);
        }
        _ => {}
    }
}

fn walk_variable_declaration_for_literals(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    variable: &oxc_ast::ast::VariableDeclaration<'_>,
) {
    for declarator in &variable.declarations {
        if let Some(init) = &declarator.init {
            walk_expression_for_literals(db, ctx, init);
        }
    }
}

fn walk_function_for_literals(db: &mut AnalysisDb, ctx: TsAstCtx<'_>, function: &Function<'_>) {
    if let Some(body) = function.body.as_deref() {
        walk_function_body_for_literals(db, ctx, body);
    }
}

fn walk_function_body_for_literals(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    body: &FunctionBody<'_>,
) {
    for directive in &body.directives {
        push_string_literal_from_oxc(
            db,
            ctx,
            directive.expression.value.to_string(),
            directive.span,
        );
    }
    for statement in &body.statements {
        walk_statement_for_literals(db, ctx, statement);
    }
}

fn walk_class_for_literals(db: &mut AnalysisDb, ctx: TsAstCtx<'_>, class: &Class<'_>) {
    if let Some(super_class) = &class.super_class {
        walk_expression_for_literals(db, ctx, super_class);
    }
    for element in &class.body.body {
        match element {
            ClassElement::StaticBlock(block) => {
                for statement in &block.body {
                    walk_statement_for_literals(db, ctx, statement);
                }
            }
            ClassElement::MethodDefinition(method) => {
                walk_property_key_for_literals(db, ctx, &method.key);
                walk_function_for_literals(db, ctx, &method.value);
            }
            ClassElement::PropertyDefinition(property) => {
                walk_property_key_for_literals(db, ctx, &property.key);
                if let Some(value) = &property.value {
                    walk_expression_for_literals(db, ctx, value);
                }
            }
            ClassElement::AccessorProperty(property) => {
                walk_property_key_for_literals(db, ctx, &property.key);
                if let Some(value) = &property.value {
                    walk_expression_for_literals(db, ctx, value);
                }
            }
            ClassElement::TSIndexSignature(_) => {}
        }
    }
}

fn walk_for_init_for_literals(db: &mut AnalysisDb, ctx: TsAstCtx<'_>, init: &ForStatementInit<'_>) {
    match init {
        ForStatementInit::VariableDeclaration(variable) => {
            walk_variable_declaration_for_literals(db, ctx, variable);
        }
        ForStatementInit::StringLiteral(literal) => {
            push_string_literal_from_oxc(db, ctx, literal.value.to_string(), literal.span);
        }
        ForStatementInit::TemplateLiteral(template) => {
            push_template_literal_from_oxc(db, ctx, template);
        }
        _ => {}
    }
}

fn walk_for_left_for_literals(db: &mut AnalysisDb, ctx: TsAstCtx<'_>, left: &ForStatementLeft<'_>) {
    if let ForStatementLeft::VariableDeclaration(variable) = left {
        walk_variable_declaration_for_literals(db, ctx, variable);
    }
}

fn walk_argument_for_literals(db: &mut AnalysisDb, ctx: TsAstCtx<'_>, argument: &Argument<'_>) {
    match argument {
        Argument::SpreadElement(spread) => walk_expression_for_literals(db, ctx, &spread.argument),
        Argument::StringLiteral(literal) => {
            push_string_literal_from_oxc(db, ctx, literal.value.to_string(), literal.span);
        }
        Argument::TemplateLiteral(template) => push_template_literal_from_oxc(db, ctx, template),
        Argument::TaggedTemplateExpression(tagged) => {
            walk_expression_for_literals(db, ctx, &tagged.tag);
            push_template_literal_from_oxc(db, ctx, &tagged.quasi);
        }
        Argument::ArrayExpression(array) => {
            for element in &array.elements {
                walk_array_element_for_literals(db, ctx, element);
            }
        }
        Argument::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        walk_property_key_for_literals(db, ctx, &property.key);
                        walk_expression_for_literals(db, ctx, &property.value);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        walk_expression_for_literals(db, ctx, &spread.argument);
                    }
                }
            }
        }
        Argument::CallExpression(call) => {
            walk_expression_for_literals(db, ctx, &call.callee);
            for argument in &call.arguments {
                walk_argument_for_literals(db, ctx, argument);
            }
        }
        Argument::ConditionalExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.test);
            walk_expression_for_literals(db, ctx, &expression.consequent);
            walk_expression_for_literals(db, ctx, &expression.alternate);
        }
        Argument::LogicalExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.left);
            walk_expression_for_literals(db, ctx, &expression.right);
        }
        Argument::ParenthesizedExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        Argument::JSXElement(element) => extract_jsx_element_attributes(db, ctx, element),
        Argument::JSXFragment(fragment) => walk_jsx_fragment_for_literals(db, ctx, fragment),
        Argument::TSAsExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        Argument::TSSatisfiesExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        Argument::TSNonNullExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        Argument::TSTypeAssertion(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        _ => {}
    }
}

fn walk_array_element_for_literals(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    element: &ArrayExpressionElement<'_>,
) {
    match element {
        ArrayExpressionElement::SpreadElement(spread) => {
            walk_expression_for_literals(db, ctx, &spread.argument);
        }
        ArrayExpressionElement::StringLiteral(literal) => {
            push_string_literal_from_oxc(db, ctx, literal.value.to_string(), literal.span);
        }
        ArrayExpressionElement::TemplateLiteral(template) => {
            push_template_literal_from_oxc(db, ctx, template);
        }
        ArrayExpressionElement::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        walk_property_key_for_literals(db, ctx, &property.key);
                        walk_expression_for_literals(db, ctx, &property.value);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        walk_expression_for_literals(db, ctx, &spread.argument);
                    }
                }
            }
        }
        ArrayExpressionElement::JSXElement(element) => {
            extract_jsx_element_attributes(db, ctx, element);
        }
        ArrayExpressionElement::JSXFragment(fragment) => {
            walk_jsx_fragment_for_literals(db, ctx, fragment);
        }
        _ => {}
    }
}

fn walk_property_key_for_literals(db: &mut AnalysisDb, ctx: TsAstCtx<'_>, key: &PropertyKey<'_>) {
    match key {
        PropertyKey::StringLiteral(literal) => {
            push_string_literal_from_oxc(db, ctx, literal.value.to_string(), literal.span);
        }
        PropertyKey::TemplateLiteral(template) => push_template_literal_from_oxc(db, ctx, template),
        _ => {}
    }
}

fn walk_jsx_attribute_value_for_literals(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    value: &JSXAttributeValue<'_>,
) {
    match value {
        JSXAttributeValue::ExpressionContainer(container) => {
            walk_jsx_expression_for_literals(db, ctx, &container.expression);
        }
        JSXAttributeValue::Element(element) => extract_jsx_element_attributes(db, ctx, element),
        JSXAttributeValue::Fragment(fragment) => walk_jsx_fragment_for_literals(db, ctx, fragment),
        JSXAttributeValue::StringLiteral(_) => {}
    }
}

fn walk_jsx_child_for_literals(db: &mut AnalysisDb, ctx: TsAstCtx<'_>, child: &JSXChild<'_>) {
    match child {
        JSXChild::Element(element) => extract_jsx_element_attributes(db, ctx, element),
        JSXChild::Fragment(fragment) => walk_jsx_fragment_for_literals(db, ctx, fragment),
        JSXChild::ExpressionContainer(container) => {
            walk_jsx_expression_for_literals(db, ctx, &container.expression);
        }
        JSXChild::Spread(spread) => walk_expression_for_literals(db, ctx, &spread.expression),
        JSXChild::Text(_) => {}
    }
}

fn walk_jsx_fragment_for_literals(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    fragment: &JSXFragment<'_>,
) {
    for child in &fragment.children {
        walk_jsx_child_for_literals(db, ctx, child);
    }
}

fn walk_jsx_expression_for_literals(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    expression: &JSXExpression<'_>,
) {
    match expression {
        JSXExpression::StringLiteral(literal) => {
            push_string_literal_from_oxc(db, ctx, literal.value.to_string(), literal.span);
        }
        JSXExpression::TemplateLiteral(template) => {
            push_template_literal_from_oxc(db, ctx, template);
        }
        JSXExpression::TaggedTemplateExpression(tagged) => {
            walk_expression_for_literals(db, ctx, &tagged.tag);
            push_template_literal_from_oxc(db, ctx, &tagged.quasi);
        }
        JSXExpression::ArrayExpression(array) => {
            for element in &array.elements {
                walk_array_element_for_literals(db, ctx, element);
            }
        }
        JSXExpression::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        walk_property_key_for_literals(db, ctx, &property.key);
                        walk_expression_for_literals(db, ctx, &property.value);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        walk_expression_for_literals(db, ctx, &spread.argument);
                    }
                }
            }
        }
        JSXExpression::CallExpression(call) => {
            walk_expression_for_literals(db, ctx, &call.callee);
            for argument in &call.arguments {
                walk_argument_for_literals(db, ctx, argument);
            }
        }
        JSXExpression::ConditionalExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.test);
            walk_expression_for_literals(db, ctx, &expression.consequent);
            walk_expression_for_literals(db, ctx, &expression.alternate);
        }
        JSXExpression::LogicalExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.left);
            walk_expression_for_literals(db, ctx, &expression.right);
        }
        JSXExpression::ParenthesizedExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        JSXExpression::JSXElement(element) => extract_jsx_element_attributes(db, ctx, element),
        JSXExpression::JSXFragment(fragment) => walk_jsx_fragment_for_literals(db, ctx, fragment),
        JSXExpression::TSAsExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        JSXExpression::TSSatisfiesExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        JSXExpression::TSNonNullExpression(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        JSXExpression::TSTypeAssertion(expression) => {
            walk_expression_for_literals(db, ctx, &expression.expression);
        }
        JSXExpression::EmptyExpression(_) => {}
        _ => {}
    }
}

fn jsx_expression_static_value(expression: &JSXExpression<'_>) -> Option<String> {
    match expression {
        JSXExpression::Identifier(identifier) => Some(identifier.name.to_string()),
        JSXExpression::StringLiteral(literal) => Some(literal.value.to_string()),
        JSXExpression::NumericLiteral(literal) => Some(
            literal
                .raw
                .as_ref()
                .map_or_else(|| literal.value.to_string(), ToString::to_string),
        ),
        JSXExpression::TemplateLiteral(template) if template.expressions.is_empty() => {
            Some(template_literal_value(template))
        }
        JSXExpression::ParenthesizedExpression(expression) => {
            expression_static_value(&expression.expression)
        }
        JSXExpression::TSAsExpression(expression) => {
            expression_static_value(&expression.expression)
        }
        JSXExpression::TSSatisfiesExpression(expression) => {
            expression_static_value(&expression.expression)
        }
        JSXExpression::TSNonNullExpression(expression) => {
            expression_static_value(&expression.expression)
        }
        JSXExpression::TSTypeAssertion(expression) => {
            expression_static_value(&expression.expression)
        }
        _ => None,
    }
}

fn expression_static_value(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
        Expression::NumericLiteral(literal) => Some(
            literal
                .raw
                .as_ref()
                .map_or_else(|| literal.value.to_string(), ToString::to_string),
        ),
        Expression::TemplateLiteral(template) if template.expressions.is_empty() => {
            Some(template_literal_value(template))
        }
        Expression::ParenthesizedExpression(expression) => {
            expression_static_value(&expression.expression)
        }
        Expression::TSAsExpression(expression) => expression_static_value(&expression.expression),
        Expression::TSSatisfiesExpression(expression) => {
            expression_static_value(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            expression_static_value(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => expression_static_value(&expression.expression),
        _ => None,
    }
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

    fn assert_no_parser_diagnostics(diagnostics: &[Diagnostic]) {
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.rule_id != "parser/ts"),
            "unexpected parser diagnostics: {diagnostics:?}"
        );
    }

    #[test]
    fn extracts_imports_and_export_from_specifiers_from_oxc_ast() {
        let source = r#"
import React from "react";
import { token } from "./tokens";
export { token } from "./tokens";
export * from "./shared";
"#;
        let (db, diagnostics) = analyze_source("imports.ts", source);
        assert_no_parser_diagnostics(&diagnostics);

        let paths: Vec<_> = db
            .imports()
            .iter()
            .map(|import| import.path.as_str())
            .collect();
        assert_eq!(paths, ["react", "./tokens", "./tokens", "./shared"]);
    }

    #[test]
    fn extracts_functions_arrows_classes_methods_and_calls_from_oxc_ast() {
        let source = r#"
function helper(label: string) {
  track(label);
  return formatLabel(label);
}

export const Button = () => helper("ok");

export class Dialog {
  render() {
    track("dialog");
    return formatLabel("dialog");
  }
}
"#;
        let (db, diagnostics) = analyze_source("component.tsx", source);
        assert_no_parser_diagnostics(&diagnostics);

        let function_names: Vec<_> = db
            .functions()
            .iter()
            .map(|function| function.name.as_str())
            .collect();
        assert_eq!(function_names, ["helper", "Button", "Dialog.render"]);

        let classes: Vec<_> = db.ts_classes().iter().collect();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name, "Dialog");
        assert!(classes[0].is_exported);
        assert!(classes[0].is_component_like);

        let helper = db
            .functions()
            .iter()
            .find(|function| function.name == "helper")
            .expect("expected helper function");
        assert_eq!(helper.calls, ["formatLabel", "track"]);

        let button = db
            .functions()
            .iter()
            .find(|function| function.name == "Button")
            .expect("expected Button function");
        assert!(button.is_exported);
        assert_eq!(button.calls, ["helper"]);

        let render = db
            .functions()
            .iter()
            .find(|function| function.name == "Dialog.render")
            .expect("expected Dialog.render method");
        assert_eq!(render.calls, ["formatLabel", "track"]);
    }

    #[test]
    fn detects_component_like_ts_facts_with_honest_heuristics() {
        let source = r#"
function helper() {
  return "ok";
}

function Button() {
  return label("ok");
}

const Link = () => <a href="/">Home</a>;

class Dialog {
  render() {
    return <section />;
  }
}

class store {}
"#;
        let (db, diagnostics) = analyze_source("components.tsx", source);
        assert_no_parser_diagnostics(&diagnostics);

        let component_names: Vec<_> = db
            .ts_components()
            .iter()
            .map(|component| component.name.as_str())
            .collect();
        assert_eq!(component_names, ["Button", "Link", "Dialog"]);

        assert!(
            db.ts_components()
                .iter()
                .all(|component| component.name != "helper" && component.name != "store"),
            "lower-case helpers without JSX should not be component facts"
        );
        assert!(
            db.ts_classes()
                .iter()
                .any(|class| class.name == "Dialog" && class.is_component_like)
        );
        assert!(
            db.ts_classes()
                .iter()
                .any(|class| class.name == "store" && !class.is_component_like)
        );

        let production_source = include_str!("lib.rs");
        assert!(
            production_source.contains("syntax-level component heuristic"),
            "component detection must identify itself as a heuristic"
        );
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
            ("util.js", "export function ok(value) { return value + 1; }"),
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
        let (db, diagnostics) =
            analyze_source("recoverable.ts", "import x from \"./x\";\nconst value = ;");

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

    #[test]
    fn parses_ts_source_from_shared_arc_without_full_source_clone() {
        let production_source = include_str!("lib.rs");
        let borrowed_source_access = ["file.source", ".as_ref()"].concat();
        let full_source_clone = ["file.source", ".to_string()"].concat();

        assert!(
            production_source.contains(&borrowed_source_access),
            "parse_ts_file should borrow from SourceFile.source"
        );
        assert!(
            !production_source.contains(&full_source_clone),
            "parse_ts_file should not clone the full source string"
        );
    }

    #[test]
    fn source_type_comes_from_file_path_for_ts_family() {
        let production_source = include_str!("lib.rs");
        let helper_signature = ["fn parse_source_type", "(path: &Path) -> SourceType"].concat();
        let source_type_from_path =
            ["SourceType::from_path", "(path).unwrap_or_default()"].concat();

        assert!(
            production_source.contains(&helper_signature),
            "expected parse_source_type helper"
        );
        assert!(
            production_source.contains(&source_type_from_path),
            "parse_source_type should derive SourceType from the file path"
        );
    }

    #[test]
    fn ast_helpers_preserve_source_byte_spans() {
        let source = "\nimport x from \"./x\";\n";
        let (db, diagnostics) = analyze_source("spans.ts", source);
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.rule_id != "parser/ts"),
            "unexpected parser diagnostics: {diagnostics:?}"
        );

        let import = db
            .imports()
            .iter()
            .find(|import| import.path == "./x")
            .expect("expected import fact for ./x");
        let start = source
            .find("\"./x\"")
            .expect("fixture should contain import");
        let end = start + "\"./x\"".len();
        assert_eq!(import.span.start_byte as usize, start);
        assert_eq!(import.span.end_byte as usize, end);

        let production_source = include_str!("lib.rs");
        let span_helper = ["fn span_from", "_oxc"].concat();
        let span_conversion = [
            "span_from_byte_range(file, source, ",
            "span.start as usize, span.end as usize)",
        ]
        .concat();
        assert!(
            production_source.contains(&span_helper),
            "expected span_from_oxc helper"
        );
        assert!(
            production_source.contains(&span_conversion),
            "span_from_oxc should convert Oxc byte spans through core span conversion"
        );
    }

    #[test]
    fn extracts_string_literals_and_static_templates_from_oxc_ast() {
        let source = r#"
const plain = "plain";
const single = 'single';
const staticTemplate = `static-template`;
const dynamicTemplate = `prefix-${name}`;
"#;
        let (db, diagnostics) = analyze_source("literals.ts", source);
        assert_no_parser_diagnostics(&diagnostics);

        let values: Vec<_> = db
            .string_literals()
            .iter()
            .map(|literal| literal.value.as_str())
            .collect();
        assert_eq!(values, ["plain", "single", "static-template", "prefix-"]);
        assert!(
            !values.iter().any(|value| value.contains("${")),
            "dynamic template expressions should not be recorded as exact strings: {values:?}"
        );
    }

    #[test]
    fn extracts_jsx_attributes_from_oxc_ast() {
        let source = r#"
const View = ({ token }) => (
  <Button aria-label="Pay" data-token={token} disabled {...spread} />
);
"#;
        let (db, diagnostics) = analyze_source("component.tsx", source);
        assert_no_parser_diagnostics(&diagnostics);

        let attributes: Vec<_> = db
            .jsx_attributes()
            .iter()
            .map(|attribute| (attribute.name.as_str(), attribute.value.as_deref()))
            .collect();
        assert_eq!(
            attributes,
            [
                ("aria-label", Some("Pay")),
                ("data-token", Some("token")),
                ("disabled", None)
            ]
        );
    }

    #[test]
    fn raw_color_literals_are_available_from_strings_and_jsx_attributes() {
        let source = r##"
const rawColor = "#ff00aa";
export function Button() {
  return <Button data-color="#00ff00" />;
}
"##;
        let (db, diagnostics) = analyze_source("raw-colors.tsx", source);
        assert_no_parser_diagnostics(&diagnostics);

        assert!(
            db.string_literals()
                .iter()
                .any(|literal| literal.value == "#ff00aa"),
            "expected raw color string literal fact"
        );
        assert!(
            db.jsx_attributes().iter().any(|attribute| {
                attribute.name == "data-color" && attribute.value.as_deref() == Some("#00ff00")
            }),
            "expected raw color JSX attribute fact"
        );
    }
}
