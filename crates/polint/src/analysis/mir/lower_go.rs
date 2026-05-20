use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::{Node, Parser};

use crate::analysis::ids::{
    CallSiteId, MirBodyId, MirOpId, MirPredicateId, PlaceId, UnsupportedId,
};
use crate::analysis::mir::body::MirOutput;
use crate::analysis::mir::body::{MirBody, MirStatus};
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

pub(crate) fn lower_go_mir(db: &AnalysisDb) -> MirOutput {
    let mut lowering = GoMirLowering::default();
    let mut files = db
        .files()
        .iter()
        .filter(|file| file.language == Language::Go)
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
struct GoMirLowering {
    bodies: Vec<MirBody>,
    places: PlaceTableBuilder,
    operations: Vec<OperationDraft>,
    unsupported: Vec<UnsupportedDraft>,
}

impl GoMirLowering {
    fn lower_file(&mut self, db: &AnalysisDb, file: &SourceFile) {
        let mut parser = Parser::new();
        if parser
            .set_language(&tree_sitter_go::LANGUAGE.into())
            .is_err()
        {
            return;
        }
        let Some(tree) = parser.parse(file.source.as_ref(), None) else {
            return;
        };
        let root = tree.root_node();
        let mut functions = Vec::new();
        visit_named_descendants(root, &mut |node| {
            if matches!(node.kind(), "function_declaration" | "method_declaration") {
                functions.push(node);
            }
        });
        functions.sort_by(|left, right| {
            (
                file.relative_path.as_str(),
                left.start_byte(),
                left.end_byte(),
                function_name(file.source.as_ref(), *left).unwrap_or_default(),
            )
                .cmp(&(
                    file.relative_path.as_str(),
                    right.start_byte(),
                    right.end_byte(),
                    function_name(file.source.as_ref(), *right).unwrap_or_default(),
                ))
        });

        for node in functions {
            let Some(body_node) = node.child_by_field_name("body") else {
                continue;
            };
            let Some(name) = function_name(file.source.as_ref(), node) else {
                continue;
            };
            let span = node_span(file.id, file.source.as_ref(), node);
            let Some(function) = matching_function(db, file.id, &name, &span) else {
                continue;
            };
            let body = self.push_body(db, file, function, span);
            let mut function_lowering =
                FunctionLowering::new(file, file.source.as_ref(), function.id, &body);
            function_lowering.lower_parameters(node, &mut self.places);
            function_lowering.lower_body(
                body_node,
                &mut self.places,
                &mut self.operations,
                &mut self.unsupported,
            );
            self.lower_parser_errors(file, body.id, &body.stable_key, body_node);
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
                ("language", "go".to_string()),
                ("path", file.relative_path.clone()),
                ("owner", owner_stable_key.clone()),
                ("start_byte", span.start_byte.to_string()),
                ("end_byte", span.end_byte.to_string()),
            ],
        )
        .into_string();
        let body = MirBody {
            id,
            language: Language::Go,
            file: file.id,
            function: function.id,
            package: db
                .packages()
                .iter()
                .find(|package| package.file == file.id && package.language == Language::Go)
                .map(|package| package.id),
            module: db
                .module_nodes()
                .iter()
                .find(|module| {
                    module.file == Some(file.id) && module.language == Some(Language::Go)
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

    fn lower_parser_errors(
        &mut self,
        file: &SourceFile,
        body: MirBodyId,
        body_stable_key: &str,
        node: Node<'_>,
    ) {
        visit_named_descendants(node, &mut |descendant| {
            if descendant.is_error() || descendant.kind() == "ERROR" {
                let unsupported_id = UnsupportedId(self.unsupported.len() as u64);
                let operation_id = MirOpId(self.operations.len() as u64);
                let span = node_span(file.id, file.source.as_ref(), descendant);
                self.unsupported.push(UnsupportedDraft::new(
                    unsupported_id,
                    Some(body),
                    Some(operation_id),
                    file.relative_path.clone(),
                    file.id,
                    span.clone(),
                    "ERROR",
                    node_text(file.source.as_ref(), descendant).unwrap_or("ERROR"),
                    vec![UnsupportedDomain::Mir, UnsupportedDomain::Cfg],
                    ConservativeAction::StopLowering,
                ));
                self.operations.push(OperationDraft::new(
                    operation_id,
                    body,
                    body_stable_key,
                    operation_id.0 as u32,
                    span,
                    OperationKindDraft::Unsupported { unsupported_id },
                    MirStatus::Unsupported,
                ));
            }
        });
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

struct FunctionLowering<'source> {
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

    fn lower_parameters(&mut self, node: Node<'_>, places: &mut PlaceTableBuilder) {
        let mut index = 0_u32;
        if let Some(receiver) = node.child_by_field_name("receiver") {
            let name = parameter_names(self.source, receiver).into_iter().next();
            let root = PlaceRoot::Parameter {
                function: self.function,
                index,
                name: name.clone(),
            };
            if let Some(name) = name {
                self.parameters.insert(name, root.clone());
            }
            self.insert_place(places, root, Vec::new(), PlaceStatus::Resolved);
            index += 1;
        }

        if let Some(parameters) = node.child_by_field_name("parameters") {
            for name in parameter_names(self.source, parameters) {
                let root = PlaceRoot::Parameter {
                    function: self.function,
                    index,
                    name: Some(name.clone()),
                };
                self.parameters.insert(name, root.clone());
                self.insert_place(places, root, Vec::new(), PlaceStatus::Resolved);
                index += 1;
            }
        }
    }

    fn lower_body(
        &mut self,
        body: Node<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        for index in 0..body.named_child_count() as u32 {
            let Some(statement) = body.named_child(index) else {
                continue;
            };
            self.lower_statement(statement, places, operations, unsupported);
        }
    }

    fn lower_statement(
        &mut self,
        statement: Node<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        self.lower_unsupported(statement, operations, unsupported);
        match statement.kind() {
            "short_var_declaration" => {
                let value = statement
                    .child_by_field_name("right")
                    .and_then(|right| self.lower_value(right, places, operations, unsupported))
                    .unwrap_or_else(|| ValueDraft::Unknown {
                        evidence: "short declaration initializer".to_string(),
                    });
                for name in assignment_left_names(self.source, statement) {
                    let key = self.insert_local(places, &name);
                    self.push_assign(
                        operations,
                        statement,
                        key,
                        value.clone(),
                        AssignMode::DeclarationBinding,
                    );
                }
            }
            "var_declaration" => {
                let mut names = BTreeSet::new();
                visit_named_descendants(statement, &mut |node| {
                    if node.kind() == "var_spec" {
                        names.extend(var_spec_names(self.source, node));
                    }
                });
                for name in names {
                    let key = self.insert_local(places, &name);
                    self.push_assign(
                        operations,
                        statement,
                        key,
                        ValueDraft::Unknown {
                            evidence: "zero value".to_string(),
                        },
                        AssignMode::DeclarationBinding,
                    );
                }
            }
            "assignment_statement" => {
                let left_places = self.assignment_left_places(statement, places);
                let value = statement
                    .child_by_field_name("right")
                    .and_then(|right| self.lower_value(right, places, operations, unsupported))
                    .unwrap_or_else(|| ValueDraft::Unknown {
                        evidence: "assignment value".to_string(),
                    });
                let simultaneous = left_places.len() > 1;
                let compound = assignment_operator(self.source, statement)
                    .is_some_and(|operator| operator != "=");
                for place in left_places {
                    let mode = if compound {
                        AssignMode::PartialWrite
                    } else if simultaneous {
                        AssignMode::Simultaneous
                    } else if !place.projections.is_empty() {
                        AssignMode::ProjectionMutation
                    } else {
                        AssignMode::Overwrite
                    };
                    self.push_assign(operations, statement, place.key, value.clone(), mode);
                }
            }
            "if_statement"
            | "for_statement"
            | "expression_switch_statement"
            | "type_switch_statement"
            | "switch_statement" => {
                self.push_branch(operations, statement);
                for index in 0..statement.named_child_count() as u32 {
                    let Some(child) = statement.named_child(index) else {
                        continue;
                    };
                    self.lower_statement(child, places, operations, unsupported);
                }
            }
            "return_statement" => {
                let value = statement
                    .named_child(0)
                    .and_then(|child| self.lower_value(child, places, operations, unsupported));
                self.push_operation(
                    operations,
                    statement,
                    OperationKindDraft::Return { value },
                    MirStatus::Partial,
                );
            }
            "call_expression" => {
                self.lower_call(statement, places, operations, unsupported);
            }
            "identifier" | "selector_expression" | "index_expression" | "binary_expression" => {
                if let Some(shape) =
                    self.lower_expression(statement, places, operations, unsupported, false)
                {
                    self.push_operation(
                        operations,
                        statement,
                        OperationKindDraft::Read {
                            place_key: shape.key,
                        },
                        MirStatus::Partial,
                    );
                }
            }
            _ => {
                for index in 0..statement.named_child_count() as u32 {
                    let Some(child) = statement.named_child(index) else {
                        continue;
                    };
                    self.lower_statement(child, places, operations, unsupported);
                }
            }
        }
    }

    fn assignment_left_places(
        &mut self,
        statement: Node<'_>,
        places: &mut PlaceTableBuilder,
    ) -> Vec<PlaceShape> {
        if let Some(left) = statement.child_by_field_name("left") {
            let mut shapes = Vec::new();
            for index in 0..left.named_child_count() as u32 {
                if let Some(child) = left.named_child(index)
                    && let Some(shape) =
                        self.lower_expression(child, places, &mut Vec::new(), &mut Vec::new(), true)
                {
                    shapes.push(shape);
                }
            }
            if !shapes.is_empty() {
                return shapes;
            }
        }

        assignment_left_names(self.source, statement)
            .into_iter()
            .map(|name| {
                if let Some(root) = self
                    .locals
                    .get(&name)
                    .or_else(|| self.parameters.get(&name))
                {
                    let shape = PlaceShape {
                        root: root.clone(),
                        projections: Vec::new(),
                        status: PlaceStatus::Resolved,
                        key: self.insert_place(
                            places,
                            root.clone(),
                            Vec::new(),
                            PlaceStatus::Resolved,
                        ),
                    };
                    shape
                } else {
                    let root = PlaceRoot::Global { symbol: None, name };
                    let key =
                        self.insert_place(places, root.clone(), Vec::new(), PlaceStatus::Partial);
                    PlaceShape {
                        root,
                        projections: Vec::new(),
                        status: PlaceStatus::Partial,
                        key,
                    }
                }
            })
            .collect()
    }

    fn insert_local(&mut self, places: &mut PlaceTableBuilder, name: &str) -> String {
        let root = PlaceRoot::Local {
            function: self.function,
            name: name.to_string(),
        };
        self.locals.insert(name.to_string(), root.clone());
        self.insert_place(places, root, Vec::new(), PlaceStatus::Resolved)
    }

    fn lower_value(
        &mut self,
        node: Node<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) -> Option<ValueDraft> {
        match node.kind() {
            "interpreted_string_literal"
            | "raw_string_literal"
            | "int_literal"
            | "float_literal"
            | "rune_literal"
            | "true"
            | "false"
            | "nil" => Some(ValueDraft::Literal {
                value: node_text(self.source, node).unwrap_or_default().to_string(),
            }),
            "call_expression" => self
                .lower_call(node, places, operations, unsupported)
                .map(ValueDraft::PlaceKey),
            _ => self
                .lower_expression(node, places, operations, unsupported, false)
                .map(|shape| ValueDraft::PlaceKey(shape.key)),
        }
    }

    fn lower_expression(
        &mut self,
        node: Node<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
        assignment_destination: bool,
    ) -> Option<PlaceShape> {
        self.lower_unsupported(node, operations, unsupported);
        match node.kind() {
            "identifier" => self.lower_identifier(node, places, assignment_destination),
            "selector_expression" => self.lower_selector(
                node,
                places,
                operations,
                unsupported,
                assignment_destination,
            ),
            "index_expression" => self.lower_index(
                node,
                places,
                operations,
                unsupported,
                assignment_destination,
            ),
            "call_expression" => {
                self.lower_call(node, places, operations, unsupported)
                    .map(|key| PlaceShape {
                        root: PlaceRoot::CallReturn {
                            call: self.call_site_for(node),
                        },
                        projections: Vec::new(),
                        status: PlaceStatus::Partial,
                        key,
                    })
            }
            "interpreted_string_literal"
            | "raw_string_literal"
            | "int_literal"
            | "float_literal"
            | "rune_literal" => None,
            _ => {
                let mut last = None;
                for index in 0..node.named_child_count() as u32 {
                    let Some(child) = node.named_child(index) else {
                        continue;
                    };
                    last = self
                        .lower_expression(
                            child,
                            places,
                            operations,
                            unsupported,
                            assignment_destination,
                        )
                        .or(last);
                }
                last
            }
        }
    }

    fn lower_identifier(
        &mut self,
        node: Node<'_>,
        places: &mut PlaceTableBuilder,
        assignment_destination: bool,
    ) -> Option<PlaceShape> {
        if is_selector_field(node) {
            return None;
        }
        let name = node_text(self.source, node)?.to_string();
        let (root, status) = if let Some(root) = self.locals.get(&name) {
            (root.clone(), PlaceStatus::Resolved)
        } else if let Some(root) = self.parameters.get(&name) {
            (root.clone(), PlaceStatus::Resolved)
        } else if assignment_destination {
            (
                PlaceRoot::Global { symbol: None, name },
                PlaceStatus::Partial,
            )
        } else {
            (
                PlaceRoot::Unknown {
                    evidence: name.clone(),
                },
                PlaceStatus::Unknown,
            )
        };
        let shape = PlaceShape {
            root,
            projections: Vec::new(),
            status,
            key: String::new(),
        };
        let key = self.insert_shape(places, &shape);
        let shape = PlaceShape { key, ..shape };
        Some(shape)
    }

    fn lower_selector(
        &mut self,
        node: Node<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
        assignment_destination: bool,
    ) -> Option<PlaceShape> {
        let operand = node
            .child_by_field_name("operand")
            .or_else(|| node.named_child(0))?;
        let field = node
            .child_by_field_name("field")
            .or_else(|| node.named_child(1))
            .and_then(|field| node_text(self.source, field))?;
        let mut shape = self.lower_expression(
            operand,
            places,
            operations,
            unsupported,
            assignment_destination,
        )?;
        shape
            .projections
            .push(PlaceProjection::Field(field.to_string()));
        shape.key = self.insert_shape(places, &shape);
        self.insert_temporary(places, node, PlaceStatus::Partial);
        Some(shape)
    }

    fn lower_index(
        &mut self,
        node: Node<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
        assignment_destination: bool,
    ) -> Option<PlaceShape> {
        let operand = node
            .child_by_field_name("operand")
            .or_else(|| node.named_child(0))?;
        let index = node
            .child_by_field_name("index")
            .or_else(|| node.named_child(1))?;
        let mut shape = self.lower_expression(
            operand,
            places,
            operations,
            unsupported,
            assignment_destination,
        )?;
        shape.projections.push(index_projection(self.source, index));
        shape.key = self.insert_shape(places, &shape);
        self.insert_temporary(places, node, PlaceStatus::Partial);
        Some(shape)
    }

    fn lower_call(
        &mut self,
        node: Node<'_>,
        places: &mut PlaceTableBuilder,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) -> Option<String> {
        self.lower_unsupported_call(node, unsupported);
        let site = self.call_site_for(node);
        let return_key = self.insert_place(
            places,
            PlaceRoot::CallReturn { call: site },
            Vec::new(),
            PlaceStatus::Partial,
        );
        let callee = node
            .child_by_field_name("function")
            .or_else(|| node.named_child(0))
            .and_then(|callee| node_text(self.source, callee))
            .map(|evidence| ValueDraft::Unknown {
                evidence: evidence.to_string(),
            })
            .unwrap_or_else(|| ValueDraft::Unknown {
                evidence: "call".to_string(),
            });
        let mut arguments = Vec::new();
        for index in 1..node.named_child_count() as u32 {
            let Some(child) = node.named_child(index) else {
                continue;
            };
            if let Some(shape) =
                self.lower_expression(child, places, operations, unsupported, false)
            {
                arguments.push(shape.key);
            }
        }
        self.push_operation(
            operations,
            node,
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

    fn lower_unsupported(
        &self,
        node: Node<'_>,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
    ) {
        let construct = match node.kind() {
            "go_statement" => Some("go_statement"),
            "defer_statement" => Some("defer_statement"),
            "select_statement" => Some("select_statement"),
            "send_statement" => Some("send_statement"),
            _ if node.is_error() || node.kind() == "ERROR" => Some("ERROR"),
            _ if node_text(self.source, node)
                .is_some_and(|text| text.trim_start().starts_with("<-")) =>
            {
                Some("channel_receive")
            }
            _ => None,
        };
        let Some(construct) = construct else {
            return;
        };
        self.push_unsupported(
            operations,
            unsupported,
            node,
            construct,
            unsupported_domains_for(construct),
            if construct == "ERROR" {
                ConservativeAction::StopLowering
            } else {
                ConservativeAction::HavocAffectedPlaces
            },
        );
    }

    fn lower_unsupported_call(&self, node: Node<'_>, unsupported: &mut Vec<UnsupportedDraft>) {
        let text = node_text(self.source, node).unwrap_or_default();
        let construct = if text.contains("panic(") {
            Some("panic")
        } else if text.contains("recover(") {
            Some("recover")
        } else if text.contains("unsafe.") {
            Some("unsafe")
        } else if text.contains("reflect.") {
            Some("reflect")
        } else {
            None
        };
        if let Some(construct) = construct {
            unsupported.push(UnsupportedDraft::new(
                UnsupportedId(unsupported.len() as u64),
                Some(self.body),
                None,
                self.stable_context.file_key().to_string(),
                self.file,
                node_span(self.file, self.source, node),
                construct,
                text,
                unsupported_domains_for(construct),
                ConservativeAction::HavocAffectedPlaces,
            ));
        }
    }

    fn push_unsupported(
        &self,
        operations: &mut Vec<OperationDraft>,
        unsupported: &mut Vec<UnsupportedDraft>,
        node: Node<'_>,
        construct: &str,
        domains: Vec<UnsupportedDomain>,
        action: ConservativeAction,
    ) {
        let unsupported_id = UnsupportedId(unsupported.len() as u64);
        let operation_id = MirOpId(operations.len() as u64);
        let span = node_span(self.file, self.source, node);
        unsupported.push(UnsupportedDraft::new(
            unsupported_id,
            Some(self.body),
            Some(operation_id),
            self.stable_context.file_key().to_string(),
            self.file,
            span.clone(),
            construct,
            node_text(self.source, node).unwrap_or(construct),
            domains,
            action,
        ));
        operations.push(OperationDraft::new(
            operation_id,
            self.body,
            self.stable_context.body_key(),
            self.ordinal_for(node),
            span,
            OperationKindDraft::Unsupported { unsupported_id },
            MirStatus::Unsupported,
        ));
    }

    fn push_assign(
        &self,
        operations: &mut Vec<OperationDraft>,
        node: Node<'_>,
        place_key: String,
        value: ValueDraft,
        mode: AssignMode,
    ) {
        self.push_operation(
            operations,
            node,
            OperationKindDraft::Assign {
                place_key,
                value,
                mode,
            },
            MirStatus::Partial,
        );
    }

    fn push_branch(&self, operations: &mut Vec<OperationDraft>, node: Node<'_>) {
        self.push_operation(
            operations,
            node,
            OperationKindDraft::Branch {
                predicate: MirPredicateId(self.ordinal_for(node) as u64),
            },
            MirStatus::Partial,
        );
    }

    fn push_operation(
        &self,
        operations: &mut Vec<OperationDraft>,
        node: Node<'_>,
        kind: OperationKindDraft,
        status: MirStatus,
    ) {
        let id = MirOpId(operations.len() as u64);
        operations.push(OperationDraft::new(
            id,
            self.body,
            self.stable_context.body_key(),
            self.ordinal_for(node),
            node_span(self.file, self.source, node),
            kind,
            status,
        ));
    }

    fn ordinal_for(&self, node: Node<'_>) -> u32 {
        node.start_byte() as u32
    }

    fn call_site_for(&self, node: Node<'_>) -> CallSiteId {
        CallSiteId(node.start_byte() as u64)
    }

    fn insert_temporary(
        &self,
        places: &mut PlaceTableBuilder,
        node: Node<'_>,
        status: PlaceStatus,
    ) -> String {
        self.insert_place(
            places,
            PlaceRoot::Temporary {
                body: self.body,
                ordinal: node.start_byte() as u32,
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
            Language::Go,
            Some(self.file),
            Some(self.function),
            &self.stable_context,
            root,
            projections,
            status,
        )
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum ValueDraft {
    Literal { value: String },
    PlaceKey(String),
    Unknown { evidence: String },
}

impl ValueDraft {
    fn to_value(&self, place_ids: &BTreeMap<String, PlaceId>) -> MirValue {
        match self {
            Self::Literal { value } => MirValue::Literal {
                value: value.clone(),
            },
            Self::PlaceKey(key) => place_ids
                .get(key)
                .map(|id| MirValue::Place(*id))
                .unwrap_or_else(|| MirValue::Unknown {
                    evidence: key.clone(),
                }),
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
        file_key: impl Into<String>,
        file: FileId,
        span: Span,
        construct: &str,
        source_evidence: &str,
        affected_domains: Vec<UnsupportedDomain>,
        conservative_action: ConservativeAction,
    ) -> Self {
        Self {
            id,
            body,
            operation,
            file_key: file_key.into(),
            file,
            span,
            construct: construct.to_string(),
            source_evidence: source_evidence.trim().to_string(),
            affected_place_keys: Vec::new(),
            affected_domains,
            conservative_action,
        }
    }

    fn to_fact(&self, place_ids: &BTreeMap<String, PlaceId>) -> UnsupportedSemanticFact {
        UnsupportedSemanticFact {
            id: self.id,
            body: self.body,
            operation: self.operation,
            language: Language::Go,
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
    body_stable_key: &str,
    ordinal: u32,
    span: &Span,
    kind: &OperationKindDraft,
) -> String {
    let mut parts = vec![
        ("language", "go".to_string()),
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
            ("language", "go".to_string()),
            ("file", draft.file_key.clone()),
            ("construct", draft.construct.clone()),
            ("start_byte", draft.span.start_byte.to_string()),
            ("end_byte", draft.span.end_byte.to_string()),
            ("evidence", draft.source_evidence.clone()),
        ],
    )
    .into_string()
}

fn unsupported_domains_for(construct: &str) -> Vec<UnsupportedDomain> {
    match construct {
        "go_statement" | "defer_statement" => vec![
            UnsupportedDomain::Mir,
            UnsupportedDomain::Cfg,
            UnsupportedDomain::Calls,
            UnsupportedDomain::DataFlow,
        ],
        "select_statement" | "send_statement" | "channel_receive" => vec![
            UnsupportedDomain::Mir,
            UnsupportedDomain::Cfg,
            UnsupportedDomain::Domains,
            UnsupportedDomain::DataFlow,
        ],
        "panic" | "recover" => vec![
            UnsupportedDomain::Mir,
            UnsupportedDomain::Cfg,
            UnsupportedDomain::Domains,
            UnsupportedDomain::DataFlow,
        ],
        "reflect" | "unsafe" => vec![
            UnsupportedDomain::Mir,
            UnsupportedDomain::Calls,
            UnsupportedDomain::Domains,
            UnsupportedDomain::Aliases,
            UnsupportedDomain::DataFlow,
        ],
        _ => vec![UnsupportedDomain::Mir, UnsupportedDomain::Cfg],
    }
}

fn matching_function<'db>(
    db: &'db AnalysisDb,
    file: FileId,
    name: &str,
    span: &Span,
) -> Option<&'db FunctionFact> {
    db.functions().iter().find(|function| {
        function.file == file
            && function.language == Language::Go
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
            ("language", "go".to_string()),
            ("path", file.relative_path.clone()),
            ("function", function.name.clone()),
            ("start_byte", function.span.start_byte.to_string()),
            ("end_byte", function.span.end_byte.to_string()),
        ],
    )
    .into_string()
}

fn node_span(file: FileId, source: &str, node: Node<'_>) -> Span {
    crate::core::span_from_byte_range(file, source, node.start_byte(), node.end_byte())
}

fn node_text<'source>(source: &'source str, node: Node<'_>) -> Option<&'source str> {
    source.get(node.start_byte()..node.end_byte())
}

fn function_name(source: &str, node: Node<'_>) -> Option<String> {
    let simple_name = declaration_name(source, node)?;
    if node.kind() == "method_declaration" {
        receiver_type_name(source, node)
            .map(|receiver| format!("{receiver}.{simple_name}"))
            .or(Some(simple_name))
    } else {
        Some(simple_name)
    }
}

fn declaration_name(source: &str, node: Node<'_>) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|name| node_text(source, name))
        .map(str::to_string)
}

fn receiver_type_name(source: &str, node: Node<'_>) -> Option<String> {
    let receiver = node.child_by_field_name("receiver")?;
    let receiver_text = node_text(source, receiver)?;
    let inner = receiver_text
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    let raw_type = inner.split_whitespace().last()?.trim_start_matches('*');
    if raw_type.is_empty() {
        None
    } else {
        Some(raw_type.to_string())
    }
}

fn parameter_names(source: &str, parameter_list: Node<'_>) -> Vec<String> {
    let mut names = Vec::new();
    for index in 0..parameter_list.named_child_count() as u32 {
        let Some(parameter) = parameter_list.named_child(index) else {
            continue;
        };
        if parameter.kind() != "parameter_declaration" {
            continue;
        }
        for child_index in 0..parameter.named_child_count() as u32 {
            let Some(child) = parameter.named_child(child_index) else {
                continue;
            };
            if matches!(child.kind(), "identifier" | "field_identifier")
                && let Some(name) = node_text(source, child)
            {
                names.push(name.to_string());
            }
        }
    }
    names
}

fn assignment_operator<'source>(source: &'source str, statement: Node<'_>) -> Option<&'source str> {
    statement
        .child_by_field_name("operator")
        .and_then(|operator| node_text(source, operator))
}

fn assignment_left_names(source: &str, statement: Node<'_>) -> Vec<String> {
    if let Some(left) = statement.child_by_field_name("left") {
        let names = direct_identifier_names(source, left);
        if !names.is_empty() {
            return names;
        }
    }

    let Some(text) = node_text(source, statement) else {
        return Vec::new();
    };
    let Some((left, _)) = assignment_delimiters()
        .iter()
        .find_map(|delimiter| text.split_once(delimiter))
    else {
        return Vec::new();
    };
    left.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .filter(|part| {
            part.chars()
                .all(|character| character == '_' || character.is_ascii_alphanumeric())
        })
        .filter(|part| *part != "_")
        .map(str::to_string)
        .collect()
}

fn var_spec_names(source: &str, node: Node<'_>) -> Vec<String> {
    let mut cursor = node.walk();
    node.children_by_field_name("name", &mut cursor)
        .filter_map(|name| node_text(source, name))
        .filter(|name| !name.trim().is_empty() && *name != "_")
        .map(str::to_string)
        .collect()
}

fn direct_identifier_names(source: &str, node: Node<'_>) -> Vec<String> {
    let mut names = Vec::new();
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "identifier"
            && let Some(name) = node_text(source, child)
            && name != "_"
        {
            names.push(name.to_string());
        }
    }
    names
}

fn assignment_delimiters() -> &'static [&'static str] {
    &[
        "&^=", "<<=", ">>=", ":=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "=",
    ]
}

fn index_projection(source: &str, node: Node<'_>) -> PlaceProjection {
    let evidence = node_text(source, node)
        .unwrap_or("unknown")
        .trim()
        .to_string();
    if matches!(
        node.kind(),
        "interpreted_string_literal" | "raw_string_literal" | "int_literal"
    ) {
        PlaceProjection::IndexKnown(evidence.trim_matches(['"', '`']).to_string())
    } else {
        PlaceProjection::IndexUnknown { evidence }
    }
}

fn is_selector_field(node: Node<'_>) -> bool {
    node.parent().is_some_and(|parent| {
        parent.kind() == "selector_expression"
            && parent
                .child_by_field_name("field")
                .is_some_and(|field| field == node)
    })
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

#[cfg(test)]
mod places {
    use super::*;
    use crate::analysis::places::{PlaceProjection, PlaceRoot};
    use crate::core::{AnalysisDb, FunctionId, Language};
    use std::path::PathBuf;

    fn lower(source: &str) -> MirOutput {
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from("auth.go"),
            "auth.go".to_string(),
            source.to_string(),
        );
        let diagnostics = crate::go::analyze(&mut db);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        lower_go_mir(&db)
    }

    #[test]
    fn go_function_places_include_parameters_locals_globals_and_projections() {
        let first = lower(
            r#"
package auth

type User struct { Tokens []string }

func authorize(user User, index int) bool {
    token := user.Tokens[index]
    global = token
    return token != ""
}
"#,
        );
        let second = lower(
            r#"
package auth

type User struct { Tokens []string }

func authorize(user User, index int) bool {
    token := user.Tokens[index]
    global = token
    return token != ""
}
"#,
        );

        assert_eq!(first.bodies.len(), 1);
        assert!(first.bodies[0].stable_key.contains("authorize"));
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
            PlaceRoot::Global { name, .. } if name == "global"
        )));
        assert!(first.places.iter().any(|place| {
            matches!(&place.root, PlaceRoot::Parameter { name: Some(name), .. } if name == "user")
                && place
                    .projections
                    .contains(&PlaceProjection::Field("Tokens".to_string()))
                && place.projections.iter().any(|projection| {
                    matches!(projection, PlaceProjection::IndexUnknown { evidence } if evidence == "index")
                })
        }));
    }

    #[test]
    fn go_method_receiver_is_parameter_zero_and_function_name_contract_is_preserved() {
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from("service.go"),
            "service.go".to_string(),
            r#"
package auth

type Service struct { cache map[string]string }

func (svc *Service) authorize(user User) bool {
    token := svc.cache[user.Name]
    return token != ""
}
"#
            .to_string(),
        );
        let diagnostics = crate::go::analyze(&mut db);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");

        let function = db
            .functions()
            .iter()
            .find(|function| function.name == "Service.authorize")
            .expect("method fact should retain existing receiver-qualified name");
        assert_eq!(function.id, FunctionId(0));
        assert_eq!(function.language, Language::Go);

        let output = lower_go_mir(&db);
        assert!(output.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Parameter {
                function,
                index: 0,
                name: Some(name),
            } if *function == FunctionId(0) && name == "svc"
        )));
        assert!(
            output.bodies[0]
                .owner_stable_key
                .contains("Service.authorize")
        );
    }

    #[test]
    fn go_mir_place_rows_do_not_carry_parser_node_debug_evidence() {
        let output = lower(
            r#"
package auth

func authorize(user User) bool {
    token := user.Token
    return token != ""
}
"#,
        );
        let debug = format!("{output:#?}");

        assert!(!debug.contains("tree_sitter::Node"));
        assert!(!debug.contains("Node<'_"));
        assert!(!debug.contains("function_declaration"));
        assert!(!debug.contains("method_declaration"));
    }
}

#[cfg(test)]
mod operations {
    use super::*;
    use crate::analysis::mir::op::{
        AssignMode, ConservativeAction, MirOperationKind, UnsupportedDomain,
    };
    use std::path::PathBuf;

    fn lower(source: &str) -> MirOutput {
        let mut db = AnalysisDb::new();
        db.add_file(
            PathBuf::from("flow.go"),
            "flow.go".to_string(),
            source.to_string(),
        );
        let diagnostics = crate::go::analyze(&mut db);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        lower_go_mir(&db)
    }

    #[test]
    fn go_statement_lowering_emits_assignment_modes_and_control_shapes() {
        let output = lower(
            r#"
package auth

type User struct { Tokens []string }

func flow(user User, index int) bool {
    var count int
    token := user.Tokens[index]
    a, b = b, a
    user.Tokens[index] = token
    count = index
    if token != "" { count = count + 1 }
    for count < 10 { count = count + 1 }
    switch token { case "": return false; default: count = count + 1 }
    return token != ""
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
        assert!(modes.contains(&AssignMode::Simultaneous));
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
    }

    #[test]
    fn go_declarations_and_compound_assignments_keep_all_mutated_places() {
        let output = lower(
            r#"
package auth

func flow(delta int) int {
    var count, limit int
    count += delta
    limit = count
    return limit
}
"#,
        );

        assert!(output.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Local { name, .. } if name == "count"
        )));
        assert!(output.places.iter().any(|place| matches!(
            &place.root,
            PlaceRoot::Local { name, .. } if name == "limit"
        )));
        assert!(output.operations.iter().any(|operation| matches!(
            operation.kind,
            MirOperationKind::Assign {
                mode: AssignMode::PartialWrite,
                ..
            }
        )));
    }

    #[test]
    fn go_call_operations_are_shape_evidence_with_deterministic_call_sites() {
        let source = r#"
package auth

func flow(token string, count int) bool {
    result := helper(token, count)
    return result
}
"#;
        let first = lower(source);
        let second = lower(source);

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
                    arguments,
                    return_place,
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
                    (key.clone(), *site, arguments.len(), **return_place)
                })
                .collect::<Vec<_>>(),
            second_calls
        );
        assert!(
            first_calls[0].2
                != &crate::analysis::mir::op::MirValue::Unknown {
                    evidence: "direct target".to_string()
                }
        );
    }

    #[test]
    fn go_unsupported_semantics_are_structured_and_conservative() {
        let output = lower(
            r#"
package auth

func flow(ch chan int, token string, count int) bool {
    go helper(token)
    defer helper(token)
    select {
    case ch <- count:
    case value := <-ch:
        count = value
    }
    reflect.ValueOf(token)
    unsafe.Sizeof(count)
    panic(token)
    recover()
    return token != ""
}
"#,
        );

        for construct in [
            "go_statement",
            "defer_statement",
            "select_statement",
            "send_statement",
            "channel_receive",
            "reflect",
            "unsafe",
            "panic",
            "recover",
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
