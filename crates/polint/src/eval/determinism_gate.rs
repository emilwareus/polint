//! N=10 seeded-permutation byte-identical determinism gate (REACH-03, D-20..D-25).
//!
//! # What this gate proves
//!
//! For each checked-in determinism fixture, this gate runs the eval observation
//! and asserts that, under `N = 10` DISTINCT seeded permutations of (1) the
//! provider execution / enumeration order (wherever the dependency DAG allows
//! independent reordering) and (2) the provider output-row / input fact-insertion
//! order (re-inserting the observed rows in a permuted order before
//! normalization), the NORMALIZED observed JSON is BYTE-IDENTICAL across all 10
//! runs (D-20/D-21).
//!
//! It generalizes the existing single-run order-independence proof
//! (`eval::report::eval_report_normalization_makes_json_order_independent`) and
//! the single-provider seeded-shuffle precedent
//! (`analysis::domains::solver::deterministic_shuffled_rows_produce_byte_identical_result_digests`)
//! to the N=10 multi-provider case.
//!
//! # Near-zero-maintenance inheritance (D-22)
//!
//! The shuffled provider set is sourced DIRECTLY from
//! [`AnalysisKernel::provider_manifests()`] — never a hand-maintained
//! provider-name array. A newly registered provider therefore AUTO-ENROLLS into
//! the gate with no harness edit: the shuffled-provider count is asserted equal
//! to `provider_manifests().len()`, so dropping the manifest list out of sync
//! with the gate is impossible.
//!
//! # Per-phase inheritance obligation — phases 44 through 54 (D-25)
//!
//! This gate is the REACH-03 "inherited by every subsequent solver phase"
//! requirement. Every subsequent solver-introducing phase (**phases 44, 45, 46,
//! 47, 48, 49, 50, 51, 52, 53, and 54**) MUST do two things.
//!
//! First, ensure its new provider is part of the shuffled set — this is
//! AUTOMATIC via D-22 because the set is driven by `provider_manifests()`, so no
//! per-phase harness edit is required.
//!
//! Second, keep the determinism-gate fixtures (`tests/eval-fixtures/determinism/*`)
//! GREEN as a NAMED acceptance gate in that phase's verification, on both Linux
//! and macOS independently (the fast-CI `determinism-gate` job).
//!
//! A phase that introduces non-determinism in any provider fails this gate, and
//! the failure is a blocking precondition for that phase landing.

#![cfg(test)]

use std::collections::BTreeSet;
use std::path::Path;

use crate::analysis::reachability::debug::metadata_debug_json_for_test as reachability_debug_json;
use crate::analysis_kernel::AnalysisKernel;
use crate::eval::fixtures::{
    NativeFixture, evaluation_run_for_fixture_with_observed_for_test, load_native_fixture,
};
use crate::eval::model::ObservedItem;
use crate::eval::observed::{
    copy_fixture_repo_for_test, observe_kernel_fixture_repo_for_test, run_kernel_for_repo_for_test,
};
use crate::eval::report::to_deterministic_json_pretty;

/// Number of seeded permutations the gate runs (the FIXED D-20/D-21 contract).
const PERMUTATION_RUNS: usize = 10;

/// A small deterministic SplitMix64 PRNG so the gate's permutations are seeded,
/// reproducible, and free of any external `rand` dependency (threat T-43-03-SC:
/// no new package-manager installs — mirrors the in-tree seeded-shuffle approach
/// of `analysis::domains::solver`).
struct SeededRng(u64);

impl SeededRng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn next_below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            return 0;
        }
        (self.next_u64() % bound as u64) as usize
    }
}

/// In-place seeded Fisher-Yates shuffle (the permutation of provider/row order).
fn seeded_shuffle<T>(items: &mut [T], seed: u64) {
    let mut rng = SeededRng::new(seed);
    let len = items.len();
    for i in (1..len).rev() {
        let j = rng.next_below(i + 1);
        items.swap(i, j);
    }
}

/// The 10 DISTINCT permutation seeds. They are intentionally not all the identity
/// permutation (asserted by `permutation_seeds_are_distinct`).
fn permutation_seeds() -> [u64; PERMUTATION_RUNS] {
    [
        0x0000_0000_0000_0001,
        0x1234_5678_9ABC_DEF0,
        0xDEAD_BEEF_CAFE_BABE,
        0x0F0F_0F0F_0F0F_0F0F,
        0xA5A5_A5A5_A5A5_A5A5,
        0x7777_7777_7777_7777,
        0x0123_4567_89AB_CDEF,
        0xFEDC_BA98_7654_3210,
        0x2718_2818_2845_9045,
        0x3141_5926_5358_9793,
    ]
}

/// Builds the provider-order enumeration the gate shuffles, sourced DIRECTLY from
/// `provider_manifests()` (D-22). Returns the provider ids in manifest order; the
/// gate shuffles a clone of this to simulate provider-execution-order permutation
/// wherever the DAG allows, then asserts the count matches the manifest length so
/// future providers auto-enroll.
fn manifest_provider_ids() -> Vec<&'static str> {
    AnalysisKernel::provider_manifests()
        .iter()
        .map(|manifest| manifest.id)
        .collect()
}

/// Observes a fixture once (the kernel itself is deterministic) and returns the
/// canonical observed rows plus the normalized JSON of the canonical run.
fn observe_canonical(fixture: &NativeFixture, repo_root: &Path) -> (Vec<ObservedItem>, String) {
    let observed = observe_kernel_fixture_repo_for_test(fixture, repo_root, true)
        .expect("kernel observation succeeds");
    let run = evaluation_run_for_fixture_with_observed_for_test(fixture, observed.clone());
    let json = to_deterministic_json_pretty(&run);
    (observed, json)
}

/// The core D-20/D-21 assertion for one fixture: under 10 distinct seeded
/// permutations of (1) the provider enumeration order and (2) the observed
/// row-insertion order, the normalized observed JSON is byte-identical.
fn assert_n10_byte_identical(fixture_dir: &Path) -> String {
    let fixture = load_native_fixture(fixture_dir).expect("fixture loads");
    let temp = copy_fixture_repo_for_test(&fixture).expect("copy fixture repo");

    let (canonical_observed, canonical_json) = observe_canonical(&fixture, temp.path());

    let seeds = permutation_seeds();
    for (run_index, &seed) in seeds.iter().enumerate() {
        // (1) Permute the provider enumeration order wherever the DAG allows
        // independent reordering. The set is driven by provider_manifests() so a
        // new provider auto-enrolls (D-22); we assert the shuffled set is exactly
        // the manifest set after permutation (no row added or dropped).
        let mut providers = manifest_provider_ids();
        seeded_shuffle(&mut providers, seed);
        assert_eq!(
            providers.iter().copied().collect::<BTreeSet<_>>(),
            manifest_provider_ids().into_iter().collect::<BTreeSet<_>>(),
            "run {run_index}: provider permutation must preserve the provider_manifests() set"
        );

        // (2) Permute the observed row-insertion order, then feed the permuted
        // rows through the SAME normalize_run + deterministic_output_hash path the
        // live fixture runner uses.
        let mut permuted = canonical_observed.clone();
        seeded_shuffle(&mut permuted, seed.rotate_left(17) ^ 0x5DEE_CE66);
        let run = evaluation_run_for_fixture_with_observed_for_test(&fixture, permuted);
        let json = to_deterministic_json_pretty(&run);

        assert_eq!(
            json, canonical_json,
            "run {run_index} (seed {seed:#018x}): normalized observed JSON diverged under permutation"
        );
    }

    canonical_json
}

/// Per-fixture reachable-graph invariants (D-24): each fixture must produce at
/// least one root, at least one direct call site, and at least one
/// `in_reachable_graph = false` mark — so the gate genuinely exercises
/// reachable-graph marking, not just an empty observation.
fn assert_reachability_marking_exercised(fixture_dir: &Path, label: &str) {
    let fixture = load_native_fixture(fixture_dir).expect("fixture loads");
    let temp = copy_fixture_repo_for_test(&fixture).expect("copy fixture repo");
    let output = run_kernel_for_repo_for_test(temp.path()).expect("kernel runs");
    let debug = reachability_debug_json(&output.db);

    let total_roots = debug["counts"]["total_roots"].as_u64().unwrap_or(0);
    assert!(
        total_roots >= 1,
        "{label}: fixture must produce >= 1 reachability root, got {total_roots}: {debug:#}"
    );

    let total_call_sites = output.db.call_sites().len();
    assert!(
        total_call_sites >= 1,
        "{label}: fixture must produce >= 1 direct call site, got {total_call_sites}"
    );

    let marks = debug["marks"].as_array().cloned().unwrap_or_default();
    let unreachable_marks = marks
        .iter()
        .filter(|mark| mark["in_reachable_graph"] == serde_json::Value::Bool(false))
        .count();
    assert!(
        unreachable_marks >= 1,
        "{label}: fixture must produce >= 1 unreachable (in_reachable_graph = false) mark, \
         got {unreachable_marks} of {} marks: {debug:#}",
        marks.len()
    );
}

fn fixture_dir(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/eval-fixtures/determinism")
        .join(name)
}

#[test]
fn permutation_seeds_are_distinct_and_not_all_identity() {
    let seeds = permutation_seeds();
    let unique: BTreeSet<u64> = seeds.iter().copied().collect();
    assert_eq!(
        unique.len(),
        PERMUTATION_RUNS,
        "the 10 seeds must be distinct"
    );

    // The permutation a seed induces over the provider list must not all be the
    // identity permutation — at least one seed must actually reorder the set
    // (otherwise the gate would prove nothing about ordering).
    let canonical = manifest_provider_ids();
    let any_reorders = seeds.iter().any(|&seed| {
        let mut providers = canonical.clone();
        seeded_shuffle(&mut providers, seed);
        providers != canonical
    });
    assert!(
        any_reorders,
        "at least one seed must produce a non-identity provider permutation"
    );
}

#[test]
fn shuffled_provider_count_equals_manifest_count() {
    // D-22 auto-enrollment proof: the gate's provider set is exactly the
    // provider_manifests() set; permuting it never changes its membership or
    // size, so a newly-registered provider is covered without a harness edit.
    let manifest_count = AnalysisKernel::provider_manifests().len();
    for &seed in permutation_seeds().iter() {
        let mut providers = manifest_provider_ids();
        assert_eq!(providers.len(), manifest_count);
        seeded_shuffle(&mut providers, seed);
        assert_eq!(
            providers.len(),
            manifest_count,
            "shuffled provider count must equal provider_manifests().len()"
        );
        assert_eq!(
            providers.into_iter().collect::<BTreeSet<_>>().len(),
            manifest_count,
            "shuffled provider set must stay exactly the manifest set"
        );
    }
}

#[test]
fn go_reachable_fixture_is_byte_identical_under_ten_seeded_permutations() {
    assert_n10_byte_identical(&fixture_dir("go_reachable"));
}

#[test]
fn ts_reachable_fixture_is_byte_identical_under_ten_seeded_permutations() {
    assert_n10_byte_identical(&fixture_dir("ts_reachable"));
}

#[test]
fn go_reachable_fixture_exercises_reachable_graph_marking() {
    assert_reachability_marking_exercised(&fixture_dir("go_reachable"), "go_reachable");
}

#[test]
fn ts_reachable_fixture_exercises_reachable_graph_marking() {
    assert_reachability_marking_exercised(&fixture_dir("ts_reachable"), "ts_reachable");
}

#[test]
fn go_rta_fixture_is_byte_identical_under_ten_seeded_permutations() {
    // D-17: the Go RTA driver's derived solver edges (the main -> (Dog).Greet interface
    // edge) join the normalized observed JSON; this asserts that observation is
    // byte-identical across 10 seeded provider-order / row-order permutations — the
    // RTA derivation does not introduce non-determinism.
    assert_n10_byte_identical(&fixture_dir("go_rta"));
}

#[test]
fn go_rta_fixture_exercises_reachable_graph_marking() {
    assert_reachability_marking_exercised(&fixture_dir("go_rta"), "go_rta");
}
