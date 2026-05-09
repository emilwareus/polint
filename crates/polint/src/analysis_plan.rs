#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        Capabilities, Rule, RuleCtx, RuleMeta, RuleOptions, run_rules_with_capability_support,
    };
    use crate::diagnostics::{Severity, TextRange as DiagnosticRange};
    use crate::rule_error::RuleResult;
    use std::collections::{BTreeMap, BTreeSet};
    use std::sync::Arc;

    struct PlanRule {
        id: &'static str,
        description: &'static str,
        severity: Severity,
        capabilities: Capabilities,
    }

    impl Rule for PlanRule {
        fn meta(&self) -> RuleMeta {
            RuleMeta {
                id: self.id.to_string(),
                description: self.description.to_string(),
                severity: self.severity,
            }
        }

        fn capabilities(&self) -> Capabilities {
            self.capabilities
        }

        fn run(&self, _ctx: &mut RuleCtx<'_>) -> RuleResult {
            Ok(())
        }
    }

    fn rule(
        id: &'static str,
        description: &'static str,
        severity: Severity,
        capabilities: Capabilities,
    ) -> Arc<dyn Rule> {
        Arc::new(PlanRule {
            id,
            description,
            severity,
            capabilities,
        })
    }

    #[test]
    fn analysis_plan_schema_is_versioned() {
        assert_eq!(ANALYSIS_PLAN_SCHEMA, "analysis-plan-v1");
    }

    #[test]
    fn analysis_plan_merges_enabled_rule_capabilities_deterministically() {
        let first_rules = vec![
            rule(
                "local/zeta",
                "Zeta rule",
                Severity::Warn,
                Capabilities::new().imports().cfg(),
            ),
            rule(
                "local/alpha",
                "Alpha rule",
                Severity::Error,
                Capabilities::new().syntax().imports(),
            ),
        ];
        let second_rules = vec![Arc::clone(&first_rules[1]), Arc::clone(&first_rules[0])];
        let enabled = BTreeSet::from(["local/*".to_string()]);
        let mut options = BTreeMap::new();
        options.insert(
            "local/alpha".to_string(),
            RuleOptions {
                max: Some(3),
                ..RuleOptions::default()
            },
        );

        let first = AnalysisPlan::from_rules(&first_rules, Some(&enabled), &options);
        let second = AnalysisPlan::from_rules(&second_rules, Some(&enabled), &options);

        assert_eq!(first.digest(), second.digest());
        assert_eq!(
            first
                .rules()
                .iter()
                .map(|rule| rule.id.as_str())
                .collect::<Vec<_>>(),
            vec!["local/alpha", "local/zeta"]
        );
        assert_eq!(
            first
                .capabilities()
                .iter()
                .map(|capability| capability.capability.as_str())
                .collect::<Vec<_>>(),
            vec!["cfg", "imports", "syntax"]
        );
    }

    #[test]
    fn analysis_plan_uses_run_rules_enabled_filter_semantics() {
        let rules = vec![
            rule(
                "local/selected",
                "Selected rule",
                Severity::Warn,
                Capabilities::new().imports(),
            ),
            rule(
                "other/skipped",
                "Skipped rule",
                Severity::Warn,
                Capabilities::new().cfg(),
            ),
        ];
        let enabled = BTreeSet::from(["local/*".to_string()]);

        let plan = AnalysisPlan::from_rules(&rules, Some(&enabled), &BTreeMap::new());

        assert_eq!(plan.rules().len(), 1);
        assert_eq!(plan.rules()[0].id, "local/selected");
        assert!(plan.diagnostics().is_empty());
    }

    #[test]
    fn analysis_plan_reports_reserved_capabilities_as_diagnostics() {
        let rules = vec![rule(
            "local/needs-metrics",
            "Needs metrics",
            Severity::Warn,
            Capabilities::new().test_suite_metrics().cfg(),
        )];

        let plan = AnalysisPlan::from_rules(&rules, None, &BTreeMap::new());
        let diagnostics = plan.diagnostics();

        assert_eq!(
            plan.support_view().status_for("test_suite_metrics"),
            Some(crate::core::CapabilitySupportStatus::Unsupported)
        );
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic.rule_id == "polint/capability"
                && diagnostic.file == "<workspace>"
                && diagnostic.range == DiagnosticRange::point(1, 1)
                && diagnostic
                    .message
                    .contains("Rule `local/needs-metrics` requested unsupported capability `cfg`.")
                && diagnostic
                    .help
                    .as_deref()
                    .is_some_and(|help| help.contains("docs/roadmap/00_ROADMAP.md"))
                && diagnostic
                    .evidence
                    .iter()
                    .any(|evidence| evidence.label == "capability" && evidence.value == "cfg")
        }));
        assert!(plan.support_view().entries().iter().any(|entry| {
            entry.capability == "test_suite_metrics"
                && entry
                    .hint
                    .as_deref()
                    .is_some_and(|hint| hint.contains("Use go_tests for current Go test evidence"))
        }));
    }

    #[test]
    fn analysis_plan_support_view_is_passed_to_rules() {
        let db = crate::core::AnalysisDb::new();
        let rules = vec![rule(
            "local/reader",
            "Reader",
            Severity::Warn,
            Capabilities::new().imports(),
        )];
        let plan = AnalysisPlan::from_rules(&rules, None, &BTreeMap::new());

        let diagnostics = run_rules_with_capability_support(
            &db,
            &rules,
            &BTreeMap::new(),
            None,
            false,
            plan.support_view(),
        );

        assert!(diagnostics.is_empty());
    }
}
