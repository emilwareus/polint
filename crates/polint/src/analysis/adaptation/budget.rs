/// Model-expansion caps for repo-local adaptation facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AdaptationModelBudget {
    pub(crate) max_model_files: usize,
    pub(crate) max_model_facts: usize,
    pub(crate) max_expansions_per_model: usize,
    pub(crate) max_targets_per_source: usize,
    pub(crate) max_model_derived_edges: usize,
}

impl Default for AdaptationModelBudget {
    fn default() -> Self {
        Self {
            max_model_files: 32,
            max_model_facts: 512,
            max_expansions_per_model: 64,
            max_targets_per_source: 16,
            max_model_derived_edges: 2_048,
        }
    }
}

impl AdaptationModelBudget {
    pub(crate) fn digest_parts(self) -> Vec<String> {
        vec![
            format!("budget.adaptation.max_model_files={}", self.max_model_files),
            format!("budget.adaptation.max_model_facts={}", self.max_model_facts),
            format!(
                "budget.adaptation.max_expansions_per_model={}",
                self.max_expansions_per_model
            ),
            format!(
                "budget.adaptation.max_targets_per_source={}",
                self.max_targets_per_source
            ),
            format!(
                "budget.adaptation.max_model_derived_edges={}",
                self.max_model_derived_edges
            ),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_strictly_positive() {
        let budget = AdaptationModelBudget::default();
        assert!(budget.max_model_files > 0);
        assert!(budget.max_model_facts > 0);
        assert!(budget.max_expansions_per_model > 0);
        assert!(budget.max_targets_per_source > 0);
        assert!(budget.max_model_derived_edges > 0);
    }

    #[test]
    fn digest_parts_include_every_budget_knob() {
        let budget = AdaptationModelBudget {
            max_model_files: 1,
            max_model_facts: 2,
            max_expansions_per_model: 3,
            max_targets_per_source: 4,
            max_model_derived_edges: 5,
        };
        assert_eq!(
            budget.digest_parts(),
            vec![
                "budget.adaptation.max_model_files=1",
                "budget.adaptation.max_model_facts=2",
                "budget.adaptation.max_expansions_per_model=3",
                "budget.adaptation.max_targets_per_source=4",
                "budget.adaptation.max_model_derived_edges=5",
            ]
        );
    }
}
