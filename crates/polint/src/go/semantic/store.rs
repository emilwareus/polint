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

    /// Drop malformed RTA-signal HARVEST rows so a single bad row never nukes the whole Go
    /// fact set (FINDING B/C). The harvest families (`method_set` / `address_taken` /
    /// `instantiated_type` / `dynamic_dispatch`) are whole-reachable-program SET facts: a
    /// discriminant-less / identity-less / stable-key-less row carries no resolvable signal,
    /// so it is dropped (honest — the honest-unresolved contract tolerates a missing dispatch
    /// detail downstream) rather than failing closed and zeroing every function, callsite,
    /// method-set, and root from the VALID rows (which would drive RTA to zero edges
    /// repo-wide). The STRUCTURAL families (packages/functions/callsites) are deliberately
    /// NOT filtered here — a conflict there is a genuine declaration bug
    /// [`validate_go_semantic_output`] must still reject. Uses the same rejection predicates
    /// as the validator, so "valid" means identically in both places. Deterministic: retains
    /// source order of the surviving rows (the caller re-`normalized()`s afterward).
    fn drop_invalid_harvest_rows(mut self) -> Self {
        use crate::go::semantic::validate::{
            address_taken_rejection, dynamic_dispatch_rejection, instantiated_type_rejection,
            method_set_rejection,
        };
        self.method_sets
            .retain(|fact| method_set_rejection(fact).is_none());
        self.address_taken
            .retain(|fact| address_taken_rejection(fact).is_none());
        self.instantiated_types
            .retain(|fact| instantiated_type_rejection(fact).is_none());
        self.dynamic_dispatch
            .retain(|fact| dynamic_dispatch_rejection(fact).is_none());
        self
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct GoSemanticStore {
    output: GoSemanticFactsOutput,
}

impl GoSemanticStore {
    pub(crate) fn from_output(output: GoSemanticFactsOutput) -> Result<Self, AnalysisError> {
        // Drop malformed RTA-signal harvest rows FIRST so a single bad row never zeroes the
        // whole Go fact set (FINDING B/C), then normalize (re-sort + dense-id over the
        // survivors) and validate. The structural families stay strictly validated.
        let output = output.drop_invalid_harvest_rows().normalized();
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
    use crate::go::semantic::facts::{
        GoSemanticAddressTakenFact, GoSemanticAddressTakenId, GoSemanticCallStatus,
        GoSemanticCallsiteFact, GoSemanticCallsiteId, GoSemanticDynamicDispatchFact,
        GoSemanticDynamicDispatchId, GoSemanticFunctionId, GoSemanticFunctionKind,
        GoSemanticInstantiatedTypeFact, GoSemanticInstantiatedTypeId, GoSemanticMethodSetFact,
        GoSemanticMethodSetId,
    };

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

    fn valid_function() -> GoSemanticFunctionFact {
        GoSemanticFunctionFact {
            id: GoSemanticFunctionId(0),
            stable_key: "fn|main".to_string(),
            package_id: "pkg".to_string(),
            package_path: "example.com/pkg".to_string(),
            name: "main".to_string(),
            qualified: "example.com/pkg.main".to_string(),
            signature: "func()".to_string(),
            kind: GoSemanticFunctionKind::Function,
            receiver: None,
            relative_file: None,
            file: None,
            span: None,
        }
    }

    fn valid_callsite() -> GoSemanticCallsiteFact {
        GoSemanticCallsiteFact {
            id: GoSemanticCallsiteId(0),
            stable_key: "cs|main".to_string(),
            package_id: "pkg".to_string(),
            package_path: "example.com/pkg".to_string(),
            caller: "example.com/pkg.main".to_string(),
            static_callee: None,
            status: GoSemanticCallStatus::UnresolvedDynamic,
            reason: None,
            relative_file: None,
            file: None,
            span: None,
        }
    }

    #[test]
    fn from_output_drops_a_single_invalid_harvest_row_and_keeps_valid_facts() {
        // FINDING B: a single malformed RTA-signal harvest row must NOT nuke the entire Go
        // fact set. Before the fix, one rejected harvest row made `from_output` return Err,
        // `replace_go_semantic_facts` failed, and the DB was left with ZERO Go facts — RTA
        // then derived zero edges repo-wide. The harvest families are row-resilient: the
        // offending row is dropped and the valid functions/callsites SURVIVE.
        let output = GoSemanticFactsOutput {
            functions: vec![valid_function()],
            callsites: vec![valid_callsite()],
            // A discriminant-less dynamic_dispatch row (empty interface_type AND empty
            // signature) — the WR-02/WR-03 honest-discriminant guard rejects it. It must be
            // DROPPED, not fatal.
            dynamic_dispatch: vec![GoSemanticDynamicDispatchFact {
                id: GoSemanticDynamicDispatchId(0),
                stable_key: "dd|bad".to_string(),
                package_id: "pkg".to_string(),
                package_path: "example.com/pkg".to_string(),
                caller: "example.com/pkg.main".to_string(),
                callsite_stable_key: "cs|main".to_string(),
                interface_type: None,
                method: None,
                signature: None,
            }],
            ..GoSemanticFactsOutput::default()
        };

        let store = GoSemanticStore::from_output(output)
            .expect("one bad harvest row must not fail the whole output");
        // The valid structural facts survive (NOT zeroed).
        assert_eq!(store.output().functions.len(), 1);
        assert_eq!(
            store.output().functions[0].qualified,
            "example.com/pkg.main"
        );
        assert_eq!(store.output().callsites.len(), 1);
        // The bad harvest row was dropped.
        assert!(store.output().dynamic_dispatch.is_empty());
    }

    #[test]
    fn from_output_drops_invalid_rows_across_all_harvest_families_keeping_valid_ones() {
        // FINDING B/C: every harvest family is row-resilient. A bad row in each family
        // (empty function identity / empty type_name / no discriminant / missing stable_key)
        // is dropped while a VALID sibling in the same family survives.
        let output = GoSemanticFactsOutput {
            functions: vec![valid_function()],
            address_taken: vec![
                GoSemanticAddressTakenFact {
                    id: GoSemanticAddressTakenId(0),
                    stable_key: "at|good".to_string(),
                    package_id: "pkg".to_string(),
                    package_path: "example.com/pkg".to_string(),
                    function: "example.com/pkg.handler".to_string(),
                },
                // Empty function identity → dropped.
                GoSemanticAddressTakenFact {
                    id: GoSemanticAddressTakenId(1),
                    stable_key: "at|bad".to_string(),
                    package_id: "pkg".to_string(),
                    package_path: "example.com/pkg".to_string(),
                    function: String::new(),
                },
            ],
            instantiated_types: vec![
                GoSemanticInstantiatedTypeFact {
                    id: GoSemanticInstantiatedTypeId(0),
                    stable_key: "it|good".to_string(),
                    package_id: "pkg".to_string(),
                    package_path: "example.com/pkg".to_string(),
                    type_name: "example.com/pkg.T".to_string(),
                },
                // Empty type_name → dropped.
                GoSemanticInstantiatedTypeFact {
                    id: GoSemanticInstantiatedTypeId(1),
                    stable_key: "it|bad".to_string(),
                    package_id: "pkg".to_string(),
                    package_path: "example.com/pkg".to_string(),
                    type_name: String::new(),
                },
            ],
            method_sets: vec![
                GoSemanticMethodSetFact {
                    id: GoSemanticMethodSetId(0),
                    stable_key: "ms|good".to_string(),
                    package_id: "pkg".to_string(),
                    package_path: "example.com/pkg".to_string(),
                    type_name: "example.com/pkg.T".to_string(),
                    methods: vec!["M".to_string()],
                },
                // Missing stable_key (the WR-03 / FINDING C condition) → dropped, not fatal.
                GoSemanticMethodSetFact {
                    id: GoSemanticMethodSetId(1),
                    stable_key: String::new(),
                    package_id: "pkg".to_string(),
                    package_path: "example.com/pkg".to_string(),
                    type_name: "example.com/pkg.U".to_string(),
                    methods: vec!["N".to_string()],
                },
            ],
            ..GoSemanticFactsOutput::default()
        };

        let store = GoSemanticStore::from_output(output)
            .expect("bad harvest rows must be dropped, not fatal");
        assert_eq!(store.output().functions.len(), 1);
        assert_eq!(store.output().address_taken.len(), 1);
        assert_eq!(
            store.output().address_taken[0].function,
            "example.com/pkg.handler"
        );
        assert_eq!(store.output().instantiated_types.len(), 1);
        assert_eq!(
            store.output().instantiated_types[0].type_name,
            "example.com/pkg.T"
        );
        // The valid method_set survives; the stable-key-less one is dropped.
        assert_eq!(store.output().method_sets.len(), 1);
        assert_eq!(store.output().method_sets[0].type_name, "example.com/pkg.T");
    }

    #[test]
    fn from_output_still_rejects_duplicate_structural_stable_keys() {
        // Row-resilience is scoped to the HARVEST families. A genuine conflict in a
        // STRUCTURAL family (two functions with the same stable key) is a real bug the
        // validator must STILL reject — never silently drop a structural declaration.
        let mut a = valid_function();
        let mut b = valid_function();
        a.stable_key = "dup".to_string();
        b.stable_key = "dup".to_string();
        let output = GoSemanticFactsOutput {
            functions: vec![a, b],
            ..GoSemanticFactsOutput::default()
        };
        let err = GoSemanticStore::from_output(output).unwrap_err();
        assert!(err.to_string().contains("duplicate Go semantic function"));
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
