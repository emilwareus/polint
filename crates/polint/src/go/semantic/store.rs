use crate::analysis::error::AnalysisError;
use crate::go::semantic::facts::{
    GoSemanticAddressTakenFact, GoSemanticCallsiteFact, GoSemanticDynamicDispatchFact,
    GoSemanticFunctionFact, GoSemanticInstantiatedTypeFact, GoSemanticMethodSetFact,
    GoSemanticPackageErrorFact, GoSemanticPackageFact,
};
use crate::go::semantic::validate::validate_go_semantic_output;

pub(crate) const GO_SEMANTIC_PROVIDER_ID: &str = "polint.go.semantic";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GoSemanticFactsOutput {
    pub(crate) packages: Vec<GoSemanticPackageFact>,
    pub(crate) functions: Vec<GoSemanticFunctionFact>,
    pub(crate) callsites: Vec<GoSemanticCallsiteFact>,
    pub(crate) method_sets: Vec<GoSemanticMethodSetFact>,
    pub(crate) address_taken: Vec<GoSemanticAddressTakenFact>,
    pub(crate) instantiated_types: Vec<GoSemanticInstantiatedTypeFact>,
    pub(crate) dynamic_dispatch: Vec<GoSemanticDynamicDispatchFact>,
    pub(crate) package_errors: Vec<GoSemanticPackageErrorFact>,
}

impl GoSemanticFactsOutput {
    pub(crate) fn normalized(mut self) -> Self {
        self.packages
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        for (index, fact) in self.packages.iter_mut().enumerate() {
            fact.id = crate::go::semantic::facts::GoSemanticPackageId(index as u64);
        }

        self.functions
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        for (index, fact) in self.functions.iter_mut().enumerate() {
            fact.id = crate::go::semantic::facts::GoSemanticFunctionId(index as u64);
        }

        // Callsites, address-taken funcs, instantiated runtime types, and dynamic-dispatch
        // detail are whole-reachable-program harvests over the SSA program: the SAME
        // official identity legitimately recurs (a function whose address is taken from
        // two call sites; a type converted to an interface in two functions; compiler
        // synthetic method-wrappers whose internal calls carry no source position). These
        // are SET facts keyed by official identity, so an identity-duplicate row is the
        // SAME set member, not a conflict — dedup by stable key (keep first) after the
        // stable-key sort, mirroring the solver fixpoint's stable-key dedup. This keeps
        // the whole-program set honest and `validate_unique` green without dropping any
        // distinct fact (Phase 48 verification surfaced this on same-named interface
        // methods + shared callees). Packages/functions/method-sets are keyed by unique
        // declaration identity and are intentionally NOT deduped (a real duplicate there
        // is a genuine conflict the validator must still reject).
        self.callsites
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.callsites
            .dedup_by(|left, right| left.stable_key == right.stable_key);
        for (index, fact) in self.callsites.iter_mut().enumerate() {
            fact.id = crate::go::semantic::facts::GoSemanticCallsiteId(index as u64);
        }

        self.method_sets
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        for (index, fact) in self.method_sets.iter_mut().enumerate() {
            fact.id = crate::go::semantic::facts::GoSemanticMethodSetId(index as u64);
        }

        self.address_taken
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.address_taken
            .dedup_by(|left, right| left.stable_key == right.stable_key);
        for (index, fact) in self.address_taken.iter_mut().enumerate() {
            fact.id = crate::go::semantic::facts::GoSemanticAddressTakenId(index as u64);
        }

        self.instantiated_types
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.instantiated_types
            .dedup_by(|left, right| left.stable_key == right.stable_key);
        for (index, fact) in self.instantiated_types.iter_mut().enumerate() {
            fact.id = crate::go::semantic::facts::GoSemanticInstantiatedTypeId(index as u64);
        }

        self.dynamic_dispatch
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        self.dynamic_dispatch
            .dedup_by(|left, right| left.stable_key == right.stable_key);
        for (index, fact) in self.dynamic_dispatch.iter_mut().enumerate() {
            fact.id = crate::go::semantic::facts::GoSemanticDynamicDispatchId(index as u64);
        }

        self.package_errors
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        for (index, fact) in self.package_errors.iter_mut().enumerate() {
            fact.id = crate::go::semantic::facts::GoSemanticPackageErrorId(index as u64);
        }
        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GoSemanticStore {
    output: GoSemanticFactsOutput,
}

impl GoSemanticStore {
    pub(crate) fn from_output(output: GoSemanticFactsOutput) -> Result<Self, AnalysisError> {
        let output = output.normalized();
        validate_go_semantic_output(&output)?;
        Ok(Self { output })
    }

    pub(crate) fn output(&self) -> &GoSemanticFactsOutput {
        &self.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::go::semantic::facts::{GoSemanticFunctionId, GoSemanticFunctionKind};

    #[test]
    fn store_normalizes_by_stable_key_before_dense_id_assignment() {
        let output = GoSemanticFactsOutput {
            functions: vec![
                GoSemanticFunctionFact {
                    id: GoSemanticFunctionId(99),
                    stable_key: "b".to_string(),
                    package_id: "pkg".to_string(),
                    package_path: "example.com/pkg".to_string(),
                    name: "B".to_string(),
                    qualified: "example.com/pkg.B".to_string(),
                    signature: "()".to_string(),
                    kind: GoSemanticFunctionKind::Function,
                    receiver: None,
                    relative_file: None,
                    file: None,
                    span: None,
                },
                GoSemanticFunctionFact {
                    id: GoSemanticFunctionId(42),
                    stable_key: "a".to_string(),
                    package_id: "pkg".to_string(),
                    package_path: "example.com/pkg".to_string(),
                    name: "A".to_string(),
                    qualified: "example.com/pkg.A".to_string(),
                    signature: "()".to_string(),
                    kind: GoSemanticFunctionKind::Function,
                    receiver: None,
                    relative_file: None,
                    file: None,
                    span: None,
                },
            ],
            ..GoSemanticFactsOutput::default()
        };

        let store = GoSemanticStore::from_output(output).expect("store validates");
        assert_eq!(store.output.functions[0].stable_key, "a");
        assert_eq!(store.output.functions[0].id, GoSemanticFunctionId(0));
        assert_eq!(store.output.functions[1].stable_key, "b");
        assert_eq!(store.output.functions[1].id, GoSemanticFunctionId(1));
    }

    #[test]
    fn normalized_dedups_identity_duplicate_set_facts_keeping_first() {
        use crate::go::semantic::facts::{GoSemanticAddressTakenFact, GoSemanticAddressTakenId};

        // The whole-reachable-program harvests (address-taken / instantiated / callsite /
        // dynamic-dispatch) legitimately produce the SAME official identity more than once:
        // a function whose address is GENUINELY taken as a value from two distinct sites
        // (e.g. `f := handler` in two places, or `handler` stored into two slices) yields
        // two address-taken rows for the same `ssa.Function.String()`. These are SET facts
        // keyed by official identity, so an identity-duplicate row is the SAME member and
        // must dedup (keep first) — NOT be rejected by `validate_unique`. Phase 48
        // verification surfaced this. (We deliberately use a genuine value-use function, not
        // a statically-called one like `fmt.Println`: FINDING 2 establishes that a
        // statically-called function is NOT address-taken at all.)
        let output = GoSemanticFactsOutput {
            address_taken: vec![
                GoSemanticAddressTakenFact {
                    id: GoSemanticAddressTakenId(7),
                    stable_key: "at|example.com/pkg.handler".to_string(),
                    package_id: "pkg".to_string(),
                    package_path: "example.com/pkg".to_string(),
                    function: "example.com/pkg.handler".to_string(),
                },
                GoSemanticAddressTakenFact {
                    id: GoSemanticAddressTakenId(9),
                    stable_key: "at|example.com/pkg.handler".to_string(),
                    package_id: "pkg".to_string(),
                    package_path: "example.com/pkg".to_string(),
                    function: "example.com/pkg.handler".to_string(),
                },
            ],
            ..GoSemanticFactsOutput::default()
        };

        // Without dedup this would fail `validate_unique`; with dedup it stores one row.
        let store = GoSemanticStore::from_output(output).expect("dedup keeps the set valid");
        assert_eq!(store.output().address_taken.len(), 1);
        assert_eq!(
            store.output().address_taken[0].function,
            "example.com/pkg.handler"
        );
        assert_eq!(
            store.output().address_taken[0].id,
            GoSemanticAddressTakenId(0)
        );
    }
}
