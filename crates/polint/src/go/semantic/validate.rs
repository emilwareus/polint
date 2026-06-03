use std::collections::BTreeSet;
use std::path::Path;

use crate::analysis::error::AnalysisError;
use crate::go::semantic::facts::{
    GoSemanticAddressTakenFact, GoSemanticCallsiteFact, GoSemanticDynamicDispatchFact,
    GoSemanticFunctionFact, GoSemanticInstantiatedTypeFact, GoSemanticMethodSetFact,
    GoSemanticPackageFact,
};
use crate::go::semantic::store::{GO_SEMANTIC_PROVIDER_ID, GoSemanticFactsOutput};

pub(crate) fn validate_go_semantic_output(
    output: &GoSemanticFactsOutput,
) -> Result<(), AnalysisError> {
    validate_unique(
        "package",
        output.packages.iter().map(|fact| fact.stable_key.as_str()),
    )?;
    validate_unique(
        "function",
        output.functions.iter().map(|fact| fact.stable_key.as_str()),
    )?;
    validate_unique(
        "callsite",
        output.callsites.iter().map(|fact| fact.stable_key.as_str()),
    )?;
    validate_unique(
        "method_set",
        output
            .method_sets
            .iter()
            .map(|fact| fact.stable_key.as_str()),
    )?;
    validate_unique(
        "address_taken",
        output
            .address_taken
            .iter()
            .map(|fact| fact.stable_key.as_str()),
    )?;
    validate_unique(
        "instantiated_type",
        output
            .instantiated_types
            .iter()
            .map(|fact| fact.stable_key.as_str()),
    )?;
    validate_unique(
        "dynamic_dispatch",
        output
            .dynamic_dispatch
            .iter()
            .map(|fact| fact.stable_key.as_str()),
    )?;
    validate_unique(
        "package_error",
        output
            .package_errors
            .iter()
            .map(|fact| fact.stable_key.as_str()),
    )?;

    for package in &output.packages {
        validate_package_paths(package)?;
    }
    for function in &output.functions {
        validate_function_path(function)?;
    }
    for callsite in &output.callsites {
        validate_callsite_path(callsite)?;
    }
    for method_set in &output.method_sets {
        if let Some(reason) = method_set_rejection(method_set) {
            return Err(invalid_fact(reason));
        }
    }
    for address_taken in &output.address_taken {
        if let Some(reason) = address_taken_rejection(address_taken) {
            return Err(invalid_fact(reason));
        }
    }
    for instantiated_type in &output.instantiated_types {
        if let Some(reason) = instantiated_type_rejection(instantiated_type) {
            return Err(invalid_fact(reason));
        }
    }
    for dynamic_dispatch in &output.dynamic_dispatch {
        if let Some(reason) = dynamic_dispatch_rejection(dynamic_dispatch) {
            return Err(invalid_fact(reason));
        }
    }
    for package_error in &output.package_errors {
        reject_empty_stable_key("package_error", &package_error.stable_key)?;
    }
    Ok(())
}

/// Row-resilience boundary (FINDING B/C): the RTA-signal HARVEST families
/// (`method_set` / `address_taken` / `instantiated_type` / `dynamic_dispatch`) are
/// whole-reachable-program SET facts. A single malformed harvest row must NOT nuke the
/// entire Go fact set (which would zero RTA repo-wide) — it is DROPPED at the store
/// boundary ([`GoSemanticFactsOutput::drop_invalid_harvest_rows`]) before validation. The
/// rejection PREDICATES below are the single source of truth shared by that drop filter
/// and by [`validate_go_semantic_output`] (a post-drop defense-in-depth assertion): each
/// returns `Some(reason)` for a row that must not be stored, `None` for a valid one. The
/// structural families (packages/functions/callsites) are NOT row-resilient — a duplicate
/// there is a genuine declaration conflict the validator must still reject.
///
/// A `method_set` row's discriminating identity is its `type_name`, which is ABSENT from
/// the lowering fallback stable-key recipe — so a missing stable_key would collide distinct
/// types and the set-dedup would silently drop a real member (WR-03). It is dropped rather
/// than fabricated.
pub(crate) fn method_set_rejection(fact: &GoSemanticMethodSetFact) -> Option<String> {
    if fact.stable_key.is_empty() {
        return Some("Go semantic method_set row is missing a stable_key".to_string());
    }
    None
}

/// Identity guard (WR-02): the address-taken function identity is the row's discriminating
/// field (`inputs.rs` keys the address-taken set on it). An empty `function`, or a missing
/// stable_key, would seed a bogus candidate keyed on the empty string — dropped.
pub(crate) fn address_taken_rejection(fact: &GoSemanticAddressTakenFact) -> Option<String> {
    if fact.stable_key.is_empty() {
        return Some("Go semantic address_taken row is missing a stable_key".to_string());
    }
    if fact.function.is_empty() {
        return Some(format!(
            "Go semantic address_taken `{}` has an empty function identity",
            fact.stable_key
        ));
    }
    None
}

/// Identity guard (WR-02): the instantiated `type_name` is the row's discriminating field.
/// An empty `type_name` normalizes to "" and could spuriously intersect a method-set keyed
/// on an empty type; a missing stable_key collides distinct types — both dropped.
pub(crate) fn instantiated_type_rejection(fact: &GoSemanticInstantiatedTypeFact) -> Option<String> {
    if fact.stable_key.is_empty() {
        return Some("Go semantic instantiated_type row is missing a stable_key".to_string());
    }
    if fact.type_name.is_empty() {
        return Some(format!(
            "Go semantic instantiated_type `{}` has an empty type_name",
            fact.stable_key
        ));
    }
    None
}

/// Honest-discriminant guard (D-15): a dynamic-dispatch detail row must carry a
/// discriminant Plan 2 can match on — either an interface invoke (`interface_type` +
/// `method`) or a func-value signature — and must join back to its callsite via a
/// non-empty `callsite_stable_key`. A row with neither discriminant (or a missing
/// stable_key / callsite key) is dropped rather than stored as a useless identity.
pub(crate) fn dynamic_dispatch_rejection(fact: &GoSemanticDynamicDispatchFact) -> Option<String> {
    if fact.stable_key.is_empty() {
        return Some("Go semantic dynamic_dispatch row is missing a stable_key".to_string());
    }
    if fact.callsite_stable_key.is_empty() {
        return Some(format!(
            "Go semantic dynamic_dispatch `{}` has an empty callsite_stable_key",
            fact.stable_key
        ));
    }
    let has_invoke = fact.interface_type.is_some() && fact.method.is_some();
    let has_signature = fact.signature.is_some();
    if !has_invoke && !has_signature {
        return Some(format!(
            "Go semantic dynamic_dispatch `{}` carries no dispatch discriminant (need interface_type+method or signature)",
            fact.stable_key
        ));
    }
    None
}

fn validate_unique<'a>(
    family: &str,
    stable_keys: impl Iterator<Item = &'a str>,
) -> Result<(), AnalysisError> {
    let mut seen = BTreeSet::new();
    for stable_key in stable_keys {
        reject_empty_stable_key(family, stable_key)?;
        if !seen.insert(stable_key) {
            return Err(invalid_fact(format!(
                "duplicate Go semantic {family} stable key `{stable_key}`"
            )));
        }
    }
    Ok(())
}

fn validate_package_paths(package: &GoSemanticPackageFact) -> Result<(), AnalysisError> {
    reject_empty_stable_key("package", &package.stable_key)?;
    for file in &package.files {
        validate_relative_path(file)?;
    }
    Ok(())
}

fn validate_function_path(function: &GoSemanticFunctionFact) -> Result<(), AnalysisError> {
    reject_empty_stable_key("function", &function.stable_key)?;
    if let Some(path) = &function.relative_file {
        validate_relative_path(path)?;
    }
    Ok(())
}

fn validate_callsite_path(callsite: &GoSemanticCallsiteFact) -> Result<(), AnalysisError> {
    reject_empty_stable_key("callsite", &callsite.stable_key)?;
    if let Some(path) = &callsite.relative_file {
        validate_relative_path(path)?;
    }
    Ok(())
}

fn reject_empty_stable_key(family: &str, stable_key: &str) -> Result<(), AnalysisError> {
    if stable_key.is_empty() {
        return Err(invalid_fact(format!(
            "empty Go semantic {family} stable key"
        )));
    }
    Ok(())
}

pub(crate) fn validate_relative_path(path: &str) -> Result<(), AnalysisError> {
    let candidate = Path::new(path);
    if candidate.is_absolute() || path == ".." || path.starts_with("../") || path.contains("/../") {
        return Err(invalid_fact(format!(
            "Go semantic sidecar file path `{path}` escapes repository"
        )));
    }
    Ok(())
}

fn invalid_fact(reason: String) -> AnalysisError {
    AnalysisError::InvalidFact {
        provider: GO_SEMANTIC_PROVIDER_ID,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::go::semantic::facts::{
        GoSemanticAddressTakenFact, GoSemanticAddressTakenId, GoSemanticCallStatus,
        GoSemanticCallsiteId, GoSemanticFunctionId, GoSemanticFunctionKind,
        GoSemanticInstantiatedTypeFact, GoSemanticInstantiatedTypeId,
    };

    #[test]
    fn validate_rejects_address_taken_with_empty_function() {
        // WR-02: the address-taken function identity is the row's discriminating
        // field; an empty one is rejected (mirrors the dynamic-dispatch guard), not
        // stored as a bogus empty-keyed candidate.
        let output = GoSemanticFactsOutput {
            address_taken: vec![GoSemanticAddressTakenFact {
                id: GoSemanticAddressTakenId(0),
                stable_key: "at".to_string(),
                package_id: "pkg".to_string(),
                package_path: "example.com/pkg".to_string(),
                function: String::new(),
            }],
            ..GoSemanticFactsOutput::default()
        };
        let err = validate_go_semantic_output(&output).unwrap_err();
        assert!(err.to_string().contains("empty function identity"), "{err}");
    }

    #[test]
    fn validate_rejects_instantiated_type_with_empty_type_name() {
        // WR-02: the instantiated type_name is the row's discriminating field; an empty
        // one would normalize to "" and could spuriously intersect an empty-keyed
        // method-set, so it is rejected.
        let output = GoSemanticFactsOutput {
            instantiated_types: vec![GoSemanticInstantiatedTypeFact {
                id: GoSemanticInstantiatedTypeId(0),
                stable_key: "it".to_string(),
                package_id: "pkg".to_string(),
                package_path: "example.com/pkg".to_string(),
                type_name: String::new(),
            }],
            ..GoSemanticFactsOutput::default()
        };
        let err = validate_go_semantic_output(&output).unwrap_err();
        assert!(err.to_string().contains("empty type_name"), "{err}");
    }

    #[test]
    fn validate_accepts_address_taken_and_instantiated_type_with_identity() {
        // The happy path: non-empty identity fields validate cleanly.
        let output = GoSemanticFactsOutput {
            address_taken: vec![GoSemanticAddressTakenFact {
                id: GoSemanticAddressTakenId(0),
                stable_key: "at".to_string(),
                package_id: "pkg".to_string(),
                package_path: "example.com/pkg".to_string(),
                function: "example.com/pkg.F".to_string(),
            }],
            instantiated_types: vec![GoSemanticInstantiatedTypeFact {
                id: GoSemanticInstantiatedTypeId(0),
                stable_key: "it".to_string(),
                package_id: "pkg".to_string(),
                package_path: "example.com/pkg".to_string(),
                type_name: "example.com/pkg.T".to_string(),
            }],
            ..GoSemanticFactsOutput::default()
        };
        assert!(validate_go_semantic_output(&output).is_ok());
    }

    #[test]
    fn validate_rejects_duplicate_stable_keys() {
        let output = GoSemanticFactsOutput {
            functions: vec![function("same", None), function("same", None)],
            ..GoSemanticFactsOutput::default()
        };
        let err = validate_go_semantic_output(&output).unwrap_err();
        assert!(err.to_string().contains("duplicate Go semantic function"));
    }

    #[test]
    fn validate_rejects_repo_escaping_paths() {
        let output = GoSemanticFactsOutput {
            callsites: vec![GoSemanticCallsiteFact {
                id: GoSemanticCallsiteId(0),
                stable_key: "call".to_string(),
                package_id: "pkg".to_string(),
                package_path: "example.com/pkg".to_string(),
                caller: "caller".to_string(),
                static_callee: None,
                status: GoSemanticCallStatus::UnresolvedDynamic,
                reason: None,
                relative_file: Some("../outside.go".to_string()),
                file: None,
                span: None,
            }],
            ..GoSemanticFactsOutput::default()
        };
        let err = validate_go_semantic_output(&output).unwrap_err();
        assert!(err.to_string().contains("escapes repository"));
    }

    #[test]
    fn validate_rejects_dynamic_dispatch_without_discriminant() {
        let output = GoSemanticFactsOutput {
            dynamic_dispatch: vec![GoSemanticDynamicDispatchFact {
                id: crate::go::semantic::facts::GoSemanticDynamicDispatchId(0),
                stable_key: "dd".to_string(),
                package_id: "pkg".to_string(),
                package_path: "example.com/pkg".to_string(),
                caller: "example.com/pkg.caller".to_string(),
                callsite_stable_key: "cs".to_string(),
                interface_type: None,
                method: None,
                signature: None,
            }],
            ..GoSemanticFactsOutput::default()
        };
        let err = validate_go_semantic_output(&output).unwrap_err();
        assert!(err.to_string().contains("no dispatch discriminant"));
    }

    #[test]
    fn validate_rejects_dynamic_dispatch_with_empty_callsite_key() {
        let output = GoSemanticFactsOutput {
            dynamic_dispatch: vec![GoSemanticDynamicDispatchFact {
                id: crate::go::semantic::facts::GoSemanticDynamicDispatchId(0),
                stable_key: "dd".to_string(),
                package_id: "pkg".to_string(),
                package_path: "example.com/pkg".to_string(),
                caller: "example.com/pkg.caller".to_string(),
                callsite_stable_key: String::new(),
                interface_type: Some("example.com/pkg.I".to_string()),
                method: Some("M".to_string()),
                signature: None,
            }],
            ..GoSemanticFactsOutput::default()
        };
        let err = validate_go_semantic_output(&output).unwrap_err();
        assert!(err.to_string().contains("empty callsite_stable_key"));
    }

    #[test]
    fn validate_accepts_dynamic_dispatch_with_invoke_discriminant() {
        let output = GoSemanticFactsOutput {
            dynamic_dispatch: vec![GoSemanticDynamicDispatchFact {
                id: crate::go::semantic::facts::GoSemanticDynamicDispatchId(0),
                stable_key: "dd".to_string(),
                package_id: "pkg".to_string(),
                package_path: "example.com/pkg".to_string(),
                caller: "example.com/pkg.caller".to_string(),
                callsite_stable_key: "cs".to_string(),
                interface_type: Some("example.com/pkg.I".to_string()),
                method: Some("M".to_string()),
                signature: None,
            }],
            ..GoSemanticFactsOutput::default()
        };
        assert!(validate_go_semantic_output(&output).is_ok());
    }

    fn function(stable_key: &str, relative_file: Option<&str>) -> GoSemanticFunctionFact {
        GoSemanticFunctionFact {
            id: GoSemanticFunctionId(0),
            stable_key: stable_key.to_string(),
            package_id: "pkg".to_string(),
            package_path: "example.com/pkg".to_string(),
            name: "F".to_string(),
            qualified: "example.com/pkg.F".to_string(),
            signature: "()".to_string(),
            kind: GoSemanticFunctionKind::Function,
            receiver: None,
            relative_file: relative_file.map(str::to_string),
            file: None,
            span: None,
        }
    }
}
