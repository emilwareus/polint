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
                (Self::Bottom, _) => true,
                (_, Self::Top(_)) => true,
                (Self::Value(left), Self::Value(right)) => left == right,
                (Self::Top(left), Self::Top(right)) => left == right,
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
                Self::Top(reason) => vec![format!("kind=top:{reason:?}")],
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
