use std::collections::BTreeSet;
use std::ops::{BitOr, BitOrAssign};

use crate::core::StableKeyId;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Changed {
    Yes,
    No,
}

impl Changed {
    pub(crate) fn is_yes(self) -> bool {
        matches!(self, Self::Yes)
    }
}

impl BitOr for Changed {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        if self.is_yes() || rhs.is_yes() {
            Self::Yes
        } else {
            Self::No
        }
    }
}

impl BitOrAssign for Changed {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum TopReason {
    UnknownValue,
    UnsupportedSemantic,
    DynamicWrite,
    UnresolvedCall,
    SetupMissing,
    BudgetExceeded,
    Widened,
    ConflictingFacts,
}

impl TopReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::UnknownValue => "unknown_value",
            Self::UnsupportedSemantic => "unsupported_semantic",
            Self::DynamicWrite => "dynamic_write",
            Self::UnresolvedCall => "unresolved_call",
            Self::SetupMissing => "setup_missing",
            Self::BudgetExceeded => "budget_exceeded",
            Self::Widened => "widened",
            Self::ConflictingFacts => "conflicting_facts",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct WidenSite {
    pub(crate) stable_key: StableKeyId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct WidenFuel {
    pub(crate) remaining: u32,
}

pub(crate) trait AbstractDomain: Clone + Send + Sync + Eq + 'static {
    const ID: &'static str;
    const VERSION: u32;

    fn bottom() -> Self;
    fn top(reason: TopReason) -> Self;
    fn is_bottom(&self) -> bool;
    fn is_top(&self) -> bool;
    fn leq(&self, other: &Self) -> bool;
    fn join(&self, other: &Self) -> Self;

    fn join_into(&mut self, incoming: &Self) -> Changed {
        let joined = self.join(incoming);
        if joined == *self {
            Changed::No
        } else {
            *self = joined;
            Changed::Yes
        }
    }

    fn widen(&self, next: &Self, _site: WidenSite, _fuel: WidenFuel) -> Self {
        self.join(next)
    }

    fn stable_digest_parts(&self) -> Vec<String>;
}

pub(crate) fn sorted_digest_parts(parts: impl IntoIterator<Item = String>) -> Vec<String> {
    BTreeSet::from_iter(parts).into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum SampleDomain {
        Bottom,
        Value(&'static str),
        Top(TopReason),
    }

    impl AbstractDomain for SampleDomain {
        const ID: &'static str = "test.sample";
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
                (Self::Top(left), Self::Top(right)) => left == right,
                (Self::Bottom, _) => true,
                (_, Self::Top(_)) => true,
                (Self::Value(left), Self::Value(right)) => left == right,
                _ => false,
            }
        }

        fn join(&self, other: &Self) -> Self {
            match (self, other) {
                (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
                (Self::Value(left), Self::Value(right)) if left == right => self.clone(),
                (Self::Top(left), Self::Top(right)) if left == right => self.clone(),
                _ => Self::Top(TopReason::ConflictingFacts),
            }
        }

        fn widen(&self, next: &Self, _site: WidenSite, _fuel: WidenFuel) -> Self {
            self.join(next)
        }

        fn stable_digest_parts(&self) -> Vec<String> {
            match self {
                Self::Bottom => vec!["kind=bottom".to_string()],
                Self::Value(value) => vec![format!("kind=value:{value}")],
                Self::Top(reason) => vec![format!("kind=top:{}", reason.as_str())],
            }
        }
    }

    #[test]
    fn join_into_changes_only_when_canonical_state_changes() {
        let mut state = SampleDomain::Value("a");

        assert_eq!(state.join_into(&SampleDomain::Value("a")), Changed::No);
        assert_eq!(state, SampleDomain::Value("a"));

        assert_eq!(state.join_into(&SampleDomain::Value("b")), Changed::Yes);
        assert_eq!(state, SampleDomain::Top(TopReason::ConflictingFacts));
    }

    #[test]
    fn stable_digest_parts_are_deterministic_and_preserve_top_reasons() {
        assert_eq!(
            SampleDomain::top(TopReason::UnsupportedSemantic).stable_digest_parts(),
            SampleDomain::top(TopReason::UnsupportedSemantic).stable_digest_parts()
        );
        assert_ne!(
            SampleDomain::top(TopReason::UnsupportedSemantic).stable_digest_parts(),
            SampleDomain::top(TopReason::SetupMissing).stable_digest_parts()
        );
    }
}
