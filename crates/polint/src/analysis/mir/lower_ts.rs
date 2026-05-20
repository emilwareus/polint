use std::collections::BTreeMap;
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, BindingPattern, Class, ClassElement, Declaration, ExportDefaultDeclarationKind,
    Expression, FormalParameters, Function, FunctionBody, LogicalOperator, MethodDefinition,
    ObjectPropertyKind, Program, PropertyKey, Statement, VariableDeclarator,
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
    PlaceProjection, PlaceRoot, PlaceStableContext, PlaceStatus, PlaceTableBuilder,
};
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, SourceFile, Span};

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
            self.unsupported.push(UnsupportedDraft::new(
                UnsupportedId(self.unsupported.len() as u64),
                None,
                None,
                file.language,
                file.relative_path.clone(),
                file.id,
                span,
                "parser recovery",
                error.to_string(),
                Vec::new(),
                vec![UnsupportedDomain::Mir, UnsupportedDomain::Cfg],
                ConservativeAction::StopLowering,
            ));
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
                span: declarator.span,
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
                    span: declarator.span,
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

fn method_name(method: &MethodDefinition<'_>) -> Option<String> {
    match &method.key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
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
        for statement in &body.statements {
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
            Statement::ForStatement(statement) => {
                if let Some(init) = &statement.init {
                    if let oxc_ast::ast::ForStatementInit::VariableDeclaration(variable) = init {
                        for declarator in &variable.declarations {
                            self.lower_variable_declarator(
                                declarator,
                                places,
                                operations,
                                unsupported,
                            );
                        }
                    }
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
                    if let oxc_ast::ast::ArrayExpressionElement::SpreadElement(spread) = element {
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
                        call: CallSiteId(call.span.start as u64),
                    },
                    projections: Vec::new(),
                    status: PlaceStatus::Partial,
                    key,
                }),
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
            Expression::ChainExpression(chain) => {
                self.push_unsupported(
                    operations,
                    unsupported,
                    chain.span,
                    "optional chaining",
                    Vec::new(),
                    ConservativeAction::HavocAffectedPlaces,
                );
                None
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
            Expression::NewExpression(new_expression) => {
                if callee_text(&new_expression.callee).as_deref() == Some("Proxy") {
                    self.push_unsupported(
                        operations,
                        unsupported,
                        new_expression.span,
                        "Proxy",
                        Vec::new(),
                        ConservativeAction::HavocAffectedPlaces,
                    );
                }
                self.lower_expression(
                    &new_expression.callee,
                    places,
                    operations,
                    unsupported,
                    false,
                );
                for argument in &new_expression.arguments {
                    self.lower_argument(argument, places, operations, unsupported);
                }
                Some(self.temporary_shape(places, new_expression.span))
            }
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
                None
            }
            _ => None,
        }
    }

    fn lower_argument(
        &mut self,
        argument: &Argument<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
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
                self.lower_expression(&spread.argument, places, operations, unsupported, false);
            }
            _ => {
                let expression = argument.to_expression();
                self.lower_expression(expression, places, operations, unsupported, false);
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
            _ => None,
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
            _ => None,
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
            self.language,
            Some(self.file),
            Some(self.function),
            &self.stable_context,
            root,
            projections,
            status,
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
        let site = CallSiteId(call.span.start as u64);
        let return_key = self.insert_place(
            places,
            PlaceRoot::CallReturn { call: site },
            Vec::new(),
            PlaceStatus::Partial,
        );
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
            call.span,
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
        unsupported.push(UnsupportedDraft::new(
            unsupported_id,
            Some(self.body),
            Some(operation_id),
            self.language,
            self.stable_context.file_key().to_string(),
            self.file,
            span.clone(),
            construct,
            source_evidence,
            affected_place_keys,
            unsupported_domains_for(construct),
            action,
        ));
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
            Self::Branch { predicate } => Some(MirOperationKind::Branch {
                predicate: *predicate,
            }),
            Self::Call {
                site,
                callee,
                arguments,
                return_place_key,
            } => Some(MirOperationKind::Call {
                site: *site,
                callee: callee.to_value(place_ids),
                arguments: arguments
                    .iter()
                    .filter_map(|key| place_ids.get(key).copied())
                    .collect(),
                return_place: *place_ids.get(return_place_key)?,
            }),
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
            Self::Literal { value } => MirValue::Literal {
                value: value.clone(),
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

impl UnsupportedDraft {
    fn new(
        id: UnsupportedId,
        body: Option<MirBodyId>,
        operation: Option<MirOpId>,
        language: Language,
        file_key: impl Into<String>,
        file: FileId,
        span: Span,
        construct: &str,
        source_evidence: impl Into<String>,
        affected_place_keys: Vec<String>,
        affected_domains: Vec<UnsupportedDomain>,
        conservative_action: ConservativeAction,
    ) -> Self {
        Self {
            id,
            body,
            operation,
            language,
            file_key: file_key.into(),
            file,
            span,
            construct: construct.to_string(),
            source_evidence: source_evidence.into().trim().to_string(),
            affected_place_keys,
            affected_domains,
            conservative_action,
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
            affected_places: self
                .affected_place_keys
                .iter()
                .filter_map(|key| place_ids.get(key).copied())
                .collect(),
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
        | "getter"
        | "setter"
        | "complex destructuring"
        | "spread"
        | "rest"
        | "JSX callback scheduling" => vec![
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
    use crate::core::{AnalysisDb, FunctionId, Language};
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

        assert_eq!(first.bodies.len(), 1);
        assert!(first.bodies[0].stable_key.contains("render"));
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
            } if *function == FunctionId(0) && name == "user"
        )));
        assert!(output.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Parameter {
                function,
                index: 0,
                name: Some(name),
            } if *function == FunctionId(1) && name == "user"
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
}
