//! File-scoping helpers shared by rule packs.
//!
//! `RuleOptions` exposes three fields that decide whether a rule should look at a given
//! repo-relative path: `files` (allowlist globs), `allow_files` (skip globs), and
//! `allow` (exact-path skip list). Every example rule used to copy/paste this 15-line
//! helper, drifting on whether `allow` was honored. [`file_in_scope`] is the canonical
//! implementation that all path-scoping rules should call.
//!
//! Rules that use `RuleOptions::allow` for *literal-value* allowlists (e.g.
//! `examples/config-denied-literal`) must **not** call [`file_in_scope`] for that
//! purpose — `allow` would then incorrectly match against file paths.
//!
//! ```no_run
//! use polint::sdk::prelude::*;
//!
//! fn run_for_each_literal(ctx: &mut RuleCtx<'_>) -> Result<()> {
//!     for literal in ctx.string_literals() {
//!         let file = ctx.file_path(literal.file);
//!         if !file_in_scope(ctx.options(), &file) {
//!             continue;
//!         }
//!         // ...inspect literal...
//!     }
//!     Ok(())
//! }
//! ```

use crate::core::RuleOptions;
use globset::Glob;

/// Match `value` against a single glob `pattern`.
///
/// Falls back to substring matching if the pattern is not a valid glob; this preserves
/// permissive `.polint.toml` semantics where users sometimes write plain prefixes.
/// The `./` prefix variant is also tried so configs that include or omit a leading
/// `./` behave identically.
#[must_use]
pub fn glob_matches(pattern: &str, value: &str) -> bool {
    match Glob::new(pattern) {
        Ok(glob) => {
            let matcher = glob.compile_matcher();
            matcher.is_match(value) || matcher.is_match(format!("./{value}"))
        }
        Err(_) => value.contains(pattern.trim_matches('*')),
    }
}

/// Return `true` when `file` is in the rule's path scope per [`RuleOptions`].
///
/// Three clauses, all must hold:
/// 1. `files` is empty **or** at least one entry matches `file` (glob).
/// 2. `allow_files` does **not** match `file` (glob).
/// 3. `allow` does **not** contain `file` exactly (string equality).
///
/// Use this for path-based scoping. Rules that use `allow` for *non-path*
/// allowlists (e.g. literal-value rules) should call [`file_matches_globs`]
/// instead so a path entry never accidentally allowlists a file.
#[must_use]
pub fn file_in_scope(options: &RuleOptions, file: &str) -> bool {
    file_matches_globs(options, file) && !options.allow.iter().any(|allowed| allowed == file)
}

/// Return `true` when `file` matches the rule's `files` / `allow_files` globs.
///
/// Like [`file_in_scope`] but does **not** consult `RuleOptions::allow` — use
/// this from rules that interpret `allow` as something other than file paths
/// (literal allowlists, ID allowlists, etc.).
#[must_use]
pub fn file_matches_globs(options: &RuleOptions, file: &str) -> bool {
    (options.files.is_empty()
        || options
            .files
            .iter()
            .any(|pattern| glob_matches(pattern, file)))
        && !options
            .allow_files
            .iter()
            .any(|pattern| glob_matches(pattern, file))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(files: &[&str], allow_files: &[&str], allow: &[&str]) -> RuleOptions {
        RuleOptions {
            files: files.iter().map(|s| (*s).to_string()).collect(),
            allow_files: allow_files.iter().map(|s| (*s).to_string()).collect(),
            allow: allow.iter().map(|s| (*s).to_string()).collect(),
            ..RuleOptions::default()
        }
    }

    #[test]
    fn empty_files_admits_everything() {
        assert!(file_in_scope(&opts(&[], &[], &[]), "src/a.ts"));
    }

    #[test]
    fn files_glob_filters_in() {
        assert!(file_in_scope(&opts(&["src/**/*.ts"], &[], &[]), "src/a.ts"));
        assert!(!file_in_scope(
            &opts(&["src/**/*.ts"], &[], &[]),
            "vendor/a.ts"
        ));
    }

    #[test]
    fn allow_files_glob_filters_out() {
        assert!(!file_in_scope(
            &opts(&[], &["**/generated/**"], &[]),
            "src/generated/x.ts"
        ));
        assert!(file_in_scope(
            &opts(&[], &["**/generated/**"], &[]),
            "src/normal/x.ts"
        ));
    }

    #[test]
    fn allow_exact_path_filters_out() {
        assert!(!file_in_scope(
            &opts(&[], &[], &["src/skip.go"]),
            "src/skip.go"
        ));
        assert!(file_in_scope(
            &opts(&[], &[], &["src/skip.go"]),
            "src/keep.go"
        ));
    }

    #[test]
    fn file_matches_globs_ignores_allow_list() {
        let options = opts(&["src/**"], &[], &["src/some.go"]);
        assert!(file_matches_globs(&options, "src/some.go"));
        assert!(!file_in_scope(&options, "src/some.go"));
    }

    #[test]
    fn dot_prefix_in_pattern_tolerated() {
        // Pattern with leading `./`, value without (common when configs use `./src/**`).
        assert!(glob_matches("./src/**/*.ts", "src/a.ts"));
        // Plain match still works.
        assert!(glob_matches("src/**/*.ts", "src/a.ts"));
    }

    #[test]
    fn invalid_glob_falls_back_to_substring() {
        assert!(glob_matches("[unclosed", "x[unclosedy"));
    }
}
