use std::ops::{BitOr, BitOrAssign};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
pub(crate) enum SummaryTopReason {
    UnresolvedCallee,
    UnsupportedSemantic,
    DynamicWrite,
    SetupMissing,
    BudgetExceeded,
    MissingDependency,
    ConflictingFacts,
}

impl SummaryTopReason {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::UnresolvedCallee => "unresolved_callee",
            Self::UnsupportedSemantic => "unsupported_semantic",
            Self::DynamicWrite => "dynamic_write",
            Self::SetupMissing => "setup_missing",
            Self::BudgetExceeded => "budget_exceeded",
            Self::MissingDependency => "missing_dependency",
            Self::ConflictingFacts => "conflicting_facts",
        }
    }
}

pub(crate) trait SummaryDomain: Clone + Send + Sync + Eq + 'static {
    const ID: &'static str;
    const VERSION: u32;

    fn bottom() -> Self;
    fn unknown_top(reason: SummaryTopReason) -> Self;
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

    fn stable_digest_parts(&self) -> Vec<String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum SampleSummary {
        Bottom,
        Value(u32),
        Top(SummaryTopReason),
    }

    impl SummaryDomain for SampleSummary {
        const ID: &'static str = "test.sample_summary";
        const VERSION: u32 = 1;

        fn bottom() -> Self {
            Self::Bottom
        }

        fn unknown_top(reason: SummaryTopReason) -> Self {
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
                (Self::Bottom, _) => true,
                (_, Self::Top(_)) if !self.is_top() => true,
                (Self::Top(left), Self::Top(right)) => left == right,
                (Self::Value(left), Self::Value(right)) => left == right,
                _ => false,
            }
        }

        fn join(&self, other: &Self) -> Self {
            match (self, other) {
                (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
                (Self::Value(left), Self::Value(right)) if left == right => self.clone(),
                (Self::Top(left), Self::Top(right)) if left == right => self.clone(),
                (Self::Top(reason), _) | (_, Self::Top(reason)) => Self::Top(*reason),
                _ => Self::Top(SummaryTopReason::ConflictingFacts),
            }
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
    fn bottom_leq_everything() {
        let bottom = SampleSummary::bottom();
        let value = SampleSummary::Value(42);
        let top = SampleSummary::unknown_top(SummaryTopReason::UnresolvedCallee);

        assert!(bottom.leq(&bottom));
        assert!(bottom.leq(&value));
        assert!(bottom.leq(&top));
    }

    #[test]
    fn top_geq_everything() {
        let bottom = SampleSummary::bottom();
        let value = SampleSummary::Value(42);
        let top = SampleSummary::unknown_top(SummaryTopReason::UnresolvedCallee);

        assert!(bottom.leq(&top));
        assert!(value.leq(&top));
        assert!(top.leq(&top));
    }

    #[test]
    fn join_is_commutative() {
        let a = SampleSummary::Value(1);
        let b = SampleSummary::Value(2);

        assert_eq!(a.join(&b), b.join(&a));
    }

    #[test]
    fn join_is_idempotent() {
        let a = SampleSummary::Value(1);
        let top = SampleSummary::unknown_top(SummaryTopReason::SetupMissing);

        assert_eq!(a.join(&a), a);
        assert_eq!(top.join(&top), top);
    }

    #[test]
    fn join_into_returns_changed_no_when_result_unchanged() {
        let mut state = SampleSummary::Value(1);
        assert_eq!(state.join_into(&SampleSummary::Value(1)), Changed::No);
        assert_eq!(state, SampleSummary::Value(1));

        assert_eq!(state.join_into(&SampleSummary::Value(2)), Changed::Yes);
        assert!(state.is_top());
    }

    #[test]
    fn bottom_join_identity() {
        let value = SampleSummary::Value(5);
        let bottom = SampleSummary::bottom();

        assert_eq!(bottom.join(&value), value);
        assert_eq!(value.join(&bottom), value);
    }

    #[test]
    fn summary_top_reason_as_str_round_trips() {
        let reasons = [
            SummaryTopReason::UnresolvedCallee,
            SummaryTopReason::UnsupportedSemantic,
            SummaryTopReason::DynamicWrite,
            SummaryTopReason::SetupMissing,
            SummaryTopReason::BudgetExceeded,
            SummaryTopReason::MissingDependency,
            SummaryTopReason::ConflictingFacts,
        ];

        for reason in &reasons {
            let s = reason.as_str();
            assert!(!s.is_empty(), "as_str must be non-empty for {reason:?}");
        }

        assert_eq!(reasons.len(), 7);
    }

    #[test]
    fn stable_digest_preserves_top_reasons() {
        let top_a = SampleSummary::unknown_top(SummaryTopReason::UnresolvedCallee);
        let top_b = SampleSummary::unknown_top(SummaryTopReason::SetupMissing);

        assert_eq!(top_a.stable_digest_parts(), top_a.stable_digest_parts());
        assert_ne!(top_a.stable_digest_parts(), top_b.stable_digest_parts());
    }
}
