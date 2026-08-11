//! Facade composition for the identity provider.

use std::collections::BTreeMap;

use polint_analysis_api::{Digest, InputSnapshot, ProviderManifest};

use crate::analysis::identity::facts::IdentityRecord;
use crate::core::{AnalysisDb, FileId};

pub(crate) use polint_analysis::identity::provider::{
    IdentityProviderRunOutput, valid_call_site_ids,
};

#[cfg(test)]
pub(crate) use crate::analysis::identity::facts::{LanguageTag, compute_identity_stable_key};

pub(crate) fn derive_identity_with_cache_stats(
    db: &mut AnalysisDb,
    input_snapshot: &InputSnapshot,
    manifest: &ProviderManifest,
    calls_provider_output_digest: Digest,
    go_semantic_output_digest: Digest,
) -> IdentityProviderRunOutput {
    let go_semantic_package_paths = db
        .go_semantic_packages()
        .iter()
        .flat_map(|package| {
            package.files.iter().filter_map(|path| {
                db.files()
                    .iter()
                    .find(|file| file.relative_path == *path)
                    .map(|file| (file.id, package.package_path.clone()))
            })
        })
        .collect::<BTreeMap<FileId, String>>();
    polint_analysis::identity::provider::derive_identity_with_cache_stats(
        db,
        input_snapshot,
        manifest,
        calls_provider_output_digest,
        go_semantic_output_digest,
        &go_semantic_package_paths,
    )
}

#[cfg(test)]
fn function_identity_record(
    db: &AnalysisDb,
    interner: &crate::core::StableKeyInterner,
    function: &crate::core::FunctionFact,
) -> Option<IdentityRecord> {
    let go_semantic_package_paths = db
        .go_semantic_packages()
        .iter()
        .flat_map(|package| {
            package.files.iter().filter_map(|path| {
                db.files()
                    .iter()
                    .find(|file| file.relative_path == *path)
                    .map(|file| (file.id, package.package_path.clone()))
            })
        })
        .collect::<BTreeMap<FileId, String>>();
    polint_analysis::identity::provider::function_identity_record(
        db,
        interner,
        function,
        &go_semantic_package_paths,
    )
}

#[cfg(test)]
fn identity_output_digest(
    interner: &crate::core::StableKeyInterner,
    manifest: &ProviderManifest,
    input_snapshot: &InputSnapshot,
    calls_provider_output_digest: &Digest,
    go_semantic_output_digest: &Digest,
    output: &crate::analysis::identity::store::IdentityProviderOutput,
) -> Digest {
    polint_analysis::identity::provider::identity_output_digest(
        interner,
        manifest,
        input_snapshot,
        calls_provider_output_digest,
        go_semantic_output_digest,
        output,
    )
}

#[cfg(test)]
fn identity_output_digest_for_test(parts: &[&str]) -> Digest {
    Digest::from_parts(
        polint_analysis_api::DigestKind::ProviderOutput,
        "identity_output",
        parts,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::identity::facts::{
        IdentityKind, IdentityRecordId, compute_signature_digest,
    };
    use crate::analysis::identity::store::IdentityProviderOutput;
    use crate::analysis_kernel::AnalysisKernel;
    use crate::analysis_kernel::incremental::{Digest, DigestKind};
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
        super::function_identity_record(&db, &db.stable_key_interner(), &function)
            .expect("Go function builds a record")
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
        let package_stable_key = db.stable_key_interner().intern("pkg");
        db.replace_go_semantic_facts(GoSemanticFactsOutput {
            packages: vec![GoSemanticPackageFact {
                id: GoSemanticPackageId(0),
                stable_key: package_stable_key,
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
        let record = super::function_identity_record(&db, &db.stable_key_interner(), &function)
            .expect("Go function builds a record");
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
        let record = super::function_identity_record(&db, &db.stable_key_interner(), &function)
            .expect("TS function builds a record");
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
            stable_key: crate::core::stable_key_for_test(&compute_identity_stable_key(
                IdentityKind::Function,
                language,
                "pkg",
                container,
                FileId(0),
                &span,
            )),
            originating_call_site_id: None,
            originating_call_target_id: None,
        }
    }

    fn digest_for(output: &IdentityProviderOutput) -> Digest {
        let temp = tempdir().expect("tempdir");
        fs::write(temp.path().join(".polint.toml"), "").expect("config");
        let loaded = load_config(temp.path()).expect("config loads");
        let db = AnalysisDb::new();
        let snapshot = crate::analysis_kernel::incremental::input_snapshot_from_run_inputs(
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
            &db.stable_key_interner(),
            manifest,
            &snapshot,
            &Digest::from_parts(DigestKind::ProviderOutput, "polint.calls", &["a"]),
            &Digest::from_parts(
                DigestKind::ProviderOutput,
                "polint.go.semantic",
                &["semantic-a"],
            ),
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
        let snapshot = crate::analysis_kernel::incremental::input_snapshot_from_run_inputs(
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
            Digest::from_parts(
                DigestKind::ProviderOutput,
                "polint.go.semantic",
                &["semantic-a"],
            ),
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
            Digest::from_parts(
                DigestKind::ProviderOutput,
                "polint.go.semantic",
                &["semantic-a"],
            ),
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
        let snapshot = crate::analysis_kernel::incremental::input_snapshot_from_run_inputs(
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
            Digest::from_parts(
                DigestKind::ProviderOutput,
                "polint.go.semantic",
                &["semantic-a"],
            ),
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
