use std::collections::BTreeMap;
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, BinaryOperator, BindingPattern, Class, ClassElement, Declaration,
    ExportDefaultDeclarationKind, Expression, FormalParameters, Function, FunctionBody,
    LogicalOperator, MethodDefinition, ObjectPropertyKind, Program, PropertyKey, Statement,
    VariableDeclarator,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use crate::analysis::ids::{
    CallSiteId, MirBodyId, MirOpId, MirPredicateId, PlaceId, UnsupportedId,
};
use crate::analysis::mir::body::{MirBody, MirOutput, MirStatus};
use crate::analysis::mir::op::{
    AssignMode, ConservativeAction, MirOperation, MirOperationKind, MirValue, UnsupportedDomain,
    UnsupportedPrecision, UnsupportedSemanticFact,
};
use crate::analysis::places::{
    PlaceInsert, PlaceProjection, PlaceRoot, PlaceStableContext, PlaceStatus, PlaceTableBuilder,
};
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{
    AnalysisDb, FileId, FunctionFact, FunctionId, Language, SourceFile, Span,
    is_synthetic_ts_js_module_function,
};
use crate::ts::{
    anonymous_callable_name,
    spans::{normalized_call_expression_span, normalized_new_expression_span},
};

pub(crate) fn lower_ts_mir(db: &AnalysisDb) -> MirOutput {
    let mut lowering = TsMirLowering::default();
    let mut files = db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    for file in files {
        lowering.lower_file(db, file);
    }

    let places = lowering.places.clone().finish();
    let place_ids = places
        .iter()
        .map(|place| (place.stable_key.clone(), place.id))
        .collect::<BTreeMap<_, _>>();
    let operations = lowering.finish_operations(&place_ids);
    let unsupported = lowering.finish_unsupported(&place_ids);

    MirOutput {
        bodies: lowering.bodies,
        places,
        operations,
        unsupported,
    }
    .normalized()
}

#[derive(Debug, Default)]
struct TsMirLowering {
    bodies: Vec<MirBody>,
    places: PlaceTableBuilder,
    operations: Vec<OperationDraft>,
    unsupported: Vec<UnsupportedDraft>,
}

impl TsMirLowering {
    fn lower_file(&mut self, db: &AnalysisDb, file: &SourceFile) {
        let allocator = Allocator::default();
        let parsed = Parser::new(
            &allocator,
            file.source.as_ref(),
            parse_source_type(&file.path),
        )
        .parse();
        for error in &parsed.errors {
            let span = error
                .labels
                .as_ref()
                .and_then(|labels| labels.first())
                .map(|label| {
                    crate::core::span_from_byte_range(
                        file.id,
                        file.source.as_ref(),
                        label.offset(),
                        label.offset() + label.len(),
                    )
                })
                .unwrap_or_else(|| Span::point(file.id, 1, 1));
            self.unsupported
                .push(UnsupportedDraft::new(UnsupportedDraftInput {
                    id: UnsupportedId(self.unsupported.len() as u64),
                    body: None,
                    operation: None,
                    language: file.language,
                    file_key: file.relative_path.clone(),
                    file: file.id,
                    span,
                    construct: "parser recovery".to_string(),
                    source_evidence: error.to_string(),
                    affected_place_keys: Vec::new(),
                    affected_domains: vec![UnsupportedDomain::Mir, UnsupportedDomain::Cfg],
                    conservative_action: ConservativeAction::StopLowering,
                }));
        }
        if let Some(module_function) = matching_module_function(db, file.id, file.language) {
            let span = crate::core::span_from_byte_range(
                file.id,
                file.source.as_ref(),
                0,
                file.source.len(),
            );
            let body = self.push_body(db, file, module_function, span);
            let mut module_lowering =
                FunctionLowering::new(file, file.source.as_ref(), module_function.id, &body);
            module_lowering.lower_statements(
                &parsed.program.body,
                &mut self.places,
                &mut self.operations,
                &mut self.unsupported,
            );
        }

        let mut functions = Vec::new();
        collect_functions(file.source.as_ref(), &parsed.program, &mut functions);
        functions.sort_by(|left, right| {
            (
                file.relative_path.as_str(),
                left.span.start,
                left.span.end,
                left.name.as_str(),
            )
                .cmp(&(
                    file.relative_path.as_str(),
                    right.span.start,
                    right.span.end,
                    right.name.as_str(),
                ))
        });

        for function in functions {
            let span = span_from_oxc(file.id, file.source.as_ref(), function.span);
            let Some(function_fact) =
                matching_function(db, file.id, file.language, &function.name, &span)
            else {
                continue;
            };
            let body = self.push_body(db, file, function_fact, span);
            let mut function_lowering =
                FunctionLowering::new(file, file.source.as_ref(), function_fact.id, &body);
            if function.r#async {
                function_lowering.push_unsupported(
                    &mut self.operations,
                    &mut self.unsupported,
                    function.span_for_unsupported,
                    "async rejection path",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
            }
            function_lowering.lower_parameters(&function.parameters, &mut self.places);
            function_lowering.lower_body(
                function.body,
                &mut self.places,
                &mut self.operations,
                &mut self.unsupported,
            );
        }
    }

    fn push_body(
        &mut self,
        db: &AnalysisDb,
        file: &SourceFile,
        function: &FunctionFact,
        span: Span,
    ) -> MirBody {
        let id = MirBodyId(self.bodies.len() as u64);
        let owner_stable_key = owner_stable_key(file, function);
        let stable_key = semantic_stable_key(
            FactFamily::MirBody,
            &[
                ("language", language_label(file.language).to_string()),
                ("path", file.relative_path.clone()),
                ("owner", owner_stable_key.clone()),
                ("start_byte", span.start_byte.to_string()),
                ("end_byte", span.end_byte.to_string()),
            ],
        )
        .into_string();
        let body = MirBody {
            id,
            language: file.language,
            file: file.id,
            function: function.id,
            package: db
                .packages()
                .iter()
                .find(|package| package.file == file.id && package.language == file.language)
                .map(|package| package.id),
            module: db
                .module_nodes()
                .iter()
                .find(|module| {
                    module.file == Some(file.id) && module.language == Some(file.language)
                })
                .map(|module| module.id),
            owner_stable_key,
            span,
            stable_key,
            status: MirStatus::Partial,
        };
        self.bodies.push(body.clone());
        body
    }

    fn finish_operations(&self, place_ids: &BTreeMap<String, PlaceId>) -> Vec<MirOperation> {
        self.operations
            .iter()
            .filter_map(|draft| draft.to_operation(place_ids))
            .collect()
    }

    fn finish_unsupported(
        &self,
        place_ids: &BTreeMap<String, PlaceId>,
    ) -> Vec<UnsupportedSemanticFact> {
        self.unsupported
            .iter()
            .map(|draft| draft.to_fact(place_ids))
            .collect()
    }
}

#[derive(Debug)]
struct TsFunctionCandidate<'ast> {
    name: String,
    span: oxc_span::Span,
    parameters: Vec<String>,
    body: &'ast FunctionBody<'ast>,
    r#async: bool,
    span_for_unsupported: oxc_span::Span,
}

fn collect_functions<'ast>(
    source: &str,
    program: &'ast Program<'ast>,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    for statement in &program.body {
        collect_statement_functions(source, statement, functions);
        collect_anonymous_functions_from_statement(statement, functions);
    }
}

fn collect_statement_functions<'ast>(
    source: &str,
    statement: &'ast Statement<'ast>,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    match statement {
        Statement::FunctionDeclaration(function) => {
            if let Some(name) = function.id.as_ref().map(|id| id.name.to_string()) {
                collect_function(name, function.span, function, functions);
            }
        }
        Statement::VariableDeclaration(variable) => {
            for declarator in &variable.declarations {
                collect_variable_function(declarator, functions);
            }
        }
        Statement::ClassDeclaration(class) => {
            if let Some(name) = class.id.as_ref().map(|id| id.name.to_string()) {
                collect_class_functions(&name, class, functions);
            }
        }
        Statement::ExportNamedDeclaration(export) => {
            if let Some(declaration) = &export.declaration {
                collect_declaration_functions(source, declaration, functions);
            }
        }
        Statement::ExportDefaultDeclaration(export) => match &export.declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                if let Some(name) = function.id.as_ref().map(|id| id.name.to_string()) {
                    collect_function(name, function.span, function, functions);
                }
            }
            ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                if let Some(name) = class.id.as_ref().map(|id| id.name.to_string()) {
                    collect_class_functions(&name, class, functions);
                }
            }
            _ => {}
        },
        _ => {}
    }
}

fn collect_declaration_functions<'ast>(
    _source: &str,
    declaration: &'ast Declaration<'ast>,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    match declaration {
        Declaration::FunctionDeclaration(function) => {
            if let Some(name) = function.id.as_ref().map(|id| id.name.to_string()) {
                collect_function(name, function.span, function, functions);
            }
        }
        Declaration::VariableDeclaration(variable) => {
            for declarator in &variable.declarations {
                collect_variable_function(declarator, functions);
            }
        }
        Declaration::ClassDeclaration(class) => {
            if let Some(name) = class.id.as_ref().map(|id| id.name.to_string()) {
                collect_class_functions(&name, class, functions);
            }
        }
        _ => {}
    }
}

fn collect_variable_function<'ast>(
    declarator: &'ast VariableDeclarator<'ast>,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    let BindingPattern::BindingIdentifier(name) = &declarator.id else {
        return;
    };
    let Some(init) = &declarator.init else {
        return;
    };
    match init {
        Expression::ArrowFunctionExpression(function) => {
            functions.push(TsFunctionCandidate {
                name: name.name.to_string(),
                span: function.span,
                parameters: parameter_names(&function.params),
                body: &function.body,
                r#async: function.r#async,
                span_for_unsupported: function.span,
            });
        }
        Expression::FunctionExpression(function) => {
            if let Some(body) = function.body.as_deref() {
                functions.push(TsFunctionCandidate {
                    name: name.name.to_string(),
                    span: function.span,
                    parameters: parameter_names(&function.params),
                    body,
                    r#async: function.r#async,
                    span_for_unsupported: function.span,
                });
            }
        }
        _ => {}
    }
}

fn collect_function<'ast>(
    name: String,
    span: oxc_span::Span,
    function: &'ast Function<'ast>,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    if let Some(body) = function.body.as_deref() {
        functions.push(TsFunctionCandidate {
            name,
            span,
            parameters: parameter_names(&function.params),
            body,
            r#async: function.r#async,
            span_for_unsupported: function.span,
        });
    }
}

fn collect_class_functions<'ast>(
    class_name: &str,
    class: &'ast Class<'ast>,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    for element in &class.body.body {
        if let ClassElement::MethodDefinition(method) = element
            && let Some(method_name) = method_name(method)
        {
            collect_function(
                format!("{class_name}.{method_name}"),
                method.span,
                &method.value,
                functions,
            );
        }
    }
}

fn collect_anonymous_functions_from_statement<'ast>(
    statement: &'ast Statement<'ast>,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    match statement {
        Statement::BlockStatement(block) => {
            for statement in &block.body {
                collect_anonymous_functions_from_statement(statement, functions);
            }
        }
        Statement::ExpressionStatement(statement) => {
            collect_anonymous_functions_from_expression(&statement.expression, true, functions);
        }
        Statement::VariableDeclaration(variable) => {
            for declarator in &variable.declarations {
                if let Some(init) = &declarator.init {
                    collect_anonymous_functions_from_expression(init, false, functions);
                }
            }
        }
        Statement::FunctionDeclaration(function) => {
            if let Some(body) = function.body.as_deref() {
                collect_anonymous_functions_from_body(body, functions);
            }
        }
        Statement::ReturnStatement(statement) => {
            if let Some(argument) = &statement.argument {
                collect_anonymous_functions_from_expression(argument, true, functions);
            }
        }
        Statement::ThrowStatement(statement) => {
            collect_anonymous_functions_from_expression(&statement.argument, true, functions);
        }
        Statement::IfStatement(statement) => {
            collect_anonymous_functions_from_expression(&statement.test, true, functions);
            collect_anonymous_functions_from_statement(&statement.consequent, functions);
            if let Some(alternate) = &statement.alternate {
                collect_anonymous_functions_from_statement(alternate, functions);
            }
        }
        Statement::WhileStatement(statement) => {
            collect_anonymous_functions_from_expression(&statement.test, true, functions);
            collect_anonymous_functions_from_statement(&statement.body, functions);
        }
        Statement::DoWhileStatement(statement) => {
            collect_anonymous_functions_from_statement(&statement.body, functions);
            collect_anonymous_functions_from_expression(&statement.test, true, functions);
        }
        Statement::ForStatement(statement) => {
            if let Some(init) = &statement.init {
                match init {
                    oxc_ast::ast::ForStatementInit::VariableDeclaration(variable) => {
                        for declarator in &variable.declarations {
                            if let Some(init) = &declarator.init {
                                collect_anonymous_functions_from_expression(init, false, functions);
                            }
                        }
                    }
                    _ => collect_anonymous_functions_from_expression(
                        init.to_expression(),
                        true,
                        functions,
                    ),
                }
            }
            if let Some(test) = &statement.test {
                collect_anonymous_functions_from_expression(test, true, functions);
            }
            if let Some(update) = &statement.update {
                collect_anonymous_functions_from_expression(update, true, functions);
            }
            collect_anonymous_functions_from_statement(&statement.body, functions);
        }
        Statement::ForInStatement(statement) => {
            collect_anonymous_functions_from_expression(&statement.right, true, functions);
            collect_anonymous_functions_from_statement(&statement.body, functions);
        }
        Statement::ForOfStatement(statement) => {
            collect_anonymous_functions_from_expression(&statement.right, true, functions);
            collect_anonymous_functions_from_statement(&statement.body, functions);
        }
        Statement::SwitchStatement(statement) => {
            collect_anonymous_functions_from_expression(&statement.discriminant, true, functions);
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    collect_anonymous_functions_from_expression(test, true, functions);
                }
                for statement in &case.consequent {
                    collect_anonymous_functions_from_statement(statement, functions);
                }
            }
        }
        Statement::ClassDeclaration(class) => {
            collect_anonymous_functions_from_class(class, functions)
        }
        _ => {}
    }
}

fn collect_anonymous_functions_from_class<'ast>(
    class: &'ast Class<'ast>,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    for element in &class.body.body {
        match element {
            ClassElement::MethodDefinition(method) => {
                if let Some(body) = method.value.body.as_deref() {
                    collect_anonymous_functions_from_body(body, functions);
                }
            }
            ClassElement::PropertyDefinition(property) => {
                if let Some(value) = &property.value {
                    collect_anonymous_functions_from_expression(value, true, functions);
                }
            }
            ClassElement::StaticBlock(block) => {
                for statement in &block.body {
                    collect_anonymous_functions_from_statement(statement, functions);
                }
            }
            _ => {}
        }
    }
}

fn collect_anonymous_functions_from_body<'ast>(
    body: &'ast FunctionBody<'ast>,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    for statement in &body.statements {
        collect_anonymous_functions_from_statement(statement, functions);
    }
}

fn collect_anonymous_functions_from_expression<'ast>(
    expression: &'ast Expression<'ast>,
    include_self: bool,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    match expression {
        Expression::ArrowFunctionExpression(function) => {
            if include_self {
                functions.push(TsFunctionCandidate {
                    name: anonymous_callable_name(function.span.start, function.span.end),
                    span: function.span,
                    parameters: parameter_names(&function.params),
                    body: &function.body,
                    r#async: function.r#async,
                    span_for_unsupported: function.span,
                });
            }
            collect_anonymous_functions_from_body(&function.body, functions);
        }
        Expression::FunctionExpression(function) => {
            if include_self && let Some(body) = function.body.as_deref() {
                functions.push(TsFunctionCandidate {
                    name: anonymous_callable_name(function.span.start, function.span.end),
                    span: function.span,
                    parameters: parameter_names(&function.params),
                    body,
                    r#async: function.r#async,
                    span_for_unsupported: function.span,
                });
            }
            if let Some(body) = function.body.as_deref() {
                collect_anonymous_functions_from_body(body, functions);
            }
        }
        Expression::CallExpression(call) => {
            collect_anonymous_functions_from_expression(&call.callee, true, functions);
            for argument in &call.arguments {
                collect_anonymous_functions_from_argument(argument, functions);
            }
        }
        Expression::NewExpression(expression) => {
            collect_anonymous_functions_from_expression(&expression.callee, true, functions);
            for argument in &expression.arguments {
                collect_anonymous_functions_from_argument(argument, functions);
            }
        }
        Expression::StaticMemberExpression(member) => {
            collect_anonymous_functions_from_expression(&member.object, true, functions);
        }
        Expression::ComputedMemberExpression(member) => {
            collect_anonymous_functions_from_expression(&member.object, true, functions);
            collect_anonymous_functions_from_expression(&member.expression, true, functions);
        }
        Expression::PrivateFieldExpression(member) => {
            collect_anonymous_functions_from_expression(&member.object, true, functions);
        }
        Expression::ChainExpression(chain) => {
            collect_anonymous_functions_from_chain_element(&chain.expression, functions);
        }
        Expression::ParenthesizedExpression(expression) => {
            collect_anonymous_functions_from_expression(
                &expression.expression,
                include_self,
                functions,
            );
        }
        Expression::AssignmentExpression(expression) => {
            collect_anonymous_functions_from_expression(&expression.right, true, functions);
        }
        Expression::AwaitExpression(expression) => {
            collect_anonymous_functions_from_expression(&expression.argument, true, functions);
        }
        Expression::YieldExpression(expression) => {
            if let Some(argument) = &expression.argument {
                collect_anonymous_functions_from_expression(argument, true, functions);
            }
        }
        Expression::SequenceExpression(expression) => {
            for expression in &expression.expressions {
                collect_anonymous_functions_from_expression(expression, true, functions);
            }
        }
        Expression::ConditionalExpression(expression) => {
            collect_anonymous_functions_from_expression(&expression.test, true, functions);
            collect_anonymous_functions_from_expression(&expression.consequent, true, functions);
            collect_anonymous_functions_from_expression(&expression.alternate, true, functions);
        }
        Expression::LogicalExpression(expression) => {
            collect_anonymous_functions_from_expression(&expression.left, true, functions);
            collect_anonymous_functions_from_expression(&expression.right, true, functions);
        }
        Expression::ObjectExpression(object) => {
            for property in &object.properties {
                match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        collect_anonymous_functions_from_expression(
                            &property.value,
                            true,
                            functions,
                        );
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => {
                        collect_anonymous_functions_from_expression(
                            &spread.argument,
                            true,
                            functions,
                        );
                    }
                }
            }
        }
        Expression::ArrayExpression(array) => {
            for element in &array.elements {
                match element {
                    oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                        collect_anonymous_functions_from_expression(
                            &spread.argument,
                            true,
                            functions,
                        );
                    }
                    oxc_ast::ast::ArrayExpressionElement::Elision(_) => {}
                    _ => collect_anonymous_functions_from_expression(
                        element.to_expression(),
                        true,
                        functions,
                    ),
                }
            }
        }
        _ => {}
    }
}

fn collect_anonymous_functions_from_chain_element<'ast>(
    element: &'ast oxc_ast::ast::ChainElement<'ast>,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    match element {
        oxc_ast::ast::ChainElement::CallExpression(call) => {
            collect_anonymous_functions_from_expression(&call.callee, true, functions);
            for argument in &call.arguments {
                collect_anonymous_functions_from_argument(argument, functions);
            }
        }
        oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
            collect_anonymous_functions_from_expression(&member.object, true, functions);
        }
        oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
            collect_anonymous_functions_from_expression(&member.object, true, functions);
            collect_anonymous_functions_from_expression(&member.expression, true, functions);
        }
        oxc_ast::ast::ChainElement::PrivateFieldExpression(member) => {
            collect_anonymous_functions_from_expression(&member.object, true, functions);
        }
        oxc_ast::ast::ChainElement::TSNonNullExpression(expression) => {
            collect_anonymous_functions_from_expression(&expression.expression, true, functions);
        }
    }
}

fn collect_anonymous_functions_from_argument<'ast>(
    argument: &'ast Argument<'ast>,
    functions: &mut Vec<TsFunctionCandidate<'ast>>,
) {
    match argument {
        Argument::SpreadElement(spread) => {
            collect_anonymous_functions_from_expression(&spread.argument, true, functions);
        }
        Argument::ArrowFunctionExpression(function) => {
            functions.push(TsFunctionCandidate {
                name: anonymous_callable_name(function.span.start, function.span.end),
                span: function.span,
                parameters: parameter_names(&function.params),
                body: &function.body,
                r#async: function.r#async,
                span_for_unsupported: function.span,
            });
            collect_anonymous_functions_from_body(&function.body, functions);
        }
        Argument::FunctionExpression(function) => {
            if let Some(body) = function.body.as_deref() {
                functions.push(TsFunctionCandidate {
                    name: anonymous_callable_name(function.span.start, function.span.end),
                    span: function.span,
                    parameters: parameter_names(&function.params),
                    body,
                    r#async: function.r#async,
                    span_for_unsupported: function.span,
                });
                collect_anonymous_functions_from_body(body, functions);
            }
        }
        _ => collect_anonymous_functions_from_expression(argument.to_expression(), true, functions),
    }
}

fn method_name(method: &MethodDefinition<'_>) -> Option<String> {
    match &method.key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
            Some(format!(
                "{}{}",
                constant_property_key_expression(&binary.left)?,
                constant_property_key_expression(&binary.right)?
            ))
        }
        _ => None,
    }
}

fn constant_property_key_expression(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::StringLiteral(literal) => Some(literal.value.to_string()),
        Expression::ParenthesizedExpression(expression) => {
            constant_property_key_expression(&expression.expression)
        }
        _ => None,
    }
}

fn parameter_names(parameters: &FormalParameters<'_>) -> Vec<String> {
    let mut names = Vec::new();
    for parameter in &parameters.items {
        if let Some(name) = binding_identifier_name(&parameter.pattern) {
            names.push(name);
        }
    }
    if let Some(rest) = &parameters.rest
        && let Some(name) = binding_identifier_name(&rest.rest.argument)
    {
        names.push(name);
    }
    names
}

fn binding_identifier_name(pattern: &BindingPattern<'_>) -> Option<String> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.to_string()),
        BindingPattern::AssignmentPattern(pattern) => binding_identifier_name(&pattern.left),
        _ => None,
    }
}

struct FunctionLowering<'source> {
    language: Language,
    file: FileId,
    source: &'source str,
    function: FunctionId,
    body: MirBodyId,
    stable_context: PlaceStableContext,
    parameters: BTreeMap<String, PlaceRoot>,
    locals: BTreeMap<String, PlaceRoot>,
}

impl<'source> FunctionLowering<'source> {
    fn new(file: &SourceFile, source: &'source str, function: FunctionId, body: &MirBody) -> Self {
        Self {
            language: file.language,
            file: file.id,
            source,
            function,
            body: body.id,
            stable_context: PlaceStableContext::new(
                file.relative_path.clone(),
                body.owner_stable_key.clone(),
                body.stable_key.clone(),
            ),
            parameters: BTreeMap::new(),
            locals: BTreeMap::new(),
        }
    }

    fn lower_parameters(&mut self, names: &[String], places: &mut PlaceTableBuilder) {
        for (index, name) in names.iter().enumerate() {
            let root = PlaceRoot::Parameter {
                function: self.function,
                index: index as u32,
                name: Some(name.clone()),
            };
            self.parameters.insert(name.clone(), root.clone());
            self.insert_place(places, root, Vec::new(), PlaceStatus::Resolved);
        }
    }

    fn lower_body(
        &mut self,
        body: &FunctionBody<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        self.lower_statements(&body.statements, places, operations, unsupported);
    }

    fn lower_statements(
        &mut self,
        statements: &[Statement<'_>],
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        for statement in statements {
            self.lower_statement(statement, places, operations, unsupported);
        }
    }

    fn lower_statement(
        &mut self,
        statement: &Statement<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        match statement {
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.lower_statement(statement, places, operations, unsupported);
                }
            }
            Statement::ExpressionStatement(statement) => {
                if let Some(shape) = self.lower_expression(
                    &statement.expression,
                    places,
                    operations,
                    unsupported,
                    false,
                ) {
                    self.push_operation(
                        operations,
                        statement.span,
                        OperationKindDraft::Read {
                            place_key: shape.key,
                        },
                        MirStatus::Partial,
                    );
                }
            }
            Statement::VariableDeclaration(variable) => {
                for declarator in &variable.declarations {
                    self.lower_variable_declarator(declarator, places, operations, unsupported);
                }
            }
            Statement::ReturnStatement(statement) => {
                let value = statement.argument.as_ref().and_then(|argument| {
                    self.lower_value(argument, places, operations, unsupported)
                });
                self.push_operation(
                    operations,
                    statement.span,
                    OperationKindDraft::Return { value },
                    MirStatus::Partial,
                );
            }
            Statement::IfStatement(statement) => {
                self.push_branch(operations, statement.span);
                self.lower_expression(&statement.test, places, operations, unsupported, false);
                self.lower_statement(&statement.consequent, places, operations, unsupported);
                if let Some(alternate) = &statement.alternate {
                    self.lower_statement(alternate, places, operations, unsupported);
                }
            }
            Statement::WhileStatement(statement) => {
                self.push_branch(operations, statement.span);
                self.lower_expression(&statement.test, places, operations, unsupported, false);
                self.lower_statement(&statement.body, places, operations, unsupported);
            }
            Statement::DoWhileStatement(statement) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    statement.span,
                    "do while",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.push_branch(operations, statement.span);
                self.lower_statement(&statement.body, places, operations, unsupported);
                self.lower_expression(&statement.test, places, operations, unsupported, false);
            }
            Statement::ForStatement(statement) => {
                if let Some(init) = &statement.init {
                    self.lower_for_statement_init(init, places, operations, unsupported);
                }
                if let Some(test) = &statement.test {
                    self.push_branch(operations, test.span());
                    self.lower_expression(test, places, operations, unsupported, false);
                }
                if let Some(update) = &statement.update {
                    self.lower_expression(update, places, operations, unsupported, false);
                }
                self.lower_statement(&statement.body, places, operations, unsupported);
            }
            Statement::ForInStatement(statement) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    statement.span,
                    "for-in",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.lower_for_statement_left(&statement.left, places, operations, unsupported);
                self.lower_expression(&statement.right, places, operations, unsupported, false);
                self.lower_statement(&statement.body, places, operations, unsupported);
            }
            Statement::ForOfStatement(statement) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    statement.span,
                    if statement.r#await {
                        "for await"
                    } else {
                        "for-of"
                    },
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.lower_for_statement_left(&statement.left, places, operations, unsupported);
                self.lower_expression(&statement.right, places, operations, unsupported, false);
                self.lower_statement(&statement.body, places, operations, unsupported);
            }
            Statement::SwitchStatement(statement) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    statement.span,
                    "switch",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.push_branch(operations, statement.span);
                self.lower_expression(
                    &statement.discriminant,
                    places,
                    operations,
                    unsupported,
                    false,
                );
                for case in &statement.cases {
                    if let Some(test) = &case.test {
                        self.push_branch(operations, test.span());
                        self.lower_expression(test, places, operations, unsupported, false);
                    }
                    for statement in &case.consequent {
                        self.lower_statement(statement, places, operations, unsupported);
                    }
                }
            }
            Statement::TryStatement(statement) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    statement.span,
                    "try",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                for statement in &statement.block.body {
                    self.lower_statement(statement, places, operations, unsupported);
                }
                if let Some(handler) = &statement.handler {
                    if let Some(param) = &handler.param {
                        if let Some(name) = binding_identifier_name(&param.pattern) {
                            self.insert_local(places, &name);
                        } else {
                            self.push_unsupported(
                                operations,
                                unsupported,
                                param.span,
                                "catch destructuring",
                                Vec::new(),
                                ConservativeAction::HavocAffectedPlaces,
                            );
                        }
                    }
                    for statement in &handler.body.body {
                        self.lower_statement(statement, places, operations, unsupported);
                    }
                }
                if let Some(finalizer) = &statement.finalizer {
                    for statement in &finalizer.body {
                        self.lower_statement(statement, places, operations, unsupported);
                    }
                }
            }
            Statement::ThrowStatement(statement) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    statement.span,
                    "throw",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.lower_expression(&statement.argument, places, operations, unsupported, false);
            }
            Statement::BreakStatement(statement) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    statement.span,
                    "break",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
            }
            Statement::ContinueStatement(statement) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    statement.span,
                    "continue",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
            }
            Statement::LabeledStatement(statement) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    statement.span,
                    "labeled statement",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.lower_statement(&statement.body, places, operations, unsupported);
            }
            Statement::WithStatement(statement) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    statement.span,
                    "with",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.lower_expression(&statement.object, places, operations, unsupported, false);
                self.lower_statement(&statement.body, places, operations, unsupported);
            }
            Statement::EmptyStatement(_) => {}
            Statement::DebuggerStatement(statement) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    statement.span,
                    "debugger",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
            }
            _ => {}
        }
    }

    fn lower_variable_declarator(
        &mut self,
        declarator: &VariableDeclarator<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        let Some(name) = binding_identifier_name(&declarator.id) else {
            if let Some(init) = &declarator.init {
                self.lower_expression(init, places, operations, unsupported, false);
            }
            self.push_unsupported(
                operations,
                unsupported,
                declarator.span,
                "complex destructuring",
                Vec::new(),
                ConservativeAction::HavocAffectedPlaces,
            );
            return;
        };
        let key = self.insert_local(places, &name);
        let value = declarator
            .init
            .as_ref()
            .and_then(|init| self.lower_value(init, places, operations, unsupported))
            .unwrap_or_else(|| ValueDraft::Unknown {
                evidence: "declaration initializer".to_string(),
            });
        self.push_assign(
            operations,
            declarator.span,
            key,
            value,
            AssignMode::DeclarationBinding,
        );
    }

    fn lower_for_statement_init(
        &mut self,
        init: &oxc_ast::ast::ForStatementInit<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        match init {
            oxc_ast::ast::ForStatementInit::VariableDeclaration(variable) => {
                for declarator in &variable.declarations {
                    self.lower_variable_declarator(declarator, places, operations, unsupported);
                }
            }
            _ => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    init.span(),
                    "for initializer",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.lower_expression(init.to_expression(), places, operations, unsupported, false);
            }
        }
    }

    fn lower_for_statement_left(
        &mut self,
        left: &oxc_ast::ast::ForStatementLeft<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        match left {
            oxc_ast::ast::ForStatementLeft::VariableDeclaration(variable) => {
                for declarator in &variable.declarations {
                    self.lower_variable_declarator(declarator, places, operations, unsupported);
                }
            }
            _ => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    left.span(),
                    "for left binding",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                if let Some(target) = self.assignment_target_shape(
                    left.to_assignment_target(),
                    places,
                    operations,
                    unsupported,
                ) {
                    self.push_assign(
                        operations,
                        left.span(),
                        target.key,
                        ValueDraft::Unknown {
                            evidence: "for left binding".to_string(),
                        },
                        if target.projections.is_empty() {
                            AssignMode::Overwrite
                        } else {
                            AssignMode::ProjectionMutation
                        },
                    );
                }
            }
        }
    }

    fn lower_value(
        &mut self,
        expression: &Expression<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) -> Option<ValueDraft> {
        match expression {
            Expression::StringLiteral(literal) => Some(ValueDraft::Literal {
                value: literal.value.to_string(),
            }),
            Expression::NumericLiteral(literal) => Some(ValueDraft::Literal {
                value: literal
                    .raw
                    .as_ref()
                    .map_or_else(|| literal.value.to_string(), ToString::to_string),
            }),
            Expression::BooleanLiteral(literal) => Some(ValueDraft::Literal {
                value: literal.value.to_string(),
            }),
            Expression::NullLiteral(_) => Some(ValueDraft::Literal {
                value: "null".to_string(),
            }),
            Expression::CallExpression(call) => self
                .lower_call(call, places, operations, unsupported)
                .map(ValueDraft::PlaceKey),
            _ => self
                .lower_expression(expression, places, operations, unsupported, false)
                .map(|shape| ValueDraft::PlaceKey(shape.key)),
        }
    }

    fn lower_expression(
        &mut self,
        expression: &Expression<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
        assignment_destination: bool,
    ) -> Option<PlaceShape> {
        match expression {
            Expression::Identifier(identifier) => {
                self.lower_identifier(identifier.name.as_str(), places, assignment_destination)
            }
            Expression::StaticMemberExpression(member) => self.lower_static_member(
                member,
                places,
                operations,
                unsupported,
                assignment_destination,
            ),
            Expression::ComputedMemberExpression(member) => self.lower_computed_member(
                member,
                places,
                operations,
                unsupported,
                assignment_destination,
            ),
            Expression::AssignmentExpression(assignment) => {
                if let Some(target) =
                    self.assignment_target_shape(&assignment.left, places, operations, unsupported)
                {
                    let value = self
                        .lower_value(&assignment.right, places, operations, unsupported)
                        .unwrap_or_else(|| ValueDraft::Unknown {
                            evidence: "assignment value".to_string(),
                        });
                    self.push_assign(
                        operations,
                        assignment.span,
                        target.key.clone(),
                        value,
                        if target.projections.is_empty() {
                            AssignMode::Overwrite
                        } else {
                            AssignMode::ProjectionMutation
                        },
                    );
                    Some(target)
                } else {
                    self.lower_expression(&assignment.right, places, operations, unsupported, false)
                }
            }
            Expression::UpdateExpression(update) => {
                let target = self.simple_assignment_target_shape(
                    &update.argument,
                    places,
                    operations,
                    unsupported,
                )?;
                self.push_assign(
                    operations,
                    update.span,
                    target.key.clone(),
                    ValueDraft::Unknown {
                        evidence: "update expression".to_string(),
                    },
                    AssignMode::Overwrite,
                );
                Some(target)
            }
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    match property {
                        ObjectPropertyKind::ObjectProperty(property) => {
                            if property.computed {
                                self.lower_expression(
                                    property.key.to_expression(),
                                    places,
                                    operations,
                                    unsupported,
                                    false,
                                );
                            }
                            self.lower_expression(
                                &property.value,
                                places,
                                operations,
                                unsupported,
                                false,
                            );
                        }
                        ObjectPropertyKind::SpreadProperty(spread) => {
                            self.push_unsupported(
                                operations,
                                unsupported,
                                spread.span,
                                "spread",
                                Vec::new(),
                                ConservativeAction::HavocAffectedPlaces,
                            );
                            self.lower_expression(
                                &spread.argument,
                                places,
                                operations,
                                unsupported,
                                false,
                            );
                        }
                    }
                }
                Some(self.temporary_shape(places, expression.span()))
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    match element {
                        oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) => {
                            self.push_unsupported(
                                operations,
                                unsupported,
                                spread.span,
                                "spread",
                                Vec::new(),
                                ConservativeAction::HavocAffectedPlaces,
                            );
                            self.lower_expression(
                                &spread.argument,
                                places,
                                operations,
                                unsupported,
                                false,
                            );
                        }
                        oxc_ast::ast::ArrayExpressionElement::Elision(_) => {}
                        _ => {
                            self.lower_expression(
                                element.to_expression(),
                                places,
                                operations,
                                unsupported,
                                false,
                            );
                        }
                    }
                }
                Some(self.temporary_shape(places, expression.span()))
            }
            Expression::ParenthesizedExpression(parenthesized) => self.lower_expression(
                &parenthesized.expression,
                places,
                operations,
                unsupported,
                assignment_destination,
            ),
            Expression::TSAsExpression(expression) => self.lower_expression(
                &expression.expression,
                places,
                operations,
                unsupported,
                assignment_destination,
            ),
            Expression::TSSatisfiesExpression(expression) => self.lower_expression(
                &expression.expression,
                places,
                operations,
                unsupported,
                assignment_destination,
            ),
            Expression::TSNonNullExpression(expression) => self.lower_expression(
                &expression.expression,
                places,
                operations,
                unsupported,
                assignment_destination,
            ),
            Expression::TSTypeAssertion(expression) => self.lower_expression(
                &expression.expression,
                places,
                operations,
                unsupported,
                assignment_destination,
            ),
            Expression::CallExpression(call) => self
                .lower_call(call, places, operations, unsupported)
                .map(|key| PlaceShape {
                    root: PlaceRoot::CallReturn {
                        call: call_site_for_span(normalized_call_expression_span(
                            self.source,
                            call,
                        )),
                    },
                    projections: Vec::new(),
                    status: PlaceStatus::Partial,
                    key,
                }),
            Expression::BinaryExpression(binary) => {
                self.lower_expression(&binary.left, places, operations, unsupported, false);
                self.lower_expression(&binary.right, places, operations, unsupported, false);
                Some(self.temporary_shape(places, binary.span))
            }
            Expression::UnaryExpression(unary) => {
                self.lower_expression(&unary.argument, places, operations, unsupported, false);
                Some(self.temporary_shape(places, unary.span))
            }
            Expression::ConditionalExpression(conditional) => {
                self.push_branch(operations, conditional.span);
                self.lower_expression(&conditional.test, places, operations, unsupported, false);
                self.lower_expression(
                    &conditional.consequent,
                    places,
                    operations,
                    unsupported,
                    false,
                );
                self.lower_expression(
                    &conditional.alternate,
                    places,
                    operations,
                    unsupported,
                    false,
                )
            }
            Expression::LogicalExpression(logical) => {
                if matches!(
                    logical.operator,
                    LogicalOperator::And | LogicalOperator::Or | LogicalOperator::Coalesce
                ) {
                    self.push_branch(operations, logical.span);
                }
                self.lower_expression(&logical.left, places, operations, unsupported, false);
                self.lower_expression(&logical.right, places, operations, unsupported, false)
            }
            Expression::TemplateLiteral(template) => {
                for expression in &template.expressions {
                    self.lower_expression(expression, places, operations, unsupported, false);
                }
                Some(self.temporary_shape(places, template.span))
            }
            Expression::TaggedTemplateExpression(tagged) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    tagged.span,
                    "tagged template",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.lower_expression(&tagged.tag, places, operations, unsupported, false);
                for expression in &tagged.quasi.expressions {
                    self.lower_expression(expression, places, operations, unsupported, false);
                }
                Some(self.temporary_shape(places, tagged.span))
            }
            Expression::SequenceExpression(sequence) => {
                let mut last = None;
                for expression in &sequence.expressions {
                    last =
                        self.lower_expression(expression, places, operations, unsupported, false);
                }
                last.or_else(|| Some(self.temporary_shape(places, sequence.span)))
            }
            Expression::ChainExpression(chain) => {
                self.lower_chain_element(&chain.expression, places, operations, unsupported)
            }
            Expression::AwaitExpression(await_expression) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    await_expression.span,
                    "await",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.lower_expression(
                    &await_expression.argument,
                    places,
                    operations,
                    unsupported,
                    false,
                )
            }
            Expression::ImportExpression(import_expression) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    import_expression.span,
                    "dynamic import",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.lower_expression(
                    &import_expression.source,
                    places,
                    operations,
                    unsupported,
                    false,
                );
                if let Some(options) = &import_expression.options {
                    self.lower_expression(options, places, operations, unsupported, false);
                }
                Some(self.temporary_shape(places, import_expression.span))
            }
            Expression::YieldExpression(yield_expression) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    yield_expression.span,
                    "yield",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                yield_expression.argument.as_ref().and_then(|argument| {
                    self.lower_expression(argument, places, operations, unsupported, false)
                })
            }
            Expression::NewExpression(new_expression) => self
                .lower_new_expression(new_expression, places, operations, unsupported)
                .map(|key| PlaceShape {
                    root: PlaceRoot::CallReturn {
                        call: call_site_for_span(normalized_new_expression_span(
                            self.source,
                            new_expression,
                        )),
                    },
                    projections: Vec::new(),
                    status: PlaceStatus::Partial,
                    key,
                }),
            Expression::PrivateFieldExpression(private) => self.lower_private_field(
                private,
                places,
                operations,
                unsupported,
                assignment_destination,
            ),
            Expression::PrivateInExpression(private_in) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    private_in.span,
                    "private in",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.lower_expression(&private_in.right, places, operations, unsupported, false);
                Some(self.temporary_shape(places, private_in.span))
            }
            Expression::ArrowFunctionExpression(function) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    function.span,
                    "function expression",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                Some(self.temporary_shape(places, function.span))
            }
            Expression::FunctionExpression(function) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    function.span,
                    "function expression",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                Some(self.temporary_shape(places, function.span))
            }
            Expression::ClassExpression(class) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    class.span,
                    "class expression",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                Some(self.temporary_shape(places, class.span))
            }
            Expression::ThisExpression(this_expression) => Some(self.keyword_shape(
                places,
                "this",
                this_expression.span,
                assignment_destination,
            )),
            Expression::Super(super_expression) => Some(self.keyword_shape(
                places,
                "super",
                super_expression.span,
                assignment_destination,
            )),
            Expression::MetaProperty(meta) => Some(self.keyword_shape(
                places,
                &format!("{}.{}", meta.meta.name, meta.property.name),
                meta.span,
                assignment_destination,
            )),
            Expression::TSInstantiationExpression(expression) => self.lower_expression(
                &expression.expression,
                places,
                operations,
                unsupported,
                assignment_destination,
            ),
            Expression::JSXElement(element) => {
                if source_text(self.source, element.span).is_some_and(|text| text.contains("=>")) {
                    self.push_unsupported(
                        operations,
                        unsupported,
                        element.span,
                        "JSX callback scheduling",
                        Vec::new(),
                        ConservativeAction::HavocAffectedPlaces,
                    );
                }
                self.lower_jsx_element(element, places, operations, unsupported);
                Some(self.temporary_shape(places, element.span))
            }
            Expression::JSXFragment(fragment) => {
                self.lower_jsx_children(&fragment.children, places, operations, unsupported);
                Some(self.temporary_shape(places, fragment.span))
            }
            Expression::V8IntrinsicExpression(intrinsic) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    intrinsic.span,
                    "v8 intrinsic",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                Some(self.temporary_shape(places, intrinsic.span))
            }
            Expression::BigIntLiteral(literal) => Some(self.temporary_shape(places, literal.span)),
            Expression::RegExpLiteral(literal) => Some(self.temporary_shape(places, literal.span)),
            Expression::BooleanLiteral(_)
            | Expression::NullLiteral(_)
            | Expression::NumericLiteral(_)
            | Expression::StringLiteral(_) => Some(self.temporary_shape(places, expression.span())),
        }
    }

    fn lower_chain_element(
        &mut self,
        element: &oxc_ast::ast::ChainElement<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) -> Option<PlaceShape> {
        match element {
            oxc_ast::ast::ChainElement::CallExpression(call) => self
                .lower_call(call, places, operations, unsupported)
                .map(|key| PlaceShape {
                    root: PlaceRoot::CallReturn {
                        call: call_site_for_span(normalized_call_expression_span(
                            self.source,
                            call,
                        )),
                    },
                    projections: Vec::new(),
                    status: PlaceStatus::Partial,
                    key,
                }),
            oxc_ast::ast::ChainElement::StaticMemberExpression(member) => {
                self.lower_static_member(member, places, operations, unsupported, false)
            }
            oxc_ast::ast::ChainElement::ComputedMemberExpression(member) => {
                self.lower_computed_member(member, places, operations, unsupported, false)
            }
            oxc_ast::ast::ChainElement::PrivateFieldExpression(private) => {
                self.lower_private_field(private, places, operations, unsupported, false)
            }
            oxc_ast::ast::ChainElement::TSNonNullExpression(expression) => self.lower_expression(
                &expression.expression,
                places,
                operations,
                unsupported,
                false,
            ),
        }
    }

    fn lower_private_field(
        &mut self,
        private: &oxc_ast::ast::PrivateFieldExpression<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
        _assignment_destination: bool,
    ) -> Option<PlaceShape> {
        if private.optional {
            self.push_unsupported(
                operations,
                unsupported,
                private.span,
                "optional chaining",
                Vec::new(),
                ConservativeAction::HavocAffectedPlaces,
            );
        }
        self.push_unsupported(
            operations,
            unsupported,
            private.field.span,
            "private field",
            Vec::new(),
            ConservativeAction::HavocAffectedPlaces,
        );
        let mut shape =
            self.lower_expression(&private.object, places, operations, unsupported, false)?;
        shape.projections.push(PlaceProjection::Unknown {
            evidence: format!("#{}", private.field.name),
        });
        shape.key = self.insert_shape(places, &shape);
        self.insert_temporary(places, private.span, PlaceStatus::Partial);
        Some(shape)
    }

    fn keyword_shape(
        &self,
        places: &mut PlaceTableBuilder,
        evidence: &str,
        span: oxc_span::Span,
        _assignment_destination: bool,
    ) -> PlaceShape {
        let root = PlaceRoot::Unknown {
            evidence: evidence.to_string(),
        };
        let key = self.insert_place(places, root.clone(), Vec::new(), PlaceStatus::Unknown);
        self.insert_temporary(places, span, PlaceStatus::Partial);
        PlaceShape {
            root,
            projections: Vec::new(),
            status: PlaceStatus::Unknown,
            key,
        }
    }

    fn lower_jsx_element(
        &mut self,
        element: &oxc_ast::ast::JSXElement<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        for attribute in &element.opening_element.attributes {
            match attribute {
                oxc_ast::ast::JSXAttributeItem::Attribute(attribute) => {
                    if let Some(value) = &attribute.value {
                        self.lower_jsx_attribute_value(value, places, operations, unsupported);
                    }
                }
                oxc_ast::ast::JSXAttributeItem::SpreadAttribute(spread) => {
                    self.push_unsupported(
                        operations,
                        unsupported,
                        spread.span,
                        "spread",
                        Vec::new(),
                        ConservativeAction::HavocAffectedPlaces,
                    );
                    self.lower_expression(&spread.argument, places, operations, unsupported, false);
                }
            }
        }
        self.lower_jsx_children(&element.children, places, operations, unsupported);
    }

    fn lower_jsx_attribute_value(
        &mut self,
        value: &oxc_ast::ast::JSXAttributeValue<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        match value {
            oxc_ast::ast::JSXAttributeValue::ExpressionContainer(container) => {
                self.lower_jsx_expression(&container.expression, places, operations, unsupported);
            }
            oxc_ast::ast::JSXAttributeValue::Element(element) => {
                self.lower_jsx_element(element, places, operations, unsupported);
            }
            oxc_ast::ast::JSXAttributeValue::Fragment(fragment) => {
                self.lower_jsx_children(&fragment.children, places, operations, unsupported);
            }
            oxc_ast::ast::JSXAttributeValue::StringLiteral(_) => {}
        }
    }

    fn lower_jsx_children(
        &mut self,
        children: &[oxc_ast::ast::JSXChild<'_>],
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        for child in children {
            match child {
                oxc_ast::ast::JSXChild::Element(element) => {
                    self.lower_jsx_element(element, places, operations, unsupported);
                }
                oxc_ast::ast::JSXChild::Fragment(fragment) => {
                    self.lower_jsx_children(&fragment.children, places, operations, unsupported);
                }
                oxc_ast::ast::JSXChild::ExpressionContainer(container) => {
                    self.lower_jsx_expression(
                        &container.expression,
                        places,
                        operations,
                        unsupported,
                    );
                }
                oxc_ast::ast::JSXChild::Spread(spread) => {
                    self.push_unsupported(
                        operations,
                        unsupported,
                        spread.span,
                        "spread",
                        Vec::new(),
                        ConservativeAction::HavocAffectedPlaces,
                    );
                    self.lower_expression(
                        &spread.expression,
                        places,
                        operations,
                        unsupported,
                        false,
                    );
                }
                oxc_ast::ast::JSXChild::Text(_) => {}
            }
        }
    }

    fn lower_jsx_expression(
        &mut self,
        expression: &oxc_ast::ast::JSXExpression<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        match expression {
            oxc_ast::ast::JSXExpression::EmptyExpression(_) => {}
            _ => {
                self.lower_expression(
                    expression.to_expression(),
                    places,
                    operations,
                    unsupported,
                    false,
                );
            }
        }
    }

    fn lower_static_member(
        &mut self,
        member: &oxc_ast::ast::StaticMemberExpression<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
        _assignment_destination: bool,
    ) -> Option<PlaceShape> {
        if member.optional {
            self.push_unsupported(
                operations,
                unsupported,
                member.span,
                "optional chaining",
                Vec::new(),
                ConservativeAction::HavocAffectedPlaces,
            );
        }
        let mut shape =
            self.lower_expression(&member.object, places, operations, unsupported, false)?;
        shape
            .projections
            .push(PlaceProjection::Property(member.property.name.to_string()));
        shape.key = self.insert_shape(places, &shape);
        self.insert_temporary(places, member.span, PlaceStatus::Partial);
        Some(shape)
    }

    fn lower_computed_member(
        &mut self,
        member: &oxc_ast::ast::ComputedMemberExpression<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
        _assignment_destination: bool,
    ) -> Option<PlaceShape> {
        if member.optional {
            self.push_unsupported(
                operations,
                unsupported,
                member.span,
                "optional chaining",
                Vec::new(),
                ConservativeAction::HavocAffectedPlaces,
            );
        }
        let mut shape =
            self.lower_expression(&member.object, places, operations, unsupported, false)?;
        shape.projections.push(index_projection(&member.expression));
        shape.key = self.insert_shape(places, &shape);
        if matches!(
            shape.projections.last(),
            Some(PlaceProjection::IndexUnknown { .. })
        ) {
            self.push_unsupported(
                operations,
                unsupported,
                member.expression.span(),
                "dynamic property key",
                vec![shape.key.clone()],
                ConservativeAction::HavocAffectedPlaces,
            );
        }
        self.lower_expression(&member.expression, places, operations, unsupported, false);
        self.insert_temporary(places, member.span, PlaceStatus::Partial);
        Some(shape)
    }

    fn lower_identifier(
        &self,
        name: &str,
        places: &mut PlaceTableBuilder,
        assignment_destination: bool,
    ) -> Option<PlaceShape> {
        let (root, status) = if let Some(root) = self.locals.get(name) {
            (root.clone(), PlaceStatus::Resolved)
        } else if let Some(root) = self.parameters.get(name) {
            (root.clone(), PlaceStatus::Resolved)
        } else if assignment_destination {
            (
                PlaceRoot::Global {
                    symbol: None,
                    name: name.to_string(),
                },
                PlaceStatus::Partial,
            )
        } else {
            (
                PlaceRoot::Unknown {
                    evidence: name.to_string(),
                },
                PlaceStatus::Unknown,
            )
        };
        let mut shape = PlaceShape {
            root,
            projections: Vec::new(),
            status,
            key: String::new(),
        };
        shape.key = self.insert_shape(places, &shape);
        Some(shape)
    }

    fn assignment_target_shape(
        &mut self,
        target: &oxc_ast::ast::AssignmentTarget<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) -> Option<PlaceShape> {
        match target {
            oxc_ast::ast::AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                self.lower_identifier(identifier.name.as_str(), places, true)
            }
            oxc_ast::ast::AssignmentTarget::StaticMemberExpression(member) => {
                self.lower_static_member(member, places, operations, unsupported, true)
            }
            oxc_ast::ast::AssignmentTarget::ComputedMemberExpression(member) => {
                self.lower_computed_member(member, places, operations, unsupported, true)
            }
            oxc_ast::ast::AssignmentTarget::PrivateFieldExpression(private) => {
                self.lower_private_field(private, places, operations, unsupported, true)
            }
            oxc_ast::ast::AssignmentTarget::TSAsExpression(expression) => self.lower_expression(
                &expression.expression,
                places,
                operations,
                unsupported,
                true,
            ),
            oxc_ast::ast::AssignmentTarget::TSSatisfiesExpression(expression) => self
                .lower_expression(
                    &expression.expression,
                    places,
                    operations,
                    unsupported,
                    true,
                ),
            oxc_ast::ast::AssignmentTarget::TSNonNullExpression(expression) => self
                .lower_expression(
                    &expression.expression,
                    places,
                    operations,
                    unsupported,
                    true,
                ),
            oxc_ast::ast::AssignmentTarget::TSTypeAssertion(expression) => self.lower_expression(
                &expression.expression,
                places,
                operations,
                unsupported,
                true,
            ),
            oxc_ast::ast::AssignmentTarget::ObjectAssignmentTarget(_)
            | oxc_ast::ast::AssignmentTarget::ArrayAssignmentTarget(_) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    target.span(),
                    "complex destructuring",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                None
            }
        }
    }

    fn simple_assignment_target_shape(
        &mut self,
        target: &oxc_ast::ast::SimpleAssignmentTarget<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) -> Option<PlaceShape> {
        match target {
            oxc_ast::ast::SimpleAssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                self.lower_identifier(identifier.name.as_str(), places, true)
            }
            oxc_ast::ast::SimpleAssignmentTarget::StaticMemberExpression(member) => {
                self.lower_static_member(member, places, operations, unsupported, true)
            }
            oxc_ast::ast::SimpleAssignmentTarget::ComputedMemberExpression(member) => {
                self.lower_computed_member(member, places, operations, unsupported, true)
            }
            oxc_ast::ast::SimpleAssignmentTarget::PrivateFieldExpression(private) => {
                self.lower_private_field(private, places, operations, unsupported, true)
            }
            oxc_ast::ast::SimpleAssignmentTarget::TSAsExpression(expression) => self
                .lower_expression(
                    &expression.expression,
                    places,
                    operations,
                    unsupported,
                    true,
                ),
            oxc_ast::ast::SimpleAssignmentTarget::TSSatisfiesExpression(expression) => self
                .lower_expression(
                    &expression.expression,
                    places,
                    operations,
                    unsupported,
                    true,
                ),
            oxc_ast::ast::SimpleAssignmentTarget::TSNonNullExpression(expression) => self
                .lower_expression(
                    &expression.expression,
                    places,
                    operations,
                    unsupported,
                    true,
                ),
            oxc_ast::ast::SimpleAssignmentTarget::TSTypeAssertion(expression) => self
                .lower_expression(
                    &expression.expression,
                    places,
                    operations,
                    unsupported,
                    true,
                ),
        }
    }

    fn insert_local(&mut self, places: &mut PlaceTableBuilder, name: &str) -> String {
        let root = PlaceRoot::Local {
            function: self.function,
            name: name.to_string(),
        };
        self.locals.insert(name.to_string(), root.clone());
        self.insert_place(places, root, Vec::new(), PlaceStatus::Resolved)
    }

    fn temporary_shape(&self, places: &mut PlaceTableBuilder, span: oxc_span::Span) -> PlaceShape {
        let root = PlaceRoot::Temporary {
            body: self.body,
            ordinal: span.start,
        };
        let key = self.insert_place(places, root.clone(), Vec::new(), PlaceStatus::Partial);
        PlaceShape {
            root,
            projections: Vec::new(),
            status: PlaceStatus::Partial,
            key,
        }
    }

    fn insert_temporary(
        &self,
        places: &mut PlaceTableBuilder,
        span: oxc_span::Span,
        status: PlaceStatus,
    ) -> String {
        self.insert_place(
            places,
            PlaceRoot::Temporary {
                body: self.body,
                ordinal: span.start,
            },
            Vec::new(),
            status,
        )
    }

    fn insert_shape(&self, places: &mut PlaceTableBuilder, shape: &PlaceShape) -> String {
        self.insert_place(
            places,
            shape.root.clone(),
            shape.projections.clone(),
            shape.status,
        )
    }

    fn insert_place(
        &self,
        places: &mut PlaceTableBuilder,
        root: PlaceRoot,
        projections: Vec<PlaceProjection>,
        status: PlaceStatus,
    ) -> String {
        places.insert_with_context(
            &self.stable_context,
            PlaceInsert {
                language: self.language,
                file: Some(self.file),
                function: Some(self.function),
                root,
                projections,
                status,
            },
        )
    }

    fn push_assign(
        &self,
        operations: &mut Vec<OperationDraft>,
        span: oxc_span::Span,
        place_key: String,
        value: ValueDraft,
        mode: AssignMode,
    ) {
        self.push_operation(
            operations,
            span,
            OperationKindDraft::Assign {
                place_key,
                value,
                mode,
            },
            MirStatus::Partial,
        );
    }

    fn lower_call(
        &mut self,
        call: &oxc_ast::ast::CallExpression<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) -> Option<String> {
        if call.optional {
            self.push_unsupported(
                operations,
                unsupported,
                call.span,
                "optional chaining",
                Vec::new(),
                ConservativeAction::HavocAffectedPlaces,
            );
        }
        if callee_text(&call.callee).as_deref() == Some("eval") {
            self.push_unsupported(
                operations,
                unsupported,
                call.span,
                "eval",
                Vec::new(),
                ConservativeAction::HavocAffectedPlaces,
            );
        }
        if callee_text(&call.callee).as_deref() == Some("require")
            && !matches!(call.arguments.first(), Some(Argument::StringLiteral(_)))
        {
            self.push_unsupported(
                operations,
                unsupported,
                call.span,
                "dynamic CommonJS require",
                Vec::new(),
                ConservativeAction::HavocAffectedPlaces,
            );
        }
        let span = normalized_call_expression_span(self.source, call);
        let site = call_site_for_span(span);
        let return_key = self.insert_place(
            places,
            PlaceRoot::CallReturn { call: site },
            Vec::new(),
            PlaceStatus::Partial,
        );
        self.lower_expression(&call.callee, places, operations, unsupported, false);
        let callee = callee_text(&call.callee).map_or_else(
            || ValueDraft::Unknown {
                evidence: "call".to_string(),
            },
            |evidence| ValueDraft::Unknown { evidence },
        );
        let mut arguments = Vec::new();
        for argument in &call.arguments {
            if let Some(shape) = self.argument_shape(argument, places, operations, unsupported) {
                arguments.push(shape.key);
            }
        }
        self.push_operation(
            operations,
            span,
            OperationKindDraft::Call {
                site,
                callee,
                arguments,
                return_place_key: return_key.clone(),
            },
            MirStatus::Partial,
        );
        Some(return_key)
    }

    fn lower_new_expression(
        &mut self,
        expression: &oxc_ast::ast::NewExpression<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) -> Option<String> {
        if callee_text(&expression.callee).as_deref() == Some("Proxy") {
            self.push_unsupported(
                operations,
                unsupported,
                expression.span,
                "Proxy",
                Vec::new(),
                ConservativeAction::HavocAffectedPlaces,
            );
        }
        let span = normalized_new_expression_span(self.source, expression);
        let site = call_site_for_span(span);
        let return_key = self.insert_place(
            places,
            PlaceRoot::CallReturn { call: site },
            Vec::new(),
            PlaceStatus::Partial,
        );
        self.lower_expression(&expression.callee, places, operations, unsupported, false);
        let callee = callee_text(&expression.callee).map_or_else(
            || ValueDraft::Unknown {
                evidence: "new".to_string(),
            },
            |evidence| ValueDraft::Unknown {
                evidence: format!("new {evidence}"),
            },
        );
        let mut arguments = Vec::new();
        for argument in &expression.arguments {
            if let Some(shape) = self.argument_shape(argument, places, operations, unsupported) {
                arguments.push(shape.key);
            }
        }
        self.push_operation(
            operations,
            span,
            OperationKindDraft::Call {
                site,
                callee,
                arguments,
                return_place_key: return_key.clone(),
            },
            MirStatus::Partial,
        );
        Some(return_key)
    }

    fn argument_shape(
        &mut self,
        argument: &Argument<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) -> Option<PlaceShape> {
        match argument {
            Argument::SpreadElement(spread) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    spread.span,
                    "spread",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                self.lower_expression(&spread.argument, places, operations, unsupported, false)
            }
            _ => self.lower_expression(
                argument.to_expression(),
                places,
                operations,
                unsupported,
                false,
            ),
        }
    }

    fn push_branch(&self, operations: &mut Vec<OperationDraft>, span: oxc_span::Span) {
        self.push_operation(
            operations,
            span,
            OperationKindDraft::Branch {
                predicate: MirPredicateId(span.start as u64),
                predicate_place_key: None,
            },
            MirStatus::Partial,
        );
    }

    fn push_unsupported(
        &self,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
        span: oxc_span::Span,
        construct: &str,
        affected_place_keys: Vec<String>,
        action: ConservativeAction,
    ) {
        let unsupported_id = UnsupportedId(unsupported.len() as u64);
        let operation_id = MirOpId(operations.len() as u64);
        let source_evidence = source_text(self.source, span)
            .unwrap_or(construct)
            .to_string();
        let span = span_from_oxc(self.file, self.source, span);
        unsupported.push(UnsupportedDraft::new(UnsupportedDraftInput {
            id: unsupported_id,
            body: Some(self.body),
            operation: Some(operation_id),
            language: self.language,
            file_key: self.stable_context.file_key().to_string(),
            file: self.file,
            span: span.clone(),
            construct: construct.to_string(),
            source_evidence,
            affected_place_keys,
            affected_domains: unsupported_domains_for(construct),
            conservative_action: action,
        }));
        operations.push(OperationDraft::new(
            operation_id,
            self.body,
            self.stable_context.body_key(),
            span.start_byte,
            span,
            OperationKindDraft::Unsupported { unsupported_id },
            MirStatus::Unsupported,
        ));
    }

    fn push_operation(
        &self,
        operations: &mut Vec<OperationDraft>,
        span: oxc_span::Span,
        kind: OperationKindDraft,
        status: MirStatus,
    ) {
        let id = MirOpId(operations.len() as u64);
        operations.push(OperationDraft::new(
            id,
            self.body,
            self.stable_context.body_key(),
            span.start,
            span_from_oxc(self.file, self.source, span),
            kind,
            status,
        ));
    }
}

#[derive(Debug, Clone)]
struct PlaceShape {
    root: PlaceRoot,
    projections: Vec<PlaceProjection>,
    status: PlaceStatus,
    key: String,
}

#[derive(Debug, Clone)]
struct OperationDraft {
    id: MirOpId,
    body: MirBodyId,
    ordinal: u32,
    span: Span,
    kind: OperationKindDraft,
    stable_key: String,
    status: MirStatus,
}

impl OperationDraft {
    fn new(
        id: MirOpId,
        body: MirBodyId,
        body_stable_key: &str,
        ordinal: u32,
        span: Span,
        kind: OperationKindDraft,
        status: MirStatus,
    ) -> Self {
        let stable_key = operation_stable_key(body_stable_key, ordinal, &span, &kind);
        Self {
            id,
            body,
            ordinal,
            span,
            kind,
            stable_key,
            status,
        }
    }

    fn to_operation(&self, place_ids: &BTreeMap<String, PlaceId>) -> Option<MirOperation> {
        Some(MirOperation {
            id: self.id,
            body: self.body,
            ordinal: self.ordinal,
            span: self.span.clone(),
            kind: self.kind.to_kind(place_ids)?,
            stable_key: self.stable_key.clone(),
            status: self.status,
        })
    }
}

#[derive(Debug, Clone)]
enum OperationKindDraft {
    Assign {
        place_key: String,
        value: ValueDraft,
        mode: AssignMode,
    },
    Read {
        place_key: String,
    },
    Branch {
        predicate: MirPredicateId,
        predicate_place_key: Option<String>,
    },
    Call {
        site: CallSiteId,
        callee: ValueDraft,
        arguments: Vec<String>,
        return_place_key: String,
    },
    Return {
        value: Option<ValueDraft>,
    },
    Unsupported {
        unsupported_id: UnsupportedId,
    },
}

impl OperationKindDraft {
    fn to_kind(&self, place_ids: &BTreeMap<String, PlaceId>) -> Option<MirOperationKind> {
        match self {
            Self::Assign {
                place_key,
                value,
                mode,
            } => Some(MirOperationKind::Assign {
                place: *place_ids.get(place_key)?,
                value: value.to_value(place_ids),
                mode: *mode,
            }),
            Self::Read { place_key } => Some(MirOperationKind::Read {
                place: *place_ids.get(place_key)?,
            }),
            Self::Branch {
                predicate,
                predicate_place_key,
            } => Some(MirOperationKind::Branch {
                predicate: *predicate,
                predicate_place: predicate_place_key
                    .as_ref()
                    .and_then(|key| place_ids.get(key))
                    .copied(),
            }),
            Self::Call {
                site,
                callee,
                arguments,
                return_place_key,
            } => {
                debug_assert!(
                    arguments.iter().all(|key| place_ids.contains_key(key)),
                    "MIR call argument place key missing from place table"
                );
                Some(MirOperationKind::Call {
                    site: *site,
                    callee: callee.to_value(place_ids),
                    arguments: arguments
                        .iter()
                        .map(|key| place_ids.get(key).copied())
                        .collect::<Option<Vec<_>>>()?,
                    return_place: *place_ids.get(return_place_key)?,
                })
            }
            Self::Return { value } => Some(MirOperationKind::Return {
                value: value.as_ref().map(|value| value.to_value(place_ids)),
            }),
            Self::Unsupported { unsupported_id } => Some(MirOperationKind::Unsupported {
                unsupported: *unsupported_id,
            }),
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Assign { .. } => "assign",
            Self::Read { .. } => "read",
            Self::Branch { .. } => "branch",
            Self::Call { .. } => "call",
            Self::Return { .. } => "return",
            Self::Unsupported { .. } => "unsupported",
        }
    }

    fn place_keys(&self) -> Vec<String> {
        match self {
            Self::Assign {
                place_key, value, ..
            } => {
                let mut keys = vec![place_key.clone()];
                keys.extend(value.place_keys());
                keys
            }
            Self::Read { place_key } => vec![place_key.clone()],
            Self::Call {
                arguments,
                return_place_key,
                callee,
                ..
            } => {
                let mut keys = arguments.clone();
                keys.push(return_place_key.clone());
                keys.extend(callee.place_keys());
                keys
            }
            Self::Return { value } => value.as_ref().map_or_else(Vec::new, ValueDraft::place_keys),
            Self::Branch { .. } | Self::Unsupported { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum ValueDraft {
    PlaceKey(String),
    Literal { value: String },
    Unknown { evidence: String },
}

impl ValueDraft {
    fn to_value(&self, place_ids: &BTreeMap<String, PlaceId>) -> MirValue {
        match self {
            Self::PlaceKey(key) => place_ids.get(key).copied().map_or_else(
                || MirValue::Unknown {
                    evidence: key.clone(),
                },
                MirValue::Place,
            ),
            Self::Literal { value } if value.trim().is_empty() => MirValue::Unknown {
                evidence: "empty literal lowering".to_string(),
            },
            Self::Literal { value } => MirValue::Literal {
                value: value.trim().to_string(),
            },
            Self::Unknown { evidence } => MirValue::Unknown {
                evidence: evidence.clone(),
            },
        }
    }

    fn place_keys(&self) -> Vec<String> {
        match self {
            Self::PlaceKey(key) => vec![key.clone()],
            Self::Literal { .. } | Self::Unknown { .. } => Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct UnsupportedDraft {
    id: UnsupportedId,
    body: Option<MirBodyId>,
    operation: Option<MirOpId>,
    language: Language,
    file_key: String,
    file: FileId,
    span: Span,
    construct: String,
    source_evidence: String,
    affected_place_keys: Vec<String>,
    affected_domains: Vec<UnsupportedDomain>,
    conservative_action: ConservativeAction,
}

struct UnsupportedDraftInput<S> {
    id: UnsupportedId,
    body: Option<MirBodyId>,
    operation: Option<MirOpId>,
    language: Language,
    file_key: String,
    file: FileId,
    span: Span,
    construct: String,
    source_evidence: S,
    affected_place_keys: Vec<String>,
    affected_domains: Vec<UnsupportedDomain>,
    conservative_action: ConservativeAction,
}

impl UnsupportedDraft {
    fn new<S>(input: UnsupportedDraftInput<S>) -> Self
    where
        S: Into<String>,
    {
        Self {
            id: input.id,
            body: input.body,
            operation: input.operation,
            language: input.language,
            file_key: input.file_key,
            file: input.file,
            span: input.span,
            construct: input.construct,
            source_evidence: input.source_evidence.into().trim().to_string(),
            affected_place_keys: input.affected_place_keys,
            affected_domains: input.affected_domains,
            conservative_action: input.conservative_action,
        }
    }

    fn to_fact(&self, place_ids: &BTreeMap<String, PlaceId>) -> UnsupportedSemanticFact {
        UnsupportedSemanticFact {
            id: self.id,
            body: self.body,
            operation: self.operation,
            language: self.language,
            file: self.file,
            span: self.span.clone(),
            construct: self.construct.clone(),
            source_evidence: self.source_evidence.clone(),
            affected_places: {
                let affected_places = self
                    .affected_place_keys
                    .iter()
                    .map(|key| place_ids.get(key).copied())
                    .collect::<Option<Vec<_>>>();
                debug_assert!(
                    affected_places.is_some(),
                    "unsupported semantic affected place key missing from place table"
                );
                affected_places.unwrap_or_default()
            },
            affected_domains: self.affected_domains.clone(),
            conservative_action: self.conservative_action,
            precision: UnsupportedPrecision::Unsupported,
            status: MirStatus::Unsupported,
            stable_key: unsupported_stable_key(self),
        }
    }
}

fn unsupported_domains_for(construct: &str) -> Vec<UnsupportedDomain> {
    match construct {
        "eval" | "with" | "Proxy" | "dynamic CommonJS require" => vec![
            UnsupportedDomain::Mir,
            UnsupportedDomain::Calls,
            UnsupportedDomain::Domains,
            UnsupportedDomain::Aliases,
            UnsupportedDomain::DataFlow,
        ],
        "dynamic property key"
        | "optional chaining"
        | "await"
        | "yield"
        | "async rejection path"
        | "switch"
        | "try"
        | "throw"
        | "for initializer"
        | "for left binding"
        | "for-in"
        | "for-of"
        | "for await"
        | "do while"
        | "break"
        | "continue"
        | "labeled statement"
        | "debugger"
        | "catch destructuring"
        | "getter"
        | "setter"
        | "complex destructuring"
        | "spread"
        | "rest"
        | "JSX callback scheduling"
        | "tagged template"
        | "dynamic import"
        | "private field"
        | "private in"
        | "function expression"
        | "class expression"
        | "v8 intrinsic"
        | "unhandled expression" => vec![
            UnsupportedDomain::Mir,
            UnsupportedDomain::Cfg,
            UnsupportedDomain::Domains,
            UnsupportedDomain::DataFlow,
        ],
        "parser recovery" => vec![UnsupportedDomain::Mir, UnsupportedDomain::Cfg],
        _ => vec![UnsupportedDomain::Mir],
    }
}

fn operation_stable_key(
    body_stable_key: &str,
    ordinal: u32,
    span: &Span,
    kind: &OperationKindDraft,
) -> String {
    let mut parts = vec![
        ("language", "ts-js".to_string()),
        ("body", body_stable_key.to_string()),
        ("ordinal", ordinal.to_string()),
        ("kind", kind.label().to_string()),
        ("start_byte", span.start_byte.to_string()),
        ("end_byte", span.end_byte.to_string()),
    ];
    for (index, key) in kind.place_keys().into_iter().enumerate() {
        parts.push((operation_place_label(index), key));
    }
    let borrowed = parts
        .iter()
        .map(|(label, value)| (*label, value.clone()))
        .collect::<Vec<_>>();
    semantic_stable_key(FactFamily::MirOperation, &borrowed).into_string()
}

fn operation_place_label(index: usize) -> &'static str {
    match index {
        0 => "place_000000",
        1 => "place_000001",
        2 => "place_000002",
        3 => "place_000003",
        _ => "place_extra",
    }
}

fn unsupported_stable_key(draft: &UnsupportedDraft) -> String {
    semantic_stable_key(
        FactFamily::UnsupportedSemantic,
        &[
            ("language", language_label(draft.language).to_string()),
            ("file", draft.file_key.clone()),
            ("construct", draft.construct.clone()),
            ("start_byte", draft.span.start_byte.to_string()),
            ("end_byte", draft.span.end_byte.to_string()),
            ("evidence", draft.source_evidence.clone()),
        ],
    )
    .into_string()
}

fn index_projection(expression: &Expression<'_>) -> PlaceProjection {
    match expression {
        Expression::StringLiteral(literal) => {
            PlaceProjection::IndexKnown(literal.value.to_string())
        }
        Expression::NumericLiteral(literal) => PlaceProjection::IndexKnown(
            literal
                .raw
                .as_ref()
                .map_or_else(|| literal.value.to_string(), ToString::to_string),
        ),
        _ => PlaceProjection::IndexUnknown {
            evidence: expression_text(expression).unwrap_or_else(|| "dynamic".to_string()),
        },
    }
}

fn expression_text(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
    }
}

fn callee_text(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => {
            let object = callee_text(&member.object)?;
            Some(format!("{}.{}", object, member.property.name))
        }
        Expression::ArrowFunctionExpression(function) => Some(anonymous_callable_name(
            function.span.start,
            function.span.end,
        )),
        Expression::FunctionExpression(function) => Some(anonymous_callable_name(
            function.span.start,
            function.span.end,
        )),
        Expression::ParenthesizedExpression(parenthesized) => {
            callee_text(&parenthesized.expression)
        }
        _ => None,
    }
}

fn source_text(source: &str, span: oxc_span::Span) -> Option<&str> {
    source.get(span.start as usize..span.end as usize)
}

fn matching_function<'db>(
    db: &'db AnalysisDb,
    file: FileId,
    language: Language,
    name: &str,
    span: &Span,
) -> Option<&'db FunctionFact> {
    db.functions().iter().find(|function| {
        function.file == file
            && function.language == language
            && function.name == name
            && span_contains(span, &function.span)
    })
}

fn matching_module_function(
    db: &AnalysisDb,
    file: FileId,
    language: Language,
) -> Option<&FunctionFact> {
    db.functions().iter().find(|function| {
        function.file == file
            && function.language == language
            && is_synthetic_ts_js_module_function(function)
    })
}

fn span_contains(outer: &Span, inner: &Span) -> bool {
    outer.start_byte <= inner.start_byte && outer.end_byte >= inner.end_byte
}

fn owner_stable_key(file: &SourceFile, function: &FunctionFact) -> String {
    semantic_stable_key(
        FactFamily::Function,
        &[
            ("language", language_label(file.language).to_string()),
            ("path", file.relative_path.clone()),
            ("function", function.name.clone()),
            ("start_byte", function.span.start_byte.to_string()),
            ("end_byte", function.span.end_byte.to_string()),
        ],
    )
    .into_string()
}

fn span_from_oxc(file: FileId, source: &str, span: oxc_span::Span) -> Span {
    crate::core::span_from_byte_range(file, source, span.start as usize, span.end as usize)
}

fn call_site_for_span(span: oxc_span::Span) -> CallSiteId {
    CallSiteId(((span.start as u64) << 32) | span.end as u64)
}

fn parse_source_type(path: &Path) -> SourceType {
    SourceType::from_path(path).unwrap_or_default()
}

fn language_label(language: Language) -> &'static str {
    match language {
        Language::TypeScript => "typescript",
        Language::Tsx => "tsx",
        Language::JavaScript => "javascript",
        Language::Jsx => "jsx",
        Language::Go | Language::Unknown => "unknown",
    }
}

#[cfg(test)]
mod places {
    use super::*;
    use crate::analysis::places::{PlaceProjection, PlaceRoot};
    use crate::core::{AnalysisDb, Language, TS_JS_MODULE_FUNCTION_NAME};
    use std::path::PathBuf;

    fn lower(path: &str, source: &str) -> MirOutput {
        let mut db = AnalysisDb::new();
        db.add_file(PathBuf::from(path), path.to_string(), source.to_string());
        let diagnostics = crate::ts::analyze(&mut db);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        lower_ts_mir(&db)
    }

    #[test]
    fn ts_function_places_include_parameters_locals_globals_properties_and_indexes() {
        let source = r#"
export function render(user, index) {
  const token = user.tokens[index];
  window.value = token;
  return token;
}
"#;
        let first = lower("src/render.ts", source);
        let second = lower("src/render.ts", source);

        assert_eq!(first.bodies.len(), 2);
        assert!(
            first
                .bodies
                .iter()
                .any(|body| body.owner_stable_key.contains(TS_JS_MODULE_FUNCTION_NAME))
        );
        assert!(
            first
                .bodies
                .iter()
                .any(|body| body.owner_stable_key.contains("render"))
        );
        assert_eq!(
            first
                .places
                .iter()
                .map(|place| place.stable_key.as_str())
                .collect::<Vec<_>>(),
            second
                .places
                .iter()
                .map(|place| place.stable_key.as_str())
                .collect::<Vec<_>>()
        );
        assert!(first.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Parameter {
                index: 0,
                name: Some(name),
                ..
            } if name == "user"
        )));
        assert!(first.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Parameter {
                index: 1,
                name: Some(name),
                ..
            } if name == "index"
        )));
        assert!(first.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Local { name, .. } if name == "token"
        )));
        assert!(first.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Unknown { evidence } if evidence == "window"
        )));
        assert!(first.places.iter().any(|place| {
            matches!(&place.root, PlaceRoot::Parameter { name: Some(name), .. } if name == "user")
                && place
                    .projections
                    .contains(&PlaceProjection::Property("tokens".to_string()))
                && place.projections.iter().any(|projection| {
                    matches!(projection, PlaceProjection::IndexUnknown { evidence } if evidence == "index")
                })
        }));
    }

    #[test]
    fn ts_arrow_functions_and_class_methods_join_existing_function_facts() {
        let source = r#"
const render = (user) => user.name;

class View {
  render(user) {
    return user.name;
  }
}
"#;
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from("src/view.tsx"),
            "src/view.tsx".to_string(),
            source.to_string(),
        );
        let diagnostics = crate::ts::analyze(&mut db);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let arrow = db
            .functions()
            .iter()
            .find(|function| function.name == "render")
            .expect("arrow function fact should use variable name");
        let method = db
            .functions()
            .iter()
            .find(|function| function.name == "View.render")
            .expect("method function fact should use class-qualified name");

        let output = lower_ts_mir(&db);
        assert!(output.bodies.iter().any(|body| {
            body.function == arrow.id
                && body.language == Language::Tsx
                && body.owner_stable_key.contains("render")
        }));
        assert!(output.bodies.iter().any(|body| {
            body.function == method.id
                && body.language == Language::Tsx
                && body.owner_stable_key.contains("View.render")
        }));
        assert!(output.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Parameter {
                function,
                index: 0,
                name: Some(name),
            } if *function == arrow.id && name == "user"
        )));
        assert!(output.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Parameter {
                function,
                index: 0,
                name: Some(name),
            } if *function == method.id && name == "user"
        )));
    }

    #[test]
    fn ts_mir_place_rows_do_not_carry_oxc_ast_debug_evidence() {
        let output = lower(
            "src/render.ts",
            r#"
export function render(user) {
  const token = user.token;
  return token;
}
"#,
        );
        let debug = format!("{output:#?}");

        assert!(!debug.contains("oxc_ast"));
        assert!(!debug.contains("oxc_span::Span"));
        assert!(!debug.contains("Program<'_"));
        assert!(!debug.contains("Expression<'_"));
        assert!(!debug.contains("Statement<'_"));
        assert!(!debug.contains("FunctionDeclaration"));
        assert!(!debug.contains("ArrowFunctionExpression"));
        assert!(!debug.contains("ClassElement"));
    }
}

#[cfg(test)]
mod operations {
    use super::*;
    use crate::analysis::mir::op::{
        AssignMode, ConservativeAction, MirOperationKind, UnsupportedDomain,
    };
    use crate::core::TS_JS_MODULE_FUNCTION_NAME;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    fn lower(path: &str, source: &str) -> MirOutput {
        let mut db = AnalysisDb::new();
        db.add_file(PathBuf::from(path), path.to_string(), source.to_string());
        let diagnostics = crate::ts::analyze(&mut db);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        lower_ts_mir(&db)
    }

    #[test]
    fn ts_statement_lowering_emits_assignment_modes_and_control_shapes() {
        let output = lower(
            "src/flow.ts",
            r#"
export function flow(user, index, enabled) {
  const token = user.tokens[index];
  let count = 0;
  count = index;
  user.tokens[index] = token;
  ({ token } = user);
  if (enabled && (token ?? false)) { count = count + 1; }
  const label = enabled ? token : "none";
  for (; count < 10; count++) { label; }
  return token;
}
"#,
        );

        let modes = output
            .operations
            .iter()
            .filter_map(|operation| match operation.kind {
                MirOperationKind::Assign { mode, .. } => Some(mode),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(modes.contains(&AssignMode::DeclarationBinding));
        assert!(modes.contains(&AssignMode::Overwrite));
        assert!(modes.contains(&AssignMode::ProjectionMutation));
        assert!(
            output
                .operations
                .iter()
                .any(|operation| matches!(operation.kind, MirOperationKind::Branch { .. }))
        );
        assert!(
            output
                .operations
                .iter()
                .any(|operation| matches!(operation.kind, MirOperationKind::Return { .. }))
        );
        assert!(
            output
                .unsupported
                .iter()
                .any(|row| row.construct.contains("destructur") && row.is_complete())
        );
    }

    #[test]
    fn ts_call_operations_are_shape_evidence_with_deterministic_call_sites() {
        let source = r#"
export function flow(token, count) {
  const result = helper(token, count);
  return result;
}
"#;
        let first = lower("src/calls.ts", source);
        let second = lower("src/calls.ts", source);

        let first_calls = first
            .operations
            .iter()
            .filter_map(|operation| match &operation.kind {
                MirOperationKind::Call {
                    site,
                    callee,
                    arguments,
                    return_place,
                } => Some((
                    operation.stable_key.clone(),
                    *site,
                    callee,
                    arguments.len(),
                    *return_place,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();
        let second_calls = second
            .operations
            .iter()
            .filter_map(|operation| match &operation.kind {
                MirOperationKind::Call {
                    site,
                    arguments,
                    return_place,
                    ..
                } => Some((
                    operation.stable_key.clone(),
                    *site,
                    arguments.len(),
                    *return_place,
                )),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(first_calls.len(), 1);
        assert_eq!(
            first_calls
                .iter()
                .map(|(key, site, _, arguments, return_place)| {
                    (key.clone(), *site, *arguments, *return_place)
                })
                .collect::<Vec<_>>(),
            second_calls
        );
        assert!(
            first_calls[0].2
                != &MirValue::Unknown {
                    evidence: "direct target".to_string()
                }
        );
    }

    #[test]
    fn ts_nested_same_start_calls_get_distinct_call_site_ids() {
        let source = r#"
const k1 = {
  a2() {},
  a4() { return this; },
};
k1.a4().a2();
"#;
        let output = lower("src/chained.js", source);
        let chain_start = source.find("k1.a4()").expect("chained call source exists") as u32;
        let same_start_calls = output
            .operations
            .iter()
            .filter_map(|operation| match &operation.kind {
                MirOperationKind::Call { site, .. } if operation.span.start_byte == chain_start => {
                    Some((*site, operation.span.end_byte))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let unique_sites = same_start_calls
            .iter()
            .map(|(site, _)| *site)
            .collect::<BTreeSet<_>>();

        assert_eq!(same_start_calls.len(), 2);
        assert_eq!(unique_sites.len(), 2);
    }

    #[test]
    fn ts_call_operation_spans_match_jelly_parenthesized_call_shapes() {
        let source = r#"
(function(){})();
((f))();
(f());
((new f()));
function f() {}
"#;
        let output = lower("src/call_shapes.js", source);
        let span_texts = output
            .operations
            .iter()
            .filter_map(|operation| match operation.kind {
                MirOperationKind::Call { .. } => Some(
                    &source[operation.span.start_byte as usize..operation.span.end_byte as usize],
                ),
                _ => None,
            })
            .collect::<BTreeSet<_>>();

        for expected in ["function(){})()", "(f))()", "(f())", "(new f())"] {
            assert!(span_texts.contains(expected), "missing span {expected:?}");
        }
    }

    #[test]
    fn ts_module_body_lowering_emits_top_level_calls() {
        let output = lower(
            "src/main.ts",
            r#"
function boot() {
  return 1;
}

boot();
"#,
        );
        let module = output
            .bodies
            .iter()
            .find(|body| body.owner_stable_key.contains(TS_JS_MODULE_FUNCTION_NAME))
            .expect("module body should be lowered");
        let module_boot_calls = output
            .operations
            .iter()
            .filter(|operation| operation.body == module.id)
            .filter(|operation| {
                matches!(
                    &operation.kind,
                    MirOperationKind::Call {
                        callee: MirValue::Unknown { evidence },
                        ..
                    } if evidence == "boot"
                )
            })
            .count();

        assert_eq!(module_boot_calls, 1);
    }

    #[test]
    fn ts_new_expression_lowers_constructor_call() {
        let output = lower(
            "src/new.ts",
            r#"
function Widget(value) {
  this.value = value;
}

new Widget(1);
"#,
        );

        let constructor_calls = output
            .operations
            .iter()
            .filter(|operation| {
                matches!(
                    &operation.kind,
                    MirOperationKind::Call {
                        callee: MirValue::Unknown { evidence },
                        arguments,
                        ..
                    } if evidence == "new Widget" && arguments.len() == 1
                )
            })
            .count();

        assert_eq!(constructor_calls, 1);
    }

    #[test]
    fn ts_iife_callees_use_anonymous_callable_identity() {
        let output = lower(
            "src/iife.ts",
            r#"
(function(value) {
  return value;
})(1);

(() => helper())();
"#,
        );

        let anonymous_bodies = output
            .bodies
            .iter()
            .filter(|body| body.owner_stable_key.contains("<polint:anonymous:"))
            .count();
        let anonymous_calls = output
            .operations
            .iter()
            .filter(|operation| {
                matches!(
                    &operation.kind,
                    MirOperationKind::Call {
                        callee: MirValue::Unknown { evidence },
                        ..
                    } if evidence.starts_with("<polint:anonymous:")
                )
            })
            .count();

        assert_eq!(anonymous_bodies, 2);
        assert_eq!(anonymous_calls, 2);
    }

    #[test]
    fn ts_unsupported_semantics_are_structured_and_conservative() {
        let output = lower(
            "src/dynamic.tsx",
            r#"
export async function flow(obj, key, promise, value) {
  eval("value");
  with (obj) { value = key; }
  const proxy = new Proxy(obj, {});
  const dynamic = obj[key];
  const maybe = obj?.value;
  await promise;
  const { nested: { name } } = obj;
  const clone = { ...obj };
  const mod = require(key);
  return <Button onClick={() => helper(value)} />;
}
"#,
        );

        for construct in [
            "eval",
            "with",
            "Proxy",
            "dynamic property key",
            "optional chaining",
            "await",
            "complex destructuring",
            "spread",
            "dynamic CommonJS require",
            "JSX callback scheduling",
        ] {
            let row = output
                .unsupported
                .iter()
                .find(|row| row.construct == construct)
                .unwrap_or_else(|| panic!("missing unsupported row: {construct}"));
            assert!(row.is_complete());
            assert!(row.affected_domains.contains(&UnsupportedDomain::Mir));
            assert!(matches!(
                row.conservative_action,
                ConservativeAction::HavocAffectedPlaces | ConservativeAction::StopLowering
            ));
        }
    }

    #[test]
    fn ts_nested_expression_lowering_preserves_calls_and_argument_places() {
        let output = lower(
            "src/nested.ts",
            r#"
export function nested(a, b, tag) {
  const computed = a + helper(b);
  const templated = `${computed}:${helper(a)}`;
  const tagged = tag`${helper(b)}`;
  const inverted = !helper(computed);
  const sequenced = (helper(a), helper(b));
  const invoked = api[helper(tag)](helper(a));
  return templated;
}
"#,
        );

        let helper_calls = output
            .operations
            .iter()
            .filter_map(|operation| match &operation.kind {
                MirOperationKind::Call {
                    callee: MirValue::Unknown { evidence },
                    arguments,
                    ..
                } if evidence == "helper" => Some(arguments.len()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(helper_calls.len(), 8);
        assert!(
            helper_calls
                .iter()
                .all(|argument_count| *argument_count == 1)
        );
        assert!(output.unsupported.iter().any(|row| {
            row.construct == "tagged template"
                && row.is_complete()
                && row.affected_domains.contains(&UnsupportedDomain::Mir)
        }));
    }

    #[test]
    fn ts_control_statement_edges_are_explicit_and_still_lower_nested_calls() {
        let output = lower(
            "src/control.ts",
            r#"
export function control(kind, items, errors) {
  try {
    helper(kind);
  } catch (err) {
    recover(err);
  } finally {
    cleanup();
  }

  switch (kind) {
    case "a":
      helper(items[0]);
      break;
    default:
      fallback();
  }

  for (const item of items) {
    helper(item);
  }

  for (const key in errors) {
    helper(errors[key]);
    continue;
  }

  do {
    helper(kind);
  } while (kind);

  throw new Error(kind);
}
"#,
        );

        for construct in [
            "try", "switch", "break", "for-of", "for-in", "continue", "do while", "throw",
        ] {
            let row = output
                .unsupported
                .iter()
                .find(|row| row.construct == construct)
                .unwrap_or_else(|| panic!("missing unsupported row: {construct}"));
            assert!(row.is_complete());
            assert!(row.affected_domains.contains(&UnsupportedDomain::Mir));
        }

        let calls = output
            .operations
            .iter()
            .filter(|operation| matches!(operation.kind, MirOperationKind::Call { .. }))
            .count();

        assert!(
            calls >= 7,
            "expected nested calls to remain visible: {output:#?}"
        );
    }

    #[test]
    fn ts_for_initializer_expression_is_explicitly_unsupported_not_silent() {
        let output = lower(
            "src/for-init.ts",
            r#"
export function loop(start, test, update) {
  for (helper(start); helper(test); helper(update)) {
    start;
  }
}
"#,
        );

        let row = output
            .unsupported
            .iter()
            .find(|row| row.construct == "for initializer")
            .expect("for initializer expression should be explicit unsupported evidence");

        assert!(row.is_complete());
        let helper_calls = output
            .operations
            .iter()
            .filter(|operation| {
                matches!(
                    &operation.kind,
                    MirOperationKind::Call {
                        callee: MirValue::Unknown { evidence },
                        ..
                    } if evidence == "helper"
                )
            })
            .count();
        assert_eq!(helper_calls, 3);
    }

    #[test]
    fn ts_for_in_and_for_of_existing_targets_record_left_writes() {
        let output = lower(
            "src/for-left.ts",
            r#"
export function loop(target, index, items, map) {
  for (target[index] of items) {
    helper(target[index]);
  }
  for (target.value in map) {
    helper(target.value);
  }
}
"#,
        );

        let projection_mutations = output
            .operations
            .iter()
            .filter(|operation| {
                matches!(
                    operation.kind,
                    MirOperationKind::Assign {
                        mode: AssignMode::ProjectionMutation,
                        ..
                    }
                )
            })
            .count();

        assert!(projection_mutations >= 2, "{output:#?}");
        for construct in ["for-of", "for-in", "for left binding"] {
            let row = output
                .unsupported
                .iter()
                .find(|row| row.construct == construct)
                .unwrap_or_else(|| panic!("missing unsupported row: {construct}"));
            assert!(row.is_complete());
            assert!(row.affected_domains.contains(&UnsupportedDomain::Mir));
        }
    }

    #[test]
    fn ts_array_object_and_jsx_expression_children_preserve_nested_calls() {
        let output = lower(
            "src/nested-values.tsx",
            r#"
export function view(a, b, items) {
  const data = [helper(a), ...items, { [helper(items)]: helper(b) }];
  return <Panel data={helper(data)}>{helper(a)}</Panel>;
}
"#,
        );

        let helper_calls = output
            .operations
            .iter()
            .filter(|operation| {
                matches!(
                    &operation.kind,
                    MirOperationKind::Call {
                        callee: MirValue::Unknown { evidence },
                        ..
                    } if evidence == "helper"
                )
            })
            .count();

        assert_eq!(helper_calls, 5);
        assert!(
            output
                .unsupported
                .iter()
                .any(|row| row.construct == "spread" && row.is_complete())
        );
    }

    #[test]
    fn ts_optional_private_and_dynamic_import_edges_are_explicit() {
        let output = lower(
            "src/private.tsx",
            r#"
class Box {
  #value;

  read(obj, modName) {
    const optional = obj?.value?.();
    const loaded = import(modName);
    obj.value! = modName;
    this.#value = obj;
    this.#value++;
    const node = <Panel data={helper(obj)}>{helper(modName)}</Panel>;
    return this.#value;
  }
}
"#,
        );

        for construct in ["optional chaining", "dynamic import", "private field"] {
            let row = output
                .unsupported
                .iter()
                .find(|row| row.construct == construct)
                .unwrap_or_else(|| panic!("missing unsupported row: {construct}"));
            assert!(row.is_complete());
        }

        let helper_calls = output
            .operations
            .iter()
            .filter(|operation| {
                matches!(
                    &operation.kind,
                    MirOperationKind::Call {
                        callee: MirValue::Unknown { evidence },
                        ..
                    } if evidence == "helper"
                )
            })
            .count();

        assert_eq!(helper_calls, 2);
        assert!(output.operations.iter().any(|operation| {
            matches!(
                operation.kind,
                MirOperationKind::Assign {
                    mode: AssignMode::ProjectionMutation,
                    ..
                }
            )
        }));
    }

    #[test]
    fn ts_optional_chaining_emits_unique_mir_and_unsupported_keys() {
        let output = lower(
            "src/optional.ts",
            r#"
export function flow(options, data) {
  if (options?.enabled) {
    return data.model?.display_name;
  }
}
"#,
        );

        let operation_keys = output
            .operations
            .iter()
            .map(|operation| operation.stable_key.as_str())
            .collect::<BTreeSet<_>>();
        let unsupported_keys = output
            .unsupported
            .iter()
            .map(|row| row.stable_key.as_str())
            .collect::<BTreeSet<_>>();

        assert_eq!(operation_keys.len(), output.operations.len());
        assert_eq!(unsupported_keys.len(), output.unsupported.len());
    }
}
