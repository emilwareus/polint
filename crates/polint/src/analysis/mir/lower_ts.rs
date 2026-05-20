use std::collections::BTreeMap;
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, BindingPattern, Class, ClassElement, Declaration, ExportDefaultDeclarationKind,
    Expression, FormalParameters, Function, FunctionBody, MethodDefinition, ObjectPropertyKind,
    Program, PropertyKey, Statement, VariableDeclarator,
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
use crate::analysis::places::{PlaceProjection, PlaceRoot, PlaceStatus, PlaceTableBuilder};
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
                FunctionLowering::new(file, file.source.as_ref(), function_fact.id, body.id);
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
            });
        }
        Expression::FunctionExpression(function) => {
            if let Some(body) = function.body.as_deref() {
                functions.push(TsFunctionCandidate {
                    name: name.name.to_string(),
                    span: declarator.span,
                    parameters: parameter_names(&function.params),
                    body,
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
    parameters: BTreeMap<String, PlaceRoot>,
    locals: BTreeMap<String, PlaceRoot>,
}

impl<'source> FunctionLowering<'source> {
    fn new(file: &SourceFile, source: &'source str, function: FunctionId, body: MirBodyId) -> Self {
        Self {
            language: file.language,
            file: file.id,
            source,
            function,
            body,
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
                self.lower_expression(
                    &statement.expression,
                    places,
                    operations,
                    unsupported,
                    false,
                );
            }
            Statement::VariableDeclaration(variable) => {
                for declarator in &variable.declarations {
                    self.lower_variable_declarator(declarator, places, operations, unsupported);
                }
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.lower_expression(argument, places, operations, unsupported, false);
                }
            }
            Statement::IfStatement(statement) => {
                self.lower_expression(&statement.test, places, operations, unsupported, false);
                self.lower_statement(&statement.consequent, places, operations, unsupported);
                if let Some(alternate) = &statement.alternate {
                    self.lower_statement(alternate, places, operations, unsupported);
                }
            }
            Statement::WhileStatement(statement) => {
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
                    self.lower_expression(test, places, operations, unsupported, false);
                }
                if let Some(update) = &statement.update {
                    self.lower_expression(update, places, operations, unsupported, false);
                }
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
            return;
        };
        let key = self.insert_local(places, &name);
        if let Some(init) = &declarator.init {
            self.lower_expression(init, places, operations, unsupported, false);
        }
        self.push_assign(
            operations,
            declarator.span,
            key,
            ValueDraft::Unknown {
                evidence: "declaration initializer".to_string(),
            },
            AssignMode::DeclarationBinding,
        );
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
                    self.lower_expression(
                        &assignment.right,
                        places,
                        operations,
                        unsupported,
                        false,
                    );
                    self.push_assign(
                        operations,
                        assignment.span,
                        target.key.clone(),
                        ValueDraft::Unknown {
                            evidence: "assignment value".to_string(),
                        },
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
            Expression::UpdateExpression(update) => self.simple_assignment_target_shape(
                &update.argument,
                places,
                operations,
                unsupported,
            ),
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
            Expression::CallExpression(call) => {
                self.lower_expression(&call.callee, places, operations, unsupported, false);
                for argument in &call.arguments {
                    self.lower_argument(argument, places, operations, unsupported);
                }
                Some(self.temporary_shape(places, call.span))
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
        places.insert(
            self.language,
            Some(self.file),
            Some(self.function),
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
        ordinal: u32,
        span: Span,
        kind: OperationKindDraft,
        status: MirStatus,
    ) -> Self {
        let stable_key = operation_stable_key(body, ordinal, &span, &kind);
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
    file: FileId,
    span: Span,
    construct: String,
    source_evidence: String,
    affected_place_keys: Vec<String>,
    affected_domains: Vec<UnsupportedDomain>,
    conservative_action: ConservativeAction,
}

impl UnsupportedDraft {
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

fn operation_stable_key(
    body: MirBodyId,
    ordinal: u32,
    span: &Span,
    kind: &OperationKindDraft,
) -> String {
    let mut parts = vec![
        ("language", "ts-js".to_string()),
        ("body", body.0.to_string()),
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
            ("file", draft.file.0.to_string()),
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
