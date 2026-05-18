use crate::analysis_kernel::incremental::{
    CacheStats, Digest, DigestKind, LayerCacheManifest, LayerCacheReadStatus, LayerCacheStore,
    LayerCacheWriteStatus, LayerKey, LayerKind, PrecisionTier,
};
use crate::analysis_plan::AnalysisPlan;
use crate::cache::CacheReadStatus;
use crate::core::{
    AnalysisDb, BranchId, BranchObligation, CachedFileAnalysis, FileId, FunctionFact, FunctionId,
    ImportFact, Language, PackageFact, SourceFile, Span, StringLiteralFact, TestFact,
    span_from_byte_range,
};
use crate::diagnostics::{Diagnostic, TextRange, fingerprint};
use anyhow::{Context, Result};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::sync::Arc;
use tree_sitter::{Node, Parser};

const GO_CACHE_SCHEMA: &str = "go-facts-v2";
const GO_PROVIDER_ID: &str = "polint.go.syntax";
const GO_SYNTAX_LAYER_SCHEMA: &str = "go-syntax-layer-v1";

thread_local! {
    static GO_PARSER: RefCell<Option<Parser>> = const { RefCell::new(None) };
}

/// Convenience wrapper used by Go-adapter unit tests (no cache, sequential).
#[cfg(test)]
pub(crate) fn analyze(db: &mut AnalysisDb) -> Vec<Diagnostic> {
    let cache = crate::cache::Cache::new("", false);
    analyze_with_options(db, &cache, "", "", false)
}

/// Sequential, cache-aware analysis used by Go-adapter unit tests only.
#[cfg(test)]
pub(crate) fn analyze_with_cache(
    db: &mut AnalysisDb,
    cache: &crate::cache::Cache,
    config_hash: &str,
    rule_hash: &str,
) -> Vec<Diagnostic> {
    analyze_with_options(db, cache, config_hash, rule_hash, false)
}

// `pub` for `polint::_bench::go` / `polint-bench`, not for direct downstream use.
#[allow(dead_code, unreachable_pub)]
pub fn analyze_with_options(
    db: &mut AnalysisDb,
    cache: &crate::cache::Cache,
    config_hash: &str,
    rule_hash: &str,
    parallel: bool,
) -> Vec<Diagnostic> {
    let plan = AnalysisPlan::empty();
    analyze_with_plan_options(db, cache, config_hash, rule_hash, &plan, parallel)
}

pub(crate) fn analyze_with_plan_options(
    db: &mut AnalysisDb,
    cache: &crate::cache::Cache,
    config_hash: &str,
    rule_hash: &str,
    plan: &AnalysisPlan,
    parallel: bool,
) -> Vec<Diagnostic> {
    analyze_with_plan_options_and_cache_stats(db, cache, config_hash, rule_hash, plan, parallel)
        .diagnostics
}

#[allow(dead_code)]
pub(crate) struct ProviderAnalysisResult {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

pub(crate) fn analyze_with_plan_options_and_cache_stats(
    db: &mut AnalysisDb,
    cache: &crate::cache::Cache,
    config_hash: &str,
    rule_hash: &str,
    plan: &AnalysisPlan,
    parallel: bool,
) -> ProviderAnalysisResult {
    let files: Vec<&SourceFile> = db
        .files()
        .iter()
        .filter(|file| file.language == Language::Go)
        .collect();

    let mut cache_stats = CacheStats::default();
    if files.is_empty() {
        return ProviderAnalysisResult {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: None,
        };
    }

    let layer_store = LayerCacheStore::new(cache.layer_cache_dir(), cache.is_enabled());
    let layer_key = go_syntax_layer_key(&files, config_hash);
    let read = layer_store.read_json_validated::<SyntaxLayerPayload, _>(
        &layer_key,
        |payload, manifest| {
            validate_syntax_layer_payload(payload, manifest, GO_SYNTAX_LAYER_SCHEMA, &files)
        },
    );

    match read.status {
        LayerCacheReadStatus::Hit => {
            cache_stats.record_hit();
            cache_stats.record_verified_reuse();
            let payload = read
                .value
                .expect("layer cache hit should include syntax payload");
            ProviderAnalysisResult {
                diagnostics: restore_syntax_layer_payload(db, payload),
                cache_stats,
                output_digest: read.output_digest,
            }
        }
        LayerCacheReadStatus::BypassedDisabled => {
            cache_stats.record_disabled_bypass();
            cache_stats.record_recompute();
            let payload = parse_go_syntax_layer_payload(
                &files,
                cache,
                config_hash,
                rule_hash,
                plan,
                parallel,
            );
            ProviderAnalysisResult {
                diagnostics: restore_syntax_layer_payload(db, payload),
                cache_stats,
                output_digest: None,
            }
        }
        LayerCacheReadStatus::Miss | LayerCacheReadStatus::InvalidEvicted => {
            if read.status == LayerCacheReadStatus::Miss {
                cache_stats.record_miss();
            } else {
                cache_stats.record_invalid_evicted_read();
            }
            cache_stats.record_recompute();
            let payload = parse_go_syntax_layer_payload(
                &files,
                cache,
                config_hash,
                rule_hash,
                plan,
                parallel,
            );
            let mut write_diagnostics = Vec::new();
            let output_digest = write_syntax_layer_payload(
                &layer_store,
                layer_key,
                &payload,
                &mut cache_stats,
                &mut write_diagnostics,
            );
            let mut diagnostics = restore_syntax_layer_payload(db, payload);
            diagnostics.extend(write_diagnostics);
            ProviderAnalysisResult {
                diagnostics,
                cache_stats,
                output_digest,
            }
        }
    }
}

struct GoFileAnalysis {
    relative_path: String,
    content_hash: String,
    diagnostics: Vec<Diagnostic>,
    facts: Option<crate::core::CachedFileFacts>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SyntaxLayerPayload {
    schema: String,
    files: Vec<SyntaxLayerFile>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct SyntaxLayerFile {
    relative_path: String,
    content_hash: String,
    diagnostics: Vec<Diagnostic>,
    facts: Option<crate::core::CachedFileFacts>,
}

fn go_syntax_layer_key(files: &[&SourceFile], config_hash: &str) -> LayerKey {
    LayerKey::syntax_layer_key(
        LayerKind::GoSyntax,
        GO_PROVIDER_ID,
        env!("CARGO_PKG_VERSION"),
        GO_CACHE_SCHEMA,
        files.iter().map(|file| source_text_digest(file)).collect(),
        Digest::from_parts(DigestKind::Config, "config_hash", &[config_hash]),
        Digest::absent(DigestKind::GoLifecycle, "go_syntax_lifecycle_absent"),
        Digest::from_parts(
            DigestKind::ToolInvocation,
            "go_syntax_parser_toolchain",
            &[env!("CARGO_PKG_VERSION")],
        ),
        parser_parameter_digest(files),
    )
}

fn source_text_digest(file: &SourceFile) -> Digest {
    Digest::from_parts(
        DigestKind::SourceText,
        "source_file",
        &[file.relative_path.as_str(), file.content_hash.as_str()],
    )
}

fn parser_parameter_digest(files: &[&SourceFile]) -> Digest {
    let mut parts = files
        .iter()
        .map(|file| format!("{}={:?}", file.relative_path, file.language))
        .collect::<Vec<_>>();
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(
        DigestKind::ProviderParameters,
        "go_parser_parameters",
        &refs,
    )
}

fn validate_syntax_layer_payload(
    payload: &SyntaxLayerPayload,
    manifest: &LayerCacheManifest,
    schema: &str,
    files: &[&SourceFile],
) -> bool {
    if payload.schema != schema || payload.files.len() != files.len() {
        return false;
    }
    let mut expected = files
        .iter()
        .map(|file| (file.relative_path.as_str(), file.content_hash.as_str()))
        .collect::<Vec<_>>();
    expected.sort();
    let actual = payload
        .files
        .iter()
        .map(|file| (file.relative_path.as_str(), file.content_hash.as_str()))
        .collect::<Vec<_>>();
    actual == expected && manifest.output_digest == go_syntax_output_digest_for_payload(payload)
}

fn parse_go_syntax_layer_payload(
    files: &[&SourceFile],
    cache: &crate::cache::Cache,
    config_hash: &str,
    rule_hash: &str,
    plan: &AnalysisPlan,
    parallel: bool,
) -> SyntaxLayerPayload {
    let mut results = if parallel {
        files
            .par_iter()
            .map(|file| analyze_go_source_file(file, cache, config_hash, rule_hash, plan.digest()))
            .collect::<Vec<_>>()
    } else {
        files
            .iter()
            .map(|file| analyze_go_source_file(file, cache, config_hash, rule_hash, plan.digest()))
            .collect::<Vec<_>>()
    };
    results.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));

    SyntaxLayerPayload {
        schema: GO_SYNTAX_LAYER_SCHEMA.to_string(),
        files: results
            .into_iter()
            .map(|result| SyntaxLayerFile {
                relative_path: result.relative_path,
                content_hash: result.content_hash,
                diagnostics: result.diagnostics,
                facts: result.facts,
            })
            .collect(),
    }
}

fn restore_syntax_layer_payload(
    db: &mut AnalysisDb,
    payload: SyntaxLayerPayload,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for file in payload.files {
        let Some(file_id) = db
            .files()
            .iter()
            .find(|source| source.relative_path == file.relative_path)
            .map(|source| source.id)
        else {
            continue;
        };
        if let Some(facts) = file.facts {
            db.restore_file_facts(file_id, facts);
        }
        diagnostics.extend(file.diagnostics);
    }
    diagnostics
}

fn write_syntax_layer_payload(
    store: &LayerCacheStore,
    layer_key: LayerKey,
    payload: &SyntaxLayerPayload,
    stats: &mut CacheStats,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Digest> {
    let payload_digest = match LayerCacheStore::payload_digest_for_json(payload) {
        Ok(digest) => digest,
        Err(error) => {
            diagnostics.push(cache_write_diagnostic("go syntax layer", error));
            return None;
        }
    };
    let output_digest = go_syntax_output_digest(&payload_digest);
    let manifest = LayerCacheManifest::new(
        layer_key,
        output_digest.clone(),
        payload_digest,
        Vec::new(),
        PrecisionTier::Syntax,
        "native_trusted",
        Vec::new(),
    );

    match store.write_json(&manifest, payload) {
        Ok(LayerCacheWriteStatus::Written) => stats.record_write(),
        Ok(LayerCacheWriteStatus::BypassedDisabled) => stats.record_disabled_bypass(),
        Err(error) => diagnostics.push(cache_write_diagnostic("go syntax layer", error)),
    }
    Some(output_digest)
}

fn go_syntax_output_digest_for_payload(payload: &SyntaxLayerPayload) -> Digest {
    let payload_digest = LayerCacheStore::payload_digest_for_json(payload)
        .unwrap_or_else(|_| Digest::unsupported(DigestKind::LayerOutput, "go_syntax", "json"));
    go_syntax_output_digest(&payload_digest)
}

fn go_syntax_output_digest(payload_digest: &Digest) -> Digest {
    Digest::from_parts(
        DigestKind::ProviderOutput,
        "go_syntax_layer_output",
        &[&payload_digest.to_string()],
    )
}

fn analyze_go_source_file(
    file: &SourceFile,
    cache: &crate::cache::Cache,
    config_hash: &str,
    rule_hash: &str,
    plan_hash: &str,
) -> GoFileAnalysis {
    let key = crate::cache::CacheKey::for_file(
        file.relative_path.as_str(),
        file.content_hash.as_str(),
        config_hash,
        rule_hash,
        plan_hash,
        GO_CACHE_SCHEMA,
    );
    let read = cache.read_json_with_status::<CachedFileAnalysis>(&key);
    if read.status == CacheReadStatus::Hit
        && let Some(cached) = read.value
        && cached.schema == GO_CACHE_SCHEMA
    {
        return GoFileAnalysis {
            relative_path: file.relative_path.clone(),
            content_hash: file.content_hash.clone(),
            diagnostics: cached.diagnostics,
            facts: Some(cached.facts),
        };
    }

    let mut local_db = AnalysisDb::new();
    let local_file = local_db.add_source_file(
        file.path.clone(),
        file.relative_path.clone(),
        file.language,
        Arc::clone(&file.source),
        file.content_hash.clone(),
    );
    let mut diagnostics = match parse_go_file(&mut local_db, local_file) {
        Ok(file_diagnostics) => file_diagnostics,
        Err(error) => {
            return GoFileAnalysis {
                relative_path: file.relative_path.clone(),
                content_hash: file.content_hash.clone(),
                diagnostics: vec![Diagnostic::error(
                    "parser/go",
                    file.relative_path.clone(),
                    TextRange::point(1, 1),
                    format!("Failed to parse Go file: {error}"),
                )],
                facts: None,
            };
        }
    };
    let facts = local_db.facts_for_file(local_file);
    let cached = CachedFileAnalysis {
        schema: GO_CACHE_SCHEMA.to_string(),
        diagnostics: diagnostics.clone(),
        facts: facts.clone(),
    };
    if let Err(error) = cache.write_json(&key, &cached) {
        diagnostics.push(cache_write_diagnostic(file.relative_path.as_str(), error));
    };
    GoFileAnalysis {
        relative_path: file.relative_path.clone(),
        content_hash: file.content_hash.clone(),
        diagnostics,
        facts: Some(facts),
    }
}

fn cache_write_diagnostic(path: &str, error: anyhow::Error) -> Diagnostic {
    Diagnostic::warning(
        "internal/cache",
        path,
        TextRange::point(1, 1),
        format!("cache write failed: {error}"),
    )
}

fn parse_go_file(db: &mut AnalysisDb, file_id: FileId) -> Result<Vec<Diagnostic>> {
    let source_owned = {
        let file = db.file(file_id).context("missing source file")?;
        Arc::clone(&file.source)
    };
    let source = source_owned.as_ref();

    let tree = GO_PARSER.with(|cell| -> Result<_> {
        let mut slot = cell.borrow_mut();
        let parser = match slot.as_mut() {
            Some(parser) => parser,
            None => {
                let mut parser = Parser::new();
                parser.set_language(&tree_sitter_go::LANGUAGE.into())?;
                slot.insert(parser)
            }
        };
        parser
            .parse(source, None)
            .context("tree-sitter returned no parse tree")
    })?;
    let root = tree.root_node();
    let mut diagnostics = Vec::new();

    if root.has_error() {
        diagnostics.push(parser_error_diagnostic(db, file_id, source, root));
    }

    extract_package(db, file_id, source, root);
    extract_imports(db, file_id, source, root);
    extract_string_literals(db, file_id, source, root);
    extract_functions(db, file_id, source, root);
    Ok(diagnostics)
}

fn parser_error_diagnostic(
    db: &AnalysisDb,
    file_id: FileId,
    source: &str,
    root: Node<'_>,
) -> Diagnostic {
    let range = first_error_node(root)
        .map(|node| node_span(file_id, source, node).diagnostic_range())
        .unwrap_or_else(|| TextRange::point(1, 1));

    Diagnostic::error(
        "parser/go",
        db.path_for(file_id),
        range,
        "Go parser reported a syntax error.",
    )
}

fn extract_package(db: &mut AnalysisDb, file: FileId, source: &str, root: Node<'_>) {
    let Some(package_clause) =
        first_named_descendant(root, &|node| node.kind() == "package_clause")
    else {
        return;
    };

    let Some(identifier) =
        first_named_descendant(package_clause, &|node| node.kind() == "package_identifier")
    else {
        return;
    };

    let Some(name) = node_text(source, identifier) else {
        return;
    };

    db.push_package(PackageFact {
        id: crate::core::PackageId(0),
        file,
        name: name.to_string(),
        span: node_span(file, source, identifier),
        language: Language::Go,
    });
}

fn node_span(file: FileId, source: &str, node: Node<'_>) -> Span {
    span_from_byte_range(file, source, node.start_byte(), node.end_byte())
}

fn node_text<'source>(source: &'source str, node: Node<'_>) -> Option<&'source str> {
    source.get(node.start_byte()..node.end_byte())
}

fn first_error_node(node: Node<'_>) -> Option<Node<'_>> {
    if node.is_error() || node.is_missing() {
        return Some(node);
    }

    for index in 0..node.named_child_count() as u32 {
        let Some(child) = node.named_child(index) else {
            continue;
        };
        if let Some(error) = first_error_node(child) {
            return Some(error);
        }
    }
    None
}

fn first_named_descendant<'tree, F>(node: Node<'tree>, predicate: &F) -> Option<Node<'tree>>
where
    F: Fn(Node<'tree>) -> bool,
{
    if predicate(node) {
        return Some(node);
    }

    walk_named_children(node, |child| {
        if let Some(descendant) = first_named_descendant(child, predicate) {
            return Some(descendant);
        }
        None
    })
}

fn walk_named_children<'tree, F>(node: Node<'tree>, mut visit: F) -> Option<Node<'tree>>
where
    F: FnMut(Node<'tree>) -> Option<Node<'tree>>,
{
    for index in 0..node.named_child_count() as u32 {
        let Some(child) = node.named_child(index) else {
            continue;
        };
        if let Some(found) = visit(child) {
            return Some(found);
        }
    }
    None
}

fn extract_imports(db: &mut AnalysisDb, file: FileId, source: &str, root: Node<'_>) {
    visit_named_descendants(root, &mut |node| {
        if node.kind() != "import_declaration" {
            return;
        }

        let mut specs = Vec::new();
        visit_named_descendants(node, &mut |descendant| {
            if descendant.kind() == "import_spec" {
                specs.push(descendant);
            }
        });

        if specs.is_empty() {
            push_import_from_node(db, file, source, node);
            return;
        }

        for spec in specs {
            push_import_from_node(db, file, source, spec);
        }
    });
}

fn push_import_from_node(db: &mut AnalysisDb, file: FileId, source: &str, node: Node<'_>) {
    let Some(path_node) = first_named_descendant(node, &is_go_string_literal) else {
        return;
    };
    let Some(path) = unquote_go_string_literal(source, path_node) else {
        return;
    };
    let package = import_alias(source, node, path_node);

    db.push_import(ImportFact {
        id: crate::core::ImportId(0),
        file,
        package,
        path,
        span: node_span(file, source, node),
        language: Language::Go,
    });
}

fn import_alias(source: &str, spec: Node<'_>, path: Node<'_>) -> Option<String> {
    let raw = source.get(spec.start_byte()..path.start_byte())?.trim();
    let alias = raw
        .strip_prefix("import")
        .unwrap_or(raw)
        .trim()
        .trim_matches('(')
        .trim();
    if alias.is_empty() {
        None
    } else {
        Some(alias.to_string())
    }
}

fn unquote_go_string_literal(source: &str, node: Node<'_>) -> Option<String> {
    let text = node_text(source, node)?.trim();
    if text.len() < 2 {
        return None;
    }

    let first = text.as_bytes()[0] as char;
    let last = text.as_bytes()[text.len() - 1] as char;
    if matches!((first, last), ('"', '"') | ('`', '`')) {
        Some(text[1..text.len() - 1].to_string())
    } else {
        None
    }
}

fn is_go_string_literal(node: Node<'_>) -> bool {
    matches!(
        node.kind(),
        "interpreted_string_literal" | "raw_string_literal"
    )
}

fn extract_string_literals(db: &mut AnalysisDb, file: FileId, source: &str, root: Node<'_>) {
    visit_named_descendants(root, &mut |node| {
        if !is_go_string_literal(node) || is_inside_go_import(node) {
            return;
        }

        let Some(value) = unquote_go_string_literal(source, node) else {
            return;
        };

        db.push_string_literal(StringLiteralFact {
            file,
            value,
            span: node_span(file, source, node),
            language: Language::Go,
        });
    });
}

fn is_inside_go_import(node: Node<'_>) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(parent.kind(), "import_spec" | "import_declaration") {
            return true;
        }
        current = parent.parent();
    }
    false
}

fn extract_functions(db: &mut AnalysisDb, file: FileId, source: &str, root: Node<'_>) {
    let file_path = db.path_for(file);
    visit_named_descendants(root, &mut |node| {
        if !matches!(node.kind(), "function_declaration" | "method_declaration") {
            return;
        }

        let Some(simple_name) = declaration_name(source, node) else {
            return;
        };
        let body_node = node.child_by_field_name("body");
        let name = if node.kind() == "method_declaration" {
            receiver_type_name(source, node)
                .map(|receiver| format!("{receiver}.{simple_name}"))
                .unwrap_or_else(|| simple_name.clone())
        } else {
            simple_name.clone()
        };
        let span = node_span(file, source, node);
        let is_test = is_go_test_entry(&file_path, &simple_name, node, source);
        let fact = FunctionFact {
            id: FunctionId(0),
            file,
            name: name.clone(),
            span: span.clone(),
            language: Language::Go,
            is_test,
            is_exported: simple_name.chars().next().is_some_and(char::is_uppercase),
            cyclomatic_complexity: body_node
                .map(|body| go_cyclomatic_complexity(source, body))
                .unwrap_or(1),
            calls: body_node
                .map(|body| extract_calls(source, body))
                .unwrap_or_default(),
        };
        let function_id = db.push_function(fact);

        if let Some(body) = body_node {
            extract_branches(
                db,
                file,
                function_id,
                &name,
                function_result_contains_error(source, node),
                source,
                body,
            );
        }
        if is_test && let Some(body) = body_node {
            db.push_test(go_test_fact(file, function_id, name, span, source, body));
        }
    });
}

fn declaration_name(source: &str, node: Node<'_>) -> Option<String> {
    node.child_by_field_name("name")
        .and_then(|name| node_text(source, name))
        .map(str::to_string)
        .or_else(|| {
            first_named_descendant(node, &|child| {
                matches!(child.kind(), "identifier" | "field_identifier")
            })
            .and_then(|name| node_text(source, name))
            .map(str::to_string)
        })
}

fn receiver_type_name(source: &str, node: Node<'_>) -> Option<String> {
    let receiver = node.child_by_field_name("receiver")?;
    let receiver_text = node_text(source, receiver)?;
    let inner = receiver_text
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .trim();
    let raw_type = inner
        .split_whitespace()
        .last()
        .unwrap_or(inner)
        .trim_start_matches('*')
        .trim_start_matches('&');
    let without_package = raw_type.rsplit('.').next().unwrap_or(raw_type);
    let without_generics = without_package
        .split_once('[')
        .map(|(name, _)| name)
        .unwrap_or(without_package);

    (!without_generics.is_empty()).then(|| without_generics.to_string())
}

fn is_go_test_entry(path: &str, name: &str, node: Node<'_>, source: &str) -> bool {
    if !path.ends_with("_test.go") || node.kind() != "function_declaration" {
        return false;
    }

    let expected_parameter = if name.starts_with("Test") {
        "*testing.T"
    } else if name.starts_with("Benchmark") {
        "*testing.B"
    } else if name.starts_with("Fuzz") {
        "*testing.F"
    } else {
        return false;
    };

    let Some(parameters) = node
        .child_by_field_name("parameters")
        .or_else(|| first_named_child(node, "parameter_list"))
    else {
        return false;
    };
    let normalized = node_text(source, parameters)
        .unwrap_or_default()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    let Some(parameters) = normalized
        .strip_prefix('(')
        .and_then(|parameters| parameters.strip_suffix(')'))
    else {
        return false;
    };

    if parameters.contains(',') {
        return false;
    }

    parameters == expected_parameter
        || parameters
            .strip_suffix(expected_parameter)
            .is_some_and(|name| {
                !name.is_empty()
                    && name
                        .chars()
                        .all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
            })
}

fn extract_calls(source: &str, body: Node<'_>) -> Vec<String> {
    let mut calls = Vec::new();
    visit_named_descendants(body, &mut |node| {
        if node.kind() != "call_expression" {
            return;
        }
        let Some(call) = call_callee(source, node) else {
            return;
        };
        calls.push(call);
    });
    calls.sort();
    calls.dedup();
    calls
}

fn call_callee(source: &str, node: Node<'_>) -> Option<String> {
    let function_node = node
        .child_by_field_name("function")
        .or_else(|| node.named_child(0))?;
    stable_call_name(source, function_node)
}

fn stable_call_name(source: &str, node: Node<'_>) -> Option<String> {
    let text = node_text(source, node)?
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("");
    if text.is_empty() || text.starts_with("func") {
        None
    } else {
        Some(text)
    }
}

fn go_test_fact(
    file: FileId,
    function: FunctionId,
    name: String,
    span: Span,
    source: &str,
    body: Node<'_>,
) -> TestFact {
    TestFact {
        file,
        function: Some(function),
        name,
        span,
        evidence_terms: go_test_evidence_terms(source, body),
        assertion_count: go_assertion_count(source, body),
        subtest_count: go_subtest_count(source, body),
        subtest_names: go_subtest_literal_names(source, body),
        table_rows: go_table_rows(source, body),
    }
}

fn go_subtest_count(source: &str, body: Node<'_>) -> u32 {
    let mut count = 0;
    visit_named_descendants(body, &mut |node| {
        if node.kind() == "call_expression" && call_callee(source, node).as_deref() == Some("t.Run")
        {
            count += 1;
        }
    });
    count
}

fn go_subtest_literal_names(source: &str, body: Node<'_>) -> Vec<String> {
    let mut names = Vec::new();
    visit_named_descendants(body, &mut |node| {
        if node.kind() != "call_expression" {
            return;
        }
        if call_callee(source, node).as_deref() != Some("t.Run") {
            return;
        }
        let Some(args) = node.child_by_field_name("arguments") else {
            return;
        };
        let Some(first) = args.named_child(0) else {
            return;
        };
        if let Some(name) = unquote_go_string_literal(source, first) {
            names.push(name);
        }
    });
    names
}

fn go_assertion_count(source: &str, body: Node<'_>) -> u32 {
    let mut count = 0;
    visit_named_descendants(body, &mut |node| match node.kind() {
        "call_expression"
            if call_callee(source, node).is_some_and(|callee| is_go_assertion_callee(&callee)) =>
        {
            count += 1;
        }
        "if_statement" if is_simple_assertion_condition(source, node) => {
            count += 1;
        }
        _ => {}
    });
    count
}

fn is_go_assertion_callee(callee: &str) -> bool {
    matches!(callee, "t.Fatal" | "t.Fatalf" | "t.Error" | "t.Errorf")
        || callee.starts_with("require.")
        || callee.starts_with("assert.")
}

fn is_simple_assertion_condition(source: &str, node: Node<'_>) -> bool {
    let Some(condition) = if_condition_text(source, node) else {
        return false;
    };
    let normalized = condition
        .trim()
        .trim_matches('(')
        .trim_matches(')')
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();

    matches!(normalized.as_str(), "err==nil" | "err!=nil" | "got!=want")
}

fn if_condition_text<'source>(source: &'source str, node: Node<'_>) -> Option<&'source str> {
    if let Some(condition) = node.child_by_field_name("condition") {
        return node_text(source, condition);
    }

    let text = node_text(source, node)?.trim();
    let condition = text.strip_prefix("if")?.split('{').next()?.trim();
    condition.rsplit(';').next().map(str::trim)
}

fn go_table_rows(source: &str, body: Node<'_>) -> u32 {
    let mut rows = std::collections::BTreeSet::new();
    visit_named_descendants(body, &mut |node| {
        if node.kind() != "literal_value" || !is_anonymous_struct_table(source, node) {
            return;
        }

        for index in 0..node.named_child_count() as u32 {
            let Some(row) = node.named_child(index) else {
                continue;
            };
            let Some(text) = node_text(source, row).map(str::trim) else {
                continue;
            };
            if matches!(
                row.kind(),
                "literal_element" | "keyed_element" | "literal_value"
            ) && text.starts_with('{')
                && text.ends_with('}')
                && text.contains(':')
            {
                rows.insert((row.start_byte(), row.end_byte()));
            }
        }
    });
    rows.len() as u32
}

fn is_anonymous_struct_table(source: &str, literal_value: Node<'_>) -> bool {
    let Some(parent) = literal_value
        .parent()
        .filter(|parent| parent.kind() == "composite_literal")
    else {
        return false;
    };
    let Some(type_text) = source.get(parent.start_byte()..literal_value.start_byte()) else {
        return false;
    };
    let compact = type_text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    compact.contains("[]struct") || compact.contains("[...]struct")
}

fn go_test_evidence_terms(source: &str, body: Node<'_>) -> Vec<String> {
    let mut terms = std::collections::BTreeSet::new();
    visit_named_descendants(body, &mut |node| match node.kind() {
        "identifier" | "field_identifier" | "package_identifier" => {
            if let Some(text) = node_text(source, node) {
                add_identifier_evidence(&mut terms, text);
            }
        }
        "interpreted_string_literal" | "raw_string_literal" => {
            if let Some(text) = unquote_go_string_literal(source, node) {
                add_string_evidence(&mut terms, source, node, &text);
            }
        }
        "nil" => {
            terms.insert("nil".to_string());
        }
        _ => {}
    });

    visit_named_descendants(body, &mut |node| {
        if node.kind() == "if_statement"
            && if_condition_text(source, node).is_some_and(|condition| condition.contains("nil"))
        {
            terms.insert("nil".to_string());
        }
    });

    terms.into_iter().collect()
}

fn add_identifier_evidence(terms: &mut std::collections::BTreeSet<String>, text: &str) {
    let lower = text.to_ascii_lowercase();
    if is_go_evidence_word(&lower) {
        terms.insert(lower);
    }
}

fn add_string_evidence(
    terms: &mut std::collections::BTreeSet<String>,
    source: &str,
    node: Node<'_>,
    text: &str,
) {
    let words = evidence_words(text);
    let is_case_or_subtest_name = string_literal_looks_like_test_case_name(source, node);
    let has_marker = words.iter().any(|word| is_go_evidence_word(word));
    for word in words {
        if is_go_evidence_word(&word)
            || (is_case_or_subtest_name && !has_marker && !word.contains('_'))
        {
            terms.insert(word);
        }
    }
}

fn evidence_words(text: &str) -> Vec<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_')
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn is_go_evidence_word(word: &str) -> bool {
    matches!(
        word,
        "allowed" | "denied" | "err" | "error" | "fail" | "invalid" | "nil"
    )
}

fn string_literal_looks_like_test_case_name(source: &str, node: Node<'_>) -> bool {
    let line_start = source[..node.start_byte()]
        .rfind('\n')
        .map(|index| index + 1)
        .unwrap_or(0);
    let prefix = &source[line_start..node.start_byte()];
    prefix.contains("name:") || prefix.contains("t.Run(")
}

fn go_cyclomatic_complexity(_source: &str, body: Node<'_>) -> u32 {
    let mut complexity = 1;
    visit_named_descendants(body, &mut |node| match node.kind() {
        "if_statement" | "for_statement" => complexity += 1,
        "expression_case" | "type_case" | "communication_case" | "default_case" => {
            complexity += 1;
        }
        "binary_expression" => complexity += boolean_operator_count(node),
        _ => {}
    });
    complexity
}

fn boolean_operator_count(node: Node<'_>) -> u32 {
    let mut count = 0;
    for index in 0..node.child_count() as u32 {
        let Some(child) = node.child(index) else {
            continue;
        };
        if matches!(child.kind(), "&&" | "||") {
            count += 1;
        }
    }
    count
}

fn visit_named_descendants<'tree, F>(node: Node<'tree>, visit: &mut F)
where
    F: FnMut(Node<'tree>),
{
    visit(node);
    for index in 0..node.named_child_count() as u32 {
        let Some(child) = node.named_child(index) else {
            continue;
        };
        visit_named_descendants(child, visit);
    }
}

fn first_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    for index in 0..node.named_child_count() as u32 {
        let Some(child) = node.named_child(index) else {
            continue;
        };
        if child.kind() == kind {
            return Some(child);
        }
    }
    None
}

fn extract_branches(
    db: &mut AnalysisDb,
    file: FileId,
    function: FunctionId,
    function_name: &str,
    function_returns_error: bool,
    source: &str,
    body: Node<'_>,
) {
    visit_named_descendants(body, &mut |node| match node.kind() {
        "if_statement" => push_if_branches(
            db,
            file,
            function,
            function_name,
            function_returns_error,
            source,
            node,
        ),
        "expression_switch_statement" | "type_switch_statement" => {
            push_switch_branch(db, file, function, function_name, source, node);
        }
        "expression_case" | "type_case" | "communication_case" => push_case_branch(
            db,
            BranchTarget {
                file,
                function,
                function_name,
            },
            function_returns_error,
            source,
            node,
            "case",
        ),
        "default_case" => push_case_branch(
            db,
            BranchTarget {
                file,
                function,
                function_name,
            },
            function_returns_error,
            source,
            node,
            "default",
        ),
        "for_statement" => push_for_branch(
            db,
            file,
            function,
            function_name,
            function_returns_error,
            source,
            node,
        ),
        "select_statement" => push_select_branch(db, file, function, function_name, source, node),
        _ => {}
    });
}

fn push_if_branches(
    db: &mut AnalysisDb,
    file: FileId,
    function: FunctionId,
    function_name: &str,
    function_returns_error: bool,
    source: &str,
    node: Node<'_>,
) {
    let decision = node.child_by_field_name("condition").unwrap_or(node);
    let condition = if_condition_text(source, node)
        .or_else(|| node_text(source, decision))
        .unwrap_or("if")
        .trim()
        .to_string();
    let span = node_span(file, source, decision);
    let true_body = node
        .child_by_field_name("consequence")
        .or_else(|| node.child_by_field_name("body"));
    let false_body = node.child_by_field_name("alternative");
    let true_is_error_path = branch_body_returns_error(source, true_body, function_returns_error)
        || condition_implies_error_edge(&condition, "true");
    let false_is_error_path = branch_body_returns_error(source, false_body, function_returns_error)
        || condition_implies_error_edge(&condition, "false");
    push_branch(
        db,
        BranchTarget {
            file,
            function,
            function_name,
        },
        span.clone(),
        condition.clone(),
        "true",
        true_is_error_path,
    );
    push_branch(
        db,
        BranchTarget {
            file,
            function,
            function_name,
        },
        span,
        condition,
        "false",
        false_is_error_path,
    );
}

fn push_switch_branch(
    db: &mut AnalysisDb,
    file: FileId,
    function: FunctionId,
    function_name: &str,
    source: &str,
    node: Node<'_>,
) {
    let Some((start, end)) = switch_decision_range(source, node) else {
        return;
    };
    let condition = trimmed_text(source, start, end)
        .unwrap_or("switch")
        .to_string();
    push_branch(
        db,
        BranchTarget {
            file,
            function,
            function_name,
        },
        trimmed_span(file, source, start, end),
        condition.clone(),
        "switch",
        is_go_error_path_heuristic(source, &condition, Some(node), false),
    );
}

fn push_case_branch(
    db: &mut AnalysisDb,
    target: BranchTarget<'_>,
    function_returns_error: bool,
    source: &str,
    node: Node<'_>,
    edge_label: &str,
) {
    let (start, end) = if edge_label == "default" {
        (node.start_byte(), node.end_byte())
    } else {
        case_header_range(source, node)
    };
    let condition = if edge_label == "default" {
        "default".to_string()
    } else {
        trimmed_text(source, start, end)
            .unwrap_or("case")
            .to_string()
    };
    push_branch(
        db,
        target,
        trimmed_span(target.file, source, start, end),
        condition.clone(),
        edge_label,
        is_go_error_path_heuristic(source, &condition, Some(node), function_returns_error),
    );
}

fn push_for_branch(
    db: &mut AnalysisDb,
    file: FileId,
    function: FunctionId,
    function_name: &str,
    function_returns_error: bool,
    source: &str,
    node: Node<'_>,
) {
    if let Some(range) = direct_range_clause(node) {
        let condition = node_text(source, range)
            .unwrap_or("range")
            .trim()
            .to_string();
        push_branch(
            db,
            BranchTarget {
                file,
                function,
                function_name,
            },
            node_span(file, source, range),
            condition.clone(),
            "range",
            is_go_error_path_heuristic(source, &condition, Some(node), function_returns_error),
        );
        return;
    }

    let (start, end) = statement_header_range(source, node);
    let condition = trimmed_text(source, start, end)
        .unwrap_or("for")
        .to_string();
    push_branch(
        db,
        BranchTarget {
            file,
            function,
            function_name,
        },
        trimmed_span(file, source, start, end),
        condition.clone(),
        "loop",
        is_go_error_path_heuristic(source, &condition, Some(node), function_returns_error),
    );
}

fn direct_range_clause(node: Node<'_>) -> Option<Node<'_>> {
    node.child_by_field_name("clause")
        .filter(|child| child.kind() == "range_clause")
        .or_else(|| first_named_child(node, "range_clause"))
}

fn push_select_branch(
    db: &mut AnalysisDb,
    file: FileId,
    function: FunctionId,
    function_name: &str,
    source: &str,
    node: Node<'_>,
) {
    let end = node.start_byte().saturating_add("select".len());
    push_branch(
        db,
        BranchTarget {
            file,
            function,
            function_name,
        },
        span_from_byte_range(file, source, node.start_byte(), end),
        "select".to_string(),
        "select",
        false,
    );
}

fn switch_decision_range(source: &str, node: Node<'_>) -> Option<(usize, usize)> {
    if node.kind() == "expression_switch_statement"
        && let Some(value) = node.child_by_field_name("value")
    {
        return Some((value.start_byte(), value.end_byte()));
    }

    let (start, end) = statement_header_range(source, node);
    let header = source.get(start..end)?;
    let switch_offset = header.find("switch")? + "switch".len();
    let after_switch = start + switch_offset;
    let rest = source.get(after_switch..end)?;
    let leading = rest.len() - rest.trim_start().len();
    let decision_start = after_switch + leading;
    let trimmed_rest = source.get(decision_start..end)?.trim_end();
    if trimmed_rest.is_empty() {
        return Some((start, end));
    }
    Some((decision_start, decision_start + trimmed_rest.len()))
}

fn case_header_range(source: &str, node: Node<'_>) -> (usize, usize) {
    if let Some(case_text) = source.get(node.start_byte()..node.end_byte())
        && let Some(colon_offset) = case_delimiter_colon(case_text)
    {
        return (node.start_byte(), node.start_byte() + colon_offset + 1);
    }

    let Some(after_node) = source.get(node.end_byte()..node.end_byte().saturating_add(16)) else {
        return (node.start_byte(), node.end_byte());
    };
    let colon_offset = after_node.find(':').filter(|offset| {
        after_node[..*offset]
            .chars()
            .all(|character| character.is_whitespace())
    });

    let end = colon_offset
        .map(|offset| node.end_byte() + offset + 1)
        .unwrap_or_else(|| node.end_byte());
    (node.start_byte(), end)
}

fn case_delimiter_colon(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut paren_depth = 0_u32;
    let mut bracket_depth = 0_u32;
    let mut brace_depth = 0_u32;

    while index < bytes.len() {
        match bytes[index] {
            b'"' | b'\'' => index = skip_quoted_literal(bytes, index)?,
            b'`' => index = skip_raw_string_literal(bytes, index)?,
            b'(' => paren_depth += 1,
            b')' => paren_depth = paren_depth.saturating_sub(1),
            b'[' => bracket_depth += 1,
            b']' => bracket_depth = bracket_depth.saturating_sub(1),
            b'{' => brace_depth += 1,
            b'}' => brace_depth = brace_depth.saturating_sub(1),
            b':' if paren_depth == 0
                && bracket_depth == 0
                && brace_depth == 0
                && bytes.get(index + 1) != Some(&b'=') =>
            {
                return Some(index);
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn skip_quoted_literal(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = *bytes.get(start)?;
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            byte if byte == quote => return Some(index),
            _ => index += 1,
        }
    }
    None
}

fn skip_raw_string_literal(bytes: &[u8], start: usize) -> Option<usize> {
    let mut index = start + 1;
    while index < bytes.len() {
        if bytes[index] == b'`' {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn statement_header_range(source: &str, node: Node<'_>) -> (usize, usize) {
    let body_start = node
        .child_by_field_name("body")
        .map(|body| body.start_byte())
        .or_else(|| {
            source
                .get(node.start_byte()..node.end_byte())
                .and_then(|text| text.find('{'))
                .map(|offset| node.start_byte() + offset)
        })
        .unwrap_or_else(|| node.end_byte());

    (node.start_byte(), body_start)
}

fn trimmed_text(source: &str, start: usize, end: usize) -> Option<&str> {
    source.get(start..end).map(str::trim)
}

fn trimmed_span(file: FileId, source: &str, start: usize, end: usize) -> Span {
    let Some(text) = source.get(start..end) else {
        return span_from_byte_range(file, source, start, end);
    };
    let leading = text.len() - text.trim_start().len();
    let trailing = text.len() - text.trim_end().len();
    span_from_byte_range(file, source, start + leading, end - trailing)
}

#[derive(Clone, Copy)]
struct BranchTarget<'name> {
    file: FileId,
    function: FunctionId,
    function_name: &'name str,
}

fn push_branch(
    db: &mut AnalysisDb,
    target: BranchTarget<'_>,
    decision_span: Span,
    condition_text: String,
    edge_label: &str,
    is_error_path: bool,
) -> BranchId {
    let start_line = decision_span.start_line.to_string();
    let start_col = decision_span.start_col.to_string();
    let normalized_condition = normalize_branch_condition(&condition_text);
    let stable_fingerprint = fingerprint(&[
        &db.path_for(target.file),
        target.function_name,
        &start_line,
        &start_col,
        &normalized_condition,
        edge_label,
    ]);
    db.push_branch(BranchObligation {
        id: BranchId(0),
        function: Some(target.function),
        file: target.file,
        decision_span,
        condition_text,
        edge_label: edge_label.to_string(),
        is_error_path,
        stable_fingerprint,
    })
}

fn normalize_branch_condition(condition_text: &str) -> String {
    condition_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn function_result_contains_error(source: &str, node: Node<'_>) -> bool {
    if node
        .child_by_field_name("result")
        .and_then(|result| node_text(source, result))
        .is_some_and(contains_error_word)
    {
        return true;
    }

    let (start, end) = statement_header_range(source, node);
    let Some(header) = trimmed_text(source, start, end) else {
        return false;
    };
    header
        .rsplit_once(')')
        .is_some_and(|(_, result)| contains_error_word(result))
}

// Syntax-only heuristic: this flags obvious Go error branches, not exact path coverage.
fn is_go_error_path_heuristic(
    source: &str,
    condition_text: &str,
    branch_node: Option<Node<'_>>,
    function_returns_error: bool,
) -> bool {
    condition_implies_error_edge(condition_text, "true")
        || branch_body_returns_error(source, branch_node, function_returns_error)
}

fn condition_implies_error_edge(condition_text: &str, edge_label: &str) -> bool {
    let compact = condition_text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    match edge_label {
        "true" => {
            compact.contains("err!=nil")
                || compact.contains("errors.is(")
                || compact.contains("errors.as(")
                || condition_text.to_ascii_lowercase().contains("error")
        }
        "false" => compact.contains("err==nil"),
        _ => false,
    }
}

fn branch_body_returns_error(
    source: &str,
    branch_node: Option<Node<'_>>,
    function_returns_error: bool,
) -> bool {
    function_returns_error
        && branch_node.is_some_and(|node| subtree_returns_error_looking_value(source, node))
}

fn subtree_returns_error_looking_value(source: &str, node: Node<'_>) -> bool {
    let mut returns_error = false;
    visit_named_descendants(node, &mut |descendant| {
        if returns_error || descendant.kind() != "return_statement" {
            return;
        }
        returns_error = return_statement_looks_error(source, descendant);
    });
    returns_error
}

fn return_statement_looks_error(source: &str, node: Node<'_>) -> bool {
    let Some(returned) = node_text(source, node)
        .map(str::trim)
        .and_then(|text| text.strip_prefix("return"))
        .map(str::trim)
    else {
        return false;
    };

    returned
        .split(',')
        .map(str::trim)
        .any(error_value_looks_error)
}

fn error_value_looks_error(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let trimmed = lower.trim_matches(|character: char| !character.is_ascii_alphanumeric());
    !matches!(trimmed, "" | "nil" | "true" | "false")
        && (trimmed == "err"
            || trimmed.starts_with("err")
            || trimmed.contains("error")
            || trimmed.contains("errors.")
            || trimmed.contains("fmt.errorf"))
}

fn contains_error_word(text: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|word| word == "error")
}

#[cfg(test)]
mod subtest_name_tests {
    use super::*;
    use crate::core::AnalysisDb;

    #[test]
    fn harvests_literal_t_run_names() {
        let src = r#"
package foo

import "testing"

func TestFoo(t *testing.T) {
    t.Run("case_a", func(t *testing.T) {})
    t.Run(`case_b`, func(t *testing.T) {})
}
"#;
        let mut db = AnalysisDb::new();
        let fid = db.add_file(
            std::path::PathBuf::from("x_test.go"),
            "x_test.go".to_string(),
            src.to_string(),
        );
        let _ = parse_go_file(&mut db, fid).unwrap();
        let names: Vec<_> = db.tests()[0].subtest_names.clone();
        assert_eq!(names, vec!["case_a".to_string(), "case_b".to_string()]);
        assert_eq!(db.tests()[0].subtest_count, 2);
    }
}
