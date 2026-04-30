use polint_core::{
    AnalysisDb, BranchId, BranchObligation, FileId, FunctionFact, FunctionId, ImportFact,
    ImportId, Language, RuleOptions, Span, StringLiteralFact, TestFact, run_rules,
};
use polint_diagnostics::{Diagnostic, OutputFormat, render};
use polint_rules::built_in_rules;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const GO_COMPLEXITY: &str = "examples/go-cyclomatic-complexity";
const TS_COMPLEXITY: &str = "examples/ts-cyclomatic-complexity";
const GO_IMPORT_BOUNDARIES: &str = "examples/go-import-boundaries";
const TS_RAW_COLORS: &str = "examples/ts-no-raw-colors";
const GO_BRANCH_OBLIGATIONS: &str = "examples/go-branch-obligations";
const GO_TEST_SUITE_SIZE: &str = "examples/go-test-suite-size";
const GO_ASSERTION_AFTER_ACTION: &str = "examples/go-assertion-after-action";
const CONFIG_NO_LITERAL: &str = "examples/config-query-no-literal";

const ALL_PHASE6_RULE_IDS: [&str; 8] = [
    GO_COMPLEXITY,
    TS_COMPLEXITY,
    GO_IMPORT_BOUNDARIES,
    TS_RAW_COLORS,
    GO_BRANCH_OBLIGATIONS,
    GO_TEST_SUITE_SIZE,
    GO_ASSERTION_AFTER_ACTION,
    CONFIG_NO_LITERAL,
];

fn add_file(db: &mut AnalysisDb, path: &str, source: &str) -> FileId {
    db.add_file(PathBuf::from(path), path.to_string(), source.to_string())
}

fn span(file: FileId, line: u32, start_col: u32, end_col: u32) -> Span {
    Span {
        file,
        start_byte: (line * 100) + start_col,
        end_byte: (line * 100) + end_col,
        start_line: line,
        start_col,
        end_line: line,
        end_col,
    }
}

fn phase6_db() -> AnalysisDb {
    let mut db = AnalysisDb::new();
    let go_file = add_file(
        &mut db,
        "src/payment.go",
        "package payment\n\nfunc AuthorizePayment() {}\n",
    );
    let go_test_file = add_file(
        &mut db,
        "src/payment_test.go",
        "package payment\n\nfunc TestPaymentMatrix(t *testing.T) {}\n",
    );
    let ts_file = add_file(
        &mut db,
        "src/component.tsx",
        "export function PaymentButton() { return <button />; }\n",
    );

    let go_function = db.push_function(FunctionFact {
        id: FunctionId(99),
        file: go_file,
        name: "AuthorizePayment".to_string(),
        span: span(go_file, 5, 6, 22),
        language: Language::Go,
        is_test: false,
        is_exported: true,
        cyclomatic_complexity: 9,
        calls: Vec::new(),
    });
    db.push_function(FunctionFact {
        id: FunctionId(99),
        file: ts_file,
        name: "PaymentButton".to_string(),
        span: span(ts_file, 4, 17, 30),
        language: Language::Tsx,
        is_test: false,
        is_exported: true,
        cyclomatic_complexity: 7,
        calls: Vec::new(),
    });
    db.push_import(ImportFact {
        id: ImportId(99),
        file: go_file,
        package: None,
        path: "github.com/acme/legacy/auth".to_string(),
        span: span(go_file, 7, 8, 38),
        language: Language::Go,
    });
    db.push_branch(BranchObligation {
        id: BranchId(99),
        function: Some(go_function),
        file: go_file,
        decision_span: span(go_file, 14, 5, 15),
        condition_text: "err != nil".to_string(),
        edge_label: "true".to_string(),
        is_error_path: true,
        stable_fingerprint: "branch-auth-error".to_string(),
    });
    db.push_test(TestFact {
        file: go_test_file,
        function: None,
        name: "TestPaymentMatrix".to_string(),
        span: span(go_test_file, 8, 6, 23),
        evidence_terms: vec!["payment matrix".to_string()],
        assertion_count: 2,
        subtest_count: 3,
        table_rows: 5,
    });
    db.push_test(TestFact {
        file: go_test_file,
        function: None,
        name: "TestPaymentAction".to_string(),
        span: span(go_test_file, 32, 6, 23),
        evidence_terms: vec!["charge action".to_string()],
        assertion_count: 0,
        subtest_count: 0,
        table_rows: 0,
    });
    db.push_string_literal(StringLiteralFact {
        file: ts_file,
        value: "#ff00aa".to_string(),
        span: span(ts_file, 6, 22, 31),
        language: Language::Tsx,
    });
    db.push_string_literal(StringLiteralFact {
        file: ts_file,
        value: "/legacy-testid/".to_string(),
        span: span(ts_file, 8, 19, 36),
        language: Language::TypeScript,
    });
    db.push_string_literal(StringLiteralFact {
        file: go_file,
        value: "legacy-token".to_string(),
        span: span(go_file, 20, 12, 26),
        language: Language::Go,
    });
    db.push_string_literal(StringLiteralFact {
        file: ts_file,
        value: "legacy-token".to_string(),
        span: span(ts_file, 10, 15, 29),
        language: Language::Tsx,
    });
    db.push_jsx_attribute(polint_core::JsxAttributeFact {
        file: ts_file,
        name: "data-color".to_string(),
        value: Some("rgba(1, 2, 3, 0.4)".to_string()),
        span: span(ts_file, 12, 18, 42),
    });

    db
}

fn phase6_options() -> BTreeMap<String, RuleOptions> {
    BTreeMap::from([
        (
            GO_COMPLEXITY.to_string(),
            RuleOptions {
                max: Some(6),
                ..RuleOptions::default()
            },
        ),
        (
            TS_COMPLEXITY.to_string(),
            RuleOptions {
                max: Some(5),
                ..RuleOptions::default()
            },
        ),
        (
            GO_IMPORT_BOUNDARIES.to_string(),
            RuleOptions {
                forbidden_imports: BTreeMap::from([(
                    "src/**/*.go".to_string(),
                    vec!["github.com/acme/legacy/*".to_string()],
                )]),
                ..RuleOptions::default()
            },
        ),
        (
            CONFIG_NO_LITERAL.to_string(),
            RuleOptions {
                deny: vec!["legacy-token".to_string(), "legacy-testid".to_string()],
                ..RuleOptions::default()
            },
        ),
    ])
}

fn diagnostics_for(rule_ids: &[&str]) -> Vec<Diagnostic> {
    let wanted = rule_ids.iter().copied().collect::<BTreeSet<_>>();
    let rules = built_in_rules()
        .into_iter()
        .filter(|rule| wanted.contains(rule.meta().id.as_str()))
        .collect::<Vec<_>>();
    assert_eq!(rules.len(), rule_ids.len(), "missing selected built-in rule");

    let db = phase6_db();
    let enabled = rule_ids
        .iter()
        .map(|rule_id| (*rule_id).to_string())
        .collect::<BTreeSet<_>>();
    let diagnostics = run_rules(&db, &rules, &phase6_options(), &enabled, false);
    assert_rule_ids_present(&diagnostics, rule_ids);
    diagnostics
}

fn assert_rule_ids_present(diagnostics: &[Diagnostic], rule_ids: &[&str]) {
    let present = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.rule_id.as_str())
        .collect::<BTreeSet<_>>();
    for rule_id in rule_ids {
        assert!(
            present.contains(rule_id),
            "expected diagnostic for {rule_id}, got {present:?}"
        );
    }
}

fn assert_json_renderer_output(rendered: &str) {
    let parsed = serde_json::from_str::<serde_json::Value>(rendered).expect("valid JSON snapshot");
    assert!(
        parsed.as_array().is_some_and(|items| !items.is_empty()),
        "expected non-empty diagnostic JSON"
    );
}

fn first_diagnostic_per_rule(diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    let mut seen = BTreeSet::new();
    diagnostics
        .into_iter()
        .filter(|diagnostic| seen.insert(diagnostic.rule_id.clone()))
        .collect()
}

#[test]
fn snapshot_complexity_and_import_boundary_human() {
    let diagnostics = diagnostics_for(&[GO_COMPLEXITY, TS_COMPLEXITY, GO_IMPORT_BOUNDARIES]);
    let rendered = render(OutputFormat::Human, &diagnostics);

    insta::assert_snapshot!(rendered, @r###"
    src/component.tsx:4:17-4:30 warn examples/ts-cyclomatic-complexity
      TS/JS function `PaymentButton` has cyclomatic complexity 7, max 5.
      evidence function: PaymentButton
      evidence complexity: 7
      help: Split condition-heavy UI or business logic into smaller named helpers.
      fingerprint: c59221445d10a49a

    src/payment.go:5:6-5:22 warn examples/go-cyclomatic-complexity
      Go function `AuthorizePayment` has cyclomatic complexity 9, max 6.
      evidence function: AuthorizePayment
      evidence complexity: 9
      help: Split deeply branched behavior into smaller focused functions or table-driven helpers.
      fingerprint: 888fcc2edc346045

    src/payment.go:7:8-7:38 error examples/go-import-boundaries
      Go import `github.com/acme/legacy/auth` violates configured import boundary.
      evidence import: github.com/acme/legacy/auth
      help: Move the dependency behind an allowed interface or update the boundary config if this is intentional.
      fingerprint: 172187f404b7e3b8
    "###);
}

#[test]
fn snapshot_go_heuristics_human() {
    let diagnostics = diagnostics_for(&[
        GO_BRANCH_OBLIGATIONS,
        GO_TEST_SUITE_SIZE,
        GO_ASSERTION_AFTER_ACTION,
    ]);
    let rendered = render(OutputFormat::Human, &diagnostics);

    insta::assert_snapshot!(rendered, @r###"
    src/payment.go:14:5-14:15 warn examples/go-branch-obligations
      No nearby test evidence found for Go branch `err != nil`.
      evidence condition: err != nil
      evidence edge: true
      evidence branch_fingerprint: branch-auth-error
      help: Add a test case that exercises this branch. This rule is heuristic and does not prove exact coverage.
      fingerprint: 5fe72dd727ae6899

    src/payment_test.go:8:6-8:23 warn examples/go-test-suite-size
      Go test `TestPaymentMatrix` has heuristic maintainability score 25, max 24.
      evidence score: 25
      evidence subtests: 3
      evidence table_rows: 5
      evidence assertions: 2
      help: Split this test suite into smaller behavior-focused suites. This rule is heuristic.
      fingerprint: 6d853dc1d70a8926

    src/payment_test.go:32:6-32:23 warn examples/go-assertion-after-action
      Go test `TestPaymentAction` has no obvious assertion or error check.
      evidence test: TestPaymentAction
      evidence assertions: 0
      evidence evidence_terms: charge action
      help: Add an explicit assertion, error check, or failure path. This rule is heuristic.
      fingerprint: 5581e444ecf89a1d
    "###);
}

#[test]
fn snapshot_raw_color_and_denied_literals_json() {
    let diagnostics = diagnostics_for(&[TS_RAW_COLORS, CONFIG_NO_LITERAL]);
    let rendered = render(OutputFormat::Json, &diagnostics);
    assert_json_renderer_output(&rendered);

    insta::assert_snapshot!(rendered, @r###"
    [
      {
        "rule_id": "examples/ts-no-raw-colors",
        "severity": "error",
        "file": "src/component.tsx",
        "range": {
          "start_line": 6,
          "start_col": 22,
          "end_line": 6,
          "end_col": 31
        },
        "message": "Raw color literal `#ff00aa` should use a design token.",
        "labels": [],
        "help": "This rule reports syntax-level literal findings; move this value to a theme/design-token file or use an existing token/CSS variable.",
        "evidence": [
          {
            "label": "literal",
            "value": "#ff00aa"
          },
          {
            "label": "source",
            "value": "string-literal"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "04e498bf841f82c1"
      },
      {
        "rule_id": "examples/config-query-no-literal",
        "severity": "error",
        "file": "src/component.tsx",
        "range": {
          "start_line": 8,
          "start_col": 19,
          "end_line": 8,
          "end_col": 36
        },
        "message": "Configured denied literal `/legacy-testid/` found.",
        "labels": [],
        "help": "Replace the literal with an allowed constant or project-specific abstraction.",
        "evidence": [
          {
            "label": "literal",
            "value": "/legacy-testid/"
          },
          {
            "label": "matched",
            "value": "legacy-testid"
          },
          {
            "label": "language",
            "value": "typescript"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "cf2f7b2a4f6fcbce"
      },
      {
        "rule_id": "examples/config-query-no-literal",
        "severity": "error",
        "file": "src/component.tsx",
        "range": {
          "start_line": 10,
          "start_col": 15,
          "end_line": 10,
          "end_col": 29
        },
        "message": "Configured denied literal `legacy-token` found.",
        "labels": [],
        "help": "Replace the literal with an allowed constant or project-specific abstraction.",
        "evidence": [
          {
            "label": "literal",
            "value": "legacy-token"
          },
          {
            "label": "matched",
            "value": "legacy-token"
          },
          {
            "label": "language",
            "value": "tsx"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "134e545bb263b734"
      },
      {
        "rule_id": "examples/ts-no-raw-colors",
        "severity": "error",
        "file": "src/component.tsx",
        "range": {
          "start_line": 12,
          "start_col": 18,
          "end_line": 12,
          "end_col": 42
        },
        "message": "Raw color literal `rgba(1, 2, 3, 0.4)` should use a design token.",
        "labels": [],
        "help": "This rule reports syntax-level literal findings; move this value to a theme/design-token file or use an existing token/CSS variable.",
        "evidence": [
          {
            "label": "literal",
            "value": "rgba(1, 2, 3, 0.4)"
          },
          {
            "label": "source",
            "value": "jsx-attribute"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "624a857144e5d036"
      },
      {
        "rule_id": "examples/config-query-no-literal",
        "severity": "error",
        "file": "src/payment.go",
        "range": {
          "start_line": 20,
          "start_col": 12,
          "end_line": 20,
          "end_col": 26
        },
        "message": "Configured denied literal `legacy-token` found.",
        "labels": [],
        "help": "Replace the literal with an allowed constant or project-specific abstraction.",
        "evidence": [
          {
            "label": "literal",
            "value": "legacy-token"
          },
          {
            "label": "matched",
            "value": "legacy-token"
          },
          {
            "label": "language",
            "value": "go"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "945e16dfad633b68"
      }
    ]
    "###);
}

#[test]
fn snapshot_all_phase6_rule_ids_json() {
    let diagnostics = first_diagnostic_per_rule(diagnostics_for(&ALL_PHASE6_RULE_IDS));
    assert_rule_ids_present(&diagnostics, &ALL_PHASE6_RULE_IDS);
    let rendered = render(OutputFormat::Json, &diagnostics);
    assert_json_renderer_output(&rendered);

    insta::assert_snapshot!(rendered, @r###"
    [
      {
        "rule_id": "examples/ts-cyclomatic-complexity",
        "severity": "warn",
        "file": "src/component.tsx",
        "range": {
          "start_line": 4,
          "start_col": 17,
          "end_line": 4,
          "end_col": 30
        },
        "message": "TS/JS function `PaymentButton` has cyclomatic complexity 7, max 5.",
        "labels": [],
        "help": "Split condition-heavy UI or business logic into smaller named helpers.",
        "evidence": [
          {
            "label": "function",
            "value": "PaymentButton"
          },
          {
            "label": "complexity",
            "value": "7"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "c59221445d10a49a"
      },
      {
        "rule_id": "examples/ts-no-raw-colors",
        "severity": "error",
        "file": "src/component.tsx",
        "range": {
          "start_line": 6,
          "start_col": 22,
          "end_line": 6,
          "end_col": 31
        },
        "message": "Raw color literal `#ff00aa` should use a design token.",
        "labels": [],
        "help": "This rule reports syntax-level literal findings; move this value to a theme/design-token file or use an existing token/CSS variable.",
        "evidence": [
          {
            "label": "literal",
            "value": "#ff00aa"
          },
          {
            "label": "source",
            "value": "string-literal"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "04e498bf841f82c1"
      },
      {
        "rule_id": "examples/config-query-no-literal",
        "severity": "error",
        "file": "src/component.tsx",
        "range": {
          "start_line": 8,
          "start_col": 19,
          "end_line": 8,
          "end_col": 36
        },
        "message": "Configured denied literal `/legacy-testid/` found.",
        "labels": [],
        "help": "Replace the literal with an allowed constant or project-specific abstraction.",
        "evidence": [
          {
            "label": "literal",
            "value": "/legacy-testid/"
          },
          {
            "label": "matched",
            "value": "legacy-testid"
          },
          {
            "label": "language",
            "value": "typescript"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "cf2f7b2a4f6fcbce"
      },
      {
        "rule_id": "examples/go-cyclomatic-complexity",
        "severity": "warn",
        "file": "src/payment.go",
        "range": {
          "start_line": 5,
          "start_col": 6,
          "end_line": 5,
          "end_col": 22
        },
        "message": "Go function `AuthorizePayment` has cyclomatic complexity 9, max 6.",
        "labels": [],
        "help": "Split deeply branched behavior into smaller focused functions or table-driven helpers.",
        "evidence": [
          {
            "label": "function",
            "value": "AuthorizePayment"
          },
          {
            "label": "complexity",
            "value": "9"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "888fcc2edc346045"
      },
      {
        "rule_id": "examples/go-import-boundaries",
        "severity": "error",
        "file": "src/payment.go",
        "range": {
          "start_line": 7,
          "start_col": 8,
          "end_line": 7,
          "end_col": 38
        },
        "message": "Go import `github.com/acme/legacy/auth` violates configured import boundary.",
        "labels": [],
        "help": "Move the dependency behind an allowed interface or update the boundary config if this is intentional.",
        "evidence": [
          {
            "label": "import",
            "value": "github.com/acme/legacy/auth"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "172187f404b7e3b8"
      },
      {
        "rule_id": "examples/go-branch-obligations",
        "severity": "warn",
        "file": "src/payment.go",
        "range": {
          "start_line": 14,
          "start_col": 5,
          "end_line": 14,
          "end_col": 15
        },
        "message": "No nearby test evidence found for Go branch `err != nil`.",
        "labels": [],
        "help": "Add a test case that exercises this branch. This rule is heuristic and does not prove exact coverage.",
        "evidence": [
          {
            "label": "condition",
            "value": "err != nil"
          },
          {
            "label": "edge",
            "value": "true"
          },
          {
            "label": "branch_fingerprint",
            "value": "branch-auth-error"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "5fe72dd727ae6899"
      },
      {
        "rule_id": "examples/go-test-suite-size",
        "severity": "warn",
        "file": "src/payment_test.go",
        "range": {
          "start_line": 8,
          "start_col": 6,
          "end_line": 8,
          "end_col": 23
        },
        "message": "Go test `TestPaymentMatrix` has heuristic maintainability score 25, max 24.",
        "labels": [],
        "help": "Split this test suite into smaller behavior-focused suites. This rule is heuristic.",
        "evidence": [
          {
            "label": "score",
            "value": "25"
          },
          {
            "label": "subtests",
            "value": "3"
          },
          {
            "label": "table_rows",
            "value": "5"
          },
          {
            "label": "assertions",
            "value": "2"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "6d853dc1d70a8926"
      },
      {
        "rule_id": "examples/go-assertion-after-action",
        "severity": "warn",
        "file": "src/payment_test.go",
        "range": {
          "start_line": 32,
          "start_col": 6,
          "end_line": 32,
          "end_col": 23
        },
        "message": "Go test `TestPaymentAction` has no obvious assertion or error check.",
        "labels": [],
        "help": "Add an explicit assertion, error check, or failure path. This rule is heuristic.",
        "evidence": [
          {
            "label": "test",
            "value": "TestPaymentAction"
          },
          {
            "label": "assertions",
            "value": "0"
          },
          {
            "label": "evidence_terms",
            "value": "charge action"
          }
        ],
        "suggestions": [],
        "fix": null,
        "stable_fingerprint": "5581e444ecf89a1d"
      }
    ]
    "###);
}
