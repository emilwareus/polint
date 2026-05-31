#![allow(
    dead_code,
    reason = "Phase 45 wires private inventory extraction into DB/graph consumers across sequential plans"
)]

use oxc_allocator::Allocator;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    BindingPattern, FunctionType, MethodDefinition, MethodDefinitionKind, Program, PropertyKey,
    VariableDeclarator,
};
use oxc_parser::Parser;
use oxc_semantic::{AstNodes, NodeId, SemanticBuilder};
use oxc_span::{GetSpan, SourceType};
use std::path::Path;

use crate::analysis::ids::TsInventoryFunctionId;
use crate::core::{SourceFile, Span, span_from_byte_range};
use crate::ts::inventory::facts::{
    TsFunctionInventoryKind, TsInventoryFunctionFact, TsInventoryStatus,
};
use crate::ts::inventory::store::TsInventoryOutput;

pub(crate) fn extract_ts_inventory(file: &SourceFile) -> TsInventoryOutput {
    let source = file.source.as_ref();
    let allocator = Allocator::default();
    let parsed = Parser::new(&allocator, source, parse_source_type(&file.path)).parse();

    if parsed.panicked && parsed.program.body.is_empty() {
        return TsInventoryOutput::default();
    }

    let semantic = SemanticBuilder::new().build(&parsed.program).semantic;
    extract_ts_inventory_from_program(file, source, &parsed.program, semantic.nodes()).normalized()
}

fn extract_ts_inventory_from_program(
    file: &SourceFile,
    source: &str,
    _program: &Program<'_>,
    nodes: &AstNodes<'_>,
) -> TsInventoryOutput {
    let mut function_rows = Vec::new();
    let mut node_entries = nodes
        .iter_enumerated()
        .filter_map(|(node_id, node)| {
            let kind = function_inventory_kind(nodes, node_id, node.kind())?;
            let span = node.kind().span();
            Some((span.start, span.end, kind, node_id, node.kind()))
        })
        .collect::<Vec<_>>();
    node_entries.sort_by_key(|(start, end, kind, _, _)| (*start, *end, kind.as_str()));

    for (_, _, kind, node_id, ast_kind) in node_entries {
        let span = span_from_oxc(file.id, source, ast_kind.span());
        let display_name = function_display_name(nodes, node_id, ast_kind);
        let lexical_parent_key = lexical_parent_key(file, nodes, node_id);
        let stable_key = function_stable_key(
            file,
            &span,
            kind,
            lexical_parent_key.as_deref(),
            &display_name,
        );
        function_rows.push(TsInventoryFunctionFact {
            id: TsInventoryFunctionId(0),
            file: file.id,
            span,
            stable_key,
            lexical_parent_key,
            display_name,
            kind,
            status: TsInventoryStatus::resolved(),
        });
    }

    TsInventoryOutput {
        functions: function_rows,
        callsites: Vec::new(),
    }
}

fn function_inventory_kind(
    nodes: &AstNodes<'_>,
    node_id: NodeId,
    kind: AstKind<'_>,
) -> Option<TsFunctionInventoryKind> {
    match kind {
        AstKind::Function(function) => {
            if matches!(nodes.parent_kind(node_id), AstKind::MethodDefinition(_)) {
                return None;
            }
            match function.r#type {
                FunctionType::FunctionDeclaration | FunctionType::TSDeclareFunction => {
                    Some(TsFunctionInventoryKind::Declaration)
                }
                FunctionType::FunctionExpression | FunctionType::TSEmptyBodyFunctionExpression => {
                    Some(TsFunctionInventoryKind::FunctionExpression)
                }
            }
        }
        AstKind::ArrowFunctionExpression(_) => Some(TsFunctionInventoryKind::Arrow),
        AstKind::MethodDefinition(method) => Some(method_inventory_kind(method)),
        AstKind::StaticBlock(_) => Some(TsFunctionInventoryKind::ClassStaticBlock),
        _ => None,
    }
}

fn method_inventory_kind(method: &MethodDefinition<'_>) -> TsFunctionInventoryKind {
    match method.kind {
        MethodDefinitionKind::Constructor => TsFunctionInventoryKind::Constructor,
        MethodDefinitionKind::Get | MethodDefinitionKind::Set => TsFunctionInventoryKind::Accessor,
        MethodDefinitionKind::Method => TsFunctionInventoryKind::Method,
    }
}

fn function_display_name(
    nodes: &AstNodes<'_>,
    node_id: NodeId,
    kind: AstKind<'_>,
) -> Option<String> {
    match kind {
        AstKind::Function(function) => function
            .id
            .as_ref()
            .map(|id| id.name.to_string())
            .or_else(|| variable_declarator_name(nodes, node_id)),
        AstKind::ArrowFunctionExpression(_) => variable_declarator_name(nodes, node_id),
        AstKind::MethodDefinition(method) => method_name(method),
        AstKind::StaticBlock(_) => Some("static".to_string()),
        _ => None,
    }
}

fn variable_declarator_name(nodes: &AstNodes<'_>, node_id: NodeId) -> Option<String> {
    match nodes.parent_kind(node_id) {
        AstKind::VariableDeclarator(declarator) => binding_identifier_name(declarator),
        _ => None,
    }
}

fn binding_identifier_name(declarator: &VariableDeclarator<'_>) -> Option<String> {
    match &declarator.id {
        BindingPattern::BindingIdentifier(identifier) => Some(identifier.name.to_string()),
        _ => None,
    }
}

fn method_name(method: &MethodDefinition<'_>) -> Option<String> {
    match &method.key {
        PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
        PropertyKey::PrivateIdentifier(identifier) => Some(format!("#{}", identifier.name)),
        PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
        _ => None,
    }
}

fn lexical_parent_key(file: &SourceFile, nodes: &AstNodes<'_>, node_id: NodeId) -> Option<String> {
    nodes
        .ancestors_enumerated(node_id)
        .find_map(|(ancestor_id, node)| {
            let kind = node.kind();
            let inventory_kind = match kind {
                AstKind::Function(function) => match function.r#type {
                    FunctionType::FunctionDeclaration | FunctionType::TSDeclareFunction => {
                        TsFunctionInventoryKind::Declaration
                    }
                    FunctionType::FunctionExpression
                    | FunctionType::TSEmptyBodyFunctionExpression => {
                        TsFunctionInventoryKind::FunctionExpression
                    }
                },
                AstKind::ArrowFunctionExpression(_) => TsFunctionInventoryKind::Arrow,
                AstKind::MethodDefinition(method) => method_inventory_kind(method),
                AstKind::StaticBlock(_) => TsFunctionInventoryKind::ClassStaticBlock,
                _ => return None,
            };
            let display_name = function_display_name(nodes, ancestor_id, kind);
            Some(parent_key(file, kind.span(), inventory_kind, &display_name))
        })
        .or_else(|| Some(format!("ts_inventory_parent:{}:module", file.relative_path)))
}

fn parent_key(
    file: &SourceFile,
    span: oxc_span::Span,
    kind: TsFunctionInventoryKind,
    display_name: &Option<String>,
) -> String {
    stable_inventory_key(
        "ts_inventory_parent",
        &[
            ("file", file.relative_path.clone()),
            ("kind", kind.as_str().to_string()),
            ("start", span.start.to_string()),
            ("end", span.end.to_string()),
            (
                "display",
                display_name.as_deref().unwrap_or("<anonymous>").to_string(),
            ),
        ],
    )
}

fn function_stable_key(
    file: &SourceFile,
    span: &Span,
    kind: TsFunctionInventoryKind,
    lexical_parent_key: Option<&str>,
    display_name: &Option<String>,
) -> String {
    stable_inventory_key(
        "ts_inventory_function",
        &[
            ("file", file.relative_path.clone()),
            ("kind", kind.as_str().to_string()),
            ("start", span.start_byte.to_string()),
            ("end", span.end_byte.to_string()),
            (
                "parent",
                lexical_parent_key.unwrap_or("<module>").to_string(),
            ),
            (
                "display",
                display_name.as_deref().unwrap_or("<anonymous>").to_string(),
            ),
        ],
    )
}

fn stable_inventory_key(prefix: &str, parts: &[(&str, String)]) -> String {
    let mut normalized = parts
        .iter()
        .map(|(label, value)| (*label, value.replace('\\', "/")))
        .collect::<Vec<_>>();
    normalized.sort_by(|left, right| left.0.cmp(right.0));

    let mut key = length_prefixed(prefix);
    for (label, value) in normalized {
        key.push('|');
        key.push_str(&length_prefixed(label));
        key.push('=');
        key.push_str(&length_prefixed(&value));
    }
    key
}

fn length_prefixed(value: &str) -> String {
    format!("{}:{}", value.len(), value)
}

fn parse_source_type(path: &Path) -> SourceType {
    SourceType::from_path(path).unwrap_or_default()
}

fn span_from_oxc(file: crate::core::FileId, source: &str, span: oxc_span::Span) -> Span {
    span_from_byte_range(file, source, span.start as usize, span.end as usize)
}

#[cfg(test)]
mod extract_function_forms {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use crate::core::AnalysisDb;
    use crate::ts::inventory::facts::TsFunctionInventoryKind;

    use super::*;

    #[test]
    fn extracts_every_required_function_form() {
        let file = fixture_file(
            r#"
function declared() {}
const expr = function namedExpr() {};
const arrow = () => {};
class Box {
  constructor() {}
  method() {}
  get value() { return 1; }
  set value(next) {}
  static {
    declared();
  }
}
"#,
        );

        let output = extract_ts_inventory(file);
        let kinds = output
            .functions
            .iter()
            .map(|function| function.kind)
            .collect::<BTreeSet<_>>();

        for expected in [
            TsFunctionInventoryKind::Declaration,
            TsFunctionInventoryKind::FunctionExpression,
            TsFunctionInventoryKind::Arrow,
            TsFunctionInventoryKind::Method,
            TsFunctionInventoryKind::Constructor,
            TsFunctionInventoryKind::Accessor,
            TsFunctionInventoryKind::ClassStaticBlock,
        ] {
            assert!(kinds.contains(&expected), "missing {expected:?}");
        }
    }

    #[test]
    fn normalized_function_rows_sort_by_stable_key_before_dense_ids() {
        let file = fixture_file(
            r#"
const second = () => {};
function first() {}
"#,
        );
        let mut output = extract_ts_inventory(file);
        output.functions.reverse();

        let normalized = output.normalized();
        let stable_keys = normalized
            .functions
            .iter()
            .map(|function| function.stable_key.as_str())
            .collect::<Vec<_>>();
        let mut sorted_keys = stable_keys.clone();
        sorted_keys.sort();
        let dense_ids = normalized
            .functions
            .iter()
            .map(|function| function.id.0)
            .collect::<Vec<_>>();

        assert_eq!(stable_keys, sorted_keys);
        assert_eq!(dense_ids, vec![0, 1]);
    }

    #[test]
    fn stable_keys_include_file_span_parent_kind_and_display_name() {
        let file = fixture_file(
            r#"
function outer() {
  const inner = () => {};
}
"#,
        );
        let output = extract_ts_inventory(file);
        let inner = output
            .functions
            .iter()
            .find(|function| function.display_name.as_deref() == Some("inner"))
            .expect("inner arrow inventory row");

        assert!(inner.stable_key.contains("src/forms.ts"));
        assert!(inner.stable_key.contains("arrow"));
        assert!(inner.stable_key.contains("inner"));
        assert!(inner.stable_key.contains("outer"));
        assert!(inner.span.start_byte < inner.span.end_byte);
    }

    fn fixture_file(source: &str) -> &'static crate::core::SourceFile {
        let mut db = Box::new(AnalysisDb::new());
        let file_id = db.add_file(
            PathBuf::from("src/forms.ts"),
            "src/forms.ts".to_string(),
            source.to_string(),
        );
        let leaked = Box::leak(db);
        leaked.file(file_id).expect("fixture file")
    }
}
