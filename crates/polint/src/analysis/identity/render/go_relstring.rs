//! Go `RelString` renderer (D-05, D-06, D-07).
//!
//! Projects an [`IdentityRecord`] into the `golang.org/x/tools/go/callgraph`
//! `RelString` shape:
//!
//! - package function: `module/path/pkg.Foo`
//! - pointer-receiver method: `(*module/path/pkg.Receiver).Method`
//! - value-receiver method: `(module/path/pkg.Receiver).Method`
//! - generic instantiation: `Func[T0,T1]` with normalized type-parameter names
//! - anonymous function: `package.parent$N` with a deterministic 1-based ordinal
//!
//! The renderer is a pure function of the [`IdentityRecord`] — it never takes
//! the analysis database, an input snapshot, or any kernel handle (D-06). Paths
//! are slash-joined manually rather than through `Path::display()` so the output
//! is byte-identical across platforms (D-25).
//!
//! The receiver/generic/anonymous shape is encoded in `identity.container_path`
//! following the `pkg.Type.Method` convention from D-02:
//!
//! - a `$` segment marks an anonymous function (`parent$N`);
//! - a leading `*` on the dotted-tail receiver marks a pointer receiver;
//! - a two-or-more dotted tail (`Receiver.Method`) marks a method;
//! - square brackets on the display name mark a generic instantiation.

use crate::analysis::identity::facts::{IdentityKind, IdentityRecord};

/// Renders the Go `RelString`-format name for an identity record (D-07).
pub(crate) fn render(identity: &IdentityRecord) -> String {
    let package = identity.package_or_module.as_ref();
    let container = identity.container_path.as_ref();
    let display = identity.display_name.as_ref();

    // Anonymous functions render as `package.parent$N` regardless of kind.
    if let Some((parent, ordinal)) = anonymous_parent_ordinal(container) {
        return format!("{package}.{parent}${ordinal}");
    }

    // Method shapes (`Receiver.Method` / `*Receiver.Method`) render with the
    // receiver-parenthesized RelString form. We detect a method by the presence
    // of a dotted tail in the container that names a receiver type plus method.
    if let Some(method) = method_parts(container) {
        let receiver_open = if method.pointer { "(*" } else { "(" };
        return format!(
            "{receiver_open}{package}.{}).{}",
            method.receiver, method.method
        );
    }

    // Generic instantiations carry their type-argument list on the display name
    // (`Func[T0,T1]`); normalize the type-parameter names (no whitespace,
    // deterministic order matching Go's RelString — type arguments are kept in
    // source order, only whitespace is stripped).
    if let Some((base, type_args)) = generic_instantiation(display) {
        let normalized = type_args
            .iter()
            .map(|arg| arg.trim())
            .collect::<Vec<_>>()
            .join(",");
        return format!("{package}.{base}[{normalized}]");
    }

    // Plain package function (or any non-method callsite container): the
    // package path slash-joined to the simple display name.
    let _ = identity.kind == IdentityKind::Function; // kind is informational here.
    format!("{package}.{display}")
}

struct MethodParts<'a> {
    pointer: bool,
    receiver: &'a str,
    method: &'a str,
}

/// Extracts `(pointer, receiver, method)` from a method-shaped container path.
///
/// A method container is the dotted form `Receiver.Method` (optionally prefixed
/// with `*` to mark a pointer receiver). A bare single segment (`Foo`) is a
/// package function, not a method, so it returns `None`.
fn method_parts(container: &str) -> Option<MethodParts<'_>> {
    let (pointer, body) = match container.strip_prefix('*') {
        Some(rest) => (true, rest),
        None => (false, container),
    };
    // The method name is the final dotted segment; everything before it is the
    // receiver type. A method must have at least one `.` separating the two.
    let (receiver, method) = body.rsplit_once('.')?;
    if receiver.is_empty() || method.is_empty() {
        return None;
    }
    // A receiver that itself contains a `$` (anonymous) is handled earlier.
    Some(MethodParts {
        pointer,
        receiver,
        method,
    })
}

/// Returns `(parent, ordinal)` for an anonymous-function container of the form
/// `parent$N`, where `N` is the deterministic 1-based ordinal within the parent
/// (mirroring `analysis::calls::extract::same_span_ordinal`, but rendered
/// 1-based for Go's `parent$1` convention).
fn anonymous_parent_ordinal(container: &str) -> Option<(&str, u32)> {
    let (parent, ordinal) = container.rsplit_once('$')?;
    if parent.is_empty() {
        return None;
    }
    let ordinal = ordinal.parse::<u32>().ok()?;
    Some((parent, ordinal))
}

/// Returns `(base, type_args)` when the display name is a generic instantiation
/// `Base[Arg0,Arg1,...]`.
fn generic_instantiation(display: &str) -> Option<(&str, Vec<&str>)> {
    let open = display.find('[')?;
    if !display.ends_with(']') {
        return None;
    }
    let base = &display[..open];
    if base.is_empty() {
        return None;
    }
    let inner = &display[open + 1..display.len() - 1];
    if inner.is_empty() {
        return None;
    }
    Some((base, inner.split(',').collect()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::analysis::identity::facts::{
        IdentityRecord, IdentityRecordId, LanguageTag, compute_identity_stable_key,
        compute_signature_digest,
    };
    use crate::core::{FileId, Span};

    fn record(kind: IdentityKind, package: &str, container: &str, display: &str) -> IdentityRecord {
        let language = LanguageTag::Go;
        let span = Span::point(FileId(1), 1, 1);
        IdentityRecord {
            id: IdentityRecordId(0),
            kind,
            file_id: FileId(1),
            span: span.clone(),
            language,
            package_or_module: Arc::from(package),
            container_path: Arc::from(container),
            display_name: Arc::from(display),
            signature_digest: compute_signature_digest(
                language, package, container, display, None, None,
            ),
            multiplicity: 1,
            stable_key: compute_identity_stable_key(
                kind,
                language,
                package,
                container,
                FileId(1),
                &span,
            ),
            originating_call_site_id: None,
            originating_call_target_id: None,
        }
    }

    #[test]
    fn package_function_format() {
        let rendered = render(&record(
            IdentityKind::Function,
            "module/path/pkg",
            "Foo",
            "Foo",
        ));
        assert_eq!(rendered, "module/path/pkg.Foo");
    }

    #[test]
    fn pointer_receiver_method_format() {
        let rendered = render(&record(
            IdentityKind::Function,
            "module/path/pkg",
            "*Receiver.Method",
            "Method",
        ));
        assert_eq!(rendered, "(*module/path/pkg.Receiver).Method");
    }

    #[test]
    fn value_receiver_method_format() {
        let rendered = render(&record(
            IdentityKind::Function,
            "module/path/pkg",
            "Receiver.Method",
            "Method",
        ));
        assert_eq!(rendered, "(module/path/pkg.Receiver).Method");
    }

    #[test]
    fn generic_instantiation_format() {
        let rendered = render(&record(
            IdentityKind::Function,
            "module/path/pkg",
            "Func",
            "Func[T0, T1]",
        ));
        assert_eq!(rendered, "module/path/pkg.Func[T0,T1]");
    }

    #[test]
    fn anonymous_function_parent_dollar_ordinal() {
        let first = render(&record(
            IdentityKind::Function,
            "module/path/pkg",
            "parent$1",
            "parent$1",
        ));
        let second = render(&record(
            IdentityKind::Function,
            "module/path/pkg",
            "parent$2",
            "parent$2",
        ));
        assert_eq!(first, "module/path/pkg.parent$1");
        assert_eq!(second, "module/path/pkg.parent$2");
    }

    #[test]
    fn render_takes_only_identity_record() {
        // Signature lock (D-06): the renderer is a pure function of an
        // IdentityRecord. This compiles only while the contract holds.
        let render_fn: fn(&IdentityRecord) -> String = render;
        let _ = render_fn;
    }

    #[test]
    fn render_is_byte_identical_for_identical_records() {
        let left = render(&record(
            IdentityKind::Function,
            "module/path/pkg",
            "*Receiver.Method",
            "Method",
        ));
        let right = render(&record(
            IdentityKind::Function,
            "module/path/pkg",
            "*Receiver.Method",
            "Method",
        ));
        assert_eq!(left, right);
    }
}
