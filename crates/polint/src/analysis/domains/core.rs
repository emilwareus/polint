#![expect(dead_code, reason = "kept for private internal consumers")]

use std::collections::BTreeSet;

use super::lattice::{AbstractDomain, TopReason, WidenFuel, WidenSite, sorted_digest_parts};

const LITERAL_SET_CAP: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReachabilityDomain {
    Unreachable,
    Reachable,
    Ambiguous,
    Top(TopReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NilnessDomain {
    Bottom,
    Nil,
    NonNil,
    MaybeNil,
    Top(TopReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TruthinessDomain {
    Bottom,
    Truthy,
    Falsy,
    Maybe,
    Top(TopReason),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ConstantLiteral {
    Bool(bool),
    Null,
    Undefined,
    Nil,
    String(String),
    Number(String),
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ConstantDomain {
    Bottom,
    Values(BTreeSet<ConstantLiteral>),
    Top(TopReason),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum StringDomain {
    Bottom,
    Values(BTreeSet<String>),
    Top(TopReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InitializednessDomain {
    Bottom,
    Initialized,
    Uninitialized,
    MaybeUninitialized,
    Top(TopReason),
}

impl ConstantDomain {
    pub(crate) fn from_literal(literal: ConstantLiteral) -> Self {
        Self::Values(BTreeSet::from([literal]))
    }

    pub(crate) fn from_literals(literals: impl IntoIterator<Item = ConstantLiteral>) -> Self {
        let values = BTreeSet::from_iter(literals);
        if values.len() > LITERAL_SET_CAP {
            Self::Top(TopReason::BudgetExceeded)
        } else {
            Self::Values(values)
        }
    }
}

impl StringDomain {
    pub(crate) fn from_literal(literal: &str) -> Self {
        Self::Values(BTreeSet::from([literal.to_string()]))
    }

    pub(crate) fn from_literals<const N: usize>(literals: [&str; N]) -> Self {
        let values = literals
            .into_iter()
            .map(str::to_string)
            .collect::<BTreeSet<_>>();
        if values.len() > LITERAL_SET_CAP {
            Self::Top(TopReason::BudgetExceeded)
        } else {
            Self::Values(values)
        }
    }
}

impl AbstractDomain for ReachabilityDomain {
    const ID: &'static str = "polint.domain.reachability";
    const VERSION: u32 = 1;

    fn bottom() -> Self {
        Self::Unreachable
    }

    fn top(reason: TopReason) -> Self {
        Self::Top(reason)
    }

    fn is_bottom(&self) -> bool {
        matches!(self, Self::Unreachable)
    }

    fn is_top(&self) -> bool {
        matches!(self, Self::Top(_))
    }

    fn leq(&self, other: &Self) -> bool {
        match (self, other) {
            (left, right) if left == right => true,
            (Self::Top(left), Self::Top(right)) => top_reason_leq(*left, *right),
            (Self::Unreachable, _) => true,
            (_, Self::Top(_)) if !self.is_top() => true,
            (Self::Reachable, Self::Ambiguous) => true,
            _ => false,
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (left, right) if left == right => *left,
            (Self::Top(left), Self::Top(right)) => Self::Top(join_top_reasons(*left, *right)),
            (Self::Top(reason), _) | (_, Self::Top(reason)) => Self::Top(*reason),
            _ => join_chain(
                *self,
                *other,
                &[Self::Unreachable, Self::Reachable, Self::Ambiguous],
            ),
        }
    }

    fn stable_digest_parts(&self) -> Vec<String> {
        let part = match self {
            Self::Unreachable => "reachability=unreachable".to_string(),
            Self::Reachable => "reachability=reachable".to_string(),
            Self::Ambiguous => "reachability=ambiguous".to_string(),
            Self::Top(reason) => top_digest_part("reachability", *reason),
        };
        vec![part]
    }
}

impl AbstractDomain for NilnessDomain {
    const ID: &'static str = "polint.domain.nilness";
    const VERSION: u32 = 1;

    fn bottom() -> Self {
        Self::Bottom
    }

    fn top(reason: TopReason) -> Self {
        Self::Top(reason)
    }

    fn is_bottom(&self) -> bool {
        matches!(self, Self::Bottom)
    }

    fn is_top(&self) -> bool {
        matches!(self, Self::Top(_))
    }

    fn leq(&self, other: &Self) -> bool {
        match (self, other) {
            (left, right) if left == right => true,
            (Self::Top(left), Self::Top(right)) => top_reason_leq(*left, *right),
            (Self::Bottom, _) => true,
            (_, Self::Top(_)) if !self.is_top() => true,
            (Self::Nil | Self::NonNil, Self::MaybeNil) => true,
            _ => false,
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (left, right) if left == right => *left,
            (Self::Nil, Self::Nil) => Self::Nil,
            (Self::NonNil, Self::NonNil) => Self::NonNil,
            (Self::Top(left), Self::Top(right)) => Self::Top(join_top_reasons(*left, *right)),
            (Self::Top(reason), _) | (_, Self::Top(reason)) => Self::Top(*reason),
            (Self::Bottom, value) | (value, Self::Bottom) => *value,
            (Self::Nil, Self::NonNil) | (Self::NonNil, Self::Nil) => Self::MaybeNil,
            (Self::MaybeNil, _) | (_, Self::MaybeNil) => Self::MaybeNil,
        }
    }

    fn stable_digest_parts(&self) -> Vec<String> {
        vec![match self {
            Self::Bottom => "nilness=bottom".to_string(),
            Self::Nil => "nilness=nil".to_string(),
            Self::NonNil => "nilness=non_nil".to_string(),
            Self::MaybeNil => "nilness=maybe_nil".to_string(),
            Self::Top(reason) => top_digest_part("nilness", *reason),
        }]
    }
}

impl AbstractDomain for TruthinessDomain {
    const ID: &'static str = "polint.domain.truthiness";
    const VERSION: u32 = 1;

    fn bottom() -> Self {
        Self::Bottom
    }

    fn top(reason: TopReason) -> Self {
        Self::Top(reason)
    }

    fn is_bottom(&self) -> bool {
        matches!(self, Self::Bottom)
    }

    fn is_top(&self) -> bool {
        matches!(self, Self::Top(_))
    }

    fn leq(&self, other: &Self) -> bool {
        match (self, other) {
            (left, right) if left == right => true,
            (Self::Top(left), Self::Top(right)) => top_reason_leq(*left, *right),
            (Self::Bottom, _) => true,
            (_, Self::Top(_)) if !self.is_top() => true,
            (Self::Truthy | Self::Falsy, Self::Maybe) => true,
            _ => false,
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (left, right) if left == right => *left,
            (Self::Truthy, Self::Truthy) => Self::Truthy,
            (Self::Falsy, Self::Falsy) => Self::Falsy,
            (Self::Top(left), Self::Top(right)) => Self::Top(join_top_reasons(*left, *right)),
            (Self::Top(reason), _) | (_, Self::Top(reason)) => Self::Top(*reason),
            (Self::Bottom, value) | (value, Self::Bottom) => *value,
            (Self::Truthy, Self::Falsy) | (Self::Falsy, Self::Truthy) => Self::Maybe,
            (Self::Maybe, _) | (_, Self::Maybe) => Self::Maybe,
        }
    }

    fn stable_digest_parts(&self) -> Vec<String> {
        vec![match self {
            Self::Bottom => "truthiness=bottom".to_string(),
            Self::Truthy => "truthiness=truthy".to_string(),
            Self::Falsy => "truthiness=falsy".to_string(),
            Self::Maybe => "truthiness=maybe".to_string(),
            Self::Top(reason) => top_digest_part("truthiness", *reason),
        }]
    }
}

impl AbstractDomain for ConstantDomain {
    const ID: &'static str = "polint.domain.constants";
    const VERSION: u32 = 1;

    fn bottom() -> Self {
        Self::Bottom
    }

    fn top(reason: TopReason) -> Self {
        Self::Top(reason)
    }

    fn is_bottom(&self) -> bool {
        matches!(self, Self::Bottom)
    }

    fn is_top(&self) -> bool {
        matches!(self, Self::Top(_))
    }

    fn leq(&self, other: &Self) -> bool {
        match (self, other) {
            (left, right) if left == right => true,
            (Self::Top(left), Self::Top(right)) => top_reason_leq(*left, *right),
            (Self::Bottom, _) => true,
            (_, Self::Top(_)) if !self.is_top() => true,
            (Self::Values(left), Self::Values(right)) => left.is_subset(right),
            _ => false,
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (left, right) if left == right => left.clone(),
            (Self::Top(left), Self::Top(right)) => Self::Top(join_top_reasons(*left, *right)),
            (Self::Top(reason), _) | (_, Self::Top(reason)) => Self::Top(*reason),
            (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
            (Self::Values(left), Self::Values(right)) => capped_constant_union(left, right),
        }
    }

    fn widen(&self, next: &Self, _site: WidenSite, fuel: WidenFuel) -> Self {
        if literal_count(next) > LITERAL_SET_CAP || (fuel.remaining == 0 && self != next) {
            Self::Top(TopReason::Widened)
        } else {
            self.join(next)
        }
    }

    fn stable_digest_parts(&self) -> Vec<String> {
        match self {
            Self::Bottom => vec!["kind=bottom".to_string()],
            Self::Values(values) => sorted_digest_parts(
                std::iter::once("kind=values".to_string())
                    .chain(values.iter().map(ConstantLiteral::digest_part)),
            ),
            Self::Top(reason) => vec![top_digest_part("constants", *reason)],
        }
    }
}

impl AbstractDomain for StringDomain {
    const ID: &'static str = "polint.domain.strings";
    const VERSION: u32 = 1;

    fn bottom() -> Self {
        Self::Bottom
    }

    fn top(reason: TopReason) -> Self {
        Self::Top(reason)
    }

    fn is_bottom(&self) -> bool {
        matches!(self, Self::Bottom)
    }

    fn is_top(&self) -> bool {
        matches!(self, Self::Top(_))
    }

    fn leq(&self, other: &Self) -> bool {
        match (self, other) {
            (left, right) if left == right => true,
            (Self::Top(left), Self::Top(right)) => top_reason_leq(*left, *right),
            (Self::Bottom, _) => true,
            (_, Self::Top(_)) if !self.is_top() => true,
            (Self::Values(left), Self::Values(right)) => left.is_subset(right),
            _ => false,
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (left, right) if left == right => left.clone(),
            (Self::Top(left), Self::Top(right)) => Self::Top(join_top_reasons(*left, *right)),
            (Self::Top(reason), _) | (_, Self::Top(reason)) => Self::Top(*reason),
            (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
            (Self::Values(left), Self::Values(right)) => capped_string_union(left, right),
        }
    }

    fn widen(&self, next: &Self, _site: WidenSite, fuel: WidenFuel) -> Self {
        if literal_count(next) > LITERAL_SET_CAP || (fuel.remaining == 0 && self != next) {
            Self::Top(TopReason::Widened)
        } else {
            self.join(next)
        }
    }

    fn stable_digest_parts(&self) -> Vec<String> {
        match self {
            Self::Bottom => vec!["kind=bottom".to_string()],
            Self::Values(values) => sorted_digest_parts(
                std::iter::once("kind=values".to_string())
                    .chain(values.iter().map(|value| format!("string:{value}"))),
            ),
            Self::Top(reason) => vec![top_digest_part("strings", *reason)],
        }
    }
}

impl AbstractDomain for InitializednessDomain {
    const ID: &'static str = "polint.domain.initializedness";
    const VERSION: u32 = 1;

    fn bottom() -> Self {
        Self::Bottom
    }

    fn top(reason: TopReason) -> Self {
        Self::Top(reason)
    }

    fn is_bottom(&self) -> bool {
        matches!(self, Self::Bottom)
    }

    fn is_top(&self) -> bool {
        matches!(self, Self::Top(_))
    }

    fn leq(&self, other: &Self) -> bool {
        match (self, other) {
            (left, right) if left == right => true,
            (Self::Top(left), Self::Top(right)) => top_reason_leq(*left, *right),
            (Self::Bottom, _) => true,
            (_, Self::Top(_)) if !self.is_top() => true,
            (Self::Initialized | Self::Uninitialized, Self::MaybeUninitialized) => true,
            _ => false,
        }
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (left, right) if left == right => *left,
            (Self::Initialized, Self::Initialized) => Self::Initialized,
            (Self::Uninitialized, Self::Uninitialized) => Self::Uninitialized,
            (Self::Top(left), Self::Top(right)) => Self::Top(join_top_reasons(*left, *right)),
            (Self::Top(reason), _) | (_, Self::Top(reason)) => Self::Top(*reason),
            (Self::Bottom, value) | (value, Self::Bottom) => *value,
            (Self::Initialized, Self::Uninitialized) | (Self::Uninitialized, Self::Initialized) => {
                Self::MaybeUninitialized
            }
            (Self::MaybeUninitialized, _) | (_, Self::MaybeUninitialized) => {
                Self::MaybeUninitialized
            }
        }
    }

    fn stable_digest_parts(&self) -> Vec<String> {
        vec![match self {
            Self::Bottom => "initializedness=bottom".to_string(),
            Self::Initialized => "initializedness=initialized".to_string(),
            Self::Uninitialized => "initializedness=uninitialized".to_string(),
            Self::MaybeUninitialized => "initializedness=maybe_uninitialized".to_string(),
            Self::Top(reason) => top_digest_part("initializedness", *reason),
        }]
    }
}

impl ConstantLiteral {
    fn digest_part(&self) -> String {
        match self {
            Self::Bool(value) => format!("constant:bool:{value}"),
            Self::Null => "constant:null".to_string(),
            Self::Undefined => "constant:undefined".to_string(),
            Self::Nil => "constant:nil".to_string(),
            Self::String(value) => format!("constant:string:{value}"),
            Self::Number(value) => format!("constant:number:{value}"),
            Self::Unknown(value) => format!("constant:unknown:{value}"),
        }
    }
}

fn join_chain<T>(left: T, right: T, order: &[T]) -> T
where
    T: Copy + Eq,
{
    if left == right {
        return left;
    }
    order
        .iter()
        .copied()
        .rev()
        .find(|candidate| *candidate == left || *candidate == right)
        .unwrap_or(right)
}

fn capped_constant_union(
    left: &BTreeSet<ConstantLiteral>,
    right: &BTreeSet<ConstantLiteral>,
) -> ConstantDomain {
    let values = left.union(right).cloned().collect::<BTreeSet<_>>();
    if values.len() > LITERAL_SET_CAP {
        ConstantDomain::Top(TopReason::BudgetExceeded)
    } else {
        ConstantDomain::Values(values)
    }
}

fn capped_string_union(left: &BTreeSet<String>, right: &BTreeSet<String>) -> StringDomain {
    let values = left.union(right).cloned().collect::<BTreeSet<_>>();
    if values.len() > LITERAL_SET_CAP {
        StringDomain::Top(TopReason::BudgetExceeded)
    } else {
        StringDomain::Values(values)
    }
}

fn literal_count<T>(domain: &T) -> usize
where
    T: LiteralCount,
{
    domain.literal_count()
}

trait LiteralCount {
    fn literal_count(&self) -> usize;
}

impl LiteralCount for ConstantDomain {
    fn literal_count(&self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Bottom | Self::Top(_) => 0,
        }
    }
}

impl LiteralCount for StringDomain {
    fn literal_count(&self) -> usize {
        match self {
            Self::Values(values) => values.len(),
            Self::Bottom | Self::Top(_) => 0,
        }
    }
}

fn top_digest_part(domain: &str, reason: TopReason) -> String {
    format!("{domain}=top:{}", reason.as_str())
}

fn join_top_reasons(left: TopReason, right: TopReason) -> TopReason {
    if left == right {
        left
    } else {
        TopReason::ConflictingFacts
    }
}

fn top_reason_leq(left: TopReason, right: TopReason) -> bool {
    left == right || right == TopReason::ConflictingFacts
}

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

    #[test]
    fn top_reason_joins_are_commutative_and_upper_bounds() {
        let setup_missing = NilnessDomain::top(TopReason::SetupMissing);
        let unsupported = NilnessDomain::top(TopReason::UnsupportedSemantic);
        let joined = NilnessDomain::top(TopReason::ConflictingFacts);

        assert_eq!(setup_missing.join(&unsupported), joined);
        assert_eq!(unsupported.join(&setup_missing), joined);
        assert!(setup_missing.leq(&joined));
        assert!(unsupported.leq(&joined));
    }
}
