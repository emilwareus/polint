use std::collections::BTreeMap;
use std::path::PathBuf;

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, ArrayExpressionElement, AssignmentTarget, BinaryOperator, BindingPattern,
    CallExpression, Class, ClassElement, Declaration, ExportDefaultDeclarationKind, Expression,
    ForStatementLeft, FunctionBody, ImportDeclarationSpecifier, MethodDefinition,
    MethodDefinitionKind, ModuleExportName, ObjectProperty, ObjectPropertyKind, Program,
    PropertyKey, PropertyKind, Statement, VariableDeclaration, VariableDeclarator,
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

/// Maximum bounded rounds for the cross-module export-summary fixpoint. Each
/// round lets a module's `require`/`import` targets observe the previous
/// round's summaries, so re-export chains such as
/// `module.exports = require('./lib/foo')` converge after a few rounds. The
/// bound keeps deep dependency graphs (e.g. express) finite.
const MAX_MODULE_SUMMARY_ROUNDS: usize = 4;

pub(crate) fn resolve_ts_value_flow_targets(
    db: &AnalysisDb,
    sites: &[CallSiteFact],
    id_offset: u64,
) -> Vec<CallTargetFact> {
    let mut rows = Vec::new();
    let mut next_id = id_offset;
    let sites_by_file = sites_by_file(sites);
    let functions_by_file_start = functions_by_file_start(db);
    let resolution_map = module_resolution_map(db);
    let module_summaries =
        compute_module_export_summaries(db, &functions_by_file_start, &resolution_map);

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
            resolution_map: &resolution_map,
            module_summaries: &module_summaries,
            function_declarations: BTreeMap::new(),
            function_flows_by_id: BTreeMap::new(),
            classes: BTreeMap::new(),
            class_expressions: Vec::new(),
            exports: ModuleExportSummary::default(),
            caller_override: None,
            current_super: None,
            invocation_depth: 0,
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

struct TsValueFlowCollector<'db, 'ast, 'env> {
    db: &'db AnalysisDb,
    file: &'db SourceFile,
    module: &'db FunctionFact,
    sites: &'db [&'db CallSiteFact],
    functions_by_start: &'db BTreeMap<(FileId, u32), Vec<&'db FunctionFact>>,
    /// `(importer file, specifier) -> resolved target file`, reusing the kernel
    /// module graph's `require`/`import` resolution.
    resolution_map: &'env BTreeMap<(FileId, String), FileId>,
    /// Converged per-file CommonJS/ESM export summaries used to seed
    /// `require(...)` / `import` results into the current file's value flow.
    module_summaries: &'env BTreeMap<FileId, ModuleExportSummary>,
    function_declarations: BTreeMap<String, FunctionFlow<'db, 'ast>>,
    function_flows_by_id: BTreeMap<FunctionId, FunctionFlow<'db, 'ast>>,
    classes: BTreeMap<String, ClassTargets>,
    /// Class **expressions** (named, anonymous, or returned from a function),
    /// each keyed by a span-derived synthetic name registered in `classes`.
    /// Their method/constructor/static bodies are walked with `this`/`super`
    /// bound just like top-level class declarations.
    class_expressions: Vec<(String, &'ast Class<'ast>)>,
    /// This file's own exports, accumulated while walking module-level
    /// statements (only meaningful during the summary pre-pass). During the
    /// summary pre-pass `sites` is empty, so the `emit_*` helpers naturally
    /// produce no rows and only `exports` is harvested.
    exports: ModuleExportSummary,
    /// When set, emitted edges use this function as the caller instead of the
    /// call site's enclosing function. Jelly attributes constructor-body (and
    /// field-initializer) calls to the class node, not the `constructor()` node,
    /// so the class-body walk sets this to the class function fact while walking
    /// the constructor.
    caller_override: Option<FunctionId>,
    /// The super-class instance/static member objects in scope while walking a
    /// class method body, used to resolve `super.m()` / `super.s()` / `super.f`.
    current_super: Option<ObjectTargets>,
    /// Nesting depth of function-return-value invocation walks. Bounds recursion
    /// for self-/mutually-returning functions (`function f() { return f(); }`),
    /// which would otherwise loop forever because the per-call `depth` resets to 0
    /// when a return statement re-enters `callable_return_targets_from_expression`.
    invocation_depth: usize,
    rows: Vec<CallTargetFact>,
    next_id: u64,
}

/// CommonJS/ESM export shape of a single module: the callable values reachable
/// through `module.exports = fn` (or `export default`) plus the property object
/// reachable through `exports.foo = ...` / `module.exports = { foo }`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ModuleExportSummary {
    object: ObjectTargets,
    callables: CollectionTargets,
}

impl ModuleExportSummary {
    fn is_empty(&self) -> bool {
        self.object.is_empty() && self.callables.is_empty()
    }
}

#[derive(Clone)]
struct FunctionFlow<'db, 'ast> {
    function: &'db FunctionFact,
    body: &'ast FunctionBody<'ast>,
    expression_body: Option<&'ast Expression<'ast>>,
    params: Vec<ParamPattern>,
    rest: Option<ParamPattern>,
    generator: bool,
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

/// Build `(importer file, specifier) -> resolved target file`. `require("x")`
/// and static `import` specifiers are captured as `ImportFact`s by the TS
/// frontend regardless of the analysis plan; we resolve each specifier against
/// the file set with a Node-style resolver. This intentionally does NOT rely on
/// the kernel module-graph layer (`resolved_imports`), which is only populated
/// when a rule requests it — the benchmark/value-flow path must work standalone.
fn module_resolution_map(db: &AnalysisDb) -> BTreeMap<(FileId, String), FileId> {
    let imports = db.imports();
    if imports.is_empty() {
        return BTreeMap::new();
    }
    // Index every analyzed file by both its stored path and its canonicalized
    // path so resolver output (which canonicalizes symlinks, e.g. macOS
    // /var -> /private/var) maps back to a FileId.
    let mut file_by_path: BTreeMap<PathBuf, FileId> = BTreeMap::new();
    for file in db.files() {
        file_by_path.entry(file.path.clone()).or_insert(file.id);
        if let Ok(canonical) = file.path.canonicalize() {
            file_by_path.entry(canonical).or_insert(file.id);
        }
    }
    let resolver = oxc_resolver::Resolver::new(module_resolve_options());
    let mut map = BTreeMap::new();
    for import in imports {
        let Some(importer) = db.file(import.file) else {
            continue;
        };
        let Ok(resolution) = resolver.resolve_file(&importer.path, import.path.as_str()) else {
            continue;
        };
        let resolved = resolution.path();
        let target = file_by_path.get(resolved).copied().or_else(|| {
            resolved
                .canonicalize()
                .ok()
                .and_then(|canonical| file_by_path.get(&canonical).copied())
        });
        if let Some(target) = target
            && target != import.file
        // A module never resolves to itself; self-edges would create
        // spurious recursion in the summary fixpoint.
        {
            map.insert((import.file, import.path.clone()), target);
        }
    }
    map
}

/// Node-style resolution options for `require`/`import` specifiers: JS/TS/JSON
/// extensions, `index` main file, and `main`/`module` package fields.
fn module_resolve_options() -> oxc_resolver::ResolveOptions {
    oxc_resolver::ResolveOptions {
        extensions: vec![
            ".js".into(),
            ".jsx".into(),
            ".ts".into(),
            ".tsx".into(),
            ".cjs".into(),
            ".mjs".into(),
            ".json".into(),
            ".node".into(),
        ],
        main_files: vec!["index".into()],
        main_fields: vec!["main".into(), "module".into()],
        condition_names: vec![
            "require".into(),
            "node".into(),
            "import".into(),
            "default".into(),
        ],
        ..oxc_resolver::ResolveOptions::default()
    }
}

/// Compute each module's export summary via a bounded fixpoint. Round 0 sees no
/// upstream summaries (so only locally-defined exports resolve); each later
/// round lets `require`/`import` results observe the previous round's
/// summaries, converging re-export chains.
fn compute_module_export_summaries(
    db: &AnalysisDb,
    functions_by_file_start: &BTreeMap<(FileId, u32), Vec<&FunctionFact>>,
    resolution_map: &BTreeMap<(FileId, String), FileId>,
) -> BTreeMap<FileId, ModuleExportSummary> {
    let mut summaries: BTreeMap<FileId, ModuleExportSummary> = BTreeMap::new();
    if resolution_map.is_empty() {
        // No cross-module imports were resolved, so there is nothing for the
        // summary fixpoint to feed; skip the (expensive) pre-pass entirely.
        return summaries;
    }
    for _round in 0..MAX_MODULE_SUMMARY_ROUNDS {
        let mut next: BTreeMap<FileId, ModuleExportSummary> = BTreeMap::new();
        for file in db
            .files()
            .iter()
            .filter(|file| file.language.is_ts_family())
        {
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
                sites: &[],
                functions_by_start: functions_by_file_start,
                resolution_map,
                module_summaries: &summaries,
                function_declarations: BTreeMap::new(),
                function_flows_by_id: BTreeMap::new(),
                classes: BTreeMap::new(),
                class_expressions: Vec::new(),
                exports: ModuleExportSummary::default(),
                caller_override: None,
                current_super: None,
                invocation_depth: 0,
                rows: Vec::new(),
                next_id: 0,
            };
            collector.collect_program(&parsed.program);
            if !collector.exports.is_empty() {
                next.insert(file.id, collector.exports);
            }
        }
        if next == summaries {
            break;
        }
        summaries = next;
    }
    summaries
}

impl<'db, 'ast, 'env> TsValueFlowCollector<'db, 'ast, 'env> {
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
        // ESM imports are hoisted: seed their bindings before walking the body.
        self.collect_esm_imports(program, &mut env);
        for statement in &program.body {
            self.collect_esm_export(statement, &mut env);
            self.collect_statement(statement, self.module, &mut env);
        }
        // Resolve intra-method calls (`this.foo()`, `super.m()`, `this.#bar()`)
        // with `this`/`super` bound, after the module env is fully populated.
        self.collect_class_body_call_flows(program, &env);
    }

    /// Walk each class method and constructor body for call resolution with
    /// `this`/`super` in scope. Jelly treats every class function as reachable,
    /// and attributes constructor-body calls to the class node (via
    /// `caller_override`).
    fn collect_class_body_call_flows(
        &mut self,
        program: &'ast Program<'ast>,
        module_env: &FlowEnv,
    ) {
        // Seed top-level function declarations as callable bindings so direct
        // calls to them inside class bodies (e.g. `f1()` in a constructor or
        // static block) resolve and are attributed via `caller_override` to the
        // class node (Jelly attributes constructor/field/static-block calls to
        // the class). `function` declarations are otherwise absent from
        // `env.bindings` (unlike `const f = () => …`).
        let mut class_env = module_env.clone();
        for (name, flow) in &self.function_declarations {
            class_env
                .bindings
                .entry(name.clone())
                .or_insert_with(|| CollectionTargets {
                    keys: Vec::new(),
                    values: vec![flow.function.id],
                    object_values: Vec::new(),
                });
        }
        for statement in &program.body {
            let Statement::ClassDeclaration(class) = statement else {
                continue;
            };
            let Some(name) = class.id.as_ref().map(|id| id.name.to_string()) else {
                continue;
            };
            self.collect_class_method_bodies(class, &name, &class_env);
        }
        // Class expressions (named, anonymous, or returned from a function) are
        // walked the same way, keyed by their span-derived synthetic name. Take
        // the registry (not read again) to avoid cloning it just to satisfy the
        // borrow checker while calling `&mut self`.
        let class_expressions = std::mem::take(&mut self.class_expressions);
        for (key, class) in &class_expressions {
            self.collect_class_method_bodies(class, key, &class_env);
        }
        self.class_expressions = class_expressions;
    }

    fn collect_class_method_bodies(
        &mut self,
        class: &'ast Class<'ast>,
        name: &str,
        module_env: &FlowEnv,
    ) {
        let instance = self
            .class_instance_targets(name, &[], module_env)
            .unwrap_or_default();
        let static_object = self.class_static_targets(name).unwrap_or_default();
        // Super-class member objects for `super.m()` / `super.s()` / `super.f`.
        let super_name = self
            .classes
            .get(name)
            .and_then(|class| class.super_name.clone());
        let super_instance = super_name
            .as_ref()
            .and_then(|super_name| self.class_instance_targets(super_name, &[], module_env));
        let super_static = super_name
            .as_ref()
            .and_then(|super_name| self.class_static_targets(super_name));
        // The class function fact owns constructor-body call attribution.
        let class_fact = self.function_for_span(class.span).map(|fact| fact.id);

        for element in &class.body.body {
            let (statements, owner, is_static, is_constructor, params) = match element {
                ClassElement::MethodDefinition(method) => {
                    let Some(body) = method.value.body.as_deref() else {
                        continue;
                    };
                    let Some(owner) = self.function_for_class_method(method) else {
                        continue;
                    };
                    let is_constructor = method.kind == MethodDefinitionKind::Constructor;
                    let params = method
                        .value
                        .params
                        .items
                        .iter()
                        .filter_map(|param| binding_identifier_name(&param.pattern))
                        .collect::<Vec<_>>();
                    (
                        &body.statements,
                        owner,
                        method.r#static,
                        is_constructor,
                        params,
                    )
                }
                ClassElement::StaticBlock(block) => {
                    let Some(owner) = self.function_for_span(block.span) else {
                        continue;
                    };
                    (&block.body, owner, true, false, Vec::new())
                }
                _ => continue,
            };
            let mut env = module_env.clone();
            env.this_object = if is_static {
                static_object.clone()
            } else {
                instance.clone()
            };
            // Bind the class name to its static object so `Class.staticM()` and
            // `Class.#priv()` resolve inside the body.
            env.objects.insert(name.to_string(), static_object.clone());
            // A parameter shadows any module-level binding of the same name (e.g.
            // a top-level function seeded into the class-body env). Its value is
            // unknown here, so drop the inherited binding rather than resolve the
            // call to the shadowed module symbol (a false positive).
            for param in &params {
                env.bindings.remove(param);
                env.class_bindings.remove(param);
                env.objects.remove(param);
            }
            self.current_super = if is_static {
                super_static.clone()
            } else {
                super_instance.clone()
            };
            // Constructor-body calls are attributed to the class node by Jelly.
            self.caller_override = if is_constructor { class_fact } else { None };
            for statement in statements {
                self.collect_statement(statement, owner, &mut env);
            }
            self.caller_override = None;
            self.current_super = None;
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
            Statement::ClassDeclaration(class) => {
                self.collect_expression_function_flows_from_class(class);
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
                        expression_body: None,
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
                        generator: false,
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
                        expression_body: function.get_expression(),
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
                        generator: false,
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
                            self.collect_expression_function_flows_from_object_property(property);
                        }
                        ObjectPropertyKind::SpreadProperty(spread) => {
                            self.collect_expression_function_flows_from_expression(
                                &spread.argument,
                            );
                        }
                    }
                }
            }
            Expression::ClassExpression(class) => {
                self.register_class_expression(class);
                self.collect_expression_function_flows_from_class(class);
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
            Expression::ConditionalExpression(expression) => {
                self.collect_expression_function_flows_from_expression(&expression.consequent);
                self.collect_expression_function_flows_from_expression(&expression.alternate);
            }
            Expression::LogicalExpression(expression) => {
                self.collect_expression_function_flows_from_expression(&expression.left);
                self.collect_expression_function_flows_from_expression(&expression.right);
            }
            _ => {}
        }
    }

    fn collect_expression_function_flows_from_object_property(
        &mut self,
        property: &'ast ObjectProperty<'ast>,
    ) {
        if property.method
            && let Expression::FunctionExpression(function) = &property.value
        {
            if let Some(body) = function.body.as_deref()
                && let Some(function_fact) = self.function_for_object_property(property)
            {
                let flow = FunctionFlow {
                    function: function_fact,
                    body,
                    expression_body: None,
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
                    generator: function.generator,
                };
                self.function_flows_by_id.insert(function_fact.id, flow);
                for statement in &body.statements {
                    self.collect_expression_function_flows_from_statement(statement);
                }
            }
            return;
        }

        self.collect_expression_function_flows_from_expression(&property.value);
    }

    fn collect_expression_function_flows_from_class(&mut self, class: &'ast Class<'ast>) {
        for element in &class.body.body {
            match element {
                ClassElement::MethodDefinition(method) => {
                    let Some(body) = method.value.body.as_deref() else {
                        continue;
                    };
                    let Some(function_fact) = self.function_for_class_method(method) else {
                        continue;
                    };
                    let flow = FunctionFlow {
                        function: function_fact,
                        body,
                        expression_body: None,
                        params: method
                            .value
                            .params
                            .items
                            .iter()
                            .map(|param| param_pattern_from_binding(&param.pattern))
                            .collect(),
                        rest: method
                            .value
                            .params
                            .rest
                            .as_ref()
                            .map(|rest| param_pattern_from_binding(&rest.rest.argument)),
                        generator: method.value.generator,
                    };
                    self.function_flows_by_id.insert(function_fact.id, flow);
                    for statement in &body.statements {
                        self.collect_expression_function_flows_from_statement(statement);
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    if let Some(value) = &property.value {
                        self.collect_expression_function_flows_from_expression(value);
                    }
                }
                ClassElement::AccessorProperty(property) => {
                    if let Some(value) = &property.value {
                        self.collect_expression_function_flows_from_expression(value);
                    }
                }
                ClassElement::StaticBlock(block) => {
                    for statement in &block.body {
                        self.collect_expression_function_flows_from_statement(statement);
                    }
                }
                ClassElement::TSIndexSignature(_) => {}
            }
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
                    expression_body: None,
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
                    generator: function.generator,
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

    /// Register a class **expression** under a span-derived synthetic key so its
    /// instance/static targets are resolvable and its bodies get walked with
    /// `this`/`super` bound (see `collect_class_body_call_flows`). Keyed by span
    /// (not by the class's own name) to avoid clobbering a same-named top-level
    /// class declaration; `new`/return flow binds variables to this key in A3.
    fn register_class_expression(&mut self, class: &'ast Class<'ast>) {
        let key = class_expression_key(class);
        if self.classes.contains_key(&key) {
            return;
        }
        let targets = self.class_targets(class);
        self.classes.insert(key.clone(), targets);
        self.class_expressions.push((key, class));
    }

    /// Resolve the class key a variable would hold for `var v = <expr>`, so a
    /// class flowed through a variable (returned from a function, aliased, or a
    /// direct class expression) resolves under `new v()` / `v.staticM()`.
    fn class_key_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<String> {
        self.class_key_from_expression_with_depth(expression, env, 0)
    }

    fn class_key_from_expression_with_depth(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
        depth: usize,
    ) -> Option<String> {
        // Bound recursion: a function whose returned expression resolves back to
        // itself (`function f() { return (f()); }`) would otherwise loop forever.
        if depth > 8 {
            return None;
        }
        match expression {
            Expression::ClassExpression(class) => Some(class_expression_key(class)),
            Expression::ParenthesizedExpression(inner) => {
                self.class_key_from_expression_with_depth(&inner.expression, env, depth + 1)
            }
            Expression::Identifier(identifier) => {
                let name = identifier.name.as_str();
                env.class_bindings
                    .get(name)
                    .cloned()
                    .or_else(|| self.classes.contains_key(name).then(|| name.to_string()))
            }
            Expression::CallExpression(call) => {
                let name = callee_identifier(&call.callee)?;
                let flow = self.function_declarations.get(name)?;
                let returned = returned_expression_from_statements(&flow.body.statements)?;
                match returned {
                    Expression::ClassExpression(class) => Some(class_expression_key(class)),
                    Expression::Identifier(_) | Expression::ParenthesizedExpression(_) => {
                        self.class_key_from_expression_with_depth(returned, env, depth + 1)
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn class_targets(&self, class: &'ast Class<'ast>) -> ClassTargets {
        let mut targets = ClassTargets {
            super_name: class
                .super_class
                .as_ref()
                .and_then(|super_class| expression_identifier(super_class))
                .map(ToOwned::to_owned),
            constructor: self
                .function_for_span(class.span)
                .map(|function| function.id),
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
                    // Jelly resolves instance properties flow-insensitively: a
                    // `this.p = fn` in any non-static instance method flows to all
                    // instances (e.g. super5's `c.www()` where `www` is assigned in
                    // `m()`, never called). Harvest those function-valued assignments.
                    // Pass NO params: a method's parameters are not the constructor's,
                    // so `m(cb) { this.cb = cb }` must not register a
                    // `ConstructorAssignment::Param` that would later resolve against
                    // the `new C(arg)` arguments (a false-positive edge).
                    if !method.r#static
                        && let Some(body) = method.value.body.as_deref()
                    {
                        self.collect_constructor_assignments_from_statements(
                            &body.statements,
                            &[],
                            &mut targets,
                        );
                    }
                    let names = self.property_key_names(&method.key, method.computed, None);
                    if names.is_empty() {
                        continue;
                    };
                    let Some(function) = self.function_for_class_method(method) else {
                        continue;
                    };
                    for name in names {
                        let object = if method.r#static {
                            &mut targets.static_object
                        } else {
                            &mut targets.instance_object
                        };
                        match method.kind {
                            MethodDefinitionKind::Get => {
                                object.add_getter_target(name, function.id);
                            }
                            MethodDefinitionKind::Set => {
                                object.add_setter_target(name, function.id);
                            }
                            MethodDefinitionKind::Method => {
                                object.add_property_target(name, function.id);
                            }
                            MethodDefinitionKind::Constructor => {}
                        }
                    }
                }
                ClassElement::PropertyDefinition(property) => {
                    let names = self.property_key_names(&property.key, property.computed, None);
                    if names.is_empty() {
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
                        for name in names {
                            if let Some(function) = self.function_for_expression(value) {
                                object.add_property_target(name, function.id);
                            } else if matches!(value, Expression::ThisExpression(_)) {
                                self_aliases.push(name);
                            }
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
        object.override_with(class.instance_object.clone());
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
        object.override_with(class.static_object.clone());
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
            // Bind a variable to a class flowed through it (returned from a
            // function, aliased, or a direct class expression) so `new v()` /
            // `v.staticM()` resolve. Seed the static object for static calls.
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(init) = &declarator.init
                && let Some(key) = self.class_key_from_expression(init, env)
            {
                if let Some(statics) = self.class_static_targets(&key) {
                    env.objects.insert(name.clone(), statics);
                }
                env.class_bindings.insert(name, key);
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(targets) = self.targets_for_declarator_init(declarator, env)
            {
                env.bindings.insert(name, targets);
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(init) = &declarator.init
                && let Some(value) = self.constant_string_from_expression(init, env)
            {
                env.string_bindings.insert(name, value);
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(init) = &declarator.init
                && let Some(value) = self.constant_bool_from_expression(init, env)
            {
                env.bool_bindings.insert(name, value);
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(Expression::ArrayExpression(array)) = &declarator.init
                && let Some(values) = self.string_array_from_literal(array, env)
            {
                env.string_arrays.insert(name, values);
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(init) = &declarator.init
                && let Expression::CallExpression(call) = init
                && let Some(bound_targets) = self.bound_function_targets_from_bind_call(call, env)
            {
                env.bindings.insert(
                    name.clone(),
                    CollectionTargets {
                        keys: Vec::new(),
                        values: bound_targets.iter().map(|target| target.function).collect(),
                        object_values: Vec::new(),
                    },
                );
                env.bound_functions.insert(name, bound_targets);
            }
            // `const l = o.foo()` where `foo` returns `() => this.bar()`: bind `l` to
            // the arrow with captured `this = o`, so `l()` walks the arrow body with
            // that `this` (and a never-called `l` emits nothing).
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(Expression::CallExpression(call)) = &declarator.init
            {
                let bound_closures = self.bound_closures_from_call(call, env);
                if !bound_closures.is_empty() {
                    env.bound_functions
                        .entry(name)
                        .or_default()
                        .extend(bound_closures);
                }
            }
            if let Some(name) = binding_identifier_name(&declarator.id)
                && let Some(init) = &declarator.init
            {
                let returned = self.callable_return_targets_from_expression(init, env, 0);
                if !returned.is_empty() {
                    env.bindings.insert(name, returned);
                }
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
            if let Some(init) = &declarator.init {
                let mut targets = self.object_targets_from_expression(init, env);
                // Object destructuring from a call to a same-file function that
                // returns an object (`const {a, b} = make()`) needs the returned
                // object's shape, which `object_targets_from_expression` (read-only)
                // cannot build; fall back to walking the callee body. Restricted to
                // object patterns so plain `const x = factory()` does not re-walk
                // the callee (it would re-emit its edges, occasionally over-resolved).
                if targets.is_none() && matches!(&declarator.id, BindingPattern::ObjectPattern(_)) {
                    targets = self.object_targets_from_local_function_call(init, env);
                }
                if let Some(targets) = targets {
                    self.collect_object_pattern_binding(&declarator.id, &targets, env);
                }
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
            Expression::NewExpression(new) => {
                // `new v()` where `v` is a variable bound to a class flowed through
                // it: emit the constructor edge (direct `new ClassName()` is already
                // resolved by the direct/MIR path).
                if let Some(name) = callee_identifier(&new.callee)
                    && let Some(key) = env.class_bindings.get(name)
                    && let Some(constructor) =
                        self.classes.get(key).and_then(|class| class.constructor)
                {
                    self.emit_call_span_targets(owner, new.span, "new", Some(vec![constructor]));
                }
                for argument in &new.arguments {
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
        self.collect_commonjs_export_assignment(assignment, env);
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

        let assignments = self.assignment_member_names(&assignment.left, env);
        if assignments.is_empty() {
            return;
        }
        let callable_targets = self
            .callable_targets_from_expression(&assignment.right, env)
            .all_targets();
        let nested_object = self.object_targets_from_expression(&assignment.right, env);
        let nested_collection = self.collection_targets_from_expression(&assignment.right, env);

        for (object_name, property) in assignments {
            if self.classes.contains_key(object_name)
                && !env.objects.contains_key(object_name)
                && let Some(statics) = self.class_static_targets(object_name)
            {
                env.objects.insert(object_name.to_string(), statics);
            }
            // A function declaration is also an object: `function f(){}; f.g = fn`
            // attaches `g` to f, so `f.g()` resolves and `this` inside `f.g` is the
            // f-object (Jelly tracks `this` to the function's allocation site).
            if !env.objects.contains_key(object_name)
                && self.function_declarations.contains_key(object_name)
            {
                env.objects
                    .insert(object_name.to_string(), ObjectTargets::default());
            }

            let setter_targets = env
                .objects
                .get(object_name)
                .and_then(|object| object.setter_properties.get(&property))
                .cloned()
                .unwrap_or_default();
            let handled_by_setter = !setter_targets.is_empty();
            if handled_by_setter {
                self.collect_setter_assignment_side_effects(
                    object_name,
                    &setter_targets,
                    &assignment.right,
                    env,
                );
            } else if let Some(object) = env.objects.get_mut(object_name) {
                add_assignment_to_object_property(
                    object,
                    property.clone(),
                    &callable_targets,
                    nested_object.clone(),
                    nested_collection.clone(),
                );
            }

            if self.classes.contains_key(object_name) {
                if handled_by_setter {
                    if let Some(object) = env.objects.get(object_name).cloned()
                        && let Some(class) = self.classes.get_mut(object_name)
                    {
                        class.static_object = object;
                    }
                } else if let Some(class) = self.classes.get_mut(object_name) {
                    add_assignment_to_object_property(
                        &mut class.static_object,
                        property,
                        &callable_targets,
                        nested_object.clone(),
                        nested_collection.clone(),
                    );
                }
                if let Some(statics) = self.class_static_targets(object_name) {
                    env.objects.insert(object_name.to_string(), statics);
                }
            }
        }
    }

    /// Capture CommonJS export writes into `self.exports`: `module.exports = X`,
    /// `module.exports.foo = X`, and `exports.foo = X`. This is what later
    /// rounds of the summary fixpoint (and requiring modules) read back.
    fn collect_commonjs_export_assignment(
        &mut self,
        assignment: &'ast oxc_ast::ast::AssignmentExpression<'ast>,
        env: &FlowEnv,
    ) {
        let Some(target) = export_assignment_target(&assignment.left) else {
            return;
        };
        let mut callables = self.callable_targets_from_expression(&assignment.right, env);
        // An identifier RHS may name a top-level function declaration (e.g.
        // `function foo() {}` ... `module.exports = foo`), which lives in
        // `function_declarations` rather than `env.bindings`.
        if let Expression::Identifier(identifier) = &assignment.right
            && let Some(flow) = self.function_declarations.get(identifier.name.as_str())
            && !callables.values.contains(&flow.function.id)
        {
            callables.values.push(flow.function.id);
        }
        let object = self.object_targets_from_expression(&assignment.right, env);
        let collection = self.collection_targets_from_expression(&assignment.right, env);
        // `module.exports = require('./x')` re-exports another module: fold its
        // summary in directly so callers see the re-exported shape.
        let reexport = match &assignment.right {
            Expression::CallExpression(call) => self.require_summary(call).cloned(),
            _ => None,
        };
        match target {
            ExportAssignmentTarget::WholeExports => {
                self.exports.callables.extend(callables);
                if let Some(object) = object {
                    self.exports.object.merge(object);
                }
                if let Some(reexport) = reexport {
                    self.exports.callables.extend(reexport.callables);
                    self.exports.object.merge(reexport.object);
                }
            }
            ExportAssignmentTarget::Property(name) => {
                for target in callables.all_targets() {
                    self.exports
                        .object
                        .add_property_target(name.clone(), target);
                }
                if let Some(object) = object {
                    self.exports
                        .object
                        .add_object_property(name.clone(), object);
                }
                if let Some(collection) = collection {
                    self.exports
                        .object
                        .add_collection_property(name, collection);
                }
            }
        }
    }

    /// Resolve a `require`/`import`/`export ... from` specifier string to the
    /// converged export summary of the target module, if any.
    fn summary_for_specifier(&self, specifier: &str) -> Option<&'env ModuleExportSummary> {
        let target = self
            .resolution_map
            .get(&(self.file.id, specifier.to_string()))?;
        self.module_summaries.get(target)
    }

    /// Bind ESM imports for this module into `env` before the body is walked.
    fn collect_esm_imports(&self, program: &'ast Program<'ast>, env: &mut FlowEnv) {
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            let Some(summary) = self.summary_for_specifier(import.source.value.as_str()) else {
                continue;
            };
            let summary = summary.clone();
            let Some(specifiers) = &import.specifiers else {
                continue;
            };
            for specifier in specifiers {
                match specifier {
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(default) => {
                        self.bind_imported_value(
                            &summary,
                            "default",
                            default.local.name.as_str(),
                            true,
                            env,
                        );
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(namespace) => {
                        if !summary.object.is_empty() {
                            env.objects
                                .insert(namespace.local.name.to_string(), summary.object.clone());
                        }
                        if !summary.callables.is_empty() {
                            env.bindings.insert(
                                namespace.local.name.to_string(),
                                summary.callables.clone(),
                            );
                        }
                    }
                    ImportDeclarationSpecifier::ImportSpecifier(named) => {
                        let imported = module_export_name_string(&named.imported);
                        self.bind_imported_value(
                            &summary,
                            &imported,
                            named.local.name.as_str(),
                            imported == "default",
                            env,
                        );
                    }
                }
            }
        }
    }

    /// Bind a single imported name `local` to the value of `key` in `summary`.
    fn bind_imported_value(
        &self,
        summary: &ModuleExportSummary,
        key: &str,
        local: &str,
        is_default: bool,
        env: &mut FlowEnv,
    ) {
        let mut callables = summary
            .object
            .properties
            .get(key)
            .cloned()
            .unwrap_or_default();
        // CommonJS default-interop: `import x from './cjs'` where the module did
        // `module.exports = fn` exposes the function as the default.
        if is_default {
            callables.extend(summary.callables.values.iter().copied());
        }
        if !callables.is_empty() {
            env.bindings.insert(
                local.to_string(),
                CollectionTargets {
                    keys: Vec::new(),
                    values: callables,
                    object_values: Vec::new(),
                },
            );
        }
        if let Some(object) = summary.object.object_properties.get(key) {
            env.objects.insert(local.to_string(), (**object).clone());
        } else if is_default && !summary.object.is_empty() && summary.callables.is_empty() {
            // CommonJS default-interop for object exports: the whole exports
            // object is the default value.
            env.objects
                .insert(local.to_string(), summary.object.clone());
        }
        if let Some(collection) = summary.object.collection_properties.get(key) {
            env.bindings.insert(local.to_string(), collection.clone());
        }
    }

    /// Capture ESM exports into `self.exports`.
    fn collect_esm_export(&mut self, statement: &'ast Statement<'ast>, env: &mut FlowEnv) {
        match statement {
            Statement::ExportNamedDeclaration(export) => {
                if let Some(source) = &export.source {
                    // Re-export: `export { a, b as c } from './m'`.
                    let Some(summary) = self.summary_for_specifier(source.value.as_str()) else {
                        return;
                    };
                    let summary = summary.clone();
                    for specifier in &export.specifiers {
                        let local = module_export_name_string(&specifier.local);
                        let exported = module_export_name_string(&specifier.exported);
                        self.reexport_property(&summary, &local, &exported);
                    }
                } else if let Some(declaration) = &export.declaration {
                    self.capture_exported_declaration(declaration, env);
                } else {
                    // Local: `export { a, b as c }`.
                    for specifier in &export.specifiers {
                        let local = module_export_name_string(&specifier.local);
                        let exported = module_export_name_string(&specifier.exported);
                        self.export_local_name(&local, &exported, env);
                    }
                }
            }
            Statement::ExportDefaultDeclaration(export) => {
                self.capture_default_export(&export.declaration, env);
            }
            Statement::ExportAllDeclaration(export) => {
                if export.exported.is_none()
                    && let Some(summary) = self.summary_for_specifier(export.source.value.as_str())
                {
                    let summary = summary.clone();
                    self.exports.object.merge(summary.object);
                    self.exports.callables.extend(summary.callables);
                }
            }
            _ => {}
        }
    }

    /// Copy property `local` from a re-exported module's summary into this
    /// module's exports under name `exported`.
    fn reexport_property(&mut self, summary: &ModuleExportSummary, local: &str, exported: &str) {
        let mut callables = summary
            .object
            .properties
            .get(local)
            .cloned()
            .unwrap_or_default();
        if local == "default" {
            callables.extend(summary.callables.values.iter().copied());
        }
        for target in callables {
            self.exports
                .object
                .add_property_target(exported.to_string(), target);
        }
        if let Some(object) = summary.object.object_properties.get(local) {
            self.exports
                .object
                .add_object_property(exported.to_string(), (**object).clone());
        }
        if let Some(collection) = summary.object.collection_properties.get(local) {
            self.exports
                .object
                .add_collection_property(exported.to_string(), collection.clone());
        }
    }

    /// Export a locally-bound name (`export { foo }` / `export { foo as bar }`).
    fn export_local_name(&mut self, local: &str, exported: &str, env: &FlowEnv) {
        if let Some(flow) = self.function_declarations.get(local) {
            self.exports
                .object
                .add_property_target(exported.to_string(), flow.function.id);
        }
        if let Some(targets) = env.bindings.get(local) {
            for target in targets.all_targets() {
                self.exports
                    .object
                    .add_property_target(exported.to_string(), target);
            }
        }
        if let Some(object) = env.objects.get(local) {
            self.exports
                .object
                .add_object_property(exported.to_string(), object.clone());
        }
    }

    /// Capture `export const/var/function/class ...` declarations.
    fn capture_exported_declaration(
        &mut self,
        declaration: &'ast Declaration<'ast>,
        env: &mut FlowEnv,
    ) {
        match declaration {
            Declaration::FunctionDeclaration(function) => {
                if let Some(id) = &function.id
                    && let Some(fact) = self.function_for_span(function.span)
                {
                    self.exports
                        .object
                        .add_property_target(id.name.to_string(), fact.id);
                }
            }
            Declaration::VariableDeclaration(variable) => {
                self.collect_variable_declaration(variable, self.module, env);
                for declarator in &variable.declarations {
                    if let Some(name) = binding_identifier_name(&declarator.id) {
                        self.export_local_name(&name, &name, env);
                    }
                }
            }
            _ => {}
        }
    }

    /// Capture `export default ...`.
    fn capture_default_export(
        &mut self,
        declaration: &'ast ExportDefaultDeclarationKind<'ast>,
        env: &FlowEnv,
    ) {
        let mut callables = CollectionTargets::default();
        match declaration {
            ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                if let Some(fact) = self.function_for_span(function.span) {
                    callables.values.push(fact.id);
                }
            }
            expression => {
                if let Some(expression) = expression.as_expression() {
                    callables = self.callable_targets_from_expression(expression, env);
                    if let Some(object) = self.object_targets_from_expression(expression, env) {
                        self.exports
                            .object
                            .add_object_property("default".to_string(), object);
                    }
                }
            }
        }
        for target in callables.all_targets() {
            self.exports
                .object
                .add_property_target("default".to_string(), target);
            self.exports.callables.values.push(target);
        }
        self.exports.callables.values.sort();
        self.exports.callables.values.dedup();
    }

    fn assignment_member_names(
        &self,
        target: &'ast AssignmentTarget<'ast>,
        env: &FlowEnv,
    ) -> Vec<(&'ast str, String)> {
        match target {
            AssignmentTarget::StaticMemberExpression(member) => {
                let Some(object) = expression_identifier(&member.object) else {
                    return Vec::new();
                };
                vec![(object, member.property.name.to_string())]
            }
            AssignmentTarget::ComputedMemberExpression(member) => {
                let Some(object) = expression_identifier(&member.object) else {
                    return Vec::new();
                };
                self.computed_member_property_names(&member.expression, env)
                    .into_iter()
                    .map(|property| (object, property))
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    fn computed_member_property_names(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Vec<String> {
        let mut properties = match expression {
            Expression::NumericLiteral(literal) => vec![literal.value.to_string()],
            Expression::ParenthesizedExpression(expression) => {
                self.computed_member_property_names(&expression.expression, env)
            }
            _ => self.constant_strings_from_expression(expression, env),
        };
        sort_dedup_strings(&mut properties);
        properties.truncate(8);
        properties
    }

    fn constant_string_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<String> {
        self.constant_strings_from_expression(expression, env)
            .into_iter()
            .next()
    }

    fn constant_strings_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Vec<String> {
        match expression {
            Expression::StringLiteral(literal) => vec![literal.value.to_string()],
            Expression::Identifier(identifier) => env
                .string_bindings
                .get(identifier.name.as_str())
                .cloned()
                .into_iter()
                .collect(),
            Expression::BinaryExpression(binary) if binary.operator == BinaryOperator::Addition => {
                let left = self.constant_strings_from_expression(&binary.left, env);
                let right = self.constant_strings_from_expression(&binary.right, env);
                if left.is_empty() || right.is_empty() {
                    return Vec::new();
                }
                bounded_string_product(&left, &right)
            }
            Expression::ConditionalExpression(conditional) => {
                if let Some(value) = self.constant_bool_from_expression(&conditional.test, env) {
                    if value {
                        self.constant_strings_from_expression(&conditional.consequent, env)
                    } else {
                        self.constant_strings_from_expression(&conditional.alternate, env)
                    }
                } else {
                    let mut values =
                        self.constant_strings_from_expression(&conditional.consequent, env);
                    values
                        .extend(self.constant_strings_from_expression(&conditional.alternate, env));
                    sort_dedup_strings(&mut values);
                    values.truncate(8);
                    values
                }
            }
            Expression::ComputedMemberExpression(member) => {
                let Some(source) = expression_identifier(&member.object)
                    .and_then(|name| env.string_arrays.get(name))
                else {
                    return Vec::new();
                };
                let Some(index) = numeric_index(&member.expression) else {
                    return Vec::new();
                };
                source.get(index).cloned().into_iter().collect()
            }
            Expression::TemplateLiteral(template) if template.expressions.is_empty() => template
                .quasis
                .iter()
                .map(|quasi| {
                    quasi
                        .value
                        .cooked
                        .as_ref()
                        .unwrap_or(&quasi.value.raw)
                        .to_string()
                })
                .collect(),
            Expression::TemplateLiteral(template) => {
                let mut values = vec![String::new()];
                for (index, quasi) in template.quasis.iter().enumerate() {
                    let text = quasi
                        .value
                        .cooked
                        .as_ref()
                        .unwrap_or(&quasi.value.raw)
                        .to_string();
                    for value in &mut values {
                        value.push_str(&text);
                    }
                    if let Some(expression) = template.expressions.get(index) {
                        let parts = self.constant_strings_from_expression(expression, env);
                        if parts.is_empty() {
                            return Vec::new();
                        }
                        values = bounded_string_product(&values, &parts);
                    }
                }
                values
            }
            Expression::ParenthesizedExpression(expression) => {
                self.constant_strings_from_expression(&expression.expression, env)
            }
            _ => Vec::new(),
        }
    }

    fn constant_bool_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<bool> {
        match expression {
            Expression::BooleanLiteral(literal) => Some(literal.value),
            Expression::Identifier(identifier) => {
                env.bool_bindings.get(identifier.name.as_str()).copied()
            }
            Expression::BinaryExpression(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Equality | BinaryOperator::StrictEquality
                ) =>
            {
                if let (Some(left), Some(right)) = (
                    self.constant_bool_from_expression(&binary.left, env),
                    self.constant_bool_from_expression(&binary.right, env),
                ) {
                    return Some(left == right);
                }
                if let (Some(left), Some(right)) = (
                    self.constant_string_from_expression(&binary.left, env),
                    self.constant_string_from_expression(&binary.right, env),
                ) {
                    return Some(left == right);
                }
                None
            }
            Expression::BinaryExpression(binary)
                if matches!(
                    binary.operator,
                    BinaryOperator::Inequality | BinaryOperator::StrictInequality
                ) =>
            {
                if let (Some(left), Some(right)) = (
                    self.constant_bool_from_expression(&binary.left, env),
                    self.constant_bool_from_expression(&binary.right, env),
                ) {
                    return Some(left != right);
                }
                if let (Some(left), Some(right)) = (
                    self.constant_string_from_expression(&binary.left, env),
                    self.constant_string_from_expression(&binary.right, env),
                ) {
                    return Some(left != right);
                }
                None
            }
            Expression::ParenthesizedExpression(expression) => {
                self.constant_bool_from_expression(&expression.expression, env)
            }
            _ => None,
        }
    }

    fn string_array_from_literal(
        &self,
        array: &'ast oxc_ast::ast::ArrayExpression<'ast>,
        env: &FlowEnv,
    ) -> Option<Vec<String>> {
        let mut values = Vec::new();
        for element in &array.elements {
            let expression = array_element_expression(element)?;
            let value = self.constant_string_from_expression(expression, env)?;
            values.push(value);
        }
        Some(values)
    }

    fn property_key_names(
        &self,
        key: &'ast PropertyKey<'ast>,
        computed: bool,
        env: Option<&FlowEnv>,
    ) -> Vec<String> {
        if !computed {
            return static_property_key(key, false).into_iter().collect();
        }

        let empty;
        let env = if let Some(env) = env {
            env
        } else {
            empty = FlowEnv::default();
            &empty
        };

        let mut names = match key {
            PropertyKey::StringLiteral(literal) => vec![literal.value.to_string()],
            PropertyKey::NumericLiteral(literal) => vec![literal.value.to_string()],
            PropertyKey::Identifier(identifier) => env
                .string_bindings
                .get(identifier.name.as_str())
                .cloned()
                .into_iter()
                .collect(),
            PropertyKey::BinaryExpression(binary)
                if binary.operator == BinaryOperator::Addition =>
            {
                let left = self.constant_strings_from_expression(&binary.left, env);
                let right = self.constant_strings_from_expression(&binary.right, env);
                if left.is_empty() || right.is_empty() {
                    Vec::new()
                } else {
                    bounded_string_product(&left, &right)
                }
            }
            PropertyKey::ConditionalExpression(conditional) => {
                if let Some(value) = self.constant_bool_from_expression(&conditional.test, env) {
                    if value {
                        self.constant_strings_from_expression(&conditional.consequent, env)
                    } else {
                        self.constant_strings_from_expression(&conditional.alternate, env)
                    }
                } else {
                    let mut values =
                        self.constant_strings_from_expression(&conditional.consequent, env);
                    values
                        .extend(self.constant_strings_from_expression(&conditional.alternate, env));
                    values
                }
            }
            PropertyKey::ComputedMemberExpression(member) => {
                let Some(source) = expression_identifier(&member.object)
                    .and_then(|name| env.string_arrays.get(name))
                else {
                    return Vec::new();
                };
                let Some(index) = numeric_index(&member.expression) else {
                    return Vec::new();
                };
                source.get(index).cloned().into_iter().collect()
            }
            PropertyKey::TemplateLiteral(template) if template.expressions.is_empty() => template
                .quasis
                .iter()
                .map(|quasi| {
                    quasi
                        .value
                        .cooked
                        .as_ref()
                        .unwrap_or(&quasi.value.raw)
                        .to_string()
                })
                .collect(),
            PropertyKey::ParenthesizedExpression(expression) => {
                self.constant_strings_from_expression(&expression.expression, env)
            }
            _ => Vec::new(),
        };
        sort_dedup_strings(&mut names);
        names.truncate(8);
        names
    }

    fn collect_call_callee_expression(
        &mut self,
        callee: &'ast Expression<'ast>,
        owner: &'db FunctionFact,
        env: &mut FlowEnv,
    ) {
        match callee {
            Expression::CallExpression(call) => {
                self.collect_call_expression(call, owner, env);
                self.collect_call_callee_expression(&call.callee, owner, env);
                for argument in &call.arguments {
                    if let Some(expression) = argument_expression(argument) {
                        self.collect_expression(expression, owner, env);
                    }
                }
            }
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
        self.collect_native_object_call(call, env);

        if let Some((source, method)) = self.promise_method_source(call, env) {
            self.collect_promise_handler_flows(call, method, &source, env);
        }

        if let Some(targets) = self.collect_function_prototype_method_call(call, owner, env)
            && !targets.is_empty()
        {
            self.emit_call_span_targets(
                owner,
                call.span,
                "call-apply-bind",
                Some(targets.all_targets()),
            );
        }

        if call_expression_callee(&call.callee).is_some() {
            let returned = self.callable_return_targets_from_expression(&call.callee, env, 0);
            self.emit_call_span_targets(
                owner,
                call.span,
                "call-result",
                Some(returned.all_targets()),
            );
            // `o.foo()()`: the result of `o.foo()` is a `this`-capturing arrow; walk
            // its body with the captured `this = o` now that it is actually invoked.
            if let Expression::CallExpression(inner) = &call.callee {
                for bound in self.bound_closures_from_call(inner, env) {
                    self.collect_bound_function_invocation(&bound, call, env);
                }
            }
        }

        if matches!(&call.callee, Expression::ThisExpression(_)) {
            self.emit_call_span_targets(
                owner,
                call.span,
                "this",
                Some(env.this_callables.all_targets()),
            );
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
            if matches!(method, "push" | "add") {
                let additions = call
                    .arguments
                    .iter()
                    .filter_map(argument_expression)
                    .filter_map(|expression| self.targets_from_argument_expression(expression, env))
                    .collect::<Vec<_>>();
                if let Some(targets) = env.bindings.get_mut(collection_name) {
                    for argument_targets in additions {
                        targets.values.extend(argument_targets.all_targets());
                        targets.object_values.extend(argument_targets.object_values);
                        targets.values.sort();
                        targets.values.dedup();
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

            let getter = env.objects.get(object_name).and_then(|object| {
                object
                    .getter_properties
                    .get(property)
                    .cloned()
                    .map(|targets| (targets, object.clone()))
            });
            if let Some((getters, receiver)) = getter {
                let (returned, receiver) = self.collect_getter_return_targets(getters, receiver, 1);
                env.objects
                    .insert(object_name.to_string(), receiver.clone());
                let targets = returned.all_targets();
                if !targets.is_empty() {
                    self.emit_call_span_targets(owner, call.span, property, Some(targets.clone()));
                    self.emit_member_targets(owner, property, call.span, Some(targets.clone()));
                    self.collect_returned_callable_invocation(
                        Some(object_name),
                        &targets,
                        call,
                        receiver,
                        env,
                    );
                }
            }
        }
        if let Expression::StaticMemberExpression(member) = &call.callee
            && let Some(object) = self.object_targets_from_expression(&member.object, env)
            && let Some(targets) = object
                .properties
                .get(member.property.name.as_str())
                .cloned()
        {
            self.emit_call_span_targets(
                owner,
                call.span,
                member.property.name.as_str(),
                Some(targets.clone()),
            );
            self.emit_member_targets(
                owner,
                member.property.name.as_str(),
                call.span,
                Some(targets),
            );
        }

        // `super.m()` / `super.s()`: resolve against the super-class member
        // object bound while walking this class method body.
        if let Expression::StaticMemberExpression(member) = &call.callee
            && matches!(&member.object, Expression::Super(_))
            && let Some(super_object) = &self.current_super
            && let Some(targets) = super_object
                .properties
                .get(member.property.name.as_str())
                .cloned()
        {
            self.emit_call_span_targets(
                owner,
                call.span,
                member.property.name.as_str(),
                Some(targets.clone()),
            );
            self.emit_member_targets(
                owner,
                member.property.name.as_str(),
                call.span,
                Some(targets),
            );
        }

        // Private member calls: `this.#foo()`, `Class.#baz()`. The callee is a
        // PrivateFieldExpression; private members are keyed by `#name`.
        if let Expression::PrivateFieldExpression(member) = &call.callee {
            let property = format!("#{}", member.field.name);
            if let Some(object) = self.object_targets_from_expression(&member.object, env)
                && let Some(targets) = object.properties.get(&property).cloned()
            {
                self.emit_call_span_targets(owner, call.span, &property, Some(targets.clone()));
                self.emit_member_targets(owner, &property, call.span, Some(targets));
            }
        }

        if let Expression::Identifier(identifier) = &call.callee {
            if let Some(flow) = self
                .function_declarations
                .get(identifier.name.as_str())
                .cloned()
            {
                self.collect_function_parameter_flows(&flow, call, env);
            }
            if let Some(bound_targets) = env.bound_functions.get(identifier.name.as_str()).cloned()
            {
                for bound in bound_targets {
                    self.collect_bound_function_invocation(&bound, call, env);
                }
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
        if let Expression::ComputedMemberExpression(member) = &call.callee {
            let object_name = expression_identifier(&member.object);
            if let Some(object) = self.object_targets_from_expression(&member.object, env) {
                for property in self.computed_member_property_names(&member.expression, env) {
                    if let Some(targets) = object.properties.get(&property).cloned() {
                        self.emit_call_span_targets(
                            owner,
                            call.span,
                            &property,
                            Some(targets.clone()),
                        );
                        self.emit_member_targets(
                            owner,
                            &property,
                            call.span,
                            Some(targets.clone()),
                        );
                        if let Some(object_name) = object_name {
                            self.collect_receiver_side_effects(object_name, &targets, call, env);
                        }
                    }
                    if let Some(getters) = object.getter_properties.get(&property).cloned() {
                        let receiver = object_name
                            .and_then(|name| env.objects.get(name).cloned())
                            .unwrap_or_else(|| object.clone());
                        let (returned, receiver) =
                            self.collect_getter_return_targets(getters, receiver, 1);
                        if let Some(object_name) = object_name {
                            env.objects
                                .insert(object_name.to_string(), receiver.clone());
                        }
                        let targets = returned.all_targets();
                        if !targets.is_empty() {
                            self.emit_call_span_targets(
                                owner,
                                call.span,
                                &property,
                                Some(targets.clone()),
                            );
                            self.emit_member_targets(
                                owner,
                                &property,
                                call.span,
                                Some(targets.clone()),
                            );
                            self.collect_returned_callable_invocation(
                                object_name,
                                &targets,
                                call,
                                receiver,
                                env,
                            );
                        }
                    }
                }
            }

            if let Some(source) = self.collection_targets_from_expression(&member.object, env)
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
    }

    fn collect_native_object_call(&mut self, call: &'ast CallExpression<'ast>, env: &mut FlowEnv) {
        match callee_text(&call.callee).as_deref() {
            Some("Object.assign") => {
                let Some(target_name) = call
                    .arguments
                    .first()
                    .and_then(argument_expression)
                    .and_then(expression_identifier)
                else {
                    return;
                };
                let mut merged = ObjectTargets::default();
                for source in call
                    .arguments
                    .iter()
                    .skip(1)
                    .filter_map(argument_expression)
                {
                    if let Some(targets) = self.object_targets_from_expression(source, env) {
                        merged.merge(targets);
                    }
                }
                if !merged.is_empty()
                    && let Some(target) = env.objects.get_mut(target_name)
                {
                    target.merge(merged);
                }
            }
            Some("Object.defineProperty") => {
                let Some(target_name) = call
                    .arguments
                    .first()
                    .and_then(argument_expression)
                    .and_then(expression_identifier)
                else {
                    return;
                };
                let Some(property) = call
                    .arguments
                    .get(1)
                    .and_then(argument_expression)
                    .and_then(|expression| self.constant_string_from_expression(expression, env))
                else {
                    return;
                };
                let descriptor = call
                    .arguments
                    .get(2)
                    .and_then(argument_expression)
                    .and_then(|expression| self.object_targets_from_expression(expression, env));
                if let (Some(target), Some(descriptor)) =
                    (env.objects.get_mut(target_name), descriptor)
                {
                    copy_descriptor_value_to_property(target, property, &descriptor);
                }
            }
            Some("Object.defineProperties") => {
                let Some(target_name) = call
                    .arguments
                    .first()
                    .and_then(argument_expression)
                    .and_then(expression_identifier)
                else {
                    return;
                };
                let descriptors = call
                    .arguments
                    .get(1)
                    .and_then(argument_expression)
                    .and_then(|expression| self.object_targets_from_expression(expression, env));
                if let (Some(target), Some(descriptors)) =
                    (env.objects.get_mut(target_name), descriptors)
                {
                    target.merge(descriptors);
                }
            }
            _ => {}
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
        let arguments = self.argument_targets_from_call(call, env);
        let object_arguments = self.object_arguments_from_call(call, env);
        let mut callee_env = FlowEnv::default();
        self.bind_invocation_arguments(flow, &arguments, &object_arguments, &mut callee_env);
        self.collect_function_flow_invocation(flow, &mut callee_env);
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

    fn collect_setter_assignment_side_effects(
        &mut self,
        object_name: &str,
        targets: &[FunctionId],
        value: &'ast Expression<'ast>,
        env: &mut FlowEnv,
    ) {
        let Some(receiver) = env.objects.get(object_name).cloned() else {
            return;
        };
        let arguments = vec![
            self.targets_from_argument_expression(value, env)
                .unwrap_or_default(),
        ];
        let object_arguments = vec![self.object_targets_from_expression(value, env)];
        let mut merged_receiver = receiver.clone();

        for target in targets {
            let Some(flow) = self.function_flows_by_id.get(target).cloned() else {
                continue;
            };
            let mut callee_env = env.clone();
            callee_env.this_object = receiver.clone();
            self.bind_invocation_arguments(&flow, &arguments, &object_arguments, &mut callee_env);
            self.collect_function_flow_invocation(&flow, &mut callee_env);
            merged_receiver.merge(callee_env.this_object);
        }

        env.objects.insert(object_name.to_string(), merged_receiver);
    }

    fn collect_getter_return_targets(
        &mut self,
        getters: Vec<FunctionId>,
        receiver: ObjectTargets,
        depth: usize,
    ) -> (CollectionTargets, ObjectTargets) {
        if depth > 8 {
            return (CollectionTargets::default(), receiver);
        }

        let mut returned = CollectionTargets::default();
        let mut merged_receiver = receiver.clone();
        for target in getters {
            let Some(flow) = self.function_flows_by_id.get(&target).cloned() else {
                continue;
            };
            let mut callee_env = FlowEnv {
                this_object: receiver.clone(),
                ..FlowEnv::default()
            };
            returned.extend(self.collect_function_flow_invocation(&flow, &mut callee_env));
            merged_receiver.merge(callee_env.this_object);
        }

        (returned, merged_receiver)
    }

    /// Bind an object-destructuring pattern against a resolved source object,
    /// handling forms the free `bind_object_pattern` cannot (they need the
    /// collector): nested object patterns (`{b: {c: y}}`), getter-valued sources
    /// (`{bar: y}` where `bar` is a getter), and default values used when the
    /// property is absent (`{d: y = () => {}}`).
    fn collect_object_pattern_binding(
        &mut self,
        pattern: &'ast BindingPattern<'ast>,
        source: &ObjectTargets,
        env: &mut FlowEnv,
    ) {
        match pattern {
            BindingPattern::BindingIdentifier(identifier) => {
                env.objects
                    .insert(identifier.name.to_string(), source.clone());
            }
            BindingPattern::ObjectPattern(object) => {
                let mut used = Vec::new();
                for property in &object.properties {
                    let Some(name) = static_property_key(&property.key, property.computed) else {
                        continue;
                    };
                    used.push(name.clone());
                    self.collect_object_pattern_property(&property.value, source, &name, env);
                }
                if let Some(rest) = &object.rest {
                    self.collect_object_pattern_binding(
                        &rest.argument,
                        &source.without_properties(&used),
                        env,
                    );
                }
            }
            BindingPattern::AssignmentPattern(pattern) => {
                self.collect_object_pattern_binding(&pattern.left, source, env);
            }
            _ => {}
        }
    }

    fn collect_object_pattern_property(
        &mut self,
        value: &'ast BindingPattern<'ast>,
        source: &ObjectTargets,
        name: &str,
        env: &mut FlowEnv,
    ) {
        // The pattern target (`value`), unwrapping a `= default` wrapper.
        let target = match value {
            BindingPattern::AssignmentPattern(pattern) => &pattern.left,
            other => other,
        };
        let nested_object = source
            .object_properties
            .get(name)
            .map(|nested| (**nested).clone());
        let mut callables = source.properties.get(name).cloned().unwrap_or_default();
        // Getter-valued source: a `get name()` resolves to its return value.
        if let Some(getters) = source.getter_properties.get(name).cloned() {
            let (returned, _) = self.collect_getter_return_targets(getters, source.clone(), 1);
            callables.extend(returned.all_targets());
        }
        // Default value: used only when the property KEY is absent from the source
        // (JS uses the default iff the property is `undefined`, regardless of whether
        // we could resolve a present value; keying on `callables.is_empty()` instead
        // would bind the dead default for a present-but-unresolved property — a
        // false positive). Jelly's reachability-pruned oracle excludes a present
        // property's dead default.
        let key_present = source.properties.contains_key(name)
            || source.getter_properties.contains_key(name)
            || source.setter_properties.contains_key(name)
            || source.object_properties.contains_key(name)
            || source.collection_properties.contains_key(name);
        if !key_present && let BindingPattern::AssignmentPattern(pattern) = value {
            callables.extend(
                self.callable_targets_from_expression(&pattern.right, env)
                    .all_targets(),
            );
        }
        callables.sort();
        callables.dedup();

        // Nested object pattern: recurse against the nested object's targets.
        if let BindingPattern::ObjectPattern(_) = target
            && let Some(nested) = nested_object
        {
            self.collect_object_pattern_binding(target, &nested, env);
            return;
        }
        if let BindingPattern::BindingIdentifier(identifier) = target {
            if let Some(nested) = &nested_object {
                env.objects
                    .insert(identifier.name.to_string(), nested.clone());
            }
            if !callables.is_empty() {
                env.bindings.insert(
                    identifier.name.to_string(),
                    CollectionTargets {
                        keys: Vec::new(),
                        values: callables,
                        object_values: Vec::new(),
                    },
                );
            }
            return;
        }
        // Array/other pattern: bind the collected callables positionally.
        bind_collection_pattern(
            target,
            &CollectionTargets {
                keys: Vec::new(),
                values: callables,
                object_values: Vec::new(),
            },
            env,
        );
    }

    fn collect_returned_callable_invocation(
        &mut self,
        object_name: Option<&str>,
        targets: &[FunctionId],
        call: &'ast CallExpression<'ast>,
        receiver: ObjectTargets,
        env: &mut FlowEnv,
    ) {
        let receiver = object_name
            .and_then(|name| env.objects.get(name).cloned())
            .unwrap_or(receiver);
        let arguments = self.argument_targets_from_call(call, env);
        let object_arguments = self.object_arguments_from_call(call, env);
        let mut merged_receiver = receiver.clone();

        for target in targets {
            let Some(flow) = self.function_flows_by_id.get(target).cloned() else {
                continue;
            };
            let mut callee_env = env.clone();
            callee_env.this_object = receiver.clone();
            self.bind_invocation_arguments(&flow, &arguments, &object_arguments, &mut callee_env);
            self.collect_function_flow_invocation(&flow, &mut callee_env);
            merged_receiver.merge(callee_env.this_object);
        }

        if let Some(object_name) = object_name {
            env.objects.insert(object_name.to_string(), merged_receiver);
        }
    }

    fn bind_call_arguments_to_flow(
        &self,
        flow: &FunctionFlow<'db, 'ast>,
        call: &'ast CallExpression<'ast>,
        parent_env: &FlowEnv,
        callee_env: &mut FlowEnv,
    ) {
        let arguments = self.argument_targets_from_call(call, parent_env);
        let object_arguments = self.object_arguments_from_call(call, parent_env);
        self.bind_invocation_arguments(flow, &arguments, &object_arguments, callee_env);
    }

    fn collect_function_prototype_method_call(
        &mut self,
        call: &'ast CallExpression<'ast>,
        owner: &'db FunctionFact,
        env: &FlowEnv,
    ) -> Option<CollectionTargets> {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        let method = member.property.name.as_str();
        if !matches!(method, "call" | "apply" | "bind") {
            return None;
        }
        let target_ids = self.function_target_ids_from_expression(&member.object, env);
        if target_ids.is_empty() {
            return None;
        }
        let targets = CollectionTargets {
            keys: Vec::new(),
            values: target_ids.clone(),
            object_values: Vec::new(),
        };
        if method == "bind" {
            return Some(targets);
        }

        let (this_object, this_callables, arguments, object_arguments) =
            self.native_call_receiver_and_arguments(call, method, env);
        for target in target_ids {
            let Some(flow) = self.function_flows_by_id.get(&target).cloned() else {
                continue;
            };
            let mut callee_env = FlowEnv {
                this_object: this_object.clone(),
                this_callables: this_callables.clone(),
                ..FlowEnv::default()
            };
            self.bind_invocation_arguments(&flow, &arguments, &object_arguments, &mut callee_env);
            self.collect_function_flow_invocation(&flow, &mut callee_env);
        }

        self.emit_call_span_targets(owner, call.span, method, Some(targets.all_targets()));
        self.emit_containing_call_span_targets(
            owner,
            call.span,
            method,
            Some(targets.all_targets()),
        );
        Some(targets)
    }

    fn collect_bound_function_invocation(
        &mut self,
        bound: &BoundFunctionTarget,
        call: &'ast CallExpression<'ast>,
        parent_env: &FlowEnv,
    ) {
        let Some(flow) = self.function_flows_by_id.get(&bound.function).cloned() else {
            return;
        };
        let mut arguments = bound.bound_arguments.clone();
        arguments.extend(self.argument_targets_from_call(call, parent_env));
        let mut object_arguments = vec![None; bound.bound_arguments.len()];
        object_arguments.extend(self.object_arguments_from_call(call, parent_env));
        let mut callee_env = FlowEnv {
            this_object: bound.this_object.clone(),
            this_callables: bound.this_callables.clone(),
            ..FlowEnv::default()
        };
        self.bind_invocation_arguments(&flow, &arguments, &object_arguments, &mut callee_env);
        self.collect_function_flow_invocation(&flow, &mut callee_env);
    }

    fn bind_invocation_arguments(
        &self,
        flow: &FunctionFlow<'db, 'ast>,
        arguments: &[CollectionTargets],
        object_arguments: &[Option<ObjectTargets>],
        callee_env: &mut FlowEnv,
    ) {
        let mut all_arguments = CollectionTargets::default();
        for argument in arguments {
            all_arguments.extend(argument.clone());
        }
        callee_env
            .bindings
            .insert("arguments".to_string(), all_arguments);

        for (index, pattern) in flow.params.iter().enumerate() {
            if let Some(targets) = arguments.get(index) {
                bind_param_pattern(pattern, targets, callee_env);
            }
            if let Some(Some(targets)) = object_arguments.get(index) {
                bind_param_object_pattern(pattern, targets, callee_env);
            }
        }
        if let Some(rest) = &flow.rest {
            let mut targets = CollectionTargets::default();
            for argument in arguments.iter().skip(flow.params.len()) {
                targets.extend(argument.clone());
            }
            bind_param_pattern(rest, &targets, callee_env);
        }
    }

    fn collect_function_flow_invocation(
        &mut self,
        flow: &FunctionFlow<'db, 'ast>,
        env: &mut FlowEnv,
    ) -> CollectionTargets {
        // Bound nesting: a self-/mutually-returning function would otherwise recurse
        // forever here (the per-call `depth` is reset to 0 each time a return
        // statement re-enters `callable_return_targets_from_expression`).
        if self.invocation_depth > 16 {
            return CollectionTargets::default();
        }
        self.invocation_depth += 1;
        // Calls in the invoked body belong to `flow.function`, not to a
        // `caller_override` active in an enclosing constructor/field body.
        let saved_override = self.caller_override.take();
        let result = if let Some(expression) = flow.expression_body {
            self.collect_expression(expression, flow.function, env);
            self.callable_return_targets_from_expression(expression, env, 0)
        } else {
            self.collect_invocation_statements(&flow.body.statements, flow.function, env)
        };
        self.caller_override = saved_override;
        self.invocation_depth -= 1;
        result
    }

    /// Bound closures produced by `recv.m()` when `m` returns an **arrow** that
    /// captures `this`: the arrow is bound to `this = recv`, so a later invocation
    /// (`const l = o.foo(); l()` or `o.foo()()`) walks the arrow body with the
    /// captured `this` and emits `this.bar()` -> `o.bar` — but only when the arrow
    /// is actually invoked (no edge for a returned-but-uncalled closure). Restricted
    /// to arrows: a returned `function` expression gets its own `this` at call time.
    fn bound_closures_from_call(
        &self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) -> Vec<BoundFunctionTarget> {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return Vec::new();
        };
        if matches!(member.property.name.as_str(), "call" | "apply" | "bind") {
            return Vec::new();
        }
        let Some(receiver) = self.object_targets_from_expression(&member.object, env) else {
            return Vec::new();
        };
        let Some(method_ids) = receiver.properties.get(member.property.name.as_str()) else {
            return Vec::new();
        };
        let mut bound = Vec::new();
        for method_id in method_ids {
            let Some(flow) = self.function_flows_by_id.get(method_id) else {
                continue;
            };
            let returned = flow
                .expression_body
                .or_else(|| returned_expression_from_statements(&flow.body.statements));
            let Some(returned) = returned else {
                continue;
            };
            let returned = match returned {
                Expression::ParenthesizedExpression(inner) => &inner.expression,
                other => other,
            };
            if matches!(returned, Expression::ArrowFunctionExpression(_))
                && let Some(arrow) = self.function_for_expression(returned)
            {
                bound.push(BoundFunctionTarget {
                    function: arrow.id,
                    this_object: receiver.clone(),
                    this_callables: CollectionTargets::default(),
                    bound_arguments: Vec::new(),
                });
            }
        }
        bound
    }

    fn collect_invocation_statements(
        &mut self,
        statements: &'ast oxc_allocator::Vec<'ast, Statement<'ast>>,
        owner: &'db FunctionFact,
        env: &mut FlowEnv,
    ) -> CollectionTargets {
        for statement in statements {
            let returned = self.collect_invocation_statement(statement, owner, env);
            if !returned.is_empty() {
                return returned;
            }
        }
        CollectionTargets::default()
    }

    fn collect_invocation_statement(
        &mut self,
        statement: &'ast Statement<'ast>,
        owner: &'db FunctionFact,
        env: &mut FlowEnv,
    ) -> CollectionTargets {
        match statement {
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.collect_expression(argument, owner, env);
                    self.callable_return_targets_from_expression(argument, env, 0)
                } else {
                    CollectionTargets::default()
                }
            }
            Statement::BlockStatement(block) => {
                self.collect_invocation_statements(&block.body, owner, env)
            }
            Statement::IfStatement(statement) => {
                self.collect_expression(&statement.test, owner, env);
                let mut targets =
                    self.collect_invocation_statement(&statement.consequent, owner, env);
                if let Some(alternate) = &statement.alternate {
                    targets.extend(self.collect_invocation_statement(alternate, owner, env));
                }
                targets
            }
            _ => {
                self.collect_statement(statement, owner, env);
                CollectionTargets::default()
            }
        }
    }

    fn callable_return_targets_from_expression(
        &mut self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
        depth: usize,
    ) -> CollectionTargets {
        if depth > 8 {
            return CollectionTargets::default();
        }
        match expression {
            Expression::CallExpression(call) => {
                self.callable_return_targets_from_call(call, env, depth + 1)
            }
            Expression::StaticMemberExpression(member) => {
                if let Some(object) = self.object_targets_from_expression(&member.object, env)
                    && let Some(getters) = object
                        .getter_properties
                        .get(member.property.name.as_str())
                        .cloned()
                {
                    return self.callable_return_targets_from_function_ids(
                        getters,
                        Vec::new(),
                        Vec::new(),
                        object,
                        CollectionTargets::default(),
                        depth + 1,
                    );
                }
                self.callable_targets_from_expression(expression, env)
            }
            Expression::ComputedMemberExpression(member) => {
                if let Some(object) = self.object_targets_from_expression(&member.object, env)
                    && let Some(property) =
                        self.constant_string_from_expression(&member.expression, env)
                    && let Some(getters) = object.getter_properties.get(&property).cloned()
                {
                    return self.callable_return_targets_from_function_ids(
                        getters,
                        Vec::new(),
                        Vec::new(),
                        object,
                        CollectionTargets::default(),
                        depth + 1,
                    );
                }
                self.callable_targets_from_expression(expression, env)
            }
            Expression::ParenthesizedExpression(expression) => {
                self.callable_return_targets_from_expression(&expression.expression, env, depth)
            }
            _ => self.callable_targets_from_expression(expression, env),
        }
    }

    fn callable_return_targets_from_call(
        &mut self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
        depth: usize,
    ) -> CollectionTargets {
        if depth > 8 {
            return CollectionTargets::default();
        }
        if let Some(name) = callee_identifier(&call.callee) {
            let mut targets = CollectionTargets::default();
            if let Some(flow) = self.function_declarations.get(name).cloned() {
                let arguments = self.argument_targets_from_call(call, env);
                let object_arguments = self.object_arguments_from_call(call, env);
                let mut callee_env = FlowEnv::default();
                self.bind_invocation_arguments(
                    &flow,
                    &arguments,
                    &object_arguments,
                    &mut callee_env,
                );
                targets.extend(self.collect_function_flow_invocation(&flow, &mut callee_env));
            }
            if let Some(bound_targets) = env.bound_functions.get(name).cloned() {
                for bound in bound_targets {
                    targets.extend(self.callable_return_targets_from_bound_function(
                        &bound,
                        call,
                        env,
                        depth + 1,
                    ));
                }
            }
            return targets;
        }

        if let Expression::ThisExpression(_) = &call.callee {
            return self.callable_return_targets_from_function_ids(
                env.this_callables.all_targets(),
                self.argument_targets_from_call(call, env),
                self.object_arguments_from_call(call, env),
                env.this_object.clone(),
                env.this_callables.clone(),
                depth + 1,
            );
        }

        if let Expression::StaticMemberExpression(member) = &call.callee {
            let method = member.property.name.as_str();
            // Regular method call `recv.m()` whose body returns a value: invoke it
            // with `this` bound to the receiver and propagate the return value
            // (e.g. `x.p()` where `p` returns `this.q`).
            if !matches!(method, "call" | "apply" | "bind")
                && let Some(receiver) = self.object_targets_from_expression(&member.object, env)
                && let Some(method_targets) = receiver.properties.get(method).cloned()
            {
                let result = self.callable_return_targets_from_function_ids(
                    method_targets,
                    self.argument_targets_from_call(call, env),
                    self.object_arguments_from_call(call, env),
                    receiver,
                    CollectionTargets::default(),
                    depth + 1,
                );
                if !result.is_empty() {
                    return result;
                }
            }
            if matches!(method, "call" | "apply") {
                let target_ids = self.function_target_ids_from_expression(&member.object, env);
                let (this_object, this_callables, arguments, object_arguments) =
                    self.native_call_receiver_and_arguments(call, method, env);
                return self.callable_return_targets_from_function_ids(
                    target_ids,
                    arguments,
                    object_arguments,
                    this_object,
                    this_callables,
                    depth + 1,
                );
            }
            if method == "bind" {
                return CollectionTargets {
                    keys: Vec::new(),
                    values: self.function_target_ids_from_expression(&member.object, env),
                    object_values: Vec::new(),
                };
            }
        }

        CollectionTargets::default()
    }

    fn callable_return_targets_from_bound_function(
        &mut self,
        bound: &BoundFunctionTarget,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
        depth: usize,
    ) -> CollectionTargets {
        let mut arguments = bound.bound_arguments.clone();
        arguments.extend(self.argument_targets_from_call(call, env));
        let mut object_arguments = vec![None; bound.bound_arguments.len()];
        object_arguments.extend(self.object_arguments_from_call(call, env));
        self.callable_return_targets_from_function_ids(
            vec![bound.function],
            arguments,
            object_arguments,
            bound.this_object.clone(),
            bound.this_callables.clone(),
            depth + 1,
        )
    }

    fn callable_return_targets_from_function_ids(
        &mut self,
        target_ids: Vec<FunctionId>,
        arguments: Vec<CollectionTargets>,
        object_arguments: Vec<Option<ObjectTargets>>,
        this_object: ObjectTargets,
        this_callables: CollectionTargets,
        depth: usize,
    ) -> CollectionTargets {
        if depth > 8 {
            return CollectionTargets::default();
        }
        let mut returned = CollectionTargets::default();
        for target in target_ids {
            let Some(flow) = self.function_flows_by_id.get(&target).cloned() else {
                continue;
            };
            let mut callee_env = FlowEnv {
                this_object: this_object.clone(),
                this_callables: this_callables.clone(),
                ..FlowEnv::default()
            };
            self.bind_invocation_arguments(&flow, &arguments, &object_arguments, &mut callee_env);
            returned.extend(self.collect_function_flow_invocation(&flow, &mut callee_env));
        }
        returned
    }

    fn native_call_receiver_and_arguments(
        &self,
        call: &'ast CallExpression<'ast>,
        method: &str,
        env: &FlowEnv,
    ) -> (
        ObjectTargets,
        CollectionTargets,
        Vec<CollectionTargets>,
        Vec<Option<ObjectTargets>>,
    ) {
        let this_expression = call.arguments.first().and_then(argument_expression);
        let this_object = this_expression
            .and_then(|expression| self.object_targets_from_expression(expression, env))
            .unwrap_or_default();
        let this_callables = this_expression
            .map(|expression| self.callable_targets_from_expression(expression, env))
            .unwrap_or_default();
        match method {
            "call" => (
                this_object,
                this_callables,
                self.argument_targets_from_expressions(
                    call.arguments
                        .iter()
                        .skip(1)
                        .filter_map(argument_expression),
                    env,
                ),
                self.object_arguments_from_expressions(
                    call.arguments
                        .iter()
                        .skip(1)
                        .filter_map(argument_expression),
                    env,
                ),
            ),
            "apply" => {
                let argument_list_expression = call.arguments.get(1).and_then(argument_expression);
                let arguments = argument_list_expression
                    .map(|expression| self.apply_argument_targets_from_expression(expression, env))
                    .unwrap_or_default();
                let object_arguments = argument_list_expression
                    .map(|expression| self.apply_object_arguments_from_expression(expression, env))
                    .unwrap_or_default();
                (this_object, this_callables, arguments, object_arguments)
            }
            _ => (this_object, this_callables, Vec::new(), Vec::new()),
        }
    }

    fn bound_function_targets_from_bind_call(
        &self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) -> Option<Vec<BoundFunctionTarget>> {
        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        if member.property.name != "bind" {
            return None;
        }
        let target_ids = self.function_target_ids_from_expression(&member.object, env);
        if target_ids.is_empty() {
            return None;
        }
        let this_expression = call.arguments.first().and_then(argument_expression);
        let this_object = this_expression
            .and_then(|expression| self.object_targets_from_expression(expression, env))
            .unwrap_or_default();
        let this_callables = this_expression
            .map(|expression| self.callable_targets_from_expression(expression, env))
            .unwrap_or_default();
        let bound_arguments = self.argument_targets_from_expressions(
            call.arguments
                .iter()
                .skip(1)
                .filter_map(argument_expression),
            env,
        );
        Some(
            target_ids
                .into_iter()
                .map(|function| BoundFunctionTarget {
                    function,
                    this_object: this_object.clone(),
                    this_callables: this_callables.clone(),
                    bound_arguments: bound_arguments.clone(),
                })
                .collect(),
        )
    }

    fn argument_targets_from_call(
        &self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) -> Vec<CollectionTargets> {
        self.argument_targets_from_expressions(
            call.arguments.iter().filter_map(argument_expression),
            env,
        )
    }

    fn object_arguments_from_call(
        &self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) -> Vec<Option<ObjectTargets>> {
        self.object_arguments_from_expressions(
            call.arguments.iter().filter_map(argument_expression),
            env,
        )
    }

    fn argument_targets_from_expressions<I>(
        &self,
        expressions: I,
        env: &FlowEnv,
    ) -> Vec<CollectionTargets>
    where
        I: Iterator<Item = &'ast Expression<'ast>>,
    {
        expressions
            .map(|expression| {
                self.targets_from_argument_expression(expression, env)
                    .unwrap_or_default()
            })
            .collect()
    }

    fn object_arguments_from_expressions<I>(
        &self,
        expressions: I,
        env: &FlowEnv,
    ) -> Vec<Option<ObjectTargets>>
    where
        I: Iterator<Item = &'ast Expression<'ast>>,
    {
        expressions
            .map(|expression| self.object_targets_from_expression(expression, env))
            .collect()
    }

    fn apply_argument_targets_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Vec<CollectionTargets> {
        match expression {
            Expression::Identifier(identifier) => env
                .bindings
                .get(identifier.name.as_str())
                .map(CollectionTargets::argument_list)
                .unwrap_or_default(),
            Expression::ArrayExpression(array) => array
                .elements
                .iter()
                .filter_map(array_element_expression)
                .map(|expression| {
                    self.targets_from_argument_expression(expression, env)
                        .unwrap_or_default()
                })
                .collect(),
            Expression::ParenthesizedExpression(expression) => {
                self.apply_argument_targets_from_expression(&expression.expression, env)
            }
            _ => self
                .collection_targets_from_expression(expression, env)
                .map(|targets| targets.argument_list())
                .unwrap_or_default(),
        }
    }

    fn apply_object_arguments_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Vec<Option<ObjectTargets>> {
        match expression {
            Expression::Identifier(identifier) => env
                .bindings
                .get(identifier.name.as_str())
                .map(|targets| {
                    targets
                        .argument_list()
                        .into_iter()
                        .map(|argument| argument.singular_object())
                        .collect()
                })
                .unwrap_or_default(),
            Expression::ArrayExpression(array) => array
                .elements
                .iter()
                .filter_map(array_element_expression)
                .map(|expression| self.object_targets_from_expression(expression, env))
                .collect(),
            Expression::ParenthesizedExpression(expression) => {
                self.apply_object_arguments_from_expression(&expression.expression, env)
            }
            _ => Vec::new(),
        }
    }

    fn function_target_ids_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Vec<FunctionId> {
        let mut targets = self
            .callable_targets_from_expression(expression, env)
            .all_targets();
        if let Some(name) = expression_identifier(expression) {
            if let Some(flow) = self.function_declarations.get(name) {
                targets.push(flow.function.id);
            }
            if let Some(bound_targets) = env.bound_functions.get(name) {
                targets.extend(bound_targets.iter().map(|target| target.function));
            }
        }
        targets.sort();
        targets.dedup();
        targets
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
        if let Some(name) = expression_identifier(expression)
            && let Some(flow) = self.function_declarations.get(name)
        {
            return Some(CollectionTargets {
                keys: Vec::new(),
                values: vec![flow.function.id],
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
            Expression::ArrayExpression(array) => {
                Some(self.array_literal_targets(array, Some(env)))
            }
            Expression::NewExpression(expression) => {
                self.collection_targets_from_new_expression(expression, env)
            }
            Expression::CallExpression(call) => self.collection_targets_from_call(call, env),
            _ => None,
        }
    }

    /// Collection elements produced by `new Array(...)` / `new Set(...)` /
    /// `new Map(...)`, so destructuring (`const [x, y] = new Set([...])`) and
    /// `for-of` over a freshly-built collection resolve their elements.
    fn collection_targets_from_new_expression(
        &self,
        expression: &'ast oxc_ast::ast::NewExpression<'ast>,
        env: &FlowEnv,
    ) -> Option<CollectionTargets> {
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

    fn targets_for_iterable(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> CollectionTargets {
        match expression {
            Expression::Identifier(identifier) => env
                .async_iterators
                .get(identifier.name.as_str())
                .or_else(|| env.bindings.get(identifier.name.as_str()))
                .cloned()
                .unwrap_or_default(),
            Expression::CallExpression(call) => {
                if let Some(name) = callee_identifier(&call.callee)
                    && let Some(targets) = env.async_generators.get(name)
                {
                    return targets.clone();
                }
                if let Some(targets) = self.generator_targets_from_call(call, env, 0) {
                    return targets;
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
            Expression::ArrayExpression(array) => {
                Some(self.array_literal_targets(array, Some(env)))
            }
            Expression::ComputedMemberExpression(member) => {
                let source = self.collection_targets_from_expression(&member.object, env)?;
                let index = numeric_index(&member.expression)?;
                Some(source.value_at(index))
            }
            Expression::StaticMemberExpression(member) => {
                let object = self.object_targets_from_expression(&member.object, env)?;
                object
                    .collection_properties
                    .get(member.property.name.as_str())
                    .cloned()
            }
            Expression::AwaitExpression(expression) => {
                Some(self.fulfilled_targets_from_expression(&expression.argument, env))
            }
            Expression::CallExpression(call) => self.collection_targets_from_call(call, env),
            Expression::NewExpression(expression) => {
                self.collection_targets_from_new_expression(expression, env)
            }
            Expression::ParenthesizedExpression(expression) => {
                self.collection_targets_from_expression(&expression.expression, env)
            }
            _ => None,
        }
    }

    /// If `call` is a static `require("literal")` or `import("literal")` whose
    /// specifier resolves (via the kernel module graph) to a module we have a
    /// converged export summary for, return that summary.
    fn require_summary(
        &self,
        call: &'ast CallExpression<'ast>,
    ) -> Option<&'env ModuleExportSummary> {
        let specifier = self.require_specifier(call)?;
        let target = self
            .resolution_map
            .get(&(self.file.id, specifier.to_string()))?;
        self.module_summaries.get(target)
    }

    /// TypeScript/Babel emit interop wrappers around `require(...)` that pass the
    /// module value through (`__importDefault`, `__importStar`,
    /// `_interopRequireDefault`, `_interopRequireWildcard`). For call-graph
    /// purposes they are identity on the module's export shape, so return the
    /// wrapped argument expression for the caller to evaluate.
    fn commonjs_interop_argument(
        &self,
        call: &'ast CallExpression<'ast>,
    ) -> Option<&'ast Expression<'ast>> {
        let name = callee_identifier(&call.callee)?;
        if matches!(
            name,
            "__importDefault"
                | "__importStar"
                | "_interopRequireDefault"
                | "_interopRequireWildcard"
        ) {
            call.arguments.first().and_then(argument_expression)
        } else {
            None
        }
    }

    /// Extract the static string specifier of a `require("x")` call, if present.
    fn require_specifier(&self, call: &'ast CallExpression<'ast>) -> Option<&'ast str> {
        let is_require = matches!(&call.callee, Expression::Identifier(identifier) if identifier.name == "require");
        if !is_require {
            return None;
        }
        match call.arguments.first().and_then(argument_expression) {
            Some(Expression::StringLiteral(literal)) => Some(literal.value.as_str()),
            _ => None,
        }
    }

    fn collection_targets_from_call(
        &self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) -> Option<CollectionTargets> {
        if let Some(summary) = self.require_summary(call)
            && !summary.callables.is_empty()
        {
            return Some(summary.callables.clone());
        }
        if let Some(inner) = self.commonjs_interop_argument(call) {
            return self.collection_targets_from_expression(inner, env);
        }
        if callee_text(&call.callee).as_deref() == Some("Array.from") {
            self.array_from_targets(call, env)
        } else if callee_text(&call.callee).as_deref() == Some("Array.of") {
            Some(self.array_of_targets(call, env))
        } else if let Some((collection_name, method)) = static_member_call(&call.callee) {
            let source = env.bindings.get(collection_name)?;
            match method {
                "concat" => Some(self.concat_targets(source, call, env)),
                "values" | "slice" | "splice" | "filter" | "map" | "flatMap" | "flat" => {
                    Some(CollectionTargets {
                        keys: Vec::new(),
                        values: source.values.clone(),
                        object_values: source.object_values.clone(),
                    })
                }
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
                let source = self.collection_targets_from_expression(&member.object, env)?;
                let index = numeric_index(&member.expression)?;
                source.object_at(index)
            }
            Expression::NewExpression(expression) => {
                let name = callee_identifier(&expression.callee)?;
                // A variable may hold a class flowed through it (`var a = f(); new a()`);
                // resolve the binding to its class key, else use the name directly.
                let key = env
                    .class_bindings
                    .get(name)
                    .map(String::as_str)
                    .unwrap_or(name);
                let arguments = expression
                    .arguments
                    .iter()
                    .filter_map(argument_expression)
                    .collect::<Vec<_>>();
                self.class_instance_targets(key, &arguments, env)
            }
            Expression::CallExpression(call) => self.object_targets_from_call(call, env),
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

    /// Object returned by a call to a same-file function declaration that builds
    /// and returns an object (`function make() { const x = {...}; return x; }`),
    /// for `const {a, b} = make()` destructuring. Walks the callee body to build
    /// the returned object's shape — the read-only `object_targets_from_expression`
    /// cannot, since the object is assembled across statements. Bounded by
    /// `invocation_depth` (shared with the callable-return cycle).
    fn object_targets_from_local_function_call(
        &mut self,
        init: &'ast Expression<'ast>,
        env: &FlowEnv,
    ) -> Option<ObjectTargets> {
        let Expression::CallExpression(call) = init else {
            return None;
        };
        let name = callee_identifier(&call.callee)?;
        let flow = self.function_declarations.get(name).cloned()?;
        if self.invocation_depth > 16 {
            return None;
        }
        let arguments = self.argument_targets_from_call(call, env);
        let object_arguments = self.object_arguments_from_call(call, env);
        let mut callee_env = FlowEnv::default();
        self.bind_invocation_arguments(&flow, &arguments, &object_arguments, &mut callee_env);
        self.invocation_depth += 1;
        let saved_override = self.caller_override.take();
        for statement in &flow.body.statements {
            self.collect_statement(statement, flow.function, &mut callee_env);
        }
        let result = returned_expression_from_statements(&flow.body.statements)
            .and_then(|expression| self.object_targets_from_expression(expression, &callee_env));
        self.caller_override = saved_override;
        self.invocation_depth -= 1;
        result
    }

    fn object_targets_from_call(
        &self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) -> Option<ObjectTargets> {
        if let Some(summary) = self.require_summary(call)
            && !summary.object.is_empty()
        {
            return Some(summary.object.clone());
        }
        if let Some(inner) = self.commonjs_interop_argument(call) {
            return self.object_targets_from_expression(inner, env);
        }
        match callee_text(&call.callee).as_deref() {
            Some("Object.create") => return Some(ObjectTargets::default()),
            Some("Object.getOwnPropertyDescriptor") => {
                let source = call
                    .arguments
                    .first()
                    .and_then(argument_expression)
                    .and_then(|expression| self.object_targets_from_expression(expression, env))?;
                let property = call
                    .arguments
                    .get(1)
                    .and_then(argument_expression)
                    .and_then(|expression| self.constant_string_from_expression(expression, env))?;
                return Some(descriptor_for_property(&source, &property));
            }
            Some("Object.getOwnPropertyDescriptors") => {
                return call
                    .arguments
                    .first()
                    .and_then(argument_expression)
                    .and_then(|expression| self.object_targets_from_expression(expression, env));
            }
            _ => {}
        }

        if let Some((iterator_name, "next")) = static_member_call(&call.callee)
            && let Some(values) = env.async_iterators.get(iterator_name)
        {
            return Some(iterator_result_object(values));
        }

        let Expression::StaticMemberExpression(member) = &call.callee else {
            return None;
        };
        let receiver = self.object_targets_from_expression(&member.object, env)?;
        let function_ids = receiver.properties.get(member.property.name.as_str())?;
        let mut returned = ObjectTargets::default();
        let mut found = false;

        for function_id in function_ids {
            let Some(flow) = self.function_flows_by_id.get(function_id) else {
                continue;
            };
            let Some(expression) = returned_expression_from_statements(&flow.body.statements)
            else {
                continue;
            };
            if let Some(object) =
                self.object_targets_from_return_expression(expression, &receiver, env)
            {
                returned.merge(object);
                found = true;
            }
        }

        found.then_some(returned)
    }

    fn object_targets_from_return_expression(
        &self,
        expression: &'ast Expression<'ast>,
        receiver: &ObjectTargets,
        env: &FlowEnv,
    ) -> Option<ObjectTargets> {
        match expression {
            Expression::ThisExpression(_) => Some(receiver.clone()),
            Expression::ObjectExpression(object) => {
                Some(self.object_literal_targets(object, Some(env)))
            }
            Expression::Identifier(identifier) => {
                env.objects.get(identifier.name.as_str()).cloned()
            }
            Expression::CallExpression(call) => self.object_targets_from_call(call, env),
            Expression::ParenthesizedExpression(expression) => {
                self.object_targets_from_return_expression(&expression.expression, receiver, env)
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
            Expression::CallExpression(call) => self.generator_targets_from_call(call, env, 0),
            Expression::ParenthesizedExpression(expression) => {
                self.generator_iterator_targets(&expression.expression, env)
            }
            _ => None,
        }
    }

    fn generator_targets_from_call(
        &self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
        depth: usize,
    ) -> Option<CollectionTargets> {
        if depth > 8 {
            return None;
        }
        if let Some(name) = callee_identifier(&call.callee) {
            if let Some(targets) = env.async_generators.get(name) {
                return Some(targets.clone());
            }
            if let Some(flow) = self.function_declarations.get(name)
                && flow.generator
            {
                return Some(self.generator_targets_from_flow(flow, env, depth + 1));
            }
        }
        if let Some((collection_name, method)) = static_member_call(&call.callee)
            && let Some(source) = env.bindings.get(collection_name)
        {
            return match method {
                "values" => Some(CollectionTargets {
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
                _ => None,
            };
        }
        if let Expression::StaticMemberExpression(member) = &call.callee
            && let Some(object) = self.object_targets_from_expression(&member.object, env)
            && let Some(function_ids) = object.properties.get(member.property.name.as_str())
        {
            let mut targets = CollectionTargets::default();
            for function_id in function_ids {
                if let Some(flow) = self.function_flows_by_id.get(function_id)
                    && flow.generator
                {
                    targets.extend(self.generator_targets_from_flow(flow, env, depth + 1));
                }
            }
            if !targets.is_empty() {
                return Some(targets);
            }
        }
        None
    }

    fn generator_targets_from_expression(
        &self,
        expression: &'ast Expression<'ast>,
        env: &FlowEnv,
        depth: usize,
    ) -> Option<CollectionTargets> {
        match expression {
            Expression::CallExpression(call) => self.generator_targets_from_call(call, env, depth),
            Expression::Identifier(identifier) => env
                .async_iterators
                .get(identifier.name.as_str())
                .or_else(|| env.async_generators.get(identifier.name.as_str()))
                .cloned(),
            Expression::ParenthesizedExpression(expression) => {
                self.generator_targets_from_expression(&expression.expression, env, depth)
            }
            _ => None,
        }
    }

    fn generator_targets_from_flow(
        &self,
        flow: &FunctionFlow<'db, 'ast>,
        env: &FlowEnv,
        depth: usize,
    ) -> CollectionTargets {
        if depth > 8 {
            return CollectionTargets::default();
        }
        self.yield_targets_from_statements(&flow.body.statements, env)
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
            Statement::ReturnStatement(statement) => statement
                .argument
                .as_ref()
                .map(|argument| self.callable_targets_from_expression(argument, env))
                .unwrap_or_default(),
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
                    self.generator_targets_from_expression(argument, env, 0)
                        .or_else(|| self.collection_targets_from_expression(argument, env))
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
        // Calls inside this nested body belong to `callback`, not to any
        // `caller_override` active in the enclosing constructor/field body (e.g.
        // super5's `super.m()` inside an IIFE arrow is attributed to the arrow,
        // not the class node). `current_super` is intentionally preserved: an
        // arrow lexically captures `super` from its enclosing method.
        let saved_override = self.caller_override.take();
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
        self.caller_override = saved_override;
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
        if let Expression::ComputedMemberExpression(member) = expression
            && let Some(object) = self.object_targets_from_expression(&member.object, env)
            && let Some(property) = self.constant_string_from_expression(&member.expression, env)
            && let Some(targets) = object.properties.get(&property)
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
        env: Option<&FlowEnv>,
    ) -> CollectionTargets {
        let mut targets = CollectionTargets::default();
        for element in &array.elements {
            let Some(expression) = array_element_expression(element) else {
                continue;
            };
            if let Some(function) = self.function_for_expression(expression) {
                targets.values.push(function.id);
                continue;
            }
            if let Some(env) = env {
                if let Some(object) = self.object_targets_from_expression(expression, env) {
                    targets.object_values.push(object);
                    continue;
                }
                if let Some(collection) = self.collection_targets_from_expression(expression, env) {
                    targets.append_ordered(collection);
                }
            }
        }
        targets
    }

    fn array_of_targets(
        &self,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) -> CollectionTargets {
        let mut targets = CollectionTargets::default();
        for argument in call.arguments.iter().filter_map(argument_expression) {
            targets.append_ordered(
                self.targets_from_argument_expression(argument, env)
                    .unwrap_or_default(),
            );
        }
        targets
    }

    fn concat_targets(
        &self,
        source: &CollectionTargets,
        call: &'ast CallExpression<'ast>,
        env: &FlowEnv,
    ) -> CollectionTargets {
        let mut targets = CollectionTargets {
            keys: Vec::new(),
            values: source.values.clone(),
            object_values: source.object_values.clone(),
        };
        for argument in call.arguments.iter().filter_map(argument_expression) {
            if let Some(collection) = self.collection_targets_from_expression(argument, env) {
                targets.append_ordered(collection);
            } else {
                targets.append_ordered(
                    self.targets_from_argument_expression(argument, env)
                        .unwrap_or_default(),
                );
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
            let names = self.property_key_names(&property.key, property.computed, env);
            if names.is_empty() {
                continue;
            };
            if let Some(function) = self.function_for_object_property(property) {
                for name in names {
                    match property.kind {
                        PropertyKind::Get => targets.add_getter_target(name, function.id),
                        PropertyKind::Set => targets.add_setter_target(name, function.id),
                        PropertyKind::Init => targets.add_property_target(name, function.id),
                    }
                }
                continue;
            }
            if let Expression::ObjectExpression(value) = &property.value {
                let nested = self.object_literal_targets(value, env);
                for name in names {
                    if name == "__proto__" {
                        targets.merge(nested.clone());
                    } else {
                        targets.add_object_property(name, nested.clone());
                    }
                }
            } else if let Some(env) = env
                && let Some(collection) =
                    self.collection_targets_from_expression(&property.value, env)
            {
                for name in names {
                    targets.add_collection_property(name, collection.clone());
                }
            } else if matches!(&property.value, Expression::ThisExpression(_))
                && let Some(env) = env
            {
                for name in names {
                    targets.add_object_property(name, env.this_object.clone());
                }
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
                    caller: self.caller_override.unwrap_or(site.caller),
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
                    caller: self.caller_override.unwrap_or(site.caller),
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
                    caller: self.caller_override.unwrap_or(site.caller),
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

    fn emit_containing_call_span_targets(
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
        let Some(site) = self
            .sites
            .iter()
            .copied()
            .filter(|site| {
                site.caller == owner.id
                    && site.span.start_byte <= span.start
                    && site.span.end_byte >= span.end
            })
            .min_by_key(|site| site.span.end_byte - site.span.start_byte)
        else {
            return;
        };
        for target in &targets {
            self.rows.push(CallTargetFact {
                id: CallTargetId(self.next_id + self.rows.len() as u64),
                site: site.id,
                caller: self.caller_override.unwrap_or(site.caller),
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
            .or_else(|| self.function_inside_span(method.span))
    }

    fn function_for_object_property(
        &self,
        property: &'ast ObjectProperty<'ast>,
    ) -> Option<&'db FunctionFact> {
        let span = object_property_function_span(property);
        self.function_for_span(span)
            .or_else(|| self.function_for_expression(&property.value))
    }

    fn function_for_span(&self, span: oxc_span::Span) -> Option<&'db FunctionFact> {
        let functions = self.functions_by_start.get(&(self.file.id, span.start))?;
        functions
            .iter()
            .copied()
            .find(|function| function.span.end_byte == span.end)
            .or_else(|| functions.first().copied())
    }

    fn function_inside_span(&self, span: oxc_span::Span) -> Option<&'db FunctionFact> {
        self.db
            .functions()
            .iter()
            .filter(|function| {
                function.file == self.file.id
                    && function.language.is_ts_family()
                    && function.span.start_byte >= span.start
                    && function.span.end_byte <= span.end
            })
            .min_by_key(|function| {
                (
                    function.span.end_byte - function.span.start_byte,
                    function.id.0,
                )
            })
    }
}

#[derive(Clone, Debug, Default)]
struct FlowEnv {
    bindings: BTreeMap<String, CollectionTargets>,
    bound_functions: BTreeMap<String, Vec<BoundFunctionTarget>>,
    objects: BTreeMap<String, ObjectTargets>,
    /// Variable name -> class key in `classes`, for classes flowed through a
    /// variable (e.g. `var a = makeClass()` then `new a()` / `a.staticM()`).
    class_bindings: BTreeMap<String, String>,
    string_bindings: BTreeMap<String, String>,
    bool_bindings: BTreeMap<String, bool>,
    string_arrays: BTreeMap<String, Vec<String>>,
    promises: BTreeMap<String, PromiseTargets>,
    async_functions: BTreeMap<String, PromiseTargets>,
    async_generators: BTreeMap<String, CollectionTargets>,
    async_iterators: BTreeMap<String, CollectionTargets>,
    this_object: ObjectTargets,
    this_callables: CollectionTargets,
}

#[derive(Clone, Debug)]
struct BoundFunctionTarget {
    function: FunctionId,
    this_object: ObjectTargets,
    this_callables: CollectionTargets,
    bound_arguments: Vec<CollectionTargets>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
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

    fn append_ordered(&mut self, other: Self) {
        for key in other.keys {
            if !self.keys.contains(&key) {
                self.keys.push(key);
            }
        }
        for value in other.values {
            if !self.values.contains(&value) {
                self.values.push(value);
            }
        }
        self.object_values.extend(other.object_values);
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

    fn argument_list(&self) -> Vec<Self> {
        let len = self.values.len().max(self.object_values.len());
        (0..len).map(|index| self.value_at(index)).collect()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ObjectTargets {
    properties: BTreeMap<String, Vec<FunctionId>>,
    getter_properties: BTreeMap<String, Vec<FunctionId>>,
    setter_properties: BTreeMap<String, Vec<FunctionId>>,
    object_properties: BTreeMap<String, Box<ObjectTargets>>,
    collection_properties: BTreeMap<String, CollectionTargets>,
}

impl ObjectTargets {
    fn is_empty(&self) -> bool {
        self.properties.is_empty()
            && self.getter_properties.is_empty()
            && self.setter_properties.is_empty()
            && self.object_properties.is_empty()
            && self.collection_properties.is_empty()
    }

    fn add_property_target(&mut self, name: String, target: FunctionId) {
        let targets = self.properties.entry(name).or_default();
        targets.push(target);
        targets.sort();
        targets.dedup();
    }

    fn add_getter_target(&mut self, name: String, target: FunctionId) {
        let targets = self.getter_properties.entry(name).or_default();
        targets.push(target);
        targets.sort();
        targets.dedup();
    }

    fn add_setter_target(&mut self, name: String, target: FunctionId) {
        let targets = self.setter_properties.entry(name).or_default();
        targets.push(target);
        targets.sort();
        targets.dedup();
    }

    fn add_object_property(&mut self, name: String, targets: ObjectTargets) {
        self.object_properties.insert(name, Box::new(targets));
    }

    fn add_collection_property(&mut self, name: String, targets: CollectionTargets) {
        self.collection_properties.insert(name, targets);
    }

    fn merge(&mut self, other: Self) {
        for (name, mut targets) in other.properties {
            let slot = self.properties.entry(name).or_default();
            slot.append(&mut targets);
            slot.sort();
            slot.dedup();
        }
        for (name, mut targets) in other.getter_properties {
            let slot = self.getter_properties.entry(name).or_default();
            slot.append(&mut targets);
            slot.sort();
            slot.dedup();
        }
        for (name, mut targets) in other.setter_properties {
            let slot = self.setter_properties.entry(name).or_default();
            slot.append(&mut targets);
            slot.sort();
            slot.dedup();
        }
        for (name, targets) in other.object_properties {
            self.object_properties.entry(name).or_insert(targets);
        }
        for (name, targets) in other.collection_properties {
            self.collection_properties.entry(name).or_insert(targets);
        }
    }

    /// Merge `other` on top of `self`, with `other`'s names **replacing** (not
    /// unioning) `self`'s. Models prototype-chain shadowing: a subclass member
    /// overrides the inherited member of the same name (`x.m()` resolves to the
    /// child's `m`, not both child's and parent's). Shadowing crosses kinds — a
    /// child data property shadows an inherited accessor of the same name and vice
    /// versa — so the inherited name is dropped from every kind before merging.
    fn override_with(&mut self, other: Self) {
        let shadowed: std::collections::BTreeSet<&str> = other
            .properties
            .keys()
            .chain(other.getter_properties.keys())
            .chain(other.setter_properties.keys())
            .chain(other.object_properties.keys())
            .chain(other.collection_properties.keys())
            .map(String::as_str)
            .collect();
        let shadowed: Vec<String> = shadowed.into_iter().map(ToOwned::to_owned).collect();
        for name in &shadowed {
            self.properties.remove(name);
            self.getter_properties.remove(name);
            self.setter_properties.remove(name);
            self.object_properties.remove(name);
            self.collection_properties.remove(name);
        }
        // `self` now has no entry for any name `other` defines, so the union in
        // `merge` is equivalent to insert for those names while leaving inherited
        // names untouched.
        self.merge(other);
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
            getter_properties: self
                .getter_properties
                .iter()
                .filter(|(name, _)| !used.contains(name))
                .map(|(name, targets)| (name.clone(), targets.clone()))
                .collect(),
            setter_properties: self
                .setter_properties
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
            collection_properties: self
                .collection_properties
                .iter()
                .filter(|(name, _)| !used.contains(name))
                .map(|(name, targets)| (name.clone(), targets.clone()))
                .collect(),
        }
    }
}

fn add_assignment_to_object_property(
    object: &mut ObjectTargets,
    property: String,
    callable_targets: &[FunctionId],
    nested_object: Option<ObjectTargets>,
    nested_collection: Option<CollectionTargets>,
) {
    for target in callable_targets {
        object.add_property_target(property.clone(), *target);
    }
    if let Some(nested_object) = nested_object {
        object.add_object_property(property.clone(), nested_object);
    }
    if let Some(nested_collection) = nested_collection {
        object.add_collection_property(property, nested_collection);
    }
}

fn descriptor_for_property(source: &ObjectTargets, property: &str) -> ObjectTargets {
    let mut descriptor = ObjectTargets::default();
    if let Some(targets) = source.properties.get(property) {
        for target in targets {
            descriptor.add_property_target("value".to_string(), *target);
        }
    }
    if let Some(targets) = source.object_properties.get(property) {
        descriptor.add_object_property("value".to_string(), (**targets).clone());
    }
    if let Some(targets) = source.collection_properties.get(property) {
        descriptor.add_collection_property("value".to_string(), targets.clone());
    }
    descriptor
}

fn copy_descriptor_value_to_property(
    target: &mut ObjectTargets,
    property: String,
    descriptor: &ObjectTargets,
) {
    if let Some(targets) = descriptor.properties.get("value") {
        for target_id in targets {
            target.add_property_target(property.clone(), *target_id);
        }
    }
    if let Some(targets) = descriptor.object_properties.get("value") {
        target.add_object_property(property.clone(), (**targets).clone());
    }
    if let Some(targets) = descriptor.collection_properties.get("value") {
        target.add_collection_property(property, targets.clone());
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
    /// The class node (constructor-callable) `FunctionFact`, used to emit the
    /// `new v()` -> constructor edge when the class is flowed through a variable.
    constructor: Option<FunctionId>,
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

fn module_export_name_string(name: &ModuleExportName<'_>) -> String {
    match name {
        ModuleExportName::IdentifierName(identifier) => identifier.name.to_string(),
        ModuleExportName::IdentifierReference(identifier) => identifier.name.to_string(),
        ModuleExportName::StringLiteral(literal) => literal.value.to_string(),
    }
}

/// Which CommonJS export slot an assignment target writes to.
enum ExportAssignmentTarget {
    /// `module.exports = ...`
    WholeExports,
    /// `exports.foo = ...` or `module.exports.foo = ...`
    Property(String),
}

fn export_assignment_target(target: &AssignmentTarget<'_>) -> Option<ExportAssignmentTarget> {
    let AssignmentTarget::StaticMemberExpression(member) = target else {
        return None;
    };
    let property = member.property.name.as_str();
    match expression_identifier(&member.object) {
        // module.exports = ...
        Some("module") if property == "exports" => {
            return Some(ExportAssignmentTarget::WholeExports);
        }
        Some("module") => return None,
        // exports.foo = ...
        Some("exports") => {
            return Some(ExportAssignmentTarget::Property(property.to_string()));
        }
        _ => {}
    }
    // module.exports.foo = ...
    if let Expression::StaticMemberExpression(inner) = &member.object
        && expression_identifier(&inner.object) == Some("module")
        && inner.property.name.as_str() == "exports"
    {
        return Some(ExportAssignmentTarget::Property(property.to_string()));
    }
    None
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

fn call_expression_callee<'a>(expression: &'a Expression<'a>) -> Option<&'a CallExpression<'a>> {
    match expression {
        Expression::CallExpression(call) => Some(call),
        Expression::ParenthesizedExpression(expression) => {
            call_expression_callee(&expression.expression)
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

/// Span-derived synthetic key for a class expression. Unique per source location,
/// so it never collides with a class declaration's name in the `classes` map.
fn class_expression_key(class: &Class<'_>) -> String {
    format!("@class@{}:{}", class.span.start, class.span.end)
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
        PropertyKey::PrivateIdentifier(identifier) => Some(format!("#{}", identifier.name)),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        PropertyKey::NumericLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

fn bounded_string_product(left: &[String], right: &[String]) -> Vec<String> {
    let mut values = Vec::new();
    for left in left {
        for right in right {
            values.push(format!("{left}{right}"));
            if values.len() >= 8 {
                sort_dedup_strings(&mut values);
                values.truncate(8);
                return values;
            }
        }
    }
    sort_dedup_strings(&mut values);
    values
}

fn sort_dedup_strings(values: &mut Vec<String>) {
    values.sort();
    values.dedup();
}

fn class_method_function_span(method: &MethodDefinition<'_>) -> oxc_span::Span {
    if method.r#static && method.kind == MethodDefinitionKind::Method {
        return oxc_span::Span::new(method.key.span().start, method.span.end);
    }
    method.span
}

fn object_property_function_span(property: &ObjectProperty<'_>) -> oxc_span::Span {
    if property.method {
        return property.span;
    }
    property.value.span()
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
    fn real_ts_pipeline_resolves_promise_resolve_handler_values() {
        let source = "Promise.resolve(() => {console.log(\"promiseresolve1\");}).then(\n    v => {\n        v();\n    },\n    r => {\n        r();\n    },\n);\n";
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let resolved_value =
            function_id_for_span(source, &db, "() => {console.log(\"promiseresolve1\");}");

        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);
        let site = site_id_for_span(source, &sites, "v()");

        assert!(
            resolved.contains(&(site, resolved_value)),
            "expected real TS pipeline to resolve Promise.resolve handler value; got {resolved:?}"
        );
    }

    #[test]
    fn real_ts_pipeline_resolves_promise_executor_values_inside_for_of_switch() {
        let source = "const p2 = new Promise((resolve, reject) => {\n    resolve(() => { console.log(\"p2resolve\"); });\n});\nfor (const round of [1,2,3,4]) {\n    const p1 = new Promise((resolve, reject) => {\n        switch (round) {\n            case 1:\n                resolve(() => { console.log(\"resolve1\"); });\n                break;\n            case 2:\n                reject(() => { console.log(\"reject2\"); });\n                break;\n            case 3:\n                throw () => { console.log(\"throw30\"); };\n            case 4:\n                resolve(p2);\n                break;\n        }\n    });\n    p1.then(a => {\n        a();\n    }, b => {\n        b();\n    });\n}\n";
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let p2_value = function_id_for_span(source, &db, "() => { console.log(\"p2resolve\"); }");
        let resolved = function_id_for_span(source, &db, "() => { console.log(\"resolve1\"); }");
        let rejected = function_id_for_span(source, &db, "() => { console.log(\"reject2\"); }");
        let thrown = function_id_for_span(source, &db, "() => { console.log(\"throw30\"); }");

        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved_targets = resolved_target_ids_by_site(targets);
        let fulfilled_site = site_id_for_span(source, &sites, "a()");
        let rejected_site = site_id_for_span(source, &sites, "b()");

        for target in [p2_value, resolved] {
            assert!(
                resolved_targets.contains(&(fulfilled_site, target)),
                "expected fulfilled handler to resolve {target:?}; got {resolved_targets:?}"
            );
        }
        for target in [rejected, thrown] {
            assert!(
                resolved_targets.contains(&(rejected_site, target)),
                "expected rejected handler to resolve {target:?}; got {resolved_targets:?}"
            );
        }
    }

    #[test]
    fn real_ts_pipeline_resolves_bind_returned_this_member_callable() {
        let source = "var x = {a: function() { console.log(\"1\"); }};\nvar f = function() {return this.a;}.bind(x);\nf()();\n";
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let bound = function_id_for_span(source, &db, "function() {return this.a;}");
        let member = function_id_for_span(source, &db, "function() { console.log(\"1\"); }");

        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);
        let inner_site = site_id_for_span(source, &sites, "f()");
        let outer_site = site_id_for_span(source, &sites, "f()()");

        assert!(
            resolved.contains(&(inner_site, bound)),
            "expected bound function call to resolve to original function; got {resolved:?}"
        );
        assert!(
            resolved.contains(&(outer_site, member)),
            "expected returned this-member callable to resolve at outer call; got {resolved:?}"
        );
    }

    #[test]
    fn real_ts_pipeline_resolves_call_and_apply_returned_argument_callables() {
        let source = "function foo(b) {return this(b);}\n(foo.call((a) => a, () => { console.log(\"2\"); }))();\nfunction bar(c) {return this(c);}\nfunction baz() {\n    return bar.apply((d) => d, arguments);\n}\nvar q = baz(() => { console.log(\"3\"); });\nq();\nfunction baz2(...args) {\n    return bar.apply((d) => d, args);\n}\nvar q2 = baz2(() => { console.log(\"4\"); });\nq2();\nfunction baz3(a) {\n    return bar.apply((d) => d, [a]);\n}\nvar q3 = baz3(() => { console.log(\"5\"); });\nq3();\nfunction baz4(f) {\n    const a = [];\n    a.push(f);\n    return bar.apply((d) => d, a);\n}\nvar q4 = baz4(() => { console.log(\"6\"); });\nq4();\n";
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let foo = function_id_for_span(source, &db, "function foo(b) {return this(b);}");
        let bar = function_id_for_span(source, &db, "function bar(c) {return this(c);}");
        let call_receiver = function_id_for_span(source, &db, "(a) => a");
        let apply_receiver_arguments = function_id_for_nth_span(source, &db, "(d) => d", 0);
        let apply_receiver_rest = function_id_for_nth_span(source, &db, "(d) => d", 1);
        let apply_receiver_array = function_id_for_nth_span(source, &db, "(d) => d", 2);
        let apply_receiver_pushed_array = function_id_for_nth_span(source, &db, "(d) => d", 3);
        let value2 = function_id_for_span(source, &db, "() => { console.log(\"2\"); }");
        let value3 = function_id_for_span(source, &db, "() => { console.log(\"3\"); }");
        let value4 = function_id_for_span(source, &db, "() => { console.log(\"4\"); }");
        let value5 = function_id_for_span(source, &db, "() => { console.log(\"5\"); }");
        let value6 = function_id_for_span(source, &db, "() => { console.log(\"6\"); }");

        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);

        let foo_call_site = site_id_containing_span(
            source,
            &sites,
            "foo.call((a) => a, () => { console.log(\"2\"); })",
        );
        let this_call_site = site_id_for_span(source, &sites, "this(b)");
        let bar_this_call_site = site_id_for_span(source, &sites, "this(c)");
        let outer_call_site = site_id_for_span(
            source,
            &sites,
            "(foo.call((a) => a, () => { console.log(\"2\"); }))()",
        );
        let bar_apply_arguments_site =
            site_id_for_span(source, &sites, "bar.apply((d) => d, arguments)");
        let bar_apply_rest_site = site_id_for_span(source, &sites, "bar.apply((d) => d, args)");
        let bar_apply_array_site = site_id_for_span(source, &sites, "bar.apply((d) => d, [a])");
        let bar_apply_pushed_array_site =
            site_id_for_span(source, &sites, "bar.apply((d) => d, a)");

        for (site, target) in [
            (foo_call_site, foo),
            (this_call_site, call_receiver),
            (outer_call_site, value2),
            (bar_apply_arguments_site, bar),
            (bar_apply_rest_site, bar),
            (bar_apply_array_site, bar),
            (bar_apply_pushed_array_site, bar),
        ] {
            assert!(
                resolved.contains(&(site, target)),
                "expected {site:?} to resolve {target:?}; got {resolved:?}"
            );
        }

        for (call_text, target) in [
            ("q()", value3),
            ("q2()", value4),
            ("q3()", value5),
            ("q4()", value6),
        ] {
            let site = site_id_for_span(source, &sites, call_text);
            assert!(
                resolved.contains(&(site, target)),
                "expected {call_text} to resolve {target:?}; got {resolved:?}"
            );
        }

        for target in [
            apply_receiver_arguments,
            apply_receiver_rest,
            apply_receiver_array,
            apply_receiver_pushed_array,
        ] {
            assert!(
                resolved.contains(&(bar_this_call_site, target)),
                "expected bar this-call to flow through apply receiver {target:?}; got {resolved:?}"
            );
        }
    }

    #[test]
    fn real_ts_pipeline_resolves_sync_generator_next_and_for_of_values() {
        let source = "function* gen() {\n    yield () => {console.log(\"1\")};\n    yield* gen2();\n    yield* [() => {console.log(\"3\")}, () => {console.log(\"4\")}];\n}\nfunction* gen2() {\n    yield () => {console.log(\"2\")};\n}\nconst x = gen();\nconst v1 = x.next();\nv1.value();\nfor (const v of x) {\n    v();\n}\nvar q = [() => {console.log(\"5\")}];\nconst y = q.values();\nconst v2 = y.next();\nv2.value();\nfunction* gen3() {\n    return () => {console.log(\"6\")};\n}\nconst z = gen3();\nconst v3 = z.next();\nv3.value();\nfunction* gen5() {\n    yield () => {console.log(\"8\")};\n    return () => {console.log(\"9\")};\n}\nconst u = gen5();\nu.next().value();\nu.next().value();\n";
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let yielded1 = function_id_for_span(source, &db, "() => {console.log(\"1\")}");
        let yielded2 = function_id_for_span(source, &db, "() => {console.log(\"2\")}");
        let yielded3 = function_id_for_span(source, &db, "() => {console.log(\"3\")}");
        let yielded4 = function_id_for_span(source, &db, "() => {console.log(\"4\")}");
        let array_value = function_id_for_span(source, &db, "() => {console.log(\"5\")}");
        let returned6 = function_id_for_span(source, &db, "() => {console.log(\"6\")}");
        let yielded8 = function_id_for_span(source, &db, "() => {console.log(\"8\")}");
        let returned9 = function_id_for_span(source, &db, "() => {console.log(\"9\")}");

        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);
        let first_next_value = site_id_for_span(source, &sites, "v1.value()");
        let loop_value = site_id_for_span(source, &sites, "v()");
        let array_next_value = site_id_for_span(source, &sites, "v2.value()");
        let return_next_value = site_id_for_span(source, &sites, "v3.value()");
        let gen5_first_next_value = site_id_for_nth_span(source, &sites, "u.next().value()", 0);
        let gen5_second_next_value = site_id_for_nth_span(source, &sites, "u.next().value()", 1);

        for target in [yielded1, yielded2, yielded3, yielded4] {
            assert!(
                resolved.contains(&(loop_value, target)),
                "expected for-of generator value {target:?}; got {resolved:?}"
            );
        }
        assert!(
            resolved.contains(&(first_next_value, yielded1)),
            "expected first generator next value; got {resolved:?}"
        );
        assert!(
            resolved.contains(&(array_next_value, array_value)),
            "expected array iterator next value; got {resolved:?}"
        );
        assert!(
            resolved.contains(&(return_next_value, returned6)),
            "expected generator return value through next().value; got {resolved:?}"
        );
        for site in [gen5_first_next_value, gen5_second_next_value] {
            for target in [yielded8, returned9] {
                assert!(
                    resolved.contains(&(site, target)),
                    "expected gen5 next value {target:?}; got {resolved:?}"
                );
            }
        }
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
            let target = if call == "A.staticG()" {
                function_id_containing_span(source, &db, target)
            } else {
                function_id_for_span(source, &db, target)
            };
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
    fn real_ts_pipeline_reports_object_method_spans_from_property_name() {
        let source = "const k1 = {\n    a2() {console.log(\"a2\")},\n    a4() {\n        return this;\n    }\n};\nk1.a2();\nk1.a4().a2();\n";
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let a2 = function_id_for_span(source, &db, "a2() {console.log(\"a2\")}");
        let a4 = function_id_for_span(source, &db, "a4() {\n        return this;\n    }");

        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);

        for (call, target) in [("k1.a2()", a2), ("k1.a4()", a4), ("k1.a4().a2()", a2)] {
            let site = site_id_for_span(source, &sites, call);
            assert!(
                resolved.contains(&(site, target)),
                "expected real TS pipeline to resolve {call} to {target:?}; got {resolved:?}"
            );
        }
    }

    #[test]
    fn real_ts_pipeline_resolves_native_object_array_and_computed_property_flows() {
        let source = r#"const p = "p";
const y = { f: function yf() {} };
const x1 = Object.create(null);
x1[p] = y;
x1.p.f();
const a = { foo: function fooFn() {} };
const b = { bar: function barFn() {}, baz: function bazFn() {} };
const x2 = {};
Object.assign(x2, a, b);
x2.foo();
x2.bar();
x2.baz();
const x3 = { val: function valFn() {}, otherVal: { t: function tFn() {} } };
const x4 = {};
const v = "val";
const desc = Object.getOwnPropertyDescriptor(x3, v);
Object.defineProperty(x4, p, desc);
x4.p();
Object.defineProperties(x4, Object.getOwnPropertyDescriptors(x3));
x4.val();
x4.otherVal.t();
function Foo() {}
const bound = Foo.bind();
const obj = {};
obj["bo" + "und"] = bound;
obj.bound();
const arr1 = Array.from([function a1() {}, function a2() {}]);
const x5 = {};
x5["arr" + "1"] = arr1;
x5.arr1[0]();
x5.arr1[1]();
const arr2 = Array.of(function of1() {}, function of2() {});
x5["arr" + "2"] = arr2;
x5.arr2[0]();
x5.arr2[1]();
const t1 = [function a3() {}];
const t2 = [function a4() {}];
const arr3 = t2.concat(t1);
x5["arr" + "3"] = arr3;
x5.arr3[0]();
x5.arr3[1]();
const t3 = [t1, t2];
x5["arr" + "4"] = t3.flat();
x5.arr4[0]();
x5.arr4[1]();
const t4 = [{ p1: function p1Fn() {} }, { p2: "foo" }];
const arr5 = t4.filter(entry => "p1" in entry);
x5["arr" + "5"] = arr5;
x5.arr5[0].p1();
const t5 = [function slicedFn() {}, "foo"];
const arr6 = t5.slice(0, 1);
x5["arr" + "6"] = arr6;
x5.arr6[0]();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);

        for (call, target) in [
            ("x1.p.f()", "function yf() {}"),
            ("x2.foo()", "function fooFn() {}"),
            ("x2.bar()", "function barFn() {}"),
            ("x2.baz()", "function bazFn() {}"),
            ("x4.p()", "function valFn() {}"),
            ("x4.val()", "function valFn() {}"),
            ("x4.otherVal.t()", "function tFn() {}"),
            ("obj.bound()", "function Foo() {}"),
            ("x5.arr1[0]()", "function a1() {}"),
            ("x5.arr1[1]()", "function a2() {}"),
            ("x5.arr2[0]()", "function of1() {}"),
            ("x5.arr2[1]()", "function of2() {}"),
            ("x5.arr3[0]()", "function a4() {}"),
            ("x5.arr3[1]()", "function a3() {}"),
            ("x5.arr4[0]()", "function a3() {}"),
            ("x5.arr4[1]()", "function a4() {}"),
            ("x5.arr5[0].p1()", "function p1Fn() {}"),
            ("x5.arr6[0]()", "function slicedFn() {}"),
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
    fn real_ts_pipeline_resolves_computed_property_creation_flows() {
        let source = r#"let x = {["fo" + "o"]: function foo() {}}
x.foo();
const tautology = true === true;
let m = {
    [tautology ? "tautology" : "impossible"](b) {
    }
}
m.tautology()
class A {
    _x;
    ["computedProperty"] = function () {console.log("e")}
    constructor(x) {
        this._x = x;
    }
    static ["static"+"F"] = function () {console.log("a")};
    static [("sta" + "tic") + "G"]() {console.log("e")}
    ["method"]() {console.log("b")};
    get ["field"] () {return this._x}
    set ["field"](x) {this._x = x}
    ["na" + "m" + "e"]() {}
}
const a = new A(function fooArg() {console.log("c")});
A.staticF();
a.method();
let f = a.field;
f();
a.field = function () {console.log("d")}
a.field();
a.computedProperty()
A.staticG();
a.name();
const arr = ["p1", "p2"];
const top = {
    mid: {
        [arr[0]]: function () {console.log("1")},
        [arr[1]]() {
            console.log("2")
        }
    },
    ["bot"]: {
        ["some" + "thing"]: {
            [arr[0]+arr[1]]: function () {console.log("3")}
        }
    }
}
top.mid.p1();
top.mid.p2();
top.bot.something.p1p2();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);

        for (call, target) in [
            ("x.foo()", "function foo() {}"),
            (
                "m.tautology()",
                "[tautology ? \"tautology\" : \"impossible\"](b) {\n    }",
            ),
            ("A.staticF()", "function () {console.log(\"a\")}"),
            ("a.method()", "[\"method\"]() {console.log(\"b\")}"),
            ("f()", "function fooArg() {console.log(\"c\")}"),
            ("a.field()", "function () {console.log(\"d\")}"),
            ("a.computedProperty()", "function () {console.log(\"e\")}"),
            ("top.mid.p1()", "function () {console.log(\"1\")}"),
            (
                "top.mid.p2()",
                "[arr[1]]() {\n            console.log(\"2\")\n        }",
            ),
            (
                "top.bot.something.p1p2()",
                "function () {console.log(\"3\")}",
            ),
        ] {
            let site = site_id_for_span(source, &sites, call);
            let target = function_id_containing_span(source, &db, target);
            assert!(
                resolved.contains(&(site, target)),
                "expected real TS pipeline to resolve {call} to {target:?}; got {resolved:?}"
            );
        }

        for (call, target_name) in [("A.staticG()", "A.staticG"), ("a.name()", "A.name")] {
            let site = site_id_for_span(source, &sites, call);
            let target = function_id_for_name(&db, target_name);
            assert!(
                resolved.contains(&(site, target)),
                "expected real TS pipeline to resolve {call} to {target_name}; got {resolved:?}"
            );
        }
    }

    #[test]
    fn real_ts_pipeline_invokes_setter_assignments_without_overwriting_accessors() {
        let source = r#"class A {
    set ["field"](x) {
        this._x = x;
    }
    get ["other"]() {
        return this._x;
    }
}
const a = new A();
a.field = function assigned() {};
a.other();
a.field();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);
        let assigned = function_id_for_span(source, &db, "function assigned() {}");
        let other_site = site_id_for_span(source, &sites, "a.other()");
        let field_site = site_id_for_span(source, &sites, "a.field()");

        assert!(
            resolved.contains(&(other_site, assigned)),
            "expected getter to return setter-assigned callable; got {resolved:?}"
        );
        assert!(
            !resolved.contains(&(field_site, assigned)),
            "setter-only field assignment must not install a callable data property; got {resolved:?}"
        );
    }

    #[test]
    fn real_ts_pipeline_resolves_computed_object_property_calls() {
        let source = r#"const obj = {};
obj["left"] = function left() {};
obj["right"] = function right() {};
obj[unknown ? "left" : "right"]();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);
        let site = site_id_for_span(source, &sites, "obj[unknown ? \"left\" : \"right\"]()");

        for target in ["function left() {}", "function right() {}"] {
            let target = function_id_for_span(source, &db, target);
            assert!(
                resolved.contains(&(site, target)),
                "expected computed object call to resolve {target:?}; got {resolved:?}"
            );
        }
    }

    #[test]
    fn real_ts_pipeline_resolves_numeric_computed_object_property_calls() {
        let source = r#"const obj = {};
obj[0] = function zero() {};
obj[0]();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);
        let site = site_id_for_span(source, &sites, "obj[0]()");
        let target = function_id_for_span(source, &db, "function zero() {}");

        assert!(
            resolved.contains(&(site, target)),
            "expected numeric computed object call to resolve zero; got {resolved:?}"
        );
    }

    #[test]
    fn real_ts_pipeline_invokes_getter_returned_callable_with_call_arguments() {
        let source = r#"class A {
    get ["cb"]() {
        return function inner(x) {
            x();
        };
    }
}
const a = new A();
a["cb"](function target() {});
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);
        let inner_site = site_id_for_span(source, &sites, "x()");
        let target = function_id_for_span(source, &db, "function target() {}");

        assert!(
            resolved.contains(&(inner_site, target)),
            "expected getter-returned callable to invoke outer argument; got {resolved:?}"
        );
    }

    #[test]
    fn real_ts_pipeline_preserves_getter_receiver_side_effects_before_returned_call() {
        let source = r#"class A {
    get ["cb"]() {
        this.prep = function target() {};
        return function inner() {};
    }
}
const a = new A();
a["cb"]();
a.prep();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);
        let site = site_id_for_span(source, &sites, "a.prep()");
        let target = function_id_for_span(source, &db, "function target() {}");

        assert!(
            resolved.contains(&(site, target)),
            "expected getter receiver side effects to be preserved; got {resolved:?}"
        );
    }

    #[test]
    fn real_ts_pipeline_merges_union_getter_receiver_side_effects() {
        let source = r#"class A {
    get ["a"]() {
        return function left(x) {
            this.left = x;
        };
    }
    get ["b"]() {
        return function right(x) {
            this.right = x;
        };
    }
}
const obj = new A();
obj[unknown ? "a" : "b"](function target() {});
obj.left();
obj.right();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);
        let target = function_id_for_span(source, &db, "function target() {}");

        for call in ["obj.left()", "obj.right()"] {
            let site = site_id_for_span(source, &sites, call);
            assert!(
                resolved.contains(&(site, target)),
                "expected union getter side effects to preserve {call}; got {resolved:?}"
            );
        }
    }

    #[test]
    fn real_ts_pipeline_preserves_computed_assignment_key_unions() {
        let source = r#"const obj = {};
obj[unknown ? "left" : "right"] = function assigned() {};
obj.left();
obj.right();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let resolved = resolved_target_ids_by_site(targets);
        let assigned = function_id_for_span(source, &db, "function assigned() {}");

        for call in ["obj.left()", "obj.right()"] {
            let site = site_id_for_span(source, &sites, call);
            assert!(
                resolved.contains(&(site, assigned)),
                "expected computed assignment union to resolve {call}; got {resolved:?}"
            );
        }
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
        let returned = push_function(
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
            vec![
                (CallSiteId(0), yielded),
                (CallSiteId(0), returned),
                (CallSiteId(1), yielded),
                (CallSiteId(1), returned),
            ]
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

    #[test]
    fn real_kernel_resolves_commonjs_require_exports_across_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("leaf.js"),
            "function leafFn() {}\nmodule.exports = leafFn;\n",
        )
        .unwrap();
        std::fs::write(
            root.join("utils.js"),
            "exports.merge = function mergeFn() {};\nmodule.exports.clone = function cloneFn() {};\n",
        )
        .unwrap();
        // Re-export chain: index re-exports the leaf module wholesale, exercising
        // the bounded summary fixpoint.
        std::fs::write(
            root.join("index.js"),
            "module.exports = require('./leaf');\n",
        )
        .unwrap();
        std::fs::write(
            root.join("app.js"),
            "const leaf = require('./index');\nleaf();\nconst utils = require('./utils');\nutils.merge();\nutils.clone();\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        // Resolved cross-module targets, keyed by (target function span text) so
        // the assertion is independent of how the frontend names the callee.
        let resolved_target_spans: std::collections::BTreeSet<String> = db
            .call_targets()
            .iter()
            .filter(|target| target.status == CallTargetStatus::Resolved)
            .filter_map(|target| target.target_function)
            .filter_map(|function_id| db.functions().iter().find(|f| f.id == function_id))
            .filter_map(|function| {
                let file = db.file(function.file)?;
                Some(
                    file.source[function.span.start_byte as usize..function.span.end_byte as usize]
                        .to_string(),
                )
            })
            .collect();

        for expected in ["function leafFn", "function mergeFn", "function cloneFn"] {
            assert!(
                resolved_target_spans
                    .iter()
                    .any(|span| span.contains(expected)),
                "expected cross-module require resolution to reach `{expected}`; got {resolved_target_spans:?}"
            );
        }
    }

    #[test]
    fn real_kernel_resolves_esm_import_export_across_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("export2.mjs"),
            "export default function function1(x) { return x; }\nexport function function2(x) { return x; }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("export1.mjs"),
            "export { default as function1, function2 } from './export2.mjs';\nfunction cube(x) { return x; }\nvar graph = { draw: function drawFn() {} };\nexport { cube, graph };\nexport default function cube2(x) { return x; }\n",
        )
        .unwrap();
        std::fs::write(
            root.join("import1.mjs"),
            "import cube2, { cube, graph, function1, function2 } from './export1.mjs';\ngraph.draw();\ncube(3);\ncube2(3);\nfunction1(2);\nfunction2(2);\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        let resolved_target_spans: std::collections::BTreeSet<String> = db
            .call_targets()
            .iter()
            .filter(|target| target.status == CallTargetStatus::Resolved)
            .filter_map(|target| target.target_function)
            .filter_map(|function_id| db.functions().iter().find(|f| f.id == function_id))
            .filter_map(|function| {
                let file = db.file(function.file)?;
                Some(
                    file.source[function.span.start_byte as usize..function.span.end_byte as usize]
                        .to_string(),
                )
            })
            .collect();

        for expected in [
            "function cube(",
            "function drawFn(",
            "function cube2(",
            "function function1(",
            "function function2(",
        ] {
            assert!(
                resolved_target_spans
                    .iter()
                    .any(|span| span.contains(expected)),
                "expected ESM import resolution to reach `{expected}`; got {resolved_target_spans:?}"
            );
        }
    }

    #[test]
    fn real_kernel_resolves_typescript_commonjs_interop_default_import() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(
            root.join("lib5a.js"),
            "Object.defineProperty(exports, \"__esModule\", { value: true });\nexports.default = function foo() {};\n",
        )
        .unwrap();
        // The standard TypeScript-emitted CommonJS default-import shape.
        std::fs::write(
            root.join("client5.js"),
            "var __importDefault = (this && this.__importDefault) || function (mod) {\n    return (mod && mod.__esModule) ? mod : { \"default\": mod };\n};\nconst lib5a = __importDefault(require('./lib5a'));\nlib5a.default();\n",
        )
        .unwrap();

        let output = crate::eval::observed::run_kernel_for_repo_for_test(root).unwrap();
        let db = &output.db;
        let resolved_target_spans: std::collections::BTreeSet<String> = db
            .call_targets()
            .iter()
            .filter(|target| target.status == CallTargetStatus::Resolved)
            .filter_map(|target| target.target_function)
            .filter_map(|function_id| db.functions().iter().find(|f| f.id == function_id))
            .filter_map(|function| {
                let file = db.file(function.file)?;
                Some(
                    file.source[function.span.start_byte as usize..function.span.end_byte as usize]
                        .to_string(),
                )
            })
            .collect();

        assert!(
            resolved_target_spans
                .iter()
                .any(|span| span.contains("function foo(")),
            "expected __importDefault interop to resolve lib5a.default() to foo; got {resolved_target_spans:?}"
        );
    }

    #[test]
    fn real_ts_pipeline_resolves_super_this_and_private_member_calls() {
        let source = r#"class A {
    m() {
        console.log("A.m");
    }
    static s() {
        console.log("A.s");
    }
}
class B extends A {
    constructor() {
        super.m();
        this.helper();
    }
    helper() {
        console.log("helper");
    }
    m() {
        super.m();
    }
    static s() {
        super.s();
    }
    #priv() {
        console.log("priv");
    }
    callPriv() {
        this.#priv();
    }
}
new B();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);

        let resolve_for = |needle: &str| -> Vec<String> {
            let site = site_id_for_span(source, &sites, needle);
            targets
                .iter()
                .filter(|t| t.site == site)
                .filter_map(|t| t.target_function)
                .filter_map(|id| db.functions().iter().find(|f| f.id == id))
                .map(|f| source[f.span.start_byte as usize..f.span.end_byte as usize].to_string())
                .collect()
        };
        // `super.m()` (both in B's constructor and B.m), `super.s()`,
        // `this.helper()`, and `this.#priv()` all resolve.
        for (site, target) in [
            ("super.m()", "\"A.m\""),
            ("super.s()", "\"A.s\""),
            ("this.helper()", "\"helper\""),
            ("this.#priv()", "\"priv\""),
        ] {
            assert!(
                resolve_for(site).iter().any(|s| s.contains(target)),
                "expected {site} to reach a target containing `{target}`; got {:?}",
                resolve_for(site)
            );
        }
    }

    #[test]
    fn real_ts_pipeline_resolves_class_returned_from_function() {
        // super4/super5 shape: an anonymous/named `class extends A` returned from a
        // function, instantiated through a variable. The class-expression bodies are
        // walked with `this`/`super` bound, and `new a()` / `x.m()` / `a.s()` resolve
        // through the returned-class binding.
        let source = r#"class A {
    constructor() {
        this.created = () => { console.log("created"); };
    }
    m() {
        this.late = () => { console.log("late"); };
    }
    static s() {
        console.log("A.s");
    }
}

function make() {
    return class B extends A {
        m() {
            super.m();
        }
        static s() {
            super.s();
        }
    };
}

var a = make();
var x = new a();
x.m();
a.s();
x.created();
x.late();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);

        let resolve_for = |needle: &str| -> Vec<String> {
            let site = site_id_for_span(source, &sites, needle);
            targets
                .iter()
                .filter(|t| t.site == site)
                .filter_map(|t| t.target_function)
                .filter_map(|id| db.functions().iter().find(|f| f.id == id))
                .map(|f| source[f.span.start_byte as usize..f.span.end_byte as usize].to_string())
                .collect()
        };
        for (site, target) in [
            ("super.m()", "this.late"),       // B.m -> A.m
            ("super.s()", "\"A.s\""),         // B.s -> A.s
            ("new a()", "class B extends A"), // new a() -> B constructor (class node)
            ("x.m()", "super.m()"),           // x.m() -> B.m
            ("a.s()", "super.s()"),           // a.s() -> B.s
            ("x.created()", "\"created\""),   // inherited constructor assignment
            ("x.late()", "\"late\""),         // flow-insensitive method assignment
        ] {
            assert!(
                resolve_for(site).iter().any(|s| s.contains(target)),
                "expected {site} to reach a target containing `{target}`; got {:?}",
                resolve_for(site)
            );
        }
    }

    #[test]
    fn real_ts_pipeline_attributes_function_calls_in_class_bodies() {
        // A direct call to a top-level `function` declaration inside a class body
        // resolves and is attributed (via `caller_override`) to the class node for
        // constructor bodies, and to the method node for methods/static blocks.
        let source = r#"function f1() {}

class C {
    constructor() {
        f1();
    }
    static s() {
        f1();
    }
}
new C();
C.s();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);

        // Both `f1()` calls inside the class body resolve to `f1`.
        let resolved: Vec<_> = targets
            .iter()
            .filter_map(|t| t.target_function)
            .filter_map(|id| db.functions().iter().find(|f| f.id == id))
            .filter(|f| f.span.start_byte == 0) // `function f1()` starts at byte 0
            .collect();
        assert!(
            resolved.len() >= 2,
            "expected both class-body f1() calls to resolve to f1; got {} resolutions",
            resolved.len()
        );
    }

    #[test]
    fn class_body_resolution_does_not_overproduce_edges() {
        // Regression guards for the review fixes:
        // - A method parameter named like a top-level function shadows the seeded
        //   function binding (no spurious edge to the global).
        // - `this.p = param` in a method must NOT bind `p` to the constructor's
        //   `new C(arg)` arguments.
        // - A self-returning function must not overflow the stack.
        let source = r#"function top() {}
function selfRet() { return (selfRet()); }

class C {
    on(cb) {
        this.cb = cb;
    }
    run(top) {
        top();
    }
}

var loop = selfRet();
var x = new C(top);
x.cb();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        // Must terminate (no unbounded recursion) — reaching here is the guard.
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);

        let targets_for = |site: CallSiteId| -> Vec<String> {
            targets
                .iter()
                .filter(|t| t.site == site)
                .filter_map(|t| t.target_function)
                .filter_map(|id| db.functions().iter().find(|f| f.id == id))
                .map(|f| source[f.span.start_byte as usize..f.span.end_byte as usize].to_string())
                .collect()
        };
        // `top()` inside run(top): the call is the 2nd `top()` occurrence (the 1st
        // is the `function top()` declaration). It must resolve to the parameter
        // (unknown), NOT the shadowed top-level function.
        let run_call = site_id_for_nth_span(source, &sites, "top()", 1);
        assert!(
            !targets_for(run_call)
                .iter()
                .any(|s| s.contains("function top()")),
            "method param `top` must shadow the top-level function; got {:?}",
            targets_for(run_call)
        );
        // `x.cb()` must not resolve to `top` via the constructor-arg conflation.
        let cb_call = site_id_for_span(source, &sites, "x.cb()");
        assert!(
            !targets_for(cb_call)
                .iter()
                .any(|s| s.contains("function top()")),
            "this.cb = cb in a method must not bind to constructor args; got {:?}",
            targets_for(cb_call)
        );
    }

    #[test]
    fn real_ts_pipeline_resolves_destructuring_forms() {
        // Phase B: object destructuring with nested patterns, getter-valued
        // sources, defaults used when the property is absent, set destructuring,
        // and destructuring from a function that returns an object.
        let source = r#"var src = {
    a: () => { console.log("a"); },
    b: { c: () => { console.log("c"); } },
    get g() { return () => { console.log("g"); }; }
};
var { b: { c: nested }, g: getterVal, missing: dflt = () => { console.log("dflt"); } } = src;
nested();
getterVal();
dflt();

const [s1, s2] = new Set([() => { console.log("s1"); }, () => { console.log("s2"); }]);
s1();
s2();

function make() {
    const o = { m1: function () { console.log("m1"); }, m2: { m3: function () { console.log("m3"); } } };
    return o;
}
const { m1, m2 } = make();
m1();
m2.m3();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);

        let resolve_for = |needle: &str| -> Vec<String> {
            let site = site_id_for_span(source, &sites, needle);
            targets
                .iter()
                .filter(|t| t.site == site)
                .filter_map(|t| t.target_function)
                .filter_map(|id| db.functions().iter().find(|f| f.id == id))
                .map(|f| source[f.span.start_byte as usize..f.span.end_byte as usize].to_string())
                .collect()
        };
        for (site, target) in [
            ("nested()", "\"c\""),    // nested object pattern b.c
            ("getterVal()", "\"g\""), // getter-valued source
            ("dflt()", "\"dflt\""),   // default used (property absent)
            ("s1()", "\"s1\""),       // set destructuring, element 0
            ("s2()", "\"s2\""),       // set destructuring, element 1
            ("m1()", "\"m1\""),       // object returned from a function
            ("m2.m3()", "\"m3\""),    // nested object returned from a function
        ] {
            assert!(
                resolve_for(site).iter().any(|s| s.contains(target)),
                "expected {site} to reach a target containing `{target}`; got {:?}",
                resolve_for(site)
            );
        }
    }

    #[test]
    fn real_ts_pipeline_resolves_this_flow() {
        // Phase C this-flow: a method returning `this.member`, function-object
        // `this` (`f.h = function(){ this.g() }`), and a returned arrow that
        // captures `this` lexically.
        let source = r#"var obj = {
    p: function () { return this.q; },
    q: function () { console.log("q"); }
};
var t = obj.p();
t();

function f() {}
f.g = function () { console.log("g"); };
f.h = function () { this.g(); };
f.h();

const o = {
    foo() { return () => this.bar(); },
    bar() { console.log("bar"); }
};
const l = o.foo();
l();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);

        let resolve_for = |needle: &str| -> Vec<String> {
            let site = site_id_for_span(source, &sites, needle);
            targets
                .iter()
                .filter(|t| t.site == site)
                .filter_map(|t| t.target_function)
                .filter_map(|id| db.functions().iter().find(|f| f.id == id))
                .map(|f| source[f.span.start_byte as usize..f.span.end_byte as usize].to_string())
                .collect()
        };
        for (site, target) in [
            ("t()", "\"q\""),          // obj.p() returns this.q -> obj.q
            ("this.g()", "\"g\""),     // function-object this: f.h's this is f
            ("this.bar()", "\"bar\""), // returned arrow captures this = o
        ] {
            assert!(
                resolve_for(site).iter().any(|s| s.contains(target)),
                "expected {site} to reach a target containing `{target}`; got {:?}",
                resolve_for(site)
            );
        }
    }

    #[test]
    fn destructuring_default_not_applied_when_property_present_but_unresolved() {
        // A destructuring default must be used only when the property KEY is absent,
        // not whenever the value fails to resolve to a callable. Here `cb` is present
        // (an opaque parameter), so the default `() => dflt()` must NOT bind to `cb`
        // (that would be a false-positive edge to `dflt`).
        let source = r#"function dflt() {}
function build(opaque) {
    var src = { cb: opaque };
    var { cb = () => { dflt(); } } = src;
    cb();
}
build(function () {});
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);
        let site = site_id_for_span(source, &sites, "cb()");
        let resolved: Vec<String> = targets
            .iter()
            .filter(|t| t.site == site)
            .filter_map(|t| t.target_function)
            .filter_map(|id| db.functions().iter().find(|f| f.id == id))
            .map(|f| source[f.span.start_byte as usize..f.span.end_byte as usize].to_string())
            .collect();
        assert!(
            !resolved.iter().any(|s| s.contains("function dflt()")),
            "present-but-unresolved `cb` must not bind the dead default; got {resolved:?}"
        );
    }

    #[test]
    fn returned_closure_body_not_walked_until_invoked() {
        // A `this`-capturing arrow returned from a method but NEVER invoked must not
        // emit its internal `this.bar()` edge (Jelly's reachability prune excludes
        // the unreached arrow). When invoked via the bound binding, it does resolve.
        let source = r#"const o = {
    foo() { return () => this.bar(); },
    bar() { barImpl(); }
};
function barImpl() {}
const unused = o.foo();
const used = o.foo();
used();
"#;
        let mut db = db_with_file(source);
        crate::ts::analyze(&mut db);
        let mir = crate::analysis::mir::lower_ts::lower_ts_mir(&db);
        db.replace_semantic_mir(mir).expect("semantic MIR stores");
        let sites = crate::analysis::calls::extract::extract_call_sites(&db);
        let targets = resolve_ts_value_flow_targets(&db, &sites, 0);

        // `this.bar()` inside the arrow resolves to o.bar (the arrow is reached via
        // `used()`), but exactly once — the `unused` binding must not have triggered
        // an eager walk. The edge set is deduped, so we assert the edge exists (used
        // path works) and rely on the FP being gone via the benchmark/no-eager-walk.
        let site = site_id_for_span(source, &sites, "this.bar()");
        let resolved: Vec<String> = targets
            .iter()
            .filter(|t| t.site == site)
            .filter_map(|t| t.target_function)
            .filter_map(|id| db.functions().iter().find(|f| f.id == id))
            .map(|f| source[f.span.start_byte as usize..f.span.end_byte as usize].to_string())
            .collect();
        assert!(
            resolved.iter().any(|s| s.contains("barImpl")),
            "invoked returned-arrow should resolve this.bar() -> o.bar; got {resolved:?}"
        );
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
        site_id_for_nth_span(source, sites, needle, 0)
    }

    fn site_id_for_nth_span(
        source: &str,
        sites: &[CallSiteFact],
        needle: &str,
        ordinal: usize,
    ) -> CallSiteId {
        let file = sites
            .first()
            .map(|site| site.file)
            .expect("at least one call site");
        let span = span_for_nth(source, file, needle, ordinal);
        sites
            .iter()
            .find(|site| {
                site.span.start_byte == span.start_byte && site.span.end_byte == span.end_byte
            })
            .map(|site| site.id)
            .unwrap_or_else(|| panic!("missing call site for {needle}"))
    }

    fn site_id_containing_span(source: &str, sites: &[CallSiteFact], needle: &str) -> CallSiteId {
        let file = sites
            .first()
            .map(|site| site.file)
            .expect("at least one call site");
        let span = span_for_nth(source, file, needle, 0);
        sites
            .iter()
            .filter(|site| {
                site.span.start_byte <= span.start_byte && site.span.end_byte >= span.end_byte
            })
            .min_by_key(|site| site.span.end_byte - site.span.start_byte)
            .map(|site| site.id)
            .unwrap_or_else(|| panic!("missing call site containing {needle}"))
    }

    fn function_id_for_span(source: &str, db: &AnalysisDb, needle: &str) -> FunctionId {
        function_id_for_nth_span(source, db, needle, 0)
    }

    fn function_id_containing_span(source: &str, db: &AnalysisDb, needle: &str) -> FunctionId {
        let file = db.files()[0].id;
        let span = span_for_nth(source, file, needle, 0);
        db.functions()
            .iter()
            .filter(|function| {
                function.file == file
                    && function.span.start_byte <= span.start_byte
                    && function.span.end_byte >= span.end_byte
            })
            .min_by_key(|function| function.span.end_byte - function.span.start_byte)
            .map(|function| function.id)
            .unwrap_or_else(|| panic!("missing function containing {needle}"))
    }

    fn function_id_for_name(db: &AnalysisDb, name: &str) -> FunctionId {
        db.functions()
            .iter()
            .find(|function| function.name == name)
            .map(|function| function.id)
            .unwrap_or_else(|| panic!("missing function named {name}"))
    }

    fn function_id_for_nth_span(
        source: &str,
        db: &AnalysisDb,
        needle: &str,
        ordinal: usize,
    ) -> FunctionId {
        let file = db.files()[0].id;
        let span = span_for_nth(source, file, needle, ordinal);
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
