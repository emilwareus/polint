//! Ceiling on the *materialised* dominance relation.
//!
//! `derive_dominators` and `derive_postdominators` emit one fact per
//! *(dominated, dominator)* pair. That relation is `O(blocks²)` per function,
//! and each pair's stable key embeds the function key **and both block keys** —
//! roughly 1.4 KB of interned identity text per pair, retained for the whole
//! run. On excalidraw (385 files) the two relations are the single largest
//! memory owner in the pipeline.
//!
//! Nothing in the relation is *information*: dominance is the reflexive
//! transitive closure of the immediate-dominator tree, which is `O(blocks)`.
//! So when the relation would be too large to materialise, polint emits the
//! tree (`immediate == true`) instead of the closure and says so, rather than
//! spending gigabytes on a derivable fact family.
//!
//! This is a *precision* budget in the same family as `solver::budget`: a fixed
//! default, reported when exceeded. The separate `analysis_kernel::resource`
//! envelope is the *resource* safety net for the run as a whole.

use super::store::CfgOutput;

/// Pairs a run may materialise across both dominance relations.
///
/// The estimate below is a worst case — a straight-line function of `b` blocks
/// contributes `b(b+1)/2` — so a repository has to be genuinely large before it
/// trips. For calibration: excalidraw (385 files, 4,193 functions averaging
/// 13.6 blocks) estimates ~900,000 pairs and is bounded; this repository's own
/// fixtures and `examples/` projects are orders of magnitude smaller and are
/// not, which the suite's diagnostic and snapshot assertions keep honest.
pub const DEFAULT_MAX_DOMINANCE_PAIRS: usize = 250_000;

/// Environment override for [`DEFAULT_MAX_DOMINANCE_PAIRS`].
///
/// `0` disables the bound and always materialises the full relation, which is
/// what the A/B in `.scale-envelope/EXPERIMENTS.md` uses to show that bounding
/// it leaves `polint check` diagnostics byte-identical.
const MAX_DOMINANCE_PAIRS_ENV: &str = "POLINT_CFG_MAX_DOMINANCE_PAIRS";

/// The active ceiling for this run.
pub fn max_dominance_pairs() -> usize {
    match std::env::var(MAX_DOMINANCE_PAIRS_ENV) {
        Ok(raw) => match raw.trim().parse::<usize>() {
            Ok(0) => usize::MAX,
            Ok(limit) => limit,
            Err(_) => {
                tracing::warn!(
                    target: "polint::kernel",
                    value = raw,
                    "ignoring unparseable {MAX_DOMINANCE_PAIRS_ENV}; using the default ceiling"
                );
                DEFAULT_MAX_DOMINANCE_PAIRS
            }
        },
        Err(_) => DEFAULT_MAX_DOMINANCE_PAIRS,
    }
}

/// Worst-case size of the dominance relation `output` would materialise.
///
/// Counting the real relation means computing it, which is the expensive half
/// of the CFG stage; this bound needs only the per-function block counts and is
/// exact for straight-line control flow. Being an over-estimate makes the
/// budget conservative — it can bound a run that would just have fitted, never
/// the reverse.
pub fn worst_case_dominance_pairs(output: &CfgOutput) -> usize {
    let mut per_function = std::collections::BTreeMap::new();
    for block in &output.blocks {
        *per_function.entry(block.cfg_function).or_insert(0usize) += 1;
    }
    // Both relations (dominance and post-dominance) are materialised, hence x2.
    per_function
        .values()
        .map(|blocks| blocks.saturating_mul(blocks.saturating_add(1)) / 2)
        .sum::<usize>()
        .saturating_mul(2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis_neutral::cfg::facts::{
        BasicBlockFact, BasicBlockKind, CfgPrecision, CfgStatus,
    };
    use crate::analysis_neutral::cfg::ids::{BasicBlockId, CfgFunctionId};

    fn block(function: u64, id: u64) -> BasicBlockFact {
        BasicBlockFact {
            id: BasicBlockId(id),
            cfg_function: CfgFunctionId(function),
            kind: BasicBlockKind::StraightLine,
            first_node: None,
            last_node: None,
            reachable: true,
            reverse_postorder: 0,
            stable_key: crate::internal_core::stable_key_for_test("block"),
            status: CfgStatus::Resolved,
            precision: CfgPrecision::ExactLowered,
        }
    }

    #[test]
    fn an_empty_output_needs_no_pairs() {
        assert_eq!(worst_case_dominance_pairs(&CfgOutput::empty()), 0);
    }

    #[test]
    fn the_bound_is_quadratic_per_function_and_counts_both_relations() {
        let mut output = CfgOutput::empty();
        // One function with 3 blocks: 3*4/2 = 6 pairs, doubled for post-dominance.
        output.blocks = vec![block(1, 1), block(1, 2), block(1, 3)];
        assert_eq!(worst_case_dominance_pairs(&output), 12);

        // Splitting the same blocks across functions is cheaper than one big one.
        output.blocks = vec![block(1, 1), block(2, 2), block(3, 3)];
        assert_eq!(worst_case_dominance_pairs(&output), 6);
    }
}
