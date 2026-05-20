use std::collections::{BTreeMap, BTreeSet};

use tree_sitter::{Node, Parser};

use crate::analysis::ids::MirBodyId;
use crate::analysis::mir::body::MirOutput;
use crate::analysis::mir::body::{MirBody, MirStatus};
use crate::analysis::places::{PlaceProjection, PlaceRoot, PlaceStatus, PlaceTableBuilder};
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

    MirOutput {
        bodies: lowering.bodies,
        places: lowering.places.finish(),
        operations: Vec::new(),
        unsupported: Vec::new(),
    }
    .normalized()
}

#[derive(Debug, Default)]
struct GoMirLowering {
    bodies: Vec<MirBody>,
    places: PlaceTableBuilder,
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
                FunctionLowering::new(file, file.source.as_ref(), function.id, body.id);
            function_lowering.lower_parameters(node, &mut self.places);
            function_lowering.lower_body(body_node, &mut self.places);
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
}

struct FunctionLowering<'source> {
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
            file: file.id,
            source,
            function,
            body,
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

    fn lower_body(&mut self, body: Node<'_>, places: &mut PlaceTableBuilder) {
        for index in 0..body.named_child_count() as u32 {
            let Some(statement) = body.named_child(index) else {
                continue;
            };
            self.lower_statement(statement, places);
        }
    }

    fn lower_statement(&mut self, statement: Node<'_>, places: &mut PlaceTableBuilder) {
        match statement.kind() {
            "short_var_declaration" => {
                for name in assignment_left_names(self.source, statement, ":=") {
                    self.insert_local(places, &name);
                }
                if let Some(right) = statement.child_by_field_name("right") {
                    self.lower_expression(right, places, false);
                } else {
                    self.lower_non_left_children(statement, places);
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
                    self.insert_local(places, &name);
                }
                self.lower_non_left_children(statement, places);
            }
            "assignment_statement" => {
                if let Some(left) = statement.child_by_field_name("left") {
                    self.lower_expression(left, places, true);
                } else {
                    for name in assignment_left_names(self.source, statement, "=") {
                        if !self.locals.contains_key(&name) && !self.parameters.contains_key(&name)
                        {
                            let root = PlaceRoot::Global { symbol: None, name };
                            self.insert_place(places, root, Vec::new(), PlaceStatus::Partial);
                        }
                    }
                }
                if let Some(right) = statement.child_by_field_name("right") {
                    self.lower_expression(right, places, false);
                }
            }
            "identifier"
            | "selector_expression"
            | "index_expression"
            | "binary_expression"
            | "call_expression"
            | "return_statement" => {
                self.lower_expression(statement, places, false);
            }
            _ => {
                for index in 0..statement.named_child_count() as u32 {
                    let Some(child) = statement.named_child(index) else {
                        continue;
                    };
                    self.lower_statement(child, places);
                }
            }
        }
    }

    fn lower_non_left_children(&mut self, statement: Node<'_>, places: &mut PlaceTableBuilder) {
        for index in 0..statement.named_child_count() as u32 {
            let Some(child) = statement.named_child(index) else {
                continue;
            };
            self.lower_expression(child, places, false);
        }
    }

    fn insert_local(&mut self, places: &mut PlaceTableBuilder, name: &str) {
        let root = PlaceRoot::Local {
            function: self.function,
            name: name.to_string(),
        };
        self.locals.insert(name.to_string(), root.clone());
        self.insert_place(places, root, Vec::new(), PlaceStatus::Resolved);
    }

    fn lower_expression(
        &mut self,
        node: Node<'_>,
        places: &mut PlaceTableBuilder,
        assignment_destination: bool,
    ) -> Option<PlaceShape> {
        match node.kind() {
            "identifier" => self.lower_identifier(node, places, assignment_destination),
            "selector_expression" => self.lower_selector(node, places, assignment_destination),
            "index_expression" => self.lower_index(node, places, assignment_destination),
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
                        .lower_expression(child, places, assignment_destination)
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
        };
        self.insert_shape(places, &shape);
        Some(shape)
    }

    fn lower_selector(
        &mut self,
        node: Node<'_>,
        places: &mut PlaceTableBuilder,
        assignment_destination: bool,
    ) -> Option<PlaceShape> {
        let operand = node
            .child_by_field_name("operand")
            .or_else(|| node.named_child(0))?;
        let field = node
            .child_by_field_name("field")
            .or_else(|| node.named_child(1))
            .and_then(|field| node_text(self.source, field))?;
        let mut shape = self.lower_expression(operand, places, assignment_destination)?;
        shape
            .projections
            .push(PlaceProjection::Field(field.to_string()));
        self.insert_shape(places, &shape);
        self.insert_temporary(places, node, PlaceStatus::Partial);
        Some(shape)
    }

    fn lower_index(
        &mut self,
        node: Node<'_>,
        places: &mut PlaceTableBuilder,
        assignment_destination: bool,
    ) -> Option<PlaceShape> {
        let operand = node
            .child_by_field_name("operand")
            .or_else(|| node.named_child(0))?;
        let index = node
            .child_by_field_name("index")
            .or_else(|| node.named_child(1))?;
        let mut shape = self.lower_expression(operand, places, assignment_destination)?;
        shape.projections.push(index_projection(self.source, index));
        self.insert_shape(places, &shape);
        self.insert_temporary(places, node, PlaceStatus::Partial);
        Some(shape)
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

    fn insert_shape(&self, places: &mut PlaceTableBuilder, shape: &PlaceShape) {
        self.insert_place(
            places,
            shape.root.clone(),
            shape.projections.clone(),
            shape.status,
        );
    }

    fn insert_place(
        &self,
        places: &mut PlaceTableBuilder,
        root: PlaceRoot,
        projections: Vec<PlaceProjection>,
        status: PlaceStatus,
    ) -> String {
        places.insert(
            Language::Go,
            Some(self.file),
            Some(self.function),
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

fn assignment_left_names(source: &str, statement: Node<'_>, delimiter: &str) -> Vec<String> {
    let Some(text) = node_text(source, statement) else {
        return Vec::new();
    };
    let Some((left, _)) = text.split_once(delimiter) else {
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
    let Some(text) = node_text(source, node) else {
        return Vec::new();
    };
    let declaration = text.split_once('=').map_or(text, |(left, _)| left);
    declaration
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty() && *name != "_")
        .map(str::to_string)
        .collect()
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
