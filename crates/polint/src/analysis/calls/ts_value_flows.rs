use std::collections::BTreeMap;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, AssignmentTarget, BindingPattern, CallExpression, Class,
    ClassElement, Expression, ForStatementLeft, FunctionBody, MethodDefinition,
    MethodDefinitionKind, ObjectPropertyKind, Program, PropertyKey, Statement, VariableDeclaration,
    VariableDeclarator,
};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};

use crate::analysis::calls::facts::{
    CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
    CallTargetFact, CallTargetStatus,
};
use crate::analysis::ids::CallTargetId;
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::FactFamily;
use crate::core::{
    AnalysisDb, FileId, FunctionFact, FunctionId, SourceFile, TS_JS_MODULE_FUNCTION_NAME,
};

pub(crate) fn resolve_ts_value_flow_targets(
    db: &AnalysisDb,
    sites: &[CallSiteFact],
    id_offset: u64,
) -> Vec<CallTargetFact> {
    let mut rows = Vec::new();
    let mut next_id = id_offset;
    let sites_by_file = sites_by_file(sites);
    let functions_by_file_start = functions_by_file_start(db);

    for file in db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
    {
        let Some(file_sites) = sites_by_file.get(&file.id) else {
            continue;
        };
        let Some(module) = module_function(db, file) else {
            continue;
        };
        let allocator = Allocator::default();
        let parsed = Parser::new(
            &allocator,
            file.source.as_ref(),
            SourceType::from_path(&file.path).unwrap_or_default(),
        )
        .parse();
        if parsed.panicked && parsed.program.body.is_empty() {
            continue;
        }
        let mut collector = TsValueFlowCollector {
            db,
            file,
            module,
            sites: file_sites,
            functions_by_start: &functions_by_file_start,
            function_declarations: BTreeMap::new(),
            function_flows_by_id: BTreeMap::new(),
            classes: BTreeMap::new(),
            rows: Vec::new(),
            next_id,
        };
        collector.collect_program(&parsed.program);
        next_id += collector.rows.len() as u64;
        rows.extend(collector.rows);
    }

    rows.sort_by(|left, right| {
        (left.stable_key.as_str(), left.site, left.target_function).cmp(&(
            right.stable_key.as_str(),
            right.site,
            right.target_function,
        ))
    });
    rows.dedup_by(|left, right| left.stable_key == right.stable_key);
    for (index, row) in rows.iter_mut().enumerate() {
        row.id = CallTargetId(id_offset + index as u64);
    }
    rows
}

struct TsValueFlowCollector<'db, 'ast> {
    db: &'db AnalysisDb,
    file: &'db SourceFile,
    module: &'db FunctionFact,
    sites: &'db [&'db CallSiteFact],
    functions_by_start: &'db BTreeMap<(FileId, u32), Vec<&'db FunctionFact>>,
    function_declarations: BTreeMap<String, FunctionFlow<'db, 'ast>>,
    function_flows_by_id: BTreeMap<FunctionId, FunctionFlow<'db, 'ast>>,
    classes: BTreeMap<String, ClassTargets>,
    rows: Vec<CallTargetFact>,
    next_id: u64,
}

#[derive(Clone)]
struct FunctionFlow<'db, 'ast> {
    function: &'db FunctionFact,
    body: &'ast FunctionBody<'ast>,
    params: Vec<ParamPattern>,
    rest: Option<ParamPattern>,
}

#[derive(Clone, Debug)]
enum ParamPattern {
    Binding(String),
    Array {
        elements: Vec<Option<ParamPattern>>,
        rest: Option<Box<ParamPattern>>,
    },
    Object {
        properties: Vec<(String, ParamPattern)>,
        rest: Option<Box<ParamPattern>>,
    },
}

impl<'db, 'ast> TsValueFlowCollector<'db, 'ast> {
    fn collect_program(&mut self, program: &'ast Program<'ast>) {
        self.collect_expression_function_flows(program);
        self.collect_function_declarations(program);
        self.collect_class_declarations(program);
        let mut env = FlowEnv::default();
        for name in self.classes.keys() {
            if let Some(targets) = self.class_static_targets(name) {
                env.objects.insert(name.clone(), targets);
            }
        }
        for statement in &program.body {
            self.collect_statement(statement, self.module, &mut env);
        }
    }

    fn collect_expression_function_flows(&mut self, program: &'ast Program<'ast>) {
        for statement in &program.body {
            self.collect_expression_function_flows_from_statement(statement);
        }
    }

    fn collect_expression_function_flows_from_statement(
        &mut self,
        statement: &'ast Statement<'ast>,
    ) {
        match statement {
            Statement::VariableDeclaration(variable) => {
                for declarator in &variable.declarations {
                    if let Some(init) = &declarator.init {
                        self.collect_expression_function_flows_from_expression(init);
                    }
                }
            }
            Statement::ExpressionStatement(statement) => {
                self.collect_expression_function_flows_from_expression(&statement.expression);
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.collect_expression_function_flows_from_expression(argument);
                }
            }
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.collect_expression_function_flows_from_statement(statement);
                }
            }
            Statement::IfStatement(statement) => {
                self.collect_expression_function_flows_from_expression(&statement.test);
                self.collect_expression_function_flows_from_statement(&statement.consequent);
                if let Some(alternate) = &statement.alternate {
                    self.collect_expression_function_flows_from_statement(alternate);
                }
            }
            Statement::ForOfStatement(statement) => {
                self.collect_expression_function_flows_from_expression(&statement.right);
                self.collect_expression_function_flows_from_statement(&statement.body);
            }
            Statement::FunctionDeclaration(function) => {
                if let Some(body) = function.body.as_deref() {
                    for statement in &body.statements {
                        self.collect_expression_function_flows_from_statement(statement);
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_expression_function_flows_from_expression(
        &mut self,
        expression: &'ast Expression<'ast>,
    ) {
        match expression {
            Expression::FunctionExpression(function) => {
                if let Some(body) = function.body.as_deref()
                    && let Some(function_fact) = self.function_for_expression(expression)
                {
                    let flow = FunctionFlow {
                        function: function_fact,
                        body,
                        params: function
                            .params
                            .items
                            .iter()
                            .map(|param| param_pattern_from_binding(&param.pattern))
                            .collect(),
                        rest: function
                            .params
                            .rest
                            .as_ref()
                            .map(|rest| param_pattern_from_binding(&rest.rest.argument)),
                    };
                    self.function_flows_by_id.insert(function_fact.id, flow);
                    for statement in &body.statements {
                        self.collect_expression_function_flows_from_statement(statement);
                    }
                }
            }
            Expression::ArrowFunctionExpression(function) => {
                if let Some(function_fact) = self.function_for_expression(expression) {
                    let flow = FunctionFlow {
                        function: function_fact,
                        body: &function.body,
                        params: function
                            .params
                            .items
                            .iter()
                            .map(|param| param_pattern_from_binding(&param.pattern))
                            .collect(),
                        rest: function
                            .params
                            .rest
                            .as_ref()
                            .map(|rest| param_pattern_from_binding(&rest.rest.argument)),
                    };
                    self.function_flows_by_id.insert(function_fact.id, flow);
                }
                if let Some(expression) = function.get_expression() {
                    self.collect_expression_function_flows_from_expression(expression);
                } else {
                    for statement in &function.body.statements {
                        self.collect_expression_function_flows_from_statement(statement);
                    }
                }
            }
            Expression::CallExpression(call) => {
                self.collect_expression_function_flows_from_expression(&call.callee);
                for argument in &call.arguments {
                    if let Some(argument) = argument_expression(argument) {
                        self.collect_expression_function_flows_from_expression(argument);
                    }
                }
            }
            Expression::NewExpression(expression) => {
                self.collect_expression_function_flows_from_expression(&expression.callee);
                for argument in &expression.arguments {
                    if let Some(argument) = argument_expression(argument) {
                        self.collect_expression_function_flows_from_expression(argument);
                    }
                }
            }
            Expression::AssignmentExpression(assignment) => {
                self.collect_expression_function_flows_from_expression(&assignment.right);
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    if let Some(expression) = array_element_expression(element) {
                        self.collect_expression_function_flows_from_expression(expression);
                    }
                }
            }
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    match property {
                        ObjectPropertyKind::ObjectProperty(property) => {
                            self.collect_expression_function_flows_from_expression(&property.value);
                        }
                        ObjectPropertyKind::SpreadProperty(spread) => {
                            self.collect_expression_function_flows_from_expression(
                                &spread.argument,
                            );
                        }
                    }
                }
            }
            Expression::StaticMemberExpression(member) => {
                self.collect_expression_function_flows_from_expression(&member.object);
            }
            Expression::ComputedMemberExpression(member) => {
                self.collect_expression_function_flows_from_expression(&member.object);
                self.collect_expression_function_flows_from_expression(&member.expression);
            }
            Expression::AwaitExpression(expression) => {
                self.collect_expression_function_flows_from_expression(&expression.argument);
            }
            Expression::YieldExpression(expression) => {
                if let Some(argument) = &expression.argument {
                    self.collect_expression_function_flows_from_expression(argument);
                }
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.collect_expression_function_flows_from_expression(expression);
                }
            }
            Expression::ParenthesizedExpression(expression) => {
                self.collect_expression_function_flows_from_expression(&expression.expression);
            }
            _ => {}
        }
    }

    fn collect_function_declarations(&mut self, program: &'ast Program<'ast>) {
        for statement in &program.body {
            let Statement::FunctionDeclaration(function) = statement else {
                continue;
            };
            let Some(name) = function.id.as_ref().map(|id| id.name.to_string()) else {
                continue;
            };
            let Some(body) = function.body.as_deref() else {
                continue;
            };
            if let Some(function_fact) = self.function_for_span(function.span) {
                let flow = FunctionFlow {
                    function: function_fact,
                    body,
                    params: function
                        .params
                        .items
                        .iter()
                        .map(|param| param_pattern_from_binding(&param.pattern))
                        .collect(),
                    rest: function
                        .params
                        .rest
                        .as_ref()
                        .map(|rest| param_pattern_from_binding(&rest.rest.argument)),
                };
                self.function_flows_by_id
                    .insert(function_fact.id, flow.clone());
                self.function_declarations.insert(name.clone(), flow);
            }
            let mut targets = ClassTargets::default();
            self.collect_constructor_assignments_from_statements(
                &body.statements,
                &function
                    .params
                    .items
                    .iter()
                    .map(|param| param_pattern_from_binding(&param.pattern))
                    .collect::<Vec<_>>(),
                &mut targets,
            );
            if !targets.is_empty() {
                self.classes.insert(name, targets);
            }
        }
    }

    fn collect_class_declarations(&mut self, program: &'ast Program<'ast>) {
        for statement in &program.body {
            let Statement::ClassDeclaration(class) = statement else {
                continue;
            };
            let Some(name) = class.id.as_ref().map(|id| id.name.to_string()) else {
                continue;
            };
            let targets = self.class_targets(class);
            self.classes.insert(name, targets);
        }
    }

    fn class_targets(&self, class: &'ast Class<'ast>) -> ClassTargets {
        let mut targets = ClassTargets {
            super_name: class
                .super_class
                .as_ref()
                .and_then(|super_class| expression_identifier(super_class))
                .map(ToOwned::to_owned),
            ..ClassTargets::default()
        };
        for element in &class.body.body {
            match element {
                ClassElement::MethodDefinition(method) => {
                    if method.kind == MethodDefinitionKind::Constructor {
                        if let Some(body) = method.value.body.as_deref() {
                            let params = method
                                .value
                                .params
                                .items
                                .iter()
                                .map(|param| param_pattern_from_binding(&param.pattern))
                                .collect::<Vec<_>>();
                            self.collect_constructor_assignments_from_statements(
                                &body.statements,
                                &params,
                                &mut targets,
                            );
                        }
                        continue;
                    }
                    let Some(name) = static_property_key(&method.key, method.computed) else {
                        continue;
                    };
                    let Some(function) = self.function_for_class_method(method) else {
                        continue;
                    };
                    if method.r#static {
                        targets.static_object.add_property_target(name, function.id);
                    } else {
                        targets
                            .instance_object
                            .add_property_target(name, function.id);
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    let Some(name) = static_property_key(&property.key, property.computed) else {
                        continue;
                    };
                    let object = if property.r#static {
                        &mut targets.static_object
                    } else {
                        &mut targets.instance_object
                    };
                    let self_aliases = if property.r#static {
                        &mut targets.static_self_aliases
                    } else {
                        &mut targets.instance_self_aliases
                    };
                    if let Some(value) = &property.value {
                        if let Some(function) = self.function_for_expression(value) {
                            object.add_property_target(name, function.id);
                        } else if matches!(value, Expression::ThisExpression(_)) {
                            self_aliases.push(name);
                        }
                    }
                }
                ClassElement::StaticBlock(block) => {
                    self.collect_static_assignments_from_statements(
                        &block.body,
                        &mut targets.static_object,
                    );
                }
                ClassElement::AccessorProperty(_) | ClassElement::TSIndexSignature(_) => {}
            }
        }
        targets
    }

    fn collect_constructor_assignments_from_statements(
        &self,
        statements: &'ast oxc_allocator::Vec<'ast, Statement<'ast>>,
        params: &[ParamPattern],
        targets: &mut ClassTargets,
    ) {
        for statement in statements {
            self.collect_constructor_assignment_from_statement(statement, params, targets);
        }
    }

    fn collect_constructor_assignment_from_statement(
        &self,
        statement: &'ast Statement<'ast>,
        params: &[ParamPattern],
        targets: &mut ClassTargets,
    ) {
        match statement {
            Statement::ExpressionStatement(statement) => {
                self.collect_constructor_assignment_from_expression(
                    &statement.expression,
                    params,
                    targets,
                );
            }
            Statement::BlockStatement(block) => {
                self.collect_constructor_assignments_from_statements(&block.body, params, targets);
            }
            Statement::IfStatement(statement) => {
                self.collect_constructor_assignment_from_statement(
                    &statement.consequent,
                    params,
                    targets,
                );
                if let Some(alternate) = &statement.alternate {
                    self.collect_constructor_assignment_from_statement(alternate, params, targets);
                }
            }
            _ => {}
        }
    }

    fn collect_constructor_assignment_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        params: &[ParamPattern],
        targets: &mut ClassTargets,
    ) {
        match expression {
            Expression::AssignmentExpression(assignment) => {
                let Some(property) = this_assignment_property(&assignment.left) else {
                    return;
                };
                if let Some(function) = self.function_for_expression(&assignment.right) {
                    targets
                        .instance_object
                        .add_property_target(property, function.id);
                    return;
                }
                if let Some(index) = param_index_for_expression(&assignment.right, params) {
                    targets
                        .constructor_assignments
                        .push(ConstructorAssignment::Param { property, index });
                } else if matches!(&assignment.right, Expression::ThisExpression(_)) {
                    targets.instance_self_aliases.push(property);
                }
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.collect_constructor_assignment_from_expression(
                        expression, params, targets,
                    );
                }
            }
            Expression::ParenthesizedExpression(expression) => self
                .collect_constructor_assignment_from_expression(
                    &expression.expression,
                    params,
                    targets,
                ),
            _ => {}
        }
    }

    fn collect_static_assignments_from_statements(
        &self,
        statements: &'ast oxc_allocator::Vec<'ast, Statement<'ast>>,
        object: &mut ObjectTargets,
    ) {
        for statement in statements {
            match statement {
                Statement::ExpressionStatement(statement) => {
                    self.collect_static_assignment_from_expression(&statement.expression, object);
                }
                Statement::BlockStatement(block) => {
                    self.collect_static_assignments_from_statements(&block.body, object);
                }
                _ => {}
            }
        }
    }

    fn collect_static_assignment_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        object: &mut ObjectTargets,
    ) {
        match expression {
            Expression::AssignmentExpression(assignment) => {
                let Some(property) = this_assignment_property(&assignment.left) else {
                    return;
                };
                if let Some(function) = self.function_for_expression(&assignment.right) {
                    object.add_property_target(property, function.id);
                }
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.collect_static_assignment_from_expression(expression, object);
                }
            }
            Expression::ParenthesizedExpression(expression) => {
                self.collect_static_assignment_from_expression(&expression.expression, object);
            }
            _ => {}
        }
    }

    fn class_instance_targets(
        &self,
        name: &str,
        arguments: &[&'ast Expression<'ast>],
        env: &FlowEnv,
    ) -> Option<ObjectTargets> {
        self.class_instance_targets_with_depth(name, arguments, env, 0)
    }

    fn class_instance_targets_with_depth(
        &self,
        name: &str,
        arguments: &[&'ast Expression<'ast>],
        env: &FlowEnv,
        depth: usize,
    ) -> Option<ObjectTargets> {
        if depth > 8 {
            return None;
        }
        let class = self.classes.get(name)?;
        let mut object = ObjectTargets::default();
        if let Some(super_name) = &class.super_name
            && let Some(super_object) =
                self.class_instance_targets_with_depth(super_name, arguments, env, depth + 1)
        {
            object.merge(super_object);
        }
        object.merge(class.instance_object.clone());
        for assignment in &class.constructor_assignments {
            match assignment {
                ConstructorAssignment::Param { property, index } => {
                    if let Some(argument) = arguments.get(*index) {
                        for target in self
                            .callable_targets_from_expression(argument, env)
                            .all_targets()
                        {
                            object.add_property_target(property.clone(), target);
                        }
                    }
                }
            }
        }
        for alias in &class.instance_self_aliases {
            object.add_object_property(alias.clone(), object.clone());
        }
        Some(object)
    }

    fn class_static_targets(&self, name: &str) -> Option<ObjectTargets> {
        self.class_static_targets_with_depth(name, 0)
    }

    fn class_static_targets_with_depth(&self, name: &str, depth: usize) -> Option<ObjectTargets> {
        if depth > 8 {
            return None;
        }
        let class = self.classes.get(name)?;
        let mut object = ObjectTargets::default();
        if let Some(super_name) = &class.super_name
            && let Some(super_object) = self.class_static_targets_with_depth(super_name, depth + 1)
        {
            object.merge(super_object);
        }
        object.merge(class.static_object.clone());
        for alias in &class.static_self_aliases {
            object.add_object_property(alias.clone(), object.clone());
        }
        Some(object)
    }

    fn collect_statement(
        &mut self,
        statement: &'ast Statement<'ast>,
        owner: &'db FunctionFact,
        env: &mut FlowEnv,
    ) {
        match statement {
            Statement::VariableDeclaration(variable) => {
                self.collect_variable_declaration(variable, owner, env);
            }
            Statement::ExpressionStatement(statement) => {
                self.collect_expression(&statement.expression, owner, env);
            }
            Statement::ForOfStatement(statement) => {
                let iterable_targets = self.targets_for_iterable(&statement.right, env);
                self.collect_for_of_bindings(
                    &statement.left,
                    &iterable_targets,
                    statement.body.span(),
                    owner,
                );
                self.collect_statement(&statement.body, owner, env);
            }
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.collect_statement(statement, owner, env);
                }
            }
            Statement::IfStatement(statement) => {
                self.collect_expression(&statement.test, owner, env);
                self.collect_statement(&statement.consequent, owner, env);
                if let Some(alternate) = &statement.alternate {
                    self.collect_statement(alternate, owner, env);
                }
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.collect_expression(argument, owner, env);
                }
            }
            _ => {}
        }
    }

    fn collect_variable_declaration(
        &mut self,
        variable: &'ast VariableDeclaration<'ast>,
        owner: &'db FunctionFact,
        env: &mut FlowEnv,
    ) {
        for declarator in &variable.declarations {
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(Expression::ClassExpression(class)) = &declarator.init
            {
                self.classes.insert(name.clone(), self.class_targets(class));
                if let Some(statics) = self.class_static_targets(&name) {
                    env.objects.insert(name, statics);
                }
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(targets) = self.targets_for_declarator_init(declarator, env)
            {
                env.bindings.insert(name, targets);
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(init) = &declarator.init
                && let Some(function) = self.function_for_expression(init)
            {
                env.bindings.insert(
                    name,
                    CollectionTargets {
                        keys: Vec::new(),
                        values: vec![function.id],
                        object_values: Vec::new(),
                    },
                );
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(init) = &declarator.init
                && let Some(targets) = self.async_function_promise_targets(init, env)
            {
                env.async_functions.insert(name, targets);
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(init) = &declarator.init
                && let Some(targets) = self.async_generator_yield_targets(init, env)
            {
                env.async_generators.insert(name, targets);
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(init) = &declarator.init
                && let Some(targets) = self.generator_iterator_targets(init, env)
            {
                env.async_iterators.insert(name, targets);
            }
            if let Some(init) = &declarator.init
                && let Some(targets) = self.collection_targets_from_expression(init, env)
            {
                bind_collection_pattern(&declarator.id, &targets, env);
            }
            if let Some(init) = &declarator.init
                && let Some(targets) = self.object_targets_from_expression(init, env)
            {
                bind_object_pattern(&declarator.id, &targets, env);
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(init) = &declarator.init
                && let Some(targets) = self.promise_targets_from_expression(init, env)
            {
                env.promises.insert(name, targets);
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(Expression::ObjectExpression(object)) = &declarator.init
            {
                env.objects
                    .insert(name, self.object_literal_targets(object, Some(env)));
            }
            if let Some(init) = &declarator.init {
                self.collect_expression(init, owner, env);
            }
        }
    }

    fn collect_expression(
        &mut self,
        expression: &'ast Expression<'ast>,
        owner: &'db FunctionFact,
        env: &mut FlowEnv,
    ) {
        match expression {
            Expression::CallExpression(call) => {
                self.collect_call_expression(call, owner, env);
                self.collect_call_callee_expression(&call.callee, owner, env);
                for argument in &call.arguments {
                    if let Some(expression) = argument_expression(argument) {
                        self.collect_expression(expression, owner, env);
                    }
                }
            }
            Expression::AwaitExpression(expression) => {
                self.collect_expression(&expression.argument, owner, env);
            }
            Expression::AssignmentExpression(assignment) => {
                self.collect_assignment_value_flow(assignment, env);
                if let Some(collection_name) = collection_assignment_target_name(&assignment.left)
                    && let Some(targets) = env.bindings.get_mut(collection_name)
                    && let Some(function) = self.function_for_expression(&assignment.right)
                {
                    targets.values.push(function.id);
                }
                self.collect_expression(&assignment.right, owner, env);
            }
            Expression::ArrayExpression(array) => {
                for element in &array.elements {
                    if let Some(expression) = array_element_expression(element) {
                        self.collect_expression(expression, owner, env);
                    }
                }
            }
            Expression::ObjectExpression(object) => {
                for property in &object.properties {
                    match property {
                        ObjectPropertyKind::ObjectProperty(property) => {
                            self.collect_expression(&property.value, owner, env);
                        }
                        ObjectPropertyKind::SpreadProperty(spread) => {
                            self.collect_expression(&spread.argument, owner, env);
                        }
                    }
                }
            }
            Expression::ParenthesizedExpression(expression) => {
                self.collect_expression(&expression.expression, owner, env);
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.collect_expression(expression, owner, env);
                }
            }
            _ => {}
        }
    }

    fn collect_assignment_value_flow(
        &mut self,
        assignment: &'ast oxc_ast::ast::AssignmentExpression<'ast>,
        env: &mut FlowEnv,
    ) {
        if let Some((class_name, super_name)) =
            prototype_assignment_super_name(&assignment.left, &assignment.right)
        {
            self.classes
                .entry(class_name.to_string())
                .or_default()
                .super_name = Some(super_name.to_string());
        }

        if let Some(property) = this_assignment_property(&assignment.left) {
            for target in self
                .callable_targets_from_expression(&assignment.right, env)
                .all_targets()
            {
                env.this_object
                    .add_property_target(property.clone(), target);
            }
            if let Some(nested_object) = self.object_targets_from_expression(&assignment.right, env)
            {
                env.this_object.add_object_property(property, nested_object);
            }
            return;
        }

        if let Some((class_name, property)) = prototype_member_assignment_name(&assignment.left) {
            let property = property.to_string();
            let targets = self
                .callable_targets_from_expression(&assignment.right, env)
                .all_targets();
            if !targets.is_empty()
                && let Some(class) = self.classes.get_mut(class_name)
            {
                for target in targets {
                    class
                        .instance_object
                        .add_property_target(property.clone(), target);
                }
            }
            return;
        }

        let Some((object_name, property)) = assignment_static_member_name(&assignment.left) else {
            return;
        };
        let property = property.to_string();
        let callable_targets = self
            .callable_targets_from_expression(&assignment.right, env)
            .all_targets();
        let nested_object = self.object_targets_from_expression(&assignment.right, env);

        if let Some(object) = env.objects.get_mut(object_name) {
            for target in &callable_targets {
                object.add_property_target(property.clone(), *target);
            }
            if let Some(nested_object) = nested_object.clone() {
                object.add_object_property(property.clone(), nested_object);
            }
        }

        if self.classes.contains_key(object_name) {
            if let Some(class) = self.classes.get_mut(object_name) {
                for target in &callable_targets {
                    class
                        .static_object
                        .add_property_target(property.clone(), *target);
                }
                if let Some(nested_object) = nested_object {
                    class
                        .static_object
                        .add_object_property(property, nested_object);
                }
            }
            if let Some(statics) = self.class_static_targets(object_name) {
                env.objects.insert(object_name.to_string(), statics);
            }
        }
    }

    fn collect_call_callee_expression(
        &mut self,
        callee: &'ast Expression<'ast>,
        owner: &'db FunctionFact,
        env: &mut FlowEnv,
    ) {
        match callee {
            Expression::StaticMemberExpression(member) => {
                self.collect_expression(&member.object, owner, env);
            }
            Expression::ComputedMemberExpression(member) => {
                self.collect_expression(&member.object, owner, env);
                self.collect_expression(&member.expression, owner, env);
            }
            Expression::ParenthesizedExpression(expression) => {
                self.collect_call_callee_expression(&expression.expression, owner, env);
            }
            _ => {}
        }
    }

    fn collect_call_expression(
        &mut self,
        call: &'ast CallExpression<'ast>,
        owner: &'db FunctionFact,
        env: &mut FlowEnv,
    ) {
        self.collect_inline_callee_value_flows(call, env);

        if let Some((source, method)) = self.promise_method_source(call, env) {
            self.collect_promise_handler_flows(call, method, &source, env);
        }

        if callee_text(&call.callee).as_deref() == Some("Array.from")
            && let Some(collection_targets) = call
                .arguments
                .first()
                .and_then(argument_expression)
                .and_then(|argument| self.collection_targets_from_expression(argument, env))
            && let Some(callback_expression) = call.arguments.get(1).and_then(argument_expression)
            && let Some(callback) = self.function_for_expression(callback_expression)
        {
            self.collect_callback_parameter_flows(
                callback,
                "from",
                &collection_targets,
                callback_expression,
            );
            if let Some(this_object) = call
                .arguments
                .get(2)
                .and_then(argument_expression)
                .and_then(expression_identifier)
                .and_then(|name| env.objects.get(name))
            {
                self.collect_this_object_member_flows(callback, this_object);
            }
        }

        if let Some((collection_name, method)) = static_member_call(&call.callee) {
            if matches!(method, "push" | "add")
                && let Some(targets) = env.bindings.get_mut(collection_name)
            {
                for argument in &call.arguments {
                    if let Some(target) = argument_expression(argument)
                        .and_then(|expression| self.function_for_expression(expression))
                    {
                        targets.values.push(target.id);
                    }
                }
            }
            if method == "set"
                && let Some(targets) = env.bindings.get_mut(collection_name)
            {
                if let Some(target) = call
                    .arguments
                    .first()
                    .and_then(argument_expression)
                    .and_then(|expression| self.function_for_expression(expression))
                {
                    targets.keys.push(target.id);
                }
                if let Some(target) = call
                    .arguments
                    .get(1)
                    .and_then(argument_expression)
                    .and_then(|expression| self.function_for_expression(expression))
                {
                    targets.values.push(target.id);
                }
            }
            if let Some(collection_targets) = env.bindings.get(collection_name).cloned()
                && let Some(argument_index) = callback_argument_index(method)
                && let Some(callback) = call
                    .arguments
                    .get(argument_index)
                    .and_then(argument_expression)
                    .and_then(|expression| self.function_for_expression(expression))
            {
                self.collect_callback_parameter_flows(
                    callback,
                    method,
                    &collection_targets,
                    call.arguments
                        .get(argument_index)
                        .and_then(argument_expression)
                        .expect("callback expression already resolved"),
                );
            }
        }

        if let Some((object_name, property)) = static_member_call(&call.callee) {
            let targets = env
                .objects
                .get(object_name)
                .and_then(|object| object.properties.get(property))
                .cloned();
            if let Some(targets) = targets {
                self.emit_member_targets(owner, property, call.span, Some(targets.clone()));
                self.collect_receiver_side_effects(object_name, &targets, call, env);
            }
        }
        if let Expression::StaticMemberExpression(member) = &call.callee
            && let Some(object) = self.object_targets_from_expression(&member.object, env)
            && let Some(targets) = object
                .properties
                .get(member.property.name.as_str())
                .cloned()
        {
            self.emit_member_targets(
                owner,
                member.property.name.as_str(),
                call.span,
                Some(targets),
            );
        }

        if let Expression::Identifier(identifier) = &call.callee {
            if let Some(flow) = self
                .function_declarations
                .get(identifier.name.as_str())
                .cloned()
            {
                self.collect_function_parameter_flows(&flow, call, env);
            }
            self.emit_binding_targets(
                owner,
                identifier.name.as_str(),
                call.span,
                env.bindings
                    .get(identifier.name.as_str())
                    .map(CollectionTargets::all_targets),
            );
        }
        if let Expression::ComputedMemberExpression(member) = &call.callee
            && let Some(source) =
                expression_identifier(&member.object).and_then(|name| env.bindings.get(name))
            && let Some(index) = numeric_index(&member.expression)
        {
            let binding = callee_text(&call.callee).unwrap_or_else(|| "indexed".to_string());
            self.emit_call_span_targets(
                owner,
                call.span,
                &binding,
                Some(source.value_at(index).all_targets()),
            );
        }
    }

    fn collect_inline_callee_value_flows(
        &mut self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) {
        let Some(expression) = expression_as_function(&call.callee) else {
            return;
        };
        let Some(function) = self.function_for_expression(expression) else {
            return;
        };
        let bindings = call
            .arguments
            .iter()
            .enumerate()
            .filter_map(|(index, argument)| {
                argument_expression(argument)
                    .and_then(|expression| self.targets_from_argument_expression(expression, env))
                    .map(|targets| (index, targets))
            })
            .collect::<Vec<_>>();
        self.collect_callback_value_flows(function, expression, bindings, env);
    }

    fn collect_function_parameter_flows(
        &mut self,
        flow: &FunctionFlow<'db, 'ast>,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) {
        let mut param_env = FlowEnv::default();
        for (index, pattern) in flow.params.iter().enumerate() {
            if let Some(argument) = call.arguments.get(index).and_then(argument_expression) {
                if let Some(targets) = self.targets_from_argument_expression(argument, env) {
                    bind_param_pattern(pattern, &targets, &mut param_env);
                }
                if let Some(targets) = self.object_targets_from_expression(argument, env) {
                    bind_param_object_pattern(pattern, &targets, &mut param_env);
                }
            }
        }
        if let Some(rest) = &flow.rest {
            let mut targets = CollectionTargets::default();
            for argument in call.arguments.iter().skip(flow.params.len()) {
                if let Some(function) = argument_expression(argument)
                    .and_then(|expression| self.function_for_expression(expression))
                {
                    targets.values.push(function.id);
                }
            }
            bind_param_pattern(rest, &targets, &mut param_env);
        }
        for statement in &flow.body.statements {
            self.collect_statement(statement, flow.function, &mut param_env);
        }
    }

    fn collect_receiver_side_effects(
        &mut self,
        object_name: &str,
        targets: &[FunctionId],
        call: &'ast CallExpression<'ast>,
        env: &mut FlowEnv,
    ) {
        let Some(receiver) = env.objects.get(object_name).cloned() else {
            return;
        };
        let mut merged_receiver = receiver.clone();
        for target in targets {
            let Some(flow) = self.function_flows_by_id.get(target).cloned() else {
                continue;
            };
            let mut callee_env = env.clone();
            callee_env.this_object = receiver.clone();
            self.bind_call_arguments_to_flow(&flow, call, env, &mut callee_env);
            for statement in &flow.body.statements {
                self.collect_statement(statement, flow.function, &mut callee_env);
            }
            merged_receiver.merge(callee_env.this_object);
        }
        env.objects.insert(object_name.to_string(), merged_receiver);
    }

    fn bind_call_arguments_to_flow(
        &self,
        flow: &FunctionFlow<'db, 'ast>,
        call: &'ast CallExpression<'ast>,
        parent_env: &FlowEnv,
        callee_env: &mut FlowEnv,
    ) {
        for (index, pattern) in flow.params.iter().enumerate() {
            if let Some(argument) = call.arguments.get(index).and_then(argument_expression) {
                if let Some(targets) = self.targets_from_argument_expression(argument, parent_env) {
                    bind_param_pattern(pattern, &targets, callee_env);
                }
                if let Some(targets) = self.object_targets_from_expression(argument, parent_env) {
                    bind_param_object_pattern(pattern, &targets, callee_env);
                }
            }
        }
        if let Some(rest) = &flow.rest {
            let mut targets = CollectionTargets::default();
            for argument in call.arguments.iter().skip(flow.params.len()) {
                if let Some(function) = argument_expression(argument)
                    .and_then(|expression| self.function_for_expression(expression))
                {
                    targets.values.push(function.id);
                }
            }
            bind_param_pattern(rest, &targets, callee_env);
        }
    }

    fn targets_from_argument_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<CollectionTargets> {
        if let Some(function) = self.function_for_expression(expression) {
            return Some(CollectionTargets {
                keys: Vec::new(),
                values: vec![function.id],
                object_values: Vec::new(),
            });
        }
        self.collection_targets_from_expression(expression, env)
    }

    fn collect_for_of_bindings(
        &mut self,
        left: &'ast ForStatementLeft<'ast>,
        iterable_targets: &CollectionTargets,
        body_span: oxc_span::Span,
        owner: &'db FunctionFact,
    ) {
        if let ForStatementLeft::VariableDeclaration(variable) = left {
            for declarator in &variable.declarations {
                self.collect_loop_binding_pattern(
                    &declarator.id,
                    iterable_targets,
                    body_span,
                    owner,
                );
            }
        }
    }

    fn collect_loop_binding_pattern(
        &mut self,
        pattern: &'ast BindingPattern<'ast>,
        iterable_targets: &CollectionTargets,
        body_span: oxc_span::Span,
        owner: &'db FunctionFact,
    ) {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => {
                self.emit_binding_targets(
                    owner,
                    identifier.name.as_str(),
                    body_span,
                    Some(iterable_targets.all_targets()),
                );
            }
            BindingPattern::ArrayPattern(array) => {
                for (index, element) in array.elements.iter().enumerate() {
                    if let Some(binding) = element
                        && let Some(name) = binding_identifier_name(binding)
                    {
                        let targets = if index == 0 {
                            iterable_targets.keys_or_values()
                        } else {
                            iterable_targets.values.clone()
                        };
                        self.emit_binding_targets(owner, &name, body_span, Some(targets));
                    }
                }
            }
            BindingPattern::AssignmentPattern(pattern) => {
                self.collect_loop_binding_pattern(
                    &pattern.left,
                    iterable_targets,
                    body_span,
                    owner,
                );
            }
            _ => {}
        }
    }

    fn collect_callback_parameter_flows(
        &mut self,
        callback: &'db FunctionFact,
        method: &str,
        collection_targets: &CollectionTargets,
        expression: &'ast Expression<'ast>,
    ) {
        let parameter_names = callback_parameter_names(expression);
        for (index, name) in parameter_names.iter().enumerate() {
            let targets = callback_targets_for_parameter(method, index, collection_targets);
            if !targets.is_empty() {
                let span = oxc_span::Span::new(callback.span.start_byte, callback.span.end_byte);
                self.emit_binding_targets(callback, name, span, Some(targets));
            }
        }
    }

    fn collect_this_object_member_flows(
        &mut self,
        callback: &'db FunctionFact,
        object: &ObjectTargets,
    ) {
        let span = oxc_span::Span::new(callback.span.start_byte, callback.span.end_byte);
        for (property, targets) in &object.properties {
            self.emit_member_targets(callback, property, span, Some(targets.clone()));
        }
    }

    fn targets_for_declarator_init(
        &self,
        declarator: &'ast VariableDeclarator<'ast>,
        env: &FlowEnv,
    ) -> Option<CollectionTargets> {
        let init = declarator.init.as_ref()?;
        match init {
            Expression::ArrayExpression(array) => Some(self.array_literal_targets(array)),
            Expression::NewExpression(expression) => {
                let name = callee_identifier(&expression.callee)?;
                match name {
                    "Array" | "Set" => expression
                        .arguments
                        .first()
                        .and_then(argument_expression)
                        .and_then(|argument| self.collection_targets_from_expression(argument, env))
                        .or_else(|| Some(CollectionTargets::default())),
                    "Map" => expression
                        .arguments
                        .first()
                        .and_then(argument_expression)
                        .map(|argument| self.map_entries_targets_from_expression(argument, env))
                        .or_else(|| Some(CollectionTargets::default())),
                    _ => None,
                }
            }
            Expression::CallExpression(call) => {
                if callee_text(&call.callee).as_deref() == Some("Array.from") {
                    self.array_from_targets(call, env)
                } else if let Some((collection_name, method)) = static_member_call(&call.callee) {
                    let source = env.bindings.get(collection_name)?;
                    match method {
                        "values" | "slice" | "splice" | "filter" | "map" | "flatMap" | "flat"
                        | "concat" => Some(CollectionTargets {
                            keys: Vec::new(),
                            values: source.values.clone(),
                            object_values: source.object_values.clone(),
                        }),
                        "keys" => Some(CollectionTargets {
                            keys: Vec::new(),
                            values: source.keys_or_values(),
                            object_values: Vec::new(),
                        }),
                        "entries" => Some(source.clone()),
                        "pop" | "at" | "find" => Some(CollectionTargets {
                            keys: Vec::new(),
                            values: source.values.clone(),
                            object_values: source.object_values.clone(),
                        }),
                        _ => None,
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn targets_for_iterable(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> CollectionTargets {
        match expression {
            Expression::Identifier(identifier) => env
                .bindings
                .get(identifier.name.as_str())
                .cloned()
                .unwrap_or_default(),
            Expression::CallExpression(call) => {
                if let Some(name) = callee_identifier(&call.callee)
                    && let Some(targets) = env.async_generators.get(name)
                {
                    return targets.clone();
                }
                if let Some((collection_name, method)) = static_member_call(&call.callee)
                    && let Some(source) = env.bindings.get(collection_name)
                {
                    match method {
                        "values" => CollectionTargets {
                            keys: Vec::new(),
                            values: source.values.clone(),
                            object_values: source.object_values.clone(),
                        },
                        "keys" => CollectionTargets {
                            keys: Vec::new(),
                            values: source.keys_or_values(),
                            object_values: Vec::new(),
                        },
                        "entries" => source.clone(),
                        _ => CollectionTargets::default(),
                    }
                } else {
                    CollectionTargets::default()
                }
            }
            _ => CollectionTargets::default(),
        }
    }

    fn collection_targets_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<CollectionTargets> {
        match expression {
            Expression::Identifier(identifier) => {
                env.bindings.get(identifier.name.as_str()).cloned()
            }
            Expression::ArrayExpression(array) => Some(self.array_literal_targets(array)),
            Expression::ComputedMemberExpression(member) => {
                let source = expression_identifier(&member.object)
                    .and_then(|name| env.bindings.get(name))?;
                let index = numeric_index(&member.expression)?;
                Some(source.value_at(index))
            }
            Expression::AwaitExpression(expression) => {
                Some(self.fulfilled_targets_from_expression(&expression.argument, env))
            }
            Expression::ParenthesizedExpression(expression) => {
                self.collection_targets_from_expression(&expression.expression, env)
            }
            _ => None,
        }
    }

    fn object_targets_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<ObjectTargets> {
        match expression {
            Expression::Identifier(identifier) => {
                env.objects.get(identifier.name.as_str()).cloned()
            }
            Expression::ThisExpression(_) => Some(env.this_object.clone()),
            Expression::ObjectExpression(object) => {
                Some(self.object_literal_targets(object, Some(env)))
            }
            Expression::ComputedMemberExpression(member) => {
                let source = expression_identifier(&member.object)
                    .and_then(|name| env.bindings.get(name))?;
                let index = numeric_index(&member.expression)?;
                source.object_at(index)
            }
            Expression::NewExpression(expression) => {
                let name = callee_identifier(&expression.callee)?;
                let arguments = expression
                    .arguments
                    .iter()
                    .filter_map(argument_expression)
                    .collect::<Vec<_>>();
                self.class_instance_targets(name, &arguments, env)
            }
            Expression::StaticMemberExpression(member) => {
                let object = self.object_targets_from_expression(&member.object, env)?;
                object
                    .object_properties
                    .get(member.property.name.as_str())
                    .map(|targets| (**targets).clone())
            }
            Expression::ParenthesizedExpression(expression) => {
                self.object_targets_from_expression(&expression.expression, env)
            }
            _ => None,
        }
    }

    fn promise_targets_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<PromiseTargets> {
        match expression {
            Expression::Identifier(identifier) => {
                env.promises.get(identifier.name.as_str()).cloned()
            }
            Expression::NewExpression(new_expression)
                if callee_identifier(&new_expression.callee) == Some("Promise") =>
            {
                new_expression
                    .arguments
                    .first()
                    .and_then(argument_expression)
                    .and_then(|executor| self.promise_targets_from_executor(executor, env))
            }
            Expression::CallExpression(call) => self.promise_targets_from_call(call, env),
            Expression::ParenthesizedExpression(expression) => {
                self.promise_targets_from_expression(&expression.expression, env)
            }
            _ => None,
        }
    }

    fn promise_targets_from_call(
        &self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) -> Option<PromiseTargets> {
        if let Some(name) = callee_identifier(&call.callee)
            && let Some(targets) = env.async_functions.get(name)
        {
            return Some(targets.clone());
        }
        if let Some((iterator_name, "next")) = static_member_call(&call.callee)
            && let Some(values) = env.async_iterators.get(iterator_name)
        {
            return Some(PromiseTargets {
                fulfilled: CollectionTargets {
                    keys: Vec::new(),
                    values: Vec::new(),
                    object_values: vec![iterator_result_object(values)],
                },
                rejected: CollectionTargets::default(),
            });
        }

        match callee_text(&call.callee).as_deref() {
            Some("Promise.resolve") => Some(PromiseTargets {
                fulfilled: call
                    .arguments
                    .first()
                    .and_then(argument_expression)
                    .map(|argument| self.fulfilled_targets_from_expression(argument, env))
                    .unwrap_or_default(),
                rejected: CollectionTargets::default(),
            }),
            Some("Promise.reject") => Some(PromiseTargets {
                fulfilled: CollectionTargets::default(),
                rejected: call
                    .arguments
                    .first()
                    .and_then(argument_expression)
                    .map(|argument| self.callable_targets_from_expression(argument, env))
                    .unwrap_or_default(),
            }),
            Some("Promise.all") | Some("Promise.any") | Some("Promise.race") => call
                .arguments
                .first()
                .and_then(argument_expression)
                .map(|argument| self.aggregate_promise_targets(argument, env)),
            Some("Promise.allSettled") => call
                .arguments
                .first()
                .and_then(argument_expression)
                .map(|argument| self.all_settled_promise_targets(argument, env)),
            _ => self
                .promise_method_source(call, env)
                .map(|(source, method)| {
                    self.promise_method_result_targets(call, method, &source, env)
                }),
        }
    }

    fn promise_targets_from_executor(
        &self,
        executor: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<PromiseTargets> {
        let names = callback_parameter_names(executor);
        let resolve_name = names.first()?;
        let reject_name = names.get(1).map(String::as_str).unwrap_or("__reject");
        let mut targets = PromiseTargets::default();
        match executor {
            Expression::ArrowFunctionExpression(function) => {
                if let Some(expression) = function.get_expression() {
                    self.collect_promise_executor_expression(
                        expression,
                        resolve_name,
                        reject_name,
                        env,
                        &mut targets,
                    );
                } else {
                    for statement in &function.body.statements {
                        self.collect_promise_executor_statement(
                            statement,
                            resolve_name,
                            reject_name,
                            env,
                            &mut targets,
                        );
                    }
                }
            }
            Expression::FunctionExpression(function) => {
                if let Some(body) = function.body.as_deref() {
                    for statement in &body.statements {
                        self.collect_promise_executor_statement(
                            statement,
                            resolve_name,
                            reject_name,
                            env,
                            &mut targets,
                        );
                    }
                }
            }
            _ => return None,
        }
        Some(targets)
    }

    fn async_function_promise_targets(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<PromiseTargets> {
        match expression {
            Expression::ArrowFunctionExpression(function) if function.r#async => {
                async_arrow_returned_expression(function)
                    .map(|expression| self.promise_targets_from_async_return(expression, env))
            }
            Expression::FunctionExpression(function) if function.r#async && !function.generator => {
                function
                    .body
                    .as_deref()
                    .and_then(|body| returned_expression_from_statements(&body.statements))
                    .map(|expression| self.promise_targets_from_async_return(expression, env))
            }
            Expression::ParenthesizedExpression(expression) => {
                self.async_function_promise_targets(&expression.expression, env)
            }
            _ => None,
        }
    }

    fn async_generator_yield_targets(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<CollectionTargets> {
        match expression {
            Expression::FunctionExpression(function) if function.r#async && function.generator => {
                function
                    .body
                    .as_deref()
                    .map(|body| self.yield_targets_from_statements(&body.statements, env))
            }
            Expression::ParenthesizedExpression(expression) => {
                self.async_generator_yield_targets(&expression.expression, env)
            }
            _ => None,
        }
    }

    fn generator_iterator_targets(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<CollectionTargets> {
        match expression {
            Expression::CallExpression(call) => {
                let name = callee_identifier(&call.callee)?;
                env.async_generators.get(name).cloned()
            }
            Expression::ParenthesizedExpression(expression) => {
                self.generator_iterator_targets(&expression.expression, env)
            }
            _ => None,
        }
    }

    fn yield_targets_from_statements(
        &self,
        statements: &'ast oxc_allocator::Vec<'ast, Statement<'ast>>,
        env: &FlowEnv,
    ) -> CollectionTargets {
        let mut targets = CollectionTargets::default();
        for statement in statements {
            targets.extend(self.yield_targets_from_statement(statement, env));
        }
        targets
    }

    fn yield_targets_from_statement(
        &self,
        statement: &'ast Statement<'ast>,
        env: &FlowEnv,
    ) -> CollectionTargets {
        match statement {
            Statement::ExpressionStatement(statement) => {
                self.yield_targets_from_expression(&statement.expression, env)
            }
            Statement::BlockStatement(block) => {
                self.yield_targets_from_statements(&block.body, env)
            }
            Statement::IfStatement(statement) => {
                let mut targets = self.yield_targets_from_statement(&statement.consequent, env);
                if let Some(alternate) = &statement.alternate {
                    targets.extend(self.yield_targets_from_statement(alternate, env));
                }
                targets
            }
            _ => CollectionTargets::default(),
        }
    }

    fn yield_targets_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> CollectionTargets {
        match expression {
            Expression::YieldExpression(yield_expression) => {
                let Some(argument) = &yield_expression.argument else {
                    return CollectionTargets::default();
                };
                if yield_expression.delegate {
                    self.collection_targets_from_expression(argument, env)
                        .unwrap_or_else(|| self.callable_targets_from_expression(argument, env))
                } else {
                    self.callable_targets_from_expression(argument, env)
                }
            }
            Expression::SequenceExpression(sequence) => {
                let mut targets = CollectionTargets::default();
                for expression in &sequence.expressions {
                    targets.extend(self.yield_targets_from_expression(expression, env));
                }
                targets
            }
            Expression::ParenthesizedExpression(expression) => {
                self.yield_targets_from_expression(&expression.expression, env)
            }
            _ => CollectionTargets::default(),
        }
    }

    fn promise_targets_from_async_return(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> PromiseTargets {
        if let Some(promise) = self.promise_targets_from_expression(expression, env) {
            return promise;
        }
        PromiseTargets {
            fulfilled: self.callable_targets_from_expression(expression, env),
            rejected: CollectionTargets::default(),
        }
    }

    fn collect_promise_executor_statement(
        &self,
        statement: &'ast Statement<'ast>,
        resolve_name: &str,
        reject_name: &str,
        env: &FlowEnv,
        targets: &mut PromiseTargets,
    ) {
        match statement {
            Statement::ExpressionStatement(statement) => self.collect_promise_executor_expression(
                &statement.expression,
                resolve_name,
                reject_name,
                env,
                targets,
            ),
            Statement::ThrowStatement(statement) => {
                targets
                    .rejected
                    .extend(self.callable_targets_from_expression(&statement.argument, env));
            }
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.collect_promise_executor_statement(
                        statement,
                        resolve_name,
                        reject_name,
                        env,
                        targets,
                    );
                }
            }
            Statement::IfStatement(statement) => {
                self.collect_promise_executor_statement(
                    &statement.consequent,
                    resolve_name,
                    reject_name,
                    env,
                    targets,
                );
                if let Some(alternate) = &statement.alternate {
                    self.collect_promise_executor_statement(
                        alternate,
                        resolve_name,
                        reject_name,
                        env,
                        targets,
                    );
                }
            }
            Statement::SwitchStatement(statement) => {
                for case in &statement.cases {
                    for statement in &case.consequent {
                        self.collect_promise_executor_statement(
                            statement,
                            resolve_name,
                            reject_name,
                            env,
                            targets,
                        );
                    }
                }
            }
            _ => {}
        }
    }

    fn collect_promise_executor_expression(
        &self,
        expression: &'ast Expression<'ast>,
        resolve_name: &str,
        reject_name: &str,
        env: &FlowEnv,
        targets: &mut PromiseTargets,
    ) {
        match expression {
            Expression::CallExpression(call) => {
                if let Expression::Identifier(identifier) = &call.callee {
                    if identifier.name == resolve_name {
                        if let Some(argument) = call.arguments.first().and_then(argument_expression)
                        {
                            targets
                                .fulfilled
                                .extend(self.fulfilled_targets_from_expression(argument, env));
                        }
                        return;
                    }
                    if identifier.name == reject_name {
                        if let Some(argument) = call.arguments.first().and_then(argument_expression)
                        {
                            targets
                                .rejected
                                .extend(self.callable_targets_from_expression(argument, env));
                        }
                        return;
                    }
                }
                for argument in &call.arguments {
                    if let Some(argument) = argument_expression(argument) {
                        self.collect_promise_executor_expression(
                            argument,
                            resolve_name,
                            reject_name,
                            env,
                            targets,
                        );
                    }
                }
            }
            Expression::SequenceExpression(sequence) => {
                for expression in &sequence.expressions {
                    self.collect_promise_executor_expression(
                        expression,
                        resolve_name,
                        reject_name,
                        env,
                        targets,
                    );
                }
            }
            Expression::ParenthesizedExpression(expression) => self
                .collect_promise_executor_expression(
                    &expression.expression,
                    resolve_name,
                    reject_name,
                    env,
                    targets,
                ),
            _ => {}
        }
    }

    fn promise_method_source(
        &self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) -> Option<(PromiseTargets, &'ast str)> {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        let method = member.property.name.as_str();
        if !matches!(method, "then" | "catch" | "finally") {
            return None;
        }
        let source = self.promise_targets_from_expression(&member.object, env)?;
        Some((source, method))
    }

    fn collect_promise_handler_flows(
        &mut self,
        call: &'ast CallExpression<'ast>,
        method: &str,
        source: &PromiseTargets,
        env: &FlowEnv,
    ) {
        match method {
            "then" => {
                self.collect_promise_handler_argument(call, 0, source.fulfilled.clone(), env);
                self.collect_promise_handler_argument(call, 1, source.rejected.clone(), env);
            }
            "catch" => {
                self.collect_promise_handler_argument(call, 0, source.rejected.clone(), env);
            }
            "finally" => {}
            _ => {}
        }
    }

    fn collect_promise_handler_argument(
        &mut self,
        call: &'ast CallExpression<'ast>,
        argument_index: usize,
        targets: CollectionTargets,
        env: &FlowEnv,
    ) {
        if targets.is_empty() {
            return;
        }
        let Some(expression) = call
            .arguments
            .get(argument_index)
            .and_then(argument_expression)
        else {
            return;
        };
        let Some(callback) = self.function_for_expression(expression) else {
            return;
        };
        self.collect_callback_value_flows(callback, expression, vec![(0, targets)], env);
    }

    fn collect_callback_value_flows(
        &mut self,
        callback: &'db FunctionFact,
        expression: &'ast Expression<'ast>,
        bindings: Vec<(usize, CollectionTargets)>,
        parent_env: &FlowEnv,
    ) {
        let patterns = callback_param_patterns(expression);
        let mut env = parent_env.clone();
        for (index, targets) in bindings {
            if let Some(pattern) = patterns.get(index) {
                bind_param_pattern(pattern, &targets, &mut env);
            }
        }
        match expression {
            Expression::ArrowFunctionExpression(function) => {
                if let Some(expression) = function.get_expression() {
                    self.collect_expression(expression, callback, &mut env);
                } else {
                    for statement in &function.body.statements {
                        self.collect_statement(statement, callback, &mut env);
                    }
                }
            }
            Expression::FunctionExpression(function) => {
                if let Some(body) = function.body.as_deref() {
                    for statement in &body.statements {
                        self.collect_statement(statement, callback, &mut env);
                    }
                }
            }
            _ => {}
        }
    }

    fn promise_method_result_targets(
        &self,
        call: &'ast CallExpression<'ast>,
        method: &str,
        source: &PromiseTargets,
        env: &FlowEnv,
    ) -> PromiseTargets {
        match method {
            "then" => {
                let mut result = PromiseTargets::default();
                if let Some(callback) = call.arguments.first().and_then(argument_expression) {
                    result.merge(self.promise_targets_from_callback_return(callback, env));
                } else {
                    result.fulfilled.extend(source.fulfilled.clone());
                }
                if let Some(callback) = call.arguments.get(1).and_then(argument_expression) {
                    result.merge(self.promise_targets_from_callback_return(callback, env));
                } else {
                    result.rejected.extend(source.rejected.clone());
                }
                result
            }
            "catch" => {
                let mut result = PromiseTargets {
                    fulfilled: source.fulfilled.clone(),
                    rejected: CollectionTargets::default(),
                };
                if let Some(callback) = call.arguments.first().and_then(argument_expression) {
                    result.merge(self.promise_targets_from_callback_return(callback, env));
                } else {
                    result.rejected.extend(source.rejected.clone());
                }
                result
            }
            "finally" => source.clone(),
            _ => PromiseTargets::default(),
        }
    }

    fn promise_targets_from_callback_return(
        &self,
        callback: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> PromiseTargets {
        let Some(expression) = callback_returned_expression(callback) else {
            return PromiseTargets::default();
        };
        if let Some(promise) = self.promise_targets_from_expression(expression, env) {
            return promise;
        }
        PromiseTargets {
            fulfilled: self.callable_targets_from_expression(expression, env),
            rejected: CollectionTargets::default(),
        }
    }

    fn callable_targets_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> CollectionTargets {
        if let Expression::ParenthesizedExpression(expression) = expression {
            return self.callable_targets_from_expression(&expression.expression, env);
        }
        if let Some(function) = self.function_for_expression(expression) {
            return CollectionTargets {
                keys: Vec::new(),
                values: vec![function.id],
                object_values: Vec::new(),
            };
        }
        if let Expression::StaticMemberExpression(member) = expression
            && let Some(object) = self.object_targets_from_expression(&member.object, env)
            && let Some(targets) = object.properties.get(member.property.name.as_str())
        {
            return CollectionTargets {
                keys: Vec::new(),
                values: targets.clone(),
                object_values: Vec::new(),
            };
        }
        self.collection_targets_from_expression(expression, env)
            .unwrap_or_default()
    }

    fn fulfilled_targets_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> CollectionTargets {
        if let Some(promise) = self.promise_targets_from_expression(expression, env) {
            return promise.fulfilled;
        }
        self.callable_targets_from_expression(expression, env)
    }

    fn aggregate_promise_targets(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> PromiseTargets {
        let mut targets = PromiseTargets::default();
        let Expression::ArrayExpression(array) = expression else {
            return targets;
        };
        for element in &array.elements {
            if let Some(expression) = array_element_expression(element)
                && let Some(promise) = self.promise_targets_from_expression(expression, env)
            {
                targets.merge(promise);
            }
        }
        targets
    }

    fn all_settled_promise_targets(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> PromiseTargets {
        let mut fulfilled = CollectionTargets::default();
        let Expression::ArrayExpression(array) = expression else {
            return PromiseTargets {
                fulfilled,
                rejected: CollectionTargets::default(),
            };
        };
        for element in &array.elements {
            let Some(expression) = array_element_expression(element) else {
                continue;
            };
            let Some(promise) = self.promise_targets_from_expression(expression, env) else {
                continue;
            };
            let mut result = ObjectTargets::default();
            for target in promise.fulfilled.all_targets() {
                result.add_property_target("value".to_string(), target);
            }
            for target in promise.rejected.all_targets() {
                result.add_property_target("reason".to_string(), target);
            }
            fulfilled.object_values.push(result);
        }
        PromiseTargets {
            fulfilled,
            rejected: CollectionTargets::default(),
        }
    }

    fn array_literal_targets(
        &self,
        array: &'ast oxc_ast::ast::ArrayExpression<'ast>,
    ) -> CollectionTargets {
        let mut targets = CollectionTargets::default();
        for element in &array.elements {
            if let Some(function) = array_element_expression(element)
                .and_then(|expression| self.function_for_expression(expression))
            {
                targets.values.push(function.id);
            }
        }
        targets
    }

    fn object_literal_targets(
        &self,
        object: &'ast oxc_ast::ast::ObjectExpression<'ast>,
        env: Option<&FlowEnv>,
    ) -> ObjectTargets {
        let mut targets = ObjectTargets::default();
        for property in &object.properties {
            let ObjectPropertyKind::ObjectProperty(property) = property else {
                continue;
            };
            let Some(name) = static_property_key(&property.key, property.computed) else {
                continue;
            };
            if let Some(function) = self
                .function_for_expression(&property.value)
                .or_else(|| self.function_for_span(property.span))
            {
                targets.add_property_target(name, function.id);
                continue;
            }
            if let Expression::ObjectExpression(value) = &property.value {
                let nested = self.object_literal_targets(value, env);
                if name == "__proto__" {
                    targets.merge(nested);
                } else {
                    targets.add_object_property(name, nested);
                }
            } else if matches!(&property.value, Expression::ThisExpression(_))
                && let Some(env) = env
            {
                targets.add_object_property(name, env.this_object.clone());
            }
        }
        targets
    }

    fn map_entries_targets_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> CollectionTargets {
        match expression {
            Expression::Identifier(identifier) => env
                .bindings
                .get(identifier.name.as_str())
                .cloned()
                .unwrap_or_default(),
            Expression::ArrayExpression(array) => {
                let mut targets = CollectionTargets::default();
                for element in &array.elements {
                    let Some(Expression::ArrayExpression(entry)) =
                        array_element_expression(element)
                    else {
                        continue;
                    };
                    if let Some(key) = entry
                        .elements
                        .first()
                        .and_then(array_element_expression)
                        .and_then(|expression| self.function_for_expression(expression))
                    {
                        targets.keys.push(key.id);
                    }
                    if let Some(value) = entry
                        .elements
                        .get(1)
                        .and_then(array_element_expression)
                        .and_then(|expression| self.function_for_expression(expression))
                    {
                        targets.values.push(value.id);
                    }
                }
                targets
            }
            Expression::ParenthesizedExpression(expression) => {
                self.map_entries_targets_from_expression(&expression.expression, env)
            }
            _ => CollectionTargets::default(),
        }
    }

    fn array_from_targets(
        &self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) -> Option<CollectionTargets> {
        let mut targets = call
            .arguments
            .first()
            .and_then(argument_expression)
            .and_then(|argument| self.collection_targets_from_expression(argument, env))?;
        if let Some(callback) = call
            .arguments
            .get(1)
            .and_then(argument_expression)
            .and_then(callback_returned_function)
            .and_then(|expression| self.function_for_expression(expression))
        {
            targets.values = vec![callback.id];
            targets.keys.clear();
        }
        Some(targets)
    }

    fn emit_binding_targets(
        &mut self,
        owner: &'db FunctionFact,
        name: &str,
        span: oxc_span::Span,
        targets: Option<Vec<FunctionId>>,
    ) {
        let Some(targets) = targets else {
            return;
        };
        if targets.is_empty() {
            return;
        }
        for site in self.sites.iter().copied().filter(|site| {
            site.caller == owner.id
                && site.span.start_byte >= span.start
                && site.span.end_byte <= span.end
                && matches!(
                    &site.callee,
                    CallCallee::Identifier { name: callee, .. } if callee == name
                )
        }) {
            for target in &targets {
                self.rows.push(CallTargetFact {
                    id: CallTargetId(self.next_id + self.rows.len() as u64),
                    site: site.id,
                    caller: site.caller,
                    target_function: Some(*target),
                    target_symbol: None,
                    edge_kind: CallEdgeKind::FunctionValue,
                    algorithm: CallAlgorithm::FunctionTokenFlow,
                    status: CallTargetStatus::Resolved,
                    reason: None,
                    provenance: CallProvenance::Model,
                    precision: CallPrecision::Heuristic,
                    stable_key: value_flow_target_stable_key(self.db, site, *target, name),
                });
            }
        }
    }

    fn emit_member_targets(
        &mut self,
        owner: &'db FunctionFact,
        property: &str,
        span: oxc_span::Span,
        targets: Option<Vec<FunctionId>>,
    ) {
        let Some(targets) = targets else {
            return;
        };
        if targets.is_empty() {
            return;
        }
        for site in self.sites.iter().copied().filter(|site| {
            site.caller == owner.id
                && site.span.start_byte >= span.start
                && site.span.end_byte <= span.end
                && matches!(
                    &site.callee,
                    CallCallee::Member {
                        property: callee,
                        ..
                    } if callee == property
                )
        }) {
            for target in &targets {
                self.rows.push(CallTargetFact {
                    id: CallTargetId(self.next_id + self.rows.len() as u64),
                    site: site.id,
                    caller: site.caller,
                    target_function: Some(*target),
                    target_symbol: None,
                    edge_kind: CallEdgeKind::FunctionValue,
                    algorithm: CallAlgorithm::FunctionTokenFlow,
                    status: CallTargetStatus::Resolved,
                    reason: None,
                    provenance: CallProvenance::Model,
                    precision: CallPrecision::Heuristic,
                    stable_key: value_flow_target_stable_key(self.db, site, *target, property),
                });
            }
        }
    }

    fn emit_call_span_targets(
        &mut self,
        owner: &'db FunctionFact,
        span: oxc_span::Span,
        binding: &str,
        targets: Option<Vec<FunctionId>>,
    ) {
        let Some(targets) = targets else {
            return;
        };
        if targets.is_empty() {
            return;
        }
        for site in self.sites.iter().copied().filter(|site| {
            site.caller == owner.id
                && site.span.start_byte == span.start
                && site.span.end_byte == span.end
        }) {
            for target in &targets {
                self.rows.push(CallTargetFact {
                    id: CallTargetId(self.next_id + self.rows.len() as u64),
                    site: site.id,
                    caller: site.caller,
                    target_function: Some(*target),
                    target_symbol: None,
                    edge_kind: CallEdgeKind::FunctionValue,
                    algorithm: CallAlgorithm::FunctionTokenFlow,
                    status: CallTargetStatus::Resolved,
                    reason: None,
                    provenance: CallProvenance::Model,
                    precision: CallPrecision::Heuristic,
                    stable_key: value_flow_target_stable_key(self.db, site, *target, binding),
                });
            }
        }
    }

    fn function_for_expression(
        &self,
        expression: &'ast Expression<'ast>,
    ) -> Option<&'db FunctionFact> {
        let span = expression.span();
        self.function_for_span(span)
    }

    fn function_for_class_method(
        &self,
        method: &'ast MethodDefinition<'ast>,
    ) -> Option<&'db FunctionFact> {
        let span = class_method_function_span(method);
        self.function_for_span(span)
            .or_else(|| self.function_for_span(method.span))
    }

    fn function_for_span(&self, span: oxc_span::Span) -> Option<&'db FunctionFact> {
        let functions = self.functions_by_start.get(&(self.file.id, span.start))?;
        functions
            .iter()
            .copied()
            .find(|function| function.span.end_byte == span.end)
            .or_else(|| functions.first().copied())
    }
}

#[derive(Clone, Debug, Default)]
struct FlowEnv {
    bindings: BTreeMap<String, CollectionTargets>,
    objects: BTreeMap<String, ObjectTargets>,
    promises: BTreeMap<String, PromiseTargets>,
    async_functions: BTreeMap<String, PromiseTargets>,
    async_generators: BTreeMap<String, CollectionTargets>,
    async_iterators: BTreeMap<String, CollectionTargets>,
    this_object: ObjectTargets,
}

#[derive(Clone, Debug, Default)]
struct CollectionTargets {
    keys: Vec<FunctionId>,
    values: Vec<FunctionId>,
    object_values: Vec<ObjectTargets>,
}

impl CollectionTargets {
    fn extend(&mut self, other: Self) {
        self.keys.extend(other.keys);
        self.values.extend(other.values);
        self.object_values.extend(other.object_values);
        self.keys.sort();
        self.keys.dedup();
        self.values.sort();
        self.values.dedup();
    }

    fn is_empty(&self) -> bool {
        self.keys.is_empty() && self.values.is_empty() && self.object_values.is_empty()
    }

    fn all_targets(&self) -> Vec<FunctionId> {
        let mut targets = self.values.clone();
        targets.extend(self.keys.iter().copied());
        targets.sort();
        targets.dedup();
        targets
    }

    fn keys_or_values(&self) -> Vec<FunctionId> {
        if self.keys.is_empty() {
            self.values.clone()
        } else {
            self.keys.clone()
        }
    }

    fn value_at(&self, index: usize) -> Self {
        Self {
            keys: Vec::new(),
            values: self.values.get(index).copied().into_iter().collect(),
            object_values: self.object_values.get(index).cloned().into_iter().collect(),
        }
    }

    fn values_from(&self, index: usize) -> Self {
        Self {
            keys: Vec::new(),
            values: self.values.iter().skip(index).copied().collect(),
            object_values: self.object_values.iter().skip(index).cloned().collect(),
        }
    }

    fn object_at(&self, index: usize) -> Option<ObjectTargets> {
        self.object_values.get(index).cloned()
    }

    fn singular_object(&self) -> Option<ObjectTargets> {
        (self.object_values.len() == 1).then(|| self.object_values[0].clone())
    }
}

#[derive(Clone, Debug, Default)]
struct ObjectTargets {
    properties: BTreeMap<String, Vec<FunctionId>>,
    object_properties: BTreeMap<String, Box<ObjectTargets>>,
}

impl ObjectTargets {
    fn is_empty(&self) -> bool {
        self.properties.is_empty() && self.object_properties.is_empty()
    }

    fn add_property_target(&mut self, name: String, target: FunctionId) {
        let targets = self.properties.entry(name).or_default();
        targets.push(target);
        targets.sort();
        targets.dedup();
    }

    fn add_object_property(&mut self, name: String, targets: ObjectTargets) {
        self.object_properties.insert(name, Box::new(targets));
    }

    fn merge(&mut self, other: Self) {
        for (name, mut targets) in other.properties {
            let slot = self.properties.entry(name).or_default();
            slot.append(&mut targets);
            slot.sort();
            slot.dedup();
        }
        for (name, targets) in other.object_properties {
            self.object_properties.entry(name).or_insert(targets);
        }
    }

    fn without_properties(&self, used: &[String]) -> Self {
        let used = used.iter().collect::<std::collections::BTreeSet<_>>();
        Self {
            properties: self
                .properties
                .iter()
                .filter(|(name, _)| !used.contains(name))
                .map(|(name, targets)| (name.clone(), targets.clone()))
                .collect(),
            object_properties: self
                .object_properties
                .iter()
                .filter(|(name, _)| !used.contains(name))
                .map(|(name, targets)| (name.clone(), targets.clone()))
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct ClassTargets {
    instance_object: ObjectTargets,
    static_object: ObjectTargets,
    constructor_assignments: Vec<ConstructorAssignment>,
    instance_self_aliases: Vec<String>,
    static_self_aliases: Vec<String>,
    super_name: Option<String>,
}

impl ClassTargets {
    fn is_empty(&self) -> bool {
        self.instance_object.is_empty()
            && self.static_object.is_empty()
            && self.constructor_assignments.is_empty()
            && self.instance_self_aliases.is_empty()
            && self.static_self_aliases.is_empty()
            && self.super_name.is_none()
    }
}

#[derive(Clone, Debug)]
enum ConstructorAssignment {
    Param { property: String, index: usize },
}

#[derive(Clone, Debug, Default)]
struct PromiseTargets {
    fulfilled: CollectionTargets,
    rejected: CollectionTargets,
}

impl PromiseTargets {
    fn merge(&mut self, other: Self) {
        self.fulfilled.extend(other.fulfilled);
        self.rejected.extend(other.rejected);
    }
}

fn sites_by_file(sites: &[CallSiteFact]) -> BTreeMap<FileId, Vec<&CallSiteFact>> {
    let mut by_file = BTreeMap::<FileId, Vec<&CallSiteFact>>::new();
    for site in sites {
        by_file.entry(site.file).or_default().push(site);
    }
    by_file
}

fn functions_by_file_start(db: &AnalysisDb) -> BTreeMap<(FileId, u32), Vec<&FunctionFact>> {
    let mut by_start = BTreeMap::<(FileId, u32), Vec<&FunctionFact>>::new();
    for function in db
        .functions()
        .iter()
        .filter(|function| function.language.is_ts_family())
    {
        by_start
            .entry((function.file, function.span.start_byte))
            .or_default()
            .push(function);
    }
    for functions in by_start.values_mut() {
        functions.sort_by_key(|function| (function.span.end_byte, function.id.0));
    }
    by_start
}

fn module_function<'db>(db: &'db AnalysisDb, file: &SourceFile) -> Option<&'db FunctionFact> {
    db.functions().iter().find(|function| {
        function.file == file.id
            && function.language == file.language
            && function.name == TS_JS_MODULE_FUNCTION_NAME
    })
}

fn binding_identifier_name(pattern: &BindingPattern<'_>) -> Option<String> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.to_string()),
        BindingPattern::AssignmentPattern(pattern) => binding_identifier_name(&pattern.left),
        _ => None,
    }
}

fn bind_collection_pattern(
    pattern: &BindingPattern<'_>,
    targets: &CollectionTargets,
    env: &mut FlowEnv,
) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            if let Some(object) = targets.singular_object() {
                env.objects.insert(identifier.name.to_string(), object);
            }
            env.bindings
                .insert(identifier.name.to_string(), targets.clone());
        }
        BindingPattern::ArrayPattern(array) => {
            for (index, element) in array.elements.iter().enumerate() {
                if let Some(element) = element {
                    bind_collection_pattern(element, &targets.value_at(index), env);
                }
            }
            if let Some(rest) = &array.rest {
                bind_collection_pattern(
                    &rest.argument,
                    &targets.values_from(array.elements.len()),
                    env,
                );
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            bind_collection_pattern(&pattern.left, targets, env);
        }
        _ => {}
    }
}

fn bind_object_pattern(pattern: &BindingPattern<'_>, targets: &ObjectTargets, env: &mut FlowEnv) {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            env.objects
                .insert(identifier.name.to_string(), targets.clone());
        }
        BindingPattern::ObjectPattern(object) => {
            let mut used = Vec::new();
            for property in &object.properties {
                let Some(name) = static_property_key(&property.key, property.computed) else {
                    continue;
                };
                used.push(name.clone());
                if let Some(values) = targets.properties.get(&name) {
                    bind_collection_pattern(
                        &property.value,
                        &CollectionTargets {
                            keys: Vec::new(),
                            values: values.clone(),
                            object_values: Vec::new(),
                        },
                        env,
                    );
                }
            }
            if let Some(rest) = &object.rest {
                bind_object_pattern(&rest.argument, &targets.without_properties(&used), env);
            }
        }
        BindingPattern::AssignmentPattern(pattern) => {
            bind_object_pattern(&pattern.left, targets, env);
        }
        _ => {}
    }
}

fn param_pattern_from_binding(pattern: &BindingPattern<'_>) -> ParamPattern {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => {
            ParamPattern::Binding(identifier.name.to_string())
        }
        BindingPattern::ArrayPattern(array) => ParamPattern::Array {
            elements: array
                .elements
                .iter()
                .map(|element| element.as_ref().map(param_pattern_from_binding))
                .collect(),
            rest: array
                .rest
                .as_ref()
                .map(|rest| Box::new(param_pattern_from_binding(&rest.argument))),
        },
        BindingPattern::ObjectPattern(object) => ParamPattern::Object {
            properties: object
                .properties
                .iter()
                .filter_map(|property| {
                    static_property_key(&property.key, property.computed)
                        .map(|name| (name, param_pattern_from_binding(&property.value)))
                })
                .collect(),
            rest: object
                .rest
                .as_ref()
                .map(|rest| Box::new(param_pattern_from_binding(&rest.argument))),
        },
        BindingPattern::AssignmentPattern(pattern) => param_pattern_from_binding(&pattern.left),
    }
}

fn bind_param_pattern(pattern: &ParamPattern, targets: &CollectionTargets, env: &mut FlowEnv) {
    match pattern {
        ParamPattern::Binding(name) => {
            if let Some(object) = targets.singular_object() {
                env.objects.insert(name.clone(), object);
            }
            env.bindings.insert(name.clone(), targets.clone());
        }
        ParamPattern::Array { elements, rest } => {
            for (index, element) in elements.iter().enumerate() {
                if let Some(element) = element {
                    bind_param_pattern(element, &targets.value_at(index), env);
                }
            }
            if let Some(rest) = rest {
                bind_param_pattern(rest, &targets.values_from(elements.len()), env);
            }
        }
        ParamPattern::Object { .. } => {}
    }
}

fn bind_param_object_pattern(pattern: &ParamPattern, targets: &ObjectTargets, env: &mut FlowEnv) {
    match pattern {
        ParamPattern::Binding(name) => {
            env.objects.insert(name.clone(), targets.clone());
        }
        ParamPattern::Object { properties, rest } => {
            let mut used = Vec::new();
            for (name, pattern) in properties {
                used.push(name.clone());
                if let Some(values) = targets.properties.get(name) {
                    bind_param_pattern(
                        pattern,
                        &CollectionTargets {
                            keys: Vec::new(),
                            values: values.clone(),
                            object_values: Vec::new(),
                        },
                        env,
                    );
                }
            }
            if let Some(rest) = rest {
                bind_param_object_pattern(rest, &targets.without_properties(&used), env);
            }
        }
        ParamPattern::Array { .. } => {}
    }
}

fn collection_assignment_target_name<'a>(target: &'a AssignmentTarget<'a>) -> Option<&'a str> {
    match target {
        AssignmentTarget::ComputedMemberExpression(member) => expression_identifier(&member.object),
        AssignmentTarget::StaticMemberExpression(member) => expression_identifier(&member.object),
        _ => None,
    }
}

fn this_assignment_property(target: &AssignmentTarget<'_>) -> Option<String> {
    match target {
        AssignmentTarget::StaticMemberExpression(member)
            if matches!(&member.object, Expression::ThisExpression(_)) =>
        {
            Some(member.property.name.to_string())
        }
        _ => None,
    }
}

fn assignment_static_member_name<'a>(
    target: &'a AssignmentTarget<'a>,
) -> Option<(&'a str, &'a str)> {
    match target {
        AssignmentTarget::StaticMemberExpression(member) => {
            let object = expression_identifier(&member.object)?;
            Some((object, member.property.name.as_str()))
        }
        _ => None,
    }
}

fn prototype_member_assignment_name<'a>(
    target: &'a AssignmentTarget<'a>,
) -> Option<(&'a str, &'a str)> {
    let AssignmentTarget::StaticMemberExpression(member) = target else {
        return None;
    };
    let Expression::StaticMemberExpression(object) = &member.object else {
        return None;
    };
    if object.property.name != "prototype" {
        return None;
    }
    let constructor = expression_identifier(&object.object)?;
    Some((constructor, member.property.name.as_str()))
}

fn prototype_assignment_super_name<'a>(
    target: &'a AssignmentTarget<'a>,
    value: &'a Expression<'a>,
) -> Option<(&'a str, &'a str)> {
    let AssignmentTarget::StaticMemberExpression(member) = target else {
        return None;
    };
    if member.property.name != "prototype" {
        return None;
    }
    let constructor = expression_identifier(&member.object)?;
    let Expression::NewExpression(new_expression) = value else {
        return None;
    };
    let super_name = callee_identifier(&new_expression.callee)?;
    Some((constructor, super_name))
}

fn param_index_for_expression(
    expression: &Expression<'_>,
    params: &[ParamPattern],
) -> Option<usize> {
    let name = expression_identifier(expression)?;
    params
        .iter()
        .enumerate()
        .find_map(|(index, param)| match param {
            ParamPattern::Binding(param_name) if param_name == name => Some(index),
            _ => None,
        })
}

fn callback_argument_index(method: &str) -> Option<usize> {
    match method {
        "forEach" | "map" | "filter" | "find" | "findIndex" | "some" | "every" | "sort"
        | "flatMap" | "reduce" | "reduceRight" => Some(0),
        "from" => Some(1),
        _ => None,
    }
}

fn callback_targets_for_parameter(
    method: &str,
    index: usize,
    collection: &CollectionTargets,
) -> Vec<FunctionId> {
    match method {
        "forEach" if index <= 1 => collection.values.clone(),
        "reduce" | "reduceRight" if index == 1 => collection.values.clone(),
        _ if index == 0 => collection.values.clone(),
        _ => Vec::new(),
    }
}

fn iterator_result_object(values: &CollectionTargets) -> ObjectTargets {
    let mut result = ObjectTargets::default();
    for target in values.all_targets() {
        result.add_property_target("value".to_string(), target);
    }
    result
}

fn callback_param_patterns(expression: &Expression<'_>) -> Vec<ParamPattern> {
    match expression {
        Expression::ArrowFunctionExpression(function) => function
            .params
            .items
            .iter()
            .map(|param| param_pattern_from_binding(&param.pattern))
            .collect(),
        Expression::FunctionExpression(function) => function
            .params
            .items
            .iter()
            .map(|param| param_pattern_from_binding(&param.pattern))
            .collect(),
        Expression::ParenthesizedExpression(expression) => {
            callback_param_patterns(&expression.expression)
        }
        _ => Vec::new(),
    }
}

fn callback_returned_expression<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::ArrowFunctionExpression(function) => function
            .get_expression()
            .or_else(|| returned_expression_from_statements(&function.body.statements)),
        Expression::FunctionExpression(function) => function
            .body
            .as_deref()
            .and_then(|body| returned_expression_from_statements(&body.statements)),
        Expression::ParenthesizedExpression(expression) => {
            callback_returned_expression(&expression.expression)
        }
        _ => None,
    }
}

fn async_arrow_returned_expression<'a>(
    function: &'a oxc_ast::ast::ArrowFunctionExpression<'a>,
) -> Option<&'a Expression<'a>> {
    function
        .get_expression()
        .or_else(|| returned_expression_from_statements(&function.body.statements))
}

fn returned_expression_from_statements<'a>(
    statements: &'a oxc_allocator::Vec<'a, Statement<'a>>,
) -> Option<&'a Expression<'a>> {
    statements
        .iter()
        .find_map(returned_expression_from_statement)
}

fn returned_expression_from_statement<'a>(
    statement: &'a Statement<'a>,
) -> Option<&'a Expression<'a>> {
    match statement {
        Statement::ReturnStatement(statement) => statement.argument.as_ref(),
        Statement::BlockStatement(block) => returned_expression_from_statements(&block.body),
        Statement::IfStatement(statement) => {
            returned_expression_from_statement(&statement.consequent).or_else(|| {
                statement
                    .alternate
                    .as_ref()
                    .and_then(|alternate| returned_expression_from_statement(alternate))
            })
        }
        _ => None,
    }
}

fn callback_returned_function<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::ArrowFunctionExpression(function) => function
            .get_expression()
            .and_then(expression_as_function)
            .or_else(|| returned_function_from_statements(&function.body.statements)),
        Expression::FunctionExpression(function) => function
            .body
            .as_deref()
            .and_then(|body| returned_function_from_statements(&body.statements)),
        Expression::ParenthesizedExpression(expression) => {
            callback_returned_function(&expression.expression)
        }
        _ => None,
    }
}

fn returned_function_from_statements<'a>(
    statements: &'a oxc_allocator::Vec<'a, Statement<'a>>,
) -> Option<&'a Expression<'a>> {
    statements.iter().find_map(returned_function_from_statement)
}

fn returned_function_from_statement<'a>(
    statement: &'a Statement<'a>,
) -> Option<&'a Expression<'a>> {
    match statement {
        Statement::ReturnStatement(statement) => {
            statement.argument.as_ref().and_then(expression_as_function)
        }
        Statement::BlockStatement(block) => returned_function_from_statements(&block.body),
        Statement::IfStatement(statement) => {
            returned_function_from_statement(&statement.consequent).or_else(|| {
                statement
                    .alternate
                    .as_ref()
                    .and_then(|alternate| returned_function_from_statement(alternate))
            })
        }
        _ => None,
    }
}

fn expression_as_function<'a>(expression: &'a Expression<'a>) -> Option<&'a Expression<'a>> {
    match expression {
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_) => {
            Some(expression)
        }
        Expression::ParenthesizedExpression(expression) => {
            expression_as_function(&expression.expression)
        }
        _ => None,
    }
}

fn callback_parameter_names(expression: &Expression<'_>) -> Vec<String> {
    match expression {
        Expression::ArrowFunctionExpression(function) => function
            .params
            .items
            .iter()
            .filter_map(|param| binding_identifier_name(&param.pattern))
            .collect(),
        Expression::FunctionExpression(function) => function
            .params
            .items
            .iter()
            .filter_map(|param| binding_identifier_name(&param.pattern))
            .collect(),
        Expression::ParenthesizedExpression(expression) => {
            callback_parameter_names(&expression.expression)
        }
        _ => Vec::new(),
    }
}

fn argument_expression<'a>(argument: &'a Argument<'a>) -> Option<&'a Expression<'a>> {
    match argument {
        Argument::SpreadElement(spread) => Some(&spread.argument),
        _ => Some(argument.to_expression()),
    }
}

fn array_element_expression<'a>(
    element: &'a ArrayExpressionElement<'a>,
) -> Option<&'a Expression<'a>> {
    match element {
        ArrayExpressionElement::SpreadElement(spread) => Some(&spread.argument),
        ArrayExpressionElement::Elision(_) => None,
        _ => Some(element.to_expression()),
    }
}

fn expression_identifier<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.as_str()),
        Expression::ParenthesizedExpression(expression) => {
            expression_identifier(&expression.expression)
        }
        _ => None,
    }
}

fn numeric_index(expression: &Expression<'_>) -> Option<usize> {
    match expression {
        Expression::NumericLiteral(literal)
            if literal.value.is_finite() && literal.value.fract() == 0.0 =>
        {
            usize::try_from(literal.value as u64).ok()
        }
        Expression::ParenthesizedExpression(expression) => numeric_index(&expression.expression),
        _ => None,
    }
}

fn callee_identifier<'a>(expression: &'a Expression<'a>) -> Option<&'a str> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.as_str()),
        Expression::ParenthesizedExpression(expression) => {
            callee_identifier(&expression.expression)
        }
        _ => None,
    }
}

fn static_member_call<'a>(expression: &'a Expression<'a>) -> Option<(&'a str, &'a str)> {
    match expression {
        Expression::StaticMemberExpression(member) => {
            let object = expression_identifier(&member.object)?;
            Some((object, member.property.name.as_str()))
        }
        Expression::ParenthesizedExpression(expression) => {
            static_member_call(&expression.expression)
        }
        _ => None,
    }
}

fn static_property_key(key: &PropertyKey<'_>, computed: bool) -> Option<String> {
    if computed {
        return match key {
            PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
            PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
            _ => None,
        };
    }
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

fn class_method_function_span(method: &MethodDefinition<'_>) -> oxc_span::Span {
    if method.r#static && method.kind == MethodDefinitionKind::Method {
        return oxc_span::Span::new(method.key.span().start, method.span.end);
    }
    method.span
}

fn callee_text(expression: &Expression<'_>) -> Option<String> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.to_string()),
        Expression::StaticMemberExpression(member) => {
            callee_text(&member.object).map(|object| format!("{object}.{}", member.property.name))
        }
        Expression::ParenthesizedExpression(expression) => callee_text(&expression.expression),
        _ => None,
    }
}

fn value_flow_target_stable_key(
    db: &AnalysisDb,
    site: &CallSiteFact,
    target: FunctionId,
    binding: &str,
) -> String {
    let target_key = db
        .functions()
        .iter()
        .find(|function| function.id == target)
        .map(|function| {
            format!(
                "{}:{}:{}:{}:{}",
                function.name,
                function.file.0,
                function.span.start_line,
                function.span.start_col,
                function.span.start_byte
            )
        })
        .unwrap_or_else(|| format!("<missing-function:{}>", target.0));
    semantic_stable_key(
        FactFamily::CallTarget,
        &[
            ("site", site.stable_key.clone()),
            (
                "algorithm",
                format!("{:?}", CallAlgorithm::FunctionTokenFlow),
            ),
            ("binding", binding.to_string()),
            ("target", target_key),
            ("provider", crate::core::CALLS_PROVIDER_ID.to_string()),
            ("schema", "calls-facts-1:1".to_string()),
            ("model", "ts-js-collection-flow-v1".to_string()),
        ],
    )
    .into_string()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::analysis::calls::facts::{
        CallCallee, CallPrecision, CallSiteFact, CallSyntaxKind, CallTargetStatus,
    };
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId};
    use crate::core::{AnalysisDb, FunctionFact, FunctionId, Language, Span};

    use super::*;

    #[test]
    fn resolves_for_of_binding_calls_to_array_element_functions() {
        let source =
            "const fns = [\n  () => {},\n  () => {},\n];\nfor (const f of fns) {\n  f();\n}\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        let module = push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let first = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let second = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let site = call_site(source, file, module, "f()", "f");

        let targets = resolve_ts_value_flow_targets(&db, &[site], 0);
        let target_ids = resolved_target_ids(targets);

        assert_eq!(target_ids, vec![first, second]);
    }

    #[test]
    fn resolves_callback_parameter_calls_to_collection_value_functions() {
        let source = "const fns = [\n  () => {},\n];\nfns.forEach((f) => {\n  f();\n});\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let value = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let callback = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_between(source, file, "(f) =>", "\n});"),
        );
        let site = call_site(source, file, callback, "f()", "f");

        let targets = resolve_ts_value_flow_targets(&db, &[site], 0);
        let target_ids = resolved_target_ids(targets);

        assert_eq!(target_ids, vec![value]);
    }

    #[test]
    fn resolves_set_map_and_array_from_collection_constructors() {
        let source = "const base = [() => {}, () => {}];\nconst set = new Set(base);\nfor (const f of set) {\n  f();\n}\nconst map = new Map([[() => {}, () => {}]]);\nfor (const [k, v] of map) {\n  k();\n  v();\n}\nconst copy = Array.from(base);\nfor (const f of copy) {\n  f();\n}\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        let module = push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let base_first = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let base_second = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let map_key = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 2),
        );
        let map_value = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 3),
        );
        let sites = vec![
            call_site_with_id(source, file, module, "f()", "f", 0, 0),
            call_site_with_id(source, file, module, "k()", "k", 0, 1),
            call_site_with_id(source, file, module, "v()", "v", 0, 2),
            call_site_with_id(source, file, module, "f()", "f", 1, 3),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let mut target_ids_by_site = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        target_ids_by_site.sort();

        assert_eq!(
            target_ids_by_site,
            vec![
                (CallSiteId(0), base_first),
                (CallSiteId(0), base_second),
                (CallSiteId(1), map_key),
                (CallSiteId(2), map_value),
                (CallSiteId(3), base_first),
                (CallSiteId(3), base_second),
            ]
        );
    }

    #[test]
    fn resolves_array_from_mapper_return_as_result_collection_value() {
        let source = "const base = [() => {}];\nconst mapped = Array.from(base, function (element) {\n  return () => {};\n});\nfor (const f of mapped) {\n  f();\n}\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        let module = push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let returned = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let site = call_site(source, file, module, "f()", "f");

        let targets = resolve_ts_value_flow_targets(&db, &[site], 0);
        let target_ids = resolved_target_ids(targets);

        assert_eq!(target_ids, vec![returned]);
    }

    #[test]
    fn resolves_object_literal_member_calls() {
        let source = "const x = {\n  f: () => {},\n};\nx.f();\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        let module = push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let target = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let site = member_call_site(source, file, module, "x.f()", "f", 0, 0);

        let targets = resolve_ts_value_flow_targets(&db, &[site], 0);
        let target_ids = resolved_target_ids(targets);

        assert_eq!(target_ids, vec![target]);
    }

    #[test]
    fn resolves_module_this_assignment_through_object_literal_alias() {
        let source = "this.kk = () => {};\nconst k1 = {\n  a3: this,\n};\nk1.a3.kk();\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        let module = push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let target = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let site = member_call_site(source, file, module, "k1.a3.kk()", "kk", 0, 0);

        let targets = resolve_ts_value_flow_targets(&db, &[site], 0);
        let target_ids = resolved_target_ids(targets);

        assert_eq!(target_ids, vec![target]);
    }

    #[test]
    fn resolves_array_from_mapper_parameters_and_this_arg_member_calls() {
        let source = "const base = [() => {}];\nconst x = { f: () => {} };\nconst mapped = Array.from(base, function (element) {\n  element();\n  this.f();\n  return () => {};\n}, x);\nfor (const f of mapped) {\n  f();\n}\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        let module = push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let base_value = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let member_value = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let mapper = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_between(source, file, "function (element)", "\n}, x);"),
        );
        let returned = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 2),
        );
        let sites = vec![
            call_site_with_id(source, file, mapper, "element()", "element", 0, 0),
            member_call_site(source, file, mapper, "this.f()", "f", 0, 1),
            call_site_with_id(source, file, module, "f()", "f", 1, 2),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let mut target_ids_by_site = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        target_ids_by_site.sort();

        assert_eq!(
            target_ids_by_site,
            vec![
                (CallSiteId(0), base_value),
                (CallSiteId(1), member_value),
                (CallSiteId(2), returned),
            ]
        );
    }

    #[test]
    fn resolves_array_destructuring_rest_and_indexed_bindings() {
        let source = "var arr = [\n  () => {},\n  () => {},\n];\narr[2] = () => {};\nvar [a0, ...rest] = arr;\na0();\nvar a1 = rest[0];\nvar a2 = rest[1];\na1();\na2();\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        let module = push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let first = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let second = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let assigned = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 2),
        );
        let sites = vec![
            call_site_with_id(source, file, module, "a0()", "a0", 0, 0),
            call_site_with_id(source, file, module, "a1()", "a1", 0, 1),
            call_site_with_id(source, file, module, "a2()", "a2", 0, 2),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let mut target_ids_by_site = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        target_ids_by_site.sort();

        assert_eq!(
            target_ids_by_site,
            vec![
                (CallSiteId(0), first),
                (CallSiteId(1), second),
                (CallSiteId(2), assigned),
            ]
        );
    }

    #[test]
    fn resolves_direct_function_parameter_and_rest_argument_flows() {
        let source = "function run(first, ...rest) {\n  first();\n  rest[0]();\n  rest[1]();\n}\nrun(\n  () => {},\n  () => {},\n  () => {},\n);\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let run = push_function(
            &mut db,
            file,
            "run",
            span_between(source, file, "function run", "\nrun("),
        );
        let first = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let second = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let third = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 2),
        );
        let sites = vec![
            call_site_with_id(source, file, run, "first()", "first", 0, 0),
            index_call_site(source, file, run, "rest[0]()", 0, 1),
            index_call_site(source, file, run, "rest[1]()", 0, 2),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let mut target_ids_by_site = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        target_ids_by_site.sort();

        assert_eq!(
            target_ids_by_site,
            vec![
                (CallSiteId(0), first),
                (CallSiteId(1), second),
                (CallSiteId(2), third),
            ]
        );
    }

    #[test]
    fn resolves_object_destructuring_and_object_rest_parameter_flows() {
        let source = "const obj = {\n  e1: () => {},\n  e2: () => {},\n  e3: () => {},\n};\nconst { e1, e2: ee2, ...rest } = obj;\ne1();\nee2();\nrest.e3();\nfunction use({ e1: first, ...tail }) {\n  first();\n  tail.e3();\n}\nuse(obj);\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let first = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let second = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let third = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 2),
        );
        let use_function = push_function(
            &mut db,
            file,
            "use",
            span_between(source, file, "function use", "\nuse(obj);"),
        );
        let sites = vec![
            call_site_with_id(source, file, FunctionId(0), "e1()", "e1", 0, 0),
            call_site_with_id(source, file, FunctionId(0), "ee2()", "ee2", 0, 1),
            member_call_site(source, file, FunctionId(0), "rest.e3()", "e3", 0, 2),
            call_site_with_id(source, file, use_function, "first()", "first", 0, 3),
            member_call_site(source, file, use_function, "tail.e3()", "e3", 0, 4),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let mut target_ids_by_site = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        target_ids_by_site.sort();

        assert_eq!(
            target_ids_by_site,
            vec![
                (CallSiteId(0), first),
                (CallSiteId(1), second),
                (CallSiteId(2), third),
                (CallSiteId(3), first),
                (CallSiteId(4), third),
            ]
        );
    }

    #[test]
    fn resolves_promise_executor_resolve_into_then_parameter_call() {
        let source = "const p = new Promise((resolve, reject) => {\n  resolve(() => {});\n});\np.then(v => {\n  v();\n});\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let resolved = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let handler = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "v => {\n  v();\n}", 0),
        );
        let site = call_site(source, file, handler, "v()", "v");

        let targets = resolve_ts_value_flow_targets(&db, &[site], 0);
        let target_ids = resolved_target_ids(targets);

        assert_eq!(target_ids, vec![resolved]);
    }

    #[test]
    fn resolves_promise_then_and_catch_returned_values_into_chained_handlers() {
        let source = "const p = Promise.resolve(() => {});\np.then(v => {\n  v();\n  return () => {};\n}).then(next => {\n  next();\n});\nPromise.reject(() => {}).catch(err => {\n  err();\n  return () => {};\n}).then(value => {\n  value();\n});\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let resolved = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let returned_from_then = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let rejected = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 2),
        );
        let returned_from_catch = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 3),
        );
        let then_handler = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "v => {\n  v();\n  return () => {};\n}", 0),
        );
        let chained_then_handler = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "next => {\n  next();\n}", 0),
        );
        let catch_handler = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "err => {\n  err();\n  return () => {};\n}", 0),
        );
        let catch_then_handler = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "value => {\n  value();\n}", 0),
        );
        let sites = vec![
            call_site_with_id(source, file, then_handler, "v()", "v", 0, 0),
            call_site_with_id(source, file, chained_then_handler, "next()", "next", 0, 1),
            call_site_with_id(source, file, catch_handler, "err()", "err", 0, 2),
            call_site_with_id(source, file, catch_then_handler, "value()", "value", 0, 3),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let mut target_ids_by_site = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        target_ids_by_site.sort();

        assert_eq!(
            target_ids_by_site,
            vec![
                (CallSiteId(0), resolved),
                (CallSiteId(1), returned_from_then),
                (CallSiteId(2), rejected),
                (CallSiteId(3), returned_from_catch),
            ]
        );
    }

    #[test]
    fn resolves_promise_handlers_inside_for_of_blocks() {
        let source = "const p2 = Promise.resolve(() => {});\nfor (const round of [1, 2, 3]) {\n  const p1 = new Promise((resolve, reject) => {\n    switch (round) {\n      case 1:\n        resolve(() => {});\n        break;\n      case 2:\n        reject(() => {});\n        break;\n      case 3:\n        resolve(p2);\n        break;\n    }\n  });\n  p1.then(a => {\n    a();\n  }, b => {\n    b();\n  });\n}\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let p2_value = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let resolved = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let rejected = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 2),
        );
        let fulfilled_handler = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "a => {\n    a();\n  }", 0),
        );
        let rejected_handler = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "b => {\n    b();\n  }", 0),
        );
        let sites = vec![
            call_site_with_id(source, file, fulfilled_handler, "a()", "a", 0, 0),
            call_site_with_id(source, file, rejected_handler, "b()", "b", 0, 1),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let mut target_ids_by_site = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        target_ids_by_site.sort();

        assert_eq!(
            target_ids_by_site,
            vec![
                (CallSiteId(0), p2_value),
                (CallSiteId(0), resolved),
                (CallSiteId(1), rejected),
            ]
        );
    }

    #[test]
    fn resolves_async_iife_awaited_values_and_async_function_returns() {
        let source = "(async function() {\n  const p = Promise.resolve(() => {});\n  const t1 = await p;\n  t1();\n\n  const t2 = await (() => {});\n  t2();\n\n  const f1 = async function() {\n    return () => {};\n  };\n  const f2 = await f1();\n  f2();\n\n  const f3 = async function() {\n    return () => {};\n  };\n  f3().then(f4 => {\n    f4();\n  });\n}());\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let iife_start = source
            .find("async function()")
            .expect("async IIFE start exists");
        let iife_end = source.rfind("}());").expect("async IIFE end exists") + 1;
        let iife = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_range(source, file, iife_start, iife_end),
        );
        let resolved = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let awaited_value = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let f1 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(
                source,
                file,
                "async function() {\n    return () => {};\n  }",
                0,
            ),
        );
        let returned_from_f1 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 2),
        );
        let f3 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(
                source,
                file,
                "async function() {\n    return () => {};\n  }",
                1,
            ),
        );
        let returned_from_f3 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 3),
        );
        let handler = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "f4 => {\n    f4();\n  }", 0),
        );
        let sites = vec![
            call_site_with_id(source, file, iife, "t1()", "t1", 0, 0),
            call_site_with_id(source, file, iife, "t2()", "t2", 0, 1),
            call_site_with_id(source, file, iife, "f1()", "f1", 0, 2),
            call_site_with_id(source, file, iife, "f2()", "f2", 0, 3),
            call_site_with_id(source, file, iife, "f3()", "f3", 0, 4),
            call_site_with_id(source, file, handler, "f4()", "f4", 0, 5),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let mut target_ids_by_site = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        target_ids_by_site.sort();

        assert_eq!(
            target_ids_by_site,
            vec![
                (CallSiteId(0), resolved),
                (CallSiteId(1), awaited_value),
                (CallSiteId(2), f1),
                (CallSiteId(3), returned_from_f1),
                (CallSiteId(4), f3),
                (CallSiteId(5), returned_from_f3),
            ]
        );
    }

    #[test]
    fn resolves_compact_async_iife_shapes_used_by_jelly() {
        let source = "(async function() {\n    const t2 = await (()=>{console.log(\"value\");});\n    t2();\n\n    const f1 = async function() {\n        return ()=>{console.log(\"async1\");};\n    }\n    const f2 = await f1();\n    f2();\n\n    const f3 = async function() {\n        return ()=>{console.log(\"async2\");};\n    }\n    f3().then(f4 => {\n        f4();\n    });\n}());\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let iife_start = source
            .find("async function()")
            .expect("async IIFE start exists");
        let iife_end = source.rfind("}());").expect("async IIFE end exists") + 1;
        let iife = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_range(source, file, iife_start, iife_end),
        );
        let awaited_value = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "()=>{console.log(\"value\");}", 0),
        );
        let f1 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(
                source,
                file,
                "async function() {\n        return ()=>{console.log(\"async1\");};\n    }",
                0,
            ),
        );
        let returned_from_f1 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "()=>{console.log(\"async1\");}", 0),
        );
        let f3 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(
                source,
                file,
                "async function() {\n        return ()=>{console.log(\"async2\");};\n    }",
                0,
            ),
        );
        let returned_from_f3 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "()=>{console.log(\"async2\");}", 0),
        );
        let handler = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "f4 => {\n        f4();\n    }", 0),
        );
        let sites = vec![
            call_site_with_id(source, file, iife, "t2()", "t2", 0, 0),
            call_site_with_id(source, file, iife, "f1()", "f1", 0, 1),
            call_site_with_id(source, file, iife, "f2()", "f2", 0, 2),
            call_site_with_id(source, file, iife, "f3()", "f3", 0, 3),
            call_site_with_id(source, file, handler, "f4()", "f4", 0, 4),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let mut target_ids_by_site = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        target_ids_by_site.sort();

        assert_eq!(
            target_ids_by_site,
            vec![
                (CallSiteId(0), awaited_value),
                (CallSiteId(1), f1),
                (CallSiteId(2), returned_from_f1),
                (CallSiteId(3), f3),
                (CallSiteId(4), returned_from_f3),
            ]
        );
    }

    #[test]
    fn resolves_class_instance_static_inherited_and_alias_member_calls() {
        let source = "class Base {\n  base(){}\n  static sb(){}\n}\nclass D extends Base {\n  constructor(a) { super(); this.a = a; }\n  field = () => {};\n  self = this;\n  method(){}\n  static sm(){}\n  static alias = this;\n  static { this.block = () => {}; }\n}\nconst f = () => {};\nconst x = new D(f);\nx.a();\nx.field();\nx.method();\nx.base();\nD.sm();\nD.sb();\nD.block();\nD.alias.sm();\nx.self.method();\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let base = push_function(
            &mut db,
            file,
            "Base.base",
            span_for_nth(source, file, "base(){}", 0),
        );
        let static_base = push_function(
            &mut db,
            file,
            "Base.sb",
            span_for_nth(source, file, "static sb(){}", 0),
        );
        let field = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let method = push_function(
            &mut db,
            file,
            "D.method",
            span_for_nth(source, file, "method(){}", 0),
        );
        let static_method = push_function(
            &mut db,
            file,
            "D.sm",
            span_for_nth(source, file, "static sm(){}", 0),
        );
        let static_block = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let argument = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 2),
        );
        let sites = vec![
            member_call_site(source, file, FunctionId(0), "x.a()", "a", 0, 0),
            member_call_site(source, file, FunctionId(0), "x.field()", "field", 0, 1),
            member_call_site(source, file, FunctionId(0), "x.method()", "method", 0, 2),
            member_call_site(source, file, FunctionId(0), "x.base()", "base", 0, 3),
            member_call_site(source, file, FunctionId(0), "D.sm()", "sm", 0, 4),
            member_call_site(source, file, FunctionId(0), "D.sb()", "sb", 0, 5),
            member_call_site(source, file, FunctionId(0), "D.block()", "block", 0, 6),
            member_call_site(source, file, FunctionId(0), "D.alias.sm()", "sm", 0, 7),
            member_call_site(
                source,
                file,
                FunctionId(0),
                "x.self.method()",
                "method",
                0,
                8,
            ),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let mut target_ids_by_site = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        target_ids_by_site.sort();

        assert_eq!(
            target_ids_by_site,
            vec![
                (CallSiteId(0), argument),
                (CallSiteId(1), field),
                (CallSiteId(2), method),
                (CallSiteId(3), base),
                (CallSiteId(4), static_method),
                (CallSiteId(5), static_base),
                (CallSiteId(6), static_block),
                (CallSiteId(7), static_method),
                (CallSiteId(8), method),
            ]
        );
    }

    #[test]
    fn resolves_function_constructor_prototype_and_static_member_calls() {
        let source = "function F1() {\n  this.q1 = () => {};\n}\nF1.s1 = () => {};\nfunction F2() {\n  this.q2 = () => {};\n}\nF2.s2 = () => {};\nF2.prototype = new F1();\nconst x = new F2();\nx.q1();\nx.q2();\nF1.s1();\nF2.s2();\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        push_function(
            &mut db,
            file,
            "F1",
            span_between(source, file, "function F1", "\nF1.s1"),
        );
        let q1 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let s1 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        push_function(
            &mut db,
            file,
            "F2",
            span_between(source, file, "function F2", "\nF2.s2"),
        );
        let q2 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 2),
        );
        let s2 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 3),
        );
        let sites = vec![
            member_call_site(source, file, FunctionId(0), "x.q1()", "q1", 0, 0),
            member_call_site(source, file, FunctionId(0), "x.q2()", "q2", 0, 1),
            member_call_site(source, file, FunctionId(0), "F1.s1()", "s1", 0, 2),
            member_call_site(source, file, FunctionId(0), "F2.s2()", "s2", 0, 3),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let mut target_ids_by_site = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        target_ids_by_site.sort();

        assert_eq!(
            target_ids_by_site,
            vec![
                (CallSiteId(0), q1),
                (CallSiteId(1), q2),
                (CallSiteId(2), s1),
                (CallSiteId(3), s2),
            ]
        );
    }

    #[test]
    fn real_ts_pipeline_resolves_function_constructor_static_and_instance_calls() {
        let source = "function F1() {\n    this.q1 = () => {console.log(\"q1\")};\n}\nF1.s1 = () => {console.log(\"s1\")};\nfunction F2() {\n    this.q2 = () => {console.log(\"q2\")};\n}\nF2.s2 = () => {console.log(\"s2\")};\nF2.prototype = new F1;\nconst x2 = new F2();\nx2.q1();\nx2.q2();\nF1.s1();\nF2.s2();\n";
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);

        for (call, target) in [
            ("x2.q1()", "() => {console.log(\"q1\")}"),
            ("x2.q2()", "() => {console.log(\"q2\")}"),
            ("F1.s1()", "() => {console.log(\"s1\")}"),
            ("F2.s2()", "() => {console.log(\"s2\")}"),
        ] {
            let site = site_id_for_span(source, &sites, call);
            let target = function_id_for_span(source, &db, target);
            assert!(
                resolved.contains(&(site, target)),
                "expected real TS pipeline to resolve {call} to {target:?}; got {resolved:?}"
            );
        }
    }

    #[test]
    fn real_ts_pipeline_reports_static_class_method_spans_from_method_name() {
        let source = "class D {\n    static m2() {console.log(\"m2\")}\n}\nD.m2();\n";
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let file = db.files()[0].id;
        let expected_span = span_for_nth(source, file, "m2() {console.log(\"m2\")}", 0);
        let (method_id, actual_span) = db
            .functions()
            .iter()
            .find(|function| function.name == "D.m2")
            .map(|function| (function.id, function.span.clone()))
            .expect("static class method function fact exists");

        assert_eq!(actual_span, expected_span);

        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);
        let site = site_id_for_span(source, &sites, "D.m2()");

        assert!(
            resolved.contains(&(site, method_id)),
            "expected real TS pipeline to resolve D.m2() to {method_id:?}; got {resolved:?}"
        );
    }

    #[test]
    fn jelly_gap_promise_all_settled_result_object_properties() {
        let source = "const p2 = new Promise((resolve, reject) => {\n  resolve(() => {\n    console.log(\"p2resolve\");\n  });\n});\nconst p3 = new Promise((resolve, reject) => {\n  reject(() => {\n    console.log(\"p3reject\");\n  });\n});\nPromise.allSettled([p2, p3]).then(\n  va => {\n    va[0].value();\n    va[1].reason();\n  }\n);\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let fulfilled = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(
                source,
                file,
                "() => {\n    console.log(\"p2resolve\");\n  }",
                0,
            ),
        );
        let rejected = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(
                source,
                file,
                "() => {\n    console.log(\"p3reject\");\n  }",
                0,
            ),
        );
        let handler = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(
                source,
                file,
                "va => {\n    va[0].value();\n    va[1].reason();\n  }",
                0,
            ),
        );
        let sites = vec![
            member_call_site(source, file, handler, "va[0].value()", "value", 0, 0),
            member_call_site(source, file, handler, "va[1].reason()", "reason", 0, 1),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let target_ids_by_site = resolved_target_ids_by_site(targets);

        assert_eq!(
            target_ids_by_site,
            vec![(CallSiteId(0), fulfilled), (CallSiteId(1), rejected)]
        );
    }

    #[test]
    fn jelly_gap_async_generator_next_and_for_await_values() {
        let source = "(async function() {\n  const f7 = async function*() {\n    yield* [() => {}];\n    return () => {};\n  };\n  const f8 = f7();\n  const p2 = f8.next();\n  p2.then(res => {\n    res.value();\n  });\n  for await (const q of f7()) {\n    q();\n  }\n}());\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        let iife_start = source
            .find("async function()")
            .expect("async IIFE start exists");
        let iife_end = source.rfind("}());").expect("async IIFE end exists") + 1;
        let iife = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_range(source, file, iife_start, iife_end),
        );
        push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(
                source,
                file,
                "async function*() {\n    yield* [() => {}];\n    return () => {};\n  }",
                0,
            ),
        );
        let yielded = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 1),
        );
        let handler = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "res => {\n    res.value();\n  }", 0),
        );
        let sites = vec![
            member_call_site(source, file, handler, "res.value()", "value", 0, 0),
            call_site_with_id(source, file, iife, "q()", "q", 0, 1),
        ];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let target_ids_by_site = resolved_target_ids_by_site(targets);

        assert_eq!(
            target_ids_by_site,
            vec![(CallSiteId(0), yielded), (CallSiteId(1), yielded)]
        );
    }

    #[test]
    fn jelly_gap_receiver_side_effect_adds_instance_method() {
        let source = "class D {\n  constructor(a) {\n    this.a1 = a;\n  }\n}\nconst q1 = new D(function() {\n  this.a2 = () => {};\n});\nq1.a1();\nq1.a2();\n";
        let mut db = db_with_file(source);
        let file = db.files()[0].id;
        let module = push_function(
            &mut db,
            file,
            TS_JS_MODULE_FUNCTION_NAME,
            span_for_range(source, file, 0, source.len()),
        );
        push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "function() {\n  this.a2 = () => {};\n}", 0),
        );
        let a2 = push_function(
            &mut db,
            file,
            "<anonymous>",
            span_for_nth(source, file, "() => {}", 0),
        );
        let sites = vec![member_call_site(
            source, file, module, "q1.a2()", "a2", 0, 0,
        )];

        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let target_ids_by_site = resolved_target_ids_by_site(targets);

        assert_eq!(target_ids_by_site, vec![(CallSiteId(0), a2)]);
    }

    fn db_with_file(source: &str) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from("src/sample.js"),
            "src/sample.js".to_string(),
            source.to_string(),
        );
        db
    }

    fn push_function(db: &mut AnalysisDb, file: FileId, name: &str, span: Span) -> FunctionId {
        db.push_function(FunctionFact {
            id: FunctionId(u64::MAX),
            file,
            name: name.to_string(),
            span,
            language: Language::JavaScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        })
    }

    fn call_site(
        source: &str,
        file: FileId,
        caller: FunctionId,
        pattern: &str,
        callee_name: &str,
    ) -> CallSiteFact {
        call_site_with_id(source, file, caller, pattern, callee_name, 0, 0)
    }

    fn call_site_with_id(
        source: &str,
        file: FileId,
        caller: FunctionId,
        pattern: &str,
        callee_name: &str,
        ordinal: usize,
        id: u64,
    ) -> CallSiteFact {
        CallSiteFact {
            id: CallSiteId(id),
            language: Language::JavaScript,
            file,
            caller,
            owner_symbol: None,
            body: MirBodyId(id),
            operation: MirOpId(id),
            span: span_for_nth(source, file, pattern, ordinal),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: None,
                name: callee_name.to_string(),
            },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Unresolved,
            precision: CallPrecision::Conservative,
            stable_key: format!("site:{callee_name}:{}:{id}", caller.0),
        }
    }

    fn member_call_site(
        source: &str,
        file: FileId,
        caller: FunctionId,
        pattern: &str,
        property: &str,
        ordinal: usize,
        id: u64,
    ) -> CallSiteFact {
        CallSiteFact {
            id: CallSiteId(id),
            language: Language::JavaScript,
            file,
            caller,
            owner_symbol: None,
            body: MirBodyId(id),
            operation: MirOpId(id),
            span: span_for_nth(source, file, pattern, ordinal),
            kind: CallSyntaxKind::Member,
            callee: CallCallee::Member {
                base: PlaceId(u64::MAX),
                property: property.to_string(),
            },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Unresolved,
            precision: CallPrecision::Conservative,
            stable_key: format!("site:member:{property}:{}:{id}", caller.0),
        }
    }

    fn index_call_site(
        source: &str,
        file: FileId,
        caller: FunctionId,
        pattern: &str,
        ordinal: usize,
        id: u64,
    ) -> CallSiteFact {
        CallSiteFact {
            id: CallSiteId(id),
            language: Language::JavaScript,
            file,
            caller,
            owner_symbol: None,
            body: MirBodyId(id),
            operation: MirOpId(id),
            span: span_for_nth(source, file, pattern, ordinal),
            kind: CallSyntaxKind::Index,
            callee: CallCallee::Index {
                base: PlaceId(u64::MAX),
                index: None,
            },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Unresolved,
            precision: CallPrecision::Conservative,
            stable_key: format!("site:index:{}:{id}", caller.0),
        }
    }

    fn resolved_target_ids(mut targets: Vec<CallTargetFact>) -> Vec<FunctionId> {
        let mut ids = targets
            .drain(..)
            .filter_map(|target| target.target_function)
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    fn resolved_target_ids_by_site(targets: Vec<CallTargetFact>) -> Vec<(CallSiteId, FunctionId)> {
        let mut ids = targets
            .into_iter()
            .filter_map(|target| {
                target
                    .target_function
                    .map(|function| (target.site, function))
            })
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    fn span_for_nth(source: &str, file: FileId, pattern: &str, ordinal: usize) -> Span {
        let start = source
            .match_indices(pattern)
            .nth(ordinal)
            .map(|(start, _)| start)
            .unwrap_or_else(|| panic!("pattern {pattern:?} not found at ordinal {ordinal}"));
        span_for_range(source, file, start, start + pattern.len())
    }

    fn span_between(source: &str, file: FileId, start_pattern: &str, end_pattern: &str) -> Span {
        let start = source
            .find(start_pattern)
            .unwrap_or_else(|| panic!("start pattern {start_pattern:?} not found"));
        let end = source[start..]
            .find(end_pattern)
            .map(|offset| start + offset)
            .unwrap_or_else(|| panic!("end pattern {end_pattern:?} not found"));
        span_for_range(source, file, start, end)
    }

    fn site_id_for_span(source: &str, sites: &[CallSiteFact], needle: &str) -> CallSiteId {
        let file = sites
            .first()
            .map(|site| site.file)
            .expect("at least one call site");
        let span = span_for_nth(source, file, needle, 0);
        sites
            .iter()
            .find(|site| {
                site.span.start_byte == span.start_byte && site.span.end_byte == span.end_byte
            })
            .map(|site| site.id)
            .unwrap_or_else(|| panic!("missing call site for {needle}"))
    }

    fn function_id_for_span(source: &str, db: &AnalysisDb, needle: &str) -> FunctionId {
        let file = db.files()[0].id;
        let span = span_for_nth(source, file, needle, 0);
        db.functions()
            .iter()
            .find(|function| {
                function.span.start_byte == span.start_byte
                    && function.span.end_byte == span.end_byte
            })
            .map(|function| function.id)
            .unwrap_or_else(|| panic!("missing function for {needle}"))
    }

    fn span_for_range(source: &str, file: FileId, start: usize, end: usize) -> Span {
        let (start_line, start_col) = line_col(source, start);
        let (end_line, end_col) = line_col(source, end);
        Span {
            file,
            start_byte: start as u32,
            end_byte: end as u32,
            start_line,
            start_col,
            end_line,
            end_col,
        }
    }

    fn line_col(source: &str, byte: usize) -> (u32, u32) {
        let prefix = &source[..byte];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32 + 1;
        let col = prefix
            .rsplit('\n')
            .next()
            .map(|line| line.len() as u32 + 1)
            .unwrap_or(1);
        (line, col)
    }
}
