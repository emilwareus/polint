#[cfg(test)]
mod tests {
    use super::validate_fact_metadata;
    use crate::analysis_kernel::{
        AnalysisKernel, FactConfidence, FactFamily, FactMeta, FactPrecision, FactRef,
        ValidationStatus,
    };
    use crate::core::{
        AnalysisDb, BranchObligation, BranchId, ComplexityMetricFact, FileId, FileMetricFact,
        FunctionFact, FunctionId, FunctionMetricFact, Language, ModuleNode, ModuleNodeId,
        ModuleNodeKind, PackageId, Span, TestFact, TsComponentFact,
    };
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    #[test]
    fn metadata_validation_conflict_records_render_internal_diagnostics_with_evidence() {
        let mut db = AnalysisDb::new();
        let existing = FactRef::new(FactFamily::Import, 1);
        let incoming = FactRef::new(FactFamily::Import, 2);

        db.fact_meta_mut_for_test()
            .insert(existing, test_meta(FactFamily::Import, "import:key", "payload:a"));
        db.fact_meta_mut_for_test()
            .insert(incoming, test_meta(FactFamily::Import, "import:key", "payload:b"));

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].rule_id, "polint/internal");
        assert!(
            diagnostics[0]
                .message
                .starts_with("Fact metadata stable key conflict detected")
        );
        assert_eq!(
            evidence_labels(&diagnostics[0]),
            BTreeSet::from([
                "existing_ref",
                "family",
                "incoming_ref",
                "stable_key",
            ])
        );
    }

    #[test]
    fn metadata_validation_span_failures_are_reported_deterministically() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.ts"),
            "src/app.ts".to_string(),
            "abc".to_string(),
        );
        let other_file = db.add_file(
            PathBuf::from("src/other.ts"),
            "src/other.ts".to_string(),
            "abcdef".to_string(),
        );
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "too_long".to_string(),
            span: span(file, 0, 4),
            language: Language::TypeScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "reversed".to_string(),
            span: span(file, 2, 1),
            language: Language::TypeScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });
        db.push_function(FunctionFact {
            id: FunctionId(99),
            file,
            name: "wrong_file".to_string(),
            span: span(other_file, 0, 1),
            language: Language::TypeScript,
            is_test: false,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        });

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let messages = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();

        assert_eq!(messages.len(), 3);
        assert!(
            messages
                .iter()
                .all(|message| message.starts_with("Fact metadata span validation failed"))
        );
    }

    #[test]
    fn metadata_validation_reference_failures_cover_current_focused_fields() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.tsx"),
            "src/app.tsx".to_string(),
            "export function Button() { return null; }\n".to_string(),
        );
        db.push_branch(BranchObligation {
            id: BranchId(99),
            function: Some(FunctionId(404)),
            file,
            decision_span: span(file, 0, 1),
            condition_text: "enabled".to_string(),
            edge_label: "true".to_string(),
            is_error_path: false,
            stable_fingerprint: "branch:key".to_string(),
        });
        db.push_test(TestFact {
            file,
            function: Some(FunctionId(405)),
            name: "TestButton".to_string(),
            span: span(file, 0, 1),
            evidence_terms: Vec::new(),
            assertion_count: 0,
            subtest_count: 0,
            subtest_names: Vec::new(),
            table_rows: 0,
        });
        db.push_ts_component(TsComponentFact {
            file,
            function: Some(FunctionId(406)),
            name: "Button".to_string(),
            span: span(file, 0, 1),
        });
        db.replace_module_graph_facts(
            Vec::new(),
            vec![ModuleNode {
                id: ModuleNodeId(99),
                kind: ModuleNodeKind::File,
                label: "missing".to_string(),
                file: Some(FileId(404)),
                package: Some(PackageId(405)),
                language: Some(Language::Tsx),
            }],
            Vec::new(),
        );
        db.replace_metric_facts(
            vec![FileMetricFact {
                file: FileId(406),
                language: Language::Tsx,
                line_count: 1,
                non_empty_line_count: 1,
                byte_count: 1,
                function_count: 0,
            }],
            vec![FunctionMetricFact {
                function: FunctionId(407),
                file: FileId(407),
                name: "Button".to_string(),
                span: span(file, 0, 1),
                language: Language::Tsx,
                line_count: 1,
                byte_count: 1,
            }],
            vec![ComplexityMetricFact {
                function: FunctionId(408),
                file: FileId(408),
                name: "Button".to_string(),
                span: span(file, 0, 1),
                language: Language::Tsx,
                cyclomatic_complexity: 1,
            }],
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());
        let evidence_values = diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic
                    .message
                    .starts_with("Fact metadata reference validation failed")
            })
            .flat_map(|diagnostic| {
                diagnostic
                    .evidence
                    .iter()
                    .filter(|evidence| evidence.label == "field")
                    .map(|evidence| evidence.value.as_str())
            })
            .collect::<BTreeSet<_>>();

        assert_eq!(
            evidence_values,
            BTreeSet::from([
                "BranchObligation.function",
                "ComplexityMetricFact.file",
                "ComplexityMetricFact.function",
                "FileMetricFact.file",
                "FunctionMetricFact.file",
                "FunctionMetricFact.function",
                "ModuleNode.file",
                "ModuleNode.package",
                "TestFact.function",
                "TsComponentFact.function",
            ])
        );
    }

    #[test]
    fn metadata_validation_precision_ceiling_violations_name_provider_family_and_precision() {
        let mut db = AnalysisDb::new();
        db.fact_meta_mut_for_test().insert(
            FactRef::new(FactFamily::FileMetric, 0),
            FactMeta {
                stable_key: "metric:key".to_string(),
                producer_id: "polint.metrics",
                layer_id: "polint.metrics",
                precision: FactPrecision::Exact,
                confidence: FactConfidence::High,
                validation: ValidationStatus::NativeTrusted,
                payload_digest: "payload:a".to_string(),
            },
        );

        let diagnostics = validate_fact_metadata(&db, AnalysisKernel::provider_manifests());

        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .message
                .starts_with("Fact metadata precision ceiling violated")
        );
        assert_eq!(
            evidence_labels(&diagnostics[0]),
            BTreeSet::from(["ceiling", "family", "precision", "producer_id"])
        );
    }

    fn test_meta(family: FactFamily, stable_key: &str, payload_digest: &str) -> FactMeta {
        FactMeta {
            stable_key: stable_key.to_string(),
            producer_id: match family {
                FactFamily::SourceFile => "polint.source",
                FactFamily::FileMetric => "polint.metrics",
                _ => "polint.go.syntax",
            },
            layer_id: "polint.go.syntax",
            precision: FactPrecision::Syntax,
            confidence: FactConfidence::High,
            validation: ValidationStatus::NativeTrusted,
            payload_digest: payload_digest.to_string(),
        }
    }

    fn span(file: FileId, start_byte: u32, end_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte,
            start_line: 1,
            start_col: start_byte + 1,
            end_line: 1,
            end_col: end_byte + 1,
        }
    }

    fn evidence_labels(diagnostic: &crate::diagnostics::Diagnostic) -> BTreeSet<&str> {
        diagnostic
            .evidence
            .iter()
            .map(|evidence| evidence.label.as_str())
            .collect()
    }
}
