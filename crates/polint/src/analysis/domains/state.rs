#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::domains::core::{
        ConstantDomain, ConstantLiteral, InitializednessDomain, NilnessDomain, StringDomain,
        TruthinessDomain,
    };
    use crate::analysis::domains::lattice::Changed;
    use crate::analysis::ids::PlaceId;

    fn string_constant(value: &str) -> ConstantDomain {
        ConstantDomain::from_literal(ConstantLiteral::String(value.to_string()))
    }

    #[test]
    fn product_join_into_joins_every_place_slot() {
        let place = PlaceId(1);
        let mut left = ProductState::entry();
        left.core.nilness.insert(place, NilnessDomain::Nil);
        left.core.truthiness.insert(place, TruthinessDomain::Truthy);
        left.core.initializedness
            .insert(place, InitializednessDomain::Initialized);

        let mut right = ProductState::entry();
        right.core.nilness.insert(place, NilnessDomain::NonNil);
        right.core.truthiness.insert(place, TruthinessDomain::Falsy);
        right
            .core
            .initializedness
            .insert(place, InitializednessDomain::Uninitialized);

        assert_eq!(left.join_into(&right), Changed::Yes);
        assert_eq!(left.core.nilness[&place], NilnessDomain::MaybeNil);
        assert_eq!(left.core.truthiness[&place], TruthinessDomain::Maybe);
        assert_eq!(
            left.core.initializedness[&place],
            InitializednessDomain::MaybeUninitialized
        );
    }

    #[test]
    fn stable_digest_parts_are_independent_of_insertion_order() {
        let mut first = ProductState::entry();
        first
            .core
            .constants
            .insert(PlaceId(2), string_constant("second"));
        first
            .core
            .constants
            .insert(PlaceId(1), string_constant("first"));

        let mut second = ProductState::entry();
        second
            .core
            .constants
            .insert(PlaceId(1), string_constant("first"));
        second
            .core
            .constants
            .insert(PlaceId(2), string_constant("second"));

        assert_eq!(first.stable_digest_parts(), second.stable_digest_parts());
    }

    #[test]
    fn reduce_value_only_derives_value_facts_from_constants() {
        let nullish = PlaceId(1);
        let route = PlaceId(2);
        let mut state = ProductState::entry();
        state
            .core
            .constants
            .insert(nullish, ConstantDomain::from_literal(ConstantLiteral::Null));
        state.core.constants.insert(route, string_constant("/users/:id"));

        assert_eq!(state.reduce_value_only(4), Changed::Yes);
        assert_eq!(state.core.nilness[&nullish], NilnessDomain::Nil);
        assert_eq!(state.core.truthiness[&nullish], TruthinessDomain::Falsy);
        assert_eq!(state.core.nilness[&route], NilnessDomain::NonNil);
        assert_eq!(state.core.truthiness[&route], TruthinessDomain::Truthy);
        assert_eq!(
            state.core.strings[&route],
            StringDomain::from_literal("/users/:id")
        );
    }
}
