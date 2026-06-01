use crate::analysis::error::AnalysisError;
use crate::go::semantic::facts::{
    GoSemanticCallsiteFact, GoSemanticFunctionFact, GoSemanticMethodSetFact,
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

        self.callsites
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        for (index, fact) in self.callsites.iter_mut().enumerate() {
            fact.id = crate::go::semantic::facts::GoSemanticCallsiteId(index as u64);
        }

        self.method_sets
            .sort_by(|left, right| left.stable_key.cmp(&right.stable_key));
        for (index, fact) in self.method_sets.iter_mut().enumerate() {
            fact.id = crate::go::semantic::facts::GoSemanticMethodSetId(index as u64);
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
}
