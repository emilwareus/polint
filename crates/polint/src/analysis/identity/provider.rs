use std::collections::BTreeSet;
use std::sync::Arc;

use crate::analysis::calls::facts::CallSiteFact;
use crate::analysis::identity::cache_key::identity_provider_parameter_digest;
use crate::analysis::identity::dedup::dedup_identity_records;
use crate::analysis::identity::facts::{
    IdentityKind, IdentityRecord, IdentityRecordId, LanguageTag, compute_identity_stable_key,
    compute_signature_digest,
};
use crate::analysis::identity::store::IdentityProviderOutput;
use crate::analysis::ids::CallSiteId;
use crate::analysis_kernel::ProviderManifest;
use crate::analysis_kernel::incremental::{CacheStats, Digest, DigestKind, InputSnapshot};
use crate::core::{AnalysisDb, FunctionFact, Language, Span};
use crate::diagnostics::{Diagnostic, TextRange};

#[derive(Debug, Clone, Default)]
pub(crate) struct IdentityProviderRunOutput {
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) cache_stats: CacheStats,
    pub(crate) output_digest: Option<Digest>,
}

/// Identity provider entry point (Pattern E).
///
/// Five-phase pipeline: extract identity records by projecting existing
/// `analysis::calls` and function facts (no mutation, D-04) -> dedup (D-09) ->
/// assign dense IDs after sort+dedup -> normalize -> compute output digest over
/// stable payloads (Pattern F) and replace identity facts.
#[allow(clippy::too_many_arguments)]
pub(crate) fn derive_identity_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    calls_provider_output_digest: Digest,
) -> IdentityProviderRunOutput {
    // Phase 1: extract.
    let mut records = extract_identity_records(db);
    // Phase 2: dedup.
    records = dedup_identity_records(records);
    // Phase 3: assign dense IDs after sort+dedup.
    for (index, record) in records.iter_mut().enumerate() {
        record.id = IdentityRecordId(index as u64);
    }
    // Phase 4: normalize (dedup already sorts, but normalize keeps the contract
    // single-sourced through IdentityProviderOutput).
    let output = IdentityProviderOutput { records }.normalized();
    // Phase 5: digest.
    let output_digest = identity_output_digest(
        manifest,
        input_snapshot,
        &calls_provider_output_digest,
        &output,
    );

    let mut cache_stats = CacheStats::default();
    cache_stats.record_recompute();

    match db.replace_identity_facts(output) {
        Ok(()) => IdentityProviderRunOutput {
            diagnostics: Vec::new(),
            cache_stats,
            output_digest: Some(output_digest),
        },
        Err(error) => IdentityProviderRunOutput {
            diagnostics: vec![provider_error_diagnostic(error.to_string())],
            cache_stats,
            output_digest: Some(output_digest),
        },
    }
}

/// Projects existing function and call-site facts into identity records.
fn extract_identity_records(db: &AnalysisDb) -> Vec<IdentityRecord> {
    let mut records = Vec::new();

    for function in db.functions() {
        if let Some(record) = function_identity_record(db, function) {
            records.push(record);
        }
    }

    for site in db.call_sites() {
        if let Some(record) = callsite_identity_record(db, site) {
            records.push(record);
        }
    }

    records
}

fn function_identity_record(db: &AnalysisDb, function: &FunctionFact) -> Option<IdentityRecord> {
    let language = language_tag(function.language)?;
    let package_or_module: Arc<str> = Arc::from(package_or_module_for_record(
        db,
        function.language,
        function.file,
    ));
    let container_path: Arc<str> = Arc::from(function.name.as_str());
    let display_name: Arc<str> = Arc::from(function.name.as_str());
    Some(build_record(
        IdentityKind::Function,
        function.file,
        &function.span,
        language,
        package_or_module,
        container_path,
        display_name,
        None,
    ))
}

fn callsite_identity_record(db: &AnalysisDb, site: &CallSiteFact) -> Option<IdentityRecord> {
    let language = language_tag(site.language)?;
    let package_or_module: Arc<str> =
        Arc::from(package_or_module_for_record(db, site.language, site.file));
    let container_path: Arc<str> = Arc::from(callsite_container_path(db, site));
    let display_name: Arc<str> = Arc::from(callsite_display_name(site));
    Some(build_record(
        IdentityKind::Callsite,
        site.file,
        &site.span,
        language,
        package_or_module,
        container_path,
        display_name,
        Some(site.id),
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_record(
    kind: IdentityKind,
    file: crate::core::FileId,
    span: &Span,
    language: LanguageTag,
    package_or_module: Arc<str>,
    container_path: Arc<str>,
    display_name: Arc<str>,
    originating_call_site_id: Option<CallSiteId>,
) -> IdentityRecord {
    let signature_digest = compute_signature_digest(
        language,
        &package_or_module,
        &container_path,
        &display_name,
        None,
        None,
    );
    let stable_key = compute_identity_stable_key(
        kind,
        language,
        &package_or_module,
        &container_path,
        file,
        span,
    );
    IdentityRecord {
        id: IdentityRecordId(0),
        kind,
        file_id: file,
        span: span.clone(),
        language,
        package_or_module,
        container_path,
        display_name,
        signature_digest,
        multiplicity: 1,
        stable_key,
        originating_call_site_id,
        originating_call_target_id: None,
    }
}

fn callsite_container_path(db: &AnalysisDb, site: &CallSiteFact) -> String {
    // The container is the enclosing caller function. Fall back to a file-scoped
    // container when the caller cannot be resolved by name.
    db.functions()
        .iter()
        .find(|function| function.id == site.caller)
        .map(|function| function.name.clone())
        .unwrap_or_else(|| format!("{}#<anon>", package_or_module_for_file(db, site.file)))
}

fn callsite_display_name(site: &CallSiteFact) -> String {
    use crate::analysis::calls::facts::CallCallee;
    match &site.callee {
        CallCallee::Identifier { name, .. } => name.clone(),
        CallCallee::Member { property, .. } => property.clone(),
        CallCallee::Constructor { name, .. } => {
            name.clone().unwrap_or_else(|| "<ctor>".to_string())
        }
        CallCallee::Index { .. } => "<index>".to_string(),
        CallCallee::Super => "super".to_string(),
        CallCallee::Import => "import".to_string(),
        CallCallee::FunctionValue { .. } => "<function-value>".to_string(),
        CallCallee::Unknown { .. } => "<unknown>".to_string(),
    }
}

/// Resolves the `package_or_module` string for a record's language and file.
///
/// For `Language::Go` records this prefers the Phase 46 semantic frontend's full
/// Go package import path when a validated package row covers the file. It falls
/// back to the Go package-clause name, then the workspace-relative file path, so
/// missing semantic setup preserves the earlier panic-free behavior.
fn package_or_module_for_record(
    db: &AnalysisDb,
    language: Language,
    file: crate::core::FileId,
) -> String {
    match language {
        Language::Go => semantic_package_path_for_go_file(db, file)
            .or_else(|| package_name_for_go_file(db, file))
            .unwrap_or_else(|| package_or_module_for_file(db, file)),
        _ => package_or_module_for_file(db, file),
    }
}

fn semantic_package_path_for_go_file(db: &AnalysisDb, file: crate::core::FileId) -> Option<String> {
    let path = db.path_for(file);
    db.go_semantic_packages()
        .iter()
        .find(|package| {
            package
                .files
                .iter()
                .any(|package_file| package_file == &path)
        })
        .map(|package| package.package_path.clone())
        .filter(|package_path| !package_path.is_empty())
}

/// Returns the Go package-clause name (e.g. `main` / `foo`) for a file by
/// scanning `db.packages()` for the first Go [`PackageFact`] on that file.
fn package_name_for_go_file(db: &AnalysisDb, file: crate::core::FileId) -> Option<String> {
    db.packages()
        .iter()
        .find(|package| package.file == file && package.language == Language::Go)
        .map(|package| package.name.clone())
}

fn package_or_module_for_file(db: &AnalysisDb, file: crate::core::FileId) -> String {
    db.path_for(file)
}

fn language_tag(language: Language) -> Option<LanguageTag> {
    match language {
        Language::Go => Some(LanguageTag::Go),
        Language::TypeScript | Language::Tsx => Some(LanguageTag::TypeScript),
        Language::JavaScript | Language::Jsx => Some(LanguageTag::JavaScript),
        Language::Unknown => None,
    }
}

/// Output digest over stable payloads, never dense IDs (Pattern F, T-42-02).
fn identity_output_digest(
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    calls_provider_output_digest: &Digest,
    output: &IdentityProviderOutput,
) -> Digest {
    let mut parts = vec![
        format!("provider_id={}", manifest.id),
        format!("provider_version={}", manifest.provider_version()),
        format!("schema={}", manifest.primary_schema_label()),
        format!("parameters={}", identity_provider_parameter_digest()),
        format!("config={}", input_snapshot.config.digest),
        format!("calls_output={calls_provider_output_digest}"),
    ];
    parts.extend(output.records.iter().map(|record| {
        format!(
            "identity_record={} language={} package_or_module={} container={} digest={} kind={:?} multiplicity={}",
            record.stable_key,
            record.language.as_str(),
            record.package_or_module,
            record.container_path,
            encode_digest_hex(record.signature_digest.0),
            record.kind,
            record.multiplicity,
        )
    }));
    if output.records.is_empty() {
        parts.push("identity_output=empty".to_string());
    }
    parts.sort();
    let refs = parts.iter().map(String::as_str).collect::<Vec<_>>();
    Digest::from_parts(DigestKind::ProviderOutput, "identity_output", &refs)
}

fn encode_digest_hex(bytes: [u8; 16]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(32);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn provider_error_diagnostic(message: String) -> Diagnostic {
    Diagnostic::error(
        "polint/internal",
        "<workspace>",
        TextRange::point(1, 1),
        format!("Identity provider failed: {message}"),
    )
}

/// Valid call-site ID set for store validation by the kernel caller.
pub(crate) fn valid_call_site_ids(db: &AnalysisDb) -> BTreeSet<CallSiteId> {
    db.call_sites().iter().map(|site| site.id).collect()
}

#[cfg(test)]
pub(crate) fn identity_output_digest_for_test(parts: &[&str]) -> Digest {
    Digest::from_parts(DigestKind::ProviderOutput, "identity_output", parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::identity::facts::{
        IdentityKind, IdentityRecordId, compute_signature_digest,
    };
    use crate::analysis::identity::store::IdentityProviderOutput;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind, InputSnapshot};
    use crate::analysis_plan::AnalysisPlan;
    use crate::config::load_config;
    use crate::core::{
        AnalysisDb, FileId, FunctionFact, FunctionId, Language, PackageFact, PackageId, Span,
    };
    use crate::go::semantic::facts::{GoSemanticPackageFact, GoSemanticPackageId};
    use crate::go::semantic::store::GoSemanticFactsOutput;
    use std::fs;
    use std::sync::Arc;
    use tempfile::tempdir;

    /// Builds a single-file db with one Go function and (optionally) a matching
    /// Go `PackageFact`, returning the function's identity record straight from
    /// the real provider builder (`function_identity_record`).
    fn go_function_record(package_name: Option<&str>) -> IdentityRecord {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            std::path::PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package foo\nfunc Bar() {}\n".to_string(),
        );
        if let Some(name) = package_name {
            db.push_package(PackageFact {
                id: PackageId(0),
                file,
                name: name.to_string(),
                span: Span::point(file, 1, 1),
                language: Language::Go,
            });
        }
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "Bar".to_string(),
            span: Span::point(file, 2, 1),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        let function = db.functions().first().expect("pushed function").clone();
        super::function_identity_record(&db, &function).expect("Go function builds a record")
    }

    #[test]
    fn go_function_with_package_resolves_package_name() {
        let record = go_function_record(Some("foo"));
        assert_eq!(record.package_or_module.as_ref(), "foo");
    }

    #[test]
    fn go_function_prefers_semantic_package_import_path() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            std::path::PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package foo\nfunc Bar() {}\n".to_string(),
        );
        db.push_package(PackageFact {
            id: PackageId(0),
            file,
            name: "foo".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::Go,
        });
        db.replace_go_semantic_facts(GoSemanticFactsOutput {
            packages: vec![GoSemanticPackageFact {
                id: GoSemanticPackageId(0),
                stable_key: "pkg".to_string(),
                package_id: "github.com/acme/project/pkg".to_string(),
                package_path: "github.com/acme/project/pkg".to_string(),
                package_name: "foo".to_string(),
                module_path: "github.com/acme/project".to_string(),
                files: vec!["src/main.go".to_string()],
            }],
            ..GoSemanticFactsOutput::default()
        })
        .expect("semantic facts replace");
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "Bar".to_string(),
            span: Span::point(file, 2, 1),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });

        let function = db.functions().first().expect("pushed function").clone();
        let record =
            super::function_identity_record(&db, &function).expect("Go function builds a record");
        assert_eq!(
            record.package_or_module.as_ref(),
            "github.com/acme/project/pkg"
        );
    }

    #[test]
    fn go_function_without_package_falls_back_to_path() {
        let record = go_function_record(None);
        assert_eq!(record.package_or_module.as_ref(), "src/main.go");
    }

    #[test]
    fn typescript_function_keeps_file_path_regardless_of_package_fact() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            std::path::PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "export function bar() {}\n".to_string(),
        );
        // A stray PackageFact must not redirect a non-Go record to a package name.
        db.push_package(PackageFact {
            id: PackageId(0),
            file,
            name: "should-be-ignored".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::Go,
        });
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "bar".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        let function = db.functions().first().expect("pushed function").clone();
        let record =
            super::function_identity_record(&db, &function).expect("TS function builds a record");
        assert_eq!(record.package_or_module.as_ref(), "src/app.ts");
    }

    fn identity_record(id: u64, container: &str, multiplicity: u32) -> IdentityRecord {
        let language = LanguageTag::Go;
        let span = Span::point(FileId(0), 1, 1);
        IdentityRecord {
            id: IdentityRecordId(id),
            kind: IdentityKind::Function,
            file_id: FileId(0),
            span: span.clone(),
            language,
            package_or_module: Arc::from("pkg"),
            container_path: Arc::from(container),
            display_name: Arc::from(container),
            signature_digest: compute_signature_digest(
                language, "pkg", container, container, None, None,
            ),
            multiplicity,
            stable_key: compute_identity_stable_key(
                IdentityKind::Function,
                language,
                "pkg",
                container,
                FileId(0),
                &span,
            ),
            originating_call_site_id: None,
            originating_call_target_id: None,
        }
    }

    fn digest_for(output: &IdentityProviderOutput) -> Digest {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let db = AnalysisDb::new();
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.identity")
            .expect("identity manifest");
        super::identity_output_digest(
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "polint.calls", &["a"]),
            output,
        )
    }

    #[test]
    fn identity_output_digest_uses_stable_payloads_not_dense_ids() {
        let base = IdentityProviderOutput {
            records: vec![identity_record(1, "pkg.A", 1)],
        };
        let renumbered = IdentityProviderOutput {
            records: vec![identity_record(100, "pkg.A", 1)],
        };
        let changed_payload = IdentityProviderOutput {
            records: vec![identity_record(1, "pkg.B", 1)],
        };
        let changed_multiplicity = IdentityProviderOutput {
            records: vec![identity_record(1, "pkg.A", 2)],
        };

        assert_eq!(digest_for(&base), digest_for(&renumbered));
        assert_ne!(digest_for(&base), digest_for(&changed_payload));
        assert_ne!(digest_for(&base), digest_for(&changed_multiplicity));
    }

    #[test]
    fn empty_digest_is_deterministic() {
        let first = super::identity_output_digest_for_test(&[]);
        let second = super::identity_output_digest_for_test(&[]);
        assert_eq!(first, second);
        assert!(!first.value.is_empty());
    }

    #[test]
    fn pipeline_extracts_dedups_and_assigns_ids() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            std::path::PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\nfunc main() {}\n".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "main".to_string(),
            span: Span::point(file, 2, 1),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });

        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.identity")
            .expect("identity manifest");

        let first = super::derive_identity_with_cache_stats(
            &mut db,
            &snapshot,
            manifest,
            Digest::from_parts(DigestKind::ProviderOutput, "polint.calls", &["a"]),
        );

        assert!(first.diagnostics.is_empty());
        assert_eq!(db.identity_records().len(), 1);
        assert_eq!(db.identity_records()[0].id, IdentityRecordId(0));
        assert_eq!(db.identity_records()[0].kind, IdentityKind::Function);

        // Determinism: a second run over a fresh equivalent db gives the same digest.
        let mut db2 = AnalysisDb::new();
        let file2 = db2.add_file(
            std::path::PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\nfunc main() {}\n".to_string(),
        );
        db2.push_function(FunctionFact {
            id: FunctionId(0),
            file: file2,
            name: "main".to_string(),
            span: Span::point(file2, 2, 1),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        let second = super::derive_identity_with_cache_stats(
            &mut db2,
            &snapshot,
            manifest,
            Digest::from_parts(DigestKind::ProviderOutput, "polint.calls", &["a"]),
        );
        assert_eq!(first.output_digest, second.output_digest);
    }

    #[test]
    fn go_function_renders_package_qualified_through_real_provider() {
        // End-to-end provider->renderer proof on a REAL Go FunctionFact (not a
        // hand-built IdentityRecord): a `package foo` file with a Go PackageFact
        // and a `Bar` function must render `foo.Bar` after running through
        // `derive_identity_with_cache_stats`. This closes the verifier-flagged gap
        // that no test exercised a record built by the real provider.
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            std::path::PathBuf::from("src/foo.go"),
            "src/foo.go".to_string(),
            "package foo\nfunc Bar() {}\n".to_string(),
        );
        db.push_package(PackageFact {
            id: PackageId(0),
            file,
            name: "foo".to_string(),
            span: Span::point(file, 1, 1),
            language: Language::Go,
        });
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: "Bar".to_string(),
            span: Span::point(file, 2, 1),
            language: Language::Go,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });

        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let snapshot = InputSnapshot::from_run_inputs(
            &loaded,
            &db,
            "config-a",
            "rules-a",
            AnalysisPlan::empty().digest(),
            AnalysisKernel::provider_manifests(),
        );
        let manifest = AnalysisKernel::provider_manifests()
            .iter()
            .find(|manifest| manifest.id == "polint.identity")
            .expect("identity manifest");

        let run = super::derive_identity_with_cache_stats(
            &mut db,
            &snapshot,
            manifest,
            Digest::from_parts(DigestKind::ProviderOutput, "polint.calls", &["a"]),
        );
        assert!(run.diagnostics.is_empty());

        let record = db
            .identity_records()
            .iter()
            .find(|record| record.kind == IdentityKind::Function)
            .expect("a Function identity record exists");
        assert_eq!(
            crate::analysis::identity::render::go_relstring::render(record),
            "foo.Bar"
        );
    }
}
