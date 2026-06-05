//! Deterministic JS/TS function-token fixpoint.
//!
//! The lattice is intentionally small: each semantic node may hold a set of
//! function tokens, or the stable `"too-many-tokens"` sentinel when the configured
//! per-variable cap is exceeded. The sentinel is monotonic and is never treated as a
//! callable target.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::analysis::ids::SemanticNodeId;
use crate::analysis::solver::budget::{BudgetReason, BudgetStatus, SolverBudget};
use crate::analysis::solver::facts::DerivedEdgeFact;

use super::dispatch::resolve_token_callsite;
use super::inputs::{TsFunctionToken, TsTokenCopyEdge, TsTokenInputs};

/// Stable render for an over-cap token set.
pub(crate) const TOO_MANY_TOKENS: &str = "too-many-tokens";

/// Result of a TS function-token solve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TsTokenSolveResult {
    pub(crate) tokens_by_var: BTreeMap<SemanticNodeId, TsTokenVarState>,
    pub(crate) derived_edges: Vec<DerivedEdgeFact>,
    pub(crate) budget_status: BudgetStatus,
    pub(crate) budget_reasons: BTreeSet<String>,
    pub(crate) steps: u64,
}

/// Token state for one semantic-node variable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TsTokenVarState {
    Tokens(BTreeMap<SemanticNodeId, BTreeSet<String>>),
    TooManyTokens,
}

impl TsTokenVarState {
    pub(crate) fn render_stable(&self) -> String {
        match self {
            Self::Tokens(tokens) => tokens
                .keys()
                .map(|node| node.0.to_string())
                .collect::<Vec<_>>()
                .join(","),
            Self::TooManyTokens => TOO_MANY_TOKENS.to_string(),
        }
    }

    pub(crate) fn tokens(&self) -> Option<&BTreeMap<SemanticNodeId, BTreeSet<String>>> {
        match self {
            Self::Tokens(tokens) => Some(tokens),
            Self::TooManyTokens => None,
        }
    }

    fn is_too_many_tokens(&self) -> bool {
        matches!(self, Self::TooManyTokens)
    }
}

/// Run the bounded function-token fixpoint and derive concrete token call edges.
pub(crate) fn solve_ts_tokens(inputs: &TsTokenInputs, budget: &SolverBudget) -> TsTokenSolveResult {
    let copy_edges_by_src = copy_edges_by_src(&inputs.copy_edges);
    let handoffs_by_callsite = handoffs_by_callsite(inputs);
    let mut states: BTreeMap<SemanticNodeId, TsTokenVarState> = BTreeMap::new();
    let mut queue = VecDeque::new();
    let mut queued = BTreeSet::new();
    let mut budget_exceeded = false;
    let mut budget_reasons = BTreeSet::new();
    let mut steps = 0_u64;

    for token in inputs.function_tokens.values() {
        let outcome = insert_token(
            &mut states,
            token.function_node,
            token.function_node,
            seed_evidence(token),
            budget.js.max_tokens_per_var,
        );
        if outcome.budget_exceeded {
            budget_exceeded = true;
            budget_reasons.insert(BudgetReason::JsMaxTokensPerVar.as_str().to_string());
        }
        if outcome.changed {
            enqueue(token.function_node, &mut queue, &mut queued);
        }
    }

    'worklist: while let Some(src) = queue.pop_front() {
        queued.remove(&src);
        let Some(edges) = copy_edges_by_src.get(&src) else {
            continue;
        };
        let Some(source_state) = states.get(&src).cloned() else {
            continue;
        };

        for edge in edges {
            steps += 1;
            if steps > budget.js.max_token_worklist_steps as u64 {
                budget_exceeded = true;
                budget_reasons.insert(BudgetReason::JsMaxTokenWorklistSteps.as_str().to_string());
                break 'worklist;
            }

            match &source_state {
                TsTokenVarState::TooManyTokens => {
                    if mark_too_many_tokens(&mut states, edge.dst) {
                        budget_exceeded = true;
                        budget_reasons.insert(BudgetReason::JsMaxTokensPerVar.as_str().to_string());
                        enqueue(edge.dst, &mut queue, &mut queued);
                    }
                }
                TsTokenVarState::Tokens(tokens) => {
                    for (&function_node, evidence) in tokens {
                        let mut propagated = evidence.clone();
                        propagated.insert(edge.stable_key.clone());
                        let outcome = insert_token(
                            &mut states,
                            edge.dst,
                            function_node,
                            propagated,
                            budget.js.max_tokens_per_var,
                        );
                        if outcome.budget_exceeded {
                            budget_exceeded = true;
                            budget_reasons
                                .insert(BudgetReason::JsMaxTokensPerVar.as_str().to_string());
                        }
                        if outcome.changed {
                            enqueue(edge.dst, &mut queue, &mut queued);
                        }
                    }
                }
            }
        }
    }

    let mut edges_by_key = BTreeMap::new();
    for callsite in &inputs.callsites {
        steps += 1;
        if steps > budget.js.max_token_worklist_steps as u64 {
            budget_exceeded = true;
            budget_reasons.insert(BudgetReason::JsMaxTokenWorklistSteps.as_str().to_string());
            break;
        }

        let Some(state) = states.get(&callsite.callsite_node) else {
            continue;
        };
        let Some(tokens) = state.tokens() else {
            continue;
        };
        let handoff_keys = handoffs_by_callsite
            .get(&callsite.callsite_node)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let resolution = resolve_token_callsite(
            callsite,
            tokens,
            handoff_keys,
            budget.js.max_candidates_per_callsite,
            steps,
        );
        if resolution.candidate_cap_exceeded {
            budget_exceeded = true;
            budget_reasons.insert(
                BudgetReason::JsMaxCandidatesPerCallsite
                    .as_str()
                    .to_string(),
            );
        }
        for edge in resolution.edges {
            edges_by_key.entry(edge.stable_key.clone()).or_insert(edge);
        }
    }

    TsTokenSolveResult {
        tokens_by_var: states,
        derived_edges: edges_by_key.into_values().collect(),
        budget_status: if budget_exceeded {
            BudgetStatus::BudgetExceeded
        } else {
            BudgetStatus::WithinBudget
        },
        budget_reasons,
        steps,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct InsertOutcome {
    changed: bool,
    budget_exceeded: bool,
}

fn insert_token(
    states: &mut BTreeMap<SemanticNodeId, TsTokenVarState>,
    var: SemanticNodeId,
    function_node: SemanticNodeId,
    evidence: BTreeSet<String>,
    max_tokens_per_var: usize,
) -> InsertOutcome {
    let state = states
        .entry(var)
        .or_insert_with(|| TsTokenVarState::Tokens(BTreeMap::new()));

    let mut overflow = false;
    let mut changed = false;
    match state {
        TsTokenVarState::TooManyTokens => return InsertOutcome::default(),
        TsTokenVarState::Tokens(tokens) => {
            if !tokens.contains_key(&function_node) && tokens.len() >= max_tokens_per_var {
                overflow = true;
            } else {
                let token_evidence = tokens.entry(function_node).or_default();
                let before = token_evidence.len();
                token_evidence.extend(evidence);
                changed = token_evidence.len() != before;
            }
        }
    }

    if overflow {
        *state = TsTokenVarState::TooManyTokens;
        InsertOutcome {
            changed: true,
            budget_exceeded: true,
        }
    } else {
        InsertOutcome {
            changed,
            budget_exceeded: false,
        }
    }
}

fn mark_too_many_tokens(
    states: &mut BTreeMap<SemanticNodeId, TsTokenVarState>,
    var: SemanticNodeId,
) -> bool {
    if states
        .get(&var)
        .is_some_and(TsTokenVarState::is_too_many_tokens)
    {
        return false;
    }
    states.insert(var, TsTokenVarState::TooManyTokens);
    true
}

fn enqueue(
    node: SemanticNodeId,
    queue: &mut VecDeque<SemanticNodeId>,
    queued: &mut BTreeSet<SemanticNodeId>,
) {
    if queued.insert(node) {
        queue.push_back(node);
    }
}

fn copy_edges_by_src(
    copy_edges: &[TsTokenCopyEdge],
) -> BTreeMap<SemanticNodeId, Vec<&TsTokenCopyEdge>> {
    let mut by_src: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for edge in copy_edges {
        by_src.entry(edge.src).or_default().push(edge);
    }
    by_src
}

fn handoffs_by_callsite(inputs: &TsTokenInputs) -> BTreeMap<SemanticNodeId, Vec<String>> {
    let mut by_callsite = BTreeMap::new();
    for handoff in &inputs.handoffs {
        by_callsite
            .entry(handoff.callsite_node)
            .or_insert_with(Vec::new)
            .push(handoff.binding_stable_key.clone());
    }
    by_callsite
}

fn seed_evidence(token: &TsFunctionToken) -> BTreeSet<String> {
    BTreeSet::from([
        token.function_stable_key.clone(),
        token.source_stable_key.clone(),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::solver::ts_tokens::inputs::{
        TsTokenCallsite, TsTokenCopyEdge, TsTokenHandoff,
    };
    use crate::ts::binding::facts::TsDirectBindingReason;

    #[test]
    fn alias_copy_propagates_token_to_alias_variable() {
        let inputs = inputs_with_edges(vec![copy_edge("copy:target-to-alias", 1, 2)]);

        let result = solve_ts_tokens(&inputs, &SolverBudget::default());

        assert!(tokens_at(&result, 2).contains_key(&SemanticNodeId(1)));
        assert_eq!(result.budget_status, BudgetStatus::WithinBudget);
    }

    #[test]
    fn parameter_return_flow_reaches_callsite_candidate() {
        let inputs = TsTokenInputs {
            function_tokens: function_tokens(&[(1, "target")]),
            copy_edges: vec![
                copy_edge("copy:arg-to-param", 1, 20),
                copy_edge("copy:param-to-return", 20, 30),
                copy_edge("copy:return-to-callsite", 30, 9),
            ],
            callsites: vec![callsite(9)],
            ..TsTokenInputs::default()
        };

        let result = solve_ts_tokens(&inputs, &SolverBudget::default());

        assert_eq!(result.derived_edges.len(), 1);
        assert_eq!(result.derived_edges[0].source, SemanticNodeId(100));
        assert_eq!(result.derived_edges[0].target, SemanticNodeId(1));
        assert!(
            result.derived_edges[0]
                .provenance
                .stable_key_fragment()
                .contains("copy:return-to-callsite")
        );
    }

    #[test]
    fn closure_capture_flow_uses_lexical_copy_inputs() {
        let inputs = TsTokenInputs {
            function_tokens: function_tokens(&[(1, "captured")]),
            copy_edges: vec![
                copy_edge("copy:captured-to-closure-slot", 1, 40),
                copy_edge("copy:closure-slot-to-return", 40, 41),
                copy_edge("copy:return-to-callsite", 41, 9),
            ],
            callsites: vec![callsite(9)],
            ..TsTokenInputs::default()
        };

        let result = solve_ts_tokens(&inputs, &SolverBudget::default());

        assert_eq!(result.derived_edges.len(), 1);
        assert_eq!(result.derived_edges[0].target, SemanticNodeId(1));
        assert!(
            result.derived_edges[0]
                .provenance
                .stable_key_fragment()
                .contains("copy:captured-to-closure-slot")
        );
    }

    #[test]
    fn overflow_collapses_to_too_many_tokens_and_latches_budget_exceeded() {
        let mut budget = SolverBudget::default();
        budget.js.max_tokens_per_var = 1;
        let inputs = TsTokenInputs {
            function_tokens: function_tokens(&[(1, "a"), (2, "b")]),
            copy_edges: vec![copy_edge("copy:a", 1, 9), copy_edge("copy:b", 2, 9)],
            ..TsTokenInputs::default()
        };

        let result = solve_ts_tokens(&inputs, &budget);

        assert_eq!(
            result.tokens_by_var[&SemanticNodeId(9)].render_stable(),
            TOO_MANY_TOKENS
        );
        assert_eq!(result.budget_status, BudgetStatus::BudgetExceeded);
    }

    #[test]
    fn sentinel_is_never_emitted_as_callable_target() {
        let mut budget = SolverBudget::default();
        budget.js.max_tokens_per_var = 1;
        let inputs = TsTokenInputs {
            function_tokens: function_tokens(&[(1, "a"), (2, "b")]),
            copy_edges: vec![copy_edge("copy:a", 1, 9), copy_edge("copy:b", 2, 9)],
            callsites: vec![callsite(9)],
            ..TsTokenInputs::default()
        };

        let result = solve_ts_tokens(&inputs, &budget);

        assert!(result.derived_edges.is_empty());
        assert_eq!(result.budget_status, BudgetStatus::BudgetExceeded);
    }

    #[test]
    fn permuting_inputs_yields_byte_identical_normalized_token_results() {
        let forward = TsTokenInputs {
            function_tokens: function_tokens(&[(1, "a"), (2, "b")]),
            copy_edges: vec![copy_edge("copy:a", 1, 9), copy_edge("copy:b", 2, 9)],
            callsites: vec![callsite(9)],
            handoffs: vec![handoff(9, "binding:token")],
        }
        .normalized();
        let reversed = TsTokenInputs {
            function_tokens: function_tokens(&[(2, "b"), (1, "a")]),
            copy_edges: vec![copy_edge("copy:b", 2, 9), copy_edge("copy:a", 1, 9)],
            callsites: vec![callsite(9)],
            handoffs: vec![handoff(9, "binding:token")],
        }
        .normalized();

        let first = solve_ts_tokens(&forward, &SolverBudget::default());
        let second = solve_ts_tokens(&reversed, &SolverBudget::default());

        let first_json = serde_json::to_string(&(
            first.tokens_by_var,
            first
                .derived_edges
                .iter()
                .map(|edge| edge.stable_key.as_str())
                .collect::<Vec<_>>(),
            first.budget_status,
        ))
        .expect("serialize first token result");
        let second_json = serde_json::to_string(&(
            second.tokens_by_var,
            second
                .derived_edges
                .iter()
                .map(|edge| edge.stable_key.as_str())
                .collect::<Vec<_>>(),
            second.budget_status,
        ))
        .expect("serialize second token result");

        assert_eq!(first_json, second_json);
    }

    #[test]
    fn callsite_candidate_cap_latches_budget_exceeded() {
        let mut budget = SolverBudget::default();
        budget.js.max_candidates_per_callsite = 1;
        let inputs = TsTokenInputs {
            function_tokens: function_tokens(&[(1, "a"), (2, "b")]),
            copy_edges: vec![copy_edge("copy:a", 1, 9), copy_edge("copy:b", 2, 9)],
            callsites: vec![callsite(9)],
            ..TsTokenInputs::default()
        };

        let result = solve_ts_tokens(&inputs, &budget);

        assert_eq!(result.derived_edges.len(), 1);
        assert_eq!(result.budget_status, BudgetStatus::BudgetExceeded);
    }

    fn inputs_with_edges(copy_edges: Vec<TsTokenCopyEdge>) -> TsTokenInputs {
        TsTokenInputs {
            function_tokens: function_tokens(&[(1, "target")]),
            copy_edges,
            ..TsTokenInputs::default()
        }
    }

    fn function_tokens(items: &[(u64, &str)]) -> BTreeMap<SemanticNodeId, TsFunctionToken> {
        items
            .iter()
            .map(|&(node, suffix)| {
                (
                    SemanticNodeId(node),
                    TsFunctionToken {
                        function_node: SemanticNodeId(node),
                        function_stable_key: format!("function:{suffix}"),
                        source_stable_key: format!("semantic:function:{suffix}"),
                    },
                )
            })
            .collect()
    }

    fn copy_edge(stable_key: &str, src: u64, dst: u64) -> TsTokenCopyEdge {
        TsTokenCopyEdge {
            dst: SemanticNodeId(dst),
            src: SemanticNodeId(src),
            stable_key: stable_key.to_string(),
        }
    }

    fn callsite(node: u64) -> TsTokenCallsite {
        TsTokenCallsite {
            caller_node: SemanticNodeId(100),
            callsite_node: SemanticNodeId(node),
            callsite_stable_key: format!("callsite:{node}"),
            constraint_stable_key: format!("constraint:call:{node}"),
        }
    }

    fn handoff(node: u64, stable_key: &str) -> TsTokenHandoff {
        TsTokenHandoff {
            callsite_node: SemanticNodeId(node),
            binding_stable_key: stable_key.to_string(),
            reason: TsDirectBindingReason::TokenFlowRequired,
        }
    }

    fn tokens_at(
        result: &TsTokenSolveResult,
        node: u64,
    ) -> &BTreeMap<SemanticNodeId, BTreeSet<String>> {
        result.tokens_by_var[&SemanticNodeId(node)]
            .tokens()
            .expect("concrete token set")
    }
}
