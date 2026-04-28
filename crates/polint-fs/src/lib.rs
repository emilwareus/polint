use anyhow::{Context, Result};
use globset::GlobSet;
use ignore::WalkBuilder;
use polint_config::LoadedConfig;
use polint_core::{AnalysisDb, Language, SourceFile};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FsError {
    #[error("failed to strip root prefix for {path}")]
    StripPrefix { path: PathBuf },
}

pub fn discover_files(config: &LoadedConfig) -> Result<Vec<DiscoveredFile>> {
    let include = config.include_set()?;
    let exclude = config.exclude_set()?;
    let mut files = Vec::new();

    let walker = WalkBuilder::new(&config.root)
        .hidden(false)
        .git_ignore(true)
        .git_exclude(true)
        .require_git(false)
        .parents(true)
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
        let language = Language::from_path(path);
        if language == Language::Unknown {
            continue;
        }
        let relative = relative_path(&config.root, path)?;
        if !matches_any(&include, &relative) || matches_any(&exclude, &relative) {
            continue;
        }
        files.push(DiscoveredFile {
            path: path.to_path_buf(),
            relative_path: relative,
            language,
        });
    }

    files.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(files)
}

pub fn load_analysis_files(config: &LoadedConfig) -> Result<AnalysisDb> {
    let mut db = AnalysisDb::new();
    for file in discover_files(config)? {
        let source = fs::read_to_string(&file.path)
            .with_context(|| format!("failed to read {}", file.path.display()))?;
        db.add_file(file.path, file.relative_path, source);
    }
    Ok(db)
}

pub fn source_files_from_db(db: &AnalysisDb) -> Vec<&SourceFile> {
    db.files().iter().collect()
}

#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: PathBuf,
    pub relative_path: String,
    pub language: Language,
}

fn matches_any(globs: &GlobSet, relative_path: &str) -> bool {
    globs.is_match(relative_path) || globs.is_match(format!("./{relative_path}"))
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
    use polint_config::load_config;

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
}
