//! Parser identity labels shared by the language frontends and the analysis kernel.
//!
//! Facts are only interchangeable when the parser that produced them is the same
//! parser that would produce them now. These labels are what "the same parser"
//! means to a cache key, a provider contract, and the semantic store, so they must
//! track the resolved dependency versions exactly: a stale label lets a parser or
//! grammar change reuse facts produced by the previous one, at a key that claims
//! they are current.
//!
//! `parser_identity_labels_track_the_resolved_dependency_versions` enforces that
//! against the workspace lockfile.

/// Backend that parses Go sources.
///
/// Also pinned by a `CHECK` constraint in the semantic-store schema, so bumping
/// `tree-sitter` needs this constant *and* a new store migration.
pub const GO_PARSER_BACKEND: &str = "tree-sitter-0.26.8";

/// Grammar the Go backend is driven with.
///
/// Pinned by the same semantic-store `CHECK` constraint as [`GO_PARSER_BACKEND`].
pub const GO_PARSER_GRAMMAR: &str = "tree-sitter-go-0.25.0";

/// Backend that parses TypeScript and JavaScript sources.
pub const TS_PARSER_BACKEND: &str = "oxc-0.129.0";

/// Every parser label this engine links, joined.
///
/// Cached artifacts derived from more than one language's facts are invalidated
/// by a change to any of those parsers, so they key on all of them at once.
#[must_use]
pub fn engine_parser_identity() -> String {
    format!("{GO_PARSER_BACKEND}+{GO_PARSER_GRAMMAR}+{TS_PARSER_BACKEND}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// The labels are hand-written strings. If they drift from the resolved
    /// dependency versions, a parser or grammar bump silently keeps the same
    /// provider identity and cache keys, so facts produced by the previous parser
    /// are reused as if current.
    ///
    /// This guards *this* workspace. A downstream host that resolves a different
    /// patch version under a caret requirement is not detected, because the label
    /// is compiled in from here.
    #[test]
    fn parser_identity_labels_track_the_resolved_dependency_versions() {
        let lock = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(Path::parent)
                .expect("workspace root")
                .join("Cargo.lock"),
        )
        .expect("Cargo.lock is readable");
        let resolved = |crate_name: &str| {
            lock.split("[[package]]")
                .find_map(|block| {
                    let mut lines = block.lines().filter_map(|line| line.split_once(" = "));
                    let name = lines
                        .clone()
                        .find(|(key, _)| *key == "name")?
                        .1
                        .trim_matches('"');
                    (name == crate_name).then(|| {
                        lines
                            .find(|(key, _)| *key == "version")
                            .expect("locked package has a version")
                            .1
                            .trim_matches('"')
                            .to_string()
                    })
                })
                .unwrap_or_else(|| panic!("{crate_name} is not in Cargo.lock"))
        };
        for (label, crate_name, prefix, constant) in [
            (
                "Go backend",
                "tree-sitter",
                "tree-sitter",
                GO_PARSER_BACKEND,
            ),
            (
                "Go grammar",
                "tree-sitter-go",
                "tree-sitter-go",
                GO_PARSER_GRAMMAR,
            ),
            ("TypeScript backend", "oxc_parser", "oxc", TS_PARSER_BACKEND),
        ] {
            assert_eq!(
                constant,
                format!("{prefix}-{}", resolved(crate_name)),
                "the {label} label drifted from the locked {crate_name} version; update the \
                 constant, and for a Go label also add a semantic-store migration for the new \
                 CHECK value"
            );
        }
    }
}
