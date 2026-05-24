use std::collections::{BTreeMap, BTreeSet};

use crate::analysis::calls::facts::{CallCallee, CallSiteFact, CallSyntaxKind};
use crate::analysis::entrypoints::facts::{
    EntrypointConfidence, EntrypointFact, EntrypointKind, EntrypointPrecision,
    EntrypointProvenance, EntrypointStatus, TriggerMetadata, UnresolvedFrameworkFact,
    UnresolvedFrameworkReason,
};
use crate::analysis::entrypoints::provider::ENTRYPOINTS_PROVIDER_ID;
use crate::analysis::ids::{EntrypointId, UnresolvedFrameworkId};
use crate::analysis::stable_key::semantic_stable_key;
use crate::analysis_kernel::{FactFamily, FactRef};
use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, Language, Span};

// ---------------------------------------------------------------------------
// Public output
// ---------------------------------------------------------------------------

pub(crate) struct TsRecognizerOutput {
    pub(crate) entrypoints: Vec<EntrypointFact>,
    pub(crate) unresolved: Vec<UnresolvedFrameworkFact>,
}

// ---------------------------------------------------------------------------
// Framework detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TsFramework {
    Express,
    McpSdk,
    Jest,
    Vitest,
    Mocha,
    Commander,
    Yargs,
}

fn classify_import(path: &str) -> Option<(&'static str, TsFramework)> {
    match path {
        "express" => Some(("ts.express", TsFramework::Express)),
        "commander" => Some(("ts.commander", TsFramework::Commander)),
        "yargs" => Some(("ts.yargs", TsFramework::Yargs)),
        "jest" | "@jest/globals" => Some(("ts.jest", TsFramework::Jest)),
        "vitest" => Some(("ts.vitest", TsFramework::Vitest)),
        "mocha" => Some(("ts.mocha", TsFramework::Mocha)),
        _ if path.starts_with("@modelcontextprotocol/") => {
            Some(("ts.mcp_sdk", TsFramework::McpSdk))
        }
        _ => None,
    }
}

/// Import paths that look like TS/JS web/HTTP framework packages but are not
/// covered by default recognizers (Express, MCP SDK). Per D-10 these should
/// produce UnresolvedFrameworkFact rows.
fn is_unrecognized_framework_import(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let markers = [
        "fastify", "koa", "hapi", "nest", "@nestjs", "next", "nuxt", "remix", "sveltekit",
        "astro",
    ];
    // Exclude known frameworks we handle natively
    if path == "express"
        || path.starts_with("@modelcontextprotocol/")
        || path == "commander"
        || path == "yargs"
        || path == "jest"
        || path == "@jest/globals"
        || path == "vitest"
        || path == "mocha"
    {
        return false;
    }
    markers.iter().any(|marker| lower.contains(marker))
}

// ---------------------------------------------------------------------------
// Main recognizer entry point
// ---------------------------------------------------------------------------

pub(crate) fn recognize_ts_entrypoints(db: &AnalysisDb) -> TsRecognizerOutput {
    let _files_by_id: BTreeMap<FileId, &crate::core::SourceFile> =
        db.files().iter().map(|file| (file.id, file)).collect();
    let functions_by_id: BTreeMap<FunctionId, &FunctionFact> =
        db.functions().iter().map(|f| (f.id, f)).collect();

    // Build per-file framework imports index
    let mut file_frameworks: BTreeMap<FileId, Vec<(&str, TsFramework, Span)>> = BTreeMap::new();
    let mut unrecognized_imports: Vec<(FileId, String, Span)> = Vec::new();

    for import in db.imports() {
        if !matches!(import.language, Language::TypeScript | Language::JavaScript) {
            continue;
        }
        if let Some((framework_id, framework)) = classify_import(&import.path) {
            file_frameworks
                .entry(import.file)
                .or_default()
                .push((framework_id, framework, import.span.clone()));
        } else if is_unrecognized_framework_import(&import.path) {
            unrecognized_imports.push((import.file, import.path.clone(), import.span.clone()));
        }
    }

    let mut entrypoints = Vec::new();
    let mut unresolved = Vec::new();

    // 1. Detect Express entrypoints from call sites
    recognize_express_entrypoints(
        db,
        &functions_by_id,
        &file_frameworks,
        &mut entrypoints,
        &mut unresolved,
    );

    // 2. Detect MCP SDK entrypoints from call sites
    recognize_mcp_entrypoints(
        db,
        &functions_by_id,
        &file_frameworks,
        &mut entrypoints,
        &mut unresolved,
    );

    // 3. Detect test framework entrypoints (jest, vitest, mocha)
    recognize_test_entrypoints(
        db,
        &functions_by_id,
        &file_frameworks,
        &mut entrypoints,
    );

    // 4. Detect CLI framework entrypoints (commander, yargs)
    recognize_cli_entrypoints(
        db,
        &functions_by_id,
        &file_frameworks,
        &mut entrypoints,
        &mut unresolved,
    );

    // 5. Emit UnresolvedFrameworkFact for unrecognized TS/JS framework imports (D-10)
    for (file, import_path, span) in &unrecognized_imports {
        let file_key = file_stable_key(db, *file);
        let stable_key = semantic_stable_key(
            FactFamily::UnresolvedFramework,
            &[
                ("language", "TypeScript".to_string()),
                ("file_key", file_key),
                ("import_path", import_path.clone()),
            ],
        )
        .into_string();

        unresolved.push(UnresolvedFrameworkFact {
            id: UnresolvedFrameworkId(0),
            language: Language::TypeScript,
            file: *file,
            span: span.clone(),
            framework_id: format!("ts.unknown:{import_path}"),
            reason: UnresolvedFrameworkReason::UnsupportedFrameworkVersion,
            evidence: format!("TS/JS import \"{import_path}\" detected"),
            scope_description: format!("unrecognized TS/JS framework import: {import_path}"),
            precision: EntrypointPrecision::Conservative,
            provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
            stable_key,
        });
    }

    // 6. Check for framework imports without matching patterns (D-10)
    emit_unresolved_for_unused_frameworks(
        db,
        &file_frameworks,
        &entrypoints,
        &mut unresolved,
    );

    // Sort output by stable key
    entrypoints.sort_by(|a, b| a.stable_key.cmp(&b.stable_key));
    unresolved.sort_by(|a, b| a.stable_key.cmp(&b.stable_key));

    TsRecognizerOutput {
        entrypoints,
        unresolved,
    }
}

// ---------------------------------------------------------------------------
// Express recognizer (D-07)
// ---------------------------------------------------------------------------

/// Express HTTP method names recognized on app/router method calls.
const EXPRESS_HTTP_METHODS: &[&str] = &[
    "get", "post", "put", "delete", "patch", "options", "head",
];

fn recognize_express_entrypoints(
    db: &AnalysisDb,
    functions_by_id: &BTreeMap<FunctionId, &FunctionFact>,
    file_frameworks: &BTreeMap<FileId, Vec<(&str, TsFramework, Span)>>,
    entrypoints: &mut Vec<EntrypointFact>,
    _unresolved: &mut Vec<UnresolvedFrameworkFact>,
) {
    for site in db.call_sites() {
        if !matches!(site.language, Language::TypeScript | Language::JavaScript) {
            continue;
        }

        let frameworks = match file_frameworks.get(&site.file) {
            Some(fw) => fw,
            None => continue,
        };

        let has_express = frameworks
            .iter()
            .any(|(_, fw, _)| *fw == TsFramework::Express);
        if !has_express {
            continue;
        }

        match &site.callee {
            // Match app.get/post/put/delete/patch/options/head calls
            CallCallee::Member { property, .. }
                if EXPRESS_HTTP_METHODS.contains(&property.as_str())
                    && matches!(
                        site.kind,
                        CallSyntaxKind::Member | CallSyntaxKind::StaticMember
                    ) =>
            {
                let route_path = extract_first_arg_literal(db, site);
                let handler_function = resolve_handler_function(functions_by_id, site);
                let file_key = file_stable_key(db, site.file);
                let method = property.to_uppercase();

                if let Some(target_function) = handler_function {
                    let stable_key = entrypoint_stable_key(
                        "TypeScript",
                        &file_key,
                        "ts.express",
                        "HttpRoute",
                        &format!("{}", target_function.0),
                        &span_key(&site.span),
                    );

                    entrypoints.push(EntrypointFact {
                        id: EntrypointId(0),
                        language: site.language,
                        framework_id: "ts.express".to_string(),
                        kind: EntrypointKind::HttpRoute,
                        target_function,
                        target_symbol: None,
                        registration_span: site.span.clone(),
                        registration_file: site.file,
                        trigger_metadata: TriggerMetadata {
                            method: Some(method),
                            path: route_path,
                            tool_name: None,
                            event_name: None,
                            test_name: None,
                        },
                        trust_boundary_link: None,
                        precision: EntrypointPrecision::Heuristic,
                        provenance: EntrypointProvenance::NativeRecognizer,
                        confidence: EntrypointConfidence::High,
                        status: EntrypointStatus::Resolved,
                        provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
                        stable_key,
                    });
                }
            }

            // Match app.use calls for middleware
            CallCallee::Member { property, .. }
                if property == "use"
                    && matches!(
                        site.kind,
                        CallSyntaxKind::Member | CallSyntaxKind::StaticMember
                    ) =>
            {
                let handler_function = resolve_handler_function(functions_by_id, site);
                let file_key = file_stable_key(db, site.file);

                if let Some(target_function) = handler_function {
                    let stable_key = entrypoint_stable_key(
                        "TypeScript",
                        &file_key,
                        "ts.express",
                        "HttpMiddleware",
                        &format!("{}", target_function.0),
                        &span_key(&site.span),
                    );

                    entrypoints.push(EntrypointFact {
                        id: EntrypointId(0),
                        language: site.language,
                        framework_id: "ts.express".to_string(),
                        kind: EntrypointKind::HttpMiddleware,
                        target_function,
                        target_symbol: None,
                        registration_span: site.span.clone(),
                        registration_file: site.file,
                        trigger_metadata: TriggerMetadata::empty(),
                        trust_boundary_link: None,
                        precision: EntrypointPrecision::Heuristic,
                        provenance: EntrypointProvenance::NativeRecognizer,
                        confidence: EntrypointConfidence::Medium,
                        status: EntrypointStatus::Resolved,
                        provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
                        stable_key,
                    });
                }
            }

            // Match app.route("/path").get(handler).post(handler) chain
            // The route() call itself: emit HttpRoute for the route prefix
            CallCallee::Member { property, .. }
                if property == "route"
                    && matches!(
                        site.kind,
                        CallSyntaxKind::Member | CallSyntaxKind::StaticMember
                    ) =>
            {
                let route_path = extract_first_arg_literal(db, site);
                let handler_function = resolve_handler_function(functions_by_id, site);
                let file_key = file_stable_key(db, site.file);

                if let Some(target_function) = handler_function {
                    let stable_key = entrypoint_stable_key(
                        "TypeScript",
                        &file_key,
                        "ts.express",
                        "HttpRoute",
                        &format!("{}", target_function.0),
                        &span_key(&site.span),
                    );

                    entrypoints.push(EntrypointFact {
                        id: EntrypointId(0),
                        language: site.language,
                        framework_id: "ts.express".to_string(),
                        kind: EntrypointKind::HttpRoute,
                        target_function,
                        target_symbol: None,
                        registration_span: site.span.clone(),
                        registration_file: site.file,
                        trigger_metadata: TriggerMetadata {
                            method: None,
                            path: route_path,
                            tool_name: None,
                            event_name: None,
                            test_name: None,
                        },
                        trust_boundary_link: None,
                        precision: EntrypointPrecision::Heuristic,
                        provenance: EntrypointProvenance::NativeRecognizer,
                        confidence: EntrypointConfidence::Medium,
                        status: EntrypointStatus::Resolved,
                        provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
                        stable_key,
                    });
                }
            }

            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// MCP TypeScript SDK recognizer (D-07)
// ---------------------------------------------------------------------------

/// MCP SDK registration method names.
const MCP_METHODS: &[(&str, EntrypointKind)] = &[
    ("tool", EntrypointKind::McpTool),
    ("resource", EntrypointKind::McpResource),
    ("prompt", EntrypointKind::McpPrompt),
];

fn recognize_mcp_entrypoints(
    db: &AnalysisDb,
    functions_by_id: &BTreeMap<FunctionId, &FunctionFact>,
    file_frameworks: &BTreeMap<FileId, Vec<(&str, TsFramework, Span)>>,
    entrypoints: &mut Vec<EntrypointFact>,
    _unresolved: &mut Vec<UnresolvedFrameworkFact>,
) {
    for site in db.call_sites() {
        if !matches!(site.language, Language::TypeScript | Language::JavaScript) {
            continue;
        }

        let frameworks = match file_frameworks.get(&site.file) {
            Some(fw) => fw,
            None => continue,
        };

        let has_mcp_sdk = frameworks
            .iter()
            .any(|(_, fw, _)| *fw == TsFramework::McpSdk);
        if !has_mcp_sdk {
            continue;
        }

        if let CallCallee::Member { property, .. } = &site.callee {
            if !matches!(
                site.kind,
                CallSyntaxKind::Member | CallSyntaxKind::StaticMember
            ) {
                continue;
            }

            if let Some((_, kind)) = MCP_METHODS
                .iter()
                .find(|(method, _)| *method == property.as_str())
            {
                let tool_name = extract_first_arg_literal(db, site);
                let handler_function = resolve_handler_function(functions_by_id, site);
                let file_key = file_stable_key(db, site.file);
                let kind_label = format!("{kind:?}");

                if let Some(target_function) = handler_function {
                    let stable_key = entrypoint_stable_key(
                        "TypeScript",
                        &file_key,
                        "ts.mcp_sdk",
                        &kind_label,
                        &format!("{}", target_function.0),
                        &span_key(&site.span),
                    );

                    entrypoints.push(EntrypointFact {
                        id: EntrypointId(0),
                        language: site.language,
                        framework_id: "ts.mcp_sdk".to_string(),
                        kind: *kind,
                        target_function,
                        target_symbol: None,
                        registration_span: site.span.clone(),
                        registration_file: site.file,
                        trigger_metadata: TriggerMetadata {
                            method: None,
                            path: None,
                            tool_name,
                            event_name: None,
                            test_name: None,
                        },
                        trust_boundary_link: None,
                        precision: EntrypointPrecision::Heuristic,
                        provenance: EntrypointProvenance::NativeRecognizer,
                        confidence: EntrypointConfidence::High,
                        status: EntrypointStatus::Resolved,
                        provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
                        stable_key,
                    });
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Test framework recognizer (D-08)
// ---------------------------------------------------------------------------

/// Test function identifiers that indicate test entrypoints.
const TEST_CALL_NAMES: &[&str] = &["describe", "it", "test"];

fn is_test_framework(fw: TsFramework) -> bool {
    matches!(
        fw,
        TsFramework::Jest | TsFramework::Vitest | TsFramework::Mocha
    )
}

fn framework_id_for_test(fw: TsFramework) -> &'static str {
    match fw {
        TsFramework::Jest => "ts.jest",
        TsFramework::Vitest => "ts.vitest",
        TsFramework::Mocha => "ts.mocha",
        _ => "ts.test",
    }
}

fn recognize_test_entrypoints(
    db: &AnalysisDb,
    _functions_by_id: &BTreeMap<FunctionId, &FunctionFact>,
    file_frameworks: &BTreeMap<FileId, Vec<(&str, TsFramework, Span)>>,
    entrypoints: &mut Vec<EntrypointFact>,
) {
    for site in db.call_sites() {
        if !matches!(site.language, Language::TypeScript | Language::JavaScript) {
            continue;
        }

        let frameworks = match file_frameworks.get(&site.file) {
            Some(fw) => fw,
            None => continue,
        };

        // Find the test framework import for this file
        let test_fw = frameworks
            .iter()
            .find(|(_, fw, _)| is_test_framework(*fw));
        let test_fw = match test_fw {
            Some((_, fw, _)) => *fw,
            None => continue,
        };

        // Match describe/it/test identifier calls
        let is_test_call = match &site.callee {
            CallCallee::Identifier { name, .. } => TEST_CALL_NAMES.contains(&name.as_str()),
            // Also match member calls like vitest.describe or jest.test
            CallCallee::Member { property, .. } => TEST_CALL_NAMES.contains(&property.as_str()),
            _ => false,
        };

        if !is_test_call {
            continue;
        }

        let test_name = extract_first_arg_literal(db, site);
        let file_key = file_stable_key(db, site.file);
        let fw_id = framework_id_for_test(test_fw);

        let callee_name = match &site.callee {
            CallCallee::Identifier { name, .. } => name.as_str(),
            CallCallee::Member { property, .. } => property.as_str(),
            _ => "test",
        };

        let stable_key = entrypoint_stable_key(
            "TypeScript",
            &file_key,
            fw_id,
            "Test",
            callee_name,
            &span_key(&site.span),
        );

        entrypoints.push(EntrypointFact {
            id: EntrypointId(0),
            language: site.language,
            framework_id: fw_id.to_string(),
            kind: EntrypointKind::Test,
            target_function: site.caller,
            target_symbol: None,
            registration_span: site.span.clone(),
            registration_file: site.file,
            trigger_metadata: TriggerMetadata {
                method: None,
                path: None,
                tool_name: None,
                event_name: None,
                test_name,
            },
            trust_boundary_link: None,
            precision: EntrypointPrecision::SetupAware,
            provenance: EntrypointProvenance::NativeRecognizer,
            confidence: EntrypointConfidence::High,
            status: EntrypointStatus::Resolved,
            provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
            stable_key,
        });
    }
}

// ---------------------------------------------------------------------------
// CLI framework recognizer (D-09)
// ---------------------------------------------------------------------------

fn recognize_cli_entrypoints(
    db: &AnalysisDb,
    functions_by_id: &BTreeMap<FunctionId, &FunctionFact>,
    file_frameworks: &BTreeMap<FileId, Vec<(&str, TsFramework, Span)>>,
    entrypoints: &mut Vec<EntrypointFact>,
    _unresolved: &mut Vec<UnresolvedFrameworkFact>,
) {
    for site in db.call_sites() {
        if !matches!(site.language, Language::TypeScript | Language::JavaScript) {
            continue;
        }

        let frameworks = match file_frameworks.get(&site.file) {
            Some(fw) => fw,
            None => continue,
        };

        let has_commander = frameworks
            .iter()
            .any(|(_, fw, _)| *fw == TsFramework::Commander);
        let has_yargs = frameworks
            .iter()
            .any(|(_, fw, _)| *fw == TsFramework::Yargs);

        if !has_commander && !has_yargs {
            continue;
        }

        match &site.callee {
            // Commander: program.command("name") or .action(handler)
            CallCallee::Member { property, .. }
                if has_commander
                    && (property == "command" || property == "action")
                    && matches!(
                        site.kind,
                        CallSyntaxKind::Member | CallSyntaxKind::StaticMember
                    ) =>
            {
                let command_name = extract_first_arg_literal(db, site);
                let handler_function = resolve_handler_function(functions_by_id, site);
                let file_key = file_stable_key(db, site.file);

                if let Some(target_function) = handler_function {
                    let stable_key = entrypoint_stable_key(
                        "TypeScript",
                        &file_key,
                        "ts.commander",
                        "CliCommand",
                        &format!("{}", target_function.0),
                        &span_key(&site.span),
                    );

                    entrypoints.push(EntrypointFact {
                        id: EntrypointId(0),
                        language: site.language,
                        framework_id: "ts.commander".to_string(),
                        kind: EntrypointKind::CliCommand,
                        target_function,
                        target_symbol: None,
                        registration_span: site.span.clone(),
                        registration_file: site.file,
                        trigger_metadata: TriggerMetadata {
                            method: None,
                            path: None,
                            tool_name: command_name,
                            event_name: None,
                            test_name: None,
                        },
                        trust_boundary_link: None,
                        precision: EntrypointPrecision::Conservative, // Per D-09
                        provenance: EntrypointProvenance::NativeRecognizer,
                        confidence: EntrypointConfidence::Medium,
                        status: EntrypointStatus::Resolved,
                        provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
                        stable_key,
                    });
                }
            }

            // Yargs: yargs.command("name", ...)
            CallCallee::Member { property, .. }
                if has_yargs
                    && property == "command"
                    && matches!(
                        site.kind,
                        CallSyntaxKind::Member | CallSyntaxKind::StaticMember
                    ) =>
            {
                let command_name = extract_first_arg_literal(db, site);
                let handler_function = resolve_handler_function(functions_by_id, site);
                let file_key = file_stable_key(db, site.file);

                if let Some(target_function) = handler_function {
                    let stable_key = entrypoint_stable_key(
                        "TypeScript",
                        &file_key,
                        "ts.yargs",
                        "CliCommand",
                        &format!("{}", target_function.0),
                        &span_key(&site.span),
                    );

                    entrypoints.push(EntrypointFact {
                        id: EntrypointId(0),
                        language: site.language,
                        framework_id: "ts.yargs".to_string(),
                        kind: EntrypointKind::CliCommand,
                        target_function,
                        target_symbol: None,
                        registration_span: site.span.clone(),
                        registration_file: site.file,
                        trigger_metadata: TriggerMetadata {
                            method: None,
                            path: None,
                            tool_name: command_name,
                            event_name: None,
                            test_name: None,
                        },
                        trust_boundary_link: None,
                        precision: EntrypointPrecision::Conservative, // Per D-09
                        provenance: EntrypointProvenance::NativeRecognizer,
                        confidence: EntrypointConfidence::Medium,
                        status: EntrypointStatus::Resolved,
                        provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
                        stable_key,
                    });
                }
            }

            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Unresolved framework detection (D-10)
// ---------------------------------------------------------------------------

/// If a framework import was detected in a file but no entrypoints were
/// recognized from that file, emit an UnresolvedFrameworkFact. This catches
/// cases where the framework is used in an unrecognized pattern.
fn emit_unresolved_for_unused_frameworks(
    db: &AnalysisDb,
    file_frameworks: &BTreeMap<FileId, Vec<(&str, TsFramework, Span)>>,
    entrypoints: &[EntrypointFact],
    unresolved: &mut Vec<UnresolvedFrameworkFact>,
) {
    // Collect files that have entrypoints already recognized
    let files_with_entrypoints: BTreeSet<(FileId, &str)> = entrypoints
        .iter()
        .map(|ep| (ep.registration_file, ep.framework_id.as_str()))
        .collect();

    for (file_id, frameworks) in file_frameworks {
        for (framework_id, framework, import_span) in frameworks {
            // Test framework entrypoints are based on call-site matching which we
            // do cover; skip this check for test frameworks since they may have
            // non-test imports in the same file
            if is_test_framework(*framework) {
                continue;
            }

            if !files_with_entrypoints.contains(&(*file_id, framework_id)) {
                let file_key = file_stable_key(db, *file_id);
                let stable_key = semantic_stable_key(
                    FactFamily::UnresolvedFramework,
                    &[
                        ("language", "TypeScript".to_string()),
                        ("file_key", file_key),
                        ("framework_id", framework_id.to_string()),
                        ("reason", "UnrecognizedPattern".to_string()),
                        ("span", span_key(import_span)),
                    ],
                )
                .into_string();

                let framework_label = match framework {
                    TsFramework::Express => "Express",
                    TsFramework::McpSdk => "MCP TypeScript SDK",
                    TsFramework::Commander => "commander",
                    TsFramework::Yargs => "yargs",
                    TsFramework::Jest => "jest",
                    TsFramework::Vitest => "vitest",
                    TsFramework::Mocha => "mocha",
                };

                unresolved.push(UnresolvedFrameworkFact {
                    id: UnresolvedFrameworkId(0),
                    language: Language::TypeScript,
                    file: *file_id,
                    span: import_span.clone(),
                    framework_id: framework_id.to_string(),
                    reason: UnresolvedFrameworkReason::UnrecognizedPattern,
                    evidence: format!(
                        "{} import detected but no matching registration patterns found",
                        framework_label
                    ),
                    scope_description: format!(
                        "{} framework usage without recognized entrypoint patterns",
                        framework_label
                    ),
                    precision: EntrypointPrecision::Conservative,
                    provider_id: ENTRYPOINTS_PROVIDER_ID.to_string(),
                    stable_key,
                });
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Try to resolve the handler function from a call site. Uses the caller
/// function as a fallback target when deeper handler resolution cannot
/// resolve the specific handler function.
fn resolve_handler_function(
    functions_by_id: &BTreeMap<FunctionId, &FunctionFact>,
    site: &CallSiteFact,
) -> Option<FunctionId> {
    if functions_by_id.contains_key(&site.caller) {
        return Some(site.caller);
    }
    None
}

/// Try to extract a string literal from the first argument of a call site.
/// This is used for route paths, tool names, test names, and command names.
fn extract_first_arg_literal(db: &AnalysisDb, site: &CallSiteFact) -> Option<String> {
    if site.arguments.is_empty() {
        return None;
    }
    let first_arg = site.arguments[0];
    // Look up the place fact for the first argument
    let place = db.mir_places().iter().find(|p| p.id == first_arg)?;
    // Check if it has a literal value through the root
    match &place.root {
        crate::analysis::places::PlaceRoot::Unknown { evidence }
            if evidence.starts_with('"') && evidence.ends_with('"') =>
        {
            Some(evidence[1..evidence.len() - 1].to_string())
        }
        _ => None,
    }
}

fn file_stable_key(db: &AnalysisDb, file: FileId) -> String {
    db.metadata_for(FactRef::new(FactFamily::SourceFile, u64::from(file.0)))
        .map(|metadata| metadata.stable_key.clone())
        .or_else(|| {
            db.files()
                .iter()
                .find(|source_file| source_file.id == file)
                .map(|source_file| source_file.relative_path.replace('\\', "/"))
        })
        .unwrap_or_else(|| format!("<missing-file:{}>", file.0))
}

fn span_key(span: &Span) -> String {
    format!(
        "{}:{}..{}:{}@{}..{}",
        span.start_line,
        span.start_col,
        span.end_line,
        span.end_col,
        span.start_byte,
        span.end_byte
    )
}

fn entrypoint_stable_key(
    language: &str,
    file_key: &str,
    framework_id: &str,
    kind: &str,
    target_function_key: &str,
    registration_span: &str,
) -> String {
    semantic_stable_key(
        FactFamily::Entrypoint,
        &[
            ("language", language.to_string()),
            ("file_key", file_key.to_string()),
            ("framework_id", framework_id.to_string()),
            ("kind", kind.to_string()),
            ("target_function_key", target_function_key.to_string()),
            ("registration_span", registration_span.to_string()),
        ],
    )
    .into_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{
        CallCallee, CallPrecision, CallSiteFact, CallSyntaxKind, CallTargetStatus,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{CallSiteId, MirBodyId, MirOpId, PlaceId};
    use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId, ImportFact, ImportId, Language, Span};
    use std::path::PathBuf;

    fn span(file: FileId, line: u32, start_byte: u32) -> Span {
        Span {
            file,
            start_byte,
            end_byte: start_byte + 4,
            start_line: line,
            start_col: 1,
            end_line: line,
            end_col: 5,
        }
    }

    fn add_ts_file(db: &mut AnalysisDb, path: &str) -> FileId {
        db.add_source_file(
            PathBuf::from(path),
            path.to_string(),
            Language::TypeScript,
            "".into(),
            "content".to_string(),
        )
    }

    fn add_function(db: &mut AnalysisDb, file: FileId, name: &str, line: u32) -> FunctionId {
        db.push_function(FunctionFact {
            id: FunctionId(999),
            file,
            name: name.to_string(),
            span: span(file, line, line * 10),
            language: Language::TypeScript,
            is_test: false,
            is_exported: true,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        })
    }

    fn add_import(db: &mut AnalysisDb, file: FileId, path: &str, line: u32) -> ImportId {
        db.push_import(ImportFact {
            id: ImportId(999),
            file,
            package: None,
            path: path.to_string(),
            span: span(file, line, line * 10),
            language: Language::TypeScript,
        })
    }

    fn make_call_site(
        id: u64,
        file: FileId,
        caller: FunctionId,
        callee: CallCallee,
        kind: CallSyntaxKind,
        line: u32,
    ) -> CallSiteFact {
        CallSiteFact {
            id: CallSiteId(id),
            language: Language::TypeScript,
            file,
            caller,
            owner_symbol: None,
            body: MirBodyId(0),
            operation: MirOpId(id),
            span: span(file, line, line * 10),
            kind,
            callee,
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Unresolved,
            precision: CallPrecision::Conservative,
            stable_key: format!("call-site:{id}"),
        }
    }

    #[test]
    fn express_app_get_produces_http_route_entrypoint() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/app.ts");
        let handler = add_function(&mut db, file, "getUsers", 5);
        add_import(&mut db, file, "express", 1);

        let site = make_call_site(
            1,
            file,
            handler,
            CallCallee::Member {
                base: PlaceId(0),
                property: "get".to_string(),
            },
            CallSyntaxKind::Member,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert!(!output.entrypoints.is_empty(), "should produce entrypoints");
        let ep = &output.entrypoints[0];
        assert_eq!(ep.kind, EntrypointKind::HttpRoute);
        assert_eq!(ep.framework_id, "ts.express");
        assert_eq!(ep.language, Language::TypeScript);
        assert_eq!(ep.precision, EntrypointPrecision::Heuristic);
        assert_eq!(ep.provenance, EntrypointProvenance::NativeRecognizer);
        assert_eq!(ep.status, EntrypointStatus::Resolved);
        assert_eq!(
            ep.trigger_metadata.method,
            Some("GET".to_string())
        );
    }

    #[test]
    fn express_app_post_produces_http_route_with_post_method() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/app.ts");
        let handler = add_function(&mut db, file, "createUser", 5);
        add_import(&mut db, file, "express", 1);

        let site = make_call_site(
            1,
            file,
            handler,
            CallCallee::Member {
                base: PlaceId(0),
                property: "post".to_string(),
            },
            CallSyntaxKind::Member,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert_eq!(output.entrypoints.len(), 1);
        assert_eq!(output.entrypoints[0].trigger_metadata.method, Some("POST".to_string()));
    }

    #[test]
    fn express_app_use_produces_http_middleware_entrypoint() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/app.ts");
        let handler = add_function(&mut db, file, "authMiddleware", 5);
        add_import(&mut db, file, "express", 1);

        let site = make_call_site(
            1,
            file,
            handler,
            CallCallee::Member {
                base: PlaceId(0),
                property: "use".to_string(),
            },
            CallSyntaxKind::Member,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert_eq!(output.entrypoints.len(), 1);
        assert_eq!(output.entrypoints[0].kind, EntrypointKind::HttpMiddleware);
        assert_eq!(output.entrypoints[0].framework_id, "ts.express");
    }

    #[test]
    fn mcp_server_tool_produces_mcp_tool_entrypoint() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/server.ts");
        let handler = add_function(&mut db, file, "handleTool", 5);
        add_import(&mut db, file, "@modelcontextprotocol/sdk/server/mcp.js", 1);

        let site = make_call_site(
            1,
            file,
            handler,
            CallCallee::Member {
                base: PlaceId(0),
                property: "tool".to_string(),
            },
            CallSyntaxKind::Member,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert!(!output.entrypoints.is_empty(), "should produce MCP tool entrypoint");
        let ep = &output.entrypoints[0];
        assert_eq!(ep.kind, EntrypointKind::McpTool);
        assert_eq!(ep.framework_id, "ts.mcp_sdk");
        assert_eq!(ep.precision, EntrypointPrecision::Heuristic);
    }

    #[test]
    fn mcp_server_resource_produces_mcp_resource_entrypoint() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/server.ts");
        let handler = add_function(&mut db, file, "handleResource", 5);
        add_import(&mut db, file, "@modelcontextprotocol/sdk", 1);

        let site = make_call_site(
            1,
            file,
            handler,
            CallCallee::Member {
                base: PlaceId(0),
                property: "resource".to_string(),
            },
            CallSyntaxKind::Member,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert_eq!(output.entrypoints.len(), 1);
        assert_eq!(output.entrypoints[0].kind, EntrypointKind::McpResource);
    }

    #[test]
    fn mcp_server_prompt_produces_mcp_prompt_entrypoint() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/server.ts");
        let handler = add_function(&mut db, file, "handlePrompt", 5);
        add_import(&mut db, file, "@modelcontextprotocol/sdk", 1);

        let site = make_call_site(
            1,
            file,
            handler,
            CallCallee::Member {
                base: PlaceId(0),
                property: "prompt".to_string(),
            },
            CallSyntaxKind::Member,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert_eq!(output.entrypoints.len(), 1);
        assert_eq!(output.entrypoints[0].kind, EntrypointKind::McpPrompt);
    }

    #[test]
    fn jest_test_call_produces_test_entrypoint() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/app.test.ts");
        let test_fn = add_function(&mut db, file, "testSuite", 5);
        add_import(&mut db, file, "jest", 1);

        let site = make_call_site(
            1,
            file,
            test_fn,
            CallCallee::Identifier {
                reference: None,
                name: "test".to_string(),
            },
            CallSyntaxKind::Function,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert!(!output.entrypoints.is_empty(), "should produce test entrypoint");
        let ep = &output.entrypoints[0];
        assert_eq!(ep.kind, EntrypointKind::Test);
        assert_eq!(ep.framework_id, "ts.jest");
        assert_eq!(ep.precision, EntrypointPrecision::SetupAware);
    }

    #[test]
    fn vitest_describe_call_produces_test_entrypoint() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/app.test.ts");
        let test_fn = add_function(&mut db, file, "testSuite", 5);
        add_import(&mut db, file, "vitest", 1);

        let site = make_call_site(
            1,
            file,
            test_fn,
            CallCallee::Identifier {
                reference: None,
                name: "describe".to_string(),
            },
            CallSyntaxKind::Function,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert_eq!(output.entrypoints.len(), 1);
        assert_eq!(output.entrypoints[0].kind, EntrypointKind::Test);
        assert_eq!(output.entrypoints[0].framework_id, "ts.vitest");
    }

    #[test]
    fn jest_globals_import_produces_test_entrypoint() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/app.test.ts");
        let test_fn = add_function(&mut db, file, "testSuite", 5);
        add_import(&mut db, file, "@jest/globals", 1);

        let site = make_call_site(
            1,
            file,
            test_fn,
            CallCallee::Identifier {
                reference: None,
                name: "it".to_string(),
            },
            CallSyntaxKind::Function,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert_eq!(output.entrypoints.len(), 1);
        assert_eq!(output.entrypoints[0].kind, EntrypointKind::Test);
        assert_eq!(output.entrypoints[0].framework_id, "ts.jest");
    }

    #[test]
    fn commander_command_produces_cli_entrypoint() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/cli.ts");
        let handler = add_function(&mut db, file, "main", 5);
        add_import(&mut db, file, "commander", 1);

        let site = make_call_site(
            1,
            file,
            handler,
            CallCallee::Member {
                base: PlaceId(0),
                property: "command".to_string(),
            },
            CallSyntaxKind::Member,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert!(!output.entrypoints.is_empty(), "should produce CLI entrypoint");
        let ep = &output.entrypoints[0];
        assert_eq!(ep.kind, EntrypointKind::CliCommand);
        assert_eq!(ep.framework_id, "ts.commander");
        assert_eq!(ep.precision, EntrypointPrecision::Conservative);
    }

    #[test]
    fn yargs_command_produces_cli_entrypoint() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/cli.ts");
        let handler = add_function(&mut db, file, "main", 5);
        add_import(&mut db, file, "yargs", 1);

        let site = make_call_site(
            1,
            file,
            handler,
            CallCallee::Member {
                base: PlaceId(0),
                property: "command".to_string(),
            },
            CallSyntaxKind::Member,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert_eq!(output.entrypoints.len(), 1);
        assert_eq!(output.entrypoints[0].kind, EntrypointKind::CliCommand);
        assert_eq!(output.entrypoints[0].framework_id, "ts.yargs");
    }

    #[test]
    fn unrecognized_framework_import_produces_unresolved_fact() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/app.ts");
        add_import(&mut db, file, "fastify", 1);

        let output = recognize_ts_entrypoints(&db);

        assert!(!output.unresolved.is_empty(), "should produce unresolved fact");
        let ur = &output.unresolved[0];
        assert_eq!(ur.language, Language::TypeScript);
        assert_eq!(ur.reason, UnresolvedFrameworkReason::UnsupportedFrameworkVersion);
        assert!(ur.evidence.contains("fastify"));
        assert!(ur.framework_id.contains("fastify"));
    }

    #[test]
    fn nestjs_import_produces_unresolved_fact() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/app.ts");
        add_import(&mut db, file, "@nestjs/core", 1);

        let output = recognize_ts_entrypoints(&db);

        assert!(!output.unresolved.is_empty(), "should produce unresolved fact for @nestjs");
        let ur = &output.unresolved[0];
        assert!(ur.framework_id.contains("@nestjs/core"));
    }

    #[test]
    fn express_import_without_matching_calls_produces_unrecognized_pattern() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/app.ts");
        add_function(&mut db, file, "setup", 5);
        add_import(&mut db, file, "express", 1);
        // No call sites matching express patterns

        let output = recognize_ts_entrypoints(&db);

        assert!(output.entrypoints.is_empty());
        let express_unresolved: Vec<_> = output
            .unresolved
            .iter()
            .filter(|u| u.framework_id == "ts.express")
            .collect();
        assert!(
            !express_unresolved.is_empty(),
            "express import without matching calls should produce unresolved"
        );
        assert_eq!(
            express_unresolved[0].reason,
            UnresolvedFrameworkReason::UnrecognizedPattern
        );
    }

    #[test]
    fn stable_keys_use_semantic_stable_key_with_fact_family_entrypoint() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/app.ts");
        let handler = add_function(&mut db, file, "getUsers", 5);
        add_import(&mut db, file, "express", 1);

        let site = make_call_site(
            1,
            file,
            handler,
            CallCallee::Member {
                base: PlaceId(0),
                property: "get".to_string(),
            },
            CallSyntaxKind::Member,
            10,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert_eq!(output.entrypoints.len(), 1);
        let key = &output.entrypoints[0].stable_key;
        assert!(key.contains("10:Entrypoint"), "key should contain FactFamily::Entrypoint");
        assert!(key.contains("8:language=10:TypeScript"), "key should contain language=TypeScript");
        assert!(key.contains("12:framework_id=10:ts.express"), "key should contain framework_id");
    }

    #[test]
    fn output_is_sorted_by_stable_key() {
        let mut db = AnalysisDb::new();
        let file = add_ts_file(&mut db, "src/app.ts");
        let handler1 = add_function(&mut db, file, "getUsers", 5);
        let handler2 = add_function(&mut db, file, "createUser", 10);
        let handler3 = add_function(&mut db, file, "deleteUser", 15);
        add_import(&mut db, file, "express", 1);

        let site1 = make_call_site(
            1,
            file,
            handler1,
            CallCallee::Member {
                base: PlaceId(0),
                property: "get".to_string(),
            },
            CallSyntaxKind::Member,
            20,
        );
        let site2 = make_call_site(
            2,
            file,
            handler2,
            CallCallee::Member {
                base: PlaceId(0),
                property: "post".to_string(),
            },
            CallSyntaxKind::Member,
            21,
        );
        let site3 = make_call_site(
            3,
            file,
            handler3,
            CallCallee::Member {
                base: PlaceId(0),
                property: "delete".to_string(),
            },
            CallSyntaxKind::Member,
            22,
        );
        db.replace_call_facts(CallOutput {
            sites: vec![site1, site2, site3],
            targets: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("call facts should store");

        let output = recognize_ts_entrypoints(&db);

        assert_eq!(output.entrypoints.len(), 3);
        let keys: Vec<&str> = output
            .entrypoints
            .iter()
            .map(|ep| ep.stable_key.as_str())
            .collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(keys, sorted, "entrypoints should be sorted by stable key");
    }
}
