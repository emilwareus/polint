mod require_error_branch_tests;
use require_error_branch_tests::RequireErrorBranchTests;

use std::process::ExitCode;
use std::sync::Arc;

fn main() -> ExitCode {
    polint_runner::run_cli(vec![Arc::new(RequireErrorBranchTests)])
}
