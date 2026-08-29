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
//! #[polint::rule(
//!     id = "local/literal-scope",
//!     description = "Example scoped literal rule.",
//!     severity = "warn"
//! )]
//! fn run_for_each_literal(
//!     ctx: &mut RuleCtx<'_>,
//!     literals: StringLiterals<'_>,
//! ) -> RuleResult {
//!     for literal in literals.iter() {
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
use globset::{Glob, GlobMatcher};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// Process-wide memo of compiled glob matchers, keyed by pattern text.
///
/// Rules call [`file_in_scope`] once per fact row (every file, function, and
/// literal they inspect), and glob compilation is regex-construction-expensive.
/// Without the memo a metrics rule over a few thousand files recompiles the same
/// handful of config patterns hundreds of thousands of times — seconds of pure
/// overhead per `polint check`. The cache is bounded by the number of distinct
/// patterns in `.polint.toml`. Invalid patterns memoize as `None` so the
/// substring fallback stays cheap too.
///
/// Entries must be matched **borrowed**, never cloned out of the map: a
/// `GlobMatcher` clone deep-copies the inner `Glob`'s owned pattern strings, and
/// at one clone per (rule, fact row) that copying costs far more than the match
/// itself — enough to undo most of what the memo saves.
fn matcher_cache() -> &'static RwLock<HashMap<String, Option<GlobMatcher>>> {
    static CACHE: OnceLock<RwLock<HashMap<String, Option<GlobMatcher>>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

thread_local! {
    /// Scratch buffer for the `./`-prefixed match candidate, reused across calls
    /// so the fallback attempt does not allocate once per non-matching row.
    static DOT_SLASH_CANDIDATE: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Match `value` against a single glob `pattern`.
///
/// Falls back to substring matching if the pattern is not a valid glob; this preserves
/// permissive `.polint.toml` semantics where users sometimes write plain prefixes.
/// The `./` prefix variant is also tried so configs that include or omit a leading
/// `./` behave identically.
#[must_use]
pub fn glob_matches(pattern: &str, value: &str) -> bool {
    let cache = matcher_cache();
    if let Ok(read) = cache.read()
        && let Some(cached) = read.get(pattern)
    {
        return matches_memoized(cached.as_ref(), pattern, value);
    }
    let compiled = Glob::new(pattern).ok().map(|glob| glob.compile_matcher());
    let Ok(mut write) = cache.write() else {
        return matches_memoized(compiled.as_ref(), pattern, value);
    };
    let cached = write.entry(pattern.to_string()).or_insert(compiled);
    matches_memoized(cached.as_ref(), pattern, value)
}

/// Decide a single match from a memo entry: `Some` is a compiled matcher tried
/// against `value` and its `./`-prefixed variant, `None` is a pattern that failed
/// to compile and falls back to a substring test.
fn matches_memoized(matcher: Option<&GlobMatcher>, pattern: &str, value: &str) -> bool {
    match matcher {
        Some(matcher) => {
            matcher.is_match(value)
                || DOT_SLASH_CANDIDATE.with_borrow_mut(|candidate| {
                    candidate.clear();
                    candidate.push_str("./");
                    candidate.push_str(value);
                    matcher.is_match(candidate.as_str())
                })
        }
        None => value.contains(pattern.trim_matches('*')),
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
    use proptest::prelude::*;

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

    /// Compile, try the value, try an allocated `./` variant, else substring —
    /// the formulation [`glob_matches`] replaced, kept as an oracle so the memo
    /// and the borrowed match path cannot drift from the documented semantics.
    fn reference_glob_matches(pattern: &str, value: &str) -> bool {
        match Glob::new(pattern).ok().map(|glob| glob.compile_matcher()) {
            Some(matcher) => matcher.is_match(value) || matcher.is_match(format!("./{value}")),
            None => value.contains(pattern.trim_matches('*')),
        }
    }

    fn repo_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("canonical repo root")
    }

    fn shipped_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).expect("readable example directory") {
            let path = entry.expect("directory entry").path();
            if path.is_dir() {
                shipped_files(&path, out);
            } else {
                out.push(path);
            }
        }
    }

    /// Every `files` / `allow_files` pattern shipped in `examples/**/.polint.toml`,
    /// plus patterns that fail to compile so the substring fallback is covered.
    fn scope_patterns() -> Vec<String> {
        let examples = repo_root().join("examples");
        let mut paths = Vec::new();
        shipped_files(&examples, &mut paths);
        let mut patterns = vec![
            "[unclosed".to_string(),
            "src/[a-".to_string(),
            "{unbalanced".to_string(),
            "**".to_string(),
            "*".to_string(),
            String::new(),
        ];
        for path in paths
            .iter()
            .filter(|path| path.file_name().is_some_and(|name| name == ".polint.toml"))
        {
            let config = std::fs::read_to_string(path).expect("readable example config");
            for line in config.lines().map(str::trim) {
                let Some((key, list)) = line.split_once('=') else {
                    continue;
                };
                if !matches!(key.trim(), "files" | "allow_files") {
                    continue;
                }
                patterns.extend(
                    list.split('"')
                        .skip(1)
                        .step_by(2)
                        .map(std::string::ToString::to_string),
                );
            }
        }
        patterns.sort();
        patterns.dedup();
        assert!(
            patterns.len() > 6,
            "no scope patterns were read from examples/**/.polint.toml"
        );
        patterns
    }

    /// Every path shipped under `examples/`, relative to its example root — the
    /// shape rules actually pass in — in both bare and `./`-prefixed form.
    fn corpus_paths() -> Vec<String> {
        let examples = repo_root().join("examples");
        let mut paths = Vec::new();
        shipped_files(&examples, &mut paths);
        let mut values = paths
            .iter()
            .filter_map(|path| path.strip_prefix(&examples).ok())
            .filter_map(|relative| {
                relative
                    .iter()
                    .skip(1)
                    .collect::<std::path::PathBuf>()
                    .to_str()
                    .map(str::to_string)
            })
            .filter(|relative| !relative.is_empty())
            .flat_map(|relative| [format!("./{relative}"), relative])
            .collect::<Vec<_>>();
        values.sort();
        values.dedup();
        assert!(
            !values.is_empty(),
            "no corpus paths were read from examples/"
        );
        values
    }

    /// Every shipped scope pattern against every shipped corpus path. The space is
    /// small enough to sweep exhaustively, which the sampled property below cannot
    /// promise.
    #[test]
    fn every_shipped_pattern_matches_every_corpus_path_as_the_reference_does() {
        let patterns = scope_patterns();
        let values = corpus_paths();
        let mut matched = 0;
        for pattern in &patterns {
            for value in &values {
                let actual = glob_matches(pattern, value);
                assert_eq!(
                    actual,
                    reference_glob_matches(pattern, value),
                    "pattern {pattern:?} against {value:?}"
                );
                matched += usize::from(actual);
            }
        }
        assert!(
            matched > 0,
            "the sweep matched nothing, so it proves nothing about the match path"
        );
    }

    proptest! {
        #[test]
        fn borrowed_matching_agrees_with_the_reference_semantics(
            (pattern, value) in (proptest::sample::select(scope_patterns()), proptest::sample::select(corpus_paths())),
        ) {
            prop_assert_eq!(
                glob_matches(&pattern, &value),
                reference_glob_matches(&pattern, &value),
                "pattern {:?} against {:?}",
                pattern,
                value
            );
        }
    }
}
