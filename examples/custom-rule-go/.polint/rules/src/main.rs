mod require_error_branch_tests;

use std::process::ExitCode;

fn main() -> ExitCode {
    polint::runner::run_cli(vec![
        require_error_branch_tests::require_error_branch_tests(),
    ])
}
