use globset::{Glob, GlobSet, GlobSetBuilder};
use polint_sdk::prelude::RuleOptions;

pub fn file_in_scope(options: &RuleOptions, file: &str) -> bool {
    (options.files.is_empty()
        || options
            .files
            .iter()
            .any(|pattern| glob_matches(pattern, file)))
        && !options
            .allow_files
            .iter()
            .any(|pattern| glob_matches(pattern, file))
        && !options.allow.iter().any(|allowed| allowed == file)
}

pub fn glob_matches(pattern: &str, value: &str) -> bool {
    build_one(pattern)
        .map(|glob| glob.is_match(value) || glob.is_match(format!("./{value}")))
        .unwrap_or_else(|| value.contains(pattern.trim_matches('*')))
}

fn build_one(pattern: &str) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    builder.add(Glob::new(pattern).ok()?);
    builder.build().ok()
}
