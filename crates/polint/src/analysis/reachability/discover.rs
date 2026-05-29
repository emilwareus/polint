use crate::analysis::entrypoints::facts::{
    EntrypointFact, EntrypointKind, EntrypointPrecision, EntrypointStatus,
};
use crate::analysis::ids::ReachabilityRootId;
use crate::analysis::reachability::facts::{
    ReachabilityRootFact, RootKind, RootPrecision, RootProvenance, RootStatus,
    compute_reachability_root_stable_key,
};
use crate::analysis::reachability::store::REACHABILITY_PROVIDER_ID;
use crate::core::{AnalysisDb, FunctionFact, Language};

/// Sentinel target for a configured root that resolves to no function. The store
/// referential check intentionally treats this as a non-member of the valid
/// function set, so unresolved configured roots are honest `RootStatus::Unresolved`
/// rows the provider keeps out of the referentially-validated graph while still
/// reporting them (never a silent drop, D-13).
const UNRESOLVED_TARGET: crate::core::FunctionId = crate::core::FunctionId(u64::MAX);

/// Discovers every whole-program reachability root by projecting EXISTING facts
/// only — no parsing, no AST, no tree-sitter/Oxc invocation (D-07).
///
/// Sources:
/// - Go `Main`/`Init` from `db.functions()` + `db.packages()` (D-08)
/// - Go/TS/JS `Exported` from `FunctionFact.is_exported` (D-09/D-10)
/// - the `Test`/`FrameworkEntrypoint` bridge over `db.entrypoint_facts()` (D-12)
/// - configured `.polint.toml` `[reachability] roots` (D-13)
///
/// Note (D-11): TS/JS never synthesize `Main`/`Init` roots — JS/TS modules have
/// no language-level `main`/`init` entry the way Go does, so those kinds are Go
/// only. The omission is intentional; see `go_main_init_roots`.
pub(crate) fn discover_reachability_roots(
    db: &AnalysisDb,
    configured_roots: &[String],
) -> Vec<ReachabilityRootFact> {
    let mut roots = Vec::new();

    for function in db.functions() {
        roots.extend(go_main_init_roots(db, function));
        roots.extend(exported_root(db, function));
    }

    for entrypoint in db.entrypoint_facts() {
        roots.push(entrypoint_bridge_root(db, entrypoint));
    }

    roots.extend(configured_roots_for(db, configured_roots));

    // IN-01: discovery does NOT assign dense IDs. Every constructor leaves
    // `id: ReachabilityRootId(0)`; the provider assigns the only dense IDs that
    // matter, AFTER the sort+normalize step and AFTER the output digest (D-06).
    // Assigning placeholder IDs here would falsely signal that IDs are meaningful
    // at discovery time when they are never persisted.
    roots
}

/// Go `Main` (`func main` in `package main`) and `Init` (`func init`, any
/// package) roots. Both are `RootPrecision::ResolvedStatic` when the underlying
/// facts are present (D-08). Returns at most one root per function.
fn go_main_init_roots(db: &AnalysisDb, function: &FunctionFact) -> Vec<ReachabilityRootFact> {
    if function.language != Language::Go {
        // D-11: TS/JS never synthesize Main/Init roots.
        return Vec::new();
    }

    let mut roots = Vec::new();
    match function.name.as_str() {
        "main" if go_package_name(db, function) == Some("main".to_string()) => {
            roots.push(native_root(
                db,
                function,
                RootKind::Main,
                RootPrecision::ResolvedStatic,
                RootStatus::Resolved,
            ));
        }
        "init" => {
            roots.push(native_root(
                db,
                function,
                RootKind::Init,
                RootPrecision::ResolvedStatic,
                RootStatus::Resolved,
            ));
        }
        _ => {}
    }
    roots
}

/// Exported-function roots from `FunctionFact.is_exported` (D-09/D-10).
///
/// Go exported functions are `ResolvedStatic`; TS/JS exported functions are
/// `SetupAware` (their export resolution is module-graph/setup-sensitive). A
/// `main`/`init` Go function is NOT also emitted as `Exported` to avoid two roots
/// for the same function.
fn exported_root(db: &AnalysisDb, function: &FunctionFact) -> Option<ReachabilityRootFact> {
    if !function.is_exported {
        return None;
    }
    if function.language == Language::Go
        && (function.name == "init"
            || (function.name == "main"
                && go_package_name(db, function) == Some("main".to_string())))
    {
        return None;
    }
    let precision = match function.language {
        Language::Go => RootPrecision::ResolvedStatic,
        _ => RootPrecision::SetupAware,
    };
    Some(native_root(
        db,
        function,
        RootKind::Exported,
        precision,
        RootStatus::Resolved,
    ))
}

/// Loss-free bridge from every `db.entrypoint_facts()` row into a root (D-12).
///
/// `EntrypointKind::Test` maps to `RootKind::Test`; every other kind maps to
/// `RootKind::FrameworkEntrypoint`. The root carries `originating_entrypoint =
/// Some(ep.id)` and inherits the entrypoint's precision and status.
fn entrypoint_bridge_root(db: &AnalysisDb, entrypoint: &EntrypointFact) -> ReachabilityRootFact {
    let kind = match entrypoint.kind {
        EntrypointKind::Test => RootKind::Test,
        _ => RootKind::FrameworkEntrypoint,
    };
    let function_identity = function_identity_for_target(db, entrypoint.target_function)
        .unwrap_or_else(|| format!("entrypoint:{}", entrypoint.stable_key));
    let stable_key = compute_reachability_root_stable_key(
        kind,
        entrypoint.language,
        &function_identity,
        entrypoint.registration_file,
        &entrypoint.registration_span,
    );
    ReachabilityRootFact {
        id: ReachabilityRootId(0),
        kind,
        language: entrypoint.language,
        target_function: entrypoint.target_function,
        target_symbol: entrypoint.target_symbol,
        originating_entrypoint: Some(entrypoint.id),
        file: entrypoint.registration_file,
        span: entrypoint.registration_span.clone(),
        precision: bridge_precision(entrypoint.precision),
        provenance: RootProvenance::EntrypointBridge,
        status: bridge_status(entrypoint.status),
        provider_id: REACHABILITY_PROVIDER_ID.to_string(),
        stable_key,
    }
}

/// Configured `.polint.toml` roots (D-13). Each entry resolves against existing
/// function facts by name; unresolvable entries become honest
/// `RootStatus::Unresolved` / `RootProvenance::Configured` roots — never silent
/// drops, never filesystem reads outside the repo (the entries are matched only
/// against in-DB facts).
fn configured_roots_for(db: &AnalysisDb, configured: &[String]) -> Vec<ReachabilityRootFact> {
    configured
        .iter()
        .map(|entry| match resolve_configured_function(db, entry) {
            Some(function) => {
                let function_identity = function_identity_for(db, function);
                let stable_key = compute_reachability_root_stable_key(
                    RootKind::ConfiguredEntrypoint,
                    function.language,
                    &function_identity,
                    function.file,
                    &function.span,
                );
                ReachabilityRootFact {
                    id: ReachabilityRootId(0),
                    kind: RootKind::ConfiguredEntrypoint,
                    language: function.language,
                    target_function: function.id,
                    target_symbol: None,
                    originating_entrypoint: None,
                    file: function.file,
                    span: function.span.clone(),
                    precision: RootPrecision::SetupAware,
                    provenance: RootProvenance::Configured,
                    status: RootStatus::Resolved,
                    provider_id: REACHABILITY_PROVIDER_ID.to_string(),
                    stable_key,
                }
            }
            None => {
                let stable_key = compute_reachability_root_stable_key(
                    RootKind::ConfiguredEntrypoint,
                    Language::Unknown,
                    &format!("configured:{entry}"),
                    crate::core::FileId(0),
                    &unresolved_span(),
                );
                ReachabilityRootFact {
                    id: ReachabilityRootId(0),
                    kind: RootKind::ConfiguredEntrypoint,
                    language: Language::Unknown,
                    target_function: UNRESOLVED_TARGET,
                    target_symbol: None,
                    originating_entrypoint: None,
                    file: crate::core::FileId(0),
                    span: unresolved_span(),
                    precision: RootPrecision::Unknown,
                    provenance: RootProvenance::Configured,
                    status: RootStatus::Unresolved,
                    provider_id: REACHABILITY_PROVIDER_ID.to_string(),
                    stable_key,
                }
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Helpers (existing-facts projection only)
// ---------------------------------------------------------------------------

fn native_root(
    db: &AnalysisDb,
    function: &FunctionFact,
    kind: RootKind,
    precision: RootPrecision,
    status: RootStatus,
) -> ReachabilityRootFact {
    let function_identity = function_identity_for(db, function);
    let stable_key = compute_reachability_root_stable_key(
        kind,
        function.language,
        &function_identity,
        function.file,
        &function.span,
    );
    ReachabilityRootFact {
        id: ReachabilityRootId(0),
        kind,
        language: function.language,
        target_function: function.id,
        target_symbol: None,
        originating_entrypoint: None,
        file: function.file,
        span: function.span.clone(),
        precision,
        provenance: RootProvenance::NativeDiscovery,
        status,
        provider_id: REACHABILITY_PROVIDER_ID.to_string(),
        stable_key,
    }
}

/// Stable function identity string for stable keys — Go uses `package.Name`,
/// other languages use the bare function name. Never a run-local ID (D-06).
fn function_identity_for(db: &AnalysisDb, function: &FunctionFact) -> String {
    match function.language {
        Language::Go => match go_package_name(db, function) {
            Some(package) => format!("{package}.{}", function.name),
            None => function.name.clone(),
        },
        _ => function.name.clone(),
    }
}

fn function_identity_for_target(
    db: &AnalysisDb,
    target: crate::core::FunctionId,
) -> Option<String> {
    db.functions()
        .iter()
        .find(|function| function.id == target)
        .map(|function| function_identity_for(db, function))
}

/// Returns the Go package-clause name for a function's file by scanning
/// `db.packages()` (D-08). Projection over existing `PackageFact` rows only.
fn go_package_name(db: &AnalysisDb, function: &FunctionFact) -> Option<String> {
    db.packages()
        .iter()
        .find(|package| package.file == function.file && package.language == Language::Go)
        .map(|package| package.name.clone())
}

/// Resolves a configured root string against existing function facts only.
/// Accepts `pkg.Name`, `path#name`, or a bare `name`; matches the trailing
/// identifier against `FunctionFact.name`.
fn resolve_configured_function<'a>(db: &'a AnalysisDb, entry: &str) -> Option<&'a FunctionFact> {
    let needle = configured_function_name(entry);
    db.functions()
        .iter()
        .find(|function| function.name == needle)
}

fn configured_function_name(entry: &str) -> &str {
    if let Some((_, name)) = entry.rsplit_once('#') {
        name
    } else if let Some((_, name)) = entry.rsplit_once('.') {
        name
    } else {
        entry
    }
}

fn bridge_precision(precision: EntrypointPrecision) -> RootPrecision {
    match precision {
        EntrypointPrecision::ResolvedStatic => RootPrecision::ResolvedStatic,
        EntrypointPrecision::SetupAware => RootPrecision::SetupAware,
        EntrypointPrecision::Heuristic => RootPrecision::Heuristic,
        EntrypointPrecision::Conservative => RootPrecision::Conservative,
        EntrypointPrecision::Unknown => RootPrecision::Unknown,
    }
}

fn bridge_status(status: EntrypointStatus) -> RootStatus {
    match status {
        EntrypointStatus::Resolved => RootStatus::Resolved,
        EntrypointStatus::Partial => RootStatus::Partial,
        EntrypointStatus::Unresolved => RootStatus::Unresolved,
        EntrypointStatus::SetupMissing => RootStatus::SetupMissing,
        EntrypointStatus::Unsupported => RootStatus::Unsupported,
    }
}

fn unresolved_span() -> crate::core::Span {
    crate::core::Span::point(crate::core::FileId(0), 0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::entrypoints::facts::{
        EntrypointConfidence, EntrypointFact, EntrypointKind, EntrypointPrecision,
        EntrypointProvenance, EntrypointStatus, TriggerMetadata,
    };
    use crate::analysis::entrypoints::store::EntrypointOutput;
    use crate::analysis::ids::EntrypointId;
    use crate::core::{
        AnalysisDb, FileId, FunctionFact, FunctionId, Language, PackageFact, PackageId, Span,
        SymbolId,
    };
    use std::path::PathBuf;

    fn span(file: FileId, start: u32, end: u32) -> Span {
        Span {
            file,
            start_byte: start,
            end_byte: end,
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 1,
        }
    }

    fn add_go_file(db: &mut AnalysisDb, path: &str, package: &str) -> FileId {
        let file = db.add_file(
            PathBuf::from(path),
            path.to_string(),
            format!("package {package}\n"),
        );
        db.push_package(PackageFact {
            id: PackageId(0),
            file,
            name: package.to_string(),
            span: span(file, 0, 1),
            language: Language::Go,
        });
        file
    }

    fn add_function(
        db: &mut AnalysisDb,
        file: FileId,
        id: u64,
        name: &str,
        language: Language,
        is_exported: bool,
    ) -> FunctionId {
        db.push_function(FunctionFact {
            id: FunctionId(id),
            file,
            name: name.to_string(),
            span: span(file, id as u32, id as u32 + 1),
            language,
            is_test: false,
            is_exported,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        })
    }

    #[test]
    fn go_main_in_package_main_yields_one_resolved_static_main_root() {
        let mut db = AnalysisDb::new();
        let file = add_go_file(&mut db, "cmd/app/main.go", "main");
        add_function(&mut db, file, 1, "main", Language::Go, false);

        let roots = discover_reachability_roots(&db, &[]);
        let mains: Vec<_> = roots.iter().filter(|r| r.kind == RootKind::Main).collect();
        assert_eq!(mains.len(), 1);
        assert_eq!(mains[0].precision, RootPrecision::ResolvedStatic);
        assert_eq!(mains[0].provenance, RootProvenance::NativeDiscovery);
        assert_eq!(mains[0].status, RootStatus::Resolved);
    }

    #[test]
    fn go_main_in_non_main_package_is_not_a_main_root() {
        let mut db = AnalysisDb::new();
        let file = add_go_file(&mut db, "pkg/util/main.go", "util");
        add_function(&mut db, file, 1, "main", Language::Go, false);

        let roots = discover_reachability_roots(&db, &[]);
        assert!(roots.iter().all(|r| r.kind != RootKind::Main));
    }

    #[test]
    fn every_go_init_yields_an_init_root() {
        let mut db = AnalysisDb::new();
        let file_a = add_go_file(&mut db, "a/a.go", "a");
        let file_b = add_go_file(&mut db, "b/b.go", "b");
        add_function(&mut db, file_a, 1, "init", Language::Go, false);
        add_function(&mut db, file_b, 2, "init", Language::Go, false);

        let roots = discover_reachability_roots(&db, &[]);
        let inits = roots.iter().filter(|r| r.kind == RootKind::Init).count();
        assert_eq!(inits, 2);
    }

    #[test]
    fn capitalized_function_in_non_main_package_is_exported_root() {
        let mut db = AnalysisDb::new();
        let file = add_go_file(&mut db, "pkg/svc/svc.go", "svc");
        add_function(&mut db, file, 1, "Handle", Language::Go, true);

        let roots = discover_reachability_roots(&db, &[]);
        let exported: Vec<_> = roots
            .iter()
            .filter(|r| r.kind == RootKind::Exported)
            .collect();
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].precision, RootPrecision::ResolvedStatic);
        // Stable key uses package.Name, not a dense ID.
        assert!(exported[0].stable_key.contains("svc.Handle"));
    }

    #[test]
    fn entrypoint_test_kind_maps_to_test_root_with_inherited_metadata() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/app.test.ts"),
            "src/app.test.ts".to_string(),
            "test('x', () => {})\n".to_string(),
        );
        let function = add_function(&mut db, file, 5, "x", Language::TypeScript, false);
        let entrypoint = EntrypointFact {
            id: EntrypointId(0),
            language: Language::TypeScript,
            framework_id: "jest".to_string(),
            kind: EntrypointKind::Test,
            target_function: function,
            target_symbol: Some(SymbolId(0)),
            registration_span: span(file, 1, 2),
            registration_file: file,
            trigger_metadata: TriggerMetadata::empty(),
            trust_boundary_link: None,
            precision: EntrypointPrecision::Heuristic,
            provenance: EntrypointProvenance::NativeRecognizer,
            confidence: EntrypointConfidence::Medium,
            status: EntrypointStatus::Partial,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: "entrypoint:jest:x".to_string(),
        };
        db.replace_entrypoint_facts(EntrypointOutput {
            entrypoints: vec![entrypoint],
            trust_boundaries: Vec::new(),
            dispatch_edges: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("store entrypoint");

        let roots = discover_reachability_roots(&db, &[]);
        let test_roots: Vec<_> = roots.iter().filter(|r| r.kind == RootKind::Test).collect();
        assert_eq!(test_roots.len(), 1);
        assert_eq!(test_roots[0].originating_entrypoint, Some(EntrypointId(0)));
        // Precision and status are inherited loss-lessly from the entrypoint.
        assert_eq!(test_roots[0].precision, RootPrecision::Heuristic);
        assert_eq!(test_roots[0].status, RootStatus::Partial);
        assert_eq!(test_roots[0].provenance, RootProvenance::EntrypointBridge);
    }

    #[test]
    fn non_test_entrypoint_kind_maps_to_framework_entrypoint_root() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/server.ts"),
            "src/server.ts".to_string(),
            "app.get('/', handler)\n".to_string(),
        );
        let function = add_function(&mut db, file, 5, "handler", Language::TypeScript, true);
        let entrypoint = EntrypointFact {
            id: EntrypointId(3),
            language: Language::TypeScript,
            framework_id: "express".to_string(),
            kind: EntrypointKind::HttpRoute,
            target_function: function,
            target_symbol: None,
            registration_span: span(file, 1, 2),
            registration_file: file,
            trigger_metadata: TriggerMetadata::empty(),
            trust_boundary_link: None,
            precision: EntrypointPrecision::ResolvedStatic,
            provenance: EntrypointProvenance::NativeRecognizer,
            confidence: EntrypointConfidence::High,
            status: EntrypointStatus::Resolved,
            provider_id: "polint.entrypoints".to_string(),
            stable_key: "entrypoint:express:get:/".to_string(),
        };
        db.replace_entrypoint_facts(EntrypointOutput {
            entrypoints: vec![entrypoint],
            trust_boundaries: Vec::new(),
            dispatch_edges: Vec::new(),
            unresolved: Vec::new(),
        })
        .expect("store entrypoint");

        // `replace_entrypoint_facts` normalizes entrypoint IDs, so read the
        // stored entrypoint's actual id rather than the pre-store value.
        let stored_entrypoint_id = db.entrypoint_facts()[0].id;
        let roots = discover_reachability_roots(&db, &[]);
        let framework: Vec<_> = roots
            .iter()
            .filter(|r| r.kind == RootKind::FrameworkEntrypoint)
            .collect();
        assert_eq!(framework.len(), 1);
        assert_eq!(
            framework[0].originating_entrypoint,
            Some(stored_entrypoint_id)
        );
    }

    #[test]
    fn configured_root_resolving_to_no_function_is_unresolved_not_dropped() {
        let db = AnalysisDb::new();
        let roots = discover_reachability_roots(&db, &["does/not.Resolve".to_string()]);
        let configured: Vec<_> = roots
            .iter()
            .filter(|r| r.kind == RootKind::ConfiguredEntrypoint)
            .collect();
        assert_eq!(configured.len(), 1);
        assert_eq!(configured[0].status, RootStatus::Unresolved);
        assert_eq!(configured[0].provenance, RootProvenance::Configured);
    }

    #[test]
    fn configured_root_resolving_to_a_function_is_resolved() {
        let mut db = AnalysisDb::new();
        let file = add_go_file(&mut db, "pkg/svc/svc.go", "svc");
        add_function(&mut db, file, 1, "Handle", Language::Go, true);

        let handle_id = db
            .functions()
            .iter()
            .find(|f| f.name == "Handle")
            .map(|f| f.id)
            .expect("Handle function present");

        let roots = discover_reachability_roots(&db, &["pkg/svc.Handle".to_string()]);
        let configured: Vec<_> = roots
            .iter()
            .filter(|r| {
                r.kind == RootKind::ConfiguredEntrypoint && r.status == RootStatus::Resolved
            })
            .collect();
        assert_eq!(configured.len(), 1);
        assert_eq!(configured[0].provenance, RootProvenance::Configured);
        assert_eq!(configured[0].target_function, handle_id);
    }

    #[test]
    fn ts_js_produce_no_main_or_init_roots() {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/index.ts"),
            "src/index.ts".to_string(),
            "function main() {}\nfunction init() {}\n".to_string(),
        );
        add_function(&mut db, file, 1, "main", Language::TypeScript, false);
        add_function(&mut db, file, 2, "init", Language::JavaScript, false);

        let roots = discover_reachability_roots(&db, &[]);
        assert!(
            roots
                .iter()
                .all(|r| r.kind != RootKind::Main && r.kind != RootKind::Init)
        );
    }
}
