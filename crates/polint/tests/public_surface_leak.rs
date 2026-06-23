// V1.3 LEAK GATE — see Phase 42 D-17, D-18, D-19
//
// This test compiles the probe crate at tests/fixtures/public-surface-leak-probe/
// and asserts that the v1.0–v1.2 polint::sdk::prelude::* allow-list is FROZEN
// for the v1.3 milestone.
//
// The ALLOWED_PRELUDE constant below is the source of truth. Phases 43–54 MUST NOT
// extend this list. Extending the list is a deliberate API change that requires
// milestone-close review and a documented promotion record under
// docs/API-VISIBILITY-PLAN.md before it can be merged.
//
// The gate runs on Linux + macOS in fast CI on every PR (D-18). Both platforms
// must pass independently — no averaging, no skipping either platform.
//
// Strategy: APPROACH B (direct cargo invocation, no new dependency). The gate
// shells out to `cargo build` on the excluded probe crate and asserts a clean
// compile, then snapshot-compares the prelude re-export block against
// ALLOWED_PRELUDE. trybuild was deliberately NOT added — the no-new-deps
// discipline of Plans 01/02 (T-42-SC) carries here, and a direct cargo
// invocation gives rustc-level granularity without a UI-test dependency.
//
// If you are reading this comment because the test failed:
//   1. Did you accidentally add `pub` to a v1.3 type? Make it `pub(crate)`.
//   2. Did you add a sanctioned new public type via milestone-close review?
//      Extend ALLOWED_PRELUDE in the same PR and reference the review record,
//      and add a witness for it in the probe crate's allowlist_witness module.
//   3. Did the probe crate's dependency on polint break? Re-read Plan 04 of
//      Phase 42 and restore the probe's `#![no_implicit_prelude]` + single
//      `use ::polint::sdk::prelude::*;` import.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The FROZEN v1.0–v1.2 `polint::sdk::prelude` re-export allow-list.
///
/// Source of truth: `crates/polint/src/sdk/mod.rs` `pub mod prelude { ... }`.
/// `allowlist_matches_prelude_source` asserts the parsed prelude block equals
/// this set EXACTLY. Any addition (sanctioned or not) trips the gate; sanctioned
/// additions extend this list in the same PR with a documented promotion record
/// in `docs/API-VISIBILITY-PLAN.md` (D-19).
const ALLOWED_PRELUDE: &[&str] = &[
    // crate::core types
    "BranchId",
    "BranchObligation",
    "CapabilitySupport",
    "CapabilitySupportStatus",
    "CapabilitySupportView",
    "ChangeStatus",
    "ComplexityMetricFact",
    "CoverageFact",
    "DefinitionFact",
    "DefinitionId",
    "DefinitionKind",
    "FileId",
    "FileMetricFact",
    "FunctionFact",
    "FunctionId",
    "FunctionMetricFact",
    "ImportFact",
    "ImportId",
    "JsxAttributeFact",
    "Language",
    "ModuleEdge",
    "ModuleEdgeId",
    "ModuleEdgeKind",
    "ModuleNode",
    "ModuleNodeId",
    "ModuleNodeKind",
    "NodeId",
    "PackageFact",
    "PackageId",
    "ReferenceFact",
    "ReferenceId",
    "ReferenceKind",
    "ResolutionPrecision",
    "ResolutionStatus",
    "ResolvedImportFact",
    "ResolvedImportId",
    "Rule",
    "RuleConfigValue",
    "RuleCtx",
    "RuleId",
    "RuleOptions",
    "SourceFile",
    "Span",
    "StringLiteralFact",
    "SymbolFact",
    "SymbolId",
    "SymbolKind",
    "SymbolNamespace",
    "SymbolPrecision",
    "SymbolResolutionStatus",
    "TestFact",
    "TextRange",
    "TsClassFact",
    "TsComponentFact",
    "UnresolvedReason",
    // crate::diagnostics
    "ColorChoice",
    "Diagnostic",
    "Evidence",
    "Fix",
    "JsonReportMeta",
    "Label",
    "OutputFormat",
    "POLINT_REPORT_JSON_SCHEMA_V1_URL",
    "PolintReport",
    "PolintToolInfo",
    "RenderOpts",
    "Severity",
    "Suggestion",
    "DiagnosticRange", // re-export alias: `TextRange as DiagnosticRange`
    "diagnostics_from_json_report",
    // crate::rule_error
    "RuleError",
    "RuleResult",
    // crate::sdk free fn
    "collect_go_tests",
    // crate::sdk::facts
    "BranchObligations",
    "CallGraph",
    "ChangedFiles",
    "Cfg",
    "ComplexityMetrics",
    "CoverageFacts",
    "DataFlow",
    "FileMetrics",
    "FunctionMetrics",
    "Functions",
    "GoTests",
    "Imports",
    "JsxAttributes",
    "ModuleGraphFacts",
    "Packages",
    "References",
    "ResolvedImports",
    "SourceFiles",
    "StringLiterals",
    "Symbols",
    "TestSuiteMetrics",
    "TsClasses",
    "TsComponents",
    // crate::sdk::scope
    "file_in_scope",
    "file_matches_globs",
    "glob_matches",
];

/// Repository root, derived from this crate's manifest dir (`crates/polint`).
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/polint has a workspace root two levels up")
        .to_path_buf()
}

fn probe_manifest_path() -> PathBuf {
    repo_root().join("tests/fixtures/public-surface-leak-probe/Cargo.toml")
}

fn sdk_mod_source() -> String {
    let path = repo_root().join("crates/polint/src/sdk/mod.rs");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn probe_lib_source() -> String {
    let path = repo_root().join("tests/fixtures/public-surface-leak-probe/src/lib.rs");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

/// Parses the `pub mod prelude { ... }` block of `sdk/mod.rs` and returns the
/// set of identifier names re-exported through it.
///
/// BLOCKER #6: this helper is exercised directly by
/// `parser_self_test_detects_synthetic_leak` so a buggy parser cannot silently
/// hollow out the gate. It returns the FINAL nameable identifier for each
/// re-export, honoring `X as Y` aliases (yielding `Y`).
pub fn parse_prelude_reexports(source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    let Some(block_start) = source.find("pub mod prelude") else {
        return names;
    };
    let after_kw = &source[block_start..];
    let Some(open_rel) = after_kw.find('{') else {
        return names;
    };

    // Walk from the opening brace tracking depth so we stop at the prelude
    // module's matching close brace and never read sibling modules.
    let bytes = after_kw.as_bytes();
    let mut depth = 0usize;
    let mut end = open_rel;
    for (i, &b) in bytes.iter().enumerate().skip(open_rel) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    let block = &after_kw[open_rel + 1..end];

    // Collect every `pub use ...;` statement, then extract the leaf identifiers.
    for raw_stmt in block.split(';') {
        let stmt = raw_stmt.trim();
        let Some(rest) = stmt.strip_prefix("pub use ") else {
            continue;
        };
        if let Some(brace_open) = rest.find('{') {
            // Grouped: `pub use crate::path::{A, B, C as D}`
            let Some(brace_close) = rest.rfind('}') else {
                continue;
            };
            let inner = &rest[brace_open + 1..brace_close];
            for item in inner.split(',') {
                if let Some(name) = leaf_identifier(item) {
                    names.insert(name);
                }
            }
        } else {
            // Single: `pub use crate::path::name` (optionally `as alias`)
            if let Some(name) = leaf_identifier(rest) {
                names.insert(name);
            }
        }
    }

    names
}

/// Extracts the final nameable identifier from a single re-export item,
/// honoring `X as Y` aliases (returns `Y`) and `path::Leaf` paths (returns
/// `Leaf`). Returns `None` for empty / glob items.
fn leaf_identifier(item: &str) -> Option<String> {
    let item = item.trim();
    if item.is_empty() || item == "*" {
        return None;
    }
    // Honor `... as alias` first.
    if let Some((_, alias)) = item.rsplit_once(" as ") {
        let alias = alias.trim();
        return (!alias.is_empty()).then(|| alias.to_string());
    }
    // Otherwise take the final path segment.
    let leaf = item.rsplit("::").next().unwrap_or(item).trim();
    (!leaf.is_empty() && leaf != "*").then(|| leaf.to_string())
}

#[test]
fn probe_crate_compiles_against_prelude_only() {
    let manifest = probe_manifest_path();
    assert!(
        manifest.exists(),
        "probe manifest missing at {} — see Phase 42 Plan 04",
        manifest.display()
    );

    // NOTE: intentionally NOT `--locked`. This gate only proves the probe still
    // compiles against `polint::sdk::prelude::*`; pinning the probe's transitive
    // dep versions is not its job. The probe's Cargo.lock pins `polint` by version,
    // but release bumps update the workspace lock without touching the excluded
    // probe lock, so `--locked` would abort with "cannot update the lock file" on
    // every release. Letting cargo refresh the stale version entry in-memory keeps
    // the gate robust. (The OUTER `cargo test --locked` in CI still validates the
    // workspace lock.)
    let output = Command::new(env!("CARGO"))
        .args([
            "build",
            "--manifest-path",
            manifest.to_str().expect("utf-8 manifest path"),
            "--message-format=short",
        ])
        .output()
        .expect("failed to invoke cargo to build the leak-gate probe crate");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "leak-gate probe crate FAILED to compile against `polint::sdk::prelude::*`.\n\
         A v1.0–v1.2 allow-listed identifier was likely dropped from the prelude, \
         or the probe was tampered with.\n\
         See crates/polint/tests/public_surface_leak.rs top comment.\n\
         --- cargo stdout ---\n{stdout}\n--- cargo stderr ---\n{stderr}"
    );

    assert!(
        !stderr.contains("error[E0"),
        "leak-gate probe emitted rustc errors:\n{stderr}"
    );
}

#[test]
fn allowlist_matches_prelude_source() {
    let source = sdk_mod_source();
    let actual = parse_prelude_reexports(&source);
    let expected: BTreeSet<String> = ALLOWED_PRELUDE.iter().map(|s| s.to_string()).collect();

    let unsanctioned: Vec<&String> = actual.difference(&expected).collect();
    let missing: Vec<&String> = expected.difference(&actual).collect();

    assert!(
        unsanctioned.is_empty() && missing.is_empty(),
        "polint::sdk::prelude exports diverged from the v1.3 locked allow-list — \
         see crates/polint/tests/public_surface_leak.rs top comment for the discipline policy.\n\
         UNSANCTIONED additions (in prelude, NOT in ALLOWED_PRELUDE): {unsanctioned:?}\n\
         MISSING (in ALLOWED_PRELUDE, NOT in prelude): {missing:?}"
    );

    // Defensive cross-check: the prelude block must never re-export private
    // analysis namespaces directly.
    let block_start = source
        .find("pub mod prelude")
        .expect("prelude block exists");
    let block = &source[block_start..];
    let block_end = block
        .find("\n}")
        .map(|i| i + block_start)
        .unwrap_or(source.len());
    let prelude_block = &source[block_start..block_end];
    assert!(
        !prelude_block.contains("pub use crate::analysis::")
            && !prelude_block.contains("pub use crate::analysis_kernel::"),
        "polint::sdk::prelude re-exports a private analysis namespace — this is a v1.3 leak (D-23).\n{prelude_block}"
    );
}

#[test]
fn ensure_no_private_namespace_in_probe() {
    let src = probe_lib_source();

    // (a) #![no_implicit_prelude] must be present (T-42-04-04 mitigation).
    assert!(
        src.lines().any(|l| l.trim() == "#![no_implicit_prelude]"),
        "probe lib.rs is missing `#![no_implicit_prelude]` — removing it lets std's \
         prelude mask accidental polint-prelude additions (T-42-04-04). \
         Restore it per Phase 42 Plan 04."
    );

    // (b) EXACTLY one `use polint::` line, and it is the prelude glob
    // (the leading `::` is required under no_implicit_prelude; T-42-04-05).
    let polint_use_lines: Vec<&str> = src
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("use ") && l.contains("polint::"))
        .collect();
    assert_eq!(
        polint_use_lines.len(),
        1,
        "probe lib.rs must contain EXACTLY one `use polint::` import; found: {polint_use_lines:?} \
         (T-42-04-05 — the single glob is the catch-all)"
    );
    assert!(
        polint_use_lines[0] == "use ::polint::sdk::prelude::*;"
            || polint_use_lines[0] == "use polint::sdk::prelude::*;",
        "probe lib.rs's single polint import must be the prelude glob, found: {}",
        polint_use_lines[0]
    );

    // (c) ZERO private-namespace path substrings anywhere in executable code.
    // Strip comment lines first so the cautionary doc-comment listing the
    // forbidden namespaces does not trip the check.
    let code_only: String = src
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "polint::analysis",
        "polint::analysis_kernel",
        "polint::core",
        "polint::cache",
        "polint::config",
        "polint::cli",
        "polint::go",
        "polint::ts",
        "polint::graph",
        "polint::eval",
        "polint::rule_manifest",
    ] {
        assert!(
            !code_only.contains(forbidden),
            "probe lib.rs references private namespace `{forbidden}` in code — the whole point \
             is that private types are NOT nameable here (D-23). Remove it."
        );
    }
}

#[test]
fn parser_self_test_detects_synthetic_leak() {
    // Negative control: a synthetic prelude block that smuggles in an
    // unsanctioned private re-export. The parser MUST surface `IdentityRecord`.
    let synthetic_leak = r#"
        pub mod prelude {
            pub use crate::core::{Rule, RuleCtx, RuleId};
            pub use crate::diagnostics::{Diagnostic, Severity};
            pub use crate::analysis::identity::IdentityRecord;
        }
    "#;
    let parsed = parse_prelude_reexports(synthetic_leak);
    assert!(
        parsed.contains("IdentityRecord"),
        "parse_prelude_reexports FAILED to detect the synthetic `IdentityRecord` leak — \
         the gate's parser is broken and would silently accept unsanctioned additions \
         (BLOCKER #6). Parsed set was: {parsed:?}"
    );

    // Positive control: a synthetic block with ONLY allow-listed names must
    // parse to exactly that set, with no false positives and no dropped names.
    let synthetic_clean = r#"
        pub mod prelude {
            pub use crate::core::{Rule, RuleCtx};
            pub use crate::diagnostics::{Diagnostic};
            pub use crate::diagnostics::{TextRange as DiagnosticRange};
        }
    "#;
    let parsed_clean = parse_prelude_reexports(synthetic_clean);
    let expected_clean: BTreeSet<String> = ["Rule", "RuleCtx", "Diagnostic", "DiagnosticRange"]
        .into_iter()
        .map(str::to_string)
        .collect();
    assert_eq!(
        parsed_clean, expected_clean,
        "parse_prelude_reexports produced a false positive or dropped an allow-listed name \
         (BLOCKER #6). Expected {expected_clean:?}, got {parsed_clean:?}"
    );
    // Every name returned by the clean control IS in the real allow-list.
    let allowed: BTreeSet<String> = ALLOWED_PRELUDE.iter().map(|s| s.to_string()).collect();
    for name in &parsed_clean {
        assert!(
            allowed.contains(name),
            "parser self-test positive control yielded `{name}`, which is not in ALLOWED_PRELUDE"
        );
    }
}

#[test]
fn allowlist_has_no_duplicates_and_expected_count() {
    let unique: BTreeSet<&&str> = ALLOWED_PRELUDE.iter().collect();
    assert_eq!(
        unique.len(),
        ALLOWED_PRELUDE.len(),
        "ALLOWED_PRELUDE contains duplicate entries"
    );
    // Locked count derived from sdk/mod.rs:28–53 at Phase 42 Plan 04 landing.
    // Bumped 97 -> 99 for the sanctioned review-rules API addition (ChangedFiles +
    // ChangeStatus); see docs/REVIEW-RULES-PLAN.md T2 and the docs/API-VISIBILITY-PLAN.md
    // review-rules promotion record (D-19).
    assert_eq!(
        ALLOWED_PRELUDE.len(),
        99,
        "ALLOWED_PRELUDE count changed — update this assertion ONLY alongside a sanctioned \
         milestone-close API change (D-19)"
    );
}
