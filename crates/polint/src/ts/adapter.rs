use crate::analysis_kernel::incremental::CacheStats;
use crate::analysis_plan::AnalysisPlan;
use crate::cache::{CacheReadStatus, CacheWriteStatus};
use crate::core::{
    AnalysisDb, CachedFileAnalysis, FileId, FunctionFact, FunctionId, ImportFact, JsxAttributeFact,
    Language, SourceFile, Span, StringLiteralFact, TsClassFact, TsComponentFact,
    span_from_byte_range,
};
use crate::diagnostics::{Diagnostic, TextRange};
use anyhow::{Context, Result};
use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, ArrowFunctionExpression, BindingPattern, Class, ClassElement,
    Declaration, ExportDefaultDeclarationKind, Expression, ForStatementInit, ForStatementLeft,
    Function, FunctionBody, ImportOrExportKind, JSXAttributeItem, JSXAttributeName,
    JSXAttributeValue, JSXChild, JSXElement, JSXExpression, JSXFragment, MethodDefinition,
    ModuleExportName, ObjectPropertyKind, Program, PropertyKey, RegExpLiteral, Statement,
    TemplateLiteral, VariableDeclarator,
};
use oxc_parser::Parser;
use oxc_span::SourceType;
use rayon::prelude::*;
use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

const TS_CACHE_SCHEMA: &str = "ts-facts-v1";

// Relationship resolution converts this non-string import expression sentinel to Dynamic.
pub(crate) const DYNAMIC_IMPORT_SPECIFIER: &str = "<dynamic>";

/// Convenience wrapper used by TS-adapter unit tests (no cache, sequential).
#[cfg(test)]
pub(crate) fn analyze(db: &mut AnalysisDb) -> Vec<Diagnostic> {
    let cache = crate::cache::Cache::new("", false);
    analyze_with_options(db, &cache, "", "", false)
}

/// Sequential, cache-aware analysis used by TS-adapter unit tests only.
#[cfg(test)]
pub(crate) fn analyze_with_cache(
    db: &mut AnalysisDb,
    cache: &crate::cache::Cache,
    config_hash: &str,
    rule_hash: &str,
) -> Vec<Diagnostic> {
    analyze_with_options(db, cache, config_hash, rule_hash, false)
}

// `pub` for `polint::_bench::ts` / `polint-bench`, not for direct downstream use.
#[allow(dead_code, unreachable_pub)]
pub fn analyze_with_options(
    db: &mut AnalysisDb,
    cache: &crate::cache::Cache,
    config_hash: &str,
    rule_hash: &str,
    parallel: bool,
) -> Vec<Diagnostic> {
    let plan = AnalysisPlan::empty();
    analyze_with_plan_options(db, cache, config_hash, rule_hash, &plan, parallel)
}

pub(crate) fn analyze_with_plan_options(
    db: &mut AnalysisDb,
    cache: &crate::cache::Cache,
    config_hash: &str,
    rule_hash: &str,
    plan: &AnalysisPlan,
    parallel: bool,
) -> Vec<Diagnostic> {
    analyze_with_plan_options_and_cache_stats(db, cache, config_hash, rule_hash, plan, parallel)
        .diagnostics
}

#[allow(dead_code)]
pub(crate) struct ProviderAnalysisResult {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
}

pub(crate) fn analyze_with_plan_options_and_cache_stats(
    db: &mut AnalysisDb,
    cache: &crate::cache::Cache,
    config_hash: &str,
    rule_hash: &str,
    plan: &AnalysisPlan,
    parallel: bool,
) -> ProviderAnalysisResult {
    let files: Vec<&SourceFile> = db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
        .collect();

    let mut results = if parallel {
        files
            .par_iter()
            .map(|file| analyze_ts_source_file(file, cache, config_hash, rule_hash, plan.digest()))
            .collect::<Vec<_>>()
    } else {
        files
            .iter()
            .map(|file| analyze_ts_source_file(file, cache, config_hash, rule_hash, plan.digest()))
            .collect::<Vec<_>>()
    };
    results.sort_by_key(|result| result.file_id);

    let mut diagnostics = Vec::new();
    let mut cache_stats = CacheStats::default();
    for result in results {
        record_cache_event(&mut cache_stats, result.cache_event);
        if let Some(facts) = result.facts {
            db.restore_file_facts(result.file_id, facts);
        }
        diagnostics.extend(result.diagnostics);
    }
    ProviderAnalysisResult {
        diagnostics,
        cache_stats,
    }
}

struct TsFileAnalysis {
    file_id: FileId,
    diagnostics: Vec<Diagnostic>,
    facts: Option<crate::core::CachedFileFacts>,
    cache_event: FileCacheEvent,
}

#[derive(Clone, Copy)]
struct FileCacheEvent {
    read_status: CacheReadStatus,
    write_status: Option<CacheWriteStatus>,
}

impl FileCacheEvent {
    fn new(read_status: CacheReadStatus) -> Self {
        Self {
            read_status,
            write_status: None,
        }
    }
}

fn analyze_ts_source_file(
    file: &SourceFile,
    cache: &crate::cache::Cache,
    config_hash: &str,
    rule_hash: &str,
    plan_hash: &str,
) -> TsFileAnalysis {
    let key = crate::cache::CacheKey::for_file(
        file.relative_path.as_str(),
        file.content_hash.as_str(),
        config_hash,
        rule_hash,
        plan_hash,
        TS_CACHE_SCHEMA,
    );
    let read = cache.read_json_with_status::<CachedFileAnalysis>(&key);
    if read.status == CacheReadStatus::Hit
        && let Some(cached) = read.value
        && cached.schema == TS_CACHE_SCHEMA
    {
        return TsFileAnalysis {
            file_id: file.id,
            diagnostics: cached.diagnostics,
            facts: Some(cached.facts),
            cache_event: FileCacheEvent::new(CacheReadStatus::Hit),
        };
    }
    let read_status = if read.status == CacheReadStatus::Hit {
        CacheReadStatus::Miss
    } else {
        read.status
    };

    let mut local_db = AnalysisDb::new();
    let local_file = local_db.add_source_file(
        file.path.clone(),
        file.relative_path.clone(),
        file.language,
        Arc::clone(&file.source),
        file.content_hash.clone(),
    );
    let mut diagnostics = match parse_ts_file(&mut local_db, local_file) {
        Ok(file_diagnostics) => file_diagnostics,
        Err(error) => {
            return TsFileAnalysis {
                file_id: file.id,
                diagnostics: vec![Diagnostic::error(
                    "parser/ts",
                    file.relative_path.clone(),
                    TextRange::point(1, 1),
                    format!("Failed to parse TS/JS file: {error}"),
                )],
                facts: None,
                cache_event: FileCacheEvent::new(read_status),
            };
        }
    };
    let facts = local_db.facts_for_file(local_file);
    let cached = CachedFileAnalysis {
        schema: TS_CACHE_SCHEMA.to_string(),
        diagnostics: diagnostics.clone(),
        facts: facts.clone(),
    };
    let mut cache_event = FileCacheEvent::new(read_status);
    match cache.write_json_with_status(&key, &cached) {
        Ok(status) => {
            cache_event.write_status = Some(status);
        }
        Err(error) => {
            diagnostics.push(cache_write_diagnostic(file.relative_path.as_str(), error));
        }
    };
    TsFileAnalysis {
        file_id: file.id,
        diagnostics,
        facts: Some(facts),
        cache_event,
    }
}

fn record_cache_event(stats: &mut CacheStats, event: FileCacheEvent) {
    match event.read_status {
        CacheReadStatus::Disabled => {
            stats.record_disabled_bypass();
            stats.record_recompute();
        }
        CacheReadStatus::Miss => {
            stats.record_miss();
            stats.record_recompute();
        }
        CacheReadStatus::Hit => stats.record_hit(),
        CacheReadStatus::InvalidEvicted => {
            stats.record_invalid_evicted_read();
            stats.record_recompute();
        }
    }
    if event.write_status == Some(CacheWriteStatus::Written) {
        stats.record_write();
    }
}

fn cache_write_diagnostic(path: &str, error: anyhow::Error) -> Diagnostic {
    Diagnostic::warning(
        "internal/cache",
        path,
        TextRange::point(1, 1),
        format!("cache write failed: {error}"),
    )
}

fn parse_ts_file(db: &mut AnalysisDb, file_id: FileId) -> Result<Vec<Diagnostic>> {
    let (source, language, path) = {
        let file = db.file(file_id).context("missing source file")?;
        (Arc::clone(&file.source), file.language, file.path.clone())
    };
    let source = source.as_ref();
    let allocator = Allocator::default();
    let source_type = parse_source_type(&path);
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
    extract_from_program(db, file_id, source, language, &parsed.program);
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
                language,
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
    extract_commonjs_require_imports(db, file, source, language, program);
}

fn extract_commonjs_require_imports(
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
        collect_require_imports_from_statement(db, ctx, statement);
    }
}

fn collect_require_imports_from_statement(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    statement: &Statement<'_>,
) {
    match statement {
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_require_imports_from_statement(db, ctx, statement);
            }
        }
        Statement::ExpressionStatement(statement) => {
            collect_require_imports_from_expression(db, ctx, &statement.expression);
        }
        Statement::ReturnStatement(statement) => {
            if let Some(argument) = &statement.argument {
                collect_require_imports_from_expression(db, ctx, argument);
            }
        }
        Statement::IfStatement(statement) => {
            collect_require_imports_from_expression(db, ctx, &statement.test);
            collect_require_imports_from_statement(db, ctx, &statement.consequent);
            if let Some(alternate) = &statement.alternate {
                collect_require_imports_from_statement(db, ctx, alternate);
            }
        }
        Statement::DoWhileStatement(statement) => {
            collect_require_imports_from_statement(db, ctx, &statement.body);
            collect_require_imports_from_expression(db, ctx, &statement.test);
        }
        Statement::WhileStatement(statement) => {
            collect_require_imports_from_expression(db, ctx, &statement.test);
            collect_require_imports_from_statement(db, ctx, &statement.body);
        }
        Statement::ForStatement(statement) => {
            if let Some(init) = &statement.init {
                collect_require_imports_from_for_init(db, ctx, init);
            }
            if let Some(test) = &statement.test {
                collect_require_imports_from_expression(db, ctx, test);
            }
            if let Some(update) = &statement.update {
                collect_require_imports_from_expression(db, ctx, update);
            }
            collect_require_imports_from_statement(db, ctx, &statement.body);
        }
        Statement::ForInStatement(statement) => {
            collect_require_imports_from_expression(db, ctx, &statement.right);
            collect_require_imports_from_statement(db, ctx, &statement.body);
        }
        Statement::ForOfStatement(statement) => {
            collect_require_imports_from_expression(db, ctx, &statement.right);
            collect_require_imports_from_statement(db, ctx, &statement.body);
        }
        Statement::SwitchStatement(statement) => {
            collect_require_imports_from_expression(db, ctx, &statement.discriminant);
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    collect_require_imports_from_expression(db, ctx, test);
                }
                for statement in &case.consequent {
                    collect_require_imports_from_statement(db, ctx, statement);
                }
            }
        }
        Statement::ThrowStatement(statement) => {
            collect_require_imports_from_expression(db, ctx, &statement.argument);
        }
        Statement::TryStatement(statement) => {
            for statement in &statement.block.body {
                collect_require_imports_from_statement(db, ctx, statement);
            }
            if let Some(handler) = &statement.handler {
                for statement in &handler.body.body {
                    collect_require_imports_from_statement(db, ctx, statement);
                }
            }
            if let Some(finalizer) = &statement.finalizer {
                for statement in &finalizer.body {
                    collect_require_imports_from_statement(db, ctx, statement);
                }
            }
        }
        Statement::VariableDeclaration(variable) => {
            for declarator in &variable.declarations {
                if let Some(init) = &declarator.init {
                    collect_require_imports_from_expression(db, ctx, init);
                }
            }
        }
        Statement::FunctionDeclaration(function) => {
            collect_require_imports_from_function(db, ctx, function);
        }
        Statement::ClassDeclaration(class) => {
            collect_require_imports_from_class(db, ctx, class);
        }
        Statement::ExportNamedDeclaration(declaration) => {
            if let Some(declaration) = &declaration.declaration {
                collect_require_imports_from_declaration(db, ctx, declaration);
            }
        }
        Statement::ExportDefaultDeclaration(declaration) => {
            collect_require_imports_from_export_default(db, ctx, &declaration.declaration);
        }
        _ => {}
    }
}

fn collect_require_imports_from_expression(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    expression: &Expression<'_>,
) {
    match expression {
        Expression::CallExpression(call) => {
            if callee_text(&call.callee).as_deref() == Some("require")
                && let Some(Argument::StringLiteral(path)) = call.arguments.first()
            {
                push_module_import(
                    db,
                    ctx.file,
                    path.value.as_str(),
                    span_from_oxc(ctx.file, ctx.source, path.span),
                    ctx.language,
                );
            }
            collect_require_imports_from_expression(db, ctx, &call.callee);
            for argument in &call.arguments {
                collect_require_imports_from_argument(db, ctx, argument);
            }
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                collect_require_imports_from_array_element(db, ctx, element);
            }
        }
        Expression::AssignmentExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.right);
        }
        Expression::AwaitExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.argument);
        }
        Expression::ArrowFunctionExpression(function) => {
            collect_require_imports_from_arrow_function(db, ctx, function);
        }
        Expression::BinaryExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.left);
            collect_require_imports_from_expression(db, ctx, &expression.right);
        }
        Expression::ClassExpression(class) => collect_require_imports_from_class(db, ctx, class),
        Expression::ConditionalExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.test);
            collect_require_imports_from_expression(db, ctx, &expression.consequent);
            collect_require_imports_from_expression(db, ctx, &expression.alternate);
        }
        Expression::FunctionExpression(function) => {
            collect_require_imports_from_function(db, ctx, function);
        }
        Expression::ImportExpression(import_expression) => {
            push_dynamic_import_expression(db, ctx, import_expression);
        }
        Expression::LogicalExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.left);
            collect_require_imports_from_expression(db, ctx, &expression.right);
        }
        Expression::NewExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.callee);
            for argument in &expression.arguments {
                collect_require_imports_from_argument(db, ctx, argument);
            }
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        collect_require_imports_from_expression(db, ctx, &property.value);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        collect_require_imports_from_expression(db, ctx, &spread.argument);
                    }
                }
            }
        }
        Expression::ParenthesizedExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.expression);
        }
        Expression::SequenceExpression(expression) => {
            for expression in &expression.expressions {
                collect_require_imports_from_expression(db, ctx, expression);
            }
        }
        Expression::TaggedTemplateExpression(tagged) => {
            collect_require_imports_from_expression(db, ctx, &tagged.tag);
            for expression in &tagged.quasi.expressions {
                collect_require_imports_from_expression(db, ctx, expression);
            }
        }
        Expression::UnaryExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.argument);
        }
        Expression::YieldExpression(expression) => {
            if let Some(argument) = &expression.argument {
                collect_require_imports_from_expression(db, ctx, argument);
            }
        }
        Expression::TSAsExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.expression);
        }
        Expression::TSSatisfiesExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.expression);
        }
        Expression::TSNonNullExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.expression);
        }
        Expression::TSTypeAssertion(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.expression);
        }
        Expression::TSInstantiationExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.expression);
        }
        Expression::ComputedMemberExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.object);
            collect_require_imports_from_expression(db, ctx, &expression.expression);
        }
        Expression::StaticMemberExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.object);
        }
        Expression::PrivateFieldExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.object);
        }
        _ => {}
    }
}

fn collect_require_imports_from_declaration(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    declaration: &Declaration<'_>,
) {
    match declaration {
        Declaration::VariableDeclaration(variable) => {
            for declarator in &variable.declarations {
                if let Some(init) = &declarator.init {
                    collect_require_imports_from_expression(db, ctx, init);
                }
            }
        }
        Declaration::FunctionDeclaration(function) => {
            collect_require_imports_from_function(db, ctx, function);
        }
        Declaration::ClassDeclaration(class) => {
            collect_require_imports_from_class(db, ctx, class);
        }
        _ => {}
    }
}

fn collect_require_imports_from_export_default(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    declaration: &ExportDefaultDeclarationKind<'_>,
) {
    match declaration {
        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
            collect_require_imports_from_function(db, ctx, function);
        }
        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
            collect_require_imports_from_class(db, ctx, class);
        }
        ExportDefaultDeclarationKind::CallExpression(call) => {
            if callee_text(&call.callee).as_deref() == Some("require")
                && let Some(Argument::StringLiteral(path)) = call.arguments.first()
            {
                push_module_import(
                    db,
                    ctx.file,
                    path.value.as_str(),
                    span_from_oxc(ctx.file, ctx.source, path.span),
                    ctx.language,
                );
            }
            for argument in &call.arguments {
                collect_require_imports_from_argument(db, ctx, argument);
            }
        }
        _ => {}
    }
}

fn collect_require_imports_from_function(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    function: &Function<'_>,
) {
    if let Some(body) = function.body.as_deref() {
        for statement in &body.statements {
            collect_require_imports_from_statement(db, ctx, statement);
        }
    }
}

fn collect_require_imports_from_arrow_function(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    function: &ArrowFunctionExpression<'_>,
) {
    if let Some(expression) = function.get_expression() {
        collect_require_imports_from_expression(db, ctx, expression);
    } else {
        for statement in &function.body.statements {
            collect_require_imports_from_statement(db, ctx, statement);
        }
    }
}

fn collect_require_imports_from_class(db: &mut AnalysisDb, ctx: TsAstCtx<'_>, class: &Class<'_>) {
    if let Some(super_class) = &class.super_class {
        collect_require_imports_from_expression(db, ctx, super_class);
    }
    for element in &class.body.body {
        match element {
            ClassElement::StaticBlock(block) => {
                for statement in &block.body {
                    collect_require_imports_from_statement(db, ctx, statement);
                }
            }
            ClassElement::MethodDefinition(method) => {
                collect_require_imports_from_function(db, ctx, &method.value);
            }
            ClassElement::PropertyDefinition(property) => {
                if let Some(value) = &property.value {
                    collect_require_imports_from_expression(db, ctx, value);
                }
            }
            ClassElement::AccessorProperty(property) => {
                if let Some(value) = &property.value {
                    collect_require_imports_from_expression(db, ctx, value);
                }
            }
            ClassElement::TSIndexSignature(_) => {}
        }
    }
}

fn collect_require_imports_from_for_init(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    init: &ForStatementInit<'_>,
) {
    match init {
        ForStatementInit::VariableDeclaration(variable) => {
            for declarator in &variable.declarations {
                if let Some(init) = &declarator.init {
                    collect_require_imports_from_expression(db, ctx, init);
                }
            }
        }
        ForStatementInit::CallExpression(call) => {
            if callee_text(&call.callee).as_deref() == Some("require")
                && let Some(Argument::StringLiteral(path)) = call.arguments.first()
            {
                push_module_import(
                    db,
                    ctx.file,
                    path.value.as_str(),
                    span_from_oxc(ctx.file, ctx.source, path.span),
                    ctx.language,
                );
            }
            for argument in &call.arguments {
                collect_require_imports_from_argument(db, ctx, argument);
            }
        }
        ForStatementInit::ParenthesizedExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.expression);
        }
        _ => {}
    }
}

fn collect_require_imports_from_argument(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    argument: &Argument<'_>,
) {
    match argument {
        Argument::SpreadElement(spread) => {
            collect_require_imports_from_expression(db, ctx, &spread.argument);
        }
        Argument::CallExpression(call) => {
            if callee_text(&call.callee).as_deref() == Some("require")
                && let Some(Argument::StringLiteral(path)) = call.arguments.first()
            {
                push_module_import(
                    db,
                    ctx.file,
                    path.value.as_str(),
                    span_from_oxc(ctx.file, ctx.source, path.span),
                    ctx.language,
                );
            }
            for argument in &call.arguments {
                collect_require_imports_from_argument(db, ctx, argument);
            }
        }
        Argument::ParenthesizedExpression(expression) => {
            collect_require_imports_from_expression(db, ctx, &expression.expression);
        }
        Argument::ImportExpression(import_expression) => {
            push_dynamic_import_expression(db, ctx, import_expression);
        }
        _ => {}
    }
}

fn push_dynamic_import_expression(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    import_expression: &oxc_ast::ast::ImportExpression<'_>,
) {
    match &import_expression.source {
        Expression::StringLiteral(path) => {
            push_module_import(
                db,
                ctx.file,
                path.value.as_str(),
                span_from_oxc(ctx.file, ctx.source, path.span),
                ctx.language,
            );
        }
        _ => {
            push_module_import(
                db,
                ctx.file,
                DYNAMIC_IMPORT_SPECIFIER,
                span_from_oxc(ctx.file, ctx.source, import_expression.span),
                ctx.language,
            );
        }
    }
}

fn collect_require_imports_from_array_element(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    element: &ArrayExpressionElement<'_>,
) {
    if let Some(expression) = element.as_expression() {
        collect_require_imports_from_expression(db, ctx, expression);
    } else if let ArrayExpressionElement::SpreadElement(spread) = element {
        collect_require_imports_from_expression(db, ctx, &spread.argument);
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
        id: crate::core::ImportId(0),
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
    let exported_names = exported_local_names(program);
    for statement in &program.body {
        match statement {
            Statement::FunctionDeclaration(function) => {
                if let Some(name) = function.id.as_ref().map(|id| id.name.to_string()) {
                    let is_exported = exported_names.contains(name.as_str());
                    let is_component_like =
                        is_component_like_name(&name) || function_returns_jsx(function);
                    push_ts_function(
                        db,
                        ctx,
                        TsFunctionSpec {
                            name,
                            span: function.span,
                            is_exported,
                            cyclomatic_complexity: ts_cyclomatic_complexity(function),
                            calls: function_body_calls(function.body.as_deref()),
                            is_component_like,
                        },
                    );
                }
            }
            Statement::VariableDeclaration(variable) => {
                for declarator in &variable.declarations {
                    extract_variable_declarator(
                        db,
                        file,
                        source,
                        language,
                        declarator,
                        false,
                        &exported_names,
                    );
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(name) = class.id.as_ref().map(|id| id.name.to_string()) {
                    let is_exported = exported_names.contains(name.as_str());
                    push_ts_class(db, file, source, language, name, class, is_exported);
                }
            }
            Statement::ExportNamedDeclaration(export) => {
                if let Some(declaration) = &export.declaration {
                    extract_declaration(
                        db,
                        file,
                        source,
                        language,
                        declaration,
                        true,
                        &exported_names,
                    );
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
                            TsFunctionSpec {
                                name,
                                span: function.span,
                                is_exported: true,
                                cyclomatic_complexity: ts_cyclomatic_complexity(function),
                                calls: function_body_calls(function.body.as_deref()),
                                is_component_like,
                            },
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
    exported_names: &BTreeSet<String>,
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
                    TsFunctionSpec {
                        name,
                        span: function.span,
                        is_exported,
                        cyclomatic_complexity: ts_cyclomatic_complexity(function),
                        calls: function_body_calls(function.body.as_deref()),
                        is_component_like,
                    },
                );
            }
        }
        Declaration::VariableDeclaration(variable) => {
            for declarator in &variable.declarations {
                extract_variable_declarator(
                    db,
                    file,
                    source,
                    language,
                    declarator,
                    is_exported,
                    exported_names,
                );
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
    exported_names: &BTreeSet<String>,
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
    let is_exported = is_exported || exported_names.contains(name.as_str());
    match init {
        Expression::ArrowFunctionExpression(function) => {
            let is_component_like = is_component_like_name(&name) || arrow_returns_jsx(function);
            push_ts_function(
                db,
                ctx,
                TsFunctionSpec {
                    name,
                    span: declarator.span,
                    is_exported,
                    cyclomatic_complexity: arrow_cyclomatic_complexity(function),
                    calls: function_body_calls(Some(&function.body)),
                    is_component_like,
                },
            );
        }
        Expression::FunctionExpression(function) => {
            let is_component_like = is_component_like_name(&name) || function_returns_jsx(function);
            push_ts_function(
                db,
                ctx,
                TsFunctionSpec {
                    name,
                    span: declarator.span,
                    is_exported,
                    cyclomatic_complexity: ts_cyclomatic_complexity(function),
                    calls: function_body_calls(function.body.as_deref()),
                    is_component_like,
                },
            );
        }
        Expression::ClassExpression(class) => {
            push_ts_class(db, file, source, language, name, class, is_exported);
        }
        _ => {}
    }
}

fn exported_local_names(program: &Program<'_>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for statement in &program.body {
        match statement {
            Statement::ExportNamedDeclaration(export)
                if export.source.is_none()
                    && matches!(export.export_kind, ImportOrExportKind::Value) =>
            {
                for specifier in &export.specifiers {
                    if matches!(specifier.export_kind, ImportOrExportKind::Value)
                        && let Some(name) = module_export_name_text(&specifier.local)
                    {
                        names.insert(name);
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                if let ExportDefaultDeclarationKind::Identifier(identifier) = &export.declaration {
                    names.insert(identifier.name.to_string());
                }
            }
            _ => {}
        }
    }

    names
}

fn module_export_name_text(name: &ModuleExportName<'_>) -> Option<String> {
    match name {
        ModuleExportName::IdentifierName(identifier) => Some(identifier.name.to_string()),
        ModuleExportName::IdentifierReference(identifier) => Some(identifier.name.to_string()),
        ModuleExportName::StringLiteral(literal) => Some(literal.value.to_string()),
    }
}

struct TsFunctionSpec {
    name: String,
    span: oxc_span::Span,
    is_exported: bool,
    cyclomatic_complexity: u32,
    calls: Vec<String>,
    is_component_like: bool,
}

fn push_ts_function(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    mut spec: TsFunctionSpec,
) -> FunctionId {
    spec.calls.sort();
    spec.calls.dedup();
    let span = span_from_oxc(ctx.file, ctx.source, spec.span);
    let function_id = db.push_function(FunctionFact {
        id: FunctionId(0),
        file: ctx.file,
        name: spec.name.clone(),
        span: span.clone(),
        language: ctx.language,
        is_test: spec.name.contains("test") || spec.name.contains("spec"),
        is_exported: spec.is_exported,
        cyclomatic_complexity: spec.cyclomatic_complexity,
        calls: spec.calls,
    });

    if spec.is_component_like {
        // syntax-level component heuristic: PascalCase or JSX-returning syntax only.
        db.push_ts_component(TsComponentFact {
            file: ctx.file,
            function: Some(function_id),
            name: spec.name,
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
                TsFunctionSpec {
                    name: format!("{name}.{method_name}"),
                    span: method.span,
                    is_exported,
                    cyclomatic_complexity: ts_cyclomatic_complexity(&method.value),
                    calls: function_body_calls(method.value.body.as_deref()),
                    is_component_like: false,
                },
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
            collect_calls_from_expression(&call.callee, calls);
            for argument in &call.arguments {
                collect_calls_from_argument(argument, calls);
            }
        }
        Expression::StaticMemberExpression(member) => {
            collect_calls_from_expression(&member.object, calls);
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                collect_calls_from_array_element(element, calls);
            }
        }
        Expression::AssignmentExpression(expression) => {
            collect_calls_from_expression(&expression.right, calls);
        }
        Expression::AwaitExpression(expression) => {
            collect_calls_from_expression(&expression.argument, calls);
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
        Expression::NewExpression(expression) => {
            if let Some(name) = callee_text(&expression.callee) {
                calls.push(name);
            }
            collect_calls_from_expression(&expression.callee, calls);
            for argument in &expression.arguments {
                collect_calls_from_argument(argument, calls);
            }
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        collect_calls_from_property_key(&property.key, calls);
                        collect_calls_from_expression(&property.value, calls);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        collect_calls_from_expression(&spread.argument, calls);
                    }
                }
            }
        }
        Expression::ParenthesizedExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Expression::SequenceExpression(expression) => {
            for expression in &expression.expressions {
                collect_calls_from_expression(expression, calls);
            }
        }
        Expression::TaggedTemplateExpression(tagged) => {
            collect_calls_from_expression(&tagged.tag, calls);
            for expression in &tagged.quasi.expressions {
                collect_calls_from_expression(expression, calls);
            }
        }
        Expression::UnaryExpression(expression) => {
            collect_calls_from_expression(&expression.argument, calls);
        }
        Expression::YieldExpression(expression) => {
            if let Some(argument) = &expression.argument {
                collect_calls_from_expression(argument, calls);
            }
        }
        Expression::JSXElement(element) => collect_calls_from_jsx_element(element, calls),
        Expression::JSXFragment(fragment) => collect_calls_from_jsx_fragment(fragment, calls),
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
        Expression::TSInstantiationExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Expression::ComputedMemberExpression(expression) => {
            collect_calls_from_expression(&expression.object, calls);
            collect_calls_from_expression(&expression.expression, calls);
        }
        Expression::PrivateFieldExpression(expression) => {
            collect_calls_from_expression(&expression.object, calls);
        }
        _ => {}
    }
}

fn ts_cyclomatic_complexity(function: &Function<'_>) -> u32 {
    1 + function
        .body
        .as_deref()
        .map_or(0, |body| complexity_from_statements(&body.statements))
}

fn arrow_cyclomatic_complexity(function: &ArrowFunctionExpression<'_>) -> u32 {
    1 + function.get_expression().map_or_else(
        || complexity_from_statements(&function.body.statements),
        complexity_from_expression,
    )
}

fn complexity_from_statements(statements: &[Statement<'_>]) -> u32 {
    statements.iter().map(complexity_from_statement).sum()
}

fn complexity_from_statement(statement: &Statement<'_>) -> u32 {
    match statement {
        Statement::BlockStatement(block) => complexity_from_statements(&block.body),
        Statement::ExpressionStatement(statement) => {
            complexity_from_expression(&statement.expression)
        }
        Statement::ReturnStatement(statement) => statement
            .argument
            .as_ref()
            .map_or(0, complexity_from_expression),
        Statement::IfStatement(statement) => {
            1 + complexity_from_expression(&statement.test)
                + complexity_from_statement(&statement.consequent)
                + statement
                    .alternate
                    .as_ref()
                    .map_or(0, |alternate| complexity_from_statement(alternate))
        }
        Statement::DoWhileStatement(statement) => {
            1 + complexity_from_statement(&statement.body)
                + complexity_from_expression(&statement.test)
        }
        Statement::WhileStatement(statement) => {
            1 + complexity_from_expression(&statement.test)
                + complexity_from_statement(&statement.body)
        }
        Statement::ForStatement(statement) => {
            1 + statement.init.as_ref().map_or(0, complexity_from_for_init)
                + statement
                    .test
                    .as_ref()
                    .map_or(0, complexity_from_expression)
                + statement
                    .update
                    .as_ref()
                    .map_or(0, complexity_from_expression)
                + complexity_from_statement(&statement.body)
        }
        Statement::ForInStatement(statement) => {
            1 + complexity_from_for_left(&statement.left)
                + complexity_from_expression(&statement.right)
                + complexity_from_statement(&statement.body)
        }
        Statement::ForOfStatement(statement) => {
            1 + complexity_from_for_left(&statement.left)
                + complexity_from_expression(&statement.right)
                + complexity_from_statement(&statement.body)
        }
        Statement::SwitchStatement(statement) => {
            complexity_from_expression(&statement.discriminant)
                + statement
                    .cases
                    .iter()
                    .map(|case| {
                        1 + case.test.as_ref().map_or(0, complexity_from_expression)
                            + complexity_from_statements(&case.consequent)
                    })
                    .sum::<u32>()
        }
        Statement::ThrowStatement(statement) => complexity_from_expression(&statement.argument),
        Statement::TryStatement(statement) => {
            complexity_from_statements(&statement.block.body)
                + statement.handler.as_ref().map_or(0, |handler| {
                    1 + complexity_from_statements(&handler.body.body)
                })
                + statement
                    .finalizer
                    .as_ref()
                    .map_or(0, |finalizer| complexity_from_statements(&finalizer.body))
        }
        Statement::VariableDeclaration(variable) => complexity_from_variable_declaration(variable),
        Statement::WithStatement(statement) => {
            complexity_from_expression(&statement.object)
                + complexity_from_statement(&statement.body)
        }
        Statement::LabeledStatement(statement) => complexity_from_statement(&statement.body),
        _ => 0,
    }
}

fn complexity_from_expression(expression: &Expression<'_>) -> u32 {
    match expression {
        Expression::ArrayExpression(array) => array
            .elements
            .iter()
            .map(complexity_from_array_element)
            .sum(),
        Expression::AssignmentExpression(expression) => {
            complexity_from_expression(&expression.right)
        }
        Expression::AwaitExpression(expression) => complexity_from_expression(&expression.argument),
        Expression::BinaryExpression(expression) => {
            complexity_from_expression(&expression.left)
                + complexity_from_expression(&expression.right)
        }
        Expression::CallExpression(call) => {
            complexity_from_expression(&call.callee)
                + call
                    .arguments
                    .iter()
                    .map(complexity_from_argument)
                    .sum::<u32>()
        }
        Expression::ConditionalExpression(expression) => {
            1 + complexity_from_expression(&expression.test)
                + complexity_from_expression(&expression.consequent)
                + complexity_from_expression(&expression.alternate)
        }
        Expression::ImportExpression(expression) => complexity_from_expression(&expression.source),
        Expression::LogicalExpression(expression) => {
            u32::from(expression.operator.is_and() || expression.operator.is_or())
                + complexity_from_expression(&expression.left)
                + complexity_from_expression(&expression.right)
        }
        Expression::NewExpression(expression) => {
            complexity_from_expression(&expression.callee)
                + expression
                    .arguments
                    .iter()
                    .map(complexity_from_argument)
                    .sum::<u32>()
        }
        Expression::ObjectExpression(object) => object
            .properties
            .iter()
            .map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => {
                    complexity_from_property_key(&property.key)
                        + complexity_from_expression(&property.value)
                }
                ObjectPropertyKind::SpreadProperty(spread) => {
                    complexity_from_expression(&spread.argument)
                }
            })
            .sum(),
        Expression::ParenthesizedExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        Expression::SequenceExpression(expression) => expression
            .expressions
            .iter()
            .map(complexity_from_expression)
            .sum(),
        Expression::TaggedTemplateExpression(tagged) => {
            complexity_from_expression(&tagged.tag)
                + tagged
                    .quasi
                    .expressions
                    .iter()
                    .map(complexity_from_expression)
                    .sum::<u32>()
        }
        Expression::UnaryExpression(expression) => complexity_from_expression(&expression.argument),
        Expression::YieldExpression(expression) => expression
            .argument
            .as_ref()
            .map_or(0, complexity_from_expression),
        Expression::JSXElement(element) => complexity_from_jsx_element(element),
        Expression::JSXFragment(fragment) => complexity_from_jsx_fragment(fragment),
        Expression::TSAsExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            complexity_from_expression(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        Expression::ComputedMemberExpression(expression) => {
            complexity_from_expression(&expression.object)
                + complexity_from_expression(&expression.expression)
        }
        Expression::StaticMemberExpression(expression) => {
            complexity_from_expression(&expression.object)
        }
        Expression::PrivateFieldExpression(expression) => {
            complexity_from_expression(&expression.object)
        }
        _ => 0,
    }
}

fn complexity_from_variable_declaration(variable: &oxc_ast::ast::VariableDeclaration<'_>) -> u32 {
    variable
        .declarations
        .iter()
        .filter_map(|declarator| declarator.init.as_ref())
        .map(complexity_from_expression)
        .sum()
}

fn complexity_from_for_init(init: &ForStatementInit<'_>) -> u32 {
    match init {
        ForStatementInit::VariableDeclaration(variable) => {
            complexity_from_variable_declaration(variable)
        }
        ForStatementInit::ArrayExpression(array) => array
            .elements
            .iter()
            .map(complexity_from_array_element)
            .sum(),
        ForStatementInit::AssignmentExpression(expression) => {
            complexity_from_expression(&expression.right)
        }
        ForStatementInit::CallExpression(call) => {
            complexity_from_expression(&call.callee)
                + call
                    .arguments
                    .iter()
                    .map(complexity_from_argument)
                    .sum::<u32>()
        }
        ForStatementInit::ConditionalExpression(expression) => {
            1 + complexity_from_expression(&expression.test)
                + complexity_from_expression(&expression.consequent)
                + complexity_from_expression(&expression.alternate)
        }
        ForStatementInit::LogicalExpression(expression) => {
            u32::from(expression.operator.is_and() || expression.operator.is_or())
                + complexity_from_expression(&expression.left)
                + complexity_from_expression(&expression.right)
        }
        ForStatementInit::ParenthesizedExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        ForStatementInit::SequenceExpression(expression) => expression
            .expressions
            .iter()
            .map(complexity_from_expression)
            .sum(),
        ForStatementInit::TSAsExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        ForStatementInit::TSSatisfiesExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        ForStatementInit::TSNonNullExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        ForStatementInit::TSTypeAssertion(expression) => {
            complexity_from_expression(&expression.expression)
        }
        _ => 0,
    }
}

fn complexity_from_for_left(left: &ForStatementLeft<'_>) -> u32 {
    if let ForStatementLeft::VariableDeclaration(variable) = left {
        complexity_from_variable_declaration(variable)
    } else {
        0
    }
}

fn complexity_from_argument(argument: &Argument<'_>) -> u32 {
    match argument {
        Argument::SpreadElement(spread) => complexity_from_expression(&spread.argument),
        Argument::ArrayExpression(array) => array
            .elements
            .iter()
            .map(complexity_from_array_element)
            .sum(),
        Argument::AssignmentExpression(expression) => complexity_from_expression(&expression.right),
        Argument::BinaryExpression(expression) => {
            complexity_from_expression(&expression.left)
                + complexity_from_expression(&expression.right)
        }
        Argument::CallExpression(call) => {
            complexity_from_expression(&call.callee)
                + call
                    .arguments
                    .iter()
                    .map(complexity_from_argument)
                    .sum::<u32>()
        }
        Argument::ConditionalExpression(expression) => {
            1 + complexity_from_expression(&expression.test)
                + complexity_from_expression(&expression.consequent)
                + complexity_from_expression(&expression.alternate)
        }
        Argument::LogicalExpression(expression) => {
            u32::from(expression.operator.is_and() || expression.operator.is_or())
                + complexity_from_expression(&expression.left)
                + complexity_from_expression(&expression.right)
        }
        Argument::ObjectExpression(object) => object
            .properties
            .iter()
            .map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => {
                    complexity_from_property_key(&property.key)
                        + complexity_from_expression(&property.value)
                }
                ObjectPropertyKind::SpreadProperty(spread) => {
                    complexity_from_expression(&spread.argument)
                }
            })
            .sum(),
        Argument::ParenthesizedExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        Argument::SequenceExpression(expression) => expression
            .expressions
            .iter()
            .map(complexity_from_expression)
            .sum(),
        Argument::TaggedTemplateExpression(tagged) => {
            complexity_from_expression(&tagged.tag)
                + tagged
                    .quasi
                    .expressions
                    .iter()
                    .map(complexity_from_expression)
                    .sum::<u32>()
        }
        Argument::UnaryExpression(expression) => complexity_from_expression(&expression.argument),
        Argument::JSXElement(element) => complexity_from_jsx_element(element),
        Argument::JSXFragment(fragment) => complexity_from_jsx_fragment(fragment),
        Argument::TSAsExpression(expression) => complexity_from_expression(&expression.expression),
        Argument::TSSatisfiesExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        Argument::TSNonNullExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        Argument::TSTypeAssertion(expression) => complexity_from_expression(&expression.expression),
        _ => 0,
    }
}

fn complexity_from_array_element(element: &ArrayExpressionElement<'_>) -> u32 {
    match element {
        ArrayExpressionElement::SpreadElement(spread) => {
            complexity_from_expression(&spread.argument)
        }
        ArrayExpressionElement::ArrayExpression(array) => array
            .elements
            .iter()
            .map(complexity_from_array_element)
            .sum(),
        ArrayExpressionElement::AssignmentExpression(expression) => {
            complexity_from_expression(&expression.right)
        }
        ArrayExpressionElement::CallExpression(call) => {
            complexity_from_expression(&call.callee)
                + call
                    .arguments
                    .iter()
                    .map(complexity_from_argument)
                    .sum::<u32>()
        }
        ArrayExpressionElement::ConditionalExpression(expression) => {
            1 + complexity_from_expression(&expression.test)
                + complexity_from_expression(&expression.consequent)
                + complexity_from_expression(&expression.alternate)
        }
        ArrayExpressionElement::LogicalExpression(expression) => {
            u32::from(expression.operator.is_and() || expression.operator.is_or())
                + complexity_from_expression(&expression.left)
                + complexity_from_expression(&expression.right)
        }
        ArrayExpressionElement::ObjectExpression(object) => object
            .properties
            .iter()
            .map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => {
                    complexity_from_property_key(&property.key)
                        + complexity_from_expression(&property.value)
                }
                ObjectPropertyKind::SpreadProperty(spread) => {
                    complexity_from_expression(&spread.argument)
                }
            })
            .sum(),
        ArrayExpressionElement::ParenthesizedExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        ArrayExpressionElement::SequenceExpression(expression) => expression
            .expressions
            .iter()
            .map(complexity_from_expression)
            .sum(),
        ArrayExpressionElement::TaggedTemplateExpression(tagged) => {
            complexity_from_expression(&tagged.tag)
                + tagged
                    .quasi
                    .expressions
                    .iter()
                    .map(complexity_from_expression)
                    .sum::<u32>()
        }
        ArrayExpressionElement::UnaryExpression(expression) => {
            complexity_from_expression(&expression.argument)
        }
        ArrayExpressionElement::JSXElement(element) => complexity_from_jsx_element(element),
        ArrayExpressionElement::JSXFragment(fragment) => complexity_from_jsx_fragment(fragment),
        ArrayExpressionElement::TSAsExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        ArrayExpressionElement::TSSatisfiesExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        ArrayExpressionElement::TSNonNullExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        ArrayExpressionElement::TSTypeAssertion(expression) => {
            complexity_from_expression(&expression.expression)
        }
        _ => 0,
    }
}

fn complexity_from_property_key(key: &PropertyKey<'_>) -> u32 {
    match key {
        PropertyKey::ConditionalExpression(expression) => {
            1 + complexity_from_expression(&expression.test)
                + complexity_from_expression(&expression.consequent)
                + complexity_from_expression(&expression.alternate)
        }
        PropertyKey::LogicalExpression(expression) => {
            u32::from(expression.operator.is_and() || expression.operator.is_or())
                + complexity_from_expression(&expression.left)
                + complexity_from_expression(&expression.right)
        }
        PropertyKey::ParenthesizedExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        PropertyKey::TSAsExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        PropertyKey::TSSatisfiesExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        PropertyKey::TSNonNullExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        PropertyKey::TSTypeAssertion(expression) => {
            complexity_from_expression(&expression.expression)
        }
        _ => 0,
    }
}

fn complexity_from_jsx_element(element: &JSXElement<'_>) -> u32 {
    element
        .opening_element
        .attributes
        .iter()
        .map(|item| match item {
            JSXAttributeItem::Attribute(attribute) => attribute
                .value
                .as_ref()
                .map_or(0, complexity_from_jsx_attribute_value),
            JSXAttributeItem::SpreadAttribute(spread) => {
                complexity_from_expression(&spread.argument)
            }
        })
        .sum::<u32>()
        + element
            .children
            .iter()
            .map(complexity_from_jsx_child)
            .sum::<u32>()
}

fn complexity_from_jsx_attribute_value(value: &JSXAttributeValue<'_>) -> u32 {
    match value {
        JSXAttributeValue::ExpressionContainer(container) => {
            complexity_from_jsx_expression(&container.expression)
        }
        JSXAttributeValue::Element(element) => complexity_from_jsx_element(element),
        JSXAttributeValue::Fragment(fragment) => complexity_from_jsx_fragment(fragment),
        JSXAttributeValue::StringLiteral(_) => 0,
    }
}

fn complexity_from_jsx_child(child: &JSXChild<'_>) -> u32 {
    match child {
        JSXChild::Element(element) => complexity_from_jsx_element(element),
        JSXChild::Fragment(fragment) => complexity_from_jsx_fragment(fragment),
        JSXChild::ExpressionContainer(container) => {
            complexity_from_jsx_expression(&container.expression)
        }
        JSXChild::Spread(spread) => complexity_from_expression(&spread.expression),
        JSXChild::Text(_) => 0,
    }
}

fn complexity_from_jsx_fragment(fragment: &JSXFragment<'_>) -> u32 {
    fragment
        .children
        .iter()
        .map(complexity_from_jsx_child)
        .sum()
}

fn complexity_from_jsx_expression(expression: &JSXExpression<'_>) -> u32 {
    match expression {
        JSXExpression::ArrayExpression(array) => array
            .elements
            .iter()
            .map(complexity_from_array_element)
            .sum(),
        JSXExpression::AssignmentExpression(expression) => {
            complexity_from_expression(&expression.right)
        }
        JSXExpression::BinaryExpression(expression) => {
            complexity_from_expression(&expression.left)
                + complexity_from_expression(&expression.right)
        }
        JSXExpression::CallExpression(call) => {
            complexity_from_expression(&call.callee)
                + call
                    .arguments
                    .iter()
                    .map(complexity_from_argument)
                    .sum::<u32>()
        }
        JSXExpression::ConditionalExpression(expression) => {
            1 + complexity_from_expression(&expression.test)
                + complexity_from_expression(&expression.consequent)
                + complexity_from_expression(&expression.alternate)
        }
        JSXExpression::LogicalExpression(expression) => {
            u32::from(expression.operator.is_and() || expression.operator.is_or())
                + complexity_from_expression(&expression.left)
                + complexity_from_expression(&expression.right)
        }
        JSXExpression::ObjectExpression(object) => object
            .properties
            .iter()
            .map(|property| match property {
                ObjectPropertyKind::ObjectProperty(property) => {
                    complexity_from_property_key(&property.key)
                        + complexity_from_expression(&property.value)
                }
                ObjectPropertyKind::SpreadProperty(spread) => {
                    complexity_from_expression(&spread.argument)
                }
            })
            .sum(),
        JSXExpression::ParenthesizedExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        JSXExpression::TaggedTemplateExpression(tagged) => {
            complexity_from_expression(&tagged.tag)
                + tagged
                    .quasi
                    .expressions
                    .iter()
                    .map(complexity_from_expression)
                    .sum::<u32>()
        }
        JSXExpression::UnaryExpression(expression) => {
            complexity_from_expression(&expression.argument)
        }
        JSXExpression::JSXElement(element) => complexity_from_jsx_element(element),
        JSXExpression::JSXFragment(fragment) => complexity_from_jsx_fragment(fragment),
        JSXExpression::TSAsExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        JSXExpression::TSSatisfiesExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        JSXExpression::TSNonNullExpression(expression) => {
            complexity_from_expression(&expression.expression)
        }
        JSXExpression::TSTypeAssertion(expression) => {
            complexity_from_expression(&expression.expression)
        }
        JSXExpression::EmptyExpression(_) => 0,
        _ => 0,
    }
}

fn collect_calls_from_argument(argument: &Argument<'_>, calls: &mut Vec<String>) {
    match argument {
        Argument::SpreadElement(spread) => collect_calls_from_expression(&spread.argument, calls),
        Argument::ArrayExpression(array) => {
            for element in &array.elements {
                collect_calls_from_array_element(element, calls);
            }
        }
        Argument::AssignmentExpression(expression) => {
            collect_calls_from_expression(&expression.right, calls);
        }
        Argument::AwaitExpression(expression) => {
            collect_calls_from_expression(&expression.argument, calls);
        }
        Argument::BinaryExpression(expression) => {
            collect_calls_from_expression(&expression.left, calls);
            collect_calls_from_expression(&expression.right, calls);
        }
        Argument::CallExpression(call) => {
            if let Some(name) = callee_text(&call.callee) {
                calls.push(name);
            }
            collect_calls_from_expression(&call.callee, calls);
            for argument in &call.arguments {
                collect_calls_from_argument(argument, calls);
            }
        }
        Argument::ConditionalExpression(expression) => {
            collect_calls_from_expression(&expression.test, calls);
            collect_calls_from_expression(&expression.consequent, calls);
            collect_calls_from_expression(&expression.alternate, calls);
        }
        Argument::ImportExpression(expression) => {
            collect_calls_from_expression(&expression.source, calls);
        }
        Argument::LogicalExpression(expression) => {
            collect_calls_from_expression(&expression.left, calls);
            collect_calls_from_expression(&expression.right, calls);
        }
        Argument::NewExpression(expression) => {
            collect_calls_from_expression(&expression.callee, calls);
            for argument in &expression.arguments {
                collect_calls_from_argument(argument, calls);
            }
        }
        Argument::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        collect_calls_from_property_key(&property.key, calls);
                        collect_calls_from_expression(&property.value, calls);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        collect_calls_from_expression(&spread.argument, calls);
                    }
                }
            }
        }
        Argument::ParenthesizedExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Argument::SequenceExpression(expression) => {
            for expression in &expression.expressions {
                collect_calls_from_expression(expression, calls);
            }
        }
        Argument::TaggedTemplateExpression(tagged) => {
            collect_calls_from_expression(&tagged.tag, calls);
            for expression in &tagged.quasi.expressions {
                collect_calls_from_expression(expression, calls);
            }
        }
        Argument::UnaryExpression(expression) => {
            collect_calls_from_expression(&expression.argument, calls);
        }
        Argument::YieldExpression(expression) => {
            if let Some(argument) = &expression.argument {
                collect_calls_from_expression(argument, calls);
            }
        }
        Argument::JSXElement(element) => collect_calls_from_jsx_element(element, calls),
        Argument::JSXFragment(fragment) => collect_calls_from_jsx_fragment(fragment, calls),
        Argument::TSAsExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Argument::TSSatisfiesExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Argument::TSNonNullExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Argument::TSTypeAssertion(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Argument::TSInstantiationExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        Argument::ComputedMemberExpression(expression) => {
            collect_calls_from_expression(&expression.object, calls);
            collect_calls_from_expression(&expression.expression, calls);
        }
        Argument::StaticMemberExpression(expression) => {
            collect_calls_from_expression(&expression.object, calls);
        }
        Argument::PrivateFieldExpression(expression) => {
            collect_calls_from_expression(&expression.object, calls);
        }
        _ => {}
    }
}

fn collect_calls_from_array_element(element: &ArrayExpressionElement<'_>, calls: &mut Vec<String>) {
    match element {
        ArrayExpressionElement::SpreadElement(spread) => {
            collect_calls_from_expression(&spread.argument, calls);
        }
        ArrayExpressionElement::ArrayExpression(array) => {
            for element in &array.elements {
                collect_calls_from_array_element(element, calls);
            }
        }
        ArrayExpressionElement::AssignmentExpression(expression) => {
            collect_calls_from_expression(&expression.right, calls);
        }
        ArrayExpressionElement::AwaitExpression(expression) => {
            collect_calls_from_expression(&expression.argument, calls);
        }
        ArrayExpressionElement::BinaryExpression(expression) => {
            collect_calls_from_expression(&expression.left, calls);
            collect_calls_from_expression(&expression.right, calls);
        }
        ArrayExpressionElement::CallExpression(call) => {
            if let Some(name) = callee_text(&call.callee) {
                calls.push(name);
            }
            collect_calls_from_expression(&call.callee, calls);
            for argument in &call.arguments {
                collect_calls_from_argument(argument, calls);
            }
        }
        ArrayExpressionElement::ConditionalExpression(expression) => {
            collect_calls_from_expression(&expression.test, calls);
            collect_calls_from_expression(&expression.consequent, calls);
            collect_calls_from_expression(&expression.alternate, calls);
        }
        ArrayExpressionElement::LogicalExpression(expression) => {
            collect_calls_from_expression(&expression.left, calls);
            collect_calls_from_expression(&expression.right, calls);
        }
        ArrayExpressionElement::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        collect_calls_from_property_key(&property.key, calls);
                        collect_calls_from_expression(&property.value, calls);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        collect_calls_from_expression(&spread.argument, calls);
                    }
                }
            }
        }
        ArrayExpressionElement::ParenthesizedExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        ArrayExpressionElement::SequenceExpression(expression) => {
            for expression in &expression.expressions {
                collect_calls_from_expression(expression, calls);
            }
        }
        ArrayExpressionElement::TaggedTemplateExpression(tagged) => {
            collect_calls_from_expression(&tagged.tag, calls);
            for expression in &tagged.quasi.expressions {
                collect_calls_from_expression(expression, calls);
            }
        }
        ArrayExpressionElement::UnaryExpression(expression) => {
            collect_calls_from_expression(&expression.argument, calls);
        }
        ArrayExpressionElement::JSXElement(element) => {
            collect_calls_from_jsx_element(element, calls)
        }
        ArrayExpressionElement::JSXFragment(fragment) => {
            collect_calls_from_jsx_fragment(fragment, calls);
        }
        ArrayExpressionElement::TSAsExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        ArrayExpressionElement::TSSatisfiesExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        ArrayExpressionElement::TSNonNullExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        ArrayExpressionElement::TSTypeAssertion(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        _ => {}
    }
}

fn collect_calls_from_property_key(key: &PropertyKey<'_>, calls: &mut Vec<String>) {
    match key {
        PropertyKey::CallExpression(call) => {
            if let Some(name) = callee_text(&call.callee) {
                calls.push(name);
            }
            for argument in &call.arguments {
                collect_calls_from_argument(argument, calls);
            }
        }
        PropertyKey::ConditionalExpression(expression) => {
            collect_calls_from_expression(&expression.test, calls);
            collect_calls_from_expression(&expression.consequent, calls);
            collect_calls_from_expression(&expression.alternate, calls);
        }
        PropertyKey::LogicalExpression(expression) => {
            collect_calls_from_expression(&expression.left, calls);
            collect_calls_from_expression(&expression.right, calls);
        }
        PropertyKey::ParenthesizedExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        PropertyKey::TSAsExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        PropertyKey::TSSatisfiesExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        PropertyKey::TSNonNullExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        PropertyKey::TSTypeAssertion(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        _ => {}
    }
}

fn collect_calls_from_jsx_element(element: &JSXElement<'_>, calls: &mut Vec<String>) {
    for item in &element.opening_element.attributes {
        match item {
            JSXAttributeItem::Attribute(attribute) => {
                if let Some(value) = &attribute.value {
                    collect_calls_from_jsx_attribute_value(value, calls);
                }
            }
            JSXAttributeItem::SpreadAttribute(spread) => {
                collect_calls_from_expression(&spread.argument, calls);
            }
        }
    }
    for child in &element.children {
        collect_calls_from_jsx_child(child, calls);
    }
}

fn collect_calls_from_jsx_attribute_value(value: &JSXAttributeValue<'_>, calls: &mut Vec<String>) {
    match value {
        JSXAttributeValue::ExpressionContainer(container) => {
            collect_calls_from_jsx_expression(&container.expression, calls);
        }
        JSXAttributeValue::Element(element) => collect_calls_from_jsx_element(element, calls),
        JSXAttributeValue::Fragment(fragment) => collect_calls_from_jsx_fragment(fragment, calls),
        JSXAttributeValue::StringLiteral(_) => {}
    }
}

fn collect_calls_from_jsx_child(child: &JSXChild<'_>, calls: &mut Vec<String>) {
    match child {
        JSXChild::Element(element) => collect_calls_from_jsx_element(element, calls),
        JSXChild::Fragment(fragment) => collect_calls_from_jsx_fragment(fragment, calls),
        JSXChild::ExpressionContainer(container) => {
            collect_calls_from_jsx_expression(&container.expression, calls);
        }
        JSXChild::Spread(spread) => collect_calls_from_expression(&spread.expression, calls),
        JSXChild::Text(_) => {}
    }
}

fn collect_calls_from_jsx_fragment(fragment: &JSXFragment<'_>, calls: &mut Vec<String>) {
    for child in &fragment.children {
        collect_calls_from_jsx_child(child, calls);
    }
}

fn collect_calls_from_jsx_expression(expression: &JSXExpression<'_>, calls: &mut Vec<String>) {
    match expression {
        JSXExpression::ArrayExpression(array) => {
            for element in &array.elements {
                collect_calls_from_array_element(element, calls);
            }
        }
        JSXExpression::AssignmentExpression(expression) => {
            collect_calls_from_expression(&expression.right, calls);
        }
        JSXExpression::BinaryExpression(expression) => {
            collect_calls_from_expression(&expression.left, calls);
            collect_calls_from_expression(&expression.right, calls);
        }
        JSXExpression::CallExpression(call) => {
            if let Some(name) = callee_text(&call.callee) {
                calls.push(name);
            }
            collect_calls_from_expression(&call.callee, calls);
            for argument in &call.arguments {
                collect_calls_from_argument(argument, calls);
            }
        }
        JSXExpression::ConditionalExpression(expression) => {
            collect_calls_from_expression(&expression.test, calls);
            collect_calls_from_expression(&expression.consequent, calls);
            collect_calls_from_expression(&expression.alternate, calls);
        }
        JSXExpression::LogicalExpression(expression) => {
            collect_calls_from_expression(&expression.left, calls);
            collect_calls_from_expression(&expression.right, calls);
        }
        JSXExpression::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        collect_calls_from_property_key(&property.key, calls);
                        collect_calls_from_expression(&property.value, calls);
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        collect_calls_from_expression(&spread.argument, calls);
                    }
                }
            }
        }
        JSXExpression::ParenthesizedExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        JSXExpression::TaggedTemplateExpression(tagged) => {
            collect_calls_from_expression(&tagged.tag, calls);
            for expression in &tagged.quasi.expressions {
                collect_calls_from_expression(expression, calls);
            }
        }
        JSXExpression::UnaryExpression(expression) => {
            collect_calls_from_expression(&expression.argument, calls);
        }
        JSXExpression::JSXElement(element) => collect_calls_from_jsx_element(element, calls),
        JSXExpression::JSXFragment(fragment) => collect_calls_from_jsx_fragment(fragment, calls),
        JSXExpression::TSAsExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        JSXExpression::TSSatisfiesExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        JSXExpression::TSNonNullExpression(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        JSXExpression::TSTypeAssertion(expression) => {
            collect_calls_from_expression(&expression.expression, calls);
        }
        JSXExpression::EmptyExpression(_) => {}
        _ => {}
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
        Expression::RegExpLiteral(literal) => {
            push_regex_literal_from_oxc(db, ctx, literal);
        }
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

fn push_regex_literal_from_oxc(
    db: &mut AnalysisDb,
    ctx: TsAstCtx<'_>,
    literal: &RegExpLiteral<'_>,
) {
    let value = literal
        .raw
        .as_ref()
        .map_or_else(|| literal.regex.to_string(), |raw| raw.as_str().to_string());
    push_string_literal_from_oxc(db, ctx, value, literal.span);
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
        ExportDefaultDeclarationKind::RegExpLiteral(literal) => {
            push_regex_literal_from_oxc(db, ctx, literal);
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
        ForStatementInit::RegExpLiteral(literal) => {
            push_regex_literal_from_oxc(db, ctx, literal);
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
        Argument::RegExpLiteral(literal) => {
            push_regex_literal_from_oxc(db, ctx, literal);
        }
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
        ArrayExpressionElement::RegExpLiteral(literal) => {
            push_regex_literal_from_oxc(db, ctx, literal);
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
        PropertyKey::RegExpLiteral(literal) => {
            push_regex_literal_from_oxc(db, ctx, literal);
        }
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
        JSXAttributeValue::StringLiteral(literal) => {
            push_string_literal_from_oxc(db, ctx, literal.value.to_string(), literal.span);
        }
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
        JSXExpression::RegExpLiteral(literal) => {
            push_regex_literal_from_oxc(db, ctx, literal);
        }
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
