//! AST → [`super::solver`] constraint harvest.
//!
//! Walks the oxc AST of one file, resolving lexical bindings through a scope
//! chain into solver cells and emitting inclusion constraints. The call sites it
//! records are mapped back to the kernel's `CallSiteFact`s by span in
//! [`super::provider`]. v1 is single-file (modules are wired in a later phase);
//! it deliberately resolves only what it can prove — an identifier with no
//! binding becomes a fresh empty cell (no tokens, no edges), never a guess.

use std::collections::BTreeMap;

use crate::core::{FileId, FunctionId};
use oxc_ast::ast::{
    Argument, BindingPattern, Expression, FunctionBody, MethodDefinitionKind, ObjectPropertyKind,
    Program, PropertyKey, Statement, VariableDeclarator,
};

use super::solver::{CallId, CellId, Constraint, FunctionCells, PointsToProgram, Token};

/// Recursion ceiling for the AST walk. The harvester runs over every JS/TS file
/// in an arbitrary repo, so adversarial / generated input (deeply nested
/// parentheses, thousand-operand string concatenations, nested destructuring)
/// must not overflow the stack. Past this depth the walk bails to an empty cell
/// (no tokens, no edges) — a soundness-preserving give-up, mirroring the
/// `invocation_depth` guard in the sibling `ts_value_flows` recognizer.
const MAX_HARVEST_DEPTH: usize = 256;

/// A call site recorded during harvest, mapped to real `CallSiteFact`s by span
/// during emission. `callee_hint` disambiguates which extracted site(s) a call
/// expression owns (an identifier name, or a member property name).
#[derive(Clone, Debug)]
pub(crate) struct CallRecord {
    pub(crate) file: FileId,
    pub(crate) start: u32,
    pub(crate) end: u32,
    pub(crate) hint: CalleeHint,
}

#[derive(Clone, Debug)]
pub(crate) enum CalleeHint {
    Ident(String),
    Member(String),
    Other,
}

/// Lexical-scope frame: name → cell.
type Scope = BTreeMap<String, CellId>;

struct FnFrame {
    return_cell: CellId,
    this_cell: Option<CellId>,
}

/// Whole-program harvester. One instance accumulates constraints across files
/// into a single [`PointsToProgram`] (so cross-file flow is just more
/// constraints once modules are wired).
pub(crate) struct Harvester<'a> {
    pub(crate) program: PointsToProgram,
    pub(crate) calls: Vec<CallRecord>,
    /// `(file, span.start, span.end) -> FunctionId` for resolving a function
    /// literal/declaration to its kernel identity (the edge target payload).
    function_id_by_span: &'a BTreeMap<(FileId, u32, u32), FunctionId>,
    scopes: Vec<Scope>,
    /// Constant string bindings (`const k = "qwe"`), per current resolution — used
    /// to resolve a computed member key `obj[k]` to a static field.
    string_consts: BTreeMap<String, String>,
    fn_stack: Vec<FnFrame>,
    file: FileId,
    /// `(importer file, specifier) -> resolved target file`, for `require`/`import`.
    resolution_map: &'a BTreeMap<(FileId, String), FileId>,
    /// One cell per file standing for its `module.exports` object (lazily made,
    /// seeded with a module-object token so property writes have a base).
    module_exports: BTreeMap<FileId, CellId>,
    /// Current AST recursion depth, bounded by [`MAX_HARVEST_DEPTH`].
    depth: usize,
}

impl<'a> Harvester<'a> {
    pub(crate) fn new(
        function_id_by_span: &'a BTreeMap<(FileId, u32, u32), FunctionId>,
        resolution_map: &'a BTreeMap<(FileId, String), FileId>,
    ) -> Self {
        Self {
            program: PointsToProgram::new(),
            calls: Vec::new(),
            function_id_by_span,
            scopes: Vec::new(),
            string_consts: BTreeMap::new(),
            fn_stack: Vec::new(),
            file: FileId(0),
            resolution_map,
            module_exports: BTreeMap::new(),
            depth: 0,
        }
    }

    /// The cell standing for `file`'s `module.exports` object, created on demand
    /// and seeded with a stable module-object token.
    fn module_exports_cell(&mut self, file: FileId) -> CellId {
        if let Some(cell) = self.module_exports.get(&file) {
            return *cell;
        }
        let cell = self.program.fresh_cell();
        // Distinct from allocation-site tokens (which pack file<<32|start, never
        // setting the high bit).
        let token = Token::Object((1u64 << 63) | file.0 as u64);
        self.program.add(Constraint::Alloc { token, into: cell });
        self.module_exports.insert(file, cell);
        cell
    }

    /// Harvest one file's module-level program into the shared constraint set.
    pub(crate) fn harvest_file(&mut self, file: FileId, program: &Program<'_>) {
        self.file = file;
        self.scopes.push(Scope::new());
        self.hoist(&program.body);
        for statement in &program.body {
            self.statement(statement);
        }
        self.scopes.pop();
    }

    // ---- scope helpers ---------------------------------------------------

    fn bind(&mut self, name: &str, cell: CellId) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), cell);
        }
    }

    /// Resolve an identifier to its binding cell, searching innermost-out. An
    /// unbound name gets a fresh empty cell (an unknown value — no tokens, so it
    /// produces no edges).
    fn lookup(&mut self, name: &str) -> CellId {
        for scope in self.scopes.iter().rev() {
            if let Some(cell) = scope.get(name) {
                return *cell;
            }
        }
        let cell = self.program.fresh_cell();
        self.bind(name, cell);
        cell
    }

    /// Pre-bind function declarations (hoisting) so forward references resolve.
    fn hoist(&mut self, statements: &[Statement<'_>]) {
        for statement in statements {
            if let Statement::FunctionDeclaration(function) = statement
                && let Some(id) = function.id.as_ref()
            {
                let cell = self.program.fresh_cell();
                self.bind(id.name.as_str(), cell);
            }
        }
    }

    // ---- statements ------------------------------------------------------

    fn statement(&mut self, statement: &Statement<'_>) {
        if self.depth > MAX_HARVEST_DEPTH {
            return;
        }
        self.depth += 1;
        self.statement_inner(statement);
        self.depth -= 1;
    }

    fn statement_inner(&mut self, statement: &Statement<'_>) {
        match statement {
            Statement::VariableDeclaration(decl) => {
                for declarator in &decl.declarations {
                    self.declarator(declarator);
                }
            }
            Statement::FunctionDeclaration(function) => {
                let Some(id) = function.id.as_ref() else {
                    return;
                };
                let name_cell = self.lookup(id.name.as_str());
                if let Some(fcell) =
                    self.function_value(function.span, &function.params, function.body.as_deref())
                {
                    self.program.add(Constraint::Subset {
                        from: fcell,
                        to: name_cell,
                    });
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(id) = class.id.as_ref() {
                    let name_cell = self.lookup(id.name.as_str());
                    let ccell = self.class_value(class, Some(id.name.as_str()));
                    self.program.add(Constraint::Subset {
                        from: ccell,
                        to: name_cell,
                    });
                }
            }
            Statement::ExpressionStatement(stmt) => {
                self.expr(&stmt.expression);
            }
            Statement::ReturnStatement(stmt) => {
                if let Some(argument) = &stmt.argument {
                    let value = self.expr(argument);
                    if let Some(frame) = self.fn_stack.last() {
                        let ret = frame.return_cell;
                        self.program.add(Constraint::Subset {
                            from: value,
                            to: ret,
                        });
                    }
                }
            }
            Statement::BlockStatement(block) => {
                for statement in &block.body {
                    self.statement(statement);
                }
            }
            Statement::IfStatement(stmt) => {
                self.expr(&stmt.test);
                self.statement(&stmt.consequent);
                if let Some(alternate) = &stmt.alternate {
                    self.statement(alternate);
                }
            }
            Statement::ForStatement(stmt) => {
                self.statement(&stmt.body);
            }
            Statement::ForOfStatement(stmt) => {
                self.statement(&stmt.body);
            }
            Statement::ForInStatement(stmt) => {
                self.statement(&stmt.body);
            }
            Statement::WhileStatement(stmt) => {
                self.statement(&stmt.body);
            }
            Statement::DoWhileStatement(stmt) => {
                self.statement(&stmt.body);
            }
            // Try/catch/switch/labeled bodies contain calls too — without these
            // arms every call inside them is invisible (common in dependency code).
            Statement::TryStatement(stmt) => {
                for statement in &stmt.block.body {
                    self.statement(statement);
                }
                if let Some(handler) = &stmt.handler {
                    for statement in &handler.body.body {
                        self.statement(statement);
                    }
                }
                if let Some(finalizer) = &stmt.finalizer {
                    for statement in &finalizer.body {
                        self.statement(statement);
                    }
                }
            }
            Statement::SwitchStatement(stmt) => {
                self.expr(&stmt.discriminant);
                for case in &stmt.cases {
                    for statement in &case.consequent {
                        self.statement(statement);
                    }
                }
            }
            Statement::LabeledStatement(stmt) => {
                self.statement(&stmt.body);
            }
            _ => {}
        }
    }

    fn declarator(&mut self, declarator: &VariableDeclarator<'_>) {
        // Record a constant string binding for computed-key resolution.
        if let Some(name) = binding_name(&declarator.id)
            && let Some(Expression::StringLiteral(literal)) = &declarator.init
        {
            self.string_consts.insert(name, literal.value.to_string());
        }
        // Always evaluate the initializer (it may contain calls), then bind the
        // pattern — a plain name, or object destructuring `const {a, b} = init`.
        let Some(init) = &declarator.init else {
            return;
        };
        let value = self.expr(init);
        self.bind_pattern(&declarator.id, value);
    }

    /// Bind a declaration/parameter pattern to the cell holding its value.
    /// Object destructuring loads each property; array/default patterns degrade
    /// gracefully (the binding gets a fresh empty cell rather than a wrong value).
    fn bind_pattern(&mut self, pattern: &BindingPattern<'_>, value: CellId) {
        if self.depth > MAX_HARVEST_DEPTH {
            return;
        }
        self.depth += 1;
        self.bind_pattern_inner(pattern, value);
        self.depth -= 1;
    }

    fn bind_pattern_inner(&mut self, pattern: &BindingPattern<'_>, value: CellId) {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => {
                let cell = self.lookup(identifier.name.as_str());
                self.program.add(Constraint::Subset {
                    from: value,
                    to: cell,
                });
            }
            BindingPattern::ObjectPattern(object) => {
                for property in &object.properties {
                    if let Some(field) = property_key_name(&property.key) {
                        let into = self.program.fresh_cell();
                        self.program.add(Constraint::FieldLoad {
                            base: value,
                            field,
                            into,
                        });
                        self.bind_pattern(&property.value, into);
                    }
                }
            }
            BindingPattern::AssignmentPattern(assignment) => {
                // `{a = default}` — bind the target to the value (ignore the default).
                self.bind_pattern(&assignment.left, value);
            }
            _ => {}
        }
    }

    // ---- expressions -----------------------------------------------------

    /// Produce a cell holding the expression's value.
    fn expr(&mut self, expression: &Expression<'_>) -> CellId {
        if self.depth > MAX_HARVEST_DEPTH {
            return self.program.fresh_cell();
        }
        self.depth += 1;
        let cell = self.expr_inner(expression);
        self.depth -= 1;
        cell
    }

    fn expr_inner(&mut self, expression: &Expression<'_>) -> CellId {
        match expression {
            Expression::Identifier(identifier) => {
                if identifier.name == "exports" {
                    self.module_exports_cell(self.file)
                } else {
                    self.lookup(identifier.name.as_str())
                }
            }
            Expression::ParenthesizedExpression(inner) => self.expr(&inner.expression),
            Expression::FunctionExpression(function) => self
                .function_value(function.span, &function.params, function.body.as_deref())
                .unwrap_or_else(|| self.program.fresh_cell()),
            Expression::ArrowFunctionExpression(arrow) => self.arrow_value(arrow),
            Expression::ClassExpression(class) => {
                let name = class.id.as_ref().map(|id| id.name.to_string());
                self.class_value(class, name.as_deref())
            }
            Expression::ObjectExpression(object) => {
                let token = Token::Object(self.alloc_site(object.span));
                let cell = self.program.fresh_cell();
                self.program.add(Constraint::Alloc { token, into: cell });
                for property in &object.properties {
                    if let ObjectPropertyKind::ObjectProperty(prop) = property
                        // Skip getters/setters: in JS a read/write of an accessor
                        // INVOKES it, it does not load/store the function. Modeling
                        // them as plain properties resolves `t1 = obj.foo; t1()` to
                        // the getter (a false positive). Accessor semantics are a
                        // later phase; for now we drop them to protect precision.
                        && prop.kind == oxc_ast::ast::PropertyKind::Init
                        && let Some(field) = property_key_name(&prop.key)
                    {
                        let value = self.expr(&prop.value);
                        self.program.add(Constraint::FieldStore {
                            base: cell,
                            field,
                            src: value,
                        });
                    }
                }
                cell
            }
            Expression::StaticMemberExpression(member) => {
                // `module.exports` reads as the module's exports object.
                if is_module_dot_exports(expression) {
                    return self.module_exports_cell(self.file);
                }
                let base = self.expr(&member.object);
                let into = self.program.fresh_cell();
                self.program.add(Constraint::FieldLoad {
                    base,
                    field: member.property.name.to_string(),
                    into,
                });
                into
            }
            Expression::ComputedMemberExpression(member) => {
                let base = self.expr(&member.object);
                let into = self.program.fresh_cell();
                if let Some(field) = self.const_key(&member.expression) {
                    self.program
                        .add(Constraint::FieldLoad { base, field, into });
                }
                into
            }
            Expression::CallExpression(call) => self.call_value(call),
            Expression::NewExpression(new) => self.new_value(new),
            Expression::ThisExpression(_) => self
                .fn_stack
                .last()
                .and_then(|frame| frame.this_cell)
                .unwrap_or_else(|| self.program.fresh_cell()),
            Expression::AssignmentExpression(assignment) => {
                self.assignment(assignment);
                // The value of an assignment expression is its right-hand side.
                self.expr(&assignment.right)
            }
            Expression::AwaitExpression(inner) => self.expr(&inner.argument),
            Expression::SequenceExpression(sequence) => {
                // `(0, lib.foo)` evaluates to its last operand (used to strip a
                // method receiver). Walk all, return the last's value.
                let mut last = self.program.fresh_cell();
                for expression in &sequence.expressions {
                    last = self.expr(expression);
                }
                last
            }
            // `cond ? a : b` is the union of both branches; `a || b` / `a && b` /
            // `a ?? b` likewise (a value-flow over-approximation, kept precise by
            // field sensitivity).
            Expression::ConditionalExpression(conditional) => {
                let result = self.program.fresh_cell();
                let c = self.expr(&conditional.consequent);
                let a = self.expr(&conditional.alternate);
                self.program.add(Constraint::Subset {
                    from: c,
                    to: result,
                });
                self.program.add(Constraint::Subset {
                    from: a,
                    to: result,
                });
                result
            }
            Expression::LogicalExpression(logical) => {
                let result = self.program.fresh_cell();
                let l = self.expr(&logical.left);
                let r = self.expr(&logical.right);
                self.program.add(Constraint::Subset {
                    from: l,
                    to: result,
                });
                self.program.add(Constraint::Subset {
                    from: r,
                    to: result,
                });
                result
            }
            _ => self.program.fresh_cell(),
        }
    }

    /// `require("./x")` resolves to the target file's exports cell, so
    /// `const lib = require("./x"); lib.foo()` flows cross-file.
    fn require_target(&mut self, call: &oxc_ast::ast::CallExpression<'_>) -> Option<CellId> {
        let Expression::Identifier(identifier) = &call.callee else {
            return None;
        };
        if identifier.name != "require" {
            return None;
        }
        let Some(Argument::StringLiteral(spec)) = call.arguments.first() else {
            return None;
        };
        let target = *self
            .resolution_map
            .get(&(self.file, spec.value.to_string()))?;
        Some(self.module_exports_cell(target))
    }

    fn call_value(&mut self, call: &oxc_ast::ast::CallExpression<'_>) -> CellId {
        if let Some(target) = self.require_target(call) {
            return target;
        }
        // `this` for a member call is the receiver object.
        let (callee_cell, this_arg, hint) = match &call.callee {
            Expression::StaticMemberExpression(member) => {
                let recv = self.expr(&member.object);
                let into = self.program.fresh_cell();
                self.program.add(Constraint::FieldLoad {
                    base: recv,
                    field: member.property.name.to_string(),
                    into,
                });
                (
                    into,
                    Some(recv),
                    CalleeHint::Member(member.property.name.to_string()),
                )
            }
            Expression::ComputedMemberExpression(member) => {
                // Resolve the value via the constant key (`x[prop]` with
                // `const prop = "qwe"` → field "qwe"), but EMIT by exact span: the
                // call site's callee carries the *syntactic* key, not the resolved
                // one, so a `Member(resolved)` hint would never match.
                let recv = self.expr(&member.object);
                let into = self.program.fresh_cell();
                if let Some(field) = self.const_key(&member.expression) {
                    self.program.add(Constraint::FieldLoad {
                        base: recv,
                        field,
                        into,
                    });
                }
                (into, Some(recv), CalleeHint::Other)
            }
            Expression::Identifier(identifier) => (
                self.lookup(identifier.name.as_str()),
                None,
                CalleeHint::Ident(identifier.name.to_string()),
            ),
            other => (self.expr(other), None, CalleeHint::Other),
        };
        let args: Vec<CellId> = call
            .arguments
            .iter()
            .map(|argument| self.argument(argument))
            .collect();
        let result = self.program.fresh_cell();
        let site = CallId(self.calls.len() as u64);
        self.calls.push(CallRecord {
            file: self.file,
            start: call.span.start,
            end: call.span.end,
            hint,
        });
        self.program.add(Constraint::Call {
            callee: callee_cell,
            args,
            this_arg,
            result,
            site,
        });
        result
    }

    fn new_value(&mut self, new: &oxc_ast::ast::NewExpression<'_>) -> CellId {
        let object = Token::Object(self.alloc_site(new.span));
        let cell = self.program.fresh_cell();
        self.program.add(Constraint::Alloc {
            token: object,
            into: cell,
        });
        // Run the constructor with `this` = the new object so `this.x = …` lands.
        let callee = self.expr(&new.callee);
        // `new C()`: the instance inherits C's prototype (instance methods), so
        // `new C().m()` resolves — token-based, so `new lib.X()` works cross-file.
        self.program.add(Constraint::Construct {
            callee,
            instance: object,
        });
        let args: Vec<CellId> = new
            .arguments
            .iter()
            .map(|argument| self.argument(argument))
            .collect();
        let throwaway = self.program.fresh_cell();
        let site = CallId(self.calls.len() as u64);
        let hint = match &new.callee {
            Expression::Identifier(identifier) => CalleeHint::Ident(identifier.name.to_string()),
            _ => CalleeHint::Other,
        };
        self.calls.push(CallRecord {
            file: self.file,
            start: new.span.start,
            end: new.span.end,
            hint,
        });
        self.program.add(Constraint::Call {
            callee,
            args,
            this_arg: Some(cell),
            result: throwaway,
            site,
        });
        cell
    }

    fn assignment(&mut self, assignment: &oxc_ast::ast::AssignmentExpression<'_>) {
        use oxc_ast::ast::AssignmentTarget;
        let value = self.expr(&assignment.right);
        match &assignment.left {
            AssignmentTarget::AssignmentTargetIdentifier(identifier) => {
                let cell = self.lookup(identifier.name.as_str());
                self.program.add(Constraint::Subset {
                    from: value,
                    to: cell,
                });
            }
            AssignmentTarget::StaticMemberExpression(member) => {
                // `module.exports = X` replaces the exports object: alias it.
                if expression_identifier(&member.object) == Some("module")
                    && member.property.name == "exports"
                {
                    let exports = self.module_exports_cell(self.file);
                    self.program.add(Constraint::Subset {
                        from: value,
                        to: exports,
                    });
                    return;
                }
                let base = self.expr(&member.object);
                self.program.add(Constraint::FieldStore {
                    base,
                    field: member.property.name.to_string(),
                    src: value,
                });
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                let base = self.expr(&member.object);
                if let Some(field) = self.const_key(&member.expression) {
                    self.program.add(Constraint::FieldStore {
                        base,
                        field,
                        src: value,
                    });
                }
            }
            // Other targets (destructuring patterns, etc.) are not yet modeled.
            _ => {}
        }
    }

    fn argument(&mut self, argument: &Argument<'_>) -> CellId {
        match argument {
            Argument::SpreadElement(spread) => self.expr(&spread.argument),
            other => match other.as_expression() {
                Some(expression) => self.expr(expression),
                None => self.program.fresh_cell(),
            },
        }
    }

    // ---- functions / classes --------------------------------------------

    fn function_value(
        &mut self,
        span: oxc_span::Span,
        params: &oxc_ast::ast::FormalParameters<'_>,
        body: Option<&FunctionBody<'_>>,
    ) -> Option<CellId> {
        let function_id = self.function_id(span)?;
        let cell = self.program.fresh_cell();
        self.program.add(Constraint::Alloc {
            token: Token::Function(function_id.0),
            into: cell,
        });
        self.walk_function_body(function_id, params, body, None, false);
        Some(cell)
    }

    fn arrow_value(&mut self, arrow: &oxc_ast::ast::ArrowFunctionExpression<'_>) -> CellId {
        let Some(function_id) = self.function_id(arrow.span) else {
            return self.program.fresh_cell();
        };
        let cell = self.program.fresh_cell();
        self.program.add(Constraint::Alloc {
            token: Token::Function(function_id.0),
            into: cell,
        });
        // Arrows capture `this` lexically: inherit the enclosing frame's this.
        let captured_this = self.fn_stack.last().and_then(|frame| frame.this_cell);
        self.walk_function_body(
            function_id,
            &arrow.params,
            Some(&arrow.body),
            captured_this,
            arrow.expression,
        );
        cell
    }

    fn walk_function_body(
        &mut self,
        function_id: FunctionId,
        params: &oxc_ast::ast::FormalParameters<'_>,
        body: Option<&FunctionBody<'_>>,
        inherited_this: Option<CellId>,
        expression_body: bool,
    ) {
        let mut param_cells = Vec::new();
        let mut scope = Scope::new();
        for param in &params.items {
            let cell = self.program.fresh_cell();
            if let Some(name) = binding_name(&param.pattern) {
                scope.insert(name, cell);
            }
            param_cells.push(cell);
        }
        let return_cell = self.program.fresh_cell();
        let this_cell = inherited_this.or_else(|| Some(self.program.fresh_cell()));
        self.program.set_function_cells(
            function_id.0,
            FunctionCells {
                params: param_cells,
                rest: None,
                this: this_cell,
                ret: Some(return_cell),
            },
        );
        self.scopes.push(scope);
        self.fn_stack.push(FnFrame {
            return_cell,
            this_cell,
        });
        if let Some(body) = body {
            if expression_body {
                // `x => expr`: oxc stores the body as one ExpressionStatement whose
                // value is the implicit return.
                if let Some(Statement::ExpressionStatement(stmt)) = body.statements.first() {
                    let value = self.expr(&stmt.expression);
                    self.program.add(Constraint::Subset {
                        from: value,
                        to: return_cell,
                    });
                }
            } else {
                self.hoist(&body.statements);
                for statement in &body.statements {
                    self.statement(statement);
                }
            }
        }
        self.fn_stack.pop();
        self.scopes.pop();
    }

    fn class_value(
        &mut self,
        class: &oxc_ast::ast::Class<'_>,
        _class_name: Option<&str>,
    ) -> CellId {
        // The class value is its constructor function token. Static methods attach
        // to the class object; instance methods go on the class's PROTOTYPE token,
        // which `new C()` instances inherit (token-based, so it works cross-file).
        let constructor_id = self.function_id(class.span);
        let cell = self.program.fresh_cell();
        let Some(constructor_id) = constructor_id else {
            return cell;
        };
        self.program.add(Constraint::Alloc {
            token: Token::Function(constructor_id.0),
            into: cell,
        });
        let proto_token = prototype_token(constructor_id);
        let proto_cell = self.program.fresh_cell();
        self.program.add(Constraint::Alloc {
            token: proto_token,
            into: proto_cell,
        });
        self.program
            .set_class_prototype(constructor_id.0, proto_token);
        for element in &class.body.body {
            if let oxc_ast::ast::ClassElement::MethodDefinition(method) = element
                // Plain methods only — getters/setters/constructor are accessor/
                // construct semantics, not plain properties.
                && method.kind == MethodDefinitionKind::Method
                && let Some(name) = property_key_name(&method.key)
                && let Some(method_id) = self.function_id(method.value.span)
            {
                let method_cell = self.program.fresh_cell();
                self.program.add(Constraint::Alloc {
                    token: Token::Function(method_id.0),
                    into: method_cell,
                });
                let base = if method.r#static { cell } else { proto_cell };
                self.program.add(Constraint::FieldStore {
                    base,
                    field: name,
                    src: method_cell,
                });
                self.walk_function_body(
                    method_id,
                    &method.value.params,
                    method.value.body.as_deref(),
                    None,
                    false,
                );
            }
        }
        cell
    }

    // ---- small helpers ---------------------------------------------------

    fn function_id(&self, span: oxc_span::Span) -> Option<FunctionId> {
        self.function_id_by_span
            .get(&(self.file, span.start, span.end))
            .copied()
    }

    fn alloc_site(&self, span: oxc_span::Span) -> u64 {
        // A stable per-file allocation-site identity: pack file + start byte.
        //
        // The `Token::Object(u64)` space is shared by three disjoint families,
        // kept apart by the two top bits: allocation sites `file<<32 | start`
        // (top two bits clear), module objects `1<<63 | file` (bit 63), and class
        // prototypes `1<<62 | constructor_id` (bit 62). The split holds as long as
        // `FileId` < 2^30 (so a shifted file never reaches bit 62); a real repo
        // has at most millions of files, far under that.
        debug_assert!(
            self.file.0 < (1u32 << 30),
            "FileId must stay below 2^30 to keep the Token::Object high bits free"
        );
        ((self.file.0 as u64) << 32) | (span.start as u64)
    }

    /// Resolve a computed member key to a constant field name (string literal or
    /// `const k = "…"`), or `None` when it cannot be proven constant.
    fn const_key(&self, expression: &Expression<'_>) -> Option<String> {
        self.const_key_at(expression, 0)
    }

    /// Constant-fold a computed-member key, bounded by `depth` so a long
    /// `"a" + "b" + …` concatenation can't recurse the stack to overflow.
    fn const_key_at(&self, expression: &Expression<'_>, depth: usize) -> Option<String> {
        if depth > MAX_HARVEST_DEPTH {
            return None;
        }
        match expression {
            Expression::StringLiteral(literal) => Some(literal.value.to_string()),
            Expression::Identifier(identifier) => {
                self.string_consts.get(identifier.name.as_str()).cloned()
            }
            Expression::ParenthesizedExpression(inner) => {
                self.const_key_at(&inner.expression, depth + 1)
            }
            // `"a" + "b"` / `prefix + "x"` — resolve constant string concatenation.
            Expression::BinaryExpression(binary)
                if binary.operator == oxc_ast::ast::BinaryOperator::Addition =>
            {
                let left = self.const_key_at(&binary.left, depth + 1)?;
                let right = self.const_key_at(&binary.right, depth + 1)?;
                Some(left + &right)
            }
            _ => None,
        }
    }
}

fn binding_name(pattern: &BindingPattern<'_>) -> Option<String> {
    match pattern {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.to_string()),
        BindingPattern::AssignmentPattern(pattern) => binding_name(&pattern.left),
        _ => None,
    }
}

fn property_key_name(key: &PropertyKey<'_>) -> Option<String> {
    match key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

/// A class's prototype-object token, distinct from allocation-site tokens
/// (`file<<32|start`, no high bits) and module tokens (`1<<63`).
fn prototype_token(constructor: FunctionId) -> Token {
    debug_assert!(
        constructor.0 < (1u64 << 62),
        "FunctionId must fit below bit 62 to avoid clobbering the prototype tag"
    );
    Token::Object((1u64 << 62) | constructor.0)
}

fn expression_identifier<'a>(expression: &'a Expression<'_>) -> Option<&'a str> {
    match expression {
        Expression::Identifier(identifier) => Some(identifier.name.as_str()),
        _ => None,
    }
}

/// Is this expression the member `module.exports`?
fn is_module_dot_exports(expression: &Expression<'_>) -> bool {
    matches!(
        expression,
        Expression::StaticMemberExpression(member)
            if expression_identifier(&member.object) == Some("module")
                && member.property.name == "exports"
    )
}
