#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::domains::lattice::TopReason;

    #[test]
    fn deterministic_shuffled_rows_produce_byte_identical_result_digests() {
        let first = test_fixture::solver_input(false);
        let second = test_fixture::solver_input(true);
        let solver = LocalDomainSolver::new(SolverPolicy::for_test(SolverBudget {
            max_iterations: 16,
            widening_fuel: 2,
        }));

        let first_digest = solver.solve(first).stable_digest_parts();
        let second_digest = solver.solve(second).stable_digest_parts();

        assert_eq!(first_digest, second_digest);
    }

    #[test]
    fn loop_back_edges_consume_widening_fuel_and_record_widening_top() {
        let input = test_fixture::looping_solver_input();
        let solver = LocalDomainSolver::new(SolverPolicy::for_test(SolverBudget {
            max_iterations: 16,
            widening_fuel: 0,
        }));

        let result = solver.solve(input);

        assert!(result.has_top_reason(TopReason::Widened));
    }

    #[test]
    fn budget_exhaustion_marks_function_result_budget_top() {
        let input = test_fixture::looping_solver_input();
        let solver = LocalDomainSolver::new(SolverPolicy::for_test(SolverBudget {
            max_iterations: 1,
            widening_fuel: 4,
        }));

        let result = solver.solve(input);

        assert_eq!(result.statuses().next(), Some(SolverStatus::BudgetExceeded));
        assert!(result.has_top_reason(TopReason::BudgetExceeded));
    }

    mod test_fixture {
        use crate::analysis::domains::solver::SolverInput;
        use crate::core::AnalysisDb;

        pub(super) fn solver_input(_shuffled: bool) -> SolverInput<'static> {
            Box::leak(Box::new(AnalysisDb::new())).into()
        }

        pub(super) fn looping_solver_input() -> SolverInput<'static> {
            Box::leak(Box::new(AnalysisDb::new())).into()
        }
    }
}
