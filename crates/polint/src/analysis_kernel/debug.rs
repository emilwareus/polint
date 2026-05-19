#![cfg(test)]

use crate::analysis_kernel::{
    FactConfidence, FactFamily, FactMeta, FactPrecision, FactRef, ValidationStatus,
};
use crate::core::{
    AnalysisDb, FileId, Language, ReferenceFact, Span, SymbolPrecision, SymbolResolutionStatus,
};
use serde::Serialize;
use serde_json::Value;

pub(crate) fn metadata_debug_json_for_test(db: &AnalysisDb) -> Value {
    let report = MetadataDebugReport {
        files: file_rows(db),
        imports: import_rows(db),
        symbols: symbol_rows(db),
        references: reference_rows(db),
    };
    serde_json::to_value(report).expect("metadata debug report should serialize")
}

#[derive(Serialize)]
struct MetadataDebugReport<'a> {
    files: Vec<FileDebugRow<'a>>,
    imports: Vec<ImportDebugRow<'a>>,
    symbols: Vec<SymbolDebugRow<'a>>,
    references: Vec<ReferenceDebugRow<'a>>,
}

#[derive(Serialize)]
struct MetadataDebugFields<'a> {
    family: &'static str,
    run_id: u64,
    stable_key: &'a str,
    producer_id: &'a str,
    layer_id: &'a str,
    precision: &'static str,
    confidence: &'static str,
    validation: &'static str,
}

#[derive(Serialize)]
struct FileDebugRow<'a> {
    #[serde(flatten)]
    metadata: MetadataDebugFields<'a>,
    path: &'a str,
    language: Language,
    content_hash: &'a str,
}

#[derive(Serialize)]
struct ImportDebugRow<'a> {
    #[serde(flatten)]
    metadata: MetadataDebugFields<'a>,
    path: &'a str,
    language: Language,
    import_path: &'a str,
    span: DebugSpan,
}

#[derive(Serialize)]
struct SymbolDebugRow<'a> {
    #[serde(flatten)]
    metadata: MetadataDebugFields<'a>,
    name: &'a str,
    qualified_name: &'a str,
    kind: &'static str,
    namespace: &'static str,
    path: Option<&'a str>,
    span: Option<DebugSpan>,
    fact_precision: &'static str,
}

#[derive(Serialize)]
struct ReferenceDebugRow<'a> {
    #[serde(flatten)]
    metadata: MetadataDebugFields<'a>,
    name: &'a str,
    qualified_name: &'a str,
    status: &'static str,
    target: Option<u64>,
    candidates: Vec<u64>,
    path: Option<&'a str>,
    span: Option<DebugSpan>,
    fact_precision: &'static str,
}

#[derive(Clone, Copy, Serialize)]
struct DebugSpan {
    start_byte: u32,
    end_byte: u32,
    start_line: u32,
    start_col: u32,
    end_line: u32,
    end_col: u32,
}

fn file_rows(db: &AnalysisDb) -> Vec<FileDebugRow<'_>> {
    let mut rows = db
        .files()
        .iter()
        .filter_map(|file| {
            let run_id = u64::from(file.id.0);
            metadata_fields(db, FactFamily::SourceFile, run_id).map(|metadata| FileDebugRow {
                metadata,
                path: file.relative_path.as_str(),
                language: file.language,
                content_hash: file.content_hash.as_str(),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        (left.path, left.metadata.stable_key, left.metadata.run_id).cmp(&(
            right.path,
            right.metadata.stable_key,
            right.metadata.run_id,
        ))
    });
    rows
}

fn import_rows(db: &AnalysisDb) -> Vec<ImportDebugRow<'_>> {
    let mut rows = db
        .imports()
        .iter()
        .filter_map(|fact| {
            let file = db.file(fact.file)?;
            metadata_fields(db, FactFamily::Import, fact.id.0).map(|metadata| ImportDebugRow {
                metadata,
                path: file.relative_path.as_str(),
                language: fact.language,
                import_path: fact.path.as_str(),
                span: debug_span(&fact.span),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        (
            left.path,
            left.span.start_byte,
            left.import_path,
            left.metadata.stable_key,
            left.metadata.run_id,
        )
            .cmp(&(
                right.path,
                right.span.start_byte,
                right.import_path,
                right.metadata.stable_key,
                right.metadata.run_id,
            ))
    });
    rows
}

fn symbol_rows(db: &AnalysisDb) -> Vec<SymbolDebugRow<'_>> {
    let mut rows = db
        .symbols()
        .iter()
        .filter_map(|fact| {
            metadata_fields(db, FactFamily::Symbol, fact.id.0).map(|metadata| SymbolDebugRow {
                metadata,
                name: fact.name.as_str(),
                qualified_name: fact.qualified_name.as_str(),
                kind: symbol_kind_label(fact.kind),
                namespace: symbol_namespace_label(fact.namespace),
                path: relative_path(db, fact.file),
                span: fact.primary_span.as_ref().map(debug_span),
                fact_precision: symbol_precision_label(fact.precision),
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        (
            left.path.unwrap_or(""),
            span_start(left.span),
            left.name,
            left.metadata.stable_key,
            left.metadata.run_id,
        )
            .cmp(&(
                right.path.unwrap_or(""),
                span_start(right.span),
                right.name,
                right.metadata.stable_key,
                right.metadata.run_id,
            ))
    });
    rows
}

fn reference_rows(db: &AnalysisDb) -> Vec<ReferenceDebugRow<'_>> {
    let mut rows = db
        .references()
        .iter()
        .filter_map(|fact| {
            metadata_fields(db, FactFamily::Reference, fact.id.0).map(|metadata| {
                ReferenceDebugRow {
                    metadata,
                    name: fact.name.as_str(),
                    qualified_name: fact.qualified_name.as_str(),
                    status: symbol_resolution_status_label(fact.status),
                    target: fact.target.map(|target| target.0),
                    candidates: sorted_candidate_ids(fact),
                    path: relative_path(db, fact.file),
                    span: fact.primary_span.as_ref().map(debug_span),
                    fact_precision: symbol_precision_label(fact.precision),
                }
            })
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        (
            left.path.unwrap_or(""),
            span_start(left.span),
            left.name,
            left.metadata.stable_key,
            left.metadata.run_id,
        )
            .cmp(&(
                right.path.unwrap_or(""),
                span_start(right.span),
                right.name,
                right.metadata.stable_key,
                right.metadata.run_id,
            ))
    });
    rows
}

fn metadata_fields(
    db: &AnalysisDb,
    family: FactFamily,
    run_id: u64,
) -> Option<MetadataDebugFields<'_>> {
    let meta = db.metadata_for(FactRef::new(family, run_id))?;
    Some(metadata_debug_fields(family, run_id, meta))
}

fn metadata_debug_fields<'a>(
    family: FactFamily,
    run_id: u64,
    meta: &'a FactMeta,
) -> MetadataDebugFields<'a> {
    MetadataDebugFields {
        family: family.label(),
        run_id,
        stable_key: meta.stable_key.as_str(),
        producer_id: meta.producer_id,
        layer_id: meta.layer_id,
        precision: fact_precision_label(meta.precision),
        confidence: fact_confidence_label(meta.confidence),
        validation: validation_status_label(meta.validation),
    }
}

fn relative_path(db: &AnalysisDb, file: Option<FileId>) -> Option<&str> {
    file.and_then(|file| db.file(file))
        .map(|file| file.relative_path.as_str())
}

fn debug_span(span: &Span) -> DebugSpan {
    DebugSpan {
        start_byte: span.start_byte,
        end_byte: span.end_byte,
        start_line: span.start_line,
        start_col: span.start_col,
        end_line: span.end_line,
        end_col: span.end_col,
    }
}

fn span_start(span: Option<DebugSpan>) -> u32 {
    span.map(|span| span.start_byte).unwrap_or(u32::MAX)
}

fn sorted_candidate_ids(reference: &ReferenceFact) -> Vec<u64> {
    let mut candidates = reference
        .candidates
        .iter()
        .map(|candidate| candidate.0)
        .collect::<Vec<_>>();
    candidates.sort_unstable();
    candidates
}

fn fact_precision_label(precision: FactPrecision) -> &'static str {
    match precision {
        FactPrecision::Exact => "exact",
        FactPrecision::Syntax => "syntax",
        FactPrecision::SetupAware => "setup_aware",
        FactPrecision::Heuristic => "heuristic",
        FactPrecision::Unresolved => "unresolved",
        FactPrecision::Ambiguous => "ambiguous",
        FactPrecision::SetupMissing => "setup_missing",
        FactPrecision::Unsupported => "unsupported",
    }
}

fn fact_confidence_label(confidence: FactConfidence) -> &'static str {
    match confidence {
        FactConfidence::High => "high",
        FactConfidence::Medium => "medium",
        FactConfidence::Low => "low",
    }
}

fn validation_status_label(status: ValidationStatus) -> &'static str {
    match status {
        ValidationStatus::NativeTrusted => "native_trusted",
        ValidationStatus::SchemaValidated => "schema_validated",
        ValidationStatus::ReferentiallyValidated => "referentially_validated",
        ValidationStatus::SpanValidated => "span_validated",
        ValidationStatus::StableKeyValidated => "stable_key_validated",
        ValidationStatus::ConflictRejected => "conflict_rejected",
    }
}

fn symbol_precision_label(precision: SymbolPrecision) -> &'static str {
    match precision {
        SymbolPrecision::ExactSemantic => "exact_semantic",
        SymbolPrecision::ExactLocal => "exact_local",
        SymbolPrecision::ModuleLinked => "module_linked",
        SymbolPrecision::Heuristic => "heuristic",
        SymbolPrecision::Unresolved => "unresolved",
        SymbolPrecision::Ambiguous => "ambiguous",
        SymbolPrecision::SetupMissing => "setup_missing",
        SymbolPrecision::Unsupported => "unsupported",
    }
}

fn symbol_resolution_status_label(status: SymbolResolutionStatus) -> &'static str {
    match status {
        SymbolResolutionStatus::Resolved => "resolved",
        SymbolResolutionStatus::Unresolved => "unresolved",
        SymbolResolutionStatus::Ambiguous => "ambiguous",
        SymbolResolutionStatus::SetupMissing => "setup_missing",
        SymbolResolutionStatus::Unsupported => "unsupported",
    }
}

fn symbol_kind_label(kind: crate::core::SymbolKind) -> &'static str {
    match kind {
        crate::core::SymbolKind::Package => "package",
        crate::core::SymbolKind::Module => "module",
        crate::core::SymbolKind::File => "file",
        crate::core::SymbolKind::Function => "function",
        crate::core::SymbolKind::Method => "method",
        crate::core::SymbolKind::Class => "class",
        crate::core::SymbolKind::Interface => "interface",
        crate::core::SymbolKind::TypeAlias => "type_alias",
        crate::core::SymbolKind::Enum => "enum",
        crate::core::SymbolKind::EnumMember => "enum_member",
        crate::core::SymbolKind::Variable => "variable",
        crate::core::SymbolKind::Constant => "constant",
        crate::core::SymbolKind::Parameter => "parameter",
        crate::core::SymbolKind::Field => "field",
        crate::core::SymbolKind::Property => "property",
        crate::core::SymbolKind::Namespace => "namespace",
        crate::core::SymbolKind::Import => "import",
        crate::core::SymbolKind::Export => "export",
        crate::core::SymbolKind::Unknown => "unknown",
    }
}

fn symbol_namespace_label(namespace: crate::core::SymbolNamespace) -> &'static str {
    match namespace {
        crate::core::SymbolNamespace::Value => "value",
        crate::core::SymbolNamespace::Type => "type",
        crate::core::SymbolNamespace::Namespace => "namespace",
        crate::core::SymbolNamespace::Package => "package",
        crate::core::SymbolNamespace::Module => "module",
        crate::core::SymbolNamespace::Unknown => "unknown",
    }
}

mod tests {
    use super::super::{AnalysisKernel, KernelInput};
    use crate::analysis_plan::AnalysisPlan;
    use crate::cache::Cache;
    use crate::config::load_config;
    use crate::core::AnalysisDb;
    use serde_json::Value;
    use std::fs;
    use std::path::{Path, PathBuf};

    struct DebugFixture {
        temp: tempfile::TempDir,
        db: AnalysisDb,
    }

    fn debug_fixture() -> DebugFixture {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(temp.path().join("src")).expect("create src directory");
        fs::write(
            temp.path().join(".polint.toml"),
            r#"
[workspace]
include = ["src/**"]
exclude = []
"#,
        )
        .expect("write config");
        fs::write(
            temp.path().join("src/tokens.ts"),
            r#"export const token = "ok";"#,
        )
        .expect("write tokens");
        fs::write(
            temp.path().join("src/app.ts"),
            r#"import { token as importedToken } from "./tokens";

export function answer() {
  return importedToken;
}

export const value = answer();
"#,
        )
        .expect("write app");

        let loaded = load_config(temp.path()).expect("load config");
        let cache = Cache::new("", false);
        let plan =
            AnalysisPlan::from_capability_names_for_test(&["imports", "symbols", "references"]);
        let output = AnalysisKernel::run(KernelInput {
            loaded: &loaded,
            cache: &cache,
            config_digest: "metadata-debug-config",
            rule_digest: "metadata-debug-rules",
            plan: &plan,
            parallel: false,
        })
        .expect("kernel run");
        assert!(
            output
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.rule_id != "polint/internal"),
            "clean debug fixture should not emit internal diagnostics: {:#?}",
            output.diagnostics
        );

        DebugFixture {
            temp,
            db: output.db,
        }
    }

    fn debug_report_from_kernel_run() -> (tempfile::TempDir, Value) {
        let fixture = debug_fixture();
        let report = AnalysisKernel::metadata_debug_json_for_test(&fixture.db);
        (fixture.temp, report)
    }

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("polint crate should live under crates/")
            .to_path_buf()
    }

    fn read_source_tree(path: &Path) -> String {
        let mut files = Vec::new();
        collect_files(path, &mut files);
        files.sort();

        files
            .into_iter()
            .map(|file| fs::read_to_string(&file).expect("read public source file"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn collect_files(path: &Path, files: &mut Vec<PathBuf>) {
        if path.is_file() {
            files.push(path.to_path_buf());
            return;
        }

        let mut entries = fs::read_dir(path)
            .expect("read source directory")
            .map(|entry| entry.expect("read source entry").path())
            .collect::<Vec<_>>();
        entries.sort();

        for entry in entries {
            collect_files(&entry, files);
        }
    }

    #[test]
    fn metadata_debug_json_contains_files_imports_symbols_and_references() {
        let (_temp, report) = debug_report_from_kernel_run();

        for key in ["files", "imports", "symbols", "references"] {
            let rows = report[key]
                .as_array()
                .unwrap_or_else(|| panic!("missing debug array `{key}`: {report:#?}"));
            assert!(!rows.is_empty(), "debug array `{key}` should not be empty");
        }
    }

    mod semantic_debug_json {
        use super::*;

        #[test]
        fn metadata_debug_json_contains_semantic_index_families() {
            let (_temp, report) = debug_report_from_kernel_run();
            let semantic = report["semantic"]
                .as_object()
                .unwrap_or_else(|| panic!("missing semantic debug object: {report:#?}"));

            for key in [
                "scopes",
                "imports",
                "exports",
                "aliases",
                "resolutions",
                "generated_symbols",
                "stable_exports",
            ] {
                assert!(
                    semantic
                        .get(key)
                        .and_then(serde_json::Value::as_array)
                        .is_some(),
                    "semantic debug object missing `{key}` array: {semantic:#?}"
                );
            }
            assert!(
                !semantic["stable_exports"].as_array().unwrap().is_empty(),
                "debug fixture should expose stable export identities: {semantic:#?}"
            );
            assert!(
                !semantic["generated_symbols"].as_array().unwrap().is_empty(),
                "debug fixture should expose native generated symbol hooks: {semantic:#?}"
            );
        }

        #[test]
        fn semantic_debug_json_rows_include_status_fact_precision_and_nested_metadata() {
            let (_temp, report) = debug_report_from_kernel_run();
            let row = report["semantic"]["generated_symbols"]
                .as_array()
                .and_then(|rows| rows.first())
                .unwrap_or_else(|| {
                    panic!("missing generated symbol semantic debug row: {report:#?}")
                });

            for field in [
                "status",
                "fact_precision",
                "stable_key",
                "producer_id",
                "layer_id",
            ] {
                assert!(
                    row.get(field).is_some(),
                    "semantic generated row missing `{field}`: {row:#?}"
                );
            }
            let metadata = row["metadata"]
                .as_object()
                .unwrap_or_else(|| panic!("missing nested metadata object: {row:#?}"));
            for field in ["precision", "confidence", "validation"] {
                assert!(
                    metadata.get(field).is_some(),
                    "semantic metadata missing `{field}`: {row:#?}"
                );
            }
        }
    }

    #[test]
    fn metadata_debug_json_rows_include_required_metadata_fields() {
        let (_temp, report) = debug_report_from_kernel_run();

        for key in ["files", "imports", "symbols", "references"] {
            for row in report[key]
                .as_array()
                .unwrap_or_else(|| panic!("missing debug array `{key}`: {report:#?}"))
            {
                for field in [
                    "family",
                    "run_id",
                    "stable_key",
                    "producer_id",
                    "layer_id",
                    "precision",
                    "confidence",
                    "validation",
                ] {
                    assert!(
                        row.get(field).is_some(),
                        "debug row in `{key}` missing `{field}`: {row:#?}"
                    );
                }
            }
        }
    }

    #[test]
    fn metadata_debug_json_serializes_byte_identically_for_same_database() {
        let fixture = debug_fixture();
        let first = AnalysisKernel::metadata_debug_json_for_test(&fixture.db);
        let second = AnalysisKernel::metadata_debug_json_for_test(&fixture.db);

        let first_json = serde_json::to_string_pretty(&first).expect("serialize first");
        let second_json = serde_json::to_string_pretty(&second).expect("serialize second");

        assert_eq!(first_json, second_json);
    }

    #[test]
    fn metadata_debug_json_excludes_absolute_paths_and_transient_runtime_details() {
        let (temp, report) = debug_report_from_kernel_run();
        let rendered = serde_json::to_string_pretty(&report).expect("serialize report");
        let temp_root = temp.path().to_string_lossy();

        for forbidden in [
            temp_root.as_ref(),
            concat!("System", "Time"),
            concat!("Inst", "ant"),
            "0x",
            "timestamp",
            "created_at",
            "updated_at",
        ] {
            assert!(
                !rendered.contains(forbidden),
                "debug JSON should not contain `{forbidden}`:\n{rendered}"
            );
        }
    }

    #[test]
    fn metadata_debug_helpers_are_not_public() {
        let root = repo_root();
        let lib_source =
            fs::read_to_string(root.join("crates/polint/src/lib.rs")).expect("read lib.rs");
        assert!(lib_source.contains("pub(crate) mod analysis_kernel;"));
        assert!(!lib_source.contains("pub mod analysis_kernel;"));

        let public_sources = [
            lib_source,
            read_source_tree(&root.join("crates/polint/src/sdk")),
            read_source_tree(&root.join("crates/polint/src/runner")),
        ]
        .join("\n");

        for forbidden in [
            "FactMeta",
            "FactMetaStore",
            "metadata_debug_json_for_test",
            "producer_id",
            "layer_id",
            "confidence",
            "validation",
            "provenance metadata",
        ] {
            assert!(
                !public_sources.contains(forbidden),
                "metadata internals must stay out of public surfaces: `{forbidden}`"
            );
        }
    }
}
