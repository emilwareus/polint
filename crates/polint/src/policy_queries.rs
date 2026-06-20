use crate::analysis::calls::facts::{CallCallee, CallPrecision, CallSiteFact, CallTargetStatus};
use crate::analysis::ids::CallSiteId;
use crate::analysis::reachability::facts::{ReachabilityRootFact, RootKind};
use crate::analysis::refined_calls::facts::{RefinedCallConfidence, RefinedCallEdgeFact};
use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId};
use crate::sdk::policy::{
    EventPattern, EventPatternKind, PolicyConfidence, PolicyPrecision, PolicyStatus,
    PolicyViolation, ReachQuery,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(crate) fn matching_events(db: &AnalysisDb, query: EventPattern) -> Vec<PolicyViolation> {
    match query.kind() {
        EventPatternKind::Call => matching_call_events(db, &query),
        EventPatternKind::WriteField => Vec::new(),
    }
}

pub(crate) fn forbidden_reachable(db: &AnalysisDb, query: ReachQuery) -> Vec<PolicyViolation> {
    forbidden_reachable_calls(db, &query)
}

fn matching_call_events(db: &AnalysisDb, pattern: &EventPattern) -> Vec<PolicyViolation> {
    let site_by_id = call_sites_by_id(db);
    if !db.refined_call_edges().is_empty() {
        return db
            .refined_call_edges()
            .iter()
            .filter_map(|edge| {
                if call_pattern_matches_edge(db, pattern, edge, &site_by_id) {
                    Some(violation_for_edge(
                        db,
                        edge,
                        &site_by_id,
                        vec![
                            ("event", "call".to_string()),
                            ("target", best_call_target_label(db, edge, &site_by_id)),
                        ],
                    ))
                } else {
                    None
                }
            })
            .collect();
    }

    db.call_sites()
        .iter()
        .filter_map(|site| {
            if call_pattern_matches_site(pattern, site) {
                Some(PolicyViolation::new(
                    db.path_for(site.file),
                    site.span.diagnostic_range(),
                    policy_status_from_call_status(site.status),
                    policy_precision_from_call_precision(site.precision),
                    vec![
                        ("event".to_string(), "call".to_string()),
                        ("target".to_string(), call_site_label(site)),
                    ],
                ))
            } else {
                None
            }
        })
        .collect()
}

fn forbidden_reachable_calls(db: &AnalysisDb, query: &ReachQuery) -> Vec<PolicyViolation> {
    if query.max_depth == 0 || query.max_paths == 0 {
        return Vec::new();
    }

    let site_by_id = call_sites_by_id(db);
    let roots = selected_reachability_roots(db, query);
    let adjacency = refined_adjacency(db, query, &site_by_id);
    let mut results = Vec::new();
    let mut truncated = false;

    'roots: for root in roots {
        let root_label = root_label(db, root);
        let mut queue = VecDeque::new();
        let mut visited = BTreeSet::new();
        queue.push_back(SearchState {
            function: root.target_function,
            depth: 0,
            path: vec![root_label.clone()],
        });
        visited.insert(root.target_function);

        while let Some(state) = queue.pop_front() {
            let Some(edges) = adjacency.get(&state.function) else {
                continue;
            };

            for edge in edges {
                let edge_depth = state.depth + 1;
                if edge_depth > query.max_depth {
                    truncated = true;
                    continue;
                }

                let target_label = best_call_target_label(db, edge, &site_by_id);
                let mut path = state.path.clone();
                path.push(target_label.clone());

                if call_pattern_matches_edge(db, &query.target, edge, &site_by_id) {
                    if results.len() >= query.max_paths {
                        truncated = true;
                        break 'roots;
                    }
                    results.push(violation_for_edge(
                        db,
                        edge,
                        &site_by_id,
                        vec![
                            ("root", root_label.clone()),
                            ("path", path.join(" -> ")),
                            ("target", target_label),
                            ("depth", edge_depth.to_string()),
                        ],
                    ));
                }

                if traversable_status(edge.status)
                    && let Some(target_function) = edge.target_function
                    && visited.insert(target_function)
                {
                    queue.push_back(SearchState {
                        function: target_function,
                        depth: edge_depth,
                        path,
                    });
                }
            }
        }
    }

    if truncated && let Some(last) = results.pop() {
        results.push(last.with_budget_exceeded("reach_query_budget"));
    }

    results
}

#[derive(Debug, Clone)]
struct SearchState {
    function: FunctionId,
    depth: usize,
    path: Vec<String>,
}

fn call_sites_by_id(db: &AnalysisDb) -> BTreeMap<CallSiteId, &CallSiteFact> {
    db.call_sites().iter().map(|site| (site.id, site)).collect()
}

fn refined_adjacency<'a>(
    db: &'a AnalysisDb,
    query: &ReachQuery,
    site_by_id: &BTreeMap<CallSiteId, &CallSiteFact>,
) -> BTreeMap<FunctionId, Vec<&'a RefinedCallEdgeFact>> {
    let mut adjacency: BTreeMap<FunctionId, Vec<&RefinedCallEdgeFact>> = BTreeMap::new();
    for edge in db.refined_call_edges() {
        if !query.include_tests
            && site_by_id
                .get(&edge.site)
                .is_some_and(|site| file_or_function_is_test(db, site.file, site.caller))
        {
            continue;
        }
        if edge_is_query_candidate(edge, query) {
            adjacency.entry(edge.caller).or_default().push(edge);
        }
    }
    for edges in adjacency.values_mut() {
        edges.sort_by(|left, right| {
            (
                best_call_target_label(db, left, site_by_id),
                left.stable_key.as_str(),
            )
                .cmp(&(
                    best_call_target_label(db, right, site_by_id),
                    right.stable_key.as_str(),
                ))
        });
    }
    adjacency
}

fn selected_reachability_roots<'a>(
    db: &'a AnalysisDb,
    query: &ReachQuery,
) -> Vec<&'a ReachabilityRootFact> {
    let mut roots = db
        .reachability_roots()
        .iter()
        .filter(|root| {
            query.include_tests
                || (root.kind != RootKind::Test
                    && !function_by_id(db, root.target_function).is_some_and(|f| f.is_test))
        })
        .filter(|root| {
            query.roots.is_empty()
                || query
                    .roots
                    .iter()
                    .any(|pattern| call_pattern_matches_root(db, pattern, root))
        })
        .collect::<Vec<_>>();
    roots.sort_by(|left, right| {
        (root_label(db, left), left.stable_key.as_str())
            .cmp(&(root_label(db, right), right.stable_key.as_str()))
    });
    roots
}

fn call_pattern_matches_edge(
    db: &AnalysisDb,
    pattern: &EventPattern,
    edge: &RefinedCallEdgeFact,
    site_by_id: &BTreeMap<CallSiteId, &CallSiteFact>,
) -> bool {
    if pattern.kind() != EventPatternKind::Call {
        return false;
    }
    let candidates = call_edge_candidates(db, edge, site_by_id);
    pattern
        .values()
        .iter()
        .any(|value| candidates.iter().any(|candidate| candidate == value))
}

fn call_pattern_matches_site(pattern: &EventPattern, site: &CallSiteFact) -> bool {
    if pattern.kind() != EventPatternKind::Call {
        return false;
    }
    let label = call_site_label(site);
    pattern.values().iter().any(|value| value == &label)
}

fn call_pattern_matches_root(
    db: &AnalysisDb,
    pattern: &EventPattern,
    root: &ReachabilityRootFact,
) -> bool {
    if pattern.kind() != EventPatternKind::Call {
        return false;
    }
    let mut candidates = vec![
        root.kind.as_str().to_string(),
        format!("root.{}", root.kind.as_str()),
        root_label(db, root),
    ];
    if let Some(function) = function_by_id(db, root.target_function) {
        candidates.push(function.name.clone());
    }
    if let Some(symbol) = root
        .target_symbol
        .and_then(|symbol| db.symbol_by_id(symbol))
    {
        candidates.push(symbol.name.clone());
        candidates.push(symbol.qualified_name.clone());
    }
    pattern
        .values()
        .iter()
        .any(|value| candidates.iter().any(|candidate| candidate == value))
}

fn call_edge_candidates(
    db: &AnalysisDb,
    edge: &RefinedCallEdgeFact,
    site_by_id: &BTreeMap<CallSiteId, &CallSiteFact>,
) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(synthetic_target) = &edge.synthetic_target {
        candidates.push(synthetic_target.clone());
    }
    if let Some(symbol) = edge
        .target_symbol
        .and_then(|symbol| db.symbol_by_id(symbol))
    {
        candidates.push(symbol.name.clone());
        candidates.push(symbol.qualified_name.clone());
    }
    if let Some(function) = edge
        .target_function
        .and_then(|function| function_by_id(db, function))
    {
        candidates.push(function.name.clone());
    }
    if let Some(site) = site_by_id.get(&edge.site) {
        candidates.push(call_site_label(site));
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn best_call_target_label(
    db: &AnalysisDb,
    edge: &RefinedCallEdgeFact,
    site_by_id: &BTreeMap<CallSiteId, &CallSiteFact>,
) -> String {
    if let Some(symbol) = edge
        .target_symbol
        .and_then(|symbol| db.symbol_by_id(symbol))
    {
        if !symbol.qualified_name.is_empty() {
            return symbol.qualified_name.clone();
        }
        return symbol.name.clone();
    }
    if let Some(function) = edge
        .target_function
        .and_then(|function| function_by_id(db, function))
    {
        return function.name.clone();
    }
    if let Some(synthetic_target) = &edge.synthetic_target {
        return synthetic_target.clone();
    }
    site_by_id
        .get(&edge.site)
        .map(|site| call_site_label(site))
        .unwrap_or_else(|| "<unknown-call>".to_string())
}

fn call_site_label(site: &CallSiteFact) -> String {
    match &site.callee {
        CallCallee::Identifier { name, .. } => name.clone(),
        CallCallee::Member { property, .. } => property.clone(),
        CallCallee::Constructor { name, .. } => name
            .clone()
            .unwrap_or_else(|| "<anonymous-constructor>".to_string()),
        CallCallee::Index { .. } => "<index-call>".to_string(),
        CallCallee::Super => "super".to_string(),
        CallCallee::Import => "import".to_string(),
        CallCallee::FunctionValue { .. } => "<function-value>".to_string(),
        CallCallee::Unknown { reason } => format!("unknown:{reason:?}"),
    }
}

fn violation_for_edge(
    db: &AnalysisDb,
    edge: &RefinedCallEdgeFact,
    site_by_id: &BTreeMap<CallSiteId, &CallSiteFact>,
    mut evidence: Vec<(&'static str, String)>,
) -> PolicyViolation {
    let (file, range) = site_by_id.get(&edge.site).map_or_else(
        || {
            (
                "<unknown>".to_string(),
                crate::diagnostics::TextRange::point(1, 1),
            )
        },
        |site| (db.path_for(site.file), site.span.diagnostic_range()),
    );
    evidence.extend([
        ("call_status", call_status_label(edge.status).to_string()),
        (
            "call_precision",
            call_precision_label(edge.precision).to_string(),
        ),
        ("confidence", confidence_label(edge.confidence).to_string()),
    ]);
    if let Some(reason) = edge.reason {
        evidence.push(("unknown_reason", format!("{reason:?}")));
    }
    PolicyViolation::new(
        file,
        range,
        policy_status_from_call_status(edge.status),
        policy_precision_from_call_precision(edge.precision),
        evidence
            .into_iter()
            .map(|(label, value)| (label.to_string(), value))
            .collect(),
    )
}

trait PolicyViolationExt {
    fn with_budget_exceeded(self, budget: &'static str) -> Self;
}

impl PolicyViolationExt for PolicyViolation {
    fn with_budget_exceeded(self, budget: &'static str) -> Self {
        let mut diagnostic = self;
        diagnostic.push_evidence("budget", budget);
        diagnostic.set_status(PolicyStatus::BudgetExceeded);
        diagnostic
    }
}

fn traversable_status(status: CallTargetStatus) -> bool {
    matches!(status, CallTargetStatus::Resolved)
}

fn edge_is_query_candidate(edge: &RefinedCallEdgeFact, query: &ReachQuery) -> bool {
    confidence_meets(edge.confidence, query.minimum_confidence)
        && (precision_meets(edge.precision, query.minimum_precision)
            || !traversable_status(edge.status))
}

fn policy_status_from_call_status(status: CallTargetStatus) -> PolicyStatus {
    match status {
        CallTargetStatus::Resolved => PolicyStatus::Exact,
        CallTargetStatus::Ambiguous | CallTargetStatus::Rejected => PolicyStatus::Heuristic,
        CallTargetStatus::BudgetExceeded => PolicyStatus::BudgetExceeded,
        CallTargetStatus::Unsupported => PolicyStatus::Unsupported,
        CallTargetStatus::Unresolved | CallTargetStatus::SetupMissing => PolicyStatus::Unknown,
    }
}

fn policy_precision_from_call_precision(precision: CallPrecision) -> PolicyPrecision {
    match precision {
        CallPrecision::Exact => PolicyPrecision::Exact,
        CallPrecision::SetupAware => PolicyPrecision::SetupAware,
        CallPrecision::Conservative | CallPrecision::Ambiguous => PolicyPrecision::Conservative,
        CallPrecision::Heuristic => PolicyPrecision::Heuristic,
        CallPrecision::Unknown | CallPrecision::Unsupported => PolicyPrecision::Unknown,
    }
}

fn precision_meets(actual: CallPrecision, minimum: PolicyPrecision) -> bool {
    precision_rank(policy_precision_from_call_precision(actual)) >= precision_rank(minimum)
}

fn precision_rank(precision: PolicyPrecision) -> u8 {
    match precision {
        PolicyPrecision::Exact => 5,
        PolicyPrecision::SetupAware => 4,
        PolicyPrecision::Syntax => 3,
        PolicyPrecision::Conservative => 2,
        PolicyPrecision::Heuristic => 1,
        PolicyPrecision::Unknown => 0,
    }
}

fn confidence_meets(actual: RefinedCallConfidence, minimum: PolicyConfidence) -> bool {
    confidence_rank(policy_confidence_from_refined(actual)) >= confidence_rank(minimum)
}

fn policy_confidence_from_refined(confidence: RefinedCallConfidence) -> PolicyConfidence {
    match confidence {
        RefinedCallConfidence::High => PolicyConfidence::High,
        RefinedCallConfidence::Medium => PolicyConfidence::Medium,
        RefinedCallConfidence::Low => PolicyConfidence::Low,
    }
}

fn confidence_rank(confidence: PolicyConfidence) -> u8 {
    match confidence {
        PolicyConfidence::High => 3,
        PolicyConfidence::Medium => 2,
        PolicyConfidence::Low => 1,
    }
}

fn function_by_id(db: &AnalysisDb, id: FunctionId) -> Option<&FunctionFact> {
    db.functions().iter().find(|function| function.id == id)
}

fn file_or_function_is_test(db: &AnalysisDb, file: FileId, function: FunctionId) -> bool {
    function_by_id(db, function).is_some_and(|function| function.is_test)
        || db.tests().iter().any(|test| test.file == file)
}

fn root_label(db: &AnalysisDb, root: &ReachabilityRootFact) -> String {
    if let Some(symbol) = root
        .target_symbol
        .and_then(|symbol| db.symbol_by_id(symbol))
    {
        if !symbol.qualified_name.is_empty() {
            return symbol.qualified_name.clone();
        }
        return symbol.name.clone();
    }
    function_by_id(db, root.target_function)
        .map(|function| function.name.clone())
        .unwrap_or_else(|| root.kind.as_str().to_string())
}

fn call_status_label(status: CallTargetStatus) -> &'static str {
    match status {
        CallTargetStatus::Resolved => "resolved",
        CallTargetStatus::Ambiguous => "ambiguous",
        CallTargetStatus::Unresolved => "unresolved",
        CallTargetStatus::Unsupported => "unsupported",
        CallTargetStatus::SetupMissing => "setup_missing",
        CallTargetStatus::BudgetExceeded => "budget_exceeded",
        CallTargetStatus::Rejected => "rejected",
    }
}

fn call_precision_label(precision: CallPrecision) -> &'static str {
    match precision {
        CallPrecision::Exact => "exact",
        CallPrecision::SetupAware => "setup_aware",
        CallPrecision::Conservative => "conservative",
        CallPrecision::Heuristic => "heuristic",
        CallPrecision::Ambiguous => "ambiguous",
        CallPrecision::Unknown => "unknown",
        CallPrecision::Unsupported => "unsupported",
    }
}

fn confidence_label(confidence: RefinedCallConfidence) -> &'static str {
    match confidence {
        RefinedCallConfidence::High => "high",
        RefinedCallConfidence::Medium => "medium",
        RefinedCallConfidence::Low => "low",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::calls::facts::{
        CallAlgorithm, CallEdgeKind, CallProvenance, CallSyntaxKind, CallTargetFact,
        UnresolvedCallReason,
    };
    use crate::analysis::calls::store::CallOutput;
    use crate::analysis::ids::{
        CallTargetId, MirBodyId, MirOpId, ReachabilityRootId, RefinedCallEdgeId,
    };
    use crate::analysis::reachability::facts::{
        RootPrecision, RootProvenance, RootStatus, compute_reachability_root_stable_key,
    };
    use crate::analysis::reachability::store::{
        REACHABILITY_PROVIDER_ID, ReachabilityProviderOutput,
    };
    use crate::analysis::refined_calls::facts::{RefinedCallTier, RefinedCallValidation};
    use crate::analysis::refined_calls::store::RefinedCallOutput;
    use crate::core::{FileId, FunctionFact, Language, Span};
    use std::path::PathBuf;

    #[test]
    fn events_matching_call_returns_provider_backed_violation() {
        let db = policy_query_db(false);

        let violations = matching_events(&db, EventPattern::call("dangerous"));

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].status(), PolicyStatus::Exact);
        assert_eq!(violations[0].precision(), PolicyPrecision::SetupAware);
    }

    #[test]
    fn calls_forbidden_reachable_finds_target_from_root() {
        let db = policy_query_db(false);
        let mut query = ReachQuery::new(EventPattern::call("dangerous"));
        query.roots = vec![EventPattern::call("main")];

        let violations = forbidden_reachable(&db, query);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].status(), PolicyStatus::Exact);
        assert_eq!(violations[0].precision(), PolicyPrecision::SetupAware);
    }

    #[test]
    fn calls_forbidden_reachable_excludes_test_roots_by_default() {
        let db = policy_query_db(true);
        let query = ReachQuery::new(EventPattern::call("dangerous"));

        assert!(forbidden_reachable(&db, query.clone()).is_empty());

        let mut include_tests = query;
        include_tests.include_tests = true;
        assert_eq!(forbidden_reachable(&db, include_tests).len(), 1);
    }

    #[test]
    fn calls_forbidden_reachable_surfaces_unresolved_matching_target() {
        let db = policy_query_db_with_unresolved_target();
        let violations = forbidden_reachable(&db, ReachQuery::new(EventPattern::call("dangerous")));

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].status(), PolicyStatus::Unknown);
        assert_eq!(violations[0].precision(), PolicyPrecision::Unknown);
    }

    #[test]
    fn calls_forbidden_reachable_marks_budget_when_path_cap_truncates() {
        let db = policy_query_db_with_two_matching_targets();
        let mut query = ReachQuery::new(EventPattern::call("dangerous"));
        query.max_paths = 1;

        let violations = forbidden_reachable(&db, query);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].status(), PolicyStatus::BudgetExceeded);
        let diagnostic = violations[0].diagnostic("local/test", "dangerous call is reachable");
        assert!(
            diagnostic.evidence.iter().any(
                |evidence| evidence.label == "budget" && evidence.value == "reach_query_budget"
            )
        );
    }

    fn policy_query_db(test_root: bool) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\nfunc main() { handler() }\nfunc handler() { dangerous() }\nfunc dangerous() {}\n".to_string(),
        );
        let main = push_test_function(&mut db, file, "main", 1, test_root);
        let handler = push_test_function(&mut db, file, "handler", 2, false);
        let dangerous = push_test_function(&mut db, file, "dangerous", 3, false);

        let main_to_handler = call_site(0, file, main, "handler");
        let handler_to_dangerous = call_site(1, file, handler, "dangerous");
        db.replace_call_facts(CallOutput {
            sites: vec![main_to_handler.clone(), handler_to_dangerous.clone()],
            targets: vec![
                call_target(0, main_to_handler.id, main, handler),
                call_target(1, handler_to_dangerous.id, handler, dangerous),
            ],
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![
                refined_edge(0, main_to_handler.id, main, handler, "handler"),
                refined_edge(1, handler_to_dangerous.id, handler, dangerous, "dangerous"),
            ],
        })
        .expect("valid refined call facts");

        let root_kind = if test_root {
            RootKind::Test
        } else {
            RootKind::Main
        };
        let root_span = Span::point(file, 1, 1);
        db.replace_reachability_facts(ReachabilityProviderOutput {
            roots: vec![ReachabilityRootFact {
                id: ReachabilityRootId(0),
                kind: root_kind,
                language: Language::Go,
                target_function: main,
                target_symbol: None,
                originating_entrypoint: None,
                file,
                span: root_span.clone(),
                precision: RootPrecision::ResolvedStatic,
                provenance: RootProvenance::NativeDiscovery,
                status: RootStatus::Resolved,
                provider_id: REACHABILITY_PROVIDER_ID.to_string(),
                stable_key: compute_reachability_root_stable_key(
                    root_kind,
                    Language::Go,
                    "main.main",
                    file,
                    &root_span,
                ),
            }],
            marks: Vec::new(),
        })
        .expect("valid reachability facts");
        db
    }

    fn policy_query_db_with_unresolved_target() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\nfunc main() { handler() }\nfunc handler() { dangerous() }\n".to_string(),
        );
        let main = push_test_function(&mut db, file, "main", 1, false);
        let handler = push_test_function(&mut db, file, "handler", 2, false);

        let main_to_handler = call_site(0, file, main, "handler");
        let mut unresolved = call_site(1, file, handler, "dangerous");
        unresolved.status = CallTargetStatus::Unresolved;
        unresolved.precision = CallPrecision::Unknown;
        db.replace_call_facts(CallOutput {
            sites: vec![main_to_handler.clone(), unresolved.clone()],
            targets: vec![call_target(0, main_to_handler.id, main, handler)],
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![
                refined_edge(0, main_to_handler.id, main, handler, "handler"),
                unresolved_refined_edge(1, unresolved.id, handler, "dangerous"),
            ],
        })
        .expect("valid refined call facts");

        let root_span = Span::point(file, 1, 1);
        db.replace_reachability_facts(ReachabilityProviderOutput {
            roots: vec![ReachabilityRootFact {
                id: ReachabilityRootId(0),
                kind: RootKind::Main,
                language: Language::Go,
                target_function: main,
                target_symbol: None,
                originating_entrypoint: None,
                file,
                span: root_span.clone(),
                precision: RootPrecision::ResolvedStatic,
                provenance: RootProvenance::NativeDiscovery,
                status: RootStatus::Resolved,
                provider_id: REACHABILITY_PROVIDER_ID.to_string(),
                stable_key: compute_reachability_root_stable_key(
                    RootKind::Main,
                    Language::Go,
                    "main.main",
                    file,
                    &root_span,
                ),
            }],
            marks: Vec::new(),
        })
        .expect("valid reachability facts");
        db
    }

    fn policy_query_db_with_two_matching_targets() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\nfunc main() { dangerous(); dangerous() }\nfunc dangerousA() {}\nfunc dangerousB() {}\n".to_string(),
        );
        let main = push_test_function(&mut db, file, "main", 1, false);
        let dangerous_a = push_test_function(&mut db, file, "dangerousA", 2, false);
        let dangerous_b = push_test_function(&mut db, file, "dangerousB", 3, false);

        let first = call_site(0, file, main, "dangerous");
        let second = call_site(1, file, main, "dangerous");
        db.replace_call_facts(CallOutput {
            sites: vec![first.clone(), second.clone()],
            targets: vec![
                call_target(0, first.id, main, dangerous_a),
                call_target(1, second.id, main, dangerous_b),
            ],
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![
                refined_edge(0, first.id, main, dangerous_a, "dangerousA"),
                refined_edge(1, second.id, main, dangerous_b, "dangerousB"),
            ],
        })
        .expect("valid refined call facts");

        let root_span = Span::point(file, 1, 1);
        db.replace_reachability_facts(ReachabilityProviderOutput {
            roots: vec![ReachabilityRootFact {
                id: ReachabilityRootId(0),
                kind: RootKind::Main,
                language: Language::Go,
                target_function: main,
                target_symbol: None,
                originating_entrypoint: None,
                file,
                span: root_span.clone(),
                precision: RootPrecision::ResolvedStatic,
                provenance: RootProvenance::NativeDiscovery,
                status: RootStatus::Resolved,
                provider_id: REACHABILITY_PROVIDER_ID.to_string(),
                stable_key: compute_reachability_root_stable_key(
                    RootKind::Main,
                    Language::Go,
                    "main.main",
                    file,
                    &root_span,
                ),
            }],
            marks: Vec::new(),
        })
        .expect("valid reachability facts");
        db
    }

    fn push_test_function(
        db: &mut AnalysisDb,
        file: FileId,
        name: &str,
        line: u32,
        is_test: bool,
    ) -> FunctionId {
        db.push_function(FunctionFact {
            id: FunctionId(0),
            file,
            name: name.to_string(),
            span: Span::point(file, line, 1),
            language: Language::Go,
            is_test,
            is_exported: false,
            cyclomatic_complexity: 1,
            calls: Vec::new(),
        })
    }

    fn call_site(id: u64, file: FileId, caller: FunctionId, callee: &str) -> CallSiteFact {
        CallSiteFact {
            id: CallSiteId(id),
            language: Language::Go,
            file,
            caller,
            owner_symbol: None,
            body: MirBodyId(caller.0),
            operation: MirOpId(id),
            span: Span::point(file, id as u32 + 1, 1),
            kind: CallSyntaxKind::Function,
            callee: CallCallee::Identifier {
                reference: None,
                name: callee.to_string(),
            },
            receiver: None,
            arguments: Vec::new(),
            result: None,
            status: CallTargetStatus::Resolved,
            precision: CallPrecision::Exact,
            in_throw: false,
            stable_key: format!("site:{id}:{callee}"),
        }
    }

    fn call_target(
        id: u64,
        site: CallSiteId,
        caller: FunctionId,
        callee: FunctionId,
    ) -> CallTargetFact {
        CallTargetFact {
            id: CallTargetId(id),
            site,
            caller,
            target_function: Some(callee),
            target_symbol: None,
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            status: CallTargetStatus::Resolved,
            reason: None,
            provenance: CallProvenance::NativeDirect,
            precision: CallPrecision::SetupAware,
            stable_key: format!("target:{id}"),
        }
    }

    fn refined_edge(
        id: u64,
        site: CallSiteId,
        caller: FunctionId,
        callee: FunctionId,
        label: &str,
    ) -> RefinedCallEdgeFact {
        RefinedCallEdgeFact {
            id: RefinedCallEdgeId(id),
            site,
            base_target: Some(CallTargetId(id)),
            caller,
            target_function: Some(callee),
            target_symbol: None,
            synthetic_target: None,
            language: Language::Go,
            edge_kind: CallEdgeKind::Direct,
            algorithm: CallAlgorithm::DirectReference,
            tier: RefinedCallTier::DirectOnly,
            status: CallTargetStatus::Resolved,
            reason: None::<UnresolvedCallReason>,
            provenance: CallProvenance::NativeDirect,
            precision: CallPrecision::SetupAware,
            validation: RefinedCallValidation::Native,
            confidence: RefinedCallConfidence::High,
            evidence: Vec::new(),
            input_stable_keys: Vec::new(),
            stable_key: format!("refined:{id}:{label}"),
        }
    }

    fn unresolved_refined_edge(
        id: u64,
        site: CallSiteId,
        caller: FunctionId,
        label: &str,
    ) -> RefinedCallEdgeFact {
        RefinedCallEdgeFact {
            id: RefinedCallEdgeId(id),
            site,
            base_target: None,
            caller,
            target_function: None,
            target_symbol: None,
            synthetic_target: None,
            language: Language::Go,
            edge_kind: CallEdgeKind::Unknown,
            algorithm: CallAlgorithm::Unsupported,
            tier: RefinedCallTier::DirectOnly,
            status: CallTargetStatus::Unresolved,
            reason: Some(UnresolvedCallReason::UnknownCallee),
            provenance: CallProvenance::NativeDirect,
            precision: CallPrecision::Unknown,
            validation: RefinedCallValidation::Native,
            confidence: RefinedCallConfidence::High,
            evidence: Vec::new(),
            input_stable_keys: Vec::new(),
            stable_key: format!("refined:{id}:{label}:unresolved"),
        }
    }
}
