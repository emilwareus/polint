use std::path::Path;

use super::{Digest, DigestKind, FileSnapshot};
use crate::core::Language;
use crate::repo_fs::normalize_repo_relative;

pub(crate) const RUN_MANIFEST_SCHEMA: &str = "polint-run-manifest-1";
const WORKSPACE_PURPOSE: &str = "canonical-workspace-root-v1";
const CONFIG_PURPOSE: &str = "complete-polint-config-v1";
const SOURCE_PURPOSE: &str = "source-text-v1";
const RUN_PURPOSE: &str = "run-manifest-fields-v1";
const DIGEST_HEX_BYTES: usize = 16;

#[derive(Clone, Copy)]
pub(crate) struct RunManifestInputs<'a> {
    workspace_root: &'a Path,
    config_hash: &'a str,
    files: &'a [FileSnapshot],
}

impl<'a> RunManifestInputs<'a> {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "private publication callers construct inputs when durable reuse is enabled"
        )
    )]
    pub(crate) fn new(
        workspace_root: &'a Path,
        config_hash: &'a str,
        files: &'a [FileSnapshot],
    ) -> Self {
        Self {
            workspace_root,
            config_hash,
            files,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RunManifest {
    schema: &'static str,
    workspace: Digest,
    config: Digest,
    sources: Vec<RunManifestSource>,
    run_identity: Digest,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RunManifestSource {
    relative_path: String,
    language: Language,
    source_text_digest: Digest,
    size_bytes: u64,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct EncodedRunManifest {
    pub(crate) schema: String,
    pub(crate) workspace_purpose: String,
    pub(crate) workspace_value: String,
    pub(crate) config_purpose: String,
    pub(crate) config_value: String,
    pub(crate) source_count: i64,
    pub(crate) run_purpose: String,
    pub(crate) run_value: String,
    pub(crate) sources: Vec<EncodedRunManifestSource>,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct EncodedRunManifestSource {
    pub(crate) relative_path: String,
    pub(crate) language: String,
    pub(crate) source_purpose: String,
    pub(crate) source_value: String,
    pub(crate) size_bytes: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum RunManifestError {
    #[error("workspace root is unavailable")]
    WorkspaceUnavailable,
    #[error("manifest schema is unsupported")]
    UnsupportedSchema,
    #[error("manifest purpose is invalid")]
    InvalidPurpose,
    #[error("manifest digest is invalid")]
    InvalidDigest,
    #[error("source digest has the wrong purpose")]
    WrongSourceDigestPurpose,
    #[error("source path is not canonical")]
    NonCanonicalSourcePath,
    #[error("source path is duplicated")]
    DuplicateSourcePath,
    #[error("source language is unsupported")]
    UnsupportedLanguage,
    #[error("manifest numeric value is invalid")]
    InvalidNumericValue,
    #[error("manifest source count does not match its rows")]
    SourceCountMismatch,
    #[error("manifest source rows are not in canonical order")]
    NonCanonicalSourceOrder,
    #[error("stored run identity does not match canonical fields")]
    RunIdentityMismatch,
}

impl RunManifest {
    pub(crate) fn from_inputs(inputs: RunManifestInputs<'_>) -> Result<Self, RunManifestError> {
        let workspace_root = inputs
            .workspace_root
            .canonicalize()
            .map_err(|_| RunManifestError::WorkspaceUnavailable)?;
        let workspace = workspace_digest(&workspace_root);
        let config = checked_digest(DigestKind::Config, inputs.config_hash)?;
        let mut sources = inputs
            .files
            .iter()
            .map(RunManifestSource::from_file_snapshot)
            .collect::<Result<Vec<_>, _>>()?;
        sources.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
        validate_canonical_sources(&sources)?;
        Self::from_canonical(workspace, config, sources)
    }

    pub(crate) fn encode(&self) -> Result<EncodedRunManifest, RunManifestError> {
        let source_count =
            i64::try_from(self.sources.len()).map_err(|_| RunManifestError::InvalidNumericValue)?;
        let sources = self
            .sources
            .iter()
            .map(RunManifestSource::encode)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EncodedRunManifest {
            schema: self.schema.to_string(),
            workspace_purpose: WORKSPACE_PURPOSE.to_string(),
            workspace_value: self.workspace.value.clone(),
            config_purpose: CONFIG_PURPOSE.to_string(),
            config_value: self.config.value.clone(),
            source_count,
            run_purpose: RUN_PURPOSE.to_string(),
            run_value: self.run_identity.value.clone(),
            sources,
        })
    }

    pub(crate) fn decode(encoded: EncodedRunManifest) -> Result<Self, RunManifestError> {
        if encoded.schema != RUN_MANIFEST_SCHEMA {
            return Err(RunManifestError::UnsupportedSchema);
        }
        if encoded.workspace_purpose != WORKSPACE_PURPOSE
            || encoded.config_purpose != CONFIG_PURPOSE
            || encoded.run_purpose != RUN_PURPOSE
        {
            return Err(RunManifestError::InvalidPurpose);
        }
        let source_count = usize::try_from(encoded.source_count)
            .map_err(|_| RunManifestError::InvalidNumericValue)?;
        if source_count != encoded.sources.len() {
            return Err(RunManifestError::SourceCountMismatch);
        }

        let workspace = checked_digest(DigestKind::Workspace, &encoded.workspace_value)?;
        let config = checked_digest(DigestKind::Config, &encoded.config_value)?;
        let stored_run = checked_digest(DigestKind::RunManifest, &encoded.run_value)?;
        let sources = encoded
            .sources
            .into_iter()
            .map(RunManifestSource::decode)
            .collect::<Result<Vec<_>, _>>()?;
        validate_canonical_sources(&sources)?;
        let manifest = Self::from_canonical(workspace, config, sources)?;
        if manifest.run_identity != stored_run {
            return Err(RunManifestError::RunIdentityMismatch);
        }
        Ok(manifest)
    }

    pub(crate) fn workspace_identity(&self) -> &Digest {
        &self.workspace
    }

    pub(crate) fn exact_match(&self, other: &Self) -> bool {
        self == other
    }

    fn from_canonical(
        workspace: Digest,
        config: Digest,
        sources: Vec<RunManifestSource>,
    ) -> Result<Self, RunManifestError> {
        if workspace.kind != DigestKind::Workspace || config.kind != DigestKind::Config {
            return Err(RunManifestError::InvalidPurpose);
        }
        validate_digest_value(&workspace.value)?;
        validate_digest_value(&config.value)?;
        validate_canonical_sources(&sources)?;
        let run_identity = run_identity(&workspace, &config, &sources)?;
        Ok(Self {
            schema: RUN_MANIFEST_SCHEMA,
            workspace,
            config,
            sources,
            run_identity,
        })
    }
}

impl RunManifestSource {
    fn from_file_snapshot(file: &FileSnapshot) -> Result<Self, RunManifestError> {
        let relative_path = canonical_source_path(&file.relative_path)?;
        let language = checked_language(file.language)?;
        if file.source_text_digest.kind != DigestKind::SourceText {
            return Err(RunManifestError::WrongSourceDigestPurpose);
        }
        validate_digest_value(&file.source_text_digest.value)?;
        let size_bytes =
            u64::try_from(file.size_bytes).map_err(|_| RunManifestError::InvalidNumericValue)?;
        Ok(Self {
            relative_path,
            language,
            source_text_digest: file.source_text_digest.clone(),
            size_bytes,
        })
    }

    fn encode(&self) -> Result<EncodedRunManifestSource, RunManifestError> {
        let size_bytes =
            i64::try_from(self.size_bytes).map_err(|_| RunManifestError::InvalidNumericValue)?;
        Ok(EncodedRunManifestSource {
            relative_path: self.relative_path.clone(),
            language: language_label(self.language)?.to_string(),
            source_purpose: SOURCE_PURPOSE.to_string(),
            source_value: self.source_text_digest.value.clone(),
            size_bytes,
        })
    }

    fn decode(encoded: EncodedRunManifestSource) -> Result<Self, RunManifestError> {
        if encoded.source_purpose != SOURCE_PURPOSE {
            return Err(RunManifestError::InvalidPurpose);
        }
        let relative_path = canonical_source_path(&encoded.relative_path)?;
        let language = decode_language(&encoded.language)?;
        let source_text_digest = checked_digest(DigestKind::SourceText, &encoded.source_value)?;
        let size_bytes =
            u64::try_from(encoded.size_bytes).map_err(|_| RunManifestError::InvalidNumericValue)?;
        Ok(Self {
            relative_path,
            language,
            source_text_digest,
            size_bytes,
        })
    }
}

fn workspace_digest(root: &Path) -> Digest {
    let mut builder = Digest::builder(DigestKind::Workspace, "workspace-root-v1");
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        builder.field("platform", "unix");
        builder.bytes_field("root-units", root.as_os_str().as_bytes());
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        builder.field("platform", "windows");
        for unit in root.as_os_str().encode_wide() {
            builder.u64_field("root-unit", u64::from(unit));
        }
    }
    builder.finish()
}

fn run_identity(
    workspace: &Digest,
    config: &Digest,
    sources: &[RunManifestSource],
) -> Result<Digest, RunManifestError> {
    let source_count =
        u64::try_from(sources.len()).map_err(|_| RunManifestError::InvalidNumericValue)?;
    let mut builder = Digest::builder(DigestKind::RunManifest, RUN_PURPOSE);
    builder.field("manifest-schema", RUN_MANIFEST_SCHEMA);
    builder.field("workspace-purpose", WORKSPACE_PURPOSE);
    builder.field("workspace-value", &workspace.value);
    builder.field("config-purpose", CONFIG_PURPOSE);
    builder.field("config-value", &config.value);
    builder.u64_field("source-count", source_count);
    for source in sources {
        builder.field("source-path", &source.relative_path);
        builder.field("source-language", language_label(source.language)?);
        builder.field("source-purpose", SOURCE_PURPOSE);
        builder.field("source-value", &source.source_text_digest.value);
        builder.u64_field("source-size", source.size_bytes);
    }
    Ok(builder.finish())
}

fn validate_canonical_sources(sources: &[RunManifestSource]) -> Result<(), RunManifestError> {
    for source in sources {
        canonical_source_path(&source.relative_path)?;
        checked_language(source.language)?;
        if source.source_text_digest.kind != DigestKind::SourceText {
            return Err(RunManifestError::WrongSourceDigestPurpose);
        }
        validate_digest_value(&source.source_text_digest.value)?;
    }
    for pair in sources.windows(2) {
        match pair[0].relative_path.cmp(&pair[1].relative_path) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => return Err(RunManifestError::DuplicateSourcePath),
            std::cmp::Ordering::Greater => return Err(RunManifestError::NonCanonicalSourceOrder),
        }
    }
    Ok(())
}

fn canonical_source_path(path: &str) -> Result<String, RunManifestError> {
    let normalized =
        normalize_repo_relative(path).ok_or(RunManifestError::NonCanonicalSourcePath)?;
    if normalized != path || normalized == "." {
        return Err(RunManifestError::NonCanonicalSourcePath);
    }
    Ok(normalized)
}

fn checked_digest(kind: DigestKind, value: &str) -> Result<Digest, RunManifestError> {
    validate_digest_value(value)?;
    Ok(Digest {
        kind,
        value: value.to_string(),
    })
}

fn validate_digest_value(value: &str) -> Result<(), RunManifestError> {
    if value.len() == DIGEST_HEX_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(RunManifestError::InvalidDigest)
    }
}

fn checked_language(language: Language) -> Result<Language, RunManifestError> {
    if language == Language::Unknown {
        Err(RunManifestError::UnsupportedLanguage)
    } else {
        Ok(language)
    }
}

fn language_label(language: Language) -> Result<&'static str, RunManifestError> {
    match language {
        Language::Go => Ok("go"),
        Language::TypeScript => Ok("typescript"),
        Language::Tsx => Ok("tsx"),
        Language::JavaScript => Ok("javascript"),
        Language::Jsx => Ok("jsx"),
        Language::Unknown => Err(RunManifestError::UnsupportedLanguage),
    }
}

fn decode_language(label: &str) -> Result<Language, RunManifestError> {
    match label {
        "go" => Ok(Language::Go),
        "typescript" => Ok(Language::TypeScript),
        "tsx" => Ok(Language::Tsx),
        "javascript" => Ok(Language::JavaScript),
        "jsx" => Ok(Language::Jsx),
        _ => Err(RunManifestError::UnsupportedLanguage),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(path: &str, language: Language, seed: &str, size_bytes: usize) -> FileSnapshot {
        FileSnapshot {
            relative_path: path.to_string(),
            language,
            source_text_digest: Digest::from_parts(DigestKind::SourceText, "test-source", &[seed]),
            size_bytes,
            mtime_hint_present: false,
        }
    }

    fn manifest(
        root: &Path,
        config_hash: &str,
        files: &[FileSnapshot],
    ) -> Result<RunManifest, RunManifestError> {
        RunManifest::from_inputs(RunManifestInputs::new(root, config_hash, files))
    }

    fn config_hash(seed: &str) -> String {
        Digest::from_parts(DigestKind::Config, "test-config", &[seed]).value
    }

    #[test]
    fn round_trip_is_deterministic_and_input_order_independent() {
        let root = tempfile::tempdir().expect("create workspace");
        let a = source("src/a.ts", Language::TypeScript, "a", 10);
        let b = source("src/b.go", Language::Go, "b", 20);
        let config = config_hash("same");
        let first = manifest(root.path(), &config, &[b.clone(), a.clone()])
            .expect("construct first manifest");
        let second = manifest(root.path(), &config, &[a, b]).expect("construct second manifest");
        let decoded =
            RunManifest::decode(first.encode().expect("encode manifest")).expect("decode manifest");

        assert!(first.exact_match(&second));
        assert!(first.exact_match(&decoded));
    }

    #[test]
    fn empty_source_membership_round_trips() {
        let root = tempfile::tempdir().expect("create workspace");
        let manifest =
            manifest(root.path(), &config_hash("empty"), &[]).expect("construct empty manifest");
        let encoded = manifest.encode().expect("encode empty manifest");

        assert_eq!(encoded.source_count, 0);
        assert!(RunManifest::decode(encoded).is_ok());
    }

    #[test]
    fn approved_field_mutations_change_match_or_are_refused() {
        let root = tempfile::tempdir().expect("create workspace");
        let file = source("src/app.ts", Language::TypeScript, "source", 12);
        let baseline = manifest(
            root.path(),
            &config_hash("one"),
            std::slice::from_ref(&file),
        )
        .expect("construct baseline");
        let changed_config = manifest(
            root.path(),
            &config_hash("two"),
            std::slice::from_ref(&file),
        )
        .expect("construct changed config");
        let changed_source = manifest(
            root.path(),
            &config_hash("one"),
            &[source("src/app.ts", Language::TypeScript, "changed", 12)],
        )
        .expect("construct changed source");
        let mut tampered = baseline.encode().expect("encode baseline");
        tampered.sources[0].size_bytes += 1;

        assert!(!baseline.exact_match(&changed_config));
        assert!(!baseline.exact_match(&changed_source));
        assert!(matches!(
            RunManifest::decode(tampered),
            Err(RunManifestError::RunIdentityMismatch)
        ));
    }

    #[test]
    fn duplicate_noncanonical_and_unknown_sources_are_rejected() {
        let root = tempfile::tempdir().expect("create workspace");
        let config = config_hash("config");
        let duplicate = source("src/app.ts", Language::TypeScript, "one", 1);

        assert!(matches!(
            manifest(root.path(), &config, &[duplicate.clone(), duplicate]),
            Err(RunManifestError::DuplicateSourcePath)
        ));
        assert!(matches!(
            manifest(
                root.path(),
                &config,
                &[source("./src/app.ts", Language::TypeScript, "one", 1)]
            ),
            Err(RunManifestError::NonCanonicalSourcePath)
        ));
        assert!(matches!(
            manifest(
                root.path(),
                &config,
                &[source("unknown.txt", Language::Unknown, "one", 1)]
            ),
            Err(RunManifestError::UnsupportedLanguage)
        ));
    }

    #[test]
    fn wrong_digest_purpose_malformed_digest_and_numeric_values_are_rejected() {
        let root = tempfile::tempdir().expect("create workspace");
        let mut file = source("src/app.ts", Language::TypeScript, "one", 1);
        file.source_text_digest.kind = DigestKind::Config;
        assert!(matches!(
            manifest(root.path(), &config_hash("config"), &[file]),
            Err(RunManifestError::WrongSourceDigestPurpose)
        ));

        let valid =
            manifest(root.path(), &config_hash("config"), &[]).expect("construct valid manifest");
        let mut malformed = valid.encode().expect("encode valid manifest");
        malformed.config_value = "NOT-A-DIGEST".to_string();
        assert!(matches!(
            RunManifest::decode(malformed),
            Err(RunManifestError::InvalidDigest)
        ));

        let mut negative = valid.encode().expect("encode valid manifest");
        negative.source_count = -1;
        assert!(matches!(
            RunManifest::decode(negative),
            Err(RunManifestError::InvalidNumericValue)
        ));
    }

    #[test]
    fn workspace_identity_is_distinct_and_does_not_retain_raw_roots() {
        let first_root = tempfile::tempdir().expect("create first workspace");
        let second_root = tempfile::tempdir().expect("create second workspace");
        let config = config_hash("config");
        let first = manifest(first_root.path(), &config, &[]).expect("construct first manifest");
        let second = manifest(second_root.path(), &config, &[]).expect("construct second manifest");
        let encoded = first.encode().expect("encode first manifest");

        assert_ne!(first.workspace_identity(), second.workspace_identity());
        assert!(
            !encoded
                .workspace_value
                .contains(first_root.path().to_str().expect("temporary path is UTF-8"))
        );
    }

    #[test]
    fn mtime_hint_is_not_part_of_manifest_identity() {
        let root = tempfile::tempdir().expect("create workspace");
        let config = config_hash("config");
        let mut first_file = source("src/app.ts", Language::TypeScript, "one", 1);
        let mut second_file = first_file.clone();
        first_file.mtime_hint_present = false;
        second_file.mtime_hint_present = true;
        let first = manifest(root.path(), &config, &[first_file]).expect("first manifest");
        let second = manifest(root.path(), &config, &[second_file]).expect("second manifest");

        assert!(first.exact_match(&second));
    }
}
