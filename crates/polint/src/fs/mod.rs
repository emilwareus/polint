use crate::config::LoadedConfig;
use crate::core::{AnalysisDb, Language};
use crate::path_context::PathContextIndex;
use anyhow::{Context, Result};
use globset::GlobSet;
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum FsError {
    #[error("failed to strip root prefix for {path}")]
    StripPrefix { path: PathBuf },
}

pub(crate) fn discover_files(config: &LoadedConfig) -> Result<Vec<DiscoveredFile>> {
    let include = config.include_set()?;
    let exclude = config.exclude_set()?;
    let mut files = Vec::new();

    let walker = WalkBuilder::new(&config.root)
        .hidden(false)
        .ignore(config.respect_gitignore)
        .git_ignore(config.respect_gitignore)
        .git_exclude(config.respect_gitignore)
        .require_git(false)
        .parents(config.respect_gitignore)
        .build();

    for entry in walker {
        let entry = entry?;
        if !entry
            .file_type()
            .is_some_and(|file_type| file_type.is_file())
        {
            continue;
        }
        let path = entry.path();
        if Language::from_path(path) == Language::Unknown {
            continue;
        }
        let relative = relative_path(&config.root, path)?;
        if !should_include_relative_path(&include, &exclude, &relative) {
            continue;
        }
        files.push(DiscoveredFile {
            path: path.to_path_buf(),
            relative_path: relative,
        });
    }

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(files)
}

/// Fine-grained timings for [`load_analysis_files_with_timings`] (`polint::_bench::fs` / `polint-bench`).
#[allow(unreachable_pub)]
#[derive(Debug, Clone, Copy, Default)]
pub struct LoadSourcesTimings {
    /// `ignore` walk, language filter, glob include/exclude, stable sort.
    pub discover: Duration,
    /// Parallel `read_to_string` for all discovered paths.
    pub read_parallel: Duration,
    /// Sequential `AnalysisDb::add_file` (content hash + file records).
    pub fingerprint_and_push: Duration,
}

impl LoadSourcesTimings {
    /// Sum of all sub-phase durations for `polint-bench`.
    #[cfg(feature = "bench")]
    pub fn total(&self) -> Duration {
        self.discover + self.read_parallel + self.fingerprint_and_push
    }
}

pub(crate) fn load_analysis_files(config: &LoadedConfig) -> Result<AnalysisDb> {
    Ok(load_analysis_files_with_timings(config)?.0)
}

/// Same as in-module `load_analysis_files`, plus per-subphase timings (for profiling).
#[allow(unreachable_pub)]
pub fn load_analysis_files_with_timings(
    config: &LoadedConfig,
) -> Result<(AnalysisDb, LoadSourcesTimings)> {
    let mut timings = LoadSourcesTimings::default();

    let t0 = Instant::now();
    let discovered = discover_files(config)?;
    timings.discover = t0.elapsed();

    let t1 = Instant::now();
    let loaded = discovered
        .into_par_iter()
        .map(|file| {
            let source = fs::read_to_string(&file.path)
                .with_context(|| format!("failed to read {}", file.path.display()))?;
            Ok((file, source))
        })
        .collect::<Result<Vec<_>>>()?;
    timings.read_parallel = t1.elapsed();

    let t2 = Instant::now();
    let mut db = AnalysisDb::new();
    for (file, source) in loaded {
        db.add_file(file.path, file.relative_path, source);
    }
    let rel_paths: Vec<String> = db.files().iter().map(|f| f.relative_path.clone()).collect();
    let path_ix = PathContextIndex::build(&config.config.path_contexts, &rel_paths);
    if !config.config.path_contexts.pairs.is_empty() {
        db.set_path_contexts(path_ix);
    }
    timings.fingerprint_and_push = t2.elapsed();

    Ok((db, timings))
}

#[cfg(test)]
fn load_analysis_files_sequential(config: &LoadedConfig) -> Result<AnalysisDb> {
    let mut db = AnalysisDb::new();
    for file in discover_files(config)? {
        let source = fs::read_to_string(&file.path)
            .with_context(|| format!("failed to read {}", file.path.display()))?;
        db.add_file(file.path, file.relative_path, source);
    }
    Ok(db)
}

#[derive(Debug, Clone)]
pub(crate) struct DiscoveredFile {
    pub(crate) path: PathBuf,
    pub(crate) relative_path: String,
}

fn matches_any(globs: &GlobSet, relative_path: &str) -> bool {
    globs.is_match(relative_path) || globs.is_match(format!("./{relative_path}"))
}

pub(crate) fn should_include_relative_path(
    include: &GlobSet,
    exclude: &GlobSet,
    relative_path: &str,
) -> bool {
    matches_any(include, relative_path) && !matches_any(exclude, relative_path)
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).map_err(|_| FsError::StripPrefix {
        path: path.to_path_buf(),
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_config;
    use proptest::prelude::*;

    #[test]
    fn detects_language_from_path() {
        assert_eq!(Language::from_path(Path::new("a.go")), Language::Go);
        assert_eq!(Language::from_path(Path::new("a.tsx")), Language::Tsx);
    }

    #[test]
    fn discovery_is_deterministic() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join("b.go"), "package main").unwrap();
        fs::write(temp.path().join("a.go"), "package main").unwrap();
        let config = load_config(temp.path()).unwrap();
        let files = discover_files(&config).unwrap();
        assert_eq!(files[0].relative_path, "a.go");
        assert_eq!(files[1].relative_path, "b.go");
    }

    #[test]
    fn discovery_order_is_root_relative_and_stable_with_nested_files() {
        let temp = tempfile::tempdir().unwrap();
        write_file(temp.path().join("src/z.tsx"), "export const z = 1;");
        write_file(temp.path().join("cmd/main.go"), "package main\n");
        write_file(temp.path().join("src/a.ts"), "export const a = 1;");
        write_file(temp.path().join("lib/b.js"), "export const b = 1;");

        let config = load_config(temp.path()).unwrap();
        let files = discover_files(&config).unwrap();

        assert_eq!(
            relative_paths(&files),
            ["cmd/main.go", "lib/b.js", "src/a.ts", "src/z.tsx"]
        );
    }

    #[test]
    fn discovery_filters_before_sorting() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join(".gitignore"), "src/ignored.tsx\n").unwrap();
        fs::write(
            temp.path().join(".polint.toml"),
            r#"
[workspace]
include = ["src/**"]
exclude = ["src/excluded.tsx", "src/vendor/**"]
"#,
        )
        .unwrap();

        write_file(
            temp.path().join("src/nested/component.tsx"),
            "export const z = '#fff';",
        );
        write_file(
            temp.path().join("src/included.js"),
            "export const a = '#fff';",
        );
        write_file(
            temp.path().join("src/excluded.tsx"),
            "export const excluded = '#fff';",
        );
        write_file(
            temp.path().join("src/vendor/generated.ts"),
            "export const vendor = '#fff';",
        );
        write_file(
            temp.path().join("src/ignored.tsx"),
            "export const ignored = '#fff';",
        );
        write_file(temp.path().join("src/notes.txt"), "#fff\n");
        write_file(
            temp.path().join("outside.ts"),
            "export const outside = '#fff';",
        );

        let config = load_config(temp.path()).unwrap();
        let files = discover_files(&config).unwrap();

        assert_eq!(
            relative_paths(&files),
            ["src/included.js", "src/nested/component.tsx"]
        );
    }

    #[test]
    fn discovery_can_bypass_gitignore_for_explicit_internal_target_files() {
        let temp = tempfile::tempdir().unwrap();
        fs::write(temp.path().join(".gitignore"), "node_modules/\n").unwrap();
        write_file(
            temp.path().join("node_modules/pkg/index.js"),
            "module.exports = function pkg() {};",
        );
        let mut config = load_config(temp.path()).unwrap();
        config.config.workspace.include = vec!["node_modules/pkg/index.js".to_string()];
        config.config.workspace.exclude.clear();
        config.respect_gitignore = false;

        let files = discover_files(&config).unwrap();

        assert_eq!(relative_paths(&files), ["node_modules/pkg/index.js"]);
    }

    proptest! {
        #[test]
        fn discovery_include_exclude_decision_is_stable(
            dir in "[a-z]{1,8}",
            name in "[a-z]{1,8}",
            ext in "(go|ts|tsx|js|jsx)",
        ) {
            let relative_path = format!("{dir}/{name}.{ext}");
            let include = crate::config::build_glob_set(&[format!("{dir}/**")]).unwrap();
            let exclude = crate::config::build_glob_set(std::slice::from_ref(&relative_path)).unwrap();
            let empty_exclude = crate::config::build_glob_set(&[]).unwrap();

            prop_assert!(!should_include_relative_path(&include, &exclude, &relative_path));
            let first = should_include_relative_path(&include, &empty_exclude, &relative_path);
            let second = should_include_relative_path(&include, &empty_exclude, &relative_path);
            prop_assert_eq!(first, second);
            prop_assert!(first);
        }
    }

    #[test]
    fn load_analysis_files_preserves_discovery_order_in_file_ids() {
        let temp = tempfile::tempdir().unwrap();
        write_file(temp.path().join("src/z.tsx"), "export const z = 1;");
        write_file(temp.path().join("src/a.ts"), "export const a = 1;");
        write_file(temp.path().join("cmd/main.go"), "package main\n");

        let config = load_config(temp.path()).unwrap();
        let db = load_analysis_files(&config).unwrap();
        let files = db.files();

        assert_eq!(files.len(), 3);
        assert_eq!(files[0].relative_path, "cmd/main.go");
        assert_eq!(files[0].id.0, 0);
        assert_eq!(files[1].relative_path, "src/a.ts");
        assert_eq!(files[1].id.0, 1);
        assert_eq!(files[2].relative_path, "src/z.tsx");
        assert_eq!(files[2].id.0, 2);
    }

    #[test]
    fn load_analysis_files_parallel_preserves_file_ids() {
        let temp = tempfile::tempdir().unwrap();
        write_file(temp.path().join("src/z.tsx"), "export const z = 1;");
        write_file(temp.path().join("src/a.ts"), "export const a = 1;");
        write_file(temp.path().join("cmd/main.go"), "package main\n");

        let config = load_config(temp.path()).unwrap();
        let db = load_analysis_files(&config).unwrap();
        let ids = db
            .files()
            .iter()
            .map(|file| (file.relative_path.as_str(), file.id.0))
            .collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![("cmd/main.go", 0), ("src/a.ts", 1), ("src/z.tsx", 2)]
        );
    }

    #[test]
    fn load_analysis_files_parallel_matches_sequential_order() {
        let temp = tempfile::tempdir().unwrap();
        write_file(temp.path().join("b/view.tsx"), "export const b = 1;");
        write_file(temp.path().join("a/main.go"), "package main\n");
        write_file(temp.path().join("c/util.js"), "export const c = 1;");

        let config = load_config(temp.path()).unwrap();
        let parallel = load_analysis_files(&config).unwrap();
        let sequential = load_analysis_files_sequential(&config).unwrap();
        let parallel_paths = parallel
            .files()
            .iter()
            .map(|file| (file.relative_path.as_str(), file.id))
            .collect::<Vec<_>>();
        let sequential_paths = sequential
            .files()
            .iter()
            .map(|file| (file.relative_path.as_str(), file.id))
            .collect::<Vec<_>>();

        assert_eq!(parallel_paths, sequential_paths);
    }

    fn write_file(path: impl AsRef<Path>, contents: &str) {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn relative_paths(files: &[DiscoveredFile]) -> Vec<&str> {
        files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect()
    }
}
