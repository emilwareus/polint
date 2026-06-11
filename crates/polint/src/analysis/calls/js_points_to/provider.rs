//! DB entry point: harvest every JS/TS file into one constraint program, solve
//! the points-to fixpoint, and map resolved `(call, function)` edges back to
//! [`CallTargetFact`]s emitted into `call_targets` beside the recognizers.
//!
//! Edges are labeled [`CallAlgorithm::PointsTo`], which the jelly reachability
//! filter treats like `ThisMethodFlow`: present in the adjacency (so they
//! propagate and un-prune) but NOT a reachability root — a points-to edge can
//! never make a dead function reachable on its own (the iteration-53 discipline,
//! applied to the fixpoint).

use std::collections::{BTreeMap, BTreeSet};

use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;

use crate::analysis::calls::facts::{
    CallAlgorithm, CallCallee, CallEdgeKind, CallPrecision, CallProvenance, CallSiteFact,
    CallTargetFact, CallTargetStatus,
};
use crate::analysis::ids::CallTargetId;
use crate::core::{AnalysisDb, FileId, FunctionId};

use super::harvest::{CalleeHint, Harvester};
use super::solver::PointsToBudget;

/// Resolve call targets via the value-token heap. Additive: returns edges to be
/// appended to `call_targets` (deduped downstream by the calls provider). Honors
/// the same span/identity conventions the recognizers use so the edges line up
/// with the kernel's `CallSiteFact`s.
pub(crate) fn resolve_js_points_to_targets(
    db: &AnalysisDb,
    sites: &[CallSiteFact],
    id_offset: u64,
) -> Vec<CallTargetFact> {
    // `(file, span.start, span.end) -> FunctionId`: a function literal's kernel
    // identity, used as the edge target payload.
    let mut function_id_by_span: BTreeMap<(FileId, u32, u32), FunctionId> = BTreeMap::new();
    for function in db
        .functions()
        .iter()
        .filter(|function| function.language.is_ts_family())
    {
        function_id_by_span
            .entry((
                function.file,
                function.span.start_byte,
                function.span.end_byte,
            ))
            .or_insert(function.id);
    }

    let resolution_map = crate::analysis::calls::ts_value_flows::module_resolution_map(db);
    let mut harvester = Harvester::new(&function_id_by_span, &resolution_map);
    for file in db
        .files()
        .iter()
        .filter(|file| file.language.is_ts_family())
    {
        let allocator = Allocator::default();
        let parsed = Parser::new(
            &allocator,
            file.source.as_ref(),
            SourceType::from_path(&file.path).unwrap_or_default(),
        )
        .parse();
        if parsed.panicked && parsed.program.body.is_empty() {
            continue;
        }
        harvester.harvest_file(file.id, &parsed.program);
        // `allocator` drops here; the harvester retains no AST references (its
        // constraints own their strings), so this is safe.
    }

    // `solve` borrows the program immutably and returns an owned result, so the
    // harvested call records can be read by reference afterward — no clone.
    let result = harvester.program.solve(&PointsToBudget::default());
    let calls = &harvester.calls;

    // Index sites by file for span matching.
    let mut sites_by_file: BTreeMap<FileId, Vec<&CallSiteFact>> = BTreeMap::new();
    for site in sites {
        sites_by_file.entry(site.file).or_default().push(site);
    }

    // Map each resolved (call, function) edge onto the real `CallSiteFact`(s) the
    // call expression owns, by span containment + callee-hint agreement.
    let mut emitted: BTreeSet<(crate::analysis::ids::CallSiteId, FunctionId)> = BTreeSet::new();
    let mut rows = Vec::new();
    for (call_id, payload) in &result.edges {
        let Some(record) = calls.get(*call_id as usize) else {
            continue;
        };
        let target = FunctionId(*payload);
        let Some(file_sites) = sites_by_file.get(&record.file) else {
            continue;
        };
        for site in file_sites {
            if site.span.start_byte < record.start || site.span.end_byte > record.end {
                continue;
            }
            // Identifier/member callees match by name within the span; a
            // call-result/computed callee (`f(a)(b)`, `x[k]()`) has no usable name
            // hint, so match the site whose span is EXACTLY the call expression's
            // (uniquely the outer call). A universal exact-span fallback was tried
            // and reverted: it surfaced over-resolved edges the name filter
            // correctly suppresses (+2 TP / +4 FP).
            let matches = match &record.hint {
                CalleeHint::Other => {
                    site.span.start_byte == record.start && site.span.end_byte == record.end
                }
                _ => callee_matches(&site.callee, &record.hint),
            };
            if !matches {
                continue;
            }
            if !emitted.insert((site.id, target)) {
                continue;
            }
            rows.push(CallTargetFact {
                id: CallTargetId(id_offset + rows.len() as u64),
                site: site.id,
                caller: site.caller,
                target_function: Some(target),
                target_symbol: None,
                edge_kind: CallEdgeKind::FunctionValue,
                algorithm: CallAlgorithm::PointsTo,
                status: CallTargetStatus::Resolved,
                reason: None,
                provenance: CallProvenance::Model,
                precision: CallPrecision::Heuristic,
                stable_key: format!("js-points-to:{}:fn:{}", site.stable_key, target.0),
            });
        }
    }
    rows
}

/// Does a resolved call site's callee agree with the harvested call's hint? An
/// identifier hint matches an `Identifier` callee of the same name; a member hint
/// matches a `Member` callee with the same property. (A call-result/curried callee
/// has no name and is matched by exact span at the emission site instead.)
fn callee_matches(callee: &CallCallee, hint: &CalleeHint) -> bool {
    match (callee, hint) {
        (CallCallee::Identifier { name, .. }, CalleeHint::Ident(expected)) => name == expected,
        (CallCallee::Member { property, .. }, CalleeHint::Member(expected)) => property == expected,
        _ => false,
    }
}
