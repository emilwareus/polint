use oxc_ast::AstKind;
use oxc_ast::ast::{CallExpression, Expression, NewExpression};
use oxc_span::{GetSpan, Span as OxcSpan};

pub(crate) fn normalized_callsite_span(source: &str, kind: AstKind<'_>) -> Option<OxcSpan> {
    match kind {
        AstKind::CallExpression(call) => Some(normalized_call_expression_span(source, call)),
        AstKind::NewExpression(expression) => {
            Some(normalized_new_expression_span(source, expression))
        }
        AstKind::TaggedTemplateExpression(expression) => Some(expression.span()),
        AstKind::ImportExpression(expression) => Some(expression.span),
        _ => None,
    }
}

pub(crate) fn normalized_call_expression_span(source: &str, call: &CallExpression<'_>) -> OxcSpan {
    let mut span = call.span;
    if let Some(start) = normalized_callee_start(&call.callee) {
        span.start = start;
    }
    if callee_is_callable_literal(&call.callee) {
        return span;
    }

    expand_single_parenthesized_expression(source, span)
}

pub(crate) fn normalized_new_expression_span(
    source: &str,
    expression: &NewExpression<'_>,
) -> OxcSpan {
    expand_single_parenthesized_expression(source, expression.span)
}

fn normalized_callee_start(expression: &Expression<'_>) -> Option<u32> {
    match expression {
        Expression::FunctionExpression(function) => return Some(function.span.start),
        Expression::ArrowFunctionExpression(function) => return Some(function.span.start),
        _ => {}
    }

    let Expression::ParenthesizedExpression(_) = expression else {
        return None;
    };

    let mut current = expression;
    let mut deepest_parenthesized_start = expression.span().start;
    loop {
        match current {
            Expression::ParenthesizedExpression(parenthesized) => {
                deepest_parenthesized_start = parenthesized.span.start;
                current = &parenthesized.expression;
            }
            Expression::TSAsExpression(expression) => current = &expression.expression,
            Expression::TSSatisfiesExpression(expression) => current = &expression.expression,
            Expression::TSNonNullExpression(expression) => current = &expression.expression,
            Expression::TSTypeAssertion(expression) => current = &expression.expression,
            Expression::TSInstantiationExpression(expression) => current = &expression.expression,
            Expression::FunctionExpression(function) => return Some(function.span.start),
            Expression::ArrowFunctionExpression(function) => return Some(function.span.start),
            _ => return Some(deepest_parenthesized_start),
        }
    }
}

fn callee_is_callable_literal(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_) => true,
        Expression::ParenthesizedExpression(expression) => {
            callee_is_callable_literal(&expression.expression)
        }
        Expression::TSAsExpression(expression) => {
            callee_is_callable_literal(&expression.expression)
        }
        Expression::TSSatisfiesExpression(expression) => {
            callee_is_callable_literal(&expression.expression)
        }
        Expression::TSNonNullExpression(expression) => {
            callee_is_callable_literal(&expression.expression)
        }
        Expression::TSTypeAssertion(expression) => {
            callee_is_callable_literal(&expression.expression)
        }
        Expression::TSInstantiationExpression(expression) => {
            callee_is_callable_literal(&expression.expression)
        }
        _ => false,
    }
}

fn expand_single_parenthesized_expression(source: &str, span: OxcSpan) -> OxcSpan {
    let bytes = source.as_bytes();
    let start = span.start as usize;
    let end = span.end as usize;
    if start >= bytes.len() || bytes.get(start) == Some(&b'(') {
        return span;
    }

    let Some(open) = previous_non_whitespace(bytes, start) else {
        return span;
    };
    if bytes[open] != b'(' {
        return span;
    }

    // Only expand into a `(` that is a grouping wrapper around this expression,
    // not the argument list of an enclosing call/index. In `g(f())` the `(`
    // preceding `f()` belongs to `g(...)`, so `f()` must keep its own span
    // rather than rendering as `(f())`. A grouping `(` is not preceded by a
    // callee token (identifier / `)` / `]`).
    if let Some(before_open) = previous_non_whitespace(bytes, open)
        && is_callee_end_byte(bytes[before_open])
    {
        return span;
    }

    let Some(close) = next_non_whitespace(bytes, end) else {
        return span;
    };
    if bytes[close] != b')' {
        return span;
    }

    OxcSpan::new(open as u32, close as u32 + 1)
}

/// Whether `byte` can end a callee expression, meaning a following `(` opens a
/// call/index argument list rather than a grouping parenthesis.
fn is_callee_end_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$' | b')' | b']')
}

fn previous_non_whitespace(bytes: &[u8], before: usize) -> Option<usize> {
    let mut index = before;
    while index > 0 {
        index -= 1;
        if !bytes[index].is_ascii_whitespace() {
            return Some(index);
        }
    }
    None
}

fn next_non_whitespace(bytes: &[u8], after: usize) -> Option<usize> {
    let mut index = after;
    while index < bytes.len() {
        if !bytes[index].is_ascii_whitespace() {
            return Some(index);
        }
        index += 1;
    }
    None
}
