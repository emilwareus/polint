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

use super::solver::{
    ARRAY_ALL, ARRAY_UNKNOWN, CallId, CellId, Constraint, FunctionCells, PointsToProgram, Token,
};

/// Recursion ceiling for the AST walk. The harvester runs over every JS/TS file
/// in an arbitrary repo, so adversarial / generated input (deeply nested
/// parentheses, thousand-operand string concatenations, nested destructuring)
/// must not overflow the stack. Past this depth the walk bails to an empty cell
/// (no tokens, no edges) — a soundness-preserving give-up, mirroring the
/// `invocation_depth` guard in the sibling `ts_value_flows` recognizer.
const MAX_HARVEST_DEPTH: usize = 256;

/// Array literals with more than this many elements keep no per-index precision —
/// every element lands in `%ARRAY_UNKNOWN` (matches Jelly's `ArrayExpression`).
const ARRAY_INDEX_LIMIT: usize = 10;

/// Sentinel call-site id for native-callback invocations (`forEach`/`map`/…). It
/// has no [`CallRecord`], so the solver wires the callback's parameters but the
/// provider emits no edge for it (native code is the real, unattributed caller).
const NATIVE_CALLBACK_SITE: u64 = u64::MAX;

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
    /// Names bound to a property-merge helper (`require('merge-descriptors')`), so a
    /// `mixin(dest, src)` call models `dest` inheriting `src`'s properties.
    merge_helper_names: std::collections::BTreeSet<String>,
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
            merge_helper_names: std::collections::BTreeSet::new(),
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
                // `for (const x of arr)`: bind the loop variable to the iterable's
                // elements (its `%ARRAY_ALL` summary) — every element is genuinely
                // visited, so this stays precise. The loop variable is block-scoped,
                // so push a fresh scope (a reused `const f` in a sibling loop must
                // NOT inherit this loop's element binding).
                let iterable = self.expr(&stmt.right);
                let values = self.read_iterator_value(iterable);
                self.scopes.push(Scope::new());
                self.bind_for_of_left(&stmt.left, values);
                self.statement(&stmt.body);
                self.scopes.pop();
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
        // Record a property-merge helper binding: `var mixin = require('merge-descriptors')`.
        if let Some(name) = binding_name(&declarator.id)
            && let Some(Expression::CallExpression(call)) = &declarator.init
            && is_merge_helper_require(call)
        {
            self.merge_helper_names.insert(name);
        }
        // Always evaluate the initializer (it may contain calls), then bind the
        // pattern — a plain name, or object destructuring `const {a, b} = init`.
        let Some(init) = &declarator.init else {
            return;
        };
        let value = self.expr(init);
        self.bind_pattern(&declarator.id, value);
    }

    /// Bind a `for…of` loop variable to the iterated element values. Handles the
    /// common `for (const x of …)` declaration and a bare assignment-target
    /// identifier; other targets (array destructuring) degrade to no binding.
    fn bind_for_of_left(&mut self, left: &oxc_ast::ast::ForStatementLeft<'_>, values: CellId) {
        use oxc_ast::ast::ForStatementLeft;
        match left {
            ForStatementLeft::VariableDeclaration(decl) => {
                if let Some(declarator) = decl.declarations.first() {
                    // A `const`/`let` loop variable is a FRESH block binding — bind
                    // its name directly in the current (loop) scope so it shadows any
                    // outer name of the same spelling rather than aliasing its cell.
                    if let Some(name) = binding_name(&declarator.id) {
                        let cell = self.program.fresh_cell();
                        self.bind(&name, cell);
                        self.program.add(Constraint::Subset {
                            from: values,
                            to: cell,
                        });
                    } else {
                        // Destructuring pattern (`for (const [k, v] of …)`): degrade.
                        self.bind_pattern(&declarator.id, values);
                    }
                }
            }
            ForStatementLeft::AssignmentTargetIdentifier(identifier) => {
                let cell = self.lookup(identifier.name.as_str());
                self.program.add(Constraint::Subset {
                    from: values,
                    to: cell,
                });
            }
            _ => {}
        }
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
            Expression::ArrayExpression(array) => self.array_literal(array),
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

    /// `Object(x)` returns `x` itself when `x` is an object, else a fresh wrapper
    /// object. Model both: the result holds `x`'s tokens (identity — so
    /// `o2 = Object(o1); o2.g = g; o1.g()` resolves) plus a fresh object token (so a
    /// primitive argument and writes onto the result still have a base). Bare
    /// `Object(...)` only; `Object.assign(...)` is a member call handled elsewhere.
    fn object_coercion_target(
        &mut self,
        call: &oxc_ast::ast::CallExpression<'_>,
    ) -> Option<CellId> {
        let Expression::Identifier(identifier) = &call.callee else {
            return None;
        };
        if identifier.name != "Object" {
            return None;
        }
        let result = self.program.fresh_cell();
        let token = Token::Object(self.alloc_site(call.span));
        self.program.add(Constraint::Alloc {
            token,
            into: result,
        });
        if let Some(arg) = call.arguments.first().and_then(|a| a.as_expression()) {
            let arg_cell = self.expr(arg);
            self.program.add(Constraint::Subset {
                from: arg_cell,
                to: result,
            });
        }
        Some(result)
    }

    fn call_value(&mut self, call: &oxc_ast::ast::CallExpression<'_>) -> CellId {
        if let Some(target) = self.require_target(call) {
            return target;
        }
        if let Some(target) = self.object_coercion_target(call) {
            return target;
        }
        // `this` for a member call is the receiver object.
        // A member call `recv.method(args)` whose method is an array native
        // (`push`/`pop`/`forEach`/…) also drives a transfer function on `recv`;
        // captured here and applied below once `args`/`result` exist.
        let mut array_native: Option<(String, CellId)> = None;
        let (callee_cell, this_arg, hint) = match &call.callee {
            Expression::StaticMemberExpression(member) => {
                let recv = self.expr(&member.object);
                let into = self.program.fresh_cell();
                self.program.add(Constraint::FieldLoad {
                    base: recv,
                    field: member.property.name.to_string(),
                    into,
                });
                array_native = Some((member.property.name.to_string(), recv));
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
            args: args.clone(),
            this_arg,
            result,
            site,
        });
        if let Some((method, recv)) = array_native {
            self.array_native_call(&method, recv, &args, result, call.span);
        }
        self.maybe_inherit_call(call, &args, result);
        result
    }

    /// `Object.assign(target, ...sources)` and `mixin(target, src)` (a
    /// `merge-descriptors` binding) copy each source's properties onto `target` and
    /// return `target`. Model the copy as prototype inheritance (`target` inherits
    /// each source) so a later `target.method()` resolves to the merged-in method —
    /// this is the express keystone (`mixin(app, proto); app.init()`).
    fn maybe_inherit_call(
        &mut self,
        call: &oxc_ast::ast::CallExpression<'_>,
        args: &[CellId],
        result: CellId,
    ) {
        let is_merge = match &call.callee {
            Expression::Identifier(identifier) => {
                self.merge_helper_names.contains(identifier.name.as_str())
            }
            Expression::StaticMemberExpression(member) => {
                expression_identifier(&member.object) == Some("Object")
                    && member.property.name == "assign"
            }
            _ => false,
        };
        if !is_merge {
            return;
        }
        let Some((target, sources)) = args.split_first() else {
            return;
        };
        for source in sources {
            self.program.add(Constraint::Inherit {
                object: *target,
                proto: *source,
            });
        }
        // The merge returns its target object.
        self.program.add(Constraint::Subset {
            from: *target,
            to: result,
        });
    }

    /// Apply Jelly's array native transfer functions for `recv.method(args)`.
    /// These are additive and no-ops for non-array receivers (they only touch the
    /// `%ARRAY_UNKNOWN` / `%ARRAY_ALL` cells, which only array consumers read), so a
    /// user object with a same-named method keeps its real call edge unaffected.
    ///
    /// Only *iterating* consumers (callbacks) read the array summary: every element
    /// is genuinely passed to the callback across the loop, so binding the callback
    /// parameter to `%ARRAY_ALL` stays precise against the dynamic oracle. Index
    /// reads (`arr[i]`) and `pop`/`at` are deliberately NOT modeled — picking one
    /// element from the union over-approximates and costs precision.
    fn array_native_call(
        &mut self,
        method: &str,
        recv: CellId,
        args: &[CellId],
        result: CellId,
        span: oxc_span::Span,
    ) {
        match method {
            // `arr.push(v)` / `unshift(v)`: value joins the unknown indices, so a
            // later `for…of` / `forEach` over the array sees it. (`fill` is NOT
            // modeled: its range form is a runtime no-op when the array is shorter,
            // which only pollutes the element set against the dynamic oracle.)
            "push" | "unshift" => {
                if let Some(arg) = args.first() {
                    self.array_store_unknown(recv, *arg);
                }
            }
            // `arr.forEach/filter/find/findIndex/some/every(cb)`: the callback runs
            // once per element with `(element, index, array)`. Bind its parameters
            // to the array summary + the array itself, and the optional `thisArg`.
            "forEach" | "filter" | "find" | "findIndex" | "some" | "every" => {
                if let Some(cb) = args.first() {
                    let elements = self.read_iterator_value(recv);
                    let discard = self.program.fresh_cell();
                    self.invoke_array_callback(*cb, elements, recv, args.get(1).copied(), discard);
                }
            }
            // `arr.map/flatMap(cb)`: like forEach, but the callback's return values
            // become the elements of a fresh result array. (flatMap is modeled as
            // map — one flattening level is deferred.)
            "map" | "flatMap" => {
                if let Some(cb) = args.first() {
                    let elements = self.read_iterator_value(recv);
                    let returns = self.program.fresh_cell();
                    self.invoke_array_callback(*cb, elements, recv, args.get(1).copied(), returns);
                    let r = self.new_array_cell(span);
                    self.array_store_unknown(r, returns);
                    self.program.add(Constraint::Subset {
                        from: r,
                        to: result,
                    });
                }
            }
            // `arr.reduce/reduceRight(cb, init?)`: the callback runs with
            // `(accumulator, currentValue, index, array)`. The accumulator and the
            // reduce result receive the initial value (or, with no init, the array
            // elements) plus every callback return value.
            "reduce" | "reduceRight" => {
                if let Some(cb) = args.first() {
                    let elements = self.read_iterator_value(recv);
                    let acc = self.program.fresh_cell();
                    let seed = args.get(1).copied().unwrap_or(elements);
                    self.program.add(Constraint::Subset {
                        from: seed,
                        to: acc,
                    });
                    self.program.add(Constraint::Subset {
                        from: seed,
                        to: result,
                    });
                    // Callback return flows back into the accumulator and the result.
                    self.program.add(Constraint::Subset {
                        from: result,
                        to: acc,
                    });
                    let idx = self.program.fresh_cell();
                    self.invoke_callback_with_args(
                        *cb,
                        vec![acc, elements, idx, recv],
                        None,
                        result,
                    );
                }
            }
            _ => {}
        }
    }

    /// Invoke an array iteration callback `cb(element, index, array)` with the given
    /// `thisArg` and return collector. Param-wiring only — no call edge is emitted
    /// (native code is the real caller, which the dynamic oracle does not attribute
    /// to the `forEach`/`map` call site).
    fn invoke_array_callback(
        &mut self,
        cb: CellId,
        elements: CellId,
        array: CellId,
        this_arg: Option<CellId>,
        result: CellId,
    ) {
        let idx = self.program.fresh_cell();
        self.invoke_callback_with_args(cb, vec![elements, idx, array], this_arg, result);
    }

    /// Emit a `Call` whose site has no `CallRecord`, so the solver wires
    /// arguments→parameters / return→result (and `this`) when `cb` resolves, but
    /// the provider emits no edge for it (the synthetic site maps to nothing).
    fn invoke_callback_with_args(
        &mut self,
        cb: CellId,
        args: Vec<CellId>,
        this_arg: Option<CellId>,
        result: CellId,
    ) {
        self.program.add(Constraint::Call {
            callee: cb,
            args,
            this_arg,
            result,
            site: CallId(NATIVE_CALLBACK_SITE),
        });
    }

    /// Mint a fresh array allocation (token + cell) keyed by `span`.
    fn new_array_cell(&mut self, span: oxc_span::Span) -> CellId {
        let token = Token::Array(self.alloc_site(span));
        let cell = self.program.fresh_cell();
        self.program.add(Constraint::Alloc { token, into: cell });
        cell
    }

    /// `src ⊆ array.%ARRAY_UNKNOWN`.
    fn array_store_unknown(&mut self, array: CellId, src: CellId) {
        self.program.add(Constraint::FieldStore {
            base: array,
            field: ARRAY_UNKNOWN.to_string(),
            src,
        });
    }

    /// The iterated values of `src` (`for…of`, spread, `concat`): for an array
    /// token, the `%ARRAY_ALL` summary; empty for anything we don't model.
    fn read_iterator_value(&mut self, src: CellId) -> CellId {
        let into = self.program.fresh_cell();
        self.program.add(Constraint::FieldLoad {
            base: src,
            field: ARRAY_ALL.to_string(),
            into,
        });
        into
    }

    /// `[e0, e1, …]`: a fresh array token with each constant-index element in its
    /// own cell, spread sources flowing into `%ARRAY_UNKNOWN`, mirroring Jelly's
    /// `ArrayExpression` (≤10 elements keep precise indices; a spread or overflow
    /// degrades the rest to unknown indices).
    fn array_literal(&mut self, array: &oxc_ast::ast::ArrayExpression<'_>) -> CellId {
        use oxc_ast::ast::ArrayExpressionElement;
        let cell = self.new_array_cell(array.span);
        let mut index_known = array.elements.len() <= ARRAY_INDEX_LIMIT;
        for (index, element) in array.elements.iter().enumerate() {
            match element {
                ArrayExpressionElement::SpreadElement(spread) => {
                    index_known = false;
                    let src = self.expr(&spread.argument);
                    let values = self.read_iterator_value(src);
                    self.array_store_unknown(cell, values);
                }
                ArrayExpressionElement::Elision(_) => {}
                _ => {
                    let value = self.expr(element.to_expression());
                    let field = if index_known {
                        index.to_string()
                    } else {
                        ARRAY_UNKNOWN.to_string()
                    };
                    self.program.add(Constraint::FieldStore {
                        base: cell,
                        field,
                        src: value,
                    });
                }
            }
        }
        cell
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
                } else {
                    // Dynamic index write `arr[i] = v` (non-constant key): the value
                    // lands in the array's `%ARRAY_UNKNOWN` cell. A no-op for
                    // non-array objects (nothing reads their `%ARRAY_UNKNOWN`).
                    self.program.add(Constraint::FieldStore {
                        base,
                        field: ARRAY_UNKNOWN.to_string(),
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
            // A non-negative integer literal is an array index / numeric property
            // key (`arr[0]`, `obj[2]`). JS coerces it to its canonical string form.
            Expression::NumericLiteral(literal) => {
                let value = literal.value;
                (value.fract() == 0.0 && value >= 0.0 && value < u32::MAX as f64)
                    .then(|| (value as u32).to_string())
            }
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

/// Is this call `require('merge-descriptors')` (or another known property-merge
/// helper)? Such a binding, when called as `mixin(dest, src)`, merges `src`'s
/// properties into `dest` (modeled as inheritance).
fn is_merge_helper_require(call: &oxc_ast::ast::CallExpression<'_>) -> bool {
    let Expression::Identifier(identifier) = &call.callee else {
        return false;
    };
    if identifier.name != "require" {
        return false;
    }
    matches!(
        call.arguments.first(),
        Some(Argument::StringLiteral(spec)) if spec.value == "merge-descriptors"
    )
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
