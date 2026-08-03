#![expect(
    dead_code,
    reason = "the private projection also supports isolated durable identity validation"
)]

use super::incremental::{Digest, DigestBuilder, DigestKind, PrecisionTier};
use super::outcome::ProviderOutcomeError;
use super::{
    ProviderManifest, ProviderOutcome, ProviderOutcomeStatus, ProviderOutputIdentity, provider,
};
use crate::core::{AnalysisDb, FileId, FunctionId, Language, SourceFile, Span};
use crate::diagnostics::{Diagnostic, TextRange};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const MAX_GO_ROWS: usize = 1_000_000;
pub(crate) const GO_FACT_SCHEMA: &str = "go-facts-v2";
pub(crate) const GO_PAYLOAD_SCHEMA: &str = "go-syntax-layer-v1";
pub(crate) const GO_PARSER_BACKEND: &str = "tree-sitter-0.26.8";
pub(crate) const GO_PARSER_GRAMMAR: &str = "tree-sitter-go-0.25.0";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CanonicalGoSyntaxSource {
    pub(crate) path: String,
    pub(crate) language: Language,
    pub(crate) source_digest: Digest,
}

impl CanonicalGoSyntaxSource {
    pub(crate) fn digest(&self) -> Digest {
        let mut digest = Digest::builder(DigestKind::SourceText, "go-syntax-source-v1");
        digest.field("path", &self.path);
        digest.field("language", "go");
        digest.field("source-kind", self.source_digest.kind.as_str());
        digest.field("source-value", &self.source_digest.value);
        digest.finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalGoSyntaxInputs {
    pub(crate) sources: Vec<CanonicalGoSyntaxSource>,
}

impl CanonicalGoSyntaxInputs {
    pub(crate) fn from_db(db: &AnalysisDb) -> Result<Self, GoSyntaxProjectionError> {
        if db.files().len() > MAX_GO_ROWS {
            return Err(GoSyntaxProjectionError::Source);
        }
        let mut sources = Vec::new();
        for file in db.files() {
            if file.relative_path.ends_with(".go") && file.language != Language::Go {
                return Err(GoSyntaxProjectionError::Source);
            }
            if file.language != Language::Go {
                continue;
            }
            let path = canonical_path(&file.relative_path)?;
            let value = crate::diagnostics::fingerprint(&[file.source.as_ref()]);
            if value != file.content_hash || !valid_digest_value(&value) {
                return Err(GoSyntaxProjectionError::Source);
            }
            sources.push(CanonicalGoSyntaxSource {
                path,
                language: Language::Go,
                source_digest: Digest {
                    kind: DigestKind::SourceText,
                    value,
                },
            });
        }
        sources.sort();
        validate_inputs(&sources)?;
        Ok(Self { sources })
    }

    pub(crate) fn source_digests(&self) -> Vec<Digest> {
        self.sources
            .iter()
            .map(CanonicalGoSyntaxSource::digest)
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GoSyntaxParserContract {
    pub(crate) provider_id: String,
    pub(crate) provider_version: String,
    pub(crate) fact_schema: String,
    pub(crate) payload_schema: String,
    pub(crate) backend: String,
    pub(crate) grammar: String,
}

impl GoSyntaxParserContract {
    pub(crate) fn current() -> Self {
        let manifest = go_manifest();
        Self {
            provider_id: manifest.id.to_string(),
            provider_version: manifest.provider_version().to_string(),
            fact_schema: GO_FACT_SCHEMA.to_string(),
            payload_schema: GO_PAYLOAD_SCHEMA.to_string(),
            backend: GO_PARSER_BACKEND.to_string(),
            grammar: GO_PARSER_GRAMMAR.to_string(),
        }
    }

    pub(crate) fn digest(&self) -> Digest {
        let mut digest = Digest::builder(DigestKind::ProviderParameters, "go-parser-contract-v1");
        for (label, value) in [
            ("provider-id", self.provider_id.as_str()),
            ("provider-version", self.provider_version.as_str()),
            ("fact-schema", self.fact_schema.as_str()),
            ("payload-schema", self.payload_schema.as_str()),
            ("backend", self.backend.as_str()),
            ("grammar", self.grammar.as_str()),
        ] {
            digest.field(label, value);
        }
        digest.finish()
    }

    pub(crate) fn validate(&self) -> Result<(), GoSyntaxProjectionError> {
        (self == &Self::current())
            .then_some(())
            .ok_or(GoSyntaxProjectionError::Parser)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CanonicalGoSyntaxOutput {
    families: [Vec<String>; 7],
}

impl CanonicalGoSyntaxOutput {
    pub(crate) fn from_db(
        db: &AnalysisDb,
        diagnostics: &[Diagnostic],
    ) -> Result<Self, GoSyntaxProjectionError> {
        let context = GoContext::new(db)?;
        for count in [
            db.packages().len(),
            db.functions().len(),
            db.imports().len(),
            db.tests().len(),
            db.branches().len(),
            db.string_literals().len(),
            diagnostics.len(),
        ] {
            if count > MAX_GO_ROWS {
                return Err(GoSyntaxProjectionError::Output);
            }
        }
        let mut functions = BTreeMap::new();
        let mut function_rows = Vec::new();
        for fact in db.functions().iter().filter(|fact| context.owns(fact.file)) {
            let source = context.source(fact.file)?;
            if fact.language != Language::Go || !fact.calls.is_sorted() {
                return Err(GoSyntaxProjectionError::Output);
            }
            let span = canonical_span(&fact.span, source)?;
            let row = row_digest("function", |row| {
                append_path_span(row, &source.relative_path, span);
                row.field("name", &fact.name);
                row.field("language", "go");
                row.bool_part(fact.is_test);
                row.bool_part(fact.is_exported);
                row.u64_field("complexity", u64::from(fact.cyclomatic_complexity));
                for call in &fact.calls {
                    row.field("call", call);
                }
            });
            if functions.insert(fact.id, row.clone()).is_some() || function_rows.contains(&row) {
                return Err(GoSyntaxProjectionError::Output);
            }
            function_rows.push(row);
        }
        let mut packages = Vec::new();
        for fact in db.packages().iter().filter(|fact| context.owns(fact.file)) {
            let source = context.source(fact.file)?;
            if fact.language != Language::Go {
                return Err(GoSyntaxProjectionError::Output);
            }
            let span = canonical_span(&fact.span, source)?;
            packages.push(row_digest("package", |row| {
                append_path_span(row, &source.relative_path, span);
                row.field("name", &fact.name);
                row.field("language", "go");
            }));
        }
        let mut imports = Vec::new();
        for fact in db.imports().iter().filter(|fact| context.owns(fact.file)) {
            let source = context.source(fact.file)?;
            if fact.language != Language::Go {
                return Err(GoSyntaxProjectionError::Output);
            }
            let span = canonical_span(&fact.span, source)?;
            imports.push(row_digest("import", |row| {
                append_path_span(row, &source.relative_path, span);
                row.field("package", fact.package.as_deref().unwrap_or(""));
                row.field("path", &fact.path);
                row.field("language", "go");
            }));
        }
        let mut tests = Vec::new();
        for fact in db.tests().iter().filter(|fact| context.owns(fact.file)) {
            let source = context.source(fact.file)?;
            let span = canonical_span(&fact.span, source)?;
            let function = function_ref(fact.function, &functions)?;
            tests.push(row_digest("test", |row| {
                append_path_span(row, &source.relative_path, span);
                row.field("function", function);
                row.field("name", &fact.name);
                for term in &fact.evidence_terms {
                    row.field("evidence", term);
                }
                row.u64_field("assertions", u64::from(fact.assertion_count));
                row.u64_field("subtests", u64::from(fact.subtest_count));
                for name in &fact.subtest_names {
                    row.field("subtest-name", name);
                }
                row.u64_field("table-rows", u64::from(fact.table_rows));
            }));
        }
        let mut branches = Vec::new();
        for fact in db.branches().iter().filter(|fact| context.owns(fact.file)) {
            let source = context.source(fact.file)?;
            let span = canonical_span(&fact.decision_span, source)?;
            let function = function_ref(fact.function, &functions)?;
            branches.push(row_digest("branch", |row| {
                append_path_span(row, &source.relative_path, span);
                row.field("function", function);
                row.field("condition", &fact.condition_text);
                row.field("edge", &fact.edge_label);
                row.bool_part(fact.is_error_path);
                row.field("fingerprint", &fact.stable_fingerprint);
            }));
        }
        let mut literals = Vec::new();
        for fact in db
            .string_literals()
            .iter()
            .filter(|fact| context.owns(fact.file))
        {
            let source = context.source(fact.file)?;
            if fact.language != Language::Go {
                return Err(GoSyntaxProjectionError::Output);
            }
            let span = canonical_span(&fact.span, source)?;
            literals.push(row_digest("string-literal", |row| {
                append_path_span(row, &source.relative_path, span);
                row.field("value", &fact.value);
                row.field("language", "go");
            }));
        }
        let mut parser_diagnostics = Vec::new();
        for diagnostic in diagnostics {
            if diagnostic.rule_id == "internal/cache" {
                return Err(GoSyntaxProjectionError::Output);
            }
            if diagnostic.rule_id != "parser/go" {
                continue;
            }
            let source = context.by_path(&diagnostic.file)?;
            validate_range(diagnostic.range, source)?;
            parser_diagnostics.push(row_digest("parser-diagnostic", |row| {
                row.field("severity", &diagnostic.severity.to_string());
                row.field("path", &diagnostic.file);
                append_range(row, diagnostic.range);
                row.field("message", &diagnostic.message);
                row.field("fingerprint", &diagnostic.stable_fingerprint);
            }));
        }
        let mut families = [
            packages,
            function_rows,
            imports,
            tests,
            branches,
            literals,
            parser_diagnostics,
        ];
        families.iter_mut().for_each(|rows| rows.sort());
        Ok(Self { families })
    }

    pub(crate) fn digest(&self) -> Digest {
        let mut digest = Digest::builder(DigestKind::ProviderOutput, "go-syntax-output-v1");
        for (family, rows) in [
            "packages",
            "functions",
            "imports",
            "go-tests",
            "branch-obligations",
            "string-literals",
            "parser-diagnostics",
        ]
        .into_iter()
        .zip(&self.families)
        {
            digest.field("family", family);
            digest.u64_field("row-count", rows.len() as u64);
            rows.iter().for_each(|row| digest.field("row", row));
        }
        digest.finish()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GoSyntaxProviderProjection {
    pub(crate) manifest: ProviderManifest,
    pub(crate) outcome: ProviderOutcome,
    pub(crate) inputs: Option<CanonicalGoSyntaxInputs>,
    pub(crate) parser: Option<GoSyntaxParserContract>,
}

impl GoSyntaxProviderProjection {
    pub(crate) fn from_db(
        outcome: ProviderOutcome,
        db: &AnalysisDb,
        diagnostics: &[Diagnostic],
    ) -> Result<Self, GoSyntaxProjectionError> {
        let (inputs, parser) = if outcome.status == ProviderOutcomeStatus::Succeeded {
            let identity = outcome
                .output_identity
                .as_ref()
                .ok_or(GoSyntaxProjectionError::Output)?;
            if identity.output_digest != CanonicalGoSyntaxOutput::from_db(db, diagnostics)?.digest()
            {
                return Err(GoSyntaxProjectionError::Output);
            }
            (
                Some(CanonicalGoSyntaxInputs::from_db(db)?),
                Some(GoSyntaxParserContract::current()),
            )
        } else {
            (None, None)
        };
        Self::from_durable_parts(outcome, inputs, parser)
    }

    pub(crate) fn from_durable_parts(
        outcome: ProviderOutcome,
        inputs: Option<CanonicalGoSyntaxInputs>,
        parser: Option<GoSyntaxParserContract>,
    ) -> Result<Self, GoSyntaxProjectionError> {
        let manifest = go_manifest();
        let outcome = ProviderOutcome::from_closed_parts(
            outcome.provider_id,
            outcome.status,
            outcome.output_identity,
            outcome.failure_stage,
            outcome.failure_reason,
            outcome.blockers,
        )?;
        if outcome.provider_id != manifest.id
            || outcome
                .blockers
                .iter()
                .any(|blocker| blocker != "polint.source")
        {
            return Err(GoSyntaxProjectionError::Output);
        }
        match (&outcome.output_identity, &inputs, &parser) {
            (Some(identity), Some(inputs), Some(parser)) => {
                validate_identity(identity, &manifest)?;
                validate_inputs(&inputs.sources)?;
                parser.validate()?;
            }
            (None, None, None) if outcome.status != ProviderOutcomeStatus::Succeeded => {}
            _ => return Err(GoSyntaxProjectionError::Output),
        }
        Ok(Self {
            manifest,
            outcome,
            inputs,
            parser,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum GoSyntaxProjectionError {
    #[error("Go syntax projection contains an invalid source")]
    Source,
    #[error("Go syntax projection contains an invalid parser contract")]
    Parser,
    #[error("Go syntax projection contains inconsistent output")]
    Output,
    #[error("Go syntax projection contains an invalid sealed outcome")]
    Outcome(#[from] ProviderOutcomeError),
}

struct GoContext<'a> {
    files: BTreeMap<FileId, &'a SourceFile>,
    paths: BTreeMap<&'a str, &'a SourceFile>,
}

impl<'a> GoContext<'a> {
    fn new(db: &'a AnalysisDb) -> Result<Self, GoSyntaxProjectionError> {
        let inputs = CanonicalGoSyntaxInputs::from_db(db)?;
        let allowed = inputs
            .sources
            .iter()
            .map(|source| source.path.as_str())
            .collect::<BTreeSet<_>>();
        let mut files = BTreeMap::new();
        let mut paths = BTreeMap::new();
        for file in db
            .files()
            .iter()
            .filter(|file| file.language == Language::Go)
        {
            if !allowed.contains(file.relative_path.as_str())
                || files.insert(file.id, file).is_some()
                || paths.insert(file.relative_path.as_str(), file).is_some()
            {
                return Err(GoSyntaxProjectionError::Source);
            }
        }
        Ok(Self { files, paths })
    }
    fn owns(&self, file: FileId) -> bool {
        self.files.contains_key(&file)
    }
    fn source(&self, file: FileId) -> Result<&'a SourceFile, GoSyntaxProjectionError> {
        self.files
            .get(&file)
            .copied()
            .ok_or(GoSyntaxProjectionError::Output)
    }
    fn by_path(&self, path: &str) -> Result<&'a SourceFile, GoSyntaxProjectionError> {
        self.paths
            .get(path)
            .copied()
            .ok_or(GoSyntaxProjectionError::Output)
    }
}

type CanonicalSpan = (u32, u32, u32, u32, u32, u32);

fn canonical_span(
    span: &Span,
    source: &SourceFile,
) -> Result<CanonicalSpan, GoSyntaxProjectionError> {
    let line_count = u32::try_from(source.source.lines().count())
        .map_err(|_| GoSyntaxProjectionError::Output)?;
    if span.file != source.id
        || span.start_byte > span.end_byte
        || usize::try_from(span.end_byte).map_or(true, |end| end > source.source.len())
        || span.start_line == 0
        || span.start_line > span.end_line
        || span.end_line > line_count
        || span.start_col == 0
        || (span.start_line, span.start_col) > (span.end_line, span.end_col)
    {
        return Err(GoSyntaxProjectionError::Output);
    }
    Ok((
        span.start_byte,
        span.end_byte,
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col,
    ))
}

fn validate_range(range: TextRange, source: &SourceFile) -> Result<(), GoSyntaxProjectionError> {
    let lines = u32::try_from(source.source.lines().count())
        .map_err(|_| GoSyntaxProjectionError::Output)?;
    if range.start_line == 0
        || range.start_line > range.end_line
        || range.end_line > lines
        || range.start_col == 0
        || (range.start_line, range.start_col) > (range.end_line, range.end_col)
    {
        return Err(GoSyntaxProjectionError::Output);
    }
    Ok(())
}

fn function_ref(
    id: Option<FunctionId>,
    functions: &BTreeMap<FunctionId, String>,
) -> Result<&str, GoSyntaxProjectionError> {
    id.map_or(Ok(""), |id| {
        functions
            .get(&id)
            .map(String::as_str)
            .ok_or(GoSyntaxProjectionError::Output)
    })
}

fn row_digest(label: &'static str, append: impl FnOnce(&mut DigestBuilder)) -> String {
    let mut digest = Digest::builder(DigestKind::ProviderParameters, label);
    append(&mut digest);
    digest.finish().to_string()
}

fn append_path_span(digest: &mut DigestBuilder, path: &str, span: CanonicalSpan) {
    digest.field("path", path);
    digest.u64_field("start-byte", u64::from(span.0));
    digest.u64_field("end-byte", u64::from(span.1));
    digest.u64_field("start-line", u64::from(span.2));
    digest.u64_field("start-col", u64::from(span.3));
    digest.u64_field("end-line", u64::from(span.4));
    digest.u64_field("end-col", u64::from(span.5));
}

fn append_range(digest: &mut DigestBuilder, range: TextRange) {
    digest.u64_field("start-line", u64::from(range.start_line));
    digest.u64_field("start-col", u64::from(range.start_col));
    digest.u64_field("end-line", u64::from(range.end_line));
    digest.u64_field("end-col", u64::from(range.end_col));
}

fn validate_inputs(sources: &[CanonicalGoSyntaxSource]) -> Result<(), GoSyntaxProjectionError> {
    if sources.len() > MAX_GO_ROWS || !sources.is_sorted() {
        return Err(GoSyntaxProjectionError::Source);
    }
    let mut paths = BTreeSet::new();
    for source in sources {
        if canonical_path(&source.path)? != source.path
            || source.language != Language::Go
            || source.source_digest.kind != DigestKind::SourceText
            || !valid_digest_value(&source.source_digest.value)
            || !paths.insert(source.path.as_str())
        {
            return Err(GoSyntaxProjectionError::Source);
        }
    }
    Ok(())
}

fn validate_identity(
    identity: &ProviderOutputIdentity,
    manifest: &ProviderManifest,
) -> Result<(), GoSyntaxProjectionError> {
    if identity.provider_id != manifest.id
        || identity.provider_version != manifest.provider_version()
        || identity.schema_version != manifest.primary_schema_label()
        || identity.precision != PrecisionTier::Syntax
        || identity.output_digest.kind != DigestKind::ProviderOutput
        || !valid_digest_value(&identity.output_digest.value)
    {
        return Err(GoSyntaxProjectionError::Output);
    }
    Ok(())
}

fn canonical_path(path: &str) -> Result<String, GoSyntaxProjectionError> {
    let bytes = path.as_bytes();
    if Path::new(path).is_absolute()
        || path.starts_with(['/', '\\'])
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
    {
        return Err(GoSyntaxProjectionError::Source);
    }
    let normalized =
        crate::repo_fs::normalize_repo_relative(path).ok_or(GoSyntaxProjectionError::Source)?;
    if normalized == "." || normalized != path {
        return Err(GoSyntaxProjectionError::Source);
    }
    Ok(normalized)
}

fn valid_digest_value(value: &str) -> bool {
    value.len() == 16
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn go_manifest() -> ProviderManifest {
    *provider::provider_manifests()
        .iter()
        .find(|manifest| manifest.id == "polint.go.syntax")
        .expect("static provider inventory contains polint.go.syntax")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_inputs_are_ordered_and_parser_contract_is_closed() {
        let mut db = AnalysisDb::new();
        db.add_file("b.go".into(), "b.go".into(), "package b\n".into());
        db.add_file("a.go".into(), "a.go".into(), "package a\n".into());
        db.add_file("x.ts".into(), "x.ts".into(), "export {};\n".into());
        let inputs = CanonicalGoSyntaxInputs::from_db(&db).unwrap();
        assert_eq!(
            inputs
                .sources
                .iter()
                .map(|source| source.path.as_str())
                .collect::<Vec<_>>(),
            ["a.go", "b.go"]
        );
        GoSyntaxParserContract::current().validate().unwrap();
    }

    #[test]
    fn every_parser_contract_member_changes_its_digest() {
        let contract = GoSyntaxParserContract::current();
        for mutate in [
            |value: &mut GoSyntaxParserContract| value.provider_id.push('x'),
            |value: &mut GoSyntaxParserContract| value.provider_version.push('x'),
            |value: &mut GoSyntaxParserContract| value.fact_schema.push('x'),
            |value: &mut GoSyntaxParserContract| value.payload_schema.push('x'),
            |value: &mut GoSyntaxParserContract| value.backend.push('x'),
            |value: &mut GoSyntaxParserContract| value.grammar.push('x'),
        ] {
            let mut changed = contract.clone();
            mutate(&mut changed);
            assert_ne!(changed.digest(), contract.digest());
            assert!(changed.validate().is_err());
        }
    }

    #[test]
    fn operational_cache_diagnostics_are_not_semantic_output() {
        let mut db = AnalysisDb::new();
        db.add_file("a.go".into(), "a.go".into(), "package a\n".into());
        let warning = Diagnostic::warning(
            "internal/cache",
            "a.go",
            TextRange::point(1, 1),
            "write failed",
        );
        assert!(CanonicalGoSyntaxOutput::from_db(&db, &[warning]).is_err());
    }
}
