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
// These label the parser that produced a cached/persisted Go syntax layer, so
// they must track the resolved dependency versions exactly: a stale label lets
// a grammar change reuse facts parsed by the previous grammar. They are also
// pinned by `CHECK` constraints in the semantic-store schema, so bumping either
// dependency needs this constant *and* a new store migration.
// `go_parser_identity_tracks_the_resolved_dependency_versions` enforces the
// first half against `Cargo.lock`.
pub(crate) const GO_PARSER_BACKEND: &str = "tree-sitter-0.26.8";
pub(crate) const GO_PARSER_GRAMMAR: &str = "tree-sitter-go-0.25.0";

fn ensure_capacity(
    count: usize,
    error: GoSyntaxProjectionError,
) -> Result<(), GoSyntaxProjectionError> {
    (count < MAX_GO_ROWS).then_some(()).ok_or(error)
}

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
        let mut sources = Vec::new();
        for file in db.files() {
            if file.relative_path.ends_with(".go") && file.language != Language::Go {
                return Err(GoSyntaxProjectionError::Source);
            }
            if file.language != Language::Go {
                continue;
            }
            ensure_capacity(sources.len(), GoSyntaxProjectionError::Source)?;
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
        let mut functions = BTreeMap::new();
        let mut function_rows = BTreeSet::new();
        for fact in db.functions().iter().filter(|fact| context.owns(fact.file)) {
            ensure_capacity(function_rows.len(), GoSyntaxProjectionError::Output)?;
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
            let function = FunctionOccurrence {
                row: row.clone(),
                file: fact.file,
                span,
            };
            if !function_rows.insert(row) || functions.insert(fact.id, function).is_some() {
                return Err(GoSyntaxProjectionError::Output);
            }
        }
        let mut packages = Vec::new();
        for fact in db.packages().iter().filter(|fact| context.owns(fact.file)) {
            ensure_capacity(packages.len(), GoSyntaxProjectionError::Output)?;
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
            ensure_capacity(imports.len(), GoSyntaxProjectionError::Output)?;
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
            ensure_capacity(tests.len(), GoSyntaxProjectionError::Output)?;
            let source = context.source(fact.file)?;
            let span = canonical_span(&fact.span, source)?;
            let function = function_ref(fact.function, fact.file, span, true, &functions)?;
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
            ensure_capacity(branches.len(), GoSyntaxProjectionError::Output)?;
            let source = context.source(fact.file)?;
            let span = canonical_span(&fact.decision_span, source)?;
            let function = function_ref(fact.function, fact.file, span, false, &functions)?;
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
            ensure_capacity(literals.len(), GoSyntaxProjectionError::Output)?;
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
            ensure_capacity(parser_diagnostics.len(), GoSyntaxProjectionError::Output)?;
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
            function_rows.into_iter().collect(),
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
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the private publication path is not consumed by normal kernel runs"
        )
    )]
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

struct FunctionOccurrence {
    row: String,
    file: FileId,
    span: CanonicalSpan,
}

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
    file: FileId,
    span: CanonicalSpan,
    exact_span: bool,
    functions: &BTreeMap<FunctionId, FunctionOccurrence>,
) -> Result<&str, GoSyntaxProjectionError> {
    let function = functions
        .get(&id.ok_or(GoSyntaxProjectionError::Output)?)
        .ok_or(GoSyntaxProjectionError::Output)?;
    let span_matches = if exact_span {
        function.span == span
    } else {
        span_contains(function.span, span)
    };
    (function.file == file && span_matches)
        .then_some(function.row.as_str())
        .ok_or(GoSyntaxProjectionError::Output)
}

fn span_contains(outer: CanonicalSpan, inner: CanonicalSpan) -> bool {
    outer.0 <= inner.0
        && inner.1 <= outer.1
        && (outer.2, outer.3) <= (inner.2, inner.3)
        && (inner.4, inner.5) <= (outer.4, outer.5)
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
    use crate::core::{BranchId, BranchObligation, FunctionFact, TestFact};

    #[rustfmt::skip] fn literal_digest(path: &str, value: &str, line: u32, language: Language, count: usize) -> Result<Digest, GoSyntaxProjectionError> {
        let mut db = AnalysisDb::new(); let file = db.add_file(path.into(), path.into(), "first\nsecond\n".into());
        for _ in 0..count { db.push_string_literal(crate::core::StringLiteralFact { file, value: value.into(), span: Span::point(file, line, 1), language }); }
        Ok(CanonicalGoSyntaxOutput::from_db(&db, &[])?.digest())
    }
    #[derive(Clone, Copy, Debug)] #[rustfmt::skip]
    enum RelatedFact { Test, Branch }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)] #[rustfmt::skip]
    enum RelationshipMutation { Valid, Missing, Dangling, CrossFile, Span }
    #[rustfmt::skip] fn span(file: FileId, start: u32, end: u32) -> Span { Span { file, start_byte: start, end_byte: end, start_line: 1, start_col: start + 1, end_line: 1, end_col: end + 1 } }
    #[rustfmt::skip] fn function_fact(file: FileId, name: String, span: Span) -> FunctionFact { FunctionFact { id: FunctionId(0), file, name, span, language: Language::Go, is_test: true, is_exported: true, cyclomatic_complexity: 1, calls: Vec::new() } }
    #[rustfmt::skip] fn relationship_projection(family: RelatedFact, mutation: RelationshipMutation) -> Result<CanonicalGoSyntaxOutput, GoSyntaxProjectionError> {
        let mut db = AnalysisDb::new(); let source = "012345678901234567890123456789012345678901234567890123456789\n";
        let file = db.add_file("a_test.go".into(), "a_test.go".into(), source.into()); let other_file = db.add_file("b_test.go".into(), "b_test.go".into(), source.into()); let function_span = span(file, 5, 40);
        let function = db.push_function(function_fact(file, "TestA".into(), function_span.clone())); let other = db.push_function(function_fact(other_file, "TestB".into(), span(other_file, 5, 40)));
        let function = match mutation { RelationshipMutation::Missing => None, RelationshipMutation::Dangling => Some(FunctionId(99)), RelationshipMutation::CrossFile => Some(other), _ => Some(function) };
        match family {
            RelatedFact::Test => db.push_test(TestFact { file, function, name: "TestA".into(), span: if mutation == RelationshipMutation::Span { span(file, 6, 39) } else { function_span }, evidence_terms: Vec::new(), assertion_count: 0, subtest_count: 0, subtest_names: Vec::new(), table_rows: 0 }),
            RelatedFact::Branch => { db.push_branch(BranchObligation { id: BranchId(0), function, file, decision_span: if mutation == RelationshipMutation::Span { span(file, 45, 46) } else { span(file, 20, 21) }, condition_text: "condition".into(), edge_label: "true".into(), is_error_path: false, stable_fingerprint: "fingerprint".into() }); }
        }
        CanonicalGoSyntaxOutput::from_db(&db, &[])
    }

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

    #[test] #[rustfmt::skip]
    fn unrelated_language_volume_is_filtered_before_owned_bounds() {
        let mut go = AnalysisDb::new(); go.add_file("a.go".into(), "a.go".into(), "package a\n".into());
        let expected = (CanonicalGoSyntaxInputs::from_db(&go).unwrap(), CanonicalGoSyntaxOutput::from_db(&go, &[]).unwrap());
        let mut mixed = go.clone(); mixed.add_file("unrelated.ts".into(), "unrelated.ts".into(), "import value from 'pkg'; export function pick(flag: boolean) { return flag ? 'yes' : value; }\n".into());
        let mut diagnostics = crate::ts::analyze(&mut mixed);
        diagnostics.push(Diagnostic::warning("parser/ts", "unrelated.ts", TextRange::point(1, 1), "unrelated"));
        assert!(!mixed.functions().is_empty() && !mixed.imports().is_empty() && !mixed.string_literals().is_empty());
        assert_eq!((CanonicalGoSyntaxInputs::from_db(&mixed).unwrap(), CanonicalGoSyntaxOutput::from_db(&mixed, &diagnostics).unwrap()), expected);
        let mut selected = 0;
        (0..10_000).map(|_| false).chain([true]).filter(|owned| *owned).try_for_each(|_| { ensure_capacity(selected, GoSyntaxProjectionError::Output)?; selected += 1; Ok::<(), GoSyntaxProjectionError>(()) }).unwrap();
        assert_eq!((selected, ensure_capacity(MAX_GO_ROWS, GoSyntaxProjectionError::Output).is_err()), (1, true));
    }
    #[test] #[rustfmt::skip]
    fn test_and_branch_function_relationships_are_exact() { for family in [RelatedFact::Test, RelatedFact::Branch] { for mutation in [RelationshipMutation::Valid, RelationshipMutation::Missing, RelationshipMutation::Dangling, RelationshipMutation::CrossFile, RelationshipMutation::Span] { assert_eq!(relationship_projection(family, mutation).is_ok(), mutation == RelationshipMutation::Valid, "{family:?} {mutation:?}"); } } }
    #[test] #[rustfmt::skip]
    fn function_row_uniqueness_scales_through_the_sorted_set() {
        let mut db = AnalysisDb::new(); let file = db.add_file("many.go".into(), "many.go".into(), "0123456789\n".into()); let span = span(file, 0, 1);
        for index in 0..4_096 { db.push_function(function_fact(file, format!("f{index}"), span.clone())); }
        let unique = CanonicalGoSyntaxOutput::from_db(&db, &[]).is_ok();
        db.push_function(function_fact(file, "f0".into(), span));
        assert_eq!((unique, CanonicalGoSyntaxOutput::from_db(&db, &[]).is_err()), (true, true));
    }
    /// The backend/grammar labels are hand-written strings. If they drift from
    /// the resolved dependency versions, a grammar bump silently keeps the same
    /// provider identity and layer-cache key, so facts parsed by the previous
    /// grammar are reused as if current.
    #[test]
    fn go_parser_identity_tracks_the_resolved_dependency_versions() {
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
        for (label, crate_name, constant) in [
            ("backend", "tree-sitter", GO_PARSER_BACKEND),
            ("grammar", "tree-sitter-go", GO_PARSER_GRAMMAR),
        ] {
            assert_eq!(
                constant,
                format!("{crate_name}-{}", resolved(crate_name)),
                "the Go parser {label} label drifted from the locked {crate_name} version; \
                 update the constant and add a semantic-store migration for the new CHECK value"
            );
        }
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

    #[test] #[rustfmt::skip]
    fn every_produced_family_and_parser_diagnostic_changes_output_identity() -> Result<(), GoSyntaxProjectionError> {
        let baseline = CanonicalGoSyntaxOutput { families: std::array::from_fn(|index| vec![format!("row-{index}")]) };
        for family in 0..baseline.families.len() { let mut changed = baseline.clone(); changed.families[family].push("additional-row".into()); assert_ne!(changed.digest(), baseline.digest(), "family {family}"); }
        let mut db = AnalysisDb::new(); db.add_file("a.go".into(), "a.go".into(), "package a\n".into());
        let diagnostic = Diagnostic::warning("parser/go", "a.go", TextRange::point(1, 1), "recoverable parse error");
        assert_ne!(CanonicalGoSyntaxOutput::from_db(&db, &[])?.digest(), CanonicalGoSyntaxOutput::from_db(&db, &[diagnostic])?.digest());
        Ok(())
    }
    #[test] #[rustfmt::skip]
    fn string_literal_path_value_span_language_and_multiplicity_are_consumed() {
        let baseline = literal_digest("a.go", "alpha", 1, Language::Go, 1).unwrap();
        for changed in [literal_digest("b.go", "alpha", 1, Language::Go, 1).unwrap(), literal_digest("a.go", "beta", 1, Language::Go, 1).unwrap(), literal_digest("a.go", "alpha", 2, Language::Go, 1).unwrap(), literal_digest("a.go", "alpha", 1, Language::Go, 2).unwrap()] { assert_ne!(changed, baseline); }
        assert!(literal_digest("a.go", "alpha", 1, Language::JavaScript, 1).is_err());
    }
}
