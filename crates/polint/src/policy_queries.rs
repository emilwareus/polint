use crate::analysis::calls::facts::{CallCallee, CallPrecision, CallSiteFact, CallTargetStatus};
use crate::analysis::data_flow::facts::{
    DataFlowConfidence, DataFlowEdgeFact, DataFlowEdgeKind, DataFlowNodeFact, DataFlowNodeKind,
    DataFlowPrecision,
};
use crate::analysis::data_flow::query::{
    DataFlowPath, DataFlowPathStatus, DataFlowSearchBudget, find_paths,
};
use crate::analysis::data_flow::store::DataFlowStore;
use crate::analysis::ids::{CallSiteId, DataFlowNodeId, MirBodyId, MirOpId, PlaceId};
use crate::analysis::places::{PlaceFact, PlaceProjection, PlaceRoot};
use crate::analysis::reachability::facts::{ReachabilityRootFact, RootKind};
use crate::analysis::refined_calls::facts::{RefinedCallConfidence, RefinedCallEdgeFact};
use crate::core::{AnalysisDb, FileId, FunctionFact, FunctionId};
use crate::sdk::policy::{
    BarrierPattern, BarrierPatternKind, EventPattern, EventPatternKind, FlowQuery, GuardPattern,
    GuardPatternKind, GuardQuery, LifecycleQuery, PolicyConfidence, PolicyPrecision, PolicyStatus,
    PolicyViolation, ReachQuery, SinkPattern, SinkPatternKind, SourcePattern, SourcePatternKind,
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

pub(crate) fn missing_guards(db: &AnalysisDb, query: GuardQuery) -> Vec<PolicyViolation> {
    missing_guard_calls(db, &query)
}

pub(crate) fn missing_cleanup(db: &AnalysisDb, query: LifecycleQuery) -> Vec<PolicyViolation> {
    missing_cleanup_calls(db, &query)
}

pub(crate) fn forbidden_flows(db: &AnalysisDb, query: FlowQuery) -> Vec<PolicyViolation> {
    forbidden_data_flows(db, &query)
}

fn forbidden_data_flows(db: &AnalysisDb, query: &FlowQuery) -> Vec<PolicyViolation> {
    let Some(store) = db.data_flow_store() else {
        return Vec::new();
    };

    let site_by_id = call_sites_by_id(db);
    let sources = matching_flow_sources(db, store, &query.source);
    let sinks = matching_flow_sinks(db, store, &query.sink, &site_by_id);
    if sources.is_empty() || sinks.is_empty() {
        return Vec::new();
    }

    let budget = DataFlowSearchBudget {
        max_depth: query.max_depth,
        max_paths: query.max_paths,
    };
    let mut results: Vec<PolicyViolation> = Vec::new();
    let mut emitted = BTreeSet::new();

    'sources: for source in &sources {
        for sink in &sinks {
            let paths = find_paths(store, source.node, sink.node, budget);
            for path in paths {
                match path.status {
                    DataFlowPathStatus::Found => {
                        if barrier_covers_path(db, store, &site_by_id, &path, &query.barriers) {
                            continue;
                        }
                        if query.max_paths > 0 && results.len() >= query.max_paths {
                            if let Some(last) = results.pop() {
                                results.push(last.with_budget_exceeded("data_flow_query_budget"));
                            }
                            break 'sources;
                        }
                        let key = flow_result_key(source, sink, &path);
                        if emitted.insert(key) {
                            results.push(flow_violation(db, store, source, sink, &path, query));
                        }
                    }
                    DataFlowPathStatus::Unknown | DataFlowPathStatus::BudgetExceeded => {
                        let key = flow_result_key(source, sink, &path);
                        if emitted.insert(key) {
                            results.push(flow_violation(db, store, source, sink, &path, query));
                        }
                    }
                    DataFlowPathStatus::NotFound => {}
                }
            }
        }
    }

    results
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

fn missing_guard_calls(db: &AnalysisDb, query: &GuardQuery) -> Vec<PolicyViolation> {
    if query.max_depth == 0
        || query.max_paths == 0
        || query.event.kind() != EventPatternKind::Call
        || query.guard.kind() != GuardPatternKind::CallAny
    {
        return Vec::new();
    }

    let events = ordered_control_call_events(db, query.minimum_precision);
    let by_function = control_events_by_function(&events);
    let mut results = Vec::new();
    let mut truncated = false;

    'functions: for function_events in by_function.values() {
        for (index, event) in function_events.iter().enumerate() {
            if !event_matches_pattern(event, &query.event) {
                continue;
            }
            let has_guard = function_events[..index]
                .iter()
                .any(|candidate| guard_matches_event(candidate, &query.guard));
            if has_guard {
                continue;
            }
            if results.len() >= query.max_paths {
                truncated = true;
                break 'functions;
            }
            results.push(control_violation(
                db,
                event,
                vec![
                    ("policy", "missing_guard".to_string()),
                    ("required_guard", query.guard.values().join(",")),
                    ("uncovered_path", format!("entry -> {}", event.target_label)),
                    ("requested_max_depth", query.max_depth.to_string()),
                ],
            ));
        }
    }

    if truncated && let Some(last) = results.pop() {
        results.push(last.with_budget_exceeded("control_flow_query_budget"));
    }

    results
}

fn missing_cleanup_calls(db: &AnalysisDb, query: &LifecycleQuery) -> Vec<PolicyViolation> {
    if query.max_depth == 0
        || query.max_paths == 0
        || query.start.kind() != EventPatternKind::Call
        || query.cleanup.kind() != EventPatternKind::Call
    {
        return Vec::new();
    }

    let events = ordered_control_call_events(db, query.minimum_precision);
    let by_function = control_events_by_function(&events);
    let mut results = Vec::new();
    let mut truncated = false;

    'functions: for function_events in by_function.values() {
        for (index, event) in function_events.iter().enumerate() {
            if !event_matches_pattern(event, &query.start) {
                continue;
            }
            let has_cleanup = function_events[index + 1..]
                .iter()
                .any(|candidate| event_matches_pattern(candidate, &query.cleanup));
            if has_cleanup {
                continue;
            }
            if results.len() >= query.max_paths {
                truncated = true;
                break 'functions;
            }
            results.push(control_violation(
                db,
                event,
                vec![
                    ("policy", "missing_cleanup".to_string()),
                    ("required_cleanup", query.cleanup.values().join(",")),
                    ("uncovered_path", format!("{} -> exit", event.target_label)),
                    (
                        "require_error_cleanup",
                        query.require_error_cleanup.to_string(),
                    ),
                    ("requested_max_depth", query.max_depth.to_string()),
                ],
            ));
        }
    }

    if truncated && let Some(last) = results.pop() {
        results.push(last.with_budget_exceeded("control_flow_query_budget"));
    }

    results
}

#[derive(Debug, Clone)]
struct FlowSource {
    node: DataFlowNodeId,
    label: String,
    precision: PolicyPrecision,
    heuristic: bool,
}

#[derive(Debug, Clone)]
struct FlowSink {
    node: DataFlowNodeId,
    site: CallSiteId,
    file: String,
    range: crate::diagnostics::TextRange,
    target_label: String,
    precision: PolicyPrecision,
    heuristic: bool,
}

fn matching_flow_sources(
    db: &AnalysisDb,
    store: &DataFlowStore,
    pattern: &SourcePattern,
) -> Vec<FlowSource> {
    let mut sources = BTreeMap::<DataFlowNodeId, FlowSource>::new();
    match pattern.kind() {
        SourcePatternKind::HttpRequest => {
            for node in store
                .nodes()
                .iter()
                .filter(|node| node.kind == DataFlowNodeKind::Source)
            {
                let Some(model) = node
                    .model
                    .and_then(|model| store.models().iter().find(|fact| fact.id == model))
                else {
                    continue;
                };
                if model
                    .payload_labels
                    .iter()
                    .any(|label| http_source_label(label))
                {
                    sources.insert(
                        node.id,
                        FlowSource {
                            node: node.id,
                            label: model
                                .model_id
                                .clone()
                                .unwrap_or_else(|| "http_request".to_string()),
                            precision: policy_precision_from_data_flow(model.precision),
                            heuristic: true,
                        },
                    );
                }
            }
        }
        SourcePatternKind::SecretLike => {
            for node in store.nodes() {
                if node.kind == DataFlowNodeKind::Source
                    && source_model_matches_values(store, node, pattern.values())
                {
                    sources.insert(
                        node.id,
                        FlowSource {
                            node: node.id,
                            label: source_node_label(store, node),
                            precision: PolicyPrecision::Heuristic,
                            heuristic: true,
                        },
                    );
                }

                if node.kind == DataFlowNodeKind::Place
                    && let Some(place) = node.place.and_then(|place| place_by_id(db, place))
                    && place_matches_values(place, pattern.values())
                {
                    sources.insert(
                        node.id,
                        FlowSource {
                            node: node.id,
                            label: place_label(place),
                            precision: PolicyPrecision::Heuristic,
                            heuristic: true,
                        },
                    );
                }
            }
        }
    }
    sources.into_values().collect()
}

fn matching_flow_sinks(
    db: &AnalysisDb,
    store: &DataFlowStore,
    pattern: &SinkPattern,
    site_by_id: &BTreeMap<CallSiteId, &CallSiteFact>,
) -> Vec<FlowSink> {
    let mut sinks = BTreeMap::<(CallSiteId, DataFlowNodeId), FlowSink>::new();

    if !db.refined_call_edges().is_empty() {
        for edge in db.refined_call_edges() {
            let Some(site) = site_by_id.get(&edge.site).copied() else {
                continue;
            };
            let candidates = call_edge_candidates(db, edge, site_by_id);
            if !sink_matches_candidates(pattern, &candidates) {
                continue;
            }
            push_flow_sinks_for_site(
                db,
                store,
                pattern,
                site,
                best_call_target_label(db, edge, site_by_id),
                policy_precision_from_call_precision(edge.precision),
                &mut sinks,
            );
        }
    } else {
        for site in db.call_sites() {
            let candidates = vec![call_site_label(site)];
            if !sink_matches_candidates(pattern, &candidates) {
                continue;
            }
            push_flow_sinks_for_site(
                db,
                store,
                pattern,
                site,
                call_site_label(site),
                policy_precision_from_call_precision(site.precision),
                &mut sinks,
            );
        }
    }

    sinks.into_values().collect()
}

fn push_flow_sinks_for_site(
    db: &AnalysisDb,
    store: &DataFlowStore,
    pattern: &SinkPattern,
    site: &CallSiteFact,
    target_label: String,
    precision: PolicyPrecision,
    sinks: &mut BTreeMap<(CallSiteId, DataFlowNodeId), FlowSink>,
) {
    for place in site.arguments.iter().copied().chain(site.receiver) {
        for node in nodes_for_place(store, place) {
            sinks.insert(
                (site.id, node.id),
                FlowSink {
                    node: node.id,
                    site: site.id,
                    file: db.path_for(site.file),
                    range: site.span.diagnostic_range(),
                    target_label: target_label.clone(),
                    precision,
                    heuristic: pattern.kind() == SinkPatternKind::Logger,
                },
            );
        }
    }
}

fn nodes_for_place(store: &DataFlowStore, place: PlaceId) -> Vec<&DataFlowNodeFact> {
    store
        .nodes()
        .iter()
        .filter(|node| node.place == Some(place))
        .collect()
}

fn sink_matches_candidates(pattern: &SinkPattern, candidates: &[String]) -> bool {
    match pattern.kind() {
        SinkPatternKind::Call => pattern
            .values()
            .iter()
            .any(|value| candidates.iter().any(|candidate| candidate == value)),
        SinkPatternKind::Logger => candidates
            .iter()
            .any(|candidate| logger_candidate(candidate)),
    }
}

fn barrier_covers_path(
    db: &AnalysisDb,
    store: &DataFlowStore,
    site_by_id: &BTreeMap<CallSiteId, &CallSiteFact>,
    path: &DataFlowPath,
    barrier: &BarrierPattern,
) -> bool {
    match barrier.kind() {
        BarrierPatternKind::None => false,
        BarrierPatternKind::CallAny => path.edges.iter().any(|edge_id| {
            store
                .edges()
                .iter()
                .find(|edge| edge.id == *edge_id)
                .and_then(edge_call_site)
                .is_some_and(|site| barrier_matches_call_site(db, site_by_id, site, barrier))
        }),
    }
}

fn barrier_matches_call_site(
    db: &AnalysisDb,
    site_by_id: &BTreeMap<CallSiteId, &CallSiteFact>,
    site: CallSiteId,
    barrier: &BarrierPattern,
) -> bool {
    if barrier.kind() != BarrierPatternKind::CallAny {
        return false;
    }
    let mut candidates = db
        .refined_call_edges()
        .iter()
        .filter(|edge| edge.site == site)
        .flat_map(|edge| call_edge_candidates(db, edge, site_by_id))
        .collect::<Vec<_>>();
    if let Some(site) = site_by_id.get(&site) {
        candidates.push(call_site_label(site));
    }
    candidates.sort();
    candidates.dedup();
    barrier
        .values()
        .iter()
        .any(|value| candidates.iter().any(|candidate| candidate == value))
}

fn edge_call_site(edge: &DataFlowEdgeFact) -> Option<CallSiteId> {
    edge.call_site.or_else(|| {
        edge.evidence.iter().find_map(|evidence| {
            evidence
                .strip_prefix("call_site=")
                .and_then(|value| value.parse::<u64>().ok())
                .map(CallSiteId)
        })
    })
}

fn flow_violation(
    _db: &AnalysisDb,
    store: &DataFlowStore,
    source: &FlowSource,
    sink: &FlowSink,
    path: &DataFlowPath,
    query: &FlowQuery,
) -> PolicyViolation {
    let (status, precision) = match path.status {
        DataFlowPathStatus::Found => {
            let path_precision = path_precision(store, path).unwrap_or(source.precision);
            let precision =
                min_policy_precision([source.precision, sink.precision, path_precision]);
            let status =
                if source.heuristic || sink.heuristic || precision == PolicyPrecision::Heuristic {
                    PolicyStatus::Heuristic
                } else {
                    PolicyStatus::Exact
                };
            (status, precision)
        }
        DataFlowPathStatus::Unknown => (PolicyStatus::Unknown, PolicyPrecision::Unknown),
        DataFlowPathStatus::BudgetExceeded => {
            (PolicyStatus::BudgetExceeded, PolicyPrecision::Unknown)
        }
        DataFlowPathStatus::NotFound => (PolicyStatus::Unknown, PolicyPrecision::Unknown),
    };

    let barrier_status = match query.barriers.kind() {
        BarrierPatternKind::None => "not_configured",
        BarrierPatternKind::CallAny => "uncovered",
    };
    let mut evidence = vec![
        ("policy".to_string(), "forbidden_flow".to_string()),
        ("source".to_string(), source.label.clone()),
        ("sink".to_string(), sink.target_label.clone()),
        ("sink_call_site".to_string(), sink.site.0.to_string()),
        (
            "path_status".to_string(),
            data_flow_path_status_label(path.status).to_string(),
        ),
        ("path".to_string(), path_label(store, path)),
        ("path_edge_count".to_string(), path.edges.len().to_string()),
        ("barrier_status".to_string(), barrier_status.to_string()),
        (
            "supported_scope".to_string(),
            "bounded_private_data_flow".to_string(),
        ),
        (
            "requested_max_depth".to_string(),
            query.max_depth.to_string(),
        ),
        (
            "requested_max_paths".to_string(),
            query.max_paths.to_string(),
        ),
        (
            "minimum_precision".to_string(),
            policy_precision_label(query.minimum_precision).to_string(),
        ),
    ];
    if query.barriers.kind() == BarrierPatternKind::CallAny {
        evidence.push((
            "required_barrier".to_string(),
            query.barriers.values().join(","),
        ));
    }
    if let Some(reason) = path.budget_reason {
        evidence.push(("budget_reason".to_string(), format!("{reason:?}")));
    }
    if let Some(confidence) = path_confidence(store, path) {
        evidence.push((
            "data_flow_confidence".to_string(),
            data_flow_confidence_label(confidence).to_string(),
        ));
    }

    PolicyViolation::new(sink.file.clone(), sink.range, status, precision, evidence)
}

fn flow_result_key(source: &FlowSource, sink: &FlowSink, path: &DataFlowPath) -> String {
    format!(
        "{}:{}:{:?}:{:?}",
        source.node.0, sink.node.0, path.status, path.edges
    )
}

fn path_precision(store: &DataFlowStore, path: &DataFlowPath) -> Option<PolicyPrecision> {
    let mut precision = None;
    for edge in path_edges(store, path) {
        precision = Some(match precision {
            Some(current) => {
                min_policy_precision([current, policy_precision_from_data_flow(edge.precision)])
            }
            None => policy_precision_from_data_flow(edge.precision),
        });
    }
    precision
}

fn path_confidence(store: &DataFlowStore, path: &DataFlowPath) -> Option<DataFlowConfidence> {
    let mut confidence = None;
    for edge in path_edges(store, path) {
        confidence = Some(match confidence {
            Some(current) => min_data_flow_confidence(current, edge.confidence),
            None => edge.confidence,
        });
    }
    confidence
}

fn path_edges<'a>(store: &'a DataFlowStore, path: &DataFlowPath) -> Vec<&'a DataFlowEdgeFact> {
    path.edges
        .iter()
        .filter_map(|id| store.edges().iter().find(|edge| edge.id == *id))
        .collect()
}

fn path_label(store: &DataFlowStore, path: &DataFlowPath) -> String {
    if path.edges.is_empty() {
        return data_flow_path_status_label(path.status).to_string();
    }
    path_edges(store, path)
        .into_iter()
        .map(|edge| data_flow_edge_step_label(edge.kind))
        .collect::<Vec<_>>()
        .join(" -> ")
}

fn min_policy_precision<const N: usize>(values: [PolicyPrecision; N]) -> PolicyPrecision {
    values
        .into_iter()
        .min_by_key(|precision| precision_rank(*precision))
        .unwrap_or(PolicyPrecision::Unknown)
}

fn min_data_flow_confidence(
    left: DataFlowConfidence,
    right: DataFlowConfidence,
) -> DataFlowConfidence {
    if data_flow_confidence_rank(left) <= data_flow_confidence_rank(right) {
        left
    } else {
        right
    }
}

fn source_model_matches_values(
    store: &DataFlowStore,
    node: &DataFlowNodeFact,
    values: &[String],
) -> bool {
    node.model
        .and_then(|model| store.models().iter().find(|fact| fact.id == model))
        .is_some_and(|model| {
            model
                .payload_labels
                .iter()
                .chain(model.model_id.iter())
                .any(|label| contains_any_folded(label, values))
        })
}

fn source_node_label(store: &DataFlowStore, node: &DataFlowNodeFact) -> String {
    node.model
        .and_then(|model| store.models().iter().find(|fact| fact.id == model))
        .and_then(|model| model.model_id.clone())
        .unwrap_or_else(|| "source".to_string())
}

fn place_by_id(db: &AnalysisDb, place: PlaceId) -> Option<&PlaceFact> {
    db.mir_places().iter().find(|fact| fact.id == place)
}

fn place_matches_values(place: &PlaceFact, values: &[String]) -> bool {
    place_name_values(place)
        .iter()
        .any(|candidate| contains_any_folded(candidate, values))
}

fn place_label(place: &PlaceFact) -> String {
    place_name_values(place)
        .into_iter()
        .find(|value| !value.is_empty())
        .unwrap_or_else(|| "place".to_string())
}

fn place_name_values(place: &PlaceFact) -> Vec<String> {
    let mut values = Vec::new();
    match &place.root {
        PlaceRoot::Local { name, .. } | PlaceRoot::Global { name, .. } => {
            values.push(name.clone());
        }
        PlaceRoot::Parameter { name, index, .. } => {
            values.push(name.clone().unwrap_or_else(|| format!("param{index}")));
        }
        PlaceRoot::Temporary { ordinal, .. } => values.push(format!("temporary{ordinal}")),
        PlaceRoot::CallReturn { call } => values.push(format!("call_return{}", call.0)),
        PlaceRoot::Unknown { evidence } => values.push(evidence.clone()),
    }
    for projection in &place.projections {
        match projection {
            PlaceProjection::Field(value)
            | PlaceProjection::Property(value)
            | PlaceProjection::IndexKnown(value) => values.push(value.clone()),
            PlaceProjection::IndexUnknown { evidence } | PlaceProjection::Unknown { evidence } => {
                values.push(evidence.clone());
            }
            PlaceProjection::Deref => values.push("deref".to_string()),
            PlaceProjection::AwaitResult => values.push("await".to_string()),
            PlaceProjection::CallReturn(call) => values.push(format!("call_return{}", call.0)),
        }
    }
    values
}

fn contains_any_folded(candidate: &str, values: &[String]) -> bool {
    if values.is_empty() {
        return false;
    }
    let candidate = candidate.to_ascii_lowercase();
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .any(|value| candidate.contains(&value))
}

fn http_source_label(label: &str) -> bool {
    [
        "source_kind=PathParam",
        "source_kind=QueryString",
        "source_kind=RequestBody",
        "source_kind=RequestHeader",
        "source_kind=Cookie",
    ]
    .contains(&label)
}

fn logger_candidate(candidate: &str) -> bool {
    let candidate = candidate.to_ascii_lowercase();
    candidate == "log"
        || candidate == "print"
        || candidate == "printf"
        || candidate == "println"
        || candidate == "log.print"
        || candidate == "log.printf"
        || candidate == "log.println"
        || candidate == "console.log"
        || candidate == "logger.log"
        || candidate == "logger.info"
        || candidate == "logger.warn"
        || candidate == "logger.error"
        || candidate.ends_with(".log")
        || candidate.ends_with(".info")
        || candidate.ends_with(".warn")
        || candidate.ends_with(".error")
}

fn policy_precision_from_data_flow(precision: DataFlowPrecision) -> PolicyPrecision {
    match precision {
        DataFlowPrecision::Exact => PolicyPrecision::Exact,
        DataFlowPrecision::SetupAware => PolicyPrecision::SetupAware,
        DataFlowPrecision::Syntax => PolicyPrecision::Syntax,
        DataFlowPrecision::Conservative => PolicyPrecision::Conservative,
        DataFlowPrecision::Heuristic => PolicyPrecision::Heuristic,
        DataFlowPrecision::Unknown => PolicyPrecision::Unknown,
    }
}

fn data_flow_path_status_label(status: DataFlowPathStatus) -> &'static str {
    match status {
        DataFlowPathStatus::Found => "found",
        DataFlowPathStatus::NotFound => "not_found",
        DataFlowPathStatus::Unknown => "unknown",
        DataFlowPathStatus::BudgetExceeded => "budget_exceeded",
    }
}

fn data_flow_edge_step_label(kind: DataFlowEdgeKind) -> &'static str {
    match kind {
        DataFlowEdgeKind::LocalBinding
        | DataFlowEdgeKind::LocalAssignment
        | DataFlowEdgeKind::LocalUse
        | DataFlowEdgeKind::LocalRead
        | DataFlowEdgeKind::LocalWrite
        | DataFlowEdgeKind::FieldProjection
        | DataFlowEdgeKind::IndexProjection
        | DataFlowEdgeKind::Dereference
        | DataFlowEdgeKind::AddressOf => "local_value",
        DataFlowEdgeKind::ReturnValue => "return_value",
        DataFlowEdgeKind::CallArgumentToParameter
        | DataFlowEdgeKind::CallArgumentToReturn
        | DataFlowEdgeKind::CallReturnToUse
        | DataFlowEdgeKind::ReceiverToMethod => "call_boundary",
        DataFlowEdgeKind::SummaryTito | DataFlowEdgeKind::SummaryProjected => "summary",
        DataFlowEdgeKind::UnknownFlow | DataFlowEdgeKind::HavocFlow => "unknown",
        DataFlowEdgeKind::BudgetTruncated => "budget",
        DataFlowEdgeKind::SourceIntroduction => "source",
        DataFlowEdgeKind::SinkReachability => "sink",
        DataFlowEdgeKind::Sanitizer => "sanitizer",
        DataFlowEdgeKind::Barrier => "barrier",
        DataFlowEdgeKind::Model => "model",
    }
}

fn data_flow_confidence_label(confidence: DataFlowConfidence) -> &'static str {
    match confidence {
        DataFlowConfidence::High => "high",
        DataFlowConfidence::Medium => "medium",
        DataFlowConfidence::Low => "low",
    }
}

fn data_flow_confidence_rank(confidence: DataFlowConfidence) -> u8 {
    match confidence {
        DataFlowConfidence::High => 3,
        DataFlowConfidence::Medium => 2,
        DataFlowConfidence::Low => 1,
    }
}

fn policy_precision_label(precision: PolicyPrecision) -> &'static str {
    match precision {
        PolicyPrecision::Exact => "exact",
        PolicyPrecision::SetupAware => "setup_aware",
        PolicyPrecision::Syntax => "syntax",
        PolicyPrecision::Conservative => "conservative",
        PolicyPrecision::Heuristic => "heuristic",
        PolicyPrecision::Unknown => "unknown",
    }
}

#[derive(Debug, Clone)]
struct SearchState {
    function: FunctionId,
    depth: usize,
    path: Vec<String>,
}

#[derive(Debug, Clone)]
struct ControlCallEvent {
    function: FunctionId,
    file: String,
    range: crate::diagnostics::TextRange,
    target_label: String,
    candidates: Vec<String>,
    order: ControlOrder,
    order_source: &'static str,
    status: CallTargetStatus,
    precision: CallPrecision,
    confidence: Option<RefinedCallConfidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ControlOrder {
    line: u32,
    col: u32,
    byte: u32,
    operation_ordinal: u32,
    stable_key: String,
}

#[derive(Debug, Clone, Copy)]
struct CfgOrder {
    operation_ordinal: u32,
}

fn call_sites_by_id(db: &AnalysisDb) -> BTreeMap<CallSiteId, &CallSiteFact> {
    db.call_sites().iter().map(|site| (site.id, site)).collect()
}

fn ordered_control_call_events(
    db: &AnalysisDb,
    minimum_precision: PolicyPrecision,
) -> Vec<ControlCallEvent> {
    let site_by_id = call_sites_by_id(db);
    let cfg_orders = cfg_orders_by_operation(db);
    let mut events = Vec::new();

    if !db.refined_call_edges().is_empty() {
        for edge in db.refined_call_edges() {
            if !precision_meets(edge.precision, minimum_precision) {
                continue;
            }
            if let Some(event) = control_event_for_edge(db, edge, &site_by_id, &cfg_orders) {
                events.push(event);
            }
        }
    } else {
        for site in db.call_sites() {
            if precision_rank(policy_precision_from_call_precision(site.precision))
                < precision_rank(minimum_precision)
            {
                continue;
            }
            events.push(control_event_for_site(db, site, &cfg_orders));
        }
    }

    events.sort_by(|left, right| {
        (
            left.function,
            &left.order,
            left.target_label.as_str(),
            left.file.as_str(),
        )
            .cmp(&(
                right.function,
                &right.order,
                right.target_label.as_str(),
                right.file.as_str(),
            ))
    });
    events
}

fn cfg_orders_by_operation(db: &AnalysisDb) -> BTreeMap<(MirBodyId, MirOpId), CfgOrder> {
    let mut orders = BTreeMap::new();
    for node in db.cfg_nodes() {
        if let Some(operation) = node.operation {
            let key = (node.body, operation);
            orders
                .entry(key)
                .and_modify(|existing: &mut CfgOrder| {
                    existing.operation_ordinal =
                        existing.operation_ordinal.min(node.operation_ordinal);
                })
                .or_insert(CfgOrder {
                    operation_ordinal: node.operation_ordinal,
                });
        }
    }
    orders
}

fn control_event_for_edge(
    db: &AnalysisDb,
    edge: &RefinedCallEdgeFact,
    site_by_id: &BTreeMap<CallSiteId, &CallSiteFact>,
    cfg_orders: &BTreeMap<(MirBodyId, MirOpId), CfgOrder>,
) -> Option<ControlCallEvent> {
    let site = site_by_id.get(&edge.site)?;
    let mut candidates = call_edge_candidates(db, edge, site_by_id);
    if candidates.is_empty() {
        candidates.push(call_site_label(site));
    }
    let target_label = best_call_target_label(db, edge, site_by_id);
    Some(ControlCallEvent {
        function: edge.caller,
        file: db.path_for(site.file),
        range: site.span.diagnostic_range(),
        target_label,
        candidates,
        order: control_order(site, cfg_orders),
        order_source: control_order_source(site, cfg_orders),
        status: edge.status,
        precision: edge.precision,
        confidence: Some(edge.confidence),
    })
}

fn control_event_for_site(
    db: &AnalysisDb,
    site: &CallSiteFact,
    cfg_orders: &BTreeMap<(MirBodyId, MirOpId), CfgOrder>,
) -> ControlCallEvent {
    let label = call_site_label(site);
    ControlCallEvent {
        function: site.caller,
        file: db.path_for(site.file),
        range: site.span.diagnostic_range(),
        target_label: label.clone(),
        candidates: vec![label],
        order: control_order(site, cfg_orders),
        order_source: control_order_source(site, cfg_orders),
        status: site.status,
        precision: site.precision,
        confidence: None,
    }
}

fn control_order(
    site: &CallSiteFact,
    cfg_orders: &BTreeMap<(MirBodyId, MirOpId), CfgOrder>,
) -> ControlOrder {
    let cfg_order = cfg_orders.get(&(site.body, site.operation));
    ControlOrder {
        line: site.span.start_line,
        col: site.span.start_col,
        byte: site.span.start_byte,
        operation_ordinal: cfg_order
            .map(|order| order.operation_ordinal)
            .unwrap_or(u32::MAX),
        stable_key: site.stable_key.clone(),
    }
}

fn control_order_source(
    site: &CallSiteFact,
    cfg_orders: &BTreeMap<(MirBodyId, MirOpId), CfgOrder>,
) -> &'static str {
    if cfg_orders.contains_key(&(site.body, site.operation)) {
        "cfg_operation_order"
    } else {
        "source_span"
    }
}

fn control_events_by_function(
    events: &[ControlCallEvent],
) -> BTreeMap<FunctionId, Vec<&ControlCallEvent>> {
    let mut by_function = BTreeMap::<FunctionId, Vec<&ControlCallEvent>>::new();
    for event in events {
        by_function.entry(event.function).or_default().push(event);
    }
    by_function
}

fn event_matches_pattern(event: &ControlCallEvent, pattern: &EventPattern) -> bool {
    if pattern.kind() != EventPatternKind::Call {
        return false;
    }
    pattern
        .values()
        .iter()
        .any(|value| event.candidates.iter().any(|candidate| candidate == value))
}

fn guard_matches_event(event: &ControlCallEvent, guard: &GuardPattern) -> bool {
    match guard.kind() {
        GuardPatternKind::CallAny => guard
            .values()
            .iter()
            .any(|value| event.candidates.iter().any(|candidate| candidate == value)),
    }
}

fn control_violation(
    db: &AnalysisDb,
    event: &ControlCallEvent,
    mut evidence: Vec<(&'static str, String)>,
) -> PolicyViolation {
    evidence.extend([
        ("target", event.target_label.clone()),
        (
            "function",
            function_by_id(db, event.function)
                .map(|function| function.name.clone())
                .unwrap_or_else(|| "<unknown-function>".to_string()),
        ),
        ("control_scope", "same_function".to_string()),
        ("supported_depth", "1".to_string()),
        ("order_source", event.order_source.to_string()),
        ("call_status", call_status_label(event.status).to_string()),
        (
            "call_precision",
            call_precision_label(event.precision).to_string(),
        ),
    ]);
    if let Some(confidence) = event.confidence {
        evidence.push(("confidence", confidence_label(confidence).to_string()));
    }

    PolicyViolation::new(
        event.file.clone(),
        event.range,
        PolicyStatus::Heuristic,
        PolicyPrecision::Conservative,
        evidence
            .into_iter()
            .map(|(label, value)| (label.to_string(), value))
            .collect(),
    )
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
    use crate::analysis::data_flow::facts::{
        DataFlowAlgorithm, DataFlowConfidence, DataFlowEdgeFact, DataFlowEdgeKind,
        DataFlowModelFact, DataFlowModelKind, DataFlowNodeFact, DataFlowNodeKind,
        DataFlowPrecision, DataFlowProvenance, DataFlowStatus, DataFlowValidation,
    };
    use crate::analysis::data_flow::store::DataFlowOutput;
    use crate::analysis::ids::{
        CallTargetId, DataFlowEdgeId, DataFlowModelId, DataFlowNodeId, MirBodyId, MirOpId, PlaceId,
        ReachabilityRootId, RefinedCallEdgeId,
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

    #[test]
    fn control_flow_missing_guard_reports_call_without_prior_guard() {
        let db = policy_query_db_with_call_sequence(&["dangerous", "authorize"]);
        let query = GuardQuery::new(
            EventPattern::call("dangerous"),
            GuardPattern::call_any(["authorize"]),
        );

        let violations = missing_guards(&db, query);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].status(), PolicyStatus::Heuristic);
        assert_eq!(violations[0].precision(), PolicyPrecision::Conservative);
        let diagnostic = violations[0].diagnostic("local/test", "missing guard");
        assert!(
            diagnostic
                .evidence
                .iter()
                .any(|evidence| evidence.label == "policy" && evidence.value == "missing_guard")
        );
        assert!(diagnostic.evidence.iter().any(|evidence| {
            evidence.label == "required_guard" && evidence.value == "authorize"
        }));
    }

    #[test]
    fn control_flow_missing_guard_ignores_prior_guard() {
        let db = policy_query_db_with_call_sequence(&["authorize", "dangerous"]);
        let query = GuardQuery::new(
            EventPattern::call("dangerous"),
            GuardPattern::call_any(["authorize"]),
        );

        assert!(missing_guards(&db, query).is_empty());
    }

    #[test]
    fn control_flow_missing_guard_ignores_write_field_until_backed() {
        let db = policy_query_db_with_call_sequence(&["dangerous"]);
        let query = GuardQuery::new(
            EventPattern::write_field("account.balance"),
            GuardPattern::call_any(["authorize"]),
        );

        assert!(missing_guards(&db, query).is_empty());
    }

    #[test]
    fn control_flow_missing_guard_marks_budget_when_path_cap_truncates() {
        let db = policy_query_db_with_two_matching_targets();
        let mut query = GuardQuery::new(
            EventPattern::call("dangerous"),
            GuardPattern::call_any(["authorize"]),
        );
        query.max_paths = 1;

        let violations = missing_guards(&db, query);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].status(), PolicyStatus::BudgetExceeded);
        let diagnostic = violations[0].diagnostic("local/test", "missing guard");
        assert!(diagnostic.evidence.iter().any(|evidence| {
            evidence.label == "budget" && evidence.value == "control_flow_query_budget"
        }));
    }

    #[test]
    fn control_flow_missing_cleanup_reports_start_without_later_cleanup() {
        let db = policy_query_db_with_call_sequence(&["Begin", "work"]);
        let query =
            LifecycleQuery::new(EventPattern::call("Begin"), EventPattern::call("Rollback"));

        let violations = missing_cleanup(&db, query);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].status(), PolicyStatus::Heuristic);
        assert_eq!(violations[0].precision(), PolicyPrecision::Conservative);
        let diagnostic = violations[0].diagnostic("local/test", "missing cleanup");
        assert!(
            diagnostic
                .evidence
                .iter()
                .any(|evidence| evidence.label == "policy" && evidence.value == "missing_cleanup")
        );
        assert!(diagnostic.evidence.iter().any(|evidence| {
            evidence.label == "required_cleanup" && evidence.value == "Rollback"
        }));
    }

    #[test]
    fn control_flow_missing_cleanup_ignores_later_cleanup() {
        let db = policy_query_db_with_call_sequence(&["Begin", "work", "Rollback"]);
        let query =
            LifecycleQuery::new(EventPattern::call("Begin"), EventPattern::call("Rollback"));

        assert!(missing_cleanup(&db, query).is_empty());
    }

    #[test]
    fn data_flow_forbidden_reports_http_source_to_call_sink() {
        let db = data_flow_policy_db(vec![data_flow_edge(
            0,
            0,
            1,
            DataFlowEdgeKind::SourceIntroduction,
            DataFlowStatus::Present,
            Vec::new(),
        )]);

        let violations = forbidden_flows(
            &db,
            FlowQuery::new(
                SourcePattern::http_request(),
                SinkPattern::call("dangerous"),
            ),
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].status(), PolicyStatus::Heuristic);
        assert_eq!(violations[0].precision(), PolicyPrecision::SetupAware);
        let diagnostic = violations[0].diagnostic("local/test", "request reaches sink");
        assert!(
            diagnostic
                .evidence
                .iter()
                .any(|evidence| evidence.label == "policy" && evidence.value == "forbidden_flow")
        );
        assert!(diagnostic.evidence.iter().any(|evidence| {
            evidence.label == "barrier_status" && evidence.value == "not_configured"
        }));
    }

    #[test]
    fn data_flow_forbidden_respects_barrier_call() {
        let db = data_flow_policy_db_with_barrier();
        let mut query = FlowQuery::new(
            SourcePattern::http_request(),
            SinkPattern::call("dangerous"),
        );
        query.barriers = BarrierPattern::call_any(["validate"]);

        assert!(forbidden_flows(&db, query).is_empty());
    }

    #[test]
    fn data_flow_forbidden_marks_budget_when_depth_cap_truncates() {
        let db = data_flow_policy_db_with_sink_place(
            vec![
                data_flow_edge(
                    0,
                    0,
                    1,
                    DataFlowEdgeKind::SourceIntroduction,
                    DataFlowStatus::Present,
                    Vec::new(),
                ),
                data_flow_edge(
                    1,
                    1,
                    2,
                    DataFlowEdgeKind::LocalAssignment,
                    DataFlowStatus::Present,
                    Vec::new(),
                ),
            ],
            PlaceId(2),
        );
        let mut query = FlowQuery::new(
            SourcePattern::http_request(),
            SinkPattern::call("dangerous"),
        );
        query.max_depth = 1;

        let violations = forbidden_flows(&db, query);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].status(), PolicyStatus::BudgetExceeded);
        let diagnostic = violations[0].diagnostic("local/test", "request reaches sink");
        assert!(diagnostic.evidence.iter().any(|evidence| {
            evidence.label == "budget_reason" && evidence.value == "PathDepth"
        }));
    }

    #[test]
    fn data_flow_forbidden_surfaces_unknown_path() {
        let db = data_flow_policy_db(vec![data_flow_edge(
            0,
            0,
            1,
            DataFlowEdgeKind::UnknownFlow,
            DataFlowStatus::Unknown,
            Vec::new(),
        )]);

        let violations = forbidden_flows(
            &db,
            FlowQuery::new(
                SourcePattern::http_request(),
                SinkPattern::call("dangerous"),
            ),
        );

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].status(), PolicyStatus::Unknown);
        assert_eq!(violations[0].precision(), PolicyPrecision::Unknown);
    }

    #[test]
    fn data_flow_forbidden_returns_no_results_without_store() {
        let db = AnalysisDb::new();

        assert!(
            forbidden_flows(
                &db,
                FlowQuery::new(
                    SourcePattern::http_request(),
                    SinkPattern::call("dangerous")
                )
            )
            .is_empty()
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

    fn policy_query_db_with_call_sequence(callees: &[&str]) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\nfunc handler() {}\n".to_string(),
        );
        let handler = push_test_function(&mut db, file, "handler", 1, false);
        let mut sites = Vec::new();
        let mut targets = Vec::new();
        let mut edges = Vec::new();

        for (index, callee) in callees.iter().enumerate() {
            let id = index as u64;
            let callee_function =
                push_test_function(&mut db, file, callee, index as u32 + 10, false);
            let site = call_site(id, file, handler, callee);
            targets.push(call_target(id, site.id, handler, callee_function));
            edges.push(refined_edge(id, site.id, handler, callee_function, callee));
            sites.push(site);
        }

        db.replace_call_facts(CallOutput {
            sites,
            targets,
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput { edges })
            .expect("valid refined call facts");
        db
    }

    fn data_flow_policy_db(edges: Vec<DataFlowEdgeFact>) -> AnalysisDb {
        data_flow_policy_db_with_sink_place(edges, PlaceId(1))
    }

    fn data_flow_policy_db_with_sink_place(
        edges: Vec<DataFlowEdgeFact>,
        sink_place: PlaceId,
    ) -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\nfunc handler() { dangerous(input) }\n".to_string(),
        );
        let handler = push_test_function(&mut db, file, "handler", 1, false);
        let dangerous = push_test_function(&mut db, file, "dangerous", 2, false);
        let site = call_site_with_args(1, file, handler, "dangerous", vec![sink_place]);

        db.replace_call_facts(CallOutput {
            sites: vec![site.clone()],
            targets: vec![call_target(1, site.id, handler, dangerous)],
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![refined_edge(1, site.id, handler, dangerous, "dangerous")],
        })
        .expect("valid refined call facts");
        db.replace_data_flow_facts(DataFlowOutput {
            nodes: data_flow_nodes(file, handler),
            edges,
            models: vec![source_model()],
            budgets: Vec::new(),
        })
        .expect("valid data-flow facts");
        db
    }

    fn data_flow_policy_db_with_barrier() -> AnalysisDb {
        let mut db = AnalysisDb::new();
        let file = db.add_file(
            PathBuf::from("src/main.go"),
            "src/main.go".to_string(),
            "package main\nfunc handler() { dangerous(validate(input)) }\n".to_string(),
        );
        let handler = push_test_function(&mut db, file, "handler", 1, false);
        let dangerous = push_test_function(&mut db, file, "dangerous", 2, false);
        let validate = push_test_function(&mut db, file, "validate", 3, false);
        let dangerous_site = call_site_with_args(1, file, handler, "dangerous", vec![PlaceId(2)]);
        let validate_site = call_site_with_args(2, file, handler, "validate", vec![PlaceId(1)]);

        db.replace_call_facts(CallOutput {
            sites: vec![dangerous_site.clone(), validate_site.clone()],
            targets: vec![
                call_target(1, dangerous_site.id, handler, dangerous),
                call_target(2, validate_site.id, handler, validate),
            ],
            unresolved: Vec::new(),
        })
        .expect("valid call facts");
        db.replace_refined_call_facts(RefinedCallOutput {
            edges: vec![
                refined_edge(1, dangerous_site.id, handler, dangerous, "dangerous"),
                refined_edge(2, validate_site.id, handler, validate, "validate"),
            ],
        })
        .expect("valid refined call facts");
        db.replace_data_flow_facts(DataFlowOutput {
            nodes: data_flow_nodes(file, handler),
            edges: vec![
                data_flow_edge(
                    0,
                    0,
                    1,
                    DataFlowEdgeKind::SourceIntroduction,
                    DataFlowStatus::Present,
                    Vec::new(),
                ),
                data_flow_edge(
                    1,
                    1,
                    2,
                    DataFlowEdgeKind::CallArgumentToReturn,
                    DataFlowStatus::Present,
                    vec!["call_site=2".to_string()],
                ),
            ],
            models: vec![source_model()],
            budgets: Vec::new(),
        })
        .expect("valid data-flow facts");
        db
    }

    fn data_flow_nodes(file: FileId, function: FunctionId) -> Vec<DataFlowNodeFact> {
        vec![
            DataFlowNodeFact {
                id: DataFlowNodeId(0),
                kind: DataFlowNodeKind::Source,
                language: Language::Go,
                file: Some(file),
                function: Some(function),
                body: None,
                operation: None,
                cfg_node: None,
                place: None,
                symbol: None,
                reference: None,
                call_site: None,
                model: Some(DataFlowModelId(0)),
                span: Some(Span::point(file, 1, 1)),
                stable_key: "node:source".to_string(),
            },
            data_flow_place_node(1, file, function, PlaceId(1)),
            data_flow_place_node(2, file, function, PlaceId(2)),
        ]
    }

    fn data_flow_place_node(
        id: u64,
        file: FileId,
        function: FunctionId,
        place: PlaceId,
    ) -> DataFlowNodeFact {
        DataFlowNodeFact {
            id: DataFlowNodeId(id),
            kind: DataFlowNodeKind::Place,
            language: Language::Go,
            file: Some(file),
            function: Some(function),
            body: None,
            operation: None,
            cfg_node: None,
            place: Some(place),
            symbol: None,
            reference: None,
            call_site: None,
            model: None,
            span: Some(Span::point(file, id as u32 + 1, 1)),
            stable_key: format!("node:place:{id}"),
        }
    }

    fn source_model() -> DataFlowModelFact {
        DataFlowModelFact {
            id: DataFlowModelId(0),
            kind: DataFlowModelKind::Source,
            language: Language::Go,
            provider_id: "test".to_string(),
            model_id: Some("QueryString".to_string()),
            source_stable_key: Some("trust-boundary:test".to_string()),
            status: DataFlowStatus::Present,
            precision: DataFlowPrecision::SetupAware,
            validation: DataFlowValidation::ReferentiallyValidated,
            confidence: DataFlowConfidence::High,
            provenance: DataFlowProvenance::Native,
            evidence: vec!["trust_boundary".to_string()],
            payload_labels: vec!["source_kind=QueryString".to_string()],
            stable_key: "model:source".to_string(),
        }
    }

    fn data_flow_edge(
        id: u64,
        from: u64,
        to: u64,
        kind: DataFlowEdgeKind,
        status: DataFlowStatus,
        evidence: Vec<String>,
    ) -> DataFlowEdgeFact {
        DataFlowEdgeFact {
            id: DataFlowEdgeId(id),
            from: DataFlowNodeId(from),
            to: DataFlowNodeId(to),
            kind,
            algorithm: DataFlowAlgorithm::LocalMir,
            status,
            precision: if status == DataFlowStatus::Present {
                match kind {
                    DataFlowEdgeKind::LocalBinding
                    | DataFlowEdgeKind::LocalAssignment
                    | DataFlowEdgeKind::LocalUse
                    | DataFlowEdgeKind::LocalRead
                    | DataFlowEdgeKind::LocalWrite => DataFlowPrecision::Syntax,
                    _ => DataFlowPrecision::SetupAware,
                }
            } else {
                DataFlowPrecision::Unknown
            },
            validation: DataFlowValidation::Native,
            confidence: if status == DataFlowStatus::Present {
                DataFlowConfidence::High
            } else {
                DataFlowConfidence::Low
            },
            provenance: DataFlowProvenance::Native,
            call_site: None,
            call_target: None,
            refined_call: None,
            model: None,
            budget: None,
            evidence,
            input_stable_keys: Vec::new(),
            stable_key: format!("edge:{id}:{from}:{to}:{status:?}"),
        }
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

    fn call_site_with_args(
        id: u64,
        file: FileId,
        caller: FunctionId,
        callee: &str,
        arguments: Vec<PlaceId>,
    ) -> CallSiteFact {
        let mut site = call_site(id, file, caller, callee);
        site.arguments = arguments;
        site
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
