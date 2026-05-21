#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::domains::lattice::{AbstractDomain, TopReason, WidenFuel, WidenSite};

    fn loop_site() -> WidenSite {
        WidenSite {
            stable_key: "body:test:block:loop".to_string(),
        }
    }

    #[test]
    fn reachability_partial_order_tracks_unreachable_reachable_ambiguous() {
        assert!(ReachabilityDomain::Unreachable.leq(&ReachabilityDomain::Reachable));
        assert!(ReachabilityDomain::Reachable.leq(&ReachabilityDomain::Ambiguous));
        assert!(!ReachabilityDomain::Reachable.leq(&ReachabilityDomain::Unreachable));
    }

    #[test]
    fn constant_join_is_sorted_capped_literal_union() {
        let first = ConstantDomain::from_literal(ConstantLiteral::String("b".to_string()));
        let second = ConstantDomain::from_literal(ConstantLiteral::String("a".to_string()));

        assert_eq!(
            first.join(&second).stable_digest_parts(),
            vec![
                "constant:string:a".to_string(),
                "constant:string:b".to_string(),
                "kind=values".to_string()
            ]
        );
    }

    #[test]
    fn string_widen_promotes_over_cap_to_widened_top() {
        let base = StringDomain::from_literal("a");
        let next = StringDomain::from_literals(["a", "b", "c", "d", "e"]);

        assert_eq!(
            base.widen(&next, loop_site(), WidenFuel { remaining: 0 }),
            StringDomain::top(TopReason::Widened)
        );
    }

    #[test]
    fn initializedness_join_is_maybe_when_paths_disagree() {
        assert_eq!(
            InitializednessDomain::Initialized.join(&InitializednessDomain::Uninitialized),
            InitializednessDomain::MaybeUninitialized
        );
    }

    #[test]
    fn nilness_stable_digest_preserves_top_reason() {
        assert_ne!(
            NilnessDomain::top(TopReason::UnsupportedSemantic).stable_digest_parts(),
            NilnessDomain::top(TopReason::SetupMissing).stable_digest_parts()
        );
    }

    #[test]
    fn truthiness_partial_order_keeps_truthy_and_falsy_incomparable() {
        assert!(!TruthinessDomain::Truthy.leq(&TruthinessDomain::Falsy));
        assert!(TruthinessDomain::Truthy.leq(&TruthinessDomain::Maybe));
        assert!(TruthinessDomain::Falsy.leq(&TruthinessDomain::Maybe));
    }
}
