use std::collections::BTreeSet;
use std::path::Path;

use crate::analysis::error::AnalysisError;
use crate::go::semantic::facts::{
    GoSemanticCallsiteFact, GoSemanticFunctionFact, GoSemanticPackageFact,
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
        reject_empty_stable_key("method_set", &method_set.stable_key)?;
    }
    for package_error in &output.package_errors {
        reject_empty_stable_key("package_error", &package_error.stable_key)?;
    }
    Ok(())
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
        GoSemanticCallStatus, GoSemanticCallsiteId, GoSemanticFunctionId, GoSemanticFunctionKind,
    };

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
