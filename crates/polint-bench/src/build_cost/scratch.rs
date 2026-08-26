//! Scratch repositories, directory accounting, and the edits scenarios make.
//!
//! A scenario has to delete compiler output and change rule and source files,
//! and the working tree must stay clean, so no cell runs against a checked-in
//! example in place. Each cell materializes its own copy under the Cargo target
//! directory with the rule pack rewritten to the standalone shape `polint init`
//! scaffolds for a consumer, so the pack is its own Cargo workspace instead of a
//! member of this repository's.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow, bail};

/// Language features a scaffolded pack requests, mirroring what the CLI writes.
const PACK_FEATURES: [&str; 2] = ["lang-go", "lang-typescript"];
/// Extensions polint analyzes today, used to pick the file a source edit touches
/// and the fixtures a synthesized case carries.
const SOURCE_EXTENSIONS: [&str; 7] = ["go", "ts", "tsx", "js", "jsx", "mts", "cts"];

/// A scratch copy of a repository under test.
#[derive(Clone, Debug)]
pub struct ScratchRepo {
    pub root: PathBuf,
    /// Rule pack directories, repo-relative, in `.polint.toml` order.
    pub rule_packs: Vec<PathBuf>,
    /// Rule ids declared in `.polint.toml`, used to synthesize fixture cases.
    pub rule_ids: Vec<String>,
}

/// Byte and file accounting for one directory, plus how much of it a run wrote.
/// `bytes_written` counts files modified at or after the mark the run started
/// with, which attributes work even when the run also deleted files.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirUsage {
    pub bytes: u64,
    pub files: u64,
    pub bytes_written: u64,
}

/// Walk `dir` without following symlinks. A missing directory is zero, the
/// honest reading for a cache that has not been created yet.
///
/// Cargo hard-links a built binary into both `deps/` and the profile root, so
/// content reached through more than one directory entry is counted once. That
/// matches what the bytes cost to keep, and what archiving the directory for a
/// CI cache would store.
pub fn dir_usage(dir: &Path, since: Option<SystemTime>) -> Result<DirUsage> {
    let mut usage = DirUsage::default();
    let mut seen = BTreeSet::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("failed to read {}", current.display()));
            }
        };
        for entry in entries {
            let path = entry?.path();
            let metadata = fs::symlink_metadata(&path)
                .with_context(|| format!("failed to stat {}", path.display()))?;
            if metadata.is_dir() {
                stack.push(path);
                continue;
            }
            if !metadata.is_file() || !first_link(&metadata, &mut seen) {
                continue;
            }
            usage.bytes += metadata.len();
            usage.files += 1;
            if let Some(mark) = since
                && metadata.modified().is_ok_and(|modified| modified >= mark)
            {
                usage.bytes_written += metadata.len();
            }
        }
    }
    Ok(usage)
}

/// Whether this is the first directory entry seen for the file's content.
/// Platforms without stable file identity count every entry.
#[cfg(unix)]
fn first_link(metadata: &fs::Metadata, seen: &mut BTreeSet<(u64, u64)>) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    metadata.nlink() < 2 || seen.insert((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn first_link(_metadata: &fs::Metadata, _seen: &mut BTreeSet<(u64, u64)>) -> bool {
    true
}

/// Copy `source` into a fresh `destination` and rewrite its rule packs so the
/// copy builds outside this repository's Cargo workspace.
pub fn materialize(
    source: &Path,
    destination: &Path,
    polint_crate_dir: &Path,
    workspace_lockfile: Option<&Path>,
) -> Result<ScratchRepo> {
    if destination.exists() {
        fs::remove_dir_all(destination)
            .with_context(|| format!("failed to clear {}", destination.display()))?;
    }
    fs::create_dir_all(destination)
        .with_context(|| format!("failed to create {}", destination.display()))?;
    copy_tree(source, destination)?;
    // A local run may have left an analysis cache in the example; the harness
    // controls cache state per scenario and must not inherit one.
    let stale = destination.join(".polint/cache");
    if stale.exists() {
        fs::remove_dir_all(&stale)
            .with_context(|| format!("failed to clear {}", stale.display()))?;
    }

    let config = read_config(destination)?;
    let mut rule_packs = Vec::new();
    for pack_rel in rule_paths(&config) {
        let pack_dir = destination.join(&pack_rel);
        let manifest_path = pack_dir.join("Cargo.toml");
        if !manifest_path.is_file() {
            continue;
        }
        let original = fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;
        fs::write(
            &manifest_path,
            standalone_pack_manifest(&original, polint_crate_dir)?,
        )
        .with_context(|| format!("failed to write {}", manifest_path.display()))?;
        if let Some(lockfile) = workspace_lockfile {
            // Seeding this repository's lockfile pins the pack's dependency
            // versions to the ones already resolved here, so a cold cell
            // measures a fixed closure rather than whatever crates.io published
            // most recently.
            fs::copy(lockfile, pack_dir.join("Cargo.lock"))
                .with_context(|| format!("failed to seed lockfile into {}", pack_dir.display()))?;
        }
        rule_packs.push(pack_rel);
    }
    if rule_packs.is_empty() {
        bail!(
            "{} has no repo-local rule pack; build cost is only defined for repos with one",
            source.display()
        );
    }
    Ok(ScratchRepo {
        root: destination.to_path_buf(),
        rule_packs,
        rule_ids: rule_ids(&config),
    })
}

/// Rewrite a workspace-member pack manifest into the standalone shape a consumer
/// repository gets, keeping the package name and pointing `polint` at this
/// checkout.
pub fn standalone_pack_manifest(original: &str, polint_crate_dir: &Path) -> Result<String> {
    let parsed: toml::Value = toml::from_str(original).context("failed to parse pack manifest")?;
    let name = parsed
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .ok_or_else(|| anyhow!("pack manifest has no [package] name"))?;
    let dependencies: BTreeSet<&str> = parsed
        .get("dependencies")
        .and_then(toml::Value::as_table)
        .map(|table| table.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if dependencies != BTreeSet::from(["polint"]) {
        bail!("build cost measures packs whose sole dependency is polint; found {dependencies:?}");
    }
    let features = PACK_FEATURES
        .iter()
        .map(|feature| format!("\"{feature}\""))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        r#"[package]
name = "{name}"
version = "{version}"
edition = "2024"
publish = false

[dependencies]
polint = {{ path = "{path}", default-features = false, features = [{features}] }}

[workspace]
"#,
        version = env!("CARGO_PKG_VERSION"),
        path = polint_crate_dir.display().to_string().replace('\\', "/"),
    ))
}

/// Append a distinguishing line to a rule source so the next check recompiles
/// the pack. Prefers a rule module over `main.rs` so the edit looks like rule
/// authoring.
pub fn edit_rule_source(repo: &ScratchRepo, marker: u32) -> Result<()> {
    let src = repo.root.join(&repo.rule_packs[0]).join("src");
    let mut candidates: Vec<PathBuf> = read_sorted(&src)?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|ext| ext == "rs"))
        .collect();
    candidates.sort_by_key(|path| path.file_name().is_some_and(|name| name == "main.rs"));
    let target = candidates
        .first()
        .ok_or_else(|| anyhow!("rule pack {} has no Rust sources", src.display()))?;
    append(target, &format!("\n// build-cost harness edit {marker}\n"))
}

/// Append a newline to the first analyzed source so its content hash changes
/// without changing what any rule reports.
pub fn edit_scanned_source(repo: &ScratchRepo) -> Result<()> {
    let target = scanned_sources(&repo.root)?
        .into_iter()
        .next()
        .ok_or_else(|| anyhow!("{} has no analyzable source", repo.root.display()))?;
    append(&target, "\n")
}

/// Fixture cases for the `test-suite` scenario: the repository's own when it has
/// them, otherwise `per_rule` generated cases per declared rule id. Returns the
/// case count and whether it was generated.
///
/// Cases are generated because no example repository ships `.polint/tests`, and
/// the cost `polint test` pays today scales with the case count. A generated
/// case asserts nothing, so its pass/fail tally carries no signal.
pub fn test_cases(repo: &ScratchRepo, per_rule: u32) -> Result<(u32, bool)> {
    let tests_root = repo.root.join(".polint/tests/rules");
    let existing = count_manifests(&tests_root)?;
    if existing > 0 {
        return Ok((existing, false));
    }
    if repo.rule_ids.is_empty() {
        bail!(
            "{} declares no rule ids in .polint.toml, so fixture cases cannot be generated",
            repo.root.display()
        );
    }
    let sources = scanned_sources(&repo.root)?;
    let mut written = 0;
    for rule_id in &repo.rule_ids {
        for index in 0..per_rule {
            let case_dir = tests_root
                .join(rule_id.replace(['/', '\\'], "-"))
                .join(format!("case-{index}"));
            fs::create_dir_all(&case_dir)
                .with_context(|| format!("failed to create {}", case_dir.display()))?;
            fs::write(
                case_dir.join("polint-test.toml"),
                format!("rule = \"{rule_id}\"\n"),
            )?;
            for source in &sources {
                let name = source
                    .file_name()
                    .ok_or_else(|| anyhow!("source {} has no file name", source.display()))?;
                fs::copy(source, case_dir.join(name))?;
            }
            written += 1;
        }
    }
    Ok((written, true))
}

fn count_manifests(root: &Path) -> Result<u32> {
    let mut count = 0;
    let mut stack = vec![root.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .file_name()
                .is_some_and(|name| name == "polint-test.toml")
            {
                count += 1;
            }
        }
    }
    Ok(count)
}

fn append(path: &Path, text: &str) -> Result<()> {
    use std::io::Write as _;
    fs::OpenOptions::new()
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open {}", path.display()))?
        .write_all(text.as_bytes())
        .with_context(|| format!("failed to append to {}", path.display()))
}

/// Entries of `dir`, sorted so every run picks the same file.
fn read_sorted(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("failed to read {}", dir.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<_>>()?;
    found.sort();
    Ok(found)
}

/// Analyzable sources at the repository root.
fn scanned_sources(root: &Path) -> Result<Vec<PathBuf>> {
    Ok(read_sorted(root)?
        .into_iter()
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| SOURCE_EXTENSIONS.contains(&ext))
        })
        .collect())
}

fn read_config(root: &Path) -> Result<toml::Value> {
    let path = root.join(".polint.toml");
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))
}

fn rule_paths(config: &toml::Value) -> Vec<PathBuf> {
    config
        .get("rules")
        .and_then(|rules| rules.get("paths"))
        .and_then(toml::Value::as_array)
        .map(|paths| {
            paths
                .iter()
                .filter_map(toml::Value::as_str)
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        })
        .filter(|paths| !paths.is_empty())
        // Mirrors the CLI default when `[rules] paths` is absent.
        .unwrap_or_else(|| vec![PathBuf::from(".polint/rules")])
}

fn rule_ids(config: &toml::Value) -> Vec<String> {
    config
        .get("rules")
        .and_then(|rules| rules.get("config"))
        .and_then(toml::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("id").and_then(toml::Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Copy a repository tree, skipping version control and compiler output.
fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        if matches!(name.to_str(), Some(".git" | "target")) {
            continue;
        }
        let (from, to) = (entry.path(), destination.join(&name));
        let metadata = fs::symlink_metadata(&from)?;
        if metadata.is_dir() {
            fs::create_dir_all(&to)
                .with_context(|| format!("failed to create {}", to.display()))?;
            copy_tree(&from, &to)?;
        } else if metadata.is_file() {
            fs::copy(&from, &to).with_context(|| format!("failed to copy {}", from.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_MANIFEST: &str = r#"[package]
name = "polint-example-basic-rule"
version.workspace = true
edition.workspace = true
publish = false

[dependencies]
polint = { workspace = true }

[lints]
workspace = true
"#;

    fn rewrite() -> String {
        standalone_pack_manifest(EXAMPLE_MANIFEST, Path::new("/repo/crates/polint")).unwrap()
    }

    #[test]
    fn standalone_manifest_keeps_the_name_and_points_polint_at_this_checkout() {
        let rewritten = rewrite();
        assert!(rewritten.contains("name = \"polint-example-basic-rule\""));
        assert!(rewritten.contains(
            r#"polint = { path = "/repo/crates/polint", default-features = false, features = ["lang-go", "lang-typescript"] }"#
        ));
    }

    #[test]
    fn standalone_manifest_leaves_this_repositorys_workspace_behind() {
        // Without its own `[workspace]` the copy is absorbed by whatever
        // workspace encloses the scratch directory, and the measured build is
        // not the consumer's; workspace lint inheritance would then not resolve.
        let rewritten = rewrite();
        assert!(rewritten.trim_end().ends_with("[workspace]"));
        assert!(!rewritten.contains("[lints]"));
    }

    #[test]
    fn standalone_manifest_rejects_a_pack_with_extra_dependencies() {
        let manifest = EXAMPLE_MANIFEST.replace(
            "polint = { workspace = true }",
            "polint = { workspace = true }\nserde = \"1\"",
        );
        let error = standalone_pack_manifest(&manifest, Path::new("/repo/crates/polint"))
            .expect_err("extra dependencies change the measured closure");
        assert!(error.to_string().contains("sole dependency"));
    }

    #[test]
    fn rule_paths_and_ids_come_from_configuration_with_the_cli_default() {
        let empty: toml::Value = toml::from_str("[workspace]\ninclude = []\n").unwrap();
        assert_eq!(rule_paths(&empty), vec![PathBuf::from(".polint/rules")]);
        let configured: toml::Value = toml::from_str(
            "[rules]\npaths = [\"tools/rules\"]\n\n[[rules.config]]\nid = \"local/no-raw-colors\"\n",
        )
        .unwrap();
        assert_eq!(rule_paths(&configured), vec![PathBuf::from("tools/rules")]);
        assert_eq!(
            rule_ids(&configured),
            vec!["local/no-raw-colors".to_string()]
        );
    }

    #[test]
    fn dir_usage_sums_bytes_and_files_and_is_zero_when_absent() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("nested")).unwrap();
        fs::write(dir.path().join("a.bin"), [0u8; 10]).unwrap();
        fs::write(dir.path().join("nested/b.bin"), [0u8; 5]).unwrap();
        let usage = dir_usage(dir.path(), None).unwrap();
        assert_eq!((usage.bytes, usage.files), (15, 2));
        assert_eq!(
            dir_usage(&dir.path().join("absent"), None).unwrap(),
            DirUsage::default()
        );
    }

    #[cfg(unix)]
    #[test]
    fn dir_usage_counts_hard_linked_content_once() {
        // Cargo hard-links a binary into both `deps/` and the profile root;
        // counting both would inflate retention by the binary's size.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("bin"), [0u8; 64]).unwrap();
        fs::hard_link(dir.path().join("bin"), dir.path().join("bin-hash")).unwrap();
        let usage = dir_usage(dir.path(), None).unwrap();
        assert_eq!((usage.bytes, usage.files), (64, 1));
    }

    #[test]
    fn dir_usage_counts_only_files_written_after_the_mark() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("old.bin"), [0u8; 8]).unwrap();
        let future = SystemTime::now() + std::time::Duration::from_secs(3600);
        let usage = dir_usage(dir.path(), Some(future)).unwrap();
        assert_eq!((usage.bytes, usage.bytes_written), (8, 0));
    }

    #[test]
    fn copy_tree_skips_compiler_output_and_version_control() {
        let (source, destination) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        fs::create_dir_all(source.path().join("target")).unwrap();
        fs::create_dir_all(source.path().join(".git")).unwrap();
        fs::write(source.path().join("target/artifact"), "x").unwrap();
        fs::write(source.path().join(".git/HEAD"), "x").unwrap();
        fs::write(source.path().join("Button.tsx"), "x").unwrap();
        copy_tree(source.path(), destination.path()).unwrap();
        assert!(destination.path().join("Button.tsx").is_file());
        assert!(!destination.path().join("target").exists());
        assert!(!destination.path().join(".git").exists());
    }

    #[test]
    fn a_repository_without_a_rule_pack_is_rejected() {
        let source = tempfile::tempdir().unwrap();
        let destination = tempfile::tempdir().unwrap();
        fs::write(source.path().join(".polint.toml"), "[workspace]\n").unwrap();
        let error = materialize(
            source.path(),
            &destination.path().join("repo"),
            Path::new("/repo/crates/polint"),
            None,
        )
        .expect_err("build cost is undefined without a rule host");
        assert!(error.to_string().contains("no repo-local rule pack"));
    }
}
